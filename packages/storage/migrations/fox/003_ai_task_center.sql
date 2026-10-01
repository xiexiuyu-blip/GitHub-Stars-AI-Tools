-- Fox Stars Lab migration 003 ai_task_center
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xed4cd1
-- Verify against schema/live_schema_018.sql before use.

PRAGMA foreign_keys = ON;

-- AI batch history deliberately uses its own tables. The original jobs table has
-- a constrained type/status vocabulary used by the upstream worker and must stay
-- compatible with existing databases.
CREATE TABLE IF NOT EXISTS ai_task_runs (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  task_type TEXT NOT NULL CHECK (task_type IN ('batch_generate_ai_documents')),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'succeeded', 'failed')),
  provider TEXT,
  model TEXT,
  only_missing INTEGER NOT NULL DEFAULT 1 CHECK (only_missing IN (0, 1)),
  requested_limit INTEGER,
  target_count INTEGER NOT NULL DEFAULT 0,
  parent_run_id TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  started_at TEXT,
  completed_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  FOREIGN KEY (parent_run_id) REFERENCES ai_task_runs(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_task_runs_account_created ON ai_task_runs(account_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_ai_task_runs_parent ON ai_task_runs(parent_run_id);

CREATE TABLE IF NOT EXISTS ai_task_items (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  repository_id TEXT NOT NULL,
  full_name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'succeeded', 'skipped', 'failed')),
  reason TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0,
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (run_id) REFERENCES ai_task_runs(id) ON DELETE CASCADE,
  FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE,
  UNIQUE (run_id, repository_id)
);

CREATE INDEX IF NOT EXISTS idx_ai_task_items_run_status ON ai_task_items(run_id, status);
CREATE INDEX IF NOT EXISTS idx_ai_task_items_repository ON ai_task_items(repository_id);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('003', 'ai_task_center');
