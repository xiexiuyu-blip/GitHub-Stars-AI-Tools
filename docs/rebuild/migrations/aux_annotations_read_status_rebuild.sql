-- Runtime legacy fixup (not a numbered migration): rebuilds annotations with Fox read_status CHECK
-- Extracted from gsat-desktop 1.4.0-fox.1 @0xeeb2ca

PRAGMA foreign_keys = OFF;
BEGIN;
DROP TABLE IF EXISTS annotations_migrated;
CREATE TABLE annotations_migrated (
  repo_id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  note_md TEXT NOT NULL DEFAULT '',
  rating INTEGER,
  read_status TEXT NOT NULL DEFAULT 'unread' CHECK (read_status IN ('unread', 'read', 'later', 'want_to_try', 'tried', 'in_use', 'watching', 'deprecated')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
INSERT INTO annotations_migrated (repo_id, account_id, note_md, rating, read_status, updated_at)
SELECT
  repo_id,
  account_id,
  note_md,
  rating,
  CASE read_status
    WHEN 'unread' THEN 'unread'
    WHEN 'read' THEN 'read'
    WHEN 'later' THEN 'later'
    WHEN 'want_to_try' THEN 'want_to_try'
    WHEN 'tried' THEN 'tried'
    WHEN 'in_use' THEN 'in_use'
    WHEN 'watching' THEN 'watching'
    WHEN 'deprecated' THEN 'deprecated'
    ELSE 'unread'
  END,
  updated_at
FROM annotations;
DROP TABLE annotations;
ALTER TABLE annotations_migrated RENAME TO annotations;
CREATE INDEX IF NOT EXISTS idx_annotations_account ON annotations(account_id);
CREATE INDEX IF NOT EXISTS idx_annotations_account_repo ON annotations(account_id, repo_id);
CREATE INDEX IF NOT EXISTS idx_annotations_read_status ON annotations(read_status);
COMMIT;
PRAGMA foreign_keys = ON;
