use super::p2p::{broadcast_block, PROPOSAL_TOPIC, START_TOPIC};
use crate::{
    pb::{
        game::GameState,
        query::{
            node_server::Node, IsInGameRequest, IsInGameResponse, StartRequest, StartResponse,
            StateRequest, StateResponse, Transaction, TransactionResponse,
        },
    },
    App,
};
use tonic::{Request, Response, Status};

pub struct NodeServicer {
    app: &'static App,
}

#[tonic::async_trait]
impl Node for NodeServicer {
    async fn start(
        &self,
        request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        let r = request.into_inner();

        self.app
            .start_game_if_possible(r.clone())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let spread = serde_json::to_string(&r).map_err(|e| Status::internal(e.to_string()))?;

        self.app
            .publish(START_TOPIC.to_owned(), spread)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(StartResponse {
            state: Some(GameState::new(r.white_player, r.black_player)),
        }))
    }

    async fn state(
        &self,
        request: Request<StateRequest>,
    ) -> Result<Response<StateResponse>, Status> {
        let r = request.into_inner();

        if let Some(state) = self
            .app
            .db
            .read()
            .await
            .get(&format!("{}:{}", r.white_player, r.black_player))
        {
            return Ok(Response::new(StateResponse {
                state: Some(state.clone()),
            }));
        }

        return Ok(Response::new(StateResponse { state: None }));
    }

    async fn transact(
        &self,
        request: Request<Transaction>,
    ) -> Result<Response<TransactionResponse>, Status> {
        let r = request.into_inner();

        if self.app.get_current_leader().await != self.app.local_peer_id.clone().unwrap() {
            let serialized =
                serde_json::to_string(&r).map_err(|e| Status::internal(e.to_string()))?;
            self.app
                .publish(PROPOSAL_TOPIC.clone(), serialized)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        } else {
            broadcast_block(r, &self.app)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }

        Ok(Response::new(TransactionResponse { ok: true }))
    }

    async fn is_in_game(
        &self,
        request: Request<IsInGameRequest>,
    ) -> Result<Response<IsInGameResponse>, Status> {
        let r = request.into_inner();

        for key in self.app.db.read().await.keys() {
            if key.split(":").any(|p| p == r.player) {
                return Ok(Response::new(IsInGameResponse {
                    state: Some(self.app.db.read().await.get(key).unwrap().clone()),
                }));
            }
        }

        return Ok(Response::new(IsInGameResponse { state: None }));
    }
}

#[derive(Default)]
pub struct NodeServicerBuilder {
    app: Option<&'static App>,
}

impl NodeServicerBuilder {
    pub fn with_app(self, app: &'static App) -> Self {
        Self {
            app: Some(app),
            ..self
        }
    }

    pub fn build(self) -> NodeServicer {
        NodeServicer {
            app: self.app.expect("App"),
        }
    }
}
