//! Visit order remains stable when a remote catalogue changes.
use super::model::{EditorPosition, View};
use crate::domain::page::PageKey;
use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct NavigationState {
    history: Vec<PageKey>,
    cursor: Option<usize>,
    positions: HashMap<PageKey, EditorPosition>,
    view: View,
    pub search_query: String,
}
impl NavigationState {
    pub fn open(&mut self, key: PageKey, position: EditorPosition) {
        self.positions.entry(key.clone()).or_insert(position);
        if self.current() != Some(&key) {
            if let Some(index) = self.cursor {
                self.history.truncate(index + 1);
            }
            self.history.push(key);
            self.cursor = Some(self.history.len() - 1);
        }
        self.view = View::Note;
    }
    pub fn current(&self) -> Option<&PageKey> {
        self.cursor.and_then(|i| self.history.get(i))
    }
    pub fn position(&self) -> Option<EditorPosition> {
        self.current()
            .and_then(|key| self.positions.get(key))
            .copied()
    }
    pub fn save_position(&mut self, position: EditorPosition) {
        if let Some(key) = self.current().cloned() {
            self.positions.insert(key, position);
        }
    }
    pub fn back(&mut self) -> Option<PageKey> {
        self.cursor = Some(self.cursor?.checked_sub(1)?);
        self.view = View::Note;
        self.current().cloned()
    }
    pub fn forward(&mut self) -> Option<PageKey> {
        let next = self.cursor? + 1;
        if next >= self.history.len() {
            return None;
        }
        self.cursor = Some(next);
        self.view = View::Note;
        self.current().cloned()
    }
    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }
    pub fn view(&self) -> View {
        self.view
    }
}
