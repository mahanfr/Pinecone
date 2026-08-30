use std::collections::HashMap;

use ark_bls12_381::{Fr, G1Projective};
use ark_ff::Zero;
use ark_poly::{DenseUVPolynomial, univariate::DensePolynomial};
use ark_serialize::CanonicalSerialize;

use crate::{kzg::KZG, utils::ToBytes};

pub struct SparseVerkleTrie<T: ToBytes> {
    kzg: KZG,
    children: VerkleNode<T>
}

impl<T: Clone + ToBytes> SparseVerkleTrie<T> {
    pub fn new() -> Self {
        let kzg = KZG::new(256);
        Self { kzg, children: VerkleNode::new() }
    }
    pub fn insert(&mut self, key: &[u8], value: T) {
        self.children.insert(key, value);
    }
    pub fn get(&mut self, key: &[u8]) -> Option<&T> {
        self.children.get(key)
    }
    pub fn commit(&self) -> [u8;48] {
        let mut bytes = [0u8;48];
        self.children.commit(&self.kzg).serialize_compressed(&mut bytes[..]).unwrap();
        bytes
    }
}

enum VerkleNode<T: ToBytes> {
    Empty,
    Leaf { path: Vec<u8>, value: T},
    Branch { children: HashMap<u8, Box<VerkleNode<T>>>, value: Option<T> },
}

impl<T: Clone + ToBytes> VerkleNode<T> {
    pub fn new() -> Self {
        Self::Empty
    }

    pub fn insert(&mut self, key: &[u8], value: T) {
        match self {
            Self::Empty => {
                *self = Self::Leaf { path: key.to_vec(), value };
            },
            Self::Leaf { path, value: lvalue } => {
                if key == path.as_slice() {
                    *lvalue = value;
                    return;
                }
                // grow a branch
                else {
                    let old_path = path.clone();
                    let old_value = lvalue.clone();
                    let mut new_branch = VerkleNode::Branch {
                        children: HashMap::new(), value: None
                    };
                    new_branch.insert(&old_path, old_value);
                    new_branch.insert(key, value);
                    *self = new_branch;
                }
            },
            Self::Branch { children, value: bvalue } => {
                // TODO: Mabey it is better for a keyless branch to be a leaf
                if key.is_empty() {
                    *bvalue = Some(value);
                    return;
                }

                let first = key[0];
                let child = children
                    .entry(first)
                    .or_insert_with(|| Box::new(VerkleNode::Empty));
                child.insert(&key[1..], value);
            }
        }
    }

    pub fn get(&self, key: &[u8]) -> Option<&T> {
        match self {
            Self::Empty => None,
            Self::Leaf { path, value } => {
                if key == path.as_slice() {
                    Some(value)
                } else {
                    None
                }
            }
            Self::Branch { children, value } => {
                if key.is_empty() {
                    return value.as_ref();
                }
                let first = key[0];
                if let Some(child) = children.get(&first) {
                    child.get(&key[1..])
                } else {
                    None
                }
            }
        }
    }

    pub fn commit(&self, kzg: &KZG) -> G1Projective {
        match self {
            Self::Empty => G1Projective::zero(),
            Self::Leaf { path, value } => {
                let mut data = Vec::new();
                data.extend_from_slice(path);
                data.extend_from_slice(&value.to_bytes());
                let scalar = KZG::hash_to_scalar(&data);
                kzg.g1 * scalar
            }
            Self::Branch { children, value } => {
                let mut coefficients = vec![Fr::zero(); 256];

                for (byte, child) in children.iter() {
                    let child_commit = child.commit(kzg);
                    let mut bytes = Vec::new();
                    child_commit.serialize_uncompressed(&mut bytes).unwrap();
                    let scalar = KZG::hash_to_scalar(&bytes);
                    coefficients[*byte as usize] = scalar;
                }

                if let Some(val) = value {
                    let scalar = KZG::hash_to_scalar(&val.to_bytes());
                    coefficients[255] = scalar;
                }
                let poly = DensePolynomial::from_coefficients_vec(coefficients);
                kzg.commit(&poly)
            }
        }
    }

    pub fn prove_key(&self, kzg: &KZG, key: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
        self.prove_key_internal(kzg, key, Vec::new())
    }

    fn prove_key_internal(&self, kzg: &KZG, key: &[u8], path_so_far: Vec<u8>)
        -> Option<(Vec<u8>, Vec<u8>)> {
        match self {
            VerkleNode::Empty => None,
            VerkleNode::Leaf { path, value } => {
                if key == path.as_slice() {
                    let proof_bytes = vec![0u8; 48];
                    Some((value.to_bytes(), proof_bytes))
                } else {
                    None
                }
            }
            VerkleNode::Branch { children, value } => {
                if key.is_empty() {
                    if let Some(val) = value {
                        let proof_bytes = vec![0u8; 48];
                        return Some((val.to_bytes(), proof_bytes));
                    }
                    return None;
                }
                let first = key[0];
                if let Some(child) = children.get(&first) {
                    let mut new_path = path_so_far.clone();
                    new_path.push(first);
                    child.prove_key_internal(kzg, &key[1..], new_path)
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::verkletrie::VerkleNode;

    #[test]
    fn insertion_and_retrival_of_trie() {
        let mut trie = VerkleNode::<u32>::new();
        trie.insert(&[1,2,3], 100);
        trie.insert(&[1,2,4], 420);
        trie.insert(&[1,2,3], 101);
        trie.insert(&[1,3],   69);
        trie.insert(&[2],     85);

        assert_eq!(trie.get(&[1,2,3]), Some(&101));
        assert_eq!(trie.get(&[1,2,4]), Some(&420));
        assert_eq!(trie.get(&[1,3]),   Some(&69));
        assert_eq!(trie.get(&[2]),     Some(&85));
    }
}
