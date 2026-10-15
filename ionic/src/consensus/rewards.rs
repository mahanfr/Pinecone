use ethnum::{AsU256, u256};

use super::validator::Validator;

pub const BLOCK_REWARD: u128 = 1_000;
pub const ATTESTATION_REWARD: u128 = 10;
pub const PROPOSER_BONUS_BPS: u16 = 1000; // 10% extra

pub fn block_reward(proposer: &Validator, total_active_stake: u256) -> u256 {
    if total_active_stake == u256::ZERO {
        return u256::ZERO;
    }
    (BLOCK_REWARD.as_u256() * proposer.effective_stake()) / total_active_stake
}

pub fn proposer_bonus(base: u256) -> u256 {
    (base * u256::from(PROPOSER_BONUS_BPS)) / 10_000u32.as_u256()
}

pub fn attestation_reward(_v: &Validator) -> u256 {
    ATTESTATION_REWARD.as_u256()
}
