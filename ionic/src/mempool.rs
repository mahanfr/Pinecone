use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    error::Error,
    fmt::Display,
    sync::{Arc, PoisonError, RwLock},
};

use crate::{
    blockchian::Blockchain,
    blocks::Block,
    transactions::{Transaction, TransactionError},
    types::{IonicAddr, IonicHash},
    utils::current_timestamp,
};

pub const MEMPOOL_MAX_CAPACITY: usize = 16;
pub const MEMPOOL_EVICTION_TIMEOUT: u64 = 3600;

type PriorityIndex = (u128, u64, IonicHash);

#[derive(Debug)]
pub enum QueueType {
    Backlog,
    Pending,
}

#[derive(Debug)]
pub struct MempoolQueues {
    pub pending: HashMap<IonicHash, Arc<Transaction>>,
    pub backlog: HashMap<IonicHash, Arc<Transaction>>,
    pub by_sender: HashMap<IonicAddr, BTreeMap<u64, IonicHash>>,
    pub priority_index: BTreeSet<PriorityIndex>,
    pub backlog_index: BTreeSet<PriorityIndex>,
    pub index_lookup: HashMap<IonicHash, (PriorityIndex, QueueType)>,
}

#[derive(Debug)]
pub struct Mempool {
    queues: RwLock<MempoolQueues>,
    capacity: usize,
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            queues: RwLock::new(MempoolQueues {
                pending: HashMap::new(),
                backlog: HashMap::new(),
                by_sender: HashMap::new(),
                priority_index: BTreeSet::new(),
                backlog_index: BTreeSet::new(),
                index_lookup: HashMap::new(),
            }),
            capacity: MEMPOOL_MAX_CAPACITY,
        }
    }

    pub async fn submit_transaction(
        &self,
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

        let mut queues = self.queues.write()?;

        // if tx with same sender and nonce exists in the queues remove it first
        if let Some(nonce_map) = queues.by_sender.get(&tx.sender()) {
            if let Some(hash) = nonce_map.get(&tx.nonce) {
                let hash_clone = hash.clone();
                let _ = Self::remove_tx(&mut queues, hash_clone).await;
            }
        }

        if account.nonce == tx.nonce {
            Self::submit_to_pending(&mut queues, key, tx_arc)?;
            Self::update_priority_index(&mut queues, QueueType::Pending, tx, key, base_fee)?;
            Self::promote_backlog(&mut queues, &tx.sender(), base_fee)?;
        } else if account.nonce < tx.nonce {
            Self::submit_to_backlog(&mut queues, key, tx_arc)?;
            Self::update_priority_index(&mut queues, QueueType::Backlog, tx, key, base_fee)?;
        } else {
            return Err(MempoolError::InvalidTransactionNonce);
        }
        Ok(())
    }

    pub async fn select_for_block(
        &self,
        min: usize,
        max: usize,
    ) -> Result<Vec<Arc<Transaction>>, MempoolError> {
        let mut transactions = Vec::with_capacity(max);
        let queues = self.queues.read()?;
        let mut already_added = HashSet::<IonicHash>::new();
        for (_, _, key) in queues.priority_index.iter().rev() {
            if transactions.len() == max {
                break;
            }
            if already_added.contains(key) {
                continue;
            }

            let Some(tx) = queues.pending.get(key) else {
                continue;
            };

            let sender_addr = tx.sender();
            let Some(sender_nonce_map) = queues.by_sender.get(&sender_addr) else {
                continue;
            };

            let mut prev_nonce: u64 = u64::MAX;
            let mut temp_added = Vec::new();
            for (nonce, inner_key) in sender_nonce_map.iter() {
                if already_added.contains(inner_key) {
                    continue;
                }
                if !queues.pending.contains_key(inner_key) {
                    break;
                }
                if prev_nonce != u64::MAX {
                    if prev_nonce + 1 != *nonce {
                        break;
                    }
                }
                if transactions.len() + temp_added.len() + 1 <= max {
                    temp_added.push(*inner_key);
                    already_added.insert(*inner_key);
                } else {
                    break;
                }
                prev_nonce = *nonce;
            }
            for tx_key in temp_added.iter() {
                let Some(tx) = queues.pending.get(tx_key) else {
                    return Err(MempoolError::MempoolOutOfSync);
                };
                transactions.push(tx.to_owned());
            }
        }
        if transactions.len() < min {
            return Err(MempoolError::NotEnoughTransactions);
        }

        Ok(transactions)
    }

    pub async fn update_after_block(&self, block: Block) -> Result<(), MempoolError> {
        let mut queues = self.queues.write()?;

        let mut affected_senders: HashSet<IonicAddr> = HashSet::new();
        for tx in block.transactions.iter() {
            let key = tx.hash();
            let sender = tx.sender();

            affected_senders.insert(sender);
            queues.pending.remove(&key);
            queues.backlog.remove(&key);
            if let Some((pkey, _)) = queues.index_lookup.remove(&key) {
                queues.priority_index.remove(&pkey);
            }
            if let Some(nonce_map) = queues.by_sender.get_mut(&sender) {
                nonce_map.remove(&tx.nonce);
                if nonce_map.is_empty() {
                    queues.by_sender.remove(&sender);
                }
            }
        }

        let base_fee = block.next_base_fee();
        for sender in affected_senders {
            Self::promote_backlog(&mut queues, &sender, base_fee)?;
        }

        Self::rebuild_priority_index(&mut queues, base_fee).await;

        Ok(())
    }

    async fn rebuild_priority_index(queues: &mut MempoolQueues, base_fee: u128) {
        let mut new_priority_index = BTreeSet::<PriorityIndex>::new();
        for (_, _, hash) in queues.priority_index.iter() {
            if let Some(tx) = queues.pending.get(hash) {
                if let Ok(gas_price) = tx.gas_price(base_fee) {
                    let pkey = (gas_price, tx.timestamp, *hash);
                    new_priority_index.insert(pkey);
                    queues
                        .index_lookup
                        .insert(*hash, (pkey, QueueType::Pending));
                }
            }
        }
        queues.priority_index = new_priority_index;
    }

    pub async fn remove_tx(queues: &mut MempoolQueues, hash: IonicHash) -> Result<Arc<Transaction>, MempoolError> {
        if let Some((pkey, q_type)) = queues.index_lookup.get(&hash) {
            let tx_arc = match q_type {
                QueueType::Pending => {
                    queues.priority_index.remove(pkey);
                    queues.pending.remove(&hash)
                },
                QueueType::Backlog => {
                    queues.backlog_index.remove(pkey);
                    queues.backlog.remove(&hash)
                },
            };
            let tx = match tx_arc {
                Some(tx) => tx,
                None => return Err(MempoolError::TransactionNotExists),
            };
            if let Some(nonce_map) = queues.by_sender.get_mut(&tx.sender()) {
                nonce_map.remove(&tx.nonce);
            }
            return Ok(tx);
        } else {
            return Err(MempoolError::TransactionNotExists);
        }
    }


    fn submit_to_pending(
        queues: &mut MempoolQueues,
        key: IonicHash,
        tx: Arc<Transaction>,
    ) -> Result<(), MempoolError> {
        queues.pending.insert(key, tx.clone());
        queues
            .by_sender
            .entry(tx.sender())
            .or_insert_with(BTreeMap::new)
            .insert(tx.nonce, key);

        Ok(())
    }

    fn update_priority_index(
        queues: &mut MempoolQueues,
        queue_type: QueueType,
        tx: &Transaction,
        key: IonicHash,
        base_fee: u128,
    ) -> Result<(), MempoolError> {
        let gas_price = tx.gas_price(base_fee)?;
        let priority_key = (gas_price, tx.timestamp, key);
        match queue_type {
            QueueType::Pending => queues.priority_index.insert(priority_key),
            QueueType::Backlog => queues.backlog_index.insert(priority_key),
        };
        queues.index_lookup.insert(key, (priority_key, queue_type));
        Ok(())
    }

    fn submit_to_backlog(
        queues: &mut MempoolQueues,
        key: IonicHash,
        tx: Arc<Transaction>,
    ) -> Result<(), MempoolError> {
        queues.backlog.insert(key, tx.clone());
        queues
            .by_sender
            .entry(tx.sender())
            .or_insert_with(BTreeMap::new)
            .insert(tx.nonce, key);

        Ok(())
    }

    fn is_transaction_already_queued(&self, key: &IonicHash) -> Result<bool, MempoolError> {
        let queues = self.queues.read()?;

        Ok(queues.backlog.contains_key(key) || queues.pending.contains_key(key))
    }

    fn should_evict(&self) -> Result<bool, MempoolError> {
        let queues = self.queues.read()?;

        Ok(queues.backlog.len() + queues.pending.len() >= self.capacity)
    }

    fn promote_backlog(
        queues: &mut MempoolQueues,
        sender: &IonicAddr,
        base_fee: u128,
    ) -> Result<(), MempoolError> {
        let nonce_map = match queues.by_sender.get(sender) {
            Some(map) => map.clone(),
            None => return Ok(()),
        };
        let nonces: Vec<u64> = nonce_map.keys().copied().collect();
        let mut prev: u64 = u64::MAX;
        for nonce in nonces {
            if prev != u64::MAX && prev + 1 != nonce {
                break;
            }
            prev = nonce;

            let hash = nonce_map[&nonce];

            if queues.pending.contains_key(&hash) {
                continue;
            }

            if let Some(tx) = queues.backlog.remove(&hash) {
                queues.pending.insert(hash, tx.clone());
                match queues.index_lookup.get(&hash) {
                    Some((key, _)) => {
                        queues.backlog_index.remove(key);
                        queues.priority_index.insert(*key);
                        queues.index_lookup.insert(hash, (*key, QueueType::Pending));
                    }
                    None => {
                        if let Ok(effective) = tx.gas_price(base_fee) {
                            let pkey = (effective, tx.timestamp, hash);
                            queues.priority_index.insert(pkey);
                            queues.index_lookup.insert(hash, (pkey, QueueType::Pending));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn evict_low_priority(
        &self,
        tx: &Transaction,
        base_fee: u128,
    ) -> Result<(), MempoolError> {
        let mut queues = self.queues.write()?;
        let current_time = current_timestamp();
        let expected_price = tx.gas_price(base_fee)?;

        let mut removed_items = Vec::new();
        for (price, time, key) in queues.backlog_index.iter() {
            if price < &expected_price || current_time - time > MEMPOOL_EVICTION_TIMEOUT {
                removed_items.push(key.clone());
            } else {
                break;
            }
        }
        if removed_items.is_empty() {
            for (price, time, key) in queues.priority_index.iter() {
                if price < &expected_price || current_time - time > MEMPOOL_EVICTION_TIMEOUT {
                    removed_items.push(key.clone());
                } else {
                    break;
                }
            }
        }
        if removed_items.is_empty() {
            return Err(MempoolError::MempoolFull);
        }
        for key in removed_items.iter() {
            if let Some((prikey, queue_type)) = queues.index_lookup.remove(key) {
                let Some(evicted_tx) = (match queue_type {
                    QueueType::Pending => {
                        queues.priority_index.remove(&prikey);
                        queues.pending.remove(&key)
                    }
                    QueueType::Backlog => {
                        queues.backlog_index.remove(&prikey);
                        queues.backlog.remove(&key)
                    }
                }) else {
                    continue;
                };
                let sender = evicted_tx.sender();
                if let Some(sender_map) = queues.by_sender.get_mut(&sender) {
                    sender_map.remove(&evicted_tx.nonce);
                    if sender_map.is_empty() {
                        queues.by_sender.remove(&evicted_tx.sender());
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum MempoolError {
    InvalidTransactionNonce,
    InvalidTransaction(TransactionError),
    TransactionAlreadyExists,
    MempoolFull,
    NotEnoughTransactions,
    InternalError,
    MempoolOutOfSync,
    TransactionNotExists,
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
            Self::TransactionNotExists => {
                write!(f, "Transaction dose not exists")
            }
            Self::InvalidTransactionNonce => {
                write!(f, "Transaction nonce smaller than the account nonce")
            }
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
            Self::NotEnoughTransactions => write!(
                f,
                "Not Enough Transactions: Pending Transactions dose not reach your desierd minimum"
            ),
            Self::MempoolOutOfSync => write!(
                f,
                "Mempool out of sync: Key pointing to transactions that dose not exists"
            ),
        }
    }
}

impl Error for MempoolError {}
