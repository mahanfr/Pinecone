use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD_INDIFFERENT};

const PINE_ADDR_DOMAIN : &[u8] = b"PINE_ADDR";

pub type PinePK = [u8;32];
pub type PineHash = [u8; 32];
pub type PineAddr = [u8;32];
pub type PineTXSignature = [u8; 64];

pub type PineBlsSigbature = [u8; 96];
pub type PineBlsPk = [u8; 48];

pub fn addr_from_pk(pk: &PinePK) -> PineAddr {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(PINE_ADDR_DOMAIN);
    bytes.extend_from_slice(pk);
    blake3::hash(&bytes).as_bytes().to_owned()
}

pub fn addr_to_string(addr: PineAddr) -> String {
    format!("B64{}", URL_SAFE_NO_PAD_INDIFFERENT.encode(addr))
}
