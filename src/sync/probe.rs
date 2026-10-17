//! Connection-scoped evidence obtained exclusively through an owned temporary page.
use super::SilverBulletClient;
use crate::domain::{connection::ConnectionId, error::SyncError, page::*};
use std::time::SystemTime;
use uuid::Uuid;
const PREFIX: &str = "Quick Notes/Bulletnotes Probe/";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardCheck {
    MatchingUpdate,
    StaleWriteRejected,
    StaleDeleteRejected,
    CreateCollisionRejected,
    ExactReadback,
    GuardedCleanup,
}
#[derive(Clone, Debug)]
pub struct CompatibilityEvidence {
    connection: ConnectionId,
    instance: Uuid,
    verified_at: SystemTime,
    checks: [GuardCheck; 6],
}
impl CompatibilityEvidence {
    pub fn permits(&self, client: &SilverBulletClient) -> bool {
        self.connection == *client.connection_id() && self.instance == client.instance_id()
    }
    pub fn connection(&self) -> &ConnectionId {
        &self.connection
    }
    pub fn verified_at(&self) -> SystemTime {
        self.verified_at
    }
    pub fn checks(&self) -> &[GuardCheck] {
        &self.checks
    }
}
pub fn new_probe_key(connection: ConnectionId) -> PageKey {
    PageKey {
        connection,
        path: PagePath::parse(&format!("{PREFIX}{}.md", Uuid::new_v4()))
            .expect("generated probe path"),
    }
}
pub(crate) fn probe_id(key: &PageKey) -> Result<Uuid, SyncError> {
    let raw = key
        .path
        .as_str()
        .strip_prefix(PREFIX)
        .and_then(|s| s.strip_suffix(".md"))
        .ok_or(SyncError::Incompatible)?;
    let id = Uuid::parse_str(raw).map_err(|_| SyncError::Incompatible)?;
    if id.get_version_num() != 4 || id.to_string() != raw {
        return Err(SyncError::Incompatible);
    }
    Ok(id)
}
fn source(id: Uuid, stage: char) -> String {
    format!("Bulletnotes temporary compatibility probe\n{id}\n{stage}\n")
}
fn operation(key: &PageKey, body: String, condition: WriteCondition) -> PendingMutation {
    PendingMutation {
        id: OperationId::new(),
        key: key.clone(),
        local_revision: LocalRevision(0),
        kind: MutationKind::Put(body),
        condition,
    }
}
fn conflict(result: Result<MutationAck, SyncError>) -> Result<(), SyncError> {
    match result {
        Err(SyncError::Conflict) => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(SyncError::Incompatible),
    }
}
pub async fn verify_write_guards(
    client: &SilverBulletClient,
    key: PageKey,
) -> Result<CompatibilityEvidence, SyncError> {
    let id = probe_id(&key)?;
    if &key.connection != client.connection_id() {
        return Err(SyncError::Incompatible);
    }
    let a = client
        .put(&operation(
            &key,
            source(id, 'A'),
            WriteCondition::CreateOnly,
        ))
        .await?
        .remote_revision
        .ok_or(SyncError::UnknownMutationOutcome)?;
    let b = client
        .put(&operation(
            &key,
            source(id, 'B'),
            WriteCondition::Match(a.clone()),
        ))
        .await?
        .remote_revision
        .ok_or(SyncError::UnknownMutationOutcome)?;
    if a == b {
        return Err(SyncError::Incompatible);
    }
    conflict(
        client
            .put(&operation(
                &key,
                source(id, 'C'),
                WriteCondition::Match(a.clone()),
            ))
            .await,
    )?;
    let mut stale_delete = operation(&key, String::new(), WriteCondition::Match(a));
    stale_delete.kind = MutationKind::Delete;
    conflict(client.delete(&stale_delete).await)?;
    conflict(
        client
            .put(&operation(
                &key,
                source(id, 'C'),
                WriteCondition::CreateOnly,
            ))
            .await,
    )?;
    match client.read(&key).await? {
        RemoteRead::Present(file)
            if file.body == source(id, 'B')
                && file.revision.as_ref() == Some(&b)
                && file.permission == Permission::ReadWrite => {}
        _ => return Err(SyncError::Incompatible),
    }
    let mut delete = operation(&key, String::new(), WriteCondition::Match(b));
    delete.kind = MutationKind::Delete;
    client.delete(&delete).await?;
    if !matches!(client.read(&key).await?, RemoteRead::Missing(_)) {
        return Err(SyncError::Incompatible);
    }
    Ok(CompatibilityEvidence {
        connection: key.connection,
        instance: client.instance_id(),
        verified_at: SystemTime::now(),
        checks: [
            GuardCheck::MatchingUpdate,
            GuardCheck::StaleWriteRejected,
            GuardCheck::StaleDeleteRejected,
            GuardCheck::CreateCollisionRejected,
            GuardCheck::ExactReadback,
            GuardCheck::GuardedCleanup,
        ],
    })
}
/// The caller retains the journal entry unless absence is confirmed. Replacement
/// content is left intact even when it occupies an old generated probe path.
pub async fn cleanup_probe(client: &SilverBulletClient, key: &PageKey) -> Result<(), SyncError> {
    let id = probe_id(key)?;
    match client.read(key).await? {
        RemoteRead::Missing(_) => return Ok(()),
        RemoteRead::Present(file) => {
            if !['A', 'B', 'C']
                .iter()
                .any(|stage| file.body == source(id, *stage))
            {
                return Err(SyncError::Conflict);
            }
            let revision = file.revision.ok_or(SyncError::Incompatible)?;
            let mut delete = operation(key, String::new(), WriteCondition::Match(revision));
            delete.kind = MutationKind::Delete;
            client.delete(&delete).await?;
        }
    }
    if matches!(client.read(key).await?, RemoteRead::Missing(_)) {
        Ok(())
    } else {
        Err(SyncError::Conflict)
    }
}
