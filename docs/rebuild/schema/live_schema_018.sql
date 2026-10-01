CREATE TABLE schema_migrations (
  version TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE TABLE github_accounts (
  id TEXT PRIMARY KEY,
  login TEXT NOT NULL UNIQUE,
  avatar_url TEXT,
  token_ref TEXT NOT NULL,
  connection_status TEXT NOT NULL DEFAULT 'connected' CHECK (connection_status IN ('connected', 'disconnected')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX idx_github_accounts_connection_status_updated ON github_accounts(connection_status, updated_at DESC);
CREATE TABLE repositories (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  full_name TEXT NOT NULL,
  description TEXT,
  language TEXT,
  topics_json TEXT NOT NULL DEFAULT '[]',
  html_url TEXT NOT NULL,
  stars_count INTEGER NOT NULL DEFAULT 0,
  forks_count INTEGER NOT NULL DEFAULT 0,
  starred_at TEXT NOT NULL,
  pushed_at TEXT,
  sync_status TEXT NOT NULL DEFAULT 'active' CHECK (sync_status IN ('active', 'removed', 'gone', 'error')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, full_name)
);
CREATE INDEX idx_repositories_account ON repositories(account_id);
CREATE INDEX idx_repositories_account_status_starred ON repositories(account_id, sync_status, starred_at DESC);
CREATE INDEX idx_repositories_account_language_status ON repositories(account_id, language, sync_status);
CREATE INDEX idx_repositories_language ON repositories(language);
CREATE INDEX idx_repositories_starred_at ON repositories(starred_at);
CREATE INDEX idx_repositories_sync_status ON repositories(sync_status);
CREATE TABLE repo_readmes (
  repo_id TEXT PRIMARY KEY,
  raw_markdown TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  source_path TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE
);
CREATE INDEX idx_repo_readmes_content_hash ON repo_readmes(content_hash);
CREATE TABLE repo_ai_documents (
  repo_id TEXT PRIMARY KEY,
  summary_zh TEXT NOT NULL,
  readme_zh TEXT,
  keywords_json TEXT NOT NULL DEFAULT '[]',
  suggested_tags_json TEXT NOT NULL DEFAULT '[]',
  model TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  source_hash TEXT NOT NULL,
  input_tokens INTEGER NOT NULL DEFAULT 0,
  output_tokens INTEGER NOT NULL DEFAULT 0,
  generated_at TEXT NOT NULL, summary_mode TEXT NOT NULL DEFAULT 'legacy', coverage_status TEXT NOT NULL DEFAULT 'legacy', source_char_count INTEGER NOT NULL DEFAULT 0, processed_char_count INTEGER NOT NULL DEFAULT 0, chunk_count INTEGER NOT NULL DEFAULT 0, citation_count INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE
);
CREATE INDEX idx_repo_ai_documents_source_hash ON repo_ai_documents(source_hash);
CREATE TABLE repo_embeddings (
  repo_id TEXT NOT NULL,
  source_kind TEXT NOT NULL CHECK (source_kind IN ('repository_knowledge')),
  source_hash TEXT NOT NULL,
  model TEXT NOT NULL,
  model_version TEXT NOT NULL,
  dimensions INTEGER NOT NULL,
  vector_json TEXT NOT NULL,
  generated_at TEXT NOT NULL,
  PRIMARY KEY (repo_id, source_kind, model, model_version),
  FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE
);
CREATE INDEX idx_repo_embeddings_model ON repo_embeddings(model, model_version);
CREATE INDEX idx_repo_embeddings_source_hash ON repo_embeddings(source_hash);
CREATE TABLE tags (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  name TEXT NOT NULL,
  color TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, name)
);
CREATE INDEX idx_tags_account ON tags(account_id);
CREATE TABLE repo_tags (
  repo_id TEXT NOT NULL,
  tag_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (repo_id, tag_id),
  FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE,
  FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);
CREATE INDEX idx_repo_tags_tag_repo ON repo_tags(tag_id, repo_id);
CREATE TABLE annotations (
  repo_id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  note_md TEXT NOT NULL DEFAULT '',
  rating INTEGER,
  read_status TEXT NOT NULL DEFAULT 'unread' CHECK (read_status IN ('unread', 'read', 'later', 'want_to_try', 'tried', 'in_use', 'watching', 'deprecated')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE INDEX idx_annotations_account ON annotations(account_id);
CREATE INDEX idx_annotations_account_repo ON annotations(account_id, repo_id);
CREATE INDEX idx_annotations_read_status ON annotations(read_status);
CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  type TEXT NOT NULL CHECK (type IN ('sync_stars', 'fetch_readme', 'summarize', 'translate', 'embed')),
  payload_json TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'skipped')),
  idempotency_key TEXT NOT NULL UNIQUE,
  retry_count INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE INDEX idx_jobs_account_status ON jobs(account_id, status);
CREATE INDEX idx_jobs_type_status ON jobs(type, status);
CREATE TABLE github_recommendation_candidates (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  full_name TEXT NOT NULL,
  description TEXT,
  language TEXT,
  topics_json TEXT NOT NULL DEFAULT '[]',
  html_url TEXT NOT NULL,
  stars_count INTEGER NOT NULL DEFAULT 0,
  forks_count INTEGER NOT NULL DEFAULT 0,
  pushed_at TEXT,
  status TEXT NOT NULL DEFAULT 'new' CHECK (status IN ('new', 'marked', 'ignored', 'starred')),
  rationale_zh TEXT,
  queries_json TEXT NOT NULL DEFAULT '[]',
  last_seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, full_name)
);
CREATE INDEX idx_recommendation_candidates_account_status ON github_recommendation_candidates(account_id, status, updated_at DESC);
CREATE INDEX idx_recommendation_candidates_account_full_name ON github_recommendation_candidates(account_id, full_name);
CREATE TABLE github_recommendation_documents (
  account_id TEXT NOT NULL,
  full_name TEXT NOT NULL,
  raw_markdown TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  source_path TEXT NOT NULL,
  fetched_at TEXT NOT NULL,
  translation_markdown_zh TEXT,
  translation_model TEXT,
  translation_input_tokens INTEGER,
  translation_output_tokens INTEGER,
  translation_source_char_count INTEGER,
  translation_translated_char_count INTEGER,
  translation_is_truncated INTEGER,
  translation_source_hash TEXT,
  translation_generated_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (account_id, full_name),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE INDEX idx_recommendation_documents_account_updated ON github_recommendation_documents(account_id, updated_at DESC);
CREATE TABLE github_ranking_cache (
  cache_key TEXT PRIMARY KEY,
  payload_json TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);
CREATE INDEX idx_github_ranking_cache_fetched_at ON github_ranking_cache(fetched_at DESC);
CREATE TABLE ai_task_runs (
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
CREATE INDEX idx_ai_task_runs_account_created ON ai_task_runs(account_id, created_at DESC);
CREATE INDEX idx_ai_task_runs_parent ON ai_task_runs(parent_run_id);
CREATE TABLE ai_task_items (
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
CREATE INDEX idx_ai_task_items_run_status ON ai_task_items(run_id, status);
CREATE INDEX idx_ai_task_items_repository ON ai_task_items(repository_id);
CREATE TABLE repo_ai_citations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  repo_id TEXT NOT NULL,
  source_hash TEXT NOT NULL,
  citation_order INTEGER NOT NULL,
  chunk_id TEXT NOT NULL,
  heading_path TEXT NOT NULL,
  start_line INTEGER NOT NULL,
  end_line INTEGER NOT NULL,
  excerpt TEXT NOT NULL,
  claim TEXT,
  FOREIGN KEY (repo_id) REFERENCES repositories(id) ON DELETE CASCADE,
  UNIQUE(repo_id, source_hash, citation_order)
);
CREATE TABLE sqlite_sequence(name,seq);
CREATE INDEX idx_repo_ai_citations_current ON repo_ai_citations(repo_id, source_hash, citation_order);
CREATE TABLE inbox_items (
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
CREATE INDEX idx_inbox_items_account_status_updated
  ON inbox_items(account_id, status, updated_at DESC);
CREATE INDEX idx_inbox_items_account_type_updated
  ON inbox_items(account_id, item_type, updated_at DESC);
CREATE INDEX idx_inbox_items_repository ON inbox_items(repository_id);
CREATE TABLE inbox_documents (
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
CREATE TRIGGER trg_inbox_documents_account_insert
BEFORE INSERT ON inbox_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items WHERE id = NEW.inbox_item_id AND account_id = NEW.account_id
)
BEGIN
  SELECT RAISE(ABORT, 'inbox document account must match inbox item');
END;
CREATE TRIGGER trg_inbox_documents_account_update
BEFORE UPDATE OF inbox_item_id, account_id ON inbox_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items WHERE id = NEW.inbox_item_id AND account_id = NEW.account_id
)
BEGIN
  SELECT RAISE(ABORT, 'inbox document account must match inbox item');
END;
CREATE INDEX idx_inbox_documents_account_updated
  ON inbox_documents(account_id, updated_at DESC);
CREATE INDEX idx_inbox_documents_account_status
  ON inbox_documents(account_id, parse_status, updated_at DESC);
CREATE TRIGGER trg_inbox_documents_source_insert
BEFORE INSERT ON inbox_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items
  WHERE id = NEW.inbox_item_id
    AND account_id = NEW.account_id
    AND normalized_url = NEW.requested_url
    AND item_type = 'webpage'
)
BEGIN
  SELECT RAISE(ABORT, 'stale inbox document source');
END;
CREATE TRIGGER trg_inbox_documents_source_update
BEFORE UPDATE ON inbox_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items
  WHERE id = NEW.inbox_item_id
    AND account_id = NEW.account_id
    AND normalized_url = NEW.requested_url
    AND item_type = 'webpage'
)
BEGIN
  SELECT RAISE(ABORT, 'stale inbox document source');
END;
CREATE TABLE "inbox_local_documents" (
  inbox_item_id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  file_name TEXT NOT NULL,
  document_kind TEXT NOT NULL CHECK (document_kind IN ('markdown', 'text', 'pdf')),
  content_text TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
  page_count INTEGER CHECK (page_count IS NULL OR page_count >= 0),
  content_truncated INTEGER NOT NULL DEFAULT 0 CHECK (content_truncated IN (0, 1)),
  imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, content_hash)
);
CREATE TRIGGER trg_inbox_local_documents_account_insert
BEFORE INSERT ON inbox_local_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items
  WHERE id = NEW.inbox_item_id
    AND account_id = NEW.account_id
    AND item_type = 'document'
    AND normalized_url = 'fox-local://document/' || NEW.inbox_item_id
)
BEGIN
  SELECT RAISE(ABORT, 'local inbox document must match local document card');
