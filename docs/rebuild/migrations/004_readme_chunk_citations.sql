-- Fox Stars Lab migration 004 readme_chunk_citations
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xee2796
-- Verify against schema/live_schema_018.sql before use.

-- AI derivatives are replaceable.  Coverage describes exactly which README
-- revision was analysed, while citations are derived server-side evidence.
ALTER TABLE repo_ai_documents ADD COLUMN summary_mode TEXT NOT NULL DEFAULT 'legacy';
ALTER TABLE repo_ai_documents ADD COLUMN coverage_status TEXT NOT NULL DEFAULT 'legacy';
ALTER TABLE repo_ai_documents ADD COLUMN source_char_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE repo_ai_documents ADD COLUMN processed_char_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE repo_ai_documents ADD COLUMN chunk_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE repo_ai_documents ADD COLUMN citation_count INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS repo_ai_citations (
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

CREATE INDEX IF NOT EXISTS idx_repo_ai_citations_current ON repo_ai_citations(repo_id, source_hash, citation_order);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('004', 'readme_chunk_citations');
