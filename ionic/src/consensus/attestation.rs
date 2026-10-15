use std::collections::HashMap;

use crate::{consensus::{error::ConsensusError, validator::{BLS_DST, ValidatorSet}}, types::{IonicAddr, IonicGroupPk, IonicGroupSignature, IonicHash}};
use blst::{BLST_ERROR::BLST_SUCCESS, min_pk::{
    AggregatePublicKey, AggregateSignature, PublicKey as BlsPublicKey, Signature as BlsSignature,
}};
use ethnum::u256;

const ATTESTATION_DOMAIN: &[u8] = b"IONIC_ATTESTATION_V1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttestationTarget {
    pub chain_id: u64,
    pub height: u64,
    pub round: u64,
    pub block_hash: IonicHash,
}

#[derive(Debug, Clone)]
pub struct Attestation {
    pub chain_id: u64,
    pub height: u64,
    pub round: u64,
    pub block_hash: IonicHash,
    pub validator: IonicAddr,
    pub signature: IonicGroupSignature,
}

impl Attestation {
    pub fn message(chain_id: u64, height: u64, round: u64, block_hash: &IonicHash) -> Vec<u8> {
        let mut m = Vec::new();
        m.extend_from_slice(ATTESTATION_DOMAIN);
        m.extend_from_slice(&chain_id.to_le_bytes());
        m.extend_from_slice(&height.to_le_bytes());
        m.extend_from_slice(&round.to_le_bytes());
        m.extend_from_slice(block_hash.as_ref());
        m
    }

    pub fn signing_root(&self) -> [u8; 32] {
        *blake3::hash(&Self::message(
            self.chain_id,
            self.height,
            self.round,
            &self.block_hash,
        ))
        .as_bytes()
    }

    pub fn verify(&self, set: &ValidatorSet) -> Result<(), ConsensusError> {
        let v = set.get(&self.validator)
            .ok_or(ConsensusError::UnknownValidator)?;
        if !v.is_eligible(self.height) {
            return Err(ConsensusError::InactiveValidator);
        }
        v.verify(&self.signing_root(), &self.signature)
    }
}

#[derive(Debug, Default, Clone)]
pub struct AttestationPool {
    pub target: Option<AttestationTarget>,
    pub by_validator: HashMap<IonicAddr, Attestation>,
}

impl AttestationPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, att: Attestation, set: &ValidatorSet) -> Result<(), ConsensusError> {
        att.verify(set)?;

        if let Some(prev) = self.by_validator.get(&att.validator) {
            if prev.height == att.height {
                if prev.round == att.round && prev.block_hash != att.block_hash {
                    return Err(ConsensusError::Equivocation("double vote"));
                }
                if prev.height == att.height {
                    return Err(ConsensusError::DuplicateAttestation);
                }
            }
        }

        let new_target = AttestationTarget {
            chain_id: att.chain_id,
            height: att.height,
            round: att.round,
            block_hash: att.block_hash,
        };

        match self.target {
            Some(t) if (t.height, t.round) > (new_target.height, new_target.round) => {
                return Ok(());
            }
            Some(t) if (t.height, t.round) < (new_target.height, new_target.round) => {
                self.by_validator.clear();
                self.target = Some(new_target);
            }
            Some(t) if t.height == new_target.height &&
                t.round == new_target.round &&
                t.block_hash != new_target.block_hash => {
                return Err(ConsensusError::Equivocation("conflicting target"));
            }
            None => {
                self.target = Some(new_target);
            }
            _ => {}
        }

