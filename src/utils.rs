pub trait ToBytes {
    fn to_bytes(&self) -> Vec<u8>;
}

impl ToBytes for u32 {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec() // or to_be_bytes()
    }
}
