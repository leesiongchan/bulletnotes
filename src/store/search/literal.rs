use crate::domain::error::StoreError;
use regex::{Regex, RegexBuilder};
use std::ops::Range;

pub(super) struct LiteralQuery(Regex);
impl LiteralQuery {
    pub(super) fn new(text: &str) -> Result<Option<Self>, StoreError> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        RegexBuilder::new(&regex::escape(text))
            .case_insensitive(true)
            .build()
            .map(|regex| Some(Self(regex)))
            .map_err(|_| StoreError::InvalidQuery)
    }
    pub(super) fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text)
    }
    pub(super) fn ranges(&self, text: &str) -> Vec<Range<usize>> {
        self.0.find_iter(text).map(|hit| hit.range()).collect()
    }
}
