//! Renaming the current note from the rename bar.
use super::{LocalUi, State};
use crate::{
    PageRename,
    app::{
        coordinator::{AppAction, LinkOutcome},
        markdown,
        model::View,
    },
    domain::page::PagePath,
};
use slint::ComponentHandle;
use std::rc::Rc;

impl State {
    /// Open the rename bar for the current note and count the other pages whose links
    /// a rename would update.
    pub(super) fn begin_rename(self: &Rc<Self>) {
        let from = {
            let app = self.coordinator.borrow();
            if app.view() != View::Note || app.loading() {
                return;
            }
            let Some(editor) = app.editor() else {
                return;
            };
            editor.key().clone()
        };
        let Some(window) = self.window.upgrade() else {
            return;
        };
        if window.get_find_visible() {
            window.invoke_close_find();
        }
        let rename = window.global::<PageRename>();
        let name = from.path.as_str();
        rename.set_name(name.strip_suffix(".md").unwrap_or(name).into());
        rename.set_message(Default::default());
        rename.set_update_links(true);
        rename.set_link_pages(-1);
        if rename.get_open() {
            window.invoke_focus_rename();
        } else {
            rename.set_open(true);
        }
        self.presentation.renaming.replace(Some(from.clone()));
        let task = self.clone();
        if slint::spawn_local(async move {
            let counted = async {
                let sources = task.store.link_sources(&from.connection).await?;
                let catalogue = task.store.link_catalogue(&from.connection).await?;
                Ok::<_, crate::domain::error::StoreError>(
                    sources
                        .iter()
                        .filter(|(path, body)| {
                            *path != from.path
                                && markdown::retarget_links(
                                    body,
                                    path,
                                    &from.path,
                                    &from.path,
                                    &catalogue.pages,
                                )
                                .is_some()
                        })
                        .count(),
                )
            }
            .await;
            match counted {
                Ok(count) if task.presentation.renaming.borrow().as_ref() == Some(&from) => {
                    if let Some(window) = task.window.upgrade() {
                        window
                            .global::<PageRename>()
                            .set_link_pages(i32::try_from(count).unwrap_or(i32::MAX));
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, "links to a renamed page could not be counted")
                }
            }
        })
        .is_err()
        {
            tracing::warn!("link count for rename could not be scheduled");
        }
    }

    /// Validate the new name, then save the note and rename it.
    pub(super) fn submit_rename(self: &Rc<Self>, name: &str, update_links: bool) {
        let Some(from) = self.presentation.renaming.borrow().clone() else {
            return;
        };
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let current = self
            .coordinator
            .borrow()
            .editor()
            .is_some_and(|editor| editor.key() == &from);
        if !current {
            self.cancel_rename();
            return;
        }
        let reject = |message: &str| {
            window.global::<PageRename>().set_message(message.into());
        };
        let name = name.trim();
        let name = name.strip_prefix('/').unwrap_or(name);
        let name = name.strip_suffix(".md").unwrap_or(name).trim();
        if name.is_empty() {
            return reject("Enter a page name.");
        }
        let Ok(to) = PagePath::parse(&format!("{name}.md")) else {
            return reject("That is not a valid page name.");
        };
        if to == from.path {
            self.cancel_rename();
            return;
        }
        if self.page_known(&to) {
            return reject("A page with that name already exists.");
        }
        self.cancel_rename();
        let task = self.clone();
        if slint::spawn_local(async move {
            let saved = LocalUi(task.clone()).flush().await;
            let revision = task
                .coordinator
                .borrow()
                .editor()
                .filter(|editor| editor.key() == &from && !editor.dirty())
                .map(|editor| editor.revision());
            match (saved, revision) {
                (Ok(()), Some(revision)) => task.dispatch(AppAction::RenamePage {
                    from,
                    revision,
                    to,
                    update_links,
                }),
                _ => task.dispatch(AppAction::FollowLink {
                    from,
                    outcome: LinkOutcome::Refused(
                        "The note could not be saved, so it was not renamed. Try again.",
                    ),
                }),
            }
        })
        .is_err()
        {
            tracing::warn!("rename could not be scheduled");
        }
    }

    pub(super) fn cancel_rename(&self) {
        self.presentation.renaming.take();
        if let Some(window) = self.window.upgrade() {
            window.global::<PageRename>().set_open(false);
            window.invoke_focus_editor();
        }
    }

    /// Close the bar, without moving focus, once its note is no longer shown.
    pub(super) fn close_stale_rename(&self, window: &crate::AppWindow) {
        let stale = self
            .presentation
            .renaming
            .borrow()
            .as_ref()
            .is_some_and(|from| {
                let app = self.coordinator.borrow();
                app.view() != View::Note || app.editor().is_none_or(|editor| editor.key() != from)
            });
        if stale {
            self.presentation.renaming.take();
            window.global::<PageRename>().set_open(false);
        }
    }
}
