//! Connection setup never treats readable access as proof of safe mutation.
use crate::{
    domain::{
        connection::ConnectionConfig,
        error::{AppError, CredentialError, SyncError},
    },
    platform::credentials::CredentialStore,
    store::StoreHandle,
    sync::{
        SecretToken, SilverBulletClient,
        probe::{self, CompatibilityEvidence},
    },
};
use std::sync::Arc;
use tokio::sync::Mutex;
#[derive(Clone)]
pub struct ConnectionController {
    store: StoreHandle,
    credentials: CredentialStore,
    serial: Arc<Mutex<()>>,
}
#[derive(Clone)]
pub struct ConnectionOutcome {
    pub client: SilverBulletClient,
    pub readable: bool,
    pub write_capability: Result<CompatibilityEvidence, SyncError>,
    pub credential_warning: Option<CredentialError>,
    pub cleanup_pending: bool,
}
impl ConnectionController {
    pub fn new(store: StoreHandle, credentials: CredentialStore) -> Self {
        Self {
            store,
            credentials,
            serial: Arc::new(Mutex::new(())),
        }
    }
    pub async fn connect(
        &self,
        config: ConnectionConfig,
        token: Option<SecretToken>,
        remember: bool,
    ) -> Result<ConnectionOutcome, AppError> {
        let _serial = self.serial.lock().await;
        let mut warning = None;
        let supplied = token.is_some();
        let token = match token {
            Some(token) => Some(token),
            None => match self.credentials.load(config.id()).await {
                Ok(token) => token,
                Err(error) => {
                    warning = Some(error);
                    None
                }
            },
        };
        let client = SilverBulletClient::new(&config, token.clone())?;
        self.finish_connect(client, token, remember, supplied, warning)
            .await
    }
    #[cfg(feature = "protocol-validation")]
    pub async fn connect_fixture(
        &self,
        client: SilverBulletClient,
        token: Option<SecretToken>,
        remember: bool,
    ) -> Result<ConnectionOutcome, AppError> {
        let _serial = self.serial.lock().await;
        let supplied = token.is_some();
        self.finish_connect(client, token, remember, supplied, None)
            .await
    }
    async fn finish_connect(
        &self,
        client: SilverBulletClient,
        token: Option<SecretToken>,
        remember: bool,
        supplied: bool,
        mut warning: Option<CredentialError>,
    ) -> Result<ConnectionOutcome, AppError> {
        client.list_files().await?;
        if let Some(token) = token.as_ref()
            && remember
        {
            if let Err(error) = self.credentials.save(client.connection_id(), token).await {
                warning = Some(error);
            }
        } else if supplied
            && !remember
            && let Err(error) = self.credentials.delete(client.connection_id()).await
        {
            warning = Some(error);
        }
        // Do not create more probes while an old owned page cannot be reconciled.
        if let Err(error) = self.cleanup_pending_inner(&client).await {
            let error = match error {
                AppError::Sync(error) => error,
                error => return Err(error),
            };
            return Ok(ConnectionOutcome {
                client,
                readable: true,
                write_capability: Err(error),
                credential_warning: warning,
                cleanup_pending: true,
            });
        }
        let key = probe::new_probe_key(client.connection_id().clone());
        self.store.register_probe(&key).await?;
        let write_capability = probe::verify_write_guards(&client, key.clone()).await;
        if write_capability.is_ok() {
            self.store.forget_probe(&key).await?;
        }
        Ok(ConnectionOutcome {
            client,
            readable: true,
            cleanup_pending: write_capability.is_err(),
            write_capability,
            credential_warning: warning,
        })
    }
    pub async fn cleanup_pending(&self, client: &SilverBulletClient) -> Result<(), AppError> {
        let _serial = self.serial.lock().await;
        self.cleanup_pending_inner(client).await
    }
    async fn cleanup_pending_inner(&self, client: &SilverBulletClient) -> Result<(), AppError> {
        for key in self.store.pending_probes(client.connection_id()).await? {
            probe::cleanup_probe(client, &key).await?;
            self.store.forget_probe(&key).await?;
        }
        Ok(())
    }
}

/// Validated form input. Debug redacts the optional credential through SecretToken.
#[derive(Debug)]
pub struct ConnectionInput {
    pub config: ConnectionConfig,
    pub token: Option<SecretToken>,
    pub remember: bool,
}
impl ConnectionInput {
    pub fn parse(url: &str, token: &str, remember: bool) -> Result<Self, AppError> {
        let config = ConnectionConfig::parse(url)?;
        let token = if token.is_empty() {
            None
        } else {
            Some(SecretToken::new(token.to_owned())?)
        };
        Ok(Self {
            config,
            token,
            remember,
        })
    }
}
