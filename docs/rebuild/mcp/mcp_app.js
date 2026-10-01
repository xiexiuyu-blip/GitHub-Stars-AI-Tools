// dist/db.js
import fs from "node:fs";
import { DatabaseSync } from "node:sqlite";

// dist/safe.js
import path from "node:path";
var MAX_EXCERPT = 800;
var MAX_RESPONSE = 32 * 1024;
var MAX_QUERY = 500;
var SECRET_KEY = /^(?:api[_-]?key|token(?:_ref)?|authorization|cookie|password|secret|headers|command|args|env)$/i;
var CREDENTIAL_ASSIGNMENT = /\b(api[_-]?key|access[_-]?token|refresh[_-]?token|token|password|passwd|pwd|cookie|authorization)\s*[:=]\s*(?:Basic\s+|Bearer\s+)?[^\s,;]+/gi;
var AUTH_HEADER = /\b(?:Authorization|Cookie)\s*:\s*[^\r\n]+/gi;
var PREFIXED_CREDENTIAL = /\b(?:sk-[A-Za-z0-9_-]{8,}|ghp_[A-Za-z0-9_-]{8,}|github_pat_[A-Za-z0-9_-]{8,}|xox[baprs]-[A-Za-z0-9_-]{8,})\b/gi;
var POSIX_ABSOLUTE_PATH = /(^|[\s('"=])\/(?:Users|home|private|var|tmp|Volumes|opt|etc)\/(?:[^\s)'",;]+\/?)+/g;
var WINDOWS_ABSOLUTE_PATH = /\b[A-Za-z]:\\(?:[^\\\s]+\\)+[^\s,;]+/g;
var CONTROL = /[\u0000-\u0008\u000b\u000c\u000e-\u001f]/g;
var PublicError = class extends Error {
  code;
  constructor(code, message) {
    super(message);
    this.code = code;
  }
};
function codePointSlice(value, max) {
  return Array.from(value).slice(0, max).join("");
}
function redactText(value) {
  return String(value ?? "").replace(AUTH_HEADER, "[credential redacted]").replace(CREDENTIAL_ASSIGNMENT, "$1=[redacted]").replace(PREFIXED_CREDENTIAL, "[credential redacted]").replace(POSIX_ABSOLUTE_PATH, "$1[local-path]").replace(WINDOWS_ABSOLUTE_PATH, "[local-path]").replace(CONTROL, " ");
}
function text(value, max = MAX_EXCERPT) {
  return codePointSlice(redactText(value), max);
}
function safeObject(value) {
  if (Array.isArray(value))
    return value.map(safeObject);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).filter(([key]) => !SECRET_KEY.test(key)).map(([key, item]) => [key, safeObject(item)]));
  }
  return typeof value === "string" ? text(value, 4e3) : value;
}
function safeUrl(value) {
  try {
    const url = new URL(String(value));
    if (!["http:", "https:"].includes(url.protocol))
      return void 0;
    url.username = "";
    url.password = "";
    for (const key of [...url.searchParams.keys()]) {
      if (/token|key|secret|auth|password|signature|cookie/i.test(key)) {
        url.searchParams.delete(key);
      }
    }
    url.hash = "";
    if (/\b(?:token|secret|password|api[_-]?key)\b/i.test(decodeURIComponent(url.pathname))) {
      url.pathname = "/[sensitive-path]";
    }
    return url.toString();
  } catch {
    return void 0;
  }
}
function localLabel(value) {
  const name = path.basename(String(value ?? "").replace(/\\/g, "/"));
  return text(name || "local-document", 200);
}
function query(value) {
  const result = String(value ?? "").trim();
  if (!result || Array.from(result).length > MAX_QUERY) {
    throw new PublicError("invalid_input", "query \u5FC5\u987B\u4E3A 1..500 \u4E2A\u5B57\u7B26");
  }
  return result;
}
function limit(value, max) {
  const result = Number(value ?? Math.min(10, max));
  if (!Number.isInteger(result) || result < 1 || result > max) {
    throw new PublicError("invalid_input", `limit \u5FC5\u987B\u4E3A 1..${max}`);
  }
  return result;
}
function like(value) {
  return `%${value.replace(/\\/g, "\\\\").replace(/%/g, "\\%").replace(/_/g, "\\_")}%`;
}
function excerptAroundMatch(body, needle) {
  const clean = redactText(body);
  const points = Array.from(clean);
  const index = clean.toLocaleLowerCase().indexOf(needle.toLocaleLowerCase());
  if (index < 0)
    return codePointSlice(clean, MAX_EXCERPT);
  const before = Array.from(clean.slice(0, index)).length;
  const start = Math.max(0, before - 240);
  return points.slice(start, start + MAX_EXCERPT).join("");
}
function budget(value) {
  const clean = safeObject(value);
  if (Buffer.byteLength(JSON.stringify(clean)) <= MAX_RESPONSE)
    return clean;
  const arrays = [];
  const visit = (item) => {
    if (Array.isArray(item)) {
      arrays.push(item);
      item.forEach(visit);
    } else if (item && typeof item === "object") {
      Object.values(item).forEach(visit);
    }
  };
  visit(clean);
  while (Buffer.byteLength(JSON.stringify(clean)) > MAX_RESPONSE) {
    const target = arrays.find((array2) => array2.length > 0);
    if (!target)
      break;
    target.pop();
    clean.truncated = true;
  }
  return clean;
}

