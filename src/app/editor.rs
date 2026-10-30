//! Exact source buffers and local durability, independent of native selection capture.
use super::model::EditorPosition;
use crate::domain::{
    error::{PositionError, RevisionError},
    page::*,
    reconciliation::{EditorRestore, EditorToken, LocalSaveOutcome, StoredPage},
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
    durable_body: String,
    sent_body: Option<String>,
    observed_position: Option<EditorPosition>,
    deadline: Option<Duration>,
    in_flight: Option<(EditorToken, LocalRevision)>,
    session_id: uuid::Uuid,
    input_generation: u64,
    stored: StoredPage,
    failed: bool,
}
impl EditorSession {
    pub fn restore(restored: impl Into<EditorRestore>) -> Self {
        let restored = restored.into();
        let mut page = restored.presented().clone();
        let stored = restored.stored;
        page.permission = stored.page.permission;
        page.remote_base = stored.page.remote_base.clone();
        let durable = Some(page.local_revision);
        Self {
            stored,
            session_id: uuid::Uuid::new_v4(),
            input_generation: 0,
            durable_body: page.body.clone(),
            sent_body: None,
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
    pub(super) fn apply_remote_snapshot(&mut self, page: PageSnapshot) -> bool {
        if self.dirty()
            || self.writing()
            || page.key != self.page.key
            || page.local_revision != self.revision()
        {
            return false;
        }
        let changed = page.body != self.page.body;
        if self
            .observed_position
            .is_some_and(|position| position.validate(&page.body).is_err())
        {
            self.observed_position = None;
        }
        self.durable_body = page.body.clone();
        self.page = page;
        changed
    }
    pub fn matches_remote_base(&self) -> bool {
        self.page
            .remote_base
            .as_ref()
            .is_some_and(|base| base.revision.is_some() && base.body == self.page.body)
    }
    pub fn baseline(&self) -> EditorBaseline {
        EditorBaseline {
            body: self.durable_body.clone(),
            remote_base: self.page.remote_base.clone(),
        }
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
        let generation = self
            .input_generation
            .checked_add(1)
            .ok_or(RevisionError::Exhausted)?;
        self.input_generation = generation;
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
        } else if let Some((_, revision)) = &self.in_flight {
            LocalSaveState::Saving(*revision)
        } else if self.dirty() {
            LocalSaveState::Dirty(self.revision())
        } else {
            LocalSaveState::Clean(self.revision())
        }
    }
    pub(super) fn start_write(&mut self, revision: LocalRevision) -> EditorToken {
        let expected = self.rebase_token();
        self.sent_body = Some(if self.needs_create() {
            String::new()
        } else {
            self.body().to_owned()
        });
        self.in_flight = Some((expected.clone(), revision));
        self.failed = false;
        self.deadline = None;
        expected
    }
    pub(super) fn acknowledge(
        &mut self,
        expected: &EditorToken,
        outcome: &LocalSaveOutcome,
    ) -> bool {
        let saved = &outcome.attempt;
        if saved.key != self.page.key
            || self.in_flight.as_ref() != Some(&(expected.clone(), saved.local_revision))
        {
            return false;
        }
        self.durable = Some(saved.local_revision);
        if let Some(body) = self.sent_body.take() {
            self.durable_body = body;
        }
        self.stored = outcome.stored.clone();
        self.page.remote_base = outcome.stored.page.remote_base.clone();
        self.page.permission = outcome.stored.page.permission;
        self.in_flight = None;
        self.failed = false;
        true
    }
    pub(super) fn fail(&mut self, expected: &EditorToken, revision: LocalRevision) -> bool {
        if self.in_flight.as_ref() != Some(&(expected.clone(), revision)) {
            return false;
        }
        self.in_flight = None;
        self.sent_body = None;
        self.deadline = None;
        self.failed = true;
        true
    }
    pub fn rebase_token(&self) -> EditorToken {
        EditorToken {
            key: self.key().clone(),
            session_id: self.session_id,
            input_generation: self.input_generation,
            source_generation: self.stored.source_generation,
        }
    }
    pub fn rebase_input(&self) -> (String, PageSnapshot) {
        (self.durable_body.clone(), self.page.clone())
    }
    pub fn apply_rebase(
        &mut self,
        expected: &EditorToken,
        stored: StoredPage,
        edits: &[crate::sync::text_merge::SourceEdit],
    ) -> bool {
        if self.writing()
            || &self.rebase_token() != expected
            || stored.page.key != *self.key()
            || stored.page.local_revision < self.revision()
        {
            return false;
        }
        self.observed_position = self
            .observed_position
            .and_then(|position| map_position(position, self.body(), &stored.page.body, edits));
        self.durable_body = stored.page.body.clone();
        self.durable = Some(stored.page.local_revision);
        self.page = stored.page.clone();
        self.stored = stored;
        self.failed = false;
        self.deadline = None;
        true
    }
}

fn map_position(
    position: EditorPosition,
    before: &str,
    after: &str,
    edits: &[crate::sync::text_merge::SourceEdit],
) -> Option<EditorPosition> {
    use unicode_segmentation::UnicodeSegmentation;
    position.validate(before).ok()?;
    let map = |offset: usize| -> Option<usize> {
        let mut shift = 0isize;
        let mut end = 0;
        for edit in edits {
            if edit.range.start < end
                || edit.range.start > edit.range.end
                || !before.is_char_boundary(edit.range.start)
                || !before.is_char_boundary(edit.range.end)
            {
                return None;
            }
            end = edit.range.end;
            if offset < edit.range.start {
                break;
            }
            if offset <= edit.range.end {
                return edit
                    .range
                    .start
                    .checked_add_signed(shift)?
                    .checked_add(edit.replacement.len());
            }
            shift += edit.replacement.len() as isize - edit.range.len() as isize;
        }
        offset.checked_add_signed(shift)
    };
    let boundary = |offset: usize| {
        after
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(after.len()))
            .find(|index| *index >= offset)
    };
    Some(EditorPosition {
        anchor: boundary(map(position.anchor)?)?,
        caret: boundary(map(position.caret)?)?,
        scroll_y: position.scroll_y,
    })
}
