use std::collections::HashMap;

use crate::vm::logs::IonicLog;
use ethnum::u256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VMSnapshot(usize, usize);

#[derive(Debug, Clone)]
enum VMJournalEntry {
    TransStorageChanaged { key: u256, prev: Option<u256> },
}

#[derive(Debug, Default, Clone)]
pub struct VMJournal {
    entries: Vec<VMJournalEntry>,
}

impl VMJournal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn record(&self, logs: &[IonicLog]) -> VMSnapshot {
        VMSnapshot(self.entries.len(), logs.len())
    }

    pub fn storage_chanaged(&mut self, key: u256, prev: Option<u256>) {
        self.entries
            .push(VMJournalEntry::TransStorageChanaged { key, prev });
    }

    pub fn revert(
        &mut self,
        snap: VMSnapshot,
        logs: &mut Vec<IonicLog>,
        tstorage: &mut HashMap<u256, u256>,
    ) {
        while self.entries.len() > snap.0 {
            let entry = self.entries.pop().expect("journal length invariant");
            Self::undo(entry, tstorage);
        }
        logs.drain(snap.1..);
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

    fn undo(entry: VMJournalEntry, tstorage: &mut HashMap<u256, u256>) {
        match entry {
            VMJournalEntry::TransStorageChanaged { key, prev } => match prev {
                Some(v) => {
                    tstorage.insert(key, v);
                }
                None => {
                    tstorage.remove(&key);
                }
            },
        }
    }
}
