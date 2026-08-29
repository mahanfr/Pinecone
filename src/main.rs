use log::debug;

use crate::{blocks::{Block, genesis}, keygen::generate_key_pair, transactions::Transaction, types::BlockPos};

mod transactions;
mod types;
mod keygen;
mod blocks;
mod verkletrie;

pub fn simulate() {
    debug!("Generating pk/sk");
    let (sk,pk) = generate_key_pair();
    debug!("creating a transaction");
    let transactions = vec![
        Transaction::new(&sk, 0, 0, pk.to_bytes(), None, 100, vec![])
    ];
    debug!("verifing the transaction");
    assert!(transactions[0].verify());
    debug!("Generate genesis block");
    let genesis = genesis();
    debug!("Generate a normal block");
    let block = Block::new(
        BlockPos::new(1,0),
        genesis.hash(),
        pk.to_bytes(),
        [0u8;32],
        transactions
    );
    debug!("validate the block based on the parent");
    assert!(block.validate_basic(&genesis.header));
}

fn main() {
    env_logger::builder().filter_level(log::LevelFilter::Trace).init();
    simulate();
}
