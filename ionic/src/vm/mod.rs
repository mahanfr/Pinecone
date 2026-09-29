pub mod callframe;
pub mod instructions;
mod journal;
pub mod logs;
pub mod memory;
pub mod opcodes;
use std::{collections::HashMap, error::Error, fmt::Display, ops::Not};

use ethnum::{AsU256, u256};

use crate::{
    blockchian::Blockchain,
    blocks::{Block, BlockHeader},
    journal::{Journal, Snapshot},
    transactions::TransactionError,
    types::{IonicAddr, IonicHash},
    utils::ToBytes,
    vm::{
        callframe::CallFrame,
        instructions::IonicInstr,
        journal::{VMJournal, VMSnapshot},
        logs::IonicLog,
        memory::IonicMemory,
        opcodes::IonicOpcode,
    },
};

const MAX_CALL_DEPTH: u32 = 1024;

#[derive(Debug, Clone, Default)]
pub struct VirtualMachine {
    pub tstorage: HashMap<u256, u256>,
    pub block: BlockHeader,
    pub call_depth: u32,
    pub gas_used: u64,
    pub logs: Vec<IonicLog>,
    pub journal: Journal,
    pub vm_journal: VMJournal,
}

#[derive(Debug, Clone, Default)]
pub struct VMInstance {
    pub stack: Vec<u256>,
    pub memory: IonicMemory,
    pub pc: usize,
    pub gas_used: u64,
    pub stopped: bool,
    pub reverted: bool,
    pub return_value: Vec<u8>,
    pub instructions: Vec<IonicInstr>,
    pub state_snapshot: Snapshot,
    pub vm_snapshot: VMSnapshot,

    pub address: IonicAddr,
    pub origin: IonicAddr,
    pub caller: IonicAddr,
    pub nonce: u64,
    pub call_value: u256,
    pub gas_price: u256,
    pub calldata: Vec<u8>,
    pub code: Vec<u8>,
    pub return_data: Vec<u8>,
    pub is_static: bool,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn eval(
        blockchain: &mut Blockchain,
        block: &Block,
        tx_index: usize,
    ) -> Result<(), VMExecutionError> {
        let mut vm = Self::default();
        let tx = &block.transactions[tx_index];
        let sender_addr = tx.sender();

        let code_bytes = tx.data.to_vec();
        let mut instance = VMInstance {
            address: tx
                .recepient
                .unwrap_or_else(|| Self::derive_contract_addr(sender_addr, tx.nonce)),
            origin: sender_addr,
            caller: sender_addr,
            call_value: tx.value,
            nonce: tx.nonce,
            gas_price: tx.gas_price(block.next_base_fee()).unwrap(),
            instructions: Self::parse(&code_bytes)?,
            calldata: code_bytes.clone(),
            code: code_bytes,
            state_snapshot: blockchain.journal.record(),
            ..Default::default()
        };
        vm.block = block.header.clone();
        vm.run(&mut instance, blockchain)
    }

    pub fn parse(code: &[u8]) -> Result<Vec<IonicInstr>, VMExecutionError> {
        let mut instrs: Vec<IonicInstr> = Vec::new();
        let mut cur = 0;
        while cur < code.len() {
            let instr = IonicInstr::parse(code, &mut cur)?;
            instrs.push(instr);
        }
        Ok(instrs)
    }

    pub fn compile(instrs: Vec<IonicInstr>) -> Vec<u8> {
        let mut code = Vec::new();
        for instr in instrs.iter() {
            code.extend_from_slice(&instr.to_bytes());
        }
        code
    }

