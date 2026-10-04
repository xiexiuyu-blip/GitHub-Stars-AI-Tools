//! README 变了以后，决定要不要把理解卡重新排队。
//!
//! 只读本地缓存，不联网，也不改额度。明显改动才入队；徽章、版本号、错别字这类小改动只更新记过的哈希。

use crate::card_pipeline::build_readme_excerpt;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::HashSet;
use std::path::Path;

/// 规范化后的相似度低于这个数，才算「明显改动」。
pub const MATERIAL_SIMILARITY: f64 = 0.80;
/// 一次同步最多重新抓这么多个 README，剩下的留给下次。
pub const MAX_REFRESH_PER_RUN: usize = 150;
/// README 明显改了以后重新整理的优先级。比新收藏（3）靠后，比补旧账（6）靠前。
pub const README_CHANGED_PRIORITY: i64 = 5;

const MAX_COMPARE_CHARS: usize = 20_000;
const LINK_TEXT_CAP: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoReadmeState {
    pub id: String,
    pub full_name: String,
    pub pushed_at: Option<String>,
    pub readme_fetched_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FollowReport {
    /// 这次真正评估了几份有理解卡、有 README 缓存、且没被确认的仓库。
    pub checked: usize,
    /// `readme_hash` 有值，且和当前 `content_hash` 不同。
    pub changed: usize,
    /// 算作明显改动的份数（含「以前没有 README，现在有了」）。
    pub material: usize,
    /// 这次新插进队列的任务数。已经排着或正在跑的不重复计。
    pub queued: usize,
}

/// 转小写，去掉图片、链接地址、裸网址、HTML、数字和标点，只留字母和汉字。最多 20000 个字符。
/// 没结束的 `<` 或 `[` 就停，已经输出的留下，后面的整段丢掉。
pub fn normalize_for_compare(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut kept = 0usize;
    let mut index = 0usize;
    while index < chars.len() && kept < MAX_COMPARE_CHARS {
        let ch = chars[index];
        if ch == '!' && chars.get(index + 1) == Some(&'[') {
            match skip_image(&chars, index) {
                Some(next) => {
                    index = next;
                    continue;
                }
                None => break,
            }
        }
        if ch == '[' {
            match take_link(&chars, index, &mut out, &mut kept) {
                Some(next) => {
                    index = next;
                    continue;
                }
                None => break,
            }
        }
        if ch == '<' {
            match skip_html(&chars, index) {
                Some(next) => {
                    index = next;
                    continue;
                }
                None => break,
            }
        }
        if is_url_at(&chars, index) {
            index = skip_until_whitespace(&chars, index);
            continue;
        }
        if ch.is_alphabetic() {
            push_letters(ch, &mut out, &mut kept);
        }
        index += 1;
    }
    out
}

/// 两边都是已经规范化的字符串。不足 4 个字符的整串当 1 个片段。
pub fn similarity(left: &str, right: &str) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let left_set = shingles(left);
    let right_set = shingles(right);
    let intersection = left_set.intersection(&right_set).count();
    let union = left_set.union(&right_set).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

/// 现在的摘录空了（README 没了或几乎没内容）→ 不重新整理。
/// 以前没有依据、现在有内容 → 算明显改动。其余看 Jaccard。
pub fn is_material_change(basis: Option<&str>, current_excerpt: Option<&str>) -> bool {
    let current = current_excerpt
        .map(normalize_for_compare)
        .unwrap_or_default();
    if current.is_empty() {
        return false;
    }
    let basis = basis.map(normalize_for_compare).unwrap_or_default();
    if basis.is_empty() {
        return true;
    }
    similarity(&basis, &current) < MATERIAL_SIMILARITY
}

/// 没有缓存的在前；其余按 `readme_fetched_at` 从旧到新。最多 `MAX_REFRESH_PER_RUN` 个。
pub fn refresh_targets(repos: &[RepoReadmeState]) -> Vec<String> {
    let mut chosen: Vec<&RepoReadmeState> =
        repos.iter().filter(|repo| needs_refresh(repo)).collect();
    chosen.sort_by(|left, right| {
        match (
            left.readme_fetched_at.is_none(),
            right.readme_fetched_at.is_none(),
        ) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => left
                .readme_fetched_at
                .as_deref()
                .unwrap_or("")
                .cmp(right.readme_fetched_at.as_deref().unwrap_or(""))
                .then_with(|| left.id.cmp(&right.id)),
        }
    });
    chosen
        .into_iter()
        .take(MAX_REFRESH_PER_RUN)
        .map(|repo| repo.id.clone())
        .collect()
}

