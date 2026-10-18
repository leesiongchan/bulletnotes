ALTER TABLE pages ADD COLUMN source_generation INTEGER NOT NULL DEFAULT 0 CHECK(source_generation>=0);
ALTER TABLE merge_drafts ADD COLUMN revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0);
-- A shared clock prevents old tokens matching a deleted and recreated page or draft.
CREATE TABLE source_clock (
 id INTEGER PRIMARY KEY CHECK(id=1),
 generation INTEGER NOT NULL CHECK(generation>=0)
) STRICT;
INSERT INTO source_clock VALUES(1,0);
CREATE TRIGGER source_clock_exhausted BEFORE UPDATE ON source_clock
WHEN old.generation=9223372036854775807 BEGIN
 SELECT RAISE(ABORT,'source generation exhausted');
END;
CREATE TRIGGER pages_source_insert AFTER INSERT ON pages BEGIN
 UPDATE source_clock SET generation=generation+1 WHERE id=1;
 UPDATE pages SET source_generation=(SELECT generation FROM source_clock WHERE id=1) WHERE id=new.id;
END;
CREATE TRIGGER pages_source_update AFTER UPDATE OF connection,path,body,local_revision,remote_body,remote_revision,remote_modified_ms,permission,remote_deleted,trashed,explicit_create ON pages
WHEN old.connection IS NOT new.connection OR old.path IS NOT new.path
 OR old.body IS NOT new.body OR old.local_revision IS NOT new.local_revision
 OR old.remote_body IS NOT new.remote_body OR old.remote_revision IS NOT new.remote_revision
 OR old.remote_modified_ms IS NOT new.remote_modified_ms OR old.permission IS NOT new.permission
 OR old.remote_deleted IS NOT new.remote_deleted OR old.trashed IS NOT new.trashed
 OR old.explicit_create IS NOT new.explicit_create BEGIN
 UPDATE source_clock SET generation=generation+1 WHERE id=1;
 UPDATE pages SET source_generation=(SELECT generation FROM source_clock WHERE id=1) WHERE id=new.id;
END;
CREATE TRIGGER merge_drafts_revision_insert AFTER INSERT ON merge_drafts BEGIN
 UPDATE source_clock SET generation=generation+1 WHERE id=1;
 UPDATE merge_drafts SET revision=(SELECT generation FROM source_clock WHERE id=1) WHERE connection=new.connection AND path=new.path;
END;
CREATE TRIGGER merge_drafts_revision_update AFTER UPDATE OF body ON merge_drafts BEGIN
 UPDATE source_clock SET generation=generation+1 WHERE id=1;
 UPDATE merge_drafts SET revision=(SELECT generation FROM source_clock WHERE id=1) WHERE connection=new.connection AND path=new.path;
END;
-- Conflict observations are reconciliation inputs even when the page source is unchanged.
CREATE TRIGGER conflicts_source_insert AFTER INSERT ON conflicts BEGIN
 UPDATE source_clock SET generation=generation+1 WHERE id=1;
 UPDATE pages SET source_generation=(SELECT generation FROM source_clock WHERE id=1) WHERE connection=new.connection AND path=new.path;
END;
CREATE TRIGGER conflicts_source_update AFTER UPDATE OF snapshot,id ON conflicts
WHEN old.snapshot IS NOT new.snapshot OR old.id IS NOT new.id BEGIN
 UPDATE source_clock SET generation=generation+1 WHERE id=1;
 UPDATE pages SET source_generation=(SELECT generation FROM source_clock WHERE id=1) WHERE connection=new.connection AND path=new.path;
END;
CREATE TRIGGER conflicts_source_delete AFTER DELETE ON conflicts BEGIN
 UPDATE source_clock SET generation=generation+1 WHERE id=1;
 UPDATE pages SET source_generation=(SELECT generation FROM source_clock WHERE id=1) WHERE connection=old.connection AND path=old.path;
END;
CREATE TABLE automatic_events (
 id TEXT PRIMARY KEY,
 connection TEXT NOT NULL,
 path TEXT NOT NULL,
 fingerprint BLOB NOT NULL,
 original_tuple TEXT NOT NULL,
 snapshot TEXT NOT NULL,
 created_ms INTEGER NOT NULL CHECK(created_ms>=0),
 UNIQUE(fingerprint,original_tuple)
) STRICT;
CREATE TABLE recovery_uploads (
 operation_id TEXT PRIMARY KEY,
 event_id TEXT NOT NULL REFERENCES automatic_events(id),
 ordinal INTEGER NOT NULL CHECK(ordinal IN (0,1)),
 path TEXT NOT NULL,
 source TEXT NOT NULL,
 local_revision TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('pending','leased','unknown','blocked','acknowledged')),
 current INTEGER NOT NULL CHECK(current IN (0,1)),
 UNIQUE(event_id,ordinal,path)
) STRICT;
CREATE UNIQUE INDEX current_recovery_upload ON recovery_uploads(event_id,ordinal) WHERE current=1;
CREATE INDEX recovery_uploads_state ON recovery_uploads(state);
CREATE TRIGGER automatic_events_immutable BEFORE UPDATE ON automatic_events BEGIN
 SELECT RAISE(ABORT,'automatic event is immutable');
END;
CREATE TRIGGER recovery_uploads_immutable BEFORE UPDATE OF operation_id,event_id,ordinal,path,source,local_revision ON recovery_uploads BEGIN
 SELECT RAISE(ABORT,'recovery attempt is immutable');
END;