    pub fn run(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        use opcodes::IonicOpcode::*;
        while instance.pc < instance.instructions.len() {
            let instr = instance.instructions[instance.pc];
            match &instr.opcode {
                STOP => {
                    break;
                }
                INVALID => return Err(VMExecutionError::IllegalInstruction(instr.opcode as u8)),
                ADD | MUL | SUB | DIV | SDIV | MOD | SMOD | EXP | SIGNEXTEND | LT | GT | SLT
                | SGT | EQ | AND | OR | XOR | BYTE | SHL | SHR | SAR => {
                    instance.eval_binary(&instr.opcode)?
                }
                ISZERO | NOT => instance.eval_unary(&instr.opcode)?,
                ADDMOD | MULMOD => instance.eval_ternary(&instr.opcode)?,
                HASH => instance.eval_hash()?,
                ADDRESS => instance.address(instance.address),
                BALANCE => {
                    let addr = instance.pop_stack()?;
                    instance.gas_used += 2100;
                    instance.balance(addr.into(), blockchain)?
                }
                SELFBALANCE => instance.balance(instance.address, blockchain)?,
                BASEFEE => {
                    instance.stack.push(blockchain.base_fee().as_u256());
                }
                GASLIMIT => {
                    instance.stack.push(self.block.gas_limit.as_u256());
                }
                GASPRICE => {
                    instance.stack.push(instance.gas_price.as_u256());
                }
                NUMBER => {
                    instance.stack.push(self.block.index.as_u256());
                }
                TIMESTAMP => {
                    instance.stack.push(self.block.timestamp.as_u256());
                }
                COINBASE => {
                    let proposer_addr = IonicAddr::from_pk(&self.block.proposer);
                    instance.stack.push(proposer_addr.into());
                }
                BLOCKHASH => {
                    let index = instance.pop_stack()?;
                    if let Some(block) = blockchain.get_block(index.as_u64()) {
                        instance.stack.push(block.hash().into())
                    } else {
                        instance.stack.push(u256::ZERO);
                    }
                }
                ORIGIN => instance.address(instance.origin),
                CALLER => instance.address(instance.caller),
                CALLVALUE => instance.stack.push(instance.call_value.as_u256()),
                CALLDATALOAD => {
                    let off = instance.pop_stack()?.as_usize();
                    let calldata: [u8; 32] = instance.calldata[off..off + 32].try_into().unwrap();
                    instance.stack.push(u256::from_le_bytes(calldata));
                }
                CALLDATASIZE => {
                    instance.stack.push(instance.calldata.len().as_u256());
                }
                CALLDATACOPY => {
                    let len = instance.pop_stack()?;
                    let ost = instance.pop_stack()?;
                    let destost = instance.pop_stack()?;
                    let cost = instance.memory.expantion_cost(destost, len);
                    instance
                        .memory
                        .write_padded(destost, len, ost, &instance.calldata);
                    instance.gas_used += cost;
                }
                CODESIZE => instance.stack.push(instance.code.len().as_u256()),
                CODECOPY => {
                    let len = instance.pop_stack()?;
                    let ost = instance.pop_stack()?;
                    let destost = instance.pop_stack()?;
                    let cost = instance.memory.expantion_cost(destost, len);
                    instance
                        .memory
                        .write_padded(destost, len, ost, &instance.code);
                    instance.gas_used += cost;
                }
                EXTCODESIZE => {
                    let addr = instance.pop_stack()?;
                    instance.gas_used += 2100;
                    let code = blockchain.get_code(&addr.into()).unwrap_or_default();
                    instance.stack.push(code.len().as_u256());
                }
                EXTCODECOPY => {
                    let len = instance.pop_stack()?;
                    let ost = instance.pop_stack()?;
                    let destost = instance.pop_stack()?;
                    let addr = instance.pop_stack()?;
                    instance.gas_used += 2100;
                    let code = blockchain.get_code(&addr.into()).unwrap_or_default();
                    let cost = instance.memory.expantion_cost(destost, len);
                    instance.memory.write_padded(destost, len, ost, &code);
                    instance.gas_used += cost;
                }
                EXTCODEHASH => {
                    let addr = instance.pop_stack()?;
                    instance.gas_used += 2100;
                    let code = blockchain.get_code(&addr.into()).unwrap_or_default();
                    let hash: IonicHash = blake3::hash(&code).into();
                    instance.stack.push(hash.into());
                }
                RETURNDATASIZE => {
                    instance.stack.push(instance.return_data.len().as_u256());
                }
                RETURNDATACOPY => {
                    let len = instance.pop_stack()?;
                    let ost = instance.pop_stack()?;
                    let destost = instance.pop_stack()?;
                    let cost = instance.memory.expantion_cost(destost, len);
                    instance
                        .memory
                        .write_padded(destost, len, ost, &instance.return_data);
                    instance.gas_used += cost;
                }
                CHAINID => {
                    instance.stack.push(blockchain.id.as_u256());
                }
                POP => instance.pop()?,
                PC => instance.pc(),
                GAS => instance.gas(),
                PUSH0 | PUSH1 | PUSH2 | PUSH3 | PUSH4 | PUSH5 | PUSH6 | PUSH7 | PUSH8 | PUSH9
                | PUSH10 | PUSH11 | PUSH12 | PUSH13 | PUSH14 | PUSH15 | PUSH16 | PUSH17
                | PUSH18 | PUSH19 | PUSH20 | PUSH21 | PUSH22 | PUSH23 | PUSH24 | PUSH25
                | PUSH26 | PUSH27 | PUSH28 | PUSH29 | PUSH30 | PUSH31 | PUSH32 => {
                    if let Some(imm) = instr.data {
                        instance.push(imm);
                    } else if instr.opcode == IonicOpcode::PUSH0 {
                        instance.push(u256::ZERO);
                    } else {
                        return Err(VMExecutionError::SyntaxError(instr));
                    }
                }
                DUP1 | DUP2 | DUP3 | DUP4 | DUP5 | DUP6 | DUP7 | DUP8 | DUP9 | DUP10 | DUP11
                | DUP12 | DUP13 | DUP14 | DUP15 | DUP16 => instance.dup(
                    instr
                        .opcode
                        .stack_position()
                        .expect("Not A DUP Instruction"),
                ),
                SWAP1 | SWAP2 | SWAP3 | SWAP4 | SWAP5 | SWAP6 | SWAP7 | SWAP8 | SWAP9 | SWAP10
                | SWAP11 | SWAP12 | SWAP13 | SWAP14 | SWAP15 | SWAP16 => {
                    instance.swap(
                        instr
                            .opcode
                            .stack_position()
                            .expect("Not a Swap Instruction"),
                    );
                }
                JUMP => {
                    instance.jump()?;
                    continue;
                }
                JUMPI => {
                    instance.jumpi()?;
                    continue;
                }
                JUMPDEST => (),
                MSTORE => instance.mstore()?,
                MSTORE8 => instance.mstore8()?,
                MLOAD => instance.mload()?,
                MSIZE => instance.stack.push(instance.memory.size()),
                MCOPY => instance.mcpy()?,
                TSTORE => self.tstore(instance)?,
                TLOAD => self.tload(instance)?,
                SSTORE => self.sstore(instance, blockchain)?,
                SLOAD => self.sload(instance, blockchain)?,
                LOG0 | LOG1 | LOG2 | LOG3 | LOG4 => {
                    self.log(
                        instance,
                        instr.opcode.log_topics().expect("Not a Log instruction"),
                    )?;
                }
                CREATE => self.create(instance, blockchain)?,
                CALL => self.call(instance, blockchain)?,
                CALLCODE => self.callcode(instance, blockchain)?,
                RETURN => {
                    self.ret(instance)?;
                    break;
                }
                DELEGATECALL => self.delegatecall(instance, blockchain)?,
                CREATE2 => self.create2(instance, blockchain)?,
                STATICCALL => self.staticcall(instance, blockchain)?,
                REVERT => {
                    self.revert(instance)?;
                    break;
                }
                SELFDESTRUCT => {
                    self.selfdestruct(instance, blockchain)?;
                    break;
                }
                PREVRANDAO => todo!(),
            }
            instance.pc += 1;
            self.gas_used += instr.opcode.gas().unwrap_or_default();
        }
        Ok(())
    }

