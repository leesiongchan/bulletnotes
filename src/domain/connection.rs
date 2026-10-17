//! Credential-free identity for one SilverBullet space.
use super::error::ConnectionError;
use url::Url;

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct ConnectionId(String);
impl ConnectionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectionConfig {
    base: Url,
    id: ConnectionId,
}
impl ConnectionConfig {
    pub fn parse(input: &str) -> Result<Self, ConnectionError> {
        // The URL parser repairs whitespace/backslashes; refuse such ambiguous input.
        if input.trim() != input || input.chars().any(|c| c.is_control() || c == '\\') {
            return Err(ConnectionError::InvalidUrl);
        }
        let mut base = Url::parse(input).map_err(|_| ConnectionError::InvalidUrl)?;
        if base.scheme() != "https" || !base.has_host() || base.cannot_be_a_base() {
            return Err(ConnectionError::HttpsRequired);
        }
        if !base.username().is_empty() || base.password().is_some() {
            return Err(ConnectionError::CredentialsInUrl);
        }
        if base.query().is_some() || base.fragment().is_some() {
            return Err(ConnectionError::QueryOrFragment);
        }
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        let id = ConnectionId(base.as_str().to_owned());
        Ok(Self { base, id })
    }
    pub fn id(&self) -> &ConnectionId {
        &self.id
    }
    pub fn base_url(&self) -> &Url {
        &self.base
    }
}

impl TryFrom<String> for ConnectionId {
    type Error = ConnectionError;
    fn try_from(input: String) -> Result<Self, Self::Error> {
        Ok(ConnectionConfig::parse(&input)?.id)
    }
}
impl From<ConnectionId> for String {
    fn from(id: ConnectionId) -> Self {
        id.0
    }
}
