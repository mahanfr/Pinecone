use ed25519_dalek::{Signature, Signer};

use crate::{Address, PrivateKey, PublicKey};

#[derive(Debug, Clone)]
pub struct TransactionFields {
    pub version: u8,
    pub chain_id: u64,
    pub nonce: u64,
    pub public_key: PublicKey,
    pub recipient: Option<Address>,

    pub value: u128,
    pub gas_limit: u64,
    pub max_fee: u128,

    pub data: Vec<u8>,
}

impl TransactionFields {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.version);
        bytes.extend_from_slice(&self.chain_id.to_le_bytes());
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.public_key);

        match self.recipient {
            Some(addr) => {
                bytes.push(1u8);
                bytes.extend_from_slice(&addr);
            }
            None => {
                bytes.push(0u8);
            }
        }

        bytes.extend_from_slice(&self.value.to_le_bytes());
        bytes.extend_from_slice(&self.gas_limit.to_le_bytes());
        bytes.extend_from_slice(&self.max_fee.to_le_bytes());

        bytes.extend_from_slice(&self.data.len().to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PINECONE_TX_V1");
        bytes.extend_from_slice(&self.encode());
        blake3::hash(&bytes).as_bytes().to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub fields: TransactionFields,
    pub signature: Signature,
}

impl Transaction {
    pub fn new_signed(sec_key: &PrivateKey, fields: TransactionFields) -> Self {
        let hash = fields.hash();
        let signature = sec_key.sign(&hash);
        Self {
            fields,
            signature
        }
    }
}

