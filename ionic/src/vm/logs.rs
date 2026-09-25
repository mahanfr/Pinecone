use ethnum::u256;

use crate::types::IonicAddr;

#[derive(Debug, Clone)]
pub struct IonicLog {
    pub address: IonicAddr,
    pub topics: Vec<u256>,
    pub data: Vec<u8>,
}

impl IonicLog {
    pub fn new(address: IonicAddr, topics: Vec<u256>, data: Vec<u8>) -> Self {
        Self {
            address,
            topics,
            data,
        }
    }
}
