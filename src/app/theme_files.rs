//! File I/O runs on a blocking worker after a native dialog selects a path.
use crate::theme::{
    Theme, ThemeError,
    format::{MAX_THEME_BYTES, decode_theme, encode_theme},
};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
#[derive(Debug, thiserror::Error)]
pub enum ThemeFileError {
    #[error("theme file could not be read or written")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Theme(#[from] ThemeError),
}
pub fn load_theme_file(path: &Path) -> Result<Theme, ThemeFileError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((MAX_THEME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(decode_theme(&bytes)?)
}
/// The caller uses the native Save dialog to confirm the selected destination.
pub fn save_theme_file(path: &Path, theme: &Theme) -> Result<(), ThemeFileError> {
    let bytes = encode_theme(theme)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
