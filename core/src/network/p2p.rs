use crate::{
    consensus::types::{Block, BlockBuilder, Commit, QuorumCertificate},
    network::utils::SwarmMessageType,
    pb::query::{StartRequest, Transaction},
    App, MAX_TXS_PER_BLOCK, PEERS,
};
use libp2p::{
    gossipsub::{
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, GossipsubMessage, IdentTopic as Topic,
        MessageAuthenticity, ValidationMode,
    },
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    identity,
    kad::{protocol, store::MemoryStore, Kademlia, KademliaEvent},
    swarm::SwarmEvent,
    NetworkBehaviour,
};
use once_cell::sync::Lazy;
use std::time::Duration;
use std::{collections::HashSet, error::Error};

pub static LOCAL_KEYS: Lazy<identity::Keypair> = Lazy::new(identity::Keypair::generate_ed25519);
pub static PROPOSAL_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("proposal"));
pub static QUORUM_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("quorum"));
pub static DECISION_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("decision"));
pub static COMMIT_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("commit"));
pub static START_TOPIC: Lazy<Topic> = Lazy::new(|| Topic::new("start"));

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "PeerBehaviour")]
pub struct Behaviour {
    pub kademlia: Kademlia<MemoryStore>,
    pub identify: Identify,
    pub gossipsub: Gossipsub,
}

#[derive(Debug)]
pub enum PeerBehaviour {
    Gossipsub(GossipsubEvent),
    Identify(IdentifyEvent),
    Kademlia(KademliaEvent),
}

impl From<IdentifyEvent> for PeerBehaviour {
    fn from(v: IdentifyEvent) -> Self {
        Self::Identify(v)
    }
}

impl From<GossipsubEvent> for PeerBehaviour {
    fn from(v: GossipsubEvent) -> Self {
        Self::Gossipsub(v)
    }
}

impl From<KademliaEvent> for PeerBehaviour {
    fn from(v: KademliaEvent) -> Self {
        Self::Kademlia(v)
    }
}

pub async fn match_behaviour(
    event: SwarmEvent<PeerBehaviour, impl Error>,
    app: &App,
) -> Result<(), Box<dyn Error>> {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            println!(
                "Listening on {:?}, {:?}",
                address,
                app.local_peer_id.clone().unwrap()
            );
            Ok(())
        }
        SwarmEvent::Behaviour(PeerBehaviour::Identify(event)) => handle_identify(event, app).await,
        SwarmEvent::Behaviour(PeerBehaviour::Gossipsub(event)) => {
            handle_gossipsub(event, app).await
        }
        SwarmEvent::Behaviour(PeerBehaviour::Kademlia(event)) => handle_kademlia(event, app).await,
        _ => Ok(()),
    }
}

async fn handle_identify(event: IdentifyEvent, app: &App) -> Result<(), Box<dyn Error>> {
    if let IdentifyEvent::Received { peer_id, info } = event {
        println!("Received peer info: {:?}", info);

        if info
            .protocols
            .iter()
            .any(|p| p.as_bytes() == protocol::DEFAULT_PROTO_NAME)
        {
            for addr in info.listen_addrs {
                app.swarm_tx
                    .send(SwarmMessageType::AddAddress(peer_id, addr))
                    .await?;
            }
        }

        app.swarm_tx.send(SwarmMessageType::Bootstrap).await?;
    }
    Ok(())
}

async fn handle_gossipsub(event: GossipsubEvent, app: &App) -> Result<(), Box<dyn Error>> {
    if let GossipsubEvent::Message { message, .. } = event {
        let msg = String::from_utf8_lossy(&message.data);
        println!("Received message decoded: {:?}", msg);

        println!(
            "\n{:?}\n{:?}\n{:?}\n",
            app.latest_block_hash.read().await,
            app.view_n.load(std::sync::atomic::Ordering::Relaxed),
            app.db.read().await,
        );

        // TODO: maybe there are some ways to do this elegant w/o traits
        if message.topic == START_TOPIC.hash() {
            handle_start_event(message, app).await?;
        } else if message.topic == PROPOSAL_TOPIC.hash() {
            handle_proposal_event(message, app).await?;
        } else if message.topic == QUORUM_TOPIC.hash() {
            handle_quorum_event(message, app).await?;
        } else if message.topic == DECISION_TOPIC.hash() {
            handle_decision_event(message, app).await?;
        } else if message.topic == COMMIT_TOPIC.hash() {
            handle_commit_event(message, app).await?;
        }
    }

    Ok(())
}

async fn handle_start_event(message: GossipsubMessage, app: &App) -> Result<(), Box<dyn Error>> {
    let msg = String::from_utf8_lossy(&message.data);
    let req: StartRequest = serde_json::from_str(&msg)?;
    app.start_game_if_possible(req).await?;
    Ok(())
}

async fn handle_proposal_event(message: GossipsubMessage, app: &App) -> Result<(), Box<dyn Error>> {
    let msg = String::from_utf8_lossy(&message.data);
    let tx: Transaction = serde_json::from_str(&msg)?;
    println!("Pushed: {:?}", tx);
    app.local_pool
        .write()
        .await
        .insert(format!("{}:{}", tx.white_player, tx.black_player), tx);

    if app.get_current_leader().await? == app.local_peer_id.clone().unwrap() {
        broadcast_block(app).await?;
    }

    Ok(())
}

