use super::types::{Block, BlockBuilder, QuorumCertificate};
use crate::errors::AppError;
use crate::network::utils::SwarmMessageType;
use crate::pb::game::Color;
use crate::pb::query::Transaction;
use crate::{
    pb::{game::GameState, query::StartRequest},
    App, PEERS,
};
use crate::{CLOCK, CONNECTED_PEERS, VIEW_N_ROT_INTERVAL};
use alloy_primitives::{keccak256, B256};
use chrono::{TimeZone, Utc};
use libp2p::gossipsub::IdentTopic;
use libsecp256k1::{verify, Message, PublicKey, Signature};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use tracing::info;

impl App {
    pub async fn get_current_leader(&self) -> Result<String, AppError> {
        match CONNECTED_PEERS
            .read()
            .await
            .get(self.view_n.load(std::sync::atomic::Ordering::Relaxed) % PEERS as usize)
        {
            Some(peer) => Ok(peer.clone()),
            None => Err(AppError::NoLeaderError),
        }
    }

    pub async fn commit_block(&self, block: Block) -> Result<(), AppError> {
        if let Some(ref qc) = block.qc {
            self.is_valid_qc(qc).await?;

            let version = self.db.read().await.clone();

            if let Some(g) = self.db.write().await.get_mut(&format!(
                "{}:{}",
                block.tx.white_player, block.tx.black_player
            )) {
                let real_block = BlockBuilder::default()
                    .with_previous_block_hash(block.previous_block_hash)
                    .with_history(g.history.clone().unwrap())
                    .with_tx(block.tx.clone())
                    .with_view_n(block.view_n)
                    .build();

                if real_block.hash != block.hash || qc.block_hash != block.hash {
                    return Err(AppError::BlockValidationError("invalid block".into()));
                }

                if let Err(e) = g.apply_move(block.tx.action[0].clone(), block.tx.action[1].clone())
                {
                    self.db.write().await.clone_from(&version);
                    return Err(AppError::InvalidTransactionError(e.to_string()));
                }
            } else {
                return Err(AppError::BlockValidationError("no such game".into()));
            }

            self.latest_block_hash.write().await.clone_from(&block.hash);
            self.latest_timestamp
                .write()
                .await
                .clone_from(&(block.timestamp as u64));
            *CLOCK.write().await = Utc.timestamp_opt(block.timestamp, 0).unwrap();

            info!("Committed block: {:?}", block);
            Ok(())
        } else {
            Err(AppError::InvalidQcError)
        }
    }

    pub async fn approve_proposal(&self, proposal: Block, source: String) -> Result<(), AppError> {
        if self.view_n.load(std::sync::atomic::Ordering::Relaxed) as u32 != proposal.view_n {
            return Err(AppError::BlockValidationError("invalid view".into()));
        }

        if source != self.get_current_leader().await? {
            return Err(AppError::BlockValidationError("incorrect leader".into()));
        }

        let latest_block_hash = self.latest_block_hash.read().await.clone();

        if latest_block_hash != proposal.previous_block_hash {
            return Err(AppError::BlockValidationError("invalid block".into()));
        }

        let real_block = BlockBuilder::default()
            .with_previous_block_hash(proposal.previous_block_hash)
            .with_tx(proposal.tx.clone())
            .with_history(
                self.db
                    .read()
                    .await
                    .get(&format!(
                        "{}:{}",
                        proposal.tx.white_player, proposal.tx.black_player
                    ))
                    .unwrap()
                    .history
                    .clone()
                    .unwrap(),
            )
            .with_view_n(proposal.view_n)
            .build();

        if real_block.hash != proposal.hash {
            return Err(AppError::BlockValidationError("invalid block".into()));
        }

        if let Err(e) = self.is_valid_tx(&proposal.tx).await {
            return Err(AppError::BlockValidationError(e.to_string()));
        }

        info!("Approve proposal: {:?}", proposal);

        if proposal.tx.game_state_hash == Some(self.calculate_game_state_hash(&proposal.tx).await?)
        {
            Ok(())
        } else {
            Err(AppError::BlockValidationError("inequal game states".into()))
        }
    }

    pub async fn is_valid_tx(&self, tx: &Transaction) -> Result<(), AppError> {
        let game = match self
            .db
            .read()
            .await
            .get(&format!("{}:{}", tx.white_player, tx.black_player))
        {
            Some(game) => game.clone(),
            None => return Err(AppError::InvalidTransactionError("no such game".into())),
        };

        game.validate_move(&tx.action[0], &tx.action[1])?;
        self.validate_signature(tx).await?;

        if tx.pub_key
            != match Color::from_i32(game.turn).expect("correct color") {
                Color::White => game.white_player,
                Color::Black => game.black_player,
            }
        {
            return Err(AppError::InvalidTransactionError("invalud turn".into()));
        }

        Ok(())
    }

