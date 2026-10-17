#[cfg(target_os = "macos")]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use bulletnotes::{
        app::{
            connection::ConnectionController, connection_ui, native_app::LocalApplication, startup,
            sync_ui,
        },
        domain::connection::ConnectionConfig,
        platform::application::{NativeApplication, database_path},
        platform::credentials::CredentialStore,
    };
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let connection = ConnectionConfig::parse("https://notes.harflix.lol")?
        .id()
        .clone();
    let startup = runtime.block_on(startup::open(&database_path()?, connection))?;
    let controller = ConnectionController::new(startup.store.clone(), CredentialStore::keychain());
    let connection = startup.connection.clone();
    let store = startup.store.clone();
    let app = LocalApplication::new(startup, native)?;
    let connection_ui = connection_ui::bind(
        app.window(),
        controller,
        &connection,
        runtime.handle().clone(),
    );
    let _sync_ui = sync_ui::bind(
        app.window(),
        app.local().clone(),
        store,
        runtime.handle().clone(),
        &connection_ui,
    )?;
    use slint::ComponentHandle;
    let weak = app.window().as_weak();
    app.window().on_local_startup_ready(move || {
        if let Some(window) = weak.upgrade() {
            window.invoke_test_connection();
        }
    });
    let result = app.run();
    connection_ui.stop();
    // Normal quit has already drained local storage and cancelled network work.
    // OS credential/DNS jobs may be non-cancellable blocking tasks.
    runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    result
}
#[cfg(target_os = "macos")]
fn main() {
    if let Err(error) = run() {
        let message =
            format!("Bulletnotes could not start. {error}. Your local data has been preserved.");
        eprintln!("{message}");
        rfd::MessageDialog::new()
            .set_title("Bulletnotes could not start")
            .set_description(message)
            .set_level(rfd::MessageLevel::Error)
            .show();
        std::process::exit(1);
    }
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Bulletnotes currently requires macOS.");
}
