//! The only persistent credential boundary; OS calls never run on the UI thread.
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
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            Self::new(Arc::new(keyring::Backend::default()))
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Self::new(Arc::new(Keychain))
        }
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
#[cfg(not(any(target_os = "windows", target_os = "linux")))]
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
const SERVICE: &str = "lol.harflix.bulletnotes.silverbullet.bearer.v1";
#[cfg(target_os = "macos")]
fn classify(error: security_framework::base::Error) -> CredentialError {
    match error.code() {
        -25293 | -128 | -25308 => CredentialError::Denied, // auth failed, user canceled, interaction disallowed
        _ => CredentialError::Unavailable,
    }
}
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
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

#[cfg(any(target_os = "windows", target_os = "linux", test))]
mod keyring {
    use super::*;
    use keyring_core::{Entry, Error};

    #[derive(Default)]
    pub(super) struct Backend {
        store: Mutex<Option<Arc<keyring_core::CredentialStore>>>,
    }

    impl Backend {
        fn with_entry<T>(
            &self,
            id: &ConnectionId,
            operation: impl FnOnce(Entry) -> keyring_core::Result<T>,
        ) -> keyring_core::Result<T> {
            // Initialize off the UI thread and serialize access to each store instance.
            let mut store = self.store.lock().unwrap_or_else(|error| error.into_inner());
            let store = match store.as_ref() {
                Some(store) => store,
                None => store.insert(native_store()?),
            };
            #[cfg(target_os = "windows")]
            let modifiers = Some(std::collections::HashMap::from([("persistence", "Local")]));
            #[cfg(not(target_os = "windows"))]
            let modifiers = None;
            operation(store.build(SERVICE, id.as_str(), modifiers.as_ref())?)
        }
    }

    fn native_store() -> keyring_core::Result<Arc<keyring_core::CredentialStore>> {
        #[cfg(target_os = "windows")]
        {
            Ok(windows_native_keyring_store::Store::new()?)
        }
        #[cfg(target_os = "linux")]
        {
            Ok(zbus_secret_service_keyring_store::Store::new()?)
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Err(Error::NoDefaultStore)
        }
    }

    impl CredentialBackend for Backend {
        fn load(&self, id: &ConnectionId) -> Result<Option<SecretToken>, CredentialError> {
            match self.with_entry(id, |entry| entry.get_secret()) {
                Ok(bytes) => {
                    let value = String::from_utf8(bytes).map_err(|_| CredentialError::Invalid)?;
                    SecretToken::new(value)
                        .map(Some)
                        .map_err(|_| CredentialError::Invalid)
                }
                Err(Error::NoEntry) => Ok(None),
                Err(error) => Err(classify(error)),
            }
        }
        fn save(&self, id: &ConnectionId, token: &SecretToken) -> Result<(), CredentialError> {
            self.with_entry(id, |entry| entry.set_secret(token.credential_bytes()))
                .map_err(classify)
        }
        fn delete(&self, id: &ConnectionId) -> Result<(), CredentialError> {
            match self.with_entry(id, |entry| entry.delete_credential()) {
                Ok(()) | Err(Error::NoEntry) => Ok(()),
                Err(error) => Err(classify(error)),
            }
        }
    }

