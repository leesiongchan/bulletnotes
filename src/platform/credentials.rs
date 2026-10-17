//! The only persistent credential boundary; Keychain calls never run on the UI thread.
use crate::{
    domain::{connection::ConnectionId, error::CredentialError},
    sync::SecretToken,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

/// Synchronous OS boundary. Implementations must never log or persist secrets elsewhere.
pub trait CredentialBackend: Send + Sync {
    fn load(&self, connection: &ConnectionId) -> Result<Option<SecretToken>, CredentialError>;
    fn save(&self, connection: &ConnectionId, token: &SecretToken) -> Result<(), CredentialError>;
    fn delete(&self, connection: &ConnectionId) -> Result<(), CredentialError>;
}
#[derive(Clone)]
pub struct CredentialStore(Arc<CredentialState>);
struct CredentialState {
    backend: Arc<dyn CredentialBackend>,
    denied: Mutex<HashSet<ConnectionId>>,
}
impl CredentialStore {
    pub fn new(backend: Arc<dyn CredentialBackend>) -> Self {
        Self(Arc::new(CredentialState {
            backend,
            denied: Mutex::new(HashSet::new()),
        }))
    }
    pub fn keychain() -> Self {
        Self::new(Arc::new(Keychain))
    }
    pub async fn load(&self, id: &ConnectionId) -> Result<Option<SecretToken>, CredentialError> {
        if self
            .0
            .denied
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
        {
            return Err(CredentialError::Denied);
        }
        let backend = self.0.backend.clone();
        let account = id.clone();
        let result = tokio::task::spawn_blocking(move || backend.load(&account))
            .await
            .map_err(|_| CredentialError::Unavailable)?;
        if matches!(&result, Err(CredentialError::Denied)) {
            self.0
                .denied
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(id.clone());
        }
        result
    }
    pub async fn save(
        &self,
        id: &ConnectionId,
        token: &SecretToken,
    ) -> Result<(), CredentialError> {
        let backend = self.0.backend.clone();
        let account = id.clone();
        let token = token.clone();
        let result = tokio::task::spawn_blocking(move || backend.save(&account, &token))
            .await
            .map_err(|_| CredentialError::Unavailable)?;
        if result.is_ok() {
            self.0
                .denied
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(id);
        }
        result
    }
    pub async fn delete(&self, id: &ConnectionId) -> Result<(), CredentialError> {
        let backend = self.0.backend.clone();
        let account = id.clone();
        let result = tokio::task::spawn_blocking(move || backend.delete(&account))
            .await
            .map_err(|_| CredentialError::Unavailable)?;
        if result.is_ok() {
            self.0
                .denied
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(id);
        }
        result
    }
}
struct Keychain;
#[cfg(target_os = "macos")]
impl CredentialBackend for Keychain {
    fn load(&self, id: &ConnectionId) -> Result<Option<SecretToken>, CredentialError> {
        use security_framework::passwords::{PasswordOptions, generic_password};
        match generic_password(PasswordOptions::new_generic_password(SERVICE, id.as_str())) {
            Ok(bytes) => {
                let value = String::from_utf8(bytes).map_err(|_| CredentialError::Invalid)?;
                SecretToken::new(value)
                    .map(Some)
                    .map_err(|_| CredentialError::Invalid)
            }
            Err(error) if error.code() == -25300 => Ok(None), // errSecItemNotFound
            Err(error) => Err(classify(error)),
        }
    }
    fn save(&self, id: &ConnectionId, token: &SecretToken) -> Result<(), CredentialError> {
        security_framework::passwords::set_generic_password(
            SERVICE,
            id.as_str(),
            token.credential_bytes(),
        )
        .map_err(classify)
    }
    fn delete(&self, id: &ConnectionId) -> Result<(), CredentialError> {
        match security_framework::passwords::delete_generic_password(SERVICE, id.as_str()) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == -25300 => Ok(()),
            Err(error) => Err(classify(error)),
        }
    }
}
#[cfg(target_os = "macos")]
const SERVICE: &str = "lol.harflix.bulletnotes.silverbullet.bearer.v1";
#[cfg(target_os = "macos")]
fn classify(error: security_framework::base::Error) -> CredentialError {
    match error.code() {
        -25293 | -128 | -25308 => CredentialError::Denied, // auth failed, user canceled, interaction disallowed
        _ => CredentialError::Unavailable,
    }
}
#[cfg(not(target_os = "macos"))]
impl CredentialBackend for Keychain {
    fn load(&self, _: &ConnectionId) -> Result<Option<SecretToken>, CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn save(&self, _: &ConnectionId, _: &SecretToken) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn delete(&self, _: &ConnectionId) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
}
