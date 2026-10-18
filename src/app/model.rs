//! UI-independent presentation state.
pub use crate::domain::editor::EditorPosition;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Note,
    AllPages,
    Appearance,
    Connection,
    Recovery,
}
