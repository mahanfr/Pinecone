use ethnum::u256;

use crate::types::IonicAddr;

#[derive(Debug)]
pub struct CallFrame {
    pub gas: u64,
    pub recepient: IonicAddr,
    pub value: u256,
    pub in_offset: u256,
    pub in_size: u256,
    pub out_offset: u256,
    pub out_size: u256,
    pub code_addr: IonicAddr,
    pub ctx_addr: IonicAddr,
    pub caller: IonicAddr,
    pub call_value: u256,
    pub is_static: bool,
    pub transfer_value: bool,
}
