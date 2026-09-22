use ionic::vm::instructions::IonicInstr;

use crate::parser::{Contract, Contracts};

pub fn compile(contracts: Contracts) -> Vec<IonicInstr> {
    let mut instrs = Vec::new();
    for contract in contracts.items {
        instrs.extend_from_slice(&compile_contract(contract));
    }
    instrs
}

pub fn compile_contract(contract: Contract) -> Vec<IonicInstr> {
    todo!()
}
