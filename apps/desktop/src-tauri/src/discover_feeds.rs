//! 发现页趋势榜与自由搜索：
//! - 支持 GitHub Trending（今日/本周，带 Search API 自动兜底）
//! - 支持近期新锐（7 天 / 30 天新建高星）
//! - 支持 Hacker News Show HN（仅保留 GitHub 项目）
//! - 支持自由搜索 GitHub 项目，并自动复用已连接的 GitHub 令牌（仅限 api.github.com）

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::Value;

use crate::discover::{active_names, in_inbox, inbox_links};
use crate::web_fetch::{
    format_unix_time, rate_limit_wait_secs, since_date, url_encode, FetchError, HttpFetcher,
    HttpResponse,
};

const MAX_DESCRIPTION_CHARS: usize = 300;
const FEED_LIMIT: usize = 25;
const TRENDING_PARSE_CAP: usize = 40;
const SEARCH_MAX_QUERY_CHARS: usize = 200;
const RATE_LIMIT_MESSAGE: &str =
    "GitHub 暂时不让查太频繁，过几分钟再点「刷新」，已找到的先列在下面";
const TRENDING_FALLBACK_MESSAGE: &str = "GitHub 趋势页暂时读不到，已自动改用近期高星项目兜底";
const GITHUB_API_HEADERS: &[(&str, &str)] = &[
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", "2022-11-28"),
];
const TRENDING_HTML_HEADERS: &[(&str, &str)] = &[(
    "Accept",
    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
)];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverFeedItem {
    pub rank: i64,
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars_count: i64,
    pub stars_delta: Option<i64>,
    pub delta_label: Option<String>,
    pub pushed_at: Option<String>,
    pub created_at: Option<String>,
    pub topics: Vec<String>,
    pub source_note: Option<String>,
    pub mine: bool,
    pub in_inbox: bool,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverFeedView {
    pub feed_id: String,
    pub language: Option<String>,
    pub feed_key: String,
    pub last_fetched_at: Option<String>,
    pub used_fallback: bool,
    pub message: Option<String>,
    pub items: Vec<DiscoverFeedItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverFeedRefreshReport {
    pub feed_id: String,
    pub language: Option<String>,
    pub feed_key: String,
    pub fetched: i64,
    pub skipped_mine: i64,
    pub used_fallback: bool,
    pub message: Option<String>,
    pub rate_limit_reset_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverSearchResult {
    pub query: String,
    pub total_count: i64,
    pub items: Vec<DiscoverFeedItem>,
    pub message: Option<String>,
    pub rate_limit_reset_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FeedRepo {
    full_name: String,
    html_url: String,
    description: Option<String>,
    language: Option<String>,
    stars_count: i64,
    stars_delta: Option<i64>,
    delta_label: Option<String>,
    pushed_at: Option<String>,
    created_at: Option<String>,
    topics: Vec<String>,
    source_note: Option<String>,
}

fn open_db(path: &Path) -> Result<Connection, String> {
    crate::v2_ops::open_db(path)
}

fn latest_account_opt(connection: &Connection) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT id FROM github_accounts ORDER BY updated_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("账号读取失败：{error}"))
}

fn load_ownership(connection: &Connection) -> Result<(HashSet<String>, HashSet<String>), String> {
    match latest_account_opt(connection)? {
        Some(account_id) => Ok((
            active_names(connection, &account_id)?,
            inbox_links(connection, &account_id)?,
        )),
        None => Ok((HashSet::new(), HashSet::new())),
    }
}

/// 请求 GitHub REST API。仅在目标严格为 `https://api.github.com/...` 时附加令牌；
/// 若带令牌返回 401（如令牌已失效），自动降级不带令牌重试一次。
pub(crate) fn github_api_get(
    fetcher: &impl HttpFetcher,
    url: &str,
    github_token: Option<&str>,
) -> Result<HttpResponse, FetchError> {
    let token = github_token
        .map(str::trim)
        .filter(|value| !value.is_empty() && url.starts_with("https://api.github.com/"));
    if let Some(token) = token {
        let auth_value = format!("Bearer {token}");
        let headers = [
            ("Accept", "application/vnd.github+json"),
            ("X-GitHub-Api-Version", "2022-11-28"),
            ("Authorization", auth_value.as_str()),
        ];
        match fetcher.get(url, &headers) {
            Ok(response) if response.status == 401 => fetcher.get(url, GITHUB_API_HEADERS),
            other => other,
        }
    } else {
        fetcher.get(url, GITHUB_API_HEADERS)
    }
}

fn validate_feed_id(feed_id: &str) -> Result<&'static str, String> {
    match feed_id.trim() {
        "trending_daily" => Ok("trending_daily"),
        "trending_weekly" => Ok("trending_weekly"),
        "rising_7d" => Ok("rising_7d"),
        "rising_30d" => Ok("rising_30d"),
        "hn_show" => Ok("hn_show"),
        _ => Err("不支持的榜单类型".to_owned()),
    }
}

fn normalize_language(language: Option<&str>) -> Result<Option<String>, String> {
    let Some(raw) = language.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let lower = raw.to_ascii_lowercase();
    if lower.len() > 32
        || !lower.bytes().all(|byte| {
            matches!(
                byte,
                b'a'..=b'z' | b'0'..=b'9' | b'+' | b'#' | b'.' | b'_' | b'-'
            )
        })
    {
        return Err("语言筛选格式不支持".to_owned());
    }
    Ok(Some(lower))
}

fn build_feed_key(feed_id: &str, language: Option<&str>) -> String {
    match language {
        Some(lang) => format!("{feed_id}:{lang}"),
        None => feed_id.to_owned(),
    }
}

fn format_thousands(value: i64) -> String {
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if negative {
        out.push('-');
    }
    out.chars().rev().collect()
}

fn format_reset_time(wait_secs: u64, now: i64) -> Result<String, String> {
    let wait = i64::try_from(wait_secs).map_err(|_| "等待时间无效".to_owned())?;
    let reset_at = now
        .checked_add(wait)
        .ok_or_else(|| "等待时间无效".to_owned())?;
    format_unix_time(reset_at)
}

fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp..];
        if let Some(semi) = after.find(';').filter(|idx| *idx <= 10) {
            let entity = &after[..=semi];
            let decoded = match entity {
                "&amp;" => Some("&"),
                "&lt;" => Some("<"),
                "&gt;" => Some(">"),
                "&quot;" => Some("\""),
                "&#39;" | "&#x27;" | "&apos;" => Some("'"),
                "&nbsp;" => Some(" "),
                _ => None,
            };
            if let Some(value) = decoded {
                out.push_str(value);
                rest = &after[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[amp + 1..];
    }
    out.push_str(rest);
    out
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clean_html_text(raw: &str) -> String {
    collapse_whitespace(&decode_html_entities(&strip_html_tags(raw)))
}

fn parse_comma_int(raw: &str) -> Option<i64> {
    let mut started = false;
    let mut digits = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            started = true;
            digits.push(ch);
        } else if ch == ',' && started {
            continue;
        } else if started {
            break;
        }
    }
    if digits.is_empty() {
        None
    } else {
        digits.parse::<i64>().ok()
    }
}