// dist/db.js
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
// dist/core.js
var FORMS = [
  "app",
  "cli",
  "skill",
  "mcp",
  "api",
  "library",
  "service",
  "other"
];
var STATUSES = [
  "unread",
  "want_to_try",
  "tried",
  "in_use",
  "watching",
  "deprecated",
  "later",
  "read"
];
var KINDS = ["repo", "web", "local", "pdf", "registry", "skill", "mcp"];
var trust = "untrusted_user_collected_content";
function enums(v, allowed, name) {
  if (v === void 0)
    return [];
  if (!Array.isArray(v) || v.some((x) => !allowed.includes(String(x))))
    throw new PublicError("invalid_input", `${name} \u975E\u6CD5`);
  return v.map(String);
}
function refs(ctx, toolId) {
  const out = [];
  for (const r of ctx.db.prepare("SELECT r.full_name,r.html_url FROM tool_repositories tr JOIN repositories r ON r.id=tr.repository_id WHERE tr.account_id=? AND tr.tool_id=?").all(ctx.account, toolId))
    out.push({
      kind: "repo",
      label: text(r.full_name, 200),
      url: safeUrl(r.html_url)
    });
  for (const r of ctx.db.prepare("SELECT i.item_type,i.title,i.original_url,l.file_name,l.document_kind FROM tool_inbox_items ti JOIN inbox_items i ON i.id=ti.inbox_item_id LEFT JOIN inbox_local_documents l ON l.inbox_item_id=i.id WHERE ti.account_id=? AND ti.tool_id=?").all(ctx.account, toolId))
    out.push(r.file_name ? {
      kind: r.document_kind === "pdf" ? "pdf" : "local",
      label: localLabel(r.file_name)
    } : {
      kind: "web",
      label: text(r.title, 200),
      url: safeUrl(r.original_url)
    });
  for (const r of ctx.db.prepare("SELECT source_name FROM tool_registry_entries WHERE account_id=? AND tool_id=?").all(ctx.account, toolId))
    out.push({ kind: "registry", label: text(r.source_name, 200) });
  for (const r of ctx.db.prepare("SELECT capability_kind,source_name FROM local_capability_entries WHERE account_id=? AND tool_id=?").all(ctx.account, toolId))
    out.push({ kind: r.capability_kind, label: text(r.source_name, 200) });
  return out;
}
function searchTools(ctx, input = {}) {
  const q = query(input.query), l = limit(input.limit, 20), forms = enums(input.forms, FORMS, "forms"), statuses = enums(input.statuses, STATUSES, "statuses"), notes = process.env.FOX_STARS_LAB_INCLUDE_NOTES === "1", p = like(q);
  const rows = ctx.db.prepare(`SELECT DISTINCT t.id,t.name,t.summary,t.selection_status,group_concat(DISTINCT f.form_kind) forms FROM tools t LEFT JOIN tool_forms f ON f.tool_id=t.id LEFT JOIN tool_repositories tr ON tr.tool_id=t.id AND tr.account_id=t.account_id LEFT JOIN repositories r ON r.id=tr.repository_id LEFT JOIN repo_readmes rr ON rr.repo_id=r.id LEFT JOIN repo_ai_documents ad ON ad.repo_id=r.id LEFT JOIN tool_inbox_items ti ON ti.tool_id=t.id AND ti.account_id=t.account_id LEFT JOIN inbox_items i ON i.id=ti.inbox_item_id LEFT JOIN inbox_documents d ON d.inbox_item_id=i.id LEFT JOIN inbox_local_documents ld ON ld.inbox_item_id=i.id LEFT JOIN tool_registry_entries re ON re.tool_id=t.id AND re.account_id=t.account_id LEFT JOIN local_capability_entries ce ON ce.tool_id=t.id AND ce.account_id=t.account_id WHERE t.account_id=? AND (?=0 OR EXISTS (SELECT 1 FROM tool_forms ff WHERE ff.tool_id=t.id AND ff.form_kind IN (${FORMS.map(() => "?").join(",")}))) AND (?=0 OR t.selection_status IN (${STATUSES.map(() => "?").join(",")})) AND (t.name LIKE ? ESCAPE '\\' OR t.summary LIKE ? ESCAPE '\\' ${notes ? "OR t.manual_note LIKE ? ESCAPE '\\'" : ""} OR r.full_name LIKE ? ESCAPE '\\' OR r.description LIKE ? ESCAPE '\\' OR rr.raw_markdown LIKE ? ESCAPE '\\' OR ad.summary_zh LIKE ? ESCAPE '\\' OR i.title LIKE ? ESCAPE '\\' OR d.content_text LIKE ? ESCAPE '\\' OR ld.content_text LIKE ? ESCAPE '\\' OR re.source_name LIKE ? ESCAPE '\\' OR re.source_purpose LIKE ? ESCAPE '\\' OR ce.source_name LIKE ? ESCAPE '\\' OR ce.source_description LIKE ? ESCAPE '\\') GROUP BY t.id ORDER BY CASE WHEN lower(t.name)=lower(?) THEN 0 WHEN t.name LIKE ? ESCAPE '\\' THEN 1 ELSE 2 END,t.name,t.id LIMIT ?`);
  const fpad = [...forms, ...Array(FORMS.length - forms.length).fill("")], spad = [...statuses, ...Array(STATUSES.length - statuses.length).fill("")], ps = Array(notes ? 14 : 13).fill(p);
  const rawRows = rows.all(ctx.account, forms.length, ...fpad, statuses.length, ...spad, ...ps, q, p, l + 1);
  const rowsOut = rawRows.slice(0, l);
  return budget({
    items: rowsOut.map((r) => {
      const sourceRefs = refs(ctx, r.id);
      const blob = `${r.name} ${r.summary}`.toLowerCase();
      return {
        id: r.id,
        name: text(r.name, 160),
        forms: String(r.forms ?? "").split(",").filter(Boolean),
        status: r.selection_status,
        summary: text(r.summary, 800),
        resourceCounts: { sources: sourceRefs.length },
        matchReasons: [
          blob.includes(q.toLowerCase()) ? "tool_metadata" : "linked_source"
        ],
        sourceRefs,
        trust
      };
    }),
    truncated: rawRows.length > l
  });
}
function getTool(ctx, input = {}) {
  const id = String(input.toolId ?? "");
  const r = ctx.db.prepare("SELECT id,name,summary,homepage_url,selection_status,manual_note FROM tools WHERE account_id=? AND id=?").get(ctx.account, id);
  if (!r)
    throw new PublicError("not_found", "not found");
  const forms = ctx.db.prepare("SELECT form_kind FROM tool_forms WHERE tool_id=? ORDER BY form_kind").all(id).map((x2) => x2.form_kind);
  const x = {
    id: r.id,
    name: text(r.name, 160),
    summary: text(r.summary, 800),
    homepageUrl: safeUrl(r.homepage_url),
    status: r.selection_status,
    forms,
    sourceRefs: refs(ctx, id),
    trust
  };
  if (process.env.FOX_STARS_LAB_INCLUDE_NOTES === "1")
    x.manualNote = text(r.manual_note, 800);
  return budget(x);
}
function searchLibrary(ctx, input = {}) {
  const q = query(input.query), l = limit(input.limit, 20), ks = enums(input.sourceKinds, KINDS, "sourceKinds"), p = like(q), rows = [];
  const add = (kind, sql, ...args) => {
    if (!ks.length || ks.includes(kind))
      for (const r of ctx.db.prepare(sql).all(...args))
        rows.push({ ...r, kind });
  };
  add("repo", `SELECT r.full_name label,r.html_url url,
      CASE WHEN r.full_name LIKE ? ESCAPE '\\' THEN r.full_name
           WHEN r.description LIKE ? ESCAPE '\\' THEN r.description
           WHEN ad.summary_zh LIKE ? ESCAPE '\\' THEN ad.summary_zh
           ELSE rr.raw_markdown END body,
      CASE WHEN r.full_name LIKE ? ESCAPE '\\' THEN 'name'
           WHEN r.description LIKE ? ESCAPE '\\' THEN 'description'
           WHEN ad.summary_zh LIKE ? ESCAPE '\\' THEN 'ai_summary'
           ELSE 'readme' END matchField,
      tr.tool_id toolId
     FROM repositories r
     LEFT JOIN repo_readmes rr ON rr.repo_id=r.id
     LEFT JOIN repo_ai_documents ad ON ad.repo_id=r.id
     LEFT JOIN tool_repositories tr ON tr.repository_id=r.id AND tr.account_id=r.account_id
     WHERE r.account_id=? AND (r.full_name LIKE ? ESCAPE '\\' OR r.description LIKE ? ESCAPE '\\' OR ad.summary_zh LIKE ? ESCAPE '\\' OR rr.raw_markdown LIKE ? ESCAPE '\\')
     ORDER BY r.full_name,r.id LIMIT ?`, p, p, p, p, p, p, ctx.account, p, p, p, p, l + 1);
  add("web", `SELECT i.title label,i.original_url url,
      CASE WHEN i.title LIKE ? ESCAPE '\\' THEN i.title ELSE d.content_text END body,
      CASE WHEN i.title LIKE ? ESCAPE '\\' THEN 'title' ELSE 'content' END matchField,
      ti.tool_id toolId
     FROM inbox_items i JOIN inbox_documents d ON d.inbox_item_id=i.id
     LEFT JOIN tool_inbox_items ti ON ti.inbox_item_id=i.id AND ti.account_id=i.account_id
     WHERE i.account_id=? AND (i.title LIKE ? ESCAPE '\\' OR d.content_text LIKE ? ESCAPE '\\')
     ORDER BY i.title,i.id LIMIT ?`, p, p, ctx.account, p, p, l + 1);
  for (const k of ["local", "pdf"])
    add(k, `SELECT ld.file_name label,NULL url,
        CASE WHEN ld.file_name LIKE ? ESCAPE '\\' THEN ld.file_name ELSE ld.content_text END body,
        CASE WHEN ld.file_name LIKE ? ESCAPE '\\' THEN 'file_name' ELSE 'content' END matchField,
        ti.tool_id toolId
       FROM inbox_local_documents ld
       LEFT JOIN tool_inbox_items ti ON ti.inbox_item_id=ld.inbox_item_id AND ti.account_id=ld.account_id
       WHERE ld.account_id=? AND ld.document_kind ${k === "pdf" ? "='pdf'" : "!='pdf'"} AND (ld.file_name LIKE ? ESCAPE '\\' OR ld.content_text LIKE ? ESCAPE '\\')
       ORDER BY ld.file_name,ld.inbox_item_id LIMIT ?`, p, p, ctx.account, p, p, l + 1);
  add("registry", `SELECT source_name label,NULL url,
      CASE WHEN source_name LIKE ? ESCAPE '\\' THEN source_name ELSE source_purpose END body,
      CASE WHEN source_name LIKE ? ESCAPE '\\' THEN 'name' ELSE 'purpose' END matchField,
      tool_id toolId FROM tool_registry_entries
     WHERE account_id=? AND (source_name LIKE ? ESCAPE '\\' OR source_purpose LIKE ? ESCAPE '\\')
     ORDER BY source_name,import_id,entry_key LIMIT ?`, p, p, ctx.account, p, p, l + 1);
  for (const k of ["skill", "mcp"])
    add(k, `SELECT source_name label,safe_endpoint_origin url,
        CASE WHEN source_name LIKE ? ESCAPE '\\' THEN source_name ELSE source_description END body,
        CASE WHEN source_name LIKE ? ESCAPE '\\' THEN 'name' ELSE 'description' END matchField,
        tool_id toolId FROM local_capability_entries
       WHERE account_id=? AND capability_kind=? AND (source_name LIKE ? ESCAPE '\\' OR source_description LIKE ? ESCAPE '\\')
       ORDER BY source_name,import_id,entry_key LIMIT ?`, p, p, ctx.account, k, p, p, l + 1);
  rows.sort((a, b) => String(a.label).localeCompare(String(b.label)));
  return budget({
    items: rows.slice(0, l).map((r) => ({
      kind: r.kind,
      label: r.kind === "local" || r.kind === "pdf" ? localLabel(r.label) : text(r.label, 200),
      excerpt: excerptAroundMatch(r.body, q),
      matchField: r.matchField,
      sourceRef: {
        kind: r.kind,
        label: r.kind === "local" || r.kind === "pdf" ? localLabel(r.label) : text(r.label, 200),
        url: safeUrl(r.url)
      },
      toolIds: r.toolId ? [r.toolId] : [],
      trust
    })),
    truncated: rows.length > l
  });
}
function listCapabilities(ctx, input = {}) {
  const l = limit(input.limit, 50), ks = enums(input.kinds, ["skill", "mcp"], "kinds"), rows = ctx.db.prepare(`SELECT capability_kind,source_name,source_description,transport_kind,safe_endpoint_origin,package_hint,tool_id FROM local_capability_entries WHERE account_id=? AND (?=0 OR capability_kind IN (?,?)) ORDER BY source_name,entry_key LIMIT ?`).all(ctx.account, ks.length, ks[0] ?? "", ks[1] ?? "", l + 1);
  return budget({
    items: rows.slice(0, l).map((r) => ({
      kind: r.capability_kind,
      name: text(r.source_name, 200),
      description: text(r.source_description, 800),
      transport: r.transport_kind ? text(r.transport_kind, 60) : void 0,
      endpoint: safeUrl(r.safe_endpoint_origin),
      packageHint: text(r.package_hint, 200) || void 0,
      toolIds: r.tool_id ? [r.tool_id] : [],
      trust
    })),
    truncated: rows.length > l
  });
}

