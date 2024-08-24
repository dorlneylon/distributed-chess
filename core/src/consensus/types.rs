use crate::pb::query::Transaction;
use alloy_primitives::{keccak256, B256};
use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

#[derive(Debug)]
pub struct Chain {
    blocks: Vec<Block>,
}

impl Default for Chain {
    fn default() -> Self {
        Self {
            blocks: vec![Block::default()],
        }
    }
}

impl Index<usize> for Chain {
    type Output = Block;

    fn index(&self, index: usize) -> &Self::Output {
        &self.blocks[index]
    }
}

impl IndexMut<usize> for Chain {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.blocks[index]
    }
}

impl Chain {
    pub fn new() -> Self {
        let genesis = Block::default();
        Self {
            blocks: vec![genesis],
        }
    }

    pub fn commit_block(&mut self, block: Block) {
        self.blocks.push(block);
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn last(&self) -> Option<&Block> {
        self.blocks.last()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Block {
    pub view_n: u32,
    pub previous_block_hash: B256,
    pub tx: Transaction,
    pub hash: B256,
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
