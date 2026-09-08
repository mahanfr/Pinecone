use std::{error::Error, fmt::Display};

use ed25519_dalek::SigningKey;
use ionic::{
    accounts::Account,
    transactions::Transaction,
    types::{IonicAddr, IonicPK, addr_from_pk},
};
use log::warn;

#[derive(Debug)]
pub struct Wallet {
    pub secret_key: SigningKey,
    pub public_key: IonicPK,
    pub address: IonicAddr,
    pub chain_id: u64,
    pub account: Option<Account>,
}

impl Wallet {
    pub fn new(sk: SigningKey, pk: IonicPK, chain_id: u64) -> Self {
        let address = addr_from_pk(&pk);
        Self {
            secret_key: sk,
            public_key: pk,
            address,
            chain_id,
            account: None,
        }
    }

    pub fn set_account_info(&mut self, account: Account) {
        self.account = Some(account);
    }

    pub fn create_tx(&self, value: u128, recepient: IonicAddr) -> Result<Transaction, WalletError> {
        let Some(account) = &self.account else {
            return Err(WalletError::AccountInfoNotExists);
        };
        let gas_limit = 1000;
        let transaction = Transaction::new_builder(
            self.chain_id,
            account.nonce,
            self.public_key,
            Some(recepient),
            value,
        )
        .with_fees(gas_limit, 2000, 1000)
        .sign(&self.secret_key)
        .build();
        Ok(transaction)
    }

    pub fn submit(&self) {
        warn!("submit to network is Not implemented")
    }
}

#[derive(Debug)]
pub enum WalletError {
    AccountInfoNotExists,
}

impl Display for WalletError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalletError::AccountInfoNotExists => write!(
                f,
                "Acount Information dose not exists: please retrive the account information form a RPC node"
            ),
        }
    }
}
impl Error for WalletError {}
