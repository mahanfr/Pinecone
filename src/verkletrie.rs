use std::fmt;

use ark_bls12_381::{Fr, G1Projective};
use ark_ec::PrimeGroup;
use ark_ff::Zero;
use ark_poly::{DenseUVPolynomial, univariate::DensePolynomial};
use ark_serialize::CanonicalSerialize;

use crate::{kzg::KZG, utils::ToBytes};

pub const KEY_LEN: usize = 32;
pub const ARITY: usize = 256;
const VERKLE_LEAF_DOMAIN: &[u8] = b"VERKLE_LEAF_V1";

pub struct SparseVerkleTrie<T: ToBytes + Clone> {
    kzg: KZG,
    root: VerkleNode<T>
}

impl<T: Clone + ToBytes> SparseVerkleTrie<T> {
    pub fn new() -> Self {
        Self {
            kzg: KZG::new(ARITY - 1),
            root: VerkleNode::Branch {
                children: empty_children(),
            }
        }
    }
    pub fn insert(&mut self, key: &[u8], value: T) -> Result<(), TrieError> {
        ensure_key(key)?;
        self.root.insert(key, 0, value);
        Ok(())
    }
    pub fn get(&mut self, key: &[u8]) -> Result<Option<&T>, TrieError> {
        ensure_key(key)?;
        Ok(self.root.get(key, 0))
    }
    pub fn delete(&mut self, key: &[u8]) -> Result<bool, TrieError> {
        ensure_key(key)?;
        Ok(self.root.delete(key, 0))
    }
    fn root_commitment(&self) -> G1Projective {
        self.root.commit(&self.kzg, 0, &[])
    }
    pub fn root_bytes(&self) -> [u8; 48] {
        let commitment = self.root_commitment();
        let mut out = [0u8; 48];
        commitment.serialize_compressed(&mut out[..])
            .expect("G1 serialize failed");
        out
    }
    pub fn prove(&self, key: &[u8]) -> Result<VerkleProof, TrieError> {
        ensure_key(key)?;
        self.root.prove(&self.kzg, key, 0)
    }
    pub fn verify_proof(&self, root: G1Projective, key: &[u8], proof: &VerkleProof) -> bool {
        if key.len() != KEY_LEN { return false; }
        if proof.key != key { return false; }
        if proof.levels.is_empty() { return false; }

        let mut current_commitment = root;

        for (depth, level) in proof.levels.iter().enumerate() {
            if depth >= KEY_LEN {return false;}
            let expected_index = key[depth];
            if level.index != expected_index {
                return false;
            }
            let z = Fr::from(level.index as u64);
            if !self.kzg.verify(current_commitment, z, level.evaluation, level.kzg_proof) {
                return false;
            }
            let expected_scalar = KZG::hash_g1_to_scalar(&level.child_commitment);
            if level.evaluation != expected_scalar {
                return false;
            }
            if level.child_commitment.is_zero() {
                return proof.value.is_none() && depth + 1 == proof.levels.len();
            }
            current_commitment = level.child_commitment;
        }
        if proof.levels.len() != KEY_LEN { return false; }
        let value = match &proof.value {
            Some(value) => value,
            None => return false,
        };
        let leaf_commitment = leaf_commitment(key, value);
        if current_commitment != leaf_commitment { return false; }
        true
    }
}

enum VerkleNode<T: ToBytes + Clone> {
    Empty,
    Leaf { value: T},
    Branch { children: Vec<Option<Box<VerkleNode<T>>>> },
}

impl<T: Clone + ToBytes> VerkleNode<T> {
    pub fn new() -> Self {
        Self::Empty
    }

    pub fn insert(&mut self, key: &[u8], depth: usize, value: T) {
        if depth == KEY_LEN {
            *self = VerkleNode::Leaf { value };
            return;
        }
        match self {
            Self::Empty => {
                *self = Self::Branch { children: empty_children() };
                self.insert(key, depth, value);
            },
            Self::Leaf { .. } => {
                panic!("invalid trie structure: leaf before depth 32");
            },
            Self::Branch { children } => {
                let index = key[depth] as usize;
                let child = children[index].get_or_insert_with(|| {
                    Box::new(VerkleNode::Empty)
                });
                child.insert(&key, depth + 1, value);
            }
        }
    }

    pub fn get(&self, key: &[u8], depth: usize) -> Option<&T> {
        if depth == KEY_LEN {
            return match self {
                Self::Leaf { value } => Some(value),
                _ => None,
            };
        }
        match self {
            Self::Empty => None,
            Self::Leaf { .. } => None,
            Self::Branch { children } => {
                children[key[depth] as usize].as_ref()
                    .and_then(|child| child.get(key, depth + 1))
            }
        }
    }

    pub fn delete(&mut self, key: &[u8], depth: usize) -> bool {
        if depth == KEY_LEN {
            if matches!(self, VerkleNode::Leaf { .. }) {
                *self = VerkleNode::Empty;
                return true;
            }
            return false;
        }
        let deleted = match self {
            VerkleNode::Branch { children } => {
                let index = key[depth] as usize;
                match children[index].as_mut() {
                    Some(child) => {
                        let deleted = child.delete(key, depth + 1);
                        if deleted && child.is_empty() {
                            children[index] = None;
                        }
                        deleted
                    }
                    None => false,
                }
            }
            _ => false,
        };
        if deleted && self.is_empty_branch() {
            *self = VerkleNode::Empty;
        }
        deleted
    }

    fn is_empty(&self) -> bool {
        matches!(self, VerkleNode::Empty)
    }

    fn is_empty_branch(&self) -> bool {
        match self {
            VerkleNode::Branch { children } => {
                children.iter().all(|c| c.is_none())
            }
            _ => false
        }
    }