END;
CREATE TRIGGER trg_inbox_local_documents_account_update
BEFORE UPDATE OF inbox_item_id, account_id, content_hash ON inbox_local_documents
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM inbox_items
  WHERE id = NEW.inbox_item_id
    AND account_id = NEW.account_id
    AND item_type = 'document'
    AND normalized_url = 'fox-local://document/' || NEW.inbox_item_id
)
BEGIN
  SELECT RAISE(ABORT, 'local inbox document must match local document card');
END;
CREATE INDEX idx_inbox_local_documents_account_imported
  ON inbox_local_documents(account_id, imported_at DESC);
CREATE INDEX idx_inbox_local_documents_account_kind
  ON inbox_local_documents(account_id, document_kind, imported_at DESC);
CREATE TABLE tools (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 160),
  normalized_name TEXT NOT NULL CHECK (normalized_name = lower(trim(normalized_name)) AND length(normalized_name) BETWEEN 1 AND 160),
  summary TEXT NOT NULL DEFAULT '' CHECK (length(summary) <= 4000),
  homepage_url TEXT CHECK (homepage_url IS NULL OR (homepage_url GLOB 'http://*' OR homepage_url GLOB 'https://*') AND instr(substr(homepage_url, instr(homepage_url, '//') + 2), '@') = 0),
  selection_status TEXT NOT NULL DEFAULT 'unread' CHECK (selection_status IN ('unread', 'want_to_try', 'tried', 'in_use', 'watching', 'deprecated', 'later', 'read')),
  manual_note TEXT NOT NULL DEFAULT '' CHECK (length(manual_note) <= 12000),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE (account_id, normalized_name)
);
CREATE TABLE tool_forms (
  tool_id TEXT NOT NULL,
  form_kind TEXT NOT NULL CHECK (form_kind IN ('app', 'cli', 'skill', 'mcp', 'api', 'library', 'service', 'other')),
  PRIMARY KEY (tool_id, form_kind),
  FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE CASCADE
);
CREATE TABLE tool_repositories (
  tool_id TEXT NOT NULL,
  repository_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  PRIMARY KEY (tool_id, repository_id),
  FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE CASCADE,
  FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE TABLE tool_inbox_items (
  tool_id TEXT NOT NULL,
  inbox_item_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  PRIMARY KEY (tool_id, inbox_item_id),
  FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE CASCADE,
  FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE,
  FOREIGN KEY (account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE INDEX idx_tools_account_updated ON tools(account_id, updated_at DESC);
CREATE INDEX idx_tools_account_status ON tools(account_id, selection_status, updated_at DESC);
CREATE INDEX idx_tool_repositories_account_repository ON tool_repositories(account_id, repository_id);
CREATE INDEX idx_tool_inbox_items_account_inbox ON tool_inbox_items(account_id, inbox_item_id);
CREATE TRIGGER trg_tool_repositories_account_insert
BEFORE INSERT ON tool_repositories
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM tools t JOIN repositories r ON r.id = NEW.repository_id
  WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id AND r.account_id = NEW.account_id
)
BEGIN SELECT RAISE(ABORT, 'tool repository must share account'); END;
CREATE TRIGGER trg_tool_repositories_account_update
BEFORE UPDATE OF tool_id, repository_id, account_id ON tool_repositories
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM tools t JOIN repositories r ON r.id = NEW.repository_id
  WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id AND r.account_id = NEW.account_id
)
BEGIN SELECT RAISE(ABORT, 'tool repository must share account'); END;
CREATE TRIGGER trg_tool_inbox_items_account_insert
BEFORE INSERT ON tool_inbox_items
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM tools t JOIN inbox_items i ON i.id = NEW.inbox_item_id
  WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id AND i.account_id = NEW.account_id
)
BEGIN SELECT RAISE(ABORT, 'tool inbox item must share account'); END;
CREATE TRIGGER trg_tool_inbox_items_account_update
BEFORE UPDATE OF tool_id, inbox_item_id, account_id ON tool_inbox_items
FOR EACH ROW WHEN NOT EXISTS (
  SELECT 1 FROM tools t JOIN inbox_items i ON i.id = NEW.inbox_item_id
  WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id AND i.account_id = NEW.account_id
)
BEGIN SELECT RAISE(ABORT, 'tool inbox item must share account'); END;
CREATE TRIGGER trg_tools_normalized_name_insert
BEFORE INSERT ON tools
FOR EACH ROW WHEN NEW.normalized_name != lower(trim(NEW.name))
BEGIN SELECT RAISE(ABORT, 'tool normalized name must match ASCII-normalized name'); END;
CREATE TRIGGER trg_tools_normalized_name_update
BEFORE UPDATE OF name, normalized_name ON tools
FOR EACH ROW WHEN NEW.normalized_name != lower(trim(NEW.name))
BEGIN SELECT RAISE(ABORT, 'tool normalized name must match ASCII-normalized name'); END;
CREATE TABLE tool_registry_imports (
  id TEXT PRIMARY KEY, account_id TEXT NOT NULL, file_name TEXT NOT NULL,
  content_hash TEXT NOT NULL, format_kind TEXT NOT NULL CHECK(format_kind='markdown_table'),
  imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  selected_count INTEGER NOT NULL, line_count INTEGER NOT NULL, entry_count INTEGER NOT NULL,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE(account_id, content_hash)
);
CREATE TABLE tool_registry_entries (
  import_id TEXT NOT NULL, account_id TEXT NOT NULL, entry_key TEXT NOT NULL,
  tool_id TEXT, source_name TEXT NOT NULL, source_type TEXT NOT NULL,
  source_purpose TEXT NOT NULL, source_status TEXT NOT NULL DEFAULT '',
  warnings_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(import_id, entry_key),
  FOREIGN KEY(import_id) REFERENCES tool_registry_imports(id) ON DELETE CASCADE,
  FOREIGN KEY(tool_id) REFERENCES tools(id) ON DELETE SET NULL,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE TRIGGER trg_tool_registry_entries_account_insert BEFORE INSERT ON tool_registry_entries
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM tool_registry_imports i WHERE i.id=NEW.import_id AND i.account_id=NEW.account_id)
 OR (NEW.tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tools t WHERE t.id=NEW.tool_id AND t.account_id=NEW.account_id))