    pub async fn calculate_game_state_hash(&self, tx: &Transaction) -> Result<String, AppError> {
        let game = self
            .db
            .read()
            .await
            .get(&format!("{}:{}", tx.white_player, tx.black_player))
            .unwrap()
            .to_owned();

        let serialized = serde_json::to_string(&game)
            .map_err(|e| AppError::BlockValidationError(e.to_string()))?;

        Ok(keccak256(serialized).to_string())
    }

    async fn validate_signature(&self, tx: &Transaction) -> Result<(), AppError> {
        let message = serde_json::json!({
            "whitePlayer": tx.white_player,
            "blackPlayer": tx.black_player,
            "action": [
                {"x": tx.action[0].x, "y": tx.action[0].y},
                {"x": tx.action[1].x, "y": tx.action[1].y},
            ],
        });

        let message_str = serde_json::to_string(&message)
            .map_err(|e| AppError::InvalidTransactionError(e.to_string()))?;
        let message_hash = Sha256::digest(message_str.as_bytes());
        let message = Message::parse_slice(&message_hash)
            .map_err(|e| AppError::InvalidTransactionError(e.to_string()))?;
        let signature_bytes = hex::decode(&tx.signature)
            .map_err(|e| AppError::InvalidTransactionError(e.to_string()))?;

        let signature = match Signature::parse_standard_slice(&signature_bytes) {
            Ok(sig) => sig,
            Err(e) => {
                return Err(AppError::InvalidTransactionError(e.to_string()));
            }
        };

        let public_key_bytes = hex::decode(&tx.pub_key)
            .map_err(|e| AppError::InvalidTransactionError(e.to_string()))?;
        let public_key = match PublicKey::parse_slice(&public_key_bytes, None) {
            Ok(key) => key,
            Err(e) => {
                return Err(AppError::InvalidTransactionError(e.to_string()));
            }
        };

        match verify(&message, &signature, &public_key) {
            true => Ok(()),
            false => Err(AppError::InvalidTransactionError(
                "invalid signature".into(),
            )),
        }
    }

    async fn is_valid_qc(&self, qc: &QuorumCertificate) -> Result<(), AppError> {
        if let Some(res) = self.state_votes.read().await.get(&qc.block_hash).cloned() {
            let intersection_count = res
                .intersection(&HashSet::from_iter(qc.signature.iter().cloned()))
                .count();
            if intersection_count > (2 * PEERS as usize) / 3 {
                return Ok(());
            } else {
                return Err(AppError::InvalidQcError);
            }
        } else {
            Err(AppError::InvalidQcError)
        }
    }

    pub async fn start_game_if_possible(&self, r: StartRequest) -> Result<(), AppError> {
        let game_key = format!("{}:{}", r.white_player, r.black_player);
        let mut db_locked = self.db.write().await;
        if db_locked.contains_key(&game_key) {
            Err(AppError::StartGameError("already in game".into()))
        } else {
            db_locked.insert(game_key, GameState::new(r.white_player, r.black_player));
            Ok(())
        }
    }

    pub async fn publish(&self, topic: IdentTopic, data: String) -> Result<(), AppError> {
        self.swarm_tx
            .send(SwarmMessageType::Publish(topic, data))
            .await
            .map_err(|e| AppError::SwarmError(e.to_string()))
    }

    pub async fn update_view_if_needed(&self) {
        let latest_timestamp = self.latest_timestamp.read().await.clone();
        let current_clock = Utc::now();
        let elapsed = current_clock.timestamp() as u64 - latest_timestamp;

        if elapsed >= VIEW_N_ROT_INTERVAL
            && self.latest_block_hash.read().await.clone() != B256::ZERO
        {
            self.view_n
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            *self.latest_timestamp.write().await = current_clock.timestamp() as u64;
            *CLOCK.write().await = current_clock;

            info!(
                "Updated view_n to {}",
                self.view_n.load(std::sync::atomic::Ordering::Relaxed)
            );
        }
    }

    pub async fn get_state_hash(&self) -> B256 {
        let db_locked = self.db.read().await;
        keccak256(serde_json::to_string(&*db_locked).unwrap().as_bytes())
    }
}
