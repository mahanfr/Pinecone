pub mod instructions;
pub mod opcodes;
pub mod memory;
use std::{cmp, error::Error, fmt::Display};

use ethnum::{AsI256, AsU256, i256, u256};

use crate::{blockchian::Blockchain, utils::ToBytes, vm::instructions::IonicInstr};

pub struct VirtualMachine {
    pub stack: Vec<u256>,
    pub pc: usize,
    pub gas_used: u64,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self { stack: Vec::new(), pc: 0, gas_used: 0 }
    }

    pub fn parse(code: Vec<u8>) -> Result<Vec<IonicInstr>, VMExecutionError> {
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

    pub fn execute(
        &mut self,
        code: Vec<u8>,
        blockchian: &Blockchain,
    ) -> Result<(), VMExecutionError> {
        let instrs = Self::parse(code)?;
        self.run(instrs, blockchian)?;
        Ok(())
    }

    pub fn run(
        &mut self,
        instrs: Vec<IonicInstr>,
        _blockchian: &Blockchain,
    ) -> Result<(), VMExecutionError> {
        Ok(())
    }

}

#[derive(Debug)]
pub enum VMExecutionError {
    IllegalInstruction(u8),
    InvalidSize,
    SyntaxError(IonicInstr),
    InvalidPC(u64),
    UnaryOverflow(usize, IonicInstr, u256),
    BinaryOverflow(usize, IonicInstr, u256, u256),
    TernaryOverflow(usize, IonicInstr, u256, u256, u256),
    StackUnderflow(usize, IonicInstr),
    OutOfBounds(usize, IonicInstr, u256),
}

impl Display for VMExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxError(opr) => write!(f, "Syntax Error at ({})", opr),
            Self::IllegalInstruction(val) => write!(f, "Illegal Instruction: ({val})"),
            Self::InvalidSize => write!(f, "Invalid Size"),
            Self::InvalidPC(pc) => write!(f, "Invalid PC: there is no instruction at ${pc}"),
            Self::UnaryOverflow(pc, instr, a) => {
                write!(f, "Operation Overflow at ${pc}: {instr} {a}")
            }
            Self::BinaryOverflow(pc, instr, a, b) => {
                write!(f, "Operation Overflow at ${pc}: {instr} {a} {b}")
            }
            Self::TernaryOverflow(pc, instr, a, b, c) => {
                write!(f, "Operation Overflow at ${pc}: {instr} {a} {b} {c}")
            }
            Self::StackUnderflow(pc, instr) => {
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
