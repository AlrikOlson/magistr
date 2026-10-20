-- Schema captured from an existing .workbench journal on 2026-10-20.
-- Fixture data is synthetic; no project source or user history is included.
CREATE TABLE ops(seq INTEGER PRIMARY KEY AUTOINCREMENT,at INTEGER NOT NULL,tool TEXT NOT NULL,args_hash TEXT NOT NULL,session_id TEXT NOT NULL,is_undo INTEGER NOT NULL DEFAULT 0);
CREATE INDEX ops_session ON ops(session_id, seq);
CREATE TABLE op_files(op_seq INTEGER NOT NULL REFERENCES ops(seq),ord INTEGER NOT NULL,kind TEXT NOT NULL CHECK(kind IN ('create','write','delete')),path TEXT NOT NULL,before_hash TEXT NOT NULL,after_hash TEXT NOT NULL,PRIMARY KEY(op_seq, ord));
CREATE TABLE blobs(hash TEXT PRIMARY KEY,zstd BLOB NOT NULL,bytes INTEGER NOT NULL,last_ref_seq INTEGER NOT NULL);
CREATE TABLE undone(op_seq INTEGER PRIMARY KEY,by_seq INTEGER NOT NULL);
CREATE TABLE native_write_intents(op_seq INTEGER PRIMARY KEY REFERENCES ops(seq),outcome TEXT NOT NULL CHECK(outcome IN ('pending','applied','rolled_back','indeterminate')));
CREATE TABLE native_intent_undo(intent_seq INTEGER NOT NULL REFERENCES ops(seq),op_seq INTEGER NOT NULL REFERENCES ops(seq),PRIMARY KEY(intent_seq,op_seq));
PRAGMA user_version=3;
