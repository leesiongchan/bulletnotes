//! Generated-text native sync lifecycle fixture. Never opens the user's store.
#[path = "../tests/support/http_fixture.rs"]
mod http_fixture;
#[path = "support/sync_acceptance.rs"]
mod sync_acceptance;
use bulletnotes::{
    app::{
        connection::ConnectionController,
        connection_ui,
        native_app::LocalApplication,
        startup::{self, Startup},
        sync_ui,
    },
    domain::{
        connection::{ConnectionConfig, ConnectionId},
        error::{CredentialError, StoreError},
        page::*,
    },
    platform::{
        application::NativeApplication,
        credentials::{CredentialBackend, CredentialStore},
    },
    store::{StoreHandle, settings::AppSettings},
    sync::{SecretToken, SilverBulletClient},
};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    path::Path,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
const OFFLINE_SOURCE: &str = "Saved offline\r\n日本語 café 👩🏽‍💻\r\n";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
struct SessionOnly(Arc<AtomicUsize>);
impl CredentialBackend for SessionOnly {
    fn load(&self, _: &ConnectionId) -> std::result::Result<Option<SecretToken>, CredentialError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }
    fn save(&self, _: &ConnectionId, _: &SecretToken) -> std::result::Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn delete(&self, _: &ConnectionId) -> std::result::Result<(), CredentialError> {
        Ok(())
    }
}
fn config() -> ConnectionConfig {
    ConnectionConfig::parse("https://notes.example/prefix/").unwrap()
}
fn key() -> PageKey {
    PageKey {
        connection: config().id().clone(),
        path: PagePath::parse("sample.md").unwrap(),
    }
}
async fn until(mut predicate: impl FnMut() -> bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !predicate() {
        if Instant::now() >= deadline {
            return Err("native sync condition timed out".into());
        }
        let (sender, receiver) = tokio::sync::oneshot::channel();
        slint::Timer::single_shot(Duration::from_millis(15), move || {
            let _ = sender.send(());
        });
        receiver.await?;
    }
    Ok(())
}
fn restart(path: &Path, nonce: &str) -> Result<()> {
    if !path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.to_string_lossy().starts_with("bulletnotes-sync-"))
        || std::fs::read_to_string(path.with_extension("fixture-marker"))? != nonce
    {
        return Err("not an owned restart fixture".into());
    }
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let state = runtime.block_on(startup::open(path, config().id().clone()))?;
    let store = state.store.clone();
    let app = LocalApplication::new(state, native)?;
    let local = app.local().clone();
    let window = app.window().as_weak();
    let passed = Rc::new(Cell::new(false));
    let observed = passed.clone();
    slint::spawn_local(async move {
        let result = async {
            until(|| local.current_key() == Some(key())).await?;
            local.wait_until_loaded(&key()).await?;
            let window = window.upgrade().ok_or("closed fixture")?;
            if window.get_note_text() != OFFLINE_SOURCE {
                return Err("restart did not restore generated offline source".into());
            }
            observed.set(true);
            window.invoke_request_quit();
            Ok::<(), Box<dyn std::error::Error>>(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("restart fixture: {error}");
            let _ = slint::quit_event_loop();
        }
    })?;
    app.run()?;
    if !passed.get()
        || !matches!(
            runtime.block_on(store.durability()),
            Err(StoreError::Closed)
        )
    {
        return Err("restart fixture did not quit durably".into());
    }
    println!("native_sync_restart_passed: true");
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--cleanup-acceptance") {
        return sync_acceptance::cleanup_owned(Path::new(
            args.get(2).ok_or("missing owned cleanup directory")?,
        ));
    }
    if args.get(1).is_some_and(|arg| arg == "--acceptance-child") {
        return sync_acceptance::child(&args);
    }
    if args
        .get(1)
        .is_some_and(|arg| arg == "--acceptance" || arg == "--live-acceptance")
    {
        return sync_acceptance::supervise(args[1] == "--live-acceptance");
    }
    if args.get(1).is_some_and(|arg| arg == "--restart") {
        return restart(
            Path::new(args.get(2).ok_or("missing fixture path")?),
            args.get(3).ok_or("missing fixture nonce")?,
        );
    }
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let directory = tempfile::Builder::new()
        .prefix("bulletnotes-sync-")
        .tempdir()?;
    let path = directory.path().join("test.sqlite");
    let nonce = uuid::Uuid::new_v4().to_string();
    std::fs::write(path.with_extension("fixture-marker"), &nonce)?;
    let store = runtime.block_on(StoreHandle::open(&path))?;
    let fixture = runtime.block_on(http_fixture::Fixture::start(None));
    let client = SilverBulletClient::for_loopback_fixture(
        &config(),
        fixture.base.clone(),
        None,
        Duration::from_secs(2),
    )?;
    let credential_loads = Arc::new(AtomicUsize::new(0));
    let controller = ConnectionController::new(
        store.clone(),
        CredentialStore::new(Arc::new(SessionOnly(credential_loads.clone()))),
    );
    let outcome = runtime.block_on(controller.connect_fixture(client, None, false))?;
    if outcome.write_capability.is_err() {
        return Err("fixture write verification failed".into());
    }
    fixture.data.lock().unwrap().body = Some(b"base".to_vec());
    let remote = runtime.block_on(outcome.client.read(&key()))?;
    runtime.block_on(store.apply_remote(remote))?;
    let settings = AppSettings {
        active_connection: Some(config().id().clone()),
        ..Default::default()
    };
    let app = LocalApplication::new(
        Startup {
            store: store.clone(),
            settings,
            connection: config().id().clone(),
            last_page: Some(key()),
        },
        native,
    )?;
    let connection = connection_ui::bind(
        app.window(),
        controller,
        config().id(),
        runtime.handle().clone(),
    );
    let sync = sync_ui::bind(
        app.window(),
        app.local().clone(),
        store.clone(),
        runtime.handle().clone(),
        &connection,
    )?;
    let local = app.local().clone();
    let window = app.window().as_weak();
    let saved = store.clone();
    let network = runtime.handle().clone();
    let base = fixture.base.clone();
    let passed = Rc::new(Cell::new(false));
    let observed = passed.clone();
    slint::spawn_local(async move {
        let result = async {
            until(|| local.current_key() == Some(key())).await?;
            local.wait_until_loaded(&key()).await?;
            let window = window.upgrade().ok_or("closed fixture")?;
            let uncached = PageKey {
                connection: config().id().clone(),
                path: PagePath::parse("uncached.md")?,
            };
            saved
                .record_catalogue(
                    config().id(),
                    vec![RemoteMetadata {
                        key: uncached.clone(),
                        modified_ms: None,
                        content_length: None,
                        permission: Permission::ReadOnly,
                        revision: None,
                    }],
                    true,
                )
                .await?;
            window.invoke_browse_pages();
            local.search_current().await?;
            window.invoke_open_page(serde_json::to_string(&uncached)?.into());
            until(|| window.get_index_status().contains("Connect")).await?;
            if window.get_note_text() != "base" {
                return Err("uncached open replaced the current source".into());
            }
            window.invoke_return_to_note();
            window.set_connection_url("https://127.0.0.1:0/".into());
            window.invoke_test_connection();
            window.set_note_text("startup offline capture".into());
            window.invoke_note_edited("startup offline capture".into());
            local.wait_until_saved().await?;
            if until(|| credential_loads.load(Ordering::Relaxed) >= 2)
                .await
                .is_err()
            {
                return Err("initial offline connection did not retry automatically".into());
            }
            if saved.load_page(&key()).await?.unwrap().body != "startup offline capture" {
                return Err("capture failed while initial connection was offline".into());
            }
            drop(connection);
            window.set_connection_url(config().id().as_str().into());
            sync.activate(outcome).await?;
            until(|| window.get_status_text() == "Synced").await?;
            window.set_note_text("local one".into());
            window.invoke_note_edited("local one".into());
            local.wait_until_saved().await?;
            if saved.load_page(&key()).await?.unwrap().body != "local one" {
                return Err("local acknowledgement missing".into());
            }
            until(|| {
                fixture.body().as_deref() == Some(b"local one")
                    && window.get_status_text() == "Synced"
            })
            .await?;
            network
                .spawn(async move {
                    let peer = SilverBulletClient::for_loopback_fixture(
                        &config(),
                        base,
                        None,
                        Duration::from_secs(2),
                    )?;
                    let RemoteRead::Present(remote) = peer.read(&key()).await? else {
                        return Err(bulletnotes::domain::error::SyncError::Missing);
                    };
                    peer.put(&PendingMutation {
                        id: OperationId::new(),
                        key: key(),
                        local_revision: LocalRevision(0),
                        kind: MutationKind::Put("remote two-client edit".into()),
                        condition: WriteCondition::Match(remote.revision.unwrap()),
                    })
                    .await
                })
                .await??;
            local.refresh_sync();
            until(|| window.get_note_text() == "remote two-client edit").await?;
            fixture.fault(http_fixture::Fault::Status(503));
            local.refresh_sync();
            until(|| window.get_status_text().contains("Offline")).await?;
            window.set_note_text(OFFLINE_SOURCE.into());
            window.invoke_note_edited(OFFLINE_SOURCE.into());
            local.wait_until_saved().await?;
            if saved.load_page(&key()).await?.unwrap().body != OFFLINE_SOURCE {
                return Err("offline source was not durable".into());
            }
            if fixture.body().as_deref() != Some(b"remote two-client edit") {
                return Err("offline mode unexpectedly changed server content".into());
            }
            fixture.fault(http_fixture::Fault::None);
            local.refresh_sync();
            until(|| {
                fixture.body().as_deref() == Some(OFFLINE_SOURCE.as_bytes())
                    && window.get_status_text() == "Synced"
            })
            .await?;
            observed.set(true);
            window.invoke_request_quit();
            Ok::<(), Box<dyn std::error::Error>>(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("native sync fixture: {error}");
            let _ = slint::quit_event_loop();
        }
    })?;
    slint::Timer::single_shot(Duration::from_secs(20), || {
        eprintln!("native sync fixture deadline");
        let _ = slint::quit_event_loop();
    });
    app.run()?;
    if !passed.get()
        || !matches!(
            runtime.block_on(store.durability()),
            Err(StoreError::Closed)
        )
    {
        return Err("native sync fixture did not finish durably".into());
    }
    drop(app);
    let child = std::process::Command::new(std::env::current_exe()?)
        .arg("--restart")
        .arg(&path)
        .arg(nonce)
        .output()?;
    if !child.status.success()
        || !String::from_utf8_lossy(&child.stdout).contains("native_sync_restart_passed: true")
    {
        return Err(format!(
            "restart fixture failed: {}",
            String::from_utf8_lossy(&child.stderr)
        )
        .into());
    }
    println!("native_sync_contract_passed: true");
    Ok(())
}
