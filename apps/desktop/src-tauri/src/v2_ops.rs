use crate::capture::extract_links_from_text;
use crate::db::migrations::migrate_database;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayActions {
    pub active_repositories: i64,
    pub suggested_cards: i64,
    pub failed_jobs: i64,
    pub starred_this_week: i64,
    pub top_languages: Vec<LanguageCount>,
    pub schema_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageCount {
    pub language: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCard {
    pub id: String,
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub html_url: String,
    pub stars_count: i64,
    pub reading_label: String,
    pub one_liner: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePage {
    pub items: Vec<WorkspaceCard>,
    pub total_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDetail {
    pub id: String,
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub html_url: String,
    pub stars_count: i64,
    pub reading_label: String,
    pub summary_zh: Option<String>,
    pub keywords: Vec<String>,
    pub readme_excerpt: Option<String>,
    pub one_liner: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteHit {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

pub fn open_db(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    Ok(connection)
}

pub fn ensure_ready(path: &Path) -> Result<Connection, String> {
    migrate_database(path)?;
    open_db(path)
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
        .ok_or_else(|| "开发库里还没有 GitHub 账号，无法写入收集或分类。".to_owned())
}

pub fn today_actions(path: &Path) -> Result<TodayActions, String> {
    let connection = open_db(path)?;
    let (active_repositories, suggested_cards, failed_jobs, starred_this_week, schema_version) = connection
        .query_row(
            r#"
SELECT
  (SELECT COUNT(*) FROM repositories WHERE sync_status = 'active'),
  (SELECT COUNT(*) FROM repo_ai_documents WHERE IFNULL(one_liner_zh, '') <> '' AND IFNULL(card_json, '') NOT LIKE '%"review_status":"confirmed"%'),
  (SELECT COUNT(*) FROM ai_jobs WHERE status = 'failed'),
  (SELECT COUNT(*) FROM repositories WHERE sync_status = 'active' AND starred_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-7 days')),
  (SELECT COALESCE(MAX(version), '') FROM schema_migrations)
"#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .map_err(|error| format!("今日行动清单读取失败：{error}"))?;
    let mut statement = connection
        .prepare(
            r#"
SELECT COALESCE(language, '未标语言'), COUNT(*)
FROM repositories
WHERE sync_status = 'active'
GROUP BY language
ORDER BY COUNT(*) DESC
LIMIT 5
"#,
        )
        .map_err(|error| format!("语言画像读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(LanguageCount {
                language: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(|error| format!("语言画像查询失败：{error}"))?;
    let mut top_languages = Vec::new();
    for row in rows {
        top_languages.push(row.map_err(|error| format!("语言画像行读取失败：{error}"))?);
    }
    Ok(TodayActions {
        active_repositories,
        suggested_cards,
        failed_jobs,
        starred_this_week,
        top_languages,
        schema_version,
    })
}

fn reading_clause(display: &str) -> Result<&'static str, String> {
    match display {
        "" | "all" => Ok(""),
        "unseen" => Ok("AND IFNULL(annotations.read_status, 'unread') IN ('unread', 'later')"),
        "want_to_try" => Ok("AND annotations.read_status IN ('want_to_try', 'watching')"),
        "tried" => Ok("AND annotations.read_status IN ('tried', 'read')"),
        "in_use" => Ok("AND annotations.read_status = 'in_use'"),
        "dropped" => Ok("AND annotations.read_status = 'deprecated'"),
        _ => Err("阅读状态筛选无效".to_owned()),
    }
}

pub fn reading_label(status: Option<&str>) -> String {
    match status.unwrap_or("unread") {
        "want_to_try" | "watching" => "想试",
        "tried" | "read" => "试过",
        "in_use" => "在用",
        "deprecated" => "不要了",
        _ => "没看",
    }
    .to_owned()
}

pub fn list_workspace(
    path: &Path,
    keyword: &str,
    language: &str,
    reading: &str,
    limit: i64,
    offset: i64,
) -> Result<WorkspacePage, String> {
    let connection = open_db(path)?;
    let clause = reading_clause(reading)?;
    let pattern = like_pattern(keyword);
    let count_sql = format!(
        r#"
SELECT COUNT(*)
FROM repositories
LEFT JOIN annotations ON annotations.repo_id = repositories.id AND annotations.account_id = repositories.account_id
LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
WHERE repositories.sync_status = 'active'
  AND (?1 = '' OR repositories.language = ?1)
  AND (?2 = '' OR repositories.full_name LIKE ?2 ESCAPE '\' OR IFNULL(repositories.description,'') LIKE ?2 ESCAPE '\' OR IFNULL(repo_ai_documents.summary_zh,'') LIKE ?2 ESCAPE '\')
  {clause}
"#
    );
    let total_count = connection
        .query_row(&count_sql, params![language, pattern], |row| row.get(0))
        .map_err(|error| format!("资料库计数失败：{error}"))?;
    let list_sql = format!(
        r#"
SELECT repositories.id, repositories.full_name, repositories.description, repositories.language,
       repositories.html_url, repositories.stars_count, annotations.read_status, repo_ai_documents.one_liner_zh,
       repo_ai_documents.summary_zh
FROM repositories
LEFT JOIN annotations ON annotations.repo_id = repositories.id AND annotations.account_id = repositories.account_id
LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
WHERE repositories.sync_status = 'active'
  AND (?1 = '' OR repositories.language = ?1)
  AND (?2 = '' OR repositories.full_name LIKE ?2 ESCAPE '\' OR IFNULL(repositories.description,'') LIKE ?2 ESCAPE '\' OR IFNULL(repo_ai_documents.summary_zh,'') LIKE ?2 ESCAPE '\')
  {clause}
ORDER BY repositories.starred_at DESC
LIMIT ?3 OFFSET ?4
"#
    );
    let mut statement = connection
        .prepare(&list_sql)
        .map_err(|error| format!("资料库查询准备失败：{error}"))?;
    let rows = statement
        .query_map(params![language, pattern, limit, offset], |row| {
            let status: Option<String> = row.get(6)?;
            let one_liner: Option<String> = row.get(7)?;
            let summary: Option<String> = row.get(8)?;
            Ok(WorkspaceCard {
                id: row.get(0)?,
                full_name: row.get(1)?,
                description: row.get(2)?,
                language: row.get(3)?,
                html_url: row.get(4)?,
                stars_count: row.get(5)?,
                reading_label: reading_label(status.as_deref()),
                one_liner: one_liner.or(summary),
            })
        })
        .map_err(|error| format!("资料库查询失败：{error}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| format!("资料库行读取失败：{error}"))?);
    }
    Ok(WorkspacePage { items, total_count })
}

pub fn workspace_detail(path: &Path, id: &str) -> Result<WorkspaceDetail, String> {
    let connection = open_db(path)?;
    connection
        .query_row(
            r#"
SELECT repositories.id, repositories.full_name, repositories.description, repositories.language,
       repositories.html_url, repositories.stars_count, annotations.read_status,
       repo_ai_documents.summary_zh, repo_ai_documents.keywords_json, substr(repo_readmes.raw_markdown, 1, 800),
       repo_ai_documents.one_liner_zh
FROM repositories
LEFT JOIN annotations ON annotations.repo_id = repositories.id AND annotations.account_id = repositories.account_id
LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
LEFT JOIN repo_readmes ON repo_readmes.repo_id = repositories.id
WHERE repositories.id = ?1
"#,
            params![id],
            |row| {
                let status: Option<String> = row.get(6)?;
                let keywords_json: Option<String> = row.get(8)?;
                Ok(WorkspaceDetail {
                    id: row.get(0)?,
                    full_name: row.get(1)?,
                    description: row.get(2)?,
                    language: row.get(3)?,
                    html_url: row.get(4)?,
                    stars_count: row.get(5)?,
                    reading_label: reading_label(status.as_deref()),
                    summary_zh: row.get(7)?,
                    keywords: keywords_json
                        .and_then(|value| serde_json::from_str(&value).ok())
                        .unwrap_or_default(),
                    readme_excerpt: row.get(9)?,
                    one_liner: row.get(10)?,
                })
            },
        )
        .map_err(|error| format!("条目详情读取失败：{error}"))
}

pub fn palette_search(path: &Path, query: &str) -> Result<Vec<PaletteHit>, String> {
    let connection = open_db(path)?;
    let pattern = like_pattern(query);
    let mut hits = Vec::new();
    let mut repos = connection
        .prepare(
            r#"
SELECT repositories.id, repositories.full_name, IFNULL(repo_ai_documents.one_liner_zh, IFNULL(repositories.description, ''))
FROM repositories
LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
WHERE repositories.sync_status = 'active'
  AND (?1 = '' OR repositories.full_name LIKE ?1 ESCAPE '\' OR IFNULL(repo_ai_documents.one_liner_zh,'') LIKE ?1 ESCAPE '\')
ORDER BY repositories.stars_count DESC
LIMIT 12
"#,
        )
        .map_err(|error| format!("命令面板仓库查询失败：{error}"))?;
    let rows = repos
        .query_map(params![pattern], |row| {
            Ok(PaletteHit {
                kind: "repository".to_owned(),
                id: row.get(0)?,
                title: row.get(1)?,
                subtitle: row.get(2)?,
            })
        })
        .map_err(|error| format!("命令面板仓库读取失败：{error}"))?;
    for row in rows {
        hits.push(row.map_err(|error| format!("命令面板行失败：{error}"))?);
    }
    let mut categories = connection
        .prepare(
            r#"
SELECT id, name_zh, slug FROM categories
WHERE (?1 = '' OR name_zh LIKE ?1 ESCAPE '\' OR slug LIKE ?1 ESCAPE '\')
LIMIT 8
"#,
        )
        .map_err(|error| format!("命令面板分类查询失败：{error}"))?;
    let rows = categories
        .query_map(params![pattern], |row| {
            Ok(PaletteHit {
                kind: "category".to_owned(),
                id: row.get(0)?,
                title: row.get(1)?,
                subtitle: row.get(2)?,
            })
        })
        .map_err(|error| format!("命令面板分类读取失败：{error}"))?;
    for row in rows {
        hits.push(row.map_err(|error| format!("命令面板分类行失败：{error}"))?);
    }
    Ok(hits)
}

/// 只给测试用。运行中不能调：它会把正在跑的任务也放回队列，而且不记那次请求的用量。
/// 启动时的恢复在 `jobs::Worker::start` 里做（保留预留、隔一会儿再认领）。
#[cfg(test)]
pub fn recover_interrupted_jobs(path: &Path) -> Result<usize, String> {
    let connection = open_db(path)?;
    let changed = connection
        .execute(
            "UPDATE ai_jobs SET status = 'queued', error_kind = 'interrupted', started_at = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE status = 'running'",
            [],
        )
        .map_err(|error| format!("中断任务恢复失败：{error}"))?;
    Ok(changed)
}

pub fn queue_control_path(database_path: &Path) -> PathBuf {
    crate::jobs::queue_control_path(database_path)
}

pub fn set_queue_paused(database_path: &Path, paused: bool) -> Result<(), String> {
    crate::jobs::set_paused(&queue_control_path(database_path), paused)
}

// 旧的 set_daily_budget 会读它来保住暂停位。预算改写已挪到 jobs，暂停位由那边保留。
// 签名不能动：测试和以后的调用方还用这个名字。cargo check 不编译测试。
#[allow(dead_code)]
pub fn queue_is_paused(database_path: &Path) -> bool {
    crate::jobs::read_control(&queue_control_path(database_path)).paused
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRow {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
}

pub fn list_jobs(path: &Path, limit: i64) -> Result<Vec<JobRow>, String> {
    let connection = open_db(path)?;
    let mut statement = connection
        .prepare(
            "SELECT id, job_type, status, error_kind, error_message FROM ai_jobs WHERE target_id NOT LIKE 'legacy-%' ORDER BY updated_at DESC LIMIT ?1",
        )
        .map_err(|error| format!("任务列表准备失败：{error}"))?;
    let rows = statement
        .query_map(params![limit], |row| {
            Ok(JobRow {
                id: row.get(0)?,
                job_type: row.get(1)?,
                status: row.get(2)?,
                error_kind: row.get(3)?,
                error_message: row.get(4)?,
            })
        })
        .map_err(|error| format!("任务列表查询失败：{error}"))?;
    let mut jobs = Vec::new();
    for row in rows {
        jobs.push(row.map_err(|error| format!("任务行读取失败：{error}"))?);
    }
    Ok(jobs)
}

pub fn cancel_job(path: &Path, id: &str) -> Result<(), String> {
    let connection = open_db(path)?;
    let changed = connection
        .execute(
            // finished_at 沿用这条任务原来的记账时间（失败的是结束时间，排队 / 在跑的是 updated_at），不改成现在：
            // 取消几天前失败的任务，不能把它的旧用量「重新记到今天」。
            "UPDATE ai_jobs SET status = 'cancelled', finished_at = COALESCE(finished_at, updated_at), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1 AND status IN ('queued','running','failed')",
            params![id],
        )
        .map_err(|error| format!("取消任务失败：{error}"))?;
    if changed == 0 {
        return Err("没有可取消的任务".to_owned());
    }
    Ok(())
}

pub fn set_daily_budget(database_path: &Path, tokens: u64) -> Result<u64, String> {
    crate::jobs::set_daily_budget(&queue_control_path(database_path), tokens)
}

pub fn daily_budget(database_path: &Path) -> u64 {
    crate::jobs::read_control(&queue_control_path(database_path)).daily_budget_tokens
}

/// 把失败的任务重新排队。确定性的失败不跟着重试：内容太大、模型回了空内容，再试一百次也一样，只会白花 token。
/// 一条一条改，`OR IGNORE`：这个仓库已经有一条新的排队 / 在跑的任务时（唯一索引），旧的失败那条留着不动，
/// 不让整批重试因为一条冲突全部回滚。
pub fn retry_failed_jobs(path: &Path) -> Result<usize, String> {
    let connection = open_db(path)?;
    let ids: Vec<String> = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM ai_jobs
                 WHERE status = 'failed'
                   AND IFNULL(error_kind, '') <> 'content_too_large'
                   AND IFNULL(error_message, '') NOT LIKE '%没有可用内容%'",
            )
            .map_err(|error| format!("失败任务读取失败：{error}"))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("失败任务读取失败：{error}"))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row.map_err(|error| format!("失败任务行读取失败：{error}"))?);
        }
        ids
    };
    let cutoff = crate::jobs::day_ago_db_time();
    let mut changed = 0;
    for id in ids {
        changed += connection
            .execute(
                // 过了 24 小时的旧失败用量一起清零：重新排队会把 updated_at 改成现在，
                // 不清的话几天前的账会被「重新记到今天」（20 条各 3 万的旧失败，重试一下就占掉 60 万）。
                "UPDATE OR IGNORE ai_jobs SET status = 'queued', attempts = 0, error_message = NULL, finished_at = NULL, not_before = NULL,
                    input_tokens = CASE WHEN updated_at >= ?2 THEN input_tokens ELSE 0 END,
                    output_tokens = CASE WHEN updated_at >= ?2 THEN output_tokens ELSE 0 END,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1 AND status = 'failed'",
                params![id, cutoff],
            )
            .map_err(|error| format!("失败任务重试失败：{error}"))?;
    }
    Ok(changed)
}

#[derive(Debug, Deserialize)]
struct SeedFile {
    categories: Vec<SeedCategory>,
}

#[derive(Debug, Deserialize)]
struct SeedCategory {
    slug: String,
    name_zh: String,
    rx: String,
}

const TAXONOMY_SEED: &str = include_str!("../../../../docs/rebuild/spec/taxonomy_seed.json");

pub fn seed_taxonomy(path: &Path) -> Result<usize, String> {
    let connection = open_db(path)?;
    let account = account_id(&connection)?;
    let seed: SeedFile = serde_json::from_str(TAXONOMY_SEED)
        .map_err(|error| format!("分类种子解析失败：{error}"))?;
    let mut inserted = 0;
    for (index, category) in seed.categories.iter().enumerate() {
        let id = format!("seed-{}", category.slug);
        let changed = connection
            .execute(
                "INSERT OR IGNORE INTO categories(id, account_id, slug, name_zh, sort_order, origin) VALUES (?1, ?2, ?3, ?4, ?5, 'seed')",
                params![id, account, category.slug, category.name_zh, index as i64],
            )
            .map_err(|error| format!("分类种子写入失败：{error}"))?;
        inserted += changed;
    }
    Ok(inserted)
}

pub fn rule_classify(path: &Path) -> Result<usize, String> {
    seed_taxonomy(path)?;
    let connection = open_db(path)?;
    let account = account_id(&connection)?;
    let seed: SeedFile = serde_json::from_str(TAXONOMY_SEED)
        .map_err(|error| format!("分类种子解析失败：{error}"))?;
    let mut repos = connection
        .prepare("SELECT id, full_name, IFNULL(description,''), IFNULL(topics_json,'[]') FROM repositories WHERE sync_status = 'active'")
        .map_err(|error| format!("分类候选读取失败：{error}"))?;
    let rows = repos
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("分类候选查询失败：{error}"))?;
    let collected = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("分类候选解析失败：{error}"))?;
    let mut linked = 0;
    for (id, full_name, description, topics) in collected {
        let haystack = format!("{full_name} {description} {topics}").to_lowercase();
        let Some(category) = seed
            .categories
            .iter()
            .find(|category| rule_matches(&category.rx, &haystack))
        else {
            continue;
        };
        let category_id = format!("seed-{}", category.slug);
        let changed = connection.execute(
            "INSERT OR IGNORE INTO entity_categories(account_id, entity_type, entity_id, category_id, is_primary, confidence, origin, status) VALUES (?1, 'repository', ?2, ?3, 1, 0.5, 'rule', 'suggested')",
            params![account, id, category_id],
        ).map_err(|error| format!("规则分类写入失败：{error}"))?;
        linked += changed;
    }
    Ok(linked)
}

fn rule_matches(pattern: &str, haystack: &str) -> bool {
    pattern
        .split('|')
        .map(|part| part.replace("\\b", "").replace('\\', "").to_lowercase())
        .filter(|part| part.len() >= 3)
        .any(|part| haystack.contains(&part))
}

pub fn import_summary_cards(path: &Path) -> Result<usize, String> {
    let connection = open_db(path)?;
    let mut statement = connection
        .prepare("SELECT repo_id, summary_zh FROM repo_ai_documents WHERE IFNULL(card_json, '') = '' AND IFNULL(summary_zh, '') <> ''")
        .map_err(|error| format!("旧摘要读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("旧摘要查询失败：{error}"))?;
    let collected = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("旧摘要解析失败：{error}"))?;
    let mut updated = 0;
    for (repo_id, summary) in collected {
        let one_liner = summary.chars().take(60).collect::<String>();
        let card = json!({
            "one_liner_zh": one_liner,
            "what_it_is": summary.chars().take(400).collect::<String>(),
            "form": "other",
            "primary_category": "dev-infra",
            "capabilities": [{"name_zh": "查看项目说明"}],
            "problems_solved": ["想知道这个收藏是做什么的"],
            "how_to_try": "打开仓库 README",
            "maturity": "usable",
            "zh_friendly": true,
            "confidence": 0.4,
            "review_status": "suggested"
        });
        validate_card(&card)?;
        updated += connection.execute(
            "UPDATE repo_ai_documents SET one_liner_zh = ?1, card_json = ?2, card_schema_version = 1 WHERE repo_id = ?3 AND IFNULL(card_json, '') NOT LIKE '%\"review_status\":\"confirmed\"%'",
            params![one_liner, card.to_string(), repo_id],
        ).map_err(|error| format!("理解卡写入失败：{error}"))?;
    }
    Ok(updated)
}

pub fn validate_card(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "理解卡必须是对象".to_owned())?;
    for key in [
        "one_liner_zh",
        "what_it_is",
        "form",
        "primary_category",
        "capabilities",
        "problems_solved",
        "how_to_try",
        "maturity",
        "zh_friendly",
        "confidence",
    ] {
        if !object.contains_key(key) {
            return Err(format!("理解卡缺少 {key}"));
        }
    }
    let one_liner = object["one_liner_zh"].as_str().unwrap_or("");
    if one_liner.chars().count() > 60 {
        return Err("一句话说明超过 60 字".to_owned());
    }
    Ok(())
}

pub fn repair_model_json(raw: &str) -> Result<Value, String> {
    let trimmed = raw.trim();
    let fenced = trimmed
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let start = fenced
        .find('{')
        .ok_or_else(|| "模型输出里没有 JSON 对象".to_owned())?;
    let end = fenced
        .rfind('}')
        .ok_or_else(|| "模型输出里没有 JSON 结束".to_owned())?;
    let mut value: Value = serde_json::from_str(&fenced[start..=end])
        .map_err(|error| format!("JSON 解析失败：{error}"))?;
    if let Some(object) = value.as_object_mut() {
        if let Some(text) = object.get("one_liner_zh").and_then(Value::as_str) {
            let shortened = text.chars().take(60).collect::<String>();
            object.insert("one_liner_zh".to_owned(), Value::String(shortened));
        }
    }
    validate_card(&value)?;
    Ok(value)
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCommit {
    /// 新写进收集箱的条目数。
    pub created: usize,
    /// 没有新建的总数：已收藏 + 已在收集箱 + 被拦截；整批重复时等于链接总数。
    pub skipped: usize,
    pub duplicate_batch: bool,
    /// 已经在你的 Stars 里，不再重复收。
    pub already_starred: usize,
    pub already_in_inbox: usize,
    /// 本机或内网地址，单独拦下，不影响其它链接。
    pub blocked: usize,
    pub links_found: usize,
}

fn short_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))[..24].to_owned()
}

/// 这一批从哪来。`platform` 记在批次上，`via` 记在每个新条目上。
/// `original_text` 有值时写成条目的原话；没有就不写这一列。
pub struct CaptureSource<'a> {
    pub platform: &'a str,
    pub via: &'a str,
    pub original_text: Option<&'a str>,
}

const CAPTURE_EXCERPT_CHARS: usize = 2000;

fn normalize_capture_platform(platform: &str) -> &'static str {
    match platform {
        "web" => "web",
        "github" => "github",
        "x" => "x",
        "wechat" => "wechat",
        "telegram" => "telegram",
        "discord" => "discord",
        "forum" => "forum",
        "xiaohongshu" => "xiaohongshu",
        "bilibili" => "bilibili",
        "youtube" => "youtube",
        "local" => "local",
        "other" => "other",
        _ => "other",
    }
}

fn normalize_capture_via(via: &str) -> &'static str {
    match via {
        "local_api" => "local_api",
        _ => "paste_batch",
    }
}

pub fn commit_capture(
    path: &Path,
    text: &str,
    source_context: &str,
) -> Result<CaptureCommit, String> {
    commit_capture_with(
        path,
        text,
        source_context,
        &CaptureSource {
            platform: "other",
            via: "paste_batch",
            original_text: None,
        },
    )
}

pub fn commit_capture_with(
    path: &Path,
    text: &str,
    source_context: &str,
    source: &CaptureSource<'_>,
) -> Result<CaptureCommit, String> {
    let links = extract_links_from_text(text);
    let mut result = CaptureCommit {
        links_found: links.len(),
        ..CaptureCommit::default()
    };
    if links.is_empty() {
        // 没有链接就不留批次记录，用户补上链接后还能再贴一次。
        return Ok(result);
    }
    let batch_platform = normalize_capture_platform(source.platform);
    let captured_via = normalize_capture_via(source.via);
    let excerpt = source.original_text.map(|original| {
        original
            .trim()
            .chars()
            .take(CAPTURE_EXCERPT_CHARS)
            .collect::<String>()
    });
    let mut connection = open_db(path)?;
    let account = account_id(&connection)?;
    let hash = format!("{:x}", Sha256::digest(text.trim().as_bytes()));
    let batch_id = format!("batch-{hash}");
    let context: String = source_context.chars().take(2000).collect();
    // 整批放进一个事务：中途出错整批都不留痕，不会出现「写了一半、却被记成已收过」。
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("收集事务开启失败：{error}"))?;
    let inserted_batch = tx
        .execute(
            "INSERT OR IGNORE INTO capture_batches(id, account_id, raw_text_hash, source_platform, source_context, url_count, github_repo_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                batch_id,
                account,
                hash,
                batch_platform,
                context,
                links.len() as i64,
                links.iter().filter(|link| link.kind == "github_repo").count() as i64
            ],
        )
        .map_err(|error| format!("收集批次写入失败：{error}"))?;
    if inserted_batch == 0 {
        result.duplicate_batch = true;
        result.skipped = links.len();
        return Ok(result);
    }
    for link in &links {
        if link.blocked_reason.is_some() {
            result.blocked += 1;
            continue;
        }
        if link.kind == "github_repo" {
            let full_name = format!(
                "{}/{}",
                link.owner.as_deref().unwrap_or(""),
                link.name.as_deref().unwrap_or("")
            );
            let starred: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM repositories WHERE account_id = ?1 AND sync_status = 'active' AND lower(full_name) = lower(?2))",
                    params![account, full_name],
                    |row| row.get(0),
                )
                .map_err(|error| format!("已收藏检查失败：{error}"))?;
            if starred {
                result.already_starred += 1;
                continue;
            }
        }
        let in_inbox: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM inbox_items WHERE account_id = ?1 AND normalized_url = ?2)",
                params![account, link.normalized_url],
                |row| row.get(0),
            )
            .map_err(|error| format!("收集箱查重失败：{error}"))?;
        if in_inbox {
            result.already_in_inbox += 1;
            continue;
        }
        let item_type = match (link.kind.as_str(), link.platform.as_str()) {
            ("github_repo", _) => "github_repository",
            (_, "youtube" | "bilibili") => "video",
            _ => "webpage",
        };
        let title = match (&link.owner, &link.name) {
            (Some(owner), Some(name)) => format!("{owner}/{name}"),
            _ => link.normalized_url.clone(),
        };
        let id = format!(
            "cap-{}",
            short_hash(&format!("{batch_id}|{}", link.normalized_url))
        );
        let write_item = if let Some(excerpt) = &excerpt {
            tx.execute(
                "INSERT INTO inbox_items(id, account_id, original_url, normalized_url, title, item_type, source_platform, source_context, captured_via, capture_batch_id, raw_excerpt) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![id, account, link.url, link.normalized_url, title, item_type, link.platform, context, captured_via, batch_id, excerpt],
            )
        } else {
            tx.execute(
                "INSERT INTO inbox_items(id, account_id, original_url, normalized_url, title, item_type, source_platform, source_context, captured_via, capture_batch_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![id, account, link.url, link.normalized_url, title, item_type, link.platform, context, captured_via, batch_id],
            )
        };
        write_item.map_err(|error| format!("收集条目写入失败：{error}"))?;
        result.created += 1;
    }
    tx.commit()
        .map_err(|error| format!("收集提交失败：{error}"))?;
    result.skipped = result.already_starred + result.already_in_inbox + result.blocked;
    Ok(result)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryBlock {
    pub slug: String,
    pub name_zh: String,
    pub count: i64,
}

