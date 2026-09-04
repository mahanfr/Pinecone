pub trait ToBytes {
    fn to_bytes(&self) -> Vec<u8>;
}

impl ToBytes for u32 {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec() // or to_be_bytes()
    }
}

pub trait FromBytes {
    fn from_bytes(bytes: &[u8]) -> Self;
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
