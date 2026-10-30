//! Search selection and revision-pinned confirmation stay independent of row indices.
use crate::domain::page::{LocalRevision, PageKey};
use std::collections::HashSet;

#[derive(Default)]
pub struct SearchManagement {
    selected: HashSet<PageKey>,
    pub(crate) prepared: Option<(u64, Vec<(PageKey, LocalRevision)>)>,
    pub(crate) operation: Option<u64>,
    pub(crate) open_after: Option<(u64, PageKey)>,
}
impl SearchManagement {
    pub fn toggle(&mut self, key: PageKey) {
        if !self.selected.remove(&key) {
            self.selected.insert(key);
        }
    }
    pub fn clear(&mut self) {
        self.selected.clear();
        self.prepared = None;
    }
    pub fn retain_loaded(&mut self, keys: &[PageKey]) {
        self.selected.retain(|key| keys.contains(key));
    }
    pub fn selected(&self) -> &HashSet<PageKey> {
        &self.selected
    }
    pub fn busy(&self) -> bool {
        self.operation.is_some()
    }
    pub fn confirmation(&self) -> Option<(u64, usize)> {
        self.prepared.as_ref().map(|(id, pages)| (*id, pages.len()))
    }
}
