// dist/db.js — extracted from Fox Stars Lab.app/Contents/Resources/mcp/server.mjs (1.4.0-fox.1 bundle)
import fs from "node:fs";
import { DatabaseSync } from "node:sqlite";

var REQUIRED = {
  github_accounts: ["id"],
  tools: [
    "id",
    "account_id",
    "name",
    "summary",
    "selection_status",
    "manual_note"
  ],
  tool_forms: ["tool_id", "form_kind"],
  tool_repositories: ["tool_id", "repository_id", "account_id"],
  tool_inbox_items: ["tool_id", "inbox_item_id", "account_id"],
  repositories: [
    "id",
    "account_id",
    "name",
    "full_name",
    "description",
    "html_url"
  ],
  repo_readmes: ["repo_id", "raw_markdown"],
  repo_ai_documents: ["repo_id", "summary_zh"],
  inbox_items: ["id", "account_id", "title", "original_url", "item_type"],
  inbox_documents: ["inbox_item_id", "account_id", "content_text"],
  inbox_local_documents: [
    "inbox_item_id",
    "account_id",
    "file_name",
    "document_kind",
    "content_text"
  ],
  tool_registry_entries: [
    "account_id",
    "tool_id",
    "source_name",
    "source_purpose"
  ],
  local_capability_entries: [
    "account_id",
    "tool_id",
    "capability_kind",
    "source_name",
    "source_description"
  ]
};
function openDb(dbPath, account) {
  try {
    if (!fs.statSync(dbPath).isFile())
      throw new Error("not-file");
  } catch {
    throw new PublicError("db_unavailable", "\u6570\u636E\u5E93\u4E0D\u53EF\u7528");
  }
  if (!account.trim())
    throw new PublicError("invalid_config", "account \u672A\u914D\u7F6E");
  let db;
  try {
    db = new DatabaseSync(dbPath, { readOnly: true, timeout: 1500 });
    db.exec("PRAGMA query_only=ON; PRAGMA busy_timeout=1500");
    const latestMigration = db.prepare("SELECT version FROM schema_migrations ORDER BY CAST(version AS INTEGER) DESC LIMIT 1").get();
    const schema = String(latestMigration?.version ?? "").trim();
    if (!/^\d{3,}$/.test(schema)) {
      throw new PublicError("incompatible_schema", "\u6570\u636E\u5E93\u7248\u672C\u4E0D\u517C\u5BB9");
    }
    for (const [table, columns] of Object.entries(REQUIRED)) {
      const present = new Set(db.prepare(`PRAGMA table_info(${table})`).all().map((row) => row.name));
      if (columns.some((column) => !present.has(column))) {
        throw new PublicError("incompatible_schema", "\u6570\u636E\u5E93\u7ED3\u6784\u4E0D\u517C\u5BB9");
      }
    }
    if (!db.prepare("SELECT 1 FROM github_accounts WHERE id=?").get(account)) {
      throw new PublicError("account_not_found", "\u8D26\u53F7\u4E0D\u5B58\u5728");
    }
    const wal = String(db.prepare("PRAGMA journal_mode").get().journal_mode).toLowerCase() === "wal";
    return { db, account, schema, wal, close: () => db?.close() };
  } catch (error2) {
    db?.close();
    if (error2 instanceof PublicError)
      throw error2;
    throw new PublicError("db_unavailable", "\u6570\u636E\u5E93\u4E0D\u53EF\u7528\u6216\u6B63\u5FD9");
  }
}


// ===== (bundled zod / @modelcontextprotocol/sdk omitted) =====
