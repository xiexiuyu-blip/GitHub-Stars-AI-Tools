-- 037: 单张重做的花钱授权。只写在这一张任务行上，到期之前认领这一行可以不看每天额度。
-- 可空：已有的行都是 NULL，等于没授权。不进控制文件，也不是全局开关。
ALTER TABLE ai_jobs ADD COLUMN spend_authorized_until TEXT;
INSERT INTO schema_migrations(version, name) VALUES('037', 'ai_job_spend_authorization');
