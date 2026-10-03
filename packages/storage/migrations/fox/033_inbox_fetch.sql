-- 033: 收集箱公开链接的读取结果。只追加列，不改已有列。
-- 已有的 GitHub 仓库条目本来就有标题，标成跳过，不联网读取。
ALTER TABLE inbox_items ADD COLUMN fetch_status TEXT NOT NULL DEFAULT 'pending'
  CHECK (fetch_status IN ('pending','done','failed','skipped'));
ALTER TABLE inbox_items ADD COLUMN fetch_error TEXT NOT NULL DEFAULT '' CHECK (length(fetch_error) <= 500);
ALTER TABLE inbox_items ADD COLUMN fetched_at TEXT;
ALTER TABLE inbox_items ADD COLUMN page_author TEXT NOT NULL DEFAULT '' CHECK (length(page_author) <= 200);
ALTER TABLE inbox_items ADD COLUMN page_excerpt TEXT NOT NULL DEFAULT '' CHECK (length(page_excerpt) <= 8000);
UPDATE inbox_items SET fetch_status = 'skipped' WHERE item_type = 'github_repository';
INSERT INTO schema_migrations(version, name) VALUES('033', 'inbox_fetch');