    fn tstore(&mut self, instance: &mut VMInstance) -> Result<(), VMExecutionError> {
        if instance.is_static {
            return Err(VMExecutionError::StaticViolation(instance.get_instr()));
        }
        let value = instance.pop_stack()?;
        let key_val = instance.pop_stack()?;
        self.tstorage.insert(key_val, value);
        Ok(())
    }

    fn tload(&mut self, instance: &mut VMInstance) -> Result<(), VMExecutionError> {
        let key = instance.pop_stack()?;
        let Some(value) = self.tstorage.get(&key) else {
            return Err(VMExecutionError::TransiantKeyNotFound(key));
        };
        instance.stack.push(*value);
        Ok(())
    }

    fn sload(
        &mut self,
        instance: &mut VMInstance,
        blockchian: &Blockchain,
    ) -> Result<(), VMExecutionError> {
        let key = instance.pop_stack()?;
        let value = blockchian
            .sload(&instance.address, &key.to_owned().into())
            .unwrap_or(&u256::ZERO);
        instance.stack.push(*value);
        Ok(())
    }

    fn sstore(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        if instance.is_static {
            return Err(VMExecutionError::StaticViolation(instance.get_instr()));
        }
        let value = instance.pop_stack()?;
        let key = instance.pop_stack()?;
        blockchain.sstore(&instance.address, key.into(), value);
        Ok(())
    }

