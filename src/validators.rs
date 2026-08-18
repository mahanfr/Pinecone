use crate::{Address, Hash};

type BlsPublicKey = [u8; 48];

pub struct Validator {
    pub address: Address,
    pub public_key: BlsPublicKey,
    pub stake: u128,
}

pub struct ValidatorSet {
    pub validators: Vec<Validator>,
}
impl ValidatorSet {
    pub fn new(mut validators: Vec<Validator>) -> Self {
        validators.sort_by_key(|v| v.address);

        Self {validators}
    }

    // TODO: This should fail gracefully
    pub fn preposer(&self, prev_block_hash: Hash, height: u64, round: u64) -> &Validator {
        assert!(!self.validators.is_empty());
        let mut input = Vec::new();
        input.extend_from_slice(&prev_block_hash);
        input.extend_from_slice(&height.to_le_bytes());
        input.extend_from_slice(&round.to_le_bytes());
        
        let hash = blake3::hash(&input);

        // TODO: This is a insecure way to make a determenestic random number
        // consider:
        // VRFs
        // threshold BLS randomness
        // RANDAO-style schemes
        // beacon-chain-style randomness
        let mut random_bytes = [0u8; 16];
        random_bytes.copy_from_slice(&hash.as_bytes()[..16]);

        let random = u128::from_le_bytes(random_bytes);
        let total_stake = self.total_stake();
        let target = random % total_stake;

        let mut accumulated = 0u128;

        for validator in &self.validators {
            accumulated += validator.stake;

            if target < accumulated {
                return validator;
            }
        }

        unreachable!("No Validator has been chosen!");
    }

    // TODO: What if Sum(validators) > u128::MAX
    pub fn total_stake(&self) -> u128 {
        self.validators.iter().map(|v| v.stake).sum()
    }
}