/// 只看缓存里的 README，不联网。每个仓库一个小的 `IMMEDIATE` 事务。
pub fn enqueue_changed(db_path: &Path) -> Result<FollowReport, String> {
    let mut conn = crate::jobs::open_db(db_path)?;
    let ids = conn
        .prepare(&candidate_sql())
        .map_err(|error| format!("README 跟进读取失败：{error}"))?
        .query_map([], |row| row.get(0))
        .map_err(|error| format!("README 跟进读取失败：{error}"))?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|error| format!("README 跟进读取失败：{error}"))?;
    let mut report = FollowReport::default();
    for repo_id in ids {
        apply_one(&mut conn, &repo_id, &mut report)?;
    }
    Ok(report)
}

fn needs_refresh(repo: &RepoReadmeState) -> bool {
    let Some(fetched_at) = repo.readme_fetched_at.as_deref() else {
        return true;
    };
    let Some(pushed_at) = repo.pushed_at.as_deref() else {
        return false;
    };
    stamp19(pushed_at) > stamp19(fetched_at)
}

/// 只取 `YYYY-MM-DDTHH:MM:SS` 这 19 个字符，按字符边界切，避免把多字节字符切坏。
fn stamp19(value: &str) -> String {
    value.chars().take(19).collect()
}

fn candidate_sql() -> String {
    format!(
        "SELECT repositories.id
         FROM repositories
         JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
         JOIN repo_readmes ON repo_readmes.repo_id = repositories.id
         WHERE repositories.sync_status = 'active'
           AND IFNULL(trim(repo_ai_documents.card_json), '') <> ''
           AND {}
         ORDER BY repositories.id",
        crate::card_handler::NOT_CONFIRMED_SQL
    )
}

fn detail_sql() -> String {
    format!(
        "SELECT repositories.account_id,
                repo_ai_documents.readme_hash,
                repo_ai_documents.readme_basis,
                repo_readmes.content_hash,
                repo_readmes.raw_markdown
         FROM repositories
         JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
         JOIN repo_readmes ON repo_readmes.repo_id = repositories.id
         WHERE repositories.id = ?1
           AND repositories.sync_status = 'active'
           AND IFNULL(trim(repo_ai_documents.card_json), '') <> ''
           AND {}",
        crate::card_handler::NOT_CONFIRMED_SQL
    )
}

fn apply_one(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    report: &mut FollowReport,
) -> Result<(), String> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("README 跟进入队失败：{error}"))?;
    let row = tx
        .query_row(&detail_sql(), params![repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .optional()
        .map_err(|error| format!("README 跟进读取失败：{error}"))?;
    let Some((account_id, readme_hash, readme_basis, content_hash, raw_markdown)) = row else {
        tx.commit()
            .map_err(|error| format!("README 跟进入队失败：{error}"))?;
        return Ok(());
    };
    let excerpt = build_readme_excerpt(&raw_markdown);
    let current = if excerpt.is_absent() || excerpt.text.is_empty() {
        None
    } else {
        Some(excerpt.text)
    };
    report.checked += 1;
    match readme_hash.as_deref() {
        None => {
            if is_material_change(None, current.as_deref()) {
                report.material += 1;
                report.queued += insert_job(&tx, &account_id, repo_id)?;
            }
        }
        Some(hash) if hash == content_hash => {
            if readme_basis.is_none() {
                tx.execute(
                    "UPDATE repo_ai_documents SET readme_basis = ?1 WHERE repo_id = ?2",
                    params![current, repo_id],
                )
                .map_err(|error| format!("README 基准补写失败：{error}"))?;
            }
        }
        Some(_) => {
            report.changed += 1;
            if readme_basis.is_none() {
                tx.execute(
                    "UPDATE repo_ai_documents SET readme_hash = ?1, readme_basis = ?2 WHERE repo_id = ?3",
                    params![content_hash, current, repo_id],
                )
                .map_err(|error| format!("README 基准补写失败：{error}"))?;
            } else if is_material_change(readme_basis.as_deref(), current.as_deref()) {
                report.material += 1;
                report.queued += insert_job(&tx, &account_id, repo_id)?;
            } else {
                tx.execute(
                    "UPDATE repo_ai_documents SET readme_hash = ?1 WHERE repo_id = ?2",
                    params![content_hash, repo_id],
                )
                .map_err(|error| format!("README 哈希更新失败：{error}"))?;
            }
        }
    }
    tx.commit()
        .map_err(|error| format!("README 跟进入队失败：{error}"))?;
    Ok(())
}

fn insert_job(tx: &Transaction<'_>, account_id: &str, repo_id: &str) -> Result<usize, String> {
    tx.execute(
        "INSERT OR IGNORE INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
         VALUES (lower(hex(randomblob(8))), ?1, 'repo_card', 'repository', ?2, ?3, 'queued')",
        params![account_id, repo_id, README_CHANGED_PRIORITY],
    )
    .map_err(|error| format!("README 跟进入队失败：{error}"))
}

fn shingles(text: &str) -> HashSet<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut set = HashSet::new();
    if chars.len() < 4 {
        set.insert(chars.into_iter().collect());
        return set;
    }
    for index in 0..=chars.len() - 4 {
        set.insert(chars[index..index + 4].iter().collect());
    }
    set
}