BEGIN SELECT RAISE(ABORT,'registry provenance must share account'); END;
CREATE TRIGGER trg_tool_registry_entries_account_update BEFORE UPDATE ON tool_registry_entries
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM tool_registry_imports i WHERE i.id=NEW.import_id AND i.account_id=NEW.account_id)
 OR (NEW.tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tools t WHERE t.id=NEW.tool_id AND t.account_id=NEW.account_id))
BEGIN SELECT RAISE(ABORT,'registry provenance must share account'); END;
CREATE INDEX idx_tool_registry_entries_tool ON tool_registry_entries(account_id,tool_id);
CREATE TABLE local_capability_imports (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  source_kind TEXT NOT NULL CHECK(source_kind IN ('skill_directory', 'mcp_config')),
  source_label TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  selected_count INTEGER NOT NULL CHECK(selected_count >= 0),
  entry_count INTEGER NOT NULL CHECK(entry_count >= 0),
  warning_count INTEGER NOT NULL CHECK(warning_count >= 0),
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE(account_id, source_kind, content_hash)
);
CREATE TABLE local_capability_entries (
  import_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  entry_key TEXT NOT NULL,
  tool_id TEXT,
  capability_kind TEXT NOT NULL CHECK(capability_kind IN ('skill', 'mcp')),
  source_name TEXT NOT NULL,
  source_description TEXT NOT NULL DEFAULT '',
  transport_kind TEXT,
  safe_endpoint_origin TEXT,
  package_hint TEXT,
  warnings_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(import_id, entry_key),
  FOREIGN KEY(import_id) REFERENCES local_capability_imports(id) ON DELETE CASCADE,
  FOREIGN KEY(tool_id) REFERENCES tools(id) ON DELETE SET NULL,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE TRIGGER trg_local_capability_entries_account_insert
BEFORE INSERT ON local_capability_entries
FOR EACH ROW WHEN
  NOT EXISTS(
    SELECT 1 FROM local_capability_imports i
    WHERE i.id = NEW.import_id AND i.account_id = NEW.account_id
  )
  OR (NEW.tool_id IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM tools t WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'local capability provenance must share account');
END;
CREATE TRIGGER trg_local_capability_entries_account_update
BEFORE UPDATE ON local_capability_entries
FOR EACH ROW WHEN
  NOT EXISTS(
    SELECT 1 FROM local_capability_imports i
    WHERE i.id = NEW.import_id AND i.account_id = NEW.account_id
  )
  OR (NEW.tool_id IS NOT NULL AND NOT EXISTS(
    SELECT 1 FROM tools t WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id
  ))
BEGIN
  SELECT RAISE(ABORT, 'local capability provenance must share account');
END;
CREATE INDEX idx_local_capability_imports_account
  ON local_capability_imports(account_id, source_kind, imported_at);
CREATE INDEX idx_local_capability_entries_tool
  ON local_capability_entries(account_id, tool_id);
CREATE TABLE scenario_evaluation_groups (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  name TEXT NOT NULL,
  normalized_name TEXT NOT NULL,
  problem_statement TEXT NOT NULL DEFAULT '',
  constraints TEXT NOT NULL DEFAULT '',
  manual_note TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft', 'evaluating', 'decided', 'archived')),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE(account_id, normalized_name)
);
CREATE TABLE scenario_evaluation_candidates (
  scenario_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  tool_id TEXT NOT NULL,
  candidate_role TEXT NOT NULL DEFAULT 'alternative' CHECK(candidate_role IN ('primary', 'alternative', 'watch')),
  evaluation_status TEXT NOT NULL DEFAULT 'unreviewed' CHECK(evaluation_status IN ('unreviewed', 'researching', 'trial', 'accepted', 'rejected')),
  manual_note TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0 CHECK(sort_order >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(scenario_id, tool_id),
  FOREIGN KEY(scenario_id) REFERENCES scenario_evaluation_groups(id) ON DELETE CASCADE,
  FOREIGN KEY(tool_id) REFERENCES tools(id) ON DELETE CASCADE,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE TRIGGER trg_scenario_candidate_account_insert
BEFORE INSERT ON scenario_evaluation_candidates
FOR EACH ROW WHEN NOT EXISTS(
  SELECT 1 FROM scenario_evaluation_groups s WHERE s.id = NEW.scenario_id AND s.account_id = NEW.account_id
) OR NOT EXISTS(
  SELECT 1 FROM tools t WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id
)
BEGIN
  SELECT RAISE(ABORT, 'scenario candidate must share account');
END;
CREATE TRIGGER trg_scenario_candidate_account_update
BEFORE UPDATE ON scenario_evaluation_candidates
FOR EACH ROW WHEN NOT EXISTS(
  SELECT 1 FROM scenario_evaluation_groups s WHERE s.id = NEW.scenario_id AND s.account_id = NEW.account_id
) OR NOT EXISTS(
  SELECT 1 FROM tools t WHERE t.id = NEW.tool_id AND t.account_id = NEW.account_id
)
BEGIN
  SELECT RAISE(ABORT, 'scenario candidate must share account');
END;
CREATE TRIGGER trg_scenario_candidate_limit_insert
BEFORE INSERT ON scenario_evaluation_candidates
FOR EACH ROW WHEN (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = NEW.scenario_id) >= 8
BEGIN
  SELECT RAISE(ABORT, 'scenario supports at most 8 candidates');
END;
CREATE TRIGGER trg_scenario_candidate_limit_update
BEFORE UPDATE OF scenario_id ON scenario_evaluation_candidates
FOR EACH ROW WHEN NEW.scenario_id <> OLD.scenario_id AND (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = NEW.scenario_id) >= 8
BEGIN
  SELECT RAISE(ABORT, 'scenario supports at most 8 candidates');
END;
CREATE TRIGGER trg_scenario_status_candidate_gate
BEFORE UPDATE OF status ON scenario_evaluation_groups
FOR EACH ROW WHEN NEW.status IN ('evaluating', 'decided') AND (
  (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = NEW.id) < 2
  OR (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = NEW.id) > 8
)
BEGIN
  SELECT RAISE(ABORT, 'evaluating or decided scenarios require 2 to 8 candidates');
END;
CREATE TRIGGER trg_scenario_insert_status_candidate_gate
BEFORE INSERT ON scenario_evaluation_groups
FOR EACH ROW WHEN NEW.status IN ('evaluating', 'decided')
BEGIN
  SELECT RAISE(ABORT, 'evaluating or decided scenarios require 2 to 8 candidates');
END;
CREATE TRIGGER trg_scenario_candidate_delete_downgrade
AFTER DELETE ON scenario_evaluation_candidates
FOR EACH ROW
BEGIN
  UPDATE scenario_evaluation_groups
  SET status = CASE
        WHEN status IN ('evaluating', 'decided')
          AND (SELECT COUNT(*) FROM scenario_evaluation_candidates WHERE scenario_id = OLD.scenario_id) < 2
        THEN 'draft'
        ELSE status
      END,
      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
  WHERE id = OLD.scenario_id AND account_id = OLD.account_id;
END;
CREATE INDEX idx_scenario_groups_account_status ON scenario_evaluation_groups(account_id, status, updated_at);
CREATE INDEX idx_scenario_candidates_account_scenario ON scenario_evaluation_candidates(account_id, scenario_id, sort_order);
CREATE UNIQUE INDEX idx_scenario_candidates_sort_order ON scenario_evaluation_candidates(scenario_id, sort_order);
CREATE TABLE scenario_comparison_criteria (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  name TEXT NOT NULL,
  normalized_name TEXT NOT NULL,
  criterion_type TEXT NOT NULL CHECK(criterion_type IN ('boolean','rating','text')),
  description TEXT NOT NULL DEFAULT '',
  importance TEXT NOT NULL DEFAULT 'important' CHECK(importance IN ('must','important','optional')),
  sort_order INTEGER NOT NULL CHECK(sort_order >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(scenario_id) REFERENCES scenario_evaluation_groups(id) ON DELETE CASCADE,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  UNIQUE(scenario_id, normalized_name),
  UNIQUE(scenario_id, sort_order)
);
CREATE TABLE scenario_comparison_cells (
  account_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  tool_id TEXT NOT NULL,
  criterion_id TEXT NOT NULL,
  fact_state TEXT NOT NULL DEFAULT 'unknown' CHECK(fact_state IN ('unknown','documented','observed','disputed')),
  value TEXT NOT NULL DEFAULT 'unknown',
  fox_judgment TEXT NOT NULL DEFAULT '',
  evidence_note TEXT NOT NULL DEFAULT '',
  evidence_repository_id TEXT,
  evidence_inbox_item_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY(scenario_id, tool_id, criterion_id),
  FOREIGN KEY(scenario_id, tool_id) REFERENCES scenario_evaluation_candidates(scenario_id, tool_id) ON DELETE CASCADE,
  FOREIGN KEY(criterion_id) REFERENCES scenario_comparison_criteria(id) ON DELETE CASCADE,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  CHECK((evidence_repository_id IS NULL) OR (evidence_inbox_item_id IS NULL))
);
CREATE TRIGGER trg_scenario_criterion_account_insert BEFORE INSERT ON scenario_comparison_criteria
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
BEGIN SELECT RAISE(ABORT, 'comparison criterion must share account'); END;
CREATE TRIGGER trg_scenario_criterion_account_update BEFORE UPDATE ON scenario_comparison_criteria
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
BEGIN SELECT RAISE(ABORT, 'comparison criterion must share account'); END;
CREATE TRIGGER trg_scenario_criterion_limit BEFORE INSERT ON scenario_comparison_criteria
FOR EACH ROW WHEN (SELECT COUNT(*) FROM scenario_comparison_criteria WHERE scenario_id=NEW.scenario_id)>=12
BEGIN SELECT RAISE(ABORT, 'scenario supports at most 12 comparison criteria'); END;
CREATE TRIGGER trg_scenario_criterion_archived_insert BEFORE INSERT ON scenario_comparison_criteria
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
BEGIN SELECT RAISE(ABORT, 'archived scenario is read only'); END;
CREATE TRIGGER trg_scenario_criterion_archived_update BEFORE UPDATE ON scenario_comparison_criteria
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
BEGIN SELECT RAISE(ABORT, 'archived scenario is read only'); END;
CREATE TRIGGER trg_scenario_criterion_type_with_cells BEFORE UPDATE OF criterion_type ON scenario_comparison_criteria
FOR EACH ROW WHEN NEW.criterion_type != OLD.criterion_type AND EXISTS(SELECT 1 FROM scenario_comparison_cells cell WHERE cell.criterion_id=OLD.id)
BEGIN SELECT RAISE(ABORT, 'criterion type cannot change while cells exist'); END;
CREATE TRIGGER trg_scenario_cell_scope_insert BEFORE INSERT ON scenario_comparison_cells
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
 OR NOT EXISTS(SELECT 1 FROM scenario_comparison_criteria c WHERE c.id=NEW.criterion_id AND c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id)
 OR (NEW.evidence_repository_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tool_repositories r WHERE r.tool_id=NEW.tool_id AND r.repository_id=NEW.evidence_repository_id AND r.account_id=NEW.account_id))
 OR (NEW.evidence_inbox_item_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tool_inbox_items i WHERE i.tool_id=NEW.tool_id AND i.inbox_item_id=NEW.evidence_inbox_item_id AND i.account_id=NEW.account_id))
BEGIN SELECT RAISE(ABORT, 'comparison cell must use owned scenario criterion and tool evidence'); END;
CREATE TRIGGER trg_scenario_cell_archived_insert BEFORE INSERT ON scenario_comparison_cells
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
BEGIN SELECT RAISE(ABORT, 'archived scenario is read only'); END;
CREATE TRIGGER trg_scenario_cell_archived_update BEFORE UPDATE OF fact_state,value,fox_judgment,evidence_note,criterion_id,account_id,tool_id,scenario_id ON scenario_comparison_cells
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
BEGIN SELECT RAISE(ABORT, 'archived scenario is read only'); END;
CREATE TRIGGER trg_scenario_cell_archived_repository_evidence BEFORE UPDATE OF evidence_repository_id ON scenario_comparison_cells
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
 AND NOT (OLD.evidence_repository_id IS NOT NULL AND NEW.evidence_repository_id IS NULL AND NEW.evidence_inbox_item_id IS OLD.evidence_inbox_item_id
  AND NOT EXISTS(SELECT 1 FROM tool_repositories r WHERE r.tool_id=OLD.tool_id AND r.repository_id=OLD.evidence_repository_id AND r.account_id=OLD.account_id))
BEGIN SELECT RAISE(ABORT, 'archived scenario evidence is read only'); END;
CREATE TRIGGER trg_scenario_cell_archived_inbox_evidence BEFORE UPDATE OF evidence_inbox_item_id ON scenario_comparison_cells
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
 AND NOT (OLD.evidence_inbox_item_id IS NOT NULL AND NEW.evidence_inbox_item_id IS NULL AND NEW.evidence_repository_id IS OLD.evidence_repository_id
  AND NOT EXISTS(SELECT 1 FROM tool_inbox_items i WHERE i.tool_id=OLD.tool_id AND i.inbox_item_id=OLD.evidence_inbox_item_id AND i.account_id=OLD.account_id))
BEGIN SELECT RAISE(ABORT, 'archived scenario evidence is read only'); END;
CREATE TRIGGER trg_scenario_cell_truth_insert BEFORE INSERT ON scenario_comparison_cells
FOR EACH ROW WHEN (NEW.fact_state='unknown' AND NEW.value!='unknown') OR (NEW.fact_state!='unknown' AND (NEW.value='unknown'
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='boolean' AND NEW.value NOT IN ('yes','no','partial'))
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='rating' AND NEW.value NOT IN ('1','2','3','4','5'))
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='text' AND (TRIM(NEW.value)='' OR NEW.value='unknown'))))
BEGIN SELECT RAISE(ABORT, 'comparison cell fact state and value conflict'); END;
CREATE TRIGGER trg_scenario_cell_truth_update BEFORE UPDATE OF fact_state,value,criterion_id ON scenario_comparison_cells
FOR EACH ROW WHEN (NEW.fact_state='unknown' AND NEW.value!='unknown') OR (NEW.fact_state!='unknown' AND (NEW.value='unknown'
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='boolean' AND NEW.value NOT IN ('yes','no','partial'))
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='rating' AND NEW.value NOT IN ('1','2','3','4','5'))
 OR ((SELECT criterion_type FROM scenario_comparison_criteria WHERE id=NEW.criterion_id)='text' AND (TRIM(NEW.value)='' OR NEW.value='unknown'))))
