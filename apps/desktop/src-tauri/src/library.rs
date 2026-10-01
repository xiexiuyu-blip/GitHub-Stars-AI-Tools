use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayOverview {
    pub active_repositories: i64,
    pub readmes: i64,
    pub ai_documents: i64,
    pub citations: i64,
    pub inbox_items: i64,
    pub tools: i64,
    pub schema_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCard {
    pub id: String,
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub html_url: String,
    pub stars_count: i64,
    pub starred_at: String,
    pub one_liner: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub items: Vec<LibraryCard>,
    pub total_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub html_url: String,
    pub stars_count: i64,
    pub starred_at: String,
    pub summary_zh: Option<String>,
    pub readme_zh: Option<String>,
    pub topics: Vec<String>,
}

pub fn today_overview(database_path: &Path) -> Result<TodayOverview, String> {
    let connection = open_read(database_path)?;
    let (active_repositories, readmes, ai_documents, citations, inbox_items, tools, schema_version) = connection
        .query_row(
            r#"
SELECT
  (SELECT COUNT(*) FROM repositories WHERE sync_status = 'active'),
  (SELECT COUNT(*) FROM repo_readmes),
  (SELECT COUNT(*) FROM repo_ai_documents),
  (SELECT COUNT(*) FROM repo_ai_citations),
  (SELECT COUNT(*) FROM inbox_items),
  (SELECT COUNT(*) FROM tools),
  (SELECT COALESCE(MAX(version), '') FROM schema_migrations)
"#,
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .map_err(|error| format!("今日概览读取失败：{error}"))?;
    Ok(TodayOverview {
        active_repositories,
        readmes,
        ai_documents,
        citations,
        inbox_items,
        tools,
        schema_version,
    })
}

pub fn list_library_cards(
    database_path: &Path,
    keyword: &str,
    limit: i64,
    offset: i64,
) -> Result<LibraryPage, String> {
    let connection = open_read(database_path)?;
    let pattern = like_pattern(keyword);
    let total_count = connection
        .query_row(
            r#"
SELECT COUNT(*)
FROM repositories
WHERE sync_status = 'active'
  AND (
    ?1 = ''
    OR full_name LIKE ?1 ESCAPE '\'
    OR IFNULL(description, '') LIKE ?1 ESCAPE '\'
  )
"#,
            params![pattern],
            |row| row.get(0),
        )
        .map_err(|error| format!("资料库计数失败：{error}"))?;
    let mut statement = connection
        .prepare(
            r#"
SELECT
  repositories.id,
  repositories.full_name,
  repositories.description,
  repositories.language,
  repositories.html_url,
  repositories.stars_count,
  repositories.starred_at,
  repo_ai_documents.summary_zh
FROM repositories
LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
WHERE repositories.sync_status = 'active'
  AND (
    ?1 = ''
    OR repositories.full_name LIKE ?1 ESCAPE '\'
    OR IFNULL(repositories.description, '') LIKE ?1 ESCAPE '\'
    OR IFNULL(repo_ai_documents.summary_zh, '') LIKE ?1 ESCAPE '\'
  )
ORDER BY repositories.starred_at DESC
LIMIT ?2 OFFSET ?3
"#,
        )
        .map_err(|error| format!("资料库查询准备失败：{error}"))?;
    let rows = statement
        .query_map(params![pattern, limit, offset], |row| {
            Ok(LibraryCard {
                id: row.get(0)?,
                full_name: row.get(1)?,
                description: row.get(2)?,
                language: row.get(3)?,
                html_url: row.get(4)?,
                stars_count: row.get(5)?,
                starred_at: row.get(6)?,
                one_liner: row.get(7)?,
            })
        })
        .map_err(|error| format!("资料库查询失败：{error}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| format!("资料库行读取失败：{error}"))?);
    }
    Ok(LibraryPage { items, total_count })
}

pub fn get_library_item(database_path: &Path, id: &str) -> Result<Option<LibraryItem>, String> {
    let connection = open_read(database_path)?;
    connection
        .query_row(
            r#"
SELECT
  repositories.id,
  repositories.full_name,
  repositories.description,
  repositories.language,
  repositories.html_url,
  repositories.stars_count,
  repositories.starred_at,
  repo_ai_documents.summary_zh,
  repo_ai_documents.readme_zh,
  repositories.topics_json
FROM repositories
LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
WHERE repositories.id = ?1
"#,
            params![id],
            |row| {
                let topics_json: String = row.get(9)?;
                Ok(LibraryItem {
                    id: row.get(0)?,
                    full_name: row.get(1)?,
                    description: row.get(2)?,
                    language: row.get(3)?,
                    html_url: row.get(4)?,
                    stars_count: row.get(5)?,
                    starred_at: row.get(6)?,
                    summary_zh: row.get(7)?,
                    readme_zh: row.get(8)?,
                    topics: parse_topics(&topics_json),
                })
            },
        )
        .optional()
        .map_err(|error| format!("条目详情读取失败：{error}"))
}

fn open_read(database_path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(database_path)
        .map_err(|error| format!("资料库打开失败：{error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
        .map_err(|error| format!("资料库参数设置失败：{error}"))?;
    Ok(connection)
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

fn parse_topics(value: &str) -> Vec<String> {
    serde_json::from_str(value).unwrap_or_default()
}
