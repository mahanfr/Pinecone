use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
};

use ethnum::u256;

use crate::{
    accounts::Account,
    blocks::Block,
    merkletrie::SparseMerkleTrie,
    state::IonicState,
    transactions::TransactionError,
    types::{IonicAddr, IonicHash, IonicPK},
    vm::VMExecutionError,
};

#[derive(Debug)]
pub struct Blockchain {
    pub id: u64,
    pub chain: VecDeque<Block>,
    pub state: IonicState,
    pub cache: HashMap<IonicAddr, Account>,
}

impl Blockchain {
    pub fn new(chain_id: u64) -> Self {
        let mut chain = VecDeque::new();
        chain.push_back(Self::genesis(chain_id));
        Self {
            id: chain_id,
            chain,
            state: IonicState::new(chain_id),
            cache: HashMap::new(),
        }
    }

    // NOTE: This is for testing remove for production
    pub fn new_account(&mut self, addr: IonicAddr, balance: u256, nonce: u64) {
        let account = Account {
            balance,
            nonce,
            code: Vec::new(),
            storage: SparseMerkleTrie::new(),
        };
        self.state.add_account(addr, account.clone());
        self.cache.insert(addr, account);
    }

    pub fn verify_block(&self, block: &Block) -> Result<(), BlockchainError> {
        let Some(head) = self.head() else {
            return Err(BlockchainError::EmptyChain);
        };
        block.validate_basic(&head.header);
        for tx in block.transactions.iter() {
            if head.header.chain_id != tx.chain_id {
                return Err(BlockchainError::InvalidChainId);
            }
            match self.state.validate_transaction(tx) {
                Ok(_) => (),
                Err(e) => return Err(BlockchainError::TxExecutionError(e)),
            }
        }
        Ok(())
    }

    pub fn genesis(chain_id: u64) -> Block {
        let transactions = Vec::new();

        let mut block = Block::new_unsigned(
            chain_id,
            crate::types::BlockPos::new(0, 0),
            0,
            IonicHash::default(),
            IonicPK::default(),
            IonicHash::default(),
            transactions,
        );
        block.header.base_fee = u256::ONE;
        block
    }

    pub fn head(&self) -> Option<&Block> {
        self.chain.front()
    }

    pub fn get_block(&self, index: u64) -> Option<&Block> {
        self.chain.iter().find(|&block| block.header.index == index).map(|v| v as _)
    }

    pub fn base_fee(&self) -> u256 {
        match self.head() {
            Some(head) => head.next_base_fee(),
            None => Self::genesis(self.id).next_base_fee(),
        }
    }

    pub fn account(&self, addr: &IonicAddr) -> Result<&Account, VMExecutionError> {
        self.state.get_account(addr).map_err(|e| e.into())
    }

    pub fn account_mut(&mut self, addr: &IonicAddr) -> Result<&mut Account, VMExecutionError> {
        self.state.get_mut_account(addr).map_err(|e| e.into())
    }

    pub fn balance(&self, addr: &IonicAddr) -> Result<u256, VMExecutionError> {
        Ok(self.state.get_account(addr)?.balance)
    }

    pub fn code(&self, addr: &IonicAddr) -> Result<Vec<u8>, VMExecutionError> {
        Ok(self.state.get_account(addr)?.code.clone())
    }

    pub fn set_code(
        &mut self,
        new_addr: &IonicAddr,
        code: Vec<u8>,
    ) -> Result<(), VMExecutionError> {
        self.account_mut(new_addr)?.code = code;
        Ok(())
    }

    pub fn transfer(
        &mut self,
        sender: &IonicAddr,
        new_addr: &IonicAddr,
        value: u256,
    ) -> Result<(), VMExecutionError> {
        self.account_mut(sender)?.balance -= value;
        self.account_mut(new_addr)?.balance += value;
        Ok(())
    }

    pub fn selfdestruct(&mut self, addr: &IonicAddr) -> bool {
        self.state.accounts.delete(&addr.as_key())
    }

    pub fn sload(&self, addr: &IonicAddr, key: &IonicHash) -> Result<&u256, VMExecutionError> {
        let account = self.account(addr)?;
        Ok(account
            .storage
            .get(&key.as_key())
            .unwrap_or(&u256::ZERO))
    }

    pub fn sstore(
        &mut self,
        addr: &IonicAddr,
        key: IonicHash,
        value: u256,
    ) -> Result<(), VMExecutionError> {
        let account = self.account_mut(addr)?;
        account.storage.insert(&key.as_key(), value);
        Ok(())
    }
}

#[derive(Debug)]
pub enum BlockchainError {
    EmptyChain,
    InvalidChainId,
    TxExecutionError(TransactionError),
}
impl Display for BlockchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyChain => write!(f, "Empty Chain: verifier needs to have the latest chian"),
            Self::InvalidChainId => write!(f, "Invalid Chain: transactions are not for this chain"),
            Self::TxExecutionError(txe) => write!(f, "<Blockchain Error> {txe}"),
        }
    }
}
