use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use log::{error, warn};

use crate::{
    transactions::{Transaction, transactions_root},
    types::{BlockPos, IonicHash, IonicPK, IonicTXSignature},
    utils::{ToBytes, current_timestamp},
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
        let hash = self.header.hash();
        let signature = sk.sign(&hash.to_bytes());
        self.signature = signature.to_bytes();
    }

    fn validate_signature(&self) -> bool {
        if self.signature.is_empty() {
            error!("Empty Signature: The Block has not been signed");
            return false;
        }
        let pk: VerifyingKey = match &self.header.proposer.try_into() {
            Ok(key) => *key,
            Err(_) => {
                error!("preposer has not a valid public key");
                return false;
            }
        };
        let hash = self.header.hash();
        pk.verify(&hash.as_ref(), &Signature::from_bytes(&self.signature))
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
        data.extend_from_slice(&self.header.hash().as_ref());
        data.extend_from_slice(&self.signature);
        blake3::hash(&data).into()
    }

    pub fn next_base_fee(&self) -> u128 {
        let last_block_header = self.header;
        let current_base_fee = last_block_header.base_fee;
        let gas_target = last_block_header.gas_limit / 2;
        if gas_target == 0 {
            return current_base_fee;
        }
        // base_fee(N) * (gas_used(N) - gas_target(N)) / (gas_target(N) * 8)
        let inflaition =
            current_base_fee * ((last_block_header.gas_used - gas_target) / gas_target * 8) as u128;
        let base_fee = current_base_fee + inflaition;
        if base_fee < 1 {
            return 1;
        }
        base_fee
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
        bytes.extend_from_slice(&self.previous_hash.as_ref());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.proposer.as_ref());
        bytes.extend_from_slice(&self.transactions_root.as_ref());
        bytes.extend_from_slice(&self.state_root.as_ref());
        bytes.extend_from_slice(&self.gas_limit.to_le_bytes());
        bytes.extend_from_slice(&self.gas_used.to_le_bytes());
        bytes.extend_from_slice(&self.base_fee.to_le_bytes());
        bytes
    }

    pub fn hash(&self) -> IonicHash {
        let mut data = Vec::new();
        data.extend_from_slice(BLOCK_HEADER_DOMAIN);
        data.extend_from_slice(&self.encode());
        blake3::hash(&data).into()
    }
}
