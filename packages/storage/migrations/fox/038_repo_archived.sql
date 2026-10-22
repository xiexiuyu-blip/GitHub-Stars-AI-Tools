-- 038: 仓库是否已在 GitHub 上归档。来自 GitHub Stars 的 archived，同步时写入。
-- 一列可空、无默认值：NULL = 不知道，0 = 没归档，1 = 已归档。旧行保持 NULL，下次同步才补。
ALTER TABLE repositories ADD COLUMN archived INTEGER;
INSERT INTO schema_migrations(version, name) VALUES('038', 'repo_archived');
