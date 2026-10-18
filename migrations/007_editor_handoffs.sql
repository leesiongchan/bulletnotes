-- One bounded latest authored branch per page; canonical source remains independent until replay.
CREATE TABLE editor_handoffs (
 connection TEXT NOT NULL,
 path TEXT NOT NULL,
 snapshot TEXT NOT NULL,
 PRIMARY KEY(connection,path),
 FOREIGN KEY(connection,path) REFERENCES pages(connection,path) ON DELETE CASCADE
) STRICT;
