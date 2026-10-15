use ethnum::{AsU256, u256};

use crate::types::{IonicAddr, IonicHash};

use super::validator::Validator;

pub const SLASH_DOUBLE_VOTE_BPS: u16 = 500; // 5%
pub const SLASH_DOUBLE_PROPOSAL_BPS: u16 = 500;
pub const JAIL_DURATION_HEIGHTS: u64 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashingKind {
    DoubleVote { first: IonicHash, second: IonicHash },
    DoubleProposal { first: IonicHash, second: IonicHash },
}

#[derive(Debug, Clone)]
pub struct SlashingEvidence {
    pub offender: IonicAddr,
    pub kind: SlashingKind,
    pub height: u64,
    pub round: u64,
}

impl SlashingEvidence {
    pub fn slash_amount(&self, v: &Validator) -> u256 {
        let bps = match self.kind {
            SlashingKind::DoubleVote { .. } => SLASH_DOUBLE_VOTE_BPS,
            SlashingKind::DoubleProposal { .. } => SLASH_DOUBLE_PROPOSAL_BPS,
        };
        (v.effective_stake() * u256::from(bps)) / 10_000u32.as_u256()
    }
}
