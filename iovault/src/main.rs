mod wallet;

use clap::{Parser, Subcommand};
use ionic::{accounts::Account, keygen::generate_key_pair, utils::IonicBase64};

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
    let mut wallet = Wallet::new(sk, pk.to_bytes(), 0);
    let account = Account {
        nonce: 0,
        balance: 1_000_000_000,
        code: Vec::new(),
    };
    wallet.set_account_info(account);

    match cli.command {
        Commands::Wallet(cmd) => match cmd {
            WalletCommands::Account => {
                let addr_str = IonicBase64::encode(wallet.address);
                println!("<{}>: {:?}", addr_str, wallet.account);
            }
            WalletCommands::Send { to, amount } => {
                let recepient = IonicBase64::decode(to);
                let tx = wallet
                    .create_tx(amount, recepient[..32].try_into().unwrap())
                    .unwrap();
                println!("{}", tx);
                wallet.submit();
            }
            WalletCommands::Balance => println!("{}", wallet.account.unwrap().balance),
        },
    }
}
