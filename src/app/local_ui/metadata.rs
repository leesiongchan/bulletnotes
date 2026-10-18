//! Latest position and visit obligations, retried in admission order.
use crate::{
    domain::{editor::EditorPosition, error::StoreError, page::PageKey},
    store::StoreHandle,
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Clone)]
struct Write {
    id: uuid::Uuid,
    key: PageKey,
    position: Option<EditorPosition>,
}
#[derive(Clone, Default)]
pub(super) struct MetadataJournal(Rc<RefCell<VecDeque<Write>>>);
impl MetadataJournal {
    pub(super) fn record(&self, key: PageKey, position: Option<EditorPosition>) {
        let mut pending = self.0.borrow_mut();
        pending.retain(|write| write.key != key || write.position.is_some() != position.is_some());
        pending.push_back(Write {
            id: uuid::Uuid::new_v4(),
            key,
            position,
        });
    }
    pub(super) async fn flush(&self, store: &StoreHandle) -> Result<(), (PageKey, StoreError)> {
        loop {
            let Some(write) = self.0.borrow().front().cloned() else {
                return Ok(());
            };
            let result = persist(&write, store).await;
            result.map_err(|error| (write.key, error))?;
            let mut pending = self.0.borrow_mut();
            // A newer intent for this field can arrive while the store is writing.
            if pending
                .front()
                .is_some_and(|current| current.id == write.id)
            {
                pending.pop_front();
            }
        }
    }
}

async fn persist(write: &Write, store: &StoreHandle) -> Result<(), StoreError> {
    let now = super::effects::now_ms()?;
    let result = match write.position {
        Some(position) => store.save_position(&write.key, position, now).await,
        None => store.record_visit(&write.key, now).await,
    };
    match result {
        Err(error @ (StoreError::InvalidData | StoreError::PageMissing)) => {
            match (store.load_page(&write.key).await?, write.position) {
                // A removed page has no remaining position/history obligation.
                (None, _) => Ok(()),
                // An offset into replaced source is obsolete; retain its visit only.
                (Some(page), Some(position)) if position.validate(&page.body).is_err() => {
                    store.record_visit(&write.key, now).await
                }
                _ => Err(error),
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        connection::ConnectionConfig,
        page::{LocalRevision, PagePath},
    };
    fn key(path: &str) -> PageKey {
        PageKey {
            connection: ConnectionConfig::parse("https://metadata.example")
                .unwrap()
                .id()
                .clone(),
            path: PagePath::parse(path).unwrap(),
        }
    }
    #[tokio::test]
    async fn obsolete_position_after_body_change_does_not_block_durability() {
        let dir = tempfile::tempdir().unwrap();
        let store = StoreHandle::open(&dir.path().join("metadata.sqlite"))
            .await
            .unwrap();
        let key = key("one.md");
        store.create_draft(key.clone()).await.unwrap();
        store
            .save_local(key.clone(), "abc".into(), LocalRevision(1))
            .await
            .unwrap();
        let journal = MetadataJournal::default();
        journal.record(
            key.clone(),
            Some(EditorPosition {
                anchor: 3,
                caret: 3,
                scroll_y: 2.0,
            }),
        );
        store
            .save_local(key.clone(), "a".into(), LocalRevision(2))
            .await
            .unwrap();
        journal.flush(&store).await.unwrap();
        let page = store.load_page(&key).await.unwrap().unwrap();
        assert_eq!(page.body, "a");
        assert!(page.position.validate(&page.body).is_ok());
        store.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn failed_metadata_retains_visit_order_when_retried() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.sqlite");
        let store = StoreHandle::open(&path).await.unwrap();
        for key in [key("a.md"), key("b.md")] {
            store.create_draft(key).await.unwrap();
        }
        let injector = rusqlite::Connection::open(&path).unwrap();
        injector.execute_batch("CREATE TRIGGER reject_visit BEFORE UPDATE OF last_visited_ms ON pages WHEN NEW.path='a.md' BEGIN SELECT RAISE(ABORT,'generated failure'); END;").unwrap();
        let journal = MetadataJournal::default();
        journal.record(key("a.md"), None);
        assert!(journal.flush(&store).await.is_err());
        journal.record(key("b.md"), None);
        assert!(journal.flush(&store).await.is_err());
        injector.execute_batch("DROP TRIGGER reject_visit").unwrap();
        journal.flush(&store).await.unwrap();
        assert_eq!(
            store
                .last_cached_page(&key("b.md").connection)
                .await
                .unwrap(),
            Some(key("b.md"))
        );
        store.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn newer_position_arriving_during_a_write_survives_acknowledgement() {
        use std::future::Future;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("metadata.sqlite");
        let store = StoreHandle::open(&path).await.unwrap();
        let key = key("one.md");
        store.create_draft(key.clone()).await.unwrap();
        let journal = MetadataJournal::default();
        journal.record(
            key.clone(),
            Some(EditorPosition {
                scroll_y: 1.0,
                ..EditorPosition::default()
            }),
        );
        let injector = rusqlite::Connection::open(&path).unwrap();
        injector.execute_batch("BEGIN IMMEDIATE").unwrap();
        let mut flushing = Box::pin(journal.flush(&store));
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(
                flushing.as_mut().poll(cx).is_pending()
            ))
            .await
        );
        journal.record(
            key.clone(),
            Some(EditorPosition {
                scroll_y: 2.0,
                ..EditorPosition::default()
            }),
        );
        injector.execute_batch("COMMIT").unwrap();
        flushing.await.unwrap();
        assert_eq!(
            store
                .load_page(&key)
                .await
                .unwrap()
                .unwrap()
                .position
                .scroll_y,
            2.0
        );
        store.shutdown().await.unwrap();
    }
}