fn push_letters(ch: char, out: &mut String, kept: &mut usize) {
    for lower in ch.to_lowercase() {
        if *kept >= MAX_COMPARE_CHARS {
            return;
        }
        out.push(lower);
        *kept += 1;
    }
}

fn skip_image(chars: &[char], start: usize) -> Option<usize> {
    let mut index = start + 2;
    while index < chars.len() {
        if chars[index] == ']' && chars.get(index + 1) == Some(&'(') {
            index += 2;
            while index < chars.len() {
                if chars[index] == ')' {
                    return Some(index + 1);
                }
                index += 1;
            }
            return None;
        }
        index += 1;
    }
    None
}

fn take_link(chars: &[char], start: usize, out: &mut String, kept: &mut usize) -> Option<usize> {
    let mut index = start + 1;
    let mut buf: Vec<char> = Vec::new();
    while index < chars.len() {
        if chars[index] == ']' && chars.get(index + 1) == Some(&'(') {
            let mut end = index + 2;
            while end < chars.len() {
                if chars[end] == ')' {
                    for ch in &buf {
                        if ch.is_alphabetic() {
                            push_letters(*ch, out, kept);
                        }
                    }
                    return Some(end + 1);
                }
                end += 1;
            }
            return None;
        }
        if buf.len() >= LINK_TEXT_CAP {
            return None;
        }
        buf.push(chars[index]);
        index += 1;
    }
    None
}

