mod consensus;
mod transactions;
mod blocks;
mod hashing;
mod validators;
mod utils;
mod network;

use anyhow::Result;

pub type PinePublicKey = [u8; 32];
pub type PineAddress = [u8; 32];
pub type PineHash = [u8; 32];
pub type PineBlsSignature = [u8; 96];
pub type PineBlsPublicKey = [u8; 48];

fn main() -> Result<()> {
    network::simulate();
    Ok(())
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, Verifier};
    use crate::utils::generate_key_pair;

    #[test]
    fn generate_valid_signature() {
        let msg = b"Hello World";
        let (priv_key, pub_key) = generate_key_pair().unwrap();
        let signature = priv_key.sign(msg);
        assert!(pub_key.verify(msg, &signature).is_ok());
    }
}