fn validate_repo_slug(owner: &str, repo: &str) -> Option<String> {
    let owner = owner.trim();
    let repo = repo.trim().trim_end_matches(".git");
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    let reserved = [
        "topics",
        "sponsors",
        "explore",
        "trending",
        "orgs",
        "users",
        "marketplace",
        "settings",
        "search",
        "collections",
        "events",
        "features",
        "about",
        "pricing",
        "login",
        "signup",
        "apps",
    ];
    if reserved.iter().any(|item| owner.eq_ignore_ascii_case(item)) {
        return None;
    }
    let valid_part = |part: &str| {
        !part.contains(char::is_whitespace)
            && !part.contains('?')
            && !part.contains('#')
            && !part.contains('<')
            && !part.contains('>')
            && !part.contains('"')
    };
    if valid_part(owner) && valid_part(repo) {
        Some(format!("{owner}/{repo}"))
    } else {
        None
    }
}

fn parse_trending_article(article: &str, period: &str) -> Option<FeedRepo> {
    let h2_start = article.find("<h2")?;
    let h2_end_rel = article[h2_start..].find("</h2>")?;
    let h2 = &article[h2_start..h2_start + h2_end_rel];

    let href_marker = "href=\"/";
    let mut full_name = None;
    let mut cursor = h2;
    while let Some(pos) = cursor.find(href_marker) {
        let after = &cursor[pos + href_marker.len()..];
        let Some(quote) = after.find('"') else {
            break;
        };
        let path = &after[..quote];
        let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        if parts.len() == 2 {
            if let Some(slug) = validate_repo_slug(parts[0], parts[1]) {
                full_name = Some(slug);
                break;
            }
        }
        cursor = &after[quote + 1..];
    }
    let full_name = full_name?;
    let html_url = format!("https://github.com/{full_name}");

    let description = article.find("<p").and_then(|p_start| {
        let gt = article[p_start..].find('>')?;
        let after_gt = &article[p_start + gt + 1..];
        let p_end = after_gt.find("</p>")?;
        let text = clean_html_text(&after_gt[..p_end]);
        if text.is_empty() || text.chars().count() > MAX_DESCRIPTION_CHARS {
            None
        } else {
            Some(text)
        }
    });

    let language = article
        .find("itemprop=\"programmingLanguage\"")
        .and_then(|pos| {
            let after_attr = &article[pos..];
            let gt = after_attr.find('>')?;
            let inside = &after_attr[gt + 1..];
            let end = inside.find("</span>")?;
            let text = clean_html_text(&inside[..end]);
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        });

    let stars_count = article
        .find("/stargazers\"")
        .and_then(|pos| {
            let after = &article[pos..];
            let gt = after.find('>')?;
            let inside = &after[gt + 1..];
            let end = inside.find("</a>")?;
            let text = clean_html_text(&inside[..end]);
            parse_comma_int(&text)
        })
        .unwrap_or(0);

    let stars_delta = article.find("float-sm-right").and_then(|pos| {
        let after = &article[pos..];
        let gt = after.find('>')?;
        let inside = &after[gt + 1..];
        let end = inside.find("</span>")?;
        let text = clean_html_text(&inside[..end]);
        let before_star = text
            .to_ascii_lowercase()
            .find("star")
            .map(|idx| &text[..idx])
            .unwrap_or(text.as_str());
        parse_comma_int(before_star)
    });

    let delta_label = stars_delta.map(|delta| {
        let unit = if period == "weekly" {
            "本周"
        } else {
            "今日"
        };
        format!("+{} {unit}", format_thousands(delta))
    });

    Some(FeedRepo {
        full_name,
        html_url,
        description,
        language,
        stars_count,
        stars_delta,
        delta_label,
        pushed_at: None,
        created_at: None,
        topics: Vec::new(),
        source_note: Some("GitHub 趋势榜".to_owned()),
    })
}