    pub fn commit(&self, kzg: &KZG, depth: usize, key_prefix: &[u8]) -> G1Projective {
        match self {
            Self::Empty => G1Projective::zero(),
            Self::Leaf { value } => {
                assert_eq!(depth, KEY_LEN, "leaf reached before depth 32");
                leaf_commitment(key_prefix, &value.to_bytes())
            }
            Self::Branch { children } => {
                let mut coefficients = vec![Fr::zero(); ARITY];

                for (index, child) in children.iter().enumerate() {
                    let child_commit = match child {
                        Some(child) => child.commit(
                            kzg,
                            depth + 1,
                            &extend_key(key_prefix, index as u8),
                        ),
                        None => G1Projective::zero(),
                    };
                    coefficients[index] = KZG::hash_g1_to_scalar(&child_commit);
                }

                let poly = DensePolynomial::from_coefficients_vec(coefficients);
                kzg.commit(&poly)
            }
        }
    }

    fn prove(&self, kzg: &KZG, key: &[u8], depth: usize) -> Result<VerkleProof, TrieError> {
        match self {
            Self::Empty => {
                Err(TrieError::InvalidKeyLength { expected: KEY_LEN, actual: key.len() })
            }
            Self::Leaf { value } => {
                if depth != KEY_LEN {
                    panic!("leaf encounterd before depth 32");
                }
                Ok(VerkleProof {
                    key: key.to_vec(),
                    value: Some(value.to_bytes()),
                    levels: Vec::new()
                })
            }
            Self::Branch { children } => {
                let index = key[depth] as usize;
                let child_commitment = match &children[index] {
                    Some(child) => child.commit(
                        kzg,
                        depth + 1,
                        &extend_key(&key[..depth], index as u8)
                    ),
                    None => G1Projective::zero()
                };
                let mut coefficients = vec![Fr::zero(); ARITY];
                for i in 0..ARITY {
                    let commitment = match &children[i] {
                        Some(child) => child.commit(
                            kzg,
                            depth + 1,
                            &extend_key(&key[..depth], i as u8)
                        ),
                        None => G1Projective::zero()
                    };

                    coefficients[i] = KZG::hash_g1_to_scalar(&commitment);
                }
                let poly = DensePolynomial::from_coefficients_vec(coefficients);
                let (evaluation, kzg_proof) = kzg.open(&poly, Fr::from(index as u64));
                let level = VerkleProofLevel {
                    index: index as u8,
                    evaluation,
                    child_commitment,
                    kzg_proof
                };

                match &children[index] {
                    Some(child) => {
                        let child_proof = child.prove(kzg, key, depth + 1)?;
                        let mut levels = Vec::with_capacity(child_proof.levels.len() + 1);
                        levels.push(level);
                        levels.extend(child_proof.levels);
                        Ok(VerkleProof {
                            key: key.to_vec(),
                            value: child_proof.value,
                            levels,
                        })
                    }
                    None => {
                        Ok(VerkleProof {
                            key: key.to_vec(),
                            value: None,
                            levels: vec![level],
                        })
                    }
                }
            }
        }
    }

}

pub struct VerkleProof {
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>,
    pub levels: Vec<VerkleProofLevel>,
}

pub struct VerkleProofLevel {
    pub index: u8,
    pub evaluation: Fr,
    pub child_commitment: G1Projective,
    pub kzg_proof: G1Projective,
}


#[derive(Debug, PartialEq)]
pub enum TrieError {
    InvalidKeyLength {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for TrieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrieError::InvalidKeyLength { expected, actual } => {
                write!(
                    f,
                    "invalid key length: expected {}, got {}",
                    expected,
                    actual
                )
            }
        }
    }
}

impl std::error::Error for TrieError {}

fn extend_key(prefix: &[u8], byte: u8) -> Vec<u8> {
    let mut result = Vec::with_capacity(prefix.len() + 1);
    result.extend_from_slice(prefix);
    result.push(byte);
    result
}

fn ensure_key(key: &[u8]) -> Result<(), TrieError> {
    if key.len() != KEY_LEN {
        return Err(TrieError::InvalidKeyLength { expected: KEY_LEN, actual: key.len() });
    }
    Ok(())
}

fn empty_children<T: ToBytes + Clone>() -> Vec<Option<Box<VerkleNode<T>>>> {
    (0..ARITY).map(|_| None).collect()
}

fn leaf_commitment(key: &[u8], value: &[u8]) -> G1Projective {
    let mut data = Vec::new();
    data.extend_from_slice(VERKLE_LEAF_DOMAIN);
    data.extend_from_slice(key);
    data.extend_from_slice(value);
    let scalar = KZG::hash_to_scalar(&data);
    G1Projective::generator() * scalar
}

#[cfg(test)]
mod tests {
    use crate::verkletrie::SparseVerkleTrie;

    fn key(x: u8) -> [u8; 32] {
        let mut k = [0u8;32];
        k[31] = x;
        k
    }

    #[test]
    fn insert_and_get() {
        let mut trie = SparseVerkleTrie::new();
        trie.insert(&key(1),  100).unwrap();
        trie.insert(&key(2),  420).unwrap();

        assert_eq!(trie.get(&key(1)), Ok(Some(&100)));
        assert_eq!(trie.get(&key(2)), Ok(Some(&420)));
    }

    #[test]
    fn update_changes_value() {
        let mut trie = SparseVerkleTrie::new();
        trie.insert(&key(1), 100).unwrap();

        let root1 = trie.root_bytes();
        trie.insert(&key(1), 200).unwrap();
        let root2 = trie.root_bytes();
        assert_ne!(root1, root2);
        assert_eq!(trie.get(&key(1)).unwrap(), Some(&200));
    }
}
