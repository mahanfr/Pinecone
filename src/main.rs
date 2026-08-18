mod transactions;
mod blocks;
mod hashing;
mod validators;

use anyhow::Result;
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::{TryRng, rngs::SysRng};

pub type PrivateKey = SigningKey;
pub type PublicKey = [u8; 32];
pub type Address = [u8; 32];
pub type Hash = [u8; 32];

fn generate_key_pair() -> Result<(PrivateKey, VerifyingKey)> {
    let mut rng = SysRng;
    let mut secret = [0u8; 32];
    rng.try_fill_bytes(&mut secret)?;
    let private_key = SigningKey::from_bytes(&secret);
    let public_key = private_key.verifying_key();

    Ok((private_key, public_key))
}

fn generate_address(pub_key: &VerifyingKey) -> String {
    blake3::hash(pub_key.as_bytes()).to_string()
}

fn main() -> Result<()> {
    let msg = b"Hello World";
    let (priv_key, pub_key) = generate_key_pair()?;
    let signature = priv_key.sign(msg);
    if pub_key.verify(msg, &signature).is_ok() {
        println!("The signature is valid");
    }
    println!("Wallet Address = {}",generate_address(&pub_key));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generate_valid_signature() {
        let msg = b"Hello World";
        let (priv_key, pub_key) = generate_key_pair().unwrap();
        let signature = priv_key.sign(msg);
        assert!(pub_key.verify(msg, &signature).is_ok());
    }
}
