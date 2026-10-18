-- Evidence survives source-page deletion; authorization is separately supersedable.
ALTER TABLE outbox ADD COLUMN delete_rejected INTEGER NOT NULL DEFAULT 0 CHECK(delete_rejected IN (0,1));
CREATE TABLE retired_delete_watches (
 operation_id TEXT PRIMARY KEY,
 connection TEXT NOT NULL,
 path TEXT NOT NULL,
 operation TEXT NOT NULL,
 original TEXT NOT NULL,
 protected TEXT NOT NULL,
 terminal_rejected INTEGER NOT NULL CHECK(terminal_rejected IN (0,1)),
 active INTEGER NOT NULL CHECK(active IN (0,1)),
 created_ms INTEGER NOT NULL
) STRICT;
CREATE INDEX retired_delete_watch_keys ON retired_delete_watches(connection,path,active);
CREATE TRIGGER retired_delete_evidence_immutable BEFORE UPDATE OF operation_id,connection,path,operation,original,terminal_rejected,created_ms ON retired_delete_watches BEGIN
 SELECT RAISE(ABORT,'retired delete evidence is immutable');
END;
