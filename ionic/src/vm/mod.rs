pub mod context;
pub mod instructions;
pub mod logs;
pub mod memory;
pub mod opcodes;
use std::{collections::HashMap, error::Error, fmt::Display, ops::Not};

use ethnum::{AsU256, u256};

use crate::{
    blockchian::Blockchain,
    blocks::Block,
    types::{IonicAddr, IonicHash},
    utils::ToBytes,
    vm::{
        context::ExecutionContext, instructions::IonicInstr, logs::IonicLog, memory::IonicMemory,
        opcodes::IonicOpcode,
    },
};

#[derive(Debug, Clone, Default)]
pub struct VirtualMachine {
    pub stack: Vec<u256>,
    pub code: Vec<IonicInstr>,
    pub memory: IonicMemory,
    pub tstorage: HashMap<u256, u256>,
    pub logs: Vec<IonicLog>,
    pub pc: usize,
    pub gas_used: u64,

    pub ctx: ExecutionContext,
    pub stopped: bool,
    pub reverted: bool,
    pub return_value: Vec<u8>,
    raw_code: Vec<u8>,
    pub call_depth: u32,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(code: &[u8]) -> Result<Vec<IonicInstr>, VMExecutionError> {
        let mut instrs: Vec<IonicInstr> = Vec::new();
        let mut cur = 0;
        while cur < code.len() {
            let instr = IonicInstr::parse(&code, &mut cur)?;
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

    pub fn load(&mut self, code: &[u8]) -> Result<(), VMExecutionError> {
        self.code = Self::parse(code)?;
        self.raw_code = code.to_vec();
        Ok(())
    }

    pub fn eval(
        &mut self,
        tx_index: usize,
        blockchain: &Blockchain,
        block: &Block,
    ) -> Result<(), VMExecutionError> {
        let tx = &block.transactions[tx_index];
        let sender_addr = tx.sender();

        let ctx = ExecutionContext {
            // TODO: derive_contract_addr(&sender_addr, tx.nonce) if none
            address: tx.recepient.unwrap(),
            origin: sender_addr,
            caller: sender_addr,
            call_value: tx.value,
            gas_price: tx.gas_price(block.next_base_fee()).unwrap(),
            calldata: tx.data.clone(),
            return_data: Vec::new(),
            is_static: false,
        };
        self.ctx = ctx;
        self.run(block, blockchain)
    }

    pub fn run(&mut self, block: &Block, blockchain: &Blockchain) -> Result<(), VMExecutionError> {
        use opcodes::IonicOpcode::*;
        while self.pc < self.code.len() {
            let instr = self.code[self.pc];
            match &instr.opcode {
                STOP | INVALID => {
                    break;
                }
                ADD | MUL | SUB | DIV | SDIV | MOD | SMOD | EXP | SIGNEXTEND | LT | GT | SLT
                | SGT | EQ | AND | OR | XOR | BYTE | SHL | SHR | SAR => {
                    self.eval_binary(&instr.opcode)?
                }
                ISZERO | NOT => self.eval_unary(&instr.opcode)?,
                ADDMOD | MULMOD => self.eval_ternary(&instr.opcode)?,
                HASH => self.eval_hash()?,
                ADDRESS => self.address(self.ctx.address),
                BALANCE => {
                    let addr = self.pop_internal()?;
                    self.gas_used += 2100;
                    self.balance(addr.into(), blockchain)?
                }
                SELFBALANCE => self.balance(self.ctx.address.into(), blockchain)?,
                BASEFEE => {
                    self.stack.push(blockchain.base_fee().as_u256());
                }
                GASLIMIT => {
                    self.stack.push(block.header.gas_limit.as_u256());
                }
                GASPRICE => {
                    self.stack.push(self.ctx.gas_price.as_u256());
                }
                NUMBER => {
                    self.stack.push(block.header.index.as_u256());
                }
                TIMESTAMP => {
                    self.stack.push(block.header.timestamp.as_u256());
                }
                COINBASE => {
                    let proposer_addr = IonicAddr::from_pk(&block.header.proposer);
                    self.stack.push(proposer_addr.into());
                }
                BLOCKHASH => {
                    let index = self.pop_internal()?;
                    if let Some(block) = blockchain.get_block(index.as_u64()) {
                        self.stack.push(block.hash().into())
                    } else {
                        self.stack.push(u256::ZERO);
                    }
                }
                ORIGIN => self.address(self.ctx.origin),
                CALLER => self.address(self.ctx.caller),
                CALLVALUE => self.stack.push(self.ctx.call_value.as_u256()),
                CALLDATALOAD => {
                    let off = self.pop_internal()?.as_usize();
                    let calldata: [u8; 32] = self.ctx.calldata[off..off + 32].try_into().unwrap();
                    self.stack.push(u256::from_le_bytes(calldata));
                }
                CALLDATASIZE => {
                    self.stack.push(self.ctx.calldata.len().as_u256());
                }
                CALLDATACOPY => {
                    let len = self.pop_internal()?;
                    let ost = self.pop_internal()?;
                    let destost = self.pop_internal()?;
                    let cost = self.memory.expantion_cost(destost, len);
                    self.memory
                        .write_padded(destost, len, ost, &self.ctx.calldata);
                    self.gas_used += cost;
                }
                CODESIZE => self.stack.push(self.raw_code.len().as_u256()),
                CODECOPY => {
                    let len = self.pop_internal()?;
                    let ost = self.pop_internal()?;
                    let destost = self.pop_internal()?;
                    let cost = self.memory.expantion_cost(destost, len);
                    self.memory.write_padded(destost, len, ost, &self.raw_code);
                    self.gas_used += cost;
                }
                EXTCODESIZE => {
                    let addr = self.pop_internal()?;
                    self.gas_used += 2100;
                    let code = blockchain.code(&addr.into()).unwrap_or_default();
                    self.stack.push(code.len().as_u256());
                }
                EXTCODECOPY => {
                    let len = self.pop_internal()?;
                    let ost = self.pop_internal()?;
                    let destost = self.pop_internal()?;
                    let addr = self.pop_internal()?;
                    self.gas_used += 2100;
                    let code = blockchain.code(&addr.into()).unwrap_or_default();
                    let cost = self.memory.expantion_cost(destost, len);
                    self.memory.write_padded(destost, len, ost, &code);
                    self.gas_used += cost;
                }
                EXTCODEHASH => {
                    let addr = self.pop_internal()?;
                    self.gas_used += 2100;
                    let code = blockchain.code(&addr.into()).unwrap_or_default();
                    let hash: IonicHash = blake3::hash(&code).into();
                    self.stack.push(hash.into());
                }
                RETURNDATASIZE => {
                    self.stack.push(self.ctx.return_data.len().as_u256());
                }
                RETURNDATACOPY => {
                    let len = self.pop_internal()?;
                    let ost = self.pop_internal()?;
                    let destost = self.pop_internal()?;
                    let cost = self.memory.expantion_cost(destost, len);
                    self.memory
                        .write_padded(destost, len, ost, &self.ctx.return_data);
                    self.gas_used += cost;
                }
                CHAINID => {
                    self.stack.push(blockchain.id.as_u256());
                }
                POP => self.pop()?,
                PC => self.pc(),
                GAS => self.gas(),
                PUSH0 | PUSH1 | PUSH2 | PUSH3 | PUSH4 | PUSH5 | PUSH6 | PUSH7 | PUSH8 | PUSH9
                | PUSH10 | PUSH11 | PUSH12 | PUSH13 | PUSH14 | PUSH15 | PUSH16 | PUSH17
                | PUSH18 | PUSH19 | PUSH20 | PUSH21 | PUSH22 | PUSH23 | PUSH24 | PUSH25
                | PUSH26 | PUSH27 | PUSH28 | PUSH29 | PUSH30 | PUSH31 | PUSH32 => {
                    if let Some(imm) = instr.data {
                        self.push(imm);
                    } else if instr.opcode == IonicOpcode::PUSH0 {
                        self.push(u256::ZERO);
                    } else {
                        return Err(VMExecutionError::SyntaxError(instr));
                    }
                }
                DUP1 | DUP2 | DUP3 | DUP4 | DUP5 | DUP6 | DUP7 | DUP8 | DUP9 | DUP10 | DUP11
                | DUP12 | DUP13 | DUP14 | DUP15 | DUP16 => self.dup(
                    instr
                        .opcode
                        .stack_position()
                        .expect("Not A DUP Instruction"),
                ),
                SWAP1 | SWAP2 | SWAP3 | SWAP4 | SWAP5 | SWAP6 | SWAP7 | SWAP8 | SWAP9 | SWAP10
                | SWAP11 | SWAP12 | SWAP13 | SWAP14 | SWAP15 | SWAP16 => {
                    self.swap(
                        instr
                            .opcode
                            .stack_position()
                            .expect("Not a Swap Instruction"),
                    );
                }
                JUMP => {
                    self.jump()?;
                    continue;
                }
                JUMPI => {
                    self.jumpi()?;
                    continue;
                }
                JUMPDEST => (),
                MSTORE => self.mstore()?,
                MSTORE8 => self.mstore8()?,
                MLOAD => self.mload()?,
                MSIZE => self.stack.push(self.memory.size()),
                MCOPY => self.mcpy()?,
                TSTORE => self.tstore()?,
                TLOAD => self.tload()?,
                _ => todo!(),
            }
            self.pc += 1;
            self.gas_used += instr.opcode.gas().unwrap_or_default();
        }
        Ok(())
    }

    fn eval_unary(&mut self, opcode: &IonicOpcode) -> Result<(), VMExecutionError> {
        let a = self.pop_internal()?;
        match opcode {
            IonicOpcode::ISZERO => self.stack.push((a == 0).as_u256()),
            IonicOpcode::NOT => self.stack.push(a.not()),
            _ => unreachable!("Not an Unary Operation"),
        }
        Ok(())
    }

    fn eval_ternary(&mut self, opcode: &IonicOpcode) -> Result<(), VMExecutionError> {
        let c = self.pop_internal()?;
        let b = self.pop_internal()?;
        let a = self.pop_internal()?;
        let Some(addition) = (match opcode {
            IonicOpcode::ADDMOD => a.checked_add(b),
            IonicOpcode::MULMOD => a.checked_mul(b),
            _ => unreachable!("Not a Turnary Operation"),
        }) else {
            return Err(VMExecutionError::BinaryOverflow(self.pc, *opcode, a, b));
        };
        let Some(rem) = addition.checked_rem(c) else {
            return Err(VMExecutionError::BinaryOverflow(
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
        let b = self.pop_internal()?;
        let a = self.pop_internal()?;
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
                        return Err(VMExecutionError::BinaryOverflow(self.pc, *opcode, a, b));
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
            return Err(VMExecutionError::BinaryOverflow(self.pc, *opcode, a, b));
        }
        Ok(())
    }

    fn balance(
        &mut self,
        addr: IonicAddr,
        blockchian: &Blockchain,
    ) -> Result<(), VMExecutionError> {
        let balance = blockchian.balance(&addr.into()).unwrap_or_default();
        self.stack.push(balance.as_u256());
        Ok(())
    }

    fn eval_hash(&mut self) -> Result<(), VMExecutionError> {
        let len = self.pop_internal()?;
        let ost = self.pop_internal()?;
        let data = self.memory.mload8(ost, len);
        let hash: IonicHash = blake3::hash(&data).into();
        self.stack.push(hash.into());
        let data_size_words = (len + 31 / 32).as_u64();
        let gas_cost = 30 + 6 * data_size_words + 3;
        self.gas_used += gas_cost;
        Ok(())
    }

    fn jump(&mut self) -> Result<(), VMExecutionError> {
        let dest = self.pop_internal()?.as_usize();
        if self.code[dest].opcode != IonicOpcode::JUMPDEST {
            return Err(VMExecutionError::InvalidJumpDest(dest));
        }
        self.pc = dest;
        Ok(())
    }

    fn tstore(&mut self) -> Result<(), VMExecutionError> {
        let value = self.pop_internal()?;
        let key_val = self.pop_internal()?;
        self.tstorage.insert(key_val, value);
        Ok(())
    }

    fn tload(&mut self) -> Result<(), VMExecutionError> {
        let key = self.pop_internal()?;
        let Some(value) = self.tstorage.get(&key) else {
            return Err(VMExecutionError::TransiantKeyNotFound(key));
        };
        self.stack.push(*value);
        Ok(())
    }

    fn mcpy(&mut self) -> Result<(), VMExecutionError> {
        let len = self.pop_internal()?;
        let ost = self.pop_internal()?;
        let destost = self.pop_internal()?;
        let cost = self.memory.expantion_cost(destost, len);
        self.memory.mcopy(ost, len, destost);
        self.gas_used += cost;
        Ok(())
    }

    fn mstore8(&mut self) -> Result<(), VMExecutionError> {
        let value = self.pop_internal()?;
        let offset = self.pop_internal()?;
        let cost = self.memory.expantion_cost(offset, 1.as_u256());
        self.memory.msotre8(offset, value);
        self.gas_used += cost;
        Ok(())
    }

    fn mstore(&mut self) -> Result<(), VMExecutionError> {
        let value = self.pop_internal()?;
        let offset = self.pop_internal()?;
        let cost = self.memory.expantion_cost(offset, 32.as_u256());
        self.memory.mstore(offset, value);
        self.gas_used += cost;
        Ok(())
    }

    fn mload(&mut self) -> Result<(), VMExecutionError> {
        let offset = self.pop_internal()?;
        let value = self.memory.mload(offset);
        self.stack.push(value);
        Ok(())
    }

    fn jumpi(&mut self) -> Result<(), VMExecutionError> {
        let cond = self.pop_internal()?;
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
        self.pop_internal()?;
        Ok(())
    }

    fn pop_internal(&mut self) -> Result<u256, VMExecutionError> {
        let Some(val) = self.stack.pop() else {
            return Err(VMExecutionError::EmptyStack(self.pc, self.get_instr()));
        };
        return Ok(val);
    }

    fn get_instr(&self) -> IonicInstr {
        self.code[self.pc]
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
    UnaryOverflow(usize, IonicOpcode, u256),
    BinaryOverflow(usize, IonicOpcode, u256, u256),
    TernaryOverflow(usize, IonicOpcode, u256, u256, u256),
    EmptyStack(usize, IonicInstr),
    OutOfBounds(usize, IonicInstr, u256),
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
            Self::UnaryOverflow(pc, instr, a) => {
                write!(f, "Operation Overflow at ${pc}: {instr} {a}")
            }
            Self::BinaryOverflow(pc, instr, a, b) => {
                write!(f, "Operation Overflow at ${pc}: {instr} {a} {b}")
            }
            Self::TernaryOverflow(pc, instr, a, b, c) => {
                write!(f, "Operation Overflow at ${pc}: {instr} {a} {b} {c}")
            }
            Self::EmptyStack(pc, instr) => {
                write!(
                    f,
                    "Empty Stack: Can not pop data out of stack at ${pc}: {instr}"
                )
            }
            Self::OutOfBounds(pc, instr, a) => {
                write!(f, "Operator Out Of Bounds at ${pc}: {instr} .. ({a})")
            }
        }
    }
}
impl Error for VMExecutionError {}
