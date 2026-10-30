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

#[derive(Default)]
pub(super) struct SearchState {
    pub(super) find: RefCell<NoteFind>,
    pub(super) request_id: Cell<u64>,
    pub(super) invalid_folder: Cell<bool>,
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
        let folder = window.get_search_folder();
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
            scope: crate::store::search::SearchScope::AllPages,
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
