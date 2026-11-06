//! The page list behind wikilink styling and suggestions.
use super::State;
use crate::{
    AppWindow, LinkSuggestion, LinkSuggestions,
    app::markdown,
    domain::{connection::ConnectionId, link},
    store::LinkCatalogue,
};
use slint::{Model, ModelRc, SharedString};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Most suggestions shown for a partly typed link.
const SUGGESTIONS: usize = 8;

#[derive(Default)]
pub(super) struct LinkPages {
    catalogue: RefCell<Option<(ConnectionId, LinkCatalogue)>>,
    refreshing: Cell<bool>,
    refresh_needed: Cell<bool>,
    /// The note text `missing-links` was last computed for, until the list changes.
    checked: RefCell<Option<SharedString>>,
}

impl State {
    /// Reread the page list, coalescing requests that arrive while a read runs.
    pub(super) fn refresh_link_pages(self: &Rc<Self>) {
        self.links.refresh_needed.set(true);
        if self.links.refreshing.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        if slint::spawn_local(async move {
            loop {
                let Some(state) = weak.upgrade() else {
                    break;
                };
                state.links.refresh_needed.set(false);
                let connection = state.coordinator.borrow().connection().clone();
                match state.store.link_catalogue(&connection).await {
                    Ok(catalogue) => {
                        state.links.catalogue.replace(Some((connection, catalogue)));
                        state.links.checked.take();
                        if let Some(window) = state.window.upgrade() {
                            window.set_link_pages_revision(
                                window.get_link_pages_revision().wrapping_add(1),
                            );
                            state.update_missing_links(&window);
                        }
                    }
                    Err(error) => tracing::warn!(%error, "wikilink page list could not be read"),
                }
                if !state.links.refresh_needed.get() {
                    state.links.refreshing.set(false);
                    break;
                }
            }
        })
        .is_err()
        {
            self.links.refreshing.set(false);
            tracing::warn!("wikilink page list refresh could not be scheduled");
        }
    }

    /// Mark the note's wikilinks whose pages do not exist.
    pub(super) fn update_missing_links(&self, window: &AppWindow) {
        let text = window.get_note_text();
        if self.links.checked.borrow().as_ref() == Some(&text) {
            return;
        }
        let connection = self.coordinator.borrow().connection().clone();
        let mut missing: Vec<SharedString> = Vec::new();
        if let Some((owner, catalogue)) = self.links.catalogue.borrow().as_ref()
            && *owner == connection
        {
            for (target, _) in markdown::wikilinks(&text) {
                if !missing.iter().any(|known| known == target.as_str())
                    && link::dangles(&target, &catalogue.pages, catalogue.listed)
                {
                    missing.push(target.into());
                }
            }
        }
        let current = window.get_missing_links();
        if !current.iter().eq(missing.iter().cloned()) {
            window.set_missing_links(ModelRc::from(missing.as_slice()));
        }
        self.links.checked.replace(Some(text));
    }

    /// Pages whose names match the unfinished `[[name` at `caret`.
    pub(super) fn suggest_links(&self, source: &str, caret: i32) -> LinkSuggestions {
        let none = LinkSuggestions {
            start: -1,
            ..Default::default()
        };
        let Some(caret) = usize::try_from(caret).ok() else {
            return none;
        };
        let Some(completion) = link::completion_at(source, caret) else {
            return none;
        };
        if markdown::in_code(source, caret) {
            return none;
        }
        // Slint evaluates this lazily, possibly while the coordinator is being updated.
        let Ok(app) = self.coordinator.try_borrow() else {
            return none;
        };
        let catalogue = self.links.catalogue.borrow();
        let Some((_, catalogue)) = catalogue
            .as_ref()
            .filter(|(owner, _)| owner == app.connection())
        else {
            return none;
        };
        let items: Vec<LinkSuggestion> =
            link::suggest(&completion.query, &catalogue.pages, SUGGESTIONS)
                .into_iter()
                .filter_map(|name| {
                    let replacement = format!("{name}]]");
                    let caret = i32::try_from(completion.start + replacement.len()).ok()?;
                    Some(LinkSuggestion {
                        name: name.into(),
                        replacement: replacement.into(),
                        caret,
                    })
                })
                .collect();
        let Ok(start) = i32::try_from(completion.start) else {
            return none;
        };
        LinkSuggestions {
            start,
            items: ModelRc::from(items.as_slice()),
        }
    }
}
