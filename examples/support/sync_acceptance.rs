//! Supervised generated-data acceptance. Parent owns server/relay across native restarts.
#[path = "acceptance_relay.rs"]
mod acceptance_relay;
#[path = "child_process.rs"]
mod child_process;
use child_process::ChildGuard;
#[path = "../../tests/support/space_fixture.rs"]
mod space_fixture;
use bulletnotes::{
    app::{
        connection::{ConnectionController, ConnectionOutcome},
        connection_ui,
        native_app::LocalApplication,
        startup, sync_ui,
    },
    domain::{
        connection::{ConnectionConfig, ConnectionId},
        error::CredentialError,
        page::*,
    },
    platform::{
        application::NativeApplication,
        credentials::{CredentialBackend, CredentialStore},
        window_chrome::native_window,
    },
    sync::{
        SecretToken, SilverBulletClient,
        probe::{new_probe_key, verify_write_guards},
    },
};
use slint::ComponentHandle;
use std::{
    cell::Cell,
    path::Path,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const BASE: &str = "red fox jumps\n\nstable café 日本語\n";
const REMOTE: &str = "red fox runs\n\nstable café 日本語\n";
const MERGED: &str = "Qred fox runs\n\nstable café 日本語\n";
const OVERLAP: &str = "green fox runs\n\nstable café 日本語\n";
const ORIGINAL: &str = "blue fox runs\n\nstable café 日本語\n\n\nlocal tail";
const CHOSEN: &str = "green fox runs\n\nstable café 日本語\n\n\nlocal tail";
const OFFLINE: &str = "offline restart\n日本語 café 👩🏽‍💻\n";
const BROWSER: &str = "offline restart\n日本語 café 👩🏽‍💻\n after delete\nbrowser independent edit";
const RECREATED: &str = "offline restart\n日本語 café 👩🏽‍💻\n after delete";
struct SessionOnly;
impl CredentialBackend for SessionOnly {
    fn load(&self, _: &ConnectionId) -> std::result::Result<Option<SecretToken>, CredentialError> {
        Ok(None)
    }
    fn save(&self, _: &ConnectionId, _: &SecretToken) -> std::result::Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn delete(&self, _: &ConnectionId) -> std::result::Result<(), CredentialError> {
        Ok(())
    }
}
fn operation(key: &PageKey, body: &str, condition: WriteCondition) -> PendingMutation {
    PendingMutation {
        id: OperationId::new(),
        key: key.clone(),
        local_revision: LocalRevision(0),
        kind: MutationKind::Put(body.into()),
        condition,
    }
}
fn mark(dir: &Path, name: &str) -> Result<()> {
    std::fs::write(dir.join(name), b"owned")?;
    Ok(())
}
fn browser_complete(dir: &Path, nonce: &str, page: &str) -> bool {
    std::fs::read(dir.join("browser-done"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|marker| {
            marker["nonce"].as_str() == Some(nonce) && marker["page"].as_str() == Some(page)
        })
}
async fn pause() -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    slint::Timer::single_shot(Duration::from_millis(10), move || {
        let _ = tx.send(());
    });
    rx.await?;
    Ok(())
}
async fn until(label: &str, mut predicate: impl FnMut() -> bool) -> Result<()> {
    let end = Instant::now() + Duration::from_secs(15);
    while !predicate() {
        if Instant::now() >= end {
            return Err(format!("acceptance condition timed out: {label}").into());
        }
        pause().await?;
    }
    Ok(())
}
fn type_text(window: &bulletnotes::AppWindow, source: &str) -> Result<()> {
    if !native_window(window.window())?.isKeyWindow() {
        return Err("owned fixture lost keyboard focus; input contaminated".into());
    }
    window.invoke_focus_editor();
    for pressed in [true, false] {
        window.window().dispatch_event(if pressed {
            slint::platform::WindowEvent::KeyPressed {
                text: source.into(),
            }
        } else {
            slint::platform::WindowEvent::KeyReleased {
                text: source.into(),
            }
        });
    }
    Ok(())
}
fn capture(window: &bulletnotes::AppWindow, dir: &Path, name: &str) -> Result<()> {
    let native = native_window(window.window())?;
    if !std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-o", &format!("-l{}", native.windowNumber())])
        .arg(dir.join(name))
        .status()?
        .success()
    {
        return Err("owned capture failed".into());
    }
    Ok(())
}
fn client(config: &ConnectionConfig, base: &url::Url) -> Result<SilverBulletClient> {
    Ok(SilverBulletClient::for_loopback_fixture(
        config,
        base.clone(),
        None,
        Duration::from_secs(12),
    )?)
}
async fn activate(config: &ConnectionConfig, base: &url::Url) -> Result<ConnectionOutcome> {
    let client = client(config, base)?;
    let probe = std::env::var("BULLETNOTES_ACCEPTANCE_PROBE")?;
    let evidence = verify_write_guards(
        &client,
        PageKey {
            connection: config.id().clone(),
            path: PagePath::parse(&probe)?,
        },
    )
    .await?;
    Ok(ConnectionOutcome {
        client,
        readable: true,
        write_capability: Ok(evidence),
        credential_warning: None,
        cleanup_pending: false,
    })
}
pub fn child(args: &[String]) -> Result<()> {
    let dir = std::path::PathBuf::from(args.get(2).ok_or("missing owned directory")?);
    let nonce = args.get(3).ok_or("missing nonce")?;
    if !dir
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("bulletnotes-acceptance-")
        || std::fs::read_to_string(dir.join("marker"))? != *nonce
    {
        return Err("not an owned native fixture".into());
    }
    let browser = args.get(4).is_some_and(|s| s == "browser");
    let resume = browser || args.get(4).is_some_and(|s| s == "resume");
    let config = ConnectionConfig::parse(&std::env::var("BULLETNOTES_ACCEPTANCE_CONFIG")?)?;
    let base = url::Url::parse(&std::env::var("BULLETNOTES_ACCEPTANCE_RELAY")?)?;
    let key = PageKey {
        connection: config.id().clone(),
        path: PagePath::parse(&std::env::var("BULLETNOTES_ACCEPTANCE_PAGE")?)?,
    };
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let path = dir.join("owned.sqlite");
    let state = runtime.block_on(startup::open(&path, config.id().clone()))?;
    let store = state.store.clone();
    if !resume {
        runtime
            .block_on(store.apply_remote(runtime.block_on(client(&config, &base)?.read(&key))?))?;
    }
    let mut state = state;
    state.last_page = Some(key.clone());
    let app = LocalApplication::new(state, native)?;
    let controller =
        ConnectionController::new(store.clone(), CredentialStore::new(Arc::new(SessionOnly)));
    let connection = connection_ui::bind(
        app.window(),
        controller,
        config.id(),
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
    let weak = app.window().as_weak();
    let passed = Rc::new(Cell::new(false));
    let observed = passed.clone();
    let owned = dir.clone();
    let saved = store.clone();
    slint::spawn_local(async move {
        let result: Result<()> = async {
            eprintln!("acceptance phase: waiting for owned page");
            until("owned page selected (line 74)", || local.current_key() == Some(key.clone())).await?;
            local.wait_until_loaded(&key).await?;
            let window = weak.upgrade().ok_or("closed acceptance fixture")?;
            eprintln!("acceptance phase: owned page loaded");
            if browser {
                sync.activate(activate(&config, &base).await?).await?;
                until("browser peer native exact source and synced (line 80)", || window.get_note_text() == BROWSER && window.get_status_text() == "Synced").await?;
                let RemoteRead::Present(file) = client(&config, &base)?.read(&key).await? else { return Err("missing browser source".into()); };
                if file.body != BROWSER { return Err("browser/native exact source differs".into()); }
                capture(&window, &owned, "browser-peer.png")?;
                println!("native_browser_independent_edit_readback: true");
            } else if resume {
                if window.get_note_text() != OFFLINE || !saved.load_sync_page(&key).await?.ok_or("missing restart source")?.1.pending { return Err("offline restart lost pending exact source".into()); }
                if client(&config, &base)?.read(&key).await.is_ok() { return Err("restart transport was not offline".into()); }
                mark(&owned, "resume-verified")?;
                until("supervisor resumed transport after offline restoration (line 89)", || owned.join("online").exists()).await?;
                sync.activate(activate(&config, &base).await?).await?;
                until("native synchronized status (line 91)", || window.get_status_text() == "Synced").await?;
                let peer = client(&config, &base)?;
                let RemoteRead::Present(file) = peer.read(&key).await? else { return Err("missing resumed upload".into()); };
                if file.body != OFFLINE { return Err("resumed upload differs".into()); }
                mark(&owned, "arm-delete")?; until("supervisor armed owned observation gate (line 95)", || owned.join("armed-delete").exists()).await?;
                let mut deletion = operation(&key, "", WriteCondition::Match(file.revision.ok_or("missing validator")?)); deletion.kind = MutationKind::Delete;
                peer.delete(&deletion).await?;
                local.refresh_sync(); until("owned observation held before typing (line 98)", || owned.join("held-delete").exists()).await?;
                window.invoke_restore_editor_position(OFFLINE.len() as i32, OFFLINE.len() as i32, 0.0);
                type_text(&window, " after delete")?;
                if window.get_note_text() != RECREATED { return Err("delete/edit typing differs".into()); }
                mark(&owned, "release-delete")?;
                local.wait_until_saved().await?;
                until("native synchronized status (line 104)", || window.get_status_text() == "Synced").await?;
                let RemoteRead::Present(file) = peer.read(&key).await? else { return Err("delete/edit did not recreate owned source".into()); };
                if file.body != RECREATED { return Err("delete/edit readback differs".into()); }
                capture(&window, &owned, "restarted.png")?;
                println!("native_offline_restart_resumed_upload_delete_edit: true");
            } else {
                sync.activate(activate(&config, &base).await?).await?;
                until("native synchronized status (line 111)", || window.get_status_text() == "Synced").await?;
                let peer = client(&config, &base)?;
                let current = saved.load_page(&key).await?.ok_or("missing source")?;
                mark(&owned, "arm-independent")?; until("supervisor armed owned observation gate (line 114)", || owned.join("armed-independent").exists()).await?;
                peer.put(&operation(&key, REMOTE, WriteCondition::Match(current.remote_base.ok_or("missing base")?.revision.ok_or("missing validator")?))).await?;
                local.refresh_sync(); until("owned observation held before typing (line 116)", || owned.join("held-independent").exists()).await?;
                window.invoke_restore_editor_position(0, 0, 0.0);
                type_text(&window, "Q")?;
                if window.get_note_text() != format!("Q{BASE}") || saved.load_page(&key).await?.unwrap().body != BASE { return Err("typing was not uncommitted at remote release".into()); }
                mark(&owned, "release-independent")?;
                until("independent native chosen source (line 121)", || window.get_note_text() == MERGED).await?;
                local.wait_until_saved().await?;
                until("native synchronized status (line 123)", || window.get_status_text() == "Synced").await?;
                eprintln!("acceptance phase: independent unsaved merge synchronized");
                capture(&window, &owned, "independent.png")?;
                let current = saved.load_page(&key).await?.unwrap();
                mark(&owned, "arm-overlap")?; until("supervisor armed owned observation gate (line 127)", || owned.join("armed-overlap").exists()).await?;
                peer.put(&operation(&key, OVERLAP, WriteCondition::Match(current.remote_base.unwrap().revision.unwrap()))).await?;
                local.refresh_sync(); until("owned observation held before typing (line 129)", || owned.join("held-overlap").exists()).await?;
                window.invoke_restore_editor_position(0, 4, 0.0); type_text(&window, "blue")?;
                window.invoke_restore_editor_position(MERGED.len() as i32, MERGED.len() as i32, 0.0); type_text(&window, "\n\nlocal tail")?;
                if window.get_note_text() != ORIGINAL { return Err("overlap original typing differs".into()); }
                mark(&owned, "release-overlap")?;
                until("overlap native chosen source (line 134)", || window.get_note_text() == CHOSEN).await?;
                local.wait_until_saved().await?;
                let recovery_deadline = Instant::now() + Duration::from_secs(8);
                let uploads = loop {
                    // Pending queue heads omit leased work; inspect only this owned fixture's identities.
                    let db = rusqlite::Connection::open(owned.join("owned.sqlite"))?;
                    let mut query = db.prepare("SELECT event_id,ordinal,operation_id FROM recovery_uploads WHERE current=1")?;
                    let uploads = query.query_map([], |row| Ok((row.get::<_,String>(0)?,row.get::<_,u32>(1)?,row.get::<_,String>(2)?)))?.collect::<std::result::Result<Vec<_>,_>>()?;
                    if !uploads.is_empty() { break uploads; }
                    if Instant::now() >= recovery_deadline { return Err("automatic overlap obligation not committed".into()); }
                    pause().await?;
                };
                let mut registrations = Vec::new();
                for (event, ordinal, operation) in &uploads {
                    let upload = saved.read_recovery_upload(uuid::Uuid::parse_str(event)?, *ordinal, OperationId(uuid::Uuid::parse_str(operation)?)).await?;
                    if let MutationKind::Put(body) = upload.operation.kind { registrations.push((upload.operation.key.path.as_str().to_owned(), body)); }
                }
                std::fs::write(owned.join("registrations.json"), serde_json::to_vec(&registrations)?)?;
                until("supervisor exact recovery registration (line 152)", || owned.join("registered").exists()).await?;
                local.refresh_sync(); until("native synchronized status (line 153)", || window.get_status_text() == "Synced").await?;
                if window.get_active_view() != "note" || window.get_recovery_selected() { return Err("automatic overlap forced recovery navigation".into()); }
                let mut original_readback = false;
                for (path, body) in &registrations { let RemoteRead::Present(file) = peer.read(&PageKey { connection: config.id().clone(), path: PagePath::parse(path)? }).await? else { return Err("missing immutable recovery readback".into()); }; if file.body != *body { return Err("immutable recovery differs".into()); } original_readback |= body == ORIGINAL; }
                if !original_readback { return Err("original overlap missing from immutable recovery".into()); }
                let durability_deadline = Instant::now() + Duration::from_secs(15);
                while saved.load_sync_page(&key).await?.unwrap().1.pending || !saved.recovery_uploads(config.id(), 100).await?.is_empty() {
                    if Instant::now() >= durability_deadline { return Err("chosen source not durably acknowledged".into()); }
                    pause().await?;
                }
                let copies = saved.recovery_copies(config.id()).await?.len();
                let counts = || -> Result<(i64,i64)> { let db = rusqlite::Connection::open(owned.join("owned.sqlite"))?; Ok((db.query_row("SELECT count(*) FROM automatic_events", [], |row| row.get(0))?,db.query_row("SELECT count(*) FROM recovery_uploads", [], |row| row.get(0))?)) };
                let identities = counts()?;
                let paths_before: std::collections::BTreeSet<_> = peer.list_files().await?.into_iter().map(|entry| entry.key.path.as_str().to_owned()).collect();
                for _ in 0..3 { local.refresh_sync(); for _ in 0..25 { pause().await?; } }
                let after = counts()?;
                let paths_after: std::collections::BTreeSet<_> = peer.list_files().await?.into_iter().map(|entry| entry.key.path.as_str().to_owned()).collect();
                eprintln!("unchanged polling counts: events/attempts before={identities:?} after={after:?}; local_history_before={copies} after={}", saved.recovery_copies(config.id()).await?.len());
                if saved.recovery_copies(config.id()).await?.len() != copies || after != identities || paths_before != paths_after { return Err("unchanged polls created duplicate recovery".into()); }
                eprintln!("acceptance phase: overlap and exact recovery synchronized");
                capture(&window, &owned, "overlap.png")?;
                mark(&owned, "offline")?; until("supervisor transport remains offline (line 174)", || owned.join("offline-ready").exists()).await?;
                local.refresh_sync(); until("native offline status (line 175)", || window.get_status_text().contains("Offline")).await?;
                window.invoke_restore_editor_position(0, CHOSEN.len() as i32, 0.0); type_text(&window, OFFLINE)?;
                local.wait_until_saved().await?;
                if saved.load_editor_page(&key).await?.unwrap().presented().body != OFFLINE || !saved.load_sync_page(&key).await?.unwrap().1.pending { return Err("offline authored source not durable/pending".into()); }
                capture(&window, &owned, "offline.png")?;
                println!("native_unsaved_remote_independent_overlap_exact_recovery_no_prompt_dedup: true");
            }
            observed.set(true); window.invoke_request_quit(); Ok(())
        }.await;
        if let Err(error) = result {
            eprintln!("acceptance fixture: {error}");
            if let Some(window) = weak.upgrade() {
                eprintln!(
                    "acceptance finite state: status={} view={} chosen={} original={}",
                    window.get_status_text(),
                    window.get_active_view(),
                    window.get_note_text() == CHOSEN,
                    window.get_note_text() == ORIGINAL
                );
            }
            if let Some(worker) = local.set_sync(None) {
                let snapshot = worker.snapshot();
                eprintln!(
                    "acceptance worker state: pending={} conflicts={} complete={} error={:?} page_errors={}",
                    snapshot.pending,
                    snapshot.conflicts,
                    snapshot.complete,
                    snapshot.error,
                    snapshot.page_errors.len()
                );
                let _ = worker.shutdown().await;
            }
            let _ = slint::quit_event_loop();
        }
    })?;
    slint::Timer::single_shot(Duration::from_secs(90), || {
        eprintln!("acceptance native deadline");
        let _ = slint::quit_event_loop();
    });
    assert!(
        tokio::runtime::Handle::try_current().is_err(),
        "the application must enter its runtime without a fixture guard"
    );
    app.run(&runtime)?;
    if !passed.get() {
        return Err("native acceptance failed".into());
    }
    println!("acceptance_child_passed: true");
    Ok(())
}
pub fn supervise(live: bool) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let directory = tempfile::Builder::new()
        .prefix("bulletnotes-acceptance-")
        .tempdir()?;
    let owned_directory = directory.keep();
    let dir = owned_directory.as_path();
    let nonce = uuid::Uuid::new_v4().to_string();
    std::fs::write(dir.join("marker"), &nonce)?;
    let config = ConnectionConfig::parse(if live {
        "https://notes.harflix.lol/"
    } else {
        "https://acceptance.example/"
    })?;
    let page = PageKey {
        connection: config.id().clone(),
        path: PagePath::parse(&format!("Quick Notes/Bulletnotes Acceptance/{nonce}.md"))?,
    };
    let probe = new_probe_key(config.id().clone());
    let server = if live {
        None
    } else {
        Some(runtime.block_on(space_fixture::SpaceFixture::start()))
    };
    let upstream = server
        .as_ref()
        .map(|s| s.base.clone())
        .unwrap_or_else(|| config.base_url().clone());
    let (relay, base, relay_task) = runtime.block_on(acceptance_relay::Relay::start(
        upstream,
        dir.join("owned-paths.json"),
        live,
    ))?;
    relay.register(
        page.path.as_str(),
        &[
            BASE, REMOTE, MERGED, OVERLAP, ORIGINAL, CHOSEN, OFFLINE, RECREATED, BROWSER,
        ],
    )?;
    let probe_id = probe
        .path
        .as_str()
        .rsplit('/')
        .next()
        .unwrap()
        .strip_suffix(".md")
        .unwrap();
    let probe_sources = ['A', 'B', 'C']
        .map(|stage| format!("Bulletnotes temporary compatibility probe\n{probe_id}\n{stage}\n"));
    relay.register(
        probe.path.as_str(),
        &probe_sources.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    let peer = client(&config, &base)?;
    runtime.block_on(peer.put(&operation(&page, BASE, WriteCondition::CreateOnly)))?;
    // Keep the complete owned journal even when a child fails; never guess cleanup bodies.
    let evidence = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".superpowers/sdd/2026-10-18-automatic-sync");
    let evidence_dir = evidence.join(if live { "task6-live" } else { "task6-native" });
    std::fs::create_dir_all(&evidence_dir)?;
    let mut failure = None;
    for phase in ["typing", "resume", "browser"] {
        if phase == "browser" && !live {
            continue;
        }
        if failure.is_some() {
            break;
        }
        if phase == "browser" {
            std::fs::write(
                evidence_dir.join("browser-ready.json"),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"nonce":nonce,"page":page.path.as_str(),"before":RECREATED,"after":BROWSER}),
                )?,
            )?;
            let deadline = Instant::now() + Duration::from_secs(300);
            while !browser_complete(&evidence_dir, &nonce, page.path.as_str()) {
                if Instant::now() >= deadline {
                    failure = Some("owned browser peer deadline");
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            if failure.is_some() {
                break;
            }
        }
        let log = std::fs::File::create(evidence_dir.join(format!("{phase}.log")))?;
        let mut command = std::process::Command::new(std::env::current_exe()?);
        command.args(["--acceptance-child", dir.to_str().unwrap(), &nonce]);
        if phase != "typing" {
            command.arg(phase);
        }
        let mut child = ChildGuard::new(
            command
                .env("BULLETNOTES_ACCEPTANCE_CONFIG", config.id().as_str())
                .env("BULLETNOTES_ACCEPTANCE_RELAY", base.as_str())
                .env("BULLETNOTES_ACCEPTANCE_PAGE", page.path.as_str())
                .env("BULLETNOTES_ACCEPTANCE_PROBE", probe.path.as_str())
                .stdout(log.try_clone()?)
                .stderr(log)
                .spawn()?,
        );
        let deadline = Instant::now() + Duration::from_secs(100);
        loop {
            for stage in ["independent", "overlap", "delete"] {
                if dir.join(format!("arm-{stage}")).exists()
                    && !dir.join(format!("armed-{stage}")).exists()
                {
                    relay.arm(page.path.as_str());
                    mark(dir, &format!("armed-{stage}"))?;
                }
                if relay.waiting()
                    && dir.join(format!("armed-{stage}")).exists()
                    && !dir.join(format!("held-{stage}")).exists()
                {
                    mark(dir, &format!("held-{stage}"))?;
                }
                if dir.join(format!("release-{stage}")).exists()
                    && !dir.join(format!("released-{stage}")).exists()
                {
                    relay.release();
                    mark(dir, &format!("released-{stage}"))?;
                }
            }
            if dir.join("registrations.json").exists() && !dir.join("registered").exists() {
                let registrations: Vec<(String, String)> =
                    serde_json::from_slice(&std::fs::read(dir.join("registrations.json"))?)?;
                for (path, source) in registrations {
                    if !path
                        .strip_prefix(&format!("Bulletnotes Recovery/{nonce}-"))
                        .and_then(|value| value.strip_suffix(".md"))
                        .is_some_and(|value| {
                            uuid::Uuid::parse_str(value).is_ok_and(|id| {
                                id.get_version_num() == 4 && id.to_string() == value
                            })
                        })
                        || source != ORIGINAL
                    {
                        return Err("unowned recovery registration".into());
                    }
                    relay.register(&path, &[&source])?;
                }
                mark(dir, "registered")?;
            }
            if dir.join("offline").exists() && !dir.join("offline-ready").exists() {
                relay.offline(true);
                mark(dir, "offline-ready")?;
            }
            if dir.join("resume-verified").exists() && !dir.join("online").exists() {
                relay.offline(false);
                mark(dir, "online")?;
            }
            if let Some(status) = child.try_wait()? {
                if !status.success() {
                    failure = Some("native child failed; see owned evidence logs");
                }
                break;
            }
            if Instant::now() >= deadline {
                child.terminate_and_reap()?;
                failure = Some("native child deadline");
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    // Child process has exited: no worker or retired-delete watch can recreate cleaned pages.
    relay.offline(false);
    let mut cleaned = 0;
    for (path, sources) in relay.paths() {
        let key = PageKey {
            connection: config.id().clone(),
            path: PagePath::parse(&path)?,
        };
        if let RemoteRead::Present(file) = runtime.block_on(peer.read(&key))? {
            if !sources.contains(&file.body) {
                return Err("cleanup refused unexpected owned source; journal retained".into());
            }
            let mut deletion = operation(
                &key,
                "",
                WriteCondition::Match(file.revision.ok_or("cleanup lacks strong validator")?),
            );
            deletion.kind = MutationKind::Delete;
            runtime.block_on(peer.delete(&deletion))?;
        }
        if !matches!(runtime.block_on(peer.read(&key))?, RemoteRead::Missing(_)) {
            return Err("cleanup absence unverified".into());
        }
        cleaned += 1;
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|e| e == "png")
            || entry.file_name() == "owned-paths.json"
        {
            std::fs::copy(entry.path(), evidence_dir.join(entry.file_name()))?;
        }
    }
    std::fs::write(
        evidence_dir.join("summary.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"live":live,"routing":"exact-path allowlist relay; normal upstream TLS for live; owned catalogue only","cleaned_paths":cleaned,"guarded_cleanup_after_child_exit":true,"relay_rejections":relay.rejected(),"passed":failure.is_none(),"unsaved_at_remote_release":dir.join("release-independent").exists(),"restart_before_reconnect":dir.join("resume-verified").exists(),"browser_peer":live && browser_complete(&evidence_dir, &nonce, page.path.as_str()),"exact_live_selection_ime_voiceover":"unverified"}),
        )?,
    )?;
    relay_task.abort();
    if let Some(message) = failure {
        return Err(message.into());
    }
    println!("supervised_native_acceptance_passed: true");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_completion_requires_current_nonce_and_exact_page() {
        let dir = tempfile::tempdir().unwrap();
        let nonce = uuid::Uuid::new_v4().to_string();
        let page = format!("Quick Notes/Bulletnotes Acceptance/{nonce}.md");
        assert!(!browser_complete(dir.path(), &nonce, &page));
        for marker in [
            b"done".to_vec(),
            serde_json::to_vec(&serde_json::json!({"nonce":"old-run","page":page})).unwrap(),
            serde_json::to_vec(&serde_json::json!({"nonce":nonce,"page":"wrong.md"})).unwrap(),
            serde_json::to_vec(&serde_json::json!({"nonce":nonce})).unwrap(),
        ] {
            std::fs::write(dir.path().join("browser-done"), marker).unwrap();
            assert!(
                !browser_complete(dir.path(), &nonce, &page),
                "stale or wrong marker cannot advance the browser phase"
            );
        }
        std::fs::write(
            dir.path().join("browser-done"),
            serde_json::to_vec(&serde_json::json!({"nonce":nonce,"page":page})).unwrap(),
        )
        .unwrap();
        assert!(browser_complete(dir.path(), &nonce, &page));
    }
}

/// Resume guarded cleanup only for a retained supervised generated fixture.
pub fn cleanup_owned(dir: &Path) -> Result<()> {
    if !dir.file_name().is_some_and(|name| {
        name.to_string_lossy()
            .starts_with("bulletnotes-acceptance-")
    }) {
        return Err("not an owned cleanup fixture".into());
    }
    let nonce = std::fs::read_to_string(dir.join("marker"))?;
    if uuid::Uuid::parse_str(&nonce)?.get_version_num() != 4 {
        return Err("invalid owned nonce".into());
    }
    let journal = dir.join("owned-paths.json");
    let paths: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_slice(&std::fs::read(&journal)?)?;
    let config = ConnectionConfig::parse("https://notes.harflix.lol/")?;
    let peer = SilverBulletClient::new(&config, None)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let mut cleaned = 0;
    for (path, sources) in &paths {
        let main = path == &format!("Quick Notes/Bulletnotes Acceptance/{nonce}.md");
        let recovery = path
            .strip_prefix(&format!("Bulletnotes Recovery/{nonce}-"))
            .and_then(|value| value.strip_suffix(".md"))
            .is_some_and(|value| {
                uuid::Uuid::parse_str(value).is_ok_and(|id| id.get_version_num() == 4)
            });
        let probe = path
            .strip_prefix("Quick Notes/Bulletnotes Probe/")
            .and_then(|value| value.strip_suffix(".md"))
            .is_some_and(|value| {
                uuid::Uuid::parse_str(value).is_ok_and(|id| id.get_version_num() == 4)
            });
        if !main && !recovery && !probe {
            return Err("cleanup journal contains unowned destination".into());
        }
        let key = PageKey {
            connection: config.id().clone(),
            path: PagePath::parse(path)?,
        };
        if let RemoteRead::Present(file) = runtime.block_on(peer.read(&key))? {
            if !sources.contains(&file.body) {
                return Err("cleanup refused unexpected owned source".into());
            }
            let mut deletion = operation(
                &key,
                "",
                WriteCondition::Match(file.revision.ok_or("cleanup lacks strong validator")?),
            );
            deletion.kind = MutationKind::Delete;
            runtime.block_on(peer.delete(&deletion))?;
        }
        if !matches!(runtime.block_on(peer.read(&key))?, RemoteRead::Missing(_)) {
            return Err("cleanup absence unverified".into());
        }
        cleaned += 1;
    }
    std::fs::write(
        dir.join("cleanup-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"paths":cleaned,"guarded":true,"absence_confirmed":true}),
        )?,
    )?;
    println!("owned_guarded_cleanup_absent_paths: {cleaned}");
    Ok(())
}