    fn log(&mut self, instance: &mut VMInstance, num_topics: u8) -> Result<(), VMExecutionError> {
        if instance.is_static {
            return Err(VMExecutionError::StaticViolation(instance.get_instr()));
        }
        let len = instance.pop_stack()?;
        let ost = instance.pop_stack()?;
        let data = instance.memory.mload8(ost, len);
        let mut topics = Vec::new();
        for _ in 0..num_topics {
            let topic = instance.pop_stack()?;
            topics.push(topic);
        }
        let log = IonicLog {
            address: instance.address,
            topics,
            data,
        };
        self.logs.push(log);
        Ok(())
    }

    fn call(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        let out_size = instance.pop_stack()?;
        let out_offset = instance.pop_stack()?;
        let in_size = instance.pop_stack()?;
        let in_offset = instance.pop_stack()?;
        let value = instance.pop_stack()?;
        let to: IonicAddr = instance.pop_stack()?.into();
        let gas = instance.pop_stack()?;

        let call_frame = CallFrame {
            gas: gas.as_u64(),
            recepient: to,
            value,
            in_offset,
            in_size,
            out_offset,
            out_size,
            code_addr: to,
            ctx_addr: to,
            caller: instance.address,
            call_value: value,
            is_static: false,
            transfer_value: true,
        };
        self.perform_call(instance, blockchain, call_frame)
    }

    fn callcode(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        let out_size = instance.pop_stack()?;
        let out_offset = instance.pop_stack()?;
        let in_size = instance.pop_stack()?;
        let in_offset = instance.pop_stack()?;
        let value = instance.pop_stack()?;
        let to: IonicAddr = instance.pop_stack()?.into();
        let gas = instance.pop_stack()?;

        let call_frame = CallFrame {
            gas: gas.as_u64(),
            recepient: to,
            value,
            in_offset,
            in_size,
            out_offset,
            out_size,
            code_addr: to,
            ctx_addr: instance.address,
            caller: instance.address,
            call_value: value,
            is_static: false,
            transfer_value: false,
        };
        self.perform_call(instance, blockchain, call_frame)
    }
    fn ret(&mut self, instance: &mut VMInstance) -> Result<(), VMExecutionError> {
        let len = instance.pop_stack()?;
        let offset = instance.pop_stack()?;
        instance.return_value = instance.memory.mload8(offset, len);
        instance.stopped = true;
        Ok(())
    }
    fn delegatecall(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        let out_size = instance.pop_stack()?;
        let out_offset = instance.pop_stack()?;
        let in_size = instance.pop_stack()?;
        let in_offset = instance.pop_stack()?;
        let to: IonicAddr = instance.pop_stack()?.into();
        let gas = instance.pop_stack()?;

        let call_frame = CallFrame {
            gas: gas.as_u64(),
            recepient: to,
            value: u256::ZERO,
            in_offset,
            in_size,
            out_offset,
            out_size,
            code_addr: to,
            ctx_addr: to,
            caller: instance.address,
            call_value: u256::ZERO,
            is_static: true,
            transfer_value: false,
        };

        self.perform_call(instance, blockchain, call_frame)
    }
    fn create(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        let size = instance.pop_stack()?;
        let offset = instance.pop_stack()?;
        let value = instance.pop_stack()?;
        let init_code = instance.memory.mload8(offset, size);
        self.create_contract(instance, value, init_code, None, blockchain)
    }
    fn create2(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        let salt = instance.pop_stack()?;
        let len = instance.pop_stack()?;
        let offset = instance.pop_stack()?;
        let value = instance.pop_stack()?;
        let init_code = instance.memory.mload8(offset, len);
        self.create_contract(instance, value, init_code, Some(salt), blockchain)
    }
    fn staticcall(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        let out_size = instance.pop_stack()?;
        let out_offset = instance.pop_stack()?;
        let in_size = instance.pop_stack()?;
        let in_offset = instance.pop_stack()?;
        let to: IonicAddr = instance.pop_stack()?.into();
        let gas = instance.pop_stack()?;

        let call_frame = CallFrame {
            gas: gas.as_u64(),
            recepient: to,
            value: u256::ZERO,
            in_offset,
            in_size,
            out_offset,
            out_size,
            code_addr: to,
            ctx_addr: to,
            caller: instance.address,
            call_value: u256::ZERO,
            is_static: true,
            transfer_value: false,
        };

        self.perform_call(instance, blockchain, call_frame)
    }
    fn revert(&mut self, instance: &mut VMInstance) -> Result<(), VMExecutionError> {
        let size = instance.pop_stack()?;
        let offset = instance.pop_stack()?;
        instance.return_value = instance.memory.mload8(offset, size);
        instance.reverted = true;
        instance.stopped = true;
        Ok(())
    }
    fn selfdestruct(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        if instance.is_static {
            return Err(VMExecutionError::StaticViolation(instance.get_instr()));
        }

        let beneficiary: IonicAddr = instance.pop_stack()?.into();
        let bal = blockchain
            .get_balance(&instance.address)
            .unwrap_or_default();

        if bal > 0 {
            if !blockchain.transfer(&instance.address, &beneficiary, bal) {
                return Err(VMExecutionError::UnknownAccount);
            }
        }

        blockchain.selfdestruct(&instance.address);
        instance.stopped = true;
        Ok(())
    }

