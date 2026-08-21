use std::collections::HashMap;

use crate::blocks::Block;
use crate::transactions::Transaction;
use crate::utils::{generate_address, generate_bls_key};
use crate::validators::{Validator, ValidatorSet};
use crate::{PineAddress, PineBlsSignature, PineHash};

use blst::min_pk::{PublicKey, SecretKey, Signature};
use blst::BLST_ERROR;

const PREVOTE_DOMAIN: &[u8] = b"PINECONE_PREVOTE_V1";
const PRECOMMIT_DOMAIN: &[u8] = b"PINECONE_PRECOMMIT_V1";

const BLS_DST: &[u8] =
    b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteType {
    PreVote,
    PreCommit
}

#[derive(Debug, Clone)]
pub struct Vote {
    pub vote_type: VoteType,
    pub height: u64,
    pub round: u64,
    pub block_hash: Option<PineHash>,

    pub validator: PineAddress,
    pub signature: PineBlsSignature,
}

impl Vote {
    fn message(&self) -> Vec<u8> {
        let domain = match self.vote_type {
            VoteType::PreVote => PREVOTE_DOMAIN,
            VoteType::PreCommit => PRECOMMIT_DOMAIN
        };

        let mut message = Vec::new();
        message.extend_from_slice(domain);
        message.extend_from_slice(&self.height.to_le_bytes());
        message.extend_from_slice(&self.round.to_le_bytes());

        match self.block_hash {
            Some(hash) => message.extend_from_slice(&hash),
            None => message.extend_from_slice(&[0u8; 32]),
        }
        message
    }
}

pub struct ConsensusValidator {
    pub validator: Validator,
    pub secret_key: SecretKey,
}
impl ConsensusValidator {
    pub fn new(stake: u128) -> Self {
        let (secret_key, public_key) = generate_bls_key();
        let address = generate_address(&public_key.to_bytes());
        let validator = Validator { address, public_key: public_key.to_bytes(), stake: stake };
        Self {
            validator,
            secret_key
        }
    }

    pub fn prevote(&self, height: u64, round: u64, block_hash: Option<PineHash>) -> Vote {
        self.sign(VoteType::PreVote, height, round, block_hash)
    }

    pub fn precommit(&self, height: u64, round: u64, block_hash: Option<PineHash>) -> Vote {
        self.sign(VoteType::PreCommit, height, round, block_hash)
    }

    fn sign(&self, vote_type: VoteType, height: u64, round: u64,
        block_hash: Option<PineHash>) -> Vote {
        let mut vote = Vote {
            vote_type,
            height,
            round,
            block_hash,
            validator: self.validator.address,
            signature: [0u8; 96],
        };

        let message = vote.message();
        let signature = self.secret_key.sign(&message, BLS_DST, &[]);
        vote.signature = signature.to_bytes();
        vote
    }
}

#[derive(Debug)]
pub enum ConsensusError {
    UnknownValidator,
    InvalidSignature,
    WrongHeight,
    WrongRound,
    DuplicateVote,
}

pub struct VoteSet {
    pub vote_type : VoteType,
    pub height: u64,
    pub round: u64,

    votes: HashMap<[u8; 32], Vote>,
}

impl VoteSet {
    pub fn new(vote_type: VoteType, height: u64, round: u64) -> Self {
        Self {
            vote_type,
            height,
            round,
            votes: HashMap::new(),
        }
    }

    pub fn add(&mut self, vote: Vote, validator_set: &ValidatorSet)
        -> Result<(),ConsensusError> {
        if vote.vote_type != self.vote_type {
            return Err(ConsensusError::WrongRound);
        }
        if vote.height != self.height {
            return Err(ConsensusError::WrongHeight);
        }
        if vote.round != self.round {
            return Err(ConsensusError::WrongRound);
        }
        if self.votes.contains_key(&vote.validator) {
            return Err(ConsensusError::DuplicateVote);
        }

        let validator = validator_set.find(&vote.validator).ok_or(ConsensusError::UnknownValidator)?;
        let public_key = PublicKey::from_bytes(&validator.public_key)
            .map_err(|_| ConsensusError::InvalidSignature)?;
        let signature = Signature::from_bytes(&vote.signature)
            .map_err(|_| ConsensusError::InvalidSignature)?;

        let message = vote.message();
        let result = signature.verify(true, &message, BLS_DST, &[], &public_key, true);
        if result != BLST_ERROR::BLST_SUCCESS {
            return Err(ConsensusError::InvalidSignature);
        }

        self.votes.insert(vote.validator, vote);
        Ok(())
    }

    pub fn stake_for(&self, block_hash: PineHash, validator_set: &ValidatorSet) -> u128 {
        self.votes
            .values()
            .filter(|vote| vote.block_hash == Some(block_hash))
            .filter_map(|vote| validator_set.find(&vote.validator))
            .map(|validator| validator.stake)
            .sum()
    }

