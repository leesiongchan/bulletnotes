//! Bounded ordered writes, latest-request reads and effect delivery.
use super::{LocalUiError, State, metadata::MetadataJournal};
use crate::{
    app::coordinator::{AppAction, AppEffect, AppEvent, SearchRequest},
    domain::{error::StoreError, page::*},
    store::StoreHandle,
};
use slint::ComponentHandle;
use std::{
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot, watch};

enum Work {
    Effect(AppEffect),
    Metadata,
    Barrier(oneshot::Sender<Result<(), StoreError>>),
}
#[derive(Clone)]
struct LoadRequest {
    id: u64,
    key: PageKey,
}
pub(super) struct Effects {
    writes: mpsc::Sender<Work>,
    loads: watch::Sender<Option<LoadRequest>>,
    searches: watch::Sender<Option<SearchRequest>>,
    save_timer: slint::Timer,
    metadata: MetadataJournal,
}
pub(super) struct Workers {
    metadata: MetadataJournal,
    work: mpsc::Receiver<Work>,
    load_requests: watch::Receiver<Option<LoadRequest>>,
    search_requests: watch::Receiver<Option<SearchRequest>>,
}
impl Effects {
    pub(super) fn channels() -> (Self, Workers) {
        let metadata = MetadataJournal::default();
        let (writes, work) = mpsc::channel(16);
        let (loads, load_requests) = watch::channel(None);
        let (searches, search_requests) = watch::channel(None);
        (
            Self {
                metadata: metadata.clone(),
                writes,
                loads,
                searches,
                save_timer: slint::Timer::default(),
            },
            Workers {
                metadata,
                work,
                load_requests,
                search_requests,
            },
        )
    }
    pub(super) async fn barrier(&self) -> Result<(), StoreError> {
        let (sender, receiver) = oneshot::channel();
        self.writes
            .send(Work::Barrier(sender))
            .await
            .map_err(|_| StoreError::Closed)?;
        receiver.await.map_err(|_| StoreError::Closed)?
    }
}

impl State {
    fn queue_metadata(
        self: &Rc<Self>,
        key: PageKey,
        position: Option<crate::domain::editor::EditorPosition>,
    ) {
        self.effects.metadata.record(key.clone(), position);
        if self.effects.writes.try_send(Work::Metadata).is_err() {
            self.receive(AppEvent::MetadataFailed(key));
        }
    }
    pub(super) fn arm_save(self: &Rc<Self>) {
        self.effects.save_timer.stop();
        let deadline = self
            .coordinator
            .borrow()
            .editor()
            .and_then(|editor| editor.next_save_at());
        if let Some(deadline) = deadline {
            let weak = Rc::downgrade(self);
            self.effects.save_timer.start(
                slint::TimerMode::SingleShot,
                deadline
                    .saturating_sub(self.started.elapsed())
                    .max(Duration::from_millis(1)),
                move || {
                    if let Some(state) = weak.upgrade() {
                        state.dispatch(AppAction::Tick(state.started.elapsed()));
                    }
                },
            );
        }
    }
    pub(super) fn execute(self: &Rc<Self>, effects: Vec<AppEffect>) {
        for effect in effects {
            match effect {
                AppEffect::RecoveryCancelled { connection } => {
                    let current = connection == *self.coordinator.borrow().connection();
                    if current && let Some(window) = self.window.upgrade() {
                        window.invoke_recovery_completed(
                            false,
                            "Recovery action cancelled. Your saved versions are still available."
                                .into(),
                        );
                    }
                }
                AppEffect::CheckTransition {
                    request_id,
                    connection,
                } => {
                    let state = self.clone();
                    let _ = slint::spawn_local(async move {
                        let result = state.effects.barrier().await;
                        state.receive(AppEvent::TransitionChecked {
                            request_id,
                            connection,
                            result,
                        });
                    });
                }
                AppEffect::LoadScratchpads {
                    request_id,
                    connection,
                } => self.load_scratchpads(request_id, connection),
                AppEffect::Search(request) => {
                    self.effects.searches.send_replace(Some(request));
                }
                AppEffect::LoadPage { request_id, key } => {
                    self.effects.loads.send_replace(Some(LoadRequest {
                        id: request_id,
                        key,
                    }));
                }
                AppEffect::SavePosition { key, position } => {
                    self.queue_metadata(key, Some(position))
                }
                AppEffect::RecordVisit(key) => self.queue_metadata(key, None),
                AppEffect::QuitReady => self.quit(),
                AppEffect::HideReady => {
                    if let Some(window) = self.window.upgrade() {
                        if let Some(sync) = self.sync.handle.borrow().as_ref() {
                            sync.set_active(false);
                        }
                        window.invoke_window_will_hide();
                        let _ = window.hide();
                    }
                    self.finish_lifecycle();
                }
                effect => {
                    if let Err(error) = self.effects.writes.try_send(Work::Effect(effect))
                        && let Work::Effect(effect) = error.into_inner()
                    {
                        let event = match effect {
                            AppEffect::Recovery {
                                request_id,
                                connection,
                                ..
                            } => AppEvent::RecoveryFinished {
                                request_id,
                                connection,
                                result: Err(StoreError::Closed),
                            },
                            AppEffect::CreateDraft { expected, .. } => AppEvent::LocalSaveFailed(
                                expected,
                                LocalRevision(0),
                                StoreError::Closed,
                            ),
                            AppEffect::SaveLocal {
                                expected, revision, ..
                            } => AppEvent::LocalSaveFailed(expected, revision, StoreError::Closed),
                            _ => continue,
                        };
                        self.receive(event);
                    }
                }
            }
        }
    }
}