fn skip_html(chars: &[char], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < chars.len() {
        if chars[index] == '>' {
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

fn is_url_at(chars: &[char], index: usize) -> bool {
    starts_with_ascii_ci(chars, index, &['h', 't', 't', 'p', 's', ':', '/', '/'])
        || starts_with_ascii_ci(chars, index, &['h', 't', 't', 'p', ':', '/', '/'])
}

fn starts_with_ascii_ci(chars: &[char], index: usize, needle: &[char]) -> bool {
    if index + needle.len() > chars.len() {
        return false;
    }
    needle
        .iter()
        .enumerate()
        .all(|(offset, expected)| chars[index + offset].to_ascii_lowercase() == *expected)
}

fn skip_until_whitespace(chars: &[char], index: usize) -> usize {
    let mut next = index;
    while next < chars.len() && !chars[next].is_whitespace() {
        next += 1;
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::migrate_database;
    use rusqlite::{params, Connection};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn prose(len: usize, start: u32) -> String {
        (0..len)
            .map(|index| {
                char::from_u32(0x4E00 + start + index as u32).expect("测试用汉字应在范围内")
            })
            .collect()
    }

    #[test]
    fn normalize_drops_badge_link_url_html_digits_case_and_keeps_han() {
        let raw = "Hello, WORLD! ![badge](https://img.shields.io/badge/v1-blue) See [文档](https://example.com/docs) and https://example.com/raw <b>粗体</b> v1.2.3 汉字保留";
        assert_eq!(
            normalize_for_compare(raw),
            "helloworldsee文档and粗体v汉字保留"
        );
        assert_eq!(normalize_for_compare("甲1乙， 丙"), "甲乙丙");
        assert_eq!(normalize_for_compare("AbC"), "abc");
    }

    #[test]
    fn normalize_truncates_output_at_20000_chars() {
        let raw: String = std::iter::repeat('A').take(20_001).collect();
        let out = normalize_for_compare(&raw);
        assert_eq!(out.chars().count(), 20_000);
        assert!(out.chars().all(|ch| ch == 'a'));
        let han = prose(20_001, 0);
        assert_eq!(normalize_for_compare(&han).chars().count(), 20_000);
    }

    #[test]
    fn normalize_unclosed_html_stops_and_keeps_prefix() {
        assert_eq!(normalize_for_compare("前面<没结束"), "前面");
        assert_eq!(normalize_for_compare("<b>粗体</b>"), "粗体");
    }

    #[test]
    fn normalize_unclosed_link_stops_and_keeps_prefix() {
        assert_eq!(normalize_for_compare("前面[没结束"), "前面");
        assert_eq!(
            normalize_for_compare("见[说明v2](https://example.com/a)。"),
            "见说明v"
        );
    }

    #[test]
    fn normalize_unclosed_image_stops() {
        assert_eq!(normalize_for_compare("前面![没结束"), "前面");
        assert_eq!(
            normalize_for_compare("留![x](https://img.example/a.png)下"),
            "留下"
        );
    }

    #[test]
    fn normalize_url_is_case_insensitive() {
        assert_eq!(normalize_for_compare("看 HTTP://EXAMPLE.com/A 完"), "看完");
        assert_eq!(normalize_for_compare("看 Https://foo.bar/x 完"), "看完");
    }

    #[test]
    fn normalize_malicious_unclosed_markup_stays_fast() {
        let mut tags = String::from("开头");
        tags.push('<');
        tags.extend(std::iter::repeat('a').take(100_000));
        let started = Instant::now();
        let out = normalize_for_compare(&tags);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "未闭合标签扫太慢：{:?}",
            started.elapsed()
        );
        assert_eq!(out, "开头");

        let mut brackets = String::from("前");
        brackets.push('[');
        brackets.extend(std::iter::repeat('字').take(100_000));
        let started = Instant::now();
        let out = normalize_for_compare(&brackets);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "未闭合链接扫太慢：{:?}",
            started.elapsed()
        );
        assert_eq!(out, "前");

        let mut packed = String::new();
        for index in 0..10_000 {
            packed.push(char::from_u32(0x4E00 + (index % 80)).expect("汉字"));
        }
        packed.push_str("https://example.com/x");
        let started = Instant::now();
        let out = normalize_for_compare(&packed);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(!out.contains("http"));
        assert!(out.chars().count() >= 80);
    }

    #[test]
    fn similarity_identical_is_exactly_one() {
        let text = prose(40, 0);
        assert_eq!(similarity(&text, &text), 1.0);
        assert_eq!(similarity("abcd", "abcd"), 1.0);
    }

    #[test]
    fn similarity_disjoint_is_zero() {
        assert_eq!(similarity(&prose(12, 0), &prose(12, 100)), 0.0);
        assert_eq!(similarity("abcd", "efgh"), 0.0);
    }

    #[test]
    fn similarity_both_empty_is_one() {
        assert_eq!(similarity("", ""), 1.0);
    }

    #[test]
    fn similarity_one_side_empty_is_zero() {
        assert_eq!(similarity("", "abcd"), 0.0);
        assert_eq!(similarity("abcd", ""), 0.0);
    }

    #[test]
    fn similarity_is_symmetric() {
        let left = prose(30, 0);
        let mut right = prose(20, 0);
        right.push_str(&prose(10, 80));
        let forward = similarity(&left, &right);
        let backward = similarity(&right, &left);
        assert_eq!(forward, backward);
        assert!(forward > 0.0 && forward < 1.0, "{forward}");
    }

    #[test]
    fn similarity_shorter_than_four_is_one_shingle() {
        assert_eq!(similarity("ab", "ab"), 1.0);
        assert_eq!(similarity("ab", "cd"), 0.0);
        assert_eq!(similarity("你", "你"), 1.0);
        assert_eq!(similarity("你", "好"), 0.0);
    }

    #[test]
    fn material_version_badge_and_typo_are_not_material() {
        let body = prose(180, 0);
        let left = format!("![b](https://img.shields.io/badge/v1.2.3-blue)\n{body}\nv1.2.3");
        let right = format!("![b](https://img.shields.io/badge/v9.0.0-red)\n{body}\nv9.9.9");
        assert!(!is_material_change(Some(&left), Some(&right)));

        let mut chars: Vec<char> = prose(200, 0).chars().collect();
        let original: String = chars.iter().collect();
        chars[100] = '\u{9FA5}';
        let edited: String = chars.iter().collect();
        assert!(!is_material_change(Some(&original), Some(&edited)));
    }

    #[test]
    fn material_append_about_thirty_percent_is_material() {
        let base = prose(400, 0);
        let mut grown = base.clone();
        grown.push_str(&prose(120, 400));
        assert!(is_material_change(Some(&base), Some(&grown)));
    }

    #[test]
    fn material_append_about_ten_percent_is_not() {
        let base = prose(400, 0);
        let mut grown = base.clone();
        grown.push_str(&prose(40, 400));
        assert!(!is_material_change(Some(&base), Some(&grown)));
    }

    #[test]
    fn material_rewriting_half_is_material() {
        let base = prose(400, 0);
        let mut rewritten = prose(200, 0);
        rewritten.push_str(&prose(200, 2_000));
        assert!(is_material_change(Some(&base), Some(&rewritten)));
    }

    #[test]
    fn material_missing_basis_with_content_is_material() {
        let current = prose(80, 0);
        assert!(is_material_change(None, Some(&current)));
        assert!(is_material_change(Some(""), Some(&current)));
        assert!(is_material_change(Some("123 !!!"), Some(&current)));
    }

    #[test]
    fn material_empty_current_is_not() {
        let basis = prose(80, 0);
        assert!(!is_material_change(Some(&basis), None));
        assert!(!is_material_change(Some(&basis), Some("")));
        assert!(!is_material_change(Some(&basis), Some("1.2.3 !!!")));
        assert!(!is_material_change(None, None));
        assert!(!is_material_change(None, Some("")));
    }

    #[test]
    fn material_one_han_sentence_in_a_long_text_is_not() {
        let mut chars: Vec<char> = prose(400, 0).chars().collect();
        let original: String = chars.iter().collect();
        for index in 120..128 {
            chars[index] = char::from_u32(0x9000 + (index as u32)).expect("替换字");
        }
        let edited: String = chars.iter().collect();
        assert!(!is_material_change(Some(&original), Some(&edited)));
    }

    fn state(
        id: &str,
        pushed_at: Option<&str>,
        readme_fetched_at: Option<&str>,
    ) -> RepoReadmeState {
        RepoReadmeState {
            id: id.to_owned(),
            full_name: format!("fox/{id}"),
            pushed_at: pushed_at.map(str::to_owned),
            readme_fetched_at: readme_fetched_at.map(str::to_owned),
        }
    }

    #[test]
    fn refresh_puts_missing_cache_first_and_skips_unchanged_pushes() {
        let repos = vec![
            state(
                "old",
                Some("2026-01-01T00:00:00Z"),
                Some("2020-01-01T00:00:00Z"),
            ),
            state(
                "fresh",
                Some("2026-01-01T00:00:00Z"),
                Some("2026-06-01T00:00:00Z"),
            ),
            state(
                "equal",
                Some("2026-06-01T00:00:00.123Z"),
                Some("2026-06-01T00:00:00Z"),
            ),
            state("none-push", None, Some("2020-01-01T00:00:00Z")),
            state("no-cache", None, None),
            state("no-cache-b", Some("2026-01-01T00:00:00Z"), None),
            state(
                "newer",
                Some("2026-08-01T00:00:01Z"),
                Some("2026-08-01T00:00:00.999Z"),
            ),
            state(
                "same-second",
                Some("2026-08-01T00:00:00.999Z"),
                Some("2026-08-01T00:00:00.000Z"),
            ),
        ];
        assert_eq!(
            refresh_targets(&repos),
            vec![
                "no-cache".to_owned(),
                "no-cache-b".to_owned(),
                "old".to_owned(),
                "newer".to_owned(),
            ]
        );
    }

    #[test]
    fn refresh_caps_at_150_and_keeps_the_oldest() {
        let mut missing = Vec::new();
        for index in 0..151 {
            missing.push(state(&format!("m{index:03}"), None, None));
        }
        let ids = refresh_targets(&missing);
        assert_eq!(ids.len(), 150);
        assert_eq!(ids[0], "m000");
        assert_eq!(ids[149], "m149");
        assert!(!ids.iter().any(|id| id == "m150"));

        let mut cached = Vec::new();
        for index in 0..160 {
            let fetched = format!("2020-01-01T00:{:02}:{:02}Z", index / 60, index % 60);
            cached.push(state(
                &format!("c{index:03}"),
                Some("2026-10-01T00:00:00Z"),
                Some(&fetched),
            ));
        }
        let ids = refresh_targets(&cached);
        assert_eq!(ids.len(), 150);
        assert_eq!(ids[0], "c000");
        assert_eq!(ids[149], "c149");
        assert!(!ids.iter().any(|id| id == "c159"));
    }

    struct TempDb {
        path: PathBuf,
    }

    impl TempDb {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应晚于纪元")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("fsl-follow-{nanos}.sqlite3"));
            migrate_database(&path).expect("测试库应能完成迁移");
            let conn = Connection::open(&path).expect("应能打开测试库");
            conn.execute_batch("PRAGMA foreign_keys = ON;")
                .expect("应能打开外键");
            conn.execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .expect("测试账号应能写入");
            drop(conn);
            Self { path }
        }

        fn open(&self) -> Connection {
            let conn = Connection::open(&self.path).expect("应能打开测试库");
            conn.execute_batch("PRAGMA foreign_keys = ON;")
                .expect("应能打开外键");
            conn
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            for suffix in ["-wal", "-shm"] {
                let mut name = self.path.as_os_str().to_os_string();
                name.push(suffix);
                let _ = fs::remove_file(name);
            }
        }
    }

    fn insert_repo(conn: &Connection, id: &str, status: &str) {
        conn.execute(
            "INSERT INTO repositories(
                id, account_id, owner, name, full_name, html_url, starred_at, sync_status, topics_json
             ) VALUES (
                ?1, 'acct', 'fox', ?1, 'fox/' || ?1, 'https://github.com/fox/' || ?1,
                '2026-09-01T00:00:00Z', ?2, '[]'
             )",
            params![id, status],
        )
        .expect("仓库应能写入");
    }

    fn insert_readme(conn: &Connection, id: &str, raw: &str, hash: &str) {
        conn.execute(
            "INSERT INTO repo_readmes(repo_id, raw_markdown, content_hash, source_path, fetched_at)
             VALUES (?1, ?2, ?3, 'README.md', '2026-09-01T00:00:00Z')",
            params![id, raw, hash],
        )
        .expect("README 应能写入");
    }

    fn insert_card(
        conn: &Connection,
        id: &str,
        card_json: &str,
        readme_hash: Option<&str>,
        readme_basis: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO repo_ai_documents(
                repo_id, summary_zh, model, prompt_version, source_hash, generated_at, card_json,
                readme_hash, readme_basis
             ) VALUES (?1, '摘要', 'm', 'card-v2', 'src', '2026-06-01T00:00:00Z', ?2, ?3, ?4)",
            params![id, card_json, readme_hash, readme_basis],
        )
        .expect("理解卡应能写入");
    }

    fn optional_text(conn: &Connection, sql: &str) -> Option<String> {
        conn.query_row(sql, [], |row| row.get(0))
            .expect("查询应返回")
    }

    fn job_count(conn: &Connection, repo_id: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM ai_jobs WHERE target_id = ?1 AND job_type = 'repo_card'",
            params![repo_id],
            |row| row.get(0),
        )
        .expect("任务数应能读出")
    }

    fn job_priority(conn: &Connection, repo_id: &str) -> i64 {
        conn.query_row(
            "SELECT priority FROM ai_jobs WHERE target_id = ?1",
            params![repo_id],
            |row| row.get(0),
        )
        .expect("优先级应能读出")
    }

    #[test]
    fn enqueue_changed_reports_the_combined_fixture_and_second_run_queues_zero() {
        let db = TempDb::new();
        let same = prose(120, 0);
        let typo_old = prose(200, 200);
        let mut typo_chars: Vec<char> = typo_old.chars().collect();
        typo_chars[80] = '\u{9FA5}';
        let typo_new: String = typo_chars.iter().collect();
        let rewrite_basis = prose(160, 500);
        let rewrite_now = prose(160, 2_000);
        let fill = prose(100, 800);
        let adopt = prose(100, 1_000);
        let fresh = prose(100, 1_200);
        let confirmed_old = prose(100, 1_400);
        let confirmed_now = prose(100, 1_600);
        let removed_old = prose(100, 1_800);
        let removed_now = prose(100, 2_200);

        let conn = db.open();
        insert_repo(&conn, "same", "active");
        insert_readme(&conn, "same", &same, "hash-same");
        insert_card(
            &conn,
            "same",
            r#"{"review_status":"suggested"}"#,
            Some("hash-same"),
            Some(&same),
        );

        insert_repo(&conn, "typo", "active");
        insert_readme(&conn, "typo", &typo_new, "hash-typo-new");
        insert_card(
            &conn,
            "typo",
            r#"{"review_status":"suggested"}"#,
            Some("hash-typo-old"),
            Some(&typo_old),
        );

        insert_repo(&conn, "rewrite", "active");
        insert_readme(&conn, "rewrite", &rewrite_now, "hash-rewrite-new");
        insert_card(
            &conn,
            "rewrite",
            r#"{"review_status":"suggested"}"#,
            Some("hash-rewrite-old"),
            Some(&rewrite_basis),
        );

        insert_repo(&conn, "confirmed", "active");
        insert_readme(&conn, "confirmed", &confirmed_now, "hash-confirmed-new");
        insert_card(
            &conn,
            "confirmed",
            r#"{"review_status":"confirmed"}"#,
            Some("hash-confirmed-old"),
            Some(&confirmed_old),
        );

        insert_repo(&conn, "nocard", "active");
        insert_readme(&conn, "nocard", &fresh, "hash-nocard");

        insert_repo(&conn, "removed", "removed");
        insert_readme(&conn, "removed", &removed_now, "hash-removed-new");
        insert_card(
            &conn,
            "removed",
            r#"{"review_status":"suggested"}"#,
            Some("hash-removed-old"),
            Some(&removed_old),
        );

        insert_repo(&conn, "fill", "active");
        insert_readme(&conn, "fill", &fill, "hash-fill");
        insert_card(
            &conn,
            "fill",
            r#"{"review_status":"suggested"}"#,
            Some("hash-fill"),
            None,
        );

        insert_repo(&conn, "adopt", "active");
        insert_readme(&conn, "adopt", &adopt, "hash-adopt-new");
        insert_card(
            &conn,
            "adopt",
            r#"{"review_status":"suggested"}"#,
            Some("hash-adopt-old"),
            None,
        );

        insert_repo(&conn, "fresh", "active");
        insert_readme(&conn, "fresh", &fresh, "hash-fresh");
        insert_card(
            &conn,
            "fresh",
            r#"{"review_status":"suggested"}"#,
            None,
            None,
        );
        drop(conn);

        let first = enqueue_changed(&db.path).expect("第一次评估应成功");
        assert_eq!(
            (first.checked, first.changed, first.material, first.queued),
            (6, 3, 2, 2)
        );
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'same'"
            )
            .as_deref(),
            Some("hash-same")
        );
        assert_eq!(job_count(&conn, "same"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'typo'"
            )
            .as_deref(),
            Some("hash-typo-new")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'typo'"
            )
            .as_deref(),
            Some(typo_old.as_str())
        );
        assert_eq!(job_count(&conn, "typo"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'rewrite'"
            )
            .as_deref(),
            Some("hash-rewrite-old")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'rewrite'"
            )
            .as_deref(),
            Some(rewrite_basis.as_str())
        );
        assert_eq!(job_count(&conn, "rewrite"), 1);
        assert_eq!(job_priority(&conn, "rewrite"), 5);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'confirmed'"
            )
            .as_deref(),
            Some("hash-confirmed-old")
        );
        assert_eq!(job_count(&conn, "confirmed"), 0);
        assert_eq!(job_count(&conn, "nocard"), 0);
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'removed'"
            )
            .as_deref(),
            Some("hash-removed-old")
        );
        assert_eq!(job_count(&conn, "removed"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'fill'"
            )
            .as_deref(),
            Some(fill.as_str())
        );
        assert_eq!(job_count(&conn, "fill"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'adopt'"
            )
            .as_deref(),
            Some("hash-adopt-new")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'adopt'"
            )
            .as_deref(),
            Some(adopt.as_str())
        );
        assert_eq!(job_count(&conn, "adopt"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'fresh'"
            ),
            None
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'fresh'"
            ),
            None
        );
        assert_eq!(job_count(&conn, "fresh"), 1);
        assert_eq!(job_priority(&conn, "fresh"), 5);
        drop(conn);

        let second = enqueue_changed(&db.path).expect("第二次评估应成功");
        assert_eq!(
            (
                second.checked,
                second.changed,
                second.material,
                second.queued
            ),
            (6, 1, 2, 0)
        );
        let conn = db.open();
        assert_eq!(job_count(&conn, "rewrite") + job_count(&conn, "fresh"), 2);
    }

    #[test]
    fn enqueue_changed_small_edit_updates_hash_only() {
        let db = TempDb::new();
        let old = prose(200, 0);
        let mut chars: Vec<char> = old.chars().collect();
        chars[40] = '\u{9FA5}';
        let now: String = chars.iter().collect();
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &now, "hash-new");
        insert_card(
            &conn,
            "r",
            r#"{"review_status":"suggested"}"#,
            Some("hash-old"),
            Some(&old),
        );
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued
            ),
            (1, 1, 0, 0)
        );
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some("hash-new")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some(old.as_str())
        );
    }

    #[test]
    fn enqueue_changed_null_basis_with_new_hash_adopts_current() {
        let db = TempDb::new();
        let now = prose(90, 0);
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &now, "hash-new");
        insert_card(
            &conn,
            "r",
            r#"{"review_status":"suggested"}"#,
            Some("hash-old"),
            None,
        );
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!((report.changed, report.material, report.queued), (1, 0, 0));
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some("hash-new")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some(now.as_str())
        );
    }

    #[test]
    fn enqueue_changed_null_hash_queues_and_leaves_columns() {
        let db = TempDb::new();
        let now = prose(90, 0);
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &now, "hash-now");
        insert_card(&conn, "r", r#"{"review_status":"suggested"}"#, None, None);
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued
            ),
            (1, 0, 1, 1)
        );
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            ),
            None
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            ),
            None
        );
        assert_eq!(job_priority(&conn, "r"), README_CHANGED_PRIORITY);
    }

    #[test]
    fn enqueue_changed_fills_null_basis_when_hash_matches() {
        let db = TempDb::new();
        let now = prose(90, 10);
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &now, "hash-now");
        insert_card(
            &conn,
            "r",
            r#"{"review_status":"suggested"}"#,
            Some("hash-now"),
            None,
        );
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued
            ),
            (1, 0, 0, 0)
        );
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some(now.as_str())
        );
        assert_eq!(job_count(&conn, "r"), 0);
    }

    #[test]
    fn enqueue_changed_does_not_duplicate_an_active_job() {
        let db = TempDb::new();
        let now = prose(90, 0);
        let conn = db.open();
        insert_repo(&conn, "queued", "active");
        insert_readme(&conn, "queued", &now, "hash-now");
        insert_card(
            &conn,
            "queued",
            r#"{"review_status":"suggested"}"#,
            None,
            None,
        );
        insert_repo(&conn, "running", "active");
        insert_readme(&conn, "running", &now, "hash-run");
        insert_card(
            &conn,
            "running",
            r#"{"review_status":"suggested"}"#,
            None,
            None,
        );
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             VALUES ('job-q', 'acct', 'repo_card', 'repository', 'queued', 3, 'queued'),
                    ('job-r', 'acct', 'repo_card', 'repository', 'running', 3, 'running')",
            [],
        )
        .expect("已有任务应能写入");
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!((report.material, report.queued), (2, 0));
        let conn = db.open();
        assert_eq!(job_count(&conn, "queued"), 1);
        assert_eq!(job_count(&conn, "running"), 1);
        assert_eq!(job_priority(&conn, "queued"), 3);
    }

    #[test]
    fn enqueue_changed_ignores_confirmed_removed_and_cardless() {
        let db = TempDb::new();
        let old = prose(80, 0);
        let now = prose(80, 400);
        let conn = db.open();
        insert_repo(&conn, "confirmed", "active");
        insert_readme(&conn, "confirmed", &now, "new");
        insert_card(
            &conn,
            "confirmed",
            r#"{"review_status":"confirmed"}"#,
            Some("old"),
            Some(&old),
        );
        insert_repo(&conn, "removed", "removed");
        insert_readme(&conn, "removed", &now, "new");
        insert_card(
            &conn,
            "removed",
            r#"{"review_status":"suggested"}"#,
            Some("old"),
            Some(&old),
        );
        insert_repo(&conn, "nocard", "active");
        insert_readme(&conn, "nocard", &now, "new");
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!(report, FollowReport::default());
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'confirmed'"
            )
            .as_deref(),
            Some("old")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'removed'"
            )
            .as_deref(),
            Some("old")
        );
        assert_eq!(
            job_count(&conn, "confirmed")
                + job_count(&conn, "removed")
                + job_count(&conn, "nocard"),
            0
        );
    }

    #[test]
    fn enqueue_changed_short_readme_with_null_hash_does_not_queue() {
        let db = TempDb::new();
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", "太短", "hash-short");
        insert_card(&conn, "r", r#"{"review_status":"suggested"}"#, None, None);
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!((report.checked, report.material, report.queued), (1, 0, 0));
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            ),
            None
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            ),
            None
        );
    }

    #[test]
    fn constants_match_the_card() {
        assert_eq!(MATERIAL_SIMILARITY, 0.80);
        assert_eq!(MAX_REFRESH_PER_RUN, 150);
        assert_eq!(README_CHANGED_PRIORITY, 5);
    }
}