pub async fn broadcast_block(app: &App) -> Result<(), Box<dyn Error>> {
    let mut block = BlockBuilder::default()
        .with_previous_block_hash(app.latest_block_hash.read().await.clone())
        .with_view_n(app.view_n.load(std::sync::atomic::Ordering::Relaxed) as u32);

    for tx in app.local_pool.read().await.iter() {
        if block.tx_size() as u32 >= MAX_TXS_PER_BLOCK {
            break;
        }

        if app.is_valid_tx(tx.1).await.is_ok() {
            block.add_tx(tx.1);
        } else {
            app.local_pool.write().await.remove(tx.0);
        }
    }

    let block = block.build();

    app.publish(QUORUM_TOPIC.clone(), serde_json::to_string(&block)?)
        .await?;

    println!(
        "Broadcasted block: {:?} for view_n: {}",
        block,
        app.view_n.load(std::sync::atomic::Ordering::Relaxed)
    );

    app.state_votes
        .write()
        .await
        .entry(block.hash)
        .or_insert_with(HashSet::new)
        .insert(app.local_peer_id.clone().unwrap());

    Ok(())
}

async fn handle_quorum_event(message: GossipsubMessage, app: &App) -> Result<(), Box<dyn Error>> {
    let msg = String::from_utf8_lossy(&message.data);
    let block: Block = serde_json::from_str(&msg)?;
    let result = app.approve_proposal(block.clone()).await;
    println!("Approve result: {:?}", result);
    let hash = block.hash;

    let commit = Commit {
        block,
        decision: result.is_ok(),
    };

    if result.is_ok() {
        app.state_votes
            .write()
            .await
            .entry(hash)
            .or_insert_with(HashSet::new)
            .insert(app.local_peer_id.clone().unwrap());
    }

    app.publish(DECISION_TOPIC.clone(), serde_json::to_string(&commit)?)
        .await?;

    println!("Sent decision: {:?}", commit);

    result
}

async fn handle_decision_event(message: GossipsubMessage, app: &App) -> Result<(), Box<dyn Error>> {
    let msg = String::from_utf8_lossy(&message.data);
    let commit: Commit = serde_json::from_str(&msg)?;

    if let Some(source) = message.source {
        if commit.decision {
            app.state_votes
                .write()
                .await
                .entry(commit.block.hash)
                .or_insert_with(HashSet::new)
                .insert(source.to_string());
        }
    }

    if app.view_n.load(std::sync::atomic::Ordering::Relaxed) == commit.block.view_n as usize {
        let my_proposal = Commit {
            block: commit.block.clone(),
            decision: app.approve_proposal(commit.block.clone()).await.is_ok(),
        };
        if my_proposal.decision {
            app.publish(DECISION_TOPIC.clone(), serde_json::to_string(&my_proposal)?)
                .await?;
        }
    }

    if app.get_current_leader().await? == app.local_peer_id.clone().unwrap() {
        handle_commitment(commit, app).await?;
    }

    Ok(())
}

async fn handle_commitment(commit: Commit, app: &App) -> Result<(), Box<dyn Error>> {
    if app.view_n.load(std::sync::atomic::Ordering::Relaxed) == commit.block.view_n as usize
        && app
            .state_votes
            .read()
            .await
            .get(&commit.block.hash)
            .is_some_and(|v| v.len() > (2 * PEERS as usize) / 3)
    {
        let mut b = commit.block;
        let qc = QuorumCertificate::default()
            .with_block_hash(b.hash)
            .with_signature(
                app.state_votes
                    .read()
                    .await
                    .get(&b.hash)
                    .unwrap()
                    .iter()
                    .cloned()
                    .collect::<Vec<String>>(),
            );
        b.qc = Some(qc);

        app.publish(COMMIT_TOPIC.clone(), serde_json::to_string(&b)?)
            .await?;

        println!("Sent commit: {:?}", b);

        app.view_n
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        app.commit_block(b).await?;
    }

    Ok(())
}

async fn handle_commit_event(message: GossipsubMessage, app: &App) -> Result<(), Box<dyn Error>> {
    let msg = String::from_utf8_lossy(&message.data);
    let block: Block = serde_json::from_str(&msg)?;

    if app.view_n.load(std::sync::atomic::Ordering::Relaxed) == block.clone().view_n as usize
        && app.get_current_leader().await? == message.source.unwrap().to_string()
    {
        app.view_n
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        app.commit_block(block.clone()).await?;
    }

    println!(
        "Committed: {:?}, view_n: {}",
        block,
        app.view_n.load(std::sync::atomic::Ordering::Relaxed)
    );

    Ok(())
}

async fn handle_kademlia(event: KademliaEvent, app: &App) -> Result<(), Box<dyn Error>> {
    match event {
        KademliaEvent::RoutingUpdated {
            peer, addresses, ..
        } => {
            for a in addresses.iter() {
                app.swarm_tx
                    .send(SwarmMessageType::AddAddress(peer, a.clone()))
                    .await?;
            }
            let _ = app.swarm_tx.send(SwarmMessageType::Bootstrap).await;
        }
        _ => {}
    }
    Ok(())
}

pub async fn create_behaviour() -> Result<Behaviour, Box<dyn Error>> {
    let mut gossipsub = Gossipsub::new(
        MessageAuthenticity::Signed(LOCAL_KEYS.clone()),
        GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Strict)
            .build()?,
    )?;

    for topic in [
        &PROPOSAL_TOPIC,
        &QUORUM_TOPIC,
        &COMMIT_TOPIC,
        &DECISION_TOPIC,
        &START_TOPIC,
    ] {
        gossipsub.subscribe(topic)?;
    }

    let kademlia = Kademlia::new(
        LOCAL_KEYS.public().to_peer_id(),
        MemoryStore::new(LOCAL_KEYS.public().to_peer_id()),
    );

    let identify = Identify::new(IdentifyConfig::new(
        "ipfs/1.0.0".to_string(),
        LOCAL_KEYS.public(),
    ));

    Ok(Behaviour {
        gossipsub,
        kademlia,
        identify,
    })
}
