ALTER TABLE pages ADD COLUMN catalogued INTEGER NOT NULL DEFAULT 0 CHECK(catalogued IN (0,1));
ALTER TABLE pages ADD COLUMN remote_deleted INTEGER NOT NULL DEFAULT 0 CHECK(remote_deleted IN (0,1));
CREATE TABLE catalogue_state (connection TEXT PRIMARY KEY) STRICT;
CREATE TABLE conflicts (
 id TEXT NOT NULL UNIQUE,
 connection TEXT NOT NULL,
 path TEXT NOT NULL,
 snapshot TEXT NOT NULL,
 PRIMARY KEY(connection,path)
) STRICT;