BEGIN SELECT RAISE(ABORT, 'comparison cell fact state and value conflict'); END;
CREATE TRIGGER trg_scenario_cell_clear_repository_evidence AFTER DELETE ON tool_repositories
FOR EACH ROW BEGIN UPDATE scenario_comparison_cells SET evidence_repository_id=NULL, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE tool_id=OLD.tool_id AND evidence_repository_id=OLD.repository_id AND account_id=OLD.account_id; END;
CREATE TRIGGER trg_scenario_cell_clear_inbox_evidence AFTER DELETE ON tool_inbox_items
FOR EACH ROW BEGIN UPDATE scenario_comparison_cells SET evidence_inbox_item_id=NULL, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE tool_id=OLD.tool_id AND evidence_inbox_item_id=OLD.inbox_item_id AND account_id=OLD.account_id; END;
CREATE TRIGGER trg_scenario_cell_scope_update BEFORE UPDATE ON scenario_comparison_cells
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
 OR NOT EXISTS(SELECT 1 FROM scenario_comparison_criteria c WHERE c.id=NEW.criterion_id AND c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id)
 OR (NEW.evidence_repository_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tool_repositories r WHERE r.tool_id=NEW.tool_id AND r.repository_id=NEW.evidence_repository_id AND r.account_id=NEW.account_id))
 OR (NEW.evidence_inbox_item_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM tool_inbox_items i WHERE i.tool_id=NEW.tool_id AND i.inbox_item_id=NEW.evidence_inbox_item_id AND i.account_id=NEW.account_id))
