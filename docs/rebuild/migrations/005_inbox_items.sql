-- Fox Stars Lab migration 005 inbox_items
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xed4cd1
-- Verify against schema/live_schema_018.sql before use.

PRAGMA foreign_keys = ON;

-- The inbox is deliberately a lightweight source card.  It does not duplicate
-- repository facts or imply that a URL has been fetched or processed by AI.
CREATE TABLE IF NOT EXISTS inbox_items (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  original_url TEXT NOT NULL,
  normalized_url TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  item_type TEXT NOT NULL CHECK (item_type IN ('github_repository', 'webpage', 'video', 'document', 'other')),
  user_note TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'ready', 'failed', 'archived')),
  repository_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE SET NULL,
  UNIQUE (account_id, normalized_url)
);

CREATE INDEX IF NOT EXISTS idx_inbox_items_account_status_updated
  ON inbox_items(account_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_inbox_items_account_type_updated
  ON inbox_items(account_id, item_type, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_inbox_items_repository ON inbox_items(repository_id);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('005', 'inbox_items');
