use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use ethnum::{AsU256, u256};

use crate::{
    blockchian::BlockchainError, transactions::{Transaction, transactions_root}, types::{BlockPos, IonicHash, IonicPK, IonicTXSignature}, utils::{ToBytes, current_timestamp}
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
        index: u64,
        position: BlockPos,
        chain_id: u64,
        previous_hash: IonicHash,
        proposer: IonicPK,
        state_root: IonicHash,
        transactions: Vec<Transaction>,
    ) -> Self {
        let header = BlockHeader {
            version: BLOCK_VERSION,
            index,
            chain_id,
            position,
            previous_hash,
            timestamp: current_timestamp(),
            proposer,
            transactions_root: transactions_root(&transactions),
            state_root,
            gas_limit: 0,
            gas_used: 0,
            base_fee: u256::ZERO,
        };

        Self {
            header,
            transactions,
            signature: [0u8; 64],
        }
    }

    pub fn sign(&mut self, sk: &SigningKey) -> &mut Self {
        let hash = self.header.hash();
        let signature = sk.sign(&hash.to_bytes());
        self.signature = signature.to_bytes();
        self
    }

    pub fn validate_signature(&self) -> Result<(), BlockchainError> {
        if self.signature.is_empty() {
            return Err(BlockchainError::InvalidSignature);
        }
        let pk: VerifyingKey = match &self.header.proposer.try_into() {
            Ok(key) => *key,
            Err(_) => {
                return Err(BlockchainError::InvalidPK);
            }
        };
        let hash = self.header.hash();
        pk.verify(hash.as_ref(), &Signature::from_bytes(&self.signature))
            .map_err(|_| BlockchainError::InvalidSignature)
    }

    pub fn validate_basic(&self, parent: &BlockHeader) -> Result<(), BlockchainError> {
        self.validate_signature()?;
        if self.header.version != 1 {
            return Err(BlockchainError::UnsupportedVersion);
        }
        if self.header.position.height != parent.position.height + 1 {
            return Err(BlockchainError::InvalidParentHeight);
        }
        if self.header.previous_hash != parent.hash() {
            return Err(BlockchainError::HashMissmatch);
        }
        let expexted_root = transactions_root(&self.transactions);
        if self.header.transactions_root != expexted_root {
            return Err(BlockchainError::InvaldRootHash);
        }
        Ok(())
    }

    pub fn hash(&self) -> IonicHash {
        let mut data = Vec::new();
        data.extend_from_slice(BLOCK_DOMAIN);
        data.extend_from_slice(self.header.hash().as_ref());
        data.extend_from_slice(&self.signature);
        blake3::hash(&data).into()
    }

    pub fn next_base_fee(&self) -> u256 {
        self.header.next_base_fee()
    }

    pub fn get_tx(&self, index: usize) -> Option<&Transaction> {
        self.transactions.get(index)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BlockHeader {
    pub version: u8,
    pub index: u64,
    pub chain_id: u64,
    pub position: BlockPos,
    pub previous_hash: IonicHash,
    pub timestamp: u64,
    pub proposer: IonicPK,
    pub transactions_root: IonicHash,
    pub state_root: IonicHash,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee: u256,
    // pub previous_rando // used for smart contract random opcode
}

impl BlockHeader {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.push(self.version);
        bytes.extend_from_slice(&self.chain_id.to_le_bytes());
        bytes.extend_from_slice(&self.index.to_le_bytes());
        bytes.extend_from_slice(&self.position.to_bytes());
        bytes.extend_from_slice(self.previous_hash.as_ref());
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(self.proposer.as_ref());
        bytes.extend_from_slice(self.transactions_root.as_ref());
        bytes.extend_from_slice(self.state_root.as_ref());
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


    pub fn next_base_fee(&self) -> u256 {
        let current = self.base_fee;
        let target = self.gas_limit / 2;
        if target == 0 || self.gas_used == target {
            return current;
        }
         if self.gas_used > target {
            let delta = u256::from(self.gas_used - target);
            let change = current * delta / u256::from(target) / 8.as_u256();
            (current + change).max(u256::ONE)
        } else {
            let delta = u256::from(target - self.gas_used);
            let change = current * delta / u256::from(target) / 8.as_u256();
            if current > change { current - change } else { u256::ONE }
        }
    }

}
