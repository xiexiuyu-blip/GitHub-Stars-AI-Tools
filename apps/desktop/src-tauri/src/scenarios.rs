//! 业务场景：种子、规则归类、AI 建议和手动确认。不依赖 Tauri。

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SCENARIOS_SEED: &str = include_str!("../../../../docs/rebuild/spec/scenarios_seed.json");

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioSummary {
    pub id: String,
    pub slug: String,
    pub name_zh: String,
    pub sort_order: i64,
    pub count: i64,
    pub thin: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyStats {
    pub repositories: usize,
    pub assigned: usize,
    pub removed: usize,
    pub per_scenario: Vec<(String, usize)>,
}

#[derive(Debug, Deserialize)]
struct SeedFile {
    thin_threshold: i64,
    thin_focus: Vec<String>,
    scenarios: Vec<SeedScenario>,
}

#[derive(Debug, Deserialize)]
struct SeedScenario {
    slug: String,
    name_zh: String,
    sort_order: i64,
    keywords: Vec<String>,
}

#[derive(Debug)]
struct ScenarioRow {
    id: String,
    slug: String,
    keywords: Vec<String>,
}

#[derive(Debug)]
struct RepoText {
    id: String,
    full_name: String,
    description: String,
    topics_json: String,
    summary_zh: String,
    one_liner_zh: String,
    keywords_json: String,
    suggested_tags_json: String,
    card_json: String,
}

fn open_db(path: &Path) -> Result<Connection, String> {
    crate::v2_ops::open_db(path)
}

fn account_id(connection: &Connection) -> Result<String, String> {
    connection
        .query_row(
            "SELECT id FROM github_accounts ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("账号读取失败：{error}"))?
        .ok_or_else(|| "还没有 GitHub 账号，无法写入场景。".to_owned())
}

fn load_seed() -> Result<SeedFile, String> {
    serde_json::from_str(SCENARIOS_SEED).map_err(|error| format!("场景种子解析失败：{error}"))
}

fn same_keywords(stored: &str, seed: &[String]) -> bool {
    serde_json::from_str::<Vec<String>>(stored).ok().as_deref() == Some(seed)
}

/// 幂等写入场景种子。只返回新插入的行数；已有的 seed 行若名称、关键词或排序变了就更新。
/// `origin='user'` 和 `archived=1` 的行保持原样。
pub fn seed_scenarios(path: &Path) -> Result<usize, String> {
    let seed = load_seed()?;
    let mut connection = open_db(path)?;
    let account = account_id(&connection)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("场景种子事务开启失败：{error}"))?;
    let mut inserted = 0;
    for scenario in &seed.scenarios {
        let id = format!("scn-{}", scenario.slug);
        let keywords = serde_json::to_string(&scenario.keywords)
            .map_err(|error| format!("场景关键词序列化失败：{error}"))?;
        let existing: Option<(String, String, i64, i64, String)> = tx
            .query_row(
                "SELECT origin, name_zh, sort_order, archived, keywords_json FROM scenarios WHERE account_id = ?1 AND slug = ?2",
                params![account, scenario.slug],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .optional()
            .map_err(|error| format!("场景读取失败：{error}"))?;
        match existing {
            None => {
                tx.execute(
                    "INSERT INTO scenarios(id, account_id, slug, name_zh, keywords_json, sort_order, origin, archived) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'seed', 0)",
                    params![id, account, scenario.slug, scenario.name_zh, keywords, scenario.sort_order],
                )
                .map_err(|error| format!("场景种子写入失败：{error}"))?;
                inserted += 1;
            }
            Some((origin, name, sort_order, archived, stored_keywords)) => {
                if origin != "seed" || archived != 0 {
                    continue;
                }
                if name == scenario.name_zh
                    && sort_order == scenario.sort_order
                    && same_keywords(&stored_keywords, &scenario.keywords)
                {
                    continue;
                }
                tx.execute(
                    "UPDATE scenarios SET name_zh = ?1, keywords_json = ?2, sort_order = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE account_id = ?4 AND slug = ?5 AND origin = 'seed' AND archived = 0",
                    params![scenario.name_zh, keywords, scenario.sort_order, account, scenario.slug],
                )
                .map_err(|error| format!("场景种子更新失败：{error}"))?;
            }
        }
    }
    tx.commit()
        .map_err(|error| format!("场景种子提交失败：{error}"))?;
    Ok(inserted)
}

/// 关键词和文本都按小写比较。
/// 全是 ASCII 字母数字、且长度不超过 4 的词，只从词首匹配：左边不能是字母或数字，右边可以继续。
/// 这样 `ppt` 能命中 `pptx-tools`，`rag` 不会从 `storage` 中间命中，`seo` 不会从 `videoseoul` 中间命中；
/// `video-seo-tools` 里连字符把 `seo` 隔成独立词，也会命中。
/// 其余关键词（中文、更长的英文、带空格或连字符）做子串匹配。
pub fn matches_keyword(haystack_lower: &str, keyword_lower: &str) -> bool {
    let haystack = haystack_lower.to_lowercase();
    let keyword = keyword_lower.trim().to_lowercase();
    if keyword.is_empty() {
        return false;
    }
    let short_token =
        keyword.len() <= 4 && keyword.bytes().all(|byte| byte.is_ascii_alphanumeric());
    if !short_token {
        return haystack.contains(&keyword);
    }
    let hay = haystack.as_bytes();
    let needle = keyword.as_bytes();
    if needle.len() > hay.len() {
        return false;
    }
    let mut index = 0;
    while index + needle.len() <= hay.len() {
        if &hay[index..index + needle.len()] == needle {
            let before_ok = index == 0 || !hay[index - 1].is_ascii_alphanumeric();
            if before_ok {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn confidence_for(hits: usize) -> f64 {
    if hits == 0 {
        return 0.0;
    }
    let tenths = 5 + (hits - 1);
    (tenths.min(8) as f64) / 10.0
}

fn keyword_hits(haystack: &str, keywords: &[String]) -> Vec<String> {
    keywords
        .iter()
        .filter(|keyword| matches_keyword(haystack, keyword))
        .cloned()
        .collect()
}

fn parse_keywords(raw: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(raw).map_err(|error| format!("不是 JSON：{error}"))?;
    let items = value
        .as_array()
        .ok_or_else(|| "关键词必须是数组".to_owned())?;
    let mut keywords = Vec::new();
    for item in items {
        let Some(text) = item.as_str() else {
            continue;
        };
        let keyword = text.trim().to_lowercase();
        if keyword.is_empty() || keywords.contains(&keyword) {
            continue;
        }
        keywords.push(keyword);
    }
    Ok(keywords)
}

fn push_text(parts: &mut Vec<String>, text: &str) {
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_owned());
    }
}

fn collect_strings(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(text) => push_text(parts, text),
        Value::Array(items) => {
            for item in items {
                collect_strings(item, parts);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_strings(item, parts);
            }
        }
        _ => {}
    }
}

fn push_json_strings(parts: &mut Vec<String>, raw: &str) {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    collect_strings(&value, parts);
}

fn push_string_list(parts: &mut Vec<String>, value: &Value) {
    match value {
        Value::String(text) => push_text(parts, text),
        Value::Array(items) => {
            for item in items {
                if let Value::String(text) = item {
                    push_text(parts, text);
                }
            }
        }
        _ => {}
    }
}

/// 理解卡字段类型不对、空串或坏 JSON 都跳过，不让整批归类失败。
fn push_card_text(parts: &mut Vec<String>, raw: &str) {
    if raw.trim().is_empty() {
        return;
    }
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    let Some(object) = value.as_object() else {
        return;
    };
    if let Some(capabilities) = object.get("capabilities") {
        match capabilities {
            Value::Array(items) => {
                for item in items {
                    match item {
                        Value::Object(map) => {
                            if let Some(Value::String(name)) = map.get("name_zh") {
                                push_text(parts, name);
                            }
                        }
                        Value::String(text) => push_text(parts, text),
                        _ => {}
                    }
                }
            }
            Value::String(text) => push_text(parts, text),
            _ => {}
        }
    }
    for key in ["problems_solved", "use_when"] {
        if let Some(value) = object.get(key) {
            push_string_list(parts, value);
        }
    }
    if let Some(Value::String(text)) = object.get("inspiration_for_fox") {
        push_text(parts, text);
    }
}

fn build_haystack(repo: &RepoText) -> String {
    let mut parts = Vec::new();
    push_text(&mut parts, &repo.full_name);
    push_text(&mut parts, &repo.description);
    push_json_strings(&mut parts, &repo.topics_json);
    push_text(&mut parts, &repo.summary_zh);
    push_text(&mut parts, &repo.one_liner_zh);
    push_json_strings(&mut parts, &repo.keywords_json);
    push_json_strings(&mut parts, &repo.suggested_tags_json);
    push_card_text(&mut parts, &repo.card_json);
    parts.join(" ").to_lowercase()
}

fn load_scenarios(connection: &Connection, account: &str) -> Result<Vec<ScenarioRow>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, slug, keywords_json FROM scenarios WHERE account_id = ?1 AND archived = 0 ORDER BY sort_order ASC, slug ASC",
        )
        .map_err(|error| format!("场景读取失败：{error}"))?;
    let rows = statement
        .query_map(params![account], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("场景查询失败：{error}"))?;
    let mut scenarios = Vec::new();
    for row in rows {
        let (id, slug, keywords_json) = row.map_err(|error| format!("场景行读取失败：{error}"))?;
        let keywords = parse_keywords(&keywords_json)
            .map_err(|error| format!("场景「{slug}」的关键词无法读取：{error}"))?;
        scenarios.push(ScenarioRow { id, slug, keywords });
    }
    Ok(scenarios)
}

const REPO_SELECT: &str = "
SELECT r.id, r.full_name, IFNULL(r.description, ''), IFNULL(r.topics_json, '[]'),
       IFNULL(d.summary_zh, ''), IFNULL(d.one_liner_zh, ''),
       IFNULL(d.keywords_json, '[]'), IFNULL(d.suggested_tags_json, '[]'),
       IFNULL(d.card_json, '')
FROM repositories r
LEFT JOIN repo_ai_documents d ON d.repo_id = r.id
";

fn placeholders(count: usize) -> String {
    let mut marks = Vec::with_capacity(count);
    for _ in 0..count {
        marks.push("?");
    }
    marks.join(",")
}

fn map_repo(row: &rusqlite::Row<'_>) -> rusqlite::Result<RepoText> {
    Ok(RepoText {
        id: row.get(0)?,
        full_name: row.get(1)?,
        description: row.get(2)?,
        topics_json: row.get(3)?,
        summary_zh: row.get(4)?,
        one_liner_zh: row.get(5)?,
        keywords_json: row.get(6)?,
        suggested_tags_json: row.get(7)?,
        card_json: row.get(8)?,
    })
}

fn load_repos(
    connection: &Connection,
    account: &str,
    repo_ids: Option<&[String]>,
) -> Result<Vec<RepoText>, String> {
    let (sql, bound) = match repo_ids {
        Some(ids) if ids.is_empty() => return Ok(Vec::new()),
        Some(ids) => {
            let sql = format!(
                "{REPO_SELECT} WHERE r.account_id = ? AND r.id IN ({}) ORDER BY r.id",
                placeholders(ids.len())
            );
            let mut bound = vec![account.to_owned()];
            bound.extend(ids.iter().cloned());
            (sql, bound)
        }
        None => (
            format!(
                "{REPO_SELECT} WHERE r.account_id = ? AND r.sync_status = 'active' ORDER BY r.id"
            ),
            vec![account.to_owned()],
        ),
    };
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("仓库读取失败：{error}"))?;
    let rows = statement
        .query_map(params_from_iter(bound.iter()), map_repo)
        .map_err(|error| format!("仓库查询失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("仓库解析失败：{error}"))
}

fn is_protected(origin: &str, status: &str) -> bool {
    status == "confirmed" || status == "rejected" || origin == "ai" || origin == "user"
}

fn write_rule(
    connection: &Connection,
    account: &str,
    repo_id: &str,
    scenario_id: &str,
    confidence: f64,
) -> Result<usize, String> {
    let existing: Option<(String, String, f64)> = connection
        .query_row(
            "SELECT origin, status, confidence FROM entity_scenarios WHERE entity_type = 'repository' AND entity_id = ?1 AND scenario_id = ?2",
            params![repo_id, scenario_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| format!("归类读取失败：{error}"))?;
    if let Some((origin, status, current)) = existing {
        if is_protected(&origin, &status) || (current - confidence).abs() < 1e-9 {
            return Ok(0);
        }
        let changed = connection
            .execute(
                "UPDATE entity_scenarios SET confidence = ?1 WHERE entity_type = 'repository' AND entity_id = ?2 AND scenario_id = ?3 AND origin = 'rule' AND status = 'suggested'",
                params![confidence, repo_id, scenario_id],
            )
            .map_err(|error| format!("归类写入失败：{error}"))?;
        if changed == 0 {
            return Err("归类写入失败：规则建议没有更新".to_owned());
        }
        return Ok(1);
    }
    connection
        .execute(
            "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, origin, confidence, status) VALUES (?1, 'repository', ?2, ?3, 'rule', ?4, 'suggested')",
            params![account, repo_id, scenario_id, confidence],
        )
        .map_err(|error| format!("归类写入失败：{error}"))?;
    Ok(1)
}

fn delete_stale(
    connection: &Connection,
    account: &str,
    repo_id: &str,
    matched: &[String],
) -> Result<usize, String> {
    if matched.is_empty() {
        return connection
            .execute(
                "DELETE FROM entity_scenarios WHERE account_id = ?1 AND entity_type = 'repository' AND entity_id = ?2 AND origin = 'rule' AND status = 'suggested'",
                params![account, repo_id],
            )
            .map_err(|error| format!("陈旧归类清理失败：{error}"));
    }
    let sql = format!(
        "DELETE FROM entity_scenarios WHERE account_id = ? AND entity_type = 'repository' AND entity_id = ? AND origin = 'rule' AND status = 'suggested' AND scenario_id NOT IN ({})",
        placeholders(matched.len())
    );
    let mut bound = vec![account.to_owned(), repo_id.to_owned()];
    bound.extend(matched.iter().cloned());
    connection
        .execute(&sql, params_from_iter(bound.iter()))
        .map_err(|error| format!("陈旧归类清理失败：{error}"))
}

/// `repo_ids` 为 `None` 时处理当前账号全部有效仓库；给出 id 时只处理这些仓库（含已移除的）。
/// 整批放在一个事务里。没有场景时直接报错，避免把已有规则建议清掉。
pub fn classify_repositories(
    path: &Path,
    repo_ids: Option<&[String]>,
) -> Result<ClassifyStats, String> {
    if matches!(repo_ids, Some(ids) if ids.is_empty()) {
        let connection = open_db(path)?;
        let _account = account_id(&connection)?;
        return Ok(ClassifyStats::default());
    }
    let mut connection = open_db(path)?;
    let account = account_id(&connection)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("归类事务开启失败：{error}"))?;
    let scenarios = load_scenarios(&tx, &account)?;
    if scenarios.is_empty() {
        return Err("还没有可归类的场景。".to_owned());
    }
    let repos = load_repos(&tx, &account, repo_ids)?;
    let mut counts = vec![0usize; scenarios.len()];
    let mut assigned = 0;
    let mut removed = 0;
    for repo in &repos {
        let haystack = build_haystack(repo);
        let mut matched = Vec::new();
        for (index, scenario) in scenarios.iter().enumerate() {
            let hits = keyword_hits(&haystack, &scenario.keywords);
            if hits.is_empty() {
                continue;
            }
            counts[index] += 1;
            matched.push(scenario.id.clone());
            assigned += write_rule(
                &tx,
                &account,
                &repo.id,
                &scenario.id,
                confidence_for(hits.len()),
            )?;
        }
        removed += delete_stale(&tx, &account, &repo.id, &matched)?;
    }
    let per_scenario = scenarios
        .iter()
        .zip(counts)
        .filter(|(_, count)| *count > 0)
        .map(|(scenario, count)| (scenario.slug.clone(), count))
        .collect();
    let repositories = repos.len();
    tx.commit()
        .map_err(|error| format!("归类提交失败：{error}"))?;
    Ok(ClassifyStats {
        repositories,
        assigned,
        removed,
        per_scenario,
    })
}

/// 替换这个仓库上 `origin='ai'` 且 `status='suggested'` 的建议。
/// 已确认、已拒绝，以及 `origin='user'` 的场景跳过；不认识或已归档的 slug 丢掉。
/// 理解卡流水线还没接上，所以现在只有测试调用。
#[allow(dead_code)]
pub fn apply_ai_scenarios(
    connection: &mut Connection,
    account_id: &str,
    repo_id: &str,
    slugs: &[String],
    confidence: f64,
) -> Result<usize, String> {
    if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
        return Err("置信度必须在 0 到 1 之间".to_owned());
    }
    let savepoint = connection
        .savepoint()
        .map_err(|error| format!("场景写入事务开启失败：{error}"))?;
    savepoint
        .execute(
            "DELETE FROM entity_scenarios WHERE account_id = ?1 AND entity_type = 'repository' AND entity_id = ?2 AND origin = 'ai' AND status = 'suggested'",
            params![account_id, repo_id],
        )
        .map_err(|error| format!("旧的 AI 场景建议清理失败：{error}"))?;
    let mut written = 0;
    let mut seen = HashSet::new();
    for slug in slugs {
        if !seen.insert(slug.clone()) {
            continue;
        }
        let scenario_id: Option<String> = savepoint
            .query_row(
                "SELECT id FROM scenarios WHERE account_id = ?1 AND slug = ?2 AND archived = 0",
                params![account_id, slug],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("场景读取失败：{error}"))?;
        let Some(scenario_id) = scenario_id else {
            continue;
        };
        let existing: Option<(String, String)> = savepoint
            .query_row(
                "SELECT origin, status FROM entity_scenarios WHERE entity_type = 'repository' AND entity_id = ?1 AND scenario_id = ?2",
                params![repo_id, scenario_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| format!("归类读取失败：{error}"))?;
        match existing {
            Some((origin, status))
                if status == "confirmed" || status == "rejected" || origin == "user" =>
            {
                continue;
            }
            Some(_) => {
                savepoint
                    .execute(
                        "UPDATE entity_scenarios SET origin = 'ai', status = 'suggested', confidence = ?1 WHERE entity_type = 'repository' AND entity_id = ?2 AND scenario_id = ?3",
                        params![confidence, repo_id, scenario_id],
                    )
                    .map_err(|error| format!("AI 场景更新失败：{error}"))?;
                written += 1;
            }
            None => {
                savepoint
                    .execute(
                        "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, origin, confidence, status) VALUES (?1, 'repository', ?2, ?3, 'ai', ?4, 'suggested')",
                        params![account_id, repo_id, scenario_id, confidence],
                    )
                    .map_err(|error| format!("AI 场景写入失败：{error}"))?;
                written += 1;
            }
        }
    }
    savepoint
        .commit()
        .map_err(|error| format!("AI 场景提交失败：{error}"))?;
    Ok(written)
}

pub fn list_scenarios_summary(path: &Path) -> Result<Vec<ScenarioSummary>, String> {
    let seed = load_seed()?;
    let connection = open_db(path)?;
    let account = account_id(&connection)?;
    let mut statement = connection
        .prepare(
            "
SELECT s.id, s.slug, s.name_zh, s.sort_order,
       (
         SELECT COUNT(*)
         FROM entity_scenarios es
         JOIN repositories r ON r.id = es.entity_id AND r.account_id = s.account_id
         WHERE es.scenario_id = s.id
           AND es.entity_type = 'repository'
           AND es.status <> 'rejected'
           AND r.sync_status = 'active'
       )
FROM scenarios s
WHERE s.account_id = ?1 AND s.archived = 0
ORDER BY s.sort_order ASC, s.slug ASC
",
        )
        .map_err(|error| format!("场景列表读取失败：{error}"))?;
    let rows = statement
        .query_map(params![account], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(|error| format!("场景列表查询失败：{error}"))?;
    let mut summaries = Vec::new();
    for row in rows {
        let (id, slug, name_zh, sort_order, count) =
            row.map_err(|error| format!("场景列表行读取失败：{error}"))?;
        let thin = count <= seed.thin_threshold && seed.thin_focus.iter().any(|item| item == &slug);
        summaries.push(ScenarioSummary {
            id,
            slug,
            name_zh,
            sort_order,
            count,
            thin,
        });
    }
    Ok(summaries)
}

pub fn set_repository_scenario(
    path: &Path,
    repo_id: &str,
    scenario_id: &str,
    status: &str,
) -> Result<(), String> {
    if !matches!(status, "confirmed" | "rejected" | "suggested") {
        return Err("状态只能是 confirmed、rejected 或 suggested".to_owned());
    }
    let mut connection = open_db(path)?;
    let account = account_id(&connection)?;
    let repo_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM repositories WHERE id = ?1 AND account_id = ?2)",
            params![repo_id, account],
            |row| row.get(0),
        )
        .map_err(|error| format!("仓库读取失败：{error}"))?;
    if !repo_exists {
        return Err("仓库不存在".to_owned());
    }
    let scenario_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM scenarios WHERE id = ?1 AND account_id = ?2)",
            params![scenario_id, account],
            |row| row.get(0),
        )
        .map_err(|error| format!("场景读取失败：{error}"))?;
    if !scenario_exists {
        return Err("场景不存在".to_owned());
    }
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("归类事务开启失败：{error}"))?;
    if status == "suggested" {
        let origin: Option<String> = tx
            .query_row(
                "SELECT origin FROM entity_scenarios WHERE entity_type = 'repository' AND entity_id = ?1 AND scenario_id = ?2",
                params![repo_id, scenario_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("归类读取失败：{error}"))?;
        if origin.as_deref() != Some("user") {
            return Err("这条归类不是你手动设置的".to_owned());
        }
        let changed = tx
            .execute(
                "DELETE FROM entity_scenarios WHERE account_id = ?1 AND entity_type = 'repository' AND entity_id = ?2 AND scenario_id = ?3 AND origin = 'user'",
                params![account, repo_id, scenario_id],
            )
            .map_err(|error| format!("归类撤销失败：{error}"))?;
        if changed == 0 {
            return Err("这条归类不是你手动设置的".to_owned());
        }
    } else {
        tx.execute(
            "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, origin, confidence, status) VALUES (?1, 'repository', ?2, ?3, 'user', 1.0, ?4) ON CONFLICT(entity_type, entity_id, scenario_id) DO UPDATE SET origin = 'user', confidence = 1.0, status = excluded.status",
            params![account, repo_id, scenario_id, status],
        )
        .map_err(|error| format!("归类写入失败：{error}"))?;
    }
    tx.commit()
        .map_err(|error| format!("归类提交失败：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::migrate_database;
    use rusqlite::params;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_DB_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDb(PathBuf);

    impl Drop for TempDb {
        fn drop(&mut self) {
            cleanup(&self.0);
        }
    }

    fn cleanup(path: &Path) {
        let text = path.to_string_lossy().to_string();
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{text}-wal"));
        let _ = std::fs::remove_file(format!("{text}-shm"));
    }

    fn temp_db(label: &str) -> TempDb {
        let seq = TEMP_DB_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时间应可用")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fsl-scn-{label}-{seq}-{nanos}.sqlite3"));
        if let Err(error) = migrate_database(&path) {
            cleanup(&path);
            panic!("测试库应能迁移：{error}");
        }
        TempDb(path)
    }

    fn count_sql(path: &Path, sql: &str) -> i64 {
        let connection = open_db(path).expect("应能打开测试库");
        connection
            .query_row(sql, [], |row| row.get(0))
            .expect("计数应成功")
    }

    fn insert_account(path: &Path) {
        let connection = open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .expect("应能写入账号");
    }

    fn insert_repo(
        path: &Path,
        id: &str,
        full_name: &str,
        description: &str,
        topics: &str,
        sync_status: &str,
    ) {
        let connection = open_db(path).expect("应能打开测试库");
        let name = full_name.split('/').nth(1).unwrap_or(full_name);
        connection
            .execute(
                "INSERT INTO repositories(id, account_id, owner, name, full_name, description, topics_json, html_url, sync_status, starred_at) VALUES (?1, 'acct', 'fox', ?2, ?3, ?4, ?5, ?6, ?7, '2026-09-01T00:00:00Z')",
                params![id, name, full_name, description, topics, format!("https://github.com/{full_name}"), sync_status],
            )
            .expect("应能写入仓库");
    }

    fn insert_doc(
        path: &Path,
        id: &str,
        summary: &str,
        one_liner: &str,
        keywords: &str,
        tags: &str,
        card: &str,
    ) {
        let connection = open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO repo_ai_documents(repo_id, summary_zh, one_liner_zh, keywords_json, suggested_tags_json, card_json, model, prompt_version, source_hash, generated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'test', 'v1', 'hash', '2026-09-01T00:00:00Z')",
                params![id, summary, one_liner, keywords, tags, card],
            )
            .expect("应能写入理解卡");
    }

    fn insert_link(
        path: &Path,
        repo_id: &str,
        scenario_id: &str,
        origin: &str,
        status: &str,
        confidence: f64,
    ) {
        let connection = open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, origin, confidence, status) VALUES ('acct', 'repository', ?1, ?2, ?3, ?4, ?5)",
                params![repo_id, scenario_id, origin, confidence, status],
            )
            .expect("应能写入归类");
    }

    fn assert_near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "置信度实际 {actual}，期望 {expected}"
        );
    }

    fn link_of(path: &Path, repo_id: &str, scenario_id: &str) -> Option<(String, String, f64)> {
        let connection = open_db(path).expect("应能打开测试库");
        connection
            .query_row(
                "SELECT origin, status, confidence FROM entity_scenarios WHERE entity_type = 'repository' AND entity_id = ?1 AND scenario_id = ?2",
                params![repo_id, scenario_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .expect("应能读取归类")
    }

    fn scenario_of(path: &Path, slug: &str) -> (String, String, String, String, i64, i64) {
        let connection = open_db(path).expect("应能打开测试库");
        connection
            .query_row(
                "SELECT id, origin, name_zh, keywords_json, sort_order, archived FROM scenarios WHERE slug = ?1",
                params![slug],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .expect("应能读取场景")
    }

    fn ready(label: &str) -> TempDb {
        let db = temp_db(label);
        insert_account(&db.0);
        let inserted = seed_scenarios(&db.0).expect("种子应写入");
        assert!(
            inserted >= 13,
            "第一次种子应插入 13 个场景，实际 {inserted}"
        );
        db
    }

    #[test]
    fn seed_twice_does_not_duplicate() {
        let db = temp_db("seed-twice");
        insert_account(&db.0);
        assert_eq!(seed_scenarios(&db.0).expect("第一次种子"), 13);
        assert_eq!(seed_scenarios(&db.0).expect("第二次种子"), 0);
        assert_eq!(count_sql(&db.0, "SELECT COUNT(*) FROM scenarios"), 13);
        assert_eq!(
            count_sql(
                &db.0,
                "SELECT COUNT(*) FROM scenarios WHERE id = 'scn-' || slug"
            ),
            13
        );
        let (id, origin, name, _, sort_order, archived) = scenario_of(&db.0, "xhs");
        assert_eq!(id, "scn-xhs");
        assert_eq!(origin, "seed");
        assert_eq!(name, "小红书");
        assert_eq!(sort_order, 1);
        assert_eq!(archived, 0);
    }

    #[test]
    fn seed_updates_seed_rows_but_not_user_or_archived() {
        let db = ready("seed-update");
        let (_, _, _, xhs_keywords, xhs_sort, _) = scenario_of(&db.0, "xhs");
        let (_, _, img_name, _, img_sort, _) = scenario_of(&db.0, "img");
        {
            let connection = open_db(&db.0).expect("应能打开测试库");
            connection
                .execute(
                    "UPDATE scenarios SET keywords_json = '[\"改过\"]' WHERE slug = 'xhs'",
                    [],
                )
                .expect("应能改种子关键词");
            connection
                .execute(
                    "UPDATE scenarios SET sort_order = 99 WHERE slug = 'img'",
                    [],
                )
                .expect("应能改排序");
            connection
                .execute(
                    "UPDATE scenarios SET origin = 'user', name_zh = '我的微信', keywords_json = '[\"用户词\"]', sort_order = 50 WHERE slug = 'wx'",
                    [],
                )
                .expect("应能改成用户场景");
            connection
                .execute(
                    "UPDATE scenarios SET archived = 1, name_zh = '已归档抖音', keywords_json = '[\"不要动\"]', sort_order = 77 WHERE slug = 'dy'",
                    [],
                )
                .expect("应能归档");
        }
        assert_eq!(seed_scenarios(&db.0).expect("重新种子"), 0);
        let (_, origin, _, keywords, sort_order, archived) = scenario_of(&db.0, "xhs");
        assert_eq!(origin, "seed");
        assert_eq!(archived, 0);
        assert_eq!(keywords, xhs_keywords);
        assert_eq!(sort_order, xhs_sort);
        let (_, img_origin, name, _, sort_order, img_archived) = scenario_of(&db.0, "img");
        assert_eq!(img_origin, "seed");
        assert_eq!(img_archived, 0);
        assert_eq!(name, img_name);
        assert_eq!(sort_order, img_sort);
        let (_, origin, name, keywords, sort_order, _) = scenario_of(&db.0, "wx");
        assert_eq!(origin, "user");
        assert_eq!(name, "我的微信");
        assert_eq!(keywords, "[\"用户词\"]");
        assert_eq!(sort_order, 50);
        let (_, origin, name, keywords, sort_order, archived) = scenario_of(&db.0, "dy");
        assert_eq!(origin, "seed");
        assert_eq!(name, "已归档抖音");
        assert_eq!(keywords, "[\"不要动\"]");
        assert_eq!(sort_order, 77);
        assert_eq!(archived, 1);
        assert_eq!(count_sql(&db.0, "SELECT COUNT(*) FROM scenarios"), 13);
    }

    #[test]
    fn seed_without_account_returns_chinese_error() {
        let db = temp_db("no-account");
        let error = seed_scenarios(&db.0).expect_err("没有账号不能写种子");
        assert!(error.contains("GitHub 账号"), "{error}");
        let error = list_scenarios_summary(&db.0).expect_err("没有账号不能读列表");
        assert!(error.contains("GitHub 账号"), "{error}");
    }

    #[test]
    fn short_ascii_keywords_match_only_at_word_start() {
        assert!(!matches_keyword("storage", "rag"));
        assert!(!matches_keyword("videoseoul", "seo"));
        assert!(matches_keyword("video-seo-tools", "seo"));
        assert!(matches_keyword("pptx-tools", "ppt"));
        assert!(matches_keyword("owner/pptx-tools", "PPT"));
        assert!(!matches_keyword("appptx", "ppt"));
        assert!(matches_keyword("my-ppt-deck", "ppt"));
        assert!(matches_keyword("use RAG daily", "rag"));
        assert!(!matches_keyword("mysql", "sql"));
        assert!(matches_keyword("sql-dashboard", "sql"));
        assert!(!matches_keyword("fragment", "rag"));
    }

    #[test]
    fn longer_and_chinese_keywords_match_by_substring() {
        assert!(matches_keyword("做小红书封面", "小红书"));
        assert!(!matches_keyword("做封面", "小红书"));
        assert!(matches_keyword("transcription service", "transcri"));
        assert!(matches_keyword("videoseoul travel", "video"));
        assert!(matches_keyword("Image Generation kit", "image generation"));
        assert!(matches_keyword("for e-commerce shops", "e-commerce"));
        assert!(!matches_keyword("anything", ""));
        assert!(!matches_keyword("小红", "小红书"));
    }

    #[test]
    fn confidence_formula_ramps_and_caps() {
        assert_near(confidence_for(1), 0.5);
        assert_near(confidence_for(3), 0.7);
        assert_near(confidence_for(4), 0.8);
        assert_near(confidence_for(9), 0.8);

        let db = ready("confidence");
        insert_repo(&db.0, "one", "fox/one", "小红书", "[]", "active");
        insert_repo(
            &db.0,
            "three",
            "fox/three",
            "公众号 微信 wechat",
            "[]",
            "active",
        );
        insert_repo(
            &db.0,
            "cap",
            "fox/cap",
            "海报 封面 poster logo figma illustration",
            "[]",
            "active",
        );
        classify_repositories(&db.0, None).expect("归类应成功");
        let (_, _, confidence) = link_of(&db.0, "one", "scn-xhs").expect("应命中小红书");
        assert_near(confidence, 0.5);
        let (_, _, confidence) = link_of(&db.0, "three", "scn-wx").expect("应命中微信");
        assert_near(confidence, 0.7);
        let (_, _, confidence) = link_of(&db.0, "cap", "scn-img").expect("应命中设计");
        assert_near(confidence, 0.8);
        assert!(link_of(&db.0, "one", "scn-wx").is_none());
        assert!(link_of(&db.0, "three", "scn-img").is_none());
    }

    #[test]
    fn repository_can_belong_to_multiple_scenarios() {
        let db = ready("multi");
        insert_repo(&db.0, "mix", "fox/mix", "小红书海报", "[]", "active");
        let stats = classify_repositories(&db.0, None).expect("归类应成功");
        assert_eq!(stats.repositories, 1);
        assert_eq!(stats.assigned, 2);
        assert_near(link_of(&db.0, "mix", "scn-xhs").expect("小红书").2, 0.5);
        assert_near(link_of(&db.0, "mix", "scn-img").expect("海报").2, 0.5);
        assert_eq!(count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios"), 2);
    }

    #[test]
    fn classify_reads_each_text_source_and_bad_card_json() {
        let db = ready("sources");
        insert_repo(&db.0, "cap", "fox/caponly", "plain", "[]", "active");
        insert_doc(
            &db.0,
            "cap",
            "",
            "",
            "[]",
            "[]",
            r#"{"capabilities":[{"name_zh":"整理小红书选题"}]}"#,
        );
        insert_repo(&db.0, "empty-card", "fox/emptycard", "微信", "[]", "active");
        insert_doc(&db.0, "empty-card", "", "", "[]", "[]", "");
        insert_repo(&db.0, "bad-card", "fox/badcard", "抖音", "[]", "active");
        insert_doc(&db.0, "bad-card", "", "", "[]", "[]", "{not json");
        insert_repo(&db.0, "problem", "fox/problem", "plain", "[]", "active");
        insert_doc(
            &db.0,
            "problem",
            "",
            "",
            "[]",
            "[]",
            r#"{"problems_solved":["想做电商详情"]}"#,
        );
        insert_repo(&db.0, "when", "fox/when", "plain", "[]", "active");
        insert_doc(
            &db.0,
            "when",
            "",
            "",
            "[]",
            "[]",
            r#"{"use_when":["做数据分析的时候"]}"#,
        );
        insert_repo(&db.0, "idea", "fox/idea", "plain", "[]", "active");
        insert_doc(
            &db.0,
            "idea",
            "",
            "",
            "[]",
            "[]",
            r#"{"inspiration_for_fox":"可以借鉴知识库结构"}"#,
        );
        insert_repo(&db.0, "kw", "fox/kw", "plain", "[]", "active");
        insert_doc(&db.0, "kw", "", "", r#"["爬虫"]"#, "[]", "");
        insert_repo(&db.0, "tag", "fox/tag", "plain", "[]", "active");
        insert_doc(&db.0, "tag", "", "", "[]", r#"["字幕"]"#, "");
        insert_repo(&db.0, "liner", "fox/liner", "plain", "[]", "active");
        insert_doc(&db.0, "liner", "", "这是写作助手", "[]", "[]", "");
        insert_repo(&db.0, "summary", "fox/summary", "plain", "[]", "active");
        insert_doc(&db.0, "summary", "自动化脚本", "", "[]", "[]", "");
        insert_repo(&db.0, "typed", "fox/typed", "京东", "[]", "active");
        insert_doc(
            &db.0,
            "typed",
            "",
            "",
            "[]",
            "[]",
            r#"{"capabilities":"bad","problems_solved":1,"use_when":{"a":true},"inspiration_for_fox":["不是字符串"]}"#,
        );
        insert_repo(
            &db.0,
            "topic",
            "fox/topic",
            "plain",
            r#"["tiktok"]"#,
            "active",
        );

        let stats = classify_repositories(&db.0, None).expect("坏 JSON 也不能让归类失败");
        assert_eq!(stats.repositories, 12);
        assert!(link_of(&db.0, "cap", "scn-xhs").is_some(), "能力名应命中");
        assert!(link_of(&db.0, "cap", "scn-img").is_none());
        assert!(link_of(&db.0, "empty-card", "scn-wx").is_some());
        assert!(link_of(&db.0, "bad-card", "scn-dy").is_some());
        assert!(link_of(&db.0, "problem", "scn-ec").is_some());
        assert!(link_of(&db.0, "when", "scn-data").is_some());
        assert!(link_of(&db.0, "idea", "scn-kb").is_some());
        assert!(link_of(&db.0, "kw", "scn-res").is_some());
        assert!(link_of(&db.0, "tag", "scn-vid").is_some());
        assert!(link_of(&db.0, "liner", "scn-wr").is_some());
        assert!(link_of(&db.0, "summary", "scn-auto").is_some());
        assert!(link_of(&db.0, "typed", "scn-ec").is_some());
        assert!(link_of(&db.0, "topic", "scn-dy").is_some());
        assert_eq!(
            count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios"),
            12
        );
    }

    #[test]
    fn reclassify_removes_stale_rule_suggestions() {
        let db = ready("stale");
        insert_repo(&db.0, "r", "fox/repo", "小红书", "[]", "active");
        classify_repositories(&db.0, None).expect("第一次归类");
        assert!(link_of(&db.0, "r", "scn-xhs").is_some());
        {
            let connection = open_db(&db.0).expect("应能打开测试库");
            connection
                .execute(
                    "UPDATE repositories SET description = '微信公众号' WHERE id = 'r'",
                    [],
                )
                .expect("应能改描述");
        }
        let stats = classify_repositories(&db.0, None).expect("第二次归类");
        assert_eq!(stats.removed, 1);
        assert!(
            link_of(&db.0, "r", "scn-xhs").is_none(),
            "不再命中的规则行应删掉"
        );
        let (origin, status, confidence) = link_of(&db.0, "r", "scn-wx").expect("应改挂到微信");
        assert_eq!(origin, "rule");
        assert_eq!(status, "suggested");
        assert_near(confidence, 0.6);
    }

    #[test]
    fn reclassify_keeps_protected_rows() {
        let db = ready("protected");
        insert_repo(&db.0, "keep", "fox/keep", "一份幻灯 小红书", "[]", "active");
        insert_link(&db.0, "keep", "scn-xhs", "rule", "confirmed", 0.42);
        insert_link(&db.0, "keep", "scn-wx", "rule", "rejected", 0.33);
        insert_link(&db.0, "keep", "scn-dy", "ai", "suggested", 0.66);
        insert_link(&db.0, "keep", "scn-img", "user", "confirmed", 1.0);
        insert_link(&db.0, "keep", "scn-vid", "rule", "suggested", 0.5);
        insert_link(&db.0, "keep", "scn-ppt", "rule", "suggested", 0.2);

        let stats = classify_repositories(&db.0, None).expect("归类应成功");
        assert_eq!(stats.assigned, 1, "只更新仍命中的规则建议");
        assert_eq!(stats.removed, 1, "不再命中的规则建议应清掉");
        assert_eq!(
            link_of(&db.0, "keep", "scn-xhs").expect("确认行应留下"),
            ("rule".to_owned(), "confirmed".to_owned(), 0.42)
        );
        assert_eq!(
            link_of(&db.0, "keep", "scn-wx").expect("拒绝行应留下"),
            ("rule".to_owned(), "rejected".to_owned(), 0.33)
        );
        assert_eq!(
            link_of(&db.0, "keep", "scn-dy").expect("AI 行应留下"),
            ("ai".to_owned(), "suggested".to_owned(), 0.66)
        );
        assert_eq!(
            link_of(&db.0, "keep", "scn-img").expect("用户行应留下"),
            ("user".to_owned(), "confirmed".to_owned(), 1.0)
        );
        assert!(link_of(&db.0, "keep", "scn-vid").is_none());
        let (origin, status, confidence) = link_of(&db.0, "keep", "scn-ppt").expect("幻灯应更新");
        assert_eq!(origin, "rule");
        assert_eq!(status, "suggested");
        assert_near(confidence, 0.5);
    }

    #[test]
    fn classify_again_changes_nothing_when_text_is_unchanged() {
        let db = ready("idempotent");
        insert_repo(&db.0, "r", "fox/repo", "小红书", "[]", "active");
        let first = classify_repositories(&db.0, None).expect("第一次");
        assert_eq!(first.assigned, 1);
        assert_eq!(first.removed, 0);
        let second = classify_repositories(&db.0, None).expect("第二次");
        assert_eq!(second.assigned, 0);
        assert_eq!(second.removed, 0);
        assert_eq!(count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios"), 1);
    }

    #[test]
    fn apply_ai_scenarios_replaces_suggestions_and_skips_protected() {
        let db = ready("ai");
        insert_repo(&db.0, "r", "fox/repo", "plain", "[]", "active");
        insert_link(&db.0, "r", "scn-xhs", "ai", "suggested", 0.4);
        insert_link(&db.0, "r", "scn-wx", "ai", "suggested", 0.4);
        insert_link(&db.0, "r", "scn-dy", "rule", "suggested", 0.5);
        insert_link(&db.0, "r", "scn-img", "rule", "confirmed", 0.5);
        insert_link(&db.0, "r", "scn-vid", "user", "suggested", 0.9);
        insert_link(&db.0, "r", "scn-ppt", "ai", "rejected", 0.4);
        insert_link(&db.0, "r", "scn-data", "ai", "confirmed", 0.8);
        {
            let connection = open_db(&db.0).expect("应能打开测试库");
            connection
                .execute("UPDATE scenarios SET archived = 1 WHERE slug = 'kb'", [])
                .expect("应能归档");
        }

        let mut connection = open_db(&db.0).expect("应能打开测试库");
        let slugs = ["xhs", "dy", "img", "vid", "ppt", "seo", "nope", "xhs", "kb"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let written =
            apply_ai_scenarios(&mut connection, "acct", "r", &slugs, 0.77).expect("AI 场景应写入");
        assert_eq!(written, 3, "xhs、dy、seo 写入；其余跳过或丢掉");
        drop(connection);

        let (origin, status, confidence) = link_of(&db.0, "r", "scn-xhs").expect("xhs");
        assert_eq!((origin, status), ("ai".to_owned(), "suggested".to_owned()));
        assert_near(confidence, 0.77);
        assert!(
            link_of(&db.0, "r", "scn-wx").is_none(),
            "不在新列表里的 AI 建议应删掉"
        );
        let (origin, status, confidence) = link_of(&db.0, "r", "scn-dy").expect("dy");
        assert_eq!((origin, status), ("ai".to_owned(), "suggested".to_owned()));
        assert_near(confidence, 0.77);
        assert_eq!(
            link_of(&db.0, "r", "scn-img").expect("已确认不改"),
            ("rule".to_owned(), "confirmed".to_owned(), 0.5)
        );
        assert_eq!(
            link_of(&db.0, "r", "scn-vid").expect("用户行不改"),
            ("user".to_owned(), "suggested".to_owned(), 0.9)
        );
        assert_eq!(
            link_of(&db.0, "r", "scn-ppt").expect("已拒绝不改"),
            ("ai".to_owned(), "rejected".to_owned(), 0.4)
        );
        assert_eq!(
            link_of(&db.0, "r", "scn-data").expect("已确认的 AI 行不删"),
            ("ai".to_owned(), "confirmed".to_owned(), 0.8)
        );
        let (origin, status, confidence) = link_of(&db.0, "r", "scn-seo").expect("seo");
        assert_eq!((origin, status), ("ai".to_owned(), "suggested".to_owned()));
        assert_near(confidence, 0.77);
        assert!(link_of(&db.0, "r", "scn-kb").is_none(), "已归档场景不写");

        let mut connection = open_db(&db.0).expect("应能打开测试库");
        let error = apply_ai_scenarios(&mut connection, "acct", "r", &slugs, 1.5)
            .expect_err("置信度越界应失败");
        assert!(error.contains("置信度"), "{error}");
        drop(connection);
        assert!(
            link_of(&db.0, "r", "scn-seo").is_some(),
            "失败不应清掉已有建议"
        );

        let mut connection = open_db(&db.0).expect("应能打开测试库");
        let written = apply_ai_scenarios(&mut connection, "acct", "r", &[], 0.5)
            .expect("空列表应清掉 AI 建议");
        assert_eq!(written, 0);
        drop(connection);
        assert!(link_of(&db.0, "r", "scn-xhs").is_none());
        assert!(link_of(&db.0, "r", "scn-dy").is_none());
        assert!(link_of(&db.0, "r", "scn-seo").is_none());
        assert!(link_of(&db.0, "r", "scn-data").is_some());
        assert!(link_of(&db.0, "r", "scn-ppt").is_some());
        assert!(link_of(&db.0, "r", "scn-img").is_some());
        assert!(link_of(&db.0, "r", "scn-vid").is_some());
    }

    #[test]
    fn list_summary_sorts_counts_thin_and_hides_archived() {
        let db = ready("list");
        {
            let connection = open_db(&db.0).expect("应能打开测试库");
            connection
                .execute("UPDATE scenarios SET archived = 1 WHERE slug = 'xhs'", [])
                .expect("应能归档");
        }
        for (id, status, sync_status) in [
            ("w1", "suggested", "active"),
            ("w2", "confirmed", "active"),
            ("w3", "rejected", "active"),
            ("w4", "suggested", "removed"),
        ] {
            insert_repo(&db.0, id, &format!("fox/{id}"), "plain", "[]", sync_status);
            insert_link(&db.0, id, "scn-wx", "rule", status, 0.5);
        }
        insert_repo(&db.0, "d1", "fox/d1", "plain", "[]", "active");
        insert_link(&db.0, "d1", "scn-data", "rule", "suggested", 0.5);
        for index in 0..14 {
            let id = format!("seo-{index}");
            insert_repo(&db.0, &id, &format!("fox/{id}"), "plain", "[]", "active");
            insert_link(&db.0, &id, "scn-seo", "user", "confirmed", 1.0);
        }
        for index in 0..15 {
            let id = format!("ec-{index}");
            insert_repo(&db.0, &id, &format!("fox/{id}"), "plain", "[]", "active");
            insert_link(&db.0, &id, "scn-ec", "rule", "confirmed", 1.0);
        }
        insert_link(&db.0, "w1", "scn-img", "rule", "suggested", 0.5);
        {
            let connection = open_db(&db.0).expect("应能打开测试库");
            connection
                .execute(
                    "UPDATE entity_scenarios SET entity_type = 'inbox_item' WHERE entity_id = 'w1' AND scenario_id = 'scn-img'",
                    [],
                )
                .expect("应能改成收集箱归类");
        }

        let listed = list_scenarios_summary(&db.0).expect("应能列出场景");
        let slugs: Vec<_> = listed.iter().map(|item| item.slug.as_str()).collect();
        assert_eq!(
            slugs,
            vec!["wx", "dy", "img", "vid", "ppt", "wr", "res", "data", "kb", "auto", "ec", "seo"]
        );
        assert!(listed
            .iter()
            .all(|item| item.id == format!("scn-{}", item.slug)));
        let wx = listed.iter().find(|item| item.slug == "wx").expect("wx");
        assert_eq!(wx.name_zh, "公众号·微信");
        assert_eq!(wx.sort_order, 2);
        assert_eq!(wx.count, 2, "拒绝和已移除的仓库不计数");
        assert!(!wx.thin);
        let data = listed
            .iter()
            .find(|item| item.slug == "data")
            .expect("data");
        assert_eq!(data.count, 1);
        assert!(data.thin);
        let seo = listed.iter().find(|item| item.slug == "seo").expect("seo");
        assert_eq!(seo.count, 14);
        assert!(seo.thin, "刚好等于阈值仍算偏少");
        let ec = listed.iter().find(|item| item.slug == "ec").expect("ec");
        assert_eq!(ec.count, 15);
        assert!(!ec.thin, "超过阈值不再标偏少");
        let img = listed.iter().find(|item| item.slug == "img").expect("img");
        assert_eq!(img.count, 0, "收集箱归类和零命中都不算仓库数");
        assert!(!img.thin, "不在 thin_focus 里的空场景不算偏少");
    }

    #[test]
    fn set_repository_scenario_confirms_rejects_and_undoes() {
        let db = ready("set-ok");
        insert_repo(&db.0, "r", "fox/repo", "plain", "[]", "active");
        set_repository_scenario(&db.0, "r", "scn-xhs", "confirmed").expect("应能确认");
        assert_eq!(
            link_of(&db.0, "r", "scn-xhs").expect("确认行"),
            ("user".to_owned(), "confirmed".to_owned(), 1.0)
        );
        set_repository_scenario(&db.0, "r", "scn-xhs", "rejected").expect("应能改成拒绝");
        assert_eq!(
            link_of(&db.0, "r", "scn-xhs").expect("拒绝行"),
            ("user".to_owned(), "rejected".to_owned(), 1.0)
        );
        set_repository_scenario(&db.0, "r", "scn-xhs", "suggested").expect("应能撤销");
        assert!(link_of(&db.0, "r", "scn-xhs").is_none());

        insert_link(&db.0, "r", "scn-wx", "rule", "suggested", 0.5);
        let error = set_repository_scenario(&db.0, "r", "scn-wx", "suggested")
            .expect_err("规则行不能用 suggested 撤销");
        assert_eq!(error, "这条归类不是你手动设置的");
        assert_eq!(
            link_of(&db.0, "r", "scn-wx").expect("规则行还在"),
            ("rule".to_owned(), "suggested".to_owned(), 0.5)
        );
        set_repository_scenario(&db.0, "r", "scn-wx", "confirmed").expect("确认应盖过规则");
        assert_eq!(
            link_of(&db.0, "r", "scn-wx").expect("变成用户确认"),
            ("user".to_owned(), "confirmed".to_owned(), 1.0)
        );
        set_repository_scenario(&db.0, "r", "scn-wx", "suggested").expect("撤销用户确认");
        assert!(link_of(&db.0, "r", "scn-wx").is_none());

        set_repository_scenario(&db.0, "r", "scn-dy", "rejected").expect("没有旧行也能拒绝");
        assert_eq!(
            link_of(&db.0, "r", "scn-dy").expect("用户拒绝"),
            ("user".to_owned(), "rejected".to_owned(), 1.0)
        );
    }

    #[test]
    fn set_repository_scenario_reports_chinese_errors() {
        let db = ready("set-err");
        insert_repo(&db.0, "r", "fox/repo", "plain", "[]", "active");
        let error = set_repository_scenario(&db.0, "missing", "scn-xhs", "confirmed")
            .expect_err("仓库不存在");
        assert_eq!(error, "仓库不存在");
        let error = set_repository_scenario(&db.0, "r", "scn-missing", "confirmed")
            .expect_err("场景不存在");
        assert_eq!(error, "场景不存在");
        let error =
            set_repository_scenario(&db.0, "r", "scn-xhs", "approve").expect_err("状态非法");
        assert_eq!(error, "状态只能是 confirmed、rejected 或 suggested");
        let error = set_repository_scenario(&db.0, "r", "scn-xhs", "suggested")
            .expect_err("没有用户行不能撤销");
        assert_eq!(error, "这条归类不是你手动设置的");
        assert_eq!(count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios"), 0);
    }

    #[test]
    fn classify_batch_rolls_back_when_one_write_fails() {
        let db = ready("rollback");
        insert_repo(&db.0, "a-ok", "fox/a-ok", "小红书", "[]", "active");
        insert_repo(&db.0, "b-fail", "fox/b-fail", "微信", "[]", "active");
        insert_repo(&db.0, "c-keep", "fox/c-keep", "保持不变", "[]", "active");
        insert_link(&db.0, "c-keep", "scn-ppt", "rule", "confirmed", 0.42);
        {
            let connection = open_db(&db.0).expect("应能打开测试库");
            connection
                .execute_batch(
                    "CREATE TRIGGER fsl_fail_repo BEFORE INSERT ON entity_scenarios
                     WHEN NEW.entity_id = 'b-fail'
                     BEGIN
                       SELECT RAISE(ABORT, '测试注入失败');
                     END;",
                )
                .expect("应能装上失败触发器");
        }
        let error = classify_repositories(&db.0, None).expect_err("中途失败应回滚");
        assert!(
            error.contains("测试注入失败"),
            "应碰到第二个仓库的写入失败：{error}"
        );
        assert!(
            link_of(&db.0, "a-ok", "scn-xhs").is_none(),
            "同一事务里先写入的归类不能留下"
        );
        assert_eq!(
            count_sql(
                &db.0,
                "SELECT COUNT(*) FROM entity_scenarios WHERE entity_id = 'b-fail'"
            ),
            0
        );
        assert_eq!(
            link_of(&db.0, "c-keep", "scn-ppt").expect("已提交的行还在"),
            ("rule".to_owned(), "confirmed".to_owned(), 0.42)
        );
    }

    #[test]
    fn scenario_summary_json_has_exact_contract_keys() {
        let summary = ScenarioSummary {
            id: "scn-xhs".to_owned(),
            slug: "xhs".to_owned(),
            name_zh: "小红书".to_owned(),
            sort_order: 1,
            count: 0,
            thin: false,
        };
        let value = serde_json::to_value(&summary).expect("应能序列化");
        let object = value.as_object().expect("应是对象");
        let mut keys: Vec<_> = object.keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec!["count", "id", "nameZh", "slug", "sortOrder", "thin"]
        );
        assert!(object["id"].is_string());
        assert!(object["slug"].is_string());
        assert!(object["nameZh"].is_string());
        assert!(object["sortOrder"].is_number());
        assert!(object["count"].is_number());
        assert!(object["thin"].is_boolean());
        assert_eq!(object["nameZh"], "小红书");
        assert_eq!(object["sortOrder"], 1);
        assert_eq!(object["count"], 0);
        assert_eq!(object["thin"], false);
    }

    #[test]
    fn classify_limits_to_requested_ids_and_skips_removed() {
        let db = ready("subset");
        insert_repo(&db.0, "active", "fox/active", "小红书", "[]", "active");
        insert_repo(&db.0, "gone", "fox/gone", "微信", "[]", "removed");
        insert_repo(&db.0, "other", "fox/other", "plain", "[]", "removed");
        insert_link(&db.0, "other", "scn-vid", "rule", "suggested", 0.5);

        let stats = classify_repositories(&db.0, None).expect("全量只处理有效仓库");
        assert_eq!(stats.repositories, 1);
        assert!(link_of(&db.0, "active", "scn-xhs").is_some());
        assert!(link_of(&db.0, "gone", "scn-wx").is_none());
        assert!(
            link_of(&db.0, "other", "scn-vid").is_some(),
            "没点名的仓库不动"
        );

        let ids = vec!["gone".to_owned()];
        let stats = classify_repositories(&db.0, Some(&ids)).expect("点名已移除的仓库");
        assert_eq!(stats.repositories, 1);
        assert!(link_of(&db.0, "gone", "scn-wx").is_some());
        assert!(link_of(&db.0, "active", "scn-xhs").is_some());
        assert!(link_of(&db.0, "other", "scn-vid").is_some());

        let stats = classify_repositories(&db.0, Some(&[])).expect("空名单不做任何事");
        assert_eq!(stats, ClassifyStats::default());
        assert_eq!(count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios"), 3);
    }

    #[test]
    fn eight_repository_fixture_matches_expected_scenarios() {
        let db = ready("fixture");
        let repos = [
            (
                "r1",
                "fox/xhs-poster",
                "小红书封面和海报生成",
                r#"["xiaohongshu","poster"]"#,
            ),
            ("r2", "fox/weixin-digest", "微信公众号周报", "[]"),
            ("r3", "fox/douyin-clips", "抖音口播剪辑", "[]"),
            ("r4", "fox/pptx-tools", "把幻灯片做成办公文档", "[]"),
            ("r5", "fox/plain-repo", "a generic rust parser", "[]"),
            ("r6", "fox/seo-lab", "SEO 增长、营销投放和获客", "[]"),
            (
                "r7",
                "fox/sql-dashboard",
                "数据分析报表",
                r#"["analytics"]"#,
            ),
            ("r8", "fox/n8n-shop", "用 n8n 做电商自动化和商品上架", "[]"),
        ];
        for (id, full_name, description, topics) in repos {
            insert_repo(&db.0, id, full_name, description, topics, "active");
        }
        insert_doc(
            &db.0,
            "r5",
            "",
            "",
            "[]",
            "[]",
            r#"{"capabilities":[{"name_zh":"用 RAG 检索笔记"}]}"#,
        );

        let expected = [
            (
                "fox/xhs-poster",
                "xhs",
                vec!["小红书", "xiaohongshu", "xhs"],
            ),
            ("fox/xhs-poster", "img", vec!["海报", "封面", "poster"]),
            ("fox/weixin-digest", "wx", vec!["公众号", "微信", "weixin"]),
            ("fox/douyin-clips", "dy", vec!["抖音", "douyin"]),
            ("fox/douyin-clips", "vid", vec!["剪辑"]),
            (
                "fox/pptx-tools",
                "ppt",
                vec!["ppt", "幻灯", "pptx", "办公文档"],
            ),
            ("fox/plain-repo", "kb", vec!["rag", "笔记"]),
            (
                "fox/seo-lab",
                "seo",
                vec!["seo", "增长", "营销", "投放", "获客"],
            ),
            (
                "fox/sql-dashboard",
                "data",
                vec!["数据分析", "analytics", "dashboard", "sql", "报表"],
            ),
            ("fox/n8n-shop", "auto", vec!["自动化", "n8n"]),
            ("fox/n8n-shop", "ec", vec!["电商", "商品"]),
        ]
        .into_iter()
        .map(|(full_name, slug, hits)| {
            (
                full_name.to_owned(),
                slug.to_owned(),
                hits.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

        let actual = {
            let connection = open_db(&db.0).expect("应能打开测试库");
            let scenarios = load_scenarios(&connection, "acct").expect("应能读场景");
            let repos = load_repos(&connection, "acct", None).expect("应能读仓库");
            let mut actual = Vec::new();
            for repo in &repos {
                let haystack = build_haystack(repo);
                for scenario in &scenarios {
                    let hits = keyword_hits(&haystack, &scenario.keywords);
                    if hits.is_empty() {
                        continue;
                    }
                    actual.push((repo.full_name.clone(), scenario.slug.clone(), hits));
                }
            }
            actual
        };
        assert_eq!(actual, expected, "命中词与夹具表不一致");

        let stats = classify_repositories(&db.0, None).expect("夹具应能归类");
        assert_eq!(stats.repositories, 8);
        assert_eq!(stats.assigned, 11);
        assert_eq!(stats.removed, 0);
        assert_eq!(
            stats.per_scenario,
            vec![
                ("xhs".to_owned(), 1),
                ("wx".to_owned(), 1),
                ("dy".to_owned(), 1),
                ("img".to_owned(), 1),
                ("vid".to_owned(), 1),
                ("ppt".to_owned(), 1),
                ("data".to_owned(), 1),
                ("kb".to_owned(), 1),
                ("auto".to_owned(), 1),
                ("ec".to_owned(), 1),
                ("seo".to_owned(), 1),
            ]
        );
        for (full_name, slug, hits) in &expected {
            let repo_id = match full_name.as_str() {
                "fox/xhs-poster" => "r1",
                "fox/weixin-digest" => "r2",
                "fox/douyin-clips" => "r3",
                "fox/pptx-tools" => "r4",
                "fox/plain-repo" => "r5",
                "fox/seo-lab" => "r6",
                "fox/sql-dashboard" => "r7",
                "fox/n8n-shop" => "r8",
                _ => panic!("未知仓库 {full_name}"),
            };
            let (origin, status, confidence) =
                link_of(&db.0, repo_id, &format!("scn-{slug}")).expect(full_name);
            assert_eq!(origin, "rule");
            assert_eq!(status, "suggested");
            assert_near(confidence, confidence_for(hits.len()));
        }
        assert_eq!(
            count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios"),
            11
        );
    }

    #[test]
    fn scenario_commands_are_allowed_and_registered() {
        let permissions = include_str!("../permissions/gsat-commands.toml");
        let manifest = include_str!("../gen/schemas/acl-manifests.json");
        let source = include_str!("lib.rs");
        let start = source
            .find("generate_handler![")
            .expect("应有 generate_handler");
        let end = source[start..].find("])").expect("generate_handler 应结束");
        let block = &source[start..start + end];
        for command in [
            "list_scenarios",
            "reclassify_scenarios",
            "set_repository_scenario",
        ] {
            assert!(
                permissions.contains(&format!("\"{command}\"")),
                "{command} 应在权限清单里"
            );
            assert!(
                manifest.contains(&format!("\"{command}\"")),
                "{command} 应在 ACL 里"
            );
            assert!(
                block.contains(command),
                "{command} 应登记到 generate_handler"
            );
        }
    }
}
