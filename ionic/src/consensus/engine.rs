use std::collections::HashMap;

use ethnum::u256;

use crate::blocks::Block;
use crate::types::{IonicAddr, IonicGroupPk, IonicHash};

use super::attestation::{AggregatedAttestation, Attestation, AttestationPool, AttestationTarget};
use super::epoch::{EpochState, EPOCH_LENGTH};
use super::error::ConsensusError;
use super::proposer::select_proposer;
use super::slashing::{SlashingEvidence, SlashingKind, JAIL_DURATION_HEIGHTS};
use super::validator::{Validator, ValidatorSet, MIN_VALIDATOR_STAKE};

#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    pub chain_id: u64,
    pub epoch_length: u64,
    pub min_validator_stake: u256,
    pub finality_threshold_bps: u16,
    pub max_rounds: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            chain_id: 0,
            epoch_length: EPOCH_LENGTH,
            min_validator_stake: u256::from(MIN_VALIDATOR_STAKE),
            finality_threshold_bps: 6667,
            max_rounds: 64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FinalizedCheckpoint {
    pub height: u64,
    pub round: u64,
    pub block_hash: IonicHash,
    pub total_stake: u256,
    pub signers: Vec<IonicAddr>,
}

#[derive(Debug)]
pub struct ConsensusEngine {
    pub config: ConsensusConfig,
    pub validators: ValidatorSet,
    pub epoch: EpochState,
    pub attestations: AttestationPool,
    pub finalized: Option<FinalizedCheckpoint>,
    pub slashing_evidence: Vec<SlashingEvidence>,
    pub proposer_history: HashMap<(u64, u64), IonicAddr>,
    pub vote_history: HashMap<(IonicAddr, u64), (u64, IonicHash)>,
    pub proposal_history: HashMap<(u64, u64), IonicHash>,
}

impl ConsensusEngine {
    pub fn new(config: ConsensusConfig, genesis_hash: IonicHash) -> Self {
        Self {
            config,
            validators: ValidatorSet::new(),
            epoch: EpochState::new(genesis_hash),
            attestations: AttestationPool::new(),
            finalized: None,
            slashing_evidence: Vec::new(),
            proposer_history: HashMap::new(),
            vote_history: HashMap::new(),
            proposal_history: HashMap::new(),
        }
    }

    pub fn register_validator(
        &mut self,
        addr: IonicAddr,
        bls_pk: IonicGroupPk,
        stake: u256,
        current_height: u64,
    ) -> Result<(), ConsensusError> {
        if stake < self.config.min_validator_stake {
            return Err(ConsensusError::InsufficientStake {
                have: stake,
                need: self.config.min_validator_stake,
            });
        }
        let mut v = Validator::new(addr, bls_pk, stake);
        v.activation_height = current_height + self.config.epoch_length;
        self.validators.insert(v)?;
        self.validators.canonicalize();
        Ok(())
    }

    pub fn unregister_validator(&mut self, addr: &IonicAddr) -> Result<(), ConsensusError> {
        self.validators
            .remove(addr)
            .ok_or(ConsensusError::NotAValidator)?;
        self.validators.canonicalize();
        Ok(())
    }

    pub fn request_exit(&mut self, addr: IonicAddr) -> Result<(), ConsensusError> {
        if self.validators.get(&addr).is_none() {
            return Err(ConsensusError::NotAValidator);
        }
        self.epoch.schedule_exit(addr);
        Ok(())
    }

    pub fn expected_proposer(
        &self,
        parent_hash: &IonicHash,
        height: u64,
        round: u64,
    ) -> Option<&Validator> {
        select_proposer(&self.validators, parent_hash, height, round)
    }

    pub fn validate_proposal(
        &mut self,
        block: &Block,
        parent: &Block,
    ) -> Result<(), ConsensusError> {
        if block.header.chain_id != self.config.chain_id {
            return Err(ConsensusError::WrongChainId);
        }

        let expected_height = parent.header.position.height + 1;
        if block.header.position.height != expected_height {
            return Err(ConsensusError::WrongHeight {
                expected: expected_height,
                got: block.header.position.height,
            });
        }
        if block.header.previous_hash != parent.hash() {
            return Err(ConsensusError::ParentHashMismatch);
        }

        let height = block.header.position.height;
        let round = block.header.position.round;

        let Some(expected) = self
            .expected_proposer(&block.header.previous_hash, height, round)
            .map(|v| v.addr)
        else {
            return Err(ConsensusError::InvalidProposer);
        };

        let proposer = IonicAddr::from_pk(&block.header.proposer);
        if proposer != expected {
            return Err(ConsensusError::InvalidProposer);
        }

        let key = (height, round);
        let block_hash = block.hash();
        if let Some(prev) = self.proposal_history.get(&key) {
            if *prev != block_hash {
                self.slashing_evidence.push(SlashingEvidence {
                    offender: proposer,
                    kind: SlashingKind::DoubleProposal {
                        first: *prev,
                        second: block_hash,
                    },
                    height,
                    round,
                });
                return Err(ConsensusError::Equivocation("double proposal"));
            }
        } else {
            self.proposal_history.insert(key, block_hash);
        }

        self.proposer_history.insert(key, proposer);
        Ok(())
    }