        self.by_validator.insert(att.validator, att);
        Ok(())
    }

    pub fn aggregate(
        &self,
        set: &ValidatorSet
    ) -> Result<Option<AggregatedAttestation>, ConsensusError> {
        let Some(target) = self.target else {
            return Ok(None);
        };

        let mut sigs: Vec<BlsSignature> = Vec::new();
        let mut pks: Vec<BlsPublicKey> = Vec::new();
        let mut signers: Vec<IonicAddr> = Vec::new();
        let mut total_stake = u256::ZERO;

        for (addr, att) in self.by_validator.iter() {
            if att.height != target.height
                || att.round != target.round
                || att.block_hash != target.block_hash
            {
                continue;
            }
            let Some(v) = set.get(addr) else { continue };
            if !v.is_eligible(target.height) {
                continue;
            }
            let Ok(pk) = v.bls_public_key() else { continue };
            let Ok(sig) = BlsSignature::from_bytes(&att.signature) else {
                continue;
            };
            sigs.push(sig);
            pks.push(pk);
            signers.push(*addr);
            total_stake += v.effective_stake();
        }

        if sigs.is_empty() {
            return Ok(None);
        }

        let sig_refs: Vec<&BlsSignature> = sigs.iter().collect();
        let pk_refs: Vec<&BlsPublicKey> = pks.iter().collect();

        let agg_sig = AggregateSignature::aggregate(&sig_refs, true)
            .map_err(|_| ConsensusError::InvalidBlsSignature)?
            .to_signature();
        let agg_pk = AggregatePublicKey::aggregate(&pk_refs, true)
            .map_err(|_| ConsensusError::InvalidBlsKey)?
            .to_public_key();

        let mut agg_sig_bytes = [0u8; 96];
        agg_sig_bytes.copy_from_slice(&agg_sig.to_bytes());
        let mut agg_pk_bytes = [0u8; 48];
        agg_pk_bytes.copy_from_slice(&agg_pk.to_bytes());

        Ok(Some(AggregatedAttestation {
            chain_id: target.chain_id,
            height: target.height,
            round: target.round,
            block_hash: target.block_hash,
            aggregate_sig: agg_sig_bytes,
            aggregate_pk: agg_pk_bytes,
            total_stake,
            signers,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct AggregatedAttestation {
    pub chain_id: u64,
    pub height: u64,
    pub round: u64,
    pub block_hash: IonicHash,
    pub aggregate_sig: IonicGroupSignature,
    pub aggregate_pk: IonicGroupPk,
    pub total_stake: u256,
    pub signers: Vec<IonicAddr>,
}

impl AggregatedAttestation {
    pub fn verify(&self) -> Result<(), ConsensusError> {
        let msg = Attestation::message(self.chain_id, self.height, self.round, &self.block_hash);
        let root = blake3::hash(&msg);
        let pk = BlsPublicKey::from_bytes(&self.aggregate_pk)
            .map_err(|_| ConsensusError::InvalidBlsKey)?;
        let sig = BlsSignature::from_bytes(&self.aggregate_sig)
            .map_err(|_| ConsensusError::InvalidBlsSignature)?;
        if sig.verify(true, root.as_bytes(), BLS_DST, &[], &pk, true) != BLST_SUCCESS {
            return Err(ConsensusError::InvalidBlsSignature);
        }
        Ok(())
    }

    pub fn has_quorum(&self, set: &ValidatorSet, threshold_bps: u16) -> bool {
        let total = set.total_active_stake(self.height);
        if total == u256::ZERO {
            return false;
        }
        let threshold = (total * u256::from(threshold_bps)) / u256::from(10_000u16);
        self.total_stake >= threshold
    }

}

pub fn sign_attestation(
    sk: &blst::min_pk::SecretKey,
    chain_id: u64,
    height: u64,
    round: u64,
    block_hash: IonicHash,
    validator: IonicAddr,
) -> Attestation {
    let msg = Attestation::message(chain_id, height, round, &block_hash);
    let root = blake3::hash(&msg);
    let sig = sk.sign(root.as_bytes(), BLS_DST, &[]);
    let mut sig_bytes = [0u8; 96];
    sig_bytes.copy_from_slice(&sig.to_bytes());
    Attestation {
        chain_id,
        height,
        round,
        block_hash,
        validator,
        signature: sig_bytes,
    }
}