/// 零正则、单遍线性解析 GitHub Trending HTML 页面。
fn parse_github_trending_html(html: &str, period: &str) -> Vec<FeedRepo> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    let mut rest = html;

    while items.len() < TRENDING_PARSE_CAP {
        let Some(start) = rest.find("<article") else {
            break;
        };
        let from_article = &rest[start..];
        let Some(tag_end) = from_article.find('>') else {
            break;
        };
        let open_tag = &from_article[..=tag_end];
        if !open_tag.contains("Box-row") {
            rest = &from_article[tag_end + 1..];
            continue;
        }
        let Some(close) = from_article.find("</article>") else {
            break;
        };
        let article_block = &from_article[..close + "</article>".len()];
        if let Some(repo) = parse_trending_article(article_block, period) {
            let key = repo.full_name.to_ascii_lowercase();
            if seen.insert(key) {
                items.push(repo);
            }
        }
        rest = &from_article[close + "</article>".len()..];
    }

    items
}

fn text_opt(item: &Value, key: &str) -> Option<String> {
    match item.get(key) {
        Some(Value::String(text)) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        _ => None,
    }
}

fn flag(item: &Value, key: &str) -> bool {
    item.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn parse_github_search_feed(
    body: &str,
    source_note: Option<&str>,
) -> Result<(i64, Vec<FeedRepo>), String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "搜索结果读不出来".to_owned())?;
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| "搜索结果读不出来".to_owned())?;
    let total_count = value
        .get("total_count")
        .and_then(Value::as_i64)
        .unwrap_or(items.len() as i64);

    let mut repos = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        if flag(item, "archived") || flag(item, "fork") || flag(item, "disabled") {
            continue;
        }
        let Some(full_name) = text_opt(item, "full_name") else {
            continue;
        };
        let Some(html_url) = text_opt(item, "html_url") else {
            continue;
        };
        let description = text_opt(item, "description")
            .filter(|text| text.chars().count() <= MAX_DESCRIPTION_CHARS);
        let key = full_name.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        let topics = match item.get("topics") {
            Some(Value::Array(list)) => list
                .iter()
                .filter_map(Value::as_str)
                .filter(|topic| !topic.is_empty())
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        repos.push(FeedRepo {
            full_name,
            html_url,
            description,
            language: text_opt(item, "language"),
            stars_count: item
                .get("stargazers_count")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            stars_delta: None,
            delta_label: None,
            pushed_at: text_opt(item, "pushed_at"),
            created_at: text_opt(item, "created_at"),
            topics,
            source_note: source_note.map(str::to_owned),
        });
    }
    Ok((total_count, repos))
}

fn github_repo_from_url(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    let marker = "https://github.com/";
    let start = if let Some(pos) = lower.find(marker) {
        pos + marker.len()
    } else {
        let http_marker = "http://github.com/";
        lower.find(http_marker)? + http_marker.len()
    };
    let rest = trimmed[start..].split(['?', '#']).next().unwrap_or("");
    let parts: Vec<&str> = rest.split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() < 2 {
        return None;
    }
    let slug = validate_repo_slug(parts[0], parts[1])?;
    let html_url = format!("https://github.com/{slug}");
    Some((slug, html_url))
}

fn parse_hn_show_github_feed(body: &str) -> Result<Vec<FeedRepo>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "HN 榜单读不出来".to_owned())?;
    let hits = value
        .get("hits")
        .and_then(Value::as_array)
        .ok_or_else(|| "HN 榜单读不出来".to_owned())?;

    let mut best: HashMap<String, (i64, FeedRepo)> = HashMap::new();
    for hit in hits {
        let Some(story_url) = hit.get("url").and_then(Value::as_str) else {
            continue;
        };
        let Some((full_name, html_url)) = github_repo_from_url(story_url) else {
            continue;
        };
        let title = hit
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned);
        let points = hit.get("points").and_then(Value::as_i64).unwrap_or(0);
        let comments = hit.get("num_comments").and_then(Value::as_i64).unwrap_or(0);
        let created_at = text_opt(hit, "created_at");
        let repo = FeedRepo {
            full_name: full_name.clone(),
            html_url,
            description: title,
            language: None,
            stars_count: 0,
            stars_delta: Some(points),
            delta_label: Some(format!("{} HN 赞", format_thousands(points))),
            pushed_at: None,
            created_at,
            topics: Vec::new(),
            source_note: Some(format!("Show HN · {comments} 条讨论")),
        };
        let key = full_name.to_ascii_lowercase();
        let replace = match best.get(&key) {
            Some((existing_points, _)) => points > *existing_points,
            None => true,
        };
        if replace {
            best.insert(key, (points, repo));
        }
    }

    let mut list: Vec<(i64, FeedRepo)> = best.into_values().collect();
    list.sort_by(|left, right| {
        right.0.cmp(&left.0).then_with(|| {
            left.1
                .full_name
                .to_ascii_lowercase()
                .cmp(&right.1.full_name.to_ascii_lowercase())
        })
    });
    list.truncate(FEED_LIMIT);
    Ok(list.into_iter().map(|(_, repo)| repo).collect())
}

fn github_search_url(query: &str, sort: &str) -> String {
    format!(
        "https://api.github.com/search/repositories?q={}&sort={sort}&order=desc&per_page={FEED_LIMIT}",
        url_encode(query)
    )
}

