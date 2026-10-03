-- 034: 收集箱文章正文里提到的 GitHub 项目。只追加列，不改已有列。
-- page_repos 是 JSON 数组，元素是 "owner/name"（最多 30 个），没找到是 []。
ALTER TABLE inbox_items ADD COLUMN page_repos TEXT NOT NULL DEFAULT '[]' CHECK (length(page_repos) <= 8000);
INSERT INTO schema_migrations(version, name) VALUES('034', 'inbox_repos');
