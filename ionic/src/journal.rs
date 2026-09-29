use crate::{accounts::Account, state::IonicState, types::IonicAddr};
use ethnum::u256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Snapshot(usize);

#[derive(Debug, Clone)]
enum JournalEntry {
    NonceChanged {
        addr: IonicAddr,
        prev: u64,
    },
    BalanceChanged {
        addr: IonicAddr,
        prev: u256,
    },
    CodeChanged {
        addr: IonicAddr,
        prev: Vec<u8>,
    },
    AccountCreated {
        addr: IonicAddr,
    },
    AccountDestroyed {
        addr: IonicAddr,
        account: Box<Account>,
    },
    StateStorageChanaged {
        addr: IonicAddr,
        key: [u8; 32],
        prev: Option<u256>,
    },
}

#[derive(Debug, Default, Clone)]
pub struct Journal {
    entries: Vec<JournalEntry>,
}

impl Journal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self) -> Snapshot {
        Snapshot(self.entries.len())
    }

    pub fn nonce_changed(&mut self, addr: IonicAddr, prev: u64) {
        self.entries.push(JournalEntry::NonceChanged { addr, prev });
    }
    pub fn balance_changed(&mut self, addr: IonicAddr, prev: u256) {
        self.entries
            .push(JournalEntry::BalanceChanged { addr, prev });
    }
    pub fn code_changed(&mut self, addr: IonicAddr, prev: Vec<u8>) {
        self.entries.push(JournalEntry::CodeChanged { addr, prev });
    }
    pub fn account_created(&mut self, addr: IonicAddr) {
        self.entries.push(JournalEntry::AccountCreated { addr });
    }
    pub fn account_destroyed(&mut self, addr: IonicAddr, account: Box<Account>) {
        self.entries
            .push(JournalEntry::AccountDestroyed { addr, account });
    }
    pub fn storage_chanaged(&mut self, addr: IonicAddr, key: [u8; 32], prev: Option<u256>) {
        self.entries
            .push(JournalEntry::StateStorageChanaged { addr, key, prev });
    }

    pub fn revert(&mut self, snap: Snapshot, state: &mut IonicState) {
        while self.entries.len() > snap.0 {
            let entry = self.entries.pop().expect("journal length invariant");
            Self::undo(entry, state);
        }
    }

    pub fn commit(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn undo(entry: JournalEntry, state: &mut IonicState) {
        match entry {
            JournalEntry::NonceChanged { addr, prev } => {
                if let Some(acc) = state.accounts.get_mut(&addr.as_key()) {
                    acc.nonce = prev;
                }
            }
            JournalEntry::BalanceChanged { addr, prev } => {
                if let Some(acc) = state.accounts.get_mut(&addr.as_key()) {
                    acc.balance = prev;
                }
            }
            JournalEntry::CodeChanged { addr, prev } => {
                if let Some(acc) = state.accounts.get_mut(&addr.as_key()) {
                    acc.code = prev;
                }
            }
            JournalEntry::StateStorageChanaged { addr, key, prev } => {
                if let Some(acc) = state.accounts.get_mut(&addr.as_key()) {
                    match prev {
                        Some(v) => {
                            acc.storage.insert(&key, v);
                        }
                        None => {
                            acc.storage.delete(&key);
                        }
                    }
                }
            }
            JournalEntry::AccountCreated { addr } => {
                state.accounts.delete(&addr.as_key());
            }
            JournalEntry::AccountDestroyed { addr, account } => {
                state.accounts.insert(&addr.as_key(), *account);
            }
        }
    }
}
