use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::{Duration, Instant};

const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
const PROMPT_VERSION: &str = "readme-summary-v1";
const TAG_NETWORK_PROMPT_VERSION: &str = "tag-network-v1";
const MAX_README_CHARS: usize = 18_000;
pub(crate) const MAX_TAG_NETWORK_REPOSITORIES: usize = 80;
pub(crate) const AI_CONNECT_TIMEOUT_SECONDS: u16 = 15;
pub(crate) const AI_REQUEST_TIMEOUT_SECONDS: u16 = 120;
pub(crate) const AI_API_MAX_ATTEMPTS: usize = 3;
pub(crate) const AI_API_RETRY_BASE_DELAY_MS: u64 = 350;
const ANTHROPIC_SUMMARY_MAX_TOKENS: u16 = 1600;
const ANTHROPIC_TAG_NETWORK_MAX_TOKENS: u16 = 2400;
const ANTHROPIC_RECOMMENDATION_MAX_TOKENS: u16 = 1000;
const ANTHROPIC_SEARCH_PLAN_MAX_TOKENS: u16 = 800;
const ANTHROPIC_SEARCH_ANSWER_MAX_TOKENS: u16 = 1200;
const ANTHROPIC_EXPLANATION_MAX_TOKENS: u16 = 1000;
const ANTHROPIC_TRANSLATION_MAX_TOKENS: u16 = 6000;
/// 服务商因为「max_tokens 比这个模型允许的输出上限还大」拒绝了请求（HTTP 400 / 422）时，错误信息里的固定说法。
/// 理解卡流水线靠它认出「该把输出额度降一档重试」（`card_pipeline::complete_with_room`）；
/// 错误文案不回显响应体（可能带密钥），所以只能在看得到响应体的这里认。
pub(crate) const OUTPUT_LIMIT_REJECTED: &str = "输出长度（max_tokens）超过了这个模型允许的上限";

