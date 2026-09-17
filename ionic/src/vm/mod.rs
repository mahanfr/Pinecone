pub mod instructions;
use std::{cmp, error::Error, fmt::Display};

use ark_ff::Zero;

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
                IonicInstr::POP => {
                    let _ = self.stack.pop();
                }
                IonicInstr::DUP => {
                    let Some(a) = self.stack.last() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    self.stack.push(*a);
                }
                IonicInstr::SWAP => {
                    let Some(b) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    self.stack.push(b);
                    self.stack.push(a);
                }
                IonicInstr::ROT => {
                    let Some(c) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(b) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    self.stack.push(b);
                    self.stack.push(c);
                    self.stack.push(a);
                }
                IonicInstr::DEPT => {
                    self.stack.push(self.stack.len() as u128);
                }
                IonicInstr::PC => {
                    self.stack.push(pc as u128);
                }
                IonicInstr::INC
                | IonicInstr::DEC
                | IonicInstr::NEG
                | IonicInstr::EXP
                | IonicInstr::ABS
                | IonicInstr::SABS
                | IonicInstr::NOT => {
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let result = Self::eval_unary(pc, instr, a)?;
                    self.stack.push(result);
                }
                IonicInstr::SMOD
                | IonicInstr::SDIV
                | IonicInstr::SMIN
                | IonicInstr::SMAX
                | IonicInstr::SAR => {
                    let Some(b) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let result = Self::eval_signed_binary(pc, instr, a as i128, b as i128)?;
                    self.stack.push(result);
                }
                IonicInstr::ADD
                | IonicInstr::SUB
                | IonicInstr::MUL
                | IonicInstr::DIV
                | IonicInstr::MOD
                | IonicInstr::MIN
                | IonicInstr::MAX
                | IonicInstr::LT
                | IonicInstr::GT
                | IonicInstr::ELT
                | IonicInstr::EGT
                | IonicInstr::EQ
                | IonicInstr::AND
                | IonicInstr::OR
                | IonicInstr::XOR
                | IonicInstr::SHL
                | IonicInstr::SHR => {
                    let Some(b) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let result = Self::eval_binary(pc, instr, a, b)?;
                    self.stack.push(result);
                }
                IonicInstr::BIT => {
                    let Some(b) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    if b > 128 {
                        return Err(VMExecutionError::OutOfBounds(pc, instr, b));
                    }
                    let result = match a & (1u128 << b as u32) > 0 {
                        true => 1,
                        false => 0,
                    };
                    self.stack.push(result);
                }
                _ => unimplemented!("Instruction {} is not implemented yet", instr),
            }
            pc += 1;
        }
        Ok(())
    }

    fn eval_unary(pc: usize, op: IonicInstr, a: u128) -> Result<u128, VMExecutionError> {
        use IonicInstr::*;
        let output = match op {
            INC => a.checked_add(1),
            DEC => a.checked_sub(1),
            NEG => a.checked_neg(),
            EXP => 2u128.checked_pow(a as u32),
            ABS | SABS => (a as i128).checked_abs().map(|x| x as u128),
            NOT => match a > 0 {
                true => Some(0),
                false => Some(1),
            },
            ISZERO => Some(a.is_zero() as u128),
            _ => unreachable!("All operaions are covered"),
        };
        let Some(result) = output else {
            return Err(VMExecutionError::UnaryOverflow(pc, op, a));
        };
        return Ok(result as u128);
    }

    fn eval_signed_binary(
        pc: usize,
        op: IonicInstr,
        a: i128,
        b: i128,
    ) -> Result<u128, VMExecutionError> {
        use IonicInstr::*;
        let output = match op {
            SMOD => a.checked_rem(b),
            SDIV => a.checked_div(b),
            SMIN => Some(a.min(b)),
            SMAX => Some(a.max(b)),
            SAR => match b >= 128 {
                true => Some(0),
                false => a.checked_shr(b as u32),
            },
            _ => unreachable!("All operaions are covered"),
        };
        let Some(result) = output else {
            return Err(VMExecutionError::BinaryOverflow(
                pc, op, a as u128, b as u128,
            ));
        };
        return Ok(result as u128);
    }

    fn eval_binary(pc: usize, op: IonicInstr, a: u128, b: u128) -> Result<u128, VMExecutionError> {
        use IonicInstr::*;
        let output = match op {
            ADD => a.checked_add(b),
            SUB => a.checked_sub(b),
            MUL => a.checked_mul(b),
            DIV => a.checked_div(b),
            MOD => a.checked_rem(b),
            MIN => Some(cmp::min(a, b)),
            MAX => Some(cmp::max(a, b)),
            LT => Some((a < b) as u128),
            GT => Some((a > b) as u128),
            ELT => Some((a <= b) as u128),
            EGT => Some((a >= b) as u128),
            EQ => Some((a == b) as u128),
            AND => Some(a & b),
            OR => Some(a | b),
            XOR => Some(a ^ b),
            SHL => match b >= 128 {
                true => Some(0),
                false => a.checked_shl(b as u32),
            },
            SHR => match b >= 128 {
                true => Some(0),
                false => a.checked_shr(b as u32),
            },
            _ => unreachable!("All operaions are covered"),
        };
        let Some(result) = output else {
            return Err(VMExecutionError::BinaryOverflow(pc, op, a, b));
        };
        return Ok(result);
    }
}

#[derive(Debug)]
pub enum VMExecutionError {
    IllegalInstruction(u8),
    InvalidSize,
    SyntaxError(IonicInstr),
    UnaryOverflow(usize, IonicInstr, u128),
    BinaryOverflow(usize, IonicInstr, u128, u128),
    TernaryOverflow(usize, IonicInstr, u128, u128, u128),
    StackUnderflow(usize, IonicInstr),
    OutOfBounds(usize, IonicInstr, u128),
}

impl Display for VMExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SyntaxError(opr) => write!(f, "Syntax Error at ({})", opr),
            Self::IllegalInstruction(val) => write!(f, "Illegal Instruction: ({val})"),
            Self::InvalidSize => write!(f, "Invalid Size"),
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
