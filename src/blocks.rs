use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Address, Hash, hashing::transactions_root, transactions::{Transaction}};

// [ ] Formal canonical serialization specification
// [ ] Binary decoder
// [ ] Malformed-input rejection
// [ ] Version negotiation
// [ ] Timestamp validation
// [ ] Maximum transaction/block size
// [ ] Merkle proof implementation
// [ ] State commitment implementation
// [ ] Genesis configuration
// [ ] BLS proposer authentication
// [ ] Validator-set commitment
// [ ] BFT rounds
// [ ] Vote validation
// [ ] Quorum certificates
// [ ] Fork-choice/finality rules
// [ ] Consensus failure handling
// [ ] Fuzz testing
// [ ] Cross-implementation test vectors

const BLOCK_DOMAIN: &[u8] = b"PINECONE_BLOCK_V1";

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
        height: u64,
        round: u64,
        previous_hash: Hash,
        proposer: Address,
        state_root: Hash,
        transactions: Vec<Transaction>
    ) -> Self {
        let transactions_root = transactions_root(&transactions);

        let header = BlockHeader {
            version: 1,
            height,
            round,
            previous_hash,
            timestamp: current_timestamp(),
            proposer,
            transactions_root,
            state_root
        };

        Self {
            header,
            transactions
        }
    }

    pub fn validate_basic(&self, parent: &BlockHeader) -> bool {
        if self.header.version != 1 {
            return false;
        }
        if self.header.height != parent.height + 1 {
            return false;
        }
        if self.header.previous_hash != parent.hash() {
            return false;
        }
        let expexted_root = transactions_root(&self.transactions);
        if self.header.transactions_root != expexted_root {
            return false;
        }
        true
    }

}

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub version: u8,
    pub height: u64,
    pub round : u64,
    pub previous_hash: Hash,
    pub timestamp: u128,
    pub proposer: Address,
    pub transactions_root: Hash,
    pub state_root: Hash
}
impl BlockHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.push(self.version);

        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&self.round.to_le_bytes());

        bytes.extend_from_slice(&self.previous_hash);
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.proposer);
        bytes.extend_from_slice(&self.transactions_root);
        bytes.extend_from_slice(&self.state_root);
        bytes
    }

    pub fn hash(&self) -> Hash {
        let encoded = self.encode();

        let mut data = Vec::new();
        data.extend_from_slice(BLOCK_DOMAIN);
        data.extend_from_slice(&encoded);

        blake3::hash(&data).as_bytes().to_owned()
    }
}

pub fn genesis() -> Block {
    let transactions = Vec::new();

    Block {
        header: BlockHeader { 
            version: 1,
            height: 1,
            round: 0,
            previous_hash: [0u8;32],
            timestamp: current_timestamp(),
            proposer: [0u8;32],
            transactions_root: transactions_root(&transactions),
            state_root: [0u8;32],
        },
        transactions,
    }
}
