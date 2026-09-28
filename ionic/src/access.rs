use std::collections::HashSet;

use crate::types::{IonicAddr, IonicHash};

pub const COLD_ACCOUNT_ACCESS_COST: u64 = 2600;
pub const WARM_ACCOUNT_ACCESS_COST: u64 = 100;
pub const COLD_SLOAD_COST:          u64 = 2100;
pub const WARM_SLOAD_COST:          u64 = 100;

#[derive(Debug, Default, Clone)]
pub struct AccessList {
    addresses:     HashSet<IonicAddr>,
    storage_slots: HashSet<(IonicAddr, IonicHash)>,
}

impl AccessList {
    pub fn new() -> Self { Self::default() }

    pub fn reset(&mut self) {
        self.addresses.clear();
        self.storage_slots.clear();
    }

    pub fn warm_account(&mut self, addr: &IonicAddr) -> u64 {
        if self.addresses.insert(*addr) {
            COLD_ACCOUNT_ACCESS_COST
        } else {
            WARM_ACCOUNT_ACCESS_COST
        }
    }

    pub fn pre_warm_account(&mut self, addr: &IonicAddr) {
        self.addresses.insert(*addr);
    }

    pub fn is_account_warm(&self, addr: &IonicAddr) -> bool {
        self.addresses.contains(addr)
    }

    pub fn warm_slot(&mut self, addr: &IonicAddr, key: &IonicHash) -> u64 {
        if self.storage_slots.insert((*addr, *key)) {
            COLD_SLOAD_COST
        } else {
            WARM_SLOAD_COST
        }
    }

    pub fn is_slot_warm(&self, addr: &IonicAddr, key: &IonicHash) -> bool {
        self.storage_slots.contains(&(*addr, *key))
    }
}
