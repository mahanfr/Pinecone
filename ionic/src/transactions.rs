use std::{error::Error, fmt::Display};

use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use log::{error, warn};

use crate::{
    types::{IonicAddr, IonicHash, IonicPK, IonicTXSignature},
    utils::IonicBase64,
};

const TX_VERSION: u8 = 1;
const TX_DOMAIN: &[u8] = b"IONIC_TX";
const TX_SIGNATURE_DOMAIN: &[u8] = b"IONIC_TX_SIGNATURE";
const TX_EMPTY_ROOT_DOMAIN: &[u8] = b"IONIC_EMPTY_TX_ROOT";
const TX_EMPTY_DOMAIN: &[u8] = b"IONIC_EMPTY_TX";
const MERKLE_TREE_DOMAIN: &[u8] = b"IONIC_MT";

#[derive(Debug, Clone)]
pub struct TransactionBuilder {
    signed: bool,
    tx: Transaction,
}

impl TransactionBuilder {
    pub fn with_fees(
        &mut self,
        gas_limit: u64,
        max_fee: u128,
        max_priority_fee: u128,
    ) -> &mut Self {
        self.tx.gas_limit = gas_limit;
        self.tx.max_fee = max_fee;
        self.tx.max_priority_fee = max_priority_fee;
        self
    }
    pub fn with_data(&mut self, data: Vec<u8>) -> &mut Self {
        self.tx.data = data;
        self
    }
    pub fn sign(&mut self, sec_key: &ed25519_dalek::SigningKey) -> &mut Self {
        self.tx.sign(sec_key);
        self.signed = true;
        self
    }
    pub fn build(&mut self) -> Transaction {
        if self.tx.gas_limit == 0 || self.tx.max_fee == 0 || self.tx.max_priority_fee == 0 {
            warn!("Operaion fees have not been set")
        }
        if !self.signed {
            warn!("Finalizing transaction without signing")
        }
        self.tx.to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub version: u8,
    pub chain_id: u64,
    // the current count of transactions from the address
    pub nonce: u64,
    pub sender_pk: IonicPK,
    pub recepient: Option<IonicAddr>,

    pub value: u128,
    pub gas_limit: u64,
    pub max_fee: u128,
    pub max_priority_fee: u128,

    pub data: Vec<u8>,

    pub signature: IonicTXSignature,
}

impl Default for Transaction {
    fn default() -> Self {
        Self {
            version: TX_VERSION,
            chain_id: 0,
            nonce: 0,
            sender_pk: IonicPK::default(),
            recepient: None,
            value: 0,
            gas_limit: 0,
            max_fee: 0,
            max_priority_fee: 0,
            data: Vec::new(),
            signature: [0u8; 64],
        }
    }
}

impl Transaction {
    pub fn new_builder(
        chain_id: u64,
        nonce: u64,
        sender_pk: IonicPK,
        recepient: Option<IonicAddr>,
        value: u128,
    ) -> TransactionBuilder {
        TransactionBuilder {
            signed: false,
            tx: Transaction {
                version: TX_VERSION,
                chain_id,
                nonce,
                sender_pk,
                recepient,
                value,
                ..Default::default()
            },
        }
    }

    pub fn sign(&mut self, sec_key: &ed25519_dalek::SigningKey) {
        let hash = self.hash_unsigned();
        let signature = sec_key.sign(&hash.as_ref());
        self.signature = signature.to_bytes();
    }

    pub fn verify_signature(&self) -> bool {
        if self.signature.is_empty() {
            error!("Empty Signature: The transaction has not been signed");
            return false;
        }
        let public_key : VerifyingKey = match &self.sender_pk.try_into() {
            Ok(key) => *key,
            Err(_) => {
                error!("Invalid Sender Publick Key");
                return false;
            }
        };
        let hash = self.hash_unsigned();
        public_key
            .verify(&hash.as_ref(), &Signature::from_bytes(&self.signature))
            .is_ok()
    }

    fn encode_unsigned(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.version);
        bytes.extend_from_slice(&self.chain_id.to_le_bytes());
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.sender_pk.as_ref());

        // encoding Option
        match self.recepient {
            Some(addr) => {
                bytes.push(1u8);
                bytes.extend_from_slice(&addr.as_ref());
            }
            None => {
                bytes.push(0u8);
            }
        }

