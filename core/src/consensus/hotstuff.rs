use std::collections::HashSet;

use libp2p::gossipsub::IdentTopic;

use crate::network::state::SwarmMessageType;
use crate::CONNECTED_PEERS;
use crate::{
    pb::{game::GameState, query::StartRequest},
    App, PEERS,
};

use super::types::{Block, BlockBuilder, QuorumCertificate};

impl App {
    pub async fn get_current_leader(&self) -> String {
        CONNECTED_PEERS
            .read()
            .await
            .get(self.view_n.load(std::sync::atomic::Ordering::Relaxed) % PEERS as usize)
            .unwrap()
            .clone()
    }

    pub async fn commit_block(&self, block: Block) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref qc) = block.qc {
            self.is_valid_qc(qc).await?;

            let real_hash = BlockBuilder::default()
                .with_previous_block_hash(block.previous_block_hash)
                .with_tx(block.tx.clone())
                .with_view_n(block.view_n)
                .build()
                .hash;

            if real_hash != block.hash || qc.block_hash != block.hash {
                return Err("invalid block".into());
            }

            self.db
                .write()
                .await
                .get_mut(&format!(
                    "{}:{}",
                    block.tx.white_player, block.tx.black_player
                ))
                .unwrap()
                .apply_move(block.tx.action[0].clone(), block.tx.action[1].clone())?;
            self.chain.write().await.commit_block(block);
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

        if let Some(block) = self.chain.read().await.last() {
            let db_locked = self.db.read().await;
            let game = db_locked.get(&format!(
                "{}:{}",
                proposal.tx.white_player, proposal.tx.black_player
            ));

            if game.is_none() {
                return Err("no such game".into());
            }

            println!(
                "Approval for {}:{} - {:?}",
                proposal.tx.white_player,
                proposal.tx.black_player,
                game.unwrap()
                    .validate_move(&proposal.tx.action[0], &proposal.tx.action[1])
            );

            game.unwrap()
                .validate_move(&proposal.tx.action[0], &proposal.tx.action[1])?;

            return match block.hash == proposal.previous_block_hash {
                true => Ok(()),
                false => Err("invalid block".into()),
            };
        }

        Err("some funky shit happened".into())
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
}