    fn perform_call(
        &mut self,
        instance: &mut VMInstance,
        blockchain: &mut Blockchain,
        frame: CallFrame,
    ) -> Result<(), VMExecutionError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            instance.stack.push(u256::ZERO);
            return Ok(());
        }
        if instance.is_static && frame.value > 0 {
            return Err(VMExecutionError::StaticViolation(instance.get_instr()));
        }
        let value = frame.value;
        let call_value = frame.call_value;

        if value > 0 {
            if blockchain
                .get_balance(&instance.address)
                .unwrap_or_default()
                < value
            {
                instance.stack.push(u256::ZERO);
                return Ok(());
            }
            if frame.transfer_value {
                let _ = blockchain.transfer(&instance.address, &frame.recepient, value);
            }
        }

        let code = blockchain.get_code(&frame.code_addr).unwrap_or_default();

        if code.is_empty() {
            instance.stack.push(u256::ONE);
            return Ok(());
        }

        let input = instance.memory.mload8(frame.in_offset, frame.in_size);

        let mut child_instance = VMInstance {
            address: frame.ctx_addr,
            origin: instance.origin,
            caller: frame.caller,
            call_value,
            nonce: instance.nonce,
            gas_price: instance.gas_price,
            instructions: Self::parse(&code)?,
            calldata: input,
            code: code,
            is_static: frame.is_static,
            state_snapshot: blockchain.journal.record(),
            vm_snapshot: self.vm_journal.record(&self.logs),
            ..Default::default()
        };

        let run_result = self.run(&mut child_instance, blockchain);
        self.gas_used += child_instance.gas_used;
        instance.return_data = child_instance.return_value.clone();
        match run_result {
            Ok(()) if !child_instance.reverted => {
                instance.memory.write_padded(
                    frame.out_offset,
                    frame.out_size,
                    u256::ZERO,
                    &child_instance.return_value,
                );
                instance.stack.push(u256::ONE);
                blockchain.apply();
                self.vm_journal.commit();
            }
            _ => {
                blockchain.revert(child_instance.state_snapshot);
                self.vm_journal.revert(
                    child_instance.vm_snapshot,
                    &mut self.logs,
                    &mut self.tstorage,
                );
                if frame.transfer_value && value > 0 {
                    instance.stack.push(u256::ZERO);
                }
            }
        }
        Ok(())
    }

    fn create_contract(
        &mut self,
        instance: &mut VMInstance,
        value: u256,
        init_code: Vec<u8>,
        salt: Option<u256>,
        blockchain: &mut Blockchain,
    ) -> Result<(), VMExecutionError> {
        if instance.is_static {
            return Err(VMExecutionError::StaticViolation(instance.get_instr()));
        }

        if self.call_depth >= MAX_CALL_DEPTH {
            instance.stack.push(u256::ZERO);
            return Ok(());
        }

        let sender = instance.address;

        if blockchain.get_balance(&sender).unwrap_or_default() < value {
            instance.stack.push(u256::ZERO);
            return Ok(());
        }

        let new_addr: IonicAddr = if let Some(salt) = salt {
            let code_hash = blake3::hash(&init_code);
            let mut buf = Vec::with_capacity(1 + 20 + 32 + 32);
            buf.push(0xff);
            buf.extend_from_slice(sender.as_ref());
            buf.extend_from_slice(&salt.to_le_bytes());
            buf.extend_from_slice(code_hash.as_bytes());
            let h = blake3::hash(&buf).as_bytes().to_owned();
            h.into()
        } else {
            Self::derive_contract_addr(sender, instance.nonce)
        };

        if value > 0 {
            if !blockchain.transfer(&sender, &new_addr, value) {
                return Err(VMExecutionError::UnknownAccount);
            }
        }

        let mut child_instance = VMInstance {
            address: new_addr,
            origin: instance.origin,
            caller: sender,
            call_value: value,
            nonce: instance.nonce,
            gas_price: instance.gas_price,
            is_static: false,
            instructions: Self::parse(&init_code)?,
            code: init_code,
            state_snapshot: blockchain.journal.record(),
            vm_snapshot: self.vm_journal.record(&self.logs),
            ..Default::default()
        };
        let run_result = self.run(&mut child_instance, blockchain);
        self.gas_used += child_instance.gas_used;

        match run_result {
            Ok(()) if !child_instance.reverted => {
                blockchain.set_code(&new_addr, child_instance.return_value.clone());
                instance.stack.push(new_addr.into());
                blockchain.apply();
                self.vm_journal.commit();
            }
            _ => {
                blockchain.revert(child_instance.state_snapshot);
                self.vm_journal.revert(
                    child_instance.vm_snapshot,
                    &mut self.logs,
                    &mut self.tstorage,
                );
                instance.return_data = child_instance.return_value.clone();
                instance.stack.push(u256::ZERO);
            }
        }

        Ok(())
    }

    fn derive_contract_addr(sender: IonicAddr, nonce: u64) -> IonicAddr {
        let mut buf = Vec::new();
        buf.extend_from_slice(sender.as_ref());
        buf.extend_from_slice(&nonce.to_le_bytes());
        let h = blake3::hash(&buf).as_bytes().to_owned();
        h.into()
    }
}
impl VMInstance {
    fn pop_stack(&mut self) -> Result<u256, VMExecutionError> {
        let Some(val) = self.stack.pop() else {
            return Err(VMExecutionError::EmptyStack(self.pc, self.get_instr()));
        };
        Ok(val)
    }