    pub fn has_quorum(&self, block_hash: PineHash, validator_set: &ValidatorSet) -> bool {
        let signed = self.stake_for(block_hash, validator_set);
        validator_set.has_quorum(signed)
    }

    pub fn votes(&self) -> impl Iterator<Item = &Vote> {
        self.votes.values()
    }
}

pub struct ConsensusEngine {
    pub validator: ConsensusValidator,
    pub height: u64,
    pub round: u64,
    pub locked_block: Option<PineHash>,
    pub locked_round: Option<u64>,
    pub prevotes: VoteSet,
    pub precommits: VoteSet,
    pub preposed_block: Option<Block>,
}

impl ConsensusEngine {
    pub fn new(validator: ConsensusValidator, height: u64, round: u64) -> Self {
        Self {
            validator,
            height,
            round,
            locked_block: None,
            locked_round: None,
            prevotes: VoteSet::new(VoteType::PreVote, height, round),
            precommits: VoteSet::new(VoteType::PreCommit, height, round),
            preposed_block: None
        }
    }

    pub fn start_round(&mut self, round: u64) {
        self.round = round;

        self.prevotes = VoteSet::new(VoteType::PreVote, self.height, round);
        self.precommits = VoteSet::new(VoteType::PreCommit, self.height, round);
        self.preposed_block = None;
    }

    pub fn is_proposer(&self, validators: &ValidatorSet, previous_hash: PineHash) -> bool {
        validators.preposer(previous_hash, self.height, self.round).address
            == self.validator.validator.address
    }

    pub fn propose(&mut self, validators: &ValidatorSet,
        previous_hash: PineHash, transactions: Vec<Transaction>,
        state_root: PineHash) -> Option<Block> {
        if !self.is_proposer(validators, previous_hash) {
            return None;
        }

        let block = Block::new(
            self.height,
            self.round,
            previous_hash,
            self.validator.validator.address,
            state_root,
            transactions
        );

        self.preposed_block = Some(block.clone());
        Some(block)
    }

    pub fn receive_proposal(
        &mut self,
        block: Block,
        validators: &ValidatorSet,
        previous_block: &Block) -> Result<Vote, ConsensusError> {
        if block.header.height != self.height {
            return Err(ConsensusError::WrongHeight);
        }
        if block.header.round != self.round {
            return Err(ConsensusError::WrongRound);
        }
        let vote = if !block.validate_basic(&previous_block.header) {
            self.prevote(None)
        } else {
            let proposer = validators.preposer(previous_block.hash(), self.height, self.round);
            if proposer.address != block.header.proposer {
                return Ok(self.prevote(None));
            }
            let block_hash = block.hash();
            self.preposed_block = Some(block);
            if let Some(locked) = self.locked_block {
                if locked != block_hash {
                    self.prevote(None)
                } else {
                    self.prevote(Some(block_hash))
                }
            } else {
                self.prevote(Some(block_hash))
            }
        };
        self.prevotes.add(
            vote.clone(),
            validators,
        )?;

        Ok(vote)
    }

    fn prevote(&self, block_hash: Option<PineHash>) -> Vote {
        self.validator.sign(VoteType::PreVote, self.height, self.round, block_hash)
    }
    fn precommit(&self, block_hash: Option<PineHash>) -> Vote {
        self.validator.sign(VoteType::PreCommit, self.height, self.round, block_hash)
    }

    pub fn receive_prevote(&mut self, vote: Vote, validators: &ValidatorSet)
        -> Result<Option<Vote>, ConsensusError> {
        self.prevotes.add(vote, validators)?;
        let block_hash = match self.prevotes.votes().find_map(|v| {v.block_hash}) {
            Some(hash) => hash,
            None => return Ok(None),
        };
        if !self.prevotes.has_quorum(block_hash, validators) {
            return Ok(None);
        }
        self.locked_block = Some(block_hash);
        self.locked_round = Some(self.round);

        let precommit = self.precommit(Some(block_hash));

        self.precommits.add(
            precommit.clone(),
            validators,
        )?;

        Ok(Some(precommit))
    }

    pub fn receive_precommit(&mut self, vote: Vote, validators: &ValidatorSet)
        -> Result<bool, ConsensusError> {
        self.precommits.add(vote, validators)?;
        let block_hash = match self.precommits.votes().find_map(|v| {v.block_hash}) {
            Some(hash) => hash,
            None => return Ok(false),
        };
        if !self.precommits.has_quorum(block_hash, validators) {
            return Ok(false);
        }
        Ok(true)
    }

    pub fn next_round(&mut self) {
        self.start_round(self.round + 1);
    }
}
