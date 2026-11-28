//! 原文、候选与出处。候选确认和出处写入共用一个事务。
use crate::capture::{
    extract_source_links, normalized_x_post_id, source_origin_link, source_url_link, ExtractedLink,
    MAX_SOURCE_LINKS, SOURCE_TEXT_LIMIT_BYTES,
};
use crate::inbox_view::CapturePreviewRow;
use crate::v2_ops::{
    account_id, capture_link_status, inbox_link_id, insert_inbox_link, normalize_capture_platform,
    open_db, short_hash, CaptureLinkStatus,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SourceInput {
    pub text: String,
    pub urls: Vec<String>,
    pub source_url: Option<String>,
    pub source_platform: String,
    pub source_context: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSource {
    pub source_id: String,
    pub created: bool,
    pub duplicate_submission: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    pub title: String,
    pub original_url: String,
    pub body: String,
    pub fetched_excerpt: String,
    pub candidates: Vec<SourceCandidate>,
}

#[derive(Debug, Serialize)]
pub struct SourceCandidate {
    #[serde(flatten)]
    pub row: CapturePreviewRow,
    pub linked: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectedCandidates {
    pub created: usize,
    pub linked: usize,
    pub source: SourceView,
}

#[derive(Debug)]
pub enum SaveSourceError {
    Invalid(String),
    KeyConflict,
    Storage(String),
}

impl fmt::Display for SaveSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) | Self::Storage(message) => f.write_str(message),
            Self::KeyConflict => f.write_str("同一个投递编号已用于不同内容"),
        }
    }
}

impl From<String> for SaveSourceError {
    fn from(value: String) -> Self {
        Self::Storage(value)
    }
}

