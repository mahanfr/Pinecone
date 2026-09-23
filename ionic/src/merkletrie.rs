use ethnum::AsU256;

use crate::{serialization::{CodecError, read_slice, read_uvarint, write_uvarint}, types::IonicHash, utils::{FromBytes, ToBytes}, verkletrie::TrieError};

const KEY_LEN: usize = 32;
pub const ARITY: usize = 256;
const MERKLE_LEAF_DOMAIN: &[u8] = b"IONIC_MERKLE_LEAF_V1";
const MERKLE_BRANCH_DOMAIN: &[u8] = b"IONIC_MERKLE_BRANCH_V1";
const MERKLE_EMPTY_HASH_DOMAIN: &[u8] = b"IONIC_MERKLE_EMPTY_HASH";

const SERIALIZATION_VERSION: u8 = 1;
const TAG_EMPTY: u8 = 0;
const TAG_LEAF: u8 = 1;
const TAG_BRANCH: u8 = 2;

#[derive(Debug, Clone)]
pub struct SparseMerkleTrie<T: ToBytes + Clone> {
    root: MerkleNode<T>,
}

impl<T: Clone + ToBytes> Default for SparseMerkleTrie<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + ToBytes> PartialEq for SparseMerkleTrie<T> {
    fn eq(&self, other: &Self) -> bool {
        let mut this = self.clone();
        let mut other = other.clone();
        this.root_hash() == other.root_hash()
    }
}

impl<T: Clone + ToBytes> SparseMerkleTrie<T> {
    pub fn new() -> Self {
        Self {
            root: MerkleNode::Branch(MerkleNodeBranch::new_empty()),
        }
    }

    pub fn insert(&mut self, key: &[u8], value: T) -> Result<(), TrieError> {
        ensure_key(key)?;
        self.root.insert(key, 0, value);
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<&T>, TrieError> {
        ensure_key(key)?;
        Ok(self.root.get(key, 0))
    }

    pub fn delete(&mut self, key: &[u8]) -> Result<bool, TrieError> {
        ensure_key(key)?;
        Ok(self.root.delete(key, 0))
    }

    pub fn root_hash(&mut self) -> IonicHash {
        self.root.hash()
    }

    pub fn static_root_hash(&self) -> IonicHash {
        self.root.static_hash()
    }
}

impl<T: Clone + ToBytes + FromBytes> SparseMerkleTrie<T> {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(SERIALIZATION_VERSION);
        self.root.encode(&mut buf);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CodecError> {
        let mut cursor = 0usize;
        let version = *bytes.get(cursor).ok_or(CodecError::UnexpectedEof)?;
        cursor += 1;
        if version != SERIALIZATION_VERSION {
            return Err(CodecError::InvalidTag(version));
        }
        let mut key = [0u8; KEY_LEN];
        let root = MerkleNode::decode(bytes, &mut cursor, &mut key, 0)?;
        if cursor != bytes.len() {
            return Err(CodecError::TrailingBytes);
        }
        Ok(Self { root })
    }
}


#[derive(Debug, Clone)]
struct MerkleNodeBranch<T: ToBytes + Clone> {
    children: Vec<Option<Box<MerkleNode<T>>>>,
    occupied: Vec<u8>,
    hash: IonicHash,
    dirty: bool,
}

impl<T: ToBytes + Clone> MerkleNodeBranch<T> {
    pub fn new_empty() -> Self {
        Self {
            children: empty_children(),
            occupied: Vec::new(),
            hash: empty_hash(),
            dirty: false,
        }
    }
}

#[derive(Debug, Clone)]
struct MerkleNodeLeaf<T: ToBytes + Clone> {
    value: T,
    hash: IonicHash,
}

#[derive(Debug, Clone)]
enum MerkleNode<T: ToBytes + Clone> {
    Empty,
    Leaf(MerkleNodeLeaf<T>),
    Branch(MerkleNodeBranch<T>),
}

impl<T: Clone + ToBytes> MerkleNode<T> {
    pub fn insert(&mut self, key: &[u8], depth: usize, value: T) {
        if depth == KEY_LEN {
            let hash = leaf_hash(key, &value.to_bytes());
            *self = MerkleNode::Leaf(MerkleNodeLeaf { hash, value });
            return;
        }
        match self {
            Self::Empty => {
                *self = Self::Branch(MerkleNodeBranch::new_empty());
                self.insert(key, depth, value);
            }
            Self::Leaf(_) => {
                panic!("invalid trie structure: leaf before depth 32");
            }
            Self::Branch(branch) => {
                branch.dirty = true;
                let index = key[depth] as usize;
                if branch.children[index].is_none() {
                    let pos = branch.occupied.partition_point(|&i| i < index as u8);
                    branch.occupied.insert(pos, index as u8);
                    branch.children[index] = Some(Box::new(MerkleNode::Empty));
                }
                branch.children[index]
                    .as_mut()
                    .unwrap()
                    .insert(key, depth + 1, value);
            }
        }
    }

