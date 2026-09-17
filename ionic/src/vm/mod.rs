pub mod instructions;
use std::{error::Error, fmt::Display};

use crate::{blockchian::Blockchain, vm::instructions::IonicInstr};

pub type IonicWord = u128;

pub struct VirtualMachine {
    pub stack: Vec<u128>,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    fn take_imm(code: &[u8], cur: &mut usize) -> Result<u128, VMExecutionError> {
        if *cur + 16 > code.len() {
            return Err(VMExecutionError::InvalidSize);
        }
        let arr: [u8; 16] = code[*cur..*cur + 16].try_into().unwrap();
        let imm = u128::from_le_bytes(arr);
        *cur += 16;
        return Ok(imm);
    }

    pub fn parse(code: Vec<u8>) -> Result<Vec<IonicInstr>, VMExecutionError> {
        let mut instrs: Vec<IonicInstr> = Vec::new();
        let mut cur = 0;
        while cur < code.len() {
            let mnemonic = IonicInstr::from_byte(code[cur])?;
            cur += 1;
            match mnemonic {
                IonicInstr::PUSH(_) => {
                    instrs.push(IonicInstr::PUSH(Self::take_imm(&code, &mut cur)?))
                }
                _ => instrs.push(mnemonic),
            }
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
        let mut pc = 0;
        while pc < instrs.len() {
            let instr = instrs[pc];
            match instr {
                IonicInstr::PUSH(imm) => {
                    self.stack.push(imm);
                }
                IonicInstr::ADD => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(a + b);
                }
                _ => unimplemented!("Instruction {} is not implemented yet", instr),
            }
            pc += 1;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum VMExecutionError {
    IllegalInstruction(u8),
    InvalidSize,
    SyntaxError(IonicInstr),
}

impl Display for VMExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxError(opr) => write!(f, "Syntax Error at ({})", opr),
            Self::IllegalInstruction(val) => write!(f, "Illegal Instruction: ({val})"),
            Self::InvalidSize => write!(f, "Invalid Size"),
        }
    }
}
impl Error for VMExecutionError {}

#[cfg(test)]
mod tests {
    use crate::{
        blockchian::Blockchain,
        vm::{VirtualMachine, instructions::IonicInstr},
    };

    #[test]
    pub fn basic_vm_test() {
        let blockchian = Blockchain::new(0);
        let mut vm = VirtualMachine::new();
        let instrs = vec![IonicInstr::PUSH(34), IonicInstr::PUSH(33), IonicInstr::ADD];
        assert!(
            vm.run(instrs, &blockchian).is_ok(),
            "Instructions return an error"
        );
        assert_eq!(
            vm.stack.last(),
            Some(67).as_ref(),
            "value is not on the stack or the result is undisiarable"
        );
    }

    #[test]
    pub fn basic_parsing_and_compiling_test() {
        let instrs = vec![IonicInstr::PUSH(34), IonicInstr::PUSH(33), IonicInstr::ADD];
        let code = VirtualMachine::compile(instrs.clone());
        let parsed_instrs = VirtualMachine::parse(code).expect("code did not parsed correctly");
        assert_eq!(instrs, parsed_instrs);
    }
}
