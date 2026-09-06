use std::fmt::Display;

use crate::{blocks::Block, state::{IonicState, TxExecutionError}};

#[derive(Debug)]
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub state: IonicState,
}

impl Blockchain {
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain: vec![Self::genesis(chain_id)],
            state: IonicState::new(),
        }
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
                Err(e) => return Err(BlockchainError::TxExecutionError(e))
            }
        }
        Ok(())
    }

    pub fn genesis(chain_id: u64) -> Block {
        let transactions = Vec::new();

        Block::new(
            crate::types::BlockPos::new(0, 0),
            chain_id,
            [0u8; 32],
            [0u8; 32],
            [0u8; 32],
            transactions,
        )
    }

    pub fn head(&self) -> Option<&Block> {
        self.chain.last()
    }

}

#[derive(Debug)]
pub enum BlockchainError {
    EmptyChain,
    InvalidChainId,
    TxExecutionError(TxExecutionError),
}
impl Display for BlockchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyChain => write!(f, "Empty Chain: verifier needs to have the latest chian"),
            Self::InvalidChainId => write!(f, "Invalid Chain: transactions are not for this chain"),
            Self::TxExecutionError(txe) =>
                write!(f, "<Blockchain Error> {txe}")
        }
    }
}