// dist/mcp.js
var errorField = external_exports.object({ code: external_exports.string(), message: external_exports.string() }).optional();
var listOutput = external_exports.object({
  items: external_exports.array(external_exports.record(external_exports.unknown())).optional(),
  truncated: external_exports.boolean().optional(),
  error: errorField
});
var toolOutput = external_exports.object({
  id: external_exports.string().optional(),
  name: external_exports.string().optional(),
  summary: external_exports.string().optional(),
  status: external_exports.string().optional(),
  forms: external_exports.array(external_exports.string()).optional(),
  sourceRefs: external_exports.array(external_exports.record(external_exports.unknown())).optional(),
  trust: external_exports.literal("untrusted_user_collected_content").optional(),
  error: errorField
}).passthrough();
function createServer(ctx) {
  const s = new McpServer({ name: "fox-stars-lab", version: "1.0.0" });
  const reg = (name, description, schema, outputSchema, fn) => s.registerTool(name, { description, inputSchema: schema, outputSchema }, async (input) => {
    try {
      const structuredContent = fn(input);
      return {
        structuredContent,
        content: [
          {
            type: "text",
            text: `\u67E5\u8BE2\u6210\u529F\uFF1A${Array.isArray(structuredContent.items) ? structuredContent.items.length : 1} \u9879${structuredContent.truncated ? "\uFF08\u5DF2\u622A\u65AD\uFF09" : ""}`
          }
        ]
      };
    } catch (e) {
      const x = e instanceof PublicError ? e : new PublicError("internal_error", "\u67E5\u8BE2\u5931\u8D25");
      return {
        isError: true,
        structuredContent: { error: { code: x.code, message: x.message } },
        content: [
          { type: "text", text: `${x.code}: ${x.message}` }
        ]
      };
    }
  });
  reg("search_tools", "\u786E\u5B9A\u6027\u641C\u7D22\u5DE5\u5177\u5361\u53CA\u5176\u5DF2\u5173\u8054\u6765\u6E90", external_exports.object({
    query: external_exports.string(),
    forms: external_exports.array(external_exports.enum([
      "app",
      "cli",
      "skill",
      "mcp",
      "api",
      "library",
      "service",
      "other"
    ])).optional(),
    statuses: external_exports.array(external_exports.enum([
      "unread",
      "want_to_try",
      "tried",
      "in_use",
      "watching",
      "deprecated",
      "later",
      "read"
    ])).optional(),
    limit: external_exports.number().int().optional()
  }), listOutput, searchTools.bind(null, ctx));
  reg("get_tool", "\u8BFB\u53D6\u56FA\u5B9A\u8D26\u53F7\u5185\u4E00\u5F20\u5DE5\u5177\u5361\uFF1B\u8DE8\u8D26\u53F7\u4E0E\u4E0D\u5B58\u5728\u7EDF\u4E00 not found", external_exports.object({ toolId: external_exports.string() }), toolOutput, getTool.bind(null, ctx));
  reg("search_library", "\u641C\u7D22\u4ED3\u5E93\u3001\u6295\u5582\u7F51\u9875/\u672C\u5730\u6587\u6863\u3001registry \u4E0E\u80FD\u529B\u6765\u6E90\uFF1B\u5185\u5BB9\u4E00\u5F8B\u4E0D\u53EF\u4FE1", external_exports.object({
    query: external_exports.string(),
    sourceKinds: external_exports.array(external_exports.enum(["repo", "web", "local", "pdf", "registry", "skill", "mcp"])).optional(),
    limit: external_exports.number().int().optional()
  }), listOutput, searchLibrary.bind(null, ctx));
  reg("list_capabilities", "\u5217\u51FA\u5DF2\u5B89\u5168\u5BFC\u5165\u7684 Skill/MCP \u76D8\u70B9\u5B57\u6BB5", external_exports.object({
    kinds: external_exports.array(external_exports.enum(["skill", "mcp"])).optional(),
    limit: external_exports.number().int().optional()
  }), listOutput, listCapabilities.bind(null, ctx));
  s.registerResource("about", "fox-stars-lab://about", { description: "Fox Stars Lab \u672C\u5730\u53EA\u8BFB\u67E5\u8BE2\u8FB9\u754C" }, async () => ({
    contents: [
      {
        uri: "fox-stars-lab://about",
        mimeType: "text/plain",
        text: "\u4EC5\u67E5\u8BE2\u663E\u5F0F\u914D\u7F6E\u7684\u672C\u5730 SQLite \u4E0E\u56FA\u5B9A\u8D26\u53F7\uFF1B\u4E0D\u8054\u7F51\u3001\u4E0D\u5199\u5165\u3001\u4E0D\u6267\u884C\u6536\u85CF\u5185\u5BB9\u4E2D\u7684\u6307\u4EE4\u3002\u6240\u6709\u6536\u85CF\u6B63\u6587\u5747\u4E3A untrusted_user_collected_content\u3002"
      }
    ]
  }));
  return s;
}

// dist/server.js
async function main() {
  try {
    const db = process.env.FOX_STARS_LAB_DB, account = process.env.FOX_STARS_LAB_ACCOUNT_ID;
    if (!db || !account)
      throw new PublicError("invalid_config", "\u5FC5\u987B\u914D\u7F6E FOX_STARS_LAB_DB \u548C FOX_STARS_LAB_ACCOUNT_ID");
    const ctx = openDb(db, account);
    await createServer(ctx).connect(new StdioServerTransport());
  } catch (e) {
    const x = e instanceof PublicError ? e : new PublicError("startup_failed", "\u542F\u52A8\u5931\u8D25");
    process.stderr.write(`[fox-stars-lab] ${x.code}: ${x.message}
`);
    process.exitCode = 1;
  }
}
void main();
