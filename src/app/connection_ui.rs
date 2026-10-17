//! Main-thread form ownership; HTTP and Keychain operations run on the Tokio runtime.
use super::connection::{ConnectionController, ConnectionInput, ConnectionOutcome};
use crate::{AppWindow, domain::connection::ConnectionId};
use slint::ComponentHandle;
use std::{cell::Cell, rc::Rc};
use tokio::{runtime::Handle, sync::watch};
struct State {
    window: slint::Weak<AppWindow>,
    controller: ConnectionController,
    runtime: Handle,
    busy: Cell<bool>,
    outcomes: watch::Sender<Option<ConnectionOutcome>>,
}
#[derive(Clone)]
pub struct ConnectionUi(Rc<State>);
impl ConnectionUi {
    pub fn subscribe(&self) -> watch::Receiver<Option<ConnectionOutcome>> {
        self.0.outcomes.subscribe()
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
    });
    let weak = Rc::downgrade(&state);
    window.on_test_connection(move || {
        let Some(state)=weak.upgrade() else { return; };
        if state.busy.get() { return; }
        let Some(window)=state.window.upgrade() else { return; };
        let input=ConnectionInput::parse(window.get_connection_url().as_str(),window.get_connection_token().as_str(),window.get_connection_remember());
        window.set_connection_token("".into());
        let input=match input { Ok(input)=>input, Err(error)=>{ window.set_connection_message(error.to_string().into()); return; } };
        state.busy.set(true); window.set_connection_busy(true); window.set_connection_message("Checking access and safe updates…".into());
        let controller=state.controller.clone();
        let task=state.runtime.spawn(async move { controller.connect(input.config,input.token,input.remember).await });
        let weak=Rc::downgrade(&state);
        if slint::spawn_local(async move {
            let result=task.await;
            let Some(state)=weak.upgrade() else { return; };
            state.busy.set(false);
            if let Some(window)=state.window.upgrade() {
                window.set_connection_busy(false);
                match result {
                    Ok(Ok(outcome))=> {
                        let mut message=match outcome.write_capability {
                            Ok(_)=>"Connection verified · automatic sync is off.".to_owned(),
                            Err(error)=>format!("Reading is available. Automatic sync is off: {error}."),
                        };
                        if let Some(warning)=outcome.credential_warning { message.push_str(&format!(" {warning}.")); }
                        if outcome.cleanup_pending { message.push_str(" Temporary-page cleanup is pending; testing again will retry its guarded cleanup."); }
                        window.set_connection_url(outcome.client.connection_id().as_str().into());
                        window.set_connection_message(message.into());
                        state.outcomes.send_replace(Some(outcome));
                    },
                    Ok(Err(error))=>window.set_connection_message(format!("Connection failed: {error}. Your local notes are still available.").into()),
                    Err(_)=>window.set_connection_message("Connection check stopped. Your local notes are still available; try again.".into()),
                }
            }
        }).is_err() {
            state.busy.set(false); window.set_connection_busy(false);
            window.set_connection_message("Connection check could not start. Try again.".into());
        }
    });
    ConnectionUi(state)
}
