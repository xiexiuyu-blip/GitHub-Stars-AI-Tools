//! 按意图从收藏里找方案。只读数据库，不写任何表。
//!
//! 打分移植自原型 `template.html` 的 `tokens` / `idf` / `LEX` / `concepts` /
//! `fieldScore` / `search`。`Math.log` 就是自然对数。
//! 和原型不同的地方只限任务卡列出的几条：只返回有理解卡的仓库、关键词块、
//! 场景动态词表、并列时按 id 升序、`why` 取值更完整。

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

const BASE_LEXICON: &[&str] = &[
    "小红书",
    "公众号",
    "抖音",
    "视频号",
    "微信",
    "短视频",
    "封面",
    "海报",
    "配图",
    "插图",
    "文案",
    "字幕",
    "详情页",
    "主图",
    "标题",
    "选题",
    "脚本",
    "排版",
    "剪辑",
    "配音",
    "口播",
    "直播",
    "私信",
    "评论",
    "涨粉",
    "投放",
    "笔记",
    "爆款",
    "竞品",
    "关键词",
    "发布",
    "采集",
    "下载",
    "翻译",
    "总结",
    "链接",
    "群聊",
    "文档",
    "表格",
    "报告",
    "简历",
    "电商",
    "淘宝",
    "亚马逊",
    "seo",
    "ppt",
    "excel",
    "word",
    "pdf",
    "图片",
    "视频",
    "文章",
    "知识库",
    "爬虫",
    "自动化",
    "数据分析",
    "品牌",
    "营销",
    "广告",
    "邮件",
    "写作",
    "设计",
];

const STOP_WORDS: &[&str] = &[
    "我想",
    "想做",
    "想要",
    "怎么",
    "如何",
    "可以",
    "一个",
    "工具",
    "帮我",
    "需要",
    "里的",
    "做个",
    "怎样",
    "什么",
    "有没有",
    "能不能",
    "一下",
    "这个",
    "那个",
    "的工",
    "具链",
];

const WHY_LIMIT: usize = 80;
const HIT_LIMIT: usize = 8;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveHit {
    pub id: String,
    pub full_name: String,
    pub html_url: String,
    pub stars_count: i64,
    pub one_liner: Option<String>,
    pub form: Option<String>,
    pub scenario_ids: Vec<String>,
    pub matched: Vec<String>,
    pub missing: Vec<String>,
    pub why: String,
    pub score: f64,
    pub full_match: bool,
    pub has_card: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveResult {
    pub query: String,
    pub intents: Vec<String>,
    pub extras: Vec<String>,
    pub hits: Vec<SolveHit>,
    pub preselected: Vec<String>,
    pub full_match_count: i64,
    pub gap_intents: Vec<String>,
    pub suggested_scenario_id: Option<String>,
    pub suggested_scenario_count: Option<i64>,
    pub pending_matches: i64,
}

struct ScenarioRecord {
    id: String,
    name_zh: String,
    keywords: Vec<String>,
    sort_order: i64,
    archived: bool,
}

struct TextBlocks {
    name: String,
    one_liner: String,
    capabilities: String,
    problems: String,
    keywords: String,
    scenarios: String,
    summary: String,
    description: String,
}

struct RepoRecord {
    id: String,
    full_name: String,
    html_url: String,
    stars: i64,
    one_liner: String,
    problems: Vec<String>,
    capabilities: Vec<String>,
    form: Option<String>,
    scenario_ids: Vec<String>,
    blocks: TextBlocks,
    has_card: bool,
}

struct CardBits {
    problems: Vec<String>,
    capabilities: Vec<String>,
    form: Option<String>,
}

struct ScoreParts {
    score: f64,
    matched: Vec<String>,
    missing: Vec<String>,
}

impl TextBlocks {
    fn contains_token(&self, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        self.name.contains(token)
            || self.one_liner.contains(token)
            || self.capabilities.contains(token)
            || self.problems.contains(token)
            || self.keywords.contains(token)
            || self.scenarios.contains(token)
            || self.summary.contains(token)
            || self.description.contains(token)
    }
}

pub fn solve_query(db_path: &Path, query: &str) -> Result<SolveResult, String> {
    if query.trim().is_empty() {
        return Err("先写一句你想做的事".to_owned());
    }
    let connection = open_read(db_path)?;
    let scenarios = load_scenarios(&connection)?;
    let lexicon = lexicon(&scenarios);
    let (main, extra) = concepts(query, &lexicon);
    if main.is_empty() && extra.is_empty() {
        return Ok(empty_result(query));
    }

    let repos = load_repos(&connection, &scenarios)?;
    let mut cache = HashMap::new();
    let mut scored = Vec::new();
    for (index, repo) in repos.iter().enumerate() {
        if let Some(parts) = score_repo(repo, &main, &extra, &repos, &mut cache) {
            scored.push((index, parts));
        }
    }
    scored.sort_by(|left, right| {
        right
            .1
            .score
            .total_cmp(&left.1.score)
            .then_with(|| repos[right.0].stars.cmp(&repos[left.0].stars))
            .then_with(|| repos[left.0].id.cmp(&repos[right.0].id))
    });

    let mut terms = main.clone();
    terms.extend(extra.iter().cloned());
    let mut hits = Vec::new();
    for (index, parts) in scored
        .into_iter()
        .filter(|(index, _)| repos[*index].has_card)
        .take(HIT_LIMIT)
    {
        let repo = &repos[index];
        // 没有意图词时不知道用户要什么，不能说「全部对上」。
        let full_match = !main.is_empty() && parts.missing.is_empty();
        hits.push(SolveHit {
            id: repo.id.clone(),
            full_name: repo.full_name.clone(),
            html_url: repo.html_url.clone(),
            stars_count: repo.stars,
            one_liner: if repo.one_liner.trim().is_empty() {
                None
            } else {
                Some(repo.one_liner.clone())
            },
            form: repo.form.clone(),
            scenario_ids: repo.scenario_ids.clone(),
            matched: parts.matched,
            missing: parts.missing,
            why: why_text(repo, &terms),
            score: parts.score,
            full_match,
            has_card: true,
        });
    }

    // 缺口只看返回的最多 8 条。排到第 9 名以后的仓库，即使对上了某个意图，也不算补上。
    let gap_intents = main
        .iter()
        .filter(|intent| {
            !hits
                .iter()
                .any(|hit| hit.matched.iter().any(|item| item == *intent))
        })
        .cloned()
        .collect::<Vec<_>>();
    let preselected = hits
        .iter()
        .filter(|hit| hit.full_match)
        .take(3)
        .map(|hit| hit.id.clone())
        .collect::<Vec<_>>();
    let full_match_count = hits.iter().filter(|hit| hit.full_match).count() as i64;
    // 名称整段包含全部意图，或描述整段包含全部意图。不把两个字段拼在一起凑。
    let pending_matches = if main.is_empty() {
        0
    } else {
        repos
            .iter()
            .filter(|repo| {
                !repo.has_card
                    && (contains_all(&repo.blocks.name, &main)
                        || contains_all(&repo.blocks.description, &main))
            })
            .count() as i64
    };
    let suggested = scenarios
        .iter()
        .find(|scenario| !scenario.archived && scenario_covers(scenario, &terms));
    let suggested_scenario_id = suggested.map(|scenario| scenario.id.clone());
    let suggested_scenario_count = suggested.map(|scenario| {
        repos
            .iter()
            .filter(|repo| repo.scenario_ids.iter().any(|id| id == &scenario.id))
            .count() as i64
    });

    Ok(SolveResult {
        query: query.to_owned(),
        intents: main,
        extras: extra,
        hits,
        preselected,
        full_match_count,
        gap_intents,
        suggested_scenario_id,
        suggested_scenario_count,
        pending_matches,
    })
}

fn empty_result(query: &str) -> SolveResult {
    SolveResult {
        query: query.to_owned(),
        intents: Vec::new(),
        extras: Vec::new(),
        hits: Vec::new(),
        preselected: Vec::new(),
        full_match_count: 0,
        gap_intents: Vec::new(),
        suggested_scenario_id: None,
        suggested_scenario_count: None,
        pending_matches: 0,
    }
}

fn open_read(db_path: &Path) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("找方案数据库打开失败：{error}"))?;
    connection
        .busy_timeout(Duration::from_millis(5000))
        .map_err(|error| format!("找方案数据库参数设置失败：{error}"))?;
    connection
        .execute_batch("PRAGMA query_only = ON;")
        .map_err(|error| format!("找方案数据库只读设置失败：{error}"))?;
    Ok(connection)
}

fn load_scenarios(connection: &Connection) -> Result<Vec<ScenarioRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, name_zh, keywords_json, sort_order, archived
             FROM scenarios
             ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|error| format!("找方案场景读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(|error| format!("找方案场景查询失败：{error}"))?;
    let mut scenarios = Vec::new();
    for row in rows {
        let (id, name_zh, keywords_json, sort_order, archived) =
            row.map_err(|error| format!("找方案场景行读取失败：{error}"))?;
        scenarios.push(ScenarioRecord {
            id,
            name_zh,
            keywords: parse_json_strings(&keywords_json),
            sort_order,
            archived: archived != 0,
        });
    }
    Ok(scenarios)
}