    fn classify(error: Error) -> CredentialError {
        let classified = match error {
            Error::NoStorageAccess(_) => CredentialError::Denied,
            Error::BadEncoding(_)
            | Error::BadDataFormat(_, _)
            | Error::Invalid(_, _)
            | Error::TooLong(_, _) => CredentialError::Invalid,
            _ => CredentialError::Unavailable,
        };
        tracing::warn!(error = ?classified, "secure credential storage operation failed");
        classified
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::connection::ConnectionConfig;
        use keyring_core::{Error, api::CredentialStoreApi, mock};
        use std::collections::HashMap;

        struct TestStore(Arc<mock::Store>);
        impl CredentialStoreApi for TestStore {
            fn vendor(&self) -> String {
                self.0.vendor()
            }
            fn id(&self) -> String {
                self.0.id()
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn build(
                &self,
                service: &str,
                user: &str,
                modifiers: Option<&HashMap<&str, &str>>,
            ) -> keyring_core::Result<keyring_core::Entry> {
                #[cfg(target_os = "windows")]
                assert_eq!(modifiers, Some(&HashMap::from([("persistence", "Local")])));
                #[cfg(not(target_os = "windows"))]
                assert_eq!(modifiers, None);
                self.0.build(service, user, None)
            }
        }

        fn fixture() -> (Arc<Backend>, keyring_core::Entry, ConnectionId) {
            let id = ConnectionConfig::parse("https://credentials.example/space/")
                .unwrap()
                .id()
                .clone();
            let store = mock::Store::new().unwrap();
            let entry = store
                .build(
                    "lol.harflix.bulletnotes.silverbullet.bearer.v1",
                    id.as_str(),
                    None,
                )
                .unwrap();
            let backend = Arc::new(Backend {
                store: Mutex::new(Some(Arc::new(TestStore(store)))),
            });
            (backend, entry, id)
        }

        fn inject(entry: &keyring_core::Entry, error: Error) {
            entry
                .as_any()
                .downcast_ref::<mock::Cred>()
                .unwrap()
                .set_error(error);
        }

        #[test]
        fn missing_credential_loads_as_none() {
            let (backend, _, id) = fixture();
            assert!(backend.load(&id).unwrap().is_none());
        }

        #[test]
        fn deleting_a_missing_credential_succeeds() {
            let (backend, _, id) = fixture();
            assert_eq!(backend.delete(&id), Ok(()));
        }

        #[test]
        fn saves_raw_token_under_existing_service_and_account() {
            let (backend, entry, id) = fixture();
            backend
                .save(
                    &id,
                    &SecretToken::new("generated-sensitive-token".into()).unwrap(),
                )
                .unwrap();
            assert_eq!(entry.get_secret().unwrap(), b"generated-sensitive-token");
        }

        #[test]
        fn loaded_token_stays_redacted() {
            let (backend, entry, id) = fixture();
            entry.set_secret(b"generated-sensitive-token").unwrap();
            let token = backend.load(&id).unwrap().unwrap();
            assert_eq!(format!("{token:?}"), "SecretToken([redacted])");
        }

        #[test]
        fn malformed_saved_secrets_are_invalid() {
            for bytes in [
                &b""[..],
                &b"token\r\nheader"[..],
                &b"\xff"[..],
                "nonascii-\u{e9}".as_bytes(),
            ] {
                let (backend, entry, id) = fixture();
                entry.set_secret(bytes).unwrap();
                assert!(matches!(backend.load(&id), Err(CredentialError::Invalid)));
            }
        }

        #[test]
        fn store_errors_are_redacted_and_never_become_missing_credentials() {
            let cases = [
                (
                    Error::NoStorageAccess(
                        std::io::Error::from(std::io::ErrorKind::PermissionDenied).into(),
                    ),
                    CredentialError::Denied,
                ),
                (
                    Error::PlatformFailure(
                        std::io::Error::other("sensitive-platform-detail").into(),
                    ),
                    CredentialError::Unavailable,
                ),
                (
                    Error::BadEncoding(b"sensitive-secret".to_vec()),
                    CredentialError::Invalid,
                ),
                (Error::Ambiguous(Vec::new()), CredentialError::Unavailable),
            ];
            for (error, expected) in cases {
                let (backend, entry, id) = fixture();
                inject(&entry, error);
                let actual = backend.load(&id).unwrap_err();
                assert_eq!(actual, expected);
                assert!(!format!("{actual:?}").contains("sensitive"));
            }
        }

        #[tokio::test]
        async fn denied_load_is_cached_until_successful_save() {
            let (backend, entry, id) = fixture();
            inject(
                &entry,
                Error::NoStorageAccess(
                    std::io::Error::from(std::io::ErrorKind::PermissionDenied).into(),
                ),
            );
            let credentials = CredentialStore::new(backend);
            assert!(matches!(
                credentials.load(&id).await,
                Err(CredentialError::Denied)
            ));
            assert!(matches!(
                credentials.load(&id).await,
                Err(CredentialError::Denied)
            ));
            credentials
                .save(&id, &SecretToken::new("new-token".into()).unwrap())
                .await
                .unwrap();
            assert!(credentials.load(&id).await.unwrap().is_some());
        }

        #[test]
        fn failed_save_does_not_persist_a_secret_elsewhere() {
            let (backend, entry, id) = fixture();
            inject(
                &entry,
                Error::NoStorageAccess(
                    std::io::Error::from(std::io::ErrorKind::PermissionDenied).into(),
                ),
            );
            assert_eq!(
                backend.save(&id, &SecretToken::new("new-token".into()).unwrap()),
                Err(CredentialError::Denied)
            );
            assert!(matches!(entry.get_secret(), Err(Error::NoEntry)));
        }

        #[test]
        fn failed_delete_retains_the_saved_secret_and_reports_unavailable() {
            let (backend, entry, id) = fixture();
            entry.set_secret(b"existing-token").unwrap();
            inject(
                &entry,
                Error::PlatformFailure(std::io::Error::other("sensitive-platform-detail").into()),
            );
            assert_eq!(backend.delete(&id), Err(CredentialError::Unavailable));
            assert_eq!(entry.get_secret().unwrap(), b"existing-token");
        }

        #[tokio::test]
        async fn successful_delete_clears_cached_denial() {
            let (backend, entry, id) = fixture();
            inject(
                &entry,
                Error::NoStorageAccess(
                    std::io::Error::from(std::io::ErrorKind::PermissionDenied).into(),
                ),
            );
            let credentials = CredentialStore::new(backend);
            assert!(matches!(
                credentials.load(&id).await,
                Err(CredentialError::Denied)
            ));
            credentials.delete(&id).await.unwrap();
            assert!(credentials.load(&id).await.unwrap().is_none());
        }
    }
}
