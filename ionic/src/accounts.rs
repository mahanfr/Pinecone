use crate::{read_bytes, utils::{FromBytes, ToBytes}};

const ACCOUNT_DOMAIN : &[u8] = b"IONIC_ACCOUNT_V1";

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub nonce: u64,
    pub balance: u128,
    pub code: Vec<u8>,
}

impl Account {
    pub fn new() -> Self {
        Self {
            nonce: 0,
            balance: 0,
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
       bytes
   }
}

impl FromBytes for Account {
    fn from_bytes(bytes: &[u8]) -> Self {
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
        let _ = w;
        Self { nonce, balance, code }
    }
}

#[cfg(test)]
mod tests {
    use crate::{accounts::Account, utils::{FromBytes, ToBytes}};
    
    #[test]
    pub fn encode_decode() {
        let account = Account {nonce: 55, balance: 500, code: vec![1,2,3]};
        let bytes = account.to_bytes();
        let encoded_account = Account::from_bytes(&bytes);
        assert_eq!(account, encoded_account);
    }
}