fn fetch_github_search_repos(
    fetcher: &impl HttpFetcher,
    query: &str,
    sort: &str,
    source_note: Option<&str>,
    now: i64,
    github_token: Option<&str>,
) -> Result<(i64, Vec<FeedRepo>), (Option<String>, String)> {
    let url = github_search_url(query, sort);
    match github_api_get(fetcher, &url, github_token) {
        Err(error) => Err((None, error.message)),
        Ok(response) => {
            if let Some(wait) = rate_limit_wait_secs(&response, now) {
                let reset = format_reset_time(wait, now).ok();
                return Err((reset, RATE_LIMIT_MESSAGE.to_owned()));
            }
            if response.status != 200 {
                return Err((None, "联网失败，保留上次的结果".to_owned()));
            }
            parse_github_search_feed(&response.body, source_note).map_err(|err| (None, err))
        }
    }
}

fn trending_url(language: Option<&str>, period: &str) -> String {
    match language {
        Some(lang) => format!(
            "https://github.com/trending/{}?since={period}",
            url_encode(lang)
        ),
        None => format!("https://github.com/trending?since={period}"),
    }
}

fn lang_qualifier(language: Option<&str>) -> String {
    match language {
        Some(lang) => format!(" language:{lang}"),
        None => String::new(),
    }
}

fn replace_feed_cache(
    db_path: &Path,
    feed_key: &str,
    items: &[FeedRepo],
    used_fallback: bool,
    message: Option<&str>,
    now: i64,
) -> Result<(i64, i64), String> {
    let fetched_at = format_unix_time(now)?;
    let mut connection = open_db(db_path)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("榜单缓存事务开启失败：{error}"))?;
    let mine = match latest_account_opt(&tx)? {
        Some(account_id) => active_names(&tx, &account_id)?,
        None => HashSet::new(),
    };
    tx.execute(
        "DELETE FROM discover_feed_items WHERE feed_key = ?1",
        params![feed_key],
    )
    .map_err(|error| format!("榜单缓存清理失败：{error}"))?;

    let mut fetched = 0i64;
    let mut skipped_mine = 0i64;
    for (idx, repo) in items.iter().take(FEED_LIMIT).enumerate() {
        let rank = (idx + 1) as i64;
        let topics = serde_json::to_string(&repo.topics).map_err(|_| "话题写不进去".to_owned())?;
        tx.execute(
            "INSERT INTO discover_feed_items(feed_key, rank, full_name, html_url, description, language, stars_count, stars_delta, delta_label, pushed_at, created_at, topics_json, source_note, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                feed_key,
                rank,
                repo.full_name,
                repo.html_url,
                repo.description,
                repo.language,
                repo.stars_count,
                repo.stars_delta,
                repo.delta_label,
                repo.pushed_at,
                repo.created_at,
                topics,
                repo.source_note,
                fetched_at,
            ],
        )
        .map_err(|error| format!("榜单条目写入失败：{error}"))?;
        if mine.contains(&repo.full_name.to_ascii_lowercase()) {
            skipped_mine += 1;
        } else {
            fetched += 1;
        }
    }

    tx.execute(
        "INSERT INTO discover_feed_runs(feed_key, last_fetched_at, item_count, used_fallback, message)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(feed_key) DO UPDATE SET
           last_fetched_at = excluded.last_fetched_at,
           item_count = excluded.item_count,
           used_fallback = excluded.used_fallback,
           message = excluded.message",
        params![
            feed_key,
            fetched_at,
            items.len().min(FEED_LIMIT) as i64,
            i64::from(used_fallback),
            message,
        ],
    )
    .map_err(|error| format!("榜单记录写入失败：{error}"))?;

    tx.commit()
        .map_err(|error| format!("榜单缓存提交失败：{error}"))?;
    Ok((fetched, skipped_mine))
}

