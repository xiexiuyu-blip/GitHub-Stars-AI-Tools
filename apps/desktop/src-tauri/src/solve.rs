//! 按意图从收藏里找方案。只读数据库，不写任何表。
//!
//! 打分移植自原型 `template.html` 的 `tokens` / `idf` / `LEX` / `concepts` /
//! `fieldScore` / `search`。`Math.log` 就是自然对数。
//! 和原型不同的地方：场景名只加分、不算「对上」；没有理解卡的仓库也能进结果；
//! 中文意图词认英文同义词；话题参与打分；`why` 写明哪个词对在哪个字段。

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

pub(crate) const BASE_LEXICON: &[&str] = &[
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

/// 任务名词。只并进本文件的 `lexicon()`，不放进 `BASE_LEXICON`（资料库分词也在用那张表）。
const TASK_LEXICON_EXTRA: &[&str] = &[
    "日报",
    "周报",
    "月报",
    "摘要",
    "二维码",
    "表情包",
    "头像",
    "水印",
    "抠图",
    "改图",
    "修图",
    "压缩",
    "尺寸",
    "缩放",
    "裁剪",
    "转写",
    "录音",
    "会议",
    "会议纪要",
    "文字稿",
    "提示词",
    "播客",
    "分镜",
    "剪映",
    "数字人",
    "旁白",
    "录屏",
    "快手",
    "知乎",
    "飞书",
    "钉钉",
    "短剧",
    "剧本",
    "动画",
    "演示文稿",
    "幻灯片",
    "流程图",
    "思维导图",
    "架构图",
    "看板",
    "用户画像",
    "聊天记录",
    "草稿箱",
    "社群",
    "朋友圈",
    "知识图谱",
];

/// 汉字动态规划里，跳过这些字不扣分：它们是连接词或虚词，本身不是要找的任务。
const GLUE_CHARS: &str = "的了和与及或并把被给让从向在就也都还将帮请先再又吧呢吗啊呀成为以个这那一些想要做用来去到可能会是有对";

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
/// README 只看前面这么多字符：再往后多半是更新日志、赞助名单，提到的词不代表这个项目是做什么的。
const README_SCAN_CHARS: i64 = 60_000;
/// 只出现 1 次多半是顺带一提（「也支持小红书」），不算证据。
const README_MIN_HITS: usize = 2;
/// 提到这么多次就算「充分」，再多不再加分。
const README_FULL_HITS: f64 = 16.0;
/// 理解卡 / 简介里已经对上时，README 提到得多再加一点（最多这个倍数 × 词的区分度）。
const README_BOOST: f64 = 1.5;
/// 理解卡 / 简介里都没对上，只有 README 里反复出现：算对上，但最多只给这个倍数（比「描述」那一档略高，比一句话低）。
const README_ONLY: f64 = 1.5;
const ONE_LINER_LIMIT: usize = 120;

/// 中文意图词 → 英文同义词（全小写）。只在内容字段没有直接命中时用来补「对上」。
/// 长度 ≤ 3 的词，以及任务卡点名的 `i18n`，匹配时要词边界。
const SYNONYMS: &[(&str, &[&str])] = &[
    (
        "字幕",
        &["subtitle", "subtitles", "caption", "captions", "srt"],
    ),
    ("封面", &["cover", "thumbnail", "poster"]),
    ("海报", &["poster", "banner"]),
    (
        "配图",
        &[
            "illustration",
            "infographic",
            "image generation",
            "text-to-image",
        ],
    ),
    ("图片", &["image", "images", "picture"]),
    ("视频", &["video", "videos"]),
    (
        "短视频",
        &[
            "short video",
            "short-video",
            "shorts",
            "reels",
            "tiktok",
            "douyin",
        ],
    ),
    (
        "剪辑",
        &["video editing", "video editor", "clip", "cut", "edit video"],
    ),
    (
        "配音",
        &[
            "voiceover",
            "voice-over",
            "tts",
            "text-to-speech",
            "dubbing",
        ],
    ),
    ("口播", &["talking head", "voiceover"]),
    ("小红书", &["xiaohongshu", "xhs", "rednote"]),
    ("抖音", &["douyin", "tiktok"]),
    ("公众号", &["wechat", "weixin", "official account"]),
    ("微信", &["wechat", "weixin"]),
    (
        "电商",
        &["ecommerce", "e-commerce", "shopify", "taobao", "amazon"],
    ),
    ("详情页", &["detail page", "product page", "listing"]),
    ("主图", &["product image", "main image"]),
    ("文案", &["copywriting", "copy writing", "marketing copy"]),
    ("ppt", &["slides", "presentation", "pptx", "powerpoint"]),
    ("文档", &["document", "documents", "docx"]),
    ("翻译", &["translate", "translation", "i18n"]),
    ("采集", &["crawler", "scraper", "scraping", "spider"]),
    ("爬虫", &["crawler", "scraper", "scraping", "spider"]),
    ("总结", &["summarize", "summary"]),
    ("知识库", &["knowledge base", "rag"]),
    ("自动化", &["automation", "workflow"]),
    (
        "压缩",
        &[
            "compress",
            "compression",
            "minify",
            "tinypng",
            "squoosh",
            "imagemagick",
        ],
    ),
    (
        "尺寸",
        &["resize", "resizing", "dimensions", "aspect ratio"],
    ),
    ("缩放", &["resize", "resizing", "scaling"]),
    ("裁剪", &["crop", "cropping"]),
    (
        "改图",
        &[
            "image editing",
            "edit image",
            "photo editing",
            "image editor",
            "retouch",
        ],
    ),
    ("修图", &["retouch", "photo editing", "image editing"]),
    (
        "抠图",
        &[
            "background removal",
            "remove background",
            "matting",
            "rembg",
        ],
    ),
    ("水印", &["watermark", "watermarks"]),
    (
        "日报",
        &[
            "daily report",
            "daily digest",
            "daily summary",
            "daily brief",
        ],
    ),
    (
        "周报",
        &["weekly report", "weekly digest", "weekly summary"],
    ),
    ("月报", &["monthly report", "monthly digest"]),
    (
        "转写",
        &[
            "transcribe",
            "transcription",
            "whisper",
            "speech-to-text",
            "asr",
            "stt",
        ],
    ),
    (
        "录音",
        &["voice memo", "audio recording", "sound recording"],
    ),
    ("会议", &["meeting", "meetings"]),
    ("会议纪要", &["meeting notes", "meeting minutes"]),
    ("文字稿", &["transcript", "transcripts"]),
    ("排版", &["typesetting", "typography"]),
    ("标题", &["headline", "headlines"]),
    ("爆款", &["viral"]),
    ("选题", &["topic ideas", "content ideas"]),
    ("脚本", &["screenplay"]),
    (
        "直播",
        &[
            "live stream",
            "livestream",
            "live streaming",
            "live commerce",
        ],
    ),
    ("私信", &["direct message", "direct messages"]),
    ("涨粉", &["audience growth", "growth hacking"]),
    ("投放", &["ad campaign", "ad placement"]),
    (
        "竞品",
        &["competitor", "competitors", "competitive analysis"],
    ),
    (
        "群聊",
        &["group chat", "group chats", "chat history", "wechat group"],
    ),
    (
        "聊天记录",
        &["chat history", "chat log", "chat logs", "chat records"],
    ),
    ("表格", &["spreadsheet", "xlsx"]),
    ("简历", &["resume"]),
    ("淘宝", &["taobao", "tmall"]),
    ("亚马逊", &["amazon"]),
    ("头像", &["avatar", "profile picture"]),
    ("表情包", &["sticker", "stickers", "meme"]),
    ("播客", &["podcast"]),
    ("分镜", &["storyboard"]),
    ("数字人", &["digital human", "talking avatar", "heygen"]),
    ("录屏", &["screen recording", "screen recorder"]),
    ("思维导图", &["mind map", "mindmap"]),
    ("流程图", &["flowchart", "flow chart", "mermaid"]),
    ("幻灯片", &["slides", "slideshow"]),
    ("演示文稿", &["presentation", "slides", "pptx"]),
    ("二维码", &["qr code", "qrcode"]),
    (
        "提示词",
        &["prompt engineering", "prompt library", "prompt templates"],
    ),
    ("知识图谱", &["knowledge graph"]),
    ("看板", &["kanban"]),
    ("飞书", &["feishu"]),
    ("钉钉", &["dingtalk"]),
    ("知乎", &["zhihu"]),
    ("快手", &["kuaishou"]),
    ("剪映", &["capcut", "jianying"]),
    ("旁白", &["narration", "voiceover"]),
    ("短剧", &["short drama", "micro drama"]),
    ("用户画像", &["user persona", "user profile"]),
    ("架构图", &["architecture diagram"]),
    ("朋友圈", &["wechat moments"]),
];

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
    topics: String,
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
    /// GitHub 描述原文。没有理解卡时，命中的「一句话」取它的第一句。
    github_description: String,
}

struct CardBits {
    problems: Vec<String>,
    capabilities: Vec<String>,
    form: Option<String>,
    use_when: Vec<String>,
    what_it_is: String,
}

struct ScoreParts {
    score: f64,
    matched: Vec<String>,
    missing: Vec<String>,
    /// 对上的意图词全靠 README 里提到（名称、一句话、能力、描述、话题里一处都没对上，同义词也没有）。
    /// 这种命中证据最弱：可以列出来，但不默认勾选。
    readme_only: bool,
}

/// 一个仓库 README 里各意图词出现了几次（含同义词），以及 README 有多长。
struct ReadmeStats {
    /// 千字数，至少 3：很短的 README 不会因为「密度」被抬高。
    kb: f64,
    hits: HashMap<String, usize>,
}

impl ReadmeStats {
    fn hits_of(&self, token: &str) -> usize {
        self.hits.get(token).copied().unwrap_or(0)
    }

    /// 0 到 1：提到得越多、占比越高越接近 1；不足 `README_MIN_HITS` 次是 0。
    fn evidence(&self, token: &str) -> f64 {
        let hits = self.hits_of(token);
        if hits < README_MIN_HITS {
            return 0.0;
        }
        let presence = ((1.0 + hits as f64).ln() / (1.0 + README_FULL_HITS).ln()).min(1.0);
        let density = hits as f64 / self.kb;
        presence * (0.35 + 0.65 * density.min(1.0))
    }
}

impl TextBlocks {
    fn contains_token(&self, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        // 场景块不参与 idf。只挂在场景上的仓库不再抬高文档频率。
        self.name.contains(token)
            || self.one_liner.contains(token)
            || self.capabilities.contains(token)
            || self.problems.contains(token)
            || self.keywords.contains(token)
            || self.summary.contains(token)
            || self.description.contains(token)
            || self.topics.contains(token)
    }
}