    pub fn on_attestation(&mut self, att: Attestation) -> Result<bool, ConsensusError> {
        if att.chain_id != self.config.chain_id {
            return Err(ConsensusError::WrongChainId);
        }

        if let Some(&(prev_round, prev_hash)) = self.vote_history.get(&(att.validator, att.height))
        {
            if prev_round == att.round && prev_hash != att.block_hash {
                self.slashing_evidence.push(SlashingEvidence {
                    offender: att.validator,
                    kind: SlashingKind::DoubleVote {
                        first: prev_hash,
                        second: att.block_hash,
                    },
                    height: att.height,
                    round: att.round,
                });
                return Err(ConsensusError::Equivocation("double vote"));
            }
        }

        self.attestations.insert(att.clone(), &self.validators)?;
        self.vote_history
            .insert((att.validator, att.height), (att.round, att.block_hash));

        if let Some(t) = self.attestations.target {
            if let Some(agg) = self.attestations.aggregate(&self.validators)? {
                if agg.has_quorum(&self.validators, self.config.finality_threshold_bps) {
                    return Ok(true);
                }
            }
            let _ = t;
        }
        Ok(false)
    }

    pub fn try_finalize(&mut self) -> Result<Option<FinalizedCheckpoint>, ConsensusError> {
        let Some(target) = self.attestations.target else {
            return Ok(None);
        };

        if let Some(fin) = &self.finalized {
            if (fin.height, fin.round) >= (target.height, target.round) {
                return Ok(None);
            }
        }

        let Some(agg) = self.attestations.aggregate(&self.validators)? else {
            return Ok(None);
        };
        agg.verify()?;
        if !agg.has_quorum(&self.validators, self.config.finality_threshold_bps) {
            return Ok(None);
        }

        let checkpoint = FinalizedCheckpoint {
            height: agg.height,
            round: agg.round,
            block_hash: agg.block_hash,
            total_stake: agg.total_stake,
            signers: agg.signers.clone(),
        };
        self.finalized = Some(checkpoint.clone());
        self.attestations = AttestationPool::new();
        Ok(Some(checkpoint))
    }

    pub fn is_finalized(&self, hash: &IonicHash) -> bool {
        self.finalized
            .as_ref()
            .is_some_and(|f| f.block_hash == *hash)
    }

    pub fn apply_slashings(&mut self) -> Result<(), ConsensusError> {
        let evidence = std::mem::take(&mut self.slashing_evidence);
        for ev in &evidence {
            if let Some(v) = self.validators.get_mut(&ev.offender) {
                let amount = ev.slash_amount(v);
                v.slashed = v.slashed.saturating_add(amount);
                v.jailed_until_height = ev.height + JAIL_DURATION_HEIGHTS;
                if v.effective_stake() < self.config.min_validator_stake {
                    v.active = false;
                }
            }
        }
        Ok(())
    }

    pub fn maybe_advance_epoch(&mut self, height: u64) {
        if height == 0 || height % self.config.epoch_length != 0 {
            return;
        }
        self.epoch.advance_epoch(height);

        for p in self.epoch.drain_activations() {
            if let Some(v) = self.validators.get_mut(&p.addr) {
                v.active = true;
                v.activation_height = height;
                v.stake = p.stake;
            } else {
                let mut v = Validator::new(p.addr, p.bls_pk, p.stake);
                v.activation_height = height;
                let _ = self.validators.insert(v);
            }
        }

        for addr in self.epoch.drain_exits() {
            if let Some(v) = self.validators.get_mut(&addr) {
                v.active = false;
                v.exit_height = Some(height);
            }
        }

        self.validators.canonicalize();
    }

    pub fn current_target(&self) -> Option<AttestationTarget> {
        self.attestations.target
    }

    pub fn aggregate(&self) -> Result<Option<AggregatedAttestation>, ConsensusError> {
        self.attestations.aggregate(&self.validators)
    }
}