BEGIN SELECT RAISE(ABORT, 'comparison cell must use owned scenario criterion and tool evidence'); END;
CREATE INDEX idx_scenario_criteria_account ON scenario_comparison_criteria(account_id, scenario_id, sort_order);
CREATE INDEX idx_scenario_cells_account ON scenario_comparison_cells(account_id, scenario_id, tool_id);
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
CREATE TABLE scenario_decisions (
  id TEXT PRIMARY KEY,
  account_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  revision INTEGER NOT NULL,
  parent_decision_id TEXT,
  status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','confirmed','superseded')),
  chosen_tool_id TEXT,
  chosen_tool_name TEXT NOT NULL DEFAULT '',
  alternative_tool_id TEXT,
  alternative_tool_name TEXT NOT NULL DEFAULT '',
  why_chosen TEXT NOT NULL DEFAULT '',
  why_not_alternative TEXT NOT NULL DEFAULT '',
  evidence_summary TEXT NOT NULL DEFAULT '',
  decided_at TEXT,
  review_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  confirmed_at TEXT,
  superseded_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE,
  FOREIGN KEY(scenario_id) REFERENCES scenario_evaluation_groups(id) ON DELETE CASCADE,
  FOREIGN KEY(parent_decision_id) REFERENCES scenario_decisions(id) ON DELETE SET NULL,
  UNIQUE(account_id, scenario_id, revision)
);
CREATE TABLE scenario_decision_evidence (
  -- Every business column is an immutable, authoritative snapshot of one
  -- current comparison cell. Decision-specific manual reasoning belongs on
  -- scenario_decisions (why_* and evidence_summary), never in this table.
  id TEXT PRIMARY KEY,
  decision_id TEXT NOT NULL,
  account_id TEXT NOT NULL,
  scenario_id TEXT NOT NULL,
  tool_id TEXT,
  tool_name TEXT NOT NULL,
  criterion_id TEXT,
  criterion_name TEXT NOT NULL,
  fact_state TEXT NOT NULL,
  value TEXT NOT NULL,
  fox_judgment TEXT NOT NULL DEFAULT '',
  evidence_note TEXT NOT NULL DEFAULT '',
  source_kind TEXT,
  source_id TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY(decision_id) REFERENCES scenario_decisions(id) ON DELETE CASCADE,
  FOREIGN KEY(account_id) REFERENCES github_accounts(id) ON DELETE CASCADE
);
CREATE INDEX idx_scenario_decisions_current ON scenario_decisions(account_id, scenario_id, revision DESC);
CREATE INDEX idx_scenario_decision_evidence_decision ON scenario_decision_evidence(decision_id);
CREATE TRIGGER trg_scenario_decision_scope_insert BEFORE INSERT ON scenario_decisions
FOR EACH ROW WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
 OR EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
 OR (NEW.chosen_tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates c WHERE c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id AND c.tool_id=NEW.chosen_tool_id))
 OR (NEW.alternative_tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates c WHERE c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id AND c.tool_id=NEW.alternative_tool_id))
