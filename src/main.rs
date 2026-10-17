#[cfg(target_os = "macos")]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use bulletnotes::{
        app::{native_app::LocalApplication, startup},
        domain::connection::ConnectionConfig,
        platform::application::{NativeApplication, database_path},
    };
    let native = NativeApplication::prepare()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let connection = ConnectionConfig::parse("https://notes.harflix.lol")?
        .id()
        .clone();
    let startup = runtime.block_on(startup::open(&database_path()?, connection))?;
    let app = LocalApplication::new(startup, native)?;
    app.run()
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