    pub fn get(&self, key: &[u8], depth: usize) -> Option<&T> {
        if depth == KEY_LEN {
            return match self {
                Self::Leaf(leaf) => Some(&leaf.value),
                _ => None,
            };
        }
        match self {
            Self::Empty => None,
            Self::Leaf(_) => None,
            Self::Branch(branch) => branch.children[key[depth] as usize]
                .as_deref()
                .and_then(|child| child.get(key, depth + 1)),
        }
    }

    pub fn delete(&mut self, key: &[u8], depth: usize) -> bool {
        if depth == KEY_LEN {
            if matches!(self, MerkleNode::Leaf(_)) {
                *self = MerkleNode::Empty;
                return true;
            }
            return false;
        }
        let deleted = match self {
            MerkleNode::Branch(branch) => {
                let index = key[depth] as usize;
                let child = match branch.children[index].as_mut() {
                    Some(child) => child,
                    None => return false,
                };
                let deleted = child.delete(key, depth + 1);
                if !deleted {
                    return false;
                }
                branch.dirty = true;
                if child.is_empty() {
                    branch.children[index] = None;
                    branch.occupied.retain(|&i| i != index as u8);
                }
                true
            }
            _ => false,
        };
        if deleted && self.is_empty_branch() {
            *self = MerkleNode::Empty;
        }
        deleted
    }

    fn is_empty(&self) -> bool {
        matches!(self, MerkleNode::Empty)
    }

    fn is_empty_branch(&self) -> bool {
        match self {
            MerkleNode::Branch(branch) => branch.occupied.is_empty(),
            _ => false,
        }
    }

    pub fn static_hash(&self) -> IonicHash {
        match self {
            Self::Empty => empty_hash(),
            Self::Leaf(leaf) => leaf.hash,
            Self::Branch(b) => {
                if b.dirty {
                    panic!("Call hash() with mutable ref by generating root_hash first")
                }
                b.hash
            },
        }
    }

    pub fn hash(&mut self) -> IonicHash {
        match self {
            Self::Empty => empty_hash(),
            Self::Leaf(leaf) => leaf.hash,
            Self::Branch(branch) => {
                if !branch.dirty {
                    return branch.hash;
                }
                let mut bytes = Vec::new();
                bytes.extend_from_slice(MERKLE_BRANCH_DOMAIN);
                for &index in &branch.occupied {
                    let child = branch.children[index as usize].as_mut().unwrap();
                    let child_hash = child.hash();
                    bytes.push(index);
                    bytes.extend_from_slice(child_hash.as_ref());
                }
                let hash: IonicHash = blake3::hash(&bytes).to_owned().into();
                branch.hash = hash;
                branch.dirty = false;
                hash
            }
        }
    }

    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Self::Empty => buf.push(TAG_EMPTY),
            Self::Leaf(leaf) => {
                buf.push(TAG_LEAF);
                let value_bytes = leaf.value.to_bytes();
                write_uvarint(buf, value_bytes.len().as_u256());
                buf.extend_from_slice(&value_bytes);
            }
            Self::Branch(branch) => {
                buf.push(TAG_BRANCH);
                write_uvarint(buf, branch.occupied.len().as_u256());
                for &index in &branch.occupied {
                    buf.push(index);
                    branch.children[index as usize]
                        .as_ref()
                        .unwrap()
                        .encode(buf);
                }
            }
        }
    }
}

impl<T: Clone + ToBytes + FromBytes> MerkleNode<T> {
    fn decode(
        bytes: &[u8],
        cursor: &mut usize,
        key: &mut [u8; KEY_LEN],
        depth: usize,
    ) -> Result<Self, CodecError> {
        let tag = *bytes.get(*cursor).ok_or(CodecError::UnexpectedEof)?;
        *cursor += 1;
        match tag {
            TAG_EMPTY => Ok(Self::Empty),
            TAG_LEAF => {
                let value_len = read_uvarint(bytes, cursor)?.as_usize();
                let value_bytes = read_slice(bytes, cursor, value_len)?;
                let value =
                    T::from_bytes(value_bytes).map_err(|e| CodecError::Value(e.to_string()))?;
                let hash = leaf_hash(&key[..], value_bytes);
                Ok(Self::Leaf(MerkleNodeLeaf { value, hash }))
            }
            TAG_BRANCH => {
                let count = read_uvarint(bytes, cursor)?.as_usize();
                let mut children = empty_children();
                let mut occupied = Vec::with_capacity(count);
                for _ in 0..count {
                    let index = *bytes.get(*cursor).ok_or(CodecError::UnexpectedEof)?;
                    *cursor += 1;
                    key[depth] = index;
                    let child = MerkleNode::decode(bytes, cursor, key, depth + 1)?;
                    children[index as usize] = Some(Box::new(child));
                    occupied.push(index);
                }
                Ok(Self::Branch(MerkleNodeBranch {
                    children,
                    occupied,
                    hash: empty_hash(),
                    dirty: true,
                }))
            }
            other => Err(CodecError::InvalidTag(other)),
        }
    }
}

