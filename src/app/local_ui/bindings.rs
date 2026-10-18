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
        window.on_return_to_note(move || state.dispatch(AppAction::Navigate(View::Note)));
    }
    {
        let state = state.clone();
        window.on_history_step(move |step| {
            state.dispatch(AppAction::StepScratchpad(step.signum() as i8))
        });
    }
    {
        let state = state.clone();
        window.on_query_changed(move |_| {
            state.dispatch(AppAction::CancelOpen);
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.on_folder_changed(move |_| {
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
