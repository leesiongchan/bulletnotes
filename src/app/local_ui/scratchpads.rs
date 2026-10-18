//! Stack metadata refreshes are coalesced independently of search and note bodies.
use super::{LocalUi, LocalUiError, State};
use crate::{
    app::coordinator::{AppAction, AppEvent},
    domain::connection::ConnectionId,
};
use std::{rc::Rc, time::Duration};
#[derive(Default)]
pub(super) struct StackRefresh {
    pub(super) timer: slint::Timer,
}
impl State {
    pub(super) fn load_scratchpads(self: &Rc<Self>, request_id: u64, connection: ConnectionId) {
        let state = self.clone();
        let _ = slint::spawn_local(async move {
            let result = state.store.scratchpad_stack(&connection).await;
            state.receive(AppEvent::ScratchpadsLoaded {
                request_id,
                connection,
                result,
            });
        });
    }
    pub(super) fn schedule_scratchpads(self: &Rc<Self>) {
        if self.scratchpads.timer.running() {
            return;
        }
        let weak = Rc::downgrade(self);
        self.scratchpads.timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(100),
            move || {
                if let Some(state) = weak.upgrade() {
                    state.dispatch(AppAction::RefreshScratchpads);
                }
            },
        );
    }
}
impl LocalUi {
    pub async fn refresh_scratchpads(&self) -> Result<(), LocalUiError> {
        let mut updates = self.0.updates.subscribe();
        self.0.dispatch(AppAction::RefreshScratchpads);
        loop {
            let pending = self.0.coordinator.borrow().scratchpads_pending();
            if !pending {
                return if self.0.coordinator.borrow().scratchpads().is_some() {
                    Ok(())
                } else {
                    Err(LocalUiError::StackFailed)
                };
            }
            updates.changed().await.map_err(|_| LocalUiError::Closed)?;
        }
    }
}