    fn eval_unary(&mut self, opcode: &IonicOpcode) -> Result<(), VMExecutionError> {
        let a = self.pop_stack()?;
        match opcode {
            IonicOpcode::ISZERO => self.stack.push((a == 0).as_u256()),
            IonicOpcode::NOT => self.stack.push(a.not()),
            _ => unreachable!("Not an Unary Operation"),
        }
        Ok(())
    }

    fn eval_ternary(&mut self, opcode: &IonicOpcode) -> Result<(), VMExecutionError> {
        let c = self.pop_stack()?;
        let b = self.pop_stack()?;
        let a = self.pop_stack()?;
        let Some(addition) = (match opcode {
            IonicOpcode::ADDMOD => a.checked_add(b),
            IonicOpcode::MULMOD => a.checked_mul(b),
            _ => unreachable!("Not a Turnary Operation"),
        }) else {
            return Err(VMExecutionError::OperationOverflow(self.pc, *opcode, a, b));
        };
        let Some(rem) = addition.checked_rem(c) else {
            return Err(VMExecutionError::OperationOverflow(
                self.pc,
                IonicOpcode::MOD,
                addition,
                c,
            ));
        };
        self.stack.push(rem);
        Ok(())
    }

    fn eval_binary(&mut self, opcode: &IonicOpcode) -> Result<(), VMExecutionError> {
        use IonicOpcode::*;
        let b = self.pop_stack()?;
        let a = self.pop_stack()?;
        let result_maybe = match opcode {
            ADD => a.checked_add(b),
            MUL => a.checked_mul(b),
            SUB => a.checked_sub(b),
            DIV => a.checked_div(b),
            SDIV => {
                let sa = a.as_i256();
                let sb = b.as_i256();
                sa.checked_div(sb).map(|x| x.as_u256())
            }
            MOD => a.checked_rem(b),
            SMOD => {
                let sa = a.as_i256();
                let sb = b.as_i256();
                sa.checked_rem(sb).map(|x| x.as_u256())
            }
            EXP => {
                let exp_bytes = exponent_bytes(b);
                let mut result = 1.as_u256();
                let mut iteration = 0.as_u256();
                while iteration < b {
                    let Some(c) = result.checked_mul(a) else {
                        return Err(VMExecutionError::OperationOverflow(self.pc, *opcode, a, b));
                    };
                    result = c;
                    iteration += 1;
                }
                self.gas_used += 10 + (50 * exp_bytes);
                Some(result)
            }
            SIGNEXTEND => {
                if b >= 31 {
                    Some(a)
                } else {
                    let byte_index = b.as_u32() as usize;
                    let sign_bit_pos = 255 - (byte_index * 8);
                    let mask: u256 = if sign_bit_pos == 255 {
                        u256::MAX
                    } else {
                        (u256::ONE << (sign_bit_pos + 1)) - u256::ONE
                    };
                    let sign = (a >> sign_bit_pos) & u256::ONE;
                    if sign == u256::ONE {
                        Some(a | !mask)
                    } else {
                        Some(a & mask)
                    }
                }
            }
            LT => Some((a < b).as_u256()),
            GT => Some((a > b).as_u256()),
            SLT => {
                let sa = a.as_i256();
                let sb = b.as_i256();
                Some((sa < sb).as_u256())
            }
            SGT => {
                let sa = a.as_i256();
                let sb = b.as_i256();
                Some((sa > sb).as_u256())
            }
            EQ => Some((a == b).as_u256()),
            AND => Some(a & b),
            OR => Some(a | b),
            XOR => Some(a ^ b),
            BYTE => {
                if b > 32 {
                    None
                } else {
                    Some((a >> (248 - b * 8)) & 0xFF)
                }
            }
            SHL => {
                if b > 256 {
                    None
                } else {
                    a.checked_shl(b.as_u32())
                }
            }
            SHR => {
                if b > 256 {
                    None
                } else {
                    a.checked_shr(b.as_u32())
                }
            }
            SAR => {
                let negative = (a >> 255u32) & u256::ONE == u256::ONE;
                if b >= 256 {
                    if negative {
                        Some(u256::MAX)
                    } else {
                        Some(u256::ZERO)
                    }
                } else {
                    let shift = b.as_u32();
                    if shift == 0 {
                        Some(a)
                    } else {
                        if negative {
                            let sign_fill: u256 = u256::MAX << (256 - shift);
                            Some((a >> shift) | sign_fill)
                        } else {
                            Some(a >> shift)
                        }
                    }
                }
            }
            _ => unreachable!("Not a Binary Operation"),
        };
        if let Some(result) = result_maybe {
            self.stack.push(result);
        } else {
            return Err(VMExecutionError::OperationOverflow(self.pc, *opcode, a, b));
        }
        Ok(())
    }

