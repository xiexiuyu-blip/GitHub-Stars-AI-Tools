-- PROPOSED 025: 通用、可断点续跑的 AI 任务队列。旧 ai_task_runs/items（task_type CHECK 只允许 batch_generate_ai_documents）保留为只读历史。
CREATE TABLE ai_jobs (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  job_type TEXT NOT NULL CHECK (job_type IN ('repo_card','repo_deep_doc','inbox_card','tool_card','categorize','graph_extract','problem_answer','discover_rationale','translate_readme','reference_pack')),
  target_type TEXT NOT NULL CHECK (target_type IN ('repository','inbox_item','tool','concept','query','candidate')),
  target_id TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 5 CHECK (priority BETWEEN 0 AND 9),
  status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','running','succeeded','failed','cancelled','skipped')),
  attempts INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 3,
  not_before TEXT,
  error_kind TEXT CHECK (error_kind IS NULL OR error_kind IN ('interrupted','timeout','rate_limit','auth','bad_json','schema','network','content_too_large','other')),
  error_message TEXT,
  input_hash TEXT,
  provider TEXT, model TEXT, prompt_version TEXT,
  input_tokens INTEGER NOT NULL DEFAULT 0, output_tokens INTEGER NOT NULL DEFAULT 0,
  legacy_task_item_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  started_at TEXT, finished_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_ai_jobs_pick ON ai_jobs(status, priority, created_at);
CREATE UNIQUE INDEX idx_ai_jobs_active_target ON ai_jobs(job_type, target_type, target_id) WHERE status IN ('queued','running');
-- 旧失败项转为可重试任务（live 库 150 个失败项，其中 139 个为“应用已退出，任务中断”）；每个仓库只取一条，已有成功的跳过
INSERT OR IGNORE INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, error_kind, error_message, legacy_task_item_id)
SELECT lower(hex(randomblob(16))), r.account_id, 'repo_card', 'repository', i.repository_id, 6, 'queued',
       CASE WHEN i.reason LIKE '%中断%' THEN 'interrupted' WHEN i.reason LIKE '%超时%' THEN 'timeout'
            WHEN i.reason LIKE '%JSON%' OR i.reason LIKE '%字段%' THEN 'bad_json' ELSE 'other' END,
       i.reason, i.id
FROM ai_task_items i JOIN ai_task_runs r ON r.id = i.run_id
WHERE i.status = 'failed' AND i.repository_id IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM ai_task_items s WHERE s.repository_id = i.repository_id AND s.status = 'succeeded');
INSERT INTO schema_migrations(version, name) VALUES('025', 'ai_jobs');
