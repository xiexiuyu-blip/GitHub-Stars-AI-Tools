-- PROPOSED 024: 结构化“理解卡”（给人看 + 给 AI 用），挂在 repo_ai_documents；工具卡加 AI 字段（评审 P0-1）。
ALTER TABLE repo_ai_documents ADD COLUMN card_json TEXT;
ALTER TABLE repo_ai_documents ADD COLUMN card_schema_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE repo_ai_documents ADD COLUMN one_liner_zh TEXT NOT NULL DEFAULT '';
ALTER TABLE tools ADD COLUMN origin TEXT NOT NULL DEFAULT 'manual' CHECK (origin IN ('manual','ai_cluster','registry','local_capability'));
ALTER TABLE tools ADD COLUMN ai_card_json TEXT;
ALTER TABLE tools ADD COLUMN ai_card_source_hash TEXT;
INSERT INTO schema_migrations(version, name) VALUES('024', 'ai_cards');
