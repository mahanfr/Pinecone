pub mod instructions;
pub mod memory;
use std::{cmp, error::Error, fmt::Display};

use ethnum::{AsI256, AsU256, i256, u256};

use crate::{blockchian::Blockchain, vm::instructions::IonicInstr};

pub struct VirtualMachine {
    pub stack: Vec<u256>,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    fn take_imm(code: &[u8], cur: &mut usize) -> Result<u256, VMExecutionError> {
        if *cur + 32 > code.len() {
            return Err(VMExecutionError::InvalidSize);
        }
        let arr: [u8; 32] = code[*cur..*cur + 32].try_into().unwrap();
        let imm = u256::from_le_bytes(arr);
        *cur += 32;
        return Ok(imm);
    }

    fn take_pc(code: &[u8], cur: &mut usize) -> Result<u64, VMExecutionError> {
        if *cur + 8 > code.len() {
            return Err(VMExecutionError::InvalidSize);
        }
        let arr: [u8; 8] = code[*cur..*cur + 8].try_into().unwrap();
        let imm = u64::from_le_bytes(arr);
        *cur += 8;
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
                IonicInstr::JUMP(_) => {
                    instrs.push(IonicInstr::JUMP(Self::take_pc(&code, &mut cur)?))
                }
                IonicInstr::JUMPC(_) => {
                    instrs.push(IonicInstr::JUMPC(Self::take_pc(&code, &mut cur)?))
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
    ) -> Result<u64, VMExecutionError> {
        let mut pc = 0;
        let mut gas = 0;
        while pc < instrs.len() {
            let instr = instrs[pc];
            match instr {
                IonicInstr::STOP => {
                    break;
                }
                IonicInstr::NOP => {
                    continue;
                }
                IonicInstr::GAS => {
                    self.stack.push(gas.as_u256());
                }
                IonicInstr::JUMP(new_pc) => {
                    if new_pc < instrs.len() as u64 {
                        pc = new_pc as usize;
                        continue;
                    } else {
                        return Err(VMExecutionError::InvalidPC(new_pc));
                    }
                }
                IonicInstr::JUMPC(new_pc) => {
                    if new_pc >= instrs.len() as u64 {
                        return Err(VMExecutionError::InvalidPC(new_pc));
                    }
                    let Some(cond) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    if cond > 0 {
                        pc = new_pc as usize;
                        continue;
                    }
                }
                IonicInstr::INVALID => {
                    return Ok(gas);
                }
                IonicInstr::PUSH(imm) => {
                    self.stack.push(imm);
                    gas += 1;
                }
                IonicInstr::POP => {
                    let _ = self.stack.pop();
                    gas += 1;
                }
                IonicInstr::DUP => {
                    let Some(a) = self.stack.last() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    self.stack.push(*a);
                    gas += 1;
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
                    gas += 1;
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
                    gas += 1;
                }
                IonicInstr::DEPT => {
                    self.stack.push((self.stack.len() as u64).into());
                    gas += 1;
                }
                IonicInstr::PC => {
                    self.stack.push((pc as u64).into());
                    gas += 1;
                }
                IonicInstr::ADDMOD | IonicInstr::MULMOD => {
                    let Some(c) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(b) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let result = Self::eval_ternary(pc, instr, a, b, c)?;
                    self.stack.push(result);
                    gas += 2;
                }
                IonicInstr::INC
                | IonicInstr::DEC
                | IonicInstr::NEG
                | IonicInstr::EXP
                | IonicInstr::ABS
                | IonicInstr::SABS
                | IonicInstr::NOT
                | IonicInstr::ISZERO => {
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let result = Self::eval_unary(pc, instr, a)?;
                    self.stack.push(result);
                    gas += 2;
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
                    let result = Self::eval_signed_binary(pc, instr, a.as_i256(), b.as_i256())?;
                    self.stack.push(result);
                    gas += 2;
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
                    gas += 2;
                }
                IonicInstr::BIT => {
                    let Some(b) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    if b > 256 {
                        return Err(VMExecutionError::OutOfBounds(pc, instr, b));
                    }
                    let result = match a & (1.as_u256() << b.as_u32()) > 0 {
                        true => 1.as_u256(),
                        false => 0.as_u256(),
                    };
                    self.stack.push(result);
                    gas += 1;
                }
                IonicInstr::POPCOUNT => {
                    let Some(a) = self.stack.pop() else {
                        return Err(VMExecutionError::StackUnderflow(pc, instr));
                    };
                    self.stack.push(a.count_ones().as_u256());
                    gas += 1;
                }
                IonicInstr::MLOAD => todo!("Implement heap first"),
                IonicInstr::MSTORE => todo!("Implement heap first"),
                IonicInstr::EXT => unimplemented!("If we reached 256 instructions use this"),
            }
            pc += 1;
        }
        Ok(gas)
    }

    fn eval_ternary(
        pc: usize,
        op: IonicInstr,
        a: u256,
        b: u256,
        c: u256,
    ) -> Result<u256, VMExecutionError> {
        use IonicInstr::*;
        let output = match op {
            ADDMOD => a.checked_add(b).map(|x| x.checked_rem(c)),
            MULMOD => a.checked_mul(b).map(|x| x.checked_rem(c)),
            _ => unreachable!("All operaions are covered"),
        };
        let Some(Some(result)) = output else {
            return Err(VMExecutionError::TernaryOverflow(pc, op, a, b, c));
        };
        return Ok(result);
    }

    fn eval_unary(pc: usize, op: IonicInstr, a: u256) -> Result<u256, VMExecutionError> {
        use IonicInstr::*;
        let output = match op {
            INC => a.checked_add(1.as_u256()),
            DEC => a.checked_sub(1.as_u256()),
            NEG => a.checked_neg(),
            EXP => 2.as_u256().checked_pow(a.as_u32()),
            ABS | SABS => (a.as_i256()).checked_abs().map(|x| x.as_u256()),
            NOT => match a > 0 {
                true => Some(0.as_u256()),
                false => Some(1.as_u256()),
            },
            ISZERO => Some((a == 0).as_u256()),
            _ => unreachable!("All operaions are covered"),
        };
        let Some(result) = output else {
            return Err(VMExecutionError::UnaryOverflow(pc, op, a));
        };
        return Ok(result);
    }

    fn eval_signed_binary(
        pc: usize,
        op: IonicInstr,
        a: i256,
        b: i256,
    ) -> Result<u256, VMExecutionError> {
        use IonicInstr::*;
        let output = match op {
            SMOD => a.checked_rem(b),
            SDIV => a.checked_div(b),
            SMIN => Some(a.min(b)),
            SMAX => Some(a.max(b)),
            SAR => match b >= 256 {
                true => Some(0.as_i256()),
                false => a.checked_shr(b.as_u32()).map(|x| x.as_i256()),
            },
            _ => unreachable!("All operaions are covered"),
        };
        let Some(result) = output else {
            return Err(VMExecutionError::BinaryOverflow(
                pc,
                op,
                a.as_u256(),
                b.as_u256(),
            ));
        };
        return Ok(result.as_u256());
    }

    fn eval_binary(pc: usize, op: IonicInstr, a: u256, b: u256) -> Result<u256, VMExecutionError> {
        use IonicInstr::*;
        let output = match op {
            ADD => a.checked_add(b),
            SUB => a.checked_sub(b),
            MUL => a.checked_mul(b),
            DIV => a.checked_div(b),
            MOD => a.checked_rem(b),
            MIN => Some(cmp::min(a, b)),
            MAX => Some(cmp::max(a, b)),
            LT => Some((a < b).as_u256()),
            GT => Some((a > b).as_u256()),
            ELT => Some((a <= b).as_u256()),
            EGT => Some((a >= b).as_u256()),
            EQ => Some((a == b).as_u256()),
            AND => Some(a & b),
            OR => Some(a | b),
            XOR => Some(a ^ b),
            SHL => match b >= 256 {
                true => Some(0.as_u256()),
                false => a.checked_shl(b.as_u32()),
            },
            SHR => match b >= 256 {
                true => Some(0.as_u256()),
                false => a.checked_shr(b.as_u32()),
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

#[cfg(test)]
mod tests {
    use ethnum::AsU256;

    use crate::{
        blockchian::Blockchain,
        vm::{VirtualMachine, instructions::IonicInstr},
    };

    #[test]
    pub fn basic_vm_test() {
        let blockchian = Blockchain::new(0);
        let mut vm = VirtualMachine::new();
        let instrs = vec![
            IonicInstr::PUSH(34.as_u256()),
            IonicInstr::PUSH(33.as_u256()),
            IonicInstr::ADD,
        ];
        assert!(
            vm.run(instrs, &blockchian).is_ok(),
            "Instructions return an error"
        );
        assert_eq!(
            vm.stack.last(),
            Some(67.as_u256()).as_ref(),
            "value is not on the stack or the result is undisiarable"
        );
    }

    #[test]
    pub fn basic_parsing_and_compiling_test() {
        let instrs = vec![
            IonicInstr::PUSH(34.as_u256()),
            IonicInstr::PUSH(33.as_u256()),
            IonicInstr::ADD,
        ];
        let code = VirtualMachine::compile(instrs.clone());
        let parsed_instrs = VirtualMachine::parse(code).expect("code did not parsed correctly");
        assert_eq!(instrs, parsed_instrs);
    }
}
