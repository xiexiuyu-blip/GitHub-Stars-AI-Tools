-- Fox Stars Lab migration 007 inbox_document_write_gate
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xed4cd1
-- Verify against schema/live_schema_018.sql before use.

PRAGMA foreign_keys = ON;

-- 006 established account ownership. 007 adds an atomic freshness gate for
-- fetches that complete after the user has edited the source card.
CREATE TRIGGER IF NOT EXISTS trg_inbox_documents_source_insert
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

CREATE TRIGGER IF NOT EXISTS trg_inbox_documents_source_update
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

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('007', 'inbox_document_write_gate');
