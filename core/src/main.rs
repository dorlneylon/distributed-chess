mod chess;
mod consensus;
mod errors;
mod network;
use alloy_primitives::B256;
use chrono::{DateTime, Utc};
use clap::{Arg, ArgAction, Command};
use dotenv::dotenv;
use futures::StreamExt;
use libp2p::{
    core::upgrade, mplex, noise, swarm::SwarmBuilder, tcp::TokioTcpConfig, Multiaddr, PeerId,
    Transport,
};
use network::backend::NodeServicerBuilder;
use network::p2p::{create_behaviour, match_behaviour, LOCAL_KEYS};
use network::utils::SwarmMessageType;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tonic::transport::Server;
use tonic_web::GrpcWebLayer;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};
use tracing_subscriber;

const PEERS: u32 = 4;
const VIEW_N_ROT_INTERVAL: u64 = 10;
static CONNECTED_PEERS: Lazy<RwLock<Vec<String>>> = Lazy::new(|| RwLock::new(Vec::new()));
static CLOCK: Lazy<RwLock<DateTime<Utc>>> = Lazy::new(|| RwLock::new(Utc::now()));

pub mod pb {
    pub mod game {
        tonic::include_proto!("game");
    }
    pub mod query {
        tonic::include_proto!("query");
    }
}

use pb::game::GameState;
use pb::query::node_server::NodeServer;

pub struct App {
    pub swarm_tx: mpsc::Sender<SwarmMessageType>,
    pub db: RwLock<HashMap<String, GameState>>,
    pub state_votes: RwLock<HashMap<B256, HashSet<String>>>,
    pub latest_block_hash: RwLock<B256>,
    pub latest_timestamp: RwLock<u64>,
    pub view_n: AtomicUsize,
    pub local_peer_id: Option<String>,
}

impl App {
    pub fn new(swarm_tx: mpsc::Sender<SwarmMessageType>) -> App {
        App {
            swarm_tx,
            db: RwLock::new(HashMap::new()),
            state_votes: RwLock::new(HashMap::new()),
            latest_block_hash: RwLock::new(B256::default()),
            latest_timestamp: RwLock::new(Utc::now().timestamp() as u64),
            view_n: AtomicUsize::new(0),
            local_peer_id: None,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let local_peer_id = LOCAL_KEYS.public().to_peer_id();

    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&LOCAL_KEYS)
        .expect("Signing libp2p-noise static DH keypair failed.");

    let transport = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let mut swarm = Box::new(
        SwarmBuilder::new(transport, create_behaviour().await?, local_peer_id.clone())
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build(),
    );

    let matches = Command::new("Chess Network Node")
        .arg(
            Arg::new("peers")
                .short('p')
                .long("peers")
                .help("List of peers to connect to, in the format 'multiaddr peer_id'")
                .num_args(2..)
                .value_names(["MULTIADDR", "PEER_ID"])
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("port")
                .short('P')
                .long("port")
                .help("Set the gRPC server port")
                .default_value("50050")
                .action(ArgAction::Set),
        )
        .get_matches();

    if let Some(peers) = matches.get_many::<String>("peers") {
        let mut peer_iter = peers.into_iter();
        while let (Some(multiaddr), Some(peer_id_str)) = (peer_iter.next(), peer_iter.next()) {
            let multiaddr: Multiaddr = multiaddr.parse()?;
            let peer_id: PeerId = peer_id_str.parse()?;
            swarm.dial(multiaddr.clone())?;

            swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, multiaddr.clone());

            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);

            info!("Dialed with {:?}, {:?}", peer_id, multiaddr);
        }
    }

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let (swarm_tx, mut swarm_rx) = mpsc::channel::<SwarmMessageType>(100);
    let app = Box::leak(Box::new(App::new(swarm_tx)));
    app.local_peer_id = Some(local_peer_id.to_string());

    let node_servicer = NodeServicerBuilder::default().with_app(&*app).build();

    let grpc_port = matches.get_one::<String>("port").unwrap();
    let addr = format!("[::]:{}", grpc_port).parse()?;
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let _ = tokio::spawn(async move {
        Server::builder()
            .accept_http1(true)
            .layer(cors)
            .layer(GrpcWebLayer::new())
            .add_service(NodeServer::new(node_servicer))
            .serve(addr)
            .await
            .expect("gRPC server running")
    });

    let _ = tokio::spawn(async {
        loop {
            app.update_view_if_needed().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    loop {
        tokio::select! {
            Some(cmd) = swarm_rx.recv() => {
                match cmd {
                    SwarmMessageType::Publish(topic, msg) => {
                        swarm.behaviour_mut().gossipsub.publish(topic, msg)?;
                    }
                    SwarmMessageType::AddAddress(peer_id, addr) => {
                        swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                    SwarmMessageType::Bootstrap => {
                        swarm.behaviour_mut().kademlia.bootstrap()?;
                        let mut peers: Vec<_> = swarm.connected_peers().map(|e| e.to_string()).collect();
                        peers.push(local_peer_id.to_string());
                        peers.sort();
                        CONNECTED_PEERS.write().await.clone_from(&peers);
                    }
                }
            }
            event = swarm.select_next_some() => {
                if let Err(e) = match_behaviour(event, &app).await {
                    error!("{:?}", e);
                }
            }
        }
    }
}
