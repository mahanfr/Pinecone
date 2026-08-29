use std::time::{SystemTime, UNIX_EPOCH};

use log::warn;

use crate::{transactions::{Transaction, transactions_root}, types::{BlockPos, PineAddr, PineHash}};

const BLOCK_DOMAIN: &[u8] = b"PINECONE_BLOCK_V1";
const BLOCK_VERSION: u8 = 1;

fn current_timestamp() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .expect("System clock is before Unix epoch")
        .as_nanos()
}

#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn new(
        position: BlockPos,
        previous_hash: PineHash,
        proposer: PineAddr,
        state_root: PineHash,
        transactions: Vec<Transaction>
    ) -> Self {
        let header = BlockHeader {
            version: BLOCK_VERSION,
            position,
            previous_hash,
            timestamp: current_timestamp(),
            proposer,
            transactions_root: transactions_root(&transactions),
            state_root,
        };

        Self { header, transactions }
    }

    pub fn validate_basic(&self, parent: &BlockHeader) -> bool {
        if self.header.version != 1 {
            warn!("header is on unsupported version");
            return false;
        }
        if self.header.position.height != parent.position.height + 1 {
            warn!("parent is at the same height or higher than the child");
            return false;
        }
        if self.header.previous_hash != parent.hash() {
            warn!("parent hash dose not match the blocks pervious hash");
            return false;
        }
        let expexted_root = transactions_root(&self.transactions);
        if self.header.transactions_root != expexted_root {
            warn!("the block Tx root dose not match the expected root");
            return false;
        }
        true
    }

    pub fn hash(&self) -> PineHash {
        self.header.hash()
    }
}

#[derive(Debug, Clone)]
// TODO: Add gas_limit/gad_used and base_fee_per_gas to the block
pub struct BlockHeader {
    pub version: u8,
    pub position: BlockPos,
    pub previous_hash: PineHash,
    pub timestamp: u128,
    pub proposer: PineAddr,
    pub transactions_root: PineHash,
    pub state_root: PineHash,
    // pub previous_rando // useed for smart contract random opcode
}

impl BlockHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.push(self.version);
        bytes.extend_from_slice(&self.position.to_bytes());
        bytes.extend_from_slice(&self.previous_hash);
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.proposer);
        bytes.extend_from_slice(&self.transactions_root);
        bytes.extend_from_slice(&self.state_root);
        bytes
    }

    pub fn hash(&self) -> PineHash {
        let mut data = Vec::new();
        data.extend_from_slice(BLOCK_DOMAIN);
        data.extend_from_slice(&self.encode());
        blake3::hash(&data).as_bytes().to_owned()
    }
}

pub fn genesis() -> Block {
    let transactions = Vec::new();

    Block::new(BlockPos::new(0, 0), [0u8;32], [0u8;32], [0u8;32], transactions)
}
