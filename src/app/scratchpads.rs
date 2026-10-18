//! Destinations use durable creation order, independently of browsing history.
use crate::{domain::page::PageKey, store::scratchpads::ScratchpadEntry};
#[derive(Default)]
pub struct ScratchpadStack {
    entries: Vec<ScratchpadEntry>,
}
impl ScratchpadStack {
    pub fn new(entries: Vec<ScratchpadEntry>) -> Self {
        Self { entries }
    }
    pub fn entries(&self) -> &[ScratchpadEntry] {
        &self.entries
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn position(&self, key: &PageKey) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| &e.key == key)
            .map(|i| i + 1)
    }
    pub fn older(&self, key: &PageKey) -> Option<PageKey> {
        self.entries.get(self.position(key)?).map(|e| e.key.clone())
    }
    pub fn newer(&self, key: &PageKey) -> Option<PageKey> {
        self.entries
            .get(self.position(key)?.checked_sub(2)?)
            .map(|e| e.key.clone())
    }
    pub fn newest(&self) -> Option<PageKey> {
        self.entries.first().map(|e| e.key.clone())
    }
    pub(super) fn remove(&mut self, key: &PageKey) {
        self.entries.retain(|e| &e.key != key);
    }
    pub fn deletion_successors(&self, key: &PageKey) -> Vec<PageKey> {
        let Some(pos) = self.position(key) else {
            return vec![];
        };
        self.entries[pos..]
            .iter()
            .chain(self.entries[..pos - 1].iter().rev())
            .map(|e| e.key.clone())
            .collect()
    }
}
