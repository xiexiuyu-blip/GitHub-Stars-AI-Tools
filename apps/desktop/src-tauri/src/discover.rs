//! 发现：按场景去 GitHub 搜还没收藏的工具。平时只读缓存，刷新才联网。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::Value;

use crate::web_fetch::{
    format_unix_time, rate_limit_wait_secs, since_date, url_encode, HttpFetcher,
};

const QUERIES: &str = include_str!("../../../../docs/rebuild/spec/discover_queries.json");
/// 候选的 GitHub 描述超过这个长度就当垃圾丢掉（实测有「个人收藏书籍列表」这类仓库，描述里塞满关键词，搜什么都出现）。
const MAX_DESCRIPTION_CHARS: usize = 300;
/// 查得太频繁时给用户看的那句。不把还要等几秒写进去。
const RATE_LIMIT_MESSAGE: &str =
    "GitHub 暂时不让查太频繁，过几分钟再点「重新找找」，已找到的先列在下面";
const GITHUB_HEADERS: &[(&str, &str)] = &[
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", "2022-11-28"),
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverScenario {
    pub id: String,
    pub slug: String,
    pub name_zh: String,
    pub sort_order: i64,
    pub my_count: i64,
    pub thin: bool,
    pub candidate_count: i64,
    pub last_fetched_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverCandidate {
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars_count: i64,
    pub pushed_at: Option<String>,
    pub topics: Vec<String>,
    pub mine: bool,
    pub in_inbox: bool,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverView {
    pub scenarios: Vec<DiscoverScenario>,
    pub selected: Option<String>,
    pub candidates: Vec<DiscoverCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverRefreshReport {
    pub scenario_id: String,
    pub queries: i64,
    pub failed_queries: i64,
    pub fetched: i64,
    pub skipped_mine: i64,
    pub message: Option<String>,
    pub rate_limit_reset_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchRepo {
    full_name: String,
    html_url: String,
    description: Option<String>,
    language: Option<String>,
    stars_count: i64,
    pushed_at: Option<String>,
    topics: Vec<String>,
    archived: bool,
    fork: bool,
    disabled: bool,
}

#[derive(Debug)]
struct ScenarioRef {
    id: String,
    slug: String,
    account_id: String,
}

#[derive(Debug, serde::Deserialize)]
struct QueryFile {
    version: i64,
    since_days: i64,
    per_scenario_limit: usize,
    scenarios: HashMap<String, Vec<String>>,
}

#[derive(Debug)]
struct CandidateRow {
    full_name: String,
    html_url: String,
    description: Option<String>,
    language: Option<String>,
    stars_count: i64,
    pushed_at: Option<String>,
    topics_json: String,
    fetched_at: String,
}

fn open_db(path: &Path) -> Result<Connection, String> {
    crate::v2_ops::open_db(path)
}

fn load_queries() -> Result<QueryFile, String> {
    let file: QueryFile =
        serde_json::from_str(QUERIES).map_err(|_| "搜索词配置读不出来".to_owned())?;
    if file.version != 1 || file.since_days < 0 {
        return Err("搜索词配置版本不对".to_owned());
    }
    Ok(file)
}

fn latest_account(connection: &Connection) -> Result<String, String> {
    connection
        .query_row(
            "SELECT id FROM github_accounts ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("账号读取失败：{error}"))?
        .ok_or_else(|| "还没有 GitHub 账号".to_owned())
}

fn load_scenario(path: &Path, scenario_id: &str) -> Result<ScenarioRef, String> {
    let connection = open_db(path)?;
    let account_id = latest_account(&connection)?;
    let row: Option<(String, String, i64)> = connection
        .query_row(
            "SELECT id, slug, archived FROM scenarios WHERE id = ?1 AND account_id = ?2",
            params![scenario_id, account_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| format!("场景读取失败：{error}"))?;
    match row {
        Some((id, slug, archived)) if archived == 0 => Ok(ScenarioRef {
            id,
            slug,
            account_id,
        }),
        _ => Err("找不到这个场景".to_owned()),
    }
}

fn expand_since(template: &str, since: &str) -> String {
    template.replace("{since}", since)
}

fn github_search_url(query: &str) -> String {
    format!(
        "https://api.github.com/search/repositories?q={}&sort=stars&order=desc&per_page=15",
        url_encode(query)
    )
}

fn text_opt(item: &Value, key: &str) -> Option<String> {
    match item.get(key) {
        Some(Value::String(text)) if !text.is_empty() => Some(text.clone()),
        _ => None,
    }
}

fn flag(item: &Value, key: &str) -> bool {
    item.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// 缺 `items`、不是 JSON，都算这条搜索词失败。空数组算成功。
fn parse_search_body(body: &str) -> Result<Vec<SearchRepo>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "搜索结果读不出来".to_owned())?;
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| "搜索结果读不出来".to_owned())?;
    let mut repos = Vec::new();
    for item in items {
        let Some(full_name) = item
            .get("full_name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
        else {
            continue;
        };
        let Some(html_url) = item
            .get("html_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
        else {
            continue;
        };
        let topics = match item.get("topics") {
            Some(Value::Array(list)) => list
                .iter()
                .filter_map(Value::as_str)
                .filter(|topic| !topic.is_empty())
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        repos.push(SearchRepo {
            full_name: full_name.to_owned(),
            html_url: html_url.to_owned(),
            description: text_opt(item, "description"),
            language: text_opt(item, "language"),
            stars_count: item
                .get("stargazers_count")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            pushed_at: text_opt(item, "pushed_at"),
            topics,
            archived: flag(item, "archived"),
            fork: flag(item, "fork"),
            disabled: flag(item, "disabled"),
        });
    }
    Ok(repos)
}

fn select_candidates(repos: Vec<SearchRepo>, limit: usize) -> Vec<SearchRepo> {
    let mut best: HashMap<String, SearchRepo> = HashMap::new();
    for repo in repos {
        if repo.archived || repo.fork || repo.disabled {
            continue;
        }
        // 描述长得离谱（几百个字符、塞满关键词和空格）的是垃圾合集，不是工具
        if repo
            .description
            .as_deref()
            .is_some_and(|text| text.chars().count() > MAX_DESCRIPTION_CHARS)
        {
            continue;
        }
        let key = repo.full_name.to_ascii_lowercase();
        let replace = match best.get(&key) {
            Some(existing) => repo.stars_count > existing.stars_count,
            None => true,
        };
        if replace {
            best.insert(key, repo);
        }
    }
    let mut list: Vec<SearchRepo> = best.into_values().collect();
    list.sort_by(|left, right| {
        right.stars_count.cmp(&left.stars_count).then_with(|| {
            left.full_name
                .to_ascii_lowercase()
                .cmp(&right.full_name.to_ascii_lowercase())
        })
    });
    list.truncate(limit);
    list
}

/// 一条都没成功（含一开始就限流）时不删旧缓存。`queries` 只记实际发出的请求数。
fn refresh_with_config(
    fetcher: &impl HttpFetcher,
    db_path: &Path,
    scenario: &ScenarioRef,
    queries: &[String],
    limit: usize,
    since: &str,
    now: i64,
) -> Result<DiscoverRefreshReport, String> {
    if queries.is_empty() {
        return Ok(DiscoverRefreshReport {
            scenario_id: scenario.id.clone(),
            queries: 0,
            failed_queries: 0,
            fetched: 0,
            skipped_mine: 0,
            message: Some("这个场景还没有配置搜索词".to_owned()),
            rate_limit_reset_at: None,
        });
    }
    let mut collected = Vec::new();
    let mut attempted = 0i64;
    let mut failed = 0i64;
    let mut successes = 0i64;
    let mut rate_wait: Option<u64> = None;
    for template in queries {
        attempted += 1;
        let url = github_search_url(&expand_since(template, since));
        match fetcher.get(&url, GITHUB_HEADERS) {
            Err(_) => failed += 1,
            Ok(response) => {
                if let Some(wait) = rate_limit_wait_secs(&response, now) {
                    failed += 1;
                    rate_wait = Some(wait);
                    break;
                }
                if response.status != 200 {
                    failed += 1;
                    continue;
                }
                match parse_search_body(&response.body) {
                    Ok(items) => {
                        successes += 1;
                        collected.extend(items);
                    }
                    Err(_) => failed += 1,
                }
            }
        }
    }
    let message = if rate_wait.is_some() {
        Some(RATE_LIMIT_MESSAGE.to_owned())
    } else if successes == 0 {
        Some("联网失败，保留上次的结果".to_owned())
    } else {
        None
    };
    let rate_limit_reset_at = match rate_wait {
        Some(wait) => {
            let wait = i64::try_from(wait).map_err(|_| "等待时间无效".to_owned())?;
            let reset_at = now
                .checked_add(wait)
                .ok_or_else(|| "等待时间无效".to_owned())?;
            Some(format_unix_time(reset_at)?)
        }
        None => None,
    };
    if successes == 0 {
        return Ok(DiscoverRefreshReport {
            scenario_id: scenario.id.clone(),
            queries: attempted,
            failed_queries: failed,
            fetched: 0,
            skipped_mine: 0,
            message,
            rate_limit_reset_at,
        });
    }
    let selected = select_candidates(collected, limit);
    let (fetched, skipped_mine) = replace_cache(
        db_path,
        scenario,
        &selected,
        attempted,
        failed,
        message.as_deref(),
        now,
    )?;
    Ok(DiscoverRefreshReport {
        scenario_id: scenario.id.clone(),
        queries: attempted,
        failed_queries: failed,
        fetched,
        skipped_mine,
        message,
        rate_limit_reset_at,
    })
}

pub fn refresh_discover_with(
    fetcher: &impl HttpFetcher,
    db_path: &Path,
    scenario_id: &str,
    now: i64,
) -> Result<DiscoverRefreshReport, String> {
    let config = load_queries()?;
    let scenario = load_scenario(db_path, scenario_id)?;
    let queries = config
        .scenarios
        .get(&scenario.slug)
        .cloned()
        .unwrap_or_default();
    let since = since_date(now, config.since_days)?;
    refresh_with_config(
        fetcher,
        db_path,
        &scenario,
        &queries,
        config.per_scenario_limit,
        &since,
        now,
    )
}

fn active_names(connection: &Connection, account_id: &str) -> Result<HashSet<String>, String> {
    let mut statement = connection
        .prepare(
            "SELECT full_name FROM repositories WHERE account_id = ?1 AND sync_status = 'active'",
        )
        .map_err(|error| format!("已收藏工具读取失败：{error}"))?;
    let rows = statement
        .query_map(params![account_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("已收藏工具查询失败：{error}"))?;
    let mut names = HashSet::new();
    for row in rows {
        let name = row.map_err(|error| format!("已收藏工具行读取失败：{error}"))?;
        names.insert(name.to_ascii_lowercase());
    }
    Ok(names)
}

fn replace_cache(
    db_path: &Path,
    scenario: &ScenarioRef,
    selected: &[SearchRepo],
    attempted: i64,
    failed: i64,
    message: Option<&str>,
    now: i64,
) -> Result<(i64, i64), String> {
    let fetched_at = format_unix_time(now)?;
    let mut connection = open_db(db_path)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("发现缓存事务开启失败：{error}"))?;
    let mine = active_names(&tx, &scenario.account_id)?;
    tx.execute(
        "DELETE FROM discover_candidates WHERE scenario_id = ?1",
        params![scenario.id],
    )
    .map_err(|error| format!("发现缓存清理失败：{error}"))?;
    let mut fetched = 0i64;
    let mut skipped_mine = 0i64;
    for repo in selected {
        let topics = serde_json::to_string(&repo.topics).map_err(|_| "话题写不进去".to_owned())?;
        tx.execute(
            "INSERT INTO discover_candidates(scenario_id, full_name, html_url, description, language, stars_count, pushed_at, topics_json, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                scenario.id,
                repo.full_name,
                repo.html_url,
                repo.description,
                repo.language,
                repo.stars_count,
                repo.pushed_at,
                topics,
                fetched_at
            ],
        )
        .map_err(|error| format!("发现候选写入失败：{error}"))?;
        if mine.contains(&repo.full_name.to_ascii_lowercase()) {
            skipped_mine += 1;
        } else {
            fetched += 1;
        }
    }
    tx.execute(
        "INSERT INTO discover_runs(scenario_id, last_fetched_at, queries, failed_queries, message) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(scenario_id) DO UPDATE SET last_fetched_at = excluded.last_fetched_at, queries = excluded.queries, failed_queries = excluded.failed_queries, message = excluded.message",
        params![scenario.id, fetched_at, attempted, failed, message],
    )
    .map_err(|error| format!("发现记录写入失败：{error}"))?;
    tx.commit()
        .map_err(|error| format!("发现缓存提交失败：{error}"))?;
    Ok((fetched, skipped_mine))
}

fn inbox_links(connection: &Connection, account_id: &str) -> Result<HashSet<String>, String> {
    let mut statement = connection
        .prepare(
            "SELECT normalized_url FROM inbox_items WHERE account_id = ?1 AND status <> 'archived'",
        )
        .map_err(|error| format!("收集箱读取失败：{error}"))?;
    let rows = statement
        .query_map(params![account_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("收集箱查询失败：{error}"))?;
    let mut links = HashSet::new();
    for row in rows {
        links.insert(row.map_err(|error| format!("收集箱行读取失败：{error}"))?);
    }
    Ok(links)
}

fn in_inbox(html_url: &str, inbox: &HashSet<String>) -> bool {
    crate::capture::extract_links_from_text(html_url)
        .into_iter()
        .any(|link| link.kind == "github_repo" && inbox.contains(&link.normalized_url))
}

fn parse_topics(text: &str) -> Vec<String> {
    serde_json::from_str(text).unwrap_or_default()
}

fn cached_candidates(
    connection: &Connection,
) -> Result<HashMap<String, Vec<CandidateRow>>, String> {
    let mut statement = connection
        .prepare(
            "SELECT scenario_id, full_name, html_url, description, language, stars_count, pushed_at, topics_json, fetched_at FROM discover_candidates",
        )
        .map_err(|error| format!("发现候选读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CandidateRow {
                    full_name: row.get(1)?,
                    html_url: row.get(2)?,
                    description: row.get(3)?,
                    language: row.get(4)?,
                    stars_count: row.get(5)?,
                    pushed_at: row.get(6)?,
                    topics_json: row.get(7)?,
                    fetched_at: row.get(8)?,
                },
            ))
        })
        .map_err(|error| format!("发现候选查询失败：{error}"))?;
    let mut grouped = HashMap::new();
    for row in rows {
        let (scenario_id, candidate) =
            row.map_err(|error| format!("发现候选行读取失败：{error}"))?;
        grouped
            .entry(scenario_id)
            .or_insert_with(Vec::new)
            .push(candidate);
    }
    Ok(grouped)
}

fn sort_candidates(rows: &mut [CandidateRow]) {
    rows.sort_by(|left, right| {
        right.stars_count.cmp(&left.stars_count).then_with(|| {
            left.full_name
                .to_ascii_lowercase()
                .cmp(&right.full_name.to_ascii_lowercase())
        })
    });
}

fn run_times(connection: &Connection) -> Result<HashMap<String, String>, String> {
    let mut statement = connection
        .prepare("SELECT scenario_id, last_fetched_at FROM discover_runs")
        .map_err(|error| format!("发现记录读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("发现记录查询失败：{error}"))?;
    let mut times = HashMap::new();
    for row in rows {
        let (id, fetched_at) = row.map_err(|error| format!("发现记录行读取失败：{error}"))?;
        times.insert(id, fetched_at);
    }
    Ok(times)
}

fn count_new(rows: &[CandidateRow], mine: &HashSet<String>) -> i64 {
    rows.iter()
        .filter(|row| !mine.contains(&row.full_name.to_ascii_lowercase()))
        .count() as i64
}

fn to_candidate(
    row: CandidateRow,
    mine: &HashSet<String>,
    inbox: &HashSet<String>,
) -> DiscoverCandidate {
    let mine_flag = mine.contains(&row.full_name.to_ascii_lowercase());
    let in_inbox = in_inbox(&row.html_url, inbox);
    DiscoverCandidate {
        full_name: row.full_name,
        html_url: row.html_url,
        description: row.description,
        language: row.language,
        stars_count: row.stars_count,
        pushed_at: row.pushed_at,
        topics: parse_topics(&row.topics_json),
        mine: mine_flag,
        in_inbox,
        fetched_at: row.fetched_at,
    }
}

/// 不认识的 id 回退到默认选中，不报错。刷新才对非法 id 报错。
fn choose_selected(scenarios: &[DiscoverScenario], requested: Option<&str>) -> Option<String> {
    if let Some(id) = requested {
        if let Some(found) = scenarios.iter().find(|item| item.id == id) {
            return Some(found.id.clone());
        }
    }
    scenarios
        .iter()
        .find(|item| item.thin)
        .or_else(|| scenarios.first())
        .map(|item| item.id.clone())
}

pub fn list_discover_view(
    db_path: &Path,
    scenario_id: Option<&str>,
) -> Result<DiscoverView, String> {
    let connection = open_db(db_path)?;
    let account = latest_account(&connection)?;
    drop(connection);
    let summaries = crate::scenarios::list_scenarios_summary(db_path)?;
    let connection = open_db(db_path)?;
    let mine = active_names(&connection, &account)?;
    let inbox = inbox_links(&connection, &account)?;
    let runs = run_times(&connection)?;
    let mut cached = cached_candidates(&connection)?;
    let mut scenarios = Vec::new();
    let mut kept_rows = HashMap::new();
    for summary in summaries {
        let rows = cached.remove(&summary.id).unwrap_or_default();
        let id = summary.id.clone();
        scenarios.push(DiscoverScenario {
            id: summary.id,
            slug: summary.slug,
            name_zh: summary.name_zh,
            sort_order: summary.sort_order,
            my_count: summary.count,
            thin: summary.thin,
            candidate_count: count_new(&rows, &mine),
            last_fetched_at: runs.get(&id).cloned(),
        });
        kept_rows.insert(id, rows);
    }
    let selected = choose_selected(&scenarios, scenario_id);
    let candidates = match selected.as_deref() {
        Some(id) => {
            let mut rows = kept_rows.remove(id).unwrap_or_default();
            sort_candidates(&mut rows);
            rows.into_iter()
                .map(|row| to_candidate(row, &mine, &inbox))
                .collect()
        }
        None => Vec::new(),
    };
    Ok(DiscoverView {
        scenarios,
        selected,
        candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::{insert_named_test_account, insert_test_account, migrate_database};
    use crate::scenarios::seed_scenarios;
    use crate::web_fetch::{
        format_unix_time, since_date, FetchError, FetchErrorKind, HttpResponse,
    };
    use rusqlite::{params, OptionalExtension};
    use serde_json::{json, Value};
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_DB_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDb(PathBuf);

    impl Drop for TempDb {
        fn drop(&mut self) {
            let text = self.0.to_string_lossy().to_string();
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(format!("{text}-wal"));
            let _ = std::fs::remove_file(format!("{text}-shm"));
        }
    }

    struct Scripted {
        responses: Mutex<Vec<Result<HttpResponse, FetchError>>>,
        calls: Mutex<Vec<String>>,
    }

    impl Scripted {
        fn new(responses: Vec<Result<HttpResponse, FetchError>>) -> Self {
            Self {
                responses: Mutex::new(responses),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("锁").clone()
        }
    }

    impl HttpFetcher for Scripted {
        fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, FetchError> {
            assert_eq!(
                headers,
                [
                    ("Accept", "application/vnd.github+json"),
                    ("X-GitHub-Api-Version", "2022-11-28"),
                ]
            );
            self.calls.lock().expect("锁").push(url.to_owned());
            let mut queue = self.responses.lock().expect("锁");
            if queue.is_empty() {
                panic!("没有更多假响应");
            }
            queue.remove(0)
        }
    }

    struct PanicFetcher;

    impl HttpFetcher for PanicFetcher {
        fn get(&self, url: &str, _: &[(&str, &str)]) -> Result<HttpResponse, FetchError> {
            panic!("不应联网：{url}");
        }
    }

    fn epoch() -> i64 {
        time::Date::from_calendar_date(2026, time::Month::October, 3)
            .expect("日期")
            .with_hms(0, 0, 0)
            .expect("时间")
            .assume_utc()
            .unix_timestamp()
    }

    fn ok_body(body: String) -> Result<HttpResponse, FetchError> {
        Ok(HttpResponse {
            status: 200,
            headers: Vec::new(),
            body,
        })
    }

    fn network() -> Result<HttpResponse, FetchError> {
        Err(FetchError {
            kind: FetchErrorKind::Network,
            message: "网络连不上".to_owned(),
        })
    }

    fn limited(reset_at: i64) -> Result<HttpResponse, FetchError> {
        Ok(HttpResponse {
            status: 403,
            headers: vec![
                ("x-ratelimit-remaining".to_owned(), "0".to_owned()),
                ("x-ratelimit-reset".to_owned(), reset_at.to_string()),
            ],
            body: "{}".to_owned(),
        })
    }

    fn search_body(items: Vec<Value>) -> String {
        json!({ "items": items }).to_string()
    }

    fn repo_value(full_name: &str, stars: i64) -> Value {
        json!({
            "full_name": full_name,
            "html_url": format!("https://github.com/{full_name}"),
            "description": "说明",
            "language": "Rust",
            "stargazers_count": stars,
            "pushed_at": "2026-09-01T00:00:00Z",
            "topics": ["cli"],
            "archived": false,
            "fork": false,
            "disabled": false
        })
    }

    fn temp_db(label: &str) -> TempDb {
        let seq = TEMP_DB_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时间应可用")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fsl-discover-{label}-{seq}-{nanos}.sqlite3"));
        if let Err(error) = migrate_database(&path) {
            let text = path.to_string_lossy().to_string();
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(format!("{text}-wal"));
            let _ = std::fs::remove_file(format!("{text}-shm"));
            panic!("测试库应能迁移：{error}");
        }
        TempDb(path)
    }

    fn ready(label: &str) -> TempDb {
        let db = temp_db(label);
        insert_test_account(&db.0);
        let inserted = seed_scenarios(&db.0).expect("种子应写入");
        assert!(inserted >= 13, "应插入 13 个场景，实际 {inserted}");
        db
    }

    fn insert_repo(path: &Path, account: &str, id: &str, full_name: &str, sync_status: &str) {
        let (owner, name) = match full_name.split_once('/') {
            Some((owner, name)) => (owner, name),
            None => (full_name, full_name),
        };
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO repositories(id, account_id, owner, name, full_name, description, topics_json, html_url, sync_status, starred_at) VALUES (?1, ?2, ?3, ?4, ?5, '', '[]', ?6, ?7, '2026-09-01T00:00:00Z')",
                params![id, account, owner, name, full_name, format!("https://github.com/{full_name}"), sync_status],
            )
            .expect("应能写入仓库");
    }

    fn insert_link(path: &Path, repo_id: &str, scenario_id: &str) {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, origin, confidence, status) VALUES ('acct', 'repository', ?1, ?2, 'user', 1.0, 'confirmed')",
                params![repo_id, scenario_id],
            )
            .expect("应能写入归类");
    }

    fn insert_inbox(path: &Path, id: &str, normalized_url: &str, status: &str) {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO inbox_items(id, account_id, original_url, normalized_url, item_type, status) VALUES (?1, 'acct', ?2, ?2, 'github_repository', ?3)",
                params![id, normalized_url, status],
            )
            .expect("应能写入收集箱");
    }

    fn insert_candidate(
        path: &Path,
        scenario_id: &str,
        full_name: &str,
        stars: i64,
        topics_json: &str,
        html_url: Option<&str>,
    ) {
        let html_url = html_url
            .map(str::to_owned)
            .unwrap_or_else(|| format!("https://github.com/{full_name}"));
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO discover_candidates(scenario_id, full_name, html_url, stars_count, topics_json, fetched_at) VALUES (?1, ?2, ?3, ?4, ?5, '2026-09-01T00:00:00Z')",
                params![scenario_id, full_name, html_url, stars, topics_json],
            )
            .expect("应能写入候选");
    }

    fn insert_run(
        path: &Path,
        scenario_id: &str,
        fetched_at: &str,
        queries: i64,
        failed: i64,
        message: Option<&str>,
    ) {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO discover_runs(scenario_id, last_fetched_at, queries, failed_queries, message) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![scenario_id, fetched_at, queries, failed, message],
            )
            .expect("应能写入发现记录");
    }

    fn insert_user_scenario(path: &Path, id: &str, slug: &str, name: &str) {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO scenarios(id, account_id, slug, name_zh, sort_order, origin) VALUES (?1, 'acct', ?2, ?3, 99, 'user')",
                params![id, slug, name],
            )
            .expect("应能写入场景");
    }

    fn candidate_names(path: &Path, scenario_id: &str) -> Vec<String> {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        let mut statement = connection
            .prepare(
                "SELECT full_name FROM discover_candidates WHERE scenario_id = ?1 ORDER BY stars_count DESC, full_name ASC",
            )
            .expect("应能准备查询");
        let rows = statement
            .query_map(params![scenario_id], |row| row.get::<_, String>(0))
            .expect("应能查询候选");
        rows.map(|row| row.expect("行")).collect()
    }

    fn run_row(path: &Path, scenario_id: &str) -> Option<(String, i64, i64, Option<String>)> {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .query_row(
                "SELECT last_fetched_at, queries, failed_queries, message FROM discover_runs WHERE scenario_id = ?1",
                params![scenario_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .expect("应能读取发现记录")
    }

    fn count_scenarios(path: &Path) -> i64 {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .query_row("SELECT COUNT(*) FROM scenarios", [], |row| row.get(0))
            .expect("计数")
    }

    fn expect_keys(value: &Value, expected: &[&str]) {
        let actual: BTreeSet<String> = value
            .as_object()
            .expect("应为对象")
            .keys()
            .cloned()
            .collect();
        let expected: BTreeSet<String> = expected.iter().map(|item| (*item).to_owned()).collect();
        assert_eq!(actual, expected);
    }

    fn command_body<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source
            .find(marker)
            .unwrap_or_else(|| panic!("找不到 {marker}"));
        let rest = &source[start..];
        let end = rest[marker.len()..]
            .find("\n#[tauri::command]")
            .map(|index| index + marker.len())
            .unwrap_or(rest.len());
        &rest[..end]
    }

    #[test]
    fn query_file_covers_thirteen_scenarios_with_safe_queries() {
        let file = load_queries().expect("配置应能读");
        assert_eq!(file.version, 1);
        assert_eq!(file.since_days, 120);
        assert_eq!(file.per_scenario_limit, 20);
        let mut slugs: Vec<&str> = file.scenarios.keys().map(String::as_str).collect();
        slugs.sort_unstable();
        assert_eq!(
            slugs,
            vec![
                "auto", "data", "dy", "ec", "img", "kb", "ppt", "res", "seo", "vid", "wr", "wx",
                "xhs"
            ]
        );
        for (slug, queries) in &file.scenarios {
            assert_eq!(queries.len(), 2, "{slug} 应有两条搜索词");
            for query in queries {
                assert!(query.contains("{since}"), "{slug}：{query} 缺 {{since}}");
                assert!(query.contains("stars:>"), "{slug}：{query} 缺星数下限");
                assert!(query.contains("pushed:>"), "{slug}：{query} 缺最近更新限定");
                // 总控用真实 GitHub 搜索验证过：一条里写多个 OR 会退化成泛泛的 agent / skills，结果不相关。
                assert!(
                    query.matches(" OR ").count() <= 1,
                    "{slug}：{query} 写了多个 OR"
                );
            }
        }
    }

    #[test]
    fn since_placeholder_is_encoded_in_the_search_url() {
        let file = load_queries().expect("配置应能读");
        let since = since_date(epoch(), file.since_days).expect("日期");
        assert_eq!(since, "2026-06-05");
        let query = expand_since(&file.scenarios["xhs"][1], &since);
        assert!(!query.contains("{since}"));
        assert!(query.contains("pushed:>2026-06-05"));
        let url = github_search_url(&query);
        assert!(url.starts_with("https://api.github.com/search/repositories?q="));
        assert!(url.contains("pushed%3A%3E2026-06-05"));
        assert!(url.contains("%E5%B0%8F%E7%BA%A2%E4%B9%A6"));
        assert!(url.contains("sort=stars"));
        assert!(url.contains("order=desc"));
        assert!(url.contains("per_page=15"));
        assert!(!url.contains(' '));
    }

    #[test]
    fn parse_search_skips_incomplete_and_keeps_nulls() {
        let body = r#"{
            "items": [
                {"full_name":"ok/repo","html_url":" https://github.com/ok/repo ","description":null,"language":"","stargazers_count":3,"topics":["a","","b"]},
                {"full_name":"missing-url"},
                {"html_url":"https://github.com/x"},
                {"full_name":"  ","html_url":"https://github.com/blank"}
            ]
        }"#;
        let repos = parse_search_body(body).expect("应能解析");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "ok/repo");
        assert_eq!(repos[0].html_url, "https://github.com/ok/repo");
        assert_eq!(repos[0].description, None);
        assert_eq!(repos[0].language, None);
        assert_eq!(repos[0].stars_count, 3);
        assert_eq!(repos[0].topics, vec!["a".to_owned(), "b".to_owned()]);
        assert!(parse_search_body("{}").is_err());
        assert!(parse_search_body("nope").is_err());
        assert!(parse_search_body(r#"{"items":[]}"#)
            .expect("空数组算成功")
            .is_empty());
    }

    #[test]
    fn select_drops_flags_dedupes_sorts_and_limits() {
        let body = json!({
            "items": [
                {"full_name":"Archived/High","html_url":"https://github.com/Archived/High","stargazers_count":100,"archived":true},
                {"full_name":"Fork/High","html_url":"https://github.com/Fork/High","stargazers_count":90,"fork":true},
                {"full_name":"Disabled/High","html_url":"https://github.com/Disabled/High","stargazers_count":80,"disabled":true},
                {"full_name":"Keep/Low","html_url":"https://github.com/Keep/Low","stargazers_count":1},
                {"full_name":"keep/low","html_url":"https://github.com/keep/low","stargazers_count":5},
                {"full_name":"b/second","html_url":"https://github.com/b/second","stargazers_count":4},
                {"full_name":"a/first","html_url":"https://github.com/a/first","stargazers_count":4,"disabled":null},
                {"full_name":"c/third","html_url":"https://github.com/c/third","stargazers_count":3}
            ]
        });
        let selected = select_candidates(parse_search_body(&body.to_string()).expect("解析"), 2);
        assert_eq!(
            selected
                .iter()
                .map(|repo| (repo.full_name.as_str(), repo.stars_count))
                .collect::<Vec<_>>(),
            vec![("keep/low", 5), ("a/first", 4)]
        );
    }

    #[test]
    fn select_drops_spammy_long_descriptions() {
        let long = "关键词 ".repeat(120);
        let body = json!({
            "items": [
                {"full_name":"spam/books","html_url":"https://github.com/spam/books","stargazers_count":99999,"description":long},
                {"full_name":"real/tool","html_url":"https://github.com/real/tool","stargazers_count":10,"description":"一个正常的工具"},
                {"full_name":"edge/ok","html_url":"https://github.com/edge/ok","stargazers_count":5,"description":"x".repeat(MAX_DESCRIPTION_CHARS)}
            ]
        });
        let selected = select_candidates(parse_search_body(&body.to_string()).expect("解析"), 10);
        let names: Vec<_> = selected
            .iter()
            .map(|repo| repo.full_name.as_str())
            .collect();
        // 星数最高的那个是垃圾合集，被丢掉；正好等于上限的保留
        assert_eq!(names, vec!["real/tool", "edge/ok"]);
    }

    #[test]
    fn missing_topics_become_empty() {
        let body = r#"{"items":[
            {"full_name":"a/a","html_url":"https://github.com/a/a"},
            {"full_name":"b/b","html_url":"https://github.com/b/b","topics":null},
            {"full_name":"c/c","html_url":"https://github.com/c/c","topics":"cli"}
        ]}"#;
        let repos = parse_search_body(body).expect("解析");
        assert_eq!(repos.len(), 3);
        assert!(repos.iter().all(|repo| repo.topics.is_empty()));
    }

    #[test]
    fn refresh_replaces_old_rows_and_counts_skipped_mine() {
        let db = ready("replace");
        insert_repo(&db.0, "acct", "mine", "fox/mine", "active");
        insert_candidate(&db.0, "scn-xhs", "gone/old", 1, "[]", None);
        insert_candidate(&db.0, "scn-wx", "keep/wx", 1, "[]", None);
        let mut fork = repo_value("Fork/Skip", 99);
        fork["fork"] = json!(true);
        let fetcher = Scripted::new(vec![
            ok_body(search_body(vec![
                repo_value("Fox/Mine", 9),
                repo_value("New/Lib", 4),
                fork,
            ])),
            ok_body(search_body(vec![repo_value("Extra/One", 2)])),
        ]);
        let report =
            refresh_discover_with(&fetcher, &db.0, "scn-xhs", epoch()).expect("刷新应成功");
        assert_eq!(report.queries, 2);
        assert_eq!(report.failed_queries, 0);
        assert_eq!(report.fetched, 2);
        assert_eq!(report.skipped_mine, 1);
        assert_eq!(report.message, None);
        assert_eq!(report.rate_limit_reset_at, None);
        assert_eq!(
            candidate_names(&db.0, "scn-xhs"),
            vec![
                "Fox/Mine".to_owned(),
                "New/Lib".to_owned(),
                "Extra/One".to_owned()
            ]
        );
        assert_eq!(candidate_names(&db.0, "scn-wx"), vec!["keep/wx".to_owned()]);
        let calls = fetcher.calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].contains("topic%3Axiaohongshu"));
        assert!(calls[1].contains("%E5%B0%8F%E7%BA%A2%E4%B9%A6"));
        assert!(calls[0].contains("pushed%3A%3E2026-06-05"));
        let run = run_row(&db.0, "scn-xhs").expect("应有记录");
        assert_eq!(run.0, format_unix_time(epoch()).expect("时间"));
        assert_eq!(run.1, 2);
        assert_eq!(run.2, 0);
        assert_eq!(run.3, None);
    }

    #[test]
    fn one_query_failure_still_writes_the_rest() {
        let db = ready("partial-fail");
        insert_candidate(&db.0, "scn-xhs", "gone/old", 1, "[]", None);
        let fetcher = Scripted::new(vec![
            network(),
            ok_body(search_body(vec![repo_value("ok/repo", 3)])),
        ]);
        let report =
            refresh_discover_with(&fetcher, &db.0, "scn-xhs", epoch()).expect("部分失败仍成功");
        assert_eq!(report.queries, 2);
        assert_eq!(report.failed_queries, 1);
        assert_eq!(report.fetched, 1);
        assert_eq!(report.skipped_mine, 0);
        assert_eq!(report.message, None);
        assert_eq!(
            candidate_names(&db.0, "scn-xhs"),
            vec!["ok/repo".to_owned()]
        );
    }

    #[test]
    fn all_queries_fail_keep_old_cache() {
        let db = ready("all-fail");
        insert_candidate(&db.0, "scn-xhs", "gone/old", 1, "[]", None);
        insert_run(&db.0, "scn-xhs", "2026-01-01T00:00:00Z", 9, 1, Some("上次"));
        let fetcher = Scripted::new(vec![network(), network()]);
        let report = refresh_discover_with(&fetcher, &db.0, "scn-xhs", epoch()).expect("应返回");
        assert_eq!(report.message.as_deref(), Some("联网失败，保留上次的结果"));
        assert_eq!(report.queries, 2);
        assert_eq!(report.failed_queries, 2);
        assert_eq!(report.fetched, 0);
        assert_eq!(report.skipped_mine, 0);
        assert_eq!(report.rate_limit_reset_at, None);
        assert_eq!(
            candidate_names(&db.0, "scn-xhs"),
            vec!["gone/old".to_owned()]
        );
        assert_eq!(
            run_row(&db.0, "scn-xhs"),
            Some((
                "2026-01-01T00:00:00Z".to_owned(),
                9,
                1,
                Some("上次".to_owned())
            ))
        );
    }

    #[test]
    fn rate_limit_on_first_query_keeps_old_cache() {
        let db = ready("limit-first");
        insert_candidate(&db.0, "scn-xhs", "gone/old", 1, "[]", None);
        insert_run(&db.0, "scn-xhs", "2026-01-01T00:00:00Z", 4, 0, None);
        let fetcher = Scripted::new(vec![limited(epoch() + 45)]);
        let report = refresh_discover_with(&fetcher, &db.0, "scn-xhs", epoch()).expect("应返回");
        assert_eq!(fetcher.calls().len(), 1);
        assert_eq!(
            report.message.as_deref(),
            Some("GitHub 暂时不让查太频繁，过几分钟再点「重新找找」，已找到的先列在下面")
        );
        let reset_at = format_unix_time(epoch() + 45).expect("时间");
        assert_eq!(
            report.rate_limit_reset_at.as_deref(),
            Some(reset_at.as_str())
        );
        assert_eq!(report.queries, 1);
        assert_eq!(report.failed_queries, 1);
        assert_eq!(report.fetched, 0);
        assert_eq!(
            candidate_names(&db.0, "scn-xhs"),
            vec!["gone/old".to_owned()]
        );
        assert_eq!(run_row(&db.0, "scn-xhs").expect("记录").1, 4);
    }

    #[test]
    fn rate_limit_after_success_stops_and_writes_partial() {
        let db = ready("limit-partial");
        insert_candidate(&db.0, "scn-xhs", "gone/old", 1, "[]", None);
        let scenario = ScenarioRef {
            id: "scn-xhs".to_owned(),
            slug: "xhs".to_owned(),
            account_id: "acct".to_owned(),
        };
        let queries = vec!["q1".to_owned(), "q2".to_owned(), "q3".to_owned()];
        let fetcher = Scripted::new(vec![
            ok_body(search_body(vec![repo_value("keep/me", 6)])),
            limited(epoch() + 45),
        ]);
        let report = refresh_with_config(
            &fetcher,
            &db.0,
            &scenario,
            &queries,
            20,
            "2026-06-05",
            epoch(),
        )
        .expect("应写入已拿到的部分");
        let calls = fetcher.calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].contains("q=q1&"));
        assert!(calls[1].contains("q=q2&"));
        assert!(calls.iter().all(|url| !url.contains("q=q3&")));
        assert_eq!(
            report.message.as_deref(),
            Some("GitHub 暂时不让查太频繁，过几分钟再点「重新找找」，已找到的先列在下面")
        );
        let reset_at = format_unix_time(epoch() + 45).expect("时间");
        assert_eq!(
            report.rate_limit_reset_at.as_deref(),
            Some(reset_at.as_str())
        );
        assert_eq!(report.queries, 2);
        assert_eq!(report.failed_queries, 1);
        assert_eq!(report.fetched, 1);
        assert_eq!(
            candidate_names(&db.0, "scn-xhs"),
            vec!["keep/me".to_owned()]
        );
    }

    #[test]
    fn only_forks_still_replace_cache() {
        let db = ready("forks");
        insert_candidate(&db.0, "scn-xhs", "gone/old", 1, "[]", None);
        let mut fork = repo_value("Fork/Only", 8);
        fork["fork"] = json!(true);
        let fetcher = Scripted::new(vec![
            ok_body(search_body(vec![fork.clone()])),
            ok_body(search_body(vec![fork])),
        ]);
        let report = refresh_discover_with(&fetcher, &db.0, "scn-xhs", epoch()).expect("应成功");
        assert_eq!(report.fetched, 0);
        assert_eq!(report.skipped_mine, 0);
        assert_eq!(report.failed_queries, 0);
        assert_eq!(report.message, None);
        assert!(candidate_names(&db.0, "scn-xhs").is_empty());
        assert!(run_row(&db.0, "scn-xhs").is_some());
    }

    #[test]
    fn unknown_or_archived_scenario_does_not_fetch() {
        let db = ready("missing-scenario");
        let error = refresh_discover_with(&PanicFetcher, &db.0, "scn-nope", epoch())
            .expect_err("不存在的场景");
        assert_eq!(error, "找不到这个场景");
        let slug = refresh_discover_with(&PanicFetcher, &db.0, "xhs", epoch()).expect_err("slug");
        assert_eq!(slug, "找不到这个场景");
        let connection = crate::v2_ops::open_db(&db.0).expect("开库");
        connection
            .execute("UPDATE scenarios SET archived = 1 WHERE id = 'scn-xhs'", [])
            .expect("归档");
        drop(connection);
        let archived =
            refresh_discover_with(&PanicFetcher, &db.0, "scn-xhs", epoch()).expect_err("已归档");
        assert_eq!(archived, "找不到这个场景");
    }

    #[test]
    fn scenario_without_queries_does_not_fetch() {
        let db = ready("no-queries");
        insert_user_scenario(&db.0, "scn-custom", "custom", "自定义");
        insert_candidate(&db.0, "scn-custom", "keep/me", 1, "[]", None);
        let report =
            refresh_discover_with(&PanicFetcher, &db.0, "scn-custom", epoch()).expect("应返回");
        assert_eq!(report.message.as_deref(), Some("这个场景还没有配置搜索词"));
        assert_eq!(report.queries, 0);
        assert_eq!(report.fetched, 0);
        assert_eq!(
            candidate_names(&db.0, "scn-custom"),
            vec!["keep/me".to_owned()]
        );
    }

    #[test]
    fn missing_account_is_an_error() {
        let db = temp_db("no-account");
        let refresh =
            refresh_discover_with(&PanicFetcher, &db.0, "scn-xhs", epoch()).expect_err("刷新");
        assert_eq!(refresh, "还没有 GitHub 账号");
        let list = list_discover_view(&db.0, None).expect_err("列表");
        assert_eq!(list, "还没有 GitHub 账号");
    }

    #[test]
    fn list_does_not_seed() {
        let db = temp_db("no-seed");
        insert_test_account(&db.0);
        let view = list_discover_view(&db.0, None).expect("应能列出");
        assert!(view.scenarios.is_empty());
        assert_eq!(view.selected, None);
        assert!(view.candidates.is_empty());
        assert_eq!(count_scenarios(&db.0), 0);
    }

    #[test]
    fn list_defaults_to_first_thin() {
        let db = ready("thin");
        let view = list_discover_view(&db.0, None).expect("列表");
        assert_eq!(view.scenarios[0].id, "scn-xhs");
        assert!(!view.scenarios[0].thin);
        let data = view
            .scenarios
            .iter()
            .find(|item| item.slug == "data")
            .expect("data");
        assert!(data.thin);
        assert_eq!(data.my_count, 0);
        assert_eq!(view.selected.as_deref(), Some("scn-data"));
    }

    #[test]
    fn list_defaults_to_first_when_none_thin() {
        let db = ready("none-thin");
        for index in 0..15 {
            let id = format!("r{index}");
            insert_repo(&db.0, "acct", &id, &format!("fox/r{index}"), "active");
            for slug in ["data", "ec", "seo"] {
                insert_link(&db.0, &id, &format!("scn-{slug}"));
            }
        }
        let view = list_discover_view(&db.0, None).expect("列表");
        let data = view
            .scenarios
            .iter()
            .find(|item| item.slug == "data")
            .expect("data");
        assert_eq!(data.my_count, 15);
        assert!(!data.thin);
        assert!(view.scenarios.iter().all(|item| !item.thin));
        assert_eq!(view.selected.as_deref(), Some("scn-xhs"));
    }

    #[test]
    fn list_uses_explicit_id_and_falls_back() {
        let db = ready("explicit");
        let seo = list_discover_view(&db.0, Some("scn-seo")).expect("seo");
        assert_eq!(seo.selected.as_deref(), Some("scn-seo"));
        let xhs = list_discover_view(&db.0, Some("scn-xhs")).expect("xhs");
        assert_eq!(xhs.selected.as_deref(), Some("scn-xhs"));
        let fallback = list_discover_view(&db.0, Some("missing")).expect("回退");
        assert_eq!(fallback.selected.as_deref(), Some("scn-data"));
    }

    #[test]
    fn mine_is_case_insensitive_and_ignores_removed() {
        let db = ready("mine");
        insert_named_test_account(&db.0, "other", "other");
        insert_repo(&db.0, "acct", "mine", "fox/mine", "active");
        insert_repo(&db.0, "acct", "gone", "fox/gone", "removed");
        insert_repo(&db.0, "other", "other-repo", "fox/other", "active");
        insert_candidate(&db.0, "scn-xhs", "Fox/Mine", 3, "[]", None);
        insert_candidate(&db.0, "scn-xhs", "fox/gone", 2, "[]", None);
        insert_candidate(&db.0, "scn-xhs", "fox/other", 1, "[]", None);
        let view = list_discover_view(&db.0, Some("scn-xhs")).expect("列表");
        let flags: Vec<(&str, bool)> = view
            .candidates
            .iter()
            .map(|item| (item.full_name.as_str(), item.mine))
            .collect();
        assert_eq!(
            flags,
            vec![
                ("Fox/Mine", true),
                ("fox/gone", false),
                ("fox/other", false)
            ]
        );
    }

    #[test]
    fn inbox_counts_open_items_not_archived() {
        let db = ready("inbox");
        insert_inbox(
            &db.0,
            "pending",
            "https://github.com/octocat/hello",
            "pending",
        );
        insert_inbox(&db.0, "ready", "https://github.com/octocat/ready", "ready");
        insert_inbox(
            &db.0,
            "failed",
            "https://github.com/octocat/failed",
            "failed",
        );
        insert_inbox(
            &db.0,
            "archived",
            "https://github.com/octocat/old",
            "archived",
        );
        insert_candidate(
            &db.0,
            "scn-xhs",
            "OctoCat/Hello",
            4,
            "[]",
            Some("https://GitHub.com/OctoCat/Hello"),
        );
        insert_candidate(
            &db.0,
            "scn-xhs",
            "octocat/ready",
            3,
            "[]",
            Some("https://github.com/octocat/ready/issues/1"),
        );
        insert_candidate(&db.0, "scn-xhs", "octocat/failed", 2, "[]", None);
        insert_candidate(&db.0, "scn-xhs", "octocat/old", 1, "[]", None);
        let view = list_discover_view(&db.0, Some("scn-xhs")).expect("列表");
        let flags: Vec<(&str, bool)> = view
            .candidates
            .iter()
            .map(|item| (item.full_name.as_str(), item.in_inbox))
            .collect();
        assert_eq!(
            flags,
            vec![
                ("OctoCat/Hello", true),
                ("octocat/ready", true),
                ("octocat/failed", true),
                ("octocat/old", false)
            ]
        );
    }

    #[test]
    fn candidate_count_excludes_mine() {
        let db = ready("count");
        insert_repo(&db.0, "acct", "mine", "fox/mine", "active");
        insert_candidate(&db.0, "scn-xhs", "Fox/Mine", 100, "[\"cli\"]", None);
        insert_candidate(&db.0, "scn-xhs", "a/mid", 50, "[]", None);
        insert_candidate(&db.0, "scn-xhs", "b/low", 10, "[]", None);
        let view = list_discover_view(&db.0, Some("scn-xhs")).expect("列表");
        let xhs = view
            .scenarios
            .iter()
            .find(|item| item.id == "scn-xhs")
            .expect("xhs");
        assert_eq!(xhs.candidate_count, 2);
        assert_eq!(view.candidates.len(), 3);
        assert_eq!(
            view.candidates
                .iter()
                .map(|item| item.full_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Fox/Mine", "a/mid", "b/low"]
        );
        assert!(view.candidates[0].mine);
        assert_eq!(view.candidates[0].topics, vec!["cli".to_owned()]);
    }

    #[test]
    fn bad_topics_json_becomes_empty() {
        let db = ready("bad-topics");
        insert_candidate(&db.0, "scn-xhs", "a/a", 1, "not-json", None);
        let view = list_discover_view(&db.0, Some("scn-xhs")).expect("坏话题不应让列表失败");
        assert_eq!(view.candidates.len(), 1);
        assert!(view.candidates[0].topics.is_empty());
    }

    #[test]
    fn discover_contract_keys_include_nulls() {
        let db = ready("contract");
        insert_candidate(&db.0, "scn-xhs", "a/nulls", 1, "[]", None);
        let view = list_discover_view(&db.0, Some("scn-xhs")).expect("列表");
        let view_json = serde_json::to_value(&view).expect("序列化");
        expect_keys(&view_json, &["scenarios", "selected", "candidates"]);
        expect_keys(
            &view_json["scenarios"][0],
            &[
                "id",
                "slug",
                "nameZh",
                "sortOrder",
                "myCount",
                "thin",
                "candidateCount",
                "lastFetchedAt",
            ],
        );
        assert!(view_json["scenarios"][0]["lastFetchedAt"].is_null());
        expect_keys(
            &view_json["candidates"][0],
            &[
                "fullName",
                "htmlUrl",
                "description",
                "language",
                "starsCount",
                "pushedAt",
                "topics",
                "mine",
                "inInbox",
                "fetchedAt",
            ],
        );
        assert!(view_json["candidates"][0]["description"].is_null());
        assert!(view_json["candidates"][0]["language"].is_null());
        assert!(view_json["candidates"][0]["pushedAt"].is_null());
        let fetcher = Scripted::new(vec![
            ok_body(search_body(vec![])),
            ok_body(search_body(vec![])),
        ]);
        let report =
            refresh_discover_with(&fetcher, &db.0, "scn-seo", epoch()).expect("空结果也成功");
        let report_json = serde_json::to_value(&report).expect("序列化");
        expect_keys(
            &report_json,
            &[
                "scenarioId",
                "queries",
                "failedQueries",
                "fetched",
                "skippedMine",
                "message",
                "rateLimitResetAt",
            ],
        );
        assert!(report_json["message"].is_null());
        assert!(report_json["rateLimitResetAt"].is_null());
    }

    #[test]
    fn discover_commands_are_registered_and_refresh_spawns_blocking() {
        let permissions = include_str!("../permissions/gsat-commands.toml");
        let manifest = include_str!("../gen/schemas/acl-manifests.json");
        let source = include_str!("lib.rs");
        let start = source
            .find("generate_handler![")
            .expect("应有 generate_handler");
        let end = source[start..].find("])").expect("generate_handler 应结束");
        let block = &source[start..start + end];
        for command in ["list_discover", "refresh_discover", "get_repo_community"] {
            assert!(
                permissions.contains(&format!("\"{command}\"")),
                "{command} 应在权限清单"
            );
            assert!(
                manifest.contains(&format!("\"{command}\"")),
                "{command} 应在 ACL"
            );
            assert!(block.contains(command), "{command} 应登记");
        }
        let refresh = command_body(source, "async fn refresh_discover(");
        assert!(refresh.contains("async fn refresh_discover("));
        assert!(refresh.contains("spawn_blocking"));
        let community = command_body(source, "async fn get_repo_community(");
        assert!(community.contains("async fn get_repo_community("));
        assert!(community.contains("spawn_blocking"));
        let list = command_body(source, "fn list_discover(");
        let list_fn = list.split("\n///").next().unwrap_or(list);
        assert!(list_fn.starts_with("fn list_discover("));
        assert!(!list_fn.contains("spawn_blocking"));
    }

    #[test]
    fn rate_limit_message_does_not_carry_the_wait() {
        // 等 3 秒和等 15 分钟说的是同一句；秒数不写给用户，只记在 rate_limit_reset_at 里。
        let mut messages = Vec::new();
        for (label, wait) in [("limit-short", 3_i64), ("limit-long", 900_i64)] {
            let db = ready(label);
            let fetcher = Scripted::new(vec![limited(epoch() + wait)]);
            let report =
                refresh_discover_with(&fetcher, &db.0, "scn-xhs", epoch()).expect("应返回");
            let text = report.message.clone().expect("有提示");
            assert!(
                !text.chars().any(|ch| ch.is_ascii_digit()),
                "提示里不该有等待的数字：{text}"
            );
            assert!(!text.contains('秒'), "不要把还要等几秒写进提示：{text}");
            assert!(!text.contains("限流"), "不要用「限流」这个词：{text}");
            let reset_at = format_unix_time(epoch() + wait).expect("时间");
            assert_eq!(
                report.rate_limit_reset_at.as_deref(),
                Some(reset_at.as_str())
            );
            messages.push(text);
        }
        assert_eq!(messages[0], messages[1]);
        assert_eq!(
            messages[0],
            "GitHub 暂时不让查太频繁，过几分钟再点「重新找找」，已找到的先列在下面"
        );
    }

    #[test]
    fn mine_lookup_errors_say_tool_not_repository() {
        // 读「已收藏」名单失败时，报错里写「工具」，不写「仓库」。两个失败点：库里没有表，和名字是空的。
        let empty = Connection::open_in_memory().expect("内存库");
        let no_table = active_names(&empty, "acct").expect_err("没有表应失败");
        assert!(no_table.starts_with("已收藏工具读取失败："), "{no_table}");
        assert!(!no_table.contains("仓库"), "{no_table}");

        let broken = Connection::open_in_memory().expect("内存库");
        broken
            .execute_batch(
                "CREATE TABLE repositories(account_id TEXT, full_name TEXT, sync_status TEXT);
                 INSERT INTO repositories VALUES ('acct', NULL, 'active');",
            )
            .expect("应能建表");
        let no_name = active_names(&broken, "acct").expect_err("名字是空的应失败");
        assert!(no_name.starts_with("已收藏工具行读取失败："), "{no_name}");
        assert!(!no_name.contains("仓库"), "{no_name}");
    }

    #[test]
    fn list_tells_fetched_without_results_from_never_fetched() {
        // 界面据此分「找过了，这次没有结果」和「还没找过」：两个场景候选数都是 0，区别只在 last_fetched_at。
        let db = ready("fetched-empty");
        let fetcher = Scripted::new(vec![
            ok_body(search_body(vec![])),
            ok_body(search_body(vec![])),
        ]);
        let report =
            refresh_discover_with(&fetcher, &db.0, "scn-seo", epoch()).expect("空结果也成功");
        assert_eq!(report.fetched, 0);
        assert_eq!(report.message, None);
        let view = list_discover_view(&db.0, None).expect("列表");
        let fetched = view
            .scenarios
            .iter()
            .find(|item| item.id == "scn-seo")
            .expect("seo");
        let never = view
            .scenarios
            .iter()
            .find(|item| item.id == "scn-ec")
            .expect("ec");
        assert_eq!(fetched.candidate_count, 0);
        assert_eq!(
            fetched.last_fetched_at.as_deref(),
            Some(format_unix_time(epoch()).expect("时间").as_str())
        );
        assert_eq!(never.candidate_count, 0);
        assert_eq!(never.last_fetched_at, None);
    }
}
