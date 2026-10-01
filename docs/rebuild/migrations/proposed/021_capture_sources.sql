-- PROPOSED 021: 收集来源扩展（X 帖子/群聊/论坛/剪贴板批量）。不重建 inbox_items（避免 010 式表重建），只加列。
ALTER TABLE inbox_items ADD COLUMN source_platform TEXT NOT NULL DEFAULT 'web'
  CHECK (source_platform IN ('web','github','x','wechat','telegram','discord','forum','xiaohongshu','bilibili','youtube','local','other'));
ALTER TABLE inbox_items ADD COLUMN source_context TEXT NOT NULL DEFAULT '' CHECK (length(source_context) <= 2000);
ALTER TABLE inbox_items ADD COLUMN captured_via TEXT NOT NULL DEFAULT 'manual'
  CHECK (captured_via IN ('manual','paste_batch','clipboard','bookmarklet','local_api','mcp','share_extension','folder_watch'));
ALTER TABLE inbox_items ADD COLUMN capture_batch_id TEXT;
ALTER TABLE inbox_items ADD COLUMN raw_excerpt TEXT NOT NULL DEFAULT '' CHECK (length(raw_excerpt) <= 8000);
CREATE INDEX idx_inbox_items_batch ON inbox_items(capture_batch_id);
CREATE TABLE capture_batches (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  raw_text_hash TEXT NOT NULL,
  source_platform TEXT NOT NULL DEFAULT 'other',
  source_context TEXT NOT NULL DEFAULT '',
  url_count INTEGER NOT NULL DEFAULT 0,
  github_repo_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE(account_id, raw_text_hash)
);
INSERT INTO schema_migrations(version, name) VALUES('021', 'capture_sources');
