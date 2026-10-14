use std::fmt::Display;

use ethnum::u256;

use crate::blockchian::BlockchainError;


#[derive(Debug)]
pub enum ConsensusError {
    UnknownValidator,
    InactiveValidator,
    InsufficientStake { have: u256, need: u256 },
    InvalidBlsKey,
    InvalidBlsSignature,
    AlreadyRegistered,
    NotAValidator,
    WrongHeight { expected: u64, got: u64 },
    WrongRound { expected: u64, got: u64 },
    WrongChainId,
    ParentHashMismatch,
    InvalidProposer,
    Equivocation(&'static str),
    FinalityNotReached,
    DuplicateAttestation,
    InvalidBitfield,
    NoQuorum,
    Blockchain(BlockchainError),
}

impl Display for ConsensusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownValidator => write!(f, "Unknown Validator"),
            Self::InactiveValidator => write!(f, "Inactive Validator"),
            Self::InsufficientStake { have, need } => write!(f, "Insufficient Stake: You have {have} but you need {need}"),
            Self::InvalidBlsKey => write!(f, "Invalid Bls Key"),
            Self::InvalidBlsSignature => write!(f, "Invalid Bls Signature"),
            Self::AlreadyRegistered => write!(f, "Already Registered"),
            Self::NotAValidator => write!(f, "Not A Validator"),
            Self::WrongHeight { expected, got } => write!(f, "Wrong Height: expected height {expected} but you got {got}"),
            Self::WrongRound { expected, got } => write!(f, "Wrong Round: expected round {expected} but you got {got}"),
            Self::WrongChainId => write!(f, "Wrong Chain Id"),
            Self::ParentHashMismatch => write!(f, "Parent Hash Mismatch"),
            Self::InvalidProposer => write!(f, "Invalid Proposer"),
            Self::Equivocation(msg) => write!(f, "Equivocation Detected: {msg}"),
            Self::FinalityNotReached => write!(f, "Finality Not Reached"),
            Self::DuplicateAttestation => write!(f, "Duplicate Attestation"),
            Self::InvalidBitfield => write!(f, "Invalid Bit field"),
            Self::NoQuorum => write!(f, "No Quorum"),
            Self::Blockchain(berror) => write!(f, "Blockchain Error: {berror}"),
        }
    }
}

impl std::error::Error for ConsensusError {}

impl From<BlockchainError> for ConsensusError {
    fn from(value: BlockchainError) -> Self {
        Self::Blockchain(value)
    }
}
