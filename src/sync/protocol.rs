use crate::domain::{connection::ConnectionId, error::SyncError, page::*};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone)]
pub struct SecretToken(HeaderValue);
impl SecretToken {
    pub fn new(value: String) -> Result<Self, SyncError> {
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(SyncError::Authentication);
        }
        let mut header = HeaderValue::from_str(&format!("Bearer {value}"))
            .map_err(|_| SyncError::Authentication)?;
        header.set_sensitive(true);
        Ok(Self(header))
    }
    #[cfg(target_os = "macos")]
    pub(crate) fn credential_bytes(&self) -> &[u8] {
        &self.0.as_bytes()["Bearer ".len()..]
    }
    pub(super) fn into_header(self) -> HeaderValue {
        self.0
    }
}
impl std::fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretToken([redacted])")
    }
}
pub(super) fn strong_revision(raw: &str) -> bool {
    raw.len() >= 2
        && raw.starts_with('"')
        && raw.ends_with('"')
        && raw.as_bytes()[1..raw.len() - 1]
            .iter()
            .all(|b| *b == 0x21 || (0x23..=0x7e).contains(b))
}
fn one_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}
pub(super) fn revision(headers: &HeaderMap) -> Option<RemoteRevision> {
    // A proxy may transform ETag. A present but invalid dedicated revision fails
    // closed; falling back would silently select a different revision namespace.
    let name = if headers.contains_key("x-silverbullet-revision") {
        "x-silverbullet-revision"
    } else {
        "etag"
    };
    one_header(headers, name)
        .filter(|value| strong_revision(value))
        .map(|value| RemoteRevision(value.into()))
}
pub(super) fn permission(headers: &HeaderMap, revision: &Option<RemoteRevision>) -> Permission {
    if one_header(headers, "x-permission") == Some("rw") && revision.is_some() {
        Permission::ReadWrite
    } else {
        Permission::ReadOnly
    }
}
pub(super) fn modified(headers: &HeaderMap) -> Result<Option<u64>, SyncError> {
    if !headers.contains_key("x-last-modified") {
        return Ok(None);
    }
    one_header(headers, "x-last-modified")
        .ok_or(SyncError::InvalidResponse)?
        .parse()
        .map(Some)
        .map_err(|_| SyncError::InvalidResponse)
}
pub(super) fn identity_encoding(headers: &HeaderMap) -> bool {
    !headers.contains_key("content-encoding")
        || one_header(headers, "content-encoding")
            .is_some_and(|value| value.eq_ignore_ascii_case("identity"))
}
pub(super) fn media_type(headers: &HeaderMap) -> Option<&str> {
    one_header(headers, "content-type")?
        .split(';')
        .next()
        .map(str::trim)
}
pub(super) fn media_is(headers: &HeaderMap, allowed: &[&str]) -> bool {
    media_type(headers).is_some_and(|value| {
        allowed
            .iter()
            .any(|allowed| value.eq_ignore_ascii_case(allowed))
    })
}
pub(super) fn available_path(path: &PagePath) -> bool {
    !path.as_str().split('/').any(|part| part.starts_with('.'))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedFile {
    name: String,
    last_modified: Option<u64>,
    size: Option<u64>,
    perm: Option<String>,
    etag: Option<String>,
}
pub(super) fn catalogue(
    bytes: &[u8],
    connection: &ConnectionId,
) -> Result<Vec<RemoteMetadata>, SyncError> {
    let entries: Vec<ListedFile> =
        serde_json::from_slice(bytes).map_err(|_| SyncError::InvalidResponse)?;
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .map(|entry| {
            let path = PagePath::parse(&entry.name).map_err(|_| SyncError::InvalidResponse)?;
            if !available_path(&path) || !seen.insert(path.clone()) {
                return Err(SyncError::InvalidResponse);
            }
            let revision = entry
                .etag
                .filter(|raw| strong_revision(raw))
                .map(RemoteRevision);
            let permission = if entry.perm.as_deref() == Some("rw") && revision.is_some() {
                Permission::ReadWrite
            } else {
                Permission::ReadOnly
            };
            Ok(RemoteMetadata {
                key: PageKey {
                    connection: connection.clone(),
                    path,
                },
                modified_ms: entry.last_modified,
                content_length: entry.size,
                permission,
                revision,
            })
        })
        .collect()
}