BEGIN SELECT RAISE(ABORT, 'decision must use an active owned scenario candidate'); END;
CREATE TRIGGER trg_scenario_decision_scope_update BEFORE UPDATE ON scenario_decisions
FOR EACH ROW WHEN EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.status='archived')
 OR NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups s WHERE s.id=NEW.scenario_id AND s.account_id=NEW.account_id)
 OR (NEW.chosen_tool_id IS NOT OLD.chosen_tool_id AND NEW.chosen_tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates c WHERE c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id AND c.tool_id=NEW.chosen_tool_id))
 OR (NEW.alternative_tool_id IS NOT OLD.alternative_tool_id AND NEW.alternative_tool_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates c WHERE c.scenario_id=NEW.scenario_id AND c.account_id=NEW.account_id AND c.tool_id=NEW.alternative_tool_id))
BEGIN SELECT RAISE(ABORT, 'decision must use an active owned scenario candidate'); END;
CREATE TRIGGER trg_scenario_decision_parent_insert BEFORE INSERT ON scenario_decisions
FOR EACH ROW WHEN
  NEW.status<>'draft'
  OR (NEW.parent_decision_id IS NULL AND EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed'))
  OR (NEW.parent_decision_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.id=NEW.parent_decision_id AND current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed'))
BEGIN SELECT RAISE(ABORT, 'decision insert must be a draft with the current confirmed parent'); END;
CREATE TRIGGER trg_scenario_decision_parent_update BEFORE UPDATE ON scenario_decisions
FOR EACH ROW WHEN NEW.status='draft' AND
  ((NEW.parent_decision_id IS NULL AND EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed')) OR (NEW.parent_decision_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.id=NEW.parent_decision_id AND current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed')))
BEGIN SELECT RAISE(ABORT, 'decision revision parent must match the current confirmed conclusion'); END;
CREATE TRIGGER trg_scenario_decision_state_update BEFORE UPDATE OF status ON scenario_decisions
FOR EACH ROW WHEN
  (OLD.status='draft' AND NEW.status NOT IN ('draft','confirmed'))
  OR (OLD.status='confirmed' AND NEW.status<>'superseded')
  OR OLD.status='superseded'
BEGIN SELECT RAISE(ABORT, 'decision status transition is not allowed'); END;
CREATE TRIGGER trg_scenario_decision_confirm_transition BEFORE UPDATE OF status ON scenario_decisions
FOR EACH ROW WHEN OLD.status='draft' AND NEW.status='confirmed'
BEGIN
  SELECT CASE WHEN NEW.id IS NOT OLD.id OR NEW.account_id IS NOT OLD.account_id OR NEW.scenario_id IS NOT OLD.scenario_id OR NEW.revision IS NOT OLD.revision OR NEW.parent_decision_id IS NOT OLD.parent_decision_id OR NEW.chosen_tool_id IS NOT OLD.chosen_tool_id OR NEW.chosen_tool_name IS NOT OLD.chosen_tool_name OR NEW.alternative_tool_id IS NOT OLD.alternative_tool_id OR NEW.alternative_tool_name IS NOT OLD.alternative_tool_name OR NEW.why_chosen IS NOT OLD.why_chosen OR NEW.why_not_alternative IS NOT OLD.why_not_alternative OR NEW.evidence_summary IS NOT OLD.evidence_summary OR NEW.decided_at IS NOT OLD.decided_at OR NEW.review_at IS NOT OLD.review_at OR NEW.created_at IS NOT OLD.created_at OR NEW.superseded_at IS NOT OLD.superseded_at THEN RAISE(ABORT, 'decision confirmation may not edit draft fields') END;
  SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM scenario_evaluation_groups scenario WHERE scenario.id=NEW.scenario_id AND scenario.account_id=NEW.account_id AND scenario.status<>'archived') THEN RAISE(ABORT, 'decision confirmation scenario is unavailable') END;
  SELECT CASE WHEN NEW.chosen_tool_id IS NULL OR NEW.chosen_tool_name='' OR NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates candidate JOIN tools tool ON tool.id=candidate.tool_id AND tool.account_id=candidate.account_id WHERE candidate.scenario_id=NEW.scenario_id AND candidate.account_id=NEW.account_id AND candidate.tool_id=NEW.chosen_tool_id AND tool.name=NEW.chosen_tool_name) THEN RAISE(ABORT, 'decision confirmation requires the current chosen tool') END;
  SELECT CASE WHEN (NEW.alternative_tool_id IS NULL AND NEW.alternative_tool_name<>'') OR (NEW.alternative_tool_id IS NOT NULL AND (NEW.alternative_tool_id=NEW.chosen_tool_id OR NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates candidate JOIN tools tool ON tool.id=candidate.tool_id AND tool.account_id=candidate.account_id WHERE candidate.scenario_id=NEW.scenario_id AND candidate.account_id=NEW.account_id AND candidate.tool_id=NEW.alternative_tool_id AND tool.name=NEW.alternative_tool_name))) THEN RAISE(ABORT, 'decision confirmation alternative is invalid') END;
  SELECT CASE WHEN trim(NEW.why_chosen)='' OR NEW.decided_at IS NULL OR NEW.decided_at NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' OR NEW.confirmed_at IS NULL THEN RAISE(ABORT, 'decision confirmation requires a reason, date and confirmation time') END;
  SELECT CASE WHEN NEW.parent_decision_id IS NULL AND EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed') THEN RAISE(ABORT, 'decision revision parent must match the current confirmed conclusion') END;
  SELECT CASE WHEN NEW.parent_decision_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM scenario_decisions current WHERE current.id=NEW.parent_decision_id AND current.account_id=NEW.account_id AND current.scenario_id=NEW.scenario_id AND current.status='confirmed') THEN RAISE(ABORT, 'decision revision parent must match the current confirmed conclusion') END;
  SELECT CASE WHEN EXISTS(SELECT 1 FROM scenario_decision_evidence evidence WHERE evidence.decision_id=NEW.id AND NOT EXISTS(SELECT 1 FROM scenario_evaluation_candidates candidate JOIN tools tool ON tool.id=evidence.tool_id AND tool.account_id=candidate.account_id JOIN scenario_comparison_criteria criterion ON criterion.id=evidence.criterion_id AND criterion.account_id=candidate.account_id AND criterion.scenario_id=candidate.scenario_id JOIN scenario_comparison_cells cell ON cell.account_id=candidate.account_id AND cell.scenario_id=candidate.scenario_id AND cell.tool_id=evidence.tool_id AND cell.criterion_id=evidence.criterion_id WHERE candidate.account_id=NEW.account_id AND candidate.scenario_id=NEW.scenario_id AND evidence.account_id=NEW.account_id AND evidence.scenario_id=NEW.scenario_id AND evidence.tool_name=tool.name AND evidence.criterion_name=criterion.name AND evidence.fact_state=cell.fact_state AND evidence.value=cell.value AND evidence.fox_judgment=cell.fox_judgment AND evidence.evidence_note=cell.evidence_note AND ((cell.evidence_repository_id IS NOT NULL AND evidence.source_kind='repository' AND evidence.source_id=cell.evidence_repository_id AND EXISTS(SELECT 1 FROM tool_repositories source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.repository_id=cell.evidence_repository_id)) OR (cell.evidence_inbox_item_id IS NOT NULL AND evidence.source_kind='inbox' AND evidence.source_id=cell.evidence_inbox_item_id AND EXISTS(SELECT 1 FROM tool_inbox_items source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.inbox_item_id=cell.evidence_inbox_item_id)) OR (cell.evidence_repository_id IS NULL AND cell.evidence_inbox_item_id IS NULL AND evidence.source_kind IS NULL AND evidence.source_id IS NULL)))) THEN RAISE(ABORT, 'decision confirmation evidence is stale') END;
