use std::fmt::Display;

use crate::{blocks::Block, state::IonicState, transactions::TransactionError};

#[derive(Debug)]
pub struct Blockchain {
    pub chain: Vec<Block>,
    pub state: IonicState,
}

impl Blockchain {
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain: vec![Self::genesis(chain_id)],
            state: IonicState::new(chain_id),
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
            [0u8; 32],
            [0u8; 32],
            [0u8; 32],
            transactions,
        );
        block.header.base_fee = 100;
        block
    }

    pub fn head(&self) -> Option<&Block> {
        self.chain.last()
    }

    pub fn base_fee(&self) -> u128 {
        let last_block_header = self.head().unwrap().header;
        let current_base_fee = last_block_header.base_fee;
        let gas_target = last_block_header.gas_limit / 2;
        if gas_target == 0 {
            return current_base_fee;
        }
        // base_fee(N) * (gas_used(N) - gas_target(N)) / (gas_target(N) * 8)
        let inflaition =
            current_base_fee * ((last_block_header.gas_used - gas_target) / gas_target * 8) as u128;
        let base_fee = current_base_fee + inflaition;
        if base_fee < 1 {
            return 1;
        }
        base_fee
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
