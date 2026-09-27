use ethnum::u256;

use crate::{types::IonicAddr, vm::logs::IonicLog};

#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub address: IonicAddr,
    pub origin: IonicAddr,
    pub caller: IonicAddr,
    pub logs: Vec<IonicLog>,
    pub nonce: u64,
    pub call_value: u256,
    pub gas_price: u256,
    pub calldata: Vec<u8>,
    pub return_data: Vec<u8>,
    pub is_static: bool,
}
