-- 原文独立于收集箱的网页摘要。收集箱条目继续承载状态、备注和原链接。
CREATE TABLE capture_sources (
  source_id TEXT PRIMARY KEY REFERENCES inbox_items(id) ON DELETE CASCADE,
  platform TEXT NOT NULL,
  body TEXT NOT NULL DEFAULT '' CHECK (length(CAST(body AS BLOB)) <= 65536),
  supplied_urls TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(supplied_urls))
);

-- 用收集箱同一套规范地址关联出处，已收藏的仓库也可以保留出处。
CREATE TABLE capture_source_links (
  source_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
  target_url TEXT NOT NULL,
  PRIMARY KEY (source_id, target_url)
);
CREATE INDEX idx_capture_source_links_target ON capture_source_links(target_url);

CREATE TABLE capture_source_receipts (
  account_id TEXT NOT NULL REFERENCES github_accounts(id) ON DELETE CASCADE,
  request_key TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  source_id TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
  PRIMARY KEY (account_id, request_key)
);

ALTER TABLE inbox_items ADD COLUMN page_capture_links TEXT NOT NULL DEFAULT '[]'
  CHECK (json_valid(page_capture_links));

INSERT INTO schema_migrations(version, name) VALUES ('040', 'capture_sources_v2');
