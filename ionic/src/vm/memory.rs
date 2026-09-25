use std::fmt::Display;

use ethnum::{AsU256, u256};

#[derive(Debug, Clone)]
pub struct IonicMemory {
    data: Vec<u8>,
}

impl Default for IonicMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl IonicMemory {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn write_padded(&mut self, offset: u256, len: u256, data_offset: u256, data: &[u8]) {
        let offset = offset.as_usize();
        let len = len.as_usize();
        let data_offset = data_offset.as_usize();
        let mut slice = Vec::new();
        let arr = &data[data_offset..data_offset + len];
        slice.extend_from_slice(arr);
        let padded_len = (len + 31) / 32;
        slice.resize(padded_len, 0);
        self.expand_memory_if_needed(offset, len);
        self.data[offset..offset + padded_len].copy_from_slice(&slice);
    }

    pub fn mload(&mut self, offset: u256) -> u256 {
        let offset = offset.as_usize();
        if offset + 32 > self.data.len() {
            return u256::ZERO;
        }
        let data: [u8; 32] = self.data[offset..offset + 32].try_into().unwrap();
        let imm = u256::from_le_bytes(data);
        return imm;
    }

    pub fn mload8(&mut self, offset: u256, len: u256) -> Vec<u8> {
        let offset = offset.as_usize();
        let len = len.as_usize();
        if offset + len > self.data.len() {
            return vec![];
        }
        self.data[offset..offset + len].to_vec()
    }

    pub fn mstore(&mut self, offset: u256, value: u256) {
        let offset = offset.as_usize();
        self.expand_memory_if_needed(offset, 32);

        self.data[offset..offset + 32].copy_from_slice(&value.to_le_bytes());
    }

    pub fn msotre8(&mut self, offset: u256, value: u256) {
        let offset = offset.as_usize();
        self.expand_memory_if_needed(offset, 1);
        self.data[offset] = (value & 0xFF).as_u8();
    }

    pub fn mcopy(&mut self, offset: u256, len: u256, new_offset: u256) {
        let offset = offset.as_usize();
        let len = len.as_usize();
        let new_offset = new_offset.as_usize();

        self.expand_memory_if_needed(new_offset, len);
        let data = self.data[offset..offset + len].to_vec();
        for i in 0..len {
            self.data[new_offset + i] = data[i];
        }
    }

    pub fn mcmp(&mut self, offset1: u256, len1: u256, offset2: u256, len2: u256) -> bool {
        let offset1 = offset1.as_usize();
        let offset2 = offset2.as_usize();
        let len1 = len1.as_usize();
        let len2 = len2.as_usize();
        if len1 != len2 {
            return false;
        }
        for i in 0..len1 {
            if self.data[offset1 + i] != self.data[offset2 + i] {
                return false;
            }
        }
        true
    }

    fn expand_memory_if_needed(&mut self, offset: usize, len: usize) {
        if len == 0 {
            return;
        }
        let end = offset + len;
        if end <= self.data.len() {
            return;
        }

        let new_size = ((end + 31) / 32) * 32;
        self.data.resize(new_size, 0);
    }

    pub fn expantion_cost(&self, offset: u256, len: u256) -> u64 {
        if len == 0 {
            return 0;
        }

        let old_words = (self.data.len() / 32) as u64;
        let new_end = offset + len;
        let new_words = ((new_end + 31) / 32).as_u64();

        if new_words <= old_words {
            return 0;
        }

        Self::memory_gas(new_words) - Self::memory_gas(old_words)
    }

    pub fn memory_gas(words: u64) -> u64 {
        3 * words + ((words * words) / 512)
    }

    pub fn size(&self) -> u256 {
        self.data.len().as_u256()
    }
}

#[derive(Debug)]
pub enum MemoryError {
    OutOfCapacity,
}

impl Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfCapacity => write!(f, "Memory Out Of Capacity"),
        }
    }
}
