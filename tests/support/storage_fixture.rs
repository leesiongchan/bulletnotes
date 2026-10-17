use bulletnotes::domain::{
    connection::ConnectionConfig,
    page::{PageKey, PagePath},
};
pub fn key(path: &str) -> PageKey {
    PageKey {
        connection: ConnectionConfig::parse("https://notes.example/a")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse(path).unwrap(),
    }
}
pub fn database(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("notes.sqlite")
}