pub fn solve_query(db_path: &Path, query: &str) -> Result<SolveResult, String> {
    if query.trim().is_empty() {
        return Err("先写一句你想做的事".to_owned());
    }
    let connection = open_read(db_path)?;
    let scenarios = load_scenarios(&connection)?;
    let lexicon = lexicon(&scenarios);
    // 分词要看收藏里出现过哪些词，所以先读仓库。没有任何概念时不再去读 README。
    let repos = load_repos(&connection, &scenarios)?;
    let (main, extra) = concepts(query, &lexicon, &repos);
    if main.is_empty() && extra.is_empty() {
        return Ok(empty_result(query));
    }

    let readme = load_readme_stats(&connection, &main)?;
    let mut cache = HashMap::new();
    let mut scored = Vec::new();
    for (index, repo) in repos.iter().enumerate() {
        if let Some(parts) = score_repo(
            repo,
            &main,
            &extra,
            &repos,
            &mut cache,
            readme.get(&repo.id),
        ) {
            scored.push((index, parts));
        }
    }
    // 同分：有卡的在前，再比星数，最后按 id，结果稳定。
    scored.sort_by(|left, right| {
        right
            .1
            .score
            .total_cmp(&left.1.score)
            .then_with(|| repos[right.0].has_card.cmp(&repos[left.0].has_card))
            .then_with(|| repos[right.0].stars.cmp(&repos[left.0].stars))
            .then_with(|| repos[left.0].id.cmp(&repos[right.0].id))
    });

    let mut terms = main.clone();
    terms.extend(extra.iter().cloned());
    let mut hits = Vec::new();
    // 和 hits 一一对应：这条命中是不是只靠 README 里提到（见 `ScoreParts::readme_only`）。
    let mut readme_only = Vec::new();
    for (index, parts) in scored.into_iter().take(HIT_LIMIT) {
        let repo = &repos[index];
        readme_only.push(parts.readme_only);
        // 没有意图词时不知道用户要什么，不能说「全部对上」。
        let full_match = !main.is_empty() && parts.missing.is_empty();
        let why = why_text(repo, &main, &parts.matched, &extra, readme.get(&repo.id));
        hits.push(SolveHit {
            id: repo.id.clone(),
            full_name: repo.full_name.clone(),
            html_url: repo.html_url.clone(),
            stars_count: repo.stars,
            one_liner: hit_one_liner(repo),
            form: repo.form.clone(),
            scenario_ids: repo.scenario_ids.clone(),
            matched: parts.matched,
            missing: parts.missing,
            why,
            score: parts.score,
            full_match,
            has_card: repo.has_card,
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
    // 默认勾选：全都对上、且至少有一个意图词在内容里对上的前 3 条。
    // 只靠 README 里提到几次的命中（「做表情包」「写周报」这类收藏里没有专门工具的问题）不默认勾选，免得
    // 一打开参考包托盘里就是三个不太相关的工具；它们仍在列表里，要用自己勾。
    let preselected = hits
        .iter()
        .zip(&readme_only)
        .filter(|(hit, weak)| hit.full_match && !**weak)
        .take(3)
        .map(|(hit, _)| hit.id.clone())
        .collect::<Vec<_>>();
    let full_match_count = hits.iter().filter(|hit| hit.full_match).count() as i64;
    // 没进前 8、没有卡、且名称或描述单独含全部意图词。两个字段拼在一起不算。
    let hit_ids = hits
        .iter()
        .map(|hit| hit.id.as_str())
        .collect::<HashSet<_>>();
    let pending_matches = if main.is_empty() {
        0
    } else {
        repos
            .iter()
            .filter(|repo| {
                !repo.has_card
                    && !hit_ids.contains(repo.id.as_str())
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
                    IFNULL(repo_ai_documents.card_json, ''),
                    IFNULL(repositories.topics_json, '[]')
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
                row.get::<_, String>(9)?,
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
            topics_json,
        ) = row.map_err(|error| format!("找方案仓库行读取失败：{error}"))?;
        let parsed = read_card_json(&card_json);
        let shown = crate::card_view::display_one_liner(Some(one_liner.as_str()), &parsed)
            .unwrap_or_default();
        let card = parse_card(&parsed);
        let keywords = parse_json_strings(&keywords_json);
        let topics = parse_json_strings(&topics_json);
        let links = grouped.get(&id).cloned().unwrap_or_default();
        let scenario_ids = links
            .iter()
            .map(|(_, scenario_id)| scenario_id.clone())
            .collect::<Vec<_>>();
        let mut scenario_bits = Vec::new();
        for scenario_id in &scenario_ids {
            if let Some(item) = by_id.get(scenario_id.as_str()) {
                scenario_bits.push(item.name_zh.clone());
                scenario_bits.extend(item.keywords.iter().cloned());
            }
        }
        let has_card = !one_liner.trim().is_empty();
        let mut problems = card.problems.clone();
        for item in &card.use_when {
            if !problems.iter().any(|existing| existing == item) {
                problems.push(item.clone());
            }
        }
        repos.push(RepoRecord {
            id,
            html_url,
            stars,
            problems: problems.clone(),
            capabilities: card.capabilities.clone(),
            form: card
                .form
                .filter(|form| !crate::repo_facts::is_unspecific_form(form))
                .or_else(|| {
                    crate::repo_facts::infer_form(&topics, &full_name, Some(description.as_str()))
                        .map(str::to_owned)
                }),
            scenario_ids,
            has_card,
            github_description: description.clone(),
            blocks: TextBlocks {
                name: full_name.to_lowercase(),
                one_liner: shown.to_lowercase(),
                capabilities: join_lower(&card.capabilities),
                problems: join_lower(&problems),
                keywords: join_lower(&keywords),
                scenarios: join_lower(&scenario_bits),
                summary: format!(
                    "{} {}",
                    summary.to_lowercase(),
                    card.what_it_is.to_lowercase()
                ),
                description: description.to_lowercase(),
                topics: join_lower(&topics),
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
    readme: Option<&ReadmeStats>,
) -> Option<ScoreParts> {
    let mut score = 0.0;
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut content_evidence = false;
    for intent in main {
        let weight = cached_idf(cache, intent, repos);
        let content = field_score(&repo.blocks, intent, weight);
        let evidence = readme.map_or(0.0, |stats| stats.evidence(intent));
        // 内容里没有中文词时，英文同义词按「描述」那一档（1.0 × idf）算对上。
        // README 里反复提到：已经对上的再加一点；只有 README 提到的也算对上，但分很低（补召回，不抢排名）。
        let gained = if content > 0.0 {
            content_evidence = true;
            content + README_BOOST * evidence * weight
        } else if synonym_matches(&repo.blocks, intent) {
            content_evidence = true;
            weight + README_BOOST * evidence * weight
        } else if evidence > 0.0 {
            README_ONLY * evidence * weight
        } else {
            0.0
        };
        if gained > 0.0 {
            matched.push(intent.clone());
            // 内容已经对上之后，场景名或场景关键词再含这个词，只加一小档。
            if repo.blocks.scenarios.contains(intent) {
                score += 0.5 * weight;
            }
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
        let readme_only = !matched.is_empty() && !content_evidence;
        Some(ScoreParts {
            score,
            matched,
            missing,
            readme_only,
        })
    } else {
        None
    }
}

/// 读出每个仓库 README 开头一段，数一数各意图词（和它的英文同义词）出现的次数。没有意图词时不读。
fn load_readme_stats(
    connection: &Connection,
    intents: &[String],
) -> Result<HashMap<String, ReadmeStats>, String> {
    let mut stats = HashMap::new();
    if intents.is_empty() {
        return Ok(stats);
    }
    let mut statement = connection
        .prepare(
            "SELECT repo_readmes.repo_id, substr(repo_readmes.raw_markdown, 1, ?1)
             FROM repo_readmes
             JOIN repositories ON repositories.id = repo_readmes.repo_id
             WHERE repositories.sync_status = 'active'",
        )
        .map_err(|error| format!("找方案 README 读取失败：{error}"))?;
    let rows = statement
        .query_map([README_SCAN_CHARS], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|error| format!("找方案 README 查询失败：{error}"))?;
    for row in rows {
        let (repo_id, text) = row.map_err(|error| format!("找方案 README 行读取失败：{error}"))?;
        let Some(text) = text else { continue };
        if text.trim().is_empty() {
            continue;
        }
        let lower = text.to_lowercase();
        let mut hits = HashMap::new();
        for intent in intents {
            let count = count_mentions(&lower, intent);
            if count > 0 {
                hits.insert(intent.clone(), count);
            }
        }
        if hits.is_empty() {
            continue;
        }
        let kb = (lower.chars().count() as f64 / 1000.0).max(3.0);
        stats.insert(repo_id, ReadmeStats { kb, hits });
    }
    Ok(stats)
}

/// 意图词本身加上它的英文同义词在文本里一共出现几次（文本已转小写）。
/// 短同义词要整词命中（`cut` 不能命中 `execute`），和 `field_has_synonym` 同一个规矩。
fn count_mentions(lower: &str, intent: &str) -> usize {
    let mut total = count_plain(lower, intent);
    if let Some(words) = synonym_list(intent) {
        for word in words {
            total += if synonym_needs_boundary(word) {
                count_whole_word(lower, word)
            } else {
                count_plain(lower, word)
            };
        }
    }
    total
}

fn count_plain(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// 数整词出现的次数：前后不能紧贴英文字母或数字（汉字不算字母，可以紧贴）。
fn count_whole_word(haystack: &str, word: &str) -> usize {
    if word.is_empty() {
        return 0;
    }
    let bytes = haystack.as_bytes();
    haystack
        .match_indices(word)
        .filter(|(start, _)| {
            let before_ok = *start == 0 || !bytes[*start - 1].is_ascii_alphanumeric();
            let end = start + word.len();
            let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
            before_ok && after_ok
        })
        .count()
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
    if blocks.summary.contains(token) {
        max = max.max(1.5 * idf_weight);
    }
    // 话题和 GitHub 描述同一档。场景名不在这里计分。
    if blocks.description.contains(token) || blocks.topics.contains(token) {
        max = max.max(idf_weight);
    }
    max
}

fn synonym_list(intent: &str) -> Option<&'static [&'static str]> {
    SYNONYMS
        .iter()
        .find(|(word, _)| *word == intent)
        .map(|(_, words)| *words)
}

fn synonym_needs_boundary(word: &str) -> bool {
    // `i18n` 有 4 个字符，任务卡仍要求词边界，避免命中 i18next。
    word.chars().count() <= 3 || word == "i18n"
}

fn synonym_matches(blocks: &TextBlocks, intent: &str) -> bool {
    let Some(words) = synonym_list(intent) else {
        return false;
    };
    let fields = [
        blocks.name.as_str(),
        blocks.description.as_str(),
        blocks.topics.as_str(),
        blocks.summary.as_str(),
        blocks.keywords.as_str(),
    ];
    words
        .iter()
        .any(|word| fields.iter().any(|field| field_has_synonym(field, word)))
}

fn field_has_synonym(field: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    if synonym_needs_boundary(word) {
        contains_word(field, word)
    } else {
        field.contains(word)
    }
}

/// 短同义词要整词命中：`cut` 不能命中 `execute`。汉字不算字母，可以紧贴英文词。
fn contains_word(haystack: &str, word: &str) -> bool {
    if word.is_empty() || haystack.is_empty() {
        return false;
    }
    let word_chars: Vec<char> = word.chars().collect();
    let hay: Vec<char> = haystack.chars().collect();
    let width = word_chars.len();
    if width > hay.len() {
        return false;
    }
    for start in 0..=hay.len() - width {
        if hay[start..start + width] != word_chars[..] {
            continue;
        }
        let before_ok = start == 0 || !is_latin_word_char(hay[start - 1]);
        let after = start + width;
        let after_ok = after == hay.len() || !is_latin_word_char(hay[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_latin_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

fn lexicon(scenarios: &[ScenarioRecord]) -> Vec<String> {
    let mut words = Vec::new();
    let mut seen = HashSet::new();
    for word in BASE_LEXICON.iter().chain(TASK_LEXICON_EXTRA.iter()) {
        push_lexicon_word(&mut words, &mut seen, word);
    }
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

enum QueryPiece {
    Latin(String),
    Han(Vec<char>),
}

struct GramStat {
    repos: usize,
    total: usize,
    left: HashMap<char, usize>,
    right: HashMap<char, usize>,
}

fn concepts(query: &str, lexicon: &[String], repos: &[RepoRecord]) -> (Vec<String>, Vec<String>) {
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

    let mut pieces = Vec::new();
    for segment in split_token_segments(&rest) {
        push_query_pieces(&segment, &mut pieces);
    }
    let han_runs = pieces
        .iter()
        .filter_map(|piece| match piece {
            QueryPiece::Han(chars) => Some(chars.as_slice()),
            QueryPiece::Latin(_) => None,
        })
        .collect::<Vec<_>>();
    let weights = candidate_weights(&han_runs, repos);
    let main_set = main.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut extra = Vec::new();
    let mut seen = HashSet::new();
    for piece in &pieces {
        let words = match piece {
            QueryPiece::Latin(text) => tokens(text),
            QueryPiece::Han(chars) => segment_han(chars, &weights),
        };
        for word in words {
            if word.chars().count() < 2 || is_stop(&word) || main_set.contains(word.as_str()) {
                continue;
            }
            if seen.insert(word.clone()) {
                extra.push(word);
            }
        }
    }
    (main, extra)
}

fn push_query_pieces(segment: &str, pieces: &mut Vec<QueryPiece>) {
    let mut latin = String::new();
    let mut han = Vec::new();
    for ch in segment.chars() {
        if is_han(ch) {
            if !latin.is_empty() {
                pieces.push(QueryPiece::Latin(std::mem::take(&mut latin)));
            }
            han.push(ch);
        } else {
            if !han.is_empty() {
                pieces.push(QueryPiece::Han(std::mem::take(&mut han)));
            }
            latin.push(ch);
        }
    }
    if !latin.is_empty() {
        pieces.push(QueryPiece::Latin(latin));
    }
    if !han.is_empty() {
        pieces.push(QueryPiece::Han(han));
    }
}

/// 只抽出英文和数字。汉字不再两两硬切，交给 `segment_han`。
fn tokens(query: &str) -> Vec<String> {
    let mut raw = Vec::new();
    for segment in split_token_segments(&query.to_lowercase()) {
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

fn candidate_weights(runs: &[&[char]], repos: &[RepoRecord]) -> HashMap<String, f64> {
    let mut grams = Vec::new();
    for run in runs {
        let len = run.len();
        for width in [2usize, 3] {
            if len < width {
                continue;
            }
            for start in 0..=len - width {
                let word: String = run[start..start + width].iter().collect();
                if is_stop(&word) {
                    continue;
                }
                grams.push(word);
            }
        }
    }
    grams.sort();
    grams.dedup();
    if grams.is_empty() || repos.is_empty() {
        return HashMap::new();
    }

    let mut stats = grams
        .iter()
        .map(|_| GramStat {
            repos: 0,
            total: 0,
            left: HashMap::new(),
            right: HashMap::new(),
        })
        .collect::<Vec<_>>();
    for repo in repos {
        let fields = [
            repo.blocks.name.as_str(),
            repo.blocks.one_liner.as_str(),
            repo.blocks.capabilities.as_str(),
            repo.blocks.problems.as_str(),
            repo.blocks.keywords.as_str(),
            repo.blocks.summary.as_str(),
            repo.blocks.description.as_str(),
            repo.blocks.topics.as_str(),
        ];
        let mut hit = vec![false; grams.len()];
        for field in fields {
            if field.is_empty() {
                continue;
            }
            for (index, word) in grams.iter().enumerate() {
                if field.contains(word.as_str()) {
                    hit[index] = true;
                    add_occurrences(field, word, &mut stats[index]);
                }
            }
        }
        for (index, found) in hit.iter().enumerate() {
            if *found {
                stats[index].repos += 1;
            }
        }
    }

    let mut kept = HashMap::new();
    for (word, stat) in grams.into_iter().zip(stats) {
        if stat.repos == 0 || glued_into_longer_word(&stat) {
            continue;
        }
        kept.insert(word, stat.repos as f64);
    }
    kept
}

fn add_occurrences(text: &str, word: &str, stat: &mut GramStat) {
    if word.is_empty() {
        return;
    }
    let mut start = 0;
    while start < text.len() {
        let Some(rel) = text[start..].find(word) else {
            break;
        };
        let at = start + rel;
        stat.total += 1;
        if at > 0 {
            if let Some(ch) = text[..at].chars().next_back() {
                if is_han(ch) {
                    *stat.left.entry(ch).or_insert(0) += 1;
                }
            }
        }
        let end = at + word.len();
        if end < text.len() {
            if let Some(ch) = text[end..].chars().next() {
                if is_han(ch) {
                    *stat.right.entry(ch).or_insert(0) += 1;
                }
            }
        }
        if end <= start {
            break;
        }
        start = end;
    }
}

/// 出现至少 3 次，且左边或右边最常见的那个汉字占了 75% 以上：这个词几乎总是更长词的一截。
fn glued_into_longer_word(stat: &GramStat) -> bool {
    if stat.total < 3 {
        return false;
    }
    let limit = stat.total * 3;
    let left = stat.left.values().copied().max().unwrap_or(0);
    let right = stat.right.values().copied().max().unwrap_or(0);
    left * 4 > limit || right * 4 > limit
}

fn segment_han(chars: &[char], weights: &HashMap<String, f64>) -> Vec<String> {
    let len = chars.len();
    if len == 0 {
        return Vec::new();
    }
    let mut best = vec![0.0; len + 1];
    let mut prev = vec![0usize; len + 1];
    let mut took = vec![false; len + 1];
    for index in 1..=len {
        let penalty = if is_glue(chars[index - 1]) { 0.0 } else { -0.6 };
        let mut score = best[index - 1] + penalty;
        let mut from = index - 1;
        let mut take = false;
        for width in [2usize, 3] {
            if index < width {
                continue;
            }
            let word: String = chars[index - width..index].iter().collect();
            let Some(df) = weights.get(&word) else {
                continue;
            };
            let gained = best[index - width] + word_gain(*df);
            if gained > score {
                score = gained;
                from = index - width;
                take = true;
            }
        }
        best[index] = score;
        prev[index] = from;
        took[index] = take;
    }

    let mut words = Vec::new();
    let mut index = len;
    while index > 0 {
        if took[index] {
            words.push(chars[prev[index]..index].iter().collect::<String>());
        }
        let next = prev[index];
        if next >= index {
            break;
        }
        index = next;
    }
    words.reverse();
    words
}

fn word_gain(df: f64) -> f64 {
    1.0 + 0.15 * (1.0 + df).ln().min(4.0)
}

fn is_glue(ch: char) -> bool {
    GLUE_CHARS.contains(ch)
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

fn why_text(
    repo: &RepoRecord,
    main: &[String],
    matched: &[String],
    extra: &[String],
    readme: Option<&ReadmeStats>,
) -> String {
    if main.is_empty() {
        return why_text_legacy(repo, extra);
    }
    let parts = matched
        .iter()
        .map(|intent| {
            let hits = readme.map_or(0, |stats| stats.hits_of(intent));
            format!("{intent}（{}）", match_label(repo, intent, hits))
        })
        .collect::<Vec<_>>();
    join_why_parts(&parts)
}

/// 没有意图词、只有额外词时，沿用原来的取句：问题、能力、一句话里第一处含这些词的原文。
fn why_text_legacy(repo: &RepoRecord, terms: &[String]) -> String {
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

/// 说明这个词在哪里对上的。`readme_hits` 是 README 里提到的次数：只在 README 里对上就写「项目说明里提到 N 次」；
/// 别处已经对上、README 里也提到得多（5 次以上）时，在后面补一句「说明里提到 N 次」（给不看代码的人读，不写 README 这个词）。
fn match_label(repo: &RepoRecord, intent: &str, readme_hits: usize) -> String {
    match field_label(repo, intent) {
        Some(label) if readme_hits >= 5 => format!("{label}·说明里提到 {readme_hits} 次"),
        Some(label) => label,
        None if readme_hits >= README_MIN_HITS => format!("项目说明里提到 {readme_hits} 次"),
        None => "描述".to_owned(),
    }
}

fn field_label(repo: &RepoRecord, intent: &str) -> Option<String> {
    if repo.blocks.name.contains(intent) {
        return Some("名称".to_owned());
    }
    if let Some(item) = find_containing(&repo.capabilities, intent) {
        return Some(format!("能力：{}", snippet20(item)));
    }
    if let Some(item) = find_containing(&repo.problems, intent) {
        return Some(format!("问题：{}", snippet20(item)));
    }
    if repo.blocks.one_liner.contains(intent) {
        return Some("一句话".to_owned());
    }
    if repo.blocks.keywords.contains(intent) {
        return Some("关键词".to_owned());
    }
    if repo.blocks.summary.contains(intent) {
        return Some("摘要".to_owned());
    }
    if repo.blocks.description.contains(intent) {
        return Some("描述".to_owned());
    }
    if repo.blocks.topics.contains(intent) {
        return Some("话题".to_owned());
    }
    synonym_label(repo, intent)
}

fn find_containing<'a>(items: &'a [String], intent: &str) -> Option<&'a str> {
    items
        .iter()
        .find(|item| item.to_lowercase().contains(intent))
        .map(String::as_str)
}

fn snippet20(text: &str) -> String {
    text.chars().take(20).collect()
}

fn synonym_label(repo: &RepoRecord, intent: &str) -> Option<String> {
    let words = synonym_list(intent)?;
    let fields = [
        (repo.blocks.name.as_str(), "名称"),
        (repo.blocks.description.as_str(), "描述"),
        (repo.blocks.keywords.as_str(), "关键词"),
        (repo.blocks.summary.as_str(), "摘要"),
        (repo.blocks.topics.as_str(), "话题"),
    ];
    for (field, label) in fields {
        if words.iter().any(|word| field_has_synonym(field, word)) {
            return Some(label.to_owned());
        }
    }
    None
}

fn join_why_parts(parts: &[String]) -> String {
    if parts.is_empty() {
        return "对上：".to_owned();
    }
    let mut kept: Vec<&str> = Vec::new();
    for part in parts {
        kept.push(part.as_str());
        if format_why(&kept).chars().count() > WHY_LIMIT {
            kept.pop();
            break;
        }
    }
    if kept.is_empty() {
        return shrink_one_why_part(&parts[0]);
    }
    format_why(&kept)
}

fn format_why(parts: &[&str]) -> String {
    format!("对上：{}", parts.join("、"))
}

/// 单条说明本身超过 80 字时，丢掉能力/问题附注，保留完整意图词，不从词中间切开。
fn shrink_one_why_part(part: &str) -> String {
    let full = format!("对上：{part}");
    if full.chars().count() <= WHY_LIMIT {
        return full;
    }
    if let Some((intent, _)) = part.split_once('（') {
        let short = format!("对上：{intent}");
        if short.chars().count() <= WHY_LIMIT {
            return short;
        }
    }
    full.chars().take(WHY_LIMIT).collect()
}

fn hit_one_liner(repo: &RepoRecord) -> Option<String> {
    if repo.has_card {
        if repo.one_liner.trim().is_empty() {
            None
        } else {
            Some(repo.one_liner.clone())
        }
    } else {
        let line = first_sentence(&repo.github_description);
        if line.is_empty() {
            None
        } else {
            Some(line)
        }
    }
}

/// 折成一行，取第一句。中文句号、叹号、问号，或英文句号后接空白（或结尾）处停。
fn first_sentence(description: &str) -> String {
    let folded = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if folded.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = folded.chars().collect();
    let mut end = chars.len();
    for index in 0..chars.len() {
        let ch = chars[index];
        if matches!(ch, '。' | '！' | '？') {
            end = index + 1;
            break;
        }
        if ch == '.' {
            let at_boundary = match chars.get(index + 1) {
                None => true,
                Some(next) => next.is_whitespace(),
            };
            if at_boundary {
                end = index + 1;
                break;
            }
        }
    }
    let sentence: String = chars[..end].iter().collect();
    truncate_at_clause(&sentence, ONE_LINER_LIMIT)
}

/// 超过上限时，在上限内最后一个逗号、分号、顿号或空格处截断，并加上省略号。
fn truncate_at_clause(text: &str, limit: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= limit {
        return text.to_owned();
    }
    let mut break_at = None;
    let window = limit.min(chars.len());
    for index in 1..window {
        if matches!(chars[index], '，' | '；' | '、' | ' ') {
            break_at = Some(index);
        }
    }
    let cut = break_at.unwrap_or(window);
    let mut out: String = chars[..cut].iter().collect();
    out.push('…');
    out
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
            use_when: Vec::new(),
            what_it_is: String::new(),
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
    let problems = string_list(obj, "problems_solved");
    let use_when = string_list(obj, "use_when");
    let what_it_is = obj
        .get("what_it_is")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
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
        use_when,
        what_it_is,
    }
}

fn string_list(obj: &serde_json::Map<String, Value>, key: &str) -> Vec<String> {
    obj.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
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
            topics: String::new(),
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

    fn set_topics(conn: &Connection, id: &str, topics_json: &str) {
        conn.execute(
            "UPDATE repositories SET topics_json = ?1 WHERE id = ?2",
            params![topics_json, id],
        )
        .unwrap_or_else(|error| panic!("更新话题 {id} 失败：{error}"));
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
        assert_eq!(tokens("做PPT封面"), vec!["ppt".to_owned()]);
        assert_eq!(tokens("做 PPT 封面"), vec!["ppt".to_owned()]);
        assert_eq!(tokens("hello世界"), vec!["hello".to_owned()]);
        assert_eq!(tokens("ab.cd"), vec!["ab.cd".to_owned()]);
        assert!(
            !tokens("做PPT封面").iter().any(|word| word == "做封"),
            "汉字不再两两切"
        );
    }

    #[test]
    fn tokens_drops_han_instead_of_keeping_fragments() {
        assert!(tokens("做").is_empty());
        assert!(tokens("中").is_empty());
        assert!(tokens("封面").is_empty());
    }

    #[test]
    fn tokens_does_not_split_han_into_bigrams() {
        assert!(tokens("小红书").is_empty());
        assert!(tokens("中文的").is_empty());
        assert!(!tokens("小红书")
            .iter()
            .any(|word| word == "小红" || word == "红书"));
    }

    #[test]
    fn tokens_removes_stopwords() {
        assert!(tokens("我想做一个工具").is_empty());
        assert!(tokens("怎么").is_empty());
        assert!(tokens("工具").is_empty());
        assert!(tokens("帮我").is_empty());
    }

    #[test]
    fn tokens_dedups_latin_and_ignores_han_punctuation() {
        assert!(tokens("封面封面").is_empty());
        assert!(tokens("封面,封面，海报。字幕;脚本；标题、笔记/海报").is_empty());
        assert_eq!(tokens("ppt/ppt"), vec!["ppt".to_owned()]);
    }

    #[test]
    fn concepts_splits_xiaohongshu_cover() {
        let (main, extra) = concepts("做小红书封面", &base_lexicon(), &[]);
        assert_eq!(main, vec!["小红书".to_owned(), "封面".to_owned()]);
        assert!(extra.is_empty());
    }

    #[test]
    fn concepts_short_video_subtitle_drops_video() {
        let (main, extra) = concepts("短视频加字幕", &base_lexicon(), &[]);
        assert_eq!(main, vec!["短视频".to_owned(), "字幕".to_owned()]);
        assert!(!main.iter().any(|word| word == "视频"));
        assert!(extra.is_empty());
    }

    #[test]
    fn concepts_prefers_longest_scenario_keyword() {
        let scenarios = vec![scenario("s", "别名", &["小红书封面"], 1, false)];
        let lex = lexicon(&scenarios);
        let (main, extra) = concepts("做小红书封面", &lex, &[]);
        assert_eq!(main, vec!["小红书封面".to_owned()]);
        assert!(extra.is_empty());
    }

    #[test]
    fn concepts_drops_extra_shorter_than_two() {
        let (main, extra) = concepts("封面 ab c 做", &base_lexicon(), &[]);
        assert_eq!(main, vec!["封面".to_owned()]);
        assert_eq!(extra, vec!["ab".to_owned()]);
    }

    #[test]
    fn concepts_ppt_document_is_case_insensitive() {
        let (main, extra) = concepts("做 PPT 和文档", &base_lexicon(), &[]);
        assert_eq!(main, vec!["ppt".to_owned(), "文档".to_owned()]);
        assert!(extra.is_empty());
    }

    #[test]
    fn concepts_punctuation_still_keeps_lexicon_words() {
        let (main, extra) = concepts(
            "封面,封面，海报。字幕;脚本；标题、笔记/海报",
            &base_lexicon(),
            &[],
        );
        for word in ["封面", "海报", "字幕", "脚本", "标题", "笔记"] {
            assert!(main.iter().any(|item| item == word), "{word} {main:?}");
        }
        assert!(extra.is_empty(), "{extra:?}");
        assert!(!extra.iter().any(|word| word == "面封"));
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
            doc("", "", "", "", "设计", "封面", "", ""),
        ];
        let df_cover = docs
            .iter()
            .filter(|item| item.contains_token("封面"))
            .count();
        assert_eq!(
            df_cover, 1,
            "同一仓库的多块命中只算 1 个 df，场景块里的封面不计入"
        );
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
            0.0,
            "场景名单独出现不算内容分"
        );
        let mut topics_only = doc("", "", "", "", "", "", "", "");
        topics_only.topics = "封面".to_owned();
        assert_eq!(field_score(&topics_only, "封面", weight), 2.0);
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
            vec!["r01", "r12", "r02", "r03"]
        );
        assert!(q1.hits[0].full_match);
        assert_eq!(
            q1.hits[0].why,
            "对上：小红书（能力：生成小红书封面）、封面（能力：生成小红书封面）"
        );
        assert_ne!(
            q1.hits[0].why,
            q1.hits[0].one_liner.as_deref().unwrap_or("")
        );
        assert_eq!(q1.hits[0].form.as_deref(), Some("app"));
        assert_eq!(q1.hits[0].one_liner.as_deref(), Some("帮你做小红书封面"));
        assert!(q1.hits[0].has_card);
        assert_eq!(q1.hits[0].stars_count, 5200);
        assert_eq!(q1.hits[0].scenario_ids, vec!["sc-xhs".to_owned()]);
        assert_eq!(q1.hits[0].html_url, "https://github.com/fox/xhs-cover");
        assert_eq!(q1.full_match_count, 2);
        assert!(q1.gap_intents.is_empty());
        assert_eq!(q1.preselected, vec!["r01".to_owned(), "r12".to_owned()]);
        assert_eq!(q1.pending_matches, 0);
        assert_eq!(q1.suggested_scenario_id.as_deref(), Some("sc-xhs"));
        assert_eq!(q1.suggested_scenario_count, Some(2));
        let pending = q1
            .hits
            .iter()
            .find(|hit| hit.id == "r12")
            .expect("没卡的 r12 应进结果");
        assert!(!pending.has_card);
        assert!(pending.full_match);
        assert_eq!(
            pending.one_liner.as_deref(),
            Some("小红书封面素材包，还没写理解卡")
        );

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
        assert!(
            q5.gap_intents.is_empty(),
            "图片应能靠描述里的 image 对上：{:?}",
            q5.gap_intents
        );
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
        // 没卡的仓库现在直接进结果。这四条都在前 8 里，所以 pending 为 0。
        assert_eq!(result.pending_matches, 0);
        let ids = result
            .hits
            .iter()
            .map(|hit| hit.id.as_str())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"card"));
        assert!(ids.contains(&"desc"));
        assert!(ids.contains(&"name"));
        assert!(ids.contains(&"split"));
        assert!(result
            .hits
            .iter()
            .any(|hit| hit.id == "desc" && !hit.has_card && hit.full_match));
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
        assert_eq!(find("order").why, "对上：封面（问题：这里有封面）");
        assert_eq!(find("fallback").why, "对上：封面（关键词）");
        let explained = &find("long").why;
        assert!(explained.starts_with("对上：封面（问题："));
        assert!(explained.chars().count() <= 80);
        assert!(!explained.ends_with('…'));
        let prefix: String = long.chars().take(20).collect();
        assert!(explained.contains(&prefix));
        assert_ne!(find("exact").why, exact);
        assert!(find("exact").why.starts_with("对上：封面（问题："));
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
        assert_eq!(good.why, "对上：封面（能力：封面能力）");
        let partial_hit = result.hits.iter().find(|hit| hit.id == "partial").unwrap();
        assert_eq!(partial_hit.form.as_deref(), Some("app"));
        assert_eq!(partial_hit.why, "对上：封面（能力：封面能力）");
        let types = result.hits.iter().find(|hit| hit.id == "types").unwrap();
        assert!(types.form.is_none());
        assert_eq!(types.why, "对上：封面（一句话）");
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
        assert_eq!(result.hits.len(), 2);
        assert_eq!(result.hits[0].id, "keep");
        assert_eq!(result.hits[0].stars_count, 10);
        assert_eq!(result.hits[1].id, "pending");
        assert!(!result.hits[1].has_card);
        assert!(result
            .hits
            .iter()
            .all(|hit| hit.id != "removed" && hit.id != "gone"));
        // 没卡的那条已经出现在 hits 里，不再计入 pending。
        assert_eq!(result.pending_matches, 0);
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
        const LINE_CHATCUT: &str = "ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件，支持通过 AI 辅助编辑 ChatCut 视频项目，具体功能包括导入媒体、修改时间线、创建动态图形…";
        // 一句话超过 WHY_LIMIT（80）后，为什么仍按原规则硬截，逻辑没改。
        const WHY_CHATCUT: &str = "ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件，支持通过 AI 辅助编辑 ChatCut 视频项目，具体功能包括导…";
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
        assert_eq!(chatcut.hits[0].why, WHY_CHATCUT);
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

    fn bare_repo<'a>(
        id: &'a str,
        full_name: &'a str,
        description: &'a str,
        stars: i64,
    ) -> NewRepo<'a> {
        NewRepo {
            id,
            full_name,
            description,
            stars,
            status: "active",
            one_liner: None,
            summary: "",
            keywords_json: "[]",
            card_json: None,
        }
    }

    fn card_repo<'a>(
        id: &'a str,
        full_name: &'a str,
        description: &'a str,
        stars: i64,
        one_liner: &'a str,
    ) -> NewRepo<'a> {
        NewRepo {
            id,
            full_name,
            description,
            stars,
            status: "active",
            one_liner: Some(one_liner),
            summary: "",
            keywords_json: "[]",
            card_json: None,
        }
    }

    #[test]
    fn contains_word_rejects_partial_latin() {
        assert!(!contains_word("execute", "cut"));
        assert!(!contains_word("precut", "cut"));
        assert!(!contains_word("cutting", "cut"));
        assert!(contains_word("cut", "cut"));
        assert!(contains_word("cut the clip", "cut"));
        assert!(contains_word("a-cut-b", "cut"));
        assert!(contains_word("please cut.", "cut"));
        assert!(!contains_word("consrt", "srt"));
        assert!(!contains_word("srtfile", "srt"));
        assert!(contains_word("demo.srt", "srt"));
        assert!(contains_word("导出 srt 字幕", "srt"));
        assert!(contains_word("生成srt字幕", "srt"));
        assert!(!contains_word("i18next", "i18n"));
        assert!(contains_word("use-i18n-kit", "i18n"));
        assert!(!contains_word("", "cut"));
        assert!(!contains_word("cut", ""));
        assert!(!contains_word("ragtime", "rag"));
        assert!(contains_word("local rag index", "rag"));
    }

    #[test]
    fn scenario_alone_does_not_count_as_match() {
        let db = prepare("scenario-only", |conn| {
            insert_scenario(conn, "dy", "dy", "抖音·短视频", "[\"字幕\",\"下载\"]", 1, 0);
            insert_repo(
                conn,
                &bare_repo("crawler", "lab/page-fetcher", "download saved pages", 99),
            );
            link_scenario(conn, "crawler", "dy", "confirmed");
            set_topics(conn, "crawler", r#"["http"]"#);
            insert_repo(
                conn,
                &card_repo("sub", "lab/sub-only", "只写了字幕", 2, "给视频加字幕"),
            );
            link_scenario(conn, "sub", "dy", "confirmed");
        });
        let result = run(&db, "短视频加字幕");
        assert!(
            result.hits.iter().all(|hit| hit.id != "crawler"),
            "只挂在场景上的爬虫不该出现：{:?}",
            result
                .hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>()
        );
        let sub = result
            .hits
            .iter()
            .find(|hit| hit.id == "sub")
            .expect("字幕仓库应出现");
        assert!(!sub.full_match);
        assert_eq!(sub.matched, vec!["字幕".to_owned()]);
        assert!(sub.missing.iter().any(|word| word == "短视频"));
        assert!(!sub.matched.iter().any(|word| word == "短视频"));
    }

    #[test]
    fn scenario_bonus_ranks_above_content_only() {
        let db = prepare("scenario-bonus", |conn| {
            insert_scenario(conn, "cap", "cap", "封面工坊", "[\"封面\"]", 1, 0);
            insert_repo(conn, &card_repo("both", "lab/封面甲", "", 5, "有卡"));
            insert_repo(conn, &card_repo("text", "lab/封面乙", "", 5, "有卡"));
            link_scenario(conn, "both", "cap", "confirmed");
        });
        let result = run(&db, "封面");
        assert_eq!(
            result
                .hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["both", "text"]
        );
        assert!(result.hits[0].score > result.hits[1].score);
        let weight = (1.0_f64 + 2.0 / 3.0).ln();
        approx(result.hits[0].score, 3.5 * weight);
        approx(result.hits[1].score, 3.0 * weight);
        assert!((result.hits[0].score - result.hits[1].score - 0.5 * weight).abs() < 1e-9);
    }

    #[test]
    fn scenario_keyword_bonus_needs_content_first() {
        let db = prepare("scenario-kw", |conn| {
            insert_scenario(conn, "video", "video", "影像", "[\"字幕\"]", 1, 0);
            insert_repo(conn, &card_repo("hit", "lab/字幕工具", "", 3, "有卡"));
            insert_repo(
                conn,
                &card_repo("miss", "lab/plain", "没有这个词", 9, "有卡"),
            );
            link_scenario(conn, "hit", "video", "confirmed");
            link_scenario(conn, "miss", "video", "confirmed");
        });
        let result = run(&db, "字幕");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].id, "hit");
        let weight = (1.0_f64 + 2.0 / 2.0).ln();
        // 名称命中 3.0，场景关键词再加 0.5。另一个仓库只有场景关键词，不进结果。
        approx(result.hits[0].score, 3.5 * weight);
    }

    #[test]
    fn cardless_english_subtitle_enters_hits() {
        let db = prepare("autoclip", |conn| {
            let repo = bare_repo(
                "auto",
                "lab/autoclip-subtitle",
                "AI video clipping with subtitle generation",
                12,
            );
            insert_repo(conn, &repo);
            set_topics(conn, "auto", r#"["video-editing"]"#);
        });
        let result = run(&db, "短视频加字幕");
        assert_eq!(result.hits.len(), 1);
        let hit = &result.hits[0];
        assert_eq!(hit.id, "auto");
        assert!(!hit.has_card);
        assert!(hit.form.is_none());
        assert_eq!(
            hit.one_liner.as_deref(),
            Some("AI video clipping with subtitle generation")
        );
        assert_eq!(hit.matched, vec!["字幕".to_owned()]);
        assert!(hit.missing.iter().any(|word| word == "短视频"));
        assert!(!hit.full_match);
        assert!(hit.why.contains("对上："));
        assert!(hit.why.contains("字幕（名称）"));
        assert_ne!(hit.why, hit.one_liner.as_deref().unwrap_or(""));
    }

    #[test]
    fn synonym_cover_matches_thumbnail() {
        let db = prepare("syn-cover", |conn| {
            insert_repo(
                conn,
                &bare_repo("thumb", "lab/thumb", "export a thumbnail for the post", 4),
            );
        });
        let result = run(&db, "封面");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].matched, vec!["封面".to_owned()]);
        assert!(result.hits[0].full_match);
        assert!(result.hits[0].why.contains("（描述）"));
        let weight = (1.0_f64 + 1.0 / 1.0).ln();
        approx(result.hits[0].score, weight);
    }

    #[test]
    fn synonym_ecommerce_matches_shopify() {
        let db = prepare("syn-ec", |conn| {
            insert_repo(
                conn,
                &bare_repo("shop", "lab/shop", "a shopify helper for listings", 4),
            );
        });
        let result = run(&db, "电商");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].matched, vec!["电商".to_owned()]);
        assert!(result.hits[0].why.starts_with("对上：电商（"));
        assert!(!result.hits[0].has_card);
    }

    #[test]
    fn synonym_poster_group_matches_banner() {
        let db = prepare("syn-poster", |conn| {
            insert_repo(
                conn,
                &bare_repo("banner", "lab/banner", "event banner maker", 2),
            );
        });
        let result = run(&db, "海报");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].matched, vec!["海报".to_owned()]);
    }

    #[test]
    fn short_synonym_cut_and_srt_need_boundaries() {
        let db = prepare("boundary", |conn| {
            insert_repo(
                conn,
                &bare_repo("exec", "lab/exec", "execute the daily job", 5),
            );
            insert_repo(conn, &bare_repo("cut", "lab/cut", "please cut now", 4));
            insert_repo(conn, &bare_repo("stuck", "lab/stuck", "consrt file", 3));
            insert_repo(
                conn,
                &bare_repo("srt", "lab/srt", "render demo.srt tonight", 2),
            );
        });
        let cut = run(&db, "剪辑");
        assert_eq!(
            cut.hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["cut"]
        );
        assert_eq!(cut.hits[0].matched, vec!["剪辑".to_owned()]);
        let srt = run(&db, "字幕");
        assert_eq!(
            srt.hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["srt"]
        );
        assert!(srt.hits.iter().all(|hit| hit.id != "stuck"));
    }

    #[test]
    fn preselected_caps_at_three_full_matches() {
        let db = prepare("pre-cap", |conn| {
            for index in 0..5 {
                let id = format!("p{index}");
                let full_name = format!("lab/封面{index}");
                insert_repo(
                    conn,
                    &card_repo(&id, &full_name, "", 10 - index as i64, "有卡"),
                );
            }
        });
        let result = run(&db, "封面");
        assert_eq!(result.full_match_count, 5);
        assert_eq!(
            result.preselected,
            vec!["p0".to_owned(), "p1".to_owned(), "p2".to_owned()]
        );
        assert!(result.hits.iter().all(|hit| hit.full_match));
    }

    #[test]
    fn preselected_can_include_cardless_full_match() {
        let db = prepare("pre-cardless", |conn| {
            insert_repo(
                conn,
                &bare_repo("free", "lab/短视频字幕", "短视频字幕工具", 8),
            );
        });
        let result = run(&db, "短视频加字幕");
        assert_eq!(result.hits.len(), 1);
        assert!(!result.hits[0].has_card);
        assert!(result.hits[0].full_match);
        assert_eq!(result.preselected, vec!["free".to_owned()]);
    }

    #[test]
    fn preselected_empty_when_nothing_fully_matches() {
        let db = prepare("pre-empty", |conn| {
            insert_repo(conn, &card_repo("sub", "lab/sub", "", 3, "只有字幕"));
            insert_repo(conn, &card_repo("short", "lab/short", "", 9, "只有短视频"));
        });
        let result = run(&db, "短视频加字幕");
        assert_eq!(result.hits.len(), 2);
        assert!(result.hits.iter().all(|hit| !hit.full_match));
        assert!(result.preselected.is_empty());
        assert_eq!(result.full_match_count, 0);
    }

    #[test]
    fn pending_matches_ignore_cardless_already_in_hits() {
        let db = prepare("pending-out", |conn| {
            insert_repo(conn, &card_repo("card", "lab/小红书封面卡", "", 0, "有卡"));
            for index in 1..=9 {
                let id = format!("n{index:02}");
                let full_name = format!("lab/小红书封面{index:02}");
                insert_repo(conn, &bare_repo(&id, &full_name, "", index as i64));
            }
            insert_repo(conn, &bare_repo("split", "lab/小红书xx", "只有封面", 0));
        });
        let result = run(&db, "做小红书封面");
        assert_eq!(result.hits.len(), 8);
        assert!(result.hits.iter().any(|hit| hit.id == "card"));
        assert!(result.hits.iter().all(|hit| hit.id != "split"));
        assert!(result
            .hits
            .iter()
            .all(|hit| hit.id != "n01" && hit.id != "n02"));
        // 排在 8 名外的两条名称全中；split 没有任何一个字段同时含两个意图词。
        assert_eq!(result.pending_matches, 2);
        let no_intent = run(&db, "zzqq");
        assert!(no_intent.intents.is_empty());
        assert_eq!(no_intent.pending_matches, 0);
    }

    #[test]
    fn why_points_at_name_before_capability() {
        let card = card_json("app", &["封面能力很长"], &["封面问题"]);
        let db = prepare("why-name", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "named",
                    full_name: "lab/封面库",
                    description: "封面描述",
                    stars: 1,
                    status: "active",
                    one_liner: Some("这是一句完整的话"),
                    summary: "封面摘要",
                    keywords_json: "[\"封面\"]",
                    card_json: Some(&card),
                },
            );
        });
        let result = run(&db, "封面");
        assert_eq!(result.hits[0].why, "对上：封面（名称）");
        assert_ne!(
            result.hits[0].why,
            result.hits[0].one_liner.as_deref().unwrap_or("")
        );
        assert!(result.hits[0].why.contains("对上："));
        assert!(result.hits[0].why.contains("（名称）"));
    }

    #[test]
    fn why_truncates_at_last_complete_separator() {
        let cap = format!("小红书封面字幕{}", "补".repeat(20));
        assert!(cap.chars().count() > 20);
        let card = card_json("app", &[&cap], &[]);
        let db = prepare("why-cut", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "long",
                    full_name: "lab/tool",
                    description: "",
                    stars: 1,
                    status: "active",
                    one_liner: Some("有卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: Some(&card),
                },
            );
        });
        let result = run(&db, "小红书封面字幕");
        let why = &result.hits[0].why;
        assert!(why.starts_with("对上："));
        assert!(
            why.chars().count() <= 80,
            "{why} 有 {} 字",
            why.chars().count()
        );
        assert!(!why.contains("字幕（能力"));
        assert!(why.contains("、"));
        assert!(!why.ends_with('、'));
        assert!(!why.ends_with('…'));
        let snippet: String = cap.chars().take(20).collect();
        assert!(why.contains(&snippet));
        assert_ne!(why, result.hits[0].one_liner.as_deref().unwrap_or(""));
    }

    #[test]
    fn extras_only_why_keeps_legacy_wording() {
        let card = card_json("app", &["用不上"], &["zzqq 问题说明"]);
        let db = prepare("why-extra", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "zz",
                    full_name: "lab/zzqq-tool",
                    description: "",
                    stars: 1,
                    status: "active",
                    one_liner: Some("一句话本身"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: Some(&card),
                },
            );
        });
        let result = run(&db, "zzqq");
        assert!(result.intents.is_empty());
        assert_eq!(result.extras, vec!["zzqq".to_owned()]);
        assert_eq!(result.hits[0].why, "zzqq 问题说明");
        assert!(!result.hits[0].why.starts_with("对上："));
        assert!(!result.hits[0].full_match);
        assert!(result.preselected.is_empty());
    }

    #[test]
    fn topics_match_without_a_card() {
        let db = prepare("topics", |conn| {
            insert_repo(conn, &bare_repo("topic", "lab/plain", "nothing here", 6));
            set_topics(conn, "topic", r#"["Subtitle"]"#);
        });
        let result = run(&db, "字幕");
        assert_eq!(result.hits.len(), 1);
        assert!(!result.hits[0].has_card);
        assert_eq!(result.hits[0].matched, vec!["字幕".to_owned()]);
        assert_eq!(result.hits[0].why, "对上：字幕（话题）");
        assert_eq!(result.hits[0].one_liner.as_deref(), Some("nothing here"));
    }

    #[test]
    fn cardless_one_liner_is_first_sentence() {
        assert_eq!(first_sentence("第一句。第二句"), "第一句。");
        assert_eq!(first_sentence("好！下一句"), "好！");
        assert_eq!(first_sentence("吗？后面"), "吗？");
        assert_eq!(
            first_sentence("First sentence. Second sentence."),
            "First sentence."
        );
        assert_eq!(first_sentence("版本 3.5 可用。后面"), "版本 3.5 可用。");
        assert_eq!(first_sentence("line1\nline2。后面"), "line1 line2。");
        assert_eq!(first_sentence("  没有句号的短句  "), "没有句号的短句");
        assert_eq!(first_sentence("Hello."), "Hello.");
        assert_eq!(first_sentence("Hello.World stays"), "Hello.World stays");
        assert!(first_sentence("").is_empty());
        assert!(first_sentence(" \n\t ").is_empty());

        let short = "甲，乙".repeat(10);
        assert!(short.chars().count() < 120);
        assert_eq!(first_sentence(&short), short);
        assert!(!first_sentence(&short).contains('…'));

        let long = format!("{}{}", "甲，".repeat(80), "尾巴");
        assert!(long.chars().count() > 120);
        let cut = first_sentence(&long);
        assert!(cut.ends_with('…'));
        assert!(cut.chars().count() <= 121);
        assert!(!cut.contains('尾'));

        let hard = "字".repeat(130);
        let hard_cut = first_sentence(&hard);
        assert_eq!(hard_cut.chars().count(), 121);
        assert!(hard_cut.ends_with('…'));
        assert_eq!(hard_cut.chars().next().unwrap(), '字');
    }

    #[test]
    fn equal_scores_put_cards_before_stars() {
        let db = prepare("tie-card", |conn| {
            insert_repo(conn, &bare_repo("free", "lab/封面无卡", "", 100));
            insert_repo(conn, &card_repo("card", "lab/封面有卡", "", 1, "有卡"));
        });
        let result = run(&db, "封面");
        assert_eq!(
            result
                .hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            vec!["card", "free"]
        );
        approx(result.hits[0].score, result.hits[1].score);
        assert!(result.hits[0].has_card);
        assert!(!result.hits[1].has_card);
        assert!(result.hits[1].stars_count > result.hits[0].stars_count);
    }

    #[test]
    fn gibberish_query_does_not_preselect_or_full_match() {
        let db = prepare("gibberish", |conn| {
            insert_repo(
                conn,
                &card_repo("one", "lab/one", "普通描述", 3, "一条普通的理解卡"),
            );
        });
        let result = run(&db, "xyzzyfoobar 随便说说");
        assert!(result.intents.is_empty(), "{:?}", result.intents);
        assert!(result.hits.iter().all(|hit| !hit.full_match));
        assert!(result.preselected.is_empty());
        assert_eq!(result.full_match_count, 0);
        assert_eq!(result.pending_matches, 0);
    }

    #[test]
    fn evidence_short_video_subtitle_fixture() {
        let card = card_json("skill", &["自动剪口播并生成字幕"], &["口播没有字幕"]);
        let db = prepare("evidence", |conn| {
            insert_scenario(conn, "dy", "dy", "抖音·短视频", "[\"下载\"]", 1, 0);
            insert_repo(
                conn,
                &bare_repo("crawler", "lab/page-fetcher", "download saved pages", 80),
            );
            link_scenario(conn, "crawler", "dy", "confirmed");
            set_topics(conn, "crawler", r#"["http"]"#);
            insert_repo(
                conn,
                &NewRepo {
                    id: "jianying",
                    full_name: "lab/jianying-skill",
                    description: "editor",
                    stars: 40,
                    status: "active",
                    one_liner: Some("给短视频加字幕"),
                    summary: "字幕工具",
                    keywords_json: "[\"短视频\",\"字幕\"]",
                    card_json: Some(&card),
                },
            );
            insert_repo(
                conn,
                &bare_repo(
                    "autoclip",
                    "lab/autoclip-subtitle",
                    "AI video clipping with subtitle generation",
                    15,
                ),
            );
            set_topics(conn, "autoclip", r#"["video-editing"]"#);
        });
        let result = run(&db, "短视频加字幕");
        assert!(result.hits.iter().all(|hit| hit.id != "crawler"));
        let tool = result
            .hits
            .iter()
            .find(|hit| hit.id == "jianying")
            .expect("有卡的字幕工具应出现");
        let auto = result
            .hits
            .iter()
            .find(|hit| hit.id == "autoclip")
            .expect("英文没卡仓库应出现");
        assert_eq!(tool.matched, vec!["短视频".to_owned(), "字幕".to_owned()]);
        assert!(tool.missing.is_empty());
        assert!(tool.full_match);
        assert!(tool.has_card);
        assert_eq!(
            tool.why,
            "对上：短视频（一句话）、字幕（能力：自动剪口播并生成字幕）"
        );
        assert_eq!(auto.matched, vec!["字幕".to_owned()]);
        assert_eq!(auto.missing, vec!["短视频".to_owned()]);
        assert!(!auto.full_match);
        assert!(!auto.has_card);
        assert_eq!(auto.why, "对上：字幕（名称）");
        assert_eq!(
            auto.one_liner.as_deref(),
            Some("AI video clipping with subtitle generation")
        );
        let weight = (1.0_f64 + 3.0 / 2.0).ln();
        approx(tool.score, 8.0 * weight);
        approx(auto.score, weight);
        assert!(tool.score > auto.score);
        assert_eq!(result.hits[0].id, "jianying");
    }

    // ───────────── README 里提到几次：加成、补召回、整词计数 ─────────────

    fn insert_readme(conn: &Connection, repo_id: &str, markdown: &str) {
        conn.execute(
            "INSERT INTO repo_readmes(repo_id, raw_markdown, content_hash, source_path, fetched_at)
             VALUES (?1, ?2, 'hash', 'README.md', '2026-09-01T00:00:00Z')",
            params![repo_id, markdown],
        )
        .unwrap_or_else(|error| panic!("插入 README {repo_id} 失败：{error}"));
    }

    fn readme_repo<'a>(id: &'a str, full_name: &'a str, description: &'a str) -> NewRepo<'a> {
        NewRepo {
            id,
            full_name,
            description,
            stars: 1,
            status: "active",
            one_liner: None,
            summary: "",
            keywords_json: "[]",
            card_json: None,
        }
    }

    #[test]
    fn count_whole_word_respects_latin_boundaries_but_not_han() {
        assert_eq!(count_whole_word("cut the video, then cut again", "cut"), 2);
        assert_eq!(count_whole_word("execute the cutter", "cut"), 0);
        assert_eq!(count_whole_word("用cut命令", "cut"), 1);
        assert_eq!(count_whole_word("cut", "cut"), 1);
        assert_eq!(count_whole_word("", "cut"), 0);
        assert_eq!(count_whole_word("cut", ""), 0);
        assert_eq!(count_whole_word("a cut2 b", "cut"), 0);
    }

    #[test]
    fn count_mentions_adds_synonyms_and_uses_boundaries_for_short_ones() {
        // 封面：本词 2 次 + cover 1 次 + thumbnail 1 次；poster 没有
        assert_eq!(
            count_mentions("封面做好了，再换一个封面。cover 和 thumbnail", "封面"),
            4
        );
        // 小红书的同义词 xhs 只有 3 个字母，必须整词：xhsdownloader 不算
        assert_eq!(
            count_mentions("xhs 和 xhsdownloader，还有 xiaohongshu", "小红书"),
            2
        );
        // 没有同义词表的词只数自己
        assert_eq!(count_mentions("发布发布发布", "发布"), 3);
        assert_eq!(count_mentions("", "封面"), 0);
    }

    #[test]
    fn readme_evidence_needs_two_mentions_and_saturates() {
        let stats = |hits: usize, kb: f64| {
            let mut map = HashMap::new();
            map.insert("封面".to_owned(), hits);
            ReadmeStats { kb, hits: map }
        };
        assert_eq!(stats(0, 10.0).evidence("封面"), 0.0);
        assert_eq!(stats(1, 10.0).evidence("封面"), 0.0, "只提到 1 次不算证据");
        assert_eq!(stats(5, 10.0).evidence("没有这个词"), 0.0);
        let few = stats(2, 10.0).evidence("封面");
        let some = stats(8, 10.0).evidence("封面");
        let many = stats(16, 10.0).evidence("封面");
        let lots = stats(400, 10.0).evidence("封面");
        assert!(0.0 < few && few < some && some < many);
        assert!(lots <= 1.0 + 1e-9);
        // 同样的次数，README 越短（占比越高）证据越强；很短的 README 按 3 千字算，不会被无限抬高
        assert!(stats(10, 3.0).evidence("封面") > stats(10, 60.0).evidence("封面"));
        assert!((stats(10, 3.0).evidence("封面") - stats(10, 3.0).evidence("封面")).abs() < 1e-12);
    }

    #[test]
    fn readme_mentions_lift_a_repo_that_already_matches() {
        let db = prepare("readme-boost", |conn| {
            insert_repo(conn, &readme_repo("quiet", "lab/quiet", "封面工具"));
            insert_repo(conn, &readme_repo("focused", "lab/focused", "封面工具"));
            insert_readme(conn, "quiet", "# quiet\n一个小工具。");
            insert_readme(conn, "focused", &"做封面。封面模板。".repeat(10));
        });
        let result = run(&db, "封面");
        assert_eq!(result.hits.len(), 2);
        assert_eq!(result.hits[0].id, "focused");
        assert!(result.hits[0].score > result.hits[1].score);
        assert!(
            result.hits[0].why.contains("说明里提到"),
            "{}",
            result.hits[0].why
        );
        assert!(
            !result.hits[1].why.contains("README"),
            "{}",
            result.hits[1].why
        );
    }

    #[test]
    fn readme_only_match_is_found_but_ranks_below_a_field_match() {
        let db = prepare("readme-only", |conn| {
            let mut direct = readme_repo("direct", "lab/direct", "做封面的工具");
            direct.one_liner = Some("专门做封面");
            insert_repo(conn, &direct);
            insert_repo(conn, &readme_repo("hidden", "lab/hidden", "一个通用工具集"));
            insert_readme(conn, "direct", "# direct");
            insert_readme(
                conn,
                "hidden",
                &"支持做封面和海报。封面模板很多。".repeat(6),
            );
        });
        let result = run(&db, "封面");
        let ids: Vec<&str> = result.hits.iter().map(|hit| hit.id.as_str()).collect();
        assert_eq!(ids, vec!["direct", "hidden"]);
        assert!(result.hits[0].score > result.hits[1].score);
        assert!(
            result.hits[1].why.contains("项目说明里提到"),
            "{}",
            result.hits[1].why
        );
        assert!(result.hits[1].full_match);
    }

    #[test]
    fn a_single_readme_mention_is_not_a_match() {
        let db = prepare("readme-once", |conn| {
            insert_repo(conn, &readme_repo("aside", "lab/aside", "一个通用工具集"));
            insert_readme(conn, "aside", "也可以顺便做一次封面。其余都是别的内容。");
        });
        let result = run(&db, "封面");
        assert!(result.hits.is_empty());
    }

    #[test]
    fn readme_stats_ignore_inactive_repos_and_empty_readmes() {
        let db = prepare("readme-inactive", |conn| {
            let mut gone = readme_repo("gone", "lab/gone", "封面");
            gone.status = "removed";
            insert_repo(conn, &gone);
            insert_repo(conn, &readme_repo("blank", "lab/blank", "工具集"));
            insert_readme(conn, "gone", &"封面。".repeat(20));
            insert_readme(conn, "blank", "   \n  ");
        });
        let connection = open_read(&db.path).unwrap();
        let stats = load_readme_stats(&connection, &["封面".to_owned()]).unwrap();
        assert!(stats.is_empty());
        let none = load_readme_stats(&connection, &[]).unwrap();
        assert!(none.is_empty());
    }

    fn text_repo(id: &str, text: &str) -> RepoRecord {
        RepoRecord {
            id: id.to_owned(),
            full_name: format!("lab/{id}"),
            html_url: String::new(),
            stars: 1,
            one_liner: String::new(),
            problems: Vec::new(),
            capabilities: Vec::new(),
            form: None,
            scenario_ids: Vec::new(),
            blocks: TextBlocks {
                name: String::new(),
                one_liner: String::new(),
                capabilities: String::new(),
                problems: String::new(),
                keywords: String::new(),
                scenarios: String::new(),
                summary: String::new(),
                description: text.to_lowercase(),
                topics: String::new(),
            },
            has_card: false,
            github_description: text.to_owned(),
        }
    }

    fn sample_corpus() -> Vec<RepoRecord> {
        let mut repos = vec![
            text_repo("rec", "保存记录"),
            text_repo("tidy", "请你整理"),
            text_repo("batch", "支持批量"),
            text_repo("boss", "老板来了"),
            text_repo("quarter", "季度计划"),
            text_repo("brief", "书面汇报"),
            text_repo("stops", "我想 想做 一个 工具 帮我"),
        ];
        for index in 0..4 {
            repos.push(text_repo(&format!("bot{index}"), "对话机器人"));
        }
        repos
    }

    fn has_term(words: &[String], expected: &str) -> bool {
        words.iter().any(|word| word == expected)
    }

    fn rich_card(
        form: &str,
        caps: &[&str],
        problems: &[&str],
        use_when: &[&str],
        what_it_is: &str,
    ) -> String {
        let capabilities = caps
            .iter()
            .map(|name| json!({"name_zh": name}))
            .collect::<Vec<_>>();
        json!({
            "form": form,
            "capabilities": capabilities,
            "problems_solved": problems,
            "use_when": use_when,
            "what_it_is": what_it_is,
        })
        .to_string()
    }

    fn assert_english_synonym(label: &str, intent: &str, english: &str) {
        let db = prepare(label, |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "en",
                    full_name: "lab/en-only",
                    description: english,
                    stars: 1,
                    status: "active",
                    one_liner: Some("有理解卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, intent);
        assert!(
            has_term(&result.intents, intent),
            "{intent} 应是意图词：{:?}",
            result.intents
        );
        assert!(
            result
                .hits
                .iter()
                .any(|hit| { hit.id == "en" && has_term(&hit.matched, intent) }),
            "{intent} 应靠 {english} 对上：{:?}",
            result.hits
        );
    }

    #[test]
    fn concepts_group_chat_daily_keeps_real_words() {
        let (main, extra) = concepts("把群聊记录整理成日报", &base_lexicon(), &sample_corpus());
        assert!(has_term(&main, "群聊"), "{main:?}");
        assert!(has_term(&main, "日报"), "{main:?}");
        assert_eq!(
            extra,
            vec!["记录".to_owned(), "整理".to_owned()],
            "{extra:?}"
        );
        for broken in ["录整", "理成", "成日", "把群"] {
            assert!(!has_term(&extra, broken), "{broken} 仍在 {extra:?}");
        }
    }

    #[test]
    fn concepts_resize_compress_keeps_batch_not_fragments() {
        let (main, extra) = concepts("批量改图尺寸和压缩", &base_lexicon(), &sample_corpus());
        for word in ["压缩", "尺寸", "改图"] {
            assert!(has_term(&main, word), "{word} 不在 {main:?}");
        }
        assert_eq!(extra, vec!["批量".to_owned()], "{extra:?}");
        for broken in ["量改", "图尺", "寸和", "和压"] {
            assert!(!has_term(&extra, broken), "{broken} 仍在 {extra:?}");
        }
    }

    #[test]
    fn concepts_cohesion_drops_inner_slices() {
        let (main, extra) = concepts("对话机器人", &base_lexicon(), &sample_corpus());
        assert!(main.is_empty(), "{main:?}");
        assert!(!has_term(&extra, "话机"), "{extra:?}");
        assert!(!has_term(&extra, "器人"), "{extra:?}");
    }

    #[test]
    fn concepts_keep_three_character_words_and_drop_glued_slices() {
        let repos = vec![
            text_repo("m1", "聊天机器人"),
            text_repo("m2", "客服机器人系统"),
            text_repo("m3", "机器人控制器"),
            text_repo("m4", "写作机器人助手"),
        ];
        let (main, extra) = concepts("做机器人", &base_lexicon(), &repos);
        assert!(main.is_empty(), "{main:?}");
        assert_eq!(extra, vec!["机器人".to_owned()]);
        // 「机器」后面几乎总是「人」（右边凝固），「器人」前面几乎总是「机」（左边凝固）：单独问也不留
        let (_main, extra) = concepts("机器", &base_lexicon(), &repos);
        assert!(!has_term(&extra, "机器"), "{extra:?}");
        let (_main, extra) = concepts("器人", &base_lexicon(), &repos);
        assert!(!has_term(&extra, "器人"), "{extra:?}");
    }

    #[test]
    fn concepts_prefer_the_whole_word_over_a_common_prefix() {
        // 「数据」几乎到处都有，「数据库」只在 3 个仓库里：问「数据库」要得到一个词，
        // 而不是「数据」加一个被跳过的「库」
        let tails: Vec<char> = "分析清洗治理采集建模标注".chars().collect();
        let mut repos: Vec<RepoRecord> = (0..60)
            .map(|index| {
                text_repo(
                    &format!("d{index}"),
                    &format!("数据{}", tails[index % tails.len()]),
                )
            })
            .collect();
        for index in 0..3 {
            repos.push(text_repo(&format!("db{index}"), "数据库"));
        }
        let (_main, extra) = concepts("数据库", &base_lexicon(), &repos);
        assert_eq!(extra, vec!["数据库".to_owned()]);
    }

    #[test]
    fn concepts_keep_the_more_common_of_two_overlapping_words() {
        // 「写稿」只在 2 个仓库里、「稿件」在 20 个里：「写稿件」切成「稿件」，不是「写稿」
        let tails: Vec<char> = "分析清洗治理采集建模标注".chars().collect();
        let mut repos: Vec<RepoRecord> = (0..20)
            .map(|index| {
                text_repo(
                    &format!("g{index}"),
                    &format!("{}稿件", tails[index % tails.len()]),
                )
            })
            .collect();
        repos.push(text_repo("w1", "写稿"));
        repos.push(text_repo("w2", "写稿"));
        let (_main, extra) = concepts("写稿件", &base_lexicon(), &repos);
        assert_eq!(extra, vec!["稿件".to_owned()]);
    }

    #[test]
    fn concepts_cohesion_threshold_is_strictly_above_three_quarters() {
        let mut dominant = HashMap::new();
        dominant.insert('对', 4usize);
        assert!(glued_into_longer_word(&GramStat {
            repos: 4,
            total: 4,
            left: dominant,
            right: HashMap::new(),
        }));
        let mut exact = HashMap::new();
        exact.insert('对', 3usize);
        assert!(
            !glued_into_longer_word(&GramStat {
                repos: 4,
                total: 4,
                left: exact,
                right: HashMap::new(),
            }),
            "刚好 75% 还不算「以上」"
        );
        let mut few = HashMap::new();
        few.insert('对', 2usize);
        assert!(!glued_into_longer_word(&GramStat {
            repos: 2,
            total: 2,
            left: few,
            right: HashMap::new(),
        }));
    }

    #[test]
    fn concepts_dp_skips_glue_characters() {
        let (_main, extra) = concepts("给老板做一份季度汇报", &base_lexicon(), &sample_corpus());
        assert!(has_term(&extra, "老板"), "{extra:?}");
        assert!(has_term(&extra, "季度"), "{extra:?}");
        assert!(has_term(&extra, "汇报"), "{extra:?}");
        for word in &extra {
            assert!(!word.contains('给'), "{word}");
            assert!(!word.contains('做'), "{word}");
            assert!(!word.contains('份'), "{word}");
        }
    }

    #[test]
    fn concepts_drops_zero_df_and_single_characters() {
        let (main, extra) = concepts("甲乙丙丁", &base_lexicon(), &sample_corpus());
        assert!(main.is_empty());
        assert!(extra.is_empty(), "df 为 0 的词不该出现：{extra:?}");
        let (main, extra) = concepts("做", &base_lexicon(), &sample_corpus());
        assert!(main.is_empty());
        assert!(extra.is_empty());
        assert!(extra.iter().all(|word| word.chars().count() != 1));
        let (_main, extra) = concepts("给老板做一份季度汇报", &base_lexicon(), &sample_corpus());
        assert!(!has_term(&extra, "度汇"));
        assert!(!has_term(&extra, "一份"));
        assert!(extra.iter().all(|word| word.chars().count() != 1));
    }

    #[test]
    fn concepts_stopwords_stay_out_of_extras() {
        let (_main, extra) = concepts("我想做一个工具", &base_lexicon(), &sample_corpus());
        for word in &extra {
            assert!(!is_stop(word), "停用词 {word} 进了 extras：{extra:?}");
        }
        for stop in ["我想", "想做", "一个", "工具", "帮我"] {
            assert!(!has_term(&extra, stop), "{stop} {extra:?}");
        }
    }

    #[test]
    fn concepts_main_words_stay_out_of_extras() {
        let (main, extra) = concepts("日报记录", &base_lexicon(), &sample_corpus());
        assert!(has_term(&main, "日报"), "{main:?}");
        assert!(has_term(&extra, "记录"), "{extra:?}");
        for word in &main {
            assert!(!has_term(&extra, word), "{word} 同时在 extras：{extra:?}");
        }
    }

    #[test]
    fn concepts_latin_fragment_goes_to_extras() {
        let (main, extra) = concepts("做 sora 视频", &base_lexicon(), &[]);
        assert!(has_term(&main, "视频"), "{main:?}");
        assert_eq!(extra, vec!["sora".to_owned()], "{extra:?}");
    }

    #[test]
    fn empty_concepts_do_not_match_readme() {
        let db = prepare("no-concept", |conn| {
            insert_repo(conn, &readme_repo("aside", "lab/aside", "一个通用工具集"));
            insert_readme(conn, "aside", &"封面。".repeat(8));
        });
        let result = run(&db, "啊");
        assert!(result.intents.is_empty());
        assert!(result.extras.is_empty());
        assert!(
            result.hits.is_empty(),
            "没有概念时不该靠 README 凑出结果：{:?}",
            result.hits
        );
    }

    #[test]
    fn base_lexicon_excludes_new_task_words() {
        assert_eq!(BASE_LEXICON.len(), 62);
        assert!(!BASE_LEXICON.contains(&"日报"));
        assert!(!BASE_LEXICON.contains(&"压缩"));
        assert!(!BASE_LEXICON.contains(&"尺寸"));
        assert!(BASE_LEXICON.contains(&"小红书"));
        assert!(BASE_LEXICON.contains(&"封面"));
        let lex = base_lexicon();
        assert!(lex.contains(&"日报".to_owned()));
        assert!(lex.contains(&"压缩".to_owned()));
        assert!(lex.contains(&"尺寸".to_owned()));
    }

    #[test]
    fn task_lexicon_makes_daily_compress_size_intents() {
        let (main, extra) = concepts("日报", &base_lexicon(), &[]);
        assert_eq!(main, vec!["日报".to_owned()]);
        assert!(extra.is_empty());
        let (main, extra) = concepts("压缩和尺寸", &base_lexicon(), &[]);
        assert!(has_term(&main, "压缩"), "{main:?}");
        assert!(has_term(&main, "尺寸"), "{main:?}");
        assert!(!has_term(&extra, "压缩"));
        assert!(!has_term(&extra, "尺寸"));
    }

    #[test]
    fn synonym_compress_matches_compress() {
        assert_english_synonym("syn-compress", "压缩", "compress png files");
    }

    #[test]
    fn synonym_size_matches_resize() {
        assert_english_synonym("syn-resize", "尺寸", "resize the canvas");
    }

    #[test]
    fn synonym_daily_matches_daily_report() {
        assert_english_synonym("syn-daily", "日报", "write a daily report");
    }

    #[test]
    fn synonym_transcribe_matches_transcribe() {
        assert_english_synonym("syn-asr", "转写", "transcribe the audio");
    }

    #[test]
    fn synonym_cutout_matches_background_removal() {
        assert_english_synonym("syn-matting", "抠图", "background removal toolkit");
    }

    #[test]
    fn synonym_watermark_matches_watermark() {
        assert_english_synonym("syn-watermark", "水印", "add a watermark");
    }

    #[test]
    fn synonym_podcast_matches_podcast() {
        assert_english_synonym("syn-podcast", "播客", "edit a podcast");
    }

    #[test]
    fn synonym_flowchart_matches_flowchart() {
        assert_english_synonym("syn-flow", "流程图", "draw a flowchart");
    }

    #[test]
    fn synonym_qrcode_matches_qrcode() {
        assert_english_synonym("syn-qr", "二维码", "generate a qr code");
    }

    #[test]
    fn synonym_feishu_matches_feishu() {
        assert_english_synonym("syn-feishu", "飞书", "export to feishu");
    }

    #[test]
    fn synonym_jianying_matches_capcut() {
        assert_english_synonym("syn-capcut", "剪映", "open in capcut");
    }

    #[test]
    fn synonym_digital_human_matches_digital_human() {
        assert_english_synonym("syn-human", "数字人", "build a digital human");
    }

    #[test]
    fn synonym_asr_does_not_match_inside_ceasr() {
        let db = prepare("syn-boundary", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "inside",
                    full_name: "lab/ceasr",
                    description: "the ceasr engine",
                    stars: 9,
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
                    id: "whole",
                    full_name: "lab/asr",
                    description: "run asr locally",
                    stars: 1,
                    status: "active",
                    one_liner: Some("有理解卡"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
        });
        let result = run(&db, "转写");
        let ids = result
            .hits
            .iter()
            .map(|hit| hit.id.as_str())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"whole"), "{ids:?}");
        assert!(!ids.contains(&"inside"), "ceasr 不该命中 asr：{ids:?}");
    }

    #[test]
    fn readme_compress_counts_as_evidence() {
        let db = prepare("readme-compress", |conn| {
            insert_repo(
                conn,
                &readme_repo("min", "lab/image-minifier", "a general image utility"),
            );
            insert_readme(
                conn,
                "min",
                "Please compress this file. Then compress it again. Finally compress the backup.",
            );
        });
        let result = run(&db, "压缩");
        assert_eq!(result.hits.len(), 1, "{:?}", result.hits);
        assert!(has_term(&result.hits[0].matched, "压缩"));
        assert!(result.hits[0].full_match);
        assert!(
            result.hits[0].why.contains("项目说明里提到"),
            "{}",
            result.hits[0].why
        );
    }

    #[test]
    fn readme_only_hits_are_listed_but_not_preselected() {
        let db = prepare("readme-only-preselect", |conn| {
            insert_repo(
                conn,
                &readme_repo("min", "lab/image-minifier", "a general image utility"),
            );
            insert_readme(
                conn,
                "min",
                "Please compress this file. Then compress it again. Finally compress the backup.",
            );
        });
        let result = run(&db, "压缩");
        assert_eq!(result.hits.len(), 1);
        assert!(result.hits[0].full_match, "仍然算全都对上");
        assert_eq!(result.full_match_count, 1);
        assert!(
            result.preselected.is_empty(),
            "只靠 README 提到的不默认勾选：{:?}",
            result.preselected
        );
    }

    #[test]
    fn content_evidence_is_preselected_next_to_readme_only_hits() {
        let db = prepare("readme-only-mixed", |conn| {
            insert_repo(conn, &readme_repo("real", "lab/real", "图片压缩工具"));
            insert_repo(
                conn,
                &readme_repo("min", "lab/image-minifier", "a general image utility"),
            );
            insert_readme(
                conn,
                "min",
                "Please compress this file. Then compress it again. Finally compress the backup.",
            );
            // 一个意图词在内容里对上、另一个只在 README 里提到：证据够，照常勾选
            insert_repo(conn, &readme_repo("mix", "lab/mixed", "批量压缩工具"));
            insert_readme(
                conn,
                "mix",
                "Resize photos. Then resize the covers. Finally resize the avatars.",
            );
        });
        let result = run(&db, "压缩");
        assert_eq!(result.full_match_count, 3, "{:?}", result.hits);
        assert_eq!(result.preselected.len(), 2, "{:?}", result.preselected);
        assert!(result.preselected.iter().any(|id| id == "real"));
        assert!(result.preselected.iter().any(|id| id == "mix"));
        assert!(!result.preselected.iter().any(|id| id == "min"));
        let mixed = run(&db, "压缩尺寸");
        let mix = mixed
            .hits
            .iter()
            .find(|hit| hit.id == "mix")
            .expect("mix 应在结果里");
        assert!(mix.full_match, "{mix:?}");
        assert!(
            mixed.preselected.iter().any(|id| id == "mix"),
            "{:?}",
            mixed.preselected
        );
    }

    #[test]
    fn english_synonym_in_the_description_counts_as_content_for_preselect() {
        let db = prepare("synonym-preselect", |conn| {
            insert_repo(
                conn,
                &readme_repo("en", "lab/squeezer", "Compress images in one click"),
            );
        });
        let result = run(&db, "压缩");
        assert_eq!(result.hits.len(), 1);
        assert!(result.hits[0].full_match);
        assert_eq!(result.preselected, vec!["en".to_owned()]);
    }

    #[test]
    fn use_when_alone_matches_subtitle() {
        let card = rich_card(
            "cli",
            &["剪切视频"],
            &["视频太长"],
            &["需要给视频添加字幕时"],
            "一个剪辑小工具",
        );
        let db = prepare("use-when", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "sub",
                    full_name: "lab/editor",
                    description: "editor",
                    stars: 3,
                    status: "active",
                    one_liner: Some("剪辑小工具"),
                    summary: "简介",
                    keywords_json: "[]",
                    card_json: Some(&card),
                },
            );
        });
        let result = run(&db, "字幕");
        assert_eq!(result.hits.len(), 1, "{:?}", result.hits);
        assert!(has_term(&result.hits[0].matched, "字幕"));
        assert!(
            result.hits[0].why.contains("问题"),
            "何时用应算进问题：{}",
            result.hits[0].why
        );
        assert!(
            result.hits[0].why.contains("需要给视频添加字幕时"),
            "{}",
            result.hits[0].why
        );
    }

    #[test]
    fn what_it_is_scores_summary_tier() {
        let card = rich_card(
            "app",
            &["处理文件"],
            &["文件太乱"],
            &["想整理文件时"],
            "这个工具专门做封面",
        );
        let db = prepare("what-it-is", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "cover",
                    full_name: "lab/files",
                    description: "files",
                    stars: 2,
                    status: "active",
                    one_liner: Some("文件工具"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: Some(&card),
                },
            );
        });
        let result = run(&db, "封面");
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].matched, vec!["封面".to_owned()]);
        let weight = (1.0_f64 + 1.0 / 2.0).ln();
        approx(result.hits[0].score, 1.5 * weight);
        assert!(
            result.hits[0].why.contains("摘要"),
            "what_it_is 应落在摘要档：{}",
            result.hits[0].why
        );
    }

    #[test]
    fn placeholder_boilerplate_does_not_match() {
        let card = json!({
            "one_liner_zh": "这是一个模板集合。",
            "what_it_is": "这是一个模板集合。",
            "form": "other",
            "primary_category": "dev-infra",
            "capabilities": [{"name_zh": "查看项目说明"}],
            "problems_solved": ["想知道这个收藏是做什么的"],
            "how_to_try": "打开仓库 README",
            "maturity": "usable",
            "zh_friendly": true,
            "confidence": 0.4
        })
        .to_string();
        let db = prepare("placeholder-vocab", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "legacy",
                    full_name: "lab/legacy-template",
                    description: "",
                    stars: 4,
                    status: "active",
                    one_liner: Some("占位"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: Some(&card),
                },
            );
        });
        let useful = run(&db, "模板");
        assert_eq!(useful.hits.len(), 1);
        assert_eq!(useful.hits[0].id, "legacy");
        assert!(!useful.hits[0].full_match);
        let boilerplate = run(&db, "想知道");
        assert!(
            boilerplate.hits.is_empty(),
            "占位卡的套话不该再被拿来匹配：{:?}",
            boilerplate.hits
        );
        assert!(boilerplate.extras.is_empty(), "{:?}", boilerplate.extras);
    }

    #[test]
    fn vocabulary_queries_rank_group_daily() {
        let subtitle = rich_card(
            "cli",
            &["剪切视频"],
            &["视频太长"],
            &["需要给视频添加字幕时"],
            "一个视频小工具",
        );
        let db = prepare("vocab-rank", |conn| {
            insert_repo(
                conn,
                &NewRepo {
                    id: "daily",
                    full_name: "fox/wechat-group-daily-群聊日报",
                    description: "group daily",
                    stars: 100,
                    status: "active",
                    one_liner: Some("把微信群聊整理成日报"),
                    summary: "群聊日报",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "hub",
                    full_name: "fox/wechat-intelligence-hub",
                    description: "intelligence",
                    stars: 80,
                    status: "active",
                    one_liner: Some("群聊和日报都能看"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "chat-only",
                    full_name: "fox/only-群聊",
                    description: "chat",
                    stars: 90,
                    status: "active",
                    one_liner: Some("只有群聊"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "daily-only",
                    full_name: "fox/only-日报",
                    description: "report",
                    stars: 90,
                    status: "active",
                    one_liner: Some("只有日报"),
                    summary: "",
                    keywords_json: "[]",
                    card_json: None,
                },
            );
            insert_repo(
                conn,
                &NewRepo {
                    id: "notes",
                    full_name: "lab/notes",
                    description: "保存记录",
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
                    id: "batch",
                    full_name: "lab/batch",
                    description: "支持批量",
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
                    id: "subtitle",
                    full_name: "lab/subtitle-tool",
                    description: "video tool",
                    stars: 5,
                    status: "active",
                    one_liner: Some("视频工具"),
                    summary: "简介",
                    keywords_json: "[]",
                    card_json: Some(&subtitle),
                },
            );
            insert_repo(
                conn,
                &readme_repo("min", "lab/image-minifier", "a general image utility"),
            );
            insert_readme(
                conn,
                "min",
                "Please compress this file. Then compress it again. Finally compress the backup.",
            );
        });

        let cases = [
            (
                "做小红书封面",
                vec!["小红书".to_owned(), "封面".to_owned()],
                Vec::<String>::new(),
            ),
            (
                "公众号文章配图",
                vec!["公众号".to_owned(), "配图".to_owned(), "文章".to_owned()],
                Vec::new(),
            ),
            (
                "给短视频加字幕",
                vec!["短视频".to_owned(), "字幕".to_owned()],
                Vec::new(),
            ),
            (
                "做电商商品详情页",
                vec!["详情页".to_owned(), "电商".to_owned()],
                Vec::new(),
            ),
            (
                "把群聊记录整理成日报",
                vec!["群聊".to_owned(), "日报".to_owned()],
                vec!["记录".to_owned(), "整理".to_owned()],
            ),
            (
                "批量改图尺寸和压缩",
                vec!["改图".to_owned(), "压缩".to_owned(), "尺寸".to_owned()],
                vec!["批量".to_owned()],
            ),
        ];
        let mut report = String::new();
        for (query, intents, extras) in &cases {
            let result = run(&db, query);
            report.push_str(&format!(
                "Q {query}\nintents={:?}\nextras={:?}\n",
                result.intents, result.extras
            ));
            assert_eq!(result.intents, *intents, "{query}");
            assert_eq!(result.extras, *extras, "{query}");
            for (index, hit) in result.hits.iter().take(3).enumerate() {
                report.push_str(&format!(
                    "  {}. {} matched={:?} full={} score={:.6}\n",
                    index + 1,
                    hit.full_name,
                    hit.matched,
                    hit.full_match,
                    hit.score
                ));
            }
        }
        let ranked = run(&db, "把群聊记录整理成日报");
        assert!(ranked.hits.len() >= 3, "{:?}", ranked.hits);
        assert_eq!(ranked.hits[0].id, "daily");
        assert_eq!(ranked.hits[1].id, "hub");
        assert!(ranked.hits[0].full_match);
        assert!(ranked.hits[1].full_match);
        assert!(has_term(&ranked.hits[0].matched, "群聊"));
        assert!(has_term(&ranked.hits[0].matched, "日报"));
        assert!(has_term(&ranked.hits[1].matched, "群聊"));
        assert!(has_term(&ranked.hits[1].matched, "日报"));
        assert!(ranked.hits[0].score > ranked.hits[1].score);
        assert!(!ranked.hits[2].full_match, "{:?}", ranked.hits[2]);
        assert!(ranked.hits[1].score > ranked.hits[2].score);
        fs::write("/tmp/fsl-27-vocab.txt", report).expect("应能写下词表对照");
    }
}
