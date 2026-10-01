use std::{
    collections::{HashMap, VecDeque},
    fmt::Display,
};

use ethnum::u256;

use crate::{
    accounts::Account,
    blocks::Block,
    journal::{Journal, Snapshot},
    merkletrie::SparseMerkleTrie,
    state::IonicState,
    transactions::TransactionError,
    types::{IonicAddr, IonicHash, IonicPK},
};

#[derive(Debug)]
pub struct Blockchain {
    pub id: u64,
    pub chain: VecDeque<Block>,
    pub state: IonicState,
    pub cache: HashMap<IonicAddr, Account>,
    pub journal: Journal,
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
            journal: Journal::new(),
        }
    }

    pub fn apply(&mut self) {
        self.journal.commit();
    }

    pub fn revert(&mut self, sn: Snapshot) {
        self.journal.revert(sn, &mut self.state);
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
        self.journal.account_created(addr);
    }

    pub fn verify_block(&self, block: &Block) -> Result<(), BlockchainError> {
        let Some(head) = self.head() else {
            return Err(BlockchainError::EmptyChain);
        };
        block.validate_basic(&head.header)?;
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
        self.chain
            .iter()
            .find(|&block| block.header.index == index)
            .map(|v| v as _)
    }

    pub fn base_fee(&self) -> u256 {
        match self.head() {
            Some(head) => head.next_base_fee(),
            None => Self::genesis(self.id).next_base_fee(),
        }
    }

    pub fn get_account(&self, addr: &IonicAddr) -> Option<&Account> {
        self.state.get_account(addr).ok()
    }

    pub fn set_account(&mut self, addr: &IonicAddr, account: Account) -> bool {
        match self.state.get_mut_account(addr) {
            Ok(ac) => {
                self.journal.account_destroyed(*addr, Box::new(ac.clone()));
                *ac = account;
                true
            }
            Err(_) => false,
        }
    }

    pub fn get_balance(&self, addr: &IonicAddr) -> Option<u256> {
        self.state.get_account(addr).ok().map(|ac| ac.balance)
    }

    pub fn set_balance(&mut self, addr: &IonicAddr, value: u256) -> bool {
        match self.state.get_mut_account(addr) {
            Ok(acc) => {
                self.journal.balance_changed(*addr, acc.balance);
                acc.balance = value;
                true
            }
            Err(_) => false,
        }
    }

    pub fn get_code(&self, addr: &IonicAddr) -> Option<Vec<u8>> {
        self.state.get_account(addr).ok().map(|ac| ac.code.clone())
    }

    pub fn set_code(&mut self, new_addr: &IonicAddr, code: Vec<u8>) -> bool {
        match self.state.get_mut_account(new_addr) {
            Ok(acc) => {
                self.journal.code_changed(*new_addr, acc.code.clone());
                acc.code = code;
                true
            }
            Err(_) => false,
        }
    }

    pub fn transfer(&mut self, sender: &IonicAddr, new_addr: &IonicAddr, value: u256) -> bool {
        if let Ok(ac) = self.state.get_mut_account(sender) {
            self.journal.balance_changed(*sender, ac.balance);
            ac.balance -= value;
        } else {
            return false;
        }
        if let Ok(ac) = self.state.get_mut_account(new_addr) {
            self.journal.balance_changed(*new_addr, ac.balance);
            ac.balance += value;
        } else {
            return false;
        }
        true
    }

    pub fn selfdestruct(&mut self, addr: &IonicAddr) -> bool {
        match self.state.accounts.delete(&addr.as_key()) {
            Some(prev) => {
                self.journal.account_destroyed(*addr, Box::new(prev));
                true
            }
            None => false,
        }
    }

    pub fn sload(&self, addr: &IonicAddr, key: &IonicHash) -> Option<&u256> {
        if let Ok(ac) = self.state.get_account(addr) {
            ac.storage.get(&key.as_key())
        } else {
            None
        }
    }

    pub fn sstore(&mut self, addr: &IonicAddr, key: u256, value: u256) -> bool {
        let nkey = key.to_le_bytes();
        if let Ok(ac) = self.state.get_mut_account(addr) {
            let prev = ac.storage.insert(&nkey, value);
            self.journal.storage_chanaged(*addr, nkey, prev);
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub enum BlockchainError {
    EmptyChain,
    InvalidChainId,
    InvalidSignature,
    InvalidPK,
    UnsupportedVersion,
    InvalidParentHeight,
    HashMissmatch,
    InvaldRootHash,
    InvaldStateHash,
    TxExecutionError(TransactionError),
}
impl Display for BlockchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyChain => write!(f, "Empty Chain: verifier needs to have the latest chian"),
            Self::InvalidChainId => write!(f, "Invalid Chain: transactions are not for this chain"),
            Self::TxExecutionError(txe) => write!(f, "<Blockchain Error> {txe}"),
            Self::InvalidSignature =>  write!(f, "Empty Signature: The Block has not been signed"),
            Self::InvalidPK =>  write!(f, "preposer has not a valid public key"),
            Self::UnsupportedVersion =>  write!(f, "header is on unsupported version"),
            Self::InvalidParentHeight =>  write!(f, "parent is at the same height or higher than the child"),
            Self::HashMissmatch =>  write!(f, "parent hash dose not match the blocks pervious hash"),
            Self::InvaldRootHash =>  write!(f, "the block Tx root dose not match the expected root"),
            Self::InvaldStateHash =>  write!(f, "the block Tx state dose not match the expected state"),
        }
    }
}
