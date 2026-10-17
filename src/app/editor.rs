//! Exact source buffers and local durability, independent of native selection capture.
use super::model::EditorPosition;
use crate::domain::{
    error::{PositionError, RevisionError},
    page::*,
    status::LocalSaveState,
};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("page is read only")]
    ReadOnly,
    #[error(transparent)]
    Revision(#[from] RevisionError),
    #[error(transparent)]
    Position(#[from] PositionError),
}
pub struct EditorSession {
    page: PageSnapshot,
    durable: Option<LocalRevision>,
    observed_position: Option<EditorPosition>,
    deadline: Option<Duration>,
    in_flight: Option<LocalRevision>,
    failed: bool,
}
impl EditorSession {
    pub fn restore(page: PageSnapshot) -> Self {
        let durable = Some(page.local_revision);
        Self {
            page,
            durable,
            observed_position: None,
            deadline: None,
            in_flight: None,
            failed: false,
        }
    }
    pub fn draft(key: PageKey) -> Self {
        let mut session = Self::restore(PageSnapshot {
            key,
            body: String::new(),
            local_revision: LocalRevision(0),
            remote_base: None,
            position: EditorPosition::default(),
            permission: Permission::ReadWrite,
        });
        session.durable = None;
        session
    }
    pub fn key(&self) -> &PageKey {
        &self.page.key
    }
    pub fn body(&self) -> &str {
        &self.page.body
    }
    pub fn revision(&self) -> LocalRevision {
        self.page.local_revision
    }
    pub fn permission(&self) -> Permission {
        self.page.permission
    }
    pub fn observed_position(&self) -> Option<EditorPosition> {
        self.observed_position
    }
    pub fn restore_target(&self) -> EditorPosition {
        self.page.position
    }
    pub fn needs_create(&self) -> bool {
        self.durable.is_none()
    }
    pub fn dirty(&self) -> bool {
        self.durable != Some(self.revision())
    }
    pub fn writing(&self) -> bool {
        self.in_flight.is_some()
    }
    pub fn next_save_at(&self) -> Option<Duration> {
        if self.writing() { None } else { self.deadline }
    }
    pub fn due(&self, now: Duration) -> bool {
        self.deadline.is_some_and(|deadline| now >= deadline)
    }
    /// Position is present only after a native adapter explicitly supplies an observation.
    pub fn snapshot(&self) -> (String, Option<EditorPosition>) {
        (self.page.body.clone(), self.observed_position)
    }
    pub fn observe_position(&mut self, position: EditorPosition) -> Result<(), EditorError> {
        self.observed_position = Some(position.validate(self.body())?);
        Ok(())
    }
    pub fn edit(&mut self, text: String, now: Duration) -> Result<bool, EditorError> {
        if text == self.page.body {
            return Ok(false);
        }
        if self.permission() == Permission::ReadOnly {
            return Err(EditorError::ReadOnly);
        }
        let revision = self.revision().next()?;
        self.page.body = text;
        self.page.local_revision = revision;
        self.observed_position = None;
        self.failed = false;
        self.deadline
            .get_or_insert(now.saturating_add(Duration::from_millis(250)));
        Ok(true)
    }
    pub fn save_state(&self) -> LocalSaveState {
        if self.failed {
            LocalSaveState::Failed(self.revision())
        } else if let Some(revision) = self.in_flight {
            LocalSaveState::Saving(revision)
        } else if self.dirty() {
            LocalSaveState::Dirty(self.revision())
        } else {
            LocalSaveState::Clean(self.revision())
        }
    }
    pub(super) fn start_write(&mut self, revision: LocalRevision) {
        self.in_flight = Some(revision);
        self.failed = false;
        self.deadline = None;
    }
    pub(super) fn acknowledge(&mut self, saved: &SavedRevision) -> bool {
        if saved.key != self.page.key || self.in_flight != Some(saved.local_revision) {
            return false;
        }
        self.durable = Some(saved.local_revision);
        self.in_flight = None;
        self.failed = false;
        true
    }
    pub(super) fn fail(&mut self, key: &PageKey, revision: LocalRevision) -> bool {
        if key != self.key() || self.in_flight != Some(revision) {
            return false;
        }
        self.in_flight = None;
        self.deadline = None;
        self.failed = true;
        true
    }
}
