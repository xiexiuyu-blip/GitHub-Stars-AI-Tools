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
    /// 非空时：材料里必须字面出现其中一个词，这个场景才成立（平台类、自动化、SEO 这类容易乱贴的）。
    #[serde(default)]
    evidence: Vec<String>,
    /// 同上，但只有出现在名称 / 描述 / topics 里才算（README 里到处都是「自动化」，不能算证据）。
    #[serde(default)]
    evidence_meta: Vec<String>,
    /// 「给 / 不给」的口径，会原样放进理解卡提示词。
    #[serde(default)]
    description_zh: String,
}

#[derive(Debug)]
struct ScenarioRow {
    id: String,
    slug: String,
    keywords: Vec<String>,
    /// 来自种子（用户自己加的场景没有）：非空时材料里必须字面出现其中一个词。
    evidence: Evidence,
}

/// 一个场景的证据要求。两个列表都空 = 不要求证据。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Evidence {
    /// 名称、描述、topics 或 README 里出现都算。
    anywhere: Vec<String>,
    /// 只有出现在名称、描述、topics 里才算。
    meta_only: Vec<String>,
}

impl Evidence {
    fn required(&self) -> bool {
        !self.anywhere.is_empty() || !self.meta_only.is_empty()
    }

    /// `meta`：名称、描述、topics；`readme`：README 文字。都已转小写。
    /// 名称 / 描述 / topics 里出现一个证据词就算；README 里要出现至少 `README_EVIDENCE_MIN_MENTIONS` 次
    /// （「加入微信群」「cron 表达式」这种顺带一提，不能让一个开发工具变成「自动化工具」）。
    fn satisfied(&self, meta: &str, readme: &str) -> bool {
        if !self.required() {
            return true;
        }
        if self
            .anywhere
            .iter()
            .chain(self.meta_only.iter())
            .any(|word| matches_keyword(meta, word))
        {
            return true;
        }
        let mentions: usize = self
            .anywhere
            .iter()
            .map(|word| count_keyword(readme, word))
            .sum();
        mentions >= README_EVIDENCE_MIN_MENTIONS
    }
}

/// 规则归类只读「仓库自己写的」文字：名称、描述、topics，加 README 开头一小段。
/// 不读 AI 写的中文说明 / 关键词 / 能力 / 分类名：它们会把「无广告」「视频号」「Notion 替代品」这类说法带成场景。
#[derive(Debug)]
struct RepoText {
    id: String,
    full_name: String,
    description: String,
    topics_json: String,
    readme_head: String,
}

/// README 开头取多少字参与规则归类。
const README_RULE_CHARS: usize = 1_500;
/// 判断「场景有没有证据」时读 README 的多少字（比归类宽松：证据可以出现在功能列表里，但只读前面这些）。
const README_EVIDENCE_CHARS: usize = 8_000;
/// README 里的证据词至少出现这么多次才算（只在名称 / 描述 / topics 里出现一次就够）：一次顺带提到不算。
const README_EVIDENCE_MIN_MENTIONS: usize = 2;

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

/// 种子写入的结果：新插入几行、更新了几行（名称、关键词、排序或口径说明变了）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SeedReport {
    pub inserted: usize,
    pub updated: usize,
}

/// 幂等写入场景种子。只返回新插入的行数；已有的 seed 行若名称、关键词、排序或口径说明变了就更新。
/// `origin='user'` 和 `archived=1` 的行保持原样。
pub fn seed_scenarios(path: &Path) -> Result<usize, String> {
    seed_scenarios_report(path).map(|report| report.inserted)
}

