// dist/core.js — extracted from Fox Stars Lab.app/Contents/Resources/mcp/server.mjs (1.4.0-fox.1 bundle)
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

