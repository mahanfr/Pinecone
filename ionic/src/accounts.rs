use ethnum::u256;

use crate::{
    merkletrie::SparseMerkleTrie,
    read_bytes,
    serialization::CodecError,
    utils::{FromBytes, ToBytes},
};

const ACCOUNT_DOMAIN: &[u8] = b"IONIC_ACCOUNT_V1";

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub nonce: u64,
    pub balance: u128,
    pub storage: SparseMerkleTrie<u256>,
    pub code: Vec<u8>,
}

impl Default for Account {
    fn default() -> Self {
        Self::new()
    }
}

impl Account {
    pub fn new() -> Self {
        Self {
            nonce: 0,
            balance: 0,
            storage: SparseMerkleTrie::new(),
            code: Vec::new(),
        }
    }
}

impl ToBytes for Account {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(ACCOUNT_DOMAIN);
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.balance.to_le_bytes());
        bytes.extend_from_slice(&(self.code.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&self.code);
        bytes.extend_from_slice(&self.storage.to_bytes());
        bytes
    }
}

impl FromBytes for Account {
    type Error = CodecError;
    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::Error> {
        // Byte stream Walker
        let mut w = ACCOUNT_DOMAIN.len();
        assert_eq!(&bytes[0..w], ACCOUNT_DOMAIN);

        let nonce_slice: &[u8; 8] = read_bytes!(bytes, w, 8).try_into().unwrap();
        let nonce = u64::from_le_bytes(*nonce_slice);
        let balance_slice: &[u8; 16] = read_bytes!(bytes, w, 16).try_into().unwrap();
        let balance = u128::from_le_bytes(*balance_slice);

        let code_size_slice: &[u8; 8] = read_bytes!(bytes, w, 8).try_into().unwrap();
        let code_size = u64::from_le_bytes(*code_size_slice) as usize;

        let code = read_bytes!(bytes, w, code_size).to_vec();
        let storage = SparseMerkleTrie::<u256>::from_bytes(&bytes[w..]).unwrap();
        Ok(Self {
            nonce,
            balance,
            code,
            storage,
        })
    }
}

#[cfg(test)]
mod tests {
    use ethnum::AsU256;

    use crate::{
        accounts::Account,
        merkletrie::SparseMerkleTrie,
        utils::{FromBytes, ToBytes},
    };

    fn key(x: u8) -> [u8; 32] {
        let mut k = [0u8; 32];
        k[31] = x;
        k
    }

    #[test]
    pub fn encode_decode() {
        let mut storage = SparseMerkleTrie::new();
        storage.insert(&key(1), 0.as_u256()).unwrap();
        storage.insert(&key(2), 1.as_u256()).unwrap();
        storage.insert(&key(3), 2.as_u256()).unwrap();
        let account = Account {
            nonce: 55,
            balance: 500,
            code: vec![1, 2, 3],
            storage,
        };
        let bytes = account.to_bytes();
        let encoded_account = Account::from_bytes(&bytes).unwrap();
        assert_eq!(account, encoded_account);
    }
}
