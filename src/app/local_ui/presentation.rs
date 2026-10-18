//! Projection of coordinator state into Slint properties and result models.
use super::State;
use crate::{
    PageRow,
    app::model::View,
    domain::{page::*, status::status_text},
};
use std::cell::{Cell, RefCell};

#[derive(Default)]
pub(super) struct Presentation {
    pub(super) key: RefCell<Option<PageKey>>,
    pub(super) revision: Cell<Option<LocalRevision>>,
    pub(super) search: Cell<Option<u64>>,
}
impl Presentation {
    pub(super) fn reset(&self) {
        self.key.borrow_mut().take();
        self.revision.set(None);
        self.search.set(None);
    }
}

impl State {
    pub(super) fn render(&self) {
        let presentation = crate::diagnostics::OperationTimer::start(
            crate::diagnostics::Operation::EditorPresentation,
        );
        let Some(window) = self.window.upgrade() else {
            return;
        };
        window.set_lifecycle_pending(self.lifecycle.closing());
        let app = self.coordinator.borrow();
        window.set_active_view(
            match app.view() {
                View::Note => "note",
                View::AllPages => "pages",
                View::Appearance => "appearance",
                View::Connection => "connection",
                View::Recovery => "recovery",
            }
            .into(),
        );
        window.set_can_go_back(app.can_go_back());
        window.set_can_go_forward(app.can_go_forward());
        if let Some(editor) = app.editor() {
            let changed = self.presentation.key.borrow().as_ref() != Some(editor.key());
            if changed
                || self.presentation.revision.get() != Some(editor.revision())
                || window.get_note_text().as_str() != editor.body()
            {
                if window.get_note_text().as_str() != editor.body() {
                    let timing = crate::diagnostics::OperationTimer::start(
                        crate::diagnostics::Operation::EditorSourceUpdate,
                    );
                    window.set_note_text(editor.body().into());
                    timing.success();
                }
                self.search.find.borrow_mut().refresh(editor.body());
                self.present_find(&window);
                window.set_current_path(editor.key().path.as_str().into());
                self.presentation.key.replace(Some(editor.key().clone()));
                self.presentation.revision.set(Some(editor.revision()));
                if changed {
                    if app.view() == View::Note {
                        let timing = crate::diagnostics::OperationTimer::start(
                            crate::diagnostics::Operation::EditorFocus,
                        );
                        window.invoke_focus_editor();
                        timing.success();
                    }
                    let position = editor.restore_target();
                    if let (Ok(anchor), Ok(caret)) = (
                        i32::try_from(position.anchor),
                        i32::try_from(position.caret),
                    ) {
                        let timing = crate::diagnostics::OperationTimer::start(
                            crate::diagnostics::Operation::SelectionRestoration,
                        );
                        window.invoke_restore_editor_position(anchor, caret, position.scroll_y);
                        timing.success();
                    }
                }
            }
            window.set_note_read_only(
                self.lifecycle.blocks_authoring() || editor.permission() == Permission::ReadOnly,
            );
            let status = app.notice().unwrap_or_else(|| {
                status_text(
                    editor.save_state(),
                    self.sync_state(editor),
                    editor.matches_remote_base(),
                )
            });
            window.set_status_text(status.into());
        } else {
            window.set_note_read_only(true);
            window.set_status_text("Open or create a local note".into());
        }
        if self.presentation.search.get() != app.search_generation() {
            let rows = app
                .search_results()
                .map(|results| {
                    results
                        .items
                        .iter()
                        .map(|page| {
                            Ok(PageRow {
                                key: serde_json::to_string(&page.key)?.into(),
                                title: page.title.as_str().into(),
                                path: page.key.path.as_str().into(),
                                excerpt: page.excerpt.as_deref().unwrap_or("").into(),
                                availability: if !page.cached {
                                    "Online only"
                                } else if page.permission == Permission::ReadOnly {
                                    "Read only"
                                } else {
                                    "Cached"
                                }
                                .into(),
                            })
                        })
                        .collect::<Result<Vec<_>, serde_json::Error>>()
                })
                .transpose();
            match rows {
                Ok(rows) => window.set_page_results(slint::ModelRc::new(slint::VecModel::from(
                    rows.unwrap_or_default(),
                ))),
                Err(_) => window.set_index_status("Page list could not be displayed".into()),
            }
            self.presentation.search.set(app.search_generation());
        }
        window.set_can_load_more_pages(
            !self.search.invalid_folder.get()
                && !app.search_pending()
                && app
                    .search_results()
                    .is_some_and(|results| results.items.len() < results.total),
        );
        if self.search.invalid_folder.get() {
            window.set_index_status("Enter a relative folder path".into());
        } else if let Some(message) = app.notice() {
            window.set_index_status(message.into());
        } else if let Some(results) = app.search_results() {
            let (indexed, total, complete) = app.catalogue_progress().unwrap_or((
                results.indexed,
                results.catalogue_total,
                results.complete,
            ));
            window.set_index_status(
                format!(
                    "{} pages · {} of {} indexed{}",
                    results.total,
                    indexed,
                    total,
                    if self.sync.snapshot.borrow().is_none() {
                        " · not syncing"
                    } else if !complete {
                        " · content search incomplete"
                    } else {
                        ""
                    }
                )
                .into(),
            );
        } else {
            window.set_index_status("Searching local pages…".into());
        }
        self.updates.send_replace(());
        presentation.success();
    }
}
