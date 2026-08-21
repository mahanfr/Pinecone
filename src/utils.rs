use std::char;

use anyhow::Result;
use rand::{TryRng, rngs::SysRng};

pub fn generate_bls_key() -> (blst::min_pk::SecretKey, blst::min_pk::PublicKey) {
    let mut rng = SysRng;
    let mut secret = [0u8; 32];
    rng.try_fill_bytes(&mut secret).expect("failed to generate random vector");
    let secret_key =
        blst::min_pk::SecretKey::key_gen(&secret, &[]).expect("failed to generate BLS key");
    let public_key =
        secret_key.sk_to_pk();
    (secret_key, public_key)
}

#[allow(dead_code)]
pub fn generate_key_pair() -> Result<(ed25519_dalek::SigningKey, ed25519_dalek::VerifyingKey)> {
    let mut rng = SysRng;
    let mut secret = [0u8; 32];
    rng.try_fill_bytes(&mut secret)?;
    let private_key = ed25519_dalek::SigningKey::from_bytes(&secret);
    let public_key = private_key.verifying_key();

    Ok((private_key, public_key))
}

pub fn generate_address(pub_key: &[u8]) -> [u8; 32] {
    blake3::hash(pub_key).as_bytes().to_owned()
}
