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
        window.set_lifecycle_pending(self.lifecycle.blocks_authoring());
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
        window.set_can_go_older(app.can_step_scratchpad(-1));
        window.set_can_go_newer(app.can_step_scratchpad(1));
        let current = app.editor().map(|editor| editor.key());
        let stack = app.scratchpads();
        let position = stack.and_then(|stack| current.and_then(|key| stack.position(key)));
        window.set_note_counter(
            position
                .zip(stack)
                .map(|(position, stack)| format!("{position} of {}", stack.len()))
                .unwrap_or_default()
                .into(),
        );
        window.set_is_newest_scratchpad(position == Some(1));
        window.set_can_jump_to_newest(
            stack.is_none_or(|stack| stack.is_empty() || stack.newest().as_ref() != current),
        );
        let created = stack
            .and_then(|stack| {
                current.and_then(|key| stack.entries().iter().find(|entry| &entry.key == key))
            })
            .map(|entry| entry.created_ms);
        window.set_note_date(
            created
                .and_then(|ms| jiff::Timestamp::from_millisecond(ms).ok())
                .map(|time| {
                    time.to_zoned(jiff::tz::TimeZone::system())
                        .strftime("%d %b %Y")
                        .to_string()
                })
                .unwrap_or_default()
                .into(),
        );
        window.set_status_attention(app.notice().is_some());
        if let Some(editor) = app.editor() {
            let changed = self.presentation.key.borrow().as_ref() != Some(editor.key());
            let source_changed = window.get_note_text().as_str() != editor.body();
            let observed_scroll = window.get_editor_scroll_y();
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
                } else if source_changed && let Some(mut position) = editor.observed_position() {
                    position.scroll_y = observed_scroll;
                    if let (Ok(anchor), Ok(caret)) = (
                        i32::try_from(position.anchor),
                        i32::try_from(position.caret),
                    ) {
                        window.invoke_restore_editor_position(anchor, caret, position.scroll_y);
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
            window.set_status_attention(
                app.notice().is_some()
                    || matches!(
                        editor.save_state(),
                        crate::domain::status::LocalSaveState::Failed(_)
                    )
                    || matches!(
                        self.sync_state(editor),
                        crate::domain::status::SyncState::Authentication
                            | crate::domain::status::SyncState::Failed
                            | crate::domain::status::SyncState::Conflict
                    ),
            );
        } else {
            window.set_note_read_only(true);
            window.set_status_text("Open or create a local note".into());
        }
        if self.search.connection.borrow().as_ref() != Some(app.connection()) {
            *self.search.connection.borrow_mut() = Some(app.connection().clone());
            window.set_search_selecting(false);
        }
        window.set_search_busy(app.search_management.busy());
        let (confirm_id, confirm_count) = app.search_management.confirmation().unwrap_or_default();
        window.set_search_confirm_count(i32::try_from(confirm_count).unwrap_or(i32::MAX));
        window.set_search_confirm_id(if confirm_count > 0 {
            confirm_id.to_string().into()
        } else {
            "".into()
        });
        let selected = app.search_management.selected();
        window.set_search_selected_count(i32::try_from(selected.len()).unwrap_or(i32::MAX));
        let selected_pages = app
            .search_results()
            .map(|r| {
                r.items
                    .iter()
                    .filter(|p| selected.contains(&p.key))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let disabled = selected_pages.iter().find_map(|p| {
            let reason = trash_disabled_reason(p);
            (!reason.is_empty()).then_some(reason)
        });
        window.set_search_can_trash(
            !selected_pages.is_empty() && disabled.is_none() && !app.search_management.busy(),
        );
        window.set_search_can_promote(
            !selected_pages.is_empty()
                && window.get_search_scratchpads()
                && !app.scratchpads_pending()
                && !app.search_management.busy(),
        );
        window.set_search_can_create(
            !self.search.invalid_folder.get()
                && !app.search_pending()
                && !app.search_management.busy()
                && app.search_results().is_some_and(|r| r.total == 0)
                && !window.get_search_query().trim().is_empty(),
        );
        window.set_search_action_message(
            if app.search_management.busy() {
                "Saving note changes…"
            } else if let Some(reason) = disabled {
                reason
            } else if window.get_search_scratchpads() && !window.get_search_query().is_empty() {
                "Clear the search to reorder scratchpads"
            } else {
                ""
            }
            .into(),
        );
        if self.presentation.search.get() != app.search_generation() {
            let rows = app
                .search_results()
                .map(|results| {
                    results
                        .items
                        .iter()
                        .map(|page| {
                            let (list_excerpt, list_ranges) =
                                crate::store::search::highlights::condense(
                                    page.excerpt.as_deref().unwrap_or(""),
                                    &page.highlights.excerpt,
                                );
                            let disabled_reason = trash_disabled_reason(page);
                            Ok(PageRow {
                                checked: selected.contains(&page.key),
                                can_promote: window.get_search_scratchpads(),
                                can_trash: disabled_reason.is_empty(),
                                trash_disabled_reason: disabled_reason.into(),
                                list_excerpt_highlights: slint::ModelRc::from(
                                    search_decorations(
                                        &list_excerpt,
                                        &list_ranges,
                                        window.get_theme().ink,
                                    )
                                    .as_slice(),
                                ),
                                list_excerpt: list_excerpt.into(),
                                key: serde_json::to_string(&page.key)?.into(),
                                title: page.title.as_str().into(),
                                path: page.key.path.as_str().into(),
                                excerpt: page.excerpt.as_deref().unwrap_or("").into(),
                                title_highlights: slint::ModelRc::from(
                                    search_decorations(
                                        &page.title,
                                        &page.highlights.title,
                                        window.get_theme().ink,
                                    )
                                    .as_slice(),
                                ),
                                path_highlights: slint::ModelRc::from(
                                    search_decorations(
                                        page.key.path.as_str(),
                                        &page.highlights.path,
                                        window.get_theme().ink,
                                    )
                                    .as_slice(),
                                ),
                                excerpt_highlights: slint::ModelRc::from(
                                    search_decorations(
                                        page.excerpt.as_deref().unwrap_or(""),
                                        &page.highlights.excerpt,
                                        window.get_theme().ink,
                                    )
                                    .as_slice(),
                                ),
                                modified_label: page
                                    .modified_ms
                                    .and_then(|ms| i64::try_from(ms).ok())
                                    .and_then(|ms| jiff::Timestamp::from_millisecond(ms).ok())
                                    .map(|time| {
                                        format!(
                                            "Edited {}",
                                            time.to_zoned(jiff::tz::TimeZone::system())
                                                .strftime("%d %b %Y")
                                        )
                                    })
                                    .unwrap_or_default()
                                    .into(),
                                availability: if !page.cached {
                                    "Online only"
                                } else if page.permission == Permission::ReadOnly {
                                    "Read only"
                                } else if page.conflicted {
                                    "Conflict"
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

fn trash_disabled_reason(page: &crate::store::search::PageSummary) -> &'static str {
    if !page.cached {
        "Open and download this note before moving it to Trash"
    } else if page.permission == Permission::ReadOnly {
        "Read-only notes cannot be moved to Trash"
    } else if page.conflicted {
        "Resolve this note's conflict before moving it to Trash"
    } else {
        ""
    }
}

fn search_decorations(
    text: &str,
    ranges: &[std::ops::Range<usize>],
    ink: slint::Color,
) -> Vec<slint::language::TextInputDecoration> {
    let mut end = 0;
    for range in ranges {
        if range.start < end
            || range.start >= range.end
            || range.end > text.len()
            || !text.is_char_boundary(range.start)
            || !text.is_char_boundary(range.end)
            || i32::try_from(range.end).is_err()
        {
            return Vec::new();
        }
        end = range.end;
    }
    ranges
        .iter()
        .map(|range| {
            let mut decoration = slint::language::TextInputDecoration::default();
            decoration.start = range.start as i32;
            decoration.end = range.end as i32;
            decoration.foreground = ink;
            decoration.bold = true;
            decoration.code = true;
            decoration
        })
        .collect()
}

#[cfg(test)]
mod search_tests {
    use super::*;
    #[test]
    fn invalid_ranges_render_plain_text() {
        let ink = slint::Color::from_rgb_u8(10, 20, 30);
        for ranges in [
            std::iter::once(std::ops::Range { start: 7, end: 6 }).collect(),
            std::iter::once(0..100).collect(),
            std::iter::once(4..5).collect(),
            vec![6..12, 0..3],
            vec![0..3, 2..5],
            vec![0..3, 4..5],
        ] {
            assert!(search_decorations("café needle", &ranges, ink).is_empty());
        }
        let ranges = std::iter::once(6..12).collect::<Vec<_>>();
        let values = search_decorations("café needle", &ranges, ink);
        assert_eq!(values.len(), 1);
        let value = &values[0];
        assert_eq!((value.start, value.end, value.foreground), (6, 12, ink));
        assert!(value.bold && !value.underline && value.code && !value.italic);
    }
}
