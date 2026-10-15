use ethnum::u256;

use crate::{consensus::validator::{Validator, ValidatorSet}, types::IonicHash};


const PROPOSER_DOMAIN: &[u8] = b"IONIC_PROPOSER_SELECTION_V1";

pub fn select_proposer<'a>(
    set: &'a ValidatorSet,
    parent_hash: &IonicHash,
    height: u64,
    round: u64,
) -> Option<&'a Validator> {
    let active: Vec<&Validator> = set
        .validators
        .iter()
        .filter(|v| v.is_eligible(height))
        .collect();

    if active.is_empty() { return None; }
    let total: u256 = active
        .iter()
        .fold(u256::ZERO, |acc, v| acc + v.effective_stake());

    if total == u256::ZERO { return None; }
    let mut seed = Vec::with_capacity(PROPOSER_DOMAIN.len() + 48);
    seed.extend_from_slice(PROPOSER_DOMAIN);
    seed.extend_from_slice(parent_hash.as_ref());
    seed.extend_from_slice(&height.to_le_bytes());
    seed.extend_from_slice(&round.to_le_bytes());
    let rand = u256::from_le_bytes(*blake3::hash(&seed).as_bytes());
    let target = rand % total;

    let mut acc = u256::ZERO;
    for v in active {
        acc += v.effective_stake();
        if acc > target {
            return Some(v);
        }
    }
    None
}

pub fn select_proposer_committee<'a>(
    set: &'a ValidatorSet,
    seed_hash: &IonicHash,
    height: u64,
    round: u64,
    size: usize,
) -> Vec<&'a Validator> {
    let mut active: Vec<&Validator> = set
        .validators
        .iter()
        .filter(|v| v.is_eligible(height))
        .collect();
    if active.is_empty() || size == 0 {
        return Vec::new();
    }

    for k in 0..active.len().saturating_sub(1) {
        let mut buf = Vec::with_capacity(PROPOSER_DOMAIN.len() + 56);
        buf.extend_from_slice(b"IONIC_COMMITTEE");
        buf.extend_from_slice(seed_hash.as_ref());
        buf.extend_from_slice(&height.to_le_bytes());
        buf.extend_from_slice(&round.to_le_bytes());
        buf.extend_from_slice(&(k as u64).to_le_bytes());
        let r = u256::from_le_bytes(*blake3::hash(&buf).as_bytes());
        let j = k + (r % u256::from((active.len() - k) as u64)).as_usize();
        active.swap(k, j);
    }

    active.truncate(size);
    active
}
