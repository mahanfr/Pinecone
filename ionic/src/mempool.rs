use std::{collections::HashMap, error::Error, fmt::Display, sync::{Arc, RwLock}};

use crate::{state::IonicState, transactions::Transaction, types::{IonicAddr, IonicHash}};

pub const MEMPOOL_MAX_CAPACITY: usize = 16;

pub struct Mempool {
    pub pending: RwLock<HashMap<IonicHash, Arc<Transaction>>>,
    pub queued: RwLock<HashMap<IonicHash, Arc<Transaction>>>,
    pub by_sender: RwLock<HashMap<IonicAddr, HashMap<u64, IonicHash>>>,
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            queued: RwLock::new(HashMap::new()),
            by_sender: RwLock::new(HashMap::new())
        }
    }

    pub async fn validate_transaction(&self, tx: &Transaction, state: &IonicState) -> Result<(), MempoolError> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum MempoolError {
    InvalidAccount,
}

impl Display for MempoolError {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAccount => write!(f, "invalid Account: Can not find account with this public key"),
        }
   }
}

impl Error for MempoolError {}