END;
CREATE TRIGGER trg_scenario_decision_supersede_parent_after_confirm AFTER UPDATE OF status ON scenario_decisions
FOR EACH ROW WHEN OLD.status='draft' AND NEW.status='confirmed' AND NEW.parent_decision_id IS NOT NULL
BEGIN
  UPDATE scenario_decisions SET status='superseded', superseded_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=NEW.parent_decision_id AND account_id=NEW.account_id AND scenario_id=NEW.scenario_id AND status='confirmed';
END;
CREATE TRIGGER trg_scenario_decision_confirmed_immutable BEFORE UPDATE ON scenario_decisions
FOR EACH ROW WHEN OLD.status='confirmed' AND NOT (NEW.status='superseded' AND NEW.id IS OLD.id AND NEW.account_id IS OLD.account_id AND NEW.scenario_id IS OLD.scenario_id AND NEW.revision IS OLD.revision AND NEW.parent_decision_id IS OLD.parent_decision_id AND NEW.chosen_tool_id IS OLD.chosen_tool_id AND NEW.chosen_tool_name IS OLD.chosen_tool_name AND NEW.alternative_tool_id IS OLD.alternative_tool_id AND NEW.alternative_tool_name IS OLD.alternative_tool_name AND NEW.why_chosen IS OLD.why_chosen AND NEW.why_not_alternative IS OLD.why_not_alternative AND NEW.evidence_summary IS OLD.evidence_summary AND NEW.decided_at IS OLD.decided_at AND NEW.review_at IS OLD.review_at AND NEW.created_at IS OLD.created_at AND NEW.confirmed_at IS OLD.confirmed_at AND NEW.superseded_at IS NOT NULL AND EXISTS(SELECT 1 FROM scenario_decisions child WHERE child.parent_decision_id=OLD.id AND child.account_id=OLD.account_id AND child.scenario_id=OLD.scenario_id AND child.status='confirmed'))
BEGIN SELECT RAISE(ABORT, 'confirmed decision is immutable; create a new revision'); END;
CREATE TRIGGER trg_scenario_decision_superseded_immutable BEFORE UPDATE ON scenario_decisions
FOR EACH ROW WHEN OLD.status='superseded'
BEGIN SELECT RAISE(ABORT, 'superseded decision is immutable'); END;
CREATE TRIGGER trg_scenario_decision_evidence_scope_insert BEFORE INSERT ON scenario_decision_evidence
FOR EACH ROW WHEN NOT EXISTS(
  SELECT 1 FROM scenario_decisions d
  JOIN scenario_evaluation_candidates candidate ON candidate.scenario_id=d.scenario_id AND candidate.account_id=d.account_id AND candidate.tool_id=NEW.tool_id
  JOIN tools tool ON tool.id=NEW.tool_id AND tool.account_id=d.account_id
  JOIN scenario_comparison_criteria criterion ON criterion.id=NEW.criterion_id AND criterion.scenario_id=d.scenario_id AND criterion.account_id=d.account_id
  JOIN scenario_comparison_cells cell ON cell.account_id=d.account_id AND cell.scenario_id=d.scenario_id AND cell.tool_id=NEW.tool_id AND cell.criterion_id=NEW.criterion_id
  WHERE d.id=NEW.decision_id AND d.account_id=NEW.account_id AND d.scenario_id=NEW.scenario_id AND d.status='draft'
    AND NEW.tool_name=tool.name AND NEW.criterion_name=criterion.name
    AND NEW.fact_state=cell.fact_state AND NEW.value=cell.value
    AND NEW.fox_judgment=cell.fox_judgment AND NEW.evidence_note=cell.evidence_note
    AND (
      (cell.evidence_repository_id IS NOT NULL AND NEW.source_kind='repository' AND NEW.source_id=cell.evidence_repository_id AND EXISTS(SELECT 1 FROM tool_repositories source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.repository_id=cell.evidence_repository_id))
      OR (cell.evidence_inbox_item_id IS NOT NULL AND NEW.source_kind='inbox' AND NEW.source_id=cell.evidence_inbox_item_id AND EXISTS(SELECT 1 FROM tool_inbox_items source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.inbox_item_id=cell.evidence_inbox_item_id))
      OR (cell.evidence_repository_id IS NULL AND cell.evidence_inbox_item_id IS NULL AND NEW.source_kind IS NULL AND NEW.source_id IS NULL)
    )
)
BEGIN SELECT RAISE(ABORT, 'decision evidence must exactly snapshot a current owned comparison cell'); END;
CREATE TRIGGER trg_scenario_decision_evidence_scope_update BEFORE UPDATE ON scenario_decision_evidence
FOR EACH ROW WHEN NOT EXISTS(
  SELECT 1 FROM scenario_decisions d
  JOIN scenario_evaluation_candidates candidate ON candidate.scenario_id=d.scenario_id AND candidate.account_id=d.account_id AND candidate.tool_id=NEW.tool_id
  JOIN tools tool ON tool.id=NEW.tool_id AND tool.account_id=d.account_id
  JOIN scenario_comparison_criteria criterion ON criterion.id=NEW.criterion_id AND criterion.scenario_id=d.scenario_id AND criterion.account_id=d.account_id
  JOIN scenario_comparison_cells cell ON cell.account_id=d.account_id AND cell.scenario_id=d.scenario_id AND cell.tool_id=NEW.tool_id AND cell.criterion_id=NEW.criterion_id
  WHERE d.id=NEW.decision_id AND d.account_id=NEW.account_id AND d.scenario_id=NEW.scenario_id AND d.status='draft'
    AND NEW.tool_name=tool.name AND NEW.criterion_name=criterion.name
    AND NEW.fact_state=cell.fact_state AND NEW.value=cell.value
    AND NEW.fox_judgment=cell.fox_judgment AND NEW.evidence_note=cell.evidence_note
    AND (
      (cell.evidence_repository_id IS NOT NULL AND NEW.source_kind='repository' AND NEW.source_id=cell.evidence_repository_id AND EXISTS(SELECT 1 FROM tool_repositories source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.repository_id=cell.evidence_repository_id))
      OR (cell.evidence_inbox_item_id IS NOT NULL AND NEW.source_kind='inbox' AND NEW.source_id=cell.evidence_inbox_item_id AND EXISTS(SELECT 1 FROM tool_inbox_items source WHERE source.tool_id=cell.tool_id AND source.account_id=cell.account_id AND source.inbox_item_id=cell.evidence_inbox_item_id))
      OR (cell.evidence_repository_id IS NULL AND cell.evidence_inbox_item_id IS NULL AND NEW.source_kind IS NULL AND NEW.source_id IS NULL)
    )
)
BEGIN SELECT RAISE(ABORT, 'decision evidence must exactly snapshot a current owned comparison cell'); END;
