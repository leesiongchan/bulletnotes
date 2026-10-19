CREATE TABLE scratchpad_order (
 connection TEXT NOT NULL,
 path TEXT NOT NULL,
 created_ms INTEGER NOT NULL,
 order_ms INTEGER NOT NULL,
 PRIMARY KEY(connection,path)
) STRICT;
CREATE INDEX scratchpad_creation_order ON scratchpad_order(connection,order_ms DESC,path COLLATE BINARY);
