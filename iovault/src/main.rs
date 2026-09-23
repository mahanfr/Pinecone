mod wallet;

use clap::{Parser, Subcommand};
use ionic::{accounts::Account, keygen::generate_key_pair, merkletrie::SparseMerkleTrie, types::IonicAddr};

use crate::wallet::Wallet;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(subcommand)]
    Wallet(WalletCommands),
}

#[derive(Subcommand)]
pub enum WalletCommands {
    Account,
    Send {
        #[arg(short, long)]
        to: String,

        #[arg(short, long)]
        amount: u128,
    },
    Balance,
}

fn main() {
    let cli = Cli::parse();
    let (sk, pk) = generate_key_pair();
    let mut wallet = Wallet::new(sk, pk.into(), 0);
    let account = Account {
        nonce: 0,
        balance: 1_000_000_000,
        code: Vec::new(),
        storage: SparseMerkleTrie::new(),
    };
    wallet.set_account_info(account);

    match cli.command {
        Commands::Wallet(cmd) => match cmd {
            WalletCommands::Account => {
                println!("<{}>: {:?}", wallet.address, wallet.account);
            }
            WalletCommands::Send { to, amount } => {
                let recepient = IonicAddr::try_from(to).unwrap();
                let tx = wallet.create_tx(amount, recepient).unwrap();
                println!("{}", tx);
                wallet.submit();
            }
            WalletCommands::Balance => println!("{}", wallet.account.unwrap().balance),
        },
    }
}
