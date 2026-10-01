-- Fox Stars Lab migration 008 inbox_document_cleanup
-- Extracted best-effort from gsat-desktop 1.4.0-fox.1 (commit c352222) string table @0xed4cd1
-- Verify against schema/live_schema_018.sql before use.

PRAGMA foreign_keys = ON;

-- 007 prevents new stale writes. 006-era databases may already contain a
-- document whose card was edited before that gate existed, so clean only rows
-- that cannot still describe their current card.
DELETE FROM inbox_documents
WHERE NOT EXISTS (
  SELECT 1 FROM inbox_items
  WHERE inbox_items.id = inbox_documents.inbox_item_id
    AND inbox_items.account_id = inbox_documents.account_id
    AND inbox_items.normalized_url = inbox_documents.requested_url
    AND inbox_items.item_type = 'webpage'
);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES ('008', 'inbox_document_cleanup');