fn ensure_key(key: &[u8]) -> Result<(), TrieError> {
    if key.len() != KEY_LEN {
        return Err(TrieError::InvalidKeyLength {
            expected: KEY_LEN,
            actual: key.len(),
        });
    }
    Ok(())
}

fn empty_children<T: ToBytes + Clone>() -> Vec<Option<Box<MerkleNode<T>>>> {
    (0..ARITY).map(|_| None).collect()
}

fn leaf_hash(key: &[u8], value: &[u8]) -> IonicHash {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MERKLE_LEAF_DOMAIN);
    bytes.extend_from_slice(key);
    bytes.extend_from_slice(value);
    blake3::hash(&bytes).to_owned().into()
}

fn empty_hash() -> IonicHash {
    blake3::hash(MERKLE_EMPTY_HASH_DOMAIN).to_owned().into()
}

#[cfg(test)]
mod tests {
    use crate::merkletrie::SparseMerkleTrie;

    fn key(x: u8) -> [u8; 32] {
        let mut k = [0u8; 32];
        k[31] = x;
        k
    }

    #[test]
    fn insert_and_get() {
        let mut trie = SparseMerkleTrie::new();
        trie.insert(&key(1), 100).unwrap();
        trie.insert(&key(2), 420).unwrap();

        assert_eq!(trie.get(&key(1)), Ok(Some(&100)));
        assert_eq!(trie.get(&key(2)), Ok(Some(&420)));
    }

    #[test]
    fn delete_item() {
        let mut trie = SparseMerkleTrie::new();
        trie.insert(&key(1), 100).unwrap();
        trie.insert(&key(2), 420).unwrap();
        assert!(trie.delete(&key(1)).unwrap());
        assert_eq!(trie.get(&key(2)), Ok(Some(&420)));
        assert!(!trie.delete(&key(1)).unwrap());
    }

    #[test]
    fn update_changes_value() {
        let mut trie = SparseMerkleTrie::new();
        trie.insert(&key(1), 100).unwrap();

        let root1 = trie.root_hash();
        trie.insert(&key(1), 200).unwrap();
        let root2 = trie.root_hash();
        assert_ne!(root1, root2);
        assert_eq!(trie.get(&key(1)).unwrap(), Some(&200));
    }

    #[test]
    fn root_hash_is_cached_until_dirty() {
        let mut trie = SparseMerkleTrie::new();
        trie.insert(&key(1), 100).unwrap();
        let root1 = trie.root_hash();
        let root2 = trie.root_hash();
        assert_eq!(root1, root2);
    }

    #[test]
    fn root_hash_independent_of_insertion_order() {
        let mut trie_a = SparseMerkleTrie::new();
        trie_a.insert(&key(1), 100).unwrap();
        trie_a.insert(&key(2), 200).unwrap();

        let mut trie_b = SparseMerkleTrie::new();
        trie_b.insert(&key(2), 200).unwrap();
        trie_b.insert(&key(1), 100).unwrap();

        assert_eq!(trie_a.root_hash(), trie_b.root_hash());
    }

    #[test]
    fn serialization_round_trips_and_hash_matches() {
        let mut trie = SparseMerkleTrie::new();
        trie.insert(&key(1), 100).unwrap();
        trie.insert(&key(2), 420).unwrap();
        trie.insert(&key(250), 7).unwrap();
        let root_before = trie.root_hash();

        let bytes = trie.to_bytes();
        let mut restored = SparseMerkleTrie::<u32>::from_bytes(&bytes).unwrap();

        assert_eq!(restored.get(&key(1)).unwrap(), Some(&100));
        assert_eq!(restored.get(&key(2)).unwrap(), Some(&420));
        assert_eq!(restored.get(&key(250)).unwrap(), Some(&7));
        assert_eq!(restored.root_hash(), root_before);
    }

    #[test]
    fn serialization_rejects_trailing_bytes() {
        let mut trie = SparseMerkleTrie::new();
        trie.insert(&key(1), 100).unwrap();
        let mut bytes = trie.to_bytes();
        bytes.push(0xff);
        assert!(SparseMerkleTrie::<u32>::from_bytes(&bytes).is_err());
    }
}
