use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use log::{error, warn};

use crate::{
    transactions::{Transaction, transactions_root},
    types::{BlockPos, IonicHash, IonicPK, IonicTXSignature},
    utils::current_timestamp,
};

const BLOCK_DOMAIN: &[u8] = b"IONIC_BLOCK";
const BLOCK_HEADER_DOMAIN: &[u8] = b"IONIC_BLOCK_HEADER";
const BLOCK_VERSION: u8 = 1;

#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub signature: IonicTXSignature,
}

impl Block {
    pub fn new_unsigned(
        position: BlockPos,
        chain_id: u64,
        previous_hash: IonicHash,
        proposer: IonicPK,
        state_root: IonicHash,
        transactions: Vec<Transaction>,
    ) -> Self {
        let header = BlockHeader {
            version: BLOCK_VERSION,
            chain_id,
            position,
            previous_hash,
            timestamp: current_timestamp(),
            proposer,
            transactions_root: transactions_root(&transactions),
            state_root,
            gas_limit: 0,
            gas_used: 0,
            base_fee: 0,
        };

        Self {
            header,
            transactions,
            signature: [0u8; 64],
        }
    }

    pub fn new_signed(
        sk: &SigningKey,
        position: BlockPos,
        chain_id: u64,
        previous_hash: IonicHash,
        proposer: IonicPK,
        state_root: IonicHash,
        transactions: Vec<Transaction>,
    ) -> Self {
        let mut block = Self::new_unsigned(
            position,
            chain_id,
            previous_hash,
            proposer,
            state_root,
            transactions,
        );
        block.sign(sk);
        block
    }

    pub fn sign(&mut self, sk: &SigningKey) {
        let bytes = self.header.hash();
        let signature = sk.sign(&bytes);
        self.signature = signature.to_bytes();
    }

    fn validate_signature(&self) -> bool {
        if self.signature.is_empty() {
            error!("Empty Signature: The Block has not been signed");
            return false;
        }
        let pk = match VerifyingKey::from_bytes(&self.header.proposer) {
            Ok(key) => key,
            Err(_) => {
                error!("preposer has not a valid public key");
                return false;
            }
        };
        let hash = self.header.hash();
        pk.verify(&hash, &Signature::from_bytes(&self.signature))
            .is_ok()
    }

    pub fn validate_basic(&self, parent: &BlockHeader) -> bool {
        if !self.validate_signature() {
            warn!("signature is invalid");
            return false;
        }
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

    pub fn hash(&self) -> IonicHash {
        assert_ne!(self.signature, [0u8; 64]);
        let mut data = Vec::new();
        data.extend_from_slice(BLOCK_DOMAIN);
        data.extend_from_slice(&self.header.hash());
        data.extend_from_slice(&self.signature);
        blake3::hash(&data).as_bytes().to_owned()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BlockHeader {
    pub version: u8,
    pub chain_id: u64,
    pub position: BlockPos,
    pub previous_hash: IonicHash,
    pub timestamp: u64,
    pub proposer: IonicPK,
    pub transactions_root: IonicHash,
    pub state_root: IonicHash,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee: u128,
    // pub previous_rando // useed for smart contract random opcode
}

impl BlockHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.push(self.version);
        bytes.extend_from_slice(&self.chain_id.to_le_bytes());
        bytes.extend_from_slice(&self.position.to_bytes());
        bytes.extend_from_slice(&self.previous_hash);
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.proposer);
        bytes.extend_from_slice(&self.transactions_root);
        bytes.extend_from_slice(&self.state_root);
        bytes.extend_from_slice(&self.gas_limit.to_le_bytes());
        bytes.extend_from_slice(&self.gas_used.to_le_bytes());
        bytes.extend_from_slice(&self.base_fee.to_le_bytes());
        bytes
    }

    pub fn hash(&self) -> IonicHash {
        let mut data = Vec::new();
        data.extend_from_slice(BLOCK_HEADER_DOMAIN);
        data.extend_from_slice(&self.encode());
        blake3::hash(&data).as_bytes().to_owned()
    }
}