    fn balance(
        &mut self,
        addr: IonicAddr,
        blockchian: &Blockchain,
    ) -> Result<(), VMExecutionError> {
        let balance = blockchian.get_balance(&addr).unwrap_or_default();
        self.stack.push(balance.as_u256());
        Ok(())
    }

    fn eval_hash(&mut self) -> Result<(), VMExecutionError> {
        let len = self.pop_stack()?;
        let ost = self.pop_stack()?;
        let data = self.memory.mload8(ost, len);
        let hash: IonicHash = blake3::hash(&data).into();
        self.stack.push(hash.into());
        let data_size_words = (len + 31 / 32).as_u64();
        let gas_cost = 30 + 6 * data_size_words + 3;
        self.gas_used += gas_cost;
        Ok(())
    }

    fn jump(&mut self) -> Result<(), VMExecutionError> {
        let dest = self.pop_stack()?.as_usize();
        if self.instructions[dest].opcode != IonicOpcode::JUMPDEST {
            return Err(VMExecutionError::InvalidJumpDest(dest));
        }
        self.pc = dest;
        Ok(())
    }

    fn mcpy(&mut self) -> Result<(), VMExecutionError> {
        let len = self.pop_stack()?;
        let ost = self.pop_stack()?;
        let destost = self.pop_stack()?;
        let cost = self.memory.expantion_cost(destost, len);
        self.memory.mcopy(ost, len, destost);
        self.gas_used += cost;
        Ok(())
    }

