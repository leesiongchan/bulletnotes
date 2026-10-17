//! Coalesced pending downloads; bodies stay outside the catalogue queue.
use crate::domain::page::PageKey;
use std::collections::{HashSet, VecDeque};
#[derive(Default)]
pub(super) struct DownloadQueue {
    items: VecDeque<PageKey>,
    queued: HashSet<PageKey>,
}
impl DownloadQueue {
    pub(super) fn len(&self) -> usize {
        self.items.len()
    }
    pub(super) fn extend(&mut self, keys: impl IntoIterator<Item = PageKey>) {
        for key in keys {
            if self.queued.insert(key.clone()) {
                self.items.push_back(key);
            }
        }
    }
    pub(super) fn prioritize(&mut self, key: PageKey) {
        self.items.retain(|queued| queued != &key);
        self.queued.insert(key.clone());
        self.items.push_front(key);
    }
    pub(super) fn pop(&mut self) -> Option<PageKey> {
        let key = self.items.pop_front()?;
        self.queued.remove(&key);
        Some(key)
    }
    pub(super) fn defer(&mut self, key: PageKey) {
        self.extend([key]);
    }
}