pub fn refresh_discover_feed_with(
    fetcher: &impl HttpFetcher,
    db_path: &Path,
    feed_id: &str,
    language: Option<&str>,
    now: i64,
    github_token: Option<&str>,
) -> Result<DiscoverFeedRefreshReport, String> {
    let feed_id = validate_feed_id(feed_id)?;
    let lang = normalize_language(language)?;
    let lang_for_key = if feed_id == "hn_show" { None } else { lang };
    let feed_key = build_feed_key(feed_id, lang_for_key.as_deref());

    let (items, used_fallback, message, reset_at) = match feed_id {
        "trending_daily" | "trending_weekly" => {
            let period = if feed_id == "trending_weekly" {
                "weekly"
            } else {
                "daily"
            };
            let html_url = trending_url(lang_for_key.as_deref(), period);
            let parsed_html = match fetcher.get(&html_url, TRENDING_HTML_HEADERS) {
                Ok(resp) if resp.status == 200 => {
                    let parsed = parse_github_trending_html(&resp.body, period);
                    if parsed.is_empty() {
                        None
                    } else {
                        Some(parsed.into_iter().take(FEED_LIMIT).collect::<Vec<_>>())
                    }
                }
                _ => None,
            };
            if let Some(items) = parsed_html {
                (items, false, None, None)
            } else {
                let (pushed_days, created_days, min_stars) = if period == "weekly" {
                    (7, 90, 80)
                } else {
                    (3, 30, 30)
                };
                let pushed_since = since_date(now, pushed_days)?;
                let created_since = since_date(now, created_days)?;
                let query = format!(
                    "pushed:>{pushed_since} created:>{created_since} stars:>{min_stars} archived:false{}",
                    lang_qualifier(lang_for_key.as_deref())
                );
                match fetch_github_search_repos(
                    fetcher,
                    &query,
                    "stars",
                    Some("GitHub 搜索兜底"),
                    now,
                    github_token,
                ) {
                    Ok((_, repos)) => (
                        repos,
                        true,
                        Some(TRENDING_FALLBACK_MESSAGE.to_owned()),
                        None,
                    ),
                    Err((reset, msg)) => {
                        return Ok(DiscoverFeedRefreshReport {
                            feed_id: feed_id.to_owned(),
                            language: lang_for_key,
                            feed_key,
                            fetched: 0,
                            skipped_mine: 0,
                            used_fallback: false,
                            message: Some(msg),
                            rate_limit_reset_at: reset,
                        });
                    }
                }
            }
        }
        "rising_7d" | "rising_30d" => {
            let (days, min_stars, note) = if feed_id == "rising_7d" {
                (7, 20, "近 7 天新锐")
            } else {
                (30, 100, "近 30 天黑马")
            };
            let since = since_date(now, days)?;
            let query = format!(
                "created:>{since} stars:>{min_stars} archived:false{}",
                lang_qualifier(lang_for_key.as_deref())
            );
            match fetch_github_search_repos(fetcher, &query, "stars", Some(note), now, github_token)
            {
                Ok((_, repos)) => (repos, false, None, None),
                Err((reset, msg)) => {
                    return Ok(DiscoverFeedRefreshReport {
                        feed_id: feed_id.to_owned(),
                        language: lang_for_key,
                        feed_key,
                        fetched: 0,
                        skipped_mine: 0,
                        used_fallback: false,
                        message: Some(msg),
                        rate_limit_reset_at: reset,
                    });
                }
            }
        }
        "hn_show" => {
            let since_14d = now.saturating_sub(14 * 86_400);
            let url = format!(
                "https://hn.algolia.com/api/v1/search?tags=show_hn&numericFilters=created_at_i%3E{since_14d},points%3E15&hitsPerPage=50"
            );
            let response = match fetcher.get(&url, &[("Accept", "application/json")]) {
                Ok(resp) if resp.status == 200 => resp,
                _ => {
                    return Ok(DiscoverFeedRefreshReport {
                        feed_id: feed_id.to_owned(),
                        language: None,
                        feed_key,
                        fetched: 0,
                        skipped_mine: 0,
                        used_fallback: false,
                        message: Some("联网失败，保留上次的结果".to_owned()),
                        rate_limit_reset_at: None,
                    });
                }
            };
            let repos = match parse_hn_show_github_feed(&response.body) {
                Ok(list) => list,
                Err(msg) => {
                    return Ok(DiscoverFeedRefreshReport {
                        feed_id: feed_id.to_owned(),
                        language: None,
                        feed_key,
                        fetched: 0,
                        skipped_mine: 0,
                        used_fallback: false,
                        message: Some(msg),
                        rate_limit_reset_at: None,
                    });
                }
            };
            (repos, false, None, None)
        }
        _ => unreachable!(),
    };

    let (fetched, skipped_mine) = replace_feed_cache(
        db_path,
        &feed_key,
        &items,
        used_fallback,
        message.as_deref(),
        now,
    )?;

    Ok(DiscoverFeedRefreshReport {
        feed_id: feed_id.to_owned(),
        language: lang_for_key,
        feed_key,
        fetched,
        skipped_mine,
        used_fallback,
        message,
        rate_limit_reset_at: reset_at,
    })
}

/// 只读本地榜单缓存，不联网。
pub fn list_discover_feed(
    db_path: &Path,
    feed_id: &str,
    language: Option<&str>,
) -> Result<DiscoverFeedView, String> {
    let feed_id = validate_feed_id(feed_id)?;
    let lang = normalize_language(language)?;
    let lang_for_key = if feed_id == "hn_show" { None } else { lang };
    let feed_key = build_feed_key(feed_id, lang_for_key.as_deref());

    let connection = open_db(db_path)?;
    let (mine, inbox) = load_ownership(&connection)?;

    let run: Option<(String, i64, Option<String>)> = connection
        .query_row(
            "SELECT last_fetched_at, used_fallback, message FROM discover_feed_runs WHERE feed_key = ?1",
            params![feed_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| format!("榜单运行记录读取失败：{error}"))?;

    let mut statement = connection
        .prepare(
            "SELECT rank, full_name, html_url, description, language, stars_count, stars_delta, delta_label, pushed_at, created_at, topics_json, source_note, fetched_at
             FROM discover_feed_items
             WHERE feed_key = ?1
             ORDER BY rank ASC",
        )
        .map_err(|error| format!("榜单缓存读取失败：{error}"))?;
    let rows = statement
        .query_map(params![feed_key], |row| {
            let full_name: String = row.get(1)?;
            let html_url: String = row.get(2)?;
            let topics_json: String = row.get(10)?;
            let topics: Vec<String> = serde_json::from_str(&topics_json).unwrap_or_default();
            Ok(DiscoverFeedItem {
                rank: row.get(0)?,
                mine: mine.contains(&full_name.to_ascii_lowercase()),
                in_inbox: in_inbox(&html_url, &inbox),
                full_name,
                html_url,
                description: row.get(3)?,
                language: row.get(4)?,
                stars_count: row.get(5)?,
                stars_delta: row.get(6)?,
                delta_label: row.get(7)?,
                pushed_at: row.get(8)?,
                created_at: row.get(9)?,
                topics,
                source_note: row.get(11)?,
                fetched_at: row.get(12)?,
            })
        })
        .map_err(|error| format!("榜单缓存查询失败：{error}"))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| format!("榜单条目读取失败：{error}"))?);
    }

    let (last_fetched_at, used_fallback, message) = match run {
        Some((fetched_at, fallback_flag, msg)) => (Some(fetched_at), fallback_flag != 0, msg),
        None => (None, false, None),
    };

    Ok(DiscoverFeedView {
        feed_id: feed_id.to_owned(),
        language: lang_for_key,
        feed_key,
        last_fetched_at,
        used_fallback,
        message,
        items,
    })
}

