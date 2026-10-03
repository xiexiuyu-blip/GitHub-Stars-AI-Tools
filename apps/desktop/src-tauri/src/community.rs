//! 社区：Hacker News 讨论和 GitHub 上评论最多的 Issues。平时只读缓存。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::web_fetch::{
    format_unix_time, parse_unix_time, rate_limit_wait_secs, url_encode, FetchError,
    FetchErrorKind, HttpFetcher,
};

const SOURCE_HN: &str = "hn";
const SOURCE_ISSUE: &str = "github_issue";
const FRESH_SECS: i64 = 24 * 60 * 60;
const GITHUB_HEADERS: &[(&str, &str)] = &[
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", "2022-11-28"),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityItem {
    pub source: String,
    pub title: String,
    pub url: String,
    pub points: Option<i64>,
    pub comments: Option<i64>,
    pub created_at: Option<String>,
    pub state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityError {
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoCommunity {
    pub repo_id: String,
    pub fetched_at: Option<String>,
    pub stale: bool,
    pub items: Vec<CommunityItem>,
    pub errors: Vec<CommunityError>,
}

struct CacheRow {
    payload_json: String,
    fetched_at: String,
    error: Option<String>,
}

fn open_db(path: &Path) -> Result<Connection, String> {
    crate::v2_ops::open_db(path)
}

fn lookup_repo(path: &Path, repo_id: &str) -> Result<String, String> {
    let connection = open_db(path)?;
    connection
        .query_row(
            "SELECT full_name FROM repositories WHERE id = ?1",
            params![repo_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("仓库读取失败：{error}"))?
        .ok_or_else(|| "找不到这个仓库".to_owned())
}

fn split_repo(full_name: &str) -> Result<(String, String), String> {
    let Some((owner, name)) = full_name.split_once('/') else {
        return Err("这个仓库的名字不完整".to_owned());
    };
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return Err("这个仓库的名字不完整".to_owned());
    }
    Ok((owner.to_owned(), name.to_owned()))
}

fn hn_search_url(owner: &str, repo: &str) -> String {
    let query = url_encode(&format!("github.com/{owner}/{repo}"));
    format!(
        "https://hn.algolia.com/api/v1/search?query={query}&restrictSearchableAttributes=url&tags=story&hitsPerPage=5"
    )
}

fn issue_search_url(owner: &str, repo: &str) -> String {
    let query = url_encode(&format!("repo:{owner}/{repo} is:issue"));
    format!("https://api.github.com/search/issues?q={query}&sort=comments&order=desc&per_page=5")
}

fn text_opt(item: &Value, key: &str) -> Option<String> {
    match item.get(key) {
        Some(Value::String(text)) if !text.is_empty() => Some(text.clone()),
        _ => None,
    }
}

fn json_i64(value: &Value) -> Option<i64> {
    value.as_i64()
}

fn object_id(item: &Value) -> Option<String> {
    match item.get("objectID") {
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                Some(text.to_owned())
            }
        }
        Some(Value::Number(number)) => Some(number.to_string()),
        _ => None,
    }
}

fn parse_hn_hits(body: &str) -> Result<Vec<CommunityItem>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "结果读不出来".to_owned())?;
    let hits = value
        .get("hits")
        .and_then(Value::as_array)
        .ok_or_else(|| "结果读不出来".to_owned())?;
    let mut items = Vec::new();
    for hit in hits {
        let Some(title) = hit
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
        else {
            continue;
        };
        let Some(object_id) = object_id(hit) else {
            continue;
        };
        items.push(CommunityItem {
            source: SOURCE_HN.to_owned(),
            title: title.to_owned(),
            url: format!("https://news.ycombinator.com/item?id={object_id}"),
            points: hit.get("points").and_then(json_i64),
            comments: hit.get("num_comments").and_then(json_i64),
            created_at: text_opt(hit, "created_at"),
            state: None,
        });
    }
    Ok(items)
}

fn parse_issue_items(body: &str) -> Result<Vec<CommunityItem>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "结果读不出来".to_owned())?;
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| "结果读不出来".to_owned())?;
    let mut parsed = Vec::new();
    for item in items {
        let Some(title) = item
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
        else {
            continue;
        };
        let Some(url) = item
            .get("html_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
        else {
            continue;
        };
        parsed.push(CommunityItem {
            source: SOURCE_ISSUE.to_owned(),
            title: title.to_owned(),
            url: url.to_owned(),
            points: None,
            comments: item.get("comments").and_then(json_i64),
            created_at: text_opt(item, "created_at"),
            state: text_opt(item, "state"),
        });
    }
    Ok(parsed)
}

