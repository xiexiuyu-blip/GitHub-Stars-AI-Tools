// 资料库与今日的查询。不依赖 Tauri，测试用临时库。
// 计数的每一维都去掉「自己」那一个筛选，所以左侧数字会跟着其他条件变。
// `in_use` 是状态组的字面量，序列化时不能改成驼峰。

use rusqlite::{
    named_params, params, params_from_iter, Connection, OpenFlags, OptionalExtension, Row,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

const CORE_SORT_MAX: i64 = 7;
const NON_CORE_RANK: i64 = 99;
const PICK_LIMIT: usize = 3;
/// 「本周新收藏」最多给多少条。统计卡上的数字是全部，点进去要对得上，所以给得宽一些；只防一周内爆量。
const WEEK_LIMIT: i64 = 60;
const MAX_READING_BATCH: usize = 200;
const README_EXCERPT_CHARS: usize = 1500;

const FILTER_WHERE: &str = r#"
r.sync_status = 'active'
AND (
  :scenario = ''
  OR EXISTS (
    SELECT 1 FROM entity_scenarios es_filter
    WHERE es_filter.entity_type = 'repository'
      AND es_filter.entity_id = r.id
      AND es_filter.scenario_id = :scenario
      AND es_filter.status <> 'rejected'
  )
)
AND (
  :reading = ''
  OR (:reading = 'unseen' AND IFNULL(a.read_status, 'unread') IN ('unread', 'later'))
  OR (:reading = 'want' AND IFNULL(a.read_status, 'unread') IN ('want_to_try', 'watching'))
  OR (:reading = 'tried' AND IFNULL(a.read_status, 'unread') IN ('tried', 'read'))
  OR (:reading = 'in_use' AND IFNULL(a.read_status, 'unread') = 'in_use')
  OR (:reading = 'dropped' AND IFNULL(a.read_status, 'unread') = 'deprecated')
)
AND (
  :card = 'all'
  OR (:card = 'done' AND TRIM(IFNULL(d.one_liner_zh, '')) <> '')
  OR (:card = 'pending' AND TRIM(IFNULL(d.one_liner_zh, '')) = '')
)
AND (
  :form = ''
  OR (
    CASE
      WHEN json_valid(d.card_json) THEN
        CASE
          WHEN json_type(d.card_json, '$.form') = 'text' THEN json_extract(d.card_json, '$.form')
        END
    END
  ) = :form
)
AND (
  :pattern = ''
  OR r.full_name LIKE :pattern ESCAPE '\'
  OR IFNULL(r.description, '') LIKE :pattern ESCAPE '\'
  OR IFNULL(d.one_liner_zh, '') LIKE :pattern ESCAPE '\'
  OR IFNULL(d.summary_zh, '') LIKE :pattern ESCAPE '\'
  OR IFNULL(d.keywords_json, '') LIKE :pattern ESCAPE '\'
  OR IFNULL(r.topics_json, '') LIKE :pattern ESCAPE '\'
)
"#;

const JOINS: &str = "
FROM repositories r
LEFT JOIN annotations a ON a.repo_id = r.id
LEFT JOIN repo_ai_documents d ON d.repo_id = r.id
";

const ITEM_SELECT: &str = "
SELECT r.id, r.full_name, r.html_url, r.description, r.language, r.stars_count,
       r.starred_at, r.pushed_at, d.one_liner_zh, d.card_json, a.read_status
";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFilters {
    #[serde(default)]
    pub scenario: Option<String>,
    #[serde(default)]
    pub reading: Option<String>,
    #[serde(default)]
    pub card_state: Option<String>,
    #[serde(default)]
    pub form: Option<String>,
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars_count: i64,
    pub starred_at: String,
    pub pushed_at: Option<String>,
    pub one_liner: Option<String>,
    /// 列表第三行。为 `None` 时仍序列化成 `null`。
    pub highlight: Option<String>,
    pub form: Option<String>,
    pub scenario_ids: Vec<String>,
    pub reading_status: String,
    pub reading_group: String,
    pub has_card: bool,
    pub confidence: Option<f64>,
    pub card_review: Option<String>,
}