pub fn graph_overview(path: &Path) -> Result<Vec<CategoryBlock>, String> {
    let connection = open_db(path)?;
    let mut statement = connection.prepare(
        r#"
SELECT categories.slug, categories.name_zh, COUNT(entity_categories.entity_id)
FROM categories
LEFT JOIN entity_categories ON entity_categories.category_id = categories.id AND entity_categories.status <> 'rejected' AND entity_categories.is_primary = 1
GROUP BY categories.id
ORDER BY COUNT(entity_categories.entity_id) DESC, categories.sort_order
"#
    ).map_err(|error| format!("地图读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(CategoryBlock {
                slug: row.get(0)?,
                name_zh: row.get(1)?,
                count: row.get(2)?,
            })
        })
        .map_err(|error| format!("地图查询失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("地图行读取失败：{error}"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveHit {
    pub id: String,
    pub full_name: String,
    pub why: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveAnswer {
    pub owned: Vec<SolveHit>,
    pub gaps: Vec<String>,
    pub external: Vec<SolveHit>,
}

pub fn solve_problem(path: &Path, question: &str) -> Result<SolveAnswer, String> {
    let connection = open_db(path)?;
    let mut tokens = question
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| token.chars().count() >= 2)
        .map(|token| token.to_lowercase())
        .collect::<Vec<_>>();
    if tokens.iter().any(|token| {
        token
            .chars()
            .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
    }) {
        let chars = question
            .chars()
            .filter(|ch| ('\u{4e00}'..='\u{9fff}').contains(ch))
            .collect::<Vec<_>>();
        for window in chars.windows(2) {
            tokens.push(window.iter().collect());
        }
    }
    let mut statement = connection.prepare(
        "SELECT repositories.id, repositories.full_name, IFNULL(repo_ai_documents.summary_zh, IFNULL(repositories.description, '')) FROM repositories LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id WHERE repositories.sync_status = 'active'"
    ).map_err(|error| format!("找方案读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("找方案查询失败：{error}"))?;
    let mut scored = Vec::new();
    for row in rows {
        let (id, full_name, evidence) = row.map_err(|error| format!("找方案行失败：{error}"))?;
        let lower = evidence.to_lowercase();
        let hits = tokens
            .iter()
            .filter(|token| lower.contains(token.as_str()))
            .count();
        if hits == 0 {
            continue;
        }
        let why = evidence.chars().take(160).collect::<String>();
        scored.push((hits, SolveHit { id, full_name, why }));
    }
    scored.sort_by(|left, right| right.0.cmp(&left.0));
    let owned = scored
        .into_iter()
        .take(5)
        .map(|(_, hit)| hit)
        .collect::<Vec<_>>();
    let gaps = if owned.len() < 3 {
        vec!["本机收藏里直接证据不足，不要把未出现的用法当成已有能力。".to_owned()]
    } else {
        Vec::new()
    };
    Ok(SolveAnswer {
        owned,
        gaps,
        external: Vec::new(),
    })
}

pub fn render_pack(title: &str, problem: &str, hits: &[SolveHit]) -> String {
    let mut lines = vec![
        format!("# {title}"),
        String::new(),
        problem.to_owned(),
        String::new(),
        "## 你已经有的".to_owned(),
    ];
    for hit in hits {
        lines.push(format!("- {}：{}", hit.full_name, hit.why));
    }
    lines.push(String::new());
    lines.push("## 给 Codex 的指令".to_owned());
    lines.push("只使用下面列出的仓库和原文摘要，不要补充未给出的安装步骤。".to_owned());
    lines.join("\n")
}

pub fn save_pack(path: &Path, title: &str, problem: &str, content: &str) -> Result<String, String> {
    let connection = open_db(path)?;
    let account = account_id(&connection)?;
    let id = format!("pack-{:x}", Sha256::digest(content.as_bytes()));
    connection.execute(
        "INSERT OR REPLACE INTO reference_packs(id, account_id, title_zh, problem_text, content, format) VALUES (?1, ?2, ?3, ?4, ?5, 'markdown')",
        params![id, account, title, problem, content],
    ).map_err(|error| format!("参考包保存失败：{error}"))?;
    Ok(id)
}

pub fn backup_database_file(database_path: &Path, backup_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(backup_dir).map_err(|error| format!("备份目录创建失败：{error}"))?;
    let destination = backup_dir.join(format!("manual-{}.sqlite3", chrono_stamp()));
    let connection = open_db(database_path)?;
    let escaped = destination.to_string_lossy().replace('\'', "''");
    connection
        .execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .map_err(|error| format!("备份失败：{error}"))?;
    let check = Connection::open(&destination).map_err(|error| format!("备份打开失败：{error}"))?;
    let result: String = check
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("备份校验失败：{error}"))?;
    if result != "ok" {
        return Err(format!("备份完整性不是 ok：{result}"));
    }
    Ok(destination)
}

pub fn export_markdown(path: &Path, destination: &Path) -> Result<usize, String> {
    let page = list_workspace(path, "", "", "", 50, 0)?;
    let mut lines = vec!["# Fox Stars Lab 资料库".to_owned(), String::new()];
    for item in &page.items {
        lines.push(format!(
            "- {}：{}",
            item.full_name,
            item.one_liner.clone().unwrap_or_default()
        ));
    }
    fs::write(destination, lines.join("\n")).map_err(|error| format!("导出失败：{error}"))?;
    Ok(page.items.len())
}

pub fn log_usage(
    path: &Path,
    entity_id: &str,
    verdict: &str,
    note: &str,
    via: &str,
) -> Result<(), String> {
    if !matches!(verdict, "useful" | "meh" | "useless") {
        return Err("使用反馈只能是 useful、meh 或 useless".to_owned());
    }
    if !matches!(via, "ui" | "mcp" | "local_api") {
        return Err("使用记录来源只能是界面、MCP 或本地 API".to_owned());
    }
    if note.chars().count() > 4000 {
        return Err("使用备注不能超过 4000 字".to_owned());
    }
    let connection = open_db(path)?;
    let account = account_id(&connection)?;
    let id = format!(
        "use-{:x}",
        Sha256::digest(format!("{entity_id}{verdict}{note}{via}").as_bytes())
    );
    connection.execute(
        "INSERT OR IGNORE INTO usage_log(id, account_id, entity_type, entity_id, verdict, note, via) VALUES (?1, ?2, 'repository', ?3, ?4, ?5, ?6)",
        params![id, account, entity_id, verdict, note, via],
    ).map_err(|error| format!("使用记录写入失败：{error}"))?;
    Ok(())
}

pub fn export_csv(path: &Path, destination: &Path) -> Result<usize, String> {
    let page = list_workspace(path, "", "", "", 200, 0)?;
    let mut lines = vec!["full_name,language,reading,one_liner".to_owned()];
    for item in &page.items {
        lines.push(format!(
            "{},{},{},{}",
            csv_cell(&item.full_name),
            csv_cell(item.language.as_deref().unwrap_or("")),
            csv_cell(&item.reading_label),
            csv_cell(item.one_liner.as_deref().unwrap_or(""))
        ));
    }
    fs::write(destination, lines.join("\n")).map_err(|error| format!("CSV 导出失败：{error}"))?;
    Ok(page.items.len())
}

fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub fn list_ranking_board(path: &Path) -> Result<Vec<RankingRow>, String> {
    let connection = open_db(path)?;
    let mut statement = connection.prepare(
        r#"
SELECT ranking_sources.id, ranking_sources.name_zh, ranking_sources.enabled, IFNULL(ranking_sources.last_error, ''),
       IFNULL(ranking_entries.full_name, ''), IFNULL(ranking_entries.description, ''),
       EXISTS(SELECT 1 FROM repositories WHERE repositories.full_name = ranking_entries.full_name AND repositories.sync_status = 'active')
FROM ranking_sources
LEFT JOIN ranking_entries ON ranking_entries.snapshot_id = (
  SELECT id FROM ranking_snapshots WHERE source_id = ranking_sources.id ORDER BY fetched_at DESC LIMIT 1
)
ORDER BY ranking_sources.id, ranking_entries.rank
"#
    ).map_err(|error| format!("榜单读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(RankingRow {
                source_id: row.get(0)?,
                source_name: row.get(1)?,
                enabled: row.get::<_, i64>(2)? == 1,
                last_error: row.get(3)?,
                full_name: row.get(4)?,
                description: row.get(5)?,
                already_starred: row.get::<_, i64>(6)? == 1,
                relevance: if row.get::<_, i64>(6)? == 1 { 1.0 } else { 0.0 },
            })
        })
        .map_err(|error| format!("榜单查询失败：{error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("榜单行读取失败：{error}"))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingRow {
    pub source_id: String,
    pub source_name: String,
    pub enabled: bool,
    pub last_error: String,
    pub full_name: String,
    pub description: String,
    pub already_starred: bool,
    pub relevance: f64,
}

pub fn source_enabled(path: &Path, source_id: &str) -> Result<bool, String> {
    let connection = open_db(path)?;
    let enabled: i64 = connection
        .query_row(
            "SELECT enabled FROM ranking_sources WHERE id = ?1",
            params![source_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("榜单来源读取失败：{error}"))?;
    Ok(enabled == 1)
}

pub fn mark_ranking_error(path: &Path, source_id: &str, error: &str) -> Result<(), String> {
    let connection = open_db(path)?;
    connection
        .execute(
            "UPDATE ranking_sources SET last_error = ?1, last_fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            params![error.chars().take(500).collect::<String>(), source_id],
        )
        .map_err(|error| format!("榜单错误写入失败：{error}"))?;
    Ok(())
}

pub fn replace_ranking_snapshot(
    path: &Path,
    source_id: &str,
    items: &[crate::rankings_live::RankItem],
) -> Result<usize, String> {
    if source_id == "github_trending_html" {
        return Err("GitHub Trending 网页抓取保持关闭，不写入快照".to_owned());
    }
    let connection = open_db(path)?;
    let stamp = chrono_stamp();
    let snapshot_id = format!("{source_id}-{stamp}");
    connection.execute(
        "INSERT INTO ranking_snapshots(id, source_id, list_key, item_count) VALUES (?1, ?2, 'default', ?3)",
        params![snapshot_id, source_id, items.len() as i64],
    ).map_err(|error| format!("榜单快照写入失败：{error}"))?;
    for (index, item) in items.iter().enumerate() {
        connection.execute(
            "INSERT INTO ranking_entries(snapshot_id, rank, full_name, description, html_url, stars_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![snapshot_id, index as i64 + 1, item.full_name, item.description, item.html_url, item.stars],
        ).map_err(|error| format!("榜单条目写入失败：{error}"))?;
    }
    connection.execute(
        "UPDATE ranking_sources SET last_error = '', last_fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
        params![source_id],
    ).map_err(|error| format!("榜单来源更新失败：{error}"))?;
    Ok(items.len())
}

pub fn set_reading_statuses(path: &Path, ids: &[String], status: &str) -> Result<usize, String> {
    let stored = match status {
        "unseen" => "unread",
        "want_to_try" | "tried" | "in_use" | "dropped" => status,
        "deprecated" => "deprecated",
        _ => return Err("阅读状态只能是没看、想试、试过、在用或不要了".to_owned()),
    };
    let stored = if stored == "dropped" {
        "deprecated"
    } else {
        stored
    };
    let connection = open_db(path)?;
    let account = account_id(&connection)?;
    let mut changed = 0;
    for id in ids {
        changed += connection.execute(
            "INSERT INTO annotations(repo_id, account_id, read_status, updated_at) VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now')) ON CONFLICT(repo_id) DO UPDATE SET read_status = excluded.read_status, updated_at = excluded.updated_at",
            params![id, account, stored],
        ).map_err(|error| format!("阅读状态写入失败：{error}"))?;
    }
    Ok(changed)
}

pub fn merge_named_concepts(
    path: &Path,
    source_name: &str,
    target_name: &str,
) -> Result<(), String> {
    let connection = open_db(path)?;
    let account = account_id(&connection)?;
    for name in [source_name, target_name] {
        let id = format!("concept-{name}");
        connection.execute(
            "INSERT OR IGNORE INTO kg_concepts(id, account_id, kind, name_zh, normalized_name, origin) VALUES (?1, ?2, 'capability', ?3, ?3, 'user')",
            params![id, account, name],
        ).map_err(|error| format!("概念准备失败：{error}"))?;
    }
    drop(connection);
    merge_concepts(
        path,
        &format!("concept-{source_name}"),
        &format!("concept-{target_name}"),
    )
}

pub fn merge_concepts(path: &Path, source_id: &str, target_id: &str) -> Result<(), String> {
    if source_id == target_id {
        return Err("不能把概念合并到自己".to_owned());
    }
    let connection = open_db(path)?;
    let changed = connection.execute(
        "UPDATE kg_concepts SET status = 'merged', merged_into_id = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2 AND status = 'active'",
        params![target_id, source_id],
    ).map_err(|error| format!("概念合并失败：{error}"))?;
    if changed == 0 {
        return Err("没有可合并的概念".to_owned());
    }
    connection
        .execute(
            "UPDATE kg_edges SET dst_id = ?1 WHERE dst_type = 'concept' AND dst_id = ?2",
            params![target_id, source_id],
        )
        .ok();
    Ok(())
}

pub fn semantic_search(path: &Path, query: &str) -> Result<Vec<SolveHit>, String> {
    let answer = solve_problem(path, query)?;
    Ok(answer.owned)
}

pub fn enqueue_missing_cards(path: &Path) -> Result<usize, String> {
    let connection = open_db(path)?;
    let account = account_id(&connection)?;
    let changed = connection
        .execute(
            r#"
INSERT OR IGNORE INTO ai_jobs(id, account_id, job_type, target_type, target_id, status)
SELECT 'card-' || repositories.id, ?1, 'repo_card', 'repository', repositories.id, 'queued'
FROM repositories
LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
WHERE repositories.sync_status = 'active'
  AND (repo_ai_documents.repo_id IS NULL OR IFNULL(repo_ai_documents.card_json, '') = '')
  AND IFNULL(repo_ai_documents.card_json, '') NOT LIKE '%"review_status":"confirmed"%'
"#,
            params![account],
        )
        .map_err(|error| format!("缺摘要任务入队失败：{error}"))?;
    Ok(changed)
}

pub fn restore_database_file(database_path: &Path, backup_path: &Path) -> Result<(), String> {
    if !backup_path.is_file() {
        return Err("备份文件不存在".to_owned());
    }
    let backup = Connection::open(backup_path).map_err(|error| format!("备份打开失败：{error}"))?;
    let integrity: String = backup
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("备份校验失败：{error}"))?;
    if integrity != "ok" {
        return Err(format!("备份已损坏，拒绝恢复：{integrity}"));
    }
    fs::copy(backup_path, database_path).map_err(|error| format!("恢复失败：{error}"))?;
    Ok(())
}

pub fn health_summary(path: &Path) -> Result<String, String> {
    let connection = open_db(path)?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("健康检查失败：{error}"))?;
    let version: String = connection
        .query_row(
            "SELECT COALESCE(MAX(version), '') FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("版本读取失败：{error}"))?;
    let budget = daily_budget(path);
    Ok(format!("完整性 {integrity}，迁移版本 {version}，每日预算 {budget} token，本地 API 默认关闭，只允许 127.0.0.1"))
}

fn like_pattern(keyword: &str) -> String {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    format!(
        "%{}%",
        trimmed
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

fn chrono_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().to_string())
        .unwrap_or_else(|_| "backup".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::url_is_safe;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        // 只用时间会让两个测试在同一纳秒拿到同一个文件名，一起迁移同一个库（复核审查员 2/4 次复现）
        std::env::temp_dir().join(format!(
            "fsl-v2-{}-{}-{nanos}.sqlite3",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }

    fn with_account(path: &Path) {
        ensure_ready(path).unwrap();
        let connection = open_db(path).unwrap();
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .unwrap();
        connection.execute("INSERT INTO repositories(id, account_id, owner, name, full_name, description, language, topics_json, html_url, sync_status, starred_at) VALUES ('repo1', 'acct', 'fox', 'tool', 'fox/tool', '批量整理群聊链接', 'TypeScript', '[]', 'https://github.com/fox/tool', 'active', '2026-10-01T00:00:00Z')", []).unwrap();
        connection.execute("INSERT INTO repo_ai_documents(repo_id, summary_zh, readme_zh, keywords_json, suggested_tags_json, model, prompt_version, source_hash, generated_at) VALUES ('repo1', '这个工具可以批量整理群聊里的链接', '', '[]', '[]', 'test', 'v1', 'hash', '2026-10-01T00:00:00Z')", []).unwrap();
    }

    #[test]
    fn interrupted_job_returns_to_queue() {
        let path = temp_db();
        with_account(&path);
        let connection = open_db(&path).unwrap();
        connection.execute("INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status) VALUES ('job1', 'acct', 'repo_card', 'repository', 'repo1', 'running')", []).unwrap();
        drop(connection);
        assert_eq!(recover_interrupted_jobs(&path).unwrap(), 1);
        let jobs = list_jobs(&path, 10).unwrap();
        assert_eq!(jobs[0].status, "queued");
        assert_eq!(jobs[0].error_kind.as_deref(), Some("interrupted"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn confirmed_card_is_not_required_for_summary_import() {
        let path = temp_db();
        with_account(&path);
        let count = import_summary_cards(&path).unwrap();
        assert_eq!(count, 1);
        let connection = open_db(&path).unwrap();
        connection.execute("UPDATE repo_ai_documents SET card_json = '{\"review_status\":\"confirmed\",\"one_liner_zh\":\"已确认\"}' WHERE repo_id = 'repo1'", []).unwrap();
        drop(connection);
        assert_eq!(import_summary_cards(&path).unwrap(), 0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn repair_strips_fence_and_rejects_private_urls() {
        let card = repair_model_json("```json\n{\"one_liner_zh\":\"整理群聊链接\",\"what_it_is\":\"工具\",\"form\":\"cli\",\"primary_category\":\"dev-infra\",\"capabilities\":[{\"name_zh\":\"抽出链接\"}],\"problems_solved\":[\"群聊太乱\"],\"how_to_try\":\"粘贴\",\"maturity\":\"usable\",\"zh_friendly\":true,\"confidence\":0.8}\n```").unwrap();
        assert_eq!(card["one_liner_zh"], "整理群聊链接");
        assert!(url_is_safe("http://127.0.0.1/secret").is_err());
        assert!(url_is_safe("http://169.254.169.254/latest").is_err());
        assert!(url_is_safe("https://github.com/fox/tool").is_ok());
    }

    #[test]
    fn paste_twice_does_not_duplicate_and_solve_quotes_summary() {
        let path = temp_db();
        with_account(&path);
        let text = "看看 https://github.com/fox/tool 和 https://example.com/docs";
        let first = commit_capture(&path, text, "测试群").unwrap();
        assert!(first.created >= 1);
        let second = commit_capture(&path, text, "测试群").unwrap();
        assert!(second.duplicate_batch);
        let answer = solve_problem(&path, "群聊链接").unwrap();
        assert!(!answer.owned.is_empty());
        assert!(answer.owned[0].why.contains("群聊"));
        let invented = solve_problem(&path, "量子刺绣").unwrap();
        assert!(invented.owned.is_empty());
        assert!(!invented.gaps.is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn capture_skips_starred_and_blocked_links_without_aborting_the_batch() {
        let path = temp_db();
        with_account(&path);
        // fox/tool 已在 Stars 里（写法大小写不同）；内网链接单独拦下；其余两条应照常写入
        let text = "已有 https://github.com/Fox/Tool，内网 http://192.168.1.10/admin 新的 https://github.com/acme/widget 和 https://x.com/u/status/1";
        let first = commit_capture(&path, text, "测试群").unwrap();
        assert_eq!(first.links_found, 4);
        assert_eq!(first.already_starred, 1);
        assert_eq!(first.blocked, 1);
        assert_eq!(first.created, 2);
        assert_eq!(first.skipped, 2);
        assert!(!first.duplicate_batch);
        let connection = open_db(&path).unwrap();
        let stored: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE capture_batch_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, 2);
        let platform: String = connection
            .query_row("SELECT source_platform FROM inbox_items WHERE normalized_url = 'https://x.com/u/status/1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(platform, "x");
        drop(connection);

        // 同一段再贴：整批重复
        assert!(
            commit_capture(&path, text, "测试群")
                .unwrap()
                .duplicate_batch
        );
        // 换一段文字，但里面是同一个仓库的另一种写法（大小写 + 跟踪参数）：不会再新建
        let again = commit_capture(
            &path,
            "再发一次 https://github.com/ACME/Widget?utm_source=wechat",
            "测试群",
        )
        .unwrap();
        assert_eq!(again.created, 0);
        assert_eq!(again.already_in_inbox, 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn text_without_links_leaves_no_batch_behind() {
        let path = temp_db();
        with_account(&path);
        let empty = commit_capture(&path, "只有文字，没有链接。", "测试群").unwrap();
        assert_eq!(empty.links_found, 0);
        assert_eq!(empty.created, 0);
        let connection = open_db(&path).unwrap();
        let batches: i64 = connection
            .query_row("SELECT COUNT(*) FROM capture_batches", [], |row| row.get(0))
            .unwrap();
        assert_eq!(batches, 0);
        drop(connection);
        // 补上链接后还能正常收
        assert_eq!(
            commit_capture(&path, "只有文字，没有链接。https://example.com/a", "测试群")
                .unwrap()
                .created,
            1
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn failed_insert_rolls_the_whole_batch_back() {
        let path = temp_db();
        with_account(&path);
        let connection = open_db(&path).unwrap();
        // 人为制造「第二条写入必然失败」：给 inbox_items 加一个触发器，遇到特定链接就报错
        connection
            .execute_batch("CREATE TRIGGER fail_on_boom BEFORE INSERT ON inbox_items WHEN NEW.normalized_url LIKE '%boom%' BEGIN SELECT RAISE(ABORT, 'boom'); END;")
            .unwrap();
        drop(connection);
        let text = "先 https://example.com/ok 后 https://example.com/boom";
        assert!(commit_capture(&path, text, "测试群").is_err());
        let connection = open_db(&path).unwrap();
        let items: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM inbox_items WHERE capture_batch_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let batches: i64 = connection
            .query_row("SELECT COUNT(*) FROM capture_batches", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            (items, batches),
            (0, 0),
            "出错时不能留下半批数据，也不能把这段文字记成已收过"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn cancel_returns_job_and_trending_html_stays_off() {
        let path = temp_db();
        with_account(&path);
        let connection = open_db(&path).unwrap();
        connection.execute("INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status) VALUES ('job-cancel', 'acct', 'repo_card', 'repository', 'repo1', 'queued')", []).unwrap();
        let trending: i64 = connection
            .query_row(
                "SELECT enabled FROM ranking_sources WHERE id = 'github_trending_html'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(connection);
        assert_eq!(trending, 0);
        cancel_job(&path, "job-cancel").unwrap();
        let jobs = list_jobs(&path, 10).unwrap();
        assert_eq!(jobs[0].status, "cancelled");
        assert_eq!(set_daily_budget(&path, 50_000).unwrap(), 50_000);
        log_usage(&path, "repo1", "useful", "测过", "mcp").unwrap();
        let _ = fs::remove_file(path);
    }

    #[test]
    fn five_interrupted_jobs_resume_and_tampered_backup_is_refused() {
        let path = temp_db();
        with_account(&path);
        let connection = open_db(&path).unwrap();
        for index in 0..5 {
            connection.execute(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status) VALUES (?1, 'acct', 'repo_card', 'repository', ?1, 'running')",
                params![format!("run-{index}")],
            ).unwrap();
        }
        connection.execute(
            "INSERT INTO kg_concepts(id, account_id, kind, name_zh, normalized_name) VALUES ('cap-a', 'acct', 'capability', '抽链接', '抽链接'), ('cap-b', 'acct', 'capability', '整理链接', '整理链接')",
            [],
        ).unwrap();
        drop(connection);
        assert_eq!(recover_interrupted_jobs(&path).unwrap(), 5);
        assert!(list_jobs(&path, 10)
            .unwrap()
            .iter()
            .all(|job| job.status == "queued" && job.error_kind.as_deref() == Some("interrupted")));
        merge_concepts(&path, "cap-a", "cap-b").unwrap();
        set_reading_statuses(&path, &["repo1".to_owned()], "want_to_try").unwrap();
        let backup_dir = std::env::temp_dir();
        let backup = backup_database_file(&path, &backup_dir).unwrap();
        let mut bytes = fs::read(&backup).unwrap();
        bytes[0] ^= 0xff;
        fs::write(&backup, bytes).unwrap();
        assert!(restore_database_file(&path, &backup).is_err());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup);
    }

    #[test]
    fn commit_capture_still_writes_paste_batch_and_other() {
        let path = temp_db();
        with_account(&path);
        let committed =
            commit_capture(&path, "旧入口 https://github.com/acme/legacy", "测试群").unwrap();
        assert_eq!(committed.created, 1);
        let connection = open_db(&path).unwrap();
        let (batch_platform, via, excerpt, link_platform): (String, String, String, String) = connection
            .query_row(
                "SELECT capture_batches.source_platform, inbox_items.captured_via, inbox_items.raw_excerpt, inbox_items.source_platform FROM inbox_items JOIN capture_batches ON capture_batches.id = inbox_items.capture_batch_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(batch_platform, "other");
        assert_eq!(via, "paste_batch");
        assert_eq!(excerpt, "");
        assert_eq!(link_platform, "github");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn commit_capture_with_unknown_platform_and_via_fall_back() {
        let path = temp_db();
        with_account(&path);
        let committed = commit_capture_with(
            &path,
            "看 https://github.com/acme/fallback",
            "上下文",
            &CaptureSource {
                platform: "WeChat",
                via: "clipboard",
                original_text: Some("  原话保留  "),
            },
        )
        .unwrap();
        assert_eq!(committed.created, 1);
        let connection = open_db(&path).unwrap();
        let (batch_platform, via, excerpt, link_platform): (String, String, String, String) = connection
            .query_row(
                "SELECT capture_batches.source_platform, inbox_items.captured_via, inbox_items.raw_excerpt, inbox_items.source_platform FROM inbox_items JOIN capture_batches ON capture_batches.id = inbox_items.capture_batch_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(batch_platform, "other");
        assert_eq!(via, "paste_batch");
        assert_eq!(excerpt, "原话保留");
        assert_eq!(link_platform, "github");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn commit_capture_with_stores_known_platform_via_and_trimmed_excerpt() {
        let path = temp_db();
        with_account(&path);
        let words = format!("  {}  ", "字".repeat(2005));
        let committed = commit_capture_with(
            &path,
            "看 https://github.com/acme/excerpt-fn",
            "群",
            &CaptureSource {
                platform: "wechat",
                via: "local_api",
                original_text: Some(&words),
            },
        )
        .unwrap();
        assert_eq!(committed.created, 1);
        let connection = open_db(&path).unwrap();
        let (batch_platform, via, excerpt): (String, String, String) = connection
            .query_row(
                "SELECT capture_batches.source_platform, inbox_items.captured_via, inbox_items.raw_excerpt FROM inbox_items JOIN capture_batches ON capture_batches.id = inbox_items.capture_batch_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(batch_platform, "wechat");
        assert_eq!(via, "local_api");
        assert_eq!(excerpt.chars().count(), 2000);
        assert_eq!(excerpt, "字".repeat(2000));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn retry_failed_jobs_skips_deterministic_failures_and_survives_conflicts() {
        let path = temp_db();
        with_account(&path);
        let connection = open_db(&path).unwrap();
        connection
            .execute_batch(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, error_kind, error_message, finished_at) VALUES
                   ('retry-me', 'acct', 'repo_card', 'repository', 'r1', 'failed', 3, 'bad_json', '模型两次都没有返回可解析的 JSON', '2026-10-03T00:00:00.000Z'),
                   ('too-big', 'acct', 'repo_card', 'repository', 'r2', 'failed', 1, 'content_too_large', '内容超过模型上下文', '2026-10-03T00:00:00.000Z'),
                   ('empty', 'acct', 'repo_card', 'repository', 'r3', 'failed', 1, 'other', 'OpenAI 响应中没有可用内容', '2026-10-03T00:00:00.000Z'),
                   ('conflict-old', 'acct', 'repo_card', 'repository', 'r4', 'failed', 3, 'timeout', '超时', '2026-10-03T00:00:00.000Z');
                 INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status) VALUES
                   ('conflict-new', 'acct', 'repo_card', 'repository', 'r4', 'queued');",
            )
            .unwrap();
        drop(connection);
        // 这个仓库已经有新的排队任务（唯一索引）：旧的失败那条留着，不能让整批重试因此一条都不成
        assert_eq!(retry_failed_jobs(&path).unwrap(), 1);
        let connection = open_db(&path).unwrap();
        let status = |id: &str| -> (String, i64) {
            connection
                .query_row(
                    "SELECT status, attempts FROM ai_jobs WHERE id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap()
        };
        assert_eq!(status("retry-me"), ("queued".to_owned(), 0));
        assert_eq!(status("too-big").0, "failed", "内容太大再试也一样");
        assert_eq!(
            status("empty").0,
            "failed",
            "模型回空内容是确定性的，再试只会白花 token"
        );
        assert_eq!(status("conflict-old").0, "failed");
        assert_eq!(status("conflict-new").0, "queued");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn retry_and_cancel_do_not_redate_old_charges_into_today() {
        let path = temp_db();
        with_account(&path);
        let connection = open_db(&path).unwrap();
        let recent = crate::jobs::now_db_time();
        let old = "2026-01-01T00:00:00.000Z";
        connection
            .execute_batch(&format!(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, error_kind, error_message, input_tokens, finished_at, updated_at, created_at) VALUES
                   ('old-failed', 'acct', 'repo_card', 'repository', 'o1', 'failed', 3, 'timeout', '超时', 30000, '{old}', '{old}', '{old}'),
                   ('new-failed', 'acct', 'repo_card', 'repository', 'n1', 'failed', 3, 'timeout', '超时', 30000, '{recent}', '{recent}', '{recent}'),
                   ('old-cancel', 'acct', 'repo_card', 'repository', 'o2', 'failed', 3, 'timeout', '超时', 30000, '{old}', '{old}', '{old}');"
            ))
            .unwrap();
        drop(connection);
        // 一天前的旧账本来就不计入；重试 / 取消之后也不能被「重新记到今天」
        assert_eq!(crate::jobs::queue_status(&path).unwrap().tokens_24h, 30_000);
        assert_eq!(retry_failed_jobs(&path).unwrap(), 3);
        let connection = open_db(&path).unwrap();
        let tokens = |id: &str| -> i64 {
            connection
                .query_row(
                    "SELECT input_tokens FROM ai_jobs WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap()
        };
        assert_eq!(tokens("old-failed"), 0, "旧失败的用量清零");
        assert_eq!(tokens("new-failed"), 30_000, "24 小时内的失败用量保留");
        assert_eq!(crate::jobs::queue_status(&path).unwrap().tokens_24h, 30_000);
        drop(connection);
        // 取消一条几天前失败的任务：finished_at 沿用原来的，不变成现在
        let connection = open_db(&path).unwrap();
        connection
            .execute("UPDATE ai_jobs SET status = 'failed', finished_at = ?1, updated_at = ?1, input_tokens = 30000 WHERE id = 'old-cancel'", [old])
            .unwrap();
        drop(connection);
        cancel_job(&path, "old-cancel").unwrap();
        let connection = open_db(&path).unwrap();
        let finished: String = connection
            .query_row(
                "SELECT finished_at FROM ai_jobs WHERE id = 'old-cancel'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(finished, old);
        assert_eq!(
            crate::jobs::queue_status(&path).unwrap().tokens_24h,
            30_000 + 0
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_job_list_hides_the_legacy_ledger_rows() {
        let path = temp_db();
        with_account(&path);
        let connection = open_db(&path).unwrap();
        connection
            .execute_batch(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status) VALUES
                   ('real', 'acct', 'repo_card', 'repository', 'r1', 'queued'),
                   ('ledger', 'acct', 'problem_answer', 'query', 'legacy-abc', 'succeeded');",
            )
            .unwrap();
        drop(connection);
        let ids: Vec<String> = list_jobs(&path, 40)
            .unwrap()
            .into_iter()
            .map(|job| job.id)
            .collect();
        assert_eq!(
            ids,
            vec!["real".to_owned()],
            "旧功能的账本行不该出现在任务列表里（也不能被取消）"
        );
        let _ = std::fs::remove_file(&path);
    }
}
