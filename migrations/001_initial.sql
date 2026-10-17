CREATE TABLE pages (
 id INTEGER PRIMARY KEY,
 connection TEXT NOT NULL,
 path TEXT NOT NULL,
 title TEXT NOT NULL,
 body TEXT,
 local_revision TEXT NOT NULL DEFAULT '0',
 remote_body TEXT,
 remote_revision TEXT,
 remote_modified_ms INTEGER,
 permission INTEGER NOT NULL DEFAULT 1 CHECK(permission IN (0,1)),
 anchor INTEGER NOT NULL DEFAULT 0,
 caret INTEGER NOT NULL DEFAULT 0,
 scroll_y REAL NOT NULL DEFAULT 0,
 modified_ms INTEGER,
 last_visited_ms INTEGER,
 UNIQUE(connection,path)
) STRICT;
CREATE TABLE outbox (
 operation_id TEXT PRIMARY KEY,
 page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
 local_revision TEXT NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN ('put','delete')),
 body TEXT,
 condition_kind TEXT NOT NULL CHECK(condition_kind IN ('create','match')),
 condition_revision TEXT,
 state TEXT NOT NULL CHECK(state IN ('pending','leased','unknown','blocked')),
 CHECK((kind='put' AND body IS NOT NULL) OR (kind='delete' AND body IS NULL)),
 CHECK(condition_kind!='create' OR condition_revision IS NULL)
) STRICT;
CREATE UNIQUE INDEX one_unsent_intent ON outbox(page_id) WHERE state IN ('pending','blocked');
CREATE UNIQUE INDEX one_inflight_mutation ON outbox(page_id) WHERE state IN ('leased','unknown');
CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL) STRICT;
CREATE TABLE recovery (id TEXT PRIMARY KEY, connection TEXT NOT NULL, path TEXT NOT NULL, snapshot TEXT NOT NULL, created_ms INTEGER NOT NULL) STRICT;
CREATE TABLE trash (id TEXT PRIMARY KEY, connection TEXT NOT NULL, path TEXT NOT NULL, snapshot TEXT NOT NULL, created_ms INTEGER NOT NULL, operation_id TEXT) STRICT;
CREATE VIRTUAL TABLE page_fts USING fts5(title,path,body,tokenize='unicode61 remove_diacritics 0');
CREATE TRIGGER pages_fts_insert AFTER INSERT ON pages BEGIN
 INSERT INTO page_fts(rowid,title,path,body) VALUES(new.id,new.title,new.path,coalesce(new.body,''));
END;
CREATE TRIGGER pages_fts_update AFTER UPDATE OF title,path,body ON pages BEGIN
 DELETE FROM page_fts WHERE rowid=old.id;
 INSERT INTO page_fts(rowid,title,path,body) VALUES(new.id,new.title,new.path,coalesce(new.body,''));
END;
CREATE TRIGGER pages_fts_delete AFTER DELETE ON pages BEGIN
 DELETE FROM page_fts WHERE rowid=old.id;
END;
