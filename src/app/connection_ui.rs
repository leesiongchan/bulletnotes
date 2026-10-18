//! Main-thread form ownership; HTTP and Keychain work runs on the Tokio runtime.
use super::connection::{ConnectionController, ConnectionInput, ConnectionOutcome};
use crate::{
    AppWindow,
    domain::{
        connection::ConnectionId,
        error::{AppError, SyncError},
    },
    sync::Retry,
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};
use tokio::{
    runtime::Handle,
    sync::watch,
    task::{AbortHandle, JoinHandle},
};
struct State {
    window: slint::Weak<AppWindow>,
    controller: ConnectionController,
    runtime: Handle,
    busy: Cell<bool>,
    outcomes: watch::Sender<Option<ConnectionOutcome>>,
    input: RefCell<Option<ConnectionInput>>,
    retry: slint::Timer,
    policy: RefCell<Retry>,
    epoch: Cell<u64>,
    abort: RefCell<Option<AbortHandle>>,
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(task) = self.abort.get_mut().take() {
            task.abort();
        }
    }
}
impl State {
    fn stop(&self) {
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.retry.stop();
        self.input.borrow_mut().take();
        if let Some(task) = self.abort.borrow_mut().take() {
            task.abort();
        }
        self.busy.set(false);
        if let Some(window) = self.window.upgrade() {
            window.set_connection_busy(false);
        }
    }
    fn retry_later(self: &Rc<Self>) {
        let delay = self.policy.borrow_mut().delay();
        let epoch = self.epoch.get();
        let weak = Rc::downgrade(self);
        self.retry
            .start(slint::TimerMode::SingleShot, delay, move || {
                if let Some(state) = weak.upgrade()
                    && state.epoch.get() == epoch
                {
                    state.start();
                }
            });
    }
    fn start(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        let Some(input) = self.input.borrow().clone() else {
            return;
        };
        let Some(window) = self.window.upgrade() else {
            return;
        };
        self.busy.set(true);
        window.set_connection_busy(true);
        window.set_connection_message("Checking access and safe updates…".into());
        let controller = self.controller.clone();
        let epoch = self.epoch.get();
        let task = self.runtime.spawn(async move {
            controller
                .connect(input.config, input.token, input.remember)
                .await
        });
        self.abort.replace(Some(task.abort_handle()));
        let weak = Rc::downgrade(self);
        if slint::spawn_local(Self::finish_attempt(weak, task, epoch)).is_err() {
            if let Some(task) = self.abort.borrow_mut().take() {
                task.abort();
            }
            self.busy.set(false);
            window.set_connection_busy(false);
            window.set_connection_message("Connection check could not start. Try again.".into());
        }
    }
    async fn finish_attempt(
        weak: Weak<Self>,
        task: JoinHandle<Result<ConnectionOutcome, AppError>>,
        epoch: u64,
    ) {
        let result = task.await;
        let Some(state) = weak.upgrade() else {
            return;
        };
        if state.epoch.get() != epoch {
            return;
        }
        state.abort.borrow_mut().take();
        state.busy.set(false);
        let Some(window) = state.window.upgrade() else {
            return;
        };
        window.set_connection_busy(false);
        match result {
            Ok(Ok(outcome)) => state.connected(&window, outcome),
            Ok(Err(AppError::Sync(SyncError::Network))) => {
                window.set_connection_message("Connection is unavailable. Retrying automatically; your local notes are still available.".into());
                state.retry_later();
            }
            Ok(Err(error)) => {
                state.input.borrow_mut().take();
                window.set_connection_message(
                    format!("Connection failed: {error}. Your local notes are still available.")
                        .into(),
                );
            }
            Err(_) => {
                state.input.borrow_mut().take();
                window.set_connection_message(
                    "Connection check stopped. Your local notes are still available; try again."
                        .into(),
                );
            }
        }
    }
    fn connected(&self, window: &AppWindow, outcome: ConnectionOutcome) {
        self.input.borrow_mut().take();
        self.policy.borrow_mut().reset();
        let mut message = match outcome.write_capability {
            Ok(_) => {
                if window.get_sync_available() {
                    "Connection verified · starting automatic sync…".to_owned()
                } else {
                    "Connection verified · automatic sync is off.".to_owned()
                }
            }
            Err(error) => format!("Reading is available. Write sync is paused: {error}."),
        };
        if let Some(warning) = outcome.credential_warning {
            message.push_str(&format!(" {warning}."));
        }
        if outcome.cleanup_pending {
            message.push_str(
                " Temporary-page cleanup is pending; reconnect to retry its guarded cleanup.",
            );
        }
        window.set_connection_url(outcome.client.connection_id().as_str().into());
        window.set_connection_message(message.into());
        self.outcomes.send_replace(Some(outcome));
    }
}
#[derive(Clone)]
pub struct ConnectionUi(Rc<State>);
impl ConnectionUi {
    pub fn subscribe(&self) -> watch::Receiver<Option<ConnectionOutcome>> {
        self.0.outcomes.subscribe()
    }
    pub fn stop(&self) {
        self.0.stop();
    }
}
pub fn bind(
    window: &AppWindow,
    controller: ConnectionController,
    active: &ConnectionId,
    runtime: Handle,
) -> ConnectionUi {
    window.set_connection_url(active.as_str().into());
    window.set_connection_message(
        "Automatic sync is off. Test your connection to check access and safe updates.".into(),
    );
    let (outcomes, _) = watch::channel(None);
    let state = Rc::new(State {
        window: window.as_weak(),
        controller,
        runtime,
        busy: Cell::new(false),
        outcomes,
        input: RefCell::new(None),
        retry: slint::Timer::default(),
        policy: RefCell::new(Retry::new(uuid::Uuid::new_v4().as_u128() as u64)),
        epoch: Cell::new(0),
        abort: RefCell::new(None),
    });
    let weak = Rc::downgrade(&state);
    window.on_test_connection(move || {
        let Some(state) = weak.upgrade() else {
            return;
        };
        if state.busy.get() {
            return;
        }
        let Some(window) = state.window.upgrade() else {
            return;
        };
        state.stop();
        let input = ConnectionInput::parse(
            window.get_connection_url().as_str(),
            window.get_connection_token().as_str(),
            window.get_connection_remember(),
        );
        window.set_connection_token("".into());
        match input {
            Ok(input) => {
                state.input.replace(Some(input));
                state.policy.borrow_mut().reset();
                state.start();
            }
            Err(error) => window.set_connection_message(error.to_string().into()),
        }
    });
    ConnectionUi(state)
}
