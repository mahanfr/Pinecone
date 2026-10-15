use ethnum::u256;

use crate::types::{IonicAddr, IonicGroupPk, IonicHash};

use super::slashing::SlashingEvidence;

pub const EPOCH_LENGTH: u64 = 32;
pub const ACTIVATION_DELAY_EPOCHS: u64 = 2;
pub const EXIT_DELAY_EPOCHS: u64 = 2;

#[derive(Debug, Clone)]
pub struct PendingValidator {
    pub addr: IonicAddr,
    pub bls_pk: IonicGroupPk,
    pub stake: u256,
    pub activation_epoch: u64,
}

#[derive(Debug, Clone)]
pub struct EpochState {
    pub current_epoch: u64,
    pub start_height: u64,
    pub randao_mix: IonicHash,
    pub pending_activations: Vec<PendingValidator>,
    pub pending_exits: Vec<(IonicAddr, u64)>,
    pub slashings: Vec<SlashingEvidence>,
}

impl EpochState {
    pub fn new(genesis_hash: IonicHash) -> Self {
        Self {
            current_epoch: 0,
            start_height: 0,
            randao_mix: genesis_hash,
            pending_activations: Vec::new(),
            pending_exits: Vec::new(),
            slashings: Vec::new(),
        }
    }

    pub fn epoch_for_height(&self, height: u64, epoch_length: u64) -> u64 {
        height / epoch_length
    }

    pub fn is_boundary(&self, height: u64, epoch_length: u64) -> bool {
        height > 0 && height % epoch_length == 0
    }

    pub fn mix_randao(&mut self, commitment: &IonicHash) {
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(self.randao_mix.as_ref());
        buf.extend_from_slice(commitment.as_ref());
        self.randao_mix = blake3::hash(&buf).into();
    }

    pub fn schedule_activation(&mut self, addr: IonicAddr, bls_pk: IonicGroupPk, stake: u256) {
        self.pending_activations.push(PendingValidator {
            addr,
            bls_pk,
            stake,
            activation_epoch: self.current_epoch + ACTIVATION_DELAY_EPOCHS,
        });
    }

    pub fn schedule_exit(&mut self, addr: IonicAddr) {
        self.pending_exits
            .push((addr, self.current_epoch + EXIT_DELAY_EPOCHS));
    }

    pub fn advance_epoch(&mut self, height: u64) {
        self.current_epoch += 1;
        self.start_height = height;
    }

    pub fn drain_activations(&mut self) -> Vec<PendingValidator> {
        let epoch = self.current_epoch;
        let mut ready = Vec::new();
        self.pending_activations.retain(|p| {
            if p.activation_epoch <= epoch {
                ready.push(p.clone());
                false
            } else {
                true
            }
        });
        ready
    }

    pub fn drain_exits(&mut self) -> Vec<IonicAddr> {
        let epoch = self.current_epoch;
        let mut ready = Vec::new();
        self.pending_exits.retain(|(addr, e)| {
            if *e <= epoch {
                ready.push(*addr);
                false
            } else {
                true
            }
        });
        ready
    }
}
