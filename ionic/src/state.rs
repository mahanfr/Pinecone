use crate::{
    accounts::Account,
    transactions::{Transaction, TransactionError},
    types::IonicAddr,
    verkletrie::SparseVerkleTrie,
};

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
    pub chain_id: u64,
    pub accounts: Option<SparseVerkleTrie<Account>>,
}

impl IonicState {
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain_id,
            accounts: None,
        }
    }

    pub fn validate_transaction(&self, tx: &Transaction) -> Result<(), TransactionError> {
        if !tx.verify_signature() {
            return Err(TransactionError::InvalidTxSignature);
        }
        if tx.chain_id != self.chain_id {
            return Err(TransactionError::InvalidChainID);
        }
        let sender_addr = &tx.sender();
        let account = self.get_account(sender_addr)?;
        Self::perform_precheck(account, tx)?;
        Ok(())
    }

    pub fn get_account(&self, addr: &IonicAddr) -> Result<&Account, TransactionError> {
        if self.accounts.is_none() {
            return Err(TransactionError::UnavailableState);
        }
        let account = match self.accounts.as_ref().unwrap().get(addr.as_ref()) {
            Ok(op_ac) => match op_ac {
                Some(ac) => ac,
                None => return Err(TransactionError::InvalidAccount),
            },
            Err(_) => return Err(TransactionError::InvalidAccount),
        };
        Ok(account)
    }

    pub fn apply_transaction(
        &mut self,
        tx: &Transaction,
    ) -> Result<TxExecutionResult, TransactionError> {
        let sender_addr = &tx.sender();
        self.validate_transaction(tx)?;
        let account = self.get_account(sender_addr)?;
        Self::perform_precheck(account, tx)?;
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
        self.accounts
            .as_mut()
            .unwrap()
            .insert(sender_addr.as_ref(), account)
            .unwrap();
        Ok(TxExecutionResult {
            status: TxExecutionStatus::Success,
            validation_tip,
            gas_used,
        })
    }

    fn perform_precheck(account: &Account, tx: &Transaction) -> Result<(), TransactionError> {
        if account.nonce <= tx.nonce {
            return Err(TransactionError::InvalidNonce);
        }
        if account.balance < tx.value + (tx.gas_limit as u128 * tx.max_fee) {
            return Err(TransactionError::InsufficientBalance);
        }
        Ok(())
    }
}
