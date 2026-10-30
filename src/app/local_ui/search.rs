//! Catalogue query admission, pagination and in-note find interactions.
use super::{LocalUi, LocalUiError, State};
use crate::{
    AppWindow,
    app::{
        coordinator::{AppAction, SearchRequest},
        find::NoteFind,
    },
    domain::page::PagePath,
    store::search::SearchQuery,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub(super) struct SearchDrag {
    pub(super) expected: Vec<crate::domain::page::PageKey>,
    pub(super) selected: Vec<crate::domain::page::PageKey>,
}
#[derive(Default)]
pub(super) struct SearchState {
    pub(super) find: RefCell<NoteFind>,
    pub(super) drag: RefCell<Option<SearchDrag>>,
    pub(super) request_id: Cell<u64>,
    pub(super) invalid_folder: Cell<bool>,
    pub(super) connection: RefCell<Option<crate::domain::connection::ConnectionId>>,
}

impl LocalUi {
    pub async fn search_current(&self) -> Result<(), LocalUiError> {
        let mut updates = self.0.updates.subscribe();
        let id = self.0.issue_search().ok_or(LocalUiError::SearchFailed)?;
        loop {
            if self.0.coordinator.borrow().search_generation() == Some(id) {
                return Ok(());
            }
            if self.0.coordinator.borrow().notice().is_some()
                || self.0.search.request_id.get() != id
            {
                return Err(LocalUiError::SearchFailed);
            }
            updates.changed().await.map_err(|_| LocalUiError::Closed)?;
        }
    }
}

impl State {
    pub(super) fn reset_search_selection(&self) {
        self.coordinator.borrow_mut().search_management.clear();
        self.presentation.search.set(None);
        if let Some(window) = self.window.upgrade() {
            window.set_search_selecting(false);
            window.set_page_results(Default::default());
        }
    }
    pub(super) fn selected_search_keys(&self) -> Vec<crate::domain::page::PageKey> {
        let app = self.coordinator.borrow();
        app.search_results()
            .map(|r| {
                r.items
                    .iter()
                    .filter(|p| app.search_management.selected().contains(&p.key))
                    .map(|p| p.key.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(super) fn search_key(&self, value: &str) -> Option<crate::domain::page::PageKey> {
        let key = serde_json::from_str::<crate::domain::page::PageKey>(value).ok()?;
        let app = self.coordinator.borrow();
        (key.connection == *app.connection()
            && app
                .search_results()
                .is_some_and(|r| r.items.iter().any(|p| p.key == key)))
        .then_some(key)
    }
    pub(super) fn promote_search(
        self: &Rc<Self>,
        selected: Vec<crate::domain::page::PageKey>,
        open: bool,
    ) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        if !window.get_search_scratchpads() || selected.is_empty() {
            return;
        }
        let expected = {
            let app = self.coordinator.borrow();
            let Some(stack) = app.scratchpads().filter(|_| !app.scratchpads_pending()) else {
                return;
            };
            stack
                .entries()
                .iter()
                .map(|e| e.key.clone())
                .collect::<Vec<_>>()
        };
        let before = expected.iter().find(|key| !selected.contains(key)).cloned();
        let open_after = open.then(|| selected[0].clone());
        self.dispatch(AppAction::MoveSearchScratchpads {
            expected,
            selected,
            before,
            open_after,
        });
    }
    pub(super) fn issue_search(self: &Rc<Self>) -> Option<u64> {
        self.issue_search_page(false)
    }
    pub(super) fn issue_search_page(self: &Rc<Self>, more: bool) -> Option<u64> {
        let offset = if more {
            let app = self.coordinator.borrow();
            if app.search_pending() {
                return None;
            }
            let results = app.search_results()?;
            if results.items.len() >= results.total {
                return None;
            }
            results.items.len()
        } else {
            0
        };
        let window = self.window.upgrade()?;
        let folder = if window.get_search_scratchpads() {
            slint::SharedString::default()
        } else {
            window.get_search_folder()
        };
        let folder = if folder.is_empty() {
            None
        } else {
            match PagePath::parse(folder.trim_end_matches('/')) {
                Ok(folder) => Some(folder),
                Err(_) => {
                    self.search.invalid_folder.set(true);
                    window.set_page_results(Default::default());
                    window.set_index_status("Enter a relative folder path".into());
                    return None;
                }
            }
        };
        let Some(id) = self.search.request_id.get().checked_add(1) else {
            window.set_index_status("Search limit reached. Restart after saving.".into());
            return None;
        };
        self.search.invalid_folder.set(false);
        self.search.request_id.set(id);
        let query = SearchQuery {
            scope: if window.get_search_scratchpads() {
                crate::store::search::SearchScope::Scratchpads
            } else {
                crate::store::search::SearchScope::AllPages
            },
            connection: self.coordinator.borrow().connection().clone(),
            text: window.get_search_query().to_string(),
            folder,
            offset,
            limit: 200,
        };
        self.dispatch(AppAction::Search(SearchRequest { id, query }));
        Some(id)
    }
    pub(super) fn present_find(&self, window: &AppWindow) {
        let find = self.search.find.borrow();
        window.set_find_status(
            match find.current() {
                Some(current) => format!("{current} of {}", find.total()),
                None => format!("{} matches", find.total()),
            }
            .into(),
        );
    }
    pub(super) fn find(&self, direction: Option<i32>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let app = self.coordinator.borrow();
        let Some(editor) = app.editor() else {
            return;
        };
        let mut find = self.search.find.borrow_mut();
        if direction.is_none() {
            find.set_query(editor.body(), window.get_find_query().to_string());
        }
        let selection = find.step(editor.body(), direction.unwrap_or(1));
        drop(find);
        self.present_find(&window);
        if let Some(selection) = selection {
            if let (Ok(anchor), Ok(caret)) =
                (i32::try_from(selection.start), i32::try_from(selection.end))
            {
                window.invoke_select_find_match(anchor, caret);
            } else {
                window.set_find_status("Match too far".into());
            }
        }
    }
}
