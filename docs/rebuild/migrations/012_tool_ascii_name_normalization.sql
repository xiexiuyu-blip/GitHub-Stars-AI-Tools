-- Fox Stars Lab migration 012 tool_ascii_name_normalization
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xee9e1c
-- Verify against schema/live_schema_018.sql before use.

-- SQLite lower() is ASCII-only. Keep Rust and SQL normalization aligned:
-- ASCII is case-insensitive; non-ASCII characters are exact.
DROP TRIGGER IF EXISTS trg_tools_normalized_name_insert;
DROP TRIGGER IF EXISTS trg_tools_normalized_name_update;
UPDATE tools SET normalized_name = lower(trim(name));
CREATE TRIGGER trg_tools_normalized_name_insert
BEFORE INSERT ON tools
FOR EACH ROW WHEN NEW.normalized_name != lower(trim(NEW.name))
BEGIN SELECT RAISE(ABORT, 'tool normalized name must match ASCII-normalized name'); END;
CREATE TRIGGER trg_tools_normalized_name_update
BEFORE UPDATE OF name, normalized_name ON tools
FOR EACH ROW WHEN NEW.normalized_name != lower(trim(NEW.name))
BEGIN SELECT RAISE(ABORT, 'tool normalized name must match ASCII-normalized name'); END;
INSERT INTO schema_migrations(version, name) VALUES ('012', 'tool_ascii_name_normalization');
