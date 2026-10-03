use crate::v2_ops;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RankItem {
    pub full_name: String,
    pub description: String,
    pub html_url: String,
    pub stars: i64,
}

pub fn parse_github_search(body: &str) -> Result<Vec<RankItem>, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| format!("GitHub 榜单解析失败：{error}"))?;
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(items
        .into_iter()
        .filter_map(|item| {
            let full_name = item.get("full_name")?.as_str()?.to_owned();
            Some(RankItem {
                full_name,
                description: item
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                html_url: item
                    .get("html_url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                stars: item
                    .get("stargazers_count")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
            })
        })
        .take(15)
        .collect())
}

pub fn parse_hn_show(body: &str) -> Result<Vec<RankItem>, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| format!("HN 榜单解析失败：{error}"))?;
    let hits = value
        .get("hits")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(hits
        .into_iter()
        .filter_map(|hit| {
            let title = hit
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let story_url = hit.get("url").and_then(Value::as_str).unwrap_or("");
            let object_id = hit.get("objectID").and_then(Value::as_str).unwrap_or("0");
            let (full_name, html_url) = github_repo(story_url).unwrap_or_else(|| {
                (
                    format!("show-hn/{object_id}"),
                    format!("https://news.ycombinator.com/item?id={object_id}"),
                )
            });
            if title.is_empty() && full_name.starts_with("show-hn/") {
                return None;
            }
            Some(RankItem {
                full_name,
                description: title,
                html_url,
                stars: hit.get("points").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .take(15)
        .collect())
}

pub fn parse_ossinsight(body: &str) -> Result<Vec<RankItem>, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| format!("OSS Insight 解析失败：{error}"))?;
    if value
        .pointer("/data_quality/status")
        .and_then(Value::as_str)
        == Some("unavailable")
    {
        let reason = value
            .pointer("/data_quality/reason")
            .and_then(Value::as_str)
            .unwrap_or("OSS Insight 趋势暂不可用");
        return Err(reason.chars().take(240).collect());
    }
    let rows = value
        .pointer("/data/rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let items = rows
        .into_iter()
        .filter_map(|row| {
            let full_name = row
                .get("repo_name")
                .or_else(|| row.get("full_name"))?
                .as_str()?
                .to_owned();
            Some(RankItem {
                full_name,
                description: row
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                html_url: row
                    .get("html_url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                stars: row.get("stars").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .take(15)
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Err("OSS Insight 没有返回可用仓库".to_owned());
    }
    Ok(items)
}

fn github_repo(url: &str) -> Option<(String, String)> {
    let marker = "github.com/";
    let start = url.find(marker)? + marker.len();
    let rest = url[start..].split(['?', '#']).next().unwrap_or("");
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?.trim();
    let name = parts.next()?.trim_end_matches(".git");
    if owner.is_empty() || name.is_empty() || owner.eq_ignore_ascii_case("topics") {
        return None;
    }
    Some((
        format!("{owner}/{name}"),
        format!("https://github.com/{owner}/{name}"),
    ))
}

pub fn refresh_default_sources(path: &Path) -> Result<String, String> {
    if path
        .to_string_lossy()
        .contains("/com.foxwork.fox-stars-lab/")
        && !path.to_string_lossy().contains(".dev")
    {
        return Err("拒绝刷新正式库榜单".to_owned());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("fox-stars-lab-dev")
        .build()
        .map_err(|error| format!("榜单客户端创建失败：{error}"))?;
    let mut notes = Vec::new();
    notes.push(fetch_into(
        path,
        "github_search",
        &client,
        "https://api.github.com/search/repositories?q=stars:%3E1000+archived:false&sort=stars&order=desc&per_page=15",
        parse_github_search,
    ));
    notes.push(fetch_into(
        path,
        "hn_show",
        &client,
        "https://hn.algolia.com/api/v1/search_by_date?tags=show_hn&hitsPerPage=15",
        parse_hn_show,
    ));
    notes.push(fetch_into(
        path,
        "ossinsight",
        &client,
        "https://api.ossinsight.io/v1/trends/repos/?period=past_24_hours",
        parse_ossinsight,
    ));
    let disabled = v2_ops::source_enabled(path, "github_trending_html")?;
    if disabled {
        return Err("GitHub Trending 网页抓取被打开了，这不符合默认关闭".to_owned());
    }
    Ok(notes.join("；"))
}

fn http_get(client: &reqwest::blocking::Client, url: &str) -> Result<String, String> {
    if let Ok(response) = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
    {
        if let Ok(response) = response.error_for_status() {
            if let Ok(body) = response.text() {
                return Ok(body);
            }
        }
    }
    let output = std::process::Command::new("curl")
        .args([
            "-fsS",
            "-A",
            "fox-stars-lab-dev",
            "-H",
            "Accept: application/vnd.github+json",
            "--max-time",
            "20",
            url,
        ])
        .output()
        .map_err(|error| format!("curl 启动失败：{error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(240)
            .collect());
    }
    String::from_utf8(output.stdout).map_err(|_| "榜单响应不是 UTF-8".to_owned())
}

fn fetch_into(
    path: &Path,
    source_id: &str,
    client: &reqwest::blocking::Client,
    url: &str,
    parse: fn(&str) -> Result<Vec<RankItem>, String>,
) -> String {
    match http_get(client, url) {
        Ok(body) => match parse(&body) {
            Ok(items) => match v2_ops::replace_ranking_snapshot(path, source_id, &items) {
                Ok(count) => format!("{source_id} 写入 {count} 条"),
                Err(error) => {
                    let _ = v2_ops::mark_ranking_error(path, source_id, &error);
                    format!("{source_id} 写入失败")
                }
            },
            Err(error) => {
                let _ = v2_ops::mark_ranking_error(path, source_id, &error);
                format!("{source_id} 保留上次快照：{error}")
            }
        },
        Err(error) => {
            let message = format!("网络失败：{error:?}");
            let _ = v2_ops::mark_ranking_error(path, source_id, &message);
            format!("{source_id} 断网，保留上次快照")
        }
    }
}

#[allow(dead_code)]
pub fn snapshot_id(source_id: &str, stamp: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{source_id}:{stamp}").as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsers_keep_real_names_and_ossinsight_outage_is_an_error() {
        let github = parse_github_search(r#"{"items":[{"full_name":"fox/tool","description":"整理链接","html_url":"https://github.com/fox/tool","stargazers_count":3}]}"#).unwrap();
        assert_eq!(github[0].full_name, "fox/tool");
        let hn = parse_hn_show(r#"{"hits":[{"title":"Show HN: 工具","url":"https://github.com/fox/tool","objectID":"9","points":4}]}"#).unwrap();
        assert_eq!(hn[0].full_name, "fox/tool");
        let error = parse_ossinsight(
            r#"{"data_quality":{"status":"unavailable","reason":"事件源不可用"}}"#,
        )
        .unwrap_err();
        assert!(error.contains("不可用"));
    }
}
