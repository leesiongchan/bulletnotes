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
        self.0
            .find_iter(text)
            .take(320)
            .map(|hit| hit.range())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_ranges_are_bounded_without_truncating_search() {
        let matcher = LiteralQuery::new("a").unwrap().unwrap();
        assert_eq!(matcher.ranges(&"a".repeat(100_000)).len(), 320);
        assert!(matcher.is_match(&format!("{}a", "z".repeat(100_000))));
    }
}