fn describe_fetch(label: &str, error: &FetchError) -> String {
    let detail = match error.kind {
        FetchErrorKind::Timeout => "联网超时",
        FetchErrorKind::Network => "网络连不上",
        FetchErrorKind::Other => error.message.as_str(),
    };
    format!("{label} {detail}")
}

fn refresh_source(
    fetcher: &impl HttpFetcher,
    db_path: &Path,
    repo_id: &str,
    source: &str,
    url: &str,
    headers: &[(&str, &str)],
    now: i64,
    label: &str,
    parse: fn(&str) -> Result<Vec<CommunityItem>, String>,
) -> Result<(), String> {
    let outcome = match fetcher.get(url, headers) {
        Err(error) => Err(describe_fetch(label, &error)),
        Ok(response) => {
            if let Some(wait) = rate_limit_wait_secs(&response, now) {
                Err(format!("{label} 太频繁了，约 {wait} 秒后再试"))
            } else if response.status != 200 {
                Err(format!("{label} 暂时查不到"))
            } else {
                match parse(&response.body) {
                    Ok(items) => Ok(items),
                    Err(_) => Err(format!("{label} 的结果读不出来")),
                }
            }
        }
    };
    let mut connection = open_db(db_path)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("社区缓存事务开启失败：{error}"))?;
    match outcome {
        Ok(items) => {
            let payload =
                serde_json::to_string(&items).map_err(|_| "社区结果写不进去".to_owned())?;
            let fetched_at = format_unix_time(now)?;
            tx.execute(
                "INSERT INTO repo_community_cache(repo_id, source, payload_json, fetched_at, error) VALUES (?1, ?2, ?3, ?4, NULL) ON CONFLICT(repo_id, source) DO UPDATE SET payload_json = excluded.payload_json, fetched_at = excluded.fetched_at, error = NULL",
                params![repo_id, source, payload, fetched_at],
            )
            .map_err(|error| format!("社区缓存写入失败：{error}"))?;
        }
        Err(message) => {
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM repo_community_cache WHERE repo_id = ?1 AND source = ?2)",
                    params![repo_id, source],
                    |row| row.get(0),
                )
                .map_err(|error| format!("社区缓存读取失败：{error}"))?;
            if exists {
                tx.execute(
                    "UPDATE repo_community_cache SET error = ?3 WHERE repo_id = ?1 AND source = ?2",
                    params![repo_id, source, message],
                )
                .map_err(|error| format!("社区错误写入失败：{error}"))?;
            } else {
                let fetched_at = format_unix_time(now)?;
                tx.execute(
                    "INSERT INTO repo_community_cache(repo_id, source, payload_json, fetched_at, error) VALUES (?1, ?2, '[]', ?3, ?4)",
                    params![repo_id, source, fetched_at, message],
                )
                .map_err(|error| format!("社区错误写入失败：{error}"))?;
            }
        }
    }
    tx.commit()
        .map_err(|error| format!("社区缓存提交失败：{error}"))?;
    Ok(())
}

fn load_rows(
    connection: &Connection,
    repo_id: &str,
) -> Result<(Option<CacheRow>, Option<CacheRow>), String> {
    let mut statement = connection
        .prepare(
            "SELECT source, payload_json, fetched_at, error FROM repo_community_cache WHERE repo_id = ?1 AND source IN ('hn', 'github_issue')",
        )
        .map_err(|error| format!("社区缓存读取失败：{error}"))?;
    let rows = statement
        .query_map(params![repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CacheRow {
                    payload_json: row.get(1)?,
                    fetched_at: row.get(2)?,
                    error: row.get(3)?,
                },
            ))
        })
        .map_err(|error| format!("社区缓存查询失败：{error}"))?;
    let mut hn = None;
    let mut issues = None;
    for row in rows {
        let (source, cache) = row.map_err(|error| format!("社区缓存行读取失败：{error}"))?;
        if source == SOURCE_HN {
            hn = Some(cache);
        } else if source == SOURCE_ISSUE {
            issues = Some(cache);
        }
    }
    Ok((hn, issues))
}

