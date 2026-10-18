//! Persisted editor position shared by storage and application state.
use super::error::PositionError;

/// Anchor/caret are UTF-8 byte boundaries, not glyph or UTF-16 indices.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EditorPosition {
    pub anchor: usize,
    pub caret: usize,
    pub scroll_y: f32,
}
impl EditorPosition {
    pub fn validate(self, text: &str) -> Result<Self, PositionError> {
        if !text.is_char_boundary(self.anchor) || !text.is_char_boundary(self.caret) {
            return Err(PositionError::InvalidBoundary);
        }
        if !self.scroll_y.is_finite() || self.scroll_y < 0.0 {
            return Err(PositionError::InvalidScroll);
        }
        Ok(self)
    }
}
