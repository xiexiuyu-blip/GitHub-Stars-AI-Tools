-- Fox Stars Lab migration 006 inbox_documents
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xed4cd1
-- Verify against schema/live_schema_018.sql before use.

PRAGMA foreign_keys = ON;

-- Parsed web pages are deliberately kept apart from the user-authored inbox
-- card. Remote content is untrusted evidence only; no HTML is stored.
CREATE TABLE IF NOT EXISTS inbox_documents (
  inbox_item_id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  requested_url TEXT NOT NULL,
  resolved_url TEXT,
  extracted_title TEXT,
  author TEXT,
  published_at TEXT,
  content_text TEXT NOT NULL DEFAULT '',
  content_hash TEXT,
  mime_type TEXT,
  content_truncated INTEGER NOT NULL DEFAULT 0 CHECK (content_truncated IN (0, 1)),
  parse_status TEXT NOT NULL DEFAULT 'not_parsed' CHECK (parse_status IN ('not_parsed', 'parsing', 'parsed', 'updated', 'unchanged', 'failed')),
  fetched_at TEXT,
  last_attempt_at TEXT,
  last_failure_at TEXT,
  failure_reason TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);

-- SQLite cannot express the required cross-column relationship without
-- changing the historic inbox table, so enforce it on every write.
CREATE TRIGGER IF NOT EXISTS trg_inbox_documents_account_insert
BEFORE INSERT ON inbox_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items WHERE id = NEW.inbox_item_id AND account_id = NEW.account_id
)
BEGIN
  SELECT RAISE(ABORT, 'inbox document account must match inbox item');
END;

CREATE TRIGGER IF NOT EXISTS trg_inbox_documents_account_update
BEFORE UPDATE OF inbox_item_id, account_id ON inbox_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items WHERE id = NEW.inbox_item_id AND account_id = NEW.account_id
)
BEGIN
  SELECT RAISE(ABORT, 'inbox document account must match inbox item');
END;

CREATE INDEX IF NOT EXISTS idx_inbox_documents_account_updated
  ON inbox_documents(account_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_inbox_documents_account_status
  ON inbox_documents(account_id, parse_status, updated_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('006', 'inbox_documents');