fn items_of(
    row: Option<&CacheRow>,
    source: &str,
    errors: &mut Vec<CommunityError>,
) -> Vec<CommunityItem> {
    let Some(row) = row else {
        return Vec::new();
    };
    if let Some(message) = &row.error {
        errors.push(CommunityError {
            source: source.to_owned(),
            message: message.clone(),
        });
    }
    match serde_json::from_str::<Vec<CommunityItem>>(&row.payload_json) {
        Ok(items) => items,
        Err(_) => {
            if row.error.is_none() {
                errors.push(CommunityError {
                    source: source.to_owned(),
                    message: "缓存读不出来".to_owned(),
                });
            }
            Vec::new()
        }
    }
}

fn sort_by_score(items: &mut [CommunityItem], by_points: bool) {
    items.sort_by(|left, right| {
        let left_score = if by_points {
            left.points
        } else {
            left.comments
        };
        let right_score = if by_points {
            right.points
        } else {
            right.comments
        };
        right_score
            .unwrap_or(i64::MIN)
            .cmp(&left_score.unwrap_or(i64::MIN))
            .then_with(|| left.url.cmp(&right.url))
    });
}

fn source_is_stale(row: Option<&CacheRow>, now: i64) -> bool {
    let Some(row) = row else {
        return true;
    };
    match parse_unix_time(&row.fetched_at) {
        Some(fetched) => fetched < now.saturating_sub(FRESH_SECS),
        None => true,
    }
}

fn earlier_fetched_at(left: Option<&CacheRow>, right: Option<&CacheRow>) -> Option<String> {
    match (left, right) {
        (None, None) => None,
        (Some(row), None) | (None, Some(row)) => Some(row.fetched_at.clone()),
        (Some(left), Some(right)) => match (
            parse_unix_time(&left.fetched_at),
            parse_unix_time(&right.fetched_at),
        ) {
            (Some(left_epoch), Some(right_epoch)) => {
                if left_epoch <= right_epoch {
                    Some(left.fetched_at.clone())
                } else {
                    Some(right.fetched_at.clone())
                }
            }
            (Some(_), None) => Some(left.fetched_at.clone()),
            (None, Some(_)) => Some(right.fetched_at.clone()),
            (None, None) => {
                if left.fetched_at <= right.fetched_at {
                    Some(left.fetched_at.clone())
                } else {
                    Some(right.fetched_at.clone())
                }
            }
        },
    }
}

fn read_community(db_path: &Path, repo_id: &str, now: i64) -> Result<RepoCommunity, String> {
    let connection = open_db(db_path)?;
    let (hn, issues) = load_rows(&connection, repo_id)?;
    let stale = source_is_stale(hn.as_ref(), now) || source_is_stale(issues.as_ref(), now);
    let mut errors = Vec::new();
    let mut hn_items = items_of(hn.as_ref(), SOURCE_HN, &mut errors);
    let mut issue_items = items_of(issues.as_ref(), SOURCE_ISSUE, &mut errors);
    sort_by_score(&mut hn_items, true);
    sort_by_score(&mut issue_items, false);
    hn_items.append(&mut issue_items);
    Ok(RepoCommunity {
        repo_id: repo_id.to_owned(),
        fetched_at: earlier_fetched_at(hn.as_ref(), issues.as_ref()),
        stale,
        items: hn_items,
        errors,
    })
}

