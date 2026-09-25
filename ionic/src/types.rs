use std::fmt::{Debug, Display};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD_INDIFFERENT};
use ed25519_dalek::VerifyingKey;
use ethnum::u256;

use crate::utils::ToBytes;

const IONIC_ADDR_DOMAIN: &[u8] = b"IONIC_ADDR";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IonicPK {
    pk: [u8; 32],
}

impl Display for IonicPK {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", URL_SAFE_NO_PAD_INDIFFERENT.encode(self.pk))
    }
}

impl Debug for IonicPK {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", URL_SAFE_NO_PAD_INDIFFERENT.encode(self.pk))
    }
}

impl Default for IonicPK {
    fn default() -> Self {
        Self { pk: [0u8; 32] }
    }
}

impl Into<[u8; 32]> for IonicPK {
    fn into(self) -> [u8; 32] {
        self.pk
    }
}

impl ToBytes for IonicPK {
    fn to_bytes(&self) -> Vec<u8> {
        self.pk.to_vec()
    }
}

impl From<VerifyingKey> for IonicPK {
    fn from(value: VerifyingKey) -> Self {
        Self {
            pk: value.as_bytes().to_owned(),
        }
    }
}

impl TryInto<VerifyingKey> for IonicPK {
    type Error = IonicParsingError;
    fn try_into(self) -> Result<VerifyingKey, Self::Error> {
        VerifyingKey::from_bytes(&self.pk).map_err(|_| IonicParsingError::InvalidData)
    }
}

impl From<[u8; 32]> for IonicPK {
    fn from(value: [u8; 32]) -> Self {
        Self { pk: value }
    }
}

impl AsRef<[u8]> for IonicPK {
    fn as_ref(&self) -> &[u8] {
        &self.pk[..]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IonicHash {
    hash: [u8; 32],
}

impl Display for IonicHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", URL_SAFE_NO_PAD_INDIFFERENT.encode(self.hash))
    }
}

impl Debug for IonicHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", URL_SAFE_NO_PAD_INDIFFERENT.encode(self.hash))
    }
}

impl Default for IonicHash {
    fn default() -> Self {
        Self { hash: [0u8; 32] }
    }
}

impl Into<[u8; 32]> for IonicHash {
    fn into(self) -> [u8; 32] {
        self.hash
    }
}

impl Into<u256> for IonicHash {
    fn into(self) -> u256 {
        u256::from_le_bytes(self.hash)
    }
}

impl ToBytes for IonicHash {
    fn to_bytes(&self) -> Vec<u8> {
        self.hash.to_vec()
    }
}

impl From<blake3::Hash> for IonicHash {
    fn from(value: blake3::Hash) -> Self {
        Self {
            hash: value.as_bytes().to_owned(),
        }
    }
}

impl From<[u8; 32]> for IonicHash {
    fn from(value: [u8; 32]) -> Self {
        Self { hash: value }
    }
}

impl AsRef<[u8]> for IonicHash {
    fn as_ref(&self) -> &[u8] {
        &self.hash[..]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IonicAddr {
    addr: [u8; 32],
}

impl Default for IonicAddr {
    fn default() -> Self {
        Self { addr: [0u8; 32] }
    }
}

impl Into<[u8; 32]> for IonicAddr {
    fn into(self) -> [u8; 32] {
        self.addr
    }
}

impl Into<u256> for IonicAddr {
    fn into(self) -> u256 {
        u256::from_le_bytes(self.addr)
    }
}

impl From<u256> for IonicAddr {
    fn from(value: u256) -> Self {
        Self {
            addr: value.to_le_bytes(),
        }
    }
}

impl ToBytes for IonicAddr {
    fn to_bytes(&self) -> Vec<u8> {
        self.addr.to_vec()
    }
}

impl From<[u8; 32]> for IonicAddr {
    fn from(value: [u8; 32]) -> Self {
        Self { addr: value }
    }
}

impl AsRef<[u8]> for IonicAddr {
    fn as_ref(&self) -> &[u8] {
        &self.addr[..]
    }
}

impl Display for IonicAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", URL_SAFE_NO_PAD_INDIFFERENT.encode(self.addr))
    }
}

impl TryFrom<String> for IonicAddr {
    type Error = IonicParsingError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let addr = URL_SAFE_NO_PAD_INDIFFERENT
            .decode(value)
            .map_err(|_| IonicParsingError::InvalidData)?;
        Ok(Self {
            addr: addr[..32]
                .try_into()
                .map_err(|_| IonicParsingError::InvalidSize)?,
        })
    }
}

impl IonicAddr {
    pub fn from_pk(pk: &IonicPK) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(IONIC_ADDR_DOMAIN);
        bytes.extend_from_slice(&pk.to_bytes());
        Self {
            addr: blake3::hash(&bytes).as_bytes().to_owned(),
        }
    }
}
pub type IonicTXSignature = [u8; 64];

pub type IonicGroupSignature = [u8; 96];
pub type IonicGroupPk = [u8; 48];

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

#[derive(Debug)]
pub enum IonicParsingError {
    InvalidSize,
    InvalidData,
}
impl Display for IonicParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSize => write!(f, "Invalid Size"),
            Self::InvalidData => write!(f, "Invalid Data"),
        }
    }
}
impl std::error::Error for IonicParsingError {}