/// 同 `seed_scenarios`，另外告诉调用方有没有更新过已有的行（更新了关键词就该重新做一遍规则归类）。
pub fn seed_scenarios_report(path: &Path) -> Result<SeedReport, String> {
    let seed = load_seed()?;
    let mut connection = open_db(path)?;
    let account = account_id(&connection)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("场景种子事务开启失败：{error}"))?;
    let mut inserted = 0;
    let mut updated = 0;
    for scenario in &seed.scenarios {
        let id = format!("scn-{}", scenario.slug);
        let keywords = serde_json::to_string(&scenario.keywords)
            .map_err(|error| format!("场景关键词序列化失败：{error}"))?;
        let existing: Option<(String, String, i64, i64, String, String)> = tx
            .query_row(
                "SELECT origin, name_zh, sort_order, archived, keywords_json, description_zh FROM scenarios WHERE account_id = ?1 AND slug = ?2",
                params![account, scenario.slug],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .optional()
            .map_err(|error| format!("场景读取失败：{error}"))?;
        match existing {
            None => {
                tx.execute(
                    "INSERT INTO scenarios(id, account_id, slug, name_zh, keywords_json, sort_order, origin, archived, description_zh) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'seed', 0, ?7)",
                    params![id, account, scenario.slug, scenario.name_zh, keywords, scenario.sort_order, scenario.description_zh],
                )
                .map_err(|error| format!("场景种子写入失败：{error}"))?;
                inserted += 1;
            }
            Some((origin, name, sort_order, archived, stored_keywords, stored_description)) => {
                if origin != "seed" || archived != 0 {
                    continue;
                }
                if name == scenario.name_zh
                    && sort_order == scenario.sort_order
                    && same_keywords(&stored_keywords, &scenario.keywords)
                    && stored_description == scenario.description_zh
                {
                    continue;
                }
                tx.execute(
                    "UPDATE scenarios SET name_zh = ?1, keywords_json = ?2, sort_order = ?3, description_zh = ?4, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE account_id = ?5 AND slug = ?6 AND origin = 'seed' AND archived = 0",
                    params![scenario.name_zh, keywords, scenario.sort_order, scenario.description_zh, account, scenario.slug],
                )
                .map_err(|error| format!("场景种子更新失败：{error}"))?;
                updated += 1;
            }
        }
    }
    tx.commit()
        .map_err(|error| format!("场景种子提交失败：{error}"))?;
    Ok(SeedReport { inserted, updated })
}