pub fn save_source(
    path: &Path,
    mut input: SourceInput,
    via: &str,
) -> Result<SavedSource, SaveSourceError> {
    if input.text.len() > SOURCE_TEXT_LIMIT_BYTES
        || input.urls.len() > MAX_SOURCE_LINKS
        || input.urls.iter().map(|url| url.len()).sum::<usize>() > SOURCE_TEXT_LIMIT_BYTES
    {
        return Err(SaveSourceError::Invalid("内容太长，请分次投递".to_owned()));
    }
    if input.text.trim().is_empty() {
        input.text.clear();
    }
    if input.text.is_empty() && input.urls.is_empty() && input.source_url.is_none() {
        return Err(SaveSourceError::Invalid("请先粘贴链接或正文".to_owned()));
    }
    let source_link = input
        .source_url
        .as_deref()
        .map(|url| {
            source_origin_link(url).ok_or_else(|| {
                SaveSourceError::Invalid("原文链接无效，或不是公开网页地址".to_owned())
            })
        })
        .transpose()?;
    let urls = input
        .urls
        .iter()
        .map(|url| {
            source_url_link(url)
                .map(|link| link.normalized_url)
                .ok_or_else(|| SaveSourceError::Invalid("有候选链接无效或无法收集".to_owned()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let key = input.idempotency_key.as_deref().map(str::trim);
    if key.is_some_and(|key| key.is_empty() || key.len() > 200 || key.chars().any(char::is_control))
    {
        return Err(SaveSourceError::Invalid("投递编号无效".to_owned()));
    }
    let platform = match (&source_link, input.source_platform.trim()) {
        (Some(link), _) if link.kind == "x_post" => "x",
        (_, "feishu") => "feishu",
        (_, "douyin") => "douyin",
        (Some(link), "" | "other") => link.platform.as_str(),
        (_, platform) => normalize_capture_platform(platform),
    };
    let context: String = input.source_context.trim().chars().take(2000).collect();
    let source_key = match &source_link {
        Some(link) => link.normalized_url.clone(),
        None => {
            let identity = source_json(&(platform, input.text.trim(), &urls))?;
            format!("capture-text:{}", short_hash(&identity))
        }
    };
    let identity = normalized_x_post_id(&source_key)
        .map_or_else(|| source_key.clone(), |id| format!("x-post:{id}"));
    let fingerprint = key
        .map(|_| source_json(&(&identity, platform, &context, &input.text, &urls)))
        .transpose()?
        .map(|value| short_hash(&value));

    let mut conn = open_db(path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_error)?;
    let account = account_id(&tx)?;
    if let Some((key, fingerprint)) = key.zip(fingerprint.as_deref()) {
        let receipt: Option<(String, String)> = tx
            .query_row(
                "SELECT request_hash, source_id FROM capture_source_receipts
                 WHERE account_id = ?1 AND request_key = ?2",
                params![account, key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(write_error)?;
        if let Some((previous, source_id)) = receipt {
            if previous != fingerprint {
                return Err(SaveSourceError::KeyConflict);
            }
            return Ok(SavedSource {
                source_id,
                created: false,
                duplicate_submission: true,
            });
        }
    }

    let existing = find_source_id(&tx, &account, &source_key)?;
    let created = existing.is_none();
    let source_id = existing.unwrap_or_else(|| link_id("source", &account, &identity));
    if created {
        match &source_link {
            Some(link) => insert_inbox_link(&tx, &source_id, &account, link, &context, via, None)?,
            None => {
                let title = input
                    .text
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(|line| {
                        line.trim()
                            .chars()
                            .filter(|ch| !ch.is_control())
                            .take(72)
                            .collect::<String>()
                    })
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| "链接清单".to_owned());
                tx.execute(
                    "INSERT INTO inbox_items(id, account_id, original_url, normalized_url, title, item_type,
                         source_platform, source_context, captured_via, fetch_status)
                     VALUES (?1, ?2, '', ?3, ?4, 'document', ?5, ?6, ?7, 'skipped')",
                    params![source_id, account, source_key, title, normalize_capture_platform(platform), context, via],
                ).map_err(write_error)?;
            }
        }
    }
    let prior_urls: Option<String> = tx
        .query_row(
            "SELECT supplied_urls FROM capture_sources WHERE source_id = ?1",
            [&source_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(write_error)?;
    let mut supplied = parse_urls(prior_urls.as_deref().unwrap_or("[]"))?;
    let mut seen: HashSet<String> = supplied.iter().cloned().collect();
    for url in urls {
        if seen.insert(url.clone()) {
            supplied.push(url);
        }
    }
    if supplied.len() > MAX_SOURCE_LINKS {
        return Err(SaveSourceError::Invalid(
            "一条来源最多保留 100 个投递链接".to_owned(),
        ));
    }
    tx.execute(
        "INSERT INTO capture_sources(source_id, platform, body, supplied_urls) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(source_id) DO UPDATE SET
             body = CASE
                 WHEN excluded.body <> '' AND (?5 OR capture_sources.body = '') THEN excluded.body
                 ELSE capture_sources.body
             END,
             supplied_urls = excluded.supplied_urls",
        params![source_id, platform, input.text, source_json(&supplied)?, source_link.is_some()],
    ).map_err(write_error)?;
    if let Some((key, fingerprint)) = key.zip(fingerprint.as_deref()) {
        tx.execute(
            "INSERT INTO capture_source_receipts(account_id, request_key, request_hash, source_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![account, key, fingerprint, source_id],
        )
        .map_err(write_error)?;
    }
    tx.commit().map_err(write_error)?;
    Ok(SavedSource {
        source_id,
        created,
        duplicate_submission: false,
    })
}

fn find_source_id(conn: &Connection, account: &str, key: &str) -> Result<Option<String>, String> {
    let exact = inbox_link_id(conn, account, key)?;
    let Some(id) = normalized_x_post_id(key).filter(|_| exact.is_none()) else {
        return Ok(exact);
    };
    // 分享链接上的用户名或 i/web 路径可能变化，帖文编号仍是同一个。
    let mut statement = conn
        .prepare(
            "SELECT id, normalized_url FROM inbox_items
         WHERE account_id = ?1 AND source_platform = 'x'
           AND normalized_url LIKE 'https://x.com/%/status/' || ?2
         ORDER BY created_at ASC, id ASC",
        )
        .map_err(read_error)?;
    let rows = statement
        .query_map(params![account, id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(read_error)?;
    for row in rows {
        let (source_id, url) = row.map_err(read_error)?;
        if normalized_x_post_id(&url) == Some(id) {
            return Ok(Some(source_id));
        }
    }
    Ok(None)
}

pub fn get_source(path: &Path, id: &str) -> Result<SourceView, String> {
    let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("数据库打开失败：{error}"))?;
    conn.busy_timeout(Duration::from_millis(5000))
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    let tx = conn.transaction().map_err(read_error)?;
    let account = account_id(&tx)?;
    let source = load_source(&tx, &account, id)?;
    source_view(&tx, &account, id, source)
}

pub fn save_body(path: &Path, id: &str, text: &str) -> Result<SourceView, String> {
    if text.trim().is_empty() {
        return Err("请先粘贴正文".to_owned());
    }
    if text.len() > SOURCE_TEXT_LIMIT_BYTES {
        return Err("正文太长，请缩短后再保存".to_owned());
    }
    let mut conn = open_db(path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_error)?;
    let account = account_id(&tx)?;
    let mut source = load_source(&tx, &account, id)?;
    tx.execute(
        "INSERT INTO capture_sources(source_id, platform, body) VALUES (?1, ?2, ?3)
         ON CONFLICT(source_id) DO UPDATE SET body = excluded.body",
        params![id, source.platform, text],
    )
    .map_err(write_error)?;
    source.body = text.to_owned();
    let view = source_view(&tx, &account, id, source)?;
    tx.commit().map_err(write_error)?;
    Ok(view)
}

pub fn collect_candidates(
    path: &Path,
    id: &str,
    urls: &[String],
) -> Result<CollectedCandidates, String> {
    if urls.is_empty() || urls.len() > MAX_SOURCE_LINKS {
        return Err("请选择要放进收集箱的候选链接".to_owned());
    }
    let selected: HashSet<&str> = urls.iter().map(String::as_str).collect();
    let mut conn = open_db(path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_error)?;
    let account = account_id(&tx)?;
    let source = load_source(&tx, &account, id)?;
    let links = source.candidates()?;
    let matching: Vec<_> = links
        .into_iter()
        .filter(|link| {
            link.blocked_reason.is_none() && selected.contains(link.normalized_url.as_str())
        })
        .collect();
    if matching.len() != selected.len() {
        return Err("部分候选已变化，请重新打开原文后再选".to_owned());
    }
    let mut created = 0;
    let mut linked = 0;
    for link in matching {
        if capture_link_status(&tx, &account, &link)? == CaptureLinkStatus::New {
            let item_id = link_id("capture", &account, &link.normalized_url);
            insert_inbox_link(&tx, &item_id, &account, &link, "", "paste_batch", None)?;
            created += 1;
        }
        linked += tx
            .execute(
                "INSERT INTO capture_source_links(source_id, target_url) VALUES (?1, ?2)
             ON CONFLICT(source_id, target_url) DO NOTHING",
                params![id, link.normalized_url],
            )
            .map_err(write_error)?;
    }
    let source = source_view(&tx, &account, id, source)?;
    tx.commit().map_err(write_error)?;
    Ok(CollectedCandidates {
        created,
        linked,
        source,
    })
}

struct Source {
    title: String,
    original_url: String,
    normalized_url: String,
    platform: String,
    body: String,
    fetched_excerpt: String,
    supplied_urls: String,
    fetched_urls: String,
}

impl Source {
    fn candidates(&self) -> Result<Vec<ExtractedLink>, String> {
        let body = if self.body.is_empty() {
            &self.fetched_excerpt
        } else {
            &self.body
        };
        let supplied = parse_urls(&self.supplied_urls)?.join("\n");
        let fetched = parse_urls(&self.fetched_urls)?.join("\n");
        let links = extract_source_links(&format!("{body}\n{supplied}\n{fetched}"));
        let own_post = normalized_x_post_id(&self.normalized_url);
        Ok(links
            .into_iter()
            .filter(|link| {
                link.normalized_url != self.normalized_url
                    && !own_post
                        .is_some_and(|id| normalized_x_post_id(&link.normalized_url) == Some(id))
            })
            .collect())
    }
}

fn load_source(conn: &Connection, account: &str, id: &str) -> Result<Source, String> {
    let (source, stored): (Source, bool) = conn
        .query_row(
            "SELECT i.title, i.original_url, i.normalized_url, COALESCE(s.platform, i.source_platform),
                    COALESCE(s.body, ''), i.page_excerpt, COALESCE(s.supplied_urls, '[]'),
                    i.page_capture_links, s.source_id IS NOT NULL
             FROM inbox_items i LEFT JOIN capture_sources s ON s.source_id = i.id
             WHERE i.id = ?1 AND i.account_id = ?2",
            params![id, account],
            |row| Ok((Source {
                title: row.get(0)?,
                original_url: row.get(1)?,
                normalized_url: row.get(2)?,
                platform: row.get(3)?,
                body: row.get(4)?,
                fetched_excerpt: row.get(5)?,
                supplied_urls: row.get(6)?,
                fetched_urls: row.get(7)?,
            }, row.get(8)?)),
        )
        .optional().map_err(read_error)?
        .ok_or_else(|| "找不到这条收集".to_owned())?;
    if !stored && normalized_x_post_id(&source.normalized_url).is_none() {
        return Err("这条收集没有原文".to_owned());
    }
    Ok(source)
}

fn source_view(
    conn: &Connection,
    account: &str,
    id: &str,
    source: Source,
) -> Result<SourceView, String> {
    let mut statement = conn
        .prepare("SELECT target_url FROM capture_source_links WHERE source_id = ?1")
        .map_err(read_error)?;
    let linked = statement
        .query_map([id], |row| row.get::<_, String>(0))
        .map_err(read_error)?
        .collect::<rusqlite::Result<HashSet<_>>>()
        .map_err(read_error)?;
    let candidates = source
        .candidates()?
        .into_iter()
        .map(|link| {
            let status = capture_link_status(conn, account, &link)?;
            let linked = linked.contains(&link.normalized_url) && status != CaptureLinkStatus::New;
            Ok(SourceCandidate {
                row: CapturePreviewRow::from_link(link, status),
                linked,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(SourceView {
        title: source.title,
        original_url: source.original_url,
        body: source.body,
        fetched_excerpt: source.fetched_excerpt,
        candidates,
    })
}

fn link_id(prefix: &str, account: &str, url: &str) -> String {
    format!("{prefix}-{}", short_hash(&format!("{account}\n{url}")))
}

fn parse_urls(json: &str) -> Result<Vec<String>, String> {
    serde_json::from_str(json).map_err(|_| "来源链接读不出来".to_owned())
}

fn source_json(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|_| "来源链接写不进去".to_owned())
}

fn read_error(error: rusqlite::Error) -> String {
    format!("原文读取失败：{error}")
}

fn write_error(error: rusqlite::Error) -> String {
    format!("原文保存失败：{error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::{insert_test_account, migrate_database};
    use crate::inbox_fetch::{fetch_pending, FetchProblem, PageFetcher, PageResponse};
    use crate::inbox_view::{list_inbox, set_inbox_note};
    use crate::v2_ops::commit_capture;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const SKILL_A: &str = "https://github.com/acme/skills/blob/main/one/SKILL.md";
    const SKILL_B: &str = "https://github.com/acme/skills/blob/main/two/SKILL.md";
    const POST: &str = "https://x.com/test/status/123";

    struct TestDb {
        root: PathBuf,
        path: PathBuf,
    }

    impl TestDb {
        fn new() -> Self {
            static SEQ: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "fsl-capture-source-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("temp.sqlite3");
            migrate_database(&path).unwrap();
            insert_test_account(&path);
            Self { root, path }
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    struct UnavailablePost;

    impl PageFetcher for UnavailablePost {
        fn get(&self, url: &str) -> Result<PageResponse, FetchProblem> {
            assert!(url.starts_with("https://publish.twitter.com/oembed?"));
            Ok(PageResponse {
                status: 404,
                content_type: "application/json".to_owned(),
                body: "{}".to_owned(),
                final_url: url.to_owned(),
                location: None,
            })
        }
    }

    #[test]
    fn skill_candidates_receive_readable_titles() {
        let db = TestDb::new();
        let urls = [
            "https://github.com/Acme/Repo/blob/main/a/SKILL.md",
            "https://github.com/Acme/Repo/blob/main/b/skill.md",
            "https://raw.githubusercontent.com/Acme/Repo/main/c/SKILL.md",
            "https://github.com/Acme/Repo/tree/main/skills/demo",
        ];
        let saved = save_source(
            &db.path,
            SourceInput {
                text: urls.join(" "),
                ..SourceInput::default()
            },
            "paste_batch",
        )
        .unwrap();
        collect_candidates(
            &db.path,
            &saved.source_id,
            &extract_source_links(&urls.join(" "))
                .into_iter()
                .map(|link| link.normalized_url)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let inbox = list_inbox(&db.path, None, None, None).unwrap();
        for title in [
            "Acme/Repo · a/SKILL.md",
            "Acme/Repo · b/skill.md",
            "Acme/Repo · c/SKILL.md",
            "Acme/Repo · skills/demo",
        ] {
            assert!(
                inbox.items.iter().any(|item| item.title == title),
                "缺少 {title}"
            );
        }
    }

    #[test]
    fn source_id_rejects_malformed_x_urls_matching_like() {
        let db = TestDb::new();
        let conn = Connection::open(&db.path).unwrap();
        let account = account_id(&conn).unwrap();
        conn.execute("INSERT INTO inbox_items(id, account_id, original_url, normalized_url, title, item_type, source_platform, created_at) VALUES ('bad', ?1, 'x', 'https://x.com/a/b/status/123', '', 'webpage', 'x', '2020')", params![account]).unwrap();
        conn.execute("INSERT INTO inbox_items(id, account_id, original_url, normalized_url, title, item_type, source_platform, created_at) VALUES ('good', ?1, 'x', 'https://x.com/valid/status/123', '', 'webpage', 'x', '2021')", params![account]).unwrap();
        assert_eq!(
            find_source_id(&conn, &account, "https://x.com/other/status/123")
                .unwrap()
                .as_deref(),
            Some("good")
        );
    }

    #[test]
    fn unavailable_post_accepts_a_body_and_keeps_only_the_selected_skill() {
        let db = TestDb::new();
        commit_capture(&db.path, POST, "原来的来源说明").unwrap();
        let post = list_inbox(&db.path, None, None, None)
            .unwrap()
            .items
            .remove(0);
        assert!(post.can_open_source);
        assert!(!post.has_source_body);
        set_inbox_note(&db.path, &post.id, "原来的备注").unwrap();
        let fetched = fetch_pending(&db.path, &UnavailablePost, Some(&post.id), 1).unwrap();
        assert_eq!(fetched.failed, 1);
        let text = format!("两个不同的工具\n{SKILL_A}\n{SKILL_B}\n回看：{POST}");
        let view = save_body(&db.path, &post.id, &text).unwrap();
        assert_eq!(view.body, text);
        assert_eq!(view.original_url, POST);
        assert_eq!(view.candidates.len(), 2);
        assert!(view
            .candidates
            .iter()
            .all(|candidate| candidate.row.kind == "github_skill"));
        let urls = vec![SKILL_B.to_owned()];
        let result = collect_candidates(&db.path, &post.id, &urls).unwrap();
        assert_eq!((result.created, result.linked), (1, 1));
        assert!(!result.source.candidates[0].linked);
        assert!(result.source.candidates[1].linked);
        let repeated = collect_candidates(&db.path, &post.id, &urls).unwrap();
        assert_eq!((repeated.created, repeated.linked), (0, 0));

        let inbox = list_inbox(&db.path, None, None, None).unwrap();
        assert_eq!(inbox.items.len(), 2);
        let saved = inbox.items.iter().find(|item| item.id == post.id).unwrap();
        assert_eq!(saved.user_note, "原来的备注");
        assert_eq!(saved.source_context, "原来的来源说明");
        assert_eq!(saved.fetch_status, "failed");
        assert!(saved.has_source_body);
        let skill = inbox
            .items
            .iter()
            .find(|item| item.normalized_url == SKILL_B)
            .unwrap();
        assert_eq!(skill.sources.len(), 1);
        assert_eq!(skill.sources[0].id, post.id);
    }

    #[test]
    fn adding_a_candidate_from_two_sources_keeps_one_item_and_two_origins() {
        let db = TestDb::new();
        let post = |url: &str| SourceInput {
            source_url: Some(url.to_owned()),
            text: SKILL_A.to_owned(),
            ..SourceInput::default()
        };
        let first = save_source(&db.path, post(POST), "paste_batch").unwrap();
        let mut repeated = post("https://twitter.com/i/web/status/123?s=20");
        repeated.text = format!("补充到原帖\n{SKILL_A}\n{POST}");
        let again = save_source(&db.path, repeated, "paste_batch").unwrap();
        assert!(!again.created);
        assert_eq!(again.source_id, first.source_id);
        let mut link_only = post(POST);
        link_only.text.clear();
        save_source(&db.path, link_only, "paste_batch").unwrap();
        let view = get_source(&db.path, &first.source_id).unwrap();
        assert!(view.body.starts_with("补充到原帖"));
        assert_eq!(view.candidates.len(), 1);
        let second = save_source(
            &db.path,
            post("https://x.com/example/status/456"),
            "local_api",
        )
        .unwrap();
        let urls = vec![SKILL_A.to_owned()];
        assert_eq!(
            collect_candidates(&db.path, &first.source_id, &urls)
                .unwrap()
                .created,
            1
        );
        let view = get_source(&db.path, &second.source_id).unwrap();
        assert_eq!(view.candidates[0].row.status, "inbox");
        assert!(!view.candidates[0].linked);
        let next = collect_candidates(&db.path, &second.source_id, &urls).unwrap();
        assert_eq!((next.created, next.linked), (0, 1));
        let inbox = list_inbox(&db.path, None, None, None).unwrap();
        assert_eq!(inbox.items.len(), 3);
        let item = inbox
            .items
            .iter()
            .find(|item| item.normalized_url == SKILL_A)
            .unwrap();
        let origins: HashSet<&str> = item.sources.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(
            origins,
            HashSet::from([first.source_id.as_str(), second.source_id.as_str()])
        );
    }

    #[test]
    fn plain_text_is_saved_without_a_link_and_preserves_line_breaks() {
        let db = TestDb::new();
        let post = || SourceInput {
            text: "一段原话\n\n  保留缩进".to_owned(),
            source_platform: "feishu".to_owned(),
            ..SourceInput::default()
        };
        let saved = save_source(&db.path, post(), "local_api").unwrap();
        assert!(saved.created);
        let found = get_source(&db.path, &saved.source_id).unwrap();
        assert_eq!(found.body, post().text);
        assert!(found.original_url.is_empty());
        assert!(found.candidates.is_empty());
        let again = save_source(&db.path, post(), "local_api").unwrap();
        assert_eq!(again.source_id, saved.source_id);
        assert!(!again.created);
        let inbox = list_inbox(&db.path, None, None, None).unwrap();
        assert_eq!(inbox.items.len(), 1);
        assert_eq!(inbox.items[0].item_type, "document");
        assert_eq!(inbox.items[0].platform, "feishu");
        assert_eq!(inbox.items[0].fetch_status, "skipped");
        assert_eq!(inbox.items[0].quote, post().text);
    }

    #[test]
    fn candidate_selection_rechecks_body_and_handles_malformed_long_lines() {
        let db = TestDb::new();
        let text = format!("https://example.org/{}。\n{SKILL_B}", ")".repeat(30_000));
        let post = save_source(
            &db.path,
            SourceInput {
                text,
                ..SourceInput::default()
            },
            "paste_batch",
        )
        .unwrap();
        let after_edit = save_body(&db.path, &post.source_id, SKILL_A).unwrap();
        assert_eq!(after_edit.candidates.len(), 1);
        assert!(collect_candidates(
            &db.path,
            &post.source_id,
            &[SKILL_A.to_owned(), SKILL_B.to_owned()]
        )
        .is_err());
        assert!(save_body(&db.path, &post.source_id, &"字".repeat(30_000)).is_err());
        assert_eq!(get_source(&db.path, &post.source_id).unwrap().body, SKILL_A);
        assert_eq!(
            list_inbox(&db.path, None, None, None).unwrap().items.len(),
            1
        );
    }

    #[test]
    fn distinct_github_subpages_keep_separate_sources_without_overwriting() {
        let db = TestDb::new();
        let issue1 = save_source(
            &db.path,
            SourceInput {
                source_url: Some(
                    "https://github.com/Acme/Repo/issues/1?utm_source=x#top".to_owned(),
                ),
                text: "第一个 issue 的工具讨论".to_owned(),
                ..SourceInput::default()
            },
            "local_api",
        )
        .unwrap();
        let issue2 = save_source(
            &db.path,
            SourceInput {
                source_url: Some("https://github.com/acme/repo/issues/2".to_owned()),
                text: "第二个 issue 的工具讨论".to_owned(),
                ..SourceInput::default()
            },
            "local_api",
        )
        .unwrap();
        assert_ne!(issue1.source_id, issue2.source_id);
        let one = get_source(&db.path, &issue1.source_id).unwrap();
        let two = get_source(&db.path, &issue2.source_id).unwrap();
        assert_eq!(
            one.original_url,
            "https://github.com/Acme/Repo/issues/1?utm_source=x#top"
        );
        assert_eq!(one.body, "第一个 issue 的工具讨论");
        assert_eq!(two.body, "第二个 issue 的工具讨论");
    }
}
