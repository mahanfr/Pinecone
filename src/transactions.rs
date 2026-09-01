use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use log::error;

use crate::types::{PineAddr, PineHash, PinePK, PineTXSignature, addr_from_pk};

const TX_VERSION: u8 = 1;
const TX_DOMAIN: &[u8] = b"PINECONE_TX";
const TX_SIGNATURE_DOMAIN: &[u8] = b"PINECONE_TX_SIGNATURE";
const TX_EMPTY_ROOT_DOMAIN: &[u8] = b"PINECONE_EMPTY_TX_ROOT";
const TX_EMPTY_DOMAIN: &[u8] = b"PINECONE_EMPTY_TX";
const MERKLE_TREE_DOMAIN: &[u8] = b"PINECONE_MT";

#[derive(Debug, Clone)]
pub struct Transaction {
    pub version: u8,
    pub chain_id: u64,
    // the current count of transactions from the address
    pub nonce: u64,
    pub sender_pk: PinePK,
    pub recepient: Option<PineAddr>,

    pub value: u128,
    pub gas_limit: u64,
    pub max_fee: u128,

    pub data: Vec<u8>,

    pub signature: PineTXSignature,
}

impl Transaction {
    pub fn new(
        sec_key: &ed25519_dalek::SigningKey,
        chain_id: u64,
        nonce: u64,
        sender_pk: [u8; 32],
        recepient: Option<PineAddr>,
        value: u128,
        data: Vec<u8>,
    ) -> Self {
        let mut unsigned = Self {
            version: TX_VERSION,
            chain_id,
            nonce,
            sender_pk,
            recepient,
            value,
            gas_limit: 0,
            max_fee: 0,
            data,
            signature: [0u8; 64],
        };
        let hash = unsigned.hash_unsigned();
        let signature = sec_key.sign(&hash);
        unsigned.signature = signature.to_bytes();
        unsigned
    }

    pub fn verify(&self) -> bool {
        if self.signature.is_empty() {
            error!("Empty Signature: The transaction has not been signed");
            return false;
        }
        let public_key = match VerifyingKey::from_bytes(&self.sender_pk) {
            Ok(key) => key,
            Err(_) => {
                error!("Invalid Sender Publick Key");
                return false;
            }
        };
        let hash = self.hash_unsigned();
        public_key
            .verify(&hash, &Signature::from_bytes(&self.signature))
            .is_ok()
    }

    fn encode_unsigned(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.version);
        bytes.extend_from_slice(&self.chain_id.to_le_bytes());
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.sender_pk);

        // encoding Option
        match self.recepient {
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

    fn hash_unsigned(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(TX_SIGNATURE_DOMAIN);
        bytes.extend_from_slice(&self.encode_unsigned());
        blake3::hash(&bytes).as_bytes().to_owned()
    }

    pub fn hash(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(TX_DOMAIN);
        bytes.extend_from_slice(&self.encode_unsigned());
        bytes.extend_from_slice(&self.signature);
        blake3::hash(&bytes).as_bytes().to_owned()
    }

    pub fn sender(&self) -> [u8; 32] {
        addr_from_pk(&self.sender_pk)
    }
}

pub fn transactions_root(transactions: &[Transaction]) -> PineHash {
    if transactions.is_empty() {
        return blake3::hash(TX_EMPTY_ROOT_DOMAIN).as_bytes().to_owned();
    }
    let mut level: Vec<PineHash> = transactions.iter().map(|tx| tx.hash()).collect();

    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let left = &pair[0];

            let right = if pair.len() == 2 {
                pair[1]
            } else {
                blake3::hash(TX_EMPTY_DOMAIN).as_bytes().to_owned()
            };

            let mut data = Vec::new();
            data.extend_from_slice(MERKLE_TREE_DOMAIN);
            data.extend_from_slice(left);
            data.extend_from_slice(&right);

            next.push(blake3::hash(&data).as_bytes().to_owned());
        }
        level = next;
    }

    level[0]
}

#[cfg(test)]
mod tests {
    use crate::{keygen::generate_key_pair, transactions::Transaction};

    #[test]
    pub fn sign_and_verify_transaction() {
        // Generate Public/Private key
        let (privk, pubk) = generate_key_pair();
        let sender_pk = pubk.as_bytes().to_owned();

        let transaction = Transaction::new(&privk, 0, 1, sender_pk, None, 0, vec![]);
        assert!(transaction.verify())
    }
}
