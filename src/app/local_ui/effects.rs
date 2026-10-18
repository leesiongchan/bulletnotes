//! Bounded ordered writes, latest-request reads and effect delivery.
use super::{LocalUiError, State};
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
    Barrier(oneshot::Sender<()>),
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
}
pub(super) struct Workers {
    work: mpsc::Receiver<Work>,
    load_requests: watch::Receiver<Option<LoadRequest>>,
    search_requests: watch::Receiver<Option<SearchRequest>>,
}
impl Effects {
    pub(super) fn channels() -> (Self, Workers) {
        let (writes, work) = mpsc::channel(16);
        let (loads, load_requests) = watch::channel(None);
        let (searches, search_requests) = watch::channel(None);
        (
            Self {
                writes,
                loads,
                searches,
                save_timer: slint::Timer::default(),
            },
            Workers {
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
        receiver.await.map_err(|_| StoreError::Closed)
    }
}

impl State {
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
                AppEffect::Search(request) => {
                    self.effects.searches.send_replace(Some(request));
                }
                AppEffect::LoadPage { request_id, key } => {
                    self.effects.loads.send_replace(Some(LoadRequest {
                        id: request_id,
                        key,
                    }));
                }
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
                            AppEffect::Recovery(_) => AppEvent::RecoveryFinished {
                                connection: self.coordinator.borrow().connection().clone(),
                                result: Err(StoreError::Closed),
                            },
                            AppEffect::CreateDraft(key) => {
                                AppEvent::LocalSaveFailed(key, LocalRevision(0), StoreError::Closed)
                            }
                            AppEffect::SaveLocal { key, revision, .. } => {
                                AppEvent::LocalSaveFailed(key, revision, StoreError::Closed)
                            }
                            AppEffect::SavePosition { key, .. } | AppEffect::RecordVisit(key) => {
                                AppEvent::MetadataFailed(key)
                            }
                            _ => continue,
                        };
                        self.receive(event);
                    }
                }
            }
        }
    }
}

fn now_ms() -> Result<u64, StoreError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::InvalidData)?;
    u64::try_from(elapsed.as_millis()).map_err(|_| StoreError::InvalidData)
}

impl Workers {
    pub(super) fn start(self, state: &Rc<State>, store: StoreHandle) -> Result<(), LocalUiError> {
        let Self {
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
                            let _ = sender.send(());
                            continue;
                        }
                        Work::Effect(AppEffect::Recovery(intent)) => {
                            let Some(state) = weak.upgrade() else {
                                break;
                            };
                            let connection = state.coordinator.borrow().connection().clone();
                            drop(state);
                            Some(AppEvent::RecoveryFinished {
                                connection: connection.clone(),
                                result: crate::app::recovery::execute(&store, &connection, intent)
                                    .await,
                            })
                        }
                        Work::Effect(AppEffect::CreateDraft(key)) => {
                            Some(match store.create_draft(key.clone()).await {
                                Ok(saved) => AppEvent::LocalSaved(saved),
                                Err(error) => {
                                    AppEvent::LocalSaveFailed(key, LocalRevision(0), error)
                                }
                            })
                        }
                        Work::Effect(AppEffect::SaveLocal {
                            key,
                            body,
                            revision,
                            baseline,
                        }) => Some(
                            match store
                                .save_local_from_base(key.clone(), body, revision, *baseline)
                                .await
                            {
                                Ok(saved) => AppEvent::LocalSaved(saved),
                                Err(error) => AppEvent::LocalSaveFailed(key, revision, error),
                            },
                        ),
                        Work::Effect(AppEffect::SavePosition { key, position }) => {
                            let result = match now_ms() {
                                Ok(now) => store.save_position(&key, position, now).await,
                                Err(error) => Err(error),
                            };
                            result.err().map(|_| AppEvent::MetadataFailed(key))
                        }
                        Work::Effect(AppEffect::RecordVisit(key)) => {
                            let result = match now_ms() {
                                Ok(now) => store.record_visit(&key, now).await,
                                Err(error) => Err(error),
                            };
                            result.err().map(|_| AppEvent::MetadataFailed(key))
                        }
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
                        let result = store.load_active_page(&request.key).await;
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