pub fn get_repo_community_with(
    fetcher: &impl HttpFetcher,
    db_path: &Path,
    repo_id: &str,
    refresh: bool,
    now: i64,
) -> Result<RepoCommunity, String> {
    let full_name = lookup_repo(db_path, repo_id)?;
    let (owner, name) = split_repo(&full_name)?;
    if refresh {
        refresh_source(
            fetcher,
            db_path,
            repo_id,
            SOURCE_HN,
            &hn_search_url(&owner, &name),
            &[],
            now,
            "Hacker News",
            parse_hn_hits,
        )?;
        refresh_source(
            fetcher,
            db_path,
            repo_id,
            SOURCE_ISSUE,
            &issue_search_url(&owner, &name),
            GITHUB_HEADERS,
            now,
            "GitHub Issues",
            parse_issue_items,
        )?;
    }
    read_community(db_path, repo_id, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::{insert_test_account, migrate_database};
    use crate::web_fetch::{format_unix_time, FetchError, FetchErrorKind, HttpResponse};
    use rusqlite::params;
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
        calls: Mutex<Vec<(String, Vec<(String, String)>)>>,
    }

    impl Scripted {
        fn new(responses: Vec<Result<HttpResponse, FetchError>>) -> Self {
            Self {
                responses: Mutex::new(responses),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(String, Vec<(String, String)>)> {
            self.calls.lock().expect("锁").clone()
        }
    }

    impl HttpFetcher for Scripted {
        fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, FetchError> {
            if url.starts_with("https://api.github.com/") {
                assert_eq!(
                    headers,
                    [
                        ("Accept", "application/vnd.github+json"),
                        ("X-GitHub-Api-Version", "2022-11-28"),
                    ]
                );
            } else {
                assert!(headers.is_empty(), "{url} 不应带头");
            }
            self.calls.lock().expect("锁").push((
                url.to_owned(),
                headers
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                    .collect(),
            ));
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

    fn temp_db(label: &str) -> TempDb {
        let seq = TEMP_DB_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时间应可用")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("fsl-community-{label}-{seq}-{nanos}.sqlite3"));
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
        db
    }

    fn insert_repo(path: &Path, id: &str, full_name: &str) {
        let (owner, name) = match full_name.split_once('/') {
            Some((owner, name)) => (owner, name),
            None => (full_name, full_name),
        };
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO repositories(id, account_id, owner, name, full_name, description, topics_json, html_url, sync_status, starred_at) VALUES (?1, 'acct', ?2, ?3, ?4, '', '[]', ?5, 'active', '2026-09-01T00:00:00Z')",
                params![id, owner, name, full_name, format!("https://github.com/{full_name}")],
            )
            .expect("应能写入仓库");
    }

    fn insert_cache(
        path: &Path,
        repo_id: &str,
        source: &str,
        payload: &str,
        fetched_at: &str,
        error: Option<&str>,
    ) {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO repo_community_cache(repo_id, source, payload_json, fetched_at, error) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![repo_id, source, payload, fetched_at, error],
            )
            .expect("应能写入社区缓存");
    }

    fn cache_row(path: &Path, repo_id: &str, source: &str) -> (String, String, Option<String>) {
        let connection = crate::v2_ops::open_db(path).expect("应能打开测试库");
        connection
            .query_row(
                "SELECT payload_json, fetched_at, error FROM repo_community_cache WHERE repo_id = ?1 AND source = ?2",
                params![repo_id, source],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("应能读取社区缓存")
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

    fn hn_item(title: &str, id: &str, points: Option<i64>) -> CommunityItem {
        CommunityItem {
            source: "hn".to_owned(),
            title: title.to_owned(),
            url: format!("https://news.ycombinator.com/item?id={id}"),
            points,
            comments: Some(1),
            created_at: None,
            state: None,
        }
    }

    #[test]
    fn parse_hn_builds_item_url_and_drops_blanks() {
        let body = r#"{
            "hits": [
                {"title":" 有讨论 ","objectID":" 7 ","points":12,"num_comments":3,"created_at":"2026-08-01T00:00:00Z","url":"https://example.com/story"},
                {"title":"   ","objectID":"8","points":1},
                {"title":"没有编号","points":9,"url":"https://example.com/nope"},
                {"title":"数字编号","objectID":42,"points":null,"num_comments":null}
            ]
        }"#;
        let items = parse_hn_hits(body).expect("应能解析");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "有讨论");
        assert_eq!(items[0].url, "https://news.ycombinator.com/item?id=7");
        assert_eq!(items[0].points, Some(12));
        assert_eq!(items[0].comments, Some(3));
        assert_eq!(items[0].state, None);
        assert_eq!(items[0].source, "hn");
        assert_ne!(items[0].url, "https://example.com/story");
        assert_eq!(items[1].url, "https://news.ycombinator.com/item?id=42");
        assert_eq!(items[1].points, None);
        assert_eq!(items[1].comments, None);
        assert!(parse_hn_hits(r#"{"hits":[]}"#).expect("空").is_empty());
    }

    #[test]
    fn parse_issues_keeps_state_and_drops_incomplete() {
        let body = r#"{
            "items": [
                {"title":" 打不开 ","html_url":" https://github.com/o/r/issues/9 ","comments":4,"created_at":"2026-08-02T00:00:00Z","state":"open"},
                {"title":"  ","html_url":"https://github.com/o/r/issues/1"},
                {"title":"没有链接","comments":8},
                {"title":"已关闭","html_url":"https://github.com/o/r/issues/2","state":"closed"}
            ]
        }"#;
        let items = parse_issue_items(body).expect("应能解析");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "打不开");
        assert_eq!(items[0].url, "https://github.com/o/r/issues/9");
        assert_eq!(items[0].points, None);
        assert_eq!(items[0].comments, Some(4));
        assert_eq!(items[0].state.as_deref(), Some("open"));
        assert_eq!(items[0].source, "github_issue");
        assert_eq!(items[1].comments, None);
        assert_eq!(items[1].state.as_deref(), Some("closed"));
        assert!(parse_issue_items(r#"{"items":[]}"#).expect("空").is_empty());
    }

    #[test]
    fn missing_collection_key_is_an_error() {
        assert!(parse_hn_hits(r#"{"nbHits":0}"#).is_err());
        assert!(parse_hn_hits("nope").is_err());
        assert!(parse_issue_items(r#"{"total_count":1}"#).is_err());
        assert!(parse_issue_items("{}").is_err());
    }

    #[test]
    fn refresh_false_does_not_fetch() {
        let db = ready("read");
        insert_repo(&db.0, "repo", "octo/hello");
        let now = format_unix_time(epoch()).expect("时间");
        let payload = serde_json::to_string(&vec![hn_item("旧帖", "1", Some(3))]).expect("json");
        insert_cache(&db.0, "repo", "hn", &payload, &now, None);
        insert_cache(&db.0, "repo", "github_issue", "[]", &now, None);
        let view =
            get_repo_community_with(&PanicFetcher, &db.0, "repo", false, epoch()).expect("只读");
        assert!(!view.stale);
        assert_eq!(view.items.len(), 1);
        assert_eq!(view.items[0].title, "旧帖");
        assert!(view.errors.is_empty());
    }

    #[test]
    fn stale_when_missing_expired_or_unparseable() {
        let db = ready("stale");
        insert_repo(&db.0, "none", "octo/none");
        insert_repo(&db.0, "one", "octo/one");
        insert_repo(&db.0, "old", "octo/old");
        insert_repo(&db.0, "bad", "octo/bad");
        let fresh = format_unix_time(epoch()).expect("时间");
        let expired = format_unix_time(epoch() - 86_401).expect("时间");
        let payload = serde_json::to_string(&vec![hn_item("还在", "1", Some(2))]).expect("json");
        insert_cache(&db.0, "one", "hn", &payload, &fresh, None);
        insert_cache(&db.0, "old", "hn", &payload, &expired, None);
        insert_cache(&db.0, "old", "github_issue", "[]", &expired, None);
        insert_cache(&db.0, "bad", "hn", &payload, "yesterday", None);
        insert_cache(&db.0, "bad", "github_issue", "[]", &fresh, None);

        let none =
            get_repo_community_with(&PanicFetcher, &db.0, "none", false, epoch()).expect("无缓存");
        assert!(none.stale);
        assert_eq!(none.fetched_at, None);
        assert!(none.items.is_empty());

        let one = get_repo_community_with(&PanicFetcher, &db.0, "one", false, epoch())
            .expect("缺一个来源");
        assert!(one.stale);
        assert_eq!(one.fetched_at.as_deref(), Some(fresh.as_str()));
        assert_eq!(one.items.len(), 1);

        let old =
            get_repo_community_with(&PanicFetcher, &db.0, "old", false, epoch()).expect("过期");
        assert!(old.stale);
        assert_eq!(old.items.len(), 1);

        let bad =
            get_repo_community_with(&PanicFetcher, &db.0, "bad", false, epoch()).expect("坏时间");
        assert!(bad.stale);
    }

    #[test]
    fn exactly_one_day_old_is_fresh() {
        let db = ready("boundary");
        insert_repo(&db.0, "repo", "octo/hello");
        let stamp = format_unix_time(epoch() - 86_400).expect("时间");
        insert_cache(&db.0, "repo", "hn", "[]", &stamp, None);
        insert_cache(&db.0, "repo", "github_issue", "[]", &stamp, None);
        let view =
            get_repo_community_with(&PanicFetcher, &db.0, "repo", false, epoch()).expect("边界");
        assert!(!view.stale);
        assert_eq!(view.fetched_at.as_deref(), Some(stamp.as_str()));
    }

    #[test]
    fn failure_keeps_old_payload_and_timestamp() {
        let db = ready("keep");
        insert_repo(&db.0, "repo", "octo/hello");
        let old_item = hn_item("旧帖", "1", Some(3));
        let payload = serde_json::to_string(&vec![old_item]).expect("json");
        insert_cache(&db.0, "repo", "hn", &payload, "2026-01-01T00:00:00Z", None);
        let issue = CommunityItem {
            source: "github_issue".to_owned(),
            title: "旧问题".to_owned(),
            url: "https://github.com/octo/hello/issues/1".to_owned(),
            points: None,
            comments: Some(2),
            created_at: None,
            state: Some("open".to_owned()),
        };
        let issue_payload = serde_json::to_string(&vec![issue]).expect("json");
        insert_cache(
            &db.0,
            "repo",
            "github_issue",
            &issue_payload,
            "2026-01-02T00:00:00Z",
            None,
        );
        let fetcher = Scripted::new(vec![
            network(),
            ok_body(
                json!({
                    "items": [{
                        "title": "新问题",
                        "html_url": "https://github.com/octo/hello/issues/9",
                        "comments": 5,
                        "state": "open"
                    }]
                })
                .to_string(),
            ),
        ]);
        let view =
            get_repo_community_with(&fetcher, &db.0, "repo", true, epoch()).expect("失败仍返回");
        let (hn_payload, hn_time, hn_error) = cache_row(&db.0, "repo", "hn");
        assert_eq!(hn_payload, payload);
        assert_eq!(hn_time, "2026-01-01T00:00:00Z");
        assert_eq!(hn_error.as_deref(), Some("Hacker News 网络连不上"));
        let (issue_payload_now, issue_time, issue_error) = cache_row(&db.0, "repo", "github_issue");
        assert!(issue_payload_now.contains("新问题"));
        assert_eq!(issue_time, format_unix_time(epoch()).expect("时间"));
        assert_eq!(issue_error, None);
        assert_eq!(view.items[0].title, "旧帖");
        assert_eq!(view.items[1].title, "新问题");
        assert_eq!(view.errors.len(), 1);
        assert_eq!(view.errors[0].source, "hn");
        assert!(view.stale);
    }

    #[test]
    fn rate_limit_keeps_old_issue_payload() {
        let db = ready("limit");
        insert_repo(&db.0, "repo", "octo/hello");
        let issue_payload = serde_json::to_string(&vec![CommunityItem {
            source: "github_issue".to_owned(),
            title: "旧问题".to_owned(),
            url: "https://github.com/octo/hello/issues/1".to_owned(),
            points: None,
            comments: Some(2),
            created_at: None,
            state: Some("open".to_owned()),
        }])
        .expect("json");
        insert_cache(
            &db.0,
            "repo",
            "github_issue",
            &issue_payload,
            "2026-02-01T00:00:00Z",
            None,
        );
        let fetcher = Scripted::new(vec![
            ok_body(
                json!({
                    "hits": [{"title": "新帖", "objectID": "3", "points": 4, "num_comments": 1}]
                })
                .to_string(),
            ),
            Ok(HttpResponse {
                status: 429,
                headers: vec![("retry-after".to_owned(), "12".to_owned())],
                body: "{}".to_owned(),
            }),
        ]);
        let view = get_repo_community_with(&fetcher, &db.0, "repo", true, epoch()).expect("限流");
        let (payload, fetched_at, error) = cache_row(&db.0, "repo", "github_issue");
        assert_eq!(payload, issue_payload);
        assert_eq!(fetched_at, "2026-02-01T00:00:00Z");
        assert_eq!(
            error.as_deref(),
            Some("GitHub Issues 太频繁了，约 12 秒后再试")
        );
        assert_eq!(
            view.errors[0].message,
            "GitHub Issues 太频繁了，约 12 秒后再试"
        );
        assert!(view.items.iter().any(|item| item.title == "新帖"));
        assert!(view.items.iter().any(|item| item.title == "旧问题"));
    }

    #[test]
    fn non_200_and_bad_json_use_fixed_messages() {
        let db = ready("messages");
        insert_repo(&db.0, "repo", "octo/hello");
        let fetcher = Scripted::new(vec![
            Ok(HttpResponse {
                status: 500,
                headers: Vec::new(),
                body: r#"{"hits":[{"title":"不该要","objectID":"1"}]}"#.to_owned(),
            }),
            ok_body(r#"{"total_count":1}"#.to_owned()),
        ]);
        let view = get_repo_community_with(&fetcher, &db.0, "repo", true, epoch()).expect("消息");
        assert!(view.items.is_empty());
        assert_eq!(view.errors[0].message, "Hacker News 暂时查不到");
        assert_eq!(view.errors[1].message, "GitHub Issues 的结果读不出来");
        let (hn_payload, _, _) = cache_row(&db.0, "repo", "hn");
        assert_eq!(hn_payload, "[]");
        let (issue_payload, _, _) = cache_row(&db.0, "repo", "github_issue");
        assert_eq!(issue_payload, "[]");
    }

    #[test]
    fn items_are_sorted_and_fetched_at_is_earlier() {
        let db = ready("sort");
        insert_repo(&db.0, "repo", "octo/hello");
        let hn = vec![
            hn_item("低", "2", Some(1)),
            hn_item("高", "1", Some(9)),
            hn_item("同B", "5", Some(4)),
            hn_item("同A", "4", Some(4)),
            hn_item("空", "3", None),
        ];
        let issues = vec![
            CommunityItem {
                source: "github_issue".to_owned(),
                title: "少".to_owned(),
                url: "https://github.com/octo/hello/issues/1".to_owned(),
                points: None,
                comments: Some(1),
                created_at: None,
                state: Some("open".to_owned()),
            },
            CommunityItem {
                source: "github_issue".to_owned(),
                title: "多".to_owned(),
                url: "https://github.com/octo/hello/issues/2".to_owned(),
                points: None,
                comments: Some(8),
                created_at: None,
                state: Some("closed".to_owned()),
            },
        ];
        insert_cache(
            &db.0,
            "repo",
            "hn",
            &serde_json::to_string(&hn).expect("json"),
            "2026-09-02T00:00:00Z",
            None,
        );
        insert_cache(
            &db.0,
            "repo",
            "github_issue",
            &serde_json::to_string(&issues).expect("json"),
            "2026-09-01T00:00:00Z",
            None,
        );
        let view =
            get_repo_community_with(&PanicFetcher, &db.0, "repo", false, epoch()).expect("排序");
        let titles: Vec<&str> = view.items.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["高", "同A", "同B", "低", "空", "多", "少"]);
        assert!(view.items.iter().take(5).all(|item| item.source == "hn"));
        assert!(view
            .items
            .iter()
            .skip(5)
            .all(|item| item.source == "github_issue"));
        assert_eq!(view.fetched_at.as_deref(), Some("2026-09-01T00:00:00Z"));
    }

    #[test]
    fn missing_or_incomplete_repo_does_not_fetch() {
        let db = ready("missing-repo");
        let missing = get_repo_community_with(&PanicFetcher, &db.0, "nope", true, epoch())
            .expect_err("缺仓库");
        assert_eq!(missing, "找不到这个仓库");
        insert_repo(&db.0, "plain", "noslash");
        insert_repo(&db.0, "deep", "a/b/c");
        let plain = get_repo_community_with(&PanicFetcher, &db.0, "plain", true, epoch())
            .expect_err("没斜杠");
        assert_eq!(plain, "这个仓库的名字不完整");
        let deep =
            get_repo_community_with(&PanicFetcher, &db.0, "deep", true, epoch()).expect_err("多段");
        assert_eq!(deep, "这个仓库的名字不完整");
    }

    #[test]
    fn new_failure_inserts_empty_payload() {
        let db = ready("empty-fail");
        insert_repo(&db.0, "repo", "octo/hello");
        let fetcher = Scripted::new(vec![network(), ok_body(r#"{"items":[]}"#.to_owned())]);
        let view = get_repo_community_with(&fetcher, &db.0, "repo", true, epoch()).expect("新失败");
        let (payload, fetched_at, error) = cache_row(&db.0, "repo", "hn");
        assert_eq!(payload, "[]");
        assert_eq!(fetched_at, format_unix_time(epoch()).expect("时间"));
        assert_eq!(error.as_deref(), Some("Hacker News 网络连不上"));
        let (issue_payload, _, issue_error) = cache_row(&db.0, "repo", "github_issue");
        assert_eq!(issue_payload, "[]");
        assert_eq!(issue_error, None);
        assert!(!view.stale);
        assert!(view.items.is_empty());
        assert_eq!(view.errors.len(), 1);
    }

    #[test]
    fn bad_payload_json_reports_read_error() {
        let db = ready("bad-json");
        insert_repo(&db.0, "repo", "octo/hello");
        let now = format_unix_time(epoch()).expect("时间");
        insert_cache(&db.0, "repo", "hn", "{", &now, None);
        let issue = serde_json::to_string(&vec![CommunityItem {
            source: "github_issue".to_owned(),
            title: "还在".to_owned(),
            url: "https://github.com/octo/hello/issues/1".to_owned(),
            points: None,
            comments: Some(1),
            created_at: None,
            state: Some("open".to_owned()),
        }])
        .expect("json");
        insert_cache(&db.0, "repo", "github_issue", &issue, &now, None);
        let view =
            get_repo_community_with(&PanicFetcher, &db.0, "repo", false, epoch()).expect("坏缓存");
        assert_eq!(view.errors.len(), 1);
        assert_eq!(view.errors[0].message, "缓存读不出来");
        assert_eq!(view.items.len(), 1);
        assert_eq!(view.items[0].title, "还在");
    }

    #[test]
    fn refresh_requests_the_documented_urls() {
        let db = ready("urls");
        insert_repo(&db.0, "repo", "Octo/Hello");
        let fetcher = Scripted::new(vec![
            ok_body(r#"{"hits":[]}"#.to_owned()),
            ok_body(r#"{"items":[]}"#.to_owned()),
        ]);
        let view = get_repo_community_with(&fetcher, &db.0, "repo", true, epoch()).expect("刷新");
        let calls = fetcher.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0].0,
            "https://hn.algolia.com/api/v1/search?query=github.com%2FOcto%2FHello&restrictSearchableAttributes=url&tags=story&hitsPerPage=5"
        );
        assert!(calls[0].1.is_empty());
        assert_eq!(
            calls[1].0,
            "https://api.github.com/search/issues?q=repo%3AOcto%2FHello%20is%3Aissue&sort=comments&order=desc&per_page=5"
        );
        assert_eq!(calls[1].1.len(), 2);
        assert!(!view.stale);
        let fetched_at = format_unix_time(epoch()).expect("时间");
        assert_eq!(view.fetched_at.as_deref(), Some(fetched_at.as_str()));
    }

    #[test]
    fn community_contract_keys_include_nulls() {
        let hn = parse_hn_hits(
            r#"{"hits":[{"title":"帖子","objectID":"1","points":2,"num_comments":1,"created_at":null}]}"#,
        )
        .expect("hn");
        let hn_json = serde_json::to_value(&hn[0]).expect("序列化");
        expect_keys(
            &hn_json,
            &[
                "source",
                "title",
                "url",
                "points",
                "comments",
                "createdAt",
                "state",
            ],
        );
        assert!(hn_json["state"].is_null());
        assert!(hn_json["createdAt"].is_null());
        let issues = parse_issue_items(
            r#"{"items":[{"title":"问题","html_url":"https://github.com/o/r/issues/1","comments":null,"state":"open"}]}"#,
        )
        .expect("issue");
        let issue_json = serde_json::to_value(&issues[0]).expect("序列化");
        expect_keys(
            &issue_json,
            &[
                "source",
                "title",
                "url",
                "points",
                "comments",
                "createdAt",
                "state",
            ],
        );
        assert!(issue_json["points"].is_null());
        assert!(issue_json["comments"].is_null());
        assert!(issue_json["createdAt"].is_null());

        let db = ready("contract");
        insert_repo(&db.0, "repo", "octo/hello");
        let empty =
            get_repo_community_with(&PanicFetcher, &db.0, "repo", false, epoch()).expect("空缓存");
        let empty_json = serde_json::to_value(&empty).expect("序列化");
        expect_keys(
            &empty_json,
            &["repoId", "fetchedAt", "stale", "items", "errors"],
        );
        assert!(empty_json["fetchedAt"].is_null());
        assert!(empty_json["stale"].as_bool().expect("bool"));
        let fetcher = Scripted::new(vec![network(), ok_body(r#"{"items":[]}"#.to_owned())]);
        let failed =
            get_repo_community_with(&fetcher, &db.0, "repo", true, epoch()).expect("错误对象");
        let error_json = serde_json::to_value(&failed.errors[0]).expect("序列化");
        expect_keys(&error_json, &["source", "message"]);
        assert_eq!(error_json["source"], "hn");
    }
}
