use std::collections::HashMap;

pub struct SparseVerkleTrie<T> {
    children: VerkleNode<T>
}

impl<T: Clone> SparseVerkleTrie<T> {
    pub fn new() -> Self {
        Self { children: VerkleNode::new() }
    }
    pub fn insert(&mut self, key: &[u8], value: T) {
        self.children.insert(key, value);
    }
    pub fn get(&mut self, key: &[u8]) -> Option<&T> {
        self.children.get(key)
    }
}

enum VerkleNode<T> {
    Empty,
    Leaf { path: Vec<u8>, value: T},
    Branch { children: HashMap<u8, Box<VerkleNode<T>>>, value: Option<T> },
}

impl<T: Clone> VerkleNode<T> {
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
