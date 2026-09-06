use std::{error::Error, fmt::Display};

use crate::{accounts::Account, transactions::Transaction, verkletrie::SparseVerkleTrie};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxExecutionStatus {
    Success,
    Appendding,
    Fail,
}

#[derive(Debug)]
pub struct TxExecutionResult {
    pub status: TxExecutionStatus,
    pub validation_tip: u128,
    pub gas_used: u64,
}

#[derive(Debug)]
pub struct IonicState {
    pub accounts: Option<SparseVerkleTrie<Account>>,
}

impl IonicState {
    pub fn new() -> Self {
        Self {
            accounts: None
        }
    }

    pub fn validate_transaction(&self, tx: &Transaction) -> Result<(), TxExecutionError> {
        if self.accounts.is_none() {
            return Err(TxExecutionError::UnavailableState);
        }
        if !tx.verify() {
            return Err(TxExecutionError::InvalidSignature);
        }
        let sender_addr = &tx.sender();
        let account = match self.accounts.as_ref().unwrap().get(sender_addr) {
            Ok(op_ac) => match op_ac {
                Some(ac) => ac,
                None => {
                    return Err(TxExecutionError::InvalidAccount)
                }
            },
            Err(_) => {
                return Err(TxExecutionError::InvalidAccount)
            }
        };
        Self::perform_precheck(&account, tx)?;
        Ok(())
    }

    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<TxExecutionResult, TxExecutionError> {
        if self.accounts.is_none() {
            return Err(TxExecutionError::UnavailableState);
        }
        let sender_addr = &tx.sender();
        let account = match self.accounts.as_ref().unwrap().get(sender_addr) {
            Ok(op_ac) => match op_ac {
                Some(ac) => ac,
                None => {
                    return Err(TxExecutionError::InvalidAccount)
                }
            },
            Err(_) => {
                return Err(TxExecutionError::InvalidAccount)
            }
        };
        // Precheck Balance and Nonce
        Self::perform_precheck(&account, tx)?;
        let mut balance = account.balance;
        // TODO: calculate base fee
        let base_fee = 100;
        // TODO: calculate Tip
        let tip = 80u128;
        // TODO: calculate gas used
        let gas_used = 10000u64;
        // Burn the base fee
        balance -= base_fee * gas_used as u128;
        let validation_tip: u128 = tip * gas_used as u128;
        // TODO: Run Bytecode
        let account = Account {
            balance,
            nonce: account.nonce + 1,
            code: account.code.clone(),
        };
        // Apply
        // TODO: Verkletrie should have a buffer qeueue that then applies the changes.
        self.accounts.as_mut().unwrap().insert(sender_addr, account).unwrap();
        Ok(TxExecutionResult { status: TxExecutionStatus::Success, validation_tip, gas_used })
    }

    fn perform_precheck(account: &Account, tx: &Transaction) -> Result<(), TxExecutionError> {
        if account.nonce != tx.nonce {
            return Err(TxExecutionError::InvalidNonce);
        }
        if account.balance < tx.value + (tx.gas_limit as u128 * tx.max_fee) {
            return Err(TxExecutionError::InsufficientBalance);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum TxExecutionError {
    InvalidNonce,
    InsufficientBalance,
    InvalidAccount,
    UnavailableState,
    InvalidSignature,
}

impl Display for TxExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNonce =>
                write!(f, "Invalid Nonce: nonce dose not match the account state"),
            Self::InsufficientBalance =>
                write!(f, "Insufficient Balance: balance is insufficient for this operation"),
            Self::InvalidAccount =>
                write!(f, "Invalid Account: can not find any account linked with the provided address"),
            Self::UnavailableState =>
                write!(f, "Unavailable State: consider downloading the state form a state owner"),
            Self::InvalidSignature =>
                write!(f, "Invalid Signature: the transaction is not signed by the creator")
        }
    }
}

impl Error for TxExecutionError {}
