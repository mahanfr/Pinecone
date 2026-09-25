use std::{collections::{HashMap, VecDeque}, fmt::Display};

use crate::{
    accounts::Account,
    blocks::Block,
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
    pub fn new_account(&mut self, addr: IonicAddr, balance: u128, nonce: u64) {
        let account = Account {
            balance,
            nonce,
            code: Vec::new(),
            storage: SparseMerkleTrie::new(),
        };
        self.state.add_account(addr, account).unwrap();
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
            crate::types::BlockPos::new(0, 0),
            chain_id,
            IonicHash::default(),
            IonicPK::default(),
            IonicHash::default(),
            transactions,
        );
        block.header.base_fee = 1;
        block
    }

    pub fn head(&self) -> Option<&Block> {
        self.chain.front()
    }

    pub fn base_fee(&self) -> u128 {
        match self.head() {
            Some(head) => head.next_base_fee(),
            None => Self::genesis(self.id).next_base_fee(),
        }
    }

    pub fn account(&self, addr: &IonicAddr) -> Result<&Account, TransactionError> {
        self.state.get_account(addr)
    }

    pub fn account_mut(&mut self, addr: &IonicAddr) -> Option<&mut Account> {
        self.state.get_mut_account(addr).ok()
    }

    pub fn balance(&self, addr: &IonicAddr) -> Result<u128, TransactionError> {
        Ok(self.state.get_account(addr)?.balance)
    }

    pub fn code(&self, addr: &IonicAddr) -> Result<Vec<u8>, TransactionError> {
        Ok(self.state.get_account(addr)?.code.clone())
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