pub(super) fn now_ms() -> Result<u64, StoreError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::InvalidData)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| StoreError::InvalidData)
}

impl Workers {
    pub(super) fn start(self, state: &Rc<State>, store: StoreHandle) -> Result<(), LocalUiError> {
        let Self {
            metadata,
            mut work,
            mut load_requests,
            mut search_requests,
        } = self;
        {
            let weak = Rc::downgrade(state);
            let store = store.clone();
            slint::spawn_local(async move {
                while let Some(work) = work.recv().await {
                    let event = match work {
                        Work::Barrier(sender) => {
                            let result = metadata.flush(&store).await.map_err(|(_, error)| error);
                            if result.is_ok()
                                && let Some(state) = weak.upgrade()
                            {
                                state.receive(AppEvent::MetadataDurable);
                            }
                            let _ = sender.send(result);
                            continue;
                        }
                        Work::Metadata => Some(match metadata.flush(&store).await {
                            Ok(()) => AppEvent::MetadataDurable,
                            Err((key, _)) => AppEvent::MetadataFailed(key),
                        }),
                        Work::Effect(AppEffect::Recovery {
                            request_id,
                            connection,
                            intent,
                        }) => Some(AppEvent::RecoveryFinished {
                            request_id,
                            connection: connection.clone(),
                            result: crate::app::recovery::execute(&store, &connection, intent)
                                .await,
                        }),
                        Work::Effect(AppEffect::CreateDraft { key, expected }) => {
                            Some(match store.create_draft_outcome(key).await {
                                Ok(outcome) => AppEvent::LocalSaved { expected, outcome },
                                Err(error) => {
                                    AppEvent::LocalSaveFailed(expected, LocalRevision(0), error)
                                }
                            })
                        }
                        Work::Effect(AppEffect::SaveLocal {
                            expected,
                            key,
                            body,
                            revision,
                            baseline,
                        }) => Some(
                            match store
                                .save_local_from_base_outcome(key, body, revision, *baseline)
                                .await
                            {
                                Ok(outcome) => AppEvent::LocalSaved { expected, outcome },
                                Err(error) => AppEvent::LocalSaveFailed(expected, revision, error),
                            },
                        ),
                        _ => None,
                    };
                    if let Some(state) = weak.upgrade()
                        && let Some(event) = event
                    {
                        state.receive(event);
                    }
                }
            })?;
        }
        {
            let weak = Rc::downgrade(state);
            let store = store.clone();
            slint::spawn_local(async move {
                while load_requests.changed().await.is_ok() {
                    let request = load_requests.borrow_and_update().clone();
                    if let Some(request) = request {
                        let result = store.load_editor_page(&request.key).await;
                        if let Some(state) = weak.upgrade() {
                            state.receive(AppEvent::PageLoaded {
                                request_id: request.id,
                                key: request.key,
                                result,
                            });
                        }
                    }
                }
            })?;
        }
        {
            let weak = Rc::downgrade(state);
            slint::spawn_local(async move {
                while search_requests.changed().await.is_ok() {
                    let request = search_requests.borrow_and_update().clone();
                    if let Some(request) = request {
                        let result = store.search(request.query).await;
                        if let Some(state) = weak.upgrade() {
                            state.receive(AppEvent::SearchReady(request.id, result));
                        }
                    }
                }
            })?;
        }
        Ok(())
    }
}
