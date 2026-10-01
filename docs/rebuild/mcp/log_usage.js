// log_usage 写回。只插入 usage_log，不改 web-search 正在读的表。
// 默认拒绝写入。必须设置 FOX_STARS_LAB_ALLOW_WRITE=1。
import { DatabaseSync } from "node:sqlite";

export function logUsage(dbPath, accountId, input = {}) {
  if (process.env.FOX_STARS_LAB_ALLOW_WRITE !== "1") {
    throw new Error("log_usage 默认关闭。确认要写回时设置 FOX_STARS_LAB_ALLOW_WRITE=1");
  }
  const verdict = String(input.verdict || "");
  if (!["useful", "meh", "useless"].includes(verdict)) {
    throw new Error("verdict 只能是 useful、meh 或 useless");
  }
  const entityId = String(input.entityId || input.entity_id || "").trim();
  if (!entityId) throw new Error("缺少 entityId");
  const note = String(input.note || "").slice(0, 4000);
  const db = new DatabaseSync(dbPath, { timeout: 1500 });
  try {
    db.exec("PRAGMA foreign_keys=ON");
    db.prepare(
      "INSERT INTO usage_log(id, account_id, entity_type, entity_id, verdict, note, via, client_name) VALUES (?, ?, 'repository', ?, ?, ?, 'mcp', ?)"
    ).run(`mcp-${Date.now()}`, accountId, entityId, verdict, note, String(input.clientName || "mcp").slice(0, 80));
    return { ok: true, via: "mcp" };
  } finally {
    db.close();
  }
}
