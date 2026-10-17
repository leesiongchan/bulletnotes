CREATE TABLE probe_journal (
 connection TEXT NOT NULL,
 path TEXT NOT NULL,
 PRIMARY KEY(connection,path)
) STRICT;
