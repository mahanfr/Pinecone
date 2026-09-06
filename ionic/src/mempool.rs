use std::{collections::HashMap, sync::{Arc, RwLock}};

use crate::{transactions::Transaction, types::{IonicAddr, IonicHash}};

pub const MEMPOOL_MAX_CAPACITY: usize = 16;

pub struct Mempool {
    pub pending: RwLock<HashMap<IonicHash, Arc<Transaction>>>,
    pub queued: RwLock<HashMap<IonicHash, Arc<Transaction>>>,
    pub by_sender: RwLock<HashMap<IonicAddr, HashMap<u64, IonicHash>>>,
}