/// 状态组的键保持契约里的字面量。`in_use` 不能被 serde 写成 `inUse`。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReadingCounts {
    pub unseen: i64,
    pub want: i64,
    pub tried: i64,
    #[serde(rename = "in_use")]
    pub in_use: i64,
    pub dropped: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardStateCounts {
    pub done: i64,
    pub pending: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCounts {
    pub total: i64,
    pub scenarios: BTreeMap<String, i64>,
    pub reading: ReadingCounts,
    pub card_state: CardStateCounts,
    pub forms: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub items: Vec<LibraryItem>,
    pub total_count: i64,
    pub counts: LibraryCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDetail {
    #[serde(flatten)]
    pub item: LibraryItem,
    pub what_it_is: Option<String>,
    pub capabilities: Vec<String>,
    pub problems_solved: Vec<String>,
    pub use_when: Vec<String>,
    pub not_for: Vec<String>,
    pub how_to_try: Option<String>,
    pub integrates_with: Vec<String>,
    pub alternatives_hint: Vec<String>,
    pub inspiration_for_fox: Option<String>,
    pub risks: Vec<String>,
    pub license: Option<String>,
    pub maturity: Option<String>,
    pub zh_friendly: Option<bool>,
    pub keywords: Vec<String>,
    pub topics: Vec<String>,
    pub forks_count: i64,
    pub readme_excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayCounts {
    pub total: i64,
    pub unseen: i64,
    pub week_new: i64,
    pub cards_pending: i64,
    pub cards_suggested: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayView {
    pub picks: Vec<LibraryItem>,
    pub week_new: Vec<LibraryItem>,
    pub counts: TodayCounts,
    pub synced_at: Option<String>,
}

#[derive(Clone)]
struct Normalized {
    scenario: String,
    reading: String,
    card: String,
    form: String,
    pattern: String,
    order_sql: &'static str,
    limit: i64,
    offset: i64,
}

#[derive(Clone)]
struct RawItem {
    id: String,
    full_name: String,
    html_url: String,
    description: Option<String>,
    language: Option<String>,
    stars_count: i64,
    starred_at: String,
    pushed_at: Option<String>,
    one_liner_zh: Option<String>,
    card_json: Option<String>,
    read_status: Option<String>,
}

struct Candidate {
    raw: RawItem,
    rank: i64,
    confidence: Option<f64>,
}

/// 没有标注行时按 `unread`。不认识的值也归到没看，避免一条坏数据弄丢整页。
pub fn reading_group_name(status: Option<&str>) -> &'static str {
    match status.unwrap_or("unread") {
        "unread" | "later" => "unseen",
        "want_to_try" | "watching" => "want",
        "tried" | "read" => "tried",
        "in_use" => "in_use",
        "deprecated" => "dropped",
        _ => "unseen",
    }
}

/// 一个状态组在库里对应哪些原始状态。
pub fn reading_statuses_for_group(group: &str) -> Result<&'static [&'static str], String> {
    match group {
        "unseen" => Ok(&["unread", "later"]),
        "want" => Ok(&["want_to_try", "watching"]),
        "tried" => Ok(&["tried", "read"]),
        "in_use" => Ok(&["in_use"]),
        "dropped" => Ok(&["deprecated"]),
        _ => Err("状态组无效".to_owned()),
    }
}

pub fn reading_status_to_store(group: &str) -> Result<&'static str, String> {
    match group {
        "unseen" => Ok("unread"),
        "want" => Ok("want_to_try"),
        "tried" => Ok("tried"),
        "in_use" => Ok("in_use"),
        "dropped" => Ok("deprecated"),
        _ => Err("状态组无效".to_owned()),
    }
}

pub fn list_library_view(path: &Path, filters: &LibraryFilters) -> Result<LibraryPage, String> {
    let normalized = normalize(filters)?;
    let connection = open_read(path)?;
    let total = scalar(
        &connection,
        "SELECT COUNT(*) FROM repositories WHERE sync_status = 'active'",
        &normalized,
        false,
    )?;
    let total_count = scalar(
        &connection,
        &format!("SELECT COUNT(*) {JOINS} WHERE {FILTER_WHERE}"),
        &normalized,
        true,
    )?;
    let items = query_items(&connection, &normalized)?;
    let counts = LibraryCounts {
        total,
        scenarios: scenario_counts(&connection, &normalized.cleared_scenario())?,
        reading: reading_counts(&connection, &normalized.cleared_reading())?,
        card_state: card_state_counts(&connection, &normalized.cleared_card())?,
        forms: form_counts(&connection, &normalized.cleared_form())?,
    };
    Ok(LibraryPage {
        items,
        total_count,
        counts,
    })
}

pub fn get_library_detail(path: &Path, id: &str) -> Result<LibraryDetail, String> {
    let connection = open_read(path)?;
    let sql = "
SELECT r.id, r.full_name, r.html_url, r.description, r.language, r.stars_count,
       r.starred_at, r.pushed_at, d.one_liner_zh, d.card_json, a.read_status,
       r.forks_count, d.keywords_json, r.topics_json, rd.raw_markdown,
       r.license_spdx, r.license_name
FROM repositories r
LEFT JOIN annotations a ON a.repo_id = r.id
LEFT JOIN repo_ai_documents d ON d.repo_id = r.id
LEFT JOIN repo_readmes rd ON rd.repo_id = r.id
WHERE r.id = ?1
";
    let found = connection
        .query_row(sql, params![id], |row| {
            Ok((
                read_raw(row)?,
                row.get::<_, i64>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
                row.get::<_, Option<String>>(16)?,
            ))
        })
        .optional()
        .map_err(|error| format!("仓库详情读取失败：{error}"))?;
    let Some((raw, forks_count, keywords_json, topics_json, readme, license_spdx, license_name)) =
        found
    else {
        return Err("找不到这个仓库".to_owned());
    };
    let scenario_ids = scenario_ids_for(&connection, &[raw.id.clone()])?;
    let topics = parse_string_list(topics_json);
    let mut item = into_item(raw, scenario_ids.get(id).cloned().unwrap_or_default());
    if item.form.is_none() {
        item.form =
            crate::repo_facts::infer_form(&topics, &item.full_name, item.description.as_deref())
                .map(str::to_owned);
    }
    // 列表结构不带整张卡。详情字段从同一行再取一次。
    let card = load_card(&connection, id)?;
    Ok(LibraryDetail {
        item,
        what_it_is: card_string(&card, "what_it_is"),
        capabilities: capabilities_of(&card),
        problems_solved: string_array(&card, "problems_solved"),
        use_when: string_array(&card, "use_when"),
        not_for: string_array(&card, "not_for"),
        how_to_try: how_to_try_of(&card),
        integrates_with: string_array(&card, "integrates_with"),
        alternatives_hint: string_array(&card, "alternatives_hint"),
        inspiration_for_fox: card_string(&card, "inspiration_for_fox"),
        risks: string_array(&card, "risks"),
        license: detail_license(license_spdx, license_name, &card),
        maturity: card_string(&card, "maturity"),
        zh_friendly: card.get("zh_friendly").and_then(Value::as_bool),
        keywords: parse_string_list(keywords_json),
        topics,
        forks_count,
        readme_excerpt: readme.as_deref().and_then(readme_excerpt),
    })
}

pub fn set_library_reading(path: &Path, ids: &[String], group: &str) -> Result<usize, String> {
    let stored = reading_status_to_store(group.trim())?;
    if ids.len() > MAX_READING_BATCH {
        return Err("一次最多设置 200 个".to_owned());
    }
    let connection = open_write(path)?;
    // 先读后写：一开始就拿写锁，避免和后台整理同时提交时立刻报 database is locked。
    let tx =
        rusqlite::Transaction::new_unchecked(&connection, rusqlite::TransactionBehavior::Immediate)
            .map_err(|error| format!("状态写入失败：{error}"))?;
    let mut seen = HashSet::new();
    let mut changed = 0usize;
    for id in ids {
        if id.is_empty() || !seen.insert(id.clone()) {
            continue;
        }
        let account: Option<String> = tx
            .query_row(
                "SELECT account_id FROM repositories WHERE id = ?1 AND sync_status = 'active'",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("仓库读取失败：{error}"))?;
        let Some(account) = account else {
            continue;
        };
        tx.execute(
            "INSERT INTO annotations(repo_id, account_id, read_status, updated_at)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(repo_id) DO UPDATE SET
               read_status = excluded.read_status,
               updated_at = excluded.updated_at",
            params![id, account, stored],
        )
        .map_err(|error| format!("阅读状态写入失败：{error}"))?;
        changed += 1;
    }
    tx.commit()
        .map_err(|error| format!("状态写入失败：{error}"))?;
    Ok(changed)
}

/// `now` 是调用方给的当前时间（RFC 3339）。近 7 天按这个时刻往前推，测试可以固定。
pub fn get_today_view(path: &Path, now: &str) -> Result<TodayView, String> {
    let connection = open_read(path)?;
    let now_ok: i64 = connection
        .query_row(
            "SELECT CASE WHEN julianday(?1) IS NULL THEN 0 ELSE 1 END",
            params![now],
            |row| row.get(0),
        )
        .map_err(|error| format!("时间读取失败：{error}"))?;
    if now_ok == 0 {
        return Err("时间格式无效".to_owned());
    }
    let picks = pick_items(&connection)?;
    let week_new = week_items(&connection, now)?;
    let counts = today_counts(&connection, now)?;
    let synced_at = connection
        .query_row("SELECT MAX(updated_at) FROM repositories", [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("同步时间读取失败：{error}"))?;
    Ok(TodayView {
        picks,
        week_new,
        counts,
        synced_at,
    })
}

fn normalize(filters: &LibraryFilters) -> Result<Normalized, String> {
    let scenario = filters.scenario.as_deref().unwrap_or("").trim().to_owned();
    let reading_raw = filters.reading.as_deref().unwrap_or("").trim();
    if !reading_raw.is_empty() && reading_statuses_for_group(reading_raw).is_err() {
        return Err("状态组筛选无效".to_owned());
    }
    let card = match filters.card_state.as_deref().unwrap_or("").trim() {
        "" | "done" => "done",
        "pending" => "pending",
        "all" => "all",
        _ => return Err("卡片状态筛选无效".to_owned()),
    };
    let order_sql = match filters.sort.as_deref().unwrap_or("").trim() {
        "" | "starred" => "r.starred_at DESC, r.id ASC",
        "stars" => "r.stars_count DESC, r.id ASC",
        "updated" => {
            "CASE WHEN r.pushed_at IS NULL OR TRIM(r.pushed_at) = '' THEN 1 ELSE 0 END, r.pushed_at DESC, r.id ASC"
        }
        _ => return Err("排序方式无效".to_owned()),
    };
    Ok(Normalized {
        scenario,
        reading: reading_raw.to_owned(),
        card: card.to_owned(),
        form: filters.form.as_deref().unwrap_or("").trim().to_owned(),
        pattern: like_pattern(filters.keyword.as_deref().unwrap_or("")),
        order_sql,
        limit: filters.limit.unwrap_or(80).clamp(1, 200),
        offset: filters.offset.unwrap_or(0).max(0),
    })
}

impl Normalized {
    fn cleared_scenario(&self) -> Self {
        let mut copy = self.clone();
        copy.scenario.clear();
        copy
    }

    fn cleared_reading(&self) -> Self {
        let mut copy = self.clone();
        copy.reading.clear();
        copy
    }

    fn cleared_card(&self) -> Self {
        let mut copy = self.clone();
        copy.card = "all".to_owned();
        copy
    }

    fn cleared_form(&self) -> Self {
        let mut copy = self.clone();
        copy.form.clear();
        copy
    }
}

fn like_pattern(keyword: &str) -> String {
    let trimmed = keyword.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let escaped = trimmed
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn open_read(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .busy_timeout(Duration::from_millis(5000))
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    Ok(connection)
}

fn open_write(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    Ok(connection)
}

macro_rules! filter_params {
    ($filters:expr) => {
        named_params! {
            ":scenario": $filters.scenario,
            ":reading": $filters.reading,
            ":card": $filters.card,
            ":form": $filters.form,
            ":pattern": $filters.pattern,
        }
    };
}

fn scalar(
    connection: &Connection,
    sql: &str,
    filters: &Normalized,
    with_filters: bool,
) -> Result<i64, String> {
    let value = if with_filters {
        connection.query_row(sql, filter_params!(filters), |row| row.get(0))
    } else {
        connection.query_row(sql, [], |row| row.get(0))
    };
    value.map_err(|error| format!("资料库计数失败：{error}"))
}

fn query_items(connection: &Connection, filters: &Normalized) -> Result<Vec<LibraryItem>, String> {
    let sql = format!(
        "{ITEM_SELECT} {JOINS} WHERE {FILTER_WHERE} ORDER BY {} LIMIT :limit OFFSET :offset",
        filters.order_sql
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("资料库查询准备失败：{error}"))?;
    let rows = statement
        .query_map(
            named_params! {
                ":scenario": filters.scenario,
                ":reading": filters.reading,
                ":card": filters.card,
                ":form": filters.form,
                ":pattern": filters.pattern,
                ":limit": filters.limit,
                ":offset": filters.offset,
            },
            read_raw,
        )
        .map_err(|error| format!("资料库查询失败：{error}"))?;
    let mut raws = Vec::new();
    for row in rows {
        raws.push(row.map_err(|error| format!("资料库行读取失败：{error}"))?);
    }
    let ids: Vec<String> = raws.iter().map(|row| row.id.clone()).collect();
    let scenarios = scenario_ids_for(connection, &ids)?;
    Ok(raws
        .into_iter()
        .map(|raw| {
            let id = raw.id.clone();
            into_item(raw, scenarios.get(&id).cloned().unwrap_or_default())
        })
        .collect())
}

fn scenario_counts(
    connection: &Connection,
    filters: &Normalized,
) -> Result<BTreeMap<String, i64>, String> {
    let sql = format!(
        "SELECT s.id, COUNT(DISTINCT CASE WHEN {FILTER_WHERE} THEN r.id END)
         FROM scenarios s
         LEFT JOIN entity_scenarios es
           ON es.scenario_id = s.id
          AND es.entity_type = 'repository'
          AND es.status <> 'rejected'
         LEFT JOIN repositories r ON r.id = es.entity_id
         LEFT JOIN annotations a ON a.repo_id = r.id
         LEFT JOIN repo_ai_documents d ON d.repo_id = r.id
         WHERE s.archived = 0
         GROUP BY s.id"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("场景计数准备失败：{error}"))?;
    let rows = statement
        .query_map(filter_params!(filters), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("场景计数失败：{error}"))?;
    let mut counts = BTreeMap::new();
    for row in rows {
        let (id, count) = row.map_err(|error| format!("场景计数读取失败：{error}"))?;
        counts.insert(id, count);
    }
    Ok(counts)
}

fn reading_counts(connection: &Connection, filters: &Normalized) -> Result<ReadingCounts, String> {
    let sql = format!(
        "SELECT
            COALESCE(SUM(CASE WHEN IFNULL(a.read_status, 'unread') IN ('unread', 'later') THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN IFNULL(a.read_status, 'unread') IN ('want_to_try', 'watching') THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN IFNULL(a.read_status, 'unread') IN ('tried', 'read') THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN IFNULL(a.read_status, 'unread') = 'in_use' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN IFNULL(a.read_status, 'unread') = 'deprecated' THEN 1 ELSE 0 END), 0)
         {JOINS}
         WHERE {FILTER_WHERE}"
    );
    connection
        .query_row(&sql, filter_params!(filters), |row| {
            Ok(ReadingCounts {
                unseen: row.get(0)?,
                want: row.get(1)?,
                tried: row.get(2)?,
                in_use: row.get(3)?,
                dropped: row.get(4)?,
            })
        })
        .map_err(|error| format!("状态计数失败：{error}"))
}

fn card_state_counts(
    connection: &Connection,
    filters: &Normalized,
) -> Result<CardStateCounts, String> {
    let sql = format!(
        "SELECT
            COALESCE(SUM(CASE WHEN TRIM(IFNULL(d.one_liner_zh, '')) <> '' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN TRIM(IFNULL(d.one_liner_zh, '')) = '' THEN 1 ELSE 0 END), 0)
         {JOINS}
         WHERE {FILTER_WHERE}"
    );
    connection
        .query_row(&sql, filter_params!(filters), |row| {
            Ok(CardStateCounts {
                done: row.get(0)?,
                pending: row.get(1)?,
            })
        })
        .map_err(|error| format!("卡片计数失败：{error}"))
}

fn form_counts(
    connection: &Connection,
    filters: &Normalized,
) -> Result<BTreeMap<String, i64>, String> {
    let sql = format!(
        "SELECT form, COUNT(*)
         FROM (
            SELECT CASE
                WHEN json_valid(d.card_json) THEN
                    CASE
                        WHEN json_type(d.card_json, '$.form') = 'text'
                        THEN json_extract(d.card_json, '$.form')
                    END
            END AS form
            {JOINS}
            WHERE {FILTER_WHERE}
         )
         WHERE form IS NOT NULL AND form <> ''
         GROUP BY form"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("形态计数准备失败：{error}"))?;
    let rows = statement
        .query_map(filter_params!(filters), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("形态计数失败：{error}"))?;
    let mut counts = BTreeMap::new();
    for row in rows {
        let (form, count) = row.map_err(|error| format!("形态计数读取失败：{error}"))?;
        counts.insert(form, count);
    }
    Ok(counts)
}

fn scenario_ids_for(
    connection: &Connection,
    ids: &[String],
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let placeholders = std::iter::repeat("?")
        .take(ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT es.entity_id, es.scenario_id
         FROM entity_scenarios es
         JOIN scenarios s ON s.id = es.scenario_id
         WHERE es.entity_type = 'repository'
           AND es.status <> 'rejected'
           AND es.entity_id IN ({placeholders})
         ORDER BY s.sort_order ASC, s.id ASC"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("场景读取准备失败：{error}"))?;
    let rows = statement
        .query_map(params_from_iter(ids.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("场景读取失败：{error}"))?;
    for row in rows {
        let (repo_id, scenario_id) = row.map_err(|error| format!("场景行读取失败：{error}"))?;
        map.entry(repo_id).or_default().push(scenario_id);
    }
    Ok(map)
}

fn read_raw(row: &Row<'_>) -> rusqlite::Result<RawItem> {
    Ok(RawItem {
        id: row.get(0)?,
        full_name: row.get(1)?,
        html_url: row.get(2)?,
        description: row.get(3)?,
        language: row.get(4)?,
        stars_count: row.get(5)?,
        starred_at: row.get(6)?,
        pushed_at: blank_to_none(row.get(7)?),
        one_liner_zh: row.get(8)?,
        card_json: row.get(9)?,
        read_status: row.get(10)?,
    })
}

fn into_item(raw: RawItem, scenario_ids: Vec<String>) -> LibraryItem {
    let raw_card = parse_card_value(raw.card_json.as_deref());
    let stored = nonempty_spaces(raw.one_liner_zh);
    let has_card = stored.is_some();
    let one_liner = crate::card_view::display_one_liner(stored.as_deref(), &raw_card);
    let card = sanitized_card(raw_card);
    let reading_status = raw.read_status.unwrap_or_else(|| "unread".to_owned());
    LibraryItem {
        id: raw.id,
        full_name: raw.full_name,
        html_url: raw.html_url,
        description: raw.description,
        language: raw.language,
        stars_count: raw.stars_count,
        starred_at: raw.starred_at,
        pushed_at: raw.pushed_at,
        form: card_string(&card, "form"),
        scenario_ids,
        reading_group: reading_group_name(Some(reading_status.as_str())).to_owned(),
        reading_status,
        one_liner,
        highlight: highlight_of(&card),
        has_card,
        confidence: card_number(&card, "confidence"),
        card_review: card_review(has_card, &card),
    }
}

fn load_card(connection: &Connection, id: &str) -> Result<Value, String> {
    let raw: Option<String> = connection
        .query_row(
            "SELECT card_json FROM repo_ai_documents WHERE repo_id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("理解卡读取失败：{error}"))?
        .flatten();
    Ok(parse_card(raw.as_deref()))
}

fn pick_items(connection: &Connection) -> Result<Vec<LibraryItem>, String> {
    let sql = format!(
        "{ITEM_SELECT} {JOINS}
         WHERE r.sync_status = 'active'
           AND TRIM(IFNULL(d.one_liner_zh, '')) <> ''
           AND IFNULL(a.read_status, 'unread') IN ('unread', 'later')"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("今日挑选准备失败：{error}"))?;
    let rows = statement
        .query_map([], read_raw)
        .map_err(|error| format!("今日挑选失败：{error}"))?;
    let ranks = core_ranks(connection)?;
    let mut candidates = Vec::new();
    for row in rows {
        let raw = row.map_err(|error| format!("今日挑选行读取失败：{error}"))?;
        let card = parse_card(raw.card_json.as_deref());
        let rank = ranks.get(&raw.id).copied().unwrap_or(NON_CORE_RANK);
        candidates.push(Candidate {
            confidence: card_number(&card, "confidence"),
            rank,
            raw,
        });
    }
    candidates.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then(cmp_confidence_desc(left.confidence, right.confidence))
            .then(right.raw.starred_at.cmp(&left.raw.starred_at))
            .then(left.raw.id.cmp(&right.raw.id))
    });
    let indexes = select_pick_indexes(&candidates);
    let chosen: Vec<RawItem> = indexes
        .into_iter()
        .map(|index| candidates[index].raw.clone())
        .collect();
    items_from_raws(connection, chosen)
}

fn select_pick_indexes(items: &[Candidate]) -> Vec<usize> {
    let mut chosen = Vec::new();
    let mut seen = HashSet::new();
    for (index, item) in items.iter().enumerate() {
        if chosen.len() == PICK_LIMIT {
            break;
        }
        if seen.insert(item.rank) {
            chosen.push(index);
        }
    }
    if chosen.len() < PICK_LIMIT {
        for index in 0..items.len() {
            if chosen.len() == PICK_LIMIT {
                break;
            }
            if !chosen.contains(&index) {
                chosen.push(index);
            }
        }
    }
    chosen.sort_unstable();
    chosen
}

fn cmp_confidence_desc(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => right.partial_cmp(&left).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn core_ranks(connection: &Connection) -> Result<HashMap<String, i64>, String> {
    let mut statement = connection
        .prepare(
            "SELECT es.entity_id, MIN(s.sort_order)
             FROM entity_scenarios es
             JOIN scenarios s ON s.id = es.scenario_id
             WHERE es.entity_type = 'repository'
               AND es.status <> 'rejected'
               AND s.sort_order <= ?1
             GROUP BY es.entity_id",
        )
        .map_err(|error| format!("方向排名准备失败：{error}"))?;
    let rows = statement
        .query_map(params![CORE_SORT_MAX], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("方向排名失败：{error}"))?;
    let mut ranks = HashMap::new();
    for row in rows {
        let (id, rank) = row.map_err(|error| format!("方向排名读取失败：{error}"))?;
        ranks.insert(id, rank);
    }
    Ok(ranks)
}

fn week_items(connection: &Connection, now: &str) -> Result<Vec<LibraryItem>, String> {
    let sql = format!(
        "{ITEM_SELECT} {JOINS}
         WHERE r.sync_status = 'active'
           AND julianday(r.starred_at) >= julianday(?1) - 7
         ORDER BY r.starred_at DESC, r.id ASC
         LIMIT {WEEK_LIMIT}"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("本周新收藏准备失败：{error}"))?;
    let rows = statement
        .query_map(params![now], read_raw)
        .map_err(|error| format!("本周新收藏失败：{error}"))?;
    let mut raws = Vec::new();
    for row in rows {
        raws.push(row.map_err(|error| format!("本周新收藏行读取失败：{error}"))?);
    }
    items_from_raws(connection, raws)
}

fn items_from_raws(
    connection: &Connection,
    raws: Vec<RawItem>,
) -> Result<Vec<LibraryItem>, String> {
    let ids: Vec<String> = raws.iter().map(|row| row.id.clone()).collect();
    let scenarios = scenario_ids_for(connection, &ids)?;
    Ok(raws
        .into_iter()
        .map(|raw| {
            let id = raw.id.clone();
            into_item(raw, scenarios.get(&id).cloned().unwrap_or_default())
        })
        .collect())
}

fn today_counts(connection: &Connection, now: &str) -> Result<TodayCounts, String> {
    connection
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM repositories WHERE sync_status = 'active'),
                (SELECT COUNT(*)
                   FROM repositories r
                   LEFT JOIN annotations a ON a.repo_id = r.id
                  WHERE r.sync_status = 'active'
                    AND IFNULL(a.read_status, 'unread') IN ('unread', 'later')),
                (SELECT COUNT(*)
                   FROM repositories
                  WHERE sync_status = 'active'
                    AND julianday(starred_at) >= julianday(?1) - 7),
                (SELECT COUNT(*)
                   FROM repositories r
                   LEFT JOIN repo_ai_documents d ON d.repo_id = r.id
                  WHERE r.sync_status = 'active'
                    AND TRIM(IFNULL(d.one_liner_zh, '')) = ''),
                (SELECT COUNT(*)
                   FROM repositories r
                   LEFT JOIN repo_ai_documents d ON d.repo_id = r.id
                  WHERE r.sync_status = 'active'
                    AND TRIM(IFNULL(d.one_liner_zh, '')) <> ''
                    AND IFNULL(
                          CASE
                            WHEN json_valid(d.card_json) THEN json_extract(d.card_json, '$.review_status')
                          END,
                          ''
                        ) <> 'confirmed')",
            params![now],
            |row| {
                Ok(TodayCounts {
                    total: row.get(0)?,
                    unseen: row.get(1)?,
                    week_new: row.get(2)?,
                    cards_pending: row.get(3)?,
                    cards_suggested: row.get(4)?,
                })
            },
        )
        .map_err(|error| format!("今日计数失败：{error}"))
}

fn parse_card(raw: Option<&str>) -> Value {
    sanitized_card(parse_card_value(raw))
}

fn parse_card_value(raw: Option<&str>) -> Value {
    let Some(raw) = raw.map(str::trim).filter(|text| !text.is_empty()) else {
        return Value::Null;
    };
    serde_json::from_str(raw).unwrap_or(Value::Null)
}

fn sanitized_card(value: Value) -> Value {
    match crate::card_view::sanitize(&value) {
        std::borrow::Cow::Borrowed(_) => value,
        std::borrow::Cow::Owned(clean) => clean,
    }
}

/// 已声明的 SPDX（含 NOASSERTION / Other）优先于 AI 卡。
/// 两列都是 NULL：还没同步。GitHub 返回 null 时同步也会写成 NULL，和「还没同步」相同，这时落到 AI 卡，没有就是 None。
/// 列里已经有空白标识时，视为明确未声明，返回「未声明」，不再退回 AI 卡。
fn detail_license(spdx: Option<String>, name: Option<String>, card: &Value) -> Option<String> {
    let label = crate::repo_facts::license_label(spdx.as_deref(), name.as_deref());
    if label.declared {
        return Some(label.text);
    }
    if spdx.is_none() && name.is_none() {
        return card_string(card, "license");
    }
    Some(label.text)
}

fn card_string(card: &Value, key: &str) -> Option<String> {
    let text = card.get(key)?.as_str()?.trim_matches(' ');
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

fn card_number(card: &Value, key: &str) -> Option<f64> {
    let number = card.get(key)?.as_f64()?;
    if number.is_finite() {
        Some(number)
    } else {
        None
    }
}

fn card_review(has_card: bool, card: &Value) -> Option<String> {
    if !has_card {
        return None;
    }
    match card.get("review_status").and_then(Value::as_str) {
        Some("confirmed") => Some("confirmed".to_owned()),
        Some("suggested") => Some("suggested".to_owned()),
        _ => None,
    }
}

/// 先取第一条非空的「解决的问题」，没有再取第一条能力名。不和一句话比较。
fn highlight_of(card: &Value) -> Option<String> {
    string_array(card, "problems_solved")
        .into_iter()
        .next()
        .or_else(|| capabilities_of(card).into_iter().next())
}

fn string_array(card: &Value, key: &str) -> Vec<String> {
    let Some(items) = card.get(key).and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let text = item.as_str()?.trim_matches(' ');
            if text.is_empty() {
                None
            } else {
                Some(text.to_owned())
            }
        })
        .collect()
}

fn capabilities_of(card: &Value) -> Vec<String> {
    let Some(items) = card.get("capabilities").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if let Some(text) = item.as_str() {
                let text = text.trim_matches(' ');
                return if text.is_empty() {
                    None
                } else {
                    Some(text.to_owned())
                };
            }
            let text = item.get("name_zh")?.as_str()?.trim_matches(' ');
            if text.is_empty() {
                None
            } else {
                Some(text.to_owned())
            }
        })
        .collect()
}

/// 字符串就用字符串。数组时把里面的字符串用换行接起来，类型不对就当没有。
fn how_to_try_of(card: &Value) -> Option<String> {
    let raw = card.get("how_to_try")?;
    if let Some(text) = raw.as_str() {
        let text = text.trim_matches(' ');
        return if text.is_empty() {
            None
        } else {
            Some(text.to_owned())
        };
    }
    let items = raw.as_array()?;
    let parts: Vec<String> = items
        .iter()
        .filter_map(|item| {
            let text = item.as_str()?.trim_matches(' ');
            if text.is_empty() {
                None
            } else {
                Some(text.to_owned())
            }
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn parse_string_list(raw: Option<String>) -> Vec<String> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(raw.trim()) else {
        return Vec::new();
    };
    string_array_value(&value)
}

fn string_array_value(value: &Value) -> Vec<String> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let text = item.as_str()?.trim_matches(' ');
            if text.is_empty() {
                None
            } else {
                Some(text.to_owned())
            }
        })
        .collect()
}

fn nonempty_spaces(value: Option<String>) -> Option<String> {
    let text = value?;
    let trimmed = text.trim_matches(' ');
    if trimmed.is_empty() {
        None
    } else if trimmed.len() == text.len() {
        Some(text)
    } else {
        Some(trimmed.to_owned())
    }
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

fn readme_excerpt(raw: &str) -> Option<String> {
    let without_comments = strip_html_comments(raw);
    let mut kept = Vec::new();
    for line in without_comments.lines() {
        if is_image_line(line) {
            continue;
        }
        kept.push(line);
    }
    let joined = kept.join("\n");
    let trimmed = joined.trim();
    if trimmed.is_empty() {
        return None;
    }
    let excerpt: String = trimmed.chars().take(README_EXCERPT_CHARS).collect();
    if excerpt.is_empty() {
        None
    } else {
        Some(excerpt)
    }
}

fn strip_html_comments(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if index + 3 < bytes.len()
            && bytes[index] == b'<'
            && bytes[index + 1] == b'!'
            && bytes[index + 2] == b'-'
            && bytes[index + 3] == b'-'
        {
            index += 4;
            let mut closed = false;
            while index + 2 < bytes.len() {
                if bytes[index] == b'-' && bytes[index + 1] == b'-' && bytes[index + 2] == b'>' {
                    index += 3;
                    closed = true;
                    break;
                }
                index += 1;
            }
            if !closed {
                break;
            }
            continue;
        }
        let Some(ch) = input[index..].chars().next() else {
            break;
        };
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn is_image_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    if is_markdown_image_line(trimmed) {
        return true;
    }
    if !trimmed.to_ascii_lowercase().contains("<img") {
        return false;
    }
    strip_html_tags(trimmed).trim().is_empty()
}

fn is_markdown_image_line(trimmed: &str) -> bool {
    let mut rest = trimmed;
    let mut saw = false;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return saw;
        }
        if !rest.starts_with("![") {
            return false;
        }
        let Some(alt_end) = rest[2..].find("](") else {
            return false;
        };
        let after_alt = &rest[2 + alt_end + 2..];
        let Some(end) = after_alt.find(')') else {
            return false;
        };
        saw = true;
        rest = &after_alt[end + 1..];
    }
}

fn strip_html_tags(input: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in input.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if in_tag {
            if ch == '>' {
                in_tag = false;
            }
            continue;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::migrate_database;
    use rusqlite::Connection;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    struct Lab {
        path: PathBuf,
    }

    impl Lab {
        fn new(label: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let n = NEXT.fetch_add(1, AtomicOrdering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应可用")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("fsl-libview-{label}-{nanos}-{n}.sqlite3"));
            migrate_database(&path).expect("测试库应能迁移");
            let conn = Connection::open(&path).expect("应能打开测试库");
            conn.execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .expect("应能写入账号");
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            drop(conn);
            Self { path }
        }

        fn with<T>(&self, body: impl FnOnce(&mut Connection) -> T) -> T {
            let mut conn = Connection::open(&self.path).expect("应能打开测试库");
            conn.execute_batch("PRAGMA foreign_keys = ON;")
                .expect("应能打开外键");
            let value = body(&mut conn);
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .expect("应能检查点");
            value
        }
    }

    impl Drop for Lab {
        fn drop(&mut self) {
            for suffix in ["", "-wal", "-shm"] {
                let _ = fs::remove_file(format!("{}{suffix}", self.path.display()));
            }
        }
    }

    struct Repo {
        id: String,
        full_name: String,
        description: Option<String>,
        language: Option<String>,
        topics_json: String,
        stars: i64,
        forks: i64,
        starred_at: String,
        pushed_at: Option<String>,
        sync_status: String,
        updated_at: String,
        write_doc: bool,
        one_liner: Option<String>,
        summary: String,
        keywords_json: String,
        card_json: Option<String>,
        reading: Option<String>,
        note: String,
        rating: Option<i64>,
    }

    impl Repo {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_owned(),
                full_name: format!("o/{id}"),
                description: None,
                language: None,
                topics_json: "[]".to_owned(),
                stars: 1,
                forks: 0,
                starred_at: "2026-10-01T00:00:00Z".to_owned(),
                pushed_at: None,
                sync_status: "active".to_owned(),
                updated_at: "2026-10-01T00:00:00Z".to_owned(),
                write_doc: true,
                one_liner: Some(format!("{id} 的一句话")),
                summary: "摘要".to_owned(),
                keywords_json: "[]".to_owned(),
                card_json: Some(
                    r#"{"form":"library","confidence":0.5,"review_status":"suggested"}"#.to_owned(),
                ),
                reading: None,
                note: String::new(),
                rating: None,
            }
        }

        fn save(&self, conn: &Connection) {
            conn.execute(
                "INSERT INTO repositories(
                    id, account_id, owner, name, full_name, description, language, topics_json,
                    html_url, stars_count, forks_count, starred_at, pushed_at, sync_status, updated_at
                 ) VALUES (?1, 'acct', 'o', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    self.id,
                    self.full_name,
                    self.description,
                    self.language,
                    self.topics_json,
                    format!("https://github.com/{}", self.full_name),
                    self.stars,
                    self.forks,
                    self.starred_at,
                    self.pushed_at,
                    self.sync_status,
                    self.updated_at,
                ],
            )
            .expect("应能写入仓库");
            if self.write_doc {
                conn.execute(
                    "INSERT INTO repo_ai_documents(
                        repo_id, summary_zh, keywords_json, suggested_tags_json, model,
                        prompt_version, source_hash, generated_at, one_liner_zh, card_json
                     ) VALUES (?1, ?2, ?3, '[]', 'test', 'v1', 'h', '2026-01-01T00:00:00Z', ?4, ?5)",
                    params![
                        self.id,
                        self.summary,
                        self.keywords_json,
                        self.one_liner.clone().unwrap_or_default(),
                        self.card_json,
                    ],
                )
                .expect("应能写入理解卡");
            }
            if self.reading.is_some() || !self.note.is_empty() || self.rating.is_some() {
                conn.execute(
                    "INSERT INTO annotations(repo_id, account_id, note_md, rating, read_status)
                     VALUES (?1, 'acct', ?2, ?3, ?4)",
                    params![
                        self.id,
                        self.note,
                        self.rating,
                        self.reading.clone().unwrap_or_else(|| "unread".to_owned()),
                    ],
                )
                .expect("应能写入标注");
            }
        }
    }

    fn scenario(conn: &Connection, id: &str, slug: &str, name: &str, sort: i64, archived: i64) {
        conn.execute(
            "INSERT INTO scenarios(id, account_id, slug, name_zh, sort_order, archived)
             VALUES (?1, 'acct', ?2, ?3, ?4, ?5)",
            params![id, slug, name, sort, archived],
        )
        .expect("应能写入场景");
    }

    fn link(conn: &Connection, repo: &str, scenario_id: &str, status: &str) {
        conn.execute(
            "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, origin, status)
             VALUES ('acct', 'repository', ?1, ?2, 'rule', ?3)",
            params![repo, scenario_id, status],
        )
        .expect("应能写入归类");
    }

    fn readme(conn: &Connection, repo: &str, markdown: &str) {
        conn.execute(
            "INSERT INTO repo_readmes(repo_id, raw_markdown, content_hash, source_path, fetched_at)
             VALUES (?1, ?2, 'h', 'README.md', '2026-01-01T00:00:00Z')",
            params![repo, markdown],
        )
        .expect("应能写入 README");
    }

    fn page_ids(path: &Path, filters: LibraryFilters) -> Vec<String> {
        list_library_view(path, &filters)
            .expect("资料库查询应成功")
            .items
            .into_iter()
            .map(|item| item.id)
            .collect()
    }

    fn object_keys(value: &Value) -> BTreeSet<String> {
        value
            .as_object()
            .expect("应为对象")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn reading_group_maps_every_raw_status_and_missing_annotation() {
        assert_eq!(reading_group_name(None), "unseen");
        assert_eq!(reading_group_name(Some("unread")), "unseen");
        assert_eq!(reading_group_name(Some("later")), "unseen");
        assert_eq!(reading_group_name(Some("want_to_try")), "want");
        assert_eq!(reading_group_name(Some("watching")), "want");
        assert_eq!(reading_group_name(Some("tried")), "tried");
        assert_eq!(reading_group_name(Some("read")), "tried");
        assert_eq!(reading_group_name(Some("in_use")), "in_use");
        assert_eq!(reading_group_name(Some("deprecated")), "dropped");
        assert_eq!(
            reading_statuses_for_group("unseen").unwrap(),
            &["unread", "later"][..]
        );
        assert_eq!(
            reading_statuses_for_group("want").unwrap(),
            &["want_to_try", "watching"][..]
        );
        assert_eq!(
            reading_statuses_for_group("tried").unwrap(),
            &["tried", "read"][..]
        );
        assert_eq!(
            reading_statuses_for_group("in_use").unwrap(),
            &["in_use"][..]
        );
        assert_eq!(
            reading_statuses_for_group("dropped").unwrap(),
            &["deprecated"][..]
        );
        assert!(reading_statuses_for_group("nope").is_err());

        let lab = Lab::new("groups");
        let rows = [
            ("unread", Some("unread"), "unseen"),
            ("later", Some("later"), "unseen"),
            ("want_to_try", Some("want_to_try"), "want"),
            ("watching", Some("watching"), "want"),
            ("tried", Some("tried"), "tried"),
            ("read", Some("read"), "tried"),
            ("in_use", Some("in_use"), "in_use"),
            ("deprecated", Some("deprecated"), "dropped"),
            ("missing", None, "unseen"),
        ];
        lab.with(|conn| {
            for (id, status, _) in rows {
                let mut repo = Repo::new(id);
                repo.reading = status.map(str::to_owned);
                repo.save(conn);
            }
        });
        let mut filters = LibraryFilters::default();
        filters.card_state = Some("all".to_owned());
        filters.limit = Some(20);
        let page = list_library_view(&lab.path, &filters).unwrap();
        assert_eq!(page.items.len(), rows.len());
        for (id, status, group) in rows {
            let item = page.items.iter().find(|item| item.id == id).unwrap();
            assert_eq!(item.reading_status, status.unwrap_or("unread"));
            assert_eq!(item.reading_group, group);
        }
    }

    #[test]
    fn card_state_defaults_to_done_and_can_show_pending() {
        let lab = Lab::new("card-state");
        lab.with(|conn| {
            let mut done = Repo::new("done");
            done.one_liner = Some("有卡".to_owned());
            done.save(conn);
            let mut empty = Repo::new("empty");
            empty.write_doc = false;
            empty.one_liner = None;
            empty.save(conn);
            let mut blank = Repo::new("blank");
            blank.one_liner = Some("   ".to_owned());
            blank.summary = "这是摘要不能拿来当一句话".to_owned();
            blank.save(conn);
        });
        let defaults = page_ids(&lab.path, LibraryFilters::default());
        assert_eq!(defaults, vec!["done".to_owned()]);

        let mut all = LibraryFilters::default();
        all.card_state = Some("all".to_owned());
        let page = list_library_view(&lab.path, &all).unwrap();
        let mut ids: Vec<_> = page.items.iter().map(|item| item.id.clone()).collect();
        ids.sort();
        assert_eq!(ids, vec!["blank", "done", "empty"]);
        let blank = page.items.iter().find(|item| item.id == "blank").unwrap();
        assert!(blank.one_liner.is_none());
        assert!(!blank.has_card);

        let mut pending = LibraryFilters::default();
        pending.card_state = Some("pending".to_owned());
        let mut pending_ids = page_ids(&lab.path, pending);
        pending_ids.sort();
        assert_eq!(pending_ids, vec!["blank", "empty"]);
    }

    #[test]
    fn one_liner_does_not_fall_back_to_summary() {
        let lab = Lab::new("one-liner");
        lab.with(|conn| {
            let mut with_line = Repo::new("with");
            with_line.one_liner = Some("真正的一句话".to_owned());
            with_line.summary = "另一段摘要".to_owned();
            with_line.save(conn);
            let mut summary_only = Repo::new("summary");
            summary_only.one_liner = Some(String::new());
            summary_only.summary = "只有摘要".to_owned();
            summary_only.save(conn);
        });
        let mut filters = LibraryFilters::default();
        filters.card_state = Some("all".to_owned());
        let page = list_library_view(&lab.path, &filters).unwrap();
        let with_line = page.items.iter().find(|item| item.id == "with").unwrap();
        assert_eq!(with_line.one_liner.as_deref(), Some("真正的一句话"));
        assert!(with_line.has_card);
        let summary_only = page.items.iter().find(|item| item.id == "summary").unwrap();
        assert!(summary_only.one_liner.is_none());
        assert!(!summary_only.has_card);
    }

    #[test]
    fn keyword_matches_each_column_case_insensitively() {
        let lab = Lab::new("keyword-cols");
        lab.with(|conn| {
            let mut name = Repo::new("name");
            name.full_name = "Alpha/Widget".to_owned();
            name.one_liner = Some("填充".to_owned());
            name.save(conn);
            let mut description = Repo::new("desc");
            description.description = Some("描述专用".to_owned());
            description.one_liner = Some("填充".to_owned());
            description.save(conn);
            let mut line = Repo::new("line");
            line.one_liner = Some("一句话专用".to_owned());
            line.save(conn);
            let mut summary = Repo::new("summary");
            summary.summary = "摘要专用".to_owned();
            summary.one_liner = Some("填充".to_owned());
            summary.save(conn);
            let mut keywords = Repo::new("keywords");
            keywords.keywords_json = r#"["关键词专用"]"#.to_owned();
            keywords.one_liner = Some("填充".to_owned());
            keywords.save(conn);
            let mut topics = Repo::new("topics");
            topics.topics_json = r#"["主题专用"]"#.to_owned();
            topics.one_liner = Some("填充".to_owned());
            topics.save(conn);
        });
        let cases = [
            ("wIdGeT", "name"),
            ("描述专用", "desc"),
            ("一句话专用", "line"),
            ("摘要专用", "summary"),
            ("关键词专用", "keywords"),
            ("主题专用", "topics"),
        ];
        for (keyword, id) in cases {
            let mut filters = LibraryFilters::default();
            filters.keyword = Some(keyword.to_owned());
            assert_eq!(
                page_ids(&lab.path, filters),
                vec![id.to_owned()],
                "{keyword}"
            );
        }
    }

    #[test]
    fn keyword_escapes_percent_and_underscore() {
        let lab = Lab::new("keyword-escape");
        lab.with(|conn| {
            let mut literal = Repo::new("literal");
            literal.full_name = "o/100%_pure".to_owned();
            literal.save(conn);
            let mut other = Repo::new("other");
            other.full_name = "o/100XYpure".to_owned();
            other.save(conn);
            let mut plain = Repo::new("plain");
            plain.full_name = "o/plain".to_owned();
            plain.save(conn);
        });
        for keyword in ["%", "_", "100%_pure"] {
            let mut filters = LibraryFilters::default();
            filters.keyword = Some(keyword.to_owned());
            assert_eq!(
                page_ids(&lab.path, filters),
                vec!["literal".to_owned()],
                "{keyword}"
            );
        }
    }

    #[test]
    fn scenario_filter_skips_rejected_links() {
        let lab = Lab::new("scenario");
        lab.with(|conn| {
            scenario(conn, "scn-a", "a", "甲", 1, 0);
            scenario(conn, "scn-b", "b", "乙", 2, 0);
            Repo::new("kept").save(conn);
            Repo::new("rejected").save(conn);
            Repo::new("other").save(conn);
            Repo::new("both").save(conn);
            link(conn, "kept", "scn-a", "confirmed");
            link(conn, "rejected", "scn-a", "rejected");
            link(conn, "other", "scn-b", "suggested");
            link(conn, "both", "scn-a", "suggested");
            link(conn, "both", "scn-b", "suggested");
        });
        let mut filters = LibraryFilters::default();
        filters.scenario = Some("scn-a".to_owned());
        let mut ids = page_ids(&lab.path, filters);
        ids.sort();
        assert_eq!(ids, vec!["both", "kept"]);
        let mut filters = LibraryFilters::default();
        filters.scenario = Some("scn-b".to_owned());
        let mut ids = page_ids(&lab.path, filters);
        ids.sort();
        assert_eq!(ids, vec!["both", "other"]);
    }

    #[test]
    fn reading_filter_uses_the_group() {
        let lab = Lab::new("reading");
        let rows = [
            ("unread", Some("unread")),
            ("later", Some("later")),
            ("missing", None),
            ("want_to_try", Some("want_to_try")),
            ("watching", Some("watching")),
            ("tried", Some("tried")),
            ("read", Some("read")),
            ("in_use", Some("in_use")),
            ("deprecated", Some("deprecated")),
        ];
        lab.with(|conn| {
            for (id, status) in rows {
                let mut repo = Repo::new(id);
                repo.reading = status.map(str::to_owned);
                repo.save(conn);
            }
        });
        let expect = [
            ("unseen", vec!["later", "missing", "unread"]),
            ("want", vec!["want_to_try", "watching"]),
            ("tried", vec!["read", "tried"]),
            ("in_use", vec!["in_use"]),
            ("dropped", vec!["deprecated"]),
        ];
        for (group, mut expected) in expect {
            expected.sort();
            let mut filters = LibraryFilters::default();
            filters.reading = Some(group.to_owned());
            filters.limit = Some(20);
            let mut ids = page_ids(&lab.path, filters);
            ids.sort();
            assert_eq!(ids, expected, "{group}");
        }
    }

    #[test]
    fn form_filter_reads_card_form() {
        let lab = Lab::new("form");
        lab.with(|conn| {
            let mut app = Repo::new("app");
            app.card_json = Some(r#"{"form":"app","review_status":"suggested"}"#.to_owned());
            app.save(conn);
            let mut cli = Repo::new("cli");
            cli.card_json = Some(r#"{"form":"cli"}"#.to_owned());
            cli.save(conn);
            let mut bad = Repo::new("bad");
            bad.card_json = Some("{".to_owned());
            bad.save(conn);
            let mut number = Repo::new("number");
            number.card_json = Some(r#"{"form":1}"#.to_owned());
            number.save(conn);
        });
        let mut filters = LibraryFilters::default();
        filters.form = Some("app".to_owned());
        assert_eq!(page_ids(&lab.path, filters), vec!["app".to_owned()]);
        let mut filters = LibraryFilters::default();
        filters.form = Some("cli".to_owned());
        assert_eq!(page_ids(&lab.path, filters), vec!["cli".to_owned()]);
    }

    #[test]
    fn combined_filters_require_every_condition() {
        let lab = Lab::new("combine");
        lab.with(|conn| {
            scenario(conn, "scn-a", "a", "甲", 1, 0);
            let mut hit = Repo::new("hit");
            hit.description = Some("这里有专用词".to_owned());
            hit.reading = Some("want_to_try".to_owned());
            hit.card_json = Some(r#"{"form":"cli","review_status":"suggested"}"#.to_owned());
            hit.one_liner = Some("填充".to_owned());
            hit.save(conn);
            link(conn, "hit", "scn-a", "suggested");

            let mut wrong_form = Repo::new("wrong-form");
            wrong_form.description = Some("这里有专用词".to_owned());
            wrong_form.reading = Some("want_to_try".to_owned());
            wrong_form.card_json = Some(r#"{"form":"app"}"#.to_owned());
            wrong_form.one_liner = Some("填充".to_owned());
            wrong_form.save(conn);
            link(conn, "wrong-form", "scn-a", "suggested");

            let mut wrong_reading = Repo::new("wrong-reading");
            wrong_reading.description = Some("这里有专用词".to_owned());
            wrong_reading.reading = Some("unread".to_owned());
            wrong_reading.card_json = Some(r#"{"form":"cli"}"#.to_owned());
            wrong_reading.one_liner = Some("填充".to_owned());
            wrong_reading.save(conn);
            link(conn, "wrong-reading", "scn-a", "suggested");

            let mut wrong_keyword = Repo::new("wrong-keyword");
            wrong_keyword.reading = Some("watching".to_owned());
            wrong_keyword.card_json = Some(r#"{"form":"cli"}"#.to_owned());
            wrong_keyword.one_liner = Some("填充".to_owned());
            wrong_keyword.save(conn);
            link(conn, "wrong-keyword", "scn-a", "suggested");

            let mut wrong_scenario = Repo::new("wrong-scenario");
            wrong_scenario.description = Some("这里有专用词".to_owned());
            wrong_scenario.reading = Some("want_to_try".to_owned());
            wrong_scenario.card_json = Some(r#"{"form":"cli"}"#.to_owned());
            wrong_scenario.one_liner = Some("填充".to_owned());
            wrong_scenario.save(conn);
        });
        let mut filters = LibraryFilters::default();
        filters.scenario = Some("scn-a".to_owned());
        filters.reading = Some("want".to_owned());
        filters.form = Some("cli".to_owned());
        filters.keyword = Some("专用词".to_owned());
        assert_eq!(page_ids(&lab.path, filters), vec!["hit".to_owned()]);
    }

    #[test]
    fn sorts_keep_ties_stable_and_pages_cover_every_row_once() {
        let lab = Lab::new("sort");
        lab.with(|conn| {
            let specs = [
                (
                    "s1",
                    "2026-01-03T00:00:00Z",
                    10,
                    Some("2026-01-01T00:00:00Z"),
                ),
                ("s2", "2026-01-03T00:00:00Z", 50, None),
                (
                    "s3",
                    "2026-01-01T00:00:00Z",
                    50,
                    Some("2026-02-01T00:00:00Z"),
                ),
                (
                    "s4",
                    "2026-01-02T00:00:00Z",
                    5,
                    Some("2026-02-01T00:00:00Z"),
                ),
                ("s5", "2026-01-02T00:00:00Z", 5, None),
            ];
            for (id, starred, stars, pushed) in specs {
                let mut repo = Repo::new(id);
                repo.starred_at = starred.to_owned();
                repo.stars = stars;
                repo.pushed_at = pushed.map(str::to_owned);
                repo.save(conn);
            }
        });
        let starred = ["s1", "s2", "s4", "s5", "s3"];
        let stars = ["s2", "s3", "s1", "s4", "s5"];
        let updated = ["s3", "s4", "s1", "s2", "s5"];
        assert_eq!(
            page_ids(&lab.path, LibraryFilters::default()),
            owned(&starred)
        );
        let mut by_stars = LibraryFilters::default();
        by_stars.sort = Some("stars".to_owned());
        assert_eq!(page_ids(&lab.path, by_stars), owned(&stars));
        let mut by_updated = LibraryFilters::default();
        by_updated.sort = Some("updated".to_owned());
        assert_eq!(page_ids(&lab.path, by_updated), owned(&updated));

        let mut seen = Vec::new();
        for offset in [0, 2, 4] {
            let mut filters = LibraryFilters::default();
            filters.limit = Some(2);
            filters.offset = Some(offset);
            seen.extend(page_ids(&lab.path, filters));
        }
        assert_eq!(seen, owned(&starred));
    }

    #[test]
    fn limit_clamps_to_range_and_negative_offset_is_zero() {
        let lab = Lab::new("limit");
        lab.with(|conn| {
            for id in ["a", "b", "c"] {
                let mut repo = Repo::new(id);
                repo.starred_at = format!("2026-01-0{}T00:00:00Z", 4 - id.len());
                repo.save(conn);
            }
            // 固定收藏时间，避免上面的写法把顺序弄乱。
            for (id, starred) in [
                ("a", "2026-01-03"),
                ("b", "2026-01-02"),
                ("c", "2026-01-01"),
            ] {
                conn.execute(
                    "UPDATE repositories SET starred_at = ?2 WHERE id = ?1",
                    params![id, format!("{starred}T00:00:00Z")],
                )
                .unwrap();
            }
        });
        let mut zero = LibraryFilters::default();
        zero.limit = Some(0);
        assert_eq!(page_ids(&lab.path, zero), vec!["a".to_owned()]);
        let mut negative = LibraryFilters::default();
        negative.limit = Some(-3);
        assert_eq!(page_ids(&lab.path, negative), vec!["a".to_owned()]);
        let mut huge = LibraryFilters::default();
        huge.limit = Some(500);
        assert_eq!(
            page_ids(&lab.path, huge),
            vec!["a".to_owned(), "b".into(), "c".into()]
        );
        let mut back = LibraryFilters::default();
        back.offset = Some(-8);
        assert_eq!(
            page_ids(&lab.path, back),
            vec!["a".to_owned(), "b".into(), "c".into()]
        );
    }

    #[test]
    fn bad_or_mistyped_card_json_does_not_fail_list() {
        let lab = Lab::new("bad-json");
        lab.with(|conn| {
            let mut good = Repo::new("good");
            good.card_json = Some(
                r#"{"form":"app","confidence":0.5,"review_status":"confirmed"}"#.to_owned(),
            );
            good.one_liner = Some("好卡".to_owned());
            good.save(conn);
            let mut broken = Repo::new("broken");
            broken.card_json = Some("{".to_owned());
            broken.one_liner = Some("坏卡也有一句话".to_owned());
            broken.save(conn);
            let mut empty = Repo::new("empty-json");
            empty.card_json = Some(String::new());
            empty.one_liner = Some("空卡".to_owned());
            empty.save(conn);
            let mut mistyped = Repo::new("mistyped");
            mistyped.card_json = Some(
                r#"{"form":1,"confidence":"high","review_status":"suggested","capabilities":{"name_zh":"不是数组"}}"#
                    .to_owned(),
            );
            mistyped.one_liner = Some("类型不对".to_owned());
            mistyped.save(conn);
            let mut no_line = Repo::new("no-line");
            no_line.one_liner = Some(String::new());
            no_line.card_json = Some(
                r#"{"form":"cli","review_status":"suggested","confidence":0.25}"#.to_owned(),
            );
            no_line.save(conn);
        });
        let mut filters = LibraryFilters::default();
        filters.card_state = Some("all".to_owned());
        let page = list_library_view(&lab.path, &filters).expect("坏 JSON 不能让查询失败");
        let by = |id: &str| page.items.iter().find(|item| item.id == id).unwrap();
        let good = by("good");
        assert!(good.has_card);
        assert_eq!(good.form.as_deref(), Some("app"));
        assert_eq!(good.confidence, Some(0.5));
        assert_eq!(good.card_review.as_deref(), Some("confirmed"));
        let broken = by("broken");
        assert!(broken.has_card);
        assert!(broken.form.is_none());
        assert!(broken.confidence.is_none());
        assert!(broken.card_review.is_none());
        let empty = by("empty-json");
        assert!(empty.has_card);
        assert!(empty.form.is_none());
        assert!(empty.card_review.is_none());
        let mistyped = by("mistyped");
        assert!(mistyped.has_card);
        assert!(mistyped.form.is_none());
        assert!(mistyped.confidence.is_none());
        assert_eq!(mistyped.card_review.as_deref(), Some("suggested"));
        let no_line = by("no-line");
        assert!(!no_line.has_card);
        assert!(no_line.one_liner.is_none());
        assert!(no_line.card_review.is_none());
        assert_eq!(no_line.form.as_deref(), Some("cli"));
        assert_eq!(no_line.confidence, Some(0.25));

        let defaults = page_ids(&lab.path, LibraryFilters::default());
        assert!(!defaults.iter().any(|id| id == "no-line"));
        assert!(defaults.iter().any(|id| id == "broken"));
    }

    #[test]
    fn counts_do_not_shrink_the_dimension_being_counted() {
        let lab = Lab::new("counts");
        lab.with(|conn| {
            scenario(conn, "scn-a", "a", "甲", 1, 0);
            scenario(conn, "scn-b", "b", "乙", 2, 0);
            scenario(conn, "scn-c", "c", "丙", 3, 0);
            scenario(conn, "scn-old", "old", "旧", 4, 1);
            let mut r1 = Repo::new("r1");
            r1.card_json = Some(r#"{"form":"app"}"#.to_owned());
            r1.save(conn);
            link(conn, "r1", "scn-a", "suggested");
            let mut r2 = Repo::new("r2");
            r2.reading = Some("want_to_try".to_owned());
            r2.card_json = Some(r#"{"form":"cli"}"#.to_owned());
            r2.save(conn);
            link(conn, "r2", "scn-a", "suggested");
            let mut r3 = Repo::new("r3");
            r3.card_json = Some(r#"{"form":"app"}"#.to_owned());
            r3.save(conn);
            link(conn, "r3", "scn-b", "confirmed");
            let mut r4 = Repo::new("r4");
            r4.reading = Some("tried".to_owned());
            r4.card_json = Some(r#"{"form":"library"}"#.to_owned());
            r4.save(conn);
            link(conn, "r4", "scn-b", "suggested");
            let mut r5 = Repo::new("r5");
            r5.card_json = Some(r#"{"form":"app"}"#.to_owned());
            r5.save(conn);
            let mut r7 = Repo::new("r7");
            r7.write_doc = false;
            r7.one_liner = None;
            r7.save(conn);
            link(conn, "r7", "scn-a", "suggested");
            let mut removed = Repo::new("removed");
            removed.sync_status = "removed".to_owned();
            removed.save(conn);
            link(conn, "removed", "scn-a", "suggested");
        });

        let base = list_library_view(&lab.path, &LibraryFilters::default()).unwrap();
        assert_eq!(base.counts.total, 6);
        assert_eq!(base.total_count, 5);
        assert_eq!(base.counts.scenarios.get("scn-a"), Some(&2));
        assert_eq!(base.counts.scenarios.get("scn-b"), Some(&2));
        assert_eq!(base.counts.scenarios.get("scn-c"), Some(&0));
        assert!(!base.counts.scenarios.contains_key("scn-old"));
        assert_eq!(base.counts.reading.unseen, 3);
        assert_eq!(base.counts.reading.want, 1);
        assert_eq!(base.counts.reading.tried, 1);
        assert_eq!(base.counts.reading.in_use, 0);
        assert_eq!(base.counts.reading.dropped, 0);
        assert_eq!(base.counts.card_state.done, 5);
        assert_eq!(base.counts.card_state.pending, 1);
        assert_eq!(base.counts.forms.get("app"), Some(&3));
        assert_eq!(base.counts.forms.get("cli"), Some(&1));
        assert_eq!(base.counts.forms.get("library"), Some(&1));

        let mut by_scenario = LibraryFilters::default();
        by_scenario.scenario = Some("scn-a".to_owned());
        let page = list_library_view(&lab.path, &by_scenario).unwrap();
        assert_eq!(page.counts.scenarios, base.counts.scenarios);
        assert_eq!(page.counts.reading.unseen, 1);
        assert_eq!(page.counts.reading.want, 1);
        assert_eq!(page.counts.reading.tried, 0);
        assert_eq!(page.counts.total, 6);
        assert_eq!(page.total_count, 2);

        let mut by_reading = LibraryFilters::default();
        by_reading.reading = Some("unseen".to_owned());
        let page = list_library_view(&lab.path, &by_reading).unwrap();
        assert_eq!(page.counts.reading, base.counts.reading);
        assert_eq!(page.counts.scenarios.get("scn-a"), Some(&1));
        assert_eq!(page.counts.scenarios.get("scn-b"), Some(&1));
        assert_eq!(page.counts.scenarios.get("scn-c"), Some(&0));
        assert_eq!(page.total_count, 3);

        let mut by_form = LibraryFilters::default();
        by_form.form = Some("app".to_owned());
        let page = list_library_view(&lab.path, &by_form).unwrap();
        assert_eq!(page.counts.forms, base.counts.forms);
        assert_eq!(page.total_count, 3);

        let mut by_pending = LibraryFilters::default();
        by_pending.card_state = Some("pending".to_owned());
        let page = list_library_view(&lab.path, &by_pending).unwrap();
        assert_eq!(page.counts.card_state.done, 5);
        assert_eq!(page.counts.card_state.pending, 1);
        assert_eq!(page.total_count, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "r7");
    }

    #[test]
    fn total_ignores_filters_and_unused_scenario_is_zero() {
        let lab = Lab::new("total");
        lab.with(|conn| {
            scenario(conn, "scn-used", "used", "用过", 1, 0);
            scenario(conn, "scn-empty", "empty", "空", 2, 0);
            scenario(conn, "scn-old", "old", "旧", 3, 1);
            Repo::new("one").save(conn);
            link(conn, "one", "scn-used", "suggested");
            Repo::new("two").save(conn);
            let mut removed = Repo::new("gone");
            removed.sync_status = "removed".to_owned();
            removed.save(conn);
        });
        let mut filters = LibraryFilters::default();
        filters.keyword = Some("绝对不会出现的词".to_owned());
        filters.scenario = Some("scn-used".to_owned());
        let page = list_library_view(&lab.path, &filters).unwrap();
        assert_eq!(page.total_count, 0);
        assert!(page.items.is_empty());
        assert_eq!(page.counts.total, 2);
        // 关键词仍作用在场景计数上；选中的场景不会让这一列只剩自己。
        assert_eq!(page.counts.scenarios.get("scn-empty"), Some(&0));
        assert_eq!(page.counts.scenarios.get("scn-used"), Some(&0));
        assert!(!page.counts.scenarios.contains_key("scn-old"));

        let mut by_scenario = LibraryFilters::default();
        by_scenario.scenario = Some("scn-used".to_owned());
        let page = list_library_view(&lab.path, &by_scenario).unwrap();
        assert_eq!(page.total_count, 1);
        assert_eq!(page.counts.total, 2);
        assert_eq!(page.counts.scenarios.get("scn-used"), Some(&1));
        assert_eq!(page.counts.scenarios.get("scn-empty"), Some(&0));
        assert!(!page.counts.scenarios.contains_key("scn-old"));
    }

    #[test]
    fn scenario_ids_are_ordered_and_rejected_is_omitted() {
        let lab = Lab::new("scenario-ids");
        lab.with(|conn| {
            scenario(conn, "scn-b", "b", "乙", 1, 0);
            scenario(conn, "scn-a", "a", "甲", 1, 0);
            scenario(conn, "scn-late", "late", "晚", 3, 0);
            scenario(conn, "scn-no", "no", "拒", 0, 0);
            Repo::new("repo").save(conn);
            link(conn, "repo", "scn-late", "suggested");
            link(conn, "repo", "scn-b", "confirmed");
            link(conn, "repo", "scn-a", "suggested");
            link(conn, "repo", "scn-no", "rejected");
        });
        let page = list_library_view(&lab.path, &LibraryFilters::default()).unwrap();
        assert_eq!(
            page.items[0].scenario_ids,
            vec![
                "scn-a".to_owned(),
                "scn-b".to_owned(),
                "scn-late".to_owned()
            ]
        );
    }

    #[test]
    fn detail_reads_fields_and_falls_back_on_wrong_types() {
        let lab = Lab::new("detail");
        lab.with(|conn| {
            scenario(conn, "scn-a", "a", "甲", 2, 0);
            scenario(conn, "scn-b", "b", "乙", 1, 0);
            let mut good = Repo::new("good");
            good.forks = 7;
            good.language = Some("Rust".to_owned());
            good.description = Some("描述".to_owned());
            good.keywords_json = r#"["封面", 1, ""]"#.to_owned();
            good.topics_json = "not-json".to_owned();
            good.one_liner = Some("  一句话  ".to_owned());
            good.summary = "摘要不应当作结论".to_owned();
            good.card_json = Some(
                r#"{
                    "what_it_is":"这是结论",
                    "form":"cli",
                    "capabilities":[{"name_zh":"生成封面"},{"name_zh":""}, "直接写的能力"],
                    "problems_solved":["配图慢", 1],
                    "use_when":["要做海报"],
                    "not_for":["写后端"],
                    "how_to_try":"直接运行",
                    "integrates_with":["Codex"],
                    "alternatives_hint":["other/repo"],
                    "inspiration_for_fox":"可以借鉴排版",
                    "risks":["授权不明"],
                    "license":"MIT",
                    "maturity":"usable",
                    "zh_friendly":true,
                    "confidence":0.5,
                    "review_status":"confirmed"
                }"#
                .to_owned(),
            );
            good.save(conn);
            link(conn, "good", "scn-a", "suggested");
            link(conn, "good", "scn-b", "suggested");
            readme(conn, "good", "短 README");

            let mut bad = Repo::new("bad");
            bad.one_liner = Some("坏类型也有卡".to_owned());
            bad.card_json = Some(
                r#"{
                    "how_to_try":["先安装","再运行", 3],
                    "capabilities":{"name_zh":"不是数组"},
                    "problems_solved":"不是数组",
                    "use_when":{"0":"对象"},
                    "zh_friendly":"yes",
                    "confidence":"high",
                    "form":["cli"],
                    "license":null,
                    "what_it_is":""
                }"#
                .to_owned(),
            );
            bad.keywords_json = "not-json".to_owned();
            bad.topics_json = r#"["Rust","CLI"]"#.to_owned();
            bad.save(conn);
        });
        let good = get_library_detail(&lab.path, "good").unwrap();
        assert_eq!(good.item.one_liner.as_deref(), Some("一句话"));
        assert!(good.item.has_card);
        assert_eq!(good.item.form.as_deref(), Some("cli"));
        assert_eq!(good.item.confidence, Some(0.5));
        assert_eq!(good.item.card_review.as_deref(), Some("confirmed"));
        assert_eq!(
            good.item.scenario_ids,
            vec!["scn-b".to_owned(), "scn-a".to_owned()]
        );
        assert_eq!(good.what_it_is.as_deref(), Some("这是结论"));
        assert_ne!(good.what_it_is.as_deref(), Some("摘要不应当作结论"));
        assert_eq!(
            good.capabilities,
            vec!["生成封面".to_owned(), "直接写的能力".to_owned()]
        );
        assert_eq!(good.problems_solved, vec!["配图慢".to_owned()]);
        assert_eq!(good.use_when, vec!["要做海报".to_owned()]);
        assert_eq!(good.not_for, vec!["写后端".to_owned()]);
        assert_eq!(good.how_to_try.as_deref(), Some("直接运行"));
        assert_eq!(good.integrates_with, vec!["Codex".to_owned()]);
        assert_eq!(good.alternatives_hint, vec!["other/repo".to_owned()]);
        assert_eq!(good.inspiration_for_fox.as_deref(), Some("可以借鉴排版"));
        assert_eq!(good.risks, vec!["授权不明".to_owned()]);
        assert_eq!(good.license.as_deref(), Some("MIT"));
        assert_eq!(good.maturity.as_deref(), Some("usable"));
        assert_eq!(good.zh_friendly, Some(true));
        assert_eq!(good.keywords, vec!["封面".to_owned()]);
        assert!(good.topics.is_empty());
        assert_eq!(good.forks_count, 7);
        assert_eq!(good.readme_excerpt.as_deref(), Some("短 README"));
        assert_eq!(good.item.language.as_deref(), Some("Rust"));
        assert_eq!(good.item.description.as_deref(), Some("描述"));

        let bad = get_library_detail(&lab.path, "bad").unwrap();
        assert_eq!(bad.how_to_try.as_deref(), Some("先安装\n再运行"));
        assert!(bad.capabilities.is_empty());
        assert!(bad.problems_solved.is_empty());
        assert!(bad.use_when.is_empty());
        assert!(bad.what_it_is.is_none());
        // 卡里的 form 是数组，仍然不算；话题 CLI 会按推断规则补成 cli。
        assert_eq!(bad.item.form.as_deref(), Some("cli"));
        assert!(bad.item.confidence.is_none());
        assert!(bad.zh_friendly.is_none());
        assert!(bad.license.is_none());
        assert!(bad.keywords.is_empty());
        assert_eq!(bad.topics, vec!["Rust".to_owned(), "CLI".to_owned()]);
        assert!(bad.readme_excerpt.is_none());
        assert!(bad.item.has_card);
    }

    #[test]
    fn detail_readme_excerpt_truncates_by_character() {
        let lab = Lab::new("readme");
        let body = "测".repeat(1800);
        let long = format!("<!-- 注释{body} -->\n![封面](https://img.example/a.png)\n{body}");
        let short = "<!-- secret -->\n![纯图](https://img.example/a.png)\n<p><img src=\"b.png\"></p>\n开头<!-- secret -->结尾\n见图 ![a](b)\n";
        lab.with(|conn| {
            Repo::new("long").save(conn);
            readme(conn, "long", &long);
            Repo::new("short").save(conn);
            readme(conn, "short", short);
        });
        let long = get_library_detail(&lab.path, "long").unwrap();
        let excerpt = long.readme_excerpt.expect("应有摘录");
        assert_eq!(excerpt.chars().count(), 1500);
        assert!(excerpt.chars().all(|ch| ch == '测'));
        assert!(!excerpt.contains("注释"));
        assert!(!excerpt.contains("img.example"));
        let short = get_library_detail(&lab.path, "short").unwrap();
        assert_eq!(
            short.readme_excerpt.as_deref(),
            Some("开头结尾\n见图 ![a](b)")
        );
    }

    #[test]
    fn detail_missing_id_returns_not_found() {
        let lab = Lab::new("missing-detail");
        let error = get_library_detail(&lab.path, "nope").expect_err("不存在的仓库应失败");
        assert_eq!(error, "找不到这个仓库");
    }

    fn set_license(conn: &Connection, id: &str, spdx: Option<&str>, name: Option<&str>) {
        conn.execute(
            "UPDATE repositories SET license_spdx = ?1, license_name = ?2 WHERE id = ?3",
            params![spdx, name, id],
        )
        .expect("应能写入许可证");
    }

    #[test]
    fn detail_license_prefers_github_then_card_then_unknown() {
        let lab = Lab::new("license-priority");
        lab.with(|conn| {
            let mut github = Repo::new("github");
            github.card_json = Some(r#"{"license":"Apache-2.0","form":"library"}"#.to_owned());
            github.save(conn);
            set_license(conn, "github", Some("MIT"), Some("MIT License"));

            let mut unrecognized = Repo::new("unrecognized");
            unrecognized.card_json = Some(r#"{"license":"MIT","form":"library"}"#.to_owned());
            unrecognized.save(conn);
            set_license(
                conn,
                "unrecognized",
                Some("NOASSERTION"),
                Some("自定义许可证"),
            );

            let mut other = Repo::new("other");
            other.card_json = Some(r#"{"license":"MIT","form":"library"}"#.to_owned());
            other.save(conn);
            set_license(conn, "other", Some("Other"), None);

            let mut from_card = Repo::new("card");
            from_card.card_json = Some(r#"{"license":"Apache-2.0","form":"library"}"#.to_owned());
            from_card.save(conn);

            let mut unknown = Repo::new("unknown");
            unknown.card_json = Some(r#"{"license":null,"form":"library"}"#.to_owned());
            unknown.save(conn);

            let mut bare = Repo::new("bare");
            bare.write_doc = false;
            bare.save(conn);
        });

        assert_eq!(
            get_library_detail(&lab.path, "github")
                .unwrap()
                .license
                .as_deref(),
            Some("MIT")
        );
        assert_eq!(
            get_library_detail(&lab.path, "unrecognized")
                .unwrap()
                .license
                .as_deref(),
            Some("自定义许可证")
        );
        assert_eq!(
            get_library_detail(&lab.path, "other")
                .unwrap()
                .license
                .as_deref(),
            Some("其他（见仓库）")
        );
        assert_eq!(
            get_library_detail(&lab.path, "card")
                .unwrap()
                .license
                .as_deref(),
            Some("Apache-2.0")
        );
        assert_eq!(
            get_library_detail(&lab.path, "unknown").unwrap().license,
            None
        );
        assert_eq!(get_library_detail(&lab.path, "bare").unwrap().license, None);
    }

    #[test]
    fn detail_license_blank_spdx_is_undeclared() {
        let lab = Lab::new("license-blank");
        lab.with(|conn| {
            let mut blank = Repo::new("blank");
            blank.card_json = Some(r#"{"license":"MIT","form":"library"}"#.to_owned());
            blank.save(conn);
            set_license(conn, "blank", Some("   "), Some("MIT License"));

            let mut empty = Repo::new("empty");
            empty.card_json = Some(r#"{"license":"Apache-2.0","form":"library"}"#.to_owned());
            empty.save(conn);
            set_license(conn, "empty", Some(""), None);
        });

        assert_eq!(
            get_library_detail(&lab.path, "blank")
                .unwrap()
                .license
                .as_deref(),
            Some("未声明")
        );
        assert_eq!(
            get_library_detail(&lab.path, "empty")
                .unwrap()
                .license
                .as_deref(),
            Some("未声明")
        );
    }

    #[test]
    fn detail_form_infers_topics_without_changing_list() {
        let lab = Lab::new("form-infer");
        lab.with(|conn| {
            let mut bare = Repo::new("bare");
            bare.write_doc = false;
            bare.one_liner = None;
            bare.full_name = "someone/plain".to_owned();
            bare.topics_json = r#"["MCP-Server", ""]"#.to_owned();
            bare.description = Some("这是一个 cli 营销简介".to_owned());
            bare.save(conn);

            let mut named = Repo::new("named");
            named.write_doc = false;
            named.one_liner = None;
            named.full_name = "mcp-fan/widget-mcp".to_owned();
            named.topics_json = "[]".to_owned();
            named.description = Some("electron desktop".to_owned());
            named.save(conn);

            let mut card = Repo::new("card");
            card.full_name = "someone/tool".to_owned();
            card.topics_json = r#"["mcp","cli"]"#.to_owned();
            card.card_json = Some(r#"{"form":"app"}"#.to_owned());
            card.save(conn);

            let mut plain = Repo::new("plain");
            plain.write_doc = false;
            plain.one_liner = None;
            plain.full_name = "mcp-fan/notes".to_owned();
            plain.topics_json = "[]".to_owned();
            plain.description = Some("mcp skill cli electron".to_owned());
            plain.save(conn);
        });

        let mut filters = LibraryFilters::default();
        filters.card_state = Some("all".to_owned());
        filters.limit = Some(20);
        let page = list_library_view(&lab.path, &filters).unwrap();
        let form_of = |id: &str| {
            page.items
                .iter()
                .find(|item| item.id == id)
                .and_then(|item| item.form.clone())
        };
        assert_eq!(form_of("bare"), None);
        assert_eq!(form_of("named"), None);
        assert_eq!(form_of("card").as_deref(), Some("app"));
        assert_eq!(form_of("plain"), None);
        assert!(page.counts.forms.get("mcp").is_none());
        assert_eq!(page.counts.forms.get("app"), Some(&1));

        assert_eq!(
            get_library_detail(&lab.path, "bare")
                .unwrap()
                .item
                .form
                .as_deref(),
            Some("mcp")
        );
        assert_eq!(
            get_library_detail(&lab.path, "named")
                .unwrap()
                .item
                .form
                .as_deref(),
            Some("mcp")
        );
        assert_eq!(
            get_library_detail(&lab.path, "card")
                .unwrap()
                .item
                .form
                .as_deref(),
            Some("app")
        );
        assert_eq!(
            get_library_detail(&lab.path, "plain").unwrap().item.form,
            None
        );
    }

    #[test]
    fn set_reading_writes_mapped_status_without_touching_note_or_rating() {
        let lab = Lab::new("set-reading");
        lab.with(|conn| {
            let mut repo = Repo::new("repo");
            repo.reading = Some("later".to_owned());
            repo.note = "别动这条笔记".to_owned();
            repo.rating = Some(4);
            repo.save(conn);
            let fresh = Repo::new("fresh");
            fresh.save(conn);
        });
        let cases = [
            ("unseen", "unread"),
            ("want", "want_to_try"),
            ("tried", "tried"),
            ("in_use", "in_use"),
            ("dropped", "deprecated"),
        ];
        for (group, stored) in cases {
            let changed = set_library_reading(&lab.path, &[("repo".to_owned())], group).unwrap();
            assert_eq!(changed, 1, "{group}");
            let (status, note, rating): (String, String, Option<i64>) = lab.with(|conn| {
                conn.query_row(
                    "SELECT read_status, note_md, rating FROM annotations WHERE repo_id = 'repo'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap()
            });
            assert_eq!(status, stored, "{group}");
            assert_eq!(note, "别动这条笔记");
            assert_eq!(rating, Some(4));
        }
        let changed = set_library_reading(&lab.path, &["fresh".to_owned()], "want").unwrap();
        assert_eq!(changed, 1);
        let (status, note, rating): (String, String, Option<i64>) = lab.with(|conn| {
            conn.query_row(
                "SELECT read_status, note_md, rating FROM annotations WHERE repo_id = 'fresh'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
        });
        assert_eq!(status, "want_to_try");
        assert_eq!(note, "");
        assert!(rating.is_none());
        let error = set_library_reading(&lab.path, &["repo".to_owned()], "nope").unwrap_err();
        assert!(error.contains("无效"), "{error}");
    }

    #[test]
    fn set_reading_ignores_unknown_and_removed_and_caps_at_200() {
        let lab = Lab::new("set-cap");
        lab.with(|conn| {
            let mut active = Repo::new("active");
            active.reading = Some("unread".to_owned());
            active.note = "保留".to_owned();
            active.rating = Some(2);
            active.save(conn);
            let mut removed = Repo::new("removed");
            removed.sync_status = "removed".to_owned();
            removed.reading = Some("unread".to_owned());
            removed.save(conn);
        });
        let changed = set_library_reading(
            &lab.path,
            &[
                "active".to_owned(),
                "removed".to_owned(),
                "missing".to_owned(),
                "active".to_owned(),
                String::new(),
            ],
            "in_use",
        )
        .unwrap();
        assert_eq!(changed, 1);
        let status: String = lab.with(|conn| {
            conn.query_row(
                "SELECT read_status FROM annotations WHERE repo_id = 'active'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        });
        assert_eq!(status, "in_use");
        let removed_status: String = lab.with(|conn| {
            conn.query_row(
                "SELECT read_status FROM annotations WHERE repo_id = 'removed'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        });
        assert_eq!(removed_status, "unread");

        let mut too_many: Vec<String> = (0..200).map(|index| format!("missing-{index}")).collect();
        too_many.push("active".to_owned());
        let error = set_library_reading(&lab.path, &too_many, "dropped").unwrap_err();
        assert!(error.contains("200"), "{error}");
        let status: String = lab.with(|conn| {
            conn.query_row(
                "SELECT read_status FROM annotations WHERE repo_id = 'active'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        });
        assert_eq!(status, "in_use");
        let note: String = lab.with(|conn| {
            conn.query_row(
                "SELECT note_md FROM annotations WHERE repo_id = 'active'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        });
        assert_eq!(note, "保留");
    }

    #[test]
    fn today_picks_prefer_distinct_core_directions() {
        let lab = Lab::new("picks");
        lab.with(|conn| {
            scenario(conn, "scn-1", "d1", "小红书", 1, 0);
            scenario(conn, "scn-2", "d2", "公众号", 2, 0);
            scenario(conn, "scn-7", "d7", "写作", 7, 0);
            scenario(conn, "scn-8", "d8", "调研", 8, 0);
            let mut best = Repo::new("p1");
            best.card_json =
                Some(r#"{"form":"app","confidence":0.9,"review_status":"suggested"}"#.to_owned());
            best.starred_at = "2026-10-01T00:00:00Z".to_owned();
            best.save(conn);
            link(conn, "p1", "scn-1", "suggested");
            let mut same_dir = Repo::new("p1b");
            same_dir.card_json =
                Some(r#"{"form":"app","confidence":0.2,"review_status":"suggested"}"#.to_owned());
            same_dir.starred_at = "2026-10-09T00:00:00Z".to_owned();
            same_dir.save(conn);
            link(conn, "p1b", "scn-1", "suggested");
            let mut second = Repo::new("p2");
            second.reading = Some("later".to_owned());
            second.card_json =
                Some(r#"{"form":"cli","confidence":0.4,"review_status":"suggested"}"#.to_owned());
            second.starred_at = "2026-10-02T00:00:00Z".to_owned();
            second.save(conn);
            link(conn, "p2", "scn-2", "confirmed");
            let mut seventh = Repo::new("p7");
            seventh.card_json =
                Some(r#"{"form":"app","confidence":0.3,"review_status":"suggested"}"#.to_owned());
            seventh.starred_at = "2026-10-03T00:00:00Z".to_owned();
            seventh.save(conn);
            link(conn, "p7", "scn-7", "suggested");
            let mut eighth = Repo::new("p8");
            eighth.card_json =
                Some(r#"{"form":"app","confidence":0.99,"review_status":"suggested"}"#.to_owned());
            eighth.starred_at = "2026-10-09T00:00:00Z".to_owned();
            eighth.save(conn);
            link(conn, "p8", "scn-8", "suggested");
            let mut wanted = Repo::new("pwant");
            wanted.reading = Some("want_to_try".to_owned());
            wanted.card_json =
                Some(r#"{"form":"app","confidence":0.99,"review_status":"suggested"}"#.to_owned());
            wanted.save(conn);
            link(conn, "pwant", "scn-1", "suggested");
            let mut no_card = Repo::new("pnocard");
            no_card.write_doc = false;
            no_card.one_liner = None;
            no_card.save(conn);
            link(conn, "pnocard", "scn-2", "suggested");
        });
        let view = get_today_view(&lab.path, "2026-10-10T00:00:00Z").unwrap();
        let ids: Vec<_> = view.picks.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, vec!["p1", "p2", "p7"]);
    }

    #[test]
    fn today_picks_relax_when_directions_are_not_enough() {
        let lab = Lab::new("picks-relax");
        lab.with(|conn| {
            scenario(conn, "scn-1", "d1", "小红书", 1, 0);
            scenario(conn, "scn-2", "d2", "公众号", 2, 0);
            let mut newer = Repo::new("a-new");
            newer.card_json = Some(r#"{"confidence":0.5,"review_status":"suggested"}"#.to_owned());
            newer.starred_at = "2026-10-08T00:00:00Z".to_owned();
            newer.save(conn);
            link(conn, "a-new", "scn-1", "suggested");
            let mut older = Repo::new("a-old");
            older.card_json = Some(r#"{"confidence":0.5,"review_status":"suggested"}"#.to_owned());
            older.starred_at = "2026-10-01T00:00:00Z".to_owned();
            older.save(conn);
            link(conn, "a-old", "scn-1", "suggested");
            let mut second = Repo::new("b1");
            second.card_json = Some(r#"{"confidence":0.1,"review_status":"suggested"}"#.to_owned());
            second.starred_at = "2026-10-02T00:00:00Z".to_owned();
            second.save(conn);
            link(conn, "b1", "scn-2", "suggested");
        });
        let view = get_today_view(&lab.path, "2026-10-10T00:00:00Z").unwrap();
        let ids: Vec<_> = view.picks.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, vec!["a-new", "a-old", "b1"]);
    }

    #[test]
    fn today_week_new_uses_seven_day_boundary() {
        let lab = Lab::new("week");
        let now = "2026-10-03T12:00:00Z";
        lab.with(|conn| {
            let specs = [
                ("w0a", "2026-10-03T12:00:00Z", true, None),
                ("w0b", "2026-10-03T12:00:00Z", true, None),
                ("w01", "2026-10-03T00:00:00Z", true, None),
                ("w02", "2026-10-02T00:00:00Z", false, Some("tried")),
                ("w03", "2026-10-01T00:00:00Z", true, None),
                ("w04", "2026-09-30T00:00:00Z", true, None),
                ("w05", "2026-09-29T00:00:00Z", true, None),
                ("w06", "2026-09-28T00:00:00Z", true, None),
                ("w07", "2026-09-27T00:00:00Z", true, None),
                ("w08", "2026-09-26T12:00:00Z", true, None),
                ("w-out", "2026-09-26T11:59:59Z", true, None),
            ];
            for (id, starred, has_card, reading) in specs {
                let mut repo = Repo::new(id);
                repo.starred_at = starred.to_owned();
                repo.reading = reading.map(str::to_owned);
                if !has_card {
                    repo.write_doc = false;
                    repo.one_liner = None;
                }
                repo.save(conn);
            }
        });
        let view = get_today_view(&lab.path, now).unwrap();
        let ids: Vec<_> = view.week_new.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["w0a", "w0b", "w01", "w02", "w03", "w04", "w05", "w06", "w07", "w08"]
        );
        assert_eq!(view.counts.week_new, 10);
        assert!(view
            .week_new
            .iter()
            .any(|item| item.id == "w02" && !item.has_card));
        // 正好七天前的算在内，差一秒的不算
        assert!(ids.contains(&"w08"));
        assert!(!ids.contains(&"w-out"));
    }

    #[test]
    fn today_week_new_list_matches_the_count_until_the_cap() {
        let lab = Lab::new("week-cap");
        let now = "2026-10-03T12:00:00Z";
        lab.with(|conn| {
            for index in 0..(WEEK_LIMIT + 5) {
                let mut repo = Repo::new(&format!("cap{index:03}"));
                repo.starred_at =
                    format!("2026-10-03T{:02}:{:02}:00Z", (index / 60) % 12, index % 60);
                repo.save(conn);
            }
        });
        let view = get_today_view(&lab.path, now).unwrap();
        // 统计卡是全部，列表只给到上限
        assert_eq!(view.counts.week_new, WEEK_LIMIT + 5);
        assert_eq!(view.week_new.len() as i64, WEEK_LIMIT);

        let small = Lab::new("week-small");
        small.with(|conn| {
            for index in 0..19 {
                let mut repo = Repo::new(&format!("s{index:02}"));
                repo.starred_at = format!("2026-10-02T{:02}:00:00Z", index);
                repo.save(conn);
            }
        });
        let view = get_today_view(&small.path, now).unwrap();
        // 十几个的时候，点进去的列表和统计卡上的数字一致
        assert_eq!(view.counts.week_new, 19);
        assert_eq!(view.week_new.len(), 19);
    }

    #[test]
    fn today_counts_and_synced_at_follow_the_rules() {
        let lab = Lab::new("today-counts");
        let now = "2026-10-03T00:00:00Z";
        let empty = get_today_view(&lab.path, now).unwrap();
        assert!(empty.picks.is_empty());
        assert!(empty.week_new.is_empty());
        assert_eq!(empty.counts.total, 0);
        assert_eq!(empty.counts.unseen, 0);
        assert_eq!(empty.counts.week_new, 0);
        assert_eq!(empty.counts.cards_pending, 0);
        assert_eq!(empty.counts.cards_suggested, 0);
        assert!(empty.synced_at.is_none());

        lab.with(|conn| {
            let mut plain = Repo::new("plain");
            plain.write_doc = false;
            plain.one_liner = None;
            plain.reading = Some("later".to_owned());
            plain.starred_at = "2026-09-01T00:00:00Z".to_owned();
            plain.updated_at = "2026-10-01T00:00:00Z".to_owned();
            plain.save(conn);
            let mut suggested = Repo::new("suggested");
            suggested.card_json = Some(r#"{"review_status":"suggested","form":"app"}"#.to_owned());
            suggested.starred_at = "2026-10-01T00:00:00Z".to_owned();
            suggested.updated_at = "2026-10-02T00:00:00Z".to_owned();
            suggested.save(conn);
            let mut confirmed = Repo::new("confirmed");
            confirmed.card_json = Some(r#"{"review_status":"confirmed","form":"app"}"#.to_owned());
            confirmed.reading = Some("read".to_owned());
            confirmed.starred_at = "2026-08-01T00:00:00Z".to_owned();
            confirmed.updated_at = "2026-10-03T00:00:00Z".to_owned();
            confirmed.save(conn);
            let mut bad = Repo::new("bad");
            bad.card_json = Some("{".to_owned());
            bad.starred_at = "2026-08-01T00:00:00Z".to_owned();
            bad.updated_at = "2026-10-04T00:00:00Z".to_owned();
            bad.save(conn);
            let mut no_review = Repo::new("noreview");
            no_review.card_json = Some(r#"{"form":"app"}"#.to_owned());
            no_review.starred_at = "2026-08-02T00:00:00Z".to_owned();
            no_review.updated_at = "2026-10-05T00:00:00Z".to_owned();
            no_review.save(conn);
            let mut blank = Repo::new("blank");
            blank.one_liner = Some("   ".to_owned());
            blank.starred_at = "2026-08-03T00:00:00Z".to_owned();
            blank.updated_at = "2026-10-06T00:00:00Z".to_owned();
            blank.save(conn);
            let mut removed = Repo::new("removed");
            removed.sync_status = "removed".to_owned();
            removed.starred_at = "2026-10-03T00:00:00Z".to_owned();
            removed.updated_at = "2026-10-09T00:00:00Z".to_owned();
            removed.save(conn);
        });
        let view = get_today_view(&lab.path, now).unwrap();
        assert_eq!(view.counts.total, 6);
        assert_eq!(view.counts.unseen, 5);
        assert_eq!(view.counts.week_new, 1);
        assert_eq!(view.counts.cards_pending, 2);
        assert_eq!(view.counts.cards_suggested, 3);
        assert_eq!(view.synced_at.as_deref(), Some("2026-10-09T00:00:00Z"));
        assert!(view.week_new.iter().any(|item| item.id == "suggested"));
    }

    fn highlight_of_id(path: &Path, id: &str) -> Option<String> {
        let mut filters = LibraryFilters::default();
        filters.card_state = Some("all".to_owned());
        filters.limit = Some(20);
        list_library_view(path, &filters)
            .expect("资料库查询应成功")
            .items
            .into_iter()
            .find(|item| item.id == id)
            .unwrap_or_else(|| panic!("应能找到 {id}"))
            .highlight
    }

    #[test]
    fn highlight_prefers_first_problem() {
        let lab = Lab::new("highlight-problem");
        lab.with(|conn| {
            let mut repo = Repo::new("repo");
            repo.one_liner = Some("配图慢".to_owned());
            repo.card_json = Some(
                r#"{"problems_solved":["  配图慢  ","另一条"],"capabilities":[{"name_zh":"生成封面"}]}"#
                    .to_owned(),
            );
            repo.save(conn);
        });
        assert_eq!(
            highlight_of_id(&lab.path, "repo").as_deref(),
            Some("配图慢")
        );
        let detail = get_library_detail(&lab.path, "repo").unwrap();
        assert_eq!(detail.item.highlight.as_deref(), Some("配图慢"));
        assert_eq!(detail.item.one_liner.as_deref(), Some("配图慢"));
    }

    #[test]
    fn highlight_uses_capability_string_when_problems_are_missing() {
        let lab = Lab::new("highlight-cap-str");
        lab.with(|conn| {
            let mut repo = Repo::new("repo");
            repo.card_json = Some(r#"{"capabilities":["  生成封面  ","第二能力"]}"#.to_owned());
            repo.save(conn);
        });
        assert_eq!(
            highlight_of_id(&lab.path, "repo").as_deref(),
            Some("生成封面")
        );
    }

    #[test]
    fn highlight_uses_capability_name_when_problems_are_missing() {
        let lab = Lab::new("highlight-cap-obj");
        lab.with(|conn| {
            let mut repo = Repo::new("repo");
            repo.card_json = Some(
                r#"{"problems_solved":[],"capabilities":[{"name_zh":""},{"name_zh":"  做字幕  "},"后面的字符串"]}"#
                    .to_owned(),
            );
            repo.save(conn);
        });
        assert_eq!(
            highlight_of_id(&lab.path, "repo").as_deref(),
            Some("做字幕")
        );
    }

    #[test]
    fn highlight_skips_blank_or_non_string_problems() {
        let lab = Lab::new("highlight-skip");
        lab.with(|conn| {
            let mut repo = Repo::new("repo");
            repo.card_json = Some(
                r#"{"problems_solved":["",1,"   ","配图慢"],"capabilities":[{"name_zh":"不该用这个"}]}"#
                    .to_owned(),
            );
            repo.save(conn);
        });
        assert_eq!(
            highlight_of_id(&lab.path, "repo").as_deref(),
            Some("配图慢")
        );
    }

    #[test]
    fn highlight_is_null_without_a_usable_card() {
        let lab = Lab::new("highlight-null");
        lab.with(|conn| {
            let mut missing = Repo::new("missing");
            missing.write_doc = false;
            missing.one_liner = None;
            missing.save(conn);
            let mut broken = Repo::new("broken");
            broken.card_json = Some("{".to_owned());
            broken.save(conn);
            let mut mistyped = Repo::new("mistyped");
            mistyped.card_json = Some(
                r#"{"problems_solved":"不是数组","capabilities":{"name_zh":"对象"}}"#.to_owned(),
            );
            mistyped.save(conn);
            let mut empty = Repo::new("empty");
            empty.card_json = Some(
                r#"{"problems_solved":["",1],"capabilities":[{"name_zh":"  "},""]}"#.to_owned(),
            );
            empty.save(conn);
        });
        for id in ["missing", "broken", "mistyped", "empty"] {
            assert_eq!(highlight_of_id(&lab.path, id), None, "{id}");
        }
    }

    #[test]
    fn contract_json_keys_and_types_match_typescript() {
        let item = sample_item(None);
        let item_json = serde_json::to_value(&item).unwrap();
        assert_eq!(
            object_keys(&item_json),
            set(&[
                "id",
                "fullName",
                "htmlUrl",
                "description",
                "language",
                "starsCount",
                "starredAt",
                "pushedAt",
                "oneLiner",
                "highlight",
                "form",
                "scenarioIds",
                "readingStatus",
                "readingGroup",
                "hasCard",
                "confidence",
                "cardReview",
            ])
        );
        for key in [
            "description",
            "language",
            "pushedAt",
            "oneLiner",
            "highlight",
            "form",
            "confidence",
            "cardReview",
        ] {
            assert!(item_json[key].is_null(), "{key}");
        }
        assert!(item_json["scenarioIds"].is_array());
        assert!(item_json["hasCard"].is_boolean());
        assert!(item_json["starsCount"].is_number());
        assert!(item_json["id"].is_string());
        assert_eq!(item_json["readingGroup"], "in_use");
        assert!(!item_json.to_string().contains("inUse"));

        let counts = LibraryCounts {
            total: 1,
            scenarios: BTreeMap::from([("scn-a".to_owned(), 0)]),
            reading: ReadingCounts {
                unseen: 1,
                want: 0,
                tried: 0,
                in_use: 0,
                dropped: 0,
            },
            card_state: CardStateCounts {
                done: 1,
                pending: 0,
            },
            forms: BTreeMap::from([("cli".to_owned(), 1)]),
        };
        let counts_json = serde_json::to_value(&counts).unwrap();
        assert_eq!(
            object_keys(&counts_json),
            set(&["total", "scenarios", "reading", "cardState", "forms"])
        );
        assert!(counts_json["total"].is_number());
        assert!(counts_json["scenarios"].is_object());
        assert!(counts_json["forms"].is_object());
        assert_eq!(
            object_keys(&counts_json["reading"]),
            set(&["unseen", "want", "tried", "in_use", "dropped"])
        );
        assert!(counts_json["reading"]["in_use"].is_number());
        assert_eq!(
            object_keys(&counts_json["cardState"]),
            set(&["done", "pending"])
        );

        let page = LibraryPage {
            items: vec![item.clone()],
            total_count: 1,
            counts: counts.clone(),
        };
        let page_json = serde_json::to_value(&page).unwrap();
        assert_eq!(
            object_keys(&page_json),
            set(&["items", "totalCount", "counts"])
        );
        assert!(page_json["items"].is_array());
        assert!(page_json["totalCount"].is_number());
        assert_eq!(object_keys(&page_json["items"][0]), object_keys(&item_json));

        let detail = LibraryDetail {
            item,
            what_it_is: None,
            capabilities: vec!["生成封面".to_owned()],
            problems_solved: vec!["配图慢".to_owned()],
            use_when: vec!["要做海报".to_owned()],
            not_for: vec!["写后端".to_owned()],
            how_to_try: None,
            integrates_with: vec!["Codex".to_owned()],
            alternatives_hint: vec!["other/repo".to_owned()],
            inspiration_for_fox: None,
            risks: vec!["授权不明".to_owned()],
            license: None,
            maturity: None,
            zh_friendly: None,
            keywords: vec!["封面".to_owned()],
            topics: vec!["Rust".to_owned()],
            forks_count: 0,
            readme_excerpt: None,
        };
        let detail_json = serde_json::to_value(&detail).unwrap();
        assert_eq!(
            object_keys(&detail_json),
            set(&[
                "id",
                "fullName",
                "htmlUrl",
                "description",
                "language",
                "starsCount",
                "starredAt",
                "pushedAt",
                "oneLiner",
                "highlight",
                "form",
                "scenarioIds",
                "readingStatus",
                "readingGroup",
                "hasCard",
                "confidence",
                "cardReview",
                "whatItIs",
                "capabilities",
                "problemsSolved",
                "useWhen",
                "notFor",
                "howToTry",
                "integratesWith",
                "alternativesHint",
                "inspirationForFox",
                "risks",
                "license",
                "maturity",
                "zhFriendly",
                "keywords",
                "topics",
                "forksCount",
                "readmeExcerpt",
            ])
        );
        for key in [
            "whatItIs",
            "howToTry",
            "inspirationForFox",
            "license",
            "maturity",
            "zhFriendly",
            "readmeExcerpt",
        ] {
            assert!(detail_json[key].is_null(), "{key}");
        }
        for key in [
            "capabilities",
            "problemsSolved",
            "useWhen",
            "notFor",
            "integratesWith",
            "alternativesHint",
            "risks",
            "keywords",
            "topics",
        ] {
            assert!(detail_json[key].is_array(), "{key}");
            assert!(detail_json[key][0].is_string(), "{key}");
        }
        assert!(detail_json["forksCount"].is_number());
        assert!(!detail_json.as_object().unwrap().contains_key("item"));

        let today = TodayView {
            picks: vec![sample_item(None)],
            week_new: vec![sample_item(Some(0.5))],
            counts: TodayCounts {
                total: 1,
                unseen: 1,
                week_new: 1,
                cards_pending: 0,
                cards_suggested: 1,
            },
            synced_at: None,
        };
        let today_json = serde_json::to_value(&today).unwrap();
        assert_eq!(
            object_keys(&today_json),
            set(&["picks", "weekNew", "counts", "syncedAt"])
        );
        assert!(today_json["picks"].is_array());
        assert!(today_json["weekNew"].is_array());
        assert!(today_json["syncedAt"].is_null());
        assert_eq!(
            object_keys(&today_json["picks"][0]),
            object_keys(&item_json)
        );
        assert!(today_json["weekNew"][0]["confidence"].is_number());
        assert_eq!(
            object_keys(&today_json["counts"]),
            set(&[
                "total",
                "unseen",
                "weekNew",
                "cardsPending",
                "cardsSuggested"
            ])
        );
        for key in [
            "total",
            "unseen",
            "weekNew",
            "cardsPending",
            "cardsSuggested",
        ] {
            assert!(today_json["counts"][key].is_number(), "{key}");
        }
    }

    #[test]
    fn default_query_on_3000_repos_stays_under_one_second() {
        let lab = Lab::new("bench");
        lab.with(|conn| {
            let tx = conn.transaction().unwrap();
            for index in 0..5 {
                scenario(
                    &tx,
                    &format!("scn-{index}"),
                    &format!("s{index}"),
                    &format!("场景{index}"),
                    index + 1,
                    0,
                );
            }
            scenario(&tx, "scn-empty", "empty", "空场景", 9, 0);
            scenario(&tx, "scn-arch", "arch", "归档", 10, 1);
            for index in 0..3000 {
                let mut repo = Repo::new(&format!("r{index:04}"));
                repo.starred_at = "2026-10-01T00:00:00Z".to_owned();
                repo.one_liner = Some(format!("仓库{index}的一句话"));
                if index % 17 == 0 {
                    repo.card_json = Some("{".to_owned());
                }
                repo.save(&tx);
                link(&tx, &repo.id, &format!("scn-{}", index % 5), "suggested");
            }
            tx.commit().unwrap();
        });
        let started = Instant::now();
        let page =
            list_library_view(&lab.path, &LibraryFilters::default()).expect("默认查询应成功");
        let elapsed = started.elapsed();
        let millis = elapsed.as_millis();
        eprintln!("list_library_view 3000 elapsed_ms={millis}");
        let _ = fs::write("/tmp/fsl-07-library-view-bench.txt", format!("{millis}"));
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "默认查询耗时 {millis} ms，超过 1 秒"
        );
        assert_eq!(page.items.len(), 80);
        assert_eq!(page.items[0].id, "r0000");
        assert_eq!(page.items[79].id, "r0079");
        assert_eq!(page.total_count, 3000);
        assert_eq!(page.counts.total, 3000);
        assert_eq!(page.counts.card_state.done, 3000);
        assert_eq!(page.counts.card_state.pending, 0);
        assert_eq!(page.counts.reading.unseen, 3000);
        assert_eq!(page.counts.scenarios.get("scn-empty"), Some(&0));
        assert!(!page.counts.scenarios.contains_key("scn-arch"));
        let bad = (0..3000).filter(|index| index % 17 == 0).count() as i64;
        assert_eq!(page.counts.forms.get("library"), Some(&(3000 - bad)));

        let mut wide = LibraryFilters::default();
        wide.limit = Some(10_000);
        let wide_page = list_library_view(&lab.path, &wide).unwrap();
        assert_eq!(wide_page.items.len(), 200);
        assert_eq!(wide_page.total_count, 3000);
    }

    #[test]
    fn imported_placeholder_cards_are_clean_in_library() {
        const SUMMARY_AWESOME: &str = "Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。每个模板均为原创、自包含的完整源代码，支持3条命令即可运行。覆盖AI Agents、多智能体团队、MCP、语音AI、RAG、微调等现代AI栈，支持Claude、Gemini、GPT、Llama等多种模型提供商。采用Apache-2.0许可证，可自由商用。";
        const SUMMARY_CHATCUT: &str = "ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件，支持通过 AI 辅助编辑 ChatCut 视频项目，具体功能包括导入媒体、修改时间线、创建动态图形、生成素材、音频转录、添加字幕、导出视频以及验证编辑可见性。";
        const LINE_AWESOME: &str =
            "Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。";
        const LINE_CHATCUT: &str = "ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件…";
        let lab = Lab::new("legacy-card");
        lab.with(|conn| {
            for (id, summary) in [("awesome", SUMMARY_AWESOME), ("chatcut", SUMMARY_CHATCUT)] {
                let mut repo = Repo::new(id);
                repo.full_name = format!("lab/{id}");
                repo.summary = summary.to_owned();
                repo.card_json = None;
                repo.one_liner = None;
                repo.description = None;
                repo.save(conn);
            }
        });
        assert_eq!(
            crate::v2_ops::import_summary_cards(&lab.path).expect("应能导入占位卡"),
            2
        );
        lab.with(|conn| {
            let stored: String = conn
                .query_row(
                    "SELECT one_liner_zh FROM repo_ai_documents WHERE repo_id = 'awesome'",
                    [],
                    |row| row.get(0),
                )
                .expect("应能读到库里的一句话");
            let hard_cut: String = SUMMARY_AWESOME.chars().take(60).collect();
            assert_eq!(stored, hard_cut);
            assert_ne!(stored, LINE_AWESOME);
        });

        let page =
            list_library_view(&lab.path, &LibraryFilters::default()).expect("资料库应能列出");
        let item = |id: &str| {
            page.items
                .iter()
                .find(|item| item.id == id)
                .expect("应能找到仓库")
        };
        let awesome = item("awesome");
        let chatcut = item("chatcut");
        assert_eq!(awesome.one_liner.as_deref(), Some(LINE_AWESOME));
        assert_eq!(chatcut.one_liner.as_deref(), Some(LINE_CHATCUT));
        assert!(awesome.highlight.is_none());
        assert!(chatcut.highlight.is_none());
        assert!(awesome.has_card);
        assert!(chatcut.has_card);
        assert_eq!(awesome.form.as_deref(), Some("other"));
        assert_eq!(awesome.card_review.as_deref(), Some("suggested"));
        assert!((awesome.confidence.unwrap_or(0.0) - 0.4).abs() < 1e-9);

        let awesome_detail = get_library_detail(&lab.path, "awesome").expect("详情应能读取");
        let chatcut_detail = get_library_detail(&lab.path, "chatcut").expect("详情应能读取");
        for detail in [&awesome_detail, &chatcut_detail] {
            assert!(detail.capabilities.is_empty());
            assert!(detail.problems_solved.is_empty());
            assert!(detail.how_to_try.is_none());
            assert!(detail.item.highlight.is_none());
            assert!(detail.item.has_card);
        }
        assert_eq!(awesome_detail.what_it_is.as_deref(), Some(SUMMARY_AWESOME));
        assert_eq!(chatcut_detail.what_it_is.as_deref(), Some(SUMMARY_CHATCUT));
        assert_eq!(awesome_detail.item.one_liner.as_deref(), Some(LINE_AWESOME));
        assert_eq!(chatcut_detail.item.one_liner.as_deref(), Some(LINE_CHATCUT));

        let today = get_today_view(&lab.path, "2026-10-03T00:00:00Z").expect("今日应能读取");
        assert!(!today.picks.is_empty());
        for item in today.picks.iter().chain(today.week_new.iter()) {
            assert!(item.highlight.is_none());
            assert!(item.has_card);
            assert!(
                item.one_liner.as_deref() == Some(LINE_AWESOME)
                    || item.one_liner.as_deref() == Some(LINE_CHATCUT)
            );
        }

        let page_json = serde_json::to_string(&page).expect("列表应能序列化");
        let awesome_json = serde_json::to_string(&awesome_detail).expect("详情应能序列化");
        let chatcut_json = serde_json::to_string(&chatcut_detail).expect("详情应能序列化");
        let today_json = serde_json::to_string(&today).expect("今日应能序列化");
        for json in [&page_json, &awesome_json, &chatcut_json, &today_json] {
            assert_no_legacy_phrases(json);
        }
        let evidence = format!(
            "awesome.oneLiner={}\nawesome.highlight={:?}\nawesome.hasCard={}\nawesome.whatItIs={}\nawesome.capabilities={:?}\nawesome.problemsSolved={:?}\nawesome.howToTry={:?}\nawesome.confidence={:?}\nawesome.cardReview={:?}\n\nchatcut.oneLiner={}\nchatcut.highlight={:?}\nchatcut.hasCard={}\nchatcut.whatItIs={}\nchatcut.capabilities={:?}\nchatcut.problemsSolved={:?}\nchatcut.howToTry={:?}\n",
            awesome.one_liner.as_deref().unwrap_or(""),
            awesome.highlight,
            awesome.has_card,
            awesome_detail.what_it_is.as_deref().unwrap_or(""),
            awesome_detail.capabilities,
            awesome_detail.problems_solved,
            awesome_detail.how_to_try,
            awesome.confidence,
            awesome.card_review,
            chatcut.one_liner.as_deref().unwrap_or(""),
            chatcut.highlight,
            chatcut.has_card,
            chatcut_detail.what_it_is.as_deref().unwrap_or(""),
            chatcut_detail.capabilities,
            chatcut_detail.problems_solved,
            chatcut_detail.how_to_try,
        );
        fs::write("/tmp/fsl-15-library.txt", &evidence).expect("应能写下核对文本");
    }

    fn assert_no_legacy_phrases(json: &str) {
        for phrase in [
            crate::card_view::LEGACY_PROBLEM,
            crate::card_view::LEGACY_CAPABILITY,
            crate::card_view::LEGACY_HOW_TO_TRY,
        ] {
            assert!(!json.contains(phrase), "不该出现「{phrase}」");
        }
    }

    fn sample_item(confidence: Option<f64>) -> LibraryItem {
        LibraryItem {
            id: "repo".to_owned(),
            full_name: "o/repo".to_owned(),
            html_url: "https://github.com/o/repo".to_owned(),
            description: None,
            language: None,
            stars_count: 0,
            starred_at: "2026-10-01T00:00:00Z".to_owned(),
            pushed_at: None,
            one_liner: None,
            highlight: None,
            form: None,
            scenario_ids: vec!["scn-a".to_owned()],
            reading_status: "in_use".to_owned(),
            reading_group: "in_use".to_owned(),
            has_card: false,
            confidence,
            card_review: None,
        }
    }

    fn set(keys: &[&str]) -> BTreeSet<String> {
        keys.iter().map(|key| (*key).to_owned()).collect()
    }

    fn owned(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }
}