    fn mstore8(&mut self) -> Result<(), VMExecutionError> {
        let value = self.pop_stack()?;
        let offset = self.pop_stack()?;
        let cost = self.memory.expantion_cost(offset, 1.as_u256());
        self.memory.msotre8(offset, value);
        self.gas_used += cost;
        Ok(())
    }

    fn mstore(&mut self) -> Result<(), VMExecutionError> {
        let value = self.pop_stack()?;
        let offset = self.pop_stack()?;
        let cost = self.memory.expantion_cost(offset, 32.as_u256());
        self.memory.mstore(offset, value);
        self.gas_used += cost;
        Ok(())
    }

    fn mload(&mut self) -> Result<(), VMExecutionError> {
        let offset = self.pop_stack()?;
        let value = self.memory.mload(offset);
        self.stack.push(value);
        Ok(())
    }

    fn jumpi(&mut self) -> Result<(), VMExecutionError> {
        let cond = self.pop_stack()?;
        if cond > 0 {
            self.jump()?;
        }
        Ok(())
    }

    fn pc(&mut self) {
        self.stack.push(self.pc.as_u256());
    }

    fn gas(&mut self) {
        self.stack.push(self.gas_used.as_u256());
    }

    fn address(&mut self, sender: IonicAddr) {
        self.stack.push(sender.into());
    }

    fn push(&mut self, val: u256) {
        self.stack.push(val);
    }

    fn dup(&mut self, nth: u8) {
        let last = self.stack.len() - nth as usize;
        let duped = self.stack[last];
        self.stack.push(duped);
    }

    fn swap(&mut self, nth: u8) {
        let nth = nth as usize;
        let last = self.stack.len() - 1;
        let swapable = self.stack[last];
        let target = self.stack[last - nth];
        self.stack[last - nth] = swapable;
        self.stack[last] = target;
    }

    fn pop(&mut self) -> Result<(), VMExecutionError> {
        self.pop_stack()?;
        Ok(())
    }

    fn get_instr(&self) -> IonicInstr {
        self.instructions[self.pc]
    }
}

fn exponent_bytes(exp: u256) -> u64 {
    let zero_bits = exp.trailing_zeros();
    32 - (zero_bits / 8) as u64
}

#[derive(Debug)]
pub enum VMExecutionError {
    IllegalInstruction(u8),
    InvalidSize,
    SyntaxError(IonicInstr),
    InvalidJumpDest(usize),
    TransiantKeyNotFound(u256),
    OperationOverflow(usize, IonicOpcode, u256, u256),
    EmptyStack(usize, IonicInstr),
    StaticViolation(IonicInstr),
    UnknownAccount,
    CallDepthExceeded,
    OutOfGas,
    InvalidCall,
    SelfDestruct,
}

impl From<TransactionError> for VMExecutionError {
    fn from(_: TransactionError) -> Self {
        Self::UnknownAccount
    }
}

impl Display for VMExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxError(opr) => write!(f, "Syntax Error at ({})", opr),
            Self::IllegalInstruction(val) => write!(f, "Illegal Instruction: ({val})"),
            Self::TransiantKeyNotFound(key) => {
                write!(f, "Transiant Storage dose not have a key: {key}")
            }
            Self::InvalidSize => write!(f, "Invalid Size"),
            Self::InvalidJumpDest(pc) => write!(f, "Invalid Jump Dest: expected JUMPDEST at ${pc}"),
            Self::OperationOverflow(pc, instr, a, b) => {
                write!(f, "Operation Overflow at ${pc}: {instr} {a} {b}")
            }
            Self::EmptyStack(pc, instr) => {
                write!(
                    f,
                    "Empty Stack: Can not pop data out of stack at ${pc}: {instr}"
                )
            }
            Self::UnknownAccount => write!(f, "Can not find Account"),
            Self::StaticViolation(instr) => write!(
                f,
                "Static Violation: Invalid Instruction ({instr}) for static call"
            ),
            Self::CallDepthExceeded => write!(f, "Call Depth Exceeds MaxCallDepth"),
            Self::OutOfGas => write!(f, "Out Of Gas"),
            Self::InvalidCall => write!(f, "Invalid Call"),
            Self::SelfDestruct => write!(f, "Contract Set For Self Destruct"),
        }
    }
}
impl Error for VMExecutionError {}
