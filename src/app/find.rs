//! Literal, case-sensitive find with UTF-8 source offsets and bounded auxiliary memory.
use std::ops::Range;
#[derive(Default)]
pub struct NoteFind {
    query: String,
    total: usize,
    selected: Option<usize>,
}
impl NoteFind {
    pub fn total(&self) -> usize {
        self.total
    }
    /// One-based current match, absent after edits until an explicit find action.
    pub fn current(&self) -> Option<usize> {
        self.selected.map(|index| index + 1)
    }
    pub fn set_query(&mut self, body: &str, query: String) {
        self.query = query;
        self.refresh(body);
    }
    pub fn refresh(&mut self, body: &str) {
        self.total = if self.query.is_empty() {
            0
        } else {
            body.match_indices(&self.query).count()
        };
        self.selected = None;
    }
    pub fn step(&mut self, body: &str, direction: i32) -> Option<Range<usize>> {
        if self.query.is_empty() {
            self.refresh(body);
            return None;
        }
        self.total = body.match_indices(&self.query).count();
        if self.total == 0 {
            self.selected = None;
            return None;
        }
        let index = match (self.selected, direction < 0) {
            (None | Some(0), true) => self.total - 1,
            (Some(index), true) => (index - 1).min(self.total - 1),
            (Some(index), false) if index < self.total - 1 => index + 1,
            _ => 0,
        };
        let (offset, text) = body.match_indices(&self.query).nth(index)?;
        self.selected = Some(index);
        Some(offset..offset + text.len())
    }
}
