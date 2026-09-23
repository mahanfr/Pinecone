use std::fmt;

use ethnum::{AsU256, u256};

#[derive(Debug)]
pub enum CodecError {
    UnexpectedEof,
    InvalidTag(u8),
    VarintOverflow,
    TrailingBytes,
    Value(String),
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodecError::UnexpectedEof => write!(f, "unexpected end of input"),
            CodecError::InvalidTag(tag) => write!(f, "invalid node tag: {}", tag),
            CodecError::VarintOverflow => write!(f, "varint overflowed u64"),
            CodecError::TrailingBytes => write!(f, "trailing bytes after decoding root"),
            CodecError::Value(msg) => write!(f, "failed to decode value: {}", msg),
        }
    }
}

impl std::error::Error for CodecError {}

pub fn write_uvarint(buf: &mut Vec<u8>, mut value: u256) {
    loop {
        let mut byte = (value & 0x7f).as_u8();
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

pub fn read_uvarint(bytes: &[u8], cursor: &mut usize) -> Result<u256, CodecError> {
    let mut result: u256 = u256::ZERO;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*cursor).ok_or(CodecError::UnexpectedEof)?;
        *cursor += 1;
        result |= ((byte & 0x7f).as_u256()) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return Err(CodecError::VarintOverflow);
        }
    }
    Ok(result)
}

pub fn read_slice<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    len: usize,
) -> Result<&'a [u8], CodecError> {
    let end = cursor.checked_add(len).ok_or(CodecError::UnexpectedEof)?;
    let slice = bytes.get(*cursor..end).ok_or(CodecError::UnexpectedEof)?;
    *cursor = end;
    Ok(slice)
}
