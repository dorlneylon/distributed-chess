use crate::pb::query::Transaction;
use alloy_primitives::{keccak256, B256};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Block {
    pub view_n: u32,
    pub previous_block_hash: B256,
    pub tx: Transaction,
    pub hash: B256,
    pub timestamp: i64,
    pub qc: Option<QuorumCertificate>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct BlockBuilder {
    view_n: u32,
    previous_block_hash: B256,
    tx: Transaction,
}

impl BlockBuilder {
    pub fn with_view_n(self, view_n: u32) -> Self {
        Self { view_n, ..self }
    }

    pub fn with_previous_block_hash(self, previous_block_hash: B256) -> Self {
        Self {
            previous_block_hash,
            ..self
        }
    }

    pub fn with_tx(self, tx: Transaction) -> Self {
        Self { tx, ..self }
    }

    pub fn build(self) -> Block {
        Block {
            view_n: self.view_n,
            previous_block_hash: self.previous_block_hash,
            tx: self.tx.clone(),
            timestamp: Utc::now().timestamp(),
            hash: keccak256(&serde_json::to_string(&self).unwrap()),
            qc: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct QuorumCertificate {
    pub block_hash: B256,
    pub signature: Vec<String>,
}

impl QuorumCertificate {
    pub fn with_block_hash(self, block_hash: B256) -> Self {
        Self { block_hash, ..self }
    }

    pub fn with_signature(self, signature: Vec<String>) -> Self {
        Self { signature, ..self }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Commit {
    pub decision: bool,
    pub block: Block,
}
