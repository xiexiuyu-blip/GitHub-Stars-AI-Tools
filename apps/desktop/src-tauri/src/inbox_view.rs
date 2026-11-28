//! 收集箱的入库前预览、列表、状态和备注。
//!
//! 预览只读，不写库。每条链接的状态和 `v2_ops::commit_capture` 用同一套顺序：
//! 先看有没有被拦原因，再看是不是已收藏的仓库，再看收集箱里有没有同一条，最后才是新收。

use crate::capture::{
    extract_links_from_text, normalized_x_post_id, ExtractedLink,
    SOURCE_TEXT_LIMIT_BYTES as TEXT_LIMIT_BYTES,
};
use crate::v2_ops::{account_id, capture_link_status, CaptureLinkStatus};
use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

const NOTE_LIMIT_CHARS: usize = 500;
const DEFAULT_PAGE_LIMIT: i64 = 100;
const MAX_PAGE_LIMIT: i64 = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePreviewRow {
    pub url: String,
    pub normalized_url: String,
    pub kind: String,
    pub platform: String,
    pub owner: Option<String>,
    pub name: Option<String>,
    pub blocked_reason: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCounts {
    #[serde(rename = "new")]
    pub r#new: i64,
    pub starred: i64,
    pub inbox: i64,
    pub blocked: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePreview {
    pub rows: Vec<CapturePreviewRow>,
    pub counts: CaptureCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoMention {
    pub full_name: String,
    pub starred: bool,
    pub in_inbox: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub original_url: String,
    pub normalized_url: String,
    pub title: String,
    pub item_type: String,
    pub platform: String,
    pub source_context: String,
    pub user_note: String,
    pub status: String,
    pub repository_id: Option<String>,
    pub created_at: String,
    pub fetch_status: String,
    pub fetch_error: String,
    pub page_author: String,
    /// `page_excerpt` 的前 400 个字符。超过时在末尾加「…」。
    pub excerpt: String,
    /// 原文或旧投递原话的前 200 个字符，超过加「…」。
    pub quote: String,
    /// 正文里提到的 GitHub 项目。没有，或 `page_repos` 解析失败，就是空数组。
    #[serde(default)]
    pub github_repos: Vec<RepoMention>,
    #[serde(default)]
    pub can_open_source: bool,
    #[serde(default)]
    pub has_source_body: bool,
    #[serde(default)]
    pub sources: Vec<InboxSourceReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxSourceReference {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxStatusCounts {
    pub pending: i64,
    pub ready: i64,
    pub failed: i64,
    pub archived: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxPage {
    pub items: Vec<InboxItem>,
    pub total_count: i64,
    pub counts: InboxStatusCounts,
}

#[derive(Clone, Copy)]
enum ListFilter {
    /// 待处理、已整理、失败。不含归档。
    Open,
    All,
    Pending,
    Ready,
    Failed,
    Archived,
}

impl ListFilter {
    fn clause(self) -> &'static str {
        match self {
            Self::Open => "status <> 'archived'",
            Self::All => "1 = 1",
            Self::Pending => "status = 'pending'",
            Self::Ready => "status = 'ready'",
            Self::Failed => "status = 'failed'",
            Self::Archived => "status = 'archived'",
        }
    }
}

fn open_readonly(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .busy_timeout(Duration::from_millis(5000))
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    connection
        .execute_batch("PRAGMA query_only = ON;")
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

fn page_window(limit: Option<i64>, offset: Option<i64>) -> (i64, i64) {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT);
    let offset = offset.unwrap_or(0).max(0);
    (limit, offset)
}

fn parse_list_filter(status: Option<&str>) -> Result<ListFilter, String> {
    match status.unwrap_or("") {
        "" => Ok(ListFilter::Open),
        "all" => Ok(ListFilter::All),
        "pending" => Ok(ListFilter::Pending),
        "ready" => Ok(ListFilter::Ready),
        "failed" => Ok(ListFilter::Failed),
        "archived" => Ok(ListFilter::Archived),
        _ => Err("收集状态不合法".to_owned()),
    }
}

/// 入库前预览。`text` 按字节算，超过 64KiB 直接拒绝。没有链接时不打开数据库。
pub fn preview_capture(db_path: &Path, text: &str) -> Result<CapturePreview, String> {
    if text.len() > TEXT_LIMIT_BYTES {
        return Err("这段文字太长，请分几次粘贴".to_owned());
    }
    let links = extract_links_from_text(text);
    if links.is_empty() {
        return Ok(CapturePreview {
            rows: Vec::new(),
            counts: CaptureCounts {
                r#new: 0,
                starred: 0,
                inbox: 0,
                blocked: 0,
            },
        });
    }
    let connection = open_readonly(db_path)?;
    let account = account_id(&connection)?;
    let mut rows = Vec::with_capacity(links.len());
    let mut counts = CaptureCounts {
        r#new: 0,
        starred: 0,
        inbox: 0,
        blocked: 0,
    };
    for link in links {
        let status = capture_link_status(&connection, &account, &link)?;
        match status {
            CaptureLinkStatus::New => counts.r#new += 1,
            CaptureLinkStatus::Starred => counts.starred += 1,
            CaptureLinkStatus::Inbox => counts.inbox += 1,
            CaptureLinkStatus::Blocked => counts.blocked += 1,
        }
        rows.push(CapturePreviewRow::from_link(link, status));
    }
    Ok(CapturePreview { rows, counts })
}

impl CapturePreviewRow {
    pub(crate) fn from_link(link: ExtractedLink, status: CaptureLinkStatus) -> Self {
        Self {
            url: link.url,
            normalized_url: link.normalized_url,
            kind: link.kind,
            platform: link.platform,
            owner: link.owner,
            name: link.name,
            blocked_reason: link.blocked_reason,
            status: status.as_str().to_owned(),
        }
    }
}

pub fn list_inbox(
    db_path: &Path,
    status: Option<&str>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<InboxPage, String> {
    let filter = parse_list_filter(status)?;
    let (limit, offset) = page_window(limit, offset);
    let connection = open_readonly(db_path)?;
    let account = account_id(&connection)?;
    let counts = status_counts(&connection, &account)?;
    let total_count = filtered_count(&connection, &account, filter)?;
    let items = load_items(&connection, &account, filter, limit, offset)?;
    Ok(InboxPage {
        items,
        total_count,
        counts,
    })
}

fn status_counts(connection: &Connection, account: &str) -> Result<InboxStatusCounts, String> {
    let mut counts = InboxStatusCounts {
        pending: 0,
        ready: 0,
        failed: 0,
        archived: 0,
    };
    let mut statement = connection
        .prepare("SELECT status, COUNT(*) FROM inbox_items WHERE account_id = ?1 GROUP BY status")
        .map_err(|error| format!("收集箱计数失败：{error}"))?;
    let rows = statement
        .query_map(params![account], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| format!("收集箱计数失败：{error}"))?;
    for row in rows {
        let (status, count) = row.map_err(|error| format!("收集箱计数失败：{error}"))?;
        match status.as_str() {
            "pending" => counts.pending = count,
            "ready" => counts.ready = count,
            "failed" => counts.failed = count,
            "archived" => counts.archived = count,
            _ => {}
        }
    }
    Ok(counts)
}

fn filtered_count(
    connection: &Connection,
    account: &str,
    filter: ListFilter,
) -> Result<i64, String> {
    let sql = format!(
        "SELECT COUNT(*) FROM inbox_items WHERE account_id = ?1 AND {}",
        filter.clause()
    );
    connection
        .query_row(&sql, params![account], |row| row.get(0))
        .map_err(|error| format!("收集箱计数失败：{error}"))
}

fn load_items(
    connection: &Connection,
    account: &str,
    filter: ListFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<InboxItem>, String> {
    let sql = format!(
        "SELECT i.id, i.original_url, i.normalized_url, i.title, i.item_type,
                COALESCE(s.platform, i.source_platform), i.source_context, i.user_note, i.status,
                i.repository_id, i.created_at, i.fetch_status, i.fetch_error, i.page_author,
                i.page_excerpt, substr(COALESCE(NULLIF(s.body, ''), i.raw_excerpt), 1, 201),
                i.page_repos, s.source_id IS NOT NULL, COALESCE(s.body <> '', 0)
         FROM inbox_items i LEFT JOIN capture_sources s ON s.source_id = i.id
         WHERE account_id = ?1 AND {}
         ORDER BY i.created_at DESC, i.id ASC
         LIMIT ?2 OFFSET ?3",
        filter.clause()
    );
    let mut drafts = {
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| format!("收集箱查询失败：{error}"))?;
        let rows = statement
            .query_map(params![account, limit, offset], read_inbox_item)
            .map_err(|error| format!("收集箱查询失败：{error}"))?;
        let mut drafts = Vec::new();
        for row in rows {
            drafts.push(row.map_err(|error| format!("收集箱行读取失败：{error}"))?);
        }
        drafts
    };
    // 这一页的仓库状态只再查两次：一次收藏，一次收集箱。没有仓库名就不查。
    fill_repo_mentions(connection, account, &mut drafts)?;
    fill_sources(connection, account, &mut drafts)?;
    Ok(drafts.into_iter().map(|(item, _)| item).collect())
}

fn read_inbox_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<(InboxItem, Vec<String>)> {
    let page_repos: String = row.get(16)?;
    let normalized_url: String = row.get(2)?;
    let can_open_source =
        row.get::<_, bool>(17)? || normalized_x_post_id(&normalized_url).is_some();
    Ok((
        InboxItem {
            id: row.get(0)?,
            original_url: row.get(1)?,
            normalized_url,
            title: row.get(3)?,
            item_type: row.get(4)?,
            platform: row.get(5)?,
            source_context: row.get(6)?,
            user_note: row.get(7)?,
            status: row.get(8)?,
            repository_id: row.get(9)?,
            created_at: row.get(10)?,
            fetch_status: row.get(11)?,
            fetch_error: row.get(12)?,
            page_author: row.get(13)?,
            excerpt: clip_chars(&row.get::<_, String>(14)?, EXCERPT_LIST_CHARS),
            quote: clip_chars(&row.get::<_, String>(15)?, QUOTE_LIST_CHARS),
            github_repos: Vec::new(),
            can_open_source,
            has_source_body: row.get(18)?,
            sources: Vec::new(),
        },
        parse_page_repos(&page_repos),
    ))
}

fn fill_sources(
    connection: &Connection,
    account: &str,
    drafts: &mut [(InboxItem, Vec<String>)],
) -> Result<(), String> {
    if drafts.is_empty() {
        return Ok(());
    }
    let urls: Vec<&str> = drafts
        .iter()
        .map(|(item, _)| item.normalized_url.as_str())
        .collect();
    let urls = serde_json::to_string(&urls).map_err(|_| "出处查询失败".to_owned())?;
    let fail = |error| format!("出处查询失败：{error}");
    let mut statement = connection
        .prepare(
            "SELECT l.target_url, i.id, i.title
         FROM capture_source_links l JOIN inbox_items i ON i.id = l.source_id
         WHERE i.account_id = ?1 AND l.target_url IN (SELECT value FROM json_each(?2))
         ORDER BY i.created_at DESC, i.id ASC",
        )
        .map_err(fail)?;
    let rows = statement
        .query_map(params![account, urls], |row| {
            Ok((
                row.get::<_, String>(0)?,
                InboxSourceReference {
                    id: row.get(1)?,
                    title: row.get(2)?,
                },
            ))
        })
        .map_err(fail)?;
    let mut by_url: HashMap<String, Vec<InboxSourceReference>> = HashMap::new();
    for row in rows {
        let (url, source) = row.map_err(fail)?;
        by_url.entry(url).or_default().push(source);
    }
    for (item, _) in drafts {
        if let Some(sources) = by_url.remove(&item.normalized_url) {
            item.sources = sources;
        }
    }
    Ok(())
}

fn parse_page_repos(raw: &str) -> Vec<String> {
    match serde_json::from_str::<Vec<String>>(raw) {
        Ok(names) => names,
        Err(_) => Vec::new(),
    }
}

fn fill_repo_mentions(
    connection: &Connection,
    account: &str,
    drafts: &mut [(InboxItem, Vec<String>)],
) -> Result<(), String> {
    let mut names = Vec::new();
    let mut seen_names = HashSet::new();
    let mut urls = Vec::new();
    let mut seen_urls = HashSet::new();
    for (_, repo_names) in drafts.iter() {
        for name in repo_names {
            let key = name.to_ascii_lowercase();
            if seen_names.insert(key.clone()) {
                names.push(key);
            }
            let url = format!("https://github.com/{}", name.to_ascii_lowercase());
            if seen_urls.insert(url.clone()) {
                urls.push(url);
            }
        }
    }
    if names.is_empty() {
        return Ok(());
    }
    let names_json = serde_json::to_string(&names).map_err(|_| "仓库名单序列化失败".to_owned())?;
    let urls_json = serde_json::to_string(&urls).map_err(|_| "仓库名单序列化失败".to_owned())?;
    let starred = starred_full_names(connection, account, &names_json)?;
    let inbox_hits = inbox_urls_by_id(connection, account, &urls_json)?;
    for (item, repo_names) in drafts.iter_mut() {
        let mut mentions = Vec::with_capacity(repo_names.len());
        for name in repo_names {
            let key = name.to_ascii_lowercase();
            let url = format!("https://github.com/{key}");
            let in_inbox = inbox_hits
                .get(&url)
                .is_some_and(|ids| ids.iter().any(|id| id != &item.id));
            mentions.push(RepoMention {
                full_name: name.clone(),
                starred: starred.contains(&key),
                in_inbox,
            });
        }
        item.github_repos = mentions;
    }
    Ok(())
}

fn starred_full_names(
    connection: &Connection,
    account: &str,
    names_json: &str,
) -> Result<HashSet<String>, String> {
    let mut statement = connection
        .prepare(
            "SELECT lower(full_name)
             FROM repositories
             WHERE account_id = ?1
               AND sync_status = 'active'
               AND lower(full_name) IN (SELECT lower(value) FROM json_each(?2))",
        )
        .map_err(|error| format!("已收藏仓库查询失败：{error}"))?;
    let rows = statement
        .query_map(params![account, names_json], |row| row.get::<_, String>(0))
        .map_err(|error| format!("已收藏仓库查询失败：{error}"))?;
    let mut found = HashSet::new();
    for row in rows {
        found.insert(row.map_err(|error| format!("已收藏仓库读取失败：{error}"))?);
    }
    Ok(found)
}

fn inbox_urls_by_id(
    connection: &Connection,
    account: &str,
    urls_json: &str,
) -> Result<HashMap<String, Vec<String>>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, lower(normalized_url)
             FROM inbox_items
             WHERE account_id = ?1
               AND lower(normalized_url) IN (SELECT lower(value) FROM json_each(?2))",
        )
        .map_err(|error| format!("收集箱仓库查询失败：{error}"))?;
    let rows = statement
        .query_map(params![account, urls_json], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("收集箱仓库查询失败：{error}"))?;
    let mut found: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (id, url) = row.map_err(|error| format!("收集箱仓库读取失败：{error}"))?;
        found.entry(url).or_default().push(id);
    }
    Ok(found)
}

const EXCERPT_LIST_CHARS: usize = 400;
const QUOTE_LIST_CHARS: usize = 200;

/// 列表只给前 `limit` 个字符。按字符截，不用字节；超过才加省略号。
fn clip_chars(text: &str, limit: usize) -> String {
    let mut clipped = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index == limit {
            clipped.push('…');
            break;
        }
        clipped.push(ch);
    }
    clipped
}

pub fn set_inbox_status(db_path: &Path, id: &str, status: &str) -> Result<(), String> {
    match status {
        "pending" | "ready" | "archived" => {}
        "failed" => return Err("失败状态由系统写入，不能手动设置".to_owned()),
        _ => return Err("收集状态不合法".to_owned()),
    }
    update_owned_inbox(
        db_path,
        id,
        "UPDATE inbox_items SET status = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2 AND account_id = ?3",
        status,
        "状态",
    )
}

pub fn set_inbox_note(db_path: &Path, id: &str, note: &str) -> Result<(), String> {
    let note = note.trim();
    if note.chars().count() > NOTE_LIMIT_CHARS {
        return Err("备注最多 500 个字".to_owned());
    }
    update_owned_inbox(
        db_path,
        id,
        "UPDATE inbox_items SET user_note = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2 AND account_id = ?3",
        note,
        "备注",
    )
}

fn update_owned_inbox(
    db_path: &Path,
    id: &str,
    sql: &str,
    value: &str,
    action: &str,
) -> Result<(), String> {
    let mut connection = open_write(db_path)?;
    let account = account_id(&connection)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("收集事务开启失败：{error}"))?;
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM inbox_items WHERE id = ?1 AND account_id = ?2)",
            params![id, account],
            |row| row.get(0),
        )
        .map_err(|error| format!("收集条目读取失败：{error}"))?;
    if !exists {
        return Err("找不到这条收集".to_owned());
    }
    tx.execute(sql, params![value, id, account])
        .map_err(|error| format!("{action}写入失败：{error}"))?;
    tx.commit()
        .map_err(|error| format!("收集提交失败：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::extract_links_from_text;
    use rusqlite::Connection;
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const SAMPLE: &str = "\
https://github.com/OctoCat/Hello.git
https://example.com/docs?utm_source=wechat&utm_medium=chat
http://10.0.0.1/x
https://github.com/fresh/widget
https://github.com/acme/tool
https://github.com/gone/repo
https://github.com/someone/else";

    struct TempDb {
        path: PathBuf,
    }

    impl TempDb {
        fn new(label: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let seq = SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应可用")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("fsl-inbox-{label}-{nanos}-{seq}.sqlite3"));
            crate::db::migrations::migrate_database(&path).expect("测试库应能迁移");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let mut wal = self.path.as_os_str().to_owned();
            wal.push("-wal");
            let _ = std::fs::remove_file(wal);
            let mut shm = self.path.as_os_str().to_owned();
            shm.push("-shm");
            let _ = std::fs::remove_file(shm);
        }
    }

    fn checkpoint(path: &Path) {
        let connection = Connection::open(path).expect("应能打开测试库");
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("应能合并预写日志");
    }

    fn count_of(path: &Path, sql: &str) -> i64 {
        let connection = Connection::open(path).expect("应能打开测试库");
        connection
            .query_row(sql, [], |row| row.get(0))
            .expect("计数应成功")
    }

    struct RowSnap {
        account_id: String,
        original_url: String,
        normalized_url: String,
        title: String,
        item_type: String,
        user_note: String,
        status: String,
        repository_id: Option<String>,
        created_at: String,
        updated_at: String,
        platform: String,
        source_context: String,
        captured_via: String,
        capture_batch_id: Option<String>,
        raw_excerpt: String,
    }

    fn snap(path: &Path, id: &str) -> RowSnap {
        let connection = Connection::open(path).expect("应能打开测试库");
        connection
            .query_row(
                "SELECT account_id, original_url, normalized_url, title, item_type, user_note, status, repository_id, created_at, updated_at, source_platform, source_context, captured_via, capture_batch_id, raw_excerpt FROM inbox_items WHERE id = ?1",
                params![id],
                |row| {
                    Ok(RowSnap {
                        account_id: row.get(0)?,
                        original_url: row.get(1)?,
                        normalized_url: row.get(2)?,
                        title: row.get(3)?,
                        item_type: row.get(4)?,
                        user_note: row.get(5)?,
                        status: row.get(6)?,
                        repository_id: row.get(7)?,
                        created_at: row.get(8)?,
                        updated_at: row.get(9)?,
                        platform: row.get(10)?,
                        source_context: row.get(11)?,
                        captured_via: row.get(12)?,
                        capture_batch_id: row.get(13)?,
                        raw_excerpt: row.get(14)?,
                    })
                },
            )
            .expect("应能读到收集条目")
    }

    fn insert_repo(
        connection: &Connection,
        id: &str,
        account: &str,
        full_name: &str,
        sync_status: &str,
    ) {
        let mut parts = full_name.split('/');
        let owner = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        connection
            .execute(
                "INSERT INTO repositories(id, account_id, owner, name, full_name, html_url, starred_at, sync_status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '2026-10-01T00:00:00Z', ?7)",
                params![
                    id,
                    account,
                    owner,
                    name,
                    full_name,
                    format!("https://github.com/{full_name}"),
                    sync_status
                ],
            )
            .expect("应能插入仓库");
    }

    fn insert_inbox(
        connection: &Connection,
        id: &str,
        account: &str,
        normalized: &str,
        status: &str,
        created_at: &str,
        platform: &str,
        context: &str,
        note: &str,
        title: &str,
        item_type: &str,
        repository_id: Option<&str>,
    ) {
        connection
            .execute(
                "INSERT INTO inbox_items(id, account_id, original_url, normalized_url, title, item_type, user_note, status, repository_id, created_at, updated_at, source_platform, source_context) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, '2000-01-01T00:00:00.000Z', ?11, ?12)",
                params![
                    id,
                    account,
                    normalized,
                    normalized,
                    title,
                    item_type,
                    note,
                    status,
                    repository_id,
                    created_at,
                    platform,
                    context
                ],
            )
            .expect("应能插入收集条目");
    }

    fn prepare_mixed() -> TempDb {
        let db = TempDb::new("mixed");
        let links = extract_links_from_text(SAMPLE);
        assert_eq!(links.len(), 7, "夹具应抽出 7 条不重复链接");
        assert!(links[0].url.ends_with(".git"));
        assert_eq!(links[0].normalized_url, "https://github.com/octocat/hello");
        assert!(links[1].url.contains("utm_"));
        assert!(!links[1].normalized_url.contains("utm_"));
        assert!(links[2].blocked_reason.is_some());
        assert!(links[2].url.contains("10.0.0.1"));
        let connection = Connection::open(db.path()).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref, updated_at) VALUES ('old', 'olduser', 'test', '2020-01-01T00:00:00.000Z')",
                [],
            )
            .expect("应能插入旧账号");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref, updated_at) VALUES ('acct', 'fox', 'test', '2026-10-03T00:00:00.000Z')",
                [],
            )
            .expect("应能插入当前账号");
        insert_repo(&connection, "hello", "acct", "octocat/hello", "active");
        insert_repo(&connection, "tool", "acct", "acme/tool", "active");
        insert_repo(&connection, "gone", "acct", "gone/repo", "removed");
        insert_repo(&connection, "else-repo", "old", "someone/else", "active");
        insert_inbox(
            &connection,
            "in-docs",
            "acct",
            &links[1].normalized_url,
            "pending",
            "2026-10-02T00:00:00.000Z",
            "web",
            "微信群",
            "已有",
            "文档",
            "webpage",
            None,
        );
        insert_inbox(
            &connection,
            "in-blocked",
            "acct",
            &links[2].normalized_url,
            "pending",
            "2026-10-02T00:00:00.000Z",
            "web",
            "",
            "",
            "内网",
            "webpage",
            None,
        );
        insert_inbox(
            &connection,
            "in-tool",
            "acct",
            &links[4].normalized_url,
            "ready",
            "2026-10-02T00:00:00.000Z",
            "github",
            "",
            "",
            "acme/tool",
            "github_repository",
            Some("tool"),
        );
        insert_inbox(
            &connection,
            "in-fresh-old",
            "old",
            &links[3].normalized_url,
            "pending",
            "2026-10-02T00:00:00.000Z",
            "github",
            "",
            "",
            "fresh/widget",
            "github_repository",
            None,
        );
        drop(connection);
        checkpoint(db.path());
        db
    }

    fn prepare_list() -> TempDb {
        let db = TempDb::new("list");
        let connection = Connection::open(db.path()).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref, updated_at) VALUES ('old', 'olduser', 'test', '2020-01-01T00:00:00.000Z')",
                [],
            )
            .expect("应能插入旧账号");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref, updated_at) VALUES ('acct', 'fox', 'test', '2026-10-03T00:00:00.000Z')",
                [],
            )
            .expect("应能插入当前账号");
        insert_repo(&connection, "repo-e", "acct", "fox/video", "active");
        insert_inbox(
            &connection,
            "b",
            "acct",
            "https://example.com/b",
            "pending",
            "2026-10-03T00:00:00.000Z",
            "wechat",
            "微信群",
            "先放着",
            "群里的链接",
            "webpage",
            None,
        );
        insert_inbox(
            &connection,
            "c",
            "acct",
            "https://github.com/fox/c",
            "ready",
            "2026-10-03T00:00:00.000Z",
            "github",
            "手动粘贴",
            "",
            "fox/c",
            "github_repository",
            None,
        );
        insert_inbox(
            &connection,
            "e",
            "acct",
            "https://bilibili.com/video/e",
            "pending",
            "2026-10-02T00:00:00.000Z",
            "bilibili",
            "视频号",
            "晚点看",
            "一条视频",
            "video",
            Some("repo-e"),
        );
        insert_inbox(
            &connection,
            "a",
            "acct",
            "https://x.com/u/status/1",
            "failed",
            "2026-10-01T00:00:00.000Z",
            "x",
            "帖子",
            "",
            "一条帖子",
            "webpage",
            None,
        );
        insert_inbox(
            &connection,
            "d",
            "acct",
            "https://example.com/old",
            "archived",
            "2026-10-04T00:00:00.000Z",
            "web",
            "旧链接",
            "已归档备注",
            "旧的",
            "webpage",
            None,
        );
        insert_inbox(
            &connection,
            "z",
            "old",
            "https://example.com/other-account",
            "pending",
            "2026-10-05T00:00:00.000Z",
            "web",
            "别人的",
            "",
            "别的账号",
            "webpage",
            None,
        );
        drop(connection);
        checkpoint(db.path());
        db
    }

    fn assert_sum(preview: &CapturePreview) {
        let sum = preview.counts.r#new
            + preview.counts.starred
            + preview.counts.inbox
            + preview.counts.blocked;
        assert_eq!(sum, preview.rows.len() as i64);
    }

    #[test]
    fn preview_classifies_the_four_statuses() {
        let db = prepare_mixed();
        let preview = preview_capture(db.path(), SAMPLE).expect("预览应成功");
        assert_eq!(preview.rows.len(), 7);
        assert_eq!(preview.rows[0].status, "starred");
        assert_eq!(preview.rows[0].owner.as_deref(), Some("OctoCat"));
        assert_eq!(preview.rows[0].name.as_deref(), Some("Hello"));
        assert_eq!(
            preview.rows[0].normalized_url,
            "https://github.com/octocat/hello"
        );
        assert!(preview.rows[0].url.ends_with(".git"));
        assert!(preview.rows[0].blocked_reason.is_none());
        assert_eq!(preview.rows[1].status, "inbox");
        assert!(preview.rows[1].url.contains("utm_"));
        assert!(!preview.rows[1].normalized_url.contains("utm_"));
        assert_eq!(preview.rows[2].status, "blocked");
        assert!(preview.rows[2].url.contains("10.0.0.1"));
        assert!(preview.rows[2].blocked_reason.is_some());
        assert_eq!(preview.rows[3].status, "new");
        assert_eq!(preview.rows[4].status, "starred");
        assert_eq!(preview.rows[5].status, "new");
        assert_eq!(preview.rows[6].status, "new");
        assert_eq!(
            preview.counts,
            CaptureCounts {
                r#new: 3,
                starred: 2,
                inbox: 1,
                blocked: 1,
            }
        );
        assert_sum(&preview);
    }

    #[test]
    fn preview_rejects_text_over_64kib() {
        let missing = std::env::temp_dir().join("fsl-inbox-missing-oversize.sqlite3");
        let _ = std::fs::remove_file(&missing);
        let mut text = "https://example.com/a".to_owned();
        text.push_str(&"字".repeat(TEXT_LIMIT_BYTES));
        assert!(text.len() > TEXT_LIMIT_BYTES);
        let error = preview_capture(&missing, &text).expect_err("超长文本应拒绝");
        assert_eq!(error, "这段文字太长，请分几次粘贴");
        assert!(!missing.exists(), "超长文本不应创建数据库");
    }

    #[test]
    fn preview_accepts_exactly_64kib() {
        let db = TempDb::new("exact");
        let connection = Connection::open(db.path()).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .expect("应能插入账号");
        drop(connection);
        checkpoint(db.path());
        let url = "https://example.com/pad";
        let mut text = url.to_owned();
        text.push_str(&" ".repeat(TEXT_LIMIT_BYTES - url.len()));
        assert_eq!(text.len(), TEXT_LIMIT_BYTES);
        let preview = preview_capture(db.path(), &text).expect("刚好 64KiB 应能预览");
        assert_eq!(preview.rows.len(), 1);
        assert_eq!(preview.rows[0].status, "new");
        assert_sum(&preview);
    }

    #[test]
    fn preview_without_links_returns_empty() {
        let missing = std::env::temp_dir().join("fsl-inbox-missing-empty.sqlite3");
        let _ = std::fs::remove_file(&missing);
        let preview = preview_capture(&missing, "只有文字，没有链接。").expect("没有链接不应报错");
        assert!(preview.rows.is_empty());
        assert_eq!(
            preview.counts,
            CaptureCounts {
                r#new: 0,
                starred: 0,
                inbox: 0,
                blocked: 0,
            }
        );
        assert!(!missing.exists(), "没有链接不应打开或创建数据库");
    }

    #[test]
    fn preview_matches_commit_capture_and_does_not_write() {
        let db = prepare_mixed();
        let before_items = count_of(db.path(), "SELECT COUNT(*) FROM inbox_items");
        let before_batches = count_of(db.path(), "SELECT COUNT(*) FROM capture_batches");
        let before_repos = count_of(db.path(), "SELECT COUNT(*) FROM repositories");
        let preview = preview_capture(db.path(), SAMPLE).expect("预览应成功");
        assert_eq!(
            count_of(db.path(), "SELECT COUNT(*) FROM inbox_items"),
            before_items,
            "预览不能写收集箱"
        );
        assert_eq!(
            count_of(db.path(), "SELECT COUNT(*) FROM capture_batches"),
            before_batches
        );
        assert_eq!(
            count_of(db.path(), "SELECT COUNT(*) FROM repositories"),
            before_repos
        );
        assert_sum(&preview);
        let committed = crate::v2_ops::commit_capture(db.path(), SAMPLE, "测试群")
            .expect("同一临时库上的入库应成功");
        assert!(!committed.duplicate_batch);
        assert_eq!(committed.links_found, preview.rows.len());
        assert_eq!(committed.created, preview.counts.r#new as usize);
        assert_eq!(committed.already_starred, preview.counts.starred as usize);
        assert_eq!(committed.already_in_inbox, preview.counts.inbox as usize);
        assert_eq!(committed.blocked, preview.counts.blocked as usize);
        assert_eq!(
            committed.skipped,
            committed.already_starred + committed.already_in_inbox + committed.blocked
        );
    }

    #[test]
    fn missing_account_is_rejected_when_a_link_needs_it() {
        let db = TempDb::new("no-account");
        checkpoint(db.path());
        let preview_error =
            preview_capture(db.path(), "https://example.com/a").expect_err("没有账号不能预览");
        assert_eq!(
            preview_error,
            "开发库里还没有 GitHub 账号，无法写入收集或分类。"
        );
        let list_error = list_inbox(db.path(), None, None, None).expect_err("没有账号不能列表");
        assert_eq!(list_error, preview_error);
    }

    #[test]
    fn list_inbox_default_excludes_archived() {
        let db = prepare_list();
        let page = list_inbox(db.path(), None, None, None).expect("默认列表应成功");
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "c", "e", "a"]
        );
        assert_eq!(page.total_count, 4);
        assert!(page.items.iter().all(|item| item.status != "archived"));
    }

    #[test]
    fn list_inbox_all_includes_archived() {
        let db = prepare_list();
        let page = list_inbox(db.path(), Some("all"), None, None).expect("all 应成功");
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["d", "b", "c", "e", "a"]
        );
        assert_eq!(page.total_count, 5);
        assert_eq!(page.items[0].status, "archived");
    }

    #[test]
    fn list_inbox_filters_each_status() {
        let db = prepare_list();
        let cases = [
            ("pending", vec!["b", "e"]),
            ("ready", vec!["c"]),
            ("failed", vec!["a"]),
            ("archived", vec!["d"]),
        ];
        for (status, ids) in cases {
            let page = list_inbox(db.path(), Some(status), None, None)
                .unwrap_or_else(|_| panic!("{status} 筛选应成功"));
            assert_eq!(
                page.items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                ids,
                "{status}"
            );
            assert_eq!(page.total_count, ids.len() as i64, "{status}");
            assert!(page.items.iter().all(|item| item.status == status));
        }
    }

    #[test]
    fn list_inbox_sort_and_pagination_are_stable() {
        let db = prepare_list();
        let first = list_inbox(db.path(), Some(""), Some(2), Some(0)).expect("第一页应成功");
        let again = list_inbox(db.path(), Some(""), Some(2), Some(0)).expect("重复第一页应成功");
        let second = list_inbox(db.path(), Some(""), Some(2), Some(2)).expect("第二页应成功");
        let third = list_inbox(db.path(), Some(""), Some(2), Some(4)).expect("第三页应成功");
        assert_eq!(ids_of(&first), vec!["b", "c"]);
        assert_eq!(ids_of(&again), ids_of(&first), "同一页再读一次应保持原顺序");
        assert_eq!(ids_of(&second), vec!["e", "a"]);
        assert!(third.items.is_empty());
        assert_eq!(first.total_count, 4);
        assert_eq!(second.total_count, 4);
        let mut seen = ids_of(&first);
        seen.extend(ids_of(&second));
        assert_eq!(seen, vec!["b", "c", "e", "a"]);
    }

    fn ids_of(page: &InboxPage) -> Vec<&str> {
        page.items.iter().map(|item| item.id.as_str()).collect()
    }

    #[test]
    fn list_inbox_counts_ignore_filter() {
        let db = prepare_list();
        let expected = InboxStatusCounts {
            pending: 2,
            ready: 1,
            failed: 1,
            archived: 1,
        };
        let filtered = list_inbox(db.path(), Some("ready"), Some(1), Some(0)).expect("筛选应成功");
        assert_eq!(filtered.counts, expected);
        assert_eq!(filtered.total_count, 1);
        assert_eq!(filtered.items.len(), 1);
        let archived = list_inbox(db.path(), Some("archived"), None, None).expect("归档筛选应成功");
        assert_eq!(archived.counts, expected);
        assert_eq!(archived.total_count, 1);
        let everything = list_inbox(db.path(), Some("all"), None, None).expect("全部应成功");
        assert_eq!(everything.counts, expected);
        assert_eq!(
            everything.counts.pending
                + everything.counts.ready
                + everything.counts.failed
                + everything.counts.archived,
            5,
            "别的账号那一条不能算进当前账号"
        );
    }

    #[test]
    fn list_inbox_rejects_invalid_status() {
        let db = prepare_list();
        for status in ["Pending", "ALL", "all ", " ", "new", "starred"] {
            let error =
                list_inbox(db.path(), Some(status), None, None).expect_err("非法状态应拒绝");
            assert_eq!(error, "收集状态不合法", "{status}");
        }
    }

    #[test]
    fn list_inbox_clamps_limit_and_offset() {
        assert_eq!(page_window(None, None), (100, 0));
        assert_eq!(page_window(Some(0), Some(-4)), (1, 0));
        assert_eq!(page_window(Some(1), Some(0)), (1, 0));
        assert_eq!(page_window(Some(100), Some(2)), (100, 2));
        assert_eq!(page_window(Some(200), Some(0)), (200, 0));
        assert_eq!(page_window(Some(201), Some(0)), (200, 0));
        assert_eq!(page_window(Some(i64::MAX), Some(i64::MIN)), (200, 0));

        let db = prepare_list();
        let clamped = list_inbox(db.path(), None, Some(0), Some(-8)).expect("夹取后应能列出");
        assert_eq!(clamped.items.len(), 1, "limit 0 应抬到 1，负偏移应落到 0");
        assert_eq!(clamped.items[0].id, "b");
        assert_eq!(clamped.total_count, 4);
        let one = list_inbox(db.path(), None, Some(1), Some(1)).expect("limit 1 应成功");
        assert_eq!(ids_of(&one), vec!["c"]);
        let wide =
            list_inbox(db.path(), None, Some(10_000), None).expect("过大的 limit 应被夹住后成功");
        assert_eq!(
            wide.items.len(),
            4,
            "默认范围内的条目应全部返回，且不超过夹取上限"
        );
    }

    #[test]
    fn list_inbox_maps_source_fields() {
        let db = prepare_list();
        let page = list_inbox(db.path(), Some("all"), None, None).expect("列表应成功");
        let group = page
            .items
            .iter()
            .find(|item| item.id == "b")
            .expect("应有微信那一条");
        assert_eq!(group.platform, "wechat");
        assert_eq!(group.source_context, "微信群");
        assert_eq!(group.user_note, "先放着");
        assert_eq!(group.title, "群里的链接");
        assert_eq!(group.item_type, "webpage");
        assert_eq!(group.status, "pending");
        assert_eq!(group.repository_id, None);
        assert_eq!(group.created_at, "2026-10-03T00:00:00.000Z");
        assert_eq!(group.original_url, "https://example.com/b");
        assert_eq!(group.normalized_url, "https://example.com/b");
        let video = page
            .items
            .iter()
            .find(|item| item.id == "e")
            .expect("应有视频那一条");
        assert_eq!(video.platform, "bilibili");
        assert_eq!(video.source_context, "视频号");
        assert_eq!(video.item_type, "video");
        assert_eq!(video.repository_id.as_deref(), Some("repo-e"));
        assert_eq!(group.fetch_status, "pending");
        assert_eq!(group.fetch_error, "");
        assert_eq!(group.page_author, "");
        assert_eq!(group.excerpt, "");
    }

    #[test]
    fn list_inbox_carries_the_original_words_clipped_to_200_chars() {
        let db = prepare_list();
        let connection = Connection::open(db.path()).expect("应能打开测试库");
        let exactly: String = "话".repeat(200);
        let longer: String = "话".repeat(201);
        connection
            .execute(
                "UPDATE inbox_items SET raw_excerpt = ?1 WHERE id = 'b'",
                params![exactly],
            )
            .expect("应能写入原话");
        connection
            .execute(
                "UPDATE inbox_items SET raw_excerpt = ?1 WHERE id = 'c'",
                params![longer],
            )
            .expect("应能写入超长原话");
        drop(connection);
        let page = list_inbox(db.path(), Some("all"), None, None).expect("列表应成功");
        let find = |id: &str| {
            page.items
                .iter()
                .find(|item| item.id == id)
                .expect("应有这条")
        };
        assert_eq!(find("b").quote, exactly);
        assert!(!find("b").quote.ends_with('…'));
        assert_eq!(find("c").quote.chars().count(), 201);
        assert!(find("c").quote.ends_with('…'));
        // 没有原话（粘贴收集）就是空串
        assert_eq!(find("a").quote, "");
    }

    #[test]
    fn list_inbox_clips_excerpt_by_chars_not_bytes() {
        let db = prepare_list();
        let connection = Connection::open(db.path()).expect("应能打开测试库");
        let exactly: String = "测".repeat(400);
        let longer: String = "测".repeat(401);
        assert!(longer.len() > 401, "用汉字才能证明按字符截");
        connection
            .execute(
                "UPDATE inbox_items SET fetch_status = 'done', fetch_error = '不应出现', page_author = '甲', page_excerpt = ?1 WHERE id = 'b'",
                params![exactly],
            )
            .expect("应能写入刚好 400 字的摘要");
        connection
            .execute(
                "UPDATE inbox_items SET fetch_status = 'failed', fetch_error = '读不到', page_author = '乙', page_excerpt = ?1 WHERE id = 'c'",
                params![longer],
            )
            .expect("应能写入超长摘要");
        drop(connection);

        let page = list_inbox(db.path(), Some("all"), None, None).expect("列表应成功");
        let exact = page
            .items
            .iter()
            .find(|item| item.id == "b")
            .expect("应有刚好 400 字的那一条");
        assert_eq!(exact.fetch_status, "done");
        assert_eq!(exact.fetch_error, "不应出现");
        assert_eq!(exact.page_author, "甲");
        assert_eq!(exact.excerpt.chars().count(), 400);
        assert_eq!(exact.excerpt, exactly);
        assert!(!exact.excerpt.ends_with('…'));
        let clipped = page
            .items
            .iter()
            .find(|item| item.id == "c")
            .expect("应有超长的那一条");
        assert_eq!(clipped.fetch_status, "failed");
        assert_eq!(clipped.fetch_error, "读不到");
        assert_eq!(clipped.page_author, "乙");
        assert_eq!(clipped.excerpt.chars().count(), 401);
        assert!(clipped.excerpt.ends_with('…'));
        assert_eq!(
            &clipped.excerpt[..clipped.excerpt.len() - '…'.len_utf8()],
            exactly
        );
    }

    #[test]
    fn list_inbox_blank_status_matches_default() {
        let db = prepare_list();
        let omitted = list_inbox(db.path(), None, None, None).expect("省略状态应成功");
        let blank = list_inbox(db.path(), Some(""), None, None).expect("空串应成功");
        assert_eq!(ids_of(&omitted), ids_of(&blank));
        assert_eq!(omitted.total_count, blank.total_count);
        assert_eq!(omitted.counts, blank.counts);
    }

    #[test]
    fn set_inbox_status_archives_and_restores() {
        let db = prepare_list();
        let before = snap(db.path(), "b");
        let steps = ["ready", "archived", "pending", "archived", "ready"];
        for status in steps {
            set_inbox_status(db.path(), "b", status)
                .unwrap_or_else(|_| panic!("设为 {status} 应成功"));
            let current = snap(db.path(), "b");
            assert_eq!(current.status, status);
            assert_eq!(current.user_note, before.user_note);
            assert_eq!(current.title, before.title);
            assert_eq!(current.created_at, before.created_at);
            assert_eq!(current.original_url, before.original_url);
            assert_eq!(current.normalized_url, before.normalized_url);
            assert_eq!(current.item_type, before.item_type);
            assert_eq!(current.platform, before.platform);
            assert_eq!(current.source_context, before.source_context);
            assert_eq!(current.repository_id, before.repository_id);
            assert_eq!(current.account_id, before.account_id);
            assert_eq!(current.captured_via, before.captured_via);
            assert_eq!(current.capture_batch_id, before.capture_batch_id);
            assert_eq!(current.raw_excerpt, before.raw_excerpt);
            assert_ne!(current.updated_at, before.updated_at);
        }
        let untouched = snap(db.path(), "c");
        assert_eq!(untouched.status, "ready");
        assert_eq!(untouched.updated_at, "2000-01-01T00:00:00.000Z");
    }

    #[test]
    fn set_inbox_status_rejects_failed_and_missing_id() {
        let db = prepare_list();
        let before = snap(db.path(), "b");
        let failed = set_inbox_status(db.path(), "b", "failed").expect_err("不能手设失败");
        assert_eq!(failed, "失败状态由系统写入，不能手动设置");
        for status in ["", "all", "new", "FAILED", "Pending"] {
            let error = set_inbox_status(db.path(), "b", status).expect_err("非法状态应拒绝");
            assert_eq!(error, "收集状态不合法", "{status}");
        }
        let missing =
            set_inbox_status(db.path(), "missing", "archived").expect_err("不存在的 id 应拒绝");
        assert_eq!(missing, "找不到这条收集");
        let other = set_inbox_status(db.path(), "z", "archived").expect_err("别的账号的条目应拒绝");
        assert_eq!(other, "找不到这条收集");
        let after = snap(db.path(), "b");
        assert_eq!(after.status, before.status);
        assert_eq!(after.updated_at, before.updated_at);
        assert_eq!(after.user_note, before.user_note);
        let other_row = snap(db.path(), "z");
        assert_eq!(other_row.status, "pending");
        assert_eq!(other_row.updated_at, "2000-01-01T00:00:00.000Z");
    }

    #[test]
    fn set_inbox_note_enforces_char_limit_and_trim() {
        let db = prepare_list();
        let before = snap(db.path(), "b");
        let exactly: String = "测".repeat(500);
        assert_eq!(exactly.chars().count(), 500);
        assert!(exactly.len() > 500, "用汉字才能证明按字符而不是按字节");
        let padded = format!(" \n{exactly}\t ");
        set_inbox_note(db.path(), "b", &padded).expect("恰好 500 字应成功");
        let stored = snap(db.path(), "b");
        assert_eq!(stored.user_note, exactly);
        assert_eq!(stored.status, before.status);
        assert_eq!(stored.title, before.title);
        assert_eq!(stored.created_at, before.created_at);
        assert_eq!(stored.original_url, before.original_url);
        assert_eq!(stored.platform, before.platform);
        assert_eq!(stored.source_context, before.source_context);
        assert_eq!(stored.repository_id, before.repository_id);
        assert_ne!(stored.updated_at, before.updated_at);

        let too_long: String = "测".repeat(501);
        let error = set_inbox_note(db.path(), "b", &too_long).expect_err("501 字应拒绝");
        assert_eq!(error, "备注最多 500 个字");
        let still = snap(db.path(), "b");
        assert_eq!(still.user_note, exactly, "超出时不能悄悄截断");
        assert_eq!(still.updated_at, stored.updated_at);

        set_inbox_note(db.path(), "b", "  你好 世界 \n").expect("去空白应成功");
        assert_eq!(snap(db.path(), "b").user_note, "你好 世界");
        set_inbox_note(db.path(), "b", " \n\t ").expect("空白备注应存成空串");
        let cleared = snap(db.path(), "b");
        assert_eq!(cleared.user_note, "");
        assert_eq!(cleared.status, "pending");
        let other = snap(db.path(), "c");
        assert_eq!(other.user_note, "");
        assert_eq!(other.status, "ready");
        assert_eq!(other.updated_at, "2000-01-01T00:00:00.000Z");
    }

    #[test]
    fn contract_serialization_matches_typescript() {
        let row = CapturePreviewRow {
            url: "https://github.com/OctoCat/Hello.git".to_owned(),
            normalized_url: "https://github.com/octocat/hello".to_owned(),
            kind: "github_repo".to_owned(),
            platform: "github".to_owned(),
            owner: Some("OctoCat".to_owned()),
            name: None,
            blocked_reason: None,
            status: "starred".to_owned(),
        };
        let preview = CapturePreview {
            rows: vec![row],
            counts: CaptureCounts {
                r#new: 1,
                starred: 0,
                inbox: 2,
                blocked: 3,
            },
        };
        let item = InboxItem {
            id: "b".to_owned(),
            original_url: "https://example.com/b".to_owned(),
            normalized_url: "https://example.com/b".to_owned(),
            title: "群里的链接".to_owned(),
            item_type: "webpage".to_owned(),
            platform: "wechat".to_owned(),
            source_context: "微信群".to_owned(),
            user_note: "先放着".to_owned(),
            status: "pending".to_owned(),
            repository_id: None,
            created_at: "2026-10-03T00:00:00.000Z".to_owned(),
            fetch_status: "pending".to_owned(),
            fetch_error: "".to_owned(),
            page_author: "作者".to_owned(),
            excerpt: "摘要".to_owned(),
            quote: "原话".to_owned(),
            can_open_source: false,
            has_source_body: false,
            sources: Vec::new(),
            github_repos: vec![RepoMention {
                full_name: "octocat/hello".to_owned(),
                starred: true,
                in_inbox: false,
            }],
        };
        let page = InboxPage {
            items: vec![item],
            total_count: 4,
            counts: InboxStatusCounts {
                pending: 2,
                ready: 1,
                failed: 0,
                archived: 1,
            },
        };

        let preview_json = serde_json::to_value(&preview).expect("预览应能序列化");
        assert_eq!(sorted_keys(&preview_json), vec!["counts", "rows"]);
        let rows = preview_json["rows"].as_array().expect("rows 应是数组");
        assert_eq!(rows.len(), 1);
        let row_json = &rows[0];
        assert_eq!(
            sorted_keys(row_json),
            vec![
                "blockedReason",
                "kind",
                "name",
                "normalizedUrl",
                "owner",
                "platform",
                "status",
                "url",
            ]
        );
        assert!(row_json["url"].is_string());
        assert!(row_json["normalizedUrl"].is_string());
        assert!(row_json["kind"].is_string());
        assert!(row_json["platform"].is_string());
        assert!(row_json["owner"].is_string());
        assert!(row_json["name"].is_null());
        assert!(row_json["blockedReason"].is_null());
        assert_eq!(row_json["status"], "starred");
        let counts = &preview_json["counts"];
        assert_eq!(
            sorted_keys(counts),
            vec!["blocked", "inbox", "new", "starred"]
        );
        assert_eq!(counts["new"], 1);
        assert_eq!(counts["starred"], 0);
        assert_eq!(counts["inbox"], 2);
        assert_eq!(counts["blocked"], 3);
        assert!(counts["new"].is_number());

        let page_json = serde_json::to_value(&page).expect("列表应能序列化");
        assert_eq!(
            sorted_keys(&page_json),
            vec!["counts", "items", "totalCount"]
        );
        assert!(page_json["totalCount"].is_number());
        assert_eq!(page_json["totalCount"], 4);
        let items = page_json["items"].as_array().expect("items 应是数组");
        let item_json = &items[0];
        assert_eq!(
            sorted_keys(item_json),
            vec![
                "canOpenSource",
                "createdAt",
                "excerpt",
                "fetchError",
                "fetchStatus",
                "githubRepos",
                "hasSourceBody",
                "id",
                "itemType",
                "normalizedUrl",
                "originalUrl",
                "pageAuthor",
                "platform",
                "quote",
                "repositoryId",
                "sourceContext",
                "sources",
                "status",
                "title",
                "userNote",
            ]
        );
        for key in [
            "id",
            "originalUrl",
            "normalizedUrl",
            "title",
            "itemType",
            "platform",
            "sourceContext",
            "userNote",
            "status",
            "createdAt",
            "fetchStatus",
            "fetchError",
            "pageAuthor",
            "excerpt",
            "quote",
        ] {
            assert!(item_json[key].is_string(), "{key} 应是字符串");
        }
        assert_eq!(item_json["fetchStatus"], "pending");
        assert_eq!(item_json["fetchError"], "");
        assert_eq!(item_json["pageAuthor"], "作者");
        assert_eq!(item_json["excerpt"], "摘要");
        assert_eq!(item_json["quote"], "原话");
        let mentions = item_json["githubRepos"]
            .as_array()
            .expect("githubRepos 应是数组");
        assert_eq!(mentions.len(), 1);
        assert_eq!(
            sorted_keys(&mentions[0]),
            vec!["fullName", "inInbox", "starred"]
        );
        assert_eq!(mentions[0]["fullName"], "octocat/hello");
        assert_eq!(mentions[0]["starred"], true);
        assert_eq!(mentions[0]["inInbox"], false);
        assert!(item_json["repositoryId"].is_null());
        assert_eq!(item_json["status"], "pending");
        assert_eq!(item_json["platform"], "wechat");
        assert_eq!(item_json["sourceContext"], "微信群");
        let inbox_counts = &page_json["counts"];
        assert_eq!(
            sorted_keys(inbox_counts),
            vec!["archived", "failed", "pending", "ready"]
        );
        assert_eq!(inbox_counts["pending"], 2);
        assert_eq!(inbox_counts["ready"], 1);
        assert_eq!(inbox_counts["failed"], 0);
        assert_eq!(inbox_counts["archived"], 1);
        for key in ["pending", "ready", "failed", "archived"] {
            assert!(inbox_counts[key].is_number(), "{key} 应是数字");
        }
    }

    fn set_page_repos(connection: &Connection, id: &str, repos: &str) {
        connection
            .execute(
                "UPDATE inbox_items SET page_repos = ?1 WHERE id = ?2",
                params![repos, id],
            )
            .expect("应能写入 page_repos");
    }

    #[test]
    fn list_inbox_marks_starred_and_inbox_mentions() {
        let db = TempDb::new("repo-flags");
        let connection = Connection::open(db.path()).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref, updated_at) VALUES ('old', 'olduser', 'test', '2020-01-01T00:00:00.000Z')",
                [],
            )
            .expect("应能插入旧账号");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref, updated_at) VALUES ('acct', 'fox', 'test', '2026-10-03T00:00:00.000Z')",
                [],
            )
            .expect("应能插入当前账号");
        insert_repo(&connection, "hello", "acct", "OctoCat/Hello", "active");
        insert_repo(&connection, "gone", "acct", "gone/repo", "removed");
        insert_repo(&connection, "other", "old", "other/star", "active");
        insert_inbox(
            &connection,
            "post",
            "acct",
            "https://mp.weixin.qq.com/s/post",
            "pending",
            "2026-10-04T00:00:00.000Z",
            "wechat",
            "公众号",
            "",
            "推荐文章",
            "webpage",
            None,
        );
        insert_inbox(
            &connection,
            "hello-archived",
            "acct",
            "https://github.com/OctoCat/Hello",
            "archived",
            "2026-10-01T00:00:00.000Z",
            "github",
            "",
            "",
            "OctoCat/Hello",
            "github_repository",
            Some("hello"),
        );
        insert_inbox(
            &connection,
            "bar-archived",
            "acct",
            "https://GitHub.com/Foo/Bar",
            "archived",
            "2026-10-01T00:00:01.000Z",
            "github",
            "",
            "",
            "Foo/Bar",
            "github_repository",
            None,
        );
        insert_inbox(
            &connection,
            "lone",
            "acct",
            "https://github.com/lone/repo",
            "pending",
            "2026-10-03T00:00:00.000Z",
            "github",
            "",
            "",
            "lone/repo",
            "github_repository",
            None,
        );
        insert_inbox(
            &connection,
            "other-inbox",
            "old",
            "https://github.com/fresh/new",
            "pending",
            "2026-10-05T00:00:00.000Z",
            "github",
            "",
            "",
            "fresh/new",
            "github_repository",
            None,
        );
        insert_inbox(
            &connection,
            "bad-json",
            "acct",
            "https://example.com/bad",
            "pending",
            "2026-10-02T00:00:00.000Z",
            "web",
            "",
            "",
            "坏的",
            "webpage",
            None,
        );
        insert_inbox(
            &connection,
            "empty-json",
            "acct",
            "https://example.com/empty",
            "pending",
            "2026-10-02T00:00:01.000Z",
            "web",
            "",
            "",
            "空的",
            "webpage",
            None,
        );
        set_page_repos(
            &connection,
            "post",
            r#"["octocat/hello","Foo/Bar","gone/repo","other/star","fresh/new"]"#,
        );
        set_page_repos(&connection, "lone", r#"["lone/repo"]"#);
        set_page_repos(&connection, "bad-json", "not-json");
        set_page_repos(&connection, "empty-json", "[]");
        drop(connection);
        checkpoint(db.path());

        let page = list_inbox(db.path(), None, None, None).expect("列表应成功");
        assert_eq!(page.total_count, 4);
        assert_eq!(
            page.counts,
            InboxStatusCounts {
                pending: 4,
                ready: 0,
                failed: 0,
                archived: 2,
            }
        );
        assert_eq!(
            ids_of(&page),
            vec!["post", "lone", "empty-json", "bad-json"]
        );
        let post = page
            .items
            .iter()
            .find(|item| item.id == "post")
            .expect("应有文章");
        assert_eq!(post.github_repos.len(), 5);
        let mention = |name: &str| {
            post.github_repos
                .iter()
                .find(|item| item.full_name == name)
                .unwrap_or_else(|| panic!("应有 {name}"))
        };
        let hello = mention("octocat/hello");
        assert!(hello.starred, "大小写不同也应算已收藏");
        assert!(hello.in_inbox, "已归档的收集也算已在收集箱");
        let bar = mention("Foo/Bar");
        assert!(!bar.starred);
        assert!(bar.in_inbox, "链接大小写不同、且已归档，仍算已在收集箱");
        let gone = mention("gone/repo");
        assert!(!gone.starred, "取消收藏的不算");
        assert!(!gone.in_inbox);
        let other = mention("other/star");
        assert!(!other.starred, "别的账号的收藏不算");
        assert!(!other.in_inbox);
        let fresh = mention("fresh/new");
        assert!(!fresh.starred);
        assert!(!fresh.in_inbox, "别的账号的收集箱不算");
        let lone = page
            .items
            .iter()
            .find(|item| item.id == "lone")
            .expect("应有单独那条");
        assert_eq!(lone.github_repos.len(), 1);
        assert_eq!(lone.github_repos[0].full_name, "lone/repo");
        assert!(!lone.github_repos[0].starred);
        assert!(
            !lone.github_repos[0].in_inbox,
            "只有它自己这条时，不能算成别的收集"
        );
        assert!(page
            .items
            .iter()
            .find(|item| item.id == "bad-json")
            .expect("应有坏 JSON")
            .github_repos
            .is_empty());
        assert!(page
            .items
            .iter()
            .find(|item| item.id == "empty-json")
            .expect("应有空数组")
            .github_repos
            .is_empty());

        let value = serde_json::to_value(post).expect("条目应能序列化");
        assert!(value["githubRepos"].is_array());
        assert_eq!(
            sorted_keys(&value["githubRepos"][0]),
            vec!["fullName", "inInbox", "starred"]
        );
    }

    #[test]
    fn list_inbox_many_items_keep_repo_flags() {
        let db = TempDb::new("repo-many");
        let connection = Connection::open(db.path()).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .expect("应能插入账号");
        insert_repo(&connection, "shared", "acct", "Shared/Tool", "active");
        insert_inbox(
            &connection,
            "bulk-arch",
            "acct",
            "https://github.com/bulk/item-00",
            "archived",
            "2026-01-01T00:00:00.000Z",
            "github",
            "",
            "",
            "bulk/item-00",
            "github_repository",
            None,
        );
        for index in 0..80 {
            let id = format!("m{index:02}");
            let created = format!("2026-10-03T{:02}:{:02}:00.000Z", index / 60, index % 60);
            insert_inbox(
                &connection,
                &id,
                "acct",
                &format!("https://example.com/post-{index:02}"),
                "pending",
                &created,
                "wechat",
                "",
                "",
                "文章",
                "webpage",
                None,
            );
            set_page_repos(
                &connection,
                &id,
                &format!(r#"["bulk/item-{index:02}","shared/tool"]"#),
            );
        }
        drop(connection);
        checkpoint(db.path());

        let page = list_inbox(db.path(), None, Some(200), Some(0)).expect("很多条目也应能列出");
        assert_eq!(page.total_count, 80);
        assert_eq!(page.items.len(), 80);
        assert_eq!(page.items[0].id, "m79");
        assert_eq!(page.items[79].id, "m00");
        assert_eq!(
            page.counts,
            InboxStatusCounts {
                pending: 80,
                ready: 0,
                failed: 0,
                archived: 1,
            }
        );
        for (index, item) in page.items.iter().enumerate() {
            let source = 79 - index;
            assert_eq!(item.github_repos.len(), 2, "{}", item.id);
            assert_eq!(
                item.github_repos[0].full_name,
                format!("bulk/item-{source:02}")
            );
            assert!(!item.github_repos[0].starred);
            assert_eq!(item.github_repos[0].in_inbox, source == 0, "{}", item.id);
            assert_eq!(item.github_repos[1].full_name, "shared/tool");
            assert!(item.github_repos[1].starred, "大小写不同的收藏要标上");
            assert!(!item.github_repos[1].in_inbox);
        }
    }

    fn sorted_keys(value: &Value) -> Vec<String> {
        let mut keys = value
            .as_object()
            .expect("应是 JSON 对象")
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        keys
    }
}
