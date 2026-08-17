use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};

use crate::{Address, PrivateKey, PublicKey};

#[derive(Debug, Clone, Default)]
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

    pub fn verify(&self) -> bool {
        let public_key = match VerifyingKey::from_bytes(&self.fields.public_key) {
            Ok(key) => key,
            // TODO: Log the error
            Err(_) => return false
        };
        let hash = self.fields.hash();
        public_key.verify(&hash, &self.signature).is_ok()
    }

    pub fn sender(&self) -> Address {
        blake3::hash(&self.fields.public_key).as_bytes().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use crate::{generate_key_pair, transactions::{Transaction, TransactionFields}};

    #[test]
    pub fn sign_and_verify_transaction() {
        // Generate Public/Private key
        let (privk, pubk) = generate_key_pair().unwrap();
        let mut trans_fields = TransactionFields::default();
        trans_fields.public_key = pubk.as_bytes().to_owned();

        let transaction = Transaction::new_signed(&privk, trans_fields);
        assert!(transaction.verify())
    }
}
