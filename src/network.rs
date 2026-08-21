use std::collections::VecDeque;
use base64::prelude::*;

use crate::{blocks::{Block, genesis}, consensus::{ConsensusEngine, ConsensusValidator, Vote, VoteType}, validators::{ValidatorSet}};

#[derive(Debug, Clone)]
enum NetworkMessage {
    Preposal(Block),
    Vote(Vote),
}

struct Network {
    queue: VecDeque<(usize, NetworkMessage)>,
}

impl Network {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new()
        }
    }

    pub fn broadcast(&mut self, from: usize, message: NetworkMessage, count: usize) {
        for to in 0..count {
            if to != from {
                self.queue.push_back((to, message.clone()));
            }
        }
    }

    pub fn pop(&mut self) -> Option<(usize, NetworkMessage)> {
        self.queue.pop_front()
    }
}

pub fn simulate() {
    let mut cons_validators = Vec::new();

    for _ in 0..4 {
        cons_validators.push(ConsensusValidator::new(100));
    }

    let validator_set = ValidatorSet::new(cons_validators.iter().map(|v| v.validator.clone()).collect());
    let mut engines: Vec<ConsensusEngine> =
        cons_validators.into_iter().map(|v| ConsensusEngine::new(v, 1, 0)).collect();
    let genesis = genesis();
    let previous_hash = genesis.hash();
    let mut network = Network::new();
    let proposer = validator_set.preposer(previous_hash, 1, 0);
    let proposer_index = engines.iter().position(|engine| engine.validator.validator.address == proposer.address).unwrap();
    println!("round 0 proposer = validator {:0x?}",BASE64_STANDARD.encode(proposer.address));
    let block = engines[proposer_index].propose(&validator_set, previous_hash, Vec::new(), [0u8; 32]).unwrap();
    println!("validator {:0x?} proposed block {:02x?}", BASE64_STANDARD.encode(proposer.address), &block.hash()[..4]);

    let proposer_vote = engines[proposer_index].receive_proposal(
        block.clone(), &validator_set, &genesis).unwrap();

    network.broadcast(proposer_index, NetworkMessage::Preposal(block), engines.len());
    network.broadcast(proposer_index, NetworkMessage::Vote(proposer_vote), engines.len());

    while let Some((reciver, message)) = network.pop() {
        match message {
            NetworkMessage::Preposal(block) => {
                let vote = engines[reciver].receive_proposal(block, &validator_set, &genesis).unwrap();
                network.broadcast(reciver, NetworkMessage::Vote(vote), engines.len());
            },
            NetworkMessage::Vote(vote) => {
                match vote.vote_type {
                    VoteType::PreVote => {
                        let precommit = engines[reciver].receive_prevote(vote, &validator_set).unwrap();
                        if let Some(precommit) = precommit {
                            network.broadcast(reciver, NetworkMessage::Vote(precommit), engines.len());
                        }
                    },
                    VoteType::PreCommit => {
                        let finalized = engines[reciver].receive_precommit(vote, &validator_set).unwrap();
                        if finalized {
                            let addr = engines[reciver].validator.validator.address;
                            println!("validator {} FINALIZED block", BASE64_STANDARD.encode(addr))
                        }
                    }
                }
            }
        }
    }
}
