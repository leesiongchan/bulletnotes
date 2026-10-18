//! Per-record admission leases bridge Send store callers to the main-thread draft owner.
use super::StoreHandle;
use crate::domain::{error::StoreError, page::PageKey};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::{OwnedMutexGuard, mpsc, oneshot};
use uuid::Uuid;

pub enum DraftAdmissionRequest {
    Close {
        key: PageKey,
        conflict_id: Uuid,
        lease_id: Uuid,
        reply: oneshot::Sender<Result<(), StoreError>>,
    },
    Release {
        key: PageKey,
        conflict_id: Uuid,
        lease_id: Uuid,
        committed: bool,
    },
}
#[derive(Default)]
pub(super) struct Admission {
    owner: Option<mpsc::UnboundedSender<DraftAdmissionRequest>>,
    keys: HashMap<PageKey, Weak<tokio::sync::Mutex<()>>>,
}
pub(super) type SharedAdmission = Arc<Mutex<Admission>>;
pub(super) struct DraftLease {
    sender: mpsc::UnboundedSender<DraftAdmissionRequest>,
    key: PageKey,
    conflict_id: Uuid,
    id: Uuid,
    committed: bool,
    _record: OwnedMutexGuard<()>,
}
impl DraftLease {
    pub(super) fn committed(&mut self) {
        self.committed = true;
    }
    pub(super) fn conflict_id(&self) -> Uuid {
        self.conflict_id
    }
}
impl Drop for DraftLease {
    fn drop(&mut self) {
        let _ = self.sender.send(DraftAdmissionRequest::Release {
            key: self.key.clone(),
            conflict_id: self.conflict_id,
            lease_id: self.id,
            committed: self.committed,
        });
    }
}
impl StoreHandle {
    /// Install exactly one main-thread owner. A disconnected owner never becomes headless again.
    pub fn install_draft_admission(
        &self,
    ) -> Result<mpsc::UnboundedReceiver<DraftAdmissionRequest>, StoreError> {
        let mut admission = self
            .draft_admission
            .lock()
            .map_err(|_| StoreError::Closed)?;
        if admission.owner.is_some() {
            return Err(StoreError::InvalidData);
        }
        let (sender, receiver) = mpsc::unbounded_channel();
        admission.owner = Some(sender);
        Ok(receiver)
    }
    pub(super) async fn legacy_admission(
        &self,
        key: &PageKey,
    ) -> Result<Option<DraftLease>, StoreError> {
        let target = key.clone();
        let conflict = self
            .call(move |w| super::remote::read_conflict(&w.db, &target))
            .await?;
        match conflict {
            Some(conflict) => self.close_draft_admission(key, conflict.id).await,
            None => Ok(None),
        }
    }
    pub(super) async fn close_draft_admission(
        &self,
        key: &PageKey,
        conflict_id: Uuid,
    ) -> Result<Option<DraftLease>, StoreError> {
        let (owner, record) = {
            let mut admission = self
                .draft_admission
                .lock()
                .map_err(|_| StoreError::Closed)?;
            let Some(owner) = admission.owner.clone() else {
                return Ok(None);
            };
            admission.keys.retain(|_, entry| entry.strong_count() > 0);
            let record = admission
                .keys
                .get(key)
                .and_then(Weak::upgrade)
                .unwrap_or_else(|| Arc::new(tokio::sync::Mutex::new(())));
            admission.keys.insert(key.clone(), Arc::downgrade(&record));
            (owner, record)
        };
        let record = record.lock_owned().await;
        let lease = DraftLease {
            sender: owner,
            key: key.clone(),
            conflict_id,
            id: Uuid::new_v4(),
            committed: false,
            _record: record,
        };
        let (reply, receiver) = oneshot::channel();
        lease
            .sender
            .send(DraftAdmissionRequest::Close {
                key: key.clone(),
                conflict_id,
                lease_id: lease.id,
                reply,
            })
            .map_err(|_| StoreError::Closed)?;
        receiver.await.map_err(|_| StoreError::Closed)??;
        Ok(Some(lease))
    }
}
