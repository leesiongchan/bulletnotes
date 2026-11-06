//! Slint callback registration for local editing and navigation.
use super::{State, presentation::LinkChoices};
use crate::{
    AppWindow, LinkPicker, SearchAdapter,
    app::{
        coordinator::{AppAction, LinkOutcome},
        markdown,
        model::View,
    },
    domain::{
        link::{self, Resolution},
        page::{PageKey, PagePath},
    },
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
        window.on_follow_link(move |source, caret| follow_link(&state, &source, caret));
    }
    {
        let state = state.clone();
        window
            .on_suggest_links(move |source, caret, _revision| state.suggest_links(&source, caret));
    }
    {
        let state = state.clone();
        window.global::<LinkPicker>().on_chosen(move |chosen| {
            if let Some((from, choices)) = close_link_choices(&state)
                && let Some((path, anchor)) = choices
                    .into_iter()
                    .find(|(path, _)| path.as_str() == chosen.as_str())
            {
                let target = PageKey {
                    connection: from.connection.clone(),
                    path,
                };
                open_link(&state, from, LinkOutcome::Open(target), anchor);
            }
        });
    }
    {
        let state = state.clone();
        window.on_show_backlinks(move || show_backlinks(&state));
    }
    {
        let state = state.clone();
        window.global::<LinkPicker>().on_dismissed(move || {
            close_link_choices(&state);
        });
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
                window.global::<SearchAdapter>().set_query("".into());
            }
            state.dispatch(AppAction::CancelOpen);
            state.dispatch(AppAction::Navigate(View::AllPages));
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window.global::<SearchAdapter>().on_load_more(move || {
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
                window.global::<SearchAdapter>().set_query("".into());
            }
            state.dispatch(AppAction::Navigate(View::Note));
        });
    }
    {
        let state = state.clone();
        window.global::<SearchAdapter>().on_query_changed(move |_| {
            state.reset_search_selection();
            state.dispatch(AppAction::CancelOpen);
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window
            .global::<SearchAdapter>()
            .on_folder_changed(move |_| {
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
        window
            .global::<SearchAdapter>()
            .on_drag_started(move |value| {
                let Some(window) = state.window.upgrade() else {
                    return;
                };
                let key = state.search_key(&value);
                let app = state.coordinator.borrow();
                if window.global::<SearchAdapter>().get_can_reorder()
                    && window.global::<SearchAdapter>().get_scratchpads()
                    && window.global::<SearchAdapter>().get_query().is_empty()
                    && let Some(key) = key
                    && let Some(stack) = app.scratchpads().filter(|_| !app.scratchpads_pending())
                {
                    let expected = stack
                        .entries()
                        .iter()
                        .map(|e| e.key.clone())
                        .collect::<Vec<_>>();
                    let selected = if window.global::<SearchAdapter>().get_selecting()
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
                    window.global::<SearchAdapter>().set_drag_active(false);
                }
            });
    }
    {
        let state = state.clone();
        window.global::<SearchAdapter>().on_drag_cancelled(move || {
            state.search.drag.borrow_mut().take();
        });
    }
    {
        let state = state.clone();
        window
            .global::<SearchAdapter>()
            .on_reordered(move |value, after| {
                let Some(drag) = state.search.drag.borrow_mut().take() else {
                    return;
                };
                let Some(window) = state.window.upgrade() else {
                    return;
                };
                if !window.global::<SearchAdapter>().get_can_reorder()
                    || !window.global::<SearchAdapter>().get_query().is_empty()
                    || !window.global::<SearchAdapter>().get_scratchpads()
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
        window.global::<SearchAdapter>().on_scope_changed(move |_| {
            state.reset_search_selection();
            state.dispatch(AppAction::CancelOpen);
            state.issue_search();
        });
    }
    {
        let state = state.clone();
        window
            .global::<SearchAdapter>()
            .on_view_changed(move |cards| {
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
                            window.global::<SearchAdapter>().set_cards(
                                settings.search_view == crate::store::settings::SearchView::Cards,
                            );
                        }
                        window.global::<SearchAdapter>().set_index_status(
                            "View preference could not be saved. Try again.".into(),
                        );
                    }
                });
            });
    }
    {
        let state = state.clone();
        window.global::<SearchAdapter>().on_select_toggled(move || {
            if let Some(window) = state.window.upgrade()
                && !window.global::<SearchAdapter>().get_selecting()
            {
                state.coordinator.borrow_mut().search_management.clear();
            }
            state.presentation.search.set(None);
            state.render();
        });
    }
    {
        let state = state.clone();
        window
            .global::<SearchAdapter>()
            .on_selection_toggled(move |value| {
                if let Some(key) = state.search_key(&value) {
                    state.coordinator.borrow_mut().search_management.toggle(key);
                    state.presentation.search.set(None);
                    state.render();
                }
            });
    }
    {
        let state = state.clone();
        window
            .global::<SearchAdapter>()
            .on_promote(move |value, open| {
                if let Some(key) = state.search_key(&value) {
                    state.promote_search(vec![key], open);
                }
            });
    }
    {
        let state = state.clone();
        window
            .global::<SearchAdapter>()
            .on_promote_selected(move || state.promote_search(state.selected_search_keys(), false));
    }
    {
        let state = state.clone();
        window.global::<SearchAdapter>().on_trash(move |value| {
            if let Some(key) = state.search_key(&value) {
                state.dispatch(AppAction::PrepareSearchTrash(vec![key]));
            }
        });
    }
    {
        let state = state.clone();
        window.global::<SearchAdapter>().on_trash_selected(move || {
            state.dispatch(AppAction::PrepareSearchTrash(state.selected_search_keys()))
        });
    }
    {
        let state = state.clone();
        window
            .global::<SearchAdapter>()
            .on_trash_confirmed(move |value| {
                if let Ok(id) = value.parse() {
                    state.dispatch(AppAction::ConfirmSearchTrash(id));
                }
            });
    }
    {
        let state = state.clone();
        window
            .global::<SearchAdapter>()
            .on_trash_cancelled(move || state.dispatch(AppAction::CancelSearchTrash));
    }
    {
        let state = state.clone();
        window.global::<SearchAdapter>().on_create(move || {
            if let Some(window) = state.window.upgrade()
                && state.can_create_search_note(&window)
            {
                state.dispatch(AppAction::NewNoteFromSearch(
                    window.global::<SearchAdapter>().get_query().to_string(),
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

/// Resolve the wikilink at `caret` and open or create its page. Returns whether the caret
/// was in a wikilink, so the editor only consumes the shortcut there.
fn follow_link(state: &Rc<State>, source: &str, caret: i32) -> bool {
    let Some(raw) = usize::try_from(caret)
        .ok()
        .and_then(|caret| markdown::wikilink_at(source, caret))
    else {
        return false;
    };
    let Some(from) = state
        .coordinator
        .borrow()
        .editor()
        .map(|editor| editor.key().clone())
    else {
        return false;
    };
    let anchor = link::anchor(&raw);
    let link = match link::parse(&raw) {
        Ok(Some(link)) => link,
        Ok(None) => {
            // `[[#Heading]]` and `[[@123]]` point into the current page.
            if let Some(anchor) = anchor {
                *state.presentation.link_target.borrow_mut() = Some((from, anchor));
                state.render();
            }
            return true;
        }
        Err(error) => {
            state.dispatch(AppAction::FollowLink {
                from,
                outcome: LinkOutcome::Refused(error.notice()),
            });
            return true;
        }
    };
    let task = state.clone();
    if slint::spawn_local(async move {
        let outcome = match task.store.link_catalogue(&from.connection).await {
            Err(error) => {
                tracing::warn!(%error, "wikilink catalogue could not be read");
                LinkOutcome::Refused("This link could not be opened. Try again.")
            }
            Ok(catalogue) => {
                let key = |path| PageKey {
                    connection: from.connection.clone(),
                    path,
                };
                match link::resolve(&link, &from.path, &catalogue.pages, catalogue.listed) {
                    Ok(Resolution::Open(path)) => LinkOutcome::Open(key(path)),
                    Ok(Resolution::Create(path)) => LinkOutcome::Create(key(path)),
                    Ok(Resolution::Choose(paths)) => {
                        let choices = paths
                            .into_iter()
                            .map(|path| (path, anchor.clone()))
                            .collect();
                        offer_link_choices(&task, from, "Several pages match this link", choices);
                        return;
                    }
                    Err(error) => LinkOutcome::Refused(error.notice()),
                }
            }
        };
        open_link(&task, from, outcome, anchor);
    })
    .is_err()
    {
        tracing::warn!("wikilink resolution could not be scheduled");
    }
    true
}

fn open_link(state: &Rc<State>, from: PageKey, outcome: LinkOutcome, anchor: Option<link::Anchor>) {
    if let (LinkOutcome::Open(target), Some(anchor)) = (&outcome, anchor) {
        *state.presentation.link_target.borrow_mut() = Some((target.clone(), anchor));
    }
    state.dispatch(AppAction::FollowLink { from, outcome });
}

/// Offer pages to open, unless the user has left the page they were offered from.
fn offer_link_choices(
    state: &Rc<State>,
    from: PageKey,
    title: &str,
    choices: Vec<(PagePath, Option<link::Anchor>)>,
) {
    let current = {
        let app = state.coordinator.borrow();
        app.view() == View::Note
            && !app.loading()
            && app.editor().is_some_and(|editor| editor.key() == &from)
    };
    let Some(window) = state.window.upgrade().filter(|_| current) else {
        return;
    };
    let paths: Vec<slint::SharedString> = choices
        .iter()
        .map(|(path, _)| path.as_str().into())
        .collect();
    *state.presentation.link_choice.borrow_mut() = Some((from, choices));
    let picker = window.global::<LinkPicker>();
    picker.set_title(title.into());
    picker.set_choices(slint::ModelRc::from(paths.as_slice()));
}

fn close_link_choices(state: &Rc<State>) -> Option<LinkChoices> {
    if let Some(window) = state.window.upgrade() {
        window
            .global::<LinkPicker>()
            .set_choices(Default::default());
        window.invoke_focus_editor();
    }
    state.presentation.link_choice.borrow_mut().take()
}

/// List the pages whose wikilinks lead to the current page, each opening at its link.
fn show_backlinks(state: &Rc<State>) {
    let from = {
        let app = state.coordinator.borrow();
        if app.view() != View::Note || app.loading() {
            return;
        }
        let Some(editor) = app.editor() else {
            return;
        };
        editor.key().clone()
    };
    let task = state.clone();
    if slint::spawn_local(async move {
        let found = async {
            let sources = task.store.link_sources(&from.connection).await?;
            let catalogue = task.store.link_catalogue(&from.connection).await?;
            Ok::<_, crate::domain::error::StoreError>(backlinks(
                &from.path,
                &sources,
                &catalogue.pages,
            ))
        }
        .await;
        match found {
            Ok(choices) if choices.is_empty() => task.dispatch(AppAction::FollowLink {
                from,
                outcome: LinkOutcome::Refused("No other pages link here."),
            }),
            Ok(choices) => offer_link_choices(&task, from, "Pages linking here", choices),
            Err(error) => {
                tracing::warn!(%error, "backlinks could not be read");
                task.dispatch(AppAction::FollowLink {
                    from,
                    outcome: LinkOutcome::Refused("Links to this page could not be listed."),
                });
            }
        }
    })
    .is_err()
    {
        tracing::warn!("backlink search could not be scheduled");
    }
}

/// Each other page with a link that can lead to `target`, opening at its first such link.
fn backlinks(
    target: &PagePath,
    sources: &[(PagePath, String)],
    pages: &[(PagePath, bool)],
) -> Vec<(PagePath, Option<link::Anchor>)> {
    // Every link to the page spells its file name, so other bodies are skipped unparsed.
    let name = target.as_str().rsplit('/').next().unwrap_or_default();
    let name = name.strip_suffix(".md").unwrap_or(name).to_lowercase();
    sources
        .iter()
        .filter(|(path, body)| path != target && body.to_lowercase().contains(&name))
        .filter_map(|(path, body)| {
            let (_, at) = markdown::wikilinks(body)
                .find(|(raw, _)| link::links_to(raw, path, target, pages))?;
            let offset = body[..at].chars().count();
            Some((
                path.clone(),
                Some(link::Anchor::Position(link::Position::Offset(offset))),
            ))
        })
        .collect()
}
