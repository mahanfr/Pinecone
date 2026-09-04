use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD_INDIFFERENT};

const IONIC_ADDR_DOMAIN: &[u8] = b"IONIC_ADDR";

pub type IonicPK = [u8; 32];
pub type IonicHash = [u8; 32];
pub type IonicAddr = [u8; 32];
pub type IonicTXSignature = [u8; 64];

pub type IonicBlsSigbature = [u8; 96];
pub type IonicBlsPk = [u8; 48];

#[derive(Debug, Clone, Copy, Default)]
pub struct BlockPos {
    pub height: u64,
    pub round: u64,
}
impl BlockPos {
    pub fn new(height: u64, round: u64) -> Self {
        Self { height, round }
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&self.height.to_le_bytes());
        bytes.extend_from_slice(&self.round.to_le_bytes());
        bytes
    }
}

pub fn addr_from_pk(pk: &IonicPK) -> IonicAddr {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(IONIC_ADDR_DOMAIN);
    bytes.extend_from_slice(pk);
    blake3::hash(&bytes).as_bytes().to_owned()
}

pub fn addr_to_string(addr: IonicAddr) -> String {
    format!("B64{}", URL_SAFE_NO_PAD_INDIFFERENT.encode(addr))
}
