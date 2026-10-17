use super::protocol::{self, SecretToken};
use crate::domain::{
    connection::{ConnectionConfig, ConnectionId},
    error::SyncError,
    page::*,
};
use reqwest::{
    Client, Method, Response, StatusCode,
    header::{self, HeaderMap, HeaderValue},
};
use std::time::Duration;
use url::Url;

const MAX_PAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CATALOGUE_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone)]
pub struct SilverBulletClient {
    client: Client,
    base: Url,
    connection: ConnectionId,
    instance: uuid::Uuid,
}
impl SilverBulletClient {
    pub fn new(config: &ConnectionConfig, token: Option<SecretToken>) -> Result<Self, SyncError> {
        Self::build(
            config,
            config.base_url().clone(),
            token,
            Duration::from_secs(30),
            true,
        )
    }
    fn build(
        config: &ConnectionConfig,
        base: Url,
        token: Option<SecretToken>,
        timeout: Duration,
        secure: bool,
    ) -> Result<Self, SyncError> {
        let mut headers = HeaderMap::new();
        headers.insert("x-sync-mode", HeaderValue::from_static("true"));
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_static("identity"),
        );
        if let Some(token) = token {
            headers.insert(header::AUTHORIZATION, token.into_header());
        }
        let client = Client::builder()
            .https_only(secure)
            .connect_timeout(Duration::from_secs(10))
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .referer(false)
            .default_headers(headers)
            .user_agent(concat!("Bulletnotes/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| SyncError::Network)?;
        Ok(Self {
            client,
            base,
            connection: config.id().clone(),
            instance: uuid::Uuid::new_v4(),
        })
    }
    /// Explicitly gated loopback transport for controlled integration fixtures.
    #[cfg(feature = "protocol-validation")]
    pub fn for_loopback_fixture(
        config: &ConnectionConfig,
        base: Url,
        token: Option<SecretToken>,
        timeout: Duration,
    ) -> Result<Self, SyncError> {
        if base.scheme() != "http"
            || !matches!(base.host(),Some(url::Host::Ipv4(ip)) if ip.is_loopback())
                && !matches!(base.host(),Some(url::Host::Ipv6(ip)) if ip.is_loopback())
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err(SyncError::Incompatible);
        }
        Self::build(config, base, token, timeout, false)
    }
    pub fn connection_id(&self) -> &ConnectionId {
        &self.connection
    }
    pub(crate) fn instance_id(&self) -> uuid::Uuid {
        self.instance
    }
    fn endpoint(&self, key: Option<&PageKey>) -> Result<Url, SyncError> {
        if key.is_some_and(|key| {
            key.connection != self.connection || !protocol::available_path(&key.path)
        }) {
            return Err(SyncError::Incompatible);
        }
        let mut url = self.base.clone();
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| SyncError::Incompatible)?;
        segments.pop_if_empty().push(".fs");
        if let Some(key) = key {
            segments.extend(key.path.as_str().split('/'));
        }
        drop(segments);
        Ok(url)
    }
    pub async fn list_files(&self) -> Result<Vec<RemoteMetadata>, SyncError> {
        let response = self
            .client
            .get(self.endpoint(None)?)
            .header(header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| SyncError::Network)?;
        classify(response.status(), false)?;
        if !protocol::media_is(response.headers(), &["application/json"]) {
            return Err(SyncError::InvalidResponse);
        }
        protocol::catalogue(
            &read_bytes(response, MAX_CATALOGUE_BYTES).await?,
            &self.connection,
        )
    }
    /// Check current revision and permission without downloading an unchanged body.
    pub async fn read_if_changed(&self, known: &RemoteFile) -> Result<RemoteRead, SyncError> {
        let response = self
            .client
            .head(self.endpoint(Some(&known.key))?)
            .send()
            .await
            .map_err(|_| SyncError::Network)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(RemoteRead::Missing(known.key.clone()));
        }
        if response.status() == StatusCode::METHOD_NOT_ALLOWED {
            return self.read(&known.key).await;
        }
        if response.status() == StatusCode::NOT_MODIFIED {
            return Err(SyncError::InvalidResponse);
        }
        classify(response.status(), false)?;
        if !protocol::media_is(
            response.headers(),
            &["text/markdown", "text/plain", "application/octet-stream"],
        ) {
            return Err(SyncError::InvalidResponse);
        }
        let revision = protocol::revision(response.headers());
        if revision.is_some() && revision == known.revision {
            let mut file = known.clone();
            file.permission = protocol::permission(response.headers(), &revision);
            file.modified_ms = protocol::modified(response.headers())?;
            return Ok(RemoteRead::Present(file));
        }
        self.read(&known.key).await
    }
    pub async fn read(&self, key: &PageKey) -> Result<RemoteRead, SyncError> {
        let response = self
            .client
            .get(self.endpoint(Some(key))?)
            .send()
            .await
            .map_err(|_| SyncError::Network)?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Err(SyncError::InvalidResponse);
        }
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(RemoteRead::Missing(key.clone()));
        }
        classify(response.status(), false)?;
        if !protocol::media_is(
            response.headers(),
            &["text/markdown", "text/plain", "application/octet-stream"],
        ) {
            return Err(SyncError::InvalidResponse);
        }
        let revision = protocol::revision(response.headers());
        let permission = protocol::permission(response.headers(), &revision);
        let modified_ms = protocol::modified(response.headers())?;
        let body = String::from_utf8(read_bytes(response, MAX_PAGE_BYTES).await?)
            .map_err(|_| SyncError::InvalidResponse)?;
        Ok(RemoteRead::Present(RemoteFile {
            key: key.clone(),
            body,
            revision,
            permission,
            modified_ms,
        }))
    }
    pub async fn put(&self, operation: &PendingMutation) -> Result<MutationAck, SyncError> {
        let MutationKind::Put(body) = &operation.kind else {
            return Err(SyncError::Incompatible);
        };
        let revision = self
            .mutate(Method::PUT, operation, Some(body))
            .await?
            .ok_or(SyncError::UnknownMutationOutcome)?;
        Ok(MutationAck {
            remote_revision: Some(revision),
        })
    }
    pub async fn delete(&self, operation: &PendingMutation) -> Result<MutationAck, SyncError> {
        if operation.kind != MutationKind::Delete
            || !matches!(operation.condition, WriteCondition::Match(_))
        {
            return Err(SyncError::Incompatible);
        }
        self.mutate(Method::DELETE, operation, None).await?;
        Ok(MutationAck {
            remote_revision: None,
        })
    }
    async fn mutate(
        &self,
        method: Method,
        operation: &PendingMutation,
        body: Option<&str>,
    ) -> Result<Option<RemoteRevision>, SyncError> {
        let mut request = self
            .client
            .request(method, self.endpoint(Some(&operation.key))?);
        request = match &operation.condition {
            WriteCondition::CreateOnly => request.header(header::IF_NONE_MATCH, "*"),
            WriteCondition::Match(revision) if protocol::strong_revision(&revision.0) => {
                request.header(header::IF_MATCH, &revision.0)
            }
            _ => return Err(SyncError::Incompatible),
        };
        if let Some(body) = body {
            request = request
                .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
                .body(body.to_owned());
        }
        let response = request
            .send()
            .await
            .map_err(|_| SyncError::UnknownMutationOutcome)?;
        classify(response.status(), true)?;
        if response.headers().contains_key(header::CONTENT_TYPE)
            && !protocol::media_is(
                response.headers(),
                &["text/plain", "text/markdown", "application/octet-stream"],
            )
        {
            return Err(SyncError::UnknownMutationOutcome);
        }
        let revision = protocol::revision(response.headers());
        let status = response.status();
        let acknowledgement = read_bytes(response, 64)
            .await
            .map_err(|_| SyncError::UnknownMutationOutcome)?;
        if acknowledgement != b"OK"
            && !(status == StatusCode::NO_CONTENT && acknowledgement.is_empty())
        {
            return Err(SyncError::UnknownMutationOutcome);
        }
        Ok(revision)
    }
}
fn classify(status: StatusCode, mutation: bool) -> Result<(), SyncError> {
    match status.as_u16() {
        200 | 201 | 204 => Ok(()),
        401 => Err(SyncError::Authentication),
        403 => Err(SyncError::Permission),
        404 => Err(SyncError::Missing),
        409 | 412 => Err(SyncError::Conflict),
        300..=399 => Err(SyncError::Incompatible),
        400 | 405 | 413 | 415 | 422 => Err(SyncError::Incompatible),
        _ if mutation => Err(SyncError::UnknownMutationOutcome),
        _ => Err(SyncError::Network),
    }
}
async fn read_bytes(mut response: Response, limit: usize) -> Result<Vec<u8>, SyncError> {
    if !protocol::identity_encoding(response.headers()) {
        return Err(SyncError::InvalidResponse);
    }
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err(SyncError::ResponseTooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| SyncError::Network)? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(SyncError::ResponseTooLarge);
        }
        bytes
            .try_reserve(chunk.len())
            .map_err(|_| SyncError::ResponseTooLarge)?;
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
