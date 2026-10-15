use std::collections::HashMap;

use ethnum::{AsU256, u256};

use blst::{BLST_ERROR::BLST_SUCCESS, min_pk::{PublicKey as BlsPublicKey, Signature as BlsSignature}};
use crate::{consensus::error::ConsensusError, types::{IonicAddr, IonicGroupPk, IonicGroupSignature}};

pub const BLS_DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

pub const MIN_VALIDATOR_STAKE: u128 = 32_000;

#[derive(Debug, Clone)]
pub struct Validator {
    pub addr: IonicAddr,
    pub bls_pk: IonicGroupPk,
    pub stake: u256,
    pub commission_bps: u16,
    pub active: bool,
    pub jailed_until_height: u64,
    pub slashed: u256,
    pub activation_height: u64,
    pub exit_height: Option<u64>,
}

impl Validator {
    pub fn new(addr: IonicAddr, bls_pk: IonicGroupPk, stake: u256) -> Self {
        Self {
            addr,
            bls_pk,
            stake,
            commission_bps: 0,
            active: true,
            jailed_until_height: 0,
            slashed: u256::ZERO,
            activation_height: 0,
            exit_height: None,
        }
    }

    pub fn bls_public_key(&self) -> Result<BlsPublicKey, ConsensusError> {
        BlsPublicKey::from_bytes(&self.bls_pk).map_err(|_| ConsensusError::InvalidBlsKey)
    }

    pub fn effective_stake(&self) -> u256 {
        self.stake.saturating_sub(self.slashed)
    }

    pub fn is_eligible(&self, height: u64) -> bool {
        self.active
            && height >= self.activation_height
            && self.exit_height.map_or(true, |h| height < h)
            && height >= self.jailed_until_height
            && self.effective_stake() > u256::ZERO
    }

    pub fn verify(&self, msg: &[u8], sig_bytes: &IonicGroupSignature) -> Result<(), ConsensusError> {
        let pk = self.bls_public_key()?;
        let sig = BlsSignature::from_bytes(sig_bytes)
            .map_err(|_| ConsensusError::InvalidBlsSignature)?;
        if sig.verify(true, msg, BLS_DST, &[], &pk, true) == BLST_SUCCESS {
            return Err(ConsensusError::InvalidBlsSignature);
        }
        return Ok(());
    }
}

#[derive(Debug, Clone, Default)]
pub struct ValidatorSet {
    pub validators: Vec<Validator>,
    pub index: HashMap<IonicAddr, usize>,
    pub bls_index: HashMap<IonicGroupPk, usize>,
    pub total_stake: u256,
}

impl ValidatorSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, v: Validator) -> Result<(), ConsensusError> {
        if self.index.contains_key(&v.addr) {
            return Err(ConsensusError::AlreadyRegistered);
        }
        if v.effective_stake() < MIN_VALIDATOR_STAKE.as_u256() {
            return Err(ConsensusError::InsufficientStake { have: v.effective_stake(), need: MIN_VALIDATOR_STAKE.as_u256() })
        }
        let idx = self.validators.len();
        self.bls_index.insert(v.bls_pk, idx);
        self.index.insert(v.addr, idx);
        self.total_stake += v.stake;
        self.validators.push(v);
        Ok(())
    }

    pub fn remove(&mut self, addr: &IonicAddr) -> Option<Validator> {
        let idx = self.index.remove(addr)?;
        let removed = self.validators.swap_remove(idx);
        self.bls_index.remove(&removed.bls_pk);
        self.total_stake = self.total_stake.saturating_sub(removed.stake);
        if idx < self.validators.len() {
            let moved = &self.validators[idx];
            self.index.insert(moved.addr, idx);
            self.bls_index.insert(moved.bls_pk, idx);
        }
        Some(removed)
    }

    pub fn get(&self, addr: &IonicAddr) -> Option<&Validator> {
        self.index.get(addr).map(|&i| &self.validators[i])
    }

    pub fn get_mut(&mut self, addr: &IonicAddr) -> Option<&mut Validator> {
        let i = *self.index.get(addr)?;
        self.validators.get_mut(i)
    }

    pub fn by_bls(&self, pk: &IonicGroupPk) -> Option<&Validator> {
        self.bls_index.get(pk).map(|&i| &self.validators[i])
    }

    pub fn total_active_stake(&self, height: u64) -> u256 {
        self.validators
            .iter()
            .filter(|v| v.is_eligible(height))
            .fold(u256::ZERO, |acc, v| acc + v.effective_stake())
    }

    pub fn quorum_threshold(&self, height: u64) -> u256 {
        let total = self.total_active_stake(height);
        (total * 2u8.as_u256()) / 3u8.as_u256() + u256::ONE
    }

    pub fn canonicalize(&mut self) {
        self.validators
            .sort_by(|a, b| a.addr.as_ref().cmp(b.addr.as_ref()));
        self.index.clear();
        self.bls_index.clear();
        for (i, v) in self.validators.iter().enumerate() {
            self.index.insert(v.addr, i);
            self.bls_index.insert(v.bls_pk, i);
        }
    }

}
