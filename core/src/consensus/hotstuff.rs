use crate::network::utils::SwarmMessageType;
use crate::pb::query::Transaction;
use crate::{
    pb::{game::GameState, query::StartRequest},
    App, PEERS,
};
use crate::{CLOCK, CONNECTED_PEERS, VIEW_N_ROT_INTERVAL};
use alloy_primitives::{keccak256, B256};
use chrono::{TimeZone, Utc};
use libp2p::gossipsub::IdentTopic;
use std::collections::HashSet;

use super::types::{Block, BlockBuilder, QuorumCertificate};

impl App {
    pub async fn get_current_leader(&self) -> Result<String, Box<dyn std::error::Error>> {
        match CONNECTED_PEERS
            .read()
            .await
            .get(self.view_n.load(std::sync::atomic::Ordering::Relaxed) % PEERS as usize)
        {
            Some(peer) => Ok(peer.clone()),
            None => Err("no leader".into()),
        }
    }

    pub async fn commit_block(&self, block: Block) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref qc) = block.qc {
            self.is_valid_qc(qc).await?;

            let real_block = BlockBuilder::default()
                .with_previous_block_hash(block.previous_block_hash)
                .with_tx(block.tx.clone())
                .with_view_n(block.view_n)
                .build();

            if real_block.hash != block.hash || qc.block_hash != block.hash {
                return Err("invalid block".into());
            }

            let version = self.db.read().await.clone();

            if let Err(e) = self
                .db
                .write()
                .await
                .get_mut(&format!(
                    "{}:{}",
                    block.tx.white_player, block.tx.black_player
                ))
                .unwrap()
                .apply_move(block.tx.action[0].clone(), block.tx.action[1].clone())
            {
                self.db.write().await.clone_from(&version);
                return Err(e.into());
            }

            self.latest_block_hash.write().await.clone_from(&block.hash);
            self.latest_block_timestamp
                .write()
                .await
                .clone_from(&(block.timestamp as u64));
            *CLOCK.write().await = Utc.timestamp_opt(block.timestamp, 0).unwrap();

            Ok(())
        } else {
            Err("no qc".into())
        }
    }

    pub async fn approve_proposal(
        &self,
        proposal: Block,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.view_n.load(std::sync::atomic::Ordering::Relaxed) as u32 != proposal.view_n {
            return Err("invalid view".into());
        }

        let latest_block_hash = self.latest_block_hash.read().await.clone();

        if latest_block_hash != proposal.previous_block_hash {
            return Err("invalid block".into());
        }

        let real_block = BlockBuilder::default()
            .with_previous_block_hash(proposal.previous_block_hash)
            .with_tx(proposal.tx.clone())
            .with_view_n(proposal.view_n)
            .build();

        if real_block.hash != proposal.hash {
            return Err("invalid block".into());
        }

        self.is_valid_tx(&proposal.tx).await
    }

    pub async fn is_valid_tx(&self, tx: &Transaction) -> Result<(), Box<dyn std::error::Error>> {
        let game = match self
            .db
            .read()
            .await
            .get(&format!("{}:{}", tx.white_player, tx.black_player))
        {
            Some(game) => game.clone(),
            None => return Err("no such game".into()),
        };

        game.validate_move(&tx.action[0], &tx.action[1])?;

        Ok(())
    }

    async fn is_valid_qc(&self, qc: &QuorumCertificate) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(res) = self.state_votes.read().await.get(&qc.block_hash).cloned() {
            let intersection_count = res
                .intersection(&HashSet::from_iter(qc.signature.iter().cloned()))
                .count();
            if intersection_count > (2 * PEERS as usize) / 3 {
                return Ok(());
            } else {
                return Err("invalid qc".into());
            }
        } else {
            Err("no such block approved".into())
        }
    }

    pub async fn start_game_if_possible(
        &self,
        r: StartRequest,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let game_key = format!("{}:{}", r.white_player, r.black_player);
        let mut db_locked = self.db.write().await;
        if db_locked.contains_key(&game_key) {
            Err("already in game".into())
        } else {
            db_locked.insert(game_key, GameState::new(r.white_player, r.black_player));
            Ok(())
        }
    }

    pub async fn publish(
        &self,
        topic: IdentTopic,
        data: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.swarm_tx
            .send(SwarmMessageType::Publish(topic, data))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    pub async fn update_view_if_needed(&self) {
        let latest_block_timestamp = self.latest_block_timestamp.read().await.clone();
        let current_clock = Utc::now();
        let elapsed = current_clock.timestamp() as u64 - latest_block_timestamp;

        if elapsed >= VIEW_N_ROT_INTERVAL
            && self.latest_block_hash.read().await.clone() != B256::ZERO
        {
            self.view_n
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            *self.latest_block_timestamp.write().await = current_clock.timestamp() as u64;
            *CLOCK.write().await = current_clock;

            println!(
                "Updated view_n to: {}",
                self.view_n.load(std::sync::atomic::Ordering::Relaxed)
            );
        }
    }

    pub async fn get_state_hash(&self) -> B256 {
        let db_locked = self.db.read().await;
        keccak256(serde_json::to_string(&*db_locked).unwrap().as_bytes())
    }
}
