//! Immutable first-create attempts are separate from the coalescing ordinary outbox.
use super::{StoreHandle, outbox, pages, remote, restoration};
use crate::domain::{connection::ConnectionId, error::StoreError, page::*, reconciliation::*};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

impl StoreHandle {
    /// Persist preservation obligations only; canonical replacement composes `register` in its transaction.
    pub async fn register_automatic_recovery(
        &self,
        input: AutomaticRecoveryInput,
    ) -> Result<AutomaticRecoveryEvent, StoreError> {
        let event = self
            .call(move |w| {
                let tx = w.db.transaction()?;
                let event = register(&tx, input)?;
                tx.commit()?;
                Ok(event)
            })
            .await?;
        self.local_changes.send_replace(());
        Ok(event)
    }
    pub async fn automatic_recovery_event(
        &self,
        event_id: Uuid,
    ) -> Result<AutomaticRecoveryEvent, StoreError> {
        self.call(move |w| event(&w.db, event_id)).await
    }
    /// Includes retired unknown attempts so late writes remain discoverable after relocation.
    pub async fn recovery_uploads(
        &self,
        connection: &ConnectionId,
        limit: usize,
    ) -> Result<Vec<RecoveryUploadHead>, StoreError> {
        let connection = connection.clone();
        self.call(move |w| heads(&w.db, &connection, limit)).await
    }
    pub async fn claim_recovery_upload(
        &self,
        event_id: Uuid,
        ordinal: u32,
    ) -> Result<Option<RecoveryUpload>, StoreError> {
        self.call(move |w| claim(&mut w.db, event_id, ordinal))
            .await
    }
    pub async fn read_recovery_upload(
        &self,
        event_id: Uuid,
        ordinal: u32,
        expected: OperationId,
    ) -> Result<RecoveryUpload, StoreError> {
        self.call(move |w| read(&w.db, event_id, ordinal, expected))
            .await
    }
    pub async fn mark_recovery_unknown(
        &self,
        event_id: Uuid,
        ordinal: u32,
        expected: OperationId,
    ) -> Result<(), StoreError> {
        self.call(move |w| {
            let changed = w.db.execute("UPDATE recovery_uploads SET state='unknown' WHERE event_id=?1 AND ordinal=?2 AND operation_id=?3 AND state IN ('leased','unknown')", params![event_id.to_string(), ordinal, expected.0.to_string()])?;
            if changed != 1 { return Err(StoreError::UnknownOperation); }
            Ok(())
        }).await
    }
    pub async fn acknowledge_recovery_upload(
        &self,
        event_id: Uuid,
        ordinal: u32,
        expected: OperationId,
        ack: MutationAck,
    ) -> Result<(), StoreError> {
        self.call(move |w| acknowledge(&mut w.db, event_id, ordinal, expected, ack))
            .await
    }
    /// Occupied-path evidence must identify this exact current attempt and contain different bytes.
    pub async fn relocate_recovery_upload(
        &self,
        event_id: Uuid,
        ordinal: u32,
        expected: OperationId,
        occupied: RemoteFile,
    ) -> Result<RecoveryUpload, StoreError> {
        self.call(move |w| relocate(&mut w.db, event_id, ordinal, expected, occupied))
            .await
    }
}

fn original_tuple(input: &AutomaticRecoveryInput) -> Result<String, StoreError> {
    let remote = match &input.remote {
        RemoteRead::Present(file) => Some((&file.body, &file.revision, file.permission)),
        RemoteRead::Missing(_) => None,
    };
    serde_json::to_string(&(
        1_u32,
        &input.local.key.connection,
        &input.local.key.path,
        input.base.as_ref().map(|base| (&base.body, &base.revision)),
        &input.local.body,
        remote,
        &input.draft,
    ))
    .map_err(|_| StoreError::InvalidData)
}
fn validate(input: &AutomaticRecoveryInput) -> Result<(), StoreError> {
    restoration::validate_page(&input.local)?;
    let key = match &input.remote {
        RemoteRead::Present(file) => &file.key,
        RemoteRead::Missing(key) => key,
    };
    if key != &input.local.key || input.base.as_ref().is_some_and(|base| base.key != *key) {
        return Err(StoreError::InvalidData);
    }
    Ok(())
}

