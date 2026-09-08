use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt::Display,
    sync::{Arc, PoisonError, RwLock},
};

use crate::{
    blockchian::Blockchain,
    transactions::{Transaction, TransactionError},
    types::{IonicAddr, IonicHash},
    utils::current_timestamp,
};

pub const MEMPOOL_MAX_CAPACITY: usize = 16;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum MempoolStatus {
    Backlog,
    Pending,
}

#[derive(Debug)]
pub struct Mempool {
    pub pending: RwLock<HashMap<IonicHash, Arc<Transaction>>>,
    pub backlog: RwLock<HashMap<IonicHash, Arc<Transaction>>>,
    pub by_sender: RwLock<HashMap<IonicAddr, HashMap<u64, IonicHash>>>,
    pub priority_index: RwLock<BTreeMap<(u128, MempoolStatus, u64, IonicHash), IonicHash>>,
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            backlog: RwLock::new(HashMap::new()),
            by_sender: RwLock::new(HashMap::new()),
            priority_index: RwLock::new(BTreeMap::new()),
        }
    }

    pub async fn submit_transaction(
        &mut self,
        tx: &Transaction,
        blockchain: &Blockchain,
    ) -> Result<(), MempoolError> {
        blockchain.state.validate_transaction(tx)?;
        let key = tx.hash();
        if self.is_transaction_already_queued(&key)? {
            return Err(MempoolError::TransactionAlreadyExists);
        }
        let base_fee = blockchain.base_fee();
        if self.should_evict()? {
            self.evict_low_priority(tx, base_fee).await?;
        }
        let account = blockchain.state.get_account(&tx.sender())?;

        let tx_arc = Arc::new(tx.clone());

        if account.nonce == tx.nonce {
            self.submit_to_pending(key, tx_arc)?;
            self.update_priority_index(tx, key, MempoolStatus::Pending, base_fee)?;
            self.promote_backlog(tx, base_fee)?;
        } else if account.nonce > tx.nonce {
            self.submit_to_backlog(key, tx_arc)?;
            self.update_priority_index(tx, key, MempoolStatus::Backlog, base_fee)?;
        }
        Ok(())
    }

    fn submit_to_pending(
        &mut self,
        key: IonicHash,
        tx: Arc<Transaction>,
    ) -> Result<(), MempoolError> {
        let mut pending = self.pending.write()?;
        let mut by_sender = self.by_sender.write()?;

        pending.insert(key, tx.clone());
        by_sender
            .entry(tx.sender())
            .or_insert_with(HashMap::new)
            .insert(tx.nonce, key);

        Ok(())
    }

    fn update_priority_index(
        &mut self,
        tx: &Transaction,
        key: IonicHash,
        status: MempoolStatus,
        base_fee: u128,
    ) -> Result<(), MempoolError> {
        let mut priority_index = self.priority_index.write()?;
        let gas_price = tx.gas_price(base_fee)?;
        let priority_key = match status {
            MempoolStatus::Pending => (gas_price, MempoolStatus::Pending, current_timestamp(), key),
            MempoolStatus::Backlog => {
                (tx.max_fee, MempoolStatus::Backlog, current_timestamp(), key)
            }
        };
        priority_index.insert(priority_key, key);
        Ok(())
    }

    fn submit_to_backlog(
        &mut self,
        key: IonicHash,
        tx: Arc<Transaction>,
    ) -> Result<(), MempoolError> {
        let mut backlog = self.backlog.write()?;
        let mut by_sender = self.by_sender.write()?;

        backlog.insert(key, tx.clone());
        by_sender
            .entry(tx.sender())
            .or_insert_with(HashMap::new)
            .insert(tx.nonce, key);

        Ok(())
    }

    fn is_transaction_already_queued(&self, key: &IonicHash) -> Result<bool, MempoolError> {
        let backlog = self.backlog.read()?;
        let pending = self.pending.read()?;

        Ok(backlog.contains_key(key) || pending.contains_key(key))
    }

    fn should_evict(&self) -> Result<bool, MempoolError> {
        let backlog = self.backlog.read()?;
        let pending = self.pending.read()?;

        Ok(backlog.len() + pending.len() >= MEMPOOL_MAX_CAPACITY)
    }

    fn promote_backlog(&mut self, tx: &Transaction, base_fee: u128) -> Result<(), MempoolError> {
        let by_sender = self.by_sender.read()?;
        let sender_map = match by_sender.get(&tx.sender()) {
            Some(map) => map.clone(),
            None => return Ok(()),
        };
        drop(by_sender);
        let mut nonce = tx.nonce + 1;
        while let Some(&tx_hash) = sender_map.get(&nonce) {
            let mut backlog = self.backlog.write()?;
            if let Some(tx_arc) = backlog.remove(&tx_hash) {
                drop(backlog);
                let mut pending = self.pending.write()?;
                pending.insert(tx_hash, tx_arc.clone());
                drop(pending);

                self.update_priority_index(&tx_arc, tx_hash, MempoolStatus::Pending, base_fee)?;

                nonce += 1;
            } else {
                break;
            }
        }
        Ok(())
    }
    async fn evict_low_priority(
        &mut self,
        tx: &Transaction,
        base_fee: u128,
    ) -> Result<(), MempoolError> {
        let mut priority_index = self.priority_index.write()?;
        if let Some((&evicting_score, &evicting_hash)) = priority_index.first_key_value() {
            let gas_price = tx.gas_price(base_fee)?;
            if gas_price <= evicting_score.0 {
                return Err(MempoolError::MempoolFull);
            }
            let mut queue = match evicting_score.1 {
                MempoolStatus::Pending => self.pending.write()?,
                MempoolStatus::Backlog => self.backlog.write()?,
            };
            if let Some(evicted_tx) = queue.remove(&evicting_hash) {
                let mut by_sender = self.by_sender.write()?;
                let sender = evicted_tx.sender();
                if let Some(sender_map) = by_sender.get_mut(&sender) {
                    sender_map.remove(&evicted_tx.nonce);
                    if sender_map.is_empty() {
                        by_sender.remove(&evicted_tx.sender());
                    }
                }
                priority_index.remove(&evicting_score);
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum MempoolError {
    InvalidTransaction(TransactionError),
    TransactionAlreadyExists,
    MempoolFull,
    InternalError,
}

impl From<TransactionError> for MempoolError {
    fn from(value: TransactionError) -> Self {
        Self::InvalidTransaction(value)
    }
}

impl<T> From<PoisonError<T>> for MempoolError {
    fn from(_: PoisonError<T>) -> Self {
        Self::InternalError
    }
}

impl Display for MempoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransaction(t) => write!(f, "Mempool Error => {t}"),
            Self::InternalError => write!(f, "Internal Error: can not access mempool queues"),
            Self::MempoolFull => write!(
                f,
                "Mempool Is Full: transaction dose not have enough priority"
            ),
            Self::TransactionAlreadyExists => write!(
                f,
                "Transaction Already Exists: can not add transaction into the mempool"
            ),
        }
    }
}

impl Error for MempoolError {}
