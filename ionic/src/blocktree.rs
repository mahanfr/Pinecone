use std::collections::HashMap;

use crate::{blocks::Block, types::IonicHash};


#[derive(Debug, Default)]
pub struct BlockTree {
    pub blocks: HashMap<IonicHash, Block>,
    pub children: HashMap<IonicHash, Vec<IonicHash>>,
    pub parent: HashMap<IonicHash, IonicHash>,
    pub head: IonicHash,
}

impl BlockTree {
    pub fn insert(&mut self, block: Block) -> IonicHash {
        let hash = block.hash();
        let parent_hash = block.header.previous_hash;

        self.blocks.insert(hash, block);
        self.parent.insert(hash, parent_hash);
        self.children.entry(parent_hash).or_default().push(hash);
        hash
    }

    pub fn path_to_genesis(&self, head: &IonicHash) -> Vec<IonicHash> {
        let mut path = Vec::new();
        let mut current = *head;
        loop {
            path.push(current);
            let Some(block) = self.blocks.get(&current) else {
                break;
            };
            if block.header.index == 0 { break; }
            match self.parent.get(&current) {
                Some(parent) if *parent != IonicHash::default() => {
                    current = *parent;
                }
                _ => break,
            }
        }
        path.reverse();
        path
    }
}
