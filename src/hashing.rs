use crate::{Hash, transactions::{self, Transaction}};

const MERKLE_TREE_DOMAIN: &[u8] = b"PINECONE_MT";
const EMPTY_TX_ROOT_DOMAIN: &[u8] = b"PINECONE_EMPTY_TX_ROOT_V1";

pub fn hash_pair(left: &Hash, right: &Hash) -> Hash {
    let mut data = Vec::new();
    data.extend_from_slice(MERKLE_TREE_DOMAIN);
    data.extend_from_slice(left);
    data.extend_from_slice(right);

    blake3::hash(&data).as_bytes().to_owned()
}

pub fn transactions_root(transactions: &[Transaction]) -> Hash {
    if transactions.is_empty() {
        return blake3::hash(EMPTY_TX_ROOT_DOMAIN).as_bytes().to_owned();
    }

    // TODO: the hash of tx might need to include the signature and not
    // just fields
    let mut level: Vec<Hash> = transactions.iter().map(|tx| tx.hash()).collect();
    
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let left = &pair[0];

            // TODO: Mabey have a unique hash for odd number of transactions
            let right = if pair.len() == 2 { &pair[1] }
            else { left };

            next.push(hash_pair(left, right));
        }
        level = next;
    }

    level[0]
}