fn load_repos(
    connection: &Connection,
    scenarios: &[ScenarioRecord],
) -> Result<Vec<RepoRecord>, String> {
    let by_id: HashMap<&str, &ScenarioRecord> = scenarios
        .iter()
        .map(|scenario| (scenario.id.as_str(), scenario))
        .collect();
    let mut grouped: HashMap<String, Vec<(i64, String)>> = HashMap::new();
    {
        let mut statement = connection
            .prepare(
                "SELECT entity_scenarios.entity_id, scenarios.sort_order, entity_scenarios.scenario_id
                 FROM entity_scenarios
                 JOIN scenarios ON scenarios.id = entity_scenarios.scenario_id
                 WHERE entity_scenarios.entity_type = 'repository'
                   AND entity_scenarios.status <> 'rejected'",
            )
            .map_err(|error| format!("找方案归类读取失败：{error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| format!("找方案归类查询失败：{error}"))?;
        for row in rows {
            let (entity_id, sort_order, scenario_id) =
                row.map_err(|error| format!("找方案归类行读取失败：{error}"))?;
            grouped
                .entry(entity_id)
                .or_default()
                .push((sort_order, scenario_id));
        }
    }
    for links in grouped.values_mut() {
        links.sort();
    }

    let mut statement = connection
        .prepare(
            "SELECT repositories.id, repositories.full_name, repositories.html_url,
                    repositories.stars_count, IFNULL(repositories.description, ''),
                    IFNULL(repo_ai_documents.one_liner_zh, ''),
                    IFNULL(repo_ai_documents.summary_zh, ''),
                    IFNULL(repo_ai_documents.keywords_json, '[]'),
                    IFNULL(repo_ai_documents.card_json, '')
             FROM repositories
             LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
             WHERE repositories.sync_status = 'active'",
        )
        .map_err(|error| format!("找方案仓库读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .map_err(|error| format!("找方案仓库查询失败：{error}"))?;

    let mut repos = Vec::new();
    for row in rows {
        let (
            id,
            full_name,
            html_url,
            stars,
            description,
            one_liner,
            summary,
            keywords_json,
            card_json,
        ) = row.map_err(|error| format!("找方案仓库行读取失败：{error}"))?;
        let parsed = read_card_json(&card_json);
        let shown = crate::card_view::display_one_liner(Some(one_liner.as_str()), &parsed)
            .unwrap_or_default();
        let card = parse_card(&parsed);
        let keywords = parse_json_strings(&keywords_json);
        let links = grouped.get(&id).cloned().unwrap_or_default();
        let scenario_ids = links
            .iter()
            .map(|(_, scenario_id)| scenario_id.clone())
            .collect::<Vec<_>>();
        let scenario_names = scenario_ids
            .iter()
            .filter_map(|scenario_id| {
                by_id
                    .get(scenario_id.as_str())
                    .map(|item| item.name_zh.clone())
            })
            .collect::<Vec<_>>();
        let has_card = !one_liner.trim().is_empty();
        repos.push(RepoRecord {
            id,
            html_url,
            stars,
            problems: card.problems.clone(),
            capabilities: card.capabilities.clone(),
            form: card.form,
            scenario_ids,
            has_card,
            blocks: TextBlocks {
                name: full_name.to_lowercase(),
                one_liner: shown.to_lowercase(),
                capabilities: join_lower(&card.capabilities),
                problems: join_lower(&card.problems),
                keywords: join_lower(&keywords),
                scenarios: join_lower(&scenario_names),
                summary: summary.to_lowercase(),
                description: description.to_lowercase(),
            },
            full_name,
            one_liner: shown,
        });
    }
    Ok(repos)
}

fn score_repo(
    repo: &RepoRecord,
    main: &[String],
    extra: &[String],
    repos: &[RepoRecord],
    cache: &mut HashMap<String, f64>,
) -> Option<ScoreParts> {
    let mut score = 0.0;
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    for intent in main {
        let weight = cached_idf(cache, intent, repos);
        let gained = field_score(&repo.blocks, intent, weight);
        if gained > 0.0 {
            matched.push(intent.clone());
            score += gained;
        } else {
            missing.push(intent.clone());
        }
    }
    let mut extra_hits = 0usize;
    for term in extra {
        let weight = cached_idf(cache, term, repos);
        let gained = field_score(&repo.blocks, term, weight);
        if gained > 0.0 {
            extra_hits += 1;
            score += 0.3 * gained;
        }
    }
    if matched.len() > 1 {
        score *= 1.0 + 0.6 * (matched.len() as f64 - 1.0);
    }
    // 有意图词时至少对上一个；没有意图词时至少对上一半额外词，避免两个字的碎片硬凑。
    if !main.is_empty() {
        if matched.is_empty() {
            return None;
        }
    } else if !extra.is_empty() {
        let needed = extra.len().div_ceil(2);
        if extra_hits < needed {
            return None;
        }
    }
    if score > 0.0 {
        Some(ScoreParts {
            score,
            matched,
            missing,
        })
    } else {
        None
    }
}

fn cached_idf(cache: &mut HashMap<String, f64>, token: &str, repos: &[RepoRecord]) -> f64 {
    if let Some(value) = cache.get(token) {
        return *value;
    }
    let docs = repos
        .iter()
        .filter(|repo| repo.blocks.contains_token(token))
        .count();
    let value = idf_from_counts(repos.len(), docs);
    cache.insert(token.to_owned(), value);
    value
}

fn idf_from_counts(total: usize, docs_with_token: usize) -> f64 {
    let n = total as f64;
    let df = docs_with_token as f64;
    (1.0 + n / (1.0 + df)).ln()
}

fn field_score(blocks: &TextBlocks, token: &str, idf_weight: f64) -> f64 {
    if token.is_empty() {
        return 0.0;
    }
    let mut max = 0.0_f64;
    if blocks.name.contains(token) {
        max = max.max(3.0 * idf_weight);
    }
    if blocks.one_liner.contains(token) {
        max = max.max(2.5 * idf_weight);
    }
    if blocks.keywords.contains(token) {
        max = max.max(2.5 * idf_weight);
    }
    if blocks.capabilities.contains(token) || blocks.problems.contains(token) {
        max = max.max(2.0 * idf_weight);
    }
    if blocks.scenarios.contains(token) {
        max = max.max(2.0 * idf_weight);
    }
    if blocks.summary.contains(token) {
        max = max.max(1.5 * idf_weight);
    }
    if blocks.description.contains(token) {
        max = max.max(idf_weight);
    }
    max
}

fn lexicon(scenarios: &[ScenarioRecord]) -> Vec<String> {
    let mut words = BASE_LEXICON
        .iter()
        .map(|word| (*word).to_owned())
        .collect::<Vec<_>>();
    let mut seen = words.iter().cloned().collect::<HashSet<_>>();
    let mut ordered = scenarios.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.sort_order
            .cmp(&right.sort_order)
            .then_with(|| left.id.cmp(&right.id))
    });
    for scenario in ordered {
        if scenario.archived {
            continue;
        }
        for segment in scenario.name_zh.split('\u{00B7}') {
            push_lexicon_word(&mut words, &mut seen, segment);
        }
        for keyword in &scenario.keywords {
            let trimmed = keyword.trim();
            if trimmed.chars().count() < 2 {
                continue;
            }
            push_lexicon_word(&mut words, &mut seen, trimmed);
        }
    }
    words.sort_by(|left, right| right.chars().count().cmp(&left.chars().count()));
    words
}

fn push_lexicon_word(words: &mut Vec<String>, seen: &mut HashSet<String>, raw: &str) {
    let word = raw.trim().to_lowercase();
    if word.is_empty() || !seen.insert(word.clone()) {
        return;
    }
    words.push(word);
}

fn concepts(query: &str, lexicon: &[String]) -> (Vec<String>, Vec<String>) {
    let mut rest = query.to_lowercase();
    let mut main = Vec::new();
    for word in lexicon {
        if word.is_empty() {
            continue;
        }
        if rest.contains(word) {
            main.push(word.clone());
            rest = rest.replace(word, " ");
        }
    }
    let extra = tokens(&rest)
        .into_iter()
        .filter(|token| token.chars().count() >= 2)
        .collect();
    (main, extra)
}

fn tokens(query: &str) -> Vec<String> {
    let mut raw = Vec::new();
    for segment in split_token_segments(&query.to_lowercase()) {
        if segment.chars().any(is_han) {
            let han = segment.chars().filter(|ch| is_han(*ch)).collect::<Vec<_>>();
            if han.is_empty() {
                continue;
            }
            if han.len() <= 2 {
                raw.push(han.into_iter().collect::<String>());
            } else {
                for index in 0..han.len() - 1 {
                    raw.push(format!("{}{}", han[index], han[index + 1]));
                }
            }
            let mut latin = String::new();
            for ch in segment.chars() {
                if is_han(ch) {
                    push_latin(&mut raw, &latin);
                    latin.clear();
                } else {
                    latin.push(ch);
                }
            }
            push_latin(&mut raw, &latin);
        } else if segment.chars().count() > 1 {
            raw.push(segment);
        }
    }
    let mut seen = HashSet::new();
    raw.into_iter()
        .filter(|token| seen.insert(token.clone()))
        .filter(|token| !is_stop(token))
        .collect()
}

fn push_latin(out: &mut Vec<String>, text: &str) {
    if text.chars().count() > 1 {
        out.push(text.to_owned());
    }
}

fn split_token_segments(input: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    for ch in input.chars() {
        if is_token_separator(ch) {
            if !current.is_empty() {
                segments.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn is_token_separator(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, ',' | '，' | '。' | ';' | '；' | '、' | '/')
}

fn is_han(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

fn is_stop(word: &str) -> bool {
    STOP_WORDS.iter().any(|stop| *stop == word)
}

fn why_text(repo: &RepoRecord, terms: &[String]) -> String {
    let mut fields = Vec::new();
    for problem in &repo.problems {
        fields.push(problem.as_str());
    }
    for capability in &repo.capabilities {
        fields.push(capability.as_str());
    }
    fields.push(repo.one_liner.as_str());
    for field in &fields {
        let lower = field.to_lowercase();
        if terms
            .iter()
            .any(|term| !term.is_empty() && lower.contains(term))
        {
            return truncate_chars(field, WHY_LIMIT);
        }
    }
    let fallback = repo
        .problems
        .iter()
        .chain(repo.capabilities.iter())
        .chain(std::iter::once(&repo.one_liner))
        .find(|text| !text.trim().is_empty())
        .map(String::as_str)
        .unwrap_or("");
    truncate_chars(fallback, WHY_LIMIT)
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut out = text.chars().take(limit).collect::<String>();
    out.push('…');
    out
}

fn scenario_covers(scenario: &ScenarioRecord, terms: &[String]) -> bool {
    let name = scenario.name_zh.to_lowercase();
    terms.iter().any(|term| {
        !term.is_empty()
            && (name.contains(term)
                || scenario
                    .keywords
                    .iter()
                    .any(|keyword| keyword.to_lowercase().contains(term)))
    })
}

fn contains_all(haystack: &str, terms: &[String]) -> bool {
    terms
        .iter()
        .all(|term| !term.is_empty() && haystack.contains(term))
}

fn join_lower(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| part.trim().to_lowercase())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_json_strings(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return Vec::new();
    };
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

fn read_card_json(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or(Value::Null)
}

fn parse_card(value: &Value) -> CardBits {
    let clean = crate::card_view::sanitize(value);
    let Some(obj) = clean.as_ref().as_object() else {
        return CardBits {
            problems: Vec::new(),
            capabilities: Vec::new(),
            form: None,
        };
    };
    let capabilities = obj
        .get("capabilities")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|fields| fields.get("name_zh"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let problems = obj
        .get("problems_solved")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let form = obj
        .get("form")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    CardBits {
        problems,
        capabilities,
        form,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    fn approx(actual: f64, expected: f64) {
        let diff = (actual - expected).abs();
        assert!(
            diff < 1e-9,
            "actual={actual} expected={expected} diff={diff}"
        );
    }

    fn doc(
        name: &str,
        one_liner: &str,
        capabilities: &str,
        problems: &str,
        keywords: &str,
        scenarios: &str,
        summary: &str,
        description: &str,
    ) -> TextBlocks {
        TextBlocks {
            name: name.to_owned(),
            one_liner: one_liner.to_owned(),
            capabilities: capabilities.to_owned(),
            problems: problems.to_owned(),
            keywords: keywords.to_owned(),
            scenarios: scenarios.to_owned(),
            summary: summary.to_owned(),
            description: description.to_owned(),
        }
    }

    struct TempDb {
        path: PathBuf,
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let text = self.path.to_string_lossy().to_string();
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(format!("{text}-wal"));
            let _ = fs::remove_file(format!("{text}-shm"));
        }
    }

    fn prepare(label: &str, setup: impl FnOnce(&mut Connection)) -> TempDb {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时间应可用")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fsl-solve-{label}-{nanos}-{n}.sqlite3"));
        crate::db::migrations::migrate_database(&path).expect("测试库应能迁移");
        let mut conn = Connection::open(&path).expect("应能打开测试库");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("应能打开外键");
        conn.execute(
            "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
            [],
        )
        .expect("应能插入账号");
        setup(&mut conn);
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("应能检查点");
        drop(conn);
        TempDb { path }
    }

    fn run(db: &TempDb, query: &str) -> SolveResult {
        solve_query(&db.path, query)
            .unwrap_or_else(|error| panic!("查询不应失败：{error}；问题：{query}"))
    }

    struct NewRepo<'a> {
        id: &'a str,
        full_name: &'a str,
        description: &'a str,
        stars: i64,
        status: &'a str,
        one_liner: Option<&'a str>,
        summary: &'a str,
        keywords_json: &'a str,
        card_json: Option<&'a str>,
    }

    fn insert_repo(conn: &Connection, repo: &NewRepo<'_>) {
        let (owner, name) = repo
            .full_name
            .split_once('/')
            .unwrap_or(("fox", repo.full_name));
        conn.execute(
            "INSERT INTO repositories(id, account_id, owner, name, full_name, description, html_url, stars_count, starred_at, sync_status)
             VALUES (?1, 'acct', ?2, ?3, ?4, ?5, ?6, ?7, '2026-09-01T00:00:00Z', ?8)",
            params![
                repo.id,
                owner,
                name,
                repo.full_name,
                repo.description,
                format!("https://github.com/{}", repo.full_name),
                repo.stars,
                repo.status,
            ],
        )
        .unwrap_or_else(|error| panic!("插入仓库 {} 失败：{error}", repo.id));
        if let Some(one_liner) = repo.one_liner {
            conn.execute(
                "INSERT INTO repo_ai_documents(repo_id, summary_zh, keywords_json, suggested_tags_json, model, prompt_version, source_hash, generated_at, one_liner_zh, card_json)
                 VALUES (?1, ?2, ?3, '[]', 'test', 'card-v1', 'hash', '2026-09-01T00:00:00Z', ?4, ?5)",
                params![repo.id, repo.summary, repo.keywords_json, one_liner, repo.card_json],
            )
            .unwrap_or_else(|error| panic!("插入理解卡 {} 失败：{error}", repo.id));
        }
    }

    fn insert_scenario(
        conn: &Connection,
        id: &str,
        slug: &str,
        name: &str,
        keywords_json: &str,
        sort_order: i64,
        archived: i64,
    ) {
        conn.execute(
            "INSERT INTO scenarios(id, account_id, slug, name_zh, keywords_json, sort_order, archived)
             VALUES (?1, 'acct', ?2, ?3, ?4, ?5, ?6)",
            params![id, slug, name, keywords_json, sort_order, archived],
        )
        .unwrap_or_else(|error| panic!("插入场景 {id} 失败：{error}"));
    }

    fn link_scenario(conn: &Connection, repo_id: &str, scenario_id: &str, status: &str) {
        conn.execute(
            "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, status)
             VALUES ('acct', 'repository', ?1, ?2, ?3)",
            params![repo_id, scenario_id, status],
        )
        .unwrap_or_else(|error| panic!("插入归类 {repo_id}/{scenario_id} 失败：{error}"));
    }

    fn card_json(form: &str, caps: &[&str], problems: &[&str]) -> String {
        let capabilities = caps
            .iter()
            .map(|name| json!({"name_zh": name}))
            .collect::<Vec<_>>();
        json!({
            "form": form,
            "capabilities": capabilities,
            "problems_solved": problems,
        })
        .to_string()
    }

    fn base_lexicon() -> Vec<String> {
        lexicon(&[])
    }

    fn scenario(
        id: &str,
        name: &str,
        keywords: &[&str],
        sort_order: i64,
        archived: bool,
    ) -> ScenarioRecord {
        ScenarioRecord {
            id: id.to_owned(),
            name_zh: name.to_owned(),
            keywords: keywords.iter().map(|word| (*word).to_owned()).collect(),
            sort_order,
            archived,
        }
    }

    #[test]
    fn tokens_keeps_latin_inside_chinese() {
        assert_eq!(
            tokens("做PPT封面"),
            vec!["做封".to_owned(), "封面".to_owned(), "ppt".to_owned()]
        );
        assert_eq!(
            tokens("做 PPT 封面"),
            vec!["做".to_owned(), "ppt".to_owned(), "封面".to_owned()]
        );
        assert_eq!(
            tokens("hello世界"),
            vec!["世界".to_owned(), "hello".to_owned()]
        );
        assert_eq!(tokens("ab.cd"), vec!["ab.cd".to_owned()]);
    }

    #[test]
    fn tokens_keeps_short_han_fragment() {
        assert_eq!(tokens("做"), vec!["做".to_owned()]);
        assert_eq!(tokens("中"), vec!["中".to_owned()]);
        assert_eq!(tokens("封面"), vec!["封面".to_owned()]);
    }

    #[test]
    fn tokens_splits_long_han_into_bigrams() {
        assert_eq!(tokens("小红书"), vec!["小红".to_owned(), "红书".to_owned()]);
        assert_eq!(tokens("中文的"), vec!["中文".to_owned(), "文的".to_owned()]);
    }

    #[test]
    fn tokens_removes_stopwords() {
        assert_eq!(
            tokens("我想做一个工具"),
            vec!["做一".to_owned(), "个工".to_owned()]
        );
        assert!(tokens("怎么").is_empty());
        assert!(tokens("工具").is_empty());
        assert!(tokens("帮我").is_empty());
    }

    #[test]
    fn tokens_dedups_and_splits_punctuation() {
        assert_eq!(
            tokens("封面封面"),
            vec!["封面".to_owned(), "面封".to_owned()]
        );
        assert_eq!(
            tokens("封面,封面，海报。字幕;脚本；标题、笔记/海报"),
            vec![
                "封面".to_owned(),
                "海报".to_owned(),
                "字幕".to_owned(),
                "脚本".to_owned(),
                "标题".to_owned(),
                "笔记".to_owned(),
            ]
        );
        assert_eq!(tokens("ppt/ppt"), vec!["ppt".to_owned()]);
    }

    #[test]
    fn concepts_splits_xiaohongshu_cover() {
        let (main, extra) = concepts("做小红书封面", &base_lexicon());
        assert_eq!(main, vec!["小红书".to_owned(), "封面".to_owned()]);
        assert!(extra.is_empty());
    }

    #[test]
    fn concepts_short_video_subtitle_drops_video() {
        let (main, extra) = concepts("短视频加字幕", &base_lexicon());
        assert_eq!(main, vec!["短视频".to_owned(), "字幕".to_owned()]);
        assert!(!main.iter().any(|word| word == "视频"));
        assert!(extra.is_empty());
    }

    #[test]
    fn concepts_prefers_longest_scenario_keyword() {
        let scenarios = vec![scenario("s", "别名", &["小红书封面"], 1, false)];
        let lex = lexicon(&scenarios);
        let (main, extra) = concepts("做小红书封面", &lex);
        assert_eq!(main, vec!["小红书封面".to_owned()]);
        assert!(extra.is_empty());
    }

    #[test]
    fn concepts_drops_extra_shorter_than_two() {
        let (main, extra) = concepts("封面 ab c 做", &base_lexicon());
        assert_eq!(main, vec!["封面".to_owned()]);
        assert_eq!(extra, vec!["ab".to_owned()]);
    }

    #[test]
    fn concepts_ppt_document_is_case_insensitive() {
        let (main, extra) = concepts("做 PPT 和文档", &base_lexicon());
        assert_eq!(main, vec!["ppt".to_owned(), "文档".to_owned()]);
        assert!(extra.is_empty());
    }

    #[test]
    fn lexicon_extends_sorts_and_dedups() {
        assert_eq!("公众号·微信".chars().nth(3), Some('\u{00B7}'));
        let scenarios = vec![
            scenario("old", "不会用的归档", &["不该出现"], 1, true),
            scenario(
                "wx",
                "公众号·微信",
                &["小红书封面", "a", "SEO", "笔记"],
                2,
                false,
            ),
            scenario("ops", "运营 · 品牌", &[], 3, false),
        ];
        let lex = lexicon(&scenarios);
        assert_eq!(
            &lex[..4],
            [
                "excel".to_owned(),
                "小红书封面".to_owned(),
                "word".to_owned(),
                "数据分析".to_owned(),
            ]
        );
        let mut previous = usize::MAX;
        let mut seen = HashSet::new();
        for word in &lex {
            let length = word.chars().count();
            assert!(length <= previous, "{word} 没有按长度从长到短");
            previous = length;
            assert!(seen.insert(word.clone()), "重复词 {word}");
        }
        assert!(lex.contains(&"运营".to_owned()));
        assert!(lex.contains(&"公众号".to_owned()));
        assert!(lex.contains(&"微信".to_owned()));
        assert!(!lex.contains(&"不该出现".to_owned()));
        assert!(!lex.contains(&"不会用的归档".to_owned()));
        assert!(!lex.iter().any(|word| word == "a"));
        assert_eq!(lex.iter().filter(|word| word.as_str() == "seo").count(), 1);
        assert_eq!(lex.iter().filter(|word| word.as_str() == "笔记").count(), 1);
        let cover = lex.iter().position(|word| word == "小红书封面").unwrap();
        let short = lex.iter().position(|word| word == "小红书").unwrap();
        assert!(cover < short);
    }

    #[test]
    fn idf_matches_hand_calculation() {
        let docs = vec![
            doc("封面", "", "", "", "封面", "", "", ""),
            doc("", "", "", "", "", "", "其他", ""),
            doc("", "", "", "", "", "", "", "无"),
            doc("", "", "", "", "设计", "", "", ""),
        ];
        let df_cover = docs
            .iter()
            .filter(|item| item.contains_token("封面"))
            .count();
        assert_eq!(df_cover, 1, "同一仓库的多块命中只算 1 个 df");
        let cover = idf_from_counts(docs.len(), df_cover);
        let missing = idf_from_counts(docs.len(), 0);
        // N=4，df(封面)=1 → ln(1 + 4/2) = ln(3)
        assert!((cover - 1.0986122886681098).abs() < 1e-12, "{cover}");
        // df=0 → ln(1 + 4/1) = ln(5)
        assert!((missing - 1.6094379124341003).abs() < 1e-12, "{missing}");
        assert!((cover - (1.0_f64 + 4.0 / 2.0).ln()).abs() < 1e-12);
        assert!((missing - (1.0_f64 + 4.0 / 1.0).ln()).abs() < 1e-12);
    }

    #[test]
    fn field_score_weights_take_the_max() {
        let weight = 2.0;
        assert_eq!(
            field_score(&doc("封面", "", "", "", "", "", "", ""), "封面", weight),
            6.0
        );
        assert_eq!(
            field_score(&doc("", "封面", "", "", "", "", "", ""), "封面", weight),
            5.0
        );
        assert_eq!(
            field_score(&doc("", "", "", "", "封面", "", "", ""), "封面", weight),
            5.0
        );
        assert_eq!(
            field_score(&doc("", "", "封面", "", "", "", "", ""), "封面", weight),
            4.0
        );
        assert_eq!(
            field_score(&doc("", "", "", "封面", "", "", "", ""), "封面", weight),
            4.0
        );
        assert_eq!(
            field_score(&doc("", "", "", "", "", "封面", "", ""), "封面", weight),
            4.0
        );
        assert_eq!(
            field_score(&doc("", "", "", "", "", "", "封面", ""), "封面", weight),
            3.0
        );
        assert_eq!(
            field_score(&doc("", "", "", "", "", "", "", "封面"), "封面", weight),
            2.0
        );
        assert_eq!(
            field_score(
                &doc("封面", "", "", "", "", "", "封面", "封面"),
                "封面",
                weight
            ),
            6.0
        );
        assert_eq!(
            field_score(
                &doc("", "封面", "封面", "", "封面", "", "", ""),
                "封面",
                weight
            ),
            5.0
        );
        assert_eq!(
            field_score(&doc("", "", "", "", "", "", "", ""), "封面", weight),
            0.0
        );
    }

    #[test]
    fn multi_intent_bonus_includes_extra_before_multiplier() {
        let db = prepare("bonus", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "both",
                    full_name: "lab/小红书封面extra",
                    description: "",
                    stars: 1,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let weight = (1.0_f64 + 1.0 / 2.0).ln();
        let both = run(&db, "小红书封面 extra");
        assert_eq!(both.intents, vec!["小红书".to_owned(), "封面".to_owned()]);
        assert_eq!(both.extras, vec!["extra".to_owned()]);
        assert_eq!(both.hits.len(), 1);
        assert!(both.hits[0].full_match);
        let with_extra = (3.0 * weight + 3.0 * weight + 0.3 * 3.0 * weight) * 1.6;
        approx(both.hits[0].score, with_extra);
        let added_after = (3.0 * weight + 3.0 * weight) * 1.6 + 0.3 * 3.0 * weight;
        assert!((both.hits[0].score - added_after).abs() > 1e-6);

        let intents_only = run(&db, "小红书封面");
        approx(
            intents_only.hits[0].score,
            (3.0 * weight + 3.0 * weight) * 1.6,
        );
        assert!(intents_only.extras.is_empty());
    }

    #[test]
    fn extra_terms_use_point_three_weight() {
        let db = prepare("extra", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "one",
                    full_name: "lab/zz-only",
                    description: "这是封面",
                    stars: 1,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "封面 zz");
        assert_eq!(result.intents, vec!["封面".to_owned()]);
        assert_eq!(result.extras, vec!["zz".to_owned()]);
        let weight = (1.0_f64 + 1.0 / 2.0).ln();
        approx(result.hits[0].score, weight + 0.3 * 3.0 * weight);
        assert_eq!(result.hits[0].matched, vec!["封面".to_owned()]);
    }

    #[test]
    fn ties_sort_by_stars_then_id() {
        let db = prepare("ties", |conn| {
            for (id, full_name, stars) in [
                ("b", "fox/aaa", 5),
                ("a", "fox/zzz", 5),
                ("c", "fox/mmm", 9),
            ] {
                let mut repo = NewRepo {
                    id,
                    full_name,
                    description: "描述",
                    stars,
                    status: "active",
                    one_liner: Some("同款封面"),
                    summary: "摘要",
                    keywords_json: "[\"封面\"]",
                    card_json: None,
                };
                repo.stars = stars;
                insert_repo(conn, &repo);
            }
        });
        let result = run(&db, "封面");
        let ids = result
            .hits
            .iter()
            .map(|hit| hit.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["c", "a", "b"]);
        approx(result.hits[0].score, result.hits[1].score);
        approx(result.hits[1].score, result.hits[2].score);
    }

    #[test]
    fn fixture_five_prototype_queries() {
        let db = prepare("fixture12", |conn| {
            insert_scenario(
                conn,
                "sc-xhs",
                "xhs",
                "小红书",
                "[\"封面\",\"笔记\"]",
                10,
                0,
            );
            insert_scenario(
                conn,
                "sc-wx",
                "wx",
                "公众号·微信",
                "[\"文章\",\"配图\"]",
                20,
                0,
            );
            insert_scenario(
                conn,
                "sc-video",
                "video",
                "短视频",
                "[\"字幕\",\"剪辑\"]",
                30,
                0,
            );
            insert_scenario(conn, "sc-doc", "doc", "文档", "[\"ppt\",\"排版\"]", 40, 0);
            insert_scenario(conn, "sc-ec", "ec", "电商", "[\"详情页\",\"主图\"]", 50, 0);
            insert_scenario(
                conn,
                "sc-arch",
                "arch",
                "不会用的归档",
                "[\"小红书封面\"]",
                1,
                1,
            );

            let samples = [
                (
                    "r01",
                    "fox/xhs-cover",
                    5200_i64,
                    "帮你做小红书封面",
                    "一个模板库",
                    r#"["小红书","封面","模板"]"#,
                    "Xiaohongshu cover templates",
                    "app",
                    "生成小红书封面",
                    "想快速做小红书封面",
                ),
                (
                    "r02",
                    "fox/xhs-notes",
                    2100,
                    "小红书笔记和选题助手",
                    "写笔记",
                    r#"["小红书","笔记"]"#,
                    "notes helper",
                    "skill",
                    "写小红书文案",
                    "不知道发什么笔记",
                ),
                (
                    "r03",
                    "fox/poster-lab",
                    8000,
                    "通用封面和海报生成器",
                    "海报",
                    r#"["封面","海报"]"#,
                    "poster maker",
                    "app",
                    "生成封面海报",
                    "需要一张海报",
                ),
                (
                    "r04",
                    "fox/wx-article-images",
                    3400,
                    "给公众号文章自动配图",
                    "配图流程",
                    r#"["公众号","文章","配图"]"#,
                    "wechat images",
                    "mcp",
                    "公众号文章配图",
                    "公众号文章缺配图",
                ),
                (
                    "r05",
                    "fox/wx-layout",
                    1800,
                    "公众号排版工具",
                    "排版",
                    r#"["公众号","排版"]"#,
                    "layout",
                    "app",
                    "公众号一键排版",
                    "公众号排版太花时间",
                ),
                (
                    "r06",
                    "fox/video-subtitle",
                    4600,
                    "给短视频自动加字幕",
                    "字幕",
                    r#"["短视频","字幕"]"#,
                    "subtitles",
                    "cli",
                    "短视频语音转字幕",
                    "短视频没有字幕",
                ),
                (
                    "r07",
                    "fox/video-editor",
                    9000,
                    "视频剪辑和时间线",
                    "剪辑视频",
                    r#"["视频","剪辑"]"#,
                    "edit long videos",
                    "app",
                    "剪辑视频片段",
                    "视频太长需要剪",
                ),
                (
                    "r08",
                    "fox/ppt-doc",
                    2700,
                    "用来做 PPT 和 Word 文档",
                    "文档生成",
                    r#"["ppt","文档","word"]"#,
                    "slides",
                    "app",
                    "生成 PPT 文档",
                    "要做一份 PPT",
                ),
                (
                    "r09",
                    "fox/excel-helper",
                    1200,
                    "Excel 表格公式助手",
                    "表格",
                    r#"["excel","表格"]"#,
                    "spreadsheets",
                    "skill",
                    "处理 Excel 表格",
                    "Excel 公式不会写",
                ),
                (
                    "r10",
                    "fox/ecom-main-image",
                    3900,
                    "生成电商主图",
                    "主图",
                    r#"["电商","主图"]"#,
                    "product main image",
                    "app",
                    "生成电商主图",
                    "电商主图不好看",
                ),
                (
                    "r11",
                    "fox/detail-layout",
                    900,
                    "商品详情页排版",
                    "详情页",
                    r#"["详情页"]"#,
                    "detail page",
                    "library",
                    "排版商品详情页",
                    "详情页太乱",
                ),
            ];
            for sample in samples {
                let card = card_json(sample.7, &[sample.8], &[sample.9]);
                insert_repo(
                    conn,
                    &NewRepo {
                        id: sample.0,
                        full_name: sample.1,
                        description: sample.6,
                        stars: sample.2,
                        status: "active",
                        one_liner: Some(sample.3),
                        summary: sample.4,
                        keywords_json: sample.5,
                        card_json: Some(&card),
                    },
                );
            }
            insert_repo(
                conn,
                &NewRepo {
                    id: "r12",
                    full_name: "fox/xhs-cover-pending",
                    description: "小红书封面素材包，还没写理解卡",
                    stars: 50,
                    status: "active",
                    one_liner: None,
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            link_scenario(conn, "r01", "sc-xhs", "confirmed");
            link_scenario(conn, "r02", "sc-xhs", "confirmed");
            link_scenario(conn, "r03", "sc-xhs", "rejected");
            link_scenario(conn, "r04", "sc-wx", "confirmed");
            link_scenario(conn, "r05", "sc-wx", "confirmed");
            link_scenario(conn, "r06", "sc-video", "confirmed");
            link_scenario(conn, "r08", "sc-doc", "confirmed");
            link_scenario(conn, "r10", "sc-ec", "confirmed");
            link_scenario(conn, "r11", "sc-ec", "suggested");
        });

        let queries = [
            "做小红书封面",
            "给公众号文章配图",
            "短视频加字幕",
            "做 PPT 和文档",
            "做电商详情页图片",
        ];
        let mut summary = String::new();
        let mut results = Vec::new();
        for query in queries {
            let result = run(&db, query);
            summary.push_str(&brief(query, &result));
            summary.push_str("\n\n");
            results.push(result);
        }
        let _ = fs::write("/tmp/fsl-08-solve-five.txt", &summary);

        let q1 = &results[0];
        assert_eq!(
            q1.intents,
            vec!["小红书".to_owned(), "封面".to_owned()],
            "{summary}"
        );
        assert!(q1.extras.is_empty());
        assert_eq!(
            q1.hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["r01", "r03", "r02"]
        );
        assert!(q1.hits[0].full_match);
        assert_eq!(q1.hits[0].why, "想快速做小红书封面");
        assert_eq!(q1.hits[0].form.as_deref(), Some("app"));
        assert_eq!(q1.hits[0].one_liner.as_deref(), Some("帮你做小红书封面"));
        assert!(q1.hits[0].has_card);
        assert_eq!(q1.hits[0].stars_count, 5200);
        assert_eq!(q1.hits[0].scenario_ids, vec!["sc-xhs".to_owned()]);
        assert_eq!(q1.hits[0].html_url, "https://github.com/fox/xhs-cover");
        assert_eq!(q1.full_match_count, 1);
        assert!(q1.gap_intents.is_empty());
        assert_eq!(q1.preselected, vec!["r01".to_owned()]);
        assert_eq!(q1.pending_matches, 1);
        assert_eq!(q1.suggested_scenario_id.as_deref(), Some("sc-xhs"));
        assert_eq!(q1.suggested_scenario_count, Some(2));
        assert!(!q1.hits.iter().any(|hit| hit.id == "r12"));

        let q2 = &results[1];
        assert_eq!(
            q2.intents,
            vec!["公众号".to_owned(), "配图".to_owned(), "文章".to_owned()]
        );
        assert_eq!(q2.hits[0].id, "r04");
        assert!(q2.hits[0].full_match);
        assert_eq!(q2.full_match_count, 1);
        assert_eq!(q2.preselected, vec!["r04".to_owned()]);
        assert_eq!(q2.suggested_scenario_id.as_deref(), Some("sc-wx"));
        assert_eq!(q2.suggested_scenario_count, Some(2));
        assert_eq!(q2.pending_matches, 0);

        let q3 = &results[2];
        assert_eq!(q3.intents, vec!["短视频".to_owned(), "字幕".to_owned()]);
        assert!(!q3.intents.iter().any(|word| word == "视频"));
        assert_eq!(q3.hits.len(), 1);
        assert_eq!(q3.hits[0].id, "r06");
        assert!(q3.hits[0].full_match);
        assert_eq!(q3.preselected, vec!["r06".to_owned()]);
        assert_eq!(q3.suggested_scenario_id.as_deref(), Some("sc-video"));
        assert_eq!(q3.suggested_scenario_count, Some(1));

        let q4 = &results[3];
        assert_eq!(q4.intents, vec!["ppt".to_owned(), "文档".to_owned()]);
        assert_eq!(q4.hits.len(), 1);
        assert_eq!(q4.hits[0].id, "r08");
        assert!(q4.hits[0].full_match);
        assert_eq!(q4.preselected, vec!["r08".to_owned()]);
        assert_eq!(q4.suggested_scenario_id.as_deref(), Some("sc-doc"));
        assert_eq!(q4.suggested_scenario_count, Some(1));

        let q5 = &results[4];
        assert_eq!(
            q5.intents,
            vec!["详情页".to_owned(), "电商".to_owned(), "图片".to_owned()]
        );
        assert_eq!(q5.full_match_count, 0);
        assert!(q5.preselected.is_empty());
        assert_eq!(q5.gap_intents, vec!["图片".to_owned()]);
        assert!(q5.hits.iter().all(|hit| !hit.full_match));
        assert_eq!(q5.suggested_scenario_id.as_deref(), Some("sc-ec"));
        assert_eq!(q5.suggested_scenario_count, Some(2));
        assert_eq!(q5.pending_matches, 0);
        for result in &results {
            assert!(result.hits.len() <= 8);
        }
    }

    fn brief(title: &str, result: &SolveResult) -> String {
        let mut lines = vec![format!("## {title}")];
        lines.push(format!("intents: {}", list_words(&result.intents)));
        lines.push(format!("extras: {}", list_words(&result.extras)));
        lines.push(format!("fullMatchCount: {}", result.full_match_count));
        lines.push(format!("gapIntents: {}", list_words(&result.gap_intents)));
        lines.push(format!("preselected: {}", list_words(&result.preselected)));
        lines.push(format!("pendingMatches: {}", result.pending_matches));
        lines.push(format!(
            "suggestedScenarioId: {}",
            result
                .suggested_scenario_id
                .clone()
                .unwrap_or_else(|| "null".to_owned())
        ));
        lines.push(format!(
            "suggestedScenarioCount: {}",
            result
                .suggested_scenario_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "null".to_owned())
        ));
        if result.hits.is_empty() {
            lines.push("（没有命中）".to_owned());
        }
        for (index, hit) in result.hits.iter().take(3).enumerate() {
            lines.push(format!(
                "{}. {} score={:.4} stars={} full={} matched={} missing={} why={}",
                index + 1,
                hit.full_name,
                hit.score,
                hit.stars_count,
                hit.full_match,
                list_words(&hit.matched),
                list_words(&hit.missing),
                hit.why
            ));
        }
        lines.join("\n")
    }

    fn list_words(items: &[String]) -> String {
        if items.is_empty() {
            "（无）".to_owned()
        } else {
            items.join("、")
        }
    }

    #[test]
    fn hits_capped_at_eight() {
        let db = prepare("cap", |conn| {
            for index in 0..10 {
                let id = format!("r{index:02}");
                let full_name = format!("lab/cap-{index:02}");
                insert_repo(
                    conn,
                    &NewRepo {
                        id: &id,
                        full_name: &full_name,
                        description: "",
                        stars: 100 - index as i64,
                        status: "active",
                        one_liner: Some("这是封面"),
                        summary: "",
                        keywords_json: "[\"封面\"]",
                        card_json: None,
                    },
                );
            }
        });
        let result = run(&db, "封面");
        assert_eq!(result.hits.len(), 8);
        assert_eq!(result.full_match_count, 8);
        assert_eq!(
            result.preselected,
            vec!["r00".to_owned(), "r01".to_owned(), "r02".to_owned()]
        );
        assert_eq!(result.hits[0].id, "r00");
        assert_eq!(result.hits[7].id, "r07");
        assert!(!result
            .hits
            .iter()
            .any(|hit| hit.id == "r08" || hit.id == "r09"));
    }

    #[test]
    fn pending_matches_need_one_field_to_hold_every_intent() {
        let db = prepare("pending", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "desc",
                    full_name: "lab/desc-both",
                    description: "小红书封面素材",
                    stars: 1,
                    status: "active",
                    one_liner: None,
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "name",
                    full_name: "lab/小红书封面",
                    description: "",
                    stars: 1,
                    status: "active",
                    one_liner: None,
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "split",
                    full_name: "lab/小红书",
                    description: "只有封面 zzqq",
                    stars: 1,
                    status: "active",
                    one_liner: None,
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "card",
                    full_name: "lab/card",
                    description: "小红书封面工具",
                    stars: 3,
                    status: "active",
                    one_liner: Some("小红书封面工具"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "gone",
                    full_name: "lab/removed-cover",
                    description: "小红书封面",
                    stars: 9,
                    status: "removed",
                    one_liner: None,
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "做小红书封面");
        assert_eq!(result.pending_matches, 2);
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].id, "card");
        assert!(!result
            .hits
            .iter()
            .any(|hit| hit.id == "desc" || hit.id == "name" || hit.id == "split"));
        let extra_only = run(&db, "zzqq");
        assert!(extra_only.intents.is_empty());
        assert_eq!(extra_only.pending_matches, 0);
    }

    #[test]
    fn why_follows_problem_capability_one_liner_and_truncates() {
        let long = format!("封面{}", "测".repeat(79));
        let exact = format!("封面{}", "测".repeat(78));
        assert_eq!(long.chars().count(), 81);
        assert_eq!(exact.chars().count(), 80);
        let long_card = card_json("app", &["小红书助手"], &[&long]);
        let exact_card = card_json("app", &[], &[&exact]);
        let order_card = card_json("app", &["小红书助手"], &["无关的说明", "这里有封面"]);
        let fallback_card = card_json("app", &["第一条能力"], &["第一条问题"]);
        let db = prepare("why", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "order",
                    full_name: "lab/order",
                    description: "",
                    stars: 1,
                    status: "active",
                    one_liner: Some("一句话"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: Some(&order_card),
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "fallback",
                    full_name: "lab/fallback",
                    description: "",
                    stars: 1,
                    status: "active",
                    one_liner: Some("一句话说明"),
                    summary: "",
                    keywords_json: "[\"封面\"]",
                    card_json: Some(&fallback_card),
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "long",
                    full_name: "lab/long",
                    description: "",
                    stars: 1,
                    status: "active",
                    one_liner: Some("够用"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: Some(&long_card),
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "exact",
                    full_name: "lab/exact",
                    description: "",
                    stars: 1,
                    status: "active",
                    one_liner: Some("够用"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: Some(&exact_card),
                },
            );
        });
        let result = run(&db, "封面");
        let find = |id: &str| {
            result
                .hits
                .iter()
                .find(|hit| hit.id == id)
                .unwrap_or_else(|| panic!("缺少 {id}"))
        };
        assert_eq!(find("order").why, "这里有封面");
        assert_eq!(find("fallback").why, "第一条问题");
        let truncated = &find("long").why;
        assert_eq!(truncated.chars().count(), 81);
        assert!(truncated.ends_with('…'));
        assert_eq!(
            truncated.chars().take(80).collect::<String>(),
            long.chars().take(80).collect::<String>()
        );
        assert_eq!(find("exact").why, exact);
    }

    #[test]
    fn suggested_scenario_skips_archived_and_uses_sort_order() {
        let db = prepare("suggest", |conn| {
            insert_scenario(conn, "old", "old", "封面", "[\"封面\"]", 1, 1);
            insert_scenario(conn, "skip", "skip", "无关", "[\"笔记\"]", 3, 0);
            insert_scenario(conn, "kw", "kw", "运营", "[\"封面设计\"]", 5, 0);
            insert_scenario(conn, "later", "later", "封面专区", "[]", 8, 0);
            for (id, scenario_id, status) in [
                ("a", "kw", "confirmed"),
                ("b", "kw", "suggested"),
                ("c", "kw", "rejected"),
                ("d", "later", "confirmed"),
            ] {
                insert_repo(
                    conn,
                    &NewRepo {
                        id,
                        full_name: &format!("lab/{id}"),
                        description: "",
                        stars: 1,
                        status: "active",
                        one_liner: Some("有封面卡"),
                        summary: "",
                        keywords_json: "[]",
                        card_json: None,
                    },
                );
                if status != "rejected" || id == "c" {
                    link_scenario(conn, id, scenario_id, status);
                }
            }
            link_scenario(conn, "a", "later", "confirmed");
            insert_repo(
                conn,
                &NewRepo {
                    id: "removed",
                    full_name: "lab/removed",
                    description: "",
                    stars: 1,
                    status: "removed",
                    one_liner: Some("有封面卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            link_scenario(conn, "removed", "kw", "confirmed");
        });
        let result = run(&db, "封面");
        assert_eq!(result.suggested_scenario_id.as_deref(), Some("kw"));
        assert_eq!(result.suggested_scenario_count, Some(2));
        let repo_a = result
            .hits
            .iter()
            .find(|hit| hit.id == "a")
            .expect("应有 a");
        assert_eq!(
            repo_a.scenario_ids,
            vec!["kw".to_owned(), "later".to_owned()]
        );
    }

    #[test]
    fn empty_query_returns_chinese_error() {
        let missing = Path::new("/tmp/fsl-solve-does-not-need-a-database.sqlite3");
        for query in ["", "   ", " \n\t"] {
            let error = solve_query(missing, query).expect_err("空白查询应失败");
            assert_eq!(error, "先写一句你想做的事");
        }
    }

    #[test]
    fn stopwords_or_punctuation_return_empty_result() {
        let db = prepare("empty", |_conn| {});
        for query in ["怎么", "工具", "，。/", "  怎么  "] {
            let result = run(&db, query);
            assert!(result.hits.is_empty(), "{query}");
            assert!(result.intents.is_empty(), "{query}");
            assert!(result.extras.is_empty(), "{query}");
            assert!(result.preselected.is_empty());
            assert_eq!(result.full_match_count, 0);
            assert!(result.gap_intents.is_empty());
            assert!(result.suggested_scenario_id.is_none());
            assert!(result.suggested_scenario_count.is_none());
            assert_eq!(result.pending_matches, 0);
            assert_eq!(result.query, query);
        }
    }

    #[test]
    fn bad_card_json_does_not_fail() {
        let partial = r#"{"capabilities":[1, {"name_zh": 2}, {"name_zh": "封面能力"}], "problems_solved":[1, "封面问题"], "form":"app"}"#;
        let wrong = r#"{"capabilities":"nope","problems_solved":{"x":1},"form":12}"#;
        let db = prepare("badjson", |conn| {
            insert_scenario(conn, "bad", "bad", "无关", "{", 1, 0);
            let cases = [
                (
                    "good",
                    "封面正常",
                    "[]",
                    Some(card_json("cli", &["封面能力"], &["封面问题"])),
                ),
                ("empty", "封面空串", "[]", Some(String::new())),
                ("text", "封面坏文本", "not-json", Some("{bad".to_owned())),
                ("types", "封面类型不对", "[]", Some(wrong.to_owned())),
                ("partial", "封面局部", "[]", Some(partial.to_owned())),
                ("null", "封面空值", "[]", Some("null".to_owned())),
                ("array", "封面数组", "[]", Some("[]".to_owned())),
                ("string", "封面字符串", "[]", Some("\"hello\"".to_owned())),
            ];
            for (id, one_liner, keywords, card) in cases {
                let full_name = format!("lab/{id}");
                insert_repo(
                    conn,
                    &NewRepo {
                        id,
                        full_name: &full_name,
                        description: "",
                        stars: 1,
                        status: "active",
                        one_liner: Some(one_liner),
                        summary: "",
                        keywords_json: keywords,
                        card_json: card.as_deref(),
                    },
                );
            }
        });
        let result = run(&db, "封面");
        assert_eq!(result.hits.len(), 8);
        let good = result.hits.iter().find(|hit| hit.id == "good").unwrap();
        assert_eq!(good.form.as_deref(), Some("cli"));
        assert_eq!(good.why, "封面问题");
        let partial_hit = result.hits.iter().find(|hit| hit.id == "partial").unwrap();
        assert_eq!(partial_hit.form.as_deref(), Some("app"));
        assert_eq!(partial_hit.why, "封面问题");
        let types = result.hits.iter().find(|hit| hit.id == "types").unwrap();
        assert!(types.form.is_none());
        assert_eq!(types.why, "封面类型不对");
    }

    #[test]
    fn removed_repositories_do_not_participate() {
        let db = prepare("removed", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "keep",
                    full_name: "lab/封面库",
                    description: "",
                    stars: 10,
                    status: "active",
                    one_liner: Some("有理解卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "pending",
                    full_name: "lab/pending",
                    description: "封面素材",
                    stars: 1,
                    status: "active",
                    one_liner: None,
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "removed",
                    full_name: "lab/封面已移除",
                    description: "封面",
                    stars: 99999,
                    status: "removed",
                    one_liner: Some("有理解卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "gone",
                    full_name: "lab/封面已消失",
                    description: "封面",
                    stars: 88888,
                    status: "gone",
                    one_liner: Some("有理解卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "封面");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].id, "keep");
        assert_eq!(result.hits[0].stars_count, 10);
        assert_eq!(result.pending_matches, 1);
        let weight = (1.0_f64 + 2.0 / 3.0).ln();
        approx(result.hits[0].score, 3.0 * weight);
    }

    #[test]
    fn no_intent_does_not_claim_full_match() {
        let db = prepare("nointent", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "zz",
                    full_name: "lab/zzqq-tool",
                    description: "",
                    stars: 4,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "zzqq");
        assert!(result.intents.is_empty());
        assert_eq!(result.extras, vec!["zzqq".to_owned()]);
        assert_eq!(result.hits.len(), 1);
        assert!(!result.hits[0].full_match);
        assert_eq!(result.full_match_count, 0);
        assert!(result.preselected.is_empty());
        assert_eq!(result.pending_matches, 0);
        assert!(result.suggested_scenario_id.is_none());
        let weight = (1.0_f64 + 1.0 / 2.0).ln();
        approx(result.hits[0].score, 0.3 * 3.0 * weight);
        let value = serde_json::to_value(&result).expect("应能序列化");
        assert!(value.get("suggestedScenarioId").unwrap().is_null());
        assert!(value.get("suggestedScenarioCount").unwrap().is_null());
        assert!(value.get("pendingMatches").unwrap().is_number());
    }

    #[test]
    fn contract_json_keys_and_types() {
        let hit = SolveHit {
            id: "r1".to_owned(),
            full_name: "fox/one".to_owned(),
            html_url: "https://github.com/fox/one".to_owned(),
            stars_count: 12,
            one_liner: Some("一句话".to_owned()),
            form: Some("app".to_owned()),
            scenario_ids: vec!["sc".to_owned()],
            matched: vec!["封面".to_owned()],
            missing: vec!["小红书".to_owned()],
            why: "为什么".to_owned(),
            score: 1.25,
            full_match: false,
            has_card: true,
        };
        let null_hit = SolveHit {
            one_liner: None,
            form: None,
            ..hit.clone()
        };
        let result = SolveResult {
            query: "做封面".to_owned(),
            intents: vec!["封面".to_owned()],
            extras: vec!["zz".to_owned()],
            hits: vec![hit.clone()],
            preselected: vec!["r1".to_owned()],
            full_match_count: 0,
            gap_intents: vec!["小红书".to_owned()],
            suggested_scenario_id: None,
            suggested_scenario_count: None,
            pending_matches: 0,
        };
        let hit_value = serde_json::to_value(&hit).unwrap();
        assert_string_keys(
            &hit_value,
            &[
                "id",
                "fullName",
                "htmlUrl",
                "starsCount",
                "oneLiner",
                "form",
                "scenarioIds",
                "matched",
                "missing",
                "why",
                "score",
                "fullMatch",
                "hasCard",
            ],
        );
        assert!(hit_value.get("id").unwrap().is_string());
        assert!(hit_value.get("fullName").unwrap().is_string());
        assert!(hit_value.get("htmlUrl").unwrap().is_string());
        assert!(hit_value.get("starsCount").unwrap().is_number());
        assert!(hit_value.get("oneLiner").unwrap().is_string());
        assert!(hit_value.get("form").unwrap().is_string());
        assert!(hit_value.get("scenarioIds").unwrap().is_array());
        assert!(hit_value.get("scenarioIds").unwrap().as_array().unwrap()[0].is_string());
        assert!(hit_value.get("matched").unwrap().as_array().unwrap()[0].is_string());
        assert!(hit_value.get("missing").unwrap().as_array().unwrap()[0].is_string());
        assert!(hit_value.get("why").unwrap().is_string());
        assert!(hit_value.get("score").unwrap().is_number());
        assert!(hit_value.get("fullMatch").unwrap().is_boolean());
        assert!(hit_value.get("hasCard").unwrap().is_boolean());
        let null_value = serde_json::to_value(&null_hit).unwrap();
        assert!(null_value.get("oneLiner").unwrap().is_null());
        assert!(null_value.get("form").unwrap().is_null());

        let result_value = serde_json::to_value(&result).unwrap();
        assert_string_keys(
            &result_value,
            &[
                "query",
                "intents",
                "extras",
                "hits",
                "preselected",
                "fullMatchCount",
                "gapIntents",
                "suggestedScenarioId",
                "suggestedScenarioCount",
                "pendingMatches",
            ],
        );
        assert!(result_value.get("query").unwrap().is_string());
        assert!(result_value.get("intents").unwrap().as_array().unwrap()[0].is_string());
        assert!(result_value.get("extras").unwrap().as_array().unwrap()[0].is_string());
        assert!(result_value.get("hits").unwrap().is_array());
        assert!(result_value.get("preselected").unwrap().as_array().unwrap()[0].is_string());
        assert!(result_value.get("fullMatchCount").unwrap().is_number());
        assert!(result_value.get("gapIntents").unwrap().as_array().unwrap()[0].is_string());
        assert!(result_value.get("suggestedScenarioId").unwrap().is_null());
        assert!(result_value
            .get("suggestedScenarioCount")
            .unwrap()
            .is_null());
        assert_eq!(
            result_value.get("pendingMatches").unwrap().as_i64(),
            Some(0)
        );
    }

    fn assert_string_keys(value: &Value, keys: &[&str]) {
        let object = value.as_object().expect("应是对象");
        let mut actual = object.keys().cloned().collect::<Vec<_>>();
        actual.sort();
        let mut expect = keys.iter().map(|key| (*key).to_owned()).collect::<Vec<_>>();
        expect.sort();
        assert_eq!(actual, expect);
    }

    #[test]
    fn solve_v2_is_listed_in_permissions() {
        let permissions = include_str!("../permissions/gsat-commands.toml");
        let manifest = include_str!("../gen/schemas/acl-manifests.json");
        assert!(permissions.contains("\"solve_v2\""));
        assert!(manifest.contains("\"solve_v2\""));
    }

    #[test]
    fn gap_intents_only_look_at_returned_hits() {
        let db = prepare("gap", |conn| {
            for index in 0..8 {
                let id = format!("p{index:02}");
                let full_name = format!("lab/小红书库{index:02}");
                insert_repo(
                    conn,
                    &NewRepo {
                        id: &id,
                        full_name: &full_name,
                        description: "",
                        stars: 1000 - index as i64,
                        status: "active",
                        one_liner: Some("有卡"),
                        summary: "",
                        keywords_json: "[]",
                        card_json: None,
                    },
                );
            }
            insert_repo(
                conn,
                &NewRepo {
                    id: "late",
                    full_name: "lab/late",
                    description: "封面",
                    stars: 1,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "小红书封面");
        assert_eq!(result.hits.len(), 8);
        assert!(result.hits.iter().all(|hit| hit.id != "late"));
        assert!(result
            .hits
            .iter()
            .all(|hit| !hit.matched.iter().any(|word| word == "封面")));
        assert_eq!(result.gap_intents, vec!["封面".to_owned()]);
        assert_eq!(result.full_match_count, 0);
        assert!(result.preselected.is_empty());
    }

    #[test]
    fn three_thousand_repos_solve_within_one_second() {
        let db = prepare("perf", |conn| {
            let tx = conn.transaction().expect("应能开事务");
            insert_scenario(&tx, "sc", "perf", "封面", "[]", 1, 0);
            let card = r#"{"form":"cli","capabilities":[{"name_zh":"生成封面"}],"problems_solved":["需要封面"]}"#;
            for index in 0..3000 {
                let id = format!("p{index:04}");
                let full_name = format!("lab/repo-{index:04}");
                let one_liner = format!("封面工具{index}");
                insert_repo(
                    &tx,
                    &NewRepo {
                        id: &id,
                        full_name: &full_name,
                        description: "描述里也写封面",
                        stars: index as i64,
                        status: "active",
                        one_liner: Some(&one_liner),
                        summary: "摘要",
                        keywords_json: "[\"封面\"]",
                        card_json: Some(card),
                    },
                );
                if index % 2 == 0 {
                    link_scenario(&tx, &id, "sc", "confirmed");
                }
            }
            tx.commit().expect("应能提交夹具");
        });
        let started = Instant::now();
        let result = run(&db, "做小红书封面");
        let elapsed = started.elapsed();
        let _ = fs::write(
            "/tmp/fsl-08-solve-perf.txt",
            format!(
                "{} ms\nhits {}\nfullMatchCount {}\n",
                elapsed.as_millis(),
                result.hits.len(),
                result.full_match_count
            ),
        );
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "3000 个仓库的 solve_query 用了 {} ms",
            elapsed.as_millis()
        );
        assert_eq!(result.hits.len(), 8);
    }

    #[test]
    fn imported_placeholder_cards_are_clean_in_solve() {
        const SUMMARY_AWESOME: &str = "Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。每个模板均为原创、自包含的完整源代码，支持3条命令即可运行。覆盖AI Agents、多智能体团队、MCP、语音AI、RAG、微调等现代AI栈，支持Claude、Gemini、GPT、Llama等多种模型提供商。采用Apache-2.0许可证，可自由商用。";
        const SUMMARY_CHATCUT: &str = "ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件，支持通过 AI 辅助编辑 ChatCut 视频项目，具体功能包括导入媒体、修改时间线、创建动态图形、生成素材、音频转录、添加字幕、导出视频以及验证编辑可见性。";
        const LINE_AWESOME: &str =
            "Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。";
        const LINE_CHATCUT: &str = "ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件…";
        let db = prepare("legacy-card", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "awesome",
                    full_name: "lab/awesome-llm-apps",
                    description: "",
                    stars: 20,
                    status: "active",
                    one_liner: Some(""),
                    summary: SUMMARY_AWESOME,
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "chatcut",
                    full_name: "lab/chatcut-plugin",
                    description: "",
                    stars: 8,
                    status: "active",
                    one_liner: Some(""),
                    summary: SUMMARY_CHATCUT,
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        assert_eq!(
            crate::v2_ops::import_summary_cards(&db.path).expect("应能导入占位卡"),
            2
        );

        let awesome = run(&db, "模板");
        assert!(awesome.intents.is_empty());
        assert_eq!(awesome.hits.len(), 1);
        assert_eq!(awesome.hits[0].id, "awesome");
        assert_eq!(awesome.hits[0].one_liner.as_deref(), Some(LINE_AWESOME));
        assert_eq!(awesome.hits[0].why, LINE_AWESOME);
        assert!(!awesome.hits[0].full_match);
        assert!(awesome.preselected.is_empty());
        assert_eq!(awesome.full_match_count, 0);

        let chatcut = run(&db, "chatcut");
        assert!(chatcut.intents.is_empty());
        assert_eq!(chatcut.hits.len(), 1);
        assert_eq!(chatcut.hits[0].id, "chatcut");
        assert_eq!(chatcut.hits[0].one_liner.as_deref(), Some(LINE_CHATCUT));
        assert_eq!(chatcut.hits[0].why, LINE_CHATCUT);
        assert!(!chatcut.hits[0].full_match);
        assert!(chatcut.preselected.is_empty());

        let junk = run(&db, "xyzzy不存在的东西");
        assert!(
            junk.intents.is_empty(),
            "乱输入不该抽出意图词：{:?}",
            junk.intents
        );
        assert!(
            junk.hits.is_empty(),
            "乱输入不该硬凑结果：{:?}",
            junk.hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(junk.preselected.is_empty());
        assert_eq!(junk.full_match_count, 0);

        for result in [&awesome, &chatcut, &junk] {
            let json = serde_json::to_string(result).expect("结果应能序列化");
            assert_no_legacy_phrases(&json);
        }
        let evidence = format!(
            "query=模板\noneLiner={}\nwhy={}\nfullMatch={}\npreselected={:?}\nfullMatchCount={}\n\nquery=chatcut\noneLiner={}\nwhy={}\nfullMatch={}\npreselected={:?}\n\nquery=xyzzy不存在的东西\nhits={}\npreselected={:?}\nfullMatchCount={}\n",
            awesome.hits[0].one_liner.as_deref().unwrap_or(""),
            awesome.hits[0].why,
            awesome.hits[0].full_match,
            awesome.preselected,
            awesome.full_match_count,
            chatcut.hits[0].one_liner.as_deref().unwrap_or(""),
            chatcut.hits[0].why,
            chatcut.hits[0].full_match,
            chatcut.preselected,
            junk.hits.len(),
            junk.preselected,
            junk.full_match_count,
        );
        fs::write("/tmp/fsl-15-solve.txt", evidence).expect("应能写下核对文本");
    }

    #[test]
    fn no_intent_needs_half_of_six_extra_words() {
        let db = prepare("half6", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "few",
                    full_name: "lab/few",
                    description: "alpha",
                    stars: 1,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "enough",
                    full_name: "lab/enough",
                    description: "alpha bravo charlie",
                    stars: 3,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "alpha bravo charlie delta echo foxtrot");
        assert!(result.intents.is_empty());
        assert_eq!(
            result.extras,
            vec![
                "alpha".to_owned(),
                "bravo".to_owned(),
                "charlie".to_owned(),
                "delta".to_owned(),
                "echo".to_owned(),
                "foxtrot".to_owned(),
            ]
        );
        assert_eq!(
            result
                .hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["enough"]
        );
        assert!(result.hits.iter().all(|hit| !hit.full_match));
        assert!(result.preselected.is_empty());
        assert_eq!(result.full_match_count, 0);
    }

    #[test]
    fn no_intent_odd_extra_count_uses_ceil() {
        let db = prepare("half5", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "two",
                    full_name: "lab/two",
                    description: "alpha bravo",
                    stars: 1,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "three",
                    full_name: "lab/three",
                    description: "alpha bravo charlie",
                    stars: 2,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "alpha bravo charlie delta echo");
        assert!(result.intents.is_empty());
        assert_eq!(result.extras.len(), 5);
        assert_eq!(
            result
                .hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["three"]
        );
        assert!(result.hits.iter().all(|hit| !hit.full_match));
        assert!(result.preselected.is_empty());
        assert_eq!(result.full_match_count, 0);
    }

    #[test]
    fn extra_only_repo_is_excluded_when_intent_is_present() {
        let db = prepare("extra-only", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "extra",
                    full_name: "lab/zzqqonly",
                    description: "",
                    stars: 9,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "intent",
                    full_name: "lab/cover",
                    description: "封面素材",
                    stars: 2,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "封面 zzqqonly");
        assert_eq!(result.intents, vec!["封面".to_owned()]);
        assert_eq!(result.extras, vec!["zzqqonly".to_owned()]);
        assert_eq!(
            result
                .hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["intent"]
        );
        assert!(result.hits[0].full_match);
        assert_eq!(result.preselected, vec!["intent".to_owned()]);
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
}
