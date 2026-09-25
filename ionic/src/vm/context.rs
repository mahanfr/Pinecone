use crate::types::IonicAddr;

// AI Slop
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub address: IonicAddr,          // ADDRESS
    pub origin: IonicAddr,           // ORIGIN (tx.origin)
    pub caller: IonicAddr,           // CALLER
    pub call_value: u128,            // CALLVALUE
    pub gas_price: u128,             // GASPRICE (effective gas price)
    pub calldata: Vec<u8>,           // CALLDATALOAD/SIZE/COPY
    pub return_data: Vec<u8>,        // RETURNDATASIZE / RETURNDATACOPY
    pub is_static: bool,             // for STATICCALL enforcement (SSTORE/LOG/CREATE forbidden)
}