const AI_SYSTEM_PROMPT: &str =
    "你是开源项目知识库助手。严格遵守用户消息中的输出格式要求；需要结构化 JSON 时只输出 JSON，需要自然语言时直接回答。";

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequestConfig {
    pub provider: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingRequestConfig {
    pub enabled: bool,
    pub provider: String,
    #[serde(default)]
    pub download_source: Option<String>,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: String,
    pub dimensions: usize,
    pub min_score: f32,
    pub max_results: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingTestResult {
    pub model: String,
    pub dimensions: usize,
}

#[derive(Debug, Clone)]
pub struct AiSearchEvidence {
    pub repository_full_name: String,
    pub description: Option<String>,
    pub topics: Vec<String>,
    pub summary_zh: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSearchAnswer {
    pub answer_zh: String,
    pub repository_full_names: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRequestDiagnostics {
    pub provider: String,
    pub wire_protocol: String,
    pub base_url: Option<String>,
    pub endpoint: Option<String>,
    pub model: String,
    pub api_key_state: String,
    pub connect_timeout_seconds: u16,
    pub request_timeout_seconds: u16,
    pub model_list_timeout_seconds: u16,
    pub retry_attempts: usize,
    pub retry_base_delay_ms: u64,
    pub readme_prompt_char_limit: usize,
    pub tag_network_batch_size: usize,
    pub summary_prompt_version: String,
    pub tag_network_prompt_version: String,
    pub status: String,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryDocument {
    pub summary_zh: String,
    pub readme_zh: Option<String>,
    pub keywords: Vec<String>,
    pub suggested_tags: Vec<String>,
    pub model: String,
    pub prompt_version: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// README 中文翻译结果及其覆盖范围。
pub struct AiReadmeTranslation {
    pub markdown_zh: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub source_char_count: usize,
    pub translated_char_count: usize,
    pub is_truncated: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiModelOption {
    pub id: String,
    pub display_name: Option<String>,
    pub owned_by: Option<String>,
}

pub struct AiTagRepositoryBrief {
    pub full_name: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub topics: Vec<String>,
    pub ai_summary: Option<String>,
    pub suggested_tags: Vec<String>,
    pub stars_count: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTagNetworkSuggestion {
    pub tag_name: String,
    pub color: Option<String>,
    pub repository_full_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiGithubRecommendationPlan {
    pub rationale_zh: String,
    pub queries: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSearchPlan {
    pub search_query: String,
    pub keywords: Vec<String>,
    pub rationale_zh: String,
}

struct AiJsonDocument {
    summary_zh: Option<String>,
    readme_zh: Option<String>,
    keywords: Option<Vec<String>>,
    suggested_tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct AiTagNetworkJson {
    tags: Option<Vec<AiTagNetworkJsonTag>>,
}

#[derive(Deserialize)]
struct AiTagNetworkJsonTag {
    tag_name: Option<String>,
    color: Option<String>,
    repository_full_names: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct AiGithubRecommendationJson {
    rationale_zh: Option<String>,
    queries: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct AiSearchPlanJson {
    search_query: Option<String>,
    keywords: Option<Vec<String>>,
    rationale_zh: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
    /// 服务商回的真实用量。旧功能的额度闸门（`spend_guard`）用它记账，没有就按文字估。
    /// 按任意 JSON 读：有的兼容接口写成小数、字符串或 null，不能因此把一个好好的回复当成解析失败。
    #[serde(default)]
    usage: Option<serde_json::Value>,
}

/// 从 `usage` 对象里取（输入，输出）token 数；取不到或为 0 就返回 `None`（改按文字估）。
fn usage_from_json(
    usage: &serde_json::Value,
    input_key: &str,
    output_key: &str,
) -> Option<(u64, u64)> {
    let count = |key: &str| -> Option<u64> {
        let value = usage.get(key)?;
        if let Some(whole) = value.as_u64() {
            return Some(whole);
        }
        if let Some(float) = value.as_f64() {
            if float.is_finite() && float >= 0.0 {
                return Some(float as u64);
            }
            return None;
        }
        value.as_str()?.trim().parse::<u64>().ok()
    };
    usage_pair(count(input_key), count(output_key))
}

fn usage_pair(input: Option<u64>, output: Option<u64>) -> Option<(u64, u64)> {
    let pair = (input.unwrap_or(0), output.unwrap_or(0));
    if pair.0.saturating_add(pair.1) > 0 {
        Some(pair)
    } else {
        None
    }
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Deserialize)]
struct OpenAiMessage {
    content: Option<OpenAiMessageContent>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OpenAiMessageContent {
    Text(String),
    Parts(Vec<OpenAiMessageContentPart>),
}

#[derive(Deserialize)]
struct OpenAiMessageContentPart {
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicMessageResponse {
    content: Vec<AnthropicContentBlock>,
    #[serde(default)]
    usage: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiStreamResponse {
    choices: Option<Vec<OpenAiStreamChoice>>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    delta: Option<OpenAiStreamDelta>,
    message: Option<OpenAiMessage>,
}

#[derive(Deserialize)]
struct OpenAiStreamDelta {
    content: Option<OpenAiMessageContent>,
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModelRecord>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingRecord>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingRecord {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct OpenAiModelRecord {
    id: String,
    owned_by: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModelRecord>,
}

#[derive(Deserialize)]
struct AnthropicModelRecord {
    id: String,
    display_name: Option<String>,
}

struct HttpResponse {
    status_success: bool,
    http_code: Option<u16>,
    body: String,
}

struct JsonPostRequest {
    endpoint: String,
    headers: Vec<(&'static str, String)>,
    body: String,
}

pub type AiStreamCallback<'a> =
    dyn FnMut(&str, &str, Option<&str>, Option<&str>, Option<&str>) + 'a;

pub fn summarize_readme(
    config: &AiRequestConfig,
    repository_full_name: &str,
    description: Option<&str>,
    readme_markdown: &str,
) -> Result<AiSummaryDocument, String> {
    let provider = validate_request_config(config)?;

    let prompt = build_summary_prompt(repository_full_name, description, readme_markdown);
    let content = match provider.as_str() {
        "openai" => request_openai(config, &prompt)?,
        "anthropic" => request_anthropic(config, &prompt, ANTHROPIC_SUMMARY_MAX_TOKENS)?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };
    let document = parse_ai_document(&content)?;

    Ok(AiSummaryDocument {
        summary_zh: document.summary_zh.unwrap_or_default(),
        readme_zh: document.readme_zh,
        keywords: document.keywords.unwrap_or_default(),
        suggested_tags: document.suggested_tags.unwrap_or_default(),
        model: config.model.trim().to_owned(),
        prompt_version: PROMPT_VERSION.to_owned(),
        input_tokens: estimate_tokens(&prompt),
        output_tokens: estimate_tokens(&content),
    })
}

pub fn summarize_readme_streaming(
    config: &AiRequestConfig,
    repository_full_name: &str,
    description: Option<&str>,
    readme_markdown: &str,
    emit: &mut AiStreamCallback<'_>,
) -> Result<AiSummaryDocument, String> {
    let provider = validate_request_config(config)?;

    let prompt = build_summary_prompt(repository_full_name, description, readme_markdown);
    emit("summarize", "started", None, None, Some("正在连接 AI 服务"));
    let content = match provider.as_str() {
        "openai" => request_openai_streaming(config, &prompt, "summarize", emit)?,
        "anthropic" => request_anthropic_streaming(
            config,
            &prompt,
            ANTHROPIC_SUMMARY_MAX_TOKENS,
            "summarize",
            emit,
        )?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };
    emit(
        "summarize",
        "finished",
        None,
        Some(&content),
        Some("AI 输出完成，正在保存知识卡"),
    );
    let document = parse_ai_document(&content)?;

    Ok(AiSummaryDocument {
        summary_zh: document.summary_zh.unwrap_or_default(),
        readme_zh: document.readme_zh,
        keywords: document.keywords.unwrap_or_default(),
        suggested_tags: document.suggested_tags.unwrap_or_default(),
        model: config.model.trim().to_owned(),
        prompt_version: PROMPT_VERSION.to_owned(),
        input_tokens: estimate_tokens(&prompt),
        output_tokens: estimate_tokens(&content),
    })
}

/// 将 README 的说明文字翻译为中文，同时保留 Markdown 与技术内容。
pub fn translate_readme(
    config: &AiRequestConfig,
    repository_full_name: &str,
    readme_markdown: &str,
) -> Result<AiReadmeTranslation, String> {
    let provider = validate_request_config(config)?;
    let prompt = build_readme_translation_prompt(repository_full_name, readme_markdown);
    let content = match provider.as_str() {
        "openai" => request_openai(config, &prompt)?,
        "anthropic" => request_anthropic(config, &prompt, ANTHROPIC_TRANSLATION_MAX_TOKENS)?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };
    let markdown_zh = content
        .trim()
        .strip_prefix("```markdown")
        .and_then(|value| value.strip_suffix("```"))
        .unwrap_or(content.trim())
        .trim()
        .to_owned();
    if markdown_zh.is_empty() {
        return Err("AI 未返回可用的 README 中文翻译".to_owned());
    }

    let source_char_count = readme_markdown.chars().count();
    Ok(AiReadmeTranslation {
        markdown_zh,
        model: config.model.trim().to_owned(),
        input_tokens: estimate_tokens(&prompt),
        output_tokens: estimate_tokens(&content),
        source_char_count,
        translated_char_count: source_char_count.min(MAX_README_CHARS),
        is_truncated: source_char_count > MAX_README_CHARS,
    })
}

pub fn generate_tag_network(
    config: &AiRequestConfig,
    repositories: &[AiTagRepositoryBrief],
) -> Result<Vec<AiTagNetworkSuggestion>, String> {
    let provider = validate_request_config(config)?;
    if repositories.is_empty() {
        return Err("没有可用于生成标签网络的 Stars 仓库".to_owned());
    }

    let prompt = build_tag_network_prompt(repositories);
    let content = match provider.as_str() {
        "openai" => request_openai(config, &prompt)?,
        "anthropic" => request_anthropic(config, &prompt, ANTHROPIC_TAG_NETWORK_MAX_TOKENS)?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };

    parse_tag_network_document(&content, repositories)
}

pub fn plan_github_recommendations(
    config: &AiRequestConfig,
    repositories: &[AiTagRepositoryBrief],
) -> Result<AiGithubRecommendationPlan, String> {
    let provider = validate_request_config(config)?;
    if repositories.is_empty() {
        return Err("请先选择至少一个 Star 仓库".to_owned());
    }

    let prompt = build_github_recommendation_prompt(repositories);
    let content = match provider.as_str() {
        "openai" => request_openai(config, &prompt)?,
        "anthropic" => request_anthropic(config, &prompt, ANTHROPIC_RECOMMENDATION_MAX_TOKENS)?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };

    parse_github_recommendation_plan(&content)
}

#[allow(dead_code)]
pub fn plan_search_query(
    config: &AiRequestConfig,
    query: &str,
    context_queries: &[String],
) -> Result<AiSearchPlan, String> {
    let provider = validate_request_config(config)?;
    let normalized_query =
        normalize_text(Some(query)).ok_or_else(|| "请输入要搜索的自然语言问题".to_owned())?;
    let prompt = build_search_query_prompt(&normalized_query, context_queries);
    let content = match provider.as_str() {
        "openai" => request_openai(config, &prompt)?,
        "anthropic" => request_anthropic(config, &prompt, ANTHROPIC_SEARCH_PLAN_MAX_TOKENS)?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };

    parse_search_plan(&content, &normalized_query)
}

pub fn plan_search_query_streaming(
    config: &AiRequestConfig,
    query: &str,
    context_queries: &[String],
    emit: &mut AiStreamCallback<'_>,
) -> Result<AiSearchPlan, String> {
    let provider = validate_request_config(config)?;
    let normalized_query =
        normalize_text(Some(query)).ok_or_else(|| "请输入要搜索的自然语言问题".to_owned())?;
    let prompt = build_search_query_prompt(&normalized_query, context_queries);
    emit(
        "plan",
        "started",
        None,
        None,
        Some("正在理解问题并改写搜索方向"),
    );
    let content = match provider.as_str() {
        "openai" => request_openai_streaming(config, &prompt, "plan", emit)?,
        "anthropic" => request_anthropic_streaming(
            config,
            &prompt,
            ANTHROPIC_SEARCH_PLAN_MAX_TOKENS,
            "plan",
            emit,
        )?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };
    emit(
        "plan",
        "finished",
        None,
        Some(&content),
        Some("AI 已完成问题理解"),
    );

    parse_search_plan(&content, &normalized_query)
}

pub fn explain_search_topic_streaming(
    config: &AiRequestConfig,
    topic: &str,
    emit: &mut AiStreamCallback<'_>,
) -> Result<String, String> {
    let provider = validate_request_config(config)?;
    let normalized_topic =
        normalize_text(Some(topic)).ok_or_else(|| "请选择要解释的问题".to_owned())?;
    let prompt = build_search_explanation_prompt(&normalized_topic);
    emit("explain", "started", None, None, Some("正在组织解释内容"));
    let content = match provider.as_str() {
        "openai" => request_openai_streaming(config, &prompt, "explain", emit)?,
        "anthropic" => request_anthropic_streaming(
            config,
            &prompt,
            ANTHROPIC_EXPLANATION_MAX_TOKENS,
            "explain",
            emit,
        )?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };
    let normalized_content =
        normalize_text(Some(&content)).ok_or_else(|| "AI 没有返回可用解释内容".to_owned())?;
    emit(
        "explain",
        "finished",
        None,
        Some(&normalized_content),
        Some("解释完成"),
    );
    Ok(normalized_content)
}

pub fn answer_search_results(
    config: &AiRequestConfig,
    query: &str,
    evidence: &[AiSearchEvidence],
) -> Result<AiSearchAnswer, String> {
    let provider = validate_request_config(config)?;
    let normalized_query =
        normalize_text(Some(query)).ok_or_else(|| "请输入要回答的搜索问题".to_owned())?;
    if evidence.is_empty() {
        return Err("没有可用于生成回答的检索结果".to_owned());
    }
    let prompt = build_search_answer_prompt(&normalized_query, evidence);
    let content = match provider.as_str() {
        "openai" => request_openai(config, &prompt)?,
        "anthropic" => request_anthropic(config, &prompt, ANTHROPIC_SEARCH_ANSWER_MAX_TOKENS)?,
        _ => return Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    };
    parse_search_answer(&content, evidence)
}

pub fn list_models(config: &AiRequestConfig) -> Result<Vec<AiModelOption>, String> {
    let provider = validate_model_list_config(config)?;
    match provider.as_str() {
        "openai" => list_openai_models(config),
        "anthropic" => list_anthropic_models(config),
        _ => Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    }
}

pub fn diagnose_request_config(config: &AiRequestConfig) -> AiRequestDiagnostics {
    let provider = config.provider.trim().to_ascii_lowercase();
    let model = config.model.trim().to_owned();
    let base_url = config
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let validation = validate_request_config(config);
    let wire_protocol = match validation.as_deref() {
        Ok(protocol) => protocol.to_owned(),
        Err(_) if provider == "anthropic" => "anthropic".to_owned(),
        Err(_) if provider == "openai" || provider == "openai-compatible" => "openai".to_owned(),
        Err(_) => "unknown".to_owned(),
    };
    let endpoint = build_diagnostic_endpoint(&provider, base_url.as_deref());
    let api_key_state = build_api_key_state(&provider, base_url.as_deref(), config.api_key.trim());
    let (status, failure_reason) = match validation {
        Ok(_) => ("ready".to_owned(), None),
        Err(error) => ("invalid".to_owned(), Some(error)),
    };

    AiRequestDiagnostics {
        provider: if provider.is_empty() {
            "none".to_owned()
        } else {
            provider
        },
        wire_protocol,
        base_url,
        endpoint,
        model,
        api_key_state,
        connect_timeout_seconds: AI_CONNECT_TIMEOUT_SECONDS,
        request_timeout_seconds: AI_REQUEST_TIMEOUT_SECONDS,
        model_list_timeout_seconds: AI_CONNECT_TIMEOUT_SECONDS,
        retry_attempts: AI_API_MAX_ATTEMPTS,
        retry_base_delay_ms: AI_API_RETRY_BASE_DELAY_MS,
        readme_prompt_char_limit: MAX_README_CHARS,
        tag_network_batch_size: MAX_TAG_NETWORK_REPOSITORIES,
        summary_prompt_version: PROMPT_VERSION.to_owned(),
        tag_network_prompt_version: TAG_NETWORK_PROMPT_VERSION.to_owned(),
        status,
        failure_reason,
    }
}

fn build_diagnostic_endpoint(provider: &str, base_url: Option<&str>) -> Option<String> {
    match provider {
        "openai" => Some(build_endpoint(
            base_url,
            DEFAULT_OPENAI_BASE_URL,
            "chat/completions",
        )),
        "openai-compatible" => {
            base_url.map(|url| build_endpoint(Some(url), url, "chat/completions"))
        }
        "anthropic" => Some(build_endpoint(
            base_url,
            DEFAULT_ANTHROPIC_BASE_URL,
            "messages",
        )),
        _ => None,
    }
}

fn build_api_key_state(provider: &str, base_url: Option<&str>, api_key: &str) -> String {
    if provider == "none" || provider.is_empty() {
        return "not_required".to_owned();
    }
    if !api_key.is_empty() {
        return "present".to_owned();
    }
    if provider == "openai-compatible" && base_url.is_some_and(is_local_ai_base_url) {
        return "optional_for_local".to_owned();
    }

    "missing".to_owned()
}

pub fn embed_text(
    config: &EmbeddingRequestConfig,
    text: &str,
) -> Result<EmbeddingTestResultWithVector, String> {
    validate_embedding_config(config)?;
    let normalized_text = text.trim();
    if normalized_text.is_empty() {
        return Err("Embedding 输入内容不能为空".to_owned());
    }
    let mut headers = vec![("Content-Type", "application/json".to_owned())];
    if !config.api_key.trim().is_empty() {
        headers.insert(
            0,
            ("Authorization", format!("Bearer {}", config.api_key.trim())),
        );
    }
    let mut body = serde_json::json!({
        "model": config.model.trim(),
        "input": truncate_chars(normalized_text, 24_000),
        "encoding_format": "float"
    });
    body["dimensions"] = serde_json::json!(config.dimensions);
    let endpoint = build_endpoint(
        config.base_url.as_deref(),
        DEFAULT_OPENAI_BASE_URL,
        "embeddings",
    );
    let response = execute_json_post(&endpoint, &headers, &body.to_string())?;
    if !response.status_success {
        return Err(format_ai_http_error(
            "OpenAI Embeddings",
            response.http_code,
            &response.body,
        ));
    }
    let parsed = serde_json::from_str::<OpenAiEmbeddingResponse>(&response.body)
        .map_err(|error| format!("Embedding 响应解析失败：{error}"))?;
    let vector = parsed
        .data
        .into_iter()
        .next()
        .map(|record| record.embedding)
        .filter(|vector| !vector.is_empty())
        .ok_or_else(|| "Embedding 响应中没有向量数据".to_owned())?;
    if vector.len() != config.dimensions {
        return Err(format!(
            "Embedding 维度不匹配：配置为 {}，服务返回 {}",
            config.dimensions,
            vector.len()
        ));
    }
    if vector.iter().any(|value| !value.is_finite()) {
        return Err("Embedding 响应包含无效数值".to_owned());
    }
    Ok(EmbeddingTestResultWithVector { vector })
}

#[derive(Debug)]
pub struct EmbeddingTestResultWithVector {
    pub vector: Vec<f32>,
}

fn validate_embedding_config(config: &EmbeddingRequestConfig) -> Result<(), String> {
    if !config.enabled {
        return Err("向量检索尚未启用".to_owned());
    }
    let provider = config.provider.trim().to_ascii_lowercase();
    if !matches!(provider.as_str(), "openai" | "openai-compatible") {
        return Err("Embedding 仅支持 OpenAI 或 OpenAI 兼容接口".to_owned());
    }
    let base_url = config.base_url.as_deref().map(str::trim).unwrap_or("");
    if provider == "openai-compatible" && base_url.is_empty() {
        return Err("请填写 Embedding 服务的 OpenAI 兼容请求地址".to_owned());
    }
    if !base_url.is_empty() && !is_allowed_ai_base_url(base_url) {
        return Err(
            "Embedding 请求地址必须使用 https://；本机或局域网服务可以使用 http://。".to_owned(),
        );
    }
    if config.model.trim().is_empty() {
        return Err("请填写 Embedding 模型 ID".to_owned());
    }
    if !(1..=8192).contains(&config.dimensions) {
        return Err("Embedding 维度必须是 1 到 8192 的整数".to_owned());
    }
    if !config.min_score.is_finite() || !(0.0..=1.0).contains(&config.min_score) {
        return Err("向量相似度阈值必须在 0 到 1 之间".to_owned());
    }
    if !(1..=10).contains(&config.max_results) {
        return Err("精准检索结果数量必须在 1 到 10 之间".to_owned());
    }
    if config.api_key.trim().is_empty()
        && !(provider == "openai-compatible" && is_local_ai_base_url(base_url))
    {
        return Err("请填写 Embedding API Key".to_owned());
    }
    Ok(())
}

fn validate_request_config(config: &AiRequestConfig) -> Result<String, String> {
    let provider = config.provider.trim().to_ascii_lowercase();
    if provider == "none" || provider.is_empty() {
        return Err("请先在设置中配置 AI 服务".to_owned());
    }

    let base_url = config.base_url.as_deref().map(str::trim).unwrap_or("");
    if provider == "openai-compatible" && base_url.is_empty() {
        return Err("请填写 OpenAI 兼容接口的请求地址".to_owned());
    }
    if !base_url.is_empty() && !is_allowed_ai_base_url(base_url) {
        return Err(
            "AI 请求地址必须使用 https://；OpenAI 兼容接口的本机或局域网服务可以使用 http://。"
                .to_owned(),
        );
    }

    if config.model.trim().is_empty() {
        return Err("请先填写 AI 模型 ID".to_owned());
    }

    if config.api_key.trim().is_empty()
        && !(provider == "openai-compatible" && is_local_ai_base_url(base_url))
    {
        return Err("请先填写 AI API Key".to_owned());
    }

    match provider.as_str() {
        "openai" | "openai-compatible" => Ok("openai".to_owned()),
        "anthropic" => Ok("anthropic".to_owned()),
        _ => Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    }
}

fn validate_model_list_config(config: &AiRequestConfig) -> Result<String, String> {
    let provider = config.provider.trim().to_ascii_lowercase();
    if provider == "none" || provider.is_empty() {
        return Err("请先选择要获取模型列表的 AI 服务".to_owned());
    }

    let base_url = config.base_url.as_deref().map(str::trim).unwrap_or("");
    if provider == "openai-compatible" && base_url.is_empty() {
        return Err("请填写 OpenAI 兼容接口的请求地址".to_owned());
    }
    if !base_url.is_empty() && !is_allowed_ai_base_url(base_url) {
        return Err(
            "AI 请求地址必须使用 https://；OpenAI 兼容接口的本机或局域网服务可以使用 http://。"
                .to_owned(),
        );
    }

    if config.api_key.trim().is_empty()
        && !(provider == "openai-compatible" && is_local_ai_base_url(base_url))
    {
        return Err("请先填写 AI API Key 后再获取模型列表".to_owned());
    }

    match provider.as_str() {
        "openai" | "openai-compatible" => Ok("openai".to_owned()),
        "anthropic" => Ok("anthropic".to_owned()),
        _ => Err("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned()),
    }
}

fn is_allowed_ai_base_url(base_url: &str) -> bool {
    let normalized = base_url.trim().to_ascii_lowercase();
    if normalized.starts_with("https://") {
        return true;
    }

    if !normalized.starts_with("http://") {
        return false;
    }

    let host_with_port = normalized
        .trim_start_matches("http://")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    if host_with_port.is_empty() || host_with_port.contains('@') {
        return false;
    }

    let host = if let Some(rest) = host_with_port.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host_with_port.split(':').next().unwrap_or("")
    };

    is_local_ai_host(host) || is_private_network_ai_host(host)
}

fn is_local_ai_base_url(base_url: &str) -> bool {
    let normalized = base_url.trim().to_ascii_lowercase();
    let Some(host_with_port) = normalized
        .strip_prefix("http://")
        .or_else(|| normalized.strip_prefix("https://"))
        .and_then(|rest| rest.split(['/', '?', '#']).next())
    else {
        return false;
    };
    if host_with_port.is_empty() || host_with_port.contains('@') {
        return false;
    }

    let host = if let Some(rest) = host_with_port.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        host_with_port.split(':').next().unwrap_or("")
    };

    is_local_ai_host(host)
}

fn is_local_ai_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "0.0.0.0" | "::1")
}

fn is_private_network_ai_host(host: &str) -> bool {
    if host == "host.docker.internal" {
        return true;
    }

    let parts = host
        .split('.')
        .map(str::parse::<u8>)
        .collect::<Result<Vec<_>, _>>();
    let Ok(parts) = parts else {
        return false;
    };
    if parts.len() != 4 {
        return false;
    }

    parts[0] == 10
        || (parts[0] == 172 && (16..=31).contains(&parts[1]))
        || (parts[0] == 192 && parts[1] == 168)
}

/// 4xx 是服务商在处理之前就拒绝的（密钥 / 模型 / 额度 / 参数不对、限流）：不计费。
/// 5xx、408（代理等不到上游就回的超时）和别的状态不确定，当作可能计费。
fn status_may_be_billed(http_code: Option<u16>) -> bool {
    !matches!(http_code, Some(400..=407 | 409..=499))
}

/// 请求没有到服务商那里：连接没建起来（拒绝连接、解析不了域名、握手失败），
/// 或者在建立连接的时限内就超时了——后一种在没有 `is_connect` 标记的网络栈上也算没发出去。
/// 别的传输错误（读响应时断了、等响应超时）请求已经发出去了，服务商可能已经开始计费。
fn transport_error_is_unsent(is_connect: bool, is_timeout: bool, elapsed: Duration) -> bool {
    is_connect
        || (is_timeout && elapsed <= Duration::from_secs(u64::from(AI_CONNECT_TIMEOUT_SECONDS) + 1))
}

/// 几秒内就失败的请求（5xx、连接被断开）是网关 / 过载保护在模型开始跑之前挡下的：没有计费。
/// 等了一阵才失败（网关超时、生成到一半断了、等响应超时）才可能已经计费。
/// 服务商故障时（持续 503）一批请求会连着快速失败，不能把每一次都记成花了钱，把额度锁 24 小时。
const FAST_FAILURE_SECONDS: u64 = 10;

#[cfg(test)]
thread_local! {
    /// 测试里假服务马上就回；要模拟「等了很久才失败」就把这个时限改成 0。
    static FAST_FAILURE_LIMIT: std::cell::Cell<Option<Duration>> =
        const { std::cell::Cell::new(None) };
}

fn fast_failure_limit() -> Duration {
    #[cfg(test)]
    if let Some(limit) = FAST_FAILURE_LIMIT.with(std::cell::Cell::get) {
        return limit;
    }
    Duration::from_secs(FAST_FAILURE_SECONDS)
}

/// 服务商回了一个失败状态：这次尝试可能已经计费吗？
fn response_may_be_billed(http_code: Option<u16>, answered_after: Duration) -> bool {
    status_may_be_billed(http_code) && answered_after >= fast_failure_limit()
}

/// 请求根本没发出去：连接没建起来，或者连请求都没拼成（密钥里夹了控制字符、地址写错了）。
fn request_error_is_unsent(error: &reqwest::Error, elapsed: Duration) -> bool {
    error.is_builder() || transport_error_is_unsent(error.is_connect(), error.is_timeout(), elapsed)
}

/// 传输层出错：这次尝试可能已经计费吗？没发出去的、几秒内就断开的都不会。
fn request_error_may_be_billed(error: &reqwest::Error, elapsed: Duration) -> bool {
    !request_error_is_unsent(error, elapsed) && elapsed >= fast_failure_limit()
}

fn request_openai(config: &AiRequestConfig, prompt: &str) -> Result<String, String> {
    // 旧功能不走队列，和队列共用每日额度：发请求前问一句，成功后记账（见 spend_guard.rs）。
    let _permit = crate::spend_guard::check_before_request(prompt)?;
    let request = build_openai_chat_request(config, prompt, false);
    let response = execute_json_post(&request.endpoint, &request.headers, &request.body)?;

    if !response.status_success {
        return Err(format_ai_http_error(
            "OpenAI",
            response.http_code,
            &response.body,
        ));
    }

    let parsed = match serde_json::from_str::<OpenAiChatResponse>(&response.body) {
        Ok(parsed) => parsed,
        Err(error) => {
            // 请求已经成功，服务商可能已经计费，读不懂回复也要记一笔
            crate::spend_guard::record_after_request(
                &config.provider,
                &config.model,
                prompt,
                "",
                None,
            );
            return Err(format!("OpenAI 响应解析失败：{error}"));
        }
    };
    let usage = parsed
        .usage
        .as_ref()
        .and_then(|value| usage_from_json(value, "prompt_tokens", "completion_tokens"));
    let content = parsed
        .choices
        .into_iter()
        .filter_map(|choice| choice.message.content.and_then(openai_content_to_text))
        .find(|content| !content.trim().is_empty());
    crate::spend_guard::record_after_request(
        &config.provider,
        &config.model,
        prompt,
        content.as_deref().unwrap_or(""),
        usage,
    );
    content.ok_or_else(|| "OpenAI 响应中没有摘要内容".to_owned())
}

fn request_openai_streaming(
    config: &AiRequestConfig,
    prompt: &str,
    stage: &str,
    emit: &mut AiStreamCallback<'_>,
) -> Result<String, String> {
    let _permit = crate::spend_guard::check_before_request(prompt)?;
    let request = build_openai_chat_request(config, prompt, true);
    match execute_sse_post(
        &request.endpoint,
        &request.headers,
        &request.body,
        |data, full_text| {
            if let Some(delta) = extract_openai_stream_delta(data)? {
                full_text.push_str(&delta);
                emit(stage, "delta", Some(&delta), Some(full_text), None);
            }
            Ok(())
        },
    ) {
        Ok(content) if !content.trim().is_empty() => {
            // 流式回复里没有用量，按文字估
            crate::spend_guard::record_after_request(
                &config.provider,
                &config.model,
                prompt,
                &content,
                None,
            );
            Ok(content)
        }
        Ok(_) => {
            crate::spend_guard::record_after_request(
                &config.provider,
                &config.model,
                prompt,
                "",
                None,
            );
            Err("OpenAI 流式响应中没有摘要内容".to_owned())
        }
        Err(stream_error) => {
            // 流式那次可能已经计费，回退的普通请求再花一次：两次都要记
            crate::spend_guard::account_failed_stream();
            emit(
                stage,
                "fallback",
                None,
                None,
                Some("当前服务未返回可用的流式响应，正在改用普通请求。"),
            );
            request_openai(config, prompt).map_err(|fallback_error| {
                format!("{stream_error}；普通请求也失败：{fallback_error}")
            })
        }
    }
}

fn request_anthropic(
    config: &AiRequestConfig,
    prompt: &str,
    max_tokens: u16,
) -> Result<String, String> {
    let _permit = crate::spend_guard::check_before_request(prompt)?;
    let request = build_anthropic_message_request(config, prompt, max_tokens, false);
    let response = execute_json_post(&request.endpoint, &request.headers, &request.body)?;

    if !response.status_success {
        return Err(format_ai_http_error(
            "Anthropic",
            response.http_code,
            &response.body,
        ));
    }

    let parsed = match serde_json::from_str::<AnthropicMessageResponse>(&response.body) {
        Ok(parsed) => parsed,
        Err(error) => {
            crate::spend_guard::record_after_request(
                &config.provider,
                &config.model,
                prompt,
                "",
                None,
            );
            return Err(format!("Anthropic 响应解析失败：{error}"));
        }
    };
    let usage = parsed
        .usage
        .as_ref()
        .and_then(|value| usage_from_json(value, "input_tokens", "output_tokens"));
    let content = parsed
        .content
        .into_iter()
        .filter_map(|block| normalize_text(block.text.as_deref()))
        .collect::<Vec<_>>()
        .join("\n");
    let content = normalize_text(Some(&content));
    crate::spend_guard::record_after_request(
        &config.provider,
        &config.model,
        prompt,
        content.as_deref().unwrap_or(""),
        usage,
    );
    content.ok_or_else(|| "Anthropic 响应中没有摘要内容".to_owned())
}

fn request_anthropic_streaming(
    config: &AiRequestConfig,
    prompt: &str,
    max_tokens: u16,
    stage: &str,
    emit: &mut AiStreamCallback<'_>,
) -> Result<String, String> {
    let _permit = crate::spend_guard::check_before_request(prompt)?;
    let request = build_anthropic_message_request(config, prompt, max_tokens, true);
    match execute_sse_post(
        &request.endpoint,
        &request.headers,
        &request.body,
        |data, full_text| {
            if let Some(delta) = extract_anthropic_stream_delta(data)? {
                full_text.push_str(&delta);
                emit(stage, "delta", Some(&delta), Some(full_text), None);
            }
            Ok(())
        },
    ) {
        Ok(content) if !content.trim().is_empty() => {
            crate::spend_guard::record_after_request(
                &config.provider,
                &config.model,
                prompt,
                &content,
                None,
            );
            Ok(content)
        }
        Ok(_) => {
            crate::spend_guard::record_after_request(
                &config.provider,
                &config.model,
                prompt,
                "",
                None,
            );
            Err("Anthropic 流式响应中没有摘要内容".to_owned())
        }
        Err(stream_error) => {
            crate::spend_guard::account_failed_stream();
            emit(
                stage,
                "fallback",
                None,
                None,
                Some("当前服务未返回可用的流式响应，正在改用普通请求。"),
            );
            request_anthropic(config, prompt, max_tokens).map_err(|fallback_error| {
                format!("{stream_error}；普通请求也失败：{fallback_error}")
            })
        }
    }
}

fn list_openai_models(config: &AiRequestConfig) -> Result<Vec<AiModelOption>, String> {
    let mut headers = Vec::new();
    let api_key = config.api_key.trim();
    if !api_key.is_empty() {
        headers.push(("Authorization", format!("Bearer {api_key}")));
    }

    let endpoint = build_endpoint(
        config.base_url.as_deref(),
        DEFAULT_OPENAI_BASE_URL,
        "models",
    );
    let response = execute_json_get(&endpoint, &headers)?;
    if !response.status_success {
        return Err(format_ai_http_error(
            "OpenAI",
            response.http_code,
            &response.body,
        ));
    }

    let parsed = serde_json::from_str::<OpenAiModelsResponse>(&response.body)
        .map_err(|error| format!("OpenAI 模型列表解析失败：{error}"))?;
    Ok(parsed
        .data
        .into_iter()
        .filter_map(|model| {
            normalize_text(Some(&model.id)).map(|id| AiModelOption {
                id,
                display_name: None,
                owned_by: model.owned_by,
            })
        })
        .collect())
}

fn list_anthropic_models(config: &AiRequestConfig) -> Result<Vec<AiModelOption>, String> {
    let endpoint = build_endpoint(
        config.base_url.as_deref(),
        DEFAULT_ANTHROPIC_BASE_URL,
        "models",
    );
    let response = execute_json_get(
        &endpoint,
        &[
            ("x-api-key", config.api_key.trim().to_owned()),
            ("anthropic-version", "2023-06-01".to_owned()),
        ],
    )?;
    if !response.status_success {
        return Err(format_ai_http_error(
            "Anthropic",
            response.http_code,
            &response.body,
        ));
    }

    let parsed = serde_json::from_str::<AnthropicModelsResponse>(&response.body)
        .map_err(|error| format!("Anthropic 模型列表解析失败：{error}"))?;
    Ok(parsed
        .data
        .into_iter()
        .filter_map(|model| {
            normalize_text(Some(&model.id)).map(|id| AiModelOption {
                id,
                display_name: normalize_text(model.display_name.as_deref()),
                owned_by: None,
            })
        })
        .collect())
}

fn build_openai_chat_request(
    config: &AiRequestConfig,
    prompt: &str,
    stream: bool,
) -> JsonPostRequest {
    let mut headers = vec![("Content-Type", "application/json".to_owned())];
    let api_key = config.api_key.trim();
    if !api_key.is_empty() {
        headers.insert(0, ("Authorization", format!("Bearer {api_key}")));
    }

    JsonPostRequest {
        endpoint: build_endpoint(
            config.base_url.as_deref(),
            DEFAULT_OPENAI_BASE_URL,
            "chat/completions",
        ),
        headers,
        body: serde_json::json!({
            "model": config.model.trim(),
            "temperature": 0.2,
            "stream": stream,
            "messages": [
                {
                    "role": "system",
                    "content": AI_SYSTEM_PROMPT
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        })
        .to_string(),
    }
}

fn build_anthropic_message_request(
    config: &AiRequestConfig,
    prompt: &str,
    max_tokens: u16,
    stream: bool,
) -> JsonPostRequest {
    JsonPostRequest {
        endpoint: build_endpoint(
            config.base_url.as_deref(),
            DEFAULT_ANTHROPIC_BASE_URL,
            "messages",
        ),
        headers: vec![
            ("x-api-key", config.api_key.trim().to_owned()),
            ("anthropic-version", "2023-06-01".to_owned()),
            ("Content-Type", "application/json".to_owned()),
        ],
        body: serde_json::json!({
            "model": config.model.trim(),
            "max_tokens": max_tokens,
            "temperature": 0.2,
            "stream": stream,
            "system": AI_SYSTEM_PROMPT,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        })
        .to_string(),
    }
}

fn build_summary_prompt(
    repository_full_name: &str,
    description: Option<&str>,
    readme_markdown: &str,
) -> String {
    let readme_excerpt = readme_markdown
        .chars()
        .take(MAX_README_CHARS)
        .collect::<String>();
    let truncated_notice = if readme_markdown.chars().count() > MAX_README_CHARS {
        "README 内容较长，以下内容已截取开头部分。请基于已提供内容总结，不要编造未出现的信息。"
    } else {
        "README 内容完整。"
    };
    format!(
        r#"请分析这个 GitHub 仓库 README，生成结构化中文知识卡片。

仓库：{repository_full_name}
描述：{description}
内容状态：{truncated_notice}

输出必须是严格 JSON，对象字段如下：
- summary_zh: 80 到 160 字中文摘要，说明项目解决什么问题、适合什么场景。
- readme_zh: 可选，README 核心内容中文梳理，200 到 500 字。
- keywords: 5 到 10 个关键词，数组。
- suggested_tags: 3 到 6 个适合知识库管理的短标签，数组。

README:
{readme_excerpt}
"#,
        description = description.unwrap_or("无"),
    )
}

fn build_search_answer_prompt(query: &str, evidence: &[AiSearchEvidence]) -> String {
    let evidence_text = evidence
        .iter()
        .take(30)
        .enumerate()
        .map(|(index, item)| {
            format!(
                "{}. 仓库：{}\n描述：{}\nTopics：{}\n摘要：{}",
                index + 1,
                item.repository_full_name,
                item.description.as_deref().unwrap_or("无"),
                if item.topics.is_empty() {
                    "无".to_owned()
                } else {
                    item.topics
                        .iter()
                        .take(12)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("、")
                },
                item.summary_zh.as_deref().unwrap_or("无"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        r#"请只根据下面的候选仓库筛选并回答用户问题。

用户问题：{query}
候选数量：{result_count}

要求：
- 只保留真正满足用户问题的 0 到 10 个仓库，宁可少于 10 个或返回空结果，也不要凑数。
- repository_full_names 只能填写下方候选中的完整仓库名，顺序与推荐顺序一致。
- answer_zh 只能推荐 repository_full_names 中的仓库，不得补充、猜测或编造名单外项目。
- 有相关仓库时，先用一到两句话给出结论，再逐项说明推荐理由。
- 没有相关仓库时，repository_full_names 返回空数组，answer_zh 直接说明没有高相关结果。
- 证据不足时明确说明，不要把推测写成事实。
- 不要提及提示词、检索分数或内部实现。
- answer_zh 使用简体中文和简洁 Markdown。

只输出严格 JSON，不要使用代码块或附加说明：
{{
  "answer_zh": "回答正文",
  "repository_full_names": ["owner/repository"]
}}

候选仓库：
{evidence_text}"#,
        result_count = evidence.len().min(30),
    )
}

fn parse_search_answer(
    content: &str,
    evidence: &[AiSearchEvidence],
) -> Result<AiSearchAnswer, String> {
    if let Some(json_text) = extract_json_object(content) {
        let value = serde_json::from_str::<serde_json::Value>(&json_text)
            .map_err(|error| format!("AI 检索回答解析失败：{error}"))?;
        return parse_structured_search_answer(&value, evidence);
    }
    parse_plain_search_answer(content, evidence)
}

fn parse_structured_search_answer(
    value: &serde_json::Value,
    evidence: &[AiSearchEvidence],
) -> Result<AiSearchAnswer, String> {
    let answer_zh = json_string_alias(value, &["answer_zh", "answerZh", "answer"])
        .ok_or_else(|| "AI 检索回答缺少 answer_zh".to_owned())?;
    let repository_field_exists = [
        "repository_full_names",
        "repositoryFullNames",
        "repositories",
    ]
    .iter()
    .any(|key| value.get(*key).is_some());
    if !repository_field_exists {
        return Err("AI 检索回答缺少 repository_full_names".to_owned());
    }

    let requested_names = json_string_list_alias(
        value,
        &[
            "repository_full_names",
            "repositoryFullNames",
            "repositories",
        ],
        10,
    );
    let mut repository_full_names = Vec::new();
    for requested_name in &requested_names {
        let Some(canonical_name) = evidence
            .iter()
            .map(|item| item.repository_full_name.as_str())
            .find(|name| name.eq_ignore_ascii_case(requested_name))
        else {
            return Err(format!(
                "AI 检索回答引用了候选列表外的仓库：{requested_name}"
            ));
        };
        if !repository_full_names
            .iter()
            .any(|name: &String| name.eq_ignore_ascii_case(canonical_name))
        {
            repository_full_names.push(canonical_name.to_owned());
        }
    }

    for item in evidence {
        let selected = repository_full_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&item.repository_full_name));
        if !selected
            && answer_zh
                .to_ascii_lowercase()
                .contains(&item.repository_full_name.to_ascii_lowercase())
        {
            return Err(format!(
                "AI 检索回答正文引用了未选中的仓库：{}",
                item.repository_full_name
            ));
        }
    }

    Ok(AiSearchAnswer {
        answer_zh,
        repository_full_names,
    })
}

fn parse_plain_search_answer(
    content: &str,
    evidence: &[AiSearchEvidence],
) -> Result<AiSearchAnswer, String> {
    let answer_zh =
        normalize_text(Some(content)).ok_or_else(|| "AI 没有返回可用的检索回答".to_owned())?;
    let lower_answer = answer_zh.to_ascii_lowercase();
    let mut matched = evidence
        .iter()
        .filter_map(|item| {
            let full_name = item.repository_full_name.to_ascii_lowercase();
            let repository_name = full_name.rsplit_once('/').map(|(_, name)| name)?;
            lower_answer
                .find(&full_name)
                .or_else(|| {
                    (repository_name.len() >= 4)
                        .then(|| lower_answer.find(repository_name))
                        .flatten()
                })
                .map(|position| (position, item.repository_full_name.clone()))
        })
        .collect::<Vec<_>>();
    matched.sort_by_key(|(position, _)| *position);
    let repository_full_names =
        matched
            .into_iter()
            .map(|(_, name)| name)
            .fold(Vec::new(), |mut names, name| {
                if !names
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(&name))
                    && names.len() < 10
                {
                    names.push(name);
                }
                names
            });
    if repository_full_names.is_empty() && !plain_answer_indicates_no_match(&lower_answer) {
        return Err("AI 检索回答没有明确引用候选池中的仓库".to_owned());
    }

    Ok(AiSearchAnswer {
        answer_zh,
        repository_full_names,
    })
}

fn plain_answer_indicates_no_match(lower_answer: &str) -> bool {
    [
        "没有找到",
        "没有高相关",
        "无高相关",
        "没有符合",
        "未找到",
        "no relevant",
        "no matching",
    ]
    .iter()
    .any(|phrase| lower_answer.contains(phrase))
}

fn build_readme_translation_prompt(repository_full_name: &str, readme_markdown: &str) -> String {
    let readme_excerpt = readme_markdown
        .chars()
        .take(MAX_README_CHARS)
        .collect::<String>();
    let content_notice = if readme_markdown.chars().count() > MAX_README_CHARS {
        "README 内容较长，本次翻译仅覆盖开头部分。不要补写未提供的章节。"
    } else {
        "README 内容完整。"
    };

    format!(
        r#"请将这个 GitHub 仓库 README 翻译为简体中文。

仓库：{repository_full_name}
内容状态：{content_notice}

要求：
- 保留原有 Markdown 结构，包括标题层级、列表、表格、引用、链接和图片。
- 代码块、命令、URL、API 名称和标识符保持原文，不要翻译或改写。
- 只翻译自然语言说明，不添加原文没有的功能、结论或示例。
- 直接输出翻译后的 Markdown，不要使用代码围栏包裹全文，不要添加解释。

README:
{readme_excerpt}
"#,
    )
}

fn build_tag_network_prompt(repositories: &[AiTagRepositoryBrief]) -> String {
    let repository_lines = repositories
        .iter()
        .take(MAX_TAG_NETWORK_REPOSITORIES)
        .map(|repository| {
            format!(
                "- full_name: {full_name}\n  language: {language}\n  stars: {stars_count}\n  topics: {topics}\n  ai_suggested_tags: {suggested_tags}\n  ai_summary: {ai_summary}\n  description: {description}",
                full_name = repository.full_name,
                language = repository.language.as_deref().unwrap_or("未知"),
                stars_count = repository.stars_count,
                topics = repository.topics.join(", "),
                suggested_tags = repository.suggested_tags.join(", "),
                ai_summary = truncate_chars(repository.ai_summary.as_deref().unwrap_or(""), 120),
                description = truncate_chars(repository.description.as_deref().unwrap_or(""), 120),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let truncated_notice = if repositories.len() > MAX_TAG_NETWORK_REPOSITORIES {
        format!(
            "仓库数量较多，本次只发送 Star 数和最近收藏排序靠前的 {MAX_TAG_NETWORK_REPOSITORIES} 个仓库。"
        )
    } else {
        "已发送本批可用仓库简略信息。".to_owned()
    };

    format!(
        r##"请根据 GitHub Stars 仓库简略信息生成知识库标签网络建议。
目标：
- 生成 8 到 24 个中文短标签，覆盖技术领域、使用场景、架构类型、开发工具、AI 能力等维度。
- 每个标签绑定 1 到 30 个最相关仓库，repository_full_names 必须来自输入的 full_name，不能编造仓库。
- 标签名要适合长期知识库管理，不要使用“工具”“项目”“开源”等过泛标签。
- color 可选，若给出必须是 #RRGGBB。

输出必须是严格 JSON，对象字段如下：
{{
  "tags": [
    {{
      "tag_name": "React 动画",
      "color": "#ec4899",
      "repository_full_names": ["owner/repo"]
    }}
  ]
}}

内容状态：{truncated_notice}
提示版本：{TAG_NETWORK_PROMPT_VERSION}

仓库：
{repository_lines}
"##
    )
}

fn build_github_recommendation_prompt(repositories: &[AiTagRepositoryBrief]) -> String {
    let repository_lines = repositories
        .iter()
        .take(8)
        .map(|repository| {
            format!(
                "- full_name: {full_name}\n  language: {language}\n  stars: {stars_count}\n  topics: {topics}\n  ai_suggested_tags: {suggested_tags}\n  ai_summary: {ai_summary}\n  description: {description}",
                full_name = repository.full_name,
                language = repository.language.as_deref().unwrap_or("未知"),
                stars_count = repository.stars_count,
                topics = repository.topics.join(", "),
                suggested_tags = repository.suggested_tags.join(", "),
                ai_summary = truncate_chars(repository.ai_summary.as_deref().unwrap_or(""), 180),
                description = truncate_chars(repository.description.as_deref().unwrap_or(""), 180),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r##"请根据用户选择的 GitHub Stars 仓库，生成用于 GitHub Search API 的相似/更优项目搜索策略。
目标：
- 给出 2 到 5 条 GitHub repository search 查询语句。
- 查询语句要能找到功能相似、维护更活跃、Star 更多或工程质量更高的项目。
- 查询语句必须简短，可以包含 language:、stars:>、topic: 等 GitHub 搜索限定符。
- 不要包含已给出的仓库 full_name。

输出必须是严格 JSON：
{{
  "rationale_zh": "一句中文说明推荐依据",
  "queries": ["react animation library stars:>1000 language:TypeScript"]
}}

用户选择的仓库：
{repository_lines}
"##
    )
}

fn build_search_query_prompt(query: &str, context_queries: &[String]) -> String {
    let context = context_queries
        .iter()
        .rev()
        .take(4)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n- ");
    let context_block = if context.is_empty() {
        "无".to_owned()
    } else {
        format!("- {context}")
    };

    format!(
        r#"请把用户对 GitHub Stars 知识库的自然语言问题改写成适合本地知识检索的短查询。
检索范围包括：仓库名称、描述、语言、Topics、README 摘要、AI 关键词、建议标签、个人标签和笔记。

要求：
- search_query 只保留核心功能、技术栈、场景词，适合 substring/token 本地召回。
- keywords 给出 3 到 8 个可参与召回的中英文关键词。
- 若本轮上下文能帮助理解当前问题，请合并上下文中的重要约束。
- 不要编造仓库名，不要输出 Markdown。

输出严格 JSON：
{{
  "search_query": "react animation transition spring",
  "keywords": ["React", "动画", "transition", "spring"],
  "rationale_zh": "一句中文说明如何理解用户意图"
}}

本轮上下文：
{context_block}

当前问题：
{query}
"#
    )
}

fn parse_ai_document(content: &str) -> Result<AiJsonDocument, String> {
    let json_text = extract_json_object(content).unwrap_or_else(|| content.trim().to_owned());
    let parsed = match serde_json::from_str::<serde_json::Value>(&json_text) {
        Ok(value) if value.is_object() => value,
        Ok(_) => {
            let fallback = normalize_plain_text_summary_fallback(content)?;
            return Ok(AiJsonDocument {
                summary_zh: Some(fallback),
                readme_zh: None,
                keywords: Some(Vec::new()),
                suggested_tags: Some(Vec::new()),
            });
        }
        Err(_) => {
            let fallback = normalize_plain_text_summary_fallback(content)?;
            return Ok(AiJsonDocument {
                summary_zh: Some(fallback),
                readme_zh: None,
                keywords: Some(Vec::new()),
                suggested_tags: Some(Vec::new()),
            });
        }
    };
    let summary = json_string_alias(
        &parsed,
        &[
            "summary_zh",
            "summaryZh",
            "summary",
            "summary_cn",
            "summaryCn",
            "overview",
            "description_zh",
            "descriptionZh",
            "description",
            "摘要",
            "中文摘要",
            "项目摘要",
        ],
    )
    .ok_or_else(|| "AI 摘要缺少 summary_zh 字段".to_owned())?;

    Ok(AiJsonDocument {
        summary_zh: Some(summary),
        readme_zh: json_string_alias(
            &parsed,
            &[
                "readme_zh",
                "readmeZh",
                "readme_summary",
                "readmeSummary",
                "readme",
                "details_zh",
                "detailsZh",
                "details",
                "README梳理",
            ],
        ),
        keywords: Some(json_string_list_alias(
            &parsed,
            &["keywords", "keyword", "key_words", "keyWords", "关键词"],
            10,
        )),
        suggested_tags: Some(json_string_list_alias(
            &parsed,
            &[
                "suggested_tags",
                "suggestedTags",
                "tags",
                "tag_list",
                "tagList",
                "建议标签",
            ],
            8,
        )),
    })
}

fn json_string_alias(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|field| match field {
            serde_json::Value::String(text) => normalize_text(Some(text)),
            _ => None,
        })
}

fn json_string_list_alias(value: &serde_json::Value, keys: &[&str], limit: usize) -> Vec<String> {
    let Some(field) = keys.iter().filter_map(|key| value.get(*key)).next() else {
        return Vec::new();
    };

    let items = match field {
        serde_json::Value::Array(values) => values
            .iter()
            .filter_map(|item| item.as_str().and_then(|text| normalize_text(Some(text))))
            .collect::<Vec<_>>(),
        serde_json::Value::String(text) => text
            .split([',', '，', ';', '；', '、', '\n'])
            .filter_map(|item| normalize_text(Some(item)))
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    normalize_list(items, limit)
}

fn normalize_plain_text_summary_fallback(content: &str) -> Result<String, String> {
    let fallback = normalize_text(Some(content))
        .map(|value| truncate_chars(&value, 180))
        .ok_or_else(|| "AI 返回内容为空，无法生成摘要".to_owned())?;
    let normalized = fallback.to_ascii_lowercase();
    if fallback.contains('{')
        || fallback.contains('}')
        || fallback.contains("```")
        || normalized.contains("summary_zh")
        || normalized.contains("readme_zh")
        || normalized.contains("keywords")
        || normalized.contains("suggested_tags")
    {
        return Err("AI 返回内容不是可解析的结构化 JSON，请重试或更换模型。".to_owned());
    }

    Ok(fallback)
}

fn parse_tag_network_document(
    content: &str,
    repositories: &[AiTagRepositoryBrief],
) -> Result<Vec<AiTagNetworkSuggestion>, String> {
    let known_repositories = repositories
        .iter()
        .map(|repository| {
            (
                normalize_repository_full_name_lookup_key(&repository.full_name),
                repository.full_name.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    let json_text = extract_json_object(content).unwrap_or_else(|| content.trim().to_owned());
    let parsed = serde_json::from_str::<AiTagNetworkJson>(&json_text)
        .map_err(|error| format!("AI 标签网络响应解析失败：{error}"))?;
    let mut suggestions = Vec::new();

    for tag in parsed.tags.unwrap_or_default() {
        let Some(tag_name) = normalize_text(tag.tag_name.as_deref()) else {
            continue;
        };
        let mut seen_repository_keys = HashSet::new();
        let repository_full_names =
            normalize_list(tag.repository_full_names.unwrap_or_default(), 30)
                .into_iter()
                .filter_map(|full_name| {
                    let lookup_key = normalize_repository_full_name_lookup_key(&full_name);
                    if lookup_key.is_empty() || !seen_repository_keys.insert(lookup_key.clone()) {
                        return None;
                    }
                    known_repositories.get(&lookup_key).cloned()
                })
                .collect::<Vec<_>>();

        if repository_full_names.is_empty() {
            continue;
        }

        suggestions.push(AiTagNetworkSuggestion {
            tag_name,
            color: normalize_color(tag.color.as_deref()),
            repository_full_names,
        });
        if suggestions.len() >= 24 {
            break;
        }
    }

    if suggestions.is_empty() {
        Err("AI 未返回可用的标签网络建议".to_owned())
    } else {
        Ok(suggestions)
    }
}

fn parse_github_recommendation_plan(content: &str) -> Result<AiGithubRecommendationPlan, String> {
    let json_text = extract_json_object(content).unwrap_or_else(|| content.trim().to_owned());
    let parsed = serde_json::from_str::<AiGithubRecommendationJson>(&json_text)
        .map_err(|error| format!("AI GitHub 推荐策略解析失败：{error}"))?;
    let queries = normalize_list(parsed.queries.unwrap_or_default(), 5)
        .into_iter()
        .filter(|query| query.len() <= 180)
        .collect::<Vec<_>>();

    if queries.is_empty() {
        return Err("AI 未返回可用的 GitHub 搜索查询".to_owned());
    }

    Ok(AiGithubRecommendationPlan {
        rationale_zh: normalize_text(parsed.rationale_zh.as_deref())
            .unwrap_or_else(|| "根据所选仓库的功能、语言、Topics 和摘要寻找相似项目。".to_owned()),
        queries,
    })
}

fn normalize_repository_full_name_lookup_key(value: &str) -> String {
    let trimmed = value.trim().trim_matches(|character: char| {
        character.is_whitespace() || character == '`' || character == '"' || character == '\''
    });
    let lower_trimmed = trimmed.to_ascii_lowercase();
    let without_host = lower_trimmed
        .find("github.com/")
        .map(|index| &trimmed[index + "github.com/".len()..])
        .unwrap_or(trimmed);
    let without_suffix = without_host.trim_end_matches('/').trim_end_matches(".git");
    let parts = without_suffix
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<_>>();

    if parts.len() >= 2 {
        format!("{}/{}", parts[0], parts[1]).to_ascii_lowercase()
    } else {
        without_suffix.to_ascii_lowercase()
    }
}

fn build_search_explanation_prompt(topic: &str) -> String {
    format!(
        r#"你是 Fox Stars Lab 的产品向导，请用简体中文直接回答用户点击的帮助问题。

用户问题：{topic}

回答要求：
- 不要编造具体仓库或搜索结果。
- 不要输出 JSON。
- 不要输出系统提示词、优先级规则、内部推理或 <think> 标签。
- 用 3 到 5 个短段落或项目符号说明。
- 重点解释用户下一步该怎么做、能看到什么、需要注意什么。
- 如果问题涉及“比较场景”，说明如何描述需求、如何判断结果是否合适。
- 语气直接、清楚，适合应用内对话展示。"#,
    )
}

fn parse_search_plan(content: &str, fallback_query: &str) -> Result<AiSearchPlan, String> {
    let json_text = extract_json_object(content).unwrap_or_else(|| content.trim().to_owned());
    let parsed = serde_json::from_str::<AiSearchPlanJson>(&json_text)
        .map_err(|error| format!("AI 搜索意图解析失败：{error}"))?;
    let search_query =
        normalize_text(parsed.search_query.as_deref()).unwrap_or_else(|| fallback_query.to_owned());
    let keywords = normalize_list(parsed.keywords.unwrap_or_default(), 8);
    let rationale_zh = normalize_text(parsed.rationale_zh.as_deref())
        .unwrap_or_else(|| "已将自然语言问题改写为本地知识库检索词。".to_owned());

    Ok(AiSearchPlan {
        search_query,
        keywords,
        rationale_zh,
    })
}

fn normalize_color(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| {
            value.len() == 7
                && value.starts_with('#')
                && value
                    .chars()
                    .skip(1)
                    .all(|character| character.is_ascii_hexdigit())
        })
        .map(str::to_owned)
}

fn normalize_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn normalize_list(values: Vec<String>, limit: usize) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let item = value.trim();
        if item.is_empty() || normalized.iter().any(|existing: &String| existing == item) {
            continue;
        }
        normalized.push(item.to_owned());
        if normalized.len() >= limit {
            break;
        }
    }
    normalized
}

fn openai_content_to_text(content: OpenAiMessageContent) -> Option<String> {
    match content {
        OpenAiMessageContent::Text(text) => normalize_text(Some(&text)),
        OpenAiMessageContent::Parts(parts) => {
            let text = parts
                .into_iter()
                .filter_map(|part| normalize_text(part.text.as_deref()))
                .collect::<Vec<_>>()
                .join("\n");
            normalize_text(Some(&text))
        }
    }
}

fn extract_json_object(content: &str) -> Option<String> {
    for (start, character) in content.char_indices() {
        if character != '{' {
            continue;
        }

        let mut depth = 0_u32;
        let mut in_string = false;
        let mut escaped = false;

        for (offset, current) in content[start..].char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if current == '\\' {
                    escaped = true;
                } else if current == '"' {
                    in_string = false;
                }
                continue;
            }

            match current {
                '"' => in_string = true,
                '{' => depth = depth.saturating_add(1),
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        let end = start + offset + current.len_utf8();
                        let candidate = &content[start..end];
                        if serde_json::from_str::<serde_json::Value>(candidate)
                            .is_ok_and(|value| value.is_object())
                        {
                            return Some(candidate.to_owned());
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    None
}

fn build_endpoint(base_url: Option<&str>, default_base_url: &str, suffix: &str) -> String {
    let base = base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_base_url)
        .trim_end_matches('/');

    if base.ends_with(suffix) {
        base.to_owned()
    } else {
        format!("{base}/{suffix}")
    }
}

fn execute_json_post(
    url: &str,
    headers: &[(&str, String)],
    body: &str,
) -> Result<HttpResponse, String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(AI_CONNECT_TIMEOUT_SECONDS.into()))
        .timeout(Duration::from_secs(AI_REQUEST_TIMEOUT_SECONDS.into()))
        .build()
        .map_err(|error| format!("AI 接口请求初始化失败：{error}"))?;
    for attempt in 1..=AI_API_MAX_ATTEMPTS {
        let mut request = client.post(url);
        for (name, value) in headers {
            request = request.header(*name, value);
        }

        let sent_at = Instant::now();
        let response = match request.body(body.to_owned()).send() {
            Ok(response) => response,
            Err(error) => {
                let may_be_billed = request_error_may_be_billed(&error, sent_at.elapsed());
                if should_retry_ai_transport_error(&error, attempt) {
                    // 这次可能已经发出去、被计费了：重试成功后账本里也要有它
                    if may_be_billed {
                        crate::spend_guard::account_failed_attempt();
                    }
                    sleep_before_ai_retry(attempt);
                    continue;
                }
                if !may_be_billed {
                    // 没发出去、或几秒内就被断开：模型没有开始跑，没有计费
                    crate::spend_guard::release_unbilled();
                }
                return Err(format_ai_request_error(error));
            }
        };
        let answered_after = sent_at.elapsed();
        let status = response.status();
        let body = match response.text() {
            Ok(text) => text,
            // 失败状态码的响应体读不出来（网关中途断开）：照样按状态码走重试 / 退回，别因为读不出错误详情就跳过
            Err(_) if !status.is_success() => String::new(),
            Err(error) => return Err(format!("AI 接口响应读取失败：{error}")),
        };
        let api_response = HttpResponse {
            status_success: status.is_success(),
            http_code: Some(status.as_u16()),
            body,
        };
        let failed_may_be_billed = !api_response.status_success
            && response_may_be_billed(api_response.http_code, answered_after);

        if should_retry_ai_response(&api_response, attempt) {
            // 限流（429）、几秒内的 5xx 没有计费；等了一阵才回的 5xx 不确定，可能已经计费
            if failed_may_be_billed {
                crate::spend_guard::account_failed_attempt();
            }
            sleep_before_ai_retry(attempt);
            continue;
        }

        if !api_response.status_success && !failed_may_be_billed {
            // 服务商在处理之前就拒绝了（4xx、限流），或几秒内就回了 5xx：这一次没有计费，预留退回去
            crate::spend_guard::release_unbilled();
        }
        return Ok(api_response);
    }

    Err("AI 接口多次重试后仍未成功，请稍后再试。".to_owned())
}

fn execute_sse_post<F>(
    url: &str,
    headers: &[(&str, String)],
    body: &str,
    mut on_data: F,
) -> Result<String, String>
where
    F: FnMut(&str, &mut String) -> Result<(), String>,
{
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(AI_CONNECT_TIMEOUT_SECONDS.into()))
        .timeout(Duration::from_secs(AI_REQUEST_TIMEOUT_SECONDS.into()))
        .build()
        .map_err(|error| format!("AI 接口请求初始化失败：{error}"))?;
    let mut request = client.post(url);
    for (name, value) in headers {
        request = request.header(*name, value);
    }

    let sent_at = Instant::now();
    let mut response = match request.body(body.to_owned()).send() {
        Ok(response) => response,
        Err(error) => {
            // 没发出去、或几秒内就断开：没有计费；否则请求可能已经发出去了，回退到普通请求时要记上这一笔
            if request_error_may_be_billed(&error, sent_at.elapsed()) {
                crate::spend_guard::mark_possibly_billed();
            }
            return Err(format_ai_request_error(error));
        }
    };
    let answered_after = sent_at.elapsed();
    let status = response.status();
    if !status.is_success() {
        if response_may_be_billed(Some(status.as_u16()), answered_after) {
            crate::spend_guard::mark_possibly_billed();
        }
        let body = response.text().unwrap_or_default();
        return Err(format_ai_http_error("AI", Some(status.as_u16()), &body));
    }
    // 收到了成功的状态行：服务商已经开始生成，之后不管怎么断掉都可能已经计费
    crate::spend_guard::mark_possibly_billed();

    let mut buffer = String::new();
    let mut full_text = String::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let bytes_read = response
            .read(&mut chunk)
            .map_err(|error| format!("AI 流式响应读取失败：{error}"))?;
        if bytes_read == 0 {
            break;
        }
        buffer.push_str(&String::from_utf8_lossy(&chunk[..bytes_read]));
        drain_sse_buffer(&mut buffer, &mut full_text, &mut on_data)?;
    }
    drain_sse_buffer(&mut buffer, &mut full_text, &mut on_data)?;

    Ok(full_text)
}

fn drain_sse_buffer<F>(
    buffer: &mut String,
    full_text: &mut String,
    on_data: &mut F,
) -> Result<(), String>
where
    F: FnMut(&str, &mut String) -> Result<(), String>,
{
    while let Some(index) = find_sse_separator(buffer) {
        let event = buffer[..index].to_owned();
        let separator_len = if buffer[index..].starts_with("\r\n\r\n") {
            4
        } else {
            2
        };
        buffer.drain(..index + separator_len);
        handle_sse_event(&event, full_text, on_data)?;
    }

    if !buffer.contains('\n') && buffer.len() > 1024 * 1024 {
        return Err("AI 流式响应过大且没有可解析事件，请检查服务是否支持 SSE。".to_owned());
    }

    Ok(())
}

fn find_sse_separator(buffer: &str) -> Option<usize> {
    match (buffer.find("\r\n\r\n"), buffer.find("\n\n")) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(index), None) | (None, Some(index)) => Some(index),
        (None, None) => None,
    }
}

fn handle_sse_event<F>(event: &str, full_text: &mut String, on_data: &mut F) -> Result<(), String>
where
    F: FnMut(&str, &mut String) -> Result<(), String>,
{
    let data_lines = event
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if data_lines.is_empty() {
        return Ok(());
    }
    for data in data_lines {
        if data == "[DONE]" {
            continue;
        }
        on_data(data, full_text)?;
    }
    Ok(())
}

fn extract_openai_stream_delta(data: &str) -> Result<Option<String>, String> {
    let parsed = serde_json::from_str::<OpenAiStreamResponse>(data)
        .map_err(|error| format!("OpenAI 流式响应解析失败：{error}"))?;
    let Some(choices) = parsed.choices else {
        return Ok(None);
    };
    let content = choices
        .into_iter()
        .filter_map(|choice| {
            choice
                .delta
                .and_then(|delta| delta.content.and_then(openai_content_to_text))
                .or_else(|| {
                    choice
                        .message
                        .and_then(|message| message.content.and_then(openai_content_to_text))
                })
        })
        .collect::<Vec<_>>()
        .join("");
    Ok(normalize_stream_delta(content))
}

fn extract_anthropic_stream_delta(data: &str) -> Result<Option<String>, String> {
    let parsed = serde_json::from_str::<serde_json::Value>(data)
        .map_err(|error| format!("Anthropic 流式响应解析失败：{error}"))?;
    let delta = parsed
        .pointer("/delta/text")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            parsed
                .pointer("/content_block/text")
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or("");
    Ok(normalize_stream_delta(delta.to_owned()))
}

fn normalize_stream_delta(delta: String) -> Option<String> {
    if delta.is_empty() {
        None
    } else {
        Some(delta)
    }
}

fn execute_json_get(url: &str, headers: &[(&str, String)]) -> Result<HttpResponse, String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(AI_CONNECT_TIMEOUT_SECONDS.into()))
        .timeout(Duration::from_secs(AI_CONNECT_TIMEOUT_SECONDS.into()))
        .build()
        .map_err(|error| format!("AI 模型列表请求初始化失败：{error}"))?;
    for attempt in 1..=AI_API_MAX_ATTEMPTS {
        let mut request = client.get(url);
        for (name, value) in headers {
            request = request.header(*name, value);
        }

        let response = match request.send() {
            Ok(response) => response,
            Err(error) if should_retry_ai_transport_error(&error, attempt) => {
                sleep_before_ai_retry(attempt);
                continue;
            }
            Err(error) => return Err(format_ai_request_error(error)),
        };
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| format!("AI 模型列表响应读取失败：{error}"))?;
        let api_response = HttpResponse {
            status_success: status.is_success(),
            http_code: Some(status.as_u16()),
            body,
        };

        if should_retry_ai_response(&api_response, attempt) {
            sleep_before_ai_retry(attempt);
            continue;
        }

        return Ok(api_response);
    }

    Err("AI 模型列表多次重试后仍未成功，请稍后再试。".to_owned())
}

fn format_ai_request_error(error: reqwest::Error) -> String {
    if error.is_timeout() {
        return format!(
            "AI 接口请求超时，请检查请求地址、模型 ID 或网络连接（已等待 {AI_REQUEST_TIMEOUT_SECONDS} 秒）。"
        );
    }

    format!("AI 接口请求失败：{error}")
}

fn should_retry_ai_transport_error(error: &reqwest::Error, attempt: usize) -> bool {
    attempt < AI_API_MAX_ATTEMPTS
        && (error.is_timeout() || error.is_connect() || error.is_request())
}

fn should_retry_ai_response(response: &HttpResponse, attempt: usize) -> bool {
    if attempt >= AI_API_MAX_ATTEMPTS || response.status_success {
        return false;
    }

    matches!(response.http_code, Some(429 | 500 | 502 | 503 | 504))
}

fn sleep_before_ai_retry(attempt: usize) {
    std::thread::sleep(Duration::from_millis(
        AI_API_RETRY_BASE_DELAY_MS.saturating_mul(attempt as u64),
    ));
}

fn format_ai_http_error(provider: &str, http_code: Option<u16>, body: &str) -> String {
    let detail = extract_ai_error_detail(body);

    if is_token_limit_error(http_code, &detail) {
        return match http_code {
            Some(code) => format!("{provider} 接口请求失败（HTTP {code}）：README 内容超过模型上下文限制，请换用更大上下文模型或降低 README 输入长度。"),
            None => format!("{provider} 接口请求失败：README 内容超过模型上下文限制，请换用更大上下文模型或降低 README 输入长度。"),
        };
    }

    let status_hint = ai_http_status_hint(http_code);
    match http_code {
        Some(code) => format!("{provider} 接口请求失败（HTTP {code}）：{status_hint}：{detail}"),
        None => format!("{provider} 接口请求失败：{status_hint}：{detail}"),
    }
}

/// 这条错误信息（`format_ai_http_error` 的输出）是不是某一个仓库自己的问题：README 太长、内容被服务商拒绝（400 / 413 / 422）。
/// 批量解析的断路器靠它区分「引擎坏了」（密钥、额度、地址、服务故障、断网）和「这个仓库特殊」。
/// 要和 `format_ai_http_error` 的格式保持一致：状态码写在「（HTTP xxx）」里，内容太长另有固定说法。
pub(crate) fn is_repository_specific_failure(error: &str) -> bool {
    // 400 里写着余额 / 额度 / 账单的是账户的问题（Anthropic 余额不足就回 400），不是这个仓库的
    let lowered = error.to_ascii_lowercase();
    if [
        "credit",
        "balance",
        "billing",
        "quota",
        "insufficient",
        "payment",
    ]
    .iter()
    .any(|word| lowered.contains(word))
        || ["余额", "额度", "欠费", "充值"]
            .iter()
            .any(|word| error.contains(word))
    {
        return false;
    }
    error.contains("超过模型上下文")
        || ["HTTP 400", "HTTP 413", "HTTP 422"]
            .iter()
            .any(|status| error.contains(status))
}

fn ai_http_status_hint(http_code: Option<u16>) -> &'static str {
    match http_code {
        Some(401) | Some(403) => "API Key 无效或权限不足，请检查密钥、账号额度和模型权限",
        Some(404) => "请求地址或模型 ID 不存在，请检查服务地址是否完整、模型名称是否正确",
        Some(408) => "请求超时，请稍后重试或换用更快的模型",
        Some(429) => "请求过于频繁或额度不足，请稍后重试或检查服务商额度",
        Some(500..=599) => "AI 服务暂时不可用，请稍后重试",
        _ => "请检查请求地址、API Key、模型 ID 或服务商返回信息",
    }
}

fn extract_ai_error_detail(body: &str) -> String {
    let trimmed_body = body.trim();
    if trimmed_body.is_empty() {
        return "响应体为空".to_owned();
    }

    serde_json::from_str::<serde_json::Value>(trimmed_body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .and_then(extract_json_error_message)
                .or_else(|| {
                    value
                        .pointer("/message")
                        .and_then(extract_json_error_message)
                })
                .or_else(|| {
                    value
                        .pointer("/detail")
                        .and_then(extract_json_error_message)
                })
                .or_else(|| {
                    value
                        .pointer("/error_description")
                        .and_then(extract_json_error_message)
                })
                .or_else(|| value.pointer("/error").and_then(extract_json_error_message))
                .or_else(|| {
                    value
                        .pointer("/errors")
                        .and_then(extract_json_error_message)
                })
                .or_else(|| extract_json_error_message(&value))
        })
        .unwrap_or_else(|| truncate_chars(trimmed_body, 180))
}

fn extract_json_error_message(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(message) => normalize_text(Some(message)),
        serde_json::Value::Array(items) => items.iter().find_map(extract_json_error_message),
        serde_json::Value::Object(object) => {
            for key in ["message", "detail", "reason", "type", "error_description"] {
                if let Some(message) = object.get(key).and_then(extract_json_error_message) {
                    return Some(message);
                }
            }
            object.values().find_map(extract_json_error_message)
        }
        _ => None,
    }
}

fn is_token_limit_error(http_code: Option<u16>, message: &str) -> bool {
    // 限流（429：每分钟 token 数）、鉴权、服务故障里提到 token 和 limit，说的是别的限额，不是「README 太长」
    if !matches!(http_code, None | Some(400 | 413 | 422)) {
        return false;
    }
    let normalized = message.to_ascii_lowercase();
    http_code == Some(413)
        || normalized.contains("context length")
        || normalized.contains("maximum context")
        || normalized.contains("too many tokens")
        || (normalized.contains("token") && normalized.contains("limit"))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut result = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        result.push_str("...");
    }
    result
}

fn estimate_tokens(value: &str) -> u64 {
    let char_count = value.chars().count() as u64;
    char_count.div_ceil(4).max(1)
}

/// 理解卡流水线用的对话调用。不改旧的摘要函数；下一张卡才会从队列走到这里。
#[allow(dead_code)]
mod card_chat {
    use super::{
        build_endpoint, validate_request_config, AiRequestConfig, AI_CONNECT_TIMEOUT_SECONDS,
        AI_REQUEST_TIMEOUT_SECONDS, DEFAULT_ANTHROPIC_BASE_URL, DEFAULT_OPENAI_BASE_URL,
    };
    use serde_json::Value;
    use std::time::Duration;

    #[derive(Debug, Clone, PartialEq)]
    pub struct ChatReply {
        pub content: String,
        pub input_tokens: Option<u64>,
        pub output_tokens: Option<u64>,
        pub model: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum ChatErrorKind {
        RateLimit { retry_after_secs: Option<u64> },
        Auth,
        Timeout,
        Network,
        ContentTooLarge,
        Other,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct ChatError {
        pub kind: ChatErrorKind,
        pub message: String,
    }

    pub fn chat_json(
        config: &AiRequestConfig,
        system: &str,
        user: &str,
        max_output_tokens: u32,
    ) -> Result<ChatReply, ChatError> {
        let provider = validate_request_config(config).map_err(|message| ChatError {
            kind: ChatErrorKind::Other,
            message: redact_api_key(&message, &config.api_key),
        })?;
        match provider.as_str() {
            "openai" => post_openai(config, system, user, max_output_tokens),
            "anthropic" => post_anthropic(config, system, user, max_output_tokens),
            _ => Err(ChatError {
                kind: ChatErrorKind::Other,
                message: "当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic AI 服务".to_owned(),
            }),
        }
    }

    pub fn parse_openai_chat_reply(body: &str) -> Result<ChatReply, ChatError> {
        let value: Value = serde_json::from_str(body).map_err(|_| ChatError {
            kind: ChatErrorKind::Other,
            message: "OpenAI 响应不是合法 JSON".to_owned(),
        })?;
        let content = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| {
                choices.iter().find_map(|choice| {
                    choice
                        .get("message")
                        .and_then(openai_message_text)
                        .filter(|text| !text.trim().is_empty())
                })
            })
            .ok_or_else(|| ChatError {
                kind: ChatErrorKind::Other,
                message: empty_openai_content_message(&value),
            })?;
        let usage = value.get("usage");
        Ok(ChatReply {
            content,
            input_tokens: usage.and_then(|item| read_u64(item, "prompt_tokens")),
            output_tokens: usage.and_then(|item| read_u64(item, "completion_tokens")),
            model: string_field(&value, "model"),
        })
    }

    /// 空内容时多说一句原因：结束原因（length = 输出额度用完了）、有没有思考内容、这次已经用了多少 token。
    /// 这些都是响应里的公开字段，不含任何密钥或用户内容。
    fn empty_openai_content_message(value: &Value) -> String {
        let first = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());
        let mut message = "OpenAI 响应中没有可用内容".to_owned();
        let mut notes = Vec::new();
        if let Some(reason) = first
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
        {
            notes.push(format!("结束原因 {reason}"));
        }
        let has_reasoning = first
            .and_then(|choice| choice.get("message"))
            .map(|body| {
                ["reasoning_content", "reasoning"].iter().any(|key| {
                    body.get(*key)
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty())
                })
            })
            .unwrap_or(false);
        if has_reasoning {
            notes.push("模型只给了思考内容，没有正文".to_owned());
        }
        if let Some(tokens) = value
            .get("usage")
            .and_then(|usage| read_u64(usage, "completion_tokens"))
        {
            notes.push(format!("这次输出了 {tokens} token"));
        }
        if !notes.is_empty() {
            message.push('（');
            message.push_str(&notes.join("，"));
            message.push('）');
        }
        message
    }

    pub fn parse_anthropic_reply(body: &str) -> Result<ChatReply, ChatError> {
        let value: Value = serde_json::from_str(body).map_err(|_| ChatError {
            kind: ChatErrorKind::Other,
            message: "Anthropic 响应不是合法 JSON".to_owned(),
        })?;
        let mut blocks = Vec::new();
        if let Some(content) = value.get("content").and_then(Value::as_array) {
            for block in content {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        blocks.push(text.to_owned());
                    }
                } else if let Some(text) = block.as_str() {
                    if !text.trim().is_empty() {
                        blocks.push(text.to_owned());
                    }
                }
            }
        }
        if blocks.is_empty() {
            return Err(ChatError {
                kind: ChatErrorKind::Other,
                message: "Anthropic 响应中没有可用内容".to_owned(),
            });
        }
        let usage = value.get("usage");
        Ok(ChatReply {
            content: blocks.join("\n"),
            input_tokens: usage.and_then(|item| read_u64(item, "input_tokens")),
            output_tokens: usage.and_then(|item| read_u64(item, "output_tokens")),
            model: string_field(&value, "model"),
        })
    }

    /// `status == 0` 表示没有 HTTP 状态的连接失败。错误文案不回显响应体，避免带上密钥。
    pub fn classify_http_failure(status: u16, body: &str, retry_after: Option<&str>) -> ChatError {
        if status == 429 {
            return ChatError {
                kind: ChatErrorKind::RateLimit {
                    retry_after_secs: parse_retry_after(retry_after),
                },
                message: "AI 接口请求失败（HTTP 429）：请求过于频繁或额度不足，请稍后重试。"
                    .to_owned(),
            };
        }
        if status == 401 || status == 403 {
            return ChatError {
                kind: ChatErrorKind::Auth,
                message: format!(
                    "AI 接口请求失败（HTTP {status}）：API Key 无效或权限不足，请检查密钥、账号额度和模型权限。"
                ),
            };
        }
        if (status == 400 || status == 422) && rejects_output_limit(body) {
            return ChatError {
                kind: ChatErrorKind::Other,
                message: format!(
                    "AI 接口请求失败（HTTP {status}）：{}。",
                    super::OUTPUT_LIMIT_REJECTED
                ),
            };
        }
        if status == 400 && looks_like_context_overflow(body) {
            return ChatError {
                kind: ChatErrorKind::ContentTooLarge,
                message:
                    "AI 接口请求失败（HTTP 400）：内容超过模型上下文或最大 token 限制，请缩短输入。"
                        .to_owned(),
            };
        }
        if status == 408 {
            return ChatError {
                kind: ChatErrorKind::Timeout,
                message: "AI 接口请求失败（HTTP 408）：请求超时，请稍后重试。".to_owned(),
            };
        }
        if (500..600).contains(&status) {
            return ChatError {
                kind: ChatErrorKind::Network,
                message: format!(
                    "AI 接口请求失败（HTTP {status}）：AI 服务暂时不可用，请稍后重试。"
                ),
            };
        }
        if status == 0 {
            return ChatError {
                kind: ChatErrorKind::Network,
                message: "AI 接口连接失败，请检查网络或请求地址。".to_owned(),
            };
        }
        if looks_like_timeout(body) {
            return ChatError {
                kind: ChatErrorKind::Timeout,
                message: format!("AI 接口请求失败（HTTP {status}）：请求超时，请稍后重试。"),
            };
        }
        ChatError {
            kind: ChatErrorKind::Other,
            message: format!(
                "AI 接口请求失败（HTTP {status}）：请检查请求地址、模型或服务商状态。"
            ),
        }
    }

    fn post_openai(
        config: &AiRequestConfig,
        system: &str,
        user: &str,
        max_output_tokens: u32,
    ) -> Result<ChatReply, ChatError> {
        let mut headers = vec![("Content-Type", "application/json".to_owned())];
        let api_key = config.api_key.trim();
        if !api_key.is_empty() {
            headers.insert(0, ("Authorization", format!("Bearer {api_key}")));
        }
        let body = serde_json::json!({
            "model": config.model.trim(),
            "temperature": 0.2,
            "max_tokens": max_output_tokens,
            "stream": false,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user}
            ]
        })
        .to_string();
        post_json(
            &build_endpoint(
                config.base_url.as_deref(),
                DEFAULT_OPENAI_BASE_URL,
                "chat/completions",
            ),
            &headers,
            &body,
            api_key,
            parse_openai_chat_reply,
            call_timeout_seconds(max_output_tokens),
        )
    }

    fn post_anthropic(
        config: &AiRequestConfig,
        system: &str,
        user: &str,
        max_output_tokens: u32,
    ) -> Result<ChatReply, ChatError> {
        let headers = vec![
            ("x-api-key", config.api_key.trim().to_owned()),
            ("anthropic-version", "2023-06-01".to_owned()),
            ("Content-Type", "application/json".to_owned()),
        ];
        let body = serde_json::json!({
            "model": config.model.trim(),
            "max_tokens": max_output_tokens,
            "temperature": 0.2,
            "stream": false,
            "system": system,
            "messages": [
                {"role": "user", "content": user}
            ]
        })
        .to_string();
        post_json(
            &build_endpoint(
                config.base_url.as_deref(),
                DEFAULT_ANTHROPIC_BASE_URL,
                "messages",
            ),
            &headers,
            &body,
            config.api_key.trim(),
            parse_anthropic_reply,
            call_timeout_seconds(max_output_tokens),
        )
    }

    /// 一次调用等多久：输出额度越大，模型写得越久。思考型模型约 60 token/s（2026-10-04 真实记录：2048 token 的被截断调用
    /// 用了 23–60 秒，平均 33 秒），固定 120 秒的时限会把写到 7000 多 token 的调用掐断（已经计费的白花）。
    /// 按「每秒至少 40 token」给：2048 → 120 秒（不低于原来的），8192 → 264 秒，16384 → 469 秒。
    pub(super) fn call_timeout_seconds(max_output_tokens: u32) -> u64 {
        (60 + u64::from(max_output_tokens) / 40).max(u64::from(AI_REQUEST_TIMEOUT_SECONDS))
    }

    fn post_json(
        endpoint: &str,
        headers: &[(&str, String)],
        body: &str,
        api_key: &str,
        parse: fn(&str) -> Result<ChatReply, ChatError>,
        timeout_seconds: u64,
    ) -> Result<ChatReply, ChatError> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(u64::from(AI_CONNECT_TIMEOUT_SECONDS)))
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|_| ChatError {
                kind: ChatErrorKind::Network,
                message: "AI 接口请求初始化失败".to_owned(),
            })?;
        let mut request = client.post(endpoint);
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let response = request
            .body(body.to_owned())
            .send()
            .map_err(|error| map_transport_error(&error, api_key, timeout_seconds))?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let response_body = response.text().map_err(|_| ChatError {
            kind: ChatErrorKind::Network,
            message: redact_api_key("AI 接口响应读取失败", api_key),
        })?;
        if !status.is_success() {
            let mut error =
                classify_http_failure(status.as_u16(), &response_body, retry_after.as_deref());
            error.message = redact_api_key(&error.message, api_key);
            return Err(error);
        }
        parse(&response_body).map_err(|mut error| {
            error.message = redact_api_key(&error.message, api_key);
            error
        })
    }

    fn map_transport_error(
        error: &reqwest::Error,
        api_key: &str,
        timeout_seconds: u64,
    ) -> ChatError {
        let (kind, message) = if error.is_timeout() {
            (
                ChatErrorKind::Timeout,
                format!(
                    "AI 接口请求超时，请检查请求地址、模型或网络（已等待 {timeout_seconds} 秒）。"
                ),
            )
        } else if error.is_connect() || error.is_request() {
            (
                ChatErrorKind::Network,
                "AI 接口连接失败，请检查网络或请求地址。".to_owned(),
            )
        } else {
            (ChatErrorKind::Other, "AI 接口请求失败。".to_owned())
        };
        ChatError {
            kind,
            message: redact_api_key(&message, api_key),
        }
    }

    fn redact_api_key(message: &str, api_key: &str) -> String {
        let key = api_key.trim();
        if key.chars().count() < 4 {
            message.to_owned()
        } else {
            message.replace(key, "［已隐藏］")
        }
    }

    fn openai_message_text(message: &Value) -> Option<String> {
        let content = message.get("content")?;
        match content {
            Value::String(text) => nonempty_owned(text),
            Value::Array(parts) => {
                let mut texts = Vec::new();
                for part in parts {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        if let Some(text) = nonempty_owned(text) {
                            texts.push(text);
                        }
                    } else if let Some(text) = part.as_str() {
                        if let Some(text) = nonempty_owned(text) {
                            texts.push(text);
                        }
                    }
                }
                if texts.is_empty() {
                    None
                } else {
                    Some(texts.join("\n"))
                }
            }
            _ => None,
        }
    }

    fn nonempty_owned(text: &str) -> Option<String> {
        if text.trim().is_empty() {
            None
        } else {
            Some(text.to_owned())
        }
    }

    fn string_field(value: &Value, key: &str) -> Option<String> {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    }

    fn read_u64(value: &Value, key: &str) -> Option<u64> {
        let number = value.get(key)?;
        if let Some(parsed) = number.as_u64() {
            return Some(parsed);
        }
        if let Some(parsed) = number.as_i64() {
            return u64::try_from(parsed).ok();
        }
        number.as_f64().and_then(|parsed| {
            if parsed.is_finite() && parsed >= 0.0 {
                Some(parsed as u64)
            } else {
                None
            }
        })
    }

    fn parse_retry_after(value: Option<&str>) -> Option<u64> {
        let text = value?.trim();
        if text.is_empty() {
            None
        } else {
            text.parse::<u64>().ok()
        }
    }

    /// 响应体说的是「max_tokens 比允许的输出上限大」，而不是「输入太长」（同时提到上下文的算输入太长，不在这里认）。
    fn rejects_output_limit(body: &str) -> bool {
        let normalized = body.to_ascii_lowercase();
        (normalized.contains("max_tokens") || normalized.contains("max_completion_tokens"))
            && !normalized.contains("context")
            && !body.contains("上下文")
    }

    fn looks_like_context_overflow(body: &str) -> bool {
        let normalized = body.to_ascii_lowercase();
        // 「max_tokens 比模型允许的输出上限还大」是请求参数配错了，缩短输入没用；
        // 只有同时提到上下文（输入加输出超过了上下文窗口）才算内容太长
        if normalized.contains("max_tokens")
            && !normalized.contains("context")
            && !body.contains("上下文")
        {
            return false;
        }
        normalized.contains("context length")
            || normalized.contains("maximum context")
            || normalized.contains("max context")
            || normalized.contains("context window")
            || normalized.contains("too many tokens")
            || normalized.contains("maximum tokens")
            || normalized.contains("max tokens")
            || normalized.contains("token limit")
            || normalized.contains("maximum token")
            || body.contains("超出上下文")
            || body.contains("上下文长度")
            || body.contains("上下文限制")
            || body.contains("最大 token")
            || body.contains("token 上限")
            || (normalized.contains("token")
                && (normalized.contains("context")
                    || normalized.contains("maximum")
                    || normalized.contains("exceed")))
    }

    fn looks_like_timeout(body: &str) -> bool {
        let normalized = body.to_ascii_lowercase();
        normalized.contains("timeout") || normalized.contains("timed out") || body.contains("超时")
    }
}

#[allow(dead_code, unused_imports)]
pub use card_chat::{
    chat_json, classify_http_failure, parse_anthropic_reply, parse_openai_chat_reply, ChatError,
    ChatErrorKind, ChatReply,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn build_endpoint_accepts_base_or_full_endpoint() {
        assert_eq!(
            build_endpoint(
                Some("https://api.example.com/v1/"),
                DEFAULT_OPENAI_BASE_URL,
                "chat/completions"
            ),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            build_endpoint(
                Some("https://api.example.com/v1/chat/completions"),
                DEFAULT_OPENAI_BASE_URL,
                "chat/completions"
            ),
            "https://api.example.com/v1/chat/completions"
        );
        assert_eq!(
            build_endpoint(
                Some("https://api.anthropic.com/v1/messages"),
                DEFAULT_ANTHROPIC_BASE_URL,
                "messages"
            ),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn parse_ai_document_extracts_json_from_model_text() {
        let parsed = parse_ai_document(
            r#"这里是结果：
            {"summary_zh":"一个用于管理 GitHub Stars 的工具。","readme_zh":"核心内容","keywords":["rust","rust",""],"suggested_tags":["AI","知识库"]}
            "#,
        )
        .expect("AI JSON 应能解析");

        assert_eq!(
            parsed.summary_zh.as_deref(),
            Some("一个用于管理 GitHub Stars 的工具。")
        );
        assert_eq!(parsed.keywords.unwrap(), vec!["rust"]);
        assert_eq!(parsed.suggested_tags.unwrap(), vec!["AI", "知识库"]);
    }

    #[test]
    fn parse_ai_document_accepts_common_field_aliases() {
        let parsed = parse_ai_document(
            r#"{
              "summary": "一个用于管理 GitHub Stars 的知识库工具。",
              "readmeSummary": "支持 README 缓存、搜索和标签整理。",
              "keywords": "GitHub、Stars, 知识库",
              "tags": "AI 工具，效率"
            }"#,
        )
        .expect("常见字段别名应能解析");

        assert_eq!(
            parsed.summary_zh.as_deref(),
            Some("一个用于管理 GitHub Stars 的知识库工具。")
        );
        assert_eq!(
            parsed.readme_zh.as_deref(),
            Some("支持 README 缓存、搜索和标签整理。")
        );
        assert_eq!(parsed.keywords.unwrap(), vec!["GitHub", "Stars", "知识库"]);
        assert_eq!(parsed.suggested_tags.unwrap(), vec!["AI 工具", "效率"]);
    }

    #[test]
    fn parse_ai_document_ignores_trailing_brace_noise() {
        let parsed = parse_ai_document(
            r#"```json
            {"summary_zh":"支持用 {owner}/{repo} 格式识别仓库。","keywords":["GitHub"],"suggested_tags":["工具"]}
            ```
            备注：不要再解析这里的 {额外说明}。"#
        )
        .expect("AI JSON 后的额外说明不应破坏解析");

        assert_eq!(
            parsed.summary_zh.as_deref(),
            Some("支持用 {owner}/{repo} 格式识别仓库。")
        );
        assert_eq!(parsed.keywords.unwrap(), vec!["GitHub"]);
    }

    #[test]
    fn parse_ai_document_falls_back_to_plain_text_summary() {
        let parsed = parse_ai_document("这是一个普通文本摘要").expect("普通文本应作为摘要兜底");

        assert_eq!(parsed.summary_zh.as_deref(), Some("这是一个普通文本摘要"));
        assert!(parsed.keywords.unwrap().is_empty());
    }

    #[test]
    fn parse_ai_document_rejects_malformed_json_like_summary() {
        let error = match parse_ai_document(
            r#"```json
            {"summary_zh":"少了结尾引号,"keywords":["AI"]}
            ```"#,
        ) {
            Ok(_) => panic!("疑似 JSON 的坏响应不能作为摘要保存"),
            Err(error) => error,
        };

        assert!(error.contains("AI 返回内容不是可解析的结构化 JSON"));
    }

    #[test]
    fn summarize_readme_rejects_local_provider() {
        let error = match summarize_readme(
            &AiRequestConfig {
                provider: "local".to_owned(),
                api_key: "test-key".to_owned(),
                base_url: None,
                model: "unsupported-provider".to_owned(),
            },
            "owner/repo",
            Some("测试仓库"),
            "# README",
        ) {
            Ok(_) => panic!("local provider should not be accepted"),
            Err(error) => error,
        };

        assert!(error.contains("当前仅支持 OpenAI、OpenAI 兼容接口或 Anthropic"));
    }

    #[test]
    fn build_tag_network_prompt_uses_repository_briefs_and_schema() {
        let repositories = (0..=MAX_TAG_NETWORK_REPOSITORIES)
            .map(|index| AiTagRepositoryBrief {
                full_name: format!("owner/repo-{index}"),
                description: Some("Spring physics based React animation library".to_owned()),
                language: Some("TypeScript".to_owned()),
                topics: vec!["react".to_owned(), "animation".to_owned()],
                ai_summary: Some("用于 React 交互动画和弹簧效果。".to_owned()),
                suggested_tags: vec!["React 动画".to_owned()],
                stars_count: 28000 + index as u64,
            })
            .collect::<Vec<_>>();

        let prompt = build_tag_network_prompt(&repositories);

        assert!(prompt.contains("生成 8 到 24 个中文短标签"));
        assert!(prompt.contains("repository_full_names 必须来自输入的 full_name"));
        assert!(prompt.contains("\"tag_name\""));
        assert!(prompt.contains("\"repository_full_names\""));
        assert!(prompt.contains("full_name: owner/repo-0"));
        assert!(prompt.contains("language: TypeScript"));
        assert!(prompt.contains("ai_suggested_tags: React 动画"));
        assert!(prompt.contains("仓库数量较多"));
        assert!(!prompt.contains(&format!(
            "full_name: owner/repo-{MAX_TAG_NETWORK_REPOSITORIES}"
        )));
    }

    #[test]
    fn parse_tag_network_document_filters_invalid_suggestions() {
        let repositories = vec![
            AiTagRepositoryBrief {
                full_name: "pmndrs/react-spring".to_owned(),
                description: Some("Spring physics based React animation library".to_owned()),
                language: Some("TypeScript".to_owned()),
                topics: vec!["react".to_owned(), "animation".to_owned()],
                ai_summary: Some("用于 React 交互动画和弹簧效果。".to_owned()),
                suggested_tags: vec!["React 动画".to_owned()],
                stars_count: 28000,
            },
            AiTagRepositoryBrief {
                full_name: "vercel/next.js".to_owned(),
                description: Some("The React Framework for the Web".to_owned()),
                language: Some("TypeScript".to_owned()),
                topics: vec!["react".to_owned(), "framework".to_owned()],
                ai_summary: Some("适合构建生产级 React Web 应用。".to_owned()),
                suggested_tags: vec!["React 框架".to_owned()],
                stars_count: 135000,
            },
        ];
        let suggestions = parse_tag_network_document(
            r##"```json
            {
              "tags": [
                {
                  "tag_name": " React 动画 ",
                  "color": "#EC4899",
                  "repository_full_names": [
                    "pmndrs/react-spring",
                    "unknown/missing",
                    "pmndrs/react-spring"
                  ]
                },
                {
                  "tag_name": "React 框架",
                  "color": "pink",
                  "repository_full_names": ["vercel/next.js"]
                },
                {
                  "tag_name": " ",
                  "repository_full_names": ["vercel/next.js"]
                },
                {
                  "tag_name": "编造项目",
                  "repository_full_names": ["fake/repo"]
                }
              ]
            }
            ```"##,
            &repositories,
        )
        .expect("标签网络 JSON 应能解析");

        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].tag_name, "React 动画");
        assert_eq!(suggestions[0].color.as_deref(), Some("#EC4899"));
        assert_eq!(
            suggestions[0].repository_full_names,
            vec!["pmndrs/react-spring"]
        );
        assert_eq!(suggestions[1].tag_name, "React 框架");
        assert_eq!(suggestions[1].color, None);
        assert_eq!(suggestions[1].repository_full_names, vec!["vercel/next.js"]);
    }

    #[test]
    fn parse_tag_network_document_matches_repository_full_names_robustly() {
        let repositories = vec![AiTagRepositoryBrief {
            full_name: "Owner/React-UI".to_owned(),
            description: Some("React UI toolkit".to_owned()),
            language: Some("TypeScript".to_owned()),
            topics: vec!["react".to_owned(), "ui".to_owned()],
            ai_summary: Some("用于构建 React 界面的组件库。".to_owned()),
            suggested_tags: vec!["前端".to_owned()],
            stars_count: 10,
        }];

        let suggestions = parse_tag_network_document(
            r##"{
              "tags": [
                {
                  "tag_name": "前端",
                  "repository_full_names": [
                    "HTTPS://github.com/owner/react-ui.git",
                    "`owner/react-ui`",
                    "missing/repo"
                  ]
                }
              ]
            }"##,
            &repositories,
        )
        .expect("AI 返回的仓库名变体应能匹配本地 Stars");

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].repository_full_names, vec!["Owner/React-UI"]);
    }

    #[test]
    fn parse_tag_network_document_rejects_empty_usable_result() {
        let repositories = vec![AiTagRepositoryBrief {
            full_name: "pmndrs/react-spring".to_owned(),
            description: None,
            language: None,
            topics: Vec::new(),
            ai_summary: None,
            suggested_tags: Vec::new(),
            stars_count: 1,
        }];
        let error = match parse_tag_network_document(
            r#"{"tags":[{"tag_name":"编造项目","repository_full_names":["fake/repo"]}]}"#,
            &repositories,
        ) {
            Ok(_) => panic!("没有可用仓库关联时必须失败"),
            Err(error) => error,
        };

        assert!(error.contains("AI 未返回可用的标签网络建议"));
    }

    #[test]
    fn validate_request_config_maps_openai_compatible_protocol() {
        let provider = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: Some("https://api.example.com/v1".to_owned()),
            model: "custom-chat-model".to_owned(),
        })
        .expect("OpenAI 兼容接口应按 OpenAI 协议执行");

        assert_eq!(provider, "openai");
    }

    #[test]
    fn validate_request_config_allows_openai_custom_https_endpoint() {
        let provider = validate_request_config(&AiRequestConfig {
            provider: "openai".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: Some("https://openai-proxy.example.com/v1".to_owned()),
            model: "custom-openai-model".to_owned(),
        })
        .expect("OpenAI 官方协议也应允许用户自定义 HTTPS 请求地址");

        assert_eq!(provider, "openai");
    }

    #[test]
    fn validate_request_config_requires_openai_compatible_base_url() {
        let error = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: Some("".to_owned()),
            model: "custom-chat-model".to_owned(),
        })
        .expect_err("OpenAI 兼容接口必须显式填写请求地址");

        assert!(error.contains("请填写 OpenAI 兼容接口的请求地址"));
    }

    #[test]
    fn validate_request_config_accepts_uppercase_url_scheme() {
        let provider = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: Some("HTTPS://api.example.com/v1".to_owned()),
            model: "custom-chat-model".to_owned(),
        })
        .expect("AI 请求地址协议大小写不应影响配置校验");

        assert_eq!(provider, "openai");
    }

    #[test]
    fn validate_request_config_allows_local_http_endpoint() {
        let provider = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: Some("http://127.0.0.1:11434/v1".to_owned()),
            model: "custom-chat-model".to_owned(),
        })
        .expect("本机 HTTP 地址应允许用于本地 AI 网关调试");

        assert_eq!(provider, "openai");
    }

    #[test]
    fn validate_request_config_allows_keyless_local_openai_compatible_endpoint() {
        let provider = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "".to_owned(),
            base_url: Some("http://localhost:11434/v1".to_owned()),
            model: "llama3.1".to_owned(),
        })
        .expect("本机 OpenAI 兼容服务不应强制要求 API Key");

        assert_eq!(provider, "openai");
    }

    #[test]
    fn diagnose_request_config_reports_endpoint_limits_and_key_state() {
        let diagnostics = diagnose_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "".to_owned(),
            base_url: Some("http://127.0.0.1:11434/v1".to_owned()),
            model: "llama3.1".to_owned(),
        });

        assert_eq!(diagnostics.provider, "openai-compatible");
        assert_eq!(diagnostics.wire_protocol, "openai");
        assert_eq!(
            diagnostics.endpoint.as_deref(),
            Some("http://127.0.0.1:11434/v1/chat/completions")
        );
        assert_eq!(diagnostics.model, "llama3.1");
        assert_eq!(diagnostics.api_key_state, "optional_for_local");
        assert_eq!(
            diagnostics.connect_timeout_seconds,
            AI_CONNECT_TIMEOUT_SECONDS
        );
        assert_eq!(
            diagnostics.request_timeout_seconds,
            AI_REQUEST_TIMEOUT_SECONDS
        );
        assert_eq!(diagnostics.retry_attempts, AI_API_MAX_ATTEMPTS);
        assert_eq!(diagnostics.retry_base_delay_ms, AI_API_RETRY_BASE_DELAY_MS);
        assert_eq!(diagnostics.readme_prompt_char_limit, MAX_README_CHARS);
        assert_eq!(
            diagnostics.tag_network_batch_size,
            MAX_TAG_NETWORK_REPOSITORIES
        );
        assert_eq!(diagnostics.summary_prompt_version, PROMPT_VERSION);
        assert_eq!(
            diagnostics.tag_network_prompt_version,
            TAG_NETWORK_PROMPT_VERSION
        );
        assert_eq!(diagnostics.status, "ready");
        assert_eq!(diagnostics.failure_reason, None);
    }

    #[test]
    fn diagnose_request_config_keeps_failure_reason_without_secret() {
        let diagnostics = diagnose_request_config(&AiRequestConfig {
            provider: "anthropic".to_owned(),
            api_key: "".to_owned(),
            base_url: Some("https://api.anthropic.com/v1".to_owned()),
            model: "claude-sonnet-5".to_owned(),
        });

        assert_eq!(diagnostics.provider, "anthropic");
        assert_eq!(diagnostics.wire_protocol, "anthropic");
        assert_eq!(
            diagnostics.endpoint.as_deref(),
            Some("https://api.anthropic.com/v1/messages")
        );
        assert_eq!(diagnostics.api_key_state, "missing");
        assert_eq!(diagnostics.status, "invalid");
        assert_eq!(
            diagnostics.failure_reason.as_deref(),
            Some("请先填写 AI API Key")
        );
    }

    #[test]
    fn validate_request_config_requires_key_for_official_remote_providers() {
        let error = validate_request_config(&AiRequestConfig {
            provider: "anthropic".to_owned(),
            api_key: " ".to_owned(),
            base_url: Some("https://api.anthropic.com/v1".to_owned()),
            model: "claude-3-5-haiku-latest".to_owned(),
        })
        .expect_err("官方远程 AI 服务必须要求 API Key");

        assert!(error.contains("请先填写 AI API Key"));
    }

    #[test]
    fn validate_request_config_allows_local_ipv6_http_endpoint() {
        let provider = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: Some("http://[::1]:11434/v1".to_owned()),
            model: "custom-chat-model".to_owned(),
        })
        .expect("IPv6 本机 HTTP 地址应允许用于本地 AI 网关调试");

        assert_eq!(provider, "openai");
    }

    #[test]
    fn validate_request_config_allows_lan_http_endpoint() {
        for base_url in [
            "http://10.0.0.8:11434/v1",
            "http://172.16.3.4:11434/v1",
            "http://172.31.3.4:11434/v1",
            "http://192.168.1.10:11434/v1",
            "http://host.docker.internal:11434/v1",
        ] {
            let provider = validate_request_config(&AiRequestConfig {
                provider: "openai-compatible".to_owned(),
                api_key: "test-key".to_owned(),
                base_url: Some(base_url.to_owned()),
                model: "custom-chat-model".to_owned(),
            })
            .expect("局域网 HTTP 地址应允许用于 OpenAI 兼容服务");

            assert_eq!(provider, "openai");
        }
    }

    #[test]
    fn validate_request_config_requires_key_for_lan_http_endpoint() {
        let error = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: String::new(),
            base_url: Some("http://192.168.1.10:11434/v1".to_owned()),
            model: "custom-chat-model".to_owned(),
        })
        .expect_err("局域网 HTTP 服务仍应要求 API Key");

        assert!(error.contains("请先填写 AI API Key"));
    }

    #[test]
    fn validate_request_config_rejects_public_http_endpoint() {
        let error = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: Some("http://api.example.com/v1".to_owned()),
            model: "custom-chat-model".to_owned(),
        })
        .expect_err("公网 AI 请求地址必须使用 HTTPS");

        assert!(error.contains("AI 请求地址必须使用 https://"));
    }

    #[test]
    fn validate_request_config_rejects_userinfo_spoofed_local_http_endpoint() {
        let error = validate_request_config(&AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: Some("http://localhost@api.example.com/v1".to_owned()),
            model: "custom-chat-model".to_owned(),
        })
        .expect_err("带 userinfo 的 HTTP 地址不能伪装成本机调试地址");

        assert!(error.contains("AI 请求地址必须使用 https://"));
    }

    #[test]
    fn build_openai_chat_request_uses_custom_endpoint_model_and_key() {
        let request = build_openai_chat_request(
            &AiRequestConfig {
                provider: "openai-compatible".to_owned(),
                api_key: "  sk-test  ".to_owned(),
                base_url: Some("https://llm.example.com/v1/".to_owned()),
                model: "  custom-chat-model  ".to_owned(),
            },
            "请总结 README",
            false,
        );
        let body: serde_json::Value =
            serde_json::from_str(&request.body).expect("OpenAI 请求体必须是 JSON");

        assert_eq!(
            request.endpoint,
            "https://llm.example.com/v1/chat/completions"
        );
        assert_eq!(
            request.headers,
            vec![
                ("Authorization", "Bearer sk-test".to_owned()),
                ("Content-Type", "application/json".to_owned()),
            ]
        );
        assert_eq!(body["model"], "custom-chat-model");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "请总结 README");
    }

    #[test]
    fn build_openai_chat_request_uses_openai_custom_endpoint_model_and_key() {
        let request = build_openai_chat_request(
            &AiRequestConfig {
                provider: "openai".to_owned(),
                api_key: "  sk-openai-test  ".to_owned(),
                base_url: Some("https://openai-proxy.example.com/v1".to_owned()),
                model: "  gpt-custom  ".to_owned(),
            },
            "请总结 README",
            false,
        );
        let body: serde_json::Value =
            serde_json::from_str(&request.body).expect("OpenAI 请求体必须是 JSON");

        assert_eq!(
            request.endpoint,
            "https://openai-proxy.example.com/v1/chat/completions"
        );
        assert_eq!(
            request.headers,
            vec![
                ("Authorization", "Bearer sk-openai-test".to_owned()),
                ("Content-Type", "application/json".to_owned()),
            ]
        );
        assert_eq!(body["model"], "gpt-custom");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "请总结 README");
    }

    #[test]
    fn build_openai_chat_request_omits_authorization_for_keyless_local_service() {
        let request = build_openai_chat_request(
            &AiRequestConfig {
                provider: "openai-compatible".to_owned(),
                api_key: "  ".to_owned(),
                base_url: Some("http://localhost:11434/v1".to_owned()),
                model: "llama3.1".to_owned(),
            },
            "请总结 README",
            false,
        );

        assert_eq!(
            request.endpoint,
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            request.headers,
            vec![("Content-Type", "application/json".to_owned())]
        );
    }

    #[test]
    fn build_anthropic_message_request_uses_custom_endpoint_model_and_key() {
        let request = build_anthropic_message_request(
            &AiRequestConfig {
                provider: "anthropic".to_owned(),
                api_key: "  ant-test  ".to_owned(),
                base_url: Some("https://anthropic.example.com/v1/messages".to_owned()),
                model: "  claude-test  ".to_owned(),
            },
            "请总结 README",
            ANTHROPIC_SUMMARY_MAX_TOKENS,
            false,
        );
        let body: serde_json::Value =
            serde_json::from_str(&request.body).expect("Anthropic 请求体必须是 JSON");

        assert_eq!(
            request.endpoint,
            "https://anthropic.example.com/v1/messages"
        );
        assert_eq!(
            request.headers,
            vec![
                ("x-api-key", "ant-test".to_owned()),
                ("anthropic-version", "2023-06-01".to_owned()),
                ("Content-Type", "application/json".to_owned()),
            ]
        );
        assert_eq!(body["model"], "claude-test");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "请总结 README");
        assert_eq!(body["max_tokens"], ANTHROPIC_SUMMARY_MAX_TOKENS);
    }

    #[test]
    fn request_openai_accepts_content_parts_array() {
        let (url, request_handle) = spawn_http_server(
            "200 OK",
            r#"{"choices":[{"message":{"content":[{"type":"text","text":"{\"summary_zh\":\"第一段\"}"},{"type":"text","text":"\n{\"summary_zh\":\"第二段\"}"}]}}]}"#,
        );
        let content = request_openai(
            &AiRequestConfig {
                provider: "openai-compatible".to_owned(),
                api_key: "sk-test".to_owned(),
                base_url: Some(url),
                model: "custom-model".to_owned(),
            },
            "请总结 README",
        )
        .expect("OpenAI 兼容网关返回 content parts 数组时也应能读取文本");
        let _ = request_handle.join().expect("本地 HTTP 服务应完成请求");

        assert!(content.contains("第一段"));
        assert!(content.contains("第二段"));
    }

    #[test]
    fn request_anthropic_concatenates_text_blocks() {
        let (url, request_handle) = spawn_http_server(
            "200 OK",
            r#"{"content":[{"type":"text","text":"{\"summary_zh\":\"第一段\"}"},{"type":"text","text":"{\"summary_zh\":\"第二段\"}"}]}"#,
        );
        let content = request_anthropic(
            &AiRequestConfig {
                provider: "anthropic".to_owned(),
                api_key: "ant-test".to_owned(),
                base_url: Some(url),
                model: "claude-test".to_owned(),
            },
            "请总结 README",
            ANTHROPIC_SUMMARY_MAX_TOKENS,
        )
        .expect("Anthropic 多个 text block 应合并为完整文本");
        let _ = request_handle.join().expect("本地 HTTP 服务应完成请求");

        assert!(content.contains("第一段"));
        assert!(content.contains("第二段"));
    }

    #[test]
    fn summarize_readme_openai_compatible_posts_to_custom_chat_endpoint() {
        let (url, request_handle) = spawn_http_server(
            "200 OK",
            r#"{"choices":[{"message":{"content":"{\"summary_zh\":\"兼容接口摘要\",\"readme_zh\":\"README 中文梳理\",\"keywords\":[\"OpenAI\"],\"suggested_tags\":[\"AI\"]}"}}]}"#,
        );
        let document = summarize_readme(
            &AiRequestConfig {
                provider: "openai-compatible".to_owned(),
                api_key: "compat-key".to_owned(),
                base_url: Some(url),
                model: "compat-chat-model".to_owned(),
            },
            "owner/repo",
            Some("测试仓库"),
            "# README\n\n用于测试 OpenAI 兼容协议。",
        )
        .expect("OpenAI 兼容接口应能通过公开摘要入口生成结构化文档");
        let request = request_handle.join().expect("本地 HTTP 服务应完成请求");
        let lower_request = request.to_ascii_lowercase();
        let body = request_json_body(&request);

        assert_eq!(document.summary_zh, "兼容接口摘要");
        assert_eq!(document.readme_zh.as_deref(), Some("README 中文梳理"));
        assert_eq!(document.keywords, vec!["OpenAI"]);
        assert_eq!(document.suggested_tags, vec!["AI"]);
        assert_eq!(document.model, "compat-chat-model");
        assert_eq!(document.prompt_version, PROMPT_VERSION);
        assert!(request.starts_with("POST /test/chat/completions HTTP/1.1"));
        assert!(lower_request.contains("authorization: bearer compat-key"));
        assert_eq!(body["model"], "compat-chat-model");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
    }

    #[test]
    fn summarize_readme_anthropic_posts_to_messages_endpoint() {
        let (url, request_handle) = spawn_http_server(
            "200 OK",
            r#"{"content":[{"type":"text","text":"{\"summary_zh\":\"Claude 摘要\",\"readme_zh\":\"Claude README 梳理\",\"keywords\":[\"Claude\"],\"suggested_tags\":[\"Anthropic\"]}"}]}"#,
        );
        let document = summarize_readme(
            &AiRequestConfig {
                provider: "anthropic".to_owned(),
                api_key: "ant-key".to_owned(),
                base_url: Some(url),
                model: "claude-custom".to_owned(),
            },
            "owner/repo",
            None,
            "# README\n\n用于测试 Anthropic Messages API。",
        )
        .expect("Anthropic Messages API 应能通过公开摘要入口生成结构化文档");
        let request = request_handle.join().expect("本地 HTTP 服务应完成请求");
        let lower_request = request.to_ascii_lowercase();
        let body = request_json_body(&request);

        assert_eq!(document.summary_zh, "Claude 摘要");
        assert_eq!(document.readme_zh.as_deref(), Some("Claude README 梳理"));
        assert_eq!(document.keywords, vec!["Claude"]);
        assert_eq!(document.suggested_tags, vec!["Anthropic"]);
        assert_eq!(document.model, "claude-custom");
        assert_eq!(document.prompt_version, PROMPT_VERSION);
        assert!(request.starts_with("POST /test/messages HTTP/1.1"));
        assert!(lower_request.contains("x-api-key: ant-key"));
        assert!(lower_request.contains("anthropic-version: 2023-06-01"));
        assert_eq!(body["model"], "claude-custom");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["max_tokens"], ANTHROPIC_SUMMARY_MAX_TOKENS);
    }

    #[test]
    fn generate_tag_network_anthropic_uses_larger_output_budget() {
        let (url, request_handle) = spawn_http_server(
            "200 OK",
            r##"{"content":[{"type":"text","text":"{\"tags\":[{\"tag_name\":\"React 动画\",\"color\":\"#EC4899\",\"repository_full_names\":[\"pmndrs/react-spring\"]}]}"}]}"##,
        );
        let suggestions = generate_tag_network(
            &AiRequestConfig {
                provider: "anthropic".to_owned(),
                api_key: "ant-key".to_owned(),
                base_url: Some(url),
                model: "claude-custom".to_owned(),
            },
            &[AiTagRepositoryBrief {
                full_name: "pmndrs/react-spring".to_owned(),
                description: Some("Spring physics based React animation library".to_owned()),
                language: Some("TypeScript".to_owned()),
                topics: vec!["react".to_owned(), "animation".to_owned()],
                ai_summary: Some("用于 React 交互动画和弹簧效果。".to_owned()),
                suggested_tags: vec!["React 动画".to_owned()],
                stars_count: 28000,
            }],
        )
        .expect("Anthropic 标签网络应能返回结构化建议");
        let request = request_handle.join().expect("本地 HTTP 服务应完成请求");
        let body = request_json_body(&request);

        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].tag_name, "React 动画");
        assert_eq!(body["max_tokens"], ANTHROPIC_TAG_NETWORK_MAX_TOKENS);
    }

    #[test]
    fn execute_json_post_sends_headers_body_and_reads_status() {
        let (url, request_handle) = spawn_http_server("201 Created", r#"{"ok":true}"#);
        let response = execute_json_post(
            &url,
            &[
                ("Authorization", "Bearer sk-test".to_owned()),
                ("Content-Type", "application/json".to_owned()),
            ],
            r#"{"model":"custom-model"}"#,
        )
        .expect("AI POST 请求应成功读取本地 HTTP 响应");
        let request = request_handle.join().expect("本地 HTTP 服务应返回请求内容");
        let lower_request = request.to_ascii_lowercase();

        assert!(response.status_success);
        assert_eq!(response.http_code, Some(201));
        assert_eq!(response.body, r#"{"ok":true}"#);
        assert!(request.starts_with("POST /test HTTP/1.1"));
        assert!(lower_request.contains("authorization: bearer sk-test"));
        assert!(lower_request.contains("content-type: application/json"));
        assert!(request.contains(r#"{"model":"custom-model"}"#));
    }

    #[test]
    fn execute_json_post_keeps_non_success_status_and_body() {
        let (url, request_handle) =
            spawn_http_server("400 Bad Request", r#"{"error":{"message":"bad request"}}"#);
        let response = execute_json_post(
            &url,
            &[("Content-Type", "application/json".to_owned())],
            "{}",
        )
        .expect("HTTP 错误状态也应返回响应体供上层格式化");
        let _ = request_handle.join().expect("本地 HTTP 服务应完成请求");

        assert!(!response.status_success);
        assert_eq!(response.http_code, Some(400));
        assert!(response.body.contains("bad request"));
    }

    #[test]
    fn execute_json_post_retries_transient_server_errors() {
        let (url, request_handle) = spawn_http_sequence_server(vec![
            (
                "500 Internal Server Error",
                r#"{"error":{"message":"temporary"}}"#,
            ),
            ("200 OK", r#"{"ok":true}"#),
        ]);
        let response = execute_json_post(
            &url,
            &[("Content-Type", "application/json".to_owned())],
            "{}",
        )
        .expect("AI POST 遇到 5xx 临时错误时应自动重试");
        let requests = request_handle.join().expect("本地 HTTP 服务应完成请求");

        assert!(response.status_success);
        assert_eq!(response.http_code, Some(200));
        assert_eq!(response.body, r#"{"ok":true}"#);
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn extract_ai_error_detail_reads_gateway_error_arrays() {
        let detail = extract_ai_error_detail(
            r#"{"errors":[{"code":"bad_request","message":"model does not exist"}]}"#,
        );

        assert_eq!(detail, "model does not exist");
    }

    #[test]
    fn format_ai_http_error_reports_nested_token_limit() {
        let message = format_ai_http_error(
            "OpenAI",
            Some(400),
            r#"{"error":{"details":[{"message":"maximum context length exceeded"}]}}"#,
        );

        assert!(message.contains("README 内容超过模型上下文限制"));
    }

    #[test]
    fn build_github_recommendation_prompt_uses_selected_repository_briefs() {
        let prompt = build_github_recommendation_prompt(&[AiTagRepositoryBrief {
            full_name: "pmndrs/react-spring".to_owned(),
            description: Some("Spring physics based React animation library".to_owned()),
            language: Some("TypeScript".to_owned()),
            topics: vec!["react".to_owned(), "animation".to_owned()],
            ai_summary: Some("用于 React 交互动画和弹簧效果。".to_owned()),
            suggested_tags: vec!["React 动画".to_owned()],
            stars_count: 28000,
        }]);

        assert!(prompt.contains("GitHub Search API"));
        assert!(prompt.contains("pmndrs/react-spring"));
        assert!(prompt.contains("language: TypeScript"));
        assert!(prompt.contains("topic:"));
        assert!(prompt.contains("不要包含已给出的仓库 full_name"));
    }

    #[test]
    fn build_readme_translation_prompt_preserves_technical_markdown() {
        let prompt = build_readme_translation_prompt(
            "owner/project",
            "# Install\n\n```bash\npnpm install\n```",
        );

        assert!(prompt.contains("保留原有 Markdown 结构"));
        assert!(prompt.contains("代码块、命令、URL、API 名称和标识符保持原文"));
        assert!(prompt.contains("# Install"));
        assert!(prompt.contains("pnpm install"));
    }

    #[test]
    fn build_readme_translation_prompt_marks_truncated_input() {
        let readme = "a".repeat(MAX_README_CHARS + 1);
        let prompt = build_readme_translation_prompt("owner/project", &readme);

        assert!(prompt.contains("README 内容较长，本次翻译仅覆盖开头部分"));
        assert!(!prompt.contains(&readme));
    }

    #[test]
    fn parse_github_recommendation_plan_normalizes_queries() {
        let plan = parse_github_recommendation_plan(
            r#"```json
            {
              "rationale_zh": "根据 React 动画能力寻找替代项目。",
              "queries": [
                " react animation library stars:>1000 language:TypeScript ",
                "react animation library stars:>1000 language:TypeScript",
                "",
                "this query is intentionally made far longer than one hundred and eighty characters so it should be discarded before reaching the GitHub Search API because long generated queries are noisy and unreliable for this workflow"
              ]
            }
            ```"#,
        )
        .expect("推荐策略 JSON 应能解析");

        assert_eq!(plan.rationale_zh, "根据 React 动画能力寻找替代项目。");
        assert_eq!(
            plan.queries,
            vec!["react animation library stars:>1000 language:TypeScript"]
        );
    }

    #[test]
    fn parse_github_recommendation_plan_rejects_empty_queries() {
        let error = parse_github_recommendation_plan(r#"{"queries":[" ",""]}"#)
            .expect_err("空 GitHub 搜索策略必须失败");

        assert!(error.contains("AI 未返回可用的 GitHub 搜索查询"));
    }

    #[test]
    fn build_search_query_prompt_uses_context_and_schema() {
        let prompt = build_search_query_prompt(
            "有没有轻量的动画库",
            &["React 组件库".to_owned(), "弹簧效果".to_owned()],
        );

        assert!(prompt.contains("自然语言问题改写"));
        assert!(prompt.contains("\"search_query\""));
        assert!(prompt.contains("\"keywords\""));
        assert!(prompt.contains("React 组件库"));
        assert!(prompt.contains("弹簧效果"));
        assert!(prompt.contains("有没有轻量的动画库"));
    }

    #[test]
    fn build_search_answer_prompt_uses_only_public_result_evidence() {
        let prompt = build_search_answer_prompt(
            "找一个 React 动画库",
            &[AiSearchEvidence {
                repository_full_name: "pmndrs/react-spring".to_owned(),
                description: Some("Spring physics based animation library".to_owned()),
                topics: vec!["react".to_owned(), "animation".to_owned()],
                summary_zh: Some("适合构建 React 弹簧动画。".to_owned()),
            }],
        );

        assert!(prompt.contains("候选数量：1"));
        assert!(prompt.contains("pmndrs/react-spring"));
        assert!(prompt.contains("react、animation"));
        assert!(prompt.contains("适合构建 React 弹簧动画"));
        assert!(prompt.contains("repository_full_names"));
        assert!(prompt.contains("宁可少于 10 个或返回空结果"));
        assert!(!prompt.contains("个人笔记"));
    }

    #[test]
    fn parse_search_answer_keeps_only_real_candidates_in_ai_order() {
        let evidence = vec![
            AiSearchEvidence {
                repository_full_name: "owner/first".to_owned(),
                description: None,
                topics: Vec::new(),
                summary_zh: None,
            },
            AiSearchEvidence {
                repository_full_name: "owner/second".to_owned(),
                description: None,
                topics: Vec::new(),
                summary_zh: None,
            },
        ];
        let answer = parse_search_answer(
            r#"{
                "answer_zh":"优先使用 owner/second，其次使用 owner/first。",
                "repository_full_names":["OWNER/SECOND","owner/first"]
            }"#,
            &evidence,
        )
        .expect("结构化检索回答应可解析");

        assert_eq!(
            answer.repository_full_names,
            vec!["owner/second", "owner/first"]
        );
        assert!(answer.answer_zh.contains("owner/second"));
    }

    #[test]
    fn parse_search_answer_rejects_repository_outside_candidates() {
        let evidence = vec![AiSearchEvidence {
            repository_full_name: "owner/real".to_owned(),
            description: None,
            topics: Vec::new(),
            summary_zh: None,
        }];
        let error = parse_search_answer(
            r#"{
                "answer_zh":"推荐 owner/hallucinated。",
                "repository_full_names":["owner/hallucinated"]
            }"#,
            &evidence,
        )
        .expect_err("候选外仓库必须被拒绝");

        assert!(error.contains("候选列表外"));
    }

    #[test]
    fn parse_search_answer_accepts_plain_markdown_from_compatible_models() {
        let evidence = vec![
            AiSearchEvidence {
                repository_full_name: "SunkenCost/grok-regkit".to_owned(),
                description: None,
                topics: Vec::new(),
                summary_zh: None,
            },
            AiSearchEvidence {
                repository_full_name: "router-for-me/CLIProxyAPI".to_owned(),
                description: None,
                topics: Vec::new(),
                summary_zh: None,
            },
        ];
        let answer = parse_search_answer(
            "最相关的是：\n- **grok-regkit**：适合账号工具链。\n- **router-for-me/CLIProxyAPI**：适合 API 接入。",
            &evidence,
        )
        .expect("普通 Markdown 回答应能映射到真实候选");

        assert_eq!(
            answer.repository_full_names,
            vec!["SunkenCost/grok-regkit", "router-for-me/CLIProxyAPI"]
        );
    }

    #[test]
    fn parse_search_answer_accepts_explicit_plain_empty_result() {
        let evidence = vec![AiSearchEvidence {
            repository_full_name: "owner/unrelated".to_owned(),
            description: None,
            topics: Vec::new(),
            summary_zh: None,
        }];
        let answer = parse_search_answer("没有找到与问题高相关的仓库。", &evidence)
            .expect("明确的空结果回答应可解析");

        assert!(answer.repository_full_names.is_empty());
    }

    #[test]
    fn parse_search_plan_normalizes_ai_query() {
        let plan = parse_search_plan(
            r#"```json
            {
              "search_query": " React animation spring ",
              "keywords": ["React", "动画", "spring", "React"],
              "rationale_zh": "结合上下文寻找 React 动画库。"
            }
            ```"#,
            "动画库",
        )
        .expect("AI 搜索计划 JSON 应能解析");

        assert_eq!(plan.search_query, "React animation spring");
        assert_eq!(plan.keywords, vec!["React", "动画", "spring"]);
        assert_eq!(plan.rationale_zh, "结合上下文寻找 React 动画库。");
    }

    #[test]
    fn format_ai_http_error_reports_token_limit() {
        let message = format_ai_http_error(
            "OpenAI",
            Some(400),
            r#"{"error":{"message":"This model's maximum context length is 128000 tokens."}}"#,
        );

        assert!(message.contains("README 内容超过模型上下文限制"));
        assert!(message.contains("OpenAI"));
        assert!(message.contains("HTTP 400"));
    }

    #[test]
    fn format_ai_http_error_reports_payload_too_large_as_token_limit() {
        let message = format_ai_http_error("Anthropic", Some(413), "");

        assert!(message.contains("README 内容超过模型上下文限制"));
        assert!(message.contains("HTTP 413"));
    }

    #[test]
    fn format_ai_http_error_reads_common_gateway_error_shapes() {
        let message_error =
            format_ai_http_error("OpenAI", Some(401), r#"{"message":"invalid api key"}"#);
        let string_error =
            format_ai_http_error("OpenAI", Some(429), r#"{"error":"rate limit exceeded"}"#);

        assert!(message_error.contains("API Key 无效或权限不足"));
        assert!(message_error.contains("invalid api key"));
        assert!(string_error.contains("请求过于频繁或额度不足"));
        assert!(string_error.contains("rate limit exceeded"));
    }

    #[test]
    fn format_ai_http_error_reports_address_or_model_hint() {
        let message = format_ai_http_error(
            "OpenAI",
            Some(404),
            r#"{"error":{"message":"model not found"}}"#,
        );

        assert!(message.contains("请求地址或模型 ID 不存在"));
        assert!(message.contains("model not found"));
    }

    #[test]
    fn embed_text_sends_openai_compatible_request_and_parses_vector() {
        let (url, request_handle) = spawn_http_server(
            "200 OK",
            r#"{"data":[{"embedding":[0.1,0.2,0.3]}],"model":"embedding-test"}"#,
        );
        let config = EmbeddingRequestConfig {
            enabled: true,
            provider: "openai-compatible".to_owned(),
            download_source: None,
            api_key: String::new(),
            base_url: Some(url),
            model: "embedding-test".to_owned(),
            dimensions: 3,
            min_score: 0.72,
            max_results: 8,
        };

        let result = embed_text(&config, "Rust semantic search").expect("应解析 Embedding 响应");
        assert_eq!(result.vector, vec![0.1, 0.2, 0.3]);
        let request = request_handle.join().expect("测试服务应正常退出");
        assert!(request.starts_with("POST /test/embeddings HTTP/1.1"));
        assert!(request.contains("\"model\":\"embedding-test\""));
        assert!(request.contains("\"dimensions\":3"));
        assert!(request.contains("\"input\":\"Rust semantic search\""));
        assert!(!request.contains("Authorization:"));
    }

    #[test]
    fn embed_text_rejects_response_dimension_mismatch() {
        let (url, request_handle) = spawn_http_server(
            "200 OK",
            r#"{"data":[{"embedding":[0.1,0.2]}],"model":"embedding-test"}"#,
        );
        let config = EmbeddingRequestConfig {
            enabled: true,
            provider: "openai-compatible".to_owned(),
            download_source: None,
            api_key: String::new(),
            base_url: Some(url),
            model: "embedding-test".to_owned(),
            dimensions: 3,
            min_score: 0.72,
            max_results: 8,
        };

        let error = embed_text(&config, "dimension test").expect_err("维度不匹配必须失败");
        assert!(error.contains("配置为 3，服务返回 2"));
        request_handle.join().expect("测试服务应正常退出");
    }

    /// 等一个连接，最多 15 秒：客户端少发了请求（实现改动、被额度闸门拒绝）时，假服务别永远阻塞、把整个测试进程挂死。
    fn accept_within(listener: &TcpListener) -> Option<std::net::TcpStream> {
        listener.set_nonblocking(true).expect("应能设置非阻塞");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    // 有的系统上，接进来的连接会继承监听口的非阻塞模式
                    stream.set_nonblocking(false).expect("应能恢复阻塞");
                    return Some(stream);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return None;
                    }
                    thread::sleep(std::time::Duration::from_millis(2));
                }
                Err(error) => panic!("接收本地 HTTP 请求失败：{error}"),
            }
        }
    }

    fn spawn_http_server(
        status: &'static str,
        response_body: &'static str,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("应能启动本地测试 HTTP 服务");
        let address = listener.local_addr().expect("应能读取本地监听地址");
        let handle = thread::spawn(move || {
            let Some(mut stream) = accept_within(&listener) else {
                return String::new();
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .expect("应能设置读取超时");
            let request = read_http_request(&mut stream);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                response_body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("应能写入本地 HTTP 响应");
            request
        });

        (format!("http://{address}/test"), handle)
    }

    fn spawn_http_sequence_server(
        responses: Vec<(&'static str, &'static str)>,
    ) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("应能启动本地测试 HTTP 服务");
        let address = listener.local_addr().expect("应能读取本地监听地址");
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, response_body) in responses {
                let Some(mut stream) = accept_within(&listener) else {
                    break;
                };
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .expect("应能设置读取超时");
                requests.push(read_http_request(&mut stream));
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                    response_body.len()
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("应能写入本地 HTTP 响应");
            }
            requests
        });

        (format!("http://{address}/test"), handle)
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> String {
        let mut buffer = Vec::new();
        let mut chunk = [0; 1024];
        let mut expected_length = None;

        loop {
            let bytes_read = stream.read(&mut chunk).unwrap_or(0);
            if bytes_read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..bytes_read]);
            if expected_length.is_none() {
                expected_length = request_length_from_headers(&buffer);
            }
            if expected_length.is_some_and(|length| buffer.len() >= length) {
                break;
            }
        }

        String::from_utf8_lossy(&buffer).to_string()
    }

    fn request_length_from_headers(buffer: &[u8]) -> Option<usize> {
        let header_end = buffer.windows(4).position(|window| window == b"\r\n\r\n")?;
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);

        Some(header_end + 4 + content_length)
    }

    fn request_json_body(request: &str) -> serde_json::Value {
        let body = request
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .expect("HTTP 请求应包含 body");
        serde_json::from_str(body).expect("HTTP 请求 body 应为 JSON")
    }

    #[test]
    fn parse_openai_chat_reply_reads_usage_model_and_content() {
        let reply = parse_openai_chat_reply(
            r#"{"model":"gpt-test","choices":[{"message":{"content":"{\"ok\":true}"}}],"usage":{"prompt_tokens":12,"completion_tokens":34}}"#,
        )
        .expect("有用量的 OpenAI 响应应能解析");

        assert_eq!(reply.content, r#"{"ok":true}"#);
        assert_eq!(reply.input_tokens, Some(12));
        assert_eq!(reply.output_tokens, Some(34));
        assert_eq!(reply.model.as_deref(), Some("gpt-test"));
    }

    #[test]
    fn parse_openai_chat_reply_reads_content_parts() {
        let reply = parse_openai_chat_reply(
            r#"{"choices":[{"message":{"content":[{"type":"text","text":"第一段"},{"type":"text","text":"第二段"}]}}]}"#,
        )
        .expect("分段内容应拼成一段");

        assert_eq!(reply.content, "第一段\n第二段");
        assert_eq!(reply.input_tokens, None);
        assert_eq!(reply.output_tokens, None);
        assert_eq!(reply.model, None);
    }

    #[test]
    fn parse_openai_chat_reply_allows_missing_usage() {
        let reply = parse_openai_chat_reply(r#"{"choices":[{"message":{"content":"hello"}}]}"#)
            .expect("没有 usage 时仍应返回内容");

        assert_eq!(reply.content, "hello");
        assert_eq!(reply.input_tokens, None);
        assert_eq!(reply.output_tokens, None);
        assert_eq!(reply.model, None);
    }

    #[test]
    fn parse_openai_chat_reply_rejects_empty_content() {
        let error = parse_openai_chat_reply(
            r#"{"choices":[{"message":{"content":"  "}}],"usage":{"prompt_tokens":1,"completion_tokens":2}}"#,
        )
        .expect_err("空白内容应失败");

        assert_eq!(error.kind, ChatErrorKind::Other);
        assert!(error.message.contains("没有可用内容"));
    }

    #[test]
    fn empty_openai_content_explains_finish_reason_reasoning_and_usage() {
        let error = parse_openai_chat_reply(
            r#"{"choices":[{"finish_reason":"length","message":{"content":"","reasoning_content":"想了很多"}}],"usage":{"prompt_tokens":3000,"completion_tokens":2048}}"#,
        )
        .expect_err("空内容应失败");
        assert!(error.message.contains("没有可用内容"));
        assert!(error.message.contains("结束原因 length"));
        assert!(error.message.contains("只给了思考内容"));
        assert!(error.message.contains("2048"));
        // 不带任何响应正文
        assert!(!error.message.contains("想了很多"));

        let plain = parse_openai_chat_reply(r#"{"choices":[{"message":{"content":null}}]}"#)
            .expect_err("没有内容应失败");
        assert_eq!(plain.message, "OpenAI 响应中没有可用内容");
    }

    #[test]
    fn parse_anthropic_reply_reads_usage_and_joins_text() {
        let reply = parse_anthropic_reply(
            r#"{"model":"claude-test","content":[{"type":"text","text":"第一段"},{"type":"text","text":"第二段"}],"usage":{"input_tokens":8,"output_tokens":9}}"#,
        )
        .expect("有用量的 Anthropic 响应应能解析");

        assert_eq!(reply.content, "第一段\n第二段");
        assert_eq!(reply.input_tokens, Some(8));
        assert_eq!(reply.output_tokens, Some(9));
        assert_eq!(reply.model.as_deref(), Some("claude-test"));
    }

    #[test]
    fn parse_anthropic_reply_allows_missing_usage() {
        let reply = parse_anthropic_reply(r#"{"content":[{"type":"text","text":"只有正文"}]}"#)
            .expect("没有 usage 时仍应返回内容");

        assert_eq!(reply.content, "只有正文");
        assert_eq!(reply.input_tokens, None);
        assert_eq!(reply.output_tokens, None);
        assert_eq!(reply.model, None);
    }

    #[test]
    fn parse_anthropic_reply_rejects_empty_content() {
        let error = parse_anthropic_reply(r#"{"content":[{"type":"text","text":""}]}"#)
            .expect_err("空内容应失败");

        assert_eq!(error.kind, ChatErrorKind::Other);
        assert!(error.message.contains("没有可用内容"));
    }

    #[test]
    fn a_bigger_output_limit_gets_a_longer_wait() {
        // 思考型模型约 60 token/s：2048 → 不低于原来的 120 秒；8192 → 264 秒；16384 → 469 秒（原来固定 120 秒会掐断写到 7000 多 token 的调用）
        assert_eq!(card_chat::call_timeout_seconds(0), 120);
        assert_eq!(card_chat::call_timeout_seconds(2_048), 120);
        assert_eq!(card_chat::call_timeout_seconds(8_192), 264);
        assert_eq!(card_chat::call_timeout_seconds(16_384), 469);
        assert!(
            card_chat::call_timeout_seconds(8_192) > 8_192 / 65 + 60,
            "按每秒 65 token 写满 8192 也等得起"
        );
    }

    #[test]
    fn classify_http_failure_maps_rate_limit_and_retry_after() {
        let parsed = classify_http_failure(429, "slow down", Some("12"));
        assert_eq!(
            parsed.kind,
            ChatErrorKind::RateLimit {
                retry_after_secs: Some(12)
            }
        );

        let unparsed = classify_http_failure(429, "slow down", Some("not-a-number"));
        assert_eq!(
            unparsed.kind,
            ChatErrorKind::RateLimit {
                retry_after_secs: None
            }
        );

        let missing = classify_http_failure(429, "slow down", None);
        assert_eq!(
            missing.kind,
            ChatErrorKind::RateLimit {
                retry_after_secs: None
            }
        );
        assert!(missing.message.contains("HTTP 429"));
    }

    #[test]
    fn classify_http_failure_maps_auth() {
        for status in [401, 403] {
            let error = classify_http_failure(status, "nope", None);
            assert_eq!(error.kind, ChatErrorKind::Auth);
            assert!(error.message.contains(&status.to_string()));
        }
    }

    #[test]
    fn classify_http_failure_maps_context_timeout_network_and_other() {
        let overflow = classify_http_failure(
            400,
            "This model's maximum context length is 128000 tokens.",
            None,
        );
        assert_eq!(overflow.kind, ChatErrorKind::ContentTooLarge);

        let plain_bad_request = classify_http_failure(400, "bad request", None);
        assert_eq!(plain_bad_request.kind, ChatErrorKind::Other);

        let timed_out = classify_http_failure(408, "", None);
        assert_eq!(timed_out.kind, ChatErrorKind::Timeout);

        let timeout_text = classify_http_failure(499, "请求超时", None);
        assert_eq!(timeout_text.kind, ChatErrorKind::Timeout);

        let server = classify_http_failure(503, "unavailable", None);
        assert_eq!(server.kind, ChatErrorKind::Network);
        assert!(server.message.contains("HTTP 503"));

        let disconnected = classify_http_failure(0, "connection refused", None);
        assert_eq!(disconnected.kind, ChatErrorKind::Network);
        assert!(disconnected.message.contains("连接失败"));

        let other = classify_http_failure(418, "teapot", None);
        assert_eq!(other.kind, ChatErrorKind::Other);
    }

    #[test]
    fn classify_http_failure_message_omits_secret() {
        let secret = "sk-SUPER-SECRET-KEY-123456";
        let cases = [
            classify_http_failure(400, &format!("bad request {secret}"), None),
            classify_http_failure(401, secret, None),
            classify_http_failure(429, secret, Some(secret)),
            classify_http_failure(500, secret, None),
        ];
        for error in cases {
            assert!(
                !error.message.contains(secret),
                "错误信息不应包含密钥：{}",
                error.message
            );
            assert!(!error.message.contains("SUPER-SECRET"));
        }
    }

    #[test]
    fn usage_is_read_leniently_and_never_fails_a_good_reply() {
        use serde_json::json;
        assert_eq!(
            usage_from_json(
                &json!({"prompt_tokens": 120, "completion_tokens": 30}),
                "prompt_tokens",
                "completion_tokens"
            ),
            Some((120, 30))
        );
        // 小数、字符串、负数、null、整个 usage 是字符串：都不能让回复解析失败，只是改按文字估
        assert_eq!(
            usage_from_json(
                &json!({"prompt_tokens": 12.0, "completion_tokens": "7"}),
                "prompt_tokens",
                "completion_tokens"
            ),
            Some((12, 7))
        );
        for bad in [
            json!({"prompt_tokens": -1, "completion_tokens": null}),
            json!("n/a"),
            json!({}),
            json!({"prompt_tokens": 0, "completion_tokens": 0}),
        ] {
            assert_eq!(
                usage_from_json(&bad, "prompt_tokens", "completion_tokens"),
                None,
                "{bad}"
            );
        }
        let reply = r#"{"choices":[{"message":{"content":"ok"}}],"usage":"n/a"}"#;
        let parsed: OpenAiChatResponse =
            serde_json::from_str(reply).expect("usage 写成什么都不能让整个回复解析失败");
        assert_eq!(parsed.choices.len(), 1);
        // 离谱的大数不能在相加时 panic
        assert!(usage_pair(Some(u64::MAX), Some(u64::MAX)).is_some());
    }

    #[test]
    fn only_client_errors_are_known_to_be_unbilled() {
        for code in [400, 401, 402, 403, 404, 409, 422, 429, 451] {
            assert!(
                !status_may_be_billed(Some(code)),
                "HTTP {code} 在处理之前就被拒绝，不计费"
            );
        }
        for code in [408, 500, 502, 503, 504, 524] {
            assert!(
                status_may_be_billed(Some(code)),
                "HTTP {code} 不确定（408 常是代理等不到上游就回的），当作可能计费"
            );
        }
        assert!(status_may_be_billed(None), "不知道状态码，当作可能计费");
    }

    #[test]
    fn a_max_tokens_misconfiguration_is_not_blamed_on_the_content() {
        // 「max_tokens 比模型允许的输出上限还大」是参数配错了，缩短输入没用：不能当成「内容太长」把任务判成致命
        let anthropic = classify_http_failure(
            400,
            r#"{"type":"error","error":{"type":"invalid_request_error","message":"max_tokens: 100000 > 8192, which is the maximum allowed number of output tokens for claude-3-5-sonnet-20241022"}}"#,
            None,
        );
        assert_eq!(
            anthropic.kind,
            ChatErrorKind::Other,
            "{}",
            anthropic.message
        );
        let openai = classify_http_failure(
            400,
            r#"{"error":{"message":"max_tokens is too large: 40000. This model supports at most 16384 completion tokens, whereas you provided 40000.","type":"invalid_request_error","param":"max_tokens"}}"#,
            None,
        );
        assert_eq!(openai.kind, ChatErrorKind::Other, "{}", openai.message);
        // 两种都写固定说法：理解卡流水线靠它认出「该把输出额度降一档重试」
        for rejected in [&anthropic, &openai] {
            assert!(
                rejected.message.contains(OUTPUT_LIMIT_REJECTED),
                "{}",
                rejected.message
            );
        }
        // 别的 400（没提 max_tokens）不会被当成输出上限被拒；422 同样认
        let plain = classify_http_failure(400, r#"{"error":{"message":"bad request"}}"#, None);
        assert!(!plain.message.contains(OUTPUT_LIMIT_REJECTED));
        let unprocessable = classify_http_failure(
            422,
            r#"{"detail":"max_completion_tokens must be <= 8192"}"#,
            None,
        );
        assert!(unprocessable.message.contains(OUTPUT_LIMIT_REJECTED));
        // 真的是内容太长的几种写法照旧认得（包括同时提到 max_tokens 和上下文窗口的）
        for genuine in [
            "This model's maximum context length is 8192 tokens. However, your messages resulted in 9000 tokens.",
            "prompt is too long: 250000 tokens > 200000 maximum",
            "input length and `max_tokens` exceed context limit: 188240 + 21333 > 200000, decrease input length or `max_tokens` and try again",
            "请求超出上下文长度限制",
        ] {
            let error = classify_http_failure(400, genuine, None);
            assert_eq!(error.kind, ChatErrorKind::ContentTooLarge, "{genuine}");
        }
    }

    #[test]
    fn transport_errors_tell_unsent_requests_from_lost_ones() {
        let secs = Duration::from_secs;
        assert!(transport_error_is_unsent(true, false, secs(0)), "没连上");
        assert!(
            transport_error_is_unsent(true, true, secs(100)),
            "连接阶段的错误不管多久都是没发出去"
        );
        assert!(
            transport_error_is_unsent(false, true, secs(15)),
            "没有连接标记、但在建立连接的时限内超时：算没发出去"
        );
        assert!(
            !transport_error_is_unsent(false, true, secs(120)),
            "等响应等到超时：请求早就发出去了，可能已经计费"
        );
        assert!(
            !transport_error_is_unsent(false, false, secs(1)),
            "读响应时断了之类：请求发出去了"
        );
    }

    // —— 旧功能的记账：走真的请求函数，打到本地假服务，看账本里留下什么 ——

    const SPEND_BUDGET: &str = r#"{"dailyBudgetTokens":100000,"paused":false}"#;
    const SPEND_LATEST: &str = "SELECT status, input_tokens, output_tokens FROM ai_jobs WHERE target_id LIKE 'legacy-%' ORDER BY created_at DESC, rowid DESC LIMIT 1";
    const SPEND_REPLY: &str = r#"{"choices":[{"message":{"content":"好的"}}],"usage":{"prompt_tokens":700,"completion_tokens":50}}"#;
    const BUSY: (&str, &str) = ("503 Service Unavailable", r#"{"error":{"message":"busy"}}"#);

    fn loopback_config(url: String) -> AiRequestConfig {
        AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "sk-test".to_owned(),
            base_url: Some(url),
            model: "custom-model".to_owned(),
        }
    }

    /// 按顺序处理连进来的请求：`Some((状态, 内容))` 照常回复；`None` 收到完整请求后什么也不回就断开
    /// （服务商可能已经处理了、回复丢了）。返回每个请求的原文。
    fn spawn_mixed_server(
        steps: Vec<Option<(&'static str, &'static str)>>,
    ) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("应能启动本地测试 HTTP 服务");
        let address = listener.local_addr().expect("应能读取本地监听地址");
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for step in steps {
                let Some(mut stream) = accept_within(&listener) else {
                    break;
                };
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .expect("应能设置读取超时");
                requests.push(read_http_request(&mut stream));
                if let Some((status, body)) = step {
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("应能写入本地 HTTP 响应");
                }
            }
            requests
        });
        (format!("http://{address}/test"), handle)
    }

    /// 把「几秒内失败就算没计费」的时限改成 0：测试里假服务马上就回，要模拟「等了很久才失败」就用它。
    struct SlowFailures;

    impl SlowFailures {
        fn enter() -> Self {
            FAST_FAILURE_LIMIT.with(|limit| limit.set(Some(Duration::ZERO)));
            Self
        }
    }

    impl Drop for SlowFailures {
        fn drop(&mut self) {
            FAST_FAILURE_LIMIT.with(|limit| limit.set(None));
        }
    }

    /// 回完状态行和响应头（声明的 Content-Length 比实际多）就断开：响应体读到一半断了。返回真正回过的次数。
    fn spawn_truncated_server(
        connections: usize,
        status: &'static str,
        partial: &'static str,
    ) -> (String, thread::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("应能启动本地测试 HTTP 服务");
        let address = listener.local_addr().expect("应能读取本地监听地址");
        let handle = thread::spawn(move || {
            let mut served = 0;
            for _ in 0..connections {
                let Some(mut stream) = accept_within(&listener) else {
                    break;
                };
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                    .expect("应能设置读取超时");
                read_http_request(&mut stream);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{partial}",
                    partial.len() + 500
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("应能写入本地 HTTP 响应");
                served += 1;
            }
            served
        });
        (format!("http://{address}/test"), handle)
    }

    fn prompt_of_1000_tokens() -> String {
        "说".repeat(1000)
    }

    fn ask(url: String) -> Result<String, String> {
        request_openai(&loopback_config(url), &prompt_of_1000_tokens())
    }

    fn ask_streaming(url: String) -> Result<String, String> {
        let mut emit = |_: &str, _: &str, _: Option<&str>, _: Option<&str>, _: Option<&str>| {};
        request_openai_streaming(
            &loopback_config(url),
            &prompt_of_1000_tokens(),
            "summarize",
            &mut emit,
        )
    }

    fn ledger(dir: &crate::spend_guard::testing::Dir) -> (String, i64, i64) {
        dir.row(SPEND_LATEST)
    }

    #[test]
    fn response_billing_follows_status_and_speed() {
        let secs = Duration::from_secs;
        // 4xx 在处理之前就被拒绝，多慢都不计费
        assert!(!response_may_be_billed(Some(401), secs(60)));
        assert!(!response_may_be_billed(Some(429), secs(60)));
        // 几秒内回的 5xx / 408 是网关或过载保护挡下的；等了一阵才回的才可能已经计费
        assert!(!response_may_be_billed(Some(503), secs(1)));
        assert!(!response_may_be_billed(Some(408), secs(1)));
        assert!(response_may_be_billed(Some(504), secs(60)));
        assert!(response_may_be_billed(Some(408), secs(60)));
        assert!(
            response_may_be_billed(Some(503), secs(10)),
            "10 秒整就不算快了"
        );
        assert!(!response_may_be_billed(None, secs(1)));
        assert!(response_may_be_billed(None, secs(60)));
    }

    #[test]
    fn a_refused_connection_is_not_billed_and_gives_the_reservation_back() {
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        // 监听后立刻关掉：端口上没人，连接被拒绝
        let address = TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("应能占一个本地端口");
        let _slow = SlowFailures::enter();
        let error = ask(format!("http://{address}/test")).expect_err("没人监听应失败");
        assert!(error.contains("请求失败"), "{error}");
        assert_eq!(ledger(&dir), ("cancelled".to_owned(), 0, 0));
        assert_eq!(dir.used(), 0, "请求没到服务商那里，一分都不该留");
    }

    #[test]
    fn a_request_that_could_not_even_be_built_is_not_billed() {
        // 密钥里夹了控制字符：请求头拼不出来，根本没发出去
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let _slow = SlowFailures::enter();
        let mut config = loopback_config("http://127.0.0.1:9/test".to_owned());
        config.api_key = "sk-\u{7}broken".to_owned();
        let error =
            request_openai(&config, &prompt_of_1000_tokens()).expect_err("请求头不合法应失败");
        assert!(error.contains("请求失败"), "{error}");
        assert_eq!(ledger(&dir), ("cancelled".to_owned(), 0, 0));
    }

    #[test]
    fn a_connection_lost_after_a_long_wait_stays_on_the_books() {
        // 等了很久才断开：服务商可能已经处理了。三次尝试都是这样，每次都按提示词的估计记一笔
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let _slow = SlowFailures::enter();
        let (url, server) = spawn_mixed_server(vec![None; AI_API_MAX_ATTEMPTS]);
        let error = ask(url).expect_err("服务商一直不回应应失败");
        assert_eq!(
            server.join().expect("假服务应正常结束").len(),
            AI_API_MAX_ATTEMPTS
        );
        assert!(error.contains("请求失败"), "{error}");
        assert_eq!(
            ledger(&dir),
            ("cancelled".to_owned(), 3 * 1000, 0),
            "三次尝试都可能已经计费"
        );
    }

    #[test]
    fn a_connection_dropped_within_seconds_is_not_billed() {
        // 几秒内就被断开：网关 / 过载保护在模型开始跑之前挡下的。服务商故障时一批请求会连着这样失败，
        // 不能把每一次都记成花了钱（15 次就把 10 万额度锁死 24 小时）
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let (url, server) = spawn_mixed_server(vec![None; AI_API_MAX_ATTEMPTS]);
        let error = ask(url).expect_err("应失败");
        assert_eq!(
            server.join().expect("假服务应正常结束").len(),
            AI_API_MAX_ATTEMPTS
        );
        assert!(error.contains("请求失败"), "{error}");
        assert_eq!(ledger(&dir), ("cancelled".to_owned(), 0, 0));
        assert_eq!(dir.used(), 0);
    }

    #[test]
    fn a_rejection_after_an_earlier_slow_server_error_keeps_the_earlier_attempt() {
        // 第一次 503（等了很久，可能已计费）→ 重试被限流 429（没计费）→ 再试被 401 拒绝（没计费）：账本里只该有第一次那笔
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let _slow = SlowFailures::enter();
        let (url, server) = spawn_http_sequence_server(vec![
            BUSY,
            (
                "429 Too Many Requests",
                r#"{"error":{"message":"slow down"}}"#,
            ),
            ("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#),
        ]);
        let error = ask(url).expect_err("密钥错误应失败");
        assert_eq!(server.join().expect("假服务应正常结束").len(), 3);
        assert!(error.contains("401"), "{error}");
        assert_eq!(
            ledger(&dir),
            ("cancelled".to_owned(), 1000, 0),
            "只留 503 那次的提示词估计：后两次在处理之前就被拒绝"
        );
    }

    #[test]
    fn a_plain_rejection_gives_the_whole_reservation_back() {
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let _slow = SlowFailures::enter();
        let (url, server) =
            spawn_http_server("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#);
        let error = ask(url).expect_err("应失败");
        server.join().expect("假服务应正常结束");
        assert!(error.contains("401"), "{error}");
        assert_eq!(ledger(&dir), ("cancelled".to_owned(), 0, 0));
    }

    #[test]
    fn a_retry_that_succeeds_after_a_slow_failure_pays_for_it() {
        // 第一次 503（等了很久，可能已计费）、重试成功：账本里是两次的总和，不是只有成功那次
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let _slow = SlowFailures::enter();
        let (url, server) = spawn_http_sequence_server(vec![BUSY, ("200 OK", SPEND_REPLY)]);
        assert_eq!(ask(url).expect("重试后应成功"), "好的");
        server.join().expect("假服务应正常结束");
        assert_eq!(
            ledger(&dir),
            ("succeeded".to_owned(), 700 + 1000, 50),
            "成功那次的真实用量，加上 503 那次的提示词估计"
        );
    }

    #[test]
    fn a_retry_that_succeeds_after_a_fast_failure_costs_only_the_real_usage() {
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let (url, server) = spawn_http_sequence_server(vec![BUSY, ("200 OK", SPEND_REPLY)]);
        assert_eq!(ask(url).expect("重试后应成功"), "好的");
        server.join().expect("假服务应正常结束");
        assert_eq!(ledger(&dir), ("succeeded".to_owned(), 700, 50));
    }

    #[test]
    fn a_provider_outage_that_fails_fast_never_locks_the_budget() {
        // 服务商故障：一批请求都连着 503，每个请求 3 次尝试都立刻失败。10 个仓库这样失败也不该在账本里留下任何用量
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        for _ in 0..10 {
            let (url, server) = spawn_http_sequence_server(vec![BUSY; AI_API_MAX_ATTEMPTS]);
            let error = ask(url).expect_err("持续 503 应失败");
            server.join().expect("假服务应正常结束");
            assert!(error.contains("503"), "{error}");
        }
        assert_eq!(dir.used(), 0, "快速失败的 5xx 没有计费");
    }

    #[test]
    fn a_provider_outage_that_fails_slowly_is_paid_for_every_attempt() {
        // 每次都是等了很久才失败（网关超时）：三次尝试都可能已经计费，宁多勿少
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let _slow = SlowFailures::enter();
        let (url, server) = spawn_http_sequence_server(vec![BUSY; AI_API_MAX_ATTEMPTS]);
        let error = ask(url).expect_err("持续 503 应失败");
        assert_eq!(
            server.join().expect("假服务应正常结束").len(),
            AI_API_MAX_ATTEMPTS
        );
        assert!(error.contains("503"), "{error}");
        assert_eq!(ledger(&dir), ("cancelled".to_owned(), 3 * 1000, 0));
    }

    #[test]
    fn a_broken_stream_and_its_fallback_are_both_paid_for() {
        // 流式回复收到了状态行但内容读不懂（服务商已经开始生成）→ 回退到普通请求成功：两次都要记
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let (url, server) = spawn_http_sequence_server(vec![
            ("200 OK", "data: not-json\n\n"),
            ("200 OK", SPEND_REPLY),
        ]);
        assert_eq!(ask_streaming(url).expect("回退的普通请求应成功"), "好的");
        server.join().expect("假服务应正常结束");
        assert_eq!(
            ledger(&dir),
            ("succeeded".to_owned(), 700 + 1000, 50),
            "回退成功那次的真实用量，加上流式那次的提示词估计"
        );
    }

    #[test]
    fn a_stream_the_provider_refused_is_not_paid_for() {
        // 很多网关对 stream 直接回 400：没有计费，回退的普通请求只记它自己的用量
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let _slow = SlowFailures::enter();
        let (url, server) = spawn_http_sequence_server(vec![
            (
                "400 Bad Request",
                r#"{"error":{"message":"stream is not supported"}}"#,
            ),
            ("200 OK", SPEND_REPLY),
        ]);
        ask_streaming(url).expect("回退的普通请求应成功");
        server.join().expect("假服务应正常结束");
        assert_eq!(ledger(&dir), ("succeeded".to_owned(), 700, 50));
    }

    #[test]
    fn a_broken_stream_followed_by_a_rejected_fallback_keeps_only_the_stream() {
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let (url, server) = spawn_http_sequence_server(vec![
            ("200 OK", "data: not-json\n\n"),
            ("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#),
        ]);
        let error = ask_streaming(url).expect_err("两次都失败");
        server.join().expect("假服务应正常结束");
        assert!(error.contains("普通请求也失败"), "{error}");
        assert_eq!(
            ledger(&dir),
            ("cancelled".to_owned(), 1000, 0),
            "流式那次可能已计费，被拒的回退请求没有"
        );
    }

    #[test]
    fn a_stream_that_failed_with_a_slow_server_error_is_paid_for_a_fast_one_is_not() {
        let slow = {
            let dir = crate::spend_guard::testing::Dir::new();
            dir.control(SPEND_BUDGET);
            let _scope = dir.use_in_this_thread();
            let _slow = SlowFailures::enter();
            let (url, server) = spawn_http_sequence_server(vec![BUSY, ("200 OK", SPEND_REPLY)]);
            ask_streaming(url).expect("回退的普通请求应成功");
            server.join().expect("假服务应正常结束");
            ledger(&dir)
        };
        assert_eq!(
            slow,
            ("succeeded".to_owned(), 700 + 1000, 50),
            "等了很久才 503 的流式请求可能已经计费"
        );
        let fast = {
            let dir = crate::spend_guard::testing::Dir::new();
            dir.control(SPEND_BUDGET);
            let _scope = dir.use_in_this_thread();
            let (url, server) = spawn_http_sequence_server(vec![BUSY, ("200 OK", SPEND_REPLY)]);
            ask_streaming(url).expect("回退的普通请求应成功");
            server.join().expect("假服务应正常结束");
            ledger(&dir)
        };
        assert_eq!(
            fast,
            ("succeeded".to_owned(), 700, 50),
            "几秒内就 503 的流式请求没有计费"
        );
    }

    #[test]
    fn a_stream_connection_lost_after_a_long_wait_is_paid_for_a_fast_drop_is_not() {
        let slow = {
            let dir = crate::spend_guard::testing::Dir::new();
            dir.control(SPEND_BUDGET);
            let _scope = dir.use_in_this_thread();
            let _slow = SlowFailures::enter();
            let (url, server) = spawn_mixed_server(vec![None, Some(("200 OK", SPEND_REPLY))]);
            ask_streaming(url).expect("回退的普通请求应成功");
            server.join().expect("假服务应正常结束");
            ledger(&dir)
        };
        assert_eq!(slow, ("succeeded".to_owned(), 700 + 1000, 50));
        let fast = {
            let dir = crate::spend_guard::testing::Dir::new();
            dir.control(SPEND_BUDGET);
            let _scope = dir.use_in_this_thread();
            let (url, server) = spawn_mixed_server(vec![None, Some(("200 OK", SPEND_REPLY))]);
            ask_streaming(url).expect("回退的普通请求应成功");
            server.join().expect("假服务应正常结束");
            ledger(&dir)
        };
        assert_eq!(fast, ("succeeded".to_owned(), 700, 50));
    }

    #[test]
    fn anthropic_requests_are_accounted_like_openai_ones() {
        // 同一个 execute_json_post：被拒绝 / 快速失败退回，慢失败和重试成功都要记，不能因为换了服务商就漏
        const ANTHROPIC_REPLY: &str = r#"{"content":[{"type":"text","text":"好的"}],"usage":{"input_tokens":700,"output_tokens":50}}"#;
        let ask_anthropic = |url: String| {
            request_anthropic(
                &AiRequestConfig {
                    provider: "anthropic".to_owned(),
                    api_key: "ant-test".to_owned(),
                    base_url: Some(url),
                    model: "claude-test".to_owned(),
                },
                &prompt_of_1000_tokens(),
                ANTHROPIC_SUMMARY_MAX_TOKENS,
            )
        };
        let run = |slow: bool, steps: Vec<(&'static str, &'static str)>| {
            let dir = crate::spend_guard::testing::Dir::new();
            dir.control(SPEND_BUDGET);
            let _scope = dir.use_in_this_thread();
            let _slow = slow.then(SlowFailures::enter);
            let (url, server) = spawn_http_sequence_server(steps);
            let result = ask_anthropic(url);
            server.join().expect("假服务应正常结束");
            (result, ledger(&dir))
        };
        // 密钥错误：整笔退回
        let (result, row) = run(
            true,
            vec![("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#)],
        );
        assert!(result.expect_err("应失败").contains("401"));
        assert_eq!(row, ("cancelled".to_owned(), 0, 0));
        // 持续快速 503：没有计费；持续慢 503：三次尝试都记
        let (result, row) = run(false, vec![BUSY; AI_API_MAX_ATTEMPTS]);
        assert!(result.expect_err("应失败").contains("503"));
        assert_eq!(row, ("cancelled".to_owned(), 0, 0));
        let (result, row) = run(true, vec![BUSY; AI_API_MAX_ATTEMPTS]);
        assert!(result.expect_err("应失败").contains("503"));
        assert_eq!(row, ("cancelled".to_owned(), 3 * 1000, 0));
        // 慢 503 后重试成功：真实用量加上失败那次
        let (result, row) = run(true, vec![BUSY, ("200 OK", ANTHROPIC_REPLY)]);
        assert_eq!(result.expect("重试后应成功"), "好的");
        assert_eq!(row, ("succeeded".to_owned(), 700 + 1000, 50));
    }

    #[test]
    fn a_failed_status_whose_body_cannot_be_read_is_still_released() {
        // 401 / 503 的响应体读到一半断了：照样按状态码处理——401 退回，503 重试到终败（快失败）后也退回
        for (status, connections) in [
            ("401 Unauthorized", 1),
            ("503 Service Unavailable", AI_API_MAX_ATTEMPTS),
        ] {
            let dir = crate::spend_guard::testing::Dir::new();
            dir.control(SPEND_BUDGET);
            let _scope = dir.use_in_this_thread();
            let (url, server) = spawn_truncated_server(connections, status, r#"{"error":"#);
            let error = ask(url).expect_err("应失败");
            assert_eq!(server.join().expect("假服务应正常结束"), connections);
            assert!(error.contains(&status[..3]), "{status}：{error}");
            assert_eq!(ledger(&dir), ("cancelled".to_owned(), 0, 0), "{status}");
        }
    }

    #[test]
    fn a_success_status_whose_body_cannot_be_read_stays_on_the_books() {
        // 200 的响应体读到一半断了：服务商已经生成完了，只是我们没收到——不能退回，留提示词那部分
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(SPEND_BUDGET);
        let _scope = dir.use_in_this_thread();
        let (url, server) = spawn_truncated_server(1, "200 OK", r#"{"choices":["#);
        let error = ask(url).expect_err("应失败");
        assert_eq!(server.join().expect("假服务应正常结束"), 1);
        assert!(error.contains("响应读取失败"), "{error}");
        assert_eq!(ledger(&dir), ("cancelled".to_owned(), 1000, 0));
    }

    #[test]
    fn rate_limits_and_server_errors_are_not_mistaken_for_a_too_long_readme() {
        // 429 的「每分钟 token 数」限流、403 / 503 里提到 token 限额：说的是别的限额，不是「README 太长」
        for (code, body) in [
            (
                429,
                r#"{"error":{"message":"Rate limit reached for model on tokens per min (TPM): Limit 30000, Used 29000"}}"#,
            ),
            (
                403,
                r#"{"error":{"message":"daily token limit reached for this key"}}"#,
            ),
            (
                503,
                r#"{"error":{"message":"token limit of the shared pool exceeded, retry later"}}"#,
            ),
        ] {
            let message = format_ai_http_error("OpenAI", Some(code), body);
            assert!(
                !message.contains("README 内容超过模型上下文限制"),
                "HTTP {code}：{message}"
            );
            assert!(message.contains(&format!("HTTP {code}")), "{message}");
        }
        // 400 / 413 / 422 里真的写着上下文超限，照旧认得
        for code in [400, 413, 422] {
            let message = format_ai_http_error(
                "OpenAI",
                Some(code),
                r#"{"error":{"message":"This model's maximum context length is 8192 tokens"}}"#,
            );
            assert!(
                message.contains("README 内容超过模型上下文限制"),
                "{message}"
            );
        }
    }

    #[test]
    fn a_repositorys_own_problems_are_told_from_engine_failures_in_real_messages() {
        let own = [
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"Content Exists Risk"}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"This model's maximum context length is 8192 tokens"}}"#,
            ),
            format_ai_http_error("Anthropic", Some(413), "request too large"),
            format_ai_http_error("Anthropic", Some(422), "unprocessable"),
        ];
        for message in &own {
            assert!(is_repository_specific_failure(message), "{message}");
        }
        // 引擎坏了的那类：密钥、额度、地址、限流、服务故障、断网、账户余额——连着出现就该停
        let engine = [
            format_ai_http_error("OpenAI", Some(401), "bad key"),
            format_ai_http_error("OpenAI", Some(402), "payment required"),
            format_ai_http_error("OpenAI", Some(404), "model not found"),
            format_ai_http_error(
                "OpenAI",
                Some(429),
                r#"{"error":{"message":"Rate limit reached for tokens per min (TPM): Limit 30000"}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(503),
                r#"{"error":{"message":"token limit of the shared pool exceeded"}}"#,
            ),
            format_ai_http_error(
                "Anthropic",
                Some(400),
                r#"{"error":{"message":"Your credit balance is too low to access the Anthropic API."}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"账户余额不足，请先充值"}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"套餐额度已用完"}}"#,
            ),
            format_ai_http_error("OpenAI", Some(400), r#"{"error":{"message":"账户已欠费"}}"#),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"Out of credit for this project"}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"Your balance is zero"}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"Please update your billing information"}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"Daily quota exhausted"}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"insufficient funds"}}"#,
            ),
            format_ai_http_error(
                "OpenAI",
                Some(400),
                r#"{"error":{"message":"Payment required for this model"}}"#,
            ),
            "AI 接口请求超时，请检查请求地址、模型 ID 或网络连接（已等待 120 秒）。".to_owned(),
            "AI 接口请求失败：error sending request".to_owned(),
        ];
        for message in &engine {
            assert!(!is_repository_specific_failure(message), "{message}");
        }
    }
}