        bytes.extend_from_slice(&self.value.to_le_bytes());
        bytes.extend_from_slice(&self.gas_limit.to_le_bytes());
        bytes.extend_from_slice(&self.max_fee.to_le_bytes());
        bytes.extend_from_slice(&self.max_priority_fee.to_le_bytes());

        bytes.extend_from_slice(&self.data.len().to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    fn hash_unsigned(&self) -> IonicHash {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(TX_SIGNATURE_DOMAIN);
        bytes.extend_from_slice(&self.encode_unsigned());
        blake3::hash(&bytes).into()
    }

    pub fn hash(&self) -> IonicHash {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(TX_DOMAIN);
        bytes.extend_from_slice(&self.encode_unsigned());
        bytes.extend_from_slice(&self.signature);
        blake3::hash(&bytes).into()
    }

    pub fn sender(&self) -> IonicAddr {
        IonicAddr::from_pk(&self.sender_pk)
    }

    pub fn priority_fee(&self, base_fee: u128) -> Result<u128, TransactionError> {
        if self.max_fee < base_fee {
            return Err(TransactionError::MaxFeeTooSmall);
        }
        Ok(std::cmp::min(
            self.max_priority_fee,
            self.max_fee - base_fee,
        ))
    }

    pub fn gas_price(&self, base_fee: u128) -> Result<u128, TransactionError> {
        let tip = self.priority_fee(base_fee)?;
        Ok(base_fee + tip)
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let recep = match self.recepient {
            Some(recp) => IonicBase64::encode(recp),
            None => "None".into(),
        };
        write!(
            f,
            "Transaction V{} => {{ChainID: {}, Nonce: {}, From: {}, To: {}, ",
            self.version,
            self.chain_id,
            self.nonce,
            IonicBase64::encode(self.sender_pk),
            recep,
        )?;
        write!(
            f,
            "Value: {}, Gas Limit: {}, Max Fee: {}, Signature: <{}>}}",
            self.value,
            self.gas_limit,
            self.max_fee,
            IonicBase64::encode(self.signature)
        )
    }
}

pub fn transactions_root(transactions: &[Transaction]) -> IonicHash {
    if transactions.is_empty() {
        return blake3::hash(TX_EMPTY_ROOT_DOMAIN).into();
    }
    let mut level: Vec<IonicHash> = transactions.iter().map(|tx| tx.hash()).collect();

    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let left = &pair[0];

            let right = if pair.len() == 2 {
                pair[1]
            } else {
                blake3::hash(TX_EMPTY_DOMAIN).into()
            };

            let mut data = Vec::new();
            data.extend_from_slice(MERKLE_TREE_DOMAIN);
            data.extend_from_slice(left.as_ref());
            data.extend_from_slice(&right.as_ref());

            next.push(blake3::hash(&data).into());
        }
        level = next;
    }

    level[0]
}

#[derive(Debug)]
pub enum TransactionError {
    InvalidNonce,
    InsufficientBalance,
    InvalidAccount,
    UnavailableState,
    InvalidTxSignature,
    InvalidChainID,
    MaxFeeTooSmall,
}

impl Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNonce => {
                write!(f, "Invalid Nonce: nonce dose not match the account state")
            }
            Self::InsufficientBalance => write!(
                f,
                "Insufficient Balance: balance is insufficient for this operation"
            ),
            Self::InvalidAccount => write!(
                f,
                "Invalid Account: can not find any account linked with the provided address"
            ),
            Self::UnavailableState => write!(
                f,
                "Unavailable State: consider downloading the state form a state owner"
            ),
            Self::InvalidTxSignature => write!(
                f,
                "Invalid Signature: the transaction is not signed by the creator"
            ),
            Self::InvalidChainID => write!(
                f,
                "Invalid Chain ID: provided chain_id dose not match stateTrie or the blockchain"
            ),
            Self::MaxFeeTooSmall => write!(
                f,
                "Max Fee Too Small: maximum fee can not cover the base fee"
            ),
        }
    }
}

impl Error for TransactionError {}

#[cfg(test)]
mod tests {
    use crate::{keygen::generate_key_pair, transactions::Transaction};

    #[test]
    pub fn sign_and_verify_transaction() {
        // Generate Public/Private key
        let (privk, pubk) = generate_key_pair();

        let transaction = Transaction::new_builder(0, 0, pubk.into(), None, 0)
            .with_fees(100, 2000, 1000)
            .sign(&privk)
            .build();
        assert!(transaction.verify_signature())
    }
}