/// 种子里每个场景的证据要求（没写就是不要求）。用户自己加的场景不在种子里，也不要求。
fn seed_evidence(slug: &str) -> Evidence {
    static EVIDENCE: std::sync::OnceLock<std::collections::HashMap<String, Evidence>> =
        std::sync::OnceLock::new();
    let clean = |words: &[String]| {
        words
            .iter()
            .map(|word| word.trim().to_lowercase())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
    };
    EVIDENCE
        .get_or_init(|| {
            load_seed()
                .map(|seed| {
                    seed.scenarios
                        .into_iter()
                        .map(|scenario| {
                            let evidence = Evidence {
                                anywhere: clean(&scenario.evidence),
                                meta_only: clean(&scenario.evidence_meta),
                            };
                            (scenario.slug, evidence)
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .get(slug)
        .cloned()
        .unwrap_or_default()
}

/// 种子里写的「给 / 不给」口径（给理解卡提示词用）；没有就是 `None`。
pub fn seed_description(slug: &str) -> Option<String> {
    let seed = load_seed().ok()?;
    seed.scenarios
        .into_iter()
        .find(|scenario| scenario.slug == slug)
        .map(|scenario| scenario.description_zh)
        .filter(|text| !text.trim().is_empty())
}

/// 这个场景在给定材料里有没有证据：种子没要求证据的场景一律算有。
/// `meta_lower`：名称、描述、topics；`readme_lower`：README 文字（都已转小写）。
pub fn has_evidence(slug: &str, meta_lower: &str, readme_lower: &str) -> bool {
    seed_evidence(slug).satisfied(meta_lower, readme_lower)
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

/// 关键词在文本里出现几次（不重叠）。边界规则和 `matches_keyword` 一样：短的纯 ASCII 词只从词首匹配。
fn count_keyword(haystack_lower: &str, keyword_lower: &str) -> usize {
    let haystack = haystack_lower.to_lowercase();
    let keyword = keyword_lower.trim().to_lowercase();
    if keyword.is_empty() {
        return 0;
    }
    let short_token =
        keyword.len() <= 4 && keyword.bytes().all(|byte| byte.is_ascii_alphanumeric());
    let hay = haystack.as_bytes();
    let needle = keyword.as_bytes();
    let mut count = 0;
    let mut index = 0;
    while index + needle.len() <= hay.len() {
        if &hay[index..index + needle.len()] == needle {
            let before_ok = !short_token || index == 0 || !hay[index - 1].is_ascii_alphanumeric();
            if before_ok {
                count += 1;
                index += needle.len();
                continue;
            }
        }
        index += 1;
    }
    count
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

/// 仓库自己写的文字：名称、描述、topics（转小写）。命中这里的一个词就够。
fn meta_haystack(repo: &RepoText) -> String {
    let mut parts = Vec::new();
    push_text(&mut parts, &repo.full_name);
    push_text(&mut parts, &repo.description);
    push_json_strings(&mut parts, &repo.topics_json);
    parts.join(" ").to_lowercase()
}

/// README 开头一小段（转小写）。只在它里面命中，要有两个不同的词才算候选。
fn readme_haystack(repo: &RepoText) -> String {
    repo.readme_head.to_lowercase()
}

/// 一个场景在这个仓库上的规则命中词；`None` 表示不成为候选。
/// 规则：命中名称 / 描述 / topics 里的一个词，或 README 开头里有两个不同的词；
/// 再加上种子要求的「证据词」必须字面出现（平台类、自动化、SEO）。
fn rule_hits(scenario: &ScenarioRow, meta: &str, readme: &str) -> Option<Vec<String>> {
    let in_meta = keyword_hits(meta, &scenario.keywords);
    let mut all = in_meta.clone();
    for hit in keyword_hits(readme, &scenario.keywords) {
        if !all.contains(&hit) {
            all.push(hit);
        }
    }
    if in_meta.is_empty() && all.len() < 2 {
        return None;
    }
    if scenario.evidence.required() {
        // 平台类、自动化、SEO：README 开头常有「微信群」「抖音号」「加入社群」这类入口，README 里的命中不单独成立
        if in_meta.is_empty() || !scenario.evidence.satisfied(meta, readme) {
            return None;
        }
    }
    Some(all)
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
        let evidence = seed_evidence(&slug);
        scenarios.push(ScenarioRow {
            id,
            slug,
            keywords,
            evidence,
        });
    }
    Ok(scenarios)
}

const REPO_SELECT: &str = "
SELECT r.id, r.full_name, IFNULL(r.description, ''), IFNULL(r.topics_json, '[]'),
       IFNULL(substr(rd.raw_markdown, 1, 1500), '')
FROM repositories r
LEFT JOIN repo_readmes rd ON rd.repo_id = r.id
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
        readme_head: row.get(4)?,
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
        let meta = meta_haystack(repo);
        let readme = readme_haystack(repo);
        let mut matched = Vec::new();
        for (index, scenario) in scenarios.iter().enumerate() {
            let Some(hits) = rule_hits(scenario, &meta, &readme) else {
                continue;
            };
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

/// 启动时调用：补种子；如果库里还没有任何规则归类（第一次用），给全部有效仓库做一次规则归类。
/// 已经归类过的库不重做（新收藏同步后单独归类），所以不会每次启动都写库。返回 `None` 表示这次没有重做。
/// 规则归类不联网、不读钥匙串、不花钱。
pub fn ensure_initial_classification(path: &Path) -> Result<Option<ClassifyStats>, String> {
    let seeded = seed_scenarios_report(path)?;
    let existing: i64 = {
        let connection = open_db(path)?;
        connection
            .query_row(
                "SELECT COUNT(*) FROM entity_scenarios WHERE entity_type = 'repository' AND origin = 'rule'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("规则归类计数失败：{error}"))?
    };
    // 关键词、证据词或口径更新过（种子换了版本）：旧的规则建议按新规矩重做一遍（`delete_stale` 会清掉不再成立的）
    let stats = if existing > 0 && seeded.updated == 0 {
        None
    } else {
        Some(classify_repositories(path, None)?)
    };
    // AI 给过的、没有证据的平台 / 自动化 / SEO 标签：去掉（不花钱，每次启动都核一遍，没有要去的就什么也不写）
    audit_scenario_tags(path)?;
    Ok(stats)
}

/// 同步之后给新出现的仓库补种子并做规则归类。没有新仓库时什么都不做。
pub fn classify_new_repositories(
    path: &Path,
    repo_ids: &[String],
) -> Result<ClassifyStats, String> {
    if repo_ids.is_empty() {
        return Ok(ClassifyStats::default());
    }
    seed_scenarios(path)?;
    classify_repositories(path, Some(repo_ids))
}

/// 判断「场景有没有证据」用的材料（都转小写）：(名称 + 描述 + topics, README 前 `README_EVIDENCE_CHARS` 字)。
fn evidence_material(connection: &Connection, repo_id: &str) -> Result<(String, String), String> {
    let row: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT r.full_name, IFNULL(r.description, ''), IFNULL(r.topics_json, '[]'), IFNULL(substr(rd.raw_markdown, 1, ?2), '')
             FROM repositories r LEFT JOIN repo_readmes rd ON rd.repo_id = r.id WHERE r.id = ?1",
            params![repo_id, README_EVIDENCE_CHARS as i64],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| format!("证据材料读取失败：{error}"))?;
    let Some((name, description, topics, readme)) = row else {
        return Ok((String::new(), String::new()));
    };
    let mut parts = Vec::new();
    push_text(&mut parts, &name);
    push_text(&mut parts, &description);
    push_json_strings(&mut parts, &topics);
    Ok((parts.join(" ").to_lowercase(), readme.to_lowercase()))
}

/// 把 AI 给的场景 slug 过一遍证据：种子要求证据的场景（平台类、自动化、SEO），材料里没有字面出现证据词就丢掉。
/// 没要求证据的场景、用户自己加的场景原样保留。
pub fn supported_slugs(
    connection: &Connection,
    repo_id: &str,
    slugs: &[String],
) -> Result<Vec<String>, String> {
    if slugs.iter().all(|slug| !seed_evidence(slug).required()) {
        return Ok(slugs.to_vec());
    }
    let (meta, readme) = evidence_material(connection, repo_id)?;
    Ok(slugs
        .iter()
        .filter(|slug| has_evidence(slug, &meta, &readme))
        .cloned()
        .collect())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditStats {
    /// 核对了几条（只看要求证据的场景上、没被确认的 AI / 规则建议）。
    pub checked: usize,
    pub removed: usize,
    pub per_scenario: Vec<(String, usize)>,
}

/// 不花钱的复核：已经写在库里的、要求证据的场景（平台类、自动化、SEO）建议，材料里找不到证据词的就删掉。
/// 只动 `origin` 是 `ai` / `rule`、`status` 是 `suggested` 的；用户确认、拒绝或手动加的不碰。
/// 重复跑没有副作用（删完就没有可删的了）。
pub fn audit_scenario_tags(path: &Path) -> Result<AuditStats, String> {
    let seed = load_seed()?;
    let risky: Vec<String> = seed
        .scenarios
        .iter()
        .filter(|scenario| !scenario.evidence.is_empty() || !scenario.evidence_meta.is_empty())
        .map(|scenario| scenario.slug.clone())
        .collect();
    if risky.is_empty() {
        return Ok(AuditStats::default());
    }
    let mut connection = open_db(path)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("场景复核事务开启失败：{error}"))?;
    let sql = format!(
        "SELECT es.entity_id, es.scenario_id, s.slug FROM entity_scenarios es
         JOIN scenarios s ON s.id = es.scenario_id
         WHERE es.entity_type = 'repository' AND es.origin IN ('ai', 'rule') AND es.status = 'suggested'
           AND s.slug IN ({}) ORDER BY es.entity_id, s.slug",
        placeholders(risky.len())
    );
    let rows: Vec<(String, String, String)> = {
        let mut statement = tx
            .prepare(&sql)
            .map_err(|error| format!("场景复核读取失败：{error}"))?;
        let mapped = statement
            .query_map(params_from_iter(risky.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|error| format!("场景复核查询失败：{error}"))?;
        mapped
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("场景复核解析失败：{error}"))?
    };
    let mut stats = AuditStats {
        checked: rows.len(),
        ..AuditStats::default()
    };
    let mut per: std::collections::BTreeMap<String, usize> = Default::default();
    let mut current: Option<(String, (String, String))> = None;
    for (repo_id, scenario_id, slug) in rows {
        let material = match &current {
            Some((id, text)) if *id == repo_id => text.clone(),
            _ => {
                let text = evidence_material(&tx, &repo_id)?;
                current = Some((repo_id.clone(), text.clone()));
                text
            }
        };
        if has_evidence(&slug, &material.0, &material.1) {
            continue;
        }
        tx.execute(
            "DELETE FROM entity_scenarios WHERE entity_type = 'repository' AND entity_id = ?1 AND scenario_id = ?2 AND origin IN ('ai', 'rule') AND status = 'suggested'",
            params![repo_id, scenario_id],
        )
        .map_err(|error| format!("场景复核删除失败：{error}"))?;
        stats.removed += 1;
        *per.entry(slug).or_default() += 1;
    }
    tx.commit()
        .map_err(|error| format!("场景复核提交失败：{error}"))?;
    stats.per_scenario = per.into_iter().collect();
    Ok(stats)
}

/// 替换这个仓库上 `origin='ai'` 且 `status='suggested'` 的建议。
/// 已确认、已拒绝，以及 `origin='user'` 的场景跳过；不认识或已归档的 slug 丢掉。
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
    // 平台类、自动化、SEO 要有字面证据；AI 凭联想给的这类标签在这里就丢掉
    let slugs = supported_slugs(connection, repo_id, slugs)?;
    let slugs = slugs.as_slice();
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

    fn insert_readme(path: &Path, repo_id: &str, markdown: &str) {
        let connection = open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO repo_readmes(repo_id, raw_markdown, content_hash, source_path, fetched_at) VALUES (?1, ?2, 'hash', 'README.md', '2026-09-01T00:00:00Z')",
                params![repo_id, markdown],
            )
            .expect("应能写入 README");
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
    fn rules_read_only_what_the_repository_itself_says() {
        let db = ready("sources");
        // 名称、描述、topics：一个词就够
        insert_repo(
            &db.0,
            "name",
            "fox/xiaohongshu-kit",
            "plain",
            "[]",
            "active",
        );
        insert_repo(&db.0, "desc", "fox/desc", "微信", "[]", "active");
        insert_repo(
            &db.0,
            "topic",
            "fox/topic",
            "plain",
            r#"["tiktok"]"#,
            "active",
        );
        // README 开头：两个不同的词才算
        insert_repo(&db.0, "readme2", "fox/readme2", "plain", "[]", "active");
        insert_readme(&db.0, "readme2", "# 工具\n用来做海报，也能做封面。");
        insert_repo(&db.0, "readme1", "fox/readme1", "plain", "[]", "active");
        insert_readme(&db.0, "readme1", "# 工具\n顺带能做海报。");
        // README 很长时，只看开头 1500 字：后面再多的词也不算
        insert_repo(&db.0, "late", "fox/late", "plain", "[]", "active");
        let late = format!("{}海报和封面", "说明。".repeat(600));
        insert_readme(&db.0, "late", &late);
        // AI 写的文字一概不读：摘要、一句话、关键词、建议标签、能力、问题、使用时机、灵感
        insert_repo(&db.0, "ai", "fox/ai", "plain", "[]", "active");
        insert_doc(
            &db.0,
            "ai",
            "做小红书选题的自动化脚本",
            "这是写作助手",
            r#"["爬虫","抖音"]"#,
            r#"["字幕","知识库"]"#,
            r#"{"capabilities":[{"name_zh":"整理小红书选题"}],"problems_solved":["想做电商详情"],"use_when":["做数据分析的时候"],"inspiration_for_fox":"可以借鉴知识库结构"}"#,
        );
        // 坏 JSON、空串、类型不对的理解卡也不会让整批归类失败
        insert_repo(&db.0, "bad-card", "fox/badcard", "抖音", "[]", "active");
        insert_doc(&db.0, "bad-card", "", "", "[]", "[]", "{not json");

        let stats = classify_repositories(&db.0, None).expect("坏 JSON 也不能让归类失败");
        assert_eq!(stats.repositories, 8);
        assert!(link_of(&db.0, "name", "scn-xhs").is_some(), "名称里的词");
        assert!(link_of(&db.0, "desc", "scn-wx").is_some(), "描述里的词");
        assert!(link_of(&db.0, "topic", "scn-dy").is_some(), "topics 里的词");
        assert!(
            link_of(&db.0, "readme2", "scn-img").is_some(),
            "README 里两个不同的词"
        );
        assert!(
            link_of(&db.0, "readme1", "scn-img").is_none(),
            "README 里只有一个词不够"
        );
        assert!(
            link_of(&db.0, "late", "scn-img").is_none(),
            "1500 字之后的不算"
        );
        assert!(link_of(&db.0, "bad-card", "scn-dy").is_some());
        let ai_links = count_sql(
            &db.0,
            "SELECT COUNT(*) FROM entity_scenarios WHERE entity_id = 'ai'",
        );
        assert_eq!(ai_links, 0, "AI 写的文字不参与规则归类");
        assert_eq!(count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios"), 5);
    }

    #[test]
    fn the_things_that_used_to_misfire_no_longer_do() {
        // 审读 50 个仓库时找到的真实错标：「无广告」→ SEO·增长、「视频号」「流媒体视频」→ 视频、「Notion 替代品」→ 知识库、
        // 「微信读书」之外的 Wechat 通用库不在这里测；这里只测词表收紧：广告、增长、营销、视频、笔记、notion、sql、dashboard、workflow
        let db = ready("misfire");
        let cases = [
            (
                "ad",
                "fox/downloader",
                "界面精美，永久免费且无广告",
                "scn-seo",
            ),
            ("grow", "fox/grow", "增长很快的营销小工具", "scn-seo"),
            (
                "video",
                "fox/streamer",
                "多协议下载器，支持流媒体视频",
                "scn-vid",
            ),
            ("channel", "fox/live", "支持视频号和快手小店", "scn-vid"),
            ("notion", "fox/huly", "Notion 替代品，团队协作", "scn-kb"),
            ("notes", "fox/xhs-mcp", "发布小红书笔记", "scn-kb"),
            ("sql", "fox/dash", "一个 sql dashboard", "scn-data"),
            (
                "flow",
                "fox/flow",
                "一个 workflow automation 的小库，自动化",
                "scn-auto",
            ),
            ("logo", "fox/logo", "项目 logo 设计规范", "scn-img"),
        ];
        for (id, full_name, description, _) in cases {
            insert_repo(&db.0, id, full_name, description, "[]", "active");
        }
        classify_repositories(&db.0, None).expect("归类应成功");
        for (id, _, description, scenario) in cases {
            let link = link_of(&db.0, id, scenario);
            // 唯一例外：「workflow automation」是自动化词表里明写的短语，该命中
            if id == "flow" {
                assert!(link.is_some(), "{description} 应命中 {scenario}");
            } else {
                assert!(link.is_none(), "{description} 不该命中 {scenario}");
            }
        }
        // 小红书这个平台名本身还在：「发布小红书笔记」归小红书，不归知识库
        assert!(link_of(&db.0, "notes", "scn-xhs").is_some());
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
    fn ai_tags_without_literal_evidence_are_dropped_but_plain_ones_stay() {
        let db = ready("ai-evidence");
        insert_repo(
            &db.0,
            "r",
            "fox/repo",
            "做长视频剪辑和字幕的工具",
            "[]",
            "active",
        );
        let mut connection = open_db(&db.0).expect("应能打开测试库");
        let slugs = ["xhs", "wx", "dy", "auto", "seo", "vid", "img"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let written =
            apply_ai_scenarios(&mut connection, "acct", "r", &slugs, 0.8).expect("应写入");
        assert_eq!(written, 2, "只有没要求证据的 vid、img 留下");
        drop(connection);
        for gone in ["xhs", "wx", "dy", "auto", "seo"] {
            assert!(
                link_of(&db.0, "r", &format!("scn-{gone}")).is_none(),
                "{gone}"
            );
        }
        assert!(link_of(&db.0, "r", "scn-vid").is_some());
        assert!(link_of(&db.0, "r", "scn-img").is_some());
        // README 里有证据也算：材料是名称 + 描述 + topics + README
        insert_readme(
            &db.0,
            "r",
            "支持把成品发布到小红书，小红书笔记也能定时发。也能接 n8n，n8n 流程模板。",
        );
        let mut connection = open_db(&db.0).expect("应能打开测试库");
        let written =
            apply_ai_scenarios(&mut connection, "acct", "r", &slugs, 0.8).expect("应写入");
        assert_eq!(written, 4, "xhs、auto 有了证据");
        drop(connection);
        assert!(link_of(&db.0, "r", "scn-xhs").is_some());
        assert!(link_of(&db.0, "r", "scn-auto").is_some());
        assert!(link_of(&db.0, "r", "scn-wx").is_none());
    }

    #[test]
    fn the_audit_removes_only_unsupported_unconfirmed_ai_and_rule_tags() {
        let db = ready("audit");
        insert_repo(&db.0, "bare", "fox/bare", "一个通用的库", "[]", "active");
        insert_repo(&db.0, "kept", "fox/kept", "做小红书封面", "[]", "active");
        // 没证据的：ai / rule 建议 → 删
        insert_link(&db.0, "bare", "scn-xhs", "ai", "suggested", 0.9);
        insert_link(&db.0, "bare", "scn-auto", "rule", "suggested", 0.5);
        insert_link(&db.0, "bare", "scn-seo", "ai", "suggested", 0.7);
        // 没证据但被 Fox 确认 / 拒绝过，或者是他自己加的 → 不碰
        insert_link(&db.0, "bare", "scn-wx", "ai", "confirmed", 0.7);
        insert_link(&db.0, "bare", "scn-dy", "user", "suggested", 0.9);
        insert_link(&db.0, "bare", "scn-img", "ai", "suggested", 0.6);
        // 有证据的 → 留
        insert_link(&db.0, "kept", "scn-xhs", "ai", "suggested", 0.9);
        let stats = audit_scenario_tags(&db.0).expect("复核应成功");
        assert_eq!(
            stats.checked, 4,
            "只核要求证据的场景上、没被确认的 ai / rule 建议"
        );
        assert_eq!(stats.removed, 3);
        assert_eq!(
            stats.per_scenario,
            vec![
                ("auto".to_owned(), 1),
                ("seo".to_owned(), 1),
                ("xhs".to_owned(), 1)
            ]
        );
        assert!(link_of(&db.0, "bare", "scn-xhs").is_none());
        assert!(link_of(&db.0, "bare", "scn-auto").is_none());
        assert!(link_of(&db.0, "bare", "scn-seo").is_none());
        assert!(link_of(&db.0, "bare", "scn-wx").is_some(), "确认过的不碰");
        assert!(link_of(&db.0, "bare", "scn-dy").is_some(), "用户加的不碰");
        assert!(
            link_of(&db.0, "bare", "scn-img").is_some(),
            "不要求证据的场景不核"
        );
        assert!(link_of(&db.0, "kept", "scn-xhs").is_some());
        // 再跑一遍什么也不删
        let again = audit_scenario_tags(&db.0).expect("复核应成功");
        assert_eq!(again.removed, 0);
    }

    #[test]
    fn a_changed_seed_redoes_the_rule_pass_and_unchanged_does_not() {
        let db = ready("reseed");
        insert_repo(&db.0, "r", "fox/repo", "小红书", "[]", "active");
        // 第一次：没有任何规则归类，做一遍
        assert!(ensure_initial_classification(&db.0)
            .expect("启动")
            .is_some());
        // 种子没变：不重做
        assert!(ensure_initial_classification(&db.0)
            .expect("启动")
            .is_none());
        // 种子里的口径说明被改过（比如旧库是 v1 种子）：重做
        {
            let connection = open_db(&db.0).expect("应能打开测试库");
            connection
                .execute(
                    "UPDATE scenarios SET description_zh = '' WHERE slug = 'auto'",
                    [],
                )
                .expect("应能改");
        }
        let stats = ensure_initial_classification(&db.0)
            .expect("启动")
            .expect("要重做");
        assert_eq!(stats.repositories, 1);
        let description: String = open_db(&db.0)
            .expect("应能打开测试库")
            .query_row(
                "SELECT description_zh FROM scenarios WHERE slug = 'auto'",
                [],
                |row| row.get(0),
            )
            .expect("应读到");
        assert!(description.contains("只在两种情况给"), "{description}");
    }

    #[test]
    fn seed_evidence_and_descriptions_are_there_for_the_risky_scenarios() {
        for slug in ["xhs", "wx", "dy", "auto", "seo"] {
            assert!(seed_evidence(slug).required(), "{slug} 要有证据词");
        }
        for slug in ["img", "vid", "ppt", "wr", "res", "data", "kb", "ec"] {
            assert!(!seed_evidence(slug).required(), "{slug} 不要求证据");
        }
        for slug in [
            "xhs", "wx", "dy", "img", "vid", "ppt", "wr", "res", "data", "kb", "auto", "ec", "seo",
        ] {
            assert!(seed_description(slug).is_some(), "{slug} 要有口径说明");
        }
        assert!(seed_description("nope").is_none());
        assert!(has_evidence("auto", "用 n8n 做的小工具", ""));
        assert!(
            has_evidence("auto", "", "支持 n8n 和 zapier"),
            "强证据词 README 里出现两次也算"
        );
        assert!(
            !has_evidence("auto", "ai 编程助手", "可以配合 cron 表达式使用"),
            "README 里顺带提到一次不算"
        );
        assert!(
            has_evidence("auto", "", "n8n 流程 n8n 模板"),
            "同一个词出现两次也算"
        );
        assert!(!has_evidence("auto", "ai 编程助手的配置切换工具", ""));
        // 「自动化」这种泛词：名称 / 描述里有才算，只在 README 里出现不算
        assert!(has_evidence("auto", "直播带货自动化辅助工具", ""));
        assert!(has_evidence(
            "auto",
            "make websites accessible. automate tasks online",
            ""
        ));
        assert!(!has_evidence(
            "auto",
            "ai 编程助手",
            "支持自动化测试和 automation"
        ));
        assert!(
            has_evidence("img", "什么都没有", ""),
            "不要求证据的场景一律算有"
        );
        assert!(
            has_evidence("user-made", "什么都没有", ""),
            "用户自己加的场景也算有"
        );
    }

    #[test]
    fn readme_community_links_do_not_make_platform_tags() {
        let db = ready("community");
        // README 开头常有「微信群」「WeChat 交流群」「抖音号」这类入口：两个不同的词也不能单独造出平台标签
        insert_repo(
            &db.0,
            "doc",
            "fox/docqa",
            "document QA system",
            "[]",
            "active",
        );
        insert_readme(
            &db.0,
            "doc",
            "# DocQA\n加入微信群交流 | WeChat group | 关注我们的抖音号",
        );
        // 名称 / 描述里写了平台：照常成立
        insert_repo(
            &db.0,
            "real",
            "fox/real",
            "微信群聊日报整理",
            "[]",
            "active",
        );
        // 非平台类的场景（海报）：README 里两个不同的词照旧算
        insert_readme(&db.0, "real", "做海报，也能做封面");
        classify_repositories(&db.0, None).expect("归类应成功");
        assert!(link_of(&db.0, "doc", "scn-wx").is_none());
        assert!(link_of(&db.0, "doc", "scn-dy").is_none());
        assert!(link_of(&db.0, "real", "scn-wx").is_some());
        assert!(link_of(&db.0, "real", "scn-img").is_some());
    }

    #[test]
    fn counting_keywords_follows_the_same_boundaries_as_matching() {
        assert_eq!(count_keyword("n8n n8n", "n8n"), 2);
        assert_eq!(
            count_keyword("rpa rpaa xrpa", "rpa"),
            2,
            "短词只从词首：xrpa 不算，rpaa 算"
        );
        assert_eq!(count_keyword("小红书和小红书", "小红书"), 2);
        assert_eq!(count_keyword("aaa", "aa"), 1, "不重叠");
        assert_eq!(count_keyword("abc", ""), 0);
        assert_eq!(count_keyword("", "abc"), 0);
    }

    #[test]
    fn a_tool_that_calls_itself_automation_keeps_the_automation_tag() {
        let db = ready("auto-meta");
        // 自己的描述里写了「自动化」：AI 给的自动化标签有证据
        insert_repo(
            &db.0,
            "live",
            "fox/live-tool",
            "多平台直播带货自动化辅助工具",
            "[]",
            "active",
        );
        // 只有 README 里提到「自动化测试」的开发工具：没有
        insert_repo(
            &db.0,
            "dev",
            "fox/dev-tool",
            "AI 编程助手的配置切换",
            "[]",
            "active",
        );
        insert_readme(&db.0, "dev", "支持自动化测试，automation friendly");
        let mut connection = open_db(&db.0).expect("应能打开测试库");
        let slugs = vec!["auto".to_owned()];
        assert_eq!(
            apply_ai_scenarios(&mut connection, "acct", "live", &slugs, 0.8).expect("应写入"),
            1
        );
        assert_eq!(
            apply_ai_scenarios(&mut connection, "acct", "dev", &slugs, 0.8).expect("应写入"),
            0
        );
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
        // 小红书、抖音、SEO 要有字面证据：描述里写上
        insert_repo(
            &db.0,
            "r",
            "fox/repo",
            "小红书 抖音 SEO 工具",
            "[]",
            "active",
        );
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
        // 只有 README 里命中时，要有两个不同的词才算候选（一个不够）
        insert_readme(
            &db.0,
            "r5",
            "# parser\n一个解析器，也可以接到知识库里做 RAG 检索。",
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
            ("fox/plain-repo", "kb", vec!["知识库", "rag"]),
            ("fox/seo-lab", "seo", vec!["seo", "投放", "获客"]),
            (
                "fox/sql-dashboard",
                "data",
                vec!["数据分析", "analytics", "报表"],
            ),
            ("fox/n8n-shop", "auto", vec!["n8n"]),
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
                let meta = meta_haystack(repo);
                let readme = readme_haystack(repo);
                for scenario in &scenarios {
                    let Some(hits) = rule_hits(scenario, &meta, &readme) else {
                        continue;
                    };
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

    #[test]
    fn initial_classification_runs_once_and_keeps_user_choices() {
        let db = temp_db("initial-classify");
        insert_account(&db.0);
        insert_repo(&db.0, "xhs", "fox/xhs-tool", "小红书 封面", "[]", "active");
        insert_repo(
            &db.0,
            "plain",
            "fox/plain",
            "nothing relevant here",
            "[]",
            "active",
        );
        // 用户先手工放了一条（没有任何规则归类）：这条必须原样保留。
        seed_scenarios(&db.0).expect("种子");
        {
            let connection = open_db(&db.0).expect("打开");
            connection
                .execute(
                    "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, origin, confidence, status) VALUES ('acct', 'repository', 'plain', 'scn-wx', 'user', 1.0, 'confirmed')",
                    [],
                )
                .expect("手工归类");
        }
        let first = ensure_initial_classification(&db.0)
            .expect("第一次应成功")
            .expect("第一次应做归类");
        assert_eq!(first.repositories, 2);
        assert!(link_of(&db.0, "xhs", "scn-xhs").is_some());
        let user = link_of(&db.0, "plain", "scn-wx").expect("用户的归类还在");
        assert_eq!(user.0, "user");
        let before = count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios");
        // 第二次：已经有规则归类，不重做、不写库。
        assert!(ensure_initial_classification(&db.0)
            .expect("第二次应成功")
            .is_none());
        assert_eq!(
            count_sql(&db.0, "SELECT COUNT(*) FROM entity_scenarios"),
            before
        );
    }

    #[test]
    fn initial_classification_without_account_is_an_error_not_a_panic() {
        let db = temp_db("initial-classify-noacct");
        assert!(ensure_initial_classification(&db.0).is_err());
    }

    #[test]
    fn classify_new_repositories_only_touches_the_given_ids() {
        let db = temp_db("classify-new");
        insert_account(&db.0);
        insert_repo(&db.0, "old", "fox/old", "小红书 封面", "[]", "active");
        insert_repo(&db.0, "new", "fox/new", "小红书 封面", "[]", "active");
        assert_eq!(
            classify_new_repositories(&db.0, &[])
                .expect("空名单")
                .repositories,
            0
        );
        let stats = classify_new_repositories(&db.0, &["new".to_owned()]).expect("新仓库归类");
        assert_eq!(stats.repositories, 1);
        assert!(link_of(&db.0, "new", "scn-xhs").is_some());
        assert!(link_of(&db.0, "old", "scn-xhs").is_none());
    }
}