pub(super) fn register(
    db: &Connection,
    input: AutomaticRecoveryInput,
) -> Result<AutomaticRecoveryEvent, StoreError> {
    validate(&input)?;
    let tuple = original_tuple(&input)?;
    let fingerprint = Sha256::digest(tuple.as_bytes());
    let existing: Option<String> = db
        .query_row(
            "SELECT id FROM automatic_events WHERE fingerprint=?1 AND original_tuple=?2",
            params![fingerprint.as_slice(), tuple],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return event(db, parse_uuid(&id)?);
    }
    let event = AutomaticRecoveryEvent {
        id: Uuid::new_v4(),
        input,
        created_ms: pages::now_ms()?
            .try_into()
            .map_err(|_| StoreError::InvalidData)?,
    };
    let key = &event.input.local.key;
    let snapshot = serde_json::to_string(&event).map_err(|_| StoreError::InvalidData)?;
    db.execute("INSERT INTO automatic_events(id,connection,path,fingerprint,original_tuple,snapshot,created_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![event.id.to_string(), key.connection.as_str(), key.path.as_str(), fingerprint.as_slice(), tuple, snapshot, i64::try_from(event.created_ms).map_err(|_| StoreError::InvalidData)?])?;
    create_attempt(db, &event, 0, event.id, &event.input.local.body, None)?;
    if let Some(draft) = &event.input.draft
        && draft != &event.input.local.body
    {
        create_attempt(db, &event, 1, Uuid::new_v4(), draft, None)?;
    }
    Ok(event)
}

fn event(db: &Connection, id: Uuid) -> Result<AutomaticRecoveryEvent, StoreError> {
    let (value, tuple): (String, String) = db
        .query_row(
            "SELECT snapshot,original_tuple FROM automatic_events WHERE id=?1",
            [id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(StoreError::UnknownOperation)?;
    let event: AutomaticRecoveryEvent =
        serde_json::from_str(&value).map_err(|_| StoreError::InvalidData)?;
    validate(&event.input)?;
    if event.id != id || original_tuple(&event.input)? != tuple {
        return Err(StoreError::InvalidData);
    }
    Ok(event)
}

fn destination(original: &PagePath, id: Uuid) -> Result<PagePath, StoreError> {
    let filename = original
        .as_str()
        .rsplit('/')
        .next()
        .ok_or(StoreError::InvalidData)?;
    let stem = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);
    let mut name = String::new();
    let mut replacing = false;
    for ch in stem.chars() {
        if name.len() == 64 {
            break;
        }
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            name.push(ch);
            replacing = false;
        } else if !replacing {
            name.push('-');
            replacing = true;
        }
    }
    if name.is_empty() {
        name = "note".into();
    }
    PagePath::parse(&format!("Bulletnotes Recovery/{name}-{id}.md"))
        .map_err(|_| StoreError::InvalidData)
}
fn create_attempt(
    db: &Connection,
    event: &AutomaticRecoveryEvent,
    ordinal: u32,
    mut suffix: Uuid,
    source: &str,
    edited: Option<(&PageSnapshot, bool)>,
) -> Result<RecoveryUpload, StoreError> {
    let key = loop {
        let key = PageKey {
            connection: event.input.local.key.connection.clone(),
            path: destination(&event.input.local.key.path, suffix)?,
        };
        let occupied: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM pages WHERE connection=?1 AND path=?2) OR EXISTS(SELECT 1 FROM recovery_uploads u JOIN automatic_events e ON e.id=u.event_id WHERE e.connection=?1 AND u.path=?2)", [key.connection.as_str(), key.path.as_str()], |row| row.get(0))?;
        if !occupied {
            break key;
        }
        suffix = Uuid::new_v4();
    };
    let operation = PendingMutation {
        id: OperationId::new(),
        key: key.clone(),
        local_revision: LocalRevision(1),
        kind: MutationKind::Put(source.into()),
        condition: WriteCondition::CreateOnly,
    };
    db.execute("INSERT INTO recovery_uploads(operation_id,event_id,ordinal,path,source,local_revision,state,current) VALUES(?1,?2,?3,?4,?5,'1','pending',1)", params![operation.id.0.to_string(), event.id.to_string(), ordinal, key.path.as_str(), source])?;
    let body = edited.map_or(source, |(page, _)| page.body.as_str());
    let revision = edited.map_or(LocalRevision(1), |(page, _)| page.local_revision);
    let position = edited.map(|(page, _)| page.position).unwrap_or_default();
    let trashed = edited.is_some_and(|(_, trashed)| trashed);
    db.execute("INSERT INTO pages(connection,path,title,body,local_revision,explicit_create,trashed,anchor,caret,scroll_y,modified_ms) VALUES(?1,?2,?3,?4,?5,1,?6,?7,?8,?9,?10)", params![key.connection.as_str(), key.path.as_str(), key.title_from(body), body, revision.0.to_string(), trashed, i64::try_from(position.anchor).map_err(|_| StoreError::InvalidData)?, i64::try_from(position.caret).map_err(|_| StoreError::InvalidData)?, position.scroll_y, pages::now_ms()?])?;
    Ok(RecoveryUpload {
        event_id: event.id,
        ordinal,
        operation,
    })
}
fn parse_uuid(value: &str) -> Result<Uuid, StoreError> {
    Uuid::parse_str(value).map_err(|_| StoreError::InvalidData)
}
fn state(value: &str) -> Result<RecoveryUploadState, StoreError> {
    match value {
        "pending" => Ok(RecoveryUploadState::Pending),
        "leased" => Ok(RecoveryUploadState::Leased),
        "unknown" => Ok(RecoveryUploadState::Unknown),
        "blocked" => Ok(RecoveryUploadState::Blocked),
        "acknowledged" => Ok(RecoveryUploadState::Acknowledged),
        _ => Err(StoreError::InvalidData),
    }
}
fn heads(
    db: &Connection,
    connection: &ConnectionId,
    limit: usize,
) -> Result<Vec<RecoveryUploadHead>, StoreError> {
    let limit = i64::try_from(limit).map_err(|_| StoreError::InvalidQuery)?;
    let mut query = db.prepare("SELECT u.event_id,u.ordinal,u.operation_id,u.path,u.state FROM recovery_uploads u JOIN automatic_events e ON e.id=u.event_id WHERE e.connection=?1 AND u.state IN ('pending','unknown') ORDER BY u.rowid LIMIT ?2")?;
    query
        .query_map(params![connection.as_str(), limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .map(|row| {
            let (event_id, ordinal, operation_id, path, value) = row?;
            Ok(RecoveryUploadHead {
                event_id: parse_uuid(&event_id)?,
                ordinal,
                operation_id: OperationId(parse_uuid(&operation_id)?),
                key: PageKey {
                    connection: connection.clone(),
                    path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
                },
                state: state(&value)?,
            })
        })
        .collect()
}
fn read(
    db: &Connection,
    event_id: Uuid,
    ordinal: u32,
    expected: OperationId,
) -> Result<RecoveryUpload, StoreError> {
    let (connection, path, body, revision): (String, String, String, String) = db.query_row("SELECT e.connection,u.path,u.source,u.local_revision FROM recovery_uploads u JOIN automatic_events e ON e.id=u.event_id WHERE u.event_id=?1 AND u.ordinal=?2 AND u.operation_id=?3", params![event_id.to_string(), ordinal, expected.0.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).optional()?.ok_or(StoreError::UnknownOperation)?;
    Ok(RecoveryUpload {
        event_id,
        ordinal,
        operation: PendingMutation {
            id: expected,
            key: PageKey {
                connection: ConnectionId::try_from(connection)
                    .map_err(|_| StoreError::InvalidData)?,
                path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
            },
            local_revision: LocalRevision(revision.parse().map_err(|_| StoreError::InvalidData)?),
            kind: MutationKind::Put(body),
            condition: WriteCondition::CreateOnly,
        },
    })
}
fn claim(
    db: &mut Connection,
    event_id: Uuid,
    ordinal: u32,
) -> Result<Option<RecoveryUpload>, StoreError> {
    let tx = db.transaction()?;
    let id: Option<String> = tx.query_row("SELECT u.operation_id FROM recovery_uploads u JOIN automatic_events e ON e.id=u.event_id JOIN pages p ON p.connection=e.connection AND p.path=u.path WHERE u.event_id=?1 AND u.ordinal=?2 AND u.current=1 AND u.state='pending' AND p.permission=1", params![event_id.to_string(), ordinal], |row| row.get(0)).optional()?;
    let Some(id) = id else {
        return Ok(None);
    };
    let upload = read(&tx, event_id, ordinal, OperationId(parse_uuid(&id)?))?;
    tx.execute(
        "UPDATE recovery_uploads SET state='leased' WHERE operation_id=?1",
        [id],
    )?;
    tx.commit()?;
    Ok(Some(upload))
}
fn acknowledge(
    db: &mut Connection,
    event_id: Uuid,
    ordinal: u32,
    expected: OperationId,
    ack: MutationAck,
) -> Result<(), StoreError> {
    let tx = db.transaction()?;
    let current: bool = tx.query_row("SELECT current FROM recovery_uploads WHERE event_id=?1 AND ordinal=?2 AND operation_id=?3 AND state IN ('leased','unknown')", params![event_id.to_string(), ordinal, expected.0.to_string()], |row| row.get(0)).optional()?.ok_or(StoreError::UnknownOperation)?;
    let upload = read(&tx, event_id, ordinal, expected)?;
    tx.execute(
        "UPDATE recovery_uploads SET state='acknowledged' WHERE operation_id=?1",
        [expected.0.to_string()],
    )?;
    if let Some(page) = pages::load(&tx, &upload.operation.key)? {
        if current {
            let MutationKind::Put(body) = &upload.operation.kind else {
                return Err(StoreError::InvalidData);
            };
            tx.execute("UPDATE pages SET remote_body=?1,remote_revision=?2,remote_modified_ms=NULL,explicit_create=0 WHERE connection=?3 AND path=?4", params![body, ack.remote_revision.map(|rev| rev.0), page.key.connection.as_str(), page.key.path.as_str()])?;
        }
        let snapshot = pages::load(&tx, &page.key)?.ok_or(StoreError::PageMissing)?;
        outbox::queue_put(&tx, pages::id(&tx, &page.key)?, &snapshot)?;
    }
    tx.commit()?;
    Ok(())
}
fn relocate(
    db: &mut Connection,
    event_id: Uuid,
    ordinal: u32,
    expected: OperationId,
    occupied: RemoteFile,
) -> Result<RecoveryUpload, StoreError> {
    let tx = db.transaction()?;
    let eligible: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM recovery_uploads WHERE event_id=?1 AND ordinal=?2 AND operation_id=?3 AND current=1 AND state!='acknowledged')", params![event_id.to_string(), ordinal, expected.0.to_string()], |row| row.get(0))?;
    if !eligible {
        return Err(StoreError::UnknownOperation);
    }
    let old = read(&tx, event_id, ordinal, expected)?;
    let MutationKind::Put(source) = &old.operation.kind else {
        return Err(StoreError::InvalidData);
    };
    if occupied.key != old.operation.key || occupied.body == *source {
        return Err(StoreError::InvalidData);
    }
    let page = pages::load(&tx, &old.operation.key)?;
    let trashed = if page.is_some() {
        tx.query_row(
            "SELECT trashed FROM pages WHERE connection=?1 AND path=?2",
            [occupied.key.connection.as_str(), occupied.key.path.as_str()],
            |row| row.get(0),
        )?
    } else {
        false
    };
    tx.execute("UPDATE recovery_uploads SET current=0,state=CASE WHEN state IN ('leased','unknown') THEN 'unknown' ELSE 'blocked' END WHERE operation_id=?1", [expected.0.to_string()])?;
    let replacement = create_attempt(
        &tx,
        &event(&tx, event_id)?,
        ordinal,
        Uuid::new_v4(),
        source,
        page.as_ref().map(|page| (page, trashed)),
    )?;
    if page.is_some() {
        remote::set_base(&tx, &occupied.key, Some(&occupied))?;
        tx.execute("UPDATE pages SET body=?1,title=?2,anchor=0,caret=0,scroll_y=0,trashed=0,remote_deleted=0,explicit_create=0 WHERE connection=?3 AND path=?4", params![occupied.body, occupied.key.title_from(&occupied.body), occupied.key.connection.as_str(), occupied.key.path.as_str()])?;
    }
    tx.commit()?;
    Ok(replacement)
}

pub(super) fn fenced(db: &Connection, key: &PageKey) -> Result<bool, StoreError> {
    Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM recovery_uploads u JOIN automatic_events e ON e.id=u.event_id WHERE e.connection=?1 AND u.path=?2 AND (u.current=1 AND u.state!='acknowledged' OR u.state IN ('leased','unknown')))", [key.connection.as_str(), key.path.as_str()], |row| row.get(0))?)
}
