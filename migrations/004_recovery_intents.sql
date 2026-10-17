ALTER TABLE pages ADD COLUMN trashed INTEGER NOT NULL DEFAULT 0 CHECK(trashed IN (0,1));
ALTER TABLE pages ADD COLUMN explicit_create INTEGER NOT NULL DEFAULT 0 CHECK(explicit_create IN (0,1));
CREATE TABLE merge_drafts (
 connection TEXT NOT NULL,
 path TEXT NOT NULL,
 body TEXT NOT NULL,
 PRIMARY KEY(connection,path)
) STRICT;
ALTER TABLE recovery ADD COLUMN label TEXT NOT NULL DEFAULT 'Retained version';
