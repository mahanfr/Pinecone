use std::fmt::Display;

use ethnum::{AsU256, u256};

use crate::{
    utils::ToBytes,
    vm::{VMExecutionError, opcodes::IonicOpcode},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IonicInstr {
    pub opcode: IonicOpcode,
    pub data: Option<u256>,
}

impl IonicInstr {
    pub fn parse(code: &[u8], cur: &mut usize) -> Result<Self, VMExecutionError> {
        let opcode_byte = code[*cur];
        let opcode = IonicOpcode::from_byte(opcode_byte)?;
        *cur += 1;
        if !opcode.has_immediate() {
            return Ok(opcode.into());
        }
        let imm_size = opcode.immediate_size();
        if *cur + imm_size <= code.len() {
            return Err(VMExecutionError::InvalidSize);
        }
        let data = match imm_size {
            1 => code[*cur].as_u256(),
            2 => u16::from_le_bytes([code[*cur], code[*cur + 1]]).as_u256(),
            3..=4 => {
                let arr: [u8; 4] = code[*cur..*cur + 4]
                    .try_into()
                    .map_err(|_| VMExecutionError::InvalidSize)?;
                u32::from_le_bytes(arr).as_u256()
            }
            5..=8 => {
                let arr: [u8; 8] = code[*cur..*cur + 8]
                    .try_into()
                    .map_err(|_| VMExecutionError::InvalidSize)?;
                u64::from_le_bytes(arr).as_u256()
            }
            9..=16 => {
                let arr: [u8; 16] = code[*cur..*cur + 16]
                    .try_into()
                    .map_err(|_| VMExecutionError::InvalidSize)?;
                u128::from_le_bytes(arr).as_u256()
            }
            17..=32 => {
                let arr: [u8; 32] = code[*cur..*cur + 32]
                    .try_into()
                    .map_err(|_| VMExecutionError::InvalidSize)?;
                u256::from_le_bytes(arr)
            }
            _ => unreachable!("The immediate value can not be bigger that 32"),
        };

        *cur += imm_size;

        Ok(Self {
            opcode,
            data: Some(data),
        })
    }
}

impl ToBytes for IonicInstr {
    fn to_bytes(&self) -> Vec<u8> {
        if !self.opcode.has_immediate() {
            vec![self.opcode.as_byte()]
        } else {
            let Some(val) = self.data else {
                return vec![IonicOpcode::STOP.as_byte()];
            };
            let mut data = vec![self.opcode.as_byte()];
            let imm = match self.opcode.immediate_size() {
                1 => val.as_u8().to_le_bytes().to_vec(),
                2 => val.as_u16().to_le_bytes().to_vec(),
                3..=4 => val.as_u32().to_le_bytes().to_vec(),
                5..=8 => val.as_u64().to_le_bytes().to_vec(),
                9..=16 => val.as_u128().to_le_bytes().to_vec(),
                17..=32 => val.to_le_bytes().to_vec(),
                _ => unreachable!("The immediate value can not be bigger that 32"),
            };
            data.extend_from_slice(&imm);
            data
        }
    }
}

impl Display for IonicInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.data {
            Some(data) => write!(f, "{} {data}", self.opcode),
            None => self.opcode.fmt(f),
        }
    }
}

impl Into<IonicOpcode> for IonicInstr {
    fn into(self) -> IonicOpcode {
        self.opcode
    }
}

impl From<IonicOpcode> for IonicInstr {
    fn from(opcode: IonicOpcode) -> Self {
        Self { opcode, data: None }
    }
}
