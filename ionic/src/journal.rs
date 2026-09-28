use ethnum::u256;

use crate::{accounts::Account, state::IonicState, types::{IonicAddr, IonicHash}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot(usize);

#[derive(Debug, Clone)]
pub enum JournalEntry {
    NonceChanged { addr: IonicAddr, prev: u64},
    BalanceChanged { addr: IonicAddr, prev: u256 },
    CodeChanged { addr: IonicAddr, prev: Vec<u8> },
    StorageChnaged { addr: IonicAddr, key: IonicHash, prev: Option<u256>},
    AccountCreated { addr: IonicAddr },
    AccountDestroyed { addr: IonicAddr, account: Box <Account>},
}

#[derive(Debug, Default, Clone)]
pub struct Journal {
    entries: Vec<JournalEntry>,
}

impl Journal {
    pub fn new() -> Self { Self::default() }

    pub fn snapshot(&self) -> Snapshot { Snapshot(self.entries.len()) }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn record(&mut self, entry: JournalEntry) {
        self.entries.push(entry);
    }

    pub fn revert(&mut self, snap: Snapshot, state: &mut IonicState) {
        while self.entries.len() > snap.0 {
            let entry = self.entries.pop().expect("journal length invariant");
            Self::undo(entry, state);
        }
    }

    pub fn commit(&mut self) { self.entries.clear(); }

    pub fn len(&self) -> usize {self.entries.len()}

    pub fn is_empty(&self) -> bool {self.entries.is_empty()}

    pub fn undo(entry: JournalEntry, state: &mut IonicState) {
        match entry {
            JournalEntry::NonceChanged { addr, prev } => {
                if let Some(acc) = state.accounts.get_mut(&addr.as_key()) {
                    acc.nonce = prev;
                }
            },
            JournalEntry::BalanceChanged { addr, prev } => {
                if let Some(acc) = state.accounts.get_mut(&addr.as_key()) {
                    acc.balance = prev;
                }
            },
            JournalEntry::CodeChanged { addr, prev } => {
                if let Some(acc) = state.accounts.get_mut(&addr.as_key()) {
                    acc.code = prev;
                }
            },
            JournalEntry::StorageChnaged { addr, key, prev } => {
                if let Some(acc) = state.accounts.get_mut(&addr.as_key()) {
                    match prev {
                        Some(v) => acc.storage.insert(&key.as_key(), v),
                        None => { acc.storage.delete(&key.as_key()); }
                    }
                }
            },
            JournalEntry::AccountCreated { addr } => {
                state.accounts.delete(&addr.as_key());
            },
            JournalEntry::AccountDestroyed { addr, account } => {
                state.accounts.insert(&addr.as_key(), *account);
            }
        }
    }
}
