-- Fox Stars Lab migration 017 inbox_ai_understandings
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xee3485
-- Verify against schema/live_schema_018.sql before use.

PRAGMA foreign_keys = ON;

CREATE TABLE inbox_ai_understandings (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  inbox_item_id TEXT NOT NULL,
  source_content_hash TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  review_status TEXT NOT NULL DEFAULT 'draft' CHECK(review_status IN ('draft','confirmed','rejected')),
  is_locked INTEGER NOT NULL DEFAULT 0 CHECK(is_locked IN (0,1)),
  is_stale INTEGER NOT NULL DEFAULT 0 CHECK(is_stale IN (0,1)),
  what_is_it TEXT NOT NULL DEFAULT '',
  problem_solved TEXT NOT NULL DEFAULT '',
  suitable_for TEXT NOT NULL DEFAULT '',
  core_capabilities TEXT NOT NULL DEFAULT '',
  limitations TEXT NOT NULL DEFAULT '',
  integration_hints TEXT NOT NULL DEFAULT '',
  suggested_tool_name TEXT NOT NULL DEFAULT '',
  citations_json TEXT NOT NULL DEFAULT '[]',
  generated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  reviewed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  FOREIGN KEY(inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE
);

CREATE INDEX idx_inbox_ai_understandings_current ON inbox_ai_understandings(account_id, inbox_item_id, source_content_hash, created_at DESC);

-- This is a narrow extension of the established AI task center: the run keeps
-- provider/model/status/retry history, while the target is an inbox item rather
-- than a repository.
CREATE TABLE inbox_ai_task_items (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  inbox_item_id TEXT NOT NULL,
  title TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','succeeded','skipped','failed')),
  reason TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0,
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(run_id) REFERENCES ai_task_runs(id) ON DELETE CASCADE,
  FOREIGN KEY(inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE,
  UNIQUE(run_id, inbox_item_id)
);
CREATE INDEX idx_inbox_ai_task_items_run_status ON inbox_ai_task_items(run_id, status);

CREATE TRIGGER trg_inbox_ai_understanding_account_insert BEFORE INSERT ON inbox_ai_understandings
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM inbox_items i WHERE i.id=NEW.inbox_item_id AND i.account_id=NEW.account_id)
BEGIN SELECT RAISE(ABORT, 'inbox AI understanding must share account'); END;
CREATE TRIGGER trg_inbox_ai_understanding_account_update BEFORE UPDATE OF account_id,inbox_item_id ON inbox_ai_understandings
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM inbox_items i WHERE i.id=NEW.inbox_item_id AND i.account_id=NEW.account_id)
BEGIN SELECT RAISE(ABORT, 'inbox AI understanding must share account'); END;
CREATE TRIGGER trg_inbox_ai_understanding_confirmed_lock BEFORE UPDATE ON inbox_ai_understandings
FOR EACH ROW WHEN OLD.review_status='confirmed' AND OLD.is_locked=1 AND (
  NEW.account_id IS NOT OLD.account_id OR NEW.inbox_item_id IS NOT OLD.inbox_item_id OR NEW.source_content_hash IS NOT OLD.source_content_hash OR NEW.provider IS NOT OLD.provider OR NEW.model IS NOT OLD.model OR NEW.prompt_version IS NOT OLD.prompt_version OR NEW.review_status IS NOT OLD.review_status OR NEW.is_locked IS NOT OLD.is_locked OR NEW.is_stale IS NOT OLD.is_stale OR NEW.what_is_it IS NOT OLD.what_is_it OR NEW.problem_solved IS NOT OLD.problem_solved OR NEW.suitable_for IS NOT OLD.suitable_for OR NEW.core_capabilities IS NOT OLD.core_capabilities OR NEW.limitations IS NOT OLD.limitations OR NEW.integration_hints IS NOT OLD.integration_hints OR NEW.suggested_tool_name IS NOT OLD.suggested_tool_name OR NEW.citations_json IS NOT OLD.citations_json OR NEW.generated_at IS NOT OLD.generated_at OR NEW.reviewed_at IS NOT OLD.reviewed_at OR NEW.created_at IS NOT OLD.created_at)
BEGIN SELECT RAISE(ABORT, 'confirmed inbox AI understanding is locked'); END;
CREATE TRIGGER trg_inbox_ai_task_item_account BEFORE INSERT ON inbox_ai_task_items
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM ai_task_runs r JOIN inbox_items i ON i.id=NEW.inbox_item_id WHERE r.id=NEW.run_id AND r.account_id=i.account_id)
BEGIN SELECT RAISE(ABORT, 'inbox AI task item must share run account'); END;

INSERT INTO schema_migrations(version, name) VALUES('017', 'inbox_ai_understandings');
