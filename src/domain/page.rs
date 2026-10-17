//! Stable page identity independent of display text.
use super::{
    connection::ConnectionId,
    error::{PathError, RevisionError},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PagePath(String);
impl PagePath {
    pub fn parse(input: &str) -> Result<Self, PathError> {
        if input.is_empty() {
            return Err(PathError::Empty);
        }
        if input.starts_with('/') || input.as_bytes().get(1) == Some(&b':') {
            return Err(PathError::Absolute);
        }
        if input.contains(['\\', '\0']) {
            return Err(PathError::ForbiddenCharacter);
        }
        if input
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(PathError::InvalidSegment);
        }
        Ok(Self(input.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageKey {
    pub connection: ConnectionId,
    pub path: PagePath,
}
impl PageKey {
    pub fn title_from<'a>(&'a self, body: &'a str) -> &'a str {
        body.lines()
            .find_map(|line| {
                let title = line.trim().trim_start_matches('#').trim();
                (!title.is_empty()).then_some(title)
            })
            .unwrap_or_else(|| {
                self.path
                    .as_str()
                    .rsplit('/')
                    .next()
                    .unwrap_or(self.path.as_str())
                    .trim_end_matches(".md")
            })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalRevision(pub u64);
impl LocalRevision {
    pub fn next(self) -> Result<Self, RevisionError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(RevisionError::Exhausted)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RemoteRevision(pub String);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(pub Uuid);
impl OperationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for OperationId {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrashId(pub Uuid);
impl TrashId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl Default for TrashId {
    fn default() -> Self {
        Self::new()
    }
}

/// Call once per draft and persist the resulting path; retries reuse it.
pub fn new_note_path(now: &jiff::Zoned, id: Uuid) -> PagePath {
    PagePath(format!(
        "Quick Notes/{}/{}-{id}.md",
        now.strftime("%Y-%m-%d"),
        now.strftime("%H%M%S")
    ))
}
