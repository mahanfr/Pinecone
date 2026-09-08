use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD_INDIFFERENT};

pub trait ToBytes {
    fn to_bytes(&self) -> Vec<u8>;
}

impl ToBytes for u32 {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

pub trait FromBytes {
    fn from_bytes(bytes: &[u8]) -> Self;
}

pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock is before Unix epoch")
        .as_secs()
}

#[macro_export]
macro_rules! read_bytes {
    ($bytes:expr, $pos:expr, $len:expr) => {{
        let end = $pos + $len;
        if $bytes.len() < end {
            panic!("Buffer too short at offset {}", $pos);
        }
        let slice = &$bytes[$pos..end];
        $pos = end;
        slice
    }};
}

pub struct IonicBase64 {}
impl IonicBase64 {
    pub fn encode<T: AsRef<[u8]>>(input: T) -> String {
        URL_SAFE_NO_PAD_INDIFFERENT.encode(input)
    }
    pub fn decode<T: AsRef<[u8]>>(input: T) -> Vec<u8> {
        URL_SAFE_NO_PAD_INDIFFERENT
            .decode(input)
            .unwrap_or(b"UNPARSABLE".to_vec())
    }
}
