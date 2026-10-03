-- 032: 仓库许可证。来自 GitHub Stars 的 license，同步时写入。
-- 两列可空、无默认值。旧行保持 NULL，下次同步才补。
ALTER TABLE repositories ADD COLUMN license_spdx TEXT;
ALTER TABLE repositories ADD COLUMN license_name TEXT;
INSERT INTO schema_migrations(version, name) VALUES('032', 'repo_license');
