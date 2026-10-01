-- PROPOSED 029: 对外稳定只读契约。MCP、web-search 的 stars 子命令、本地 API 只读这些视图，不再直连物理表。
CREATE VIEW v_public_contract AS
SELECT 1 AS contract_version, (SELECT max(version) FROM schema_migrations) AS schema_version;
CREATE VIEW v_public_repositories AS
SELECT r.id, r.account_id, r.full_name, r.description, r.language, r.topics_json, r.html_url, r.stars_count, r.starred_at, r.pushed_at,
       d.one_liner_zh, d.summary_zh, d.keywords_json, d.card_json, d.coverage_status,
       a.read_status, a.rating, a.note_md,
       (SELECT json_group_array(c.name_zh) FROM entity_categories ec JOIN categories c ON c.id = ec.category_id
         WHERE ec.entity_type = 'repository' AND ec.entity_id = r.id AND ec.status <> 'rejected') AS categories_json,
       (SELECT json_group_array(t.name) FROM repo_tags rt JOIN tags t ON t.id = rt.tag_id WHERE rt.repo_id = r.id) AS tags_json
FROM repositories r
LEFT JOIN repo_ai_documents d ON d.repo_id = r.id
LEFT JOIN annotations a ON a.repo_id = r.id AND a.account_id = r.account_id
WHERE r.sync_status = 'active';
CREATE VIEW v_public_tools AS
SELECT t.id, t.account_id, t.name, t.summary, t.homepage_url, t.selection_status, t.origin, t.ai_card_json,
       (SELECT json_group_array(form_kind) FROM tool_forms f WHERE f.tool_id = t.id) AS forms_json,
       (SELECT json_group_array(tr.repository_id) FROM tool_repositories tr WHERE tr.tool_id = t.id) AS repository_ids_json
FROM tools t;
CREATE VIEW v_public_graph_edges AS
SELECT account_id, src_type, src_id, relation, dst_type, dst_id, confidence, status FROM kg_edges WHERE status <> 'rejected';
INSERT INTO schema_migrations(version, name) VALUES('029', 'public_views');
