//! Slint callback registration for local editing and navigation.
use super::State;
use crate::{
    AppWindow,
    app::{coordinator::AppAction, model::View},
    domain::page::PageKey,
};
use slint::ComponentHandle;
use std::rc::Rc;

pub(super) fn bind(window: &AppWindow, state: &Rc<State>) {
    {
        let state = state.clone();
        window.on_search_palette_changed(move || {
            state.presentation.search.set(None);
            state.render();
        });
    }
    {
        let state = state.clone();
        window.on_find_opened(move || state.find(None));
    }
    {
        let state = state.clone();
        window.on_find_query_changed(move |_| state.find(None));
    }
    {
        let state = state.clone();
        window.on_find_step(move |direction| state.find(Some(direction)));
    }
    {
        let state = state.clone();
        window.on_note_edited(move |text| state.dispatch(AppAction::EditText(text.to_string())));
    }
    {
        let state = state.clone();
        window.on_new_note(move || state.dispatch(AppAction::NewNote));
    }
    {
        let state = state.clone();
        window.on_browse_pages(move || {
            state.reset_search_selection();
            if let Some(window) = state.window.upgrade() {
                window.set_search_query("".into());
            }
            state.dispatch(AppAction::CancelOpen);
            state.dispatch(AppAction::Navigate(View::AllPages));
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_load_more_pages(move || {
            state.issue_search_page(true);
        });
    }
    {
        let state = state.clone();
        window.on_show_appearance(move || state.dispatch(AppAction::Navigate(View::Appearance)));
    }
    {
        let state = state.clone();
        window.on_show_connection(move || state.dispatch(AppAction::Navigate(View::Connection)));
    }
    {
        let state = state.clone();
        window.on_return_to_note(move || {
            state.reset_search_selection();
            if let Some(window) = state.window.upgrade() {
                window.set_search_query("".into());
            }
            state.dispatch(AppAction::Navigate(View::Note));
        });
    }
    {
        let state = state.clone();
        window.on_query_changed(move |_| {
            state.reset_search_selection();
            state.dispatch(AppAction::CancelOpen);
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_folder_changed(move |_| {
            state.reset_search_selection();
            state.dispatch(AppAction::CancelOpen);
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_open_page(move |key| {
            if let Ok(key) = serde_json::from_str::<PageKey>(&key) {
                state.dispatch(AppAction::OpenPage(key));
            }
        });
    }
    {
        let state = state.clone();
        window.on_search_drag_started(move |value| {
            let Some(window) = state.window.upgrade() else {
                return;
            };
            let key = state.search_key(&value);
            let app = state.coordinator.borrow();
            if window.get_search_can_reorder()
                && window.get_search_scratchpads()
                && window.get_search_query().is_empty()
                && let Some(key) = key
                && let Some(stack) = app.scratchpads().filter(|_| !app.scratchpads_pending())
            {
                let expected = stack
                    .entries()
                    .iter()
                    .map(|e| e.key.clone())
                    .collect::<Vec<_>>();
                let selected = if window.get_search_selecting()
                    && app.search_management.selected().contains(&key)
                {
                    expected
                        .iter()
                        .filter(|k| app.search_management.selected().contains(*k))
                        .cloned()
                        .collect()
                } else {
                    vec![key]
                };
                *state.search.drag.borrow_mut() =
                    Some(super::search::SearchDrag { expected, selected });
            } else {
                window.set_search_drag_active(false);
            }
        });
    }
    {
        let state = state.clone();
        window.on_search_drag_cancelled(move || {
            state.search.drag.borrow_mut().take();
        });
    }
    {
        let state = state.clone();
        window.on_search_reordered(move |value, after| {
            let Some(drag) = state.search.drag.borrow_mut().take() else {
                return;
            };
            let Some(window) = state.window.upgrade() else {
                return;
            };
            if !window.get_search_can_reorder()
                || !window.get_search_query().is_empty()
                || !window.get_search_scratchpads()
            {
                return;
            }
            let before = if value.is_empty() {
                None
            } else {
                let Ok(target) = serde_json::from_str::<PageKey>(&value) else {
                    return;
                };
                if drag.selected.contains(&target) {
                    return;
                }
                let Some(index) = drag.expected.iter().position(|key| key == &target) else {
                    return;
                };
                if after {
                    drag.expected[index + 1..]
                        .iter()
                        .find(|key| !drag.selected.contains(key))
                        .cloned()
                } else {
                    Some(target)
                }
            };
            state.dispatch(AppAction::MoveSearchScratchpads {
                expected: drag.expected,
                selected: drag.selected,
                before,
                open_after: None,
            });
        });
    }
    {
        let state = state.clone();
        window.on_search_scope_changed(move |_| {
            state.reset_search_selection();
            state.dispatch(AppAction::CancelOpen);
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_search_view_changed(move |cards| {
            let state = state.clone();
            let _ = slint::spawn_local(async move {
                let view = if cards {
                    crate::store::settings::SearchView::Cards
                } else {
                    crate::store::settings::SearchView::List
                };
                if state.appearance.save_search_view(view).await.is_err()
                    && let Some(window) = state.window.upgrade()
                {
                    if let Ok(settings) = state.store.load_settings().await {
                        window.set_search_cards(
                            settings.search_view == crate::store::settings::SearchView::Cards,
                        );
                    }
                    window
                        .set_index_status("View preference could not be saved. Try again.".into());
                }
            });
        });
    }
    {
        let state = state.clone();
        window.on_search_select_toggled(move || {
            if let Some(window) = state.window.upgrade()
                && !window.get_search_selecting()
            {
                state.coordinator.borrow_mut().search_management.clear();
            }
            state.presentation.search.set(None);
            state.render();
        });
    }
    {
        let state = state.clone();
        window.on_search_selection_toggled(move |value| {
            if let Some(key) = state.search_key(&value) {
                state.coordinator.borrow_mut().search_management.toggle(key);
                state.presentation.search.set(None);
                state.render();
            }
        });
    }
    {
        let state = state.clone();
        window.on_search_promote(move |value, open| {
            if let Some(key) = state.search_key(&value) {
                state.promote_search(vec![key], open);
            }
        });
    }
    {
        let state = state.clone();
        window.on_search_promote_selected(move || {
            state.promote_search(state.selected_search_keys(), false)
        });
    }
    {
        let state = state.clone();
        window.on_search_trash(move |value| {
            if let Some(key) = state.search_key(&value) {
                state.dispatch(AppAction::PrepareSearchTrash(vec![key]));
            }
        });
    }
    {
        let state = state.clone();
        window.on_search_trash_selected(move || {
            state.dispatch(AppAction::PrepareSearchTrash(state.selected_search_keys()))
        });
    }
    {
        let state = state.clone();
        window.on_search_trash_confirmed(move |value| {
            if let Ok(id) = value.parse() {
                state.dispatch(AppAction::ConfirmSearchTrash(id));
            }
        });
    }
    {
        let state = state.clone();
        window.on_search_trash_cancelled(move || state.dispatch(AppAction::CancelSearchTrash));
    }
    {
        let state = state.clone();
        window.on_search_create(move || {
            if let Some(window) = state.window.upgrade()
                && window.get_search_can_create()
            {
                state.dispatch(AppAction::NewNoteFromSearch(
                    window.get_search_query().to_string(),
                ));
            }
        });
    }
    {
        let state = state.clone();
        window.on_scratchpad_step(move |step| {
            state.dispatch(AppAction::StepScratchpad(step.signum() as i8))
        });
    }
    {
        let state = state.clone();
        window.on_jump_to_newest(move || state.dispatch(AppAction::JumpToNewest));
    }
    {
        let weak = window.as_weak();
        window.on_toggle_pin(move || {
            if let Some(window) = weak.upgrade() {
                window.set_is_pinned(!window.get_is_pinned());
            }
        });
    }
    {
        let state = state.clone();
        window.on_save_now(move || state.dispatch(AppAction::Flush));
    }
    {
        let state = state.clone();
        window.on_request_quit(move || state.dispatch(AppAction::RequestQuit));
    }
    {
        let state = state.clone();
        window.on_request_hide(move || state.dispatch(AppAction::RequestHide));
    }
    {
        let weak = window.as_weak();
        window.window().on_close_requested(move || {
            if let Some(window) = weak.upgrade() {
                window.invoke_request_hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        });
    }
}
