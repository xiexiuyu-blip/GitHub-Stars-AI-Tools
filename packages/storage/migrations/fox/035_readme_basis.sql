-- 035: 理解卡记下生成时读过的 README。只追加列，不改已有列。
-- readme_hash：最近一次「评估过」的 README 的 content_hash（生成这张卡时读到的，或之后评估为「改动不明显」的）。NULL = 生成这张卡时没有 README / 还不知道。
-- readme_basis：生成这张卡时喂给模型的 README 摘录。NULL = 没有 README，或旧卡还没记（SQL 里算不出摘录，留给后面惰性补）。
ALTER TABLE repo_ai_documents ADD COLUMN readme_hash TEXT;
ALTER TABLE repo_ai_documents ADD COLUMN readme_basis TEXT;
-- 旧卡回填：README 是在这张卡生成之前抓的（fetched_at <= generated_at），就认为这张卡读的就是它。
UPDATE repo_ai_documents SET readme_hash = (
  SELECT content_hash FROM repo_readmes
  WHERE repo_readmes.repo_id = repo_ai_documents.repo_id AND repo_readmes.fetched_at <= repo_ai_documents.generated_at
) WHERE IFNULL(trim(card_json), '') <> '';
INSERT INTO schema_migrations(version, name) VALUES('035', 'readme_basis');