/// 自由搜索 GitHub 项目（不写发现缓存，直接返回实时列表与收藏/收集箱状态）。
pub fn search_discover_github_with(
    fetcher: &impl HttpFetcher,
    db_path: &Path,
    query: &str,
    time_range: Option<&str>,
    sort: Option<&str>,
    language: Option<&str>,
    now: i64,
    github_token: Option<&str>,
) -> Result<DiscoverSearchResult, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err("请先输入想搜的关键词".to_owned());
    }
    if trimmed.chars().count() > SEARCH_MAX_QUERY_CHARS {
        return Err("搜索词太长了，精简一下再搜".to_owned());
    }
    let lang = normalize_language(language)?;
    let sort_key = match sort.map(str::trim) {
        Some("updated") => "updated",
        _ => "stars",
    };

    let mut parts = vec![trimmed.to_owned()];
    if !trimmed.to_ascii_lowercase().contains("archived:") {
        parts.push("archived:false".to_owned());
    }
    if let Some(lang) = lang.as_deref() {
        parts.push(format!("language:{lang}"));
    }
    match time_range.map(str::trim) {
        Some("7d") => parts.push(format!("created:>{}", since_date(now, 7)?)),
        Some("30d") => parts.push(format!("created:>{}", since_date(now, 30)?)),
        Some("90d") => parts.push(format!("created:>{}", since_date(now, 90)?)),
        Some("180d") => parts.push(format!("pushed:>{}", since_date(now, 180)?)),
        _ => {}
    }

    let full_query = parts.join(" ");
    match fetch_github_search_repos(fetcher, &full_query, sort_key, None, now, github_token) {
        Ok((total_count, repos)) => {
            let fetched_at = format_unix_time(now)?;
            let connection = open_db(db_path)?;
            let (mine, inbox) = load_ownership(&connection)?;
            let items = repos
                .into_iter()
                .take(FEED_LIMIT)
                .enumerate()
                .map(|(index, repo)| DiscoverFeedItem {
                    rank: (index + 1) as i64,
                    mine: mine.contains(&repo.full_name.to_ascii_lowercase()),
                    in_inbox: in_inbox(&repo.html_url, &inbox),
                    full_name: repo.full_name,
                    html_url: repo.html_url,
                    description: repo.description,
                    language: repo.language,
                    stars_count: repo.stars_count,
                    stars_delta: repo.stars_delta,
                    delta_label: repo.delta_label,
                    pushed_at: repo.pushed_at,
                    created_at: repo.created_at,
                    topics: repo.topics,
                    source_note: repo.source_note,
                    fetched_at: fetched_at.clone(),
                })
                .collect();
            Ok(DiscoverSearchResult {
                query: trimmed.to_owned(),
                total_count,
                items,
                message: None,
                rate_limit_reset_at: None,
            })
        }
        Err((reset, msg)) => Ok(DiscoverSearchResult {
            query: trimmed.to_owned(),
            total_count: 0,
            items: Vec::new(),
            message: Some(msg),
            rate_limit_reset_at: reset,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::{insert_test_account, migrate_database};
    use crate::web_fetch::{FetchErrorKind, HttpResponse};
    use serde_json::json;
    use std::path::PathBuf;
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

    struct CallRecord {
        url: String,
        headers: Vec<(String, String)>,
    }

    struct Scripted {
        responses: Mutex<Vec<Result<HttpResponse, FetchError>>>,
        calls: Mutex<Vec<CallRecord>>,
    }

    impl Scripted {
        fn new(responses: Vec<Result<HttpResponse, FetchError>>) -> Self {
            Self {
                responses: Mutex::new(responses),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<CallRecord> {
            self.calls
                .lock()
                .expect("锁")
                .iter()
                .map(|c| CallRecord {
                    url: c.url.clone(),
                    headers: c.headers.clone(),
                })
                .collect()
        }
    }

    impl HttpFetcher for Scripted {
        fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, FetchError> {
            self.calls.lock().expect("锁").push(CallRecord {
                url: url.to_owned(),
                headers: headers
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            });
            let mut queue = self.responses.lock().expect("锁");
            if queue.is_empty() {
                panic!("没有更多假响应: {url}");
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
        time::Date::from_calendar_date(2026, time::Month::November, 25)
            .expect("日期")
            .with_hms(12, 0, 0)
            .expect("时间")
            .assume_utc()
            .unix_timestamp()
    }

    fn temp_db(label: &str) -> TempDb {
        let seq = TEMP_DB_SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时间")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("fsl-discover-feeds-{label}-{seq}-{nanos}.sqlite3"));
        migrate_database(&path).expect("迁移应成功");
        TempDb(path)
    }

    fn ok_response(body: impl Into<String>) -> Result<HttpResponse, FetchError> {
        Ok(HttpResponse {
            status: 200,
            headers: Vec::new(),
            body: body.into(),
        })
    }

    fn network_error() -> Result<HttpResponse, FetchError> {
        Err(FetchError {
            kind: FetchErrorKind::Network,
            message: "网络连不上".to_owned(),
        })
    }

    const SAMPLE_TRENDING_HTML: &str = r#"
    <html><body>
      <article class="Box-row">
        <h2 class="h3 lh-condensed">
          <a href="/elementalsouls/Claude-BugHunter" class="Link">
            <span class="text-normal">elementalsouls /</span> Claude-BugHunter
          </a>
        </h2>
        <p class="col-9 color-fg-muted my-1">A Claude Code &amp; Agent skill bundle &lt;demo&gt;.</p>
        <div class="f6 color-fg-muted mt-2">
          <span><span itemprop="programmingLanguage">Python</span></span>
          <a href="/elementalsouls/Claude-BugHunter/stargazers"><svg></svg> 21,631</a>
          <span class="d-inline-block float-sm-right"><svg></svg> 1,826 stars today</span>
        </div>
      </article>
      <article class="Box-row">
        <h2 class="h3 lh-condensed">
          <a href="/sponsors/nobody" class="Link">sponsor</a>
          <a href="/fox/fast-cli" class="Link">fox / fast-cli</a>
        </h2>
        <p class="col-9">Tiny &amp; fast CLI</p>
        <div class="f6">
          <a href="/fox/fast-cli/stargazers">890</a>
          <span class="float-sm-right">210 stars this week</span>
        </div>
      </article>
    </body></html>
    "#;

    #[test]
    fn parse_github_trending_html_extracts_repos_entities_and_delta() {
        let daily = parse_github_trending_html(SAMPLE_TRENDING_HTML, "daily");
        assert_eq!(daily.len(), 2);
        assert_eq!(daily[0].full_name, "elementalsouls/Claude-BugHunter");
        assert_eq!(
            daily[0].description.as_deref(),
            Some("A Claude Code & Agent skill bundle <demo>.")
        );
        assert_eq!(daily[0].language.as_deref(), Some("Python"));
        assert_eq!(daily[0].stars_count, 21631);
        assert_eq!(daily[0].stars_delta, Some(1826));
        assert_eq!(daily[0].delta_label.as_deref(), Some("+1,826 今日"));
        assert_eq!(daily[0].source_note.as_deref(), Some("GitHub 趋势榜"));

        let weekly = parse_github_trending_html(SAMPLE_TRENDING_HTML, "weekly");
        assert_eq!(weekly[1].full_name, "fox/fast-cli");
        assert_eq!(weekly[1].stars_count, 890);
        assert_eq!(weekly[1].delta_label.as_deref(), Some("+210 本周"));
    }

    #[test]
    fn trending_html_does_not_leak_token_and_falls_back_to_api_when_unavailable() {
        let db = temp_db("trending-fallback");
        insert_test_account(&db.0);

        let fallback_json = json!({
            "total_count": 1,
            "items": [{
                "full_name": "new/rising-tool",
                "html_url": "https://github.com/new/rising-tool",
                "description": "Rising agent tool",
                "language": "TypeScript",
                "stargazers_count": 512,
                "created_at": "2026-11-20T00:00:00Z",
                "pushed_at": "2026-11-24T00:00:00Z",
                "topics": ["agent"],
                "archived": false,
                "fork": false,
                "disabled": false
            }]
        })
        .to_string();

        let fetcher = Scripted::new(vec![network_error(), ok_response(fallback_json)]);
        let report = refresh_discover_feed_with(
            &fetcher,
            &db.0,
            "trending_daily",
            Some("TypeScript"),
            epoch(),
            Some("ghp_secret"),
        )
        .expect("兜底应成功");

        assert!(report.used_fallback);
        assert_eq!(report.fetched, 1);
        assert_eq!(report.message.as_deref(), Some(TRENDING_FALLBACK_MESSAGE));
        let calls = fetcher.calls();
        assert_eq!(calls.len(), 2);
        // HTML 抓取不带 Authorization
        assert!(calls[0]
            .url
            .starts_with("https://github.com/trending/typescript?since=daily"));
        assert!(!calls[0]
            .headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("authorization")));
        // GitHub API 兜底带 Authorization
        assert!(calls[1]
            .url
            .starts_with("https://api.github.com/search/repositories"));
        assert!(calls[1]
            .headers
            .iter()
            .any(|(k, v)| k == "Authorization" && v == "Bearer ghp_secret"));

        // 只读缓存视图应能读到
        let view = list_discover_feed(&db.0, "trending_daily", Some("typescript")).expect("读缓存");
        assert!(view.used_fallback);
        assert_eq!(view.items.len(), 1);
        assert_eq!(view.items[0].full_name, "new/rising-tool");
        assert_eq!(
            view.items[0].source_note.as_deref(),
            Some("GitHub 搜索兜底")
        );
    }

    #[test]
    fn github_api_get_retries_without_token_on_401() {
        let fetcher = Scripted::new(vec![
            Ok(HttpResponse {
                status: 401,
                headers: Vec::new(),
                body: "Bad credentials".to_owned(),
            }),
            ok_response(r#"{"total_count":0,"items":[]}"#),
        ]);
        let resp = github_api_get(
            &fetcher,
            "https://api.github.com/search/repositories?q=mcp",
            Some("expired_tok"),
        )
        .expect("重试应成功");
        assert_eq!(resp.status, 200);
        let calls = fetcher.calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].headers.iter().any(|(k, _)| k == "Authorization"));
        assert!(!calls[1].headers.iter().any(|(k, _)| k == "Authorization"));
    }

    #[test]
    fn hn_show_keeps_only_github_repos_dedupes_and_does_not_send_token() {
        let db = temp_db("hn-show");
        insert_test_account(&db.0);

        let hn_body = json!({
            "hits": [
                {"title": "Show HN: Blog post", "url": "https://example.com/post", "points": 200, "num_comments": 40},
                {"title": "Show HN: GitHub Topics", "url": "https://github.com/topics/ai", "points": 300, "num_comments": 50},
                {"title": "Show HN: Agent A", "url": "https://github.com/acme/agent-a/tree/main", "points": 120, "num_comments": 25, "created_at": "2026-11-24T00:00:00Z"},
                {"title": "Show HN: Agent A duplicate higher", "url": "https://github.com/Acme/Agent-A", "points": 240, "num_comments": 60, "created_at": "2026-11-24T01:00:00Z"},
                {"title": "Show HN: Agent B", "url": "https://github.com/acme/agent-b.git", "points": 90, "num_comments": 12}
            ]
        })
        .to_string();

        let fetcher = Scripted::new(vec![ok_response(hn_body)]);
        let report = refresh_discover_feed_with(
            &fetcher,
            &db.0,
            "hn_show",
            Some("python"),
            epoch(),
            Some("ghp_secret"),
        )
        .expect("HN 刷新应成功");

        assert_eq!(report.feed_key, "hn_show");
        assert_eq!(report.language, None);
        assert_eq!(report.fetched, 2);
        let calls = fetcher.calls();
        assert!(!calls[0]
            .headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("authorization")));

        let view = list_discover_feed(&db.0, "hn_show", Some("python")).expect("读缓存");
        assert_eq!(view.items.len(), 2);
        assert_eq!(view.items[0].full_name, "Acme/Agent-A");
        assert_eq!(view.items[0].stars_delta, Some(240));
        assert_eq!(view.items[0].delta_label.as_deref(), Some("240 HN 赞"));
        assert_eq!(
            view.items[0].source_note.as_deref(),
            Some("Show HN · 60 条讨论")
        );
        assert_eq!(view.items[1].full_name, "acme/agent-b");
    }

    #[test]
    fn list_discover_feed_works_without_account_and_never_fetches() {
        let db = temp_db("no-account");
        let view = list_discover_feed(&db.0, "rising_7d", None).expect("空库也应返回");
        assert!(view.items.is_empty());
        assert_eq!(view.last_fetched_at, None);
    }

    #[test]
    fn search_discover_github_validates_query_and_maps_ownership() {
        let db = temp_db("search");
        insert_test_account(&db.0);
        let conn = open_db(&db.0).expect("开库");
        conn.execute(
            "INSERT INTO repositories(id, account_id, owner, name, full_name, description, topics_json, html_url, sync_status, starred_at)
             VALUES ('r1', 'acct', 'fox', 'owned', 'fox/owned', '', '[]', 'https://github.com/fox/owned', 'active', '2026-10-01T00:00:00Z')",
            [],
        )
        .expect("写收藏");
        drop(conn);

        let err = search_discover_github_with(
            &PanicFetcher,
            &db.0,
            "   ",
            Some("30d"),
            Some("stars"),
            None,
            epoch(),
            None,
        )
        .expect_err("空关键词报错");
        assert_eq!(err, "请先输入想搜的关键词");

        let body = json!({
            "total_count": 2,
            "items": [
                {"full_name": "fox/owned", "html_url": "https://github.com/fox/owned", "description": "Mine", "stargazers_count": 300, "archived": false, "fork": false, "disabled": false},
                {"full_name": "other/new", "html_url": "https://github.com/other/new", "description": "Fresh", "stargazers_count": 120, "archived": false, "fork": false, "disabled": false}
            ]
        })
        .to_string();
        let fetcher = Scripted::new(vec![ok_response(body)]);
        let res = search_discover_github_with(
            &fetcher,
            &db.0,
            "claude skill",
            Some("30d"),
            Some("stars"),
            Some("rust"),
            epoch(),
            Some("tok"),
        )
        .expect("搜索应成功");

        assert_eq!(res.total_count, 2);
        assert_eq!(res.items.len(), 2);
        assert!(res.items[0].mine);
        assert!(!res.items[1].mine);
        let calls = fetcher.calls();
        assert!(calls[0].url.contains("language%3Arust"));
        assert!(calls[0].url.contains("created%3A%3E2026-10-26"));
        assert!(calls[0].url.contains("sort=stars"));
    }

    #[test]
    fn discover_feed_commands_are_registered_and_refresh_spawns_blocking() {
        let permissions = include_str!("../permissions/gsat-commands.toml");
        let manifest = include_str!("../gen/schemas/acl-manifests.json");
        let source = include_str!("lib.rs");
        let start = source
            .find("generate_handler![")
            .expect("应有 generate_handler");
        let end = source[start..].find("])").expect("generate_handler 应结束");
        let block = &source[start..start + end];
        for command in [
            "list_discover_feed",
            "refresh_discover_feed",
            "search_discover_github",
        ] {
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
    }
}
