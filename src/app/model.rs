//! UI-independent presentation state.
use crate::domain::error::PositionError;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Note,
    AllPages,
    Appearance,
    Connection,
    Recovery,
}

/// Anchor/caret are UTF-8 byte boundaries, not glyph or UTF-16 indices.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
