//! 理解卡任务：把队列、流水线和场景归类接成一条线。
//! 预算默认是 0。没有授权时只入队，处理器不会被领走，也就不会读密钥。

use crate::ai::AiRequestConfig;
use crate::card_pipeline::{
    self, CardRunError, CardRunErrorKind, CardRunOutcome, LlmClient, RealLlmClient,
};
use crate::jobs::{self, ErrorKind, JobFailure, JobHandler, JobRecord, JobSuccess};
use crate::scenarios;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const NOT_CONFIGURED_MESSAGE: &str = "还没有配置 AI 服务或密钥，已暂停整理队列";
const BACKFILL_PRIORITY: i64 = 6;
const NEW_STAR_PRIORITY: i64 = 3;
const REGENERATE_PRIORITY: i64 = 2;
/// Fox 确认「重新整理这一张」之后，这一行可以不看每天额度的时长（秒）。
const ONE_OFF_AUTHORIZATION_SECS: i64 = 1800;
/// 空串和坏 JSON 上直接 `json_extract` 会让整句失败。`CASE` 只在合法 JSON 上取值。
/// 「说明过期」：有卡，但结构还是第 2 版之前的，或者是旧版提示词写的（提示词升版后，旧说明的文风、场景标签口径都变了）。
fn outdated_sql() -> String {
    format!(
        "IFNULL(trim(repo_ai_documents.card_json), '') <> '' AND (IFNULL(repo_ai_documents.card_schema_version, 0) < 2 OR IFNULL(repo_ai_documents.prompt_version, '') <> '{}')",
        card_pipeline::PROMPT_VERSION
    )
}

pub(crate) const NOT_CONFIRMED_SQL: &str = "CASE WHEN json_valid(repo_ai_documents.card_json) THEN IFNULL(json_extract(repo_ai_documents.card_json, '$.review_status'), '') ELSE '' END <> 'confirmed'";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardEnqueueScope {
    /// 有效仓库里还没有理解卡。
    Missing,
    /// 有卡，但还是第 2 版之前的结构，或者是旧版提示词（`card_pipeline::PROMPT_VERSION` 之前的）写的，而且没有被确认。
    Outdated,
}

type ConfigLoader = Arc<dyn Fn() -> Result<AiRequestConfig, String> + Send + Sync>;
type ClientFactory = Arc<dyn Fn() -> Box<dyn LlmClient + Send> + Send + Sync>;

/// `run_repo_card` 要求客户端是有固定大小的类型。测试注入的是 trait 对象，这里包一层。
struct SizedClient(Box<dyn LlmClient + Send>);

impl LlmClient for SizedClient {
    fn complete_json(
        &self,
        request: &card_pipeline::LlmRequest,
    ) -> Result<card_pipeline::LlmReply, card_pipeline::LlmError> {
        self.0.complete_json(request)
    }
}

/// 连续这么多个请求都被服务商直接拒绝（HTTP 4xx），整条队列就停下来。
/// 一个错的参数 / 一个过期的账户会让积压的几百个任务一个个瞬间失败（都不计费），白白把它们全标成失败。
const CONSECUTIVE_REJECTIONS_BEFORE_PAUSE: u32 = 5;
/// 同一套配置（服务商、模型、地址）已经成功跑过之后，零星的 4xx 多半是某几个仓库自己的问题
/// （比如内容被服务商审核拦下），阈值放宽；但连着这么多个还是不对劲（比如 Anthropic 余额没了也回 400）。
const CONSECUTIVE_REJECTIONS_BEFORE_PAUSE_ONCE_PROVEN: u32 = 20;

/// 「同一套配置」的标识：换了服务商、模型或地址，之前的成功就不再证明什么。
fn config_fingerprint(config: &AiRequestConfig) -> String {
    format!(
        "{}|{}|{}",
        config.provider.trim(),
        config.model.trim(),
        config.base_url.as_deref().unwrap_or("").trim()
    )
}

pub struct RepoCardHandler {
    config: ConfigLoader,
    client_factory: Option<ClientFactory>,
    /// 连续被服务商拒绝的次数（线程之间共用）。任何一次成功或别的失败都会清零。
    rejections: AtomicU32,
    /// 真的调过模型并成功的那套配置（见 `config_fingerprint`）。跳过的（卡已经是最新、没调模型）不算。
    proven: Mutex<Option<String>>,
}

impl RepoCardHandler {
    pub fn new(config: ConfigLoader) -> Self {
        Self {
            config,
            client_factory: None,
            rejections: AtomicU32::new(0),
            proven: Mutex::new(None),
        }
    }

    /// 测试注入假模型。正式构建不会调用，所以只在测试里编译。
    #[cfg(test)]
    pub fn with_client_factory(config: ConfigLoader, client_factory: ClientFactory) -> Self {
        Self {
            config,
            client_factory: Some(client_factory),
            rejections: AtomicU32::new(0),
            proven: Mutex::new(None),
        }
    }

    fn is_proven(&self, fingerprint: &str) -> bool {
        self.proven
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_deref()
            == Some(fingerprint)
    }

    fn mark_proven(&self, fingerprint: String) {
        *self
            .proven
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(fingerprint);
    }

    /// 数「连续被拒绝」：被服务商拒绝（不计费的 4xx）的累加，够数了就把这一次升级成暂停整条队列；别的结果清零。
    /// `proven`：这套配置已经真的成功过，阈值放宽（零星的内容被拒绝不该把整条队列停掉）。
    fn track_rejections(&self, failure: JobFailure, proven: bool) -> JobFailure {
        let rejected = matches!(
            &failure,
            JobFailure::Fatal {
                kind: ErrorKind::ContentTooLarge,
                message,
            } if http_status_in(message).is_some_and(|status| (400..500).contains(&status))
                // 真正的「内容超过模型上下文」是这一条自己的问题（一个特别长的 README），不是配置错了
                && !message.contains("超过模型上下文")
        );
        if !rejected {
            self.rejections.store(0, Ordering::SeqCst);
            return failure;
        }
        let count = self.rejections.fetch_add(1, Ordering::SeqCst) + 1;
        let limit = if proven {
            CONSECUTIVE_REJECTIONS_BEFORE_PAUSE_ONCE_PROVEN
        } else {
            CONSECUTIVE_REJECTIONS_BEFORE_PAUSE
        };
        if count < limit {
            return failure;
        }
        self.rejections.store(0, Ordering::SeqCst);
        let JobFailure::Fatal { message, .. } = failure else {
            return failure;
        };
        JobFailure::PauseQueue {
            kind: ErrorKind::Other,
            message: format!(
                "连续 {count} 个请求都被 AI 服务拒绝，先停下整理：{message} 请检查模型、请求地址、参数和账户余额，改好后点「继续整理」。"
            ),
        }
    }
}

impl JobHandler for RepoCardHandler {
    fn job_type(&self) -> &'static str {
        "repo_card"
    }

    fn spends_tokens(&self) -> bool {
        true
    }

    fn run(&self, job: &JobRecord, db_path: &Path) -> Result<JobSuccess, JobFailure> {
        if job.target_type != "repository" || job.target_id.trim().is_empty() {
            return Err(JobFailure::Fatal {
                kind: ErrorKind::Other,
                message: "任务目标不是仓库".to_owned(),
            });
        }
        let repo_id = job.target_id.trim();
        let config = match (self.config)() {
            Ok(config) if config_ready(&config) => config,
            _ => {
                return Err(JobFailure::PauseQueue {
                    kind: ErrorKind::Auth,
                    message: NOT_CONFIGURED_MESSAGE.to_owned(),
                })
            }
        };
        let client = SizedClient(match &self.client_factory {
            Some(factory) => factory(),
            None => Box::new(RealLlmClient {
                config: config.clone(),
            }),
        });
        let mut conn = open_db(db_path).map_err(|message| JobFailure::Retryable {
            kind: ErrorKind::Other,
            message,
            retry_after_secs: None,
        })?;
        let fingerprint = config_fingerprint(&config);
        let proven = self.is_proven(&fingerprint);
        let report = card_pipeline::run_repo_card(
            &mut conn,
            &client,
            repo_id,
            &card_pipeline::CardRunOptions {
                force: false,
                now: now_stamp(),
            },
        )
        .map_err(|error| {
            // 第一次调用已经计费、修复调用才失败：把已经花掉的记到这条任务上，
            // 否则网络 / 限流这类「没花钱」的失败会把它丢掉
            if error.spent_tokens > 0 {
                let _ = jobs::add_charge(db_path, &job.id, error.spent_tokens);
            }
            self.track_rejections(map_card_error(error), proven)
        })?;
        self.rejections.store(0, Ordering::SeqCst);
        // 只有真的调过模型并成功才算证明过这套配置；卡已经是最新、直接跳过的不算
        if report.input_tokens.saturating_add(report.output_tokens) > 0 {
            self.mark_proven(fingerprint);
        }
        if let Err(message) = apply_scenario_tags(&mut conn, &job.account_id, repo_id) {
            return Err(JobFailure::Retryable {
                kind: ErrorKind::Other,
                message,
                retry_after_secs: None,
            });
        }
        let model = if report.model.trim().is_empty() {
            config.model
        } else {
            report.model
        };
        Ok(JobSuccess {
            input_tokens: report.input_tokens,
            output_tokens: report.output_tokens,
            provider: config.provider,
            model,
            prompt_version: report.prompt_version,
            input_hash: report.source_hash,
            skipped: matches!(report.outcome, CardRunOutcome::Skipped(_)),
        })
    }
}

pub fn enqueue_cards(
    conn: &mut Connection,
    scope: CardEnqueueScope,
    priority: i64,
) -> Result<usize, String> {
    if !(0..=9).contains(&priority) {
        return Err("任务优先级必须在 0 到 9 之间".to_owned());
    }
    let tx = conn
        .transaction()
        .map_err(|error| format!("理解卡入队失败：{error}"))?;
    let inserted = insert_scoped(&tx, scope, priority)?;
    tx.commit()
        .map_err(|error| format!("理解卡入队失败：{error}"))?;
    Ok(inserted)
}

pub fn enqueue_new_stars(db_path: &Path, repo_ids: &[String]) -> Result<usize, String> {
    if repo_ids.is_empty() {
        return Ok(0);
    }
    let mut conn = open_db(db_path)?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("新收藏入队失败：{error}"))?;
    let mut inserted = 0;
    for chunk in repo_ids.chunks(400) {
        let mut sql = format!(
            "INSERT OR IGNORE INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             SELECT lower(hex(randomblob(8))), repositories.account_id, 'repo_card', 'repository', repositories.id, {NEW_STAR_PRIORITY}, 'queued'
             FROM repositories
             LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
             WHERE repositories.sync_status = 'active'
               AND repositories.id IN ("
        );
        sql.push_str(
            &std::iter::repeat("?")
                .take(chunk.len())
                .collect::<Vec<_>>()
                .join(", "),
        );
        sql.push_str(
            ") AND (repo_ai_documents.repo_id IS NULL OR IFNULL(trim(repo_ai_documents.card_json), '') = '')
               AND ",
        );
        sql.push_str(NOT_CONFIRMED_SQL);
        inserted += tx
            .execute(&sql, params_from_iter(chunk.iter()))
            .map_err(|error| format!("新收藏入队失败：{error}"))?;
    }
    tx.commit()
        .map_err(|error| format!("新收藏入队失败：{error}"))?;
    Ok(inserted)
}

pub fn confirm_card(db_path: &Path, repo_id: &str) -> Result<(), String> {
    let mut conn = open_db(db_path)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| format!("理解卡确认失败：{error}"))?;
    let card_json = read_card_json(&tx, repo_id)?;
    let updated = rewrite_review_status(card_json.as_deref(), "confirmed")?;
    let changed = tx
        .execute(
            "UPDATE repo_ai_documents SET card_json = ?1 WHERE repo_id = ?2",
            params![updated, repo_id],
        )
        .map_err(|error| format!("理解卡确认失败：{error}"))?;
    if changed == 0 {
        return Err("这个仓库还没有理解卡".to_owned());
    }
    tx.execute(
        "UPDATE entity_categories
         SET status = 'confirmed', origin = 'user'
         WHERE entity_type = 'repository' AND entity_id = ?1
           AND is_primary = 1 AND status <> 'rejected'",
        params![repo_id],
    )
    .map_err(|error| format!("分类确认失败：{error}"))?;
    tx.commit()
        .map_err(|error| format!("理解卡确认失败：{error}"))?;
    Ok(())
}

/// 重新整理这一张。必须 `confirmed`：没确认、没配置 AI、队列暂停时数据库一个字不动。
/// 确认后在一个 `BEGIN IMMEDIATE` 里把理解卡改回待确认，并只给这一张 `repo_card` 写上 30 分钟授权。
pub fn request_regenerate(
    db_path: &Path,
    settings: &Value,
    repo_id: &str,
    confirmed: bool,
) -> Result<(), String> {
    if !confirmed {
        return Err("重新整理这一张会花 token，需要你在界面上确认之后才能开始。".to_owned());
    }
    let (provider_configured, _) = provider_from_settings(settings);
    if !provider_configured {
        return Err(
            "还没配置 AI 服务，先到「设置 → AI 设置」里选好服务商和模型、保存密钥。".to_owned(),
        );
    }
    if jobs::read_control(&jobs::queue_control_path(db_path)).paused {
        return Err("自动整理现在是暂停的，先到「自动整理」里恢复，再重新整理这一张。".to_owned());
    }
    let now = jobs::now_db_time();
    let until = jobs::shift_db_time(&now, ONE_OFF_AUTHORIZATION_SECS)?;
    let mut conn = open_db(db_path)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| format!("理解卡重做失败：{error}"))?;
    let account_id: Option<String> = tx
        .query_row(
            "SELECT account_id FROM repositories WHERE id = ?1",
            params![repo_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("仓库读取失败：{error}"))?;
    let Some(account_id) = account_id else {
        return Err("找不到这个仓库".to_owned());
    };
    let card_json = read_card_json(&tx, repo_id)?;
    let updated = rewrite_review_status(card_json.as_deref(), "suggested")?;
    let running: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM ai_jobs
             WHERE job_type = 'repo_card' AND target_type = 'repository' AND target_id = ?1 AND status = 'running'",
            params![repo_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("理解卡重做失败：{error}"))?;
    if running > 0 {
        return Err("这一张正在整理中，等它结束再试。".to_owned());
    }
    let changed = tx
        .execute(
            "UPDATE repo_ai_documents SET card_json = ?1, source_hash = '' WHERE repo_id = ?2",
            params![updated, repo_id],
        )
        .map_err(|error| format!("理解卡重做失败：{error}"))?;
    if changed == 0 {
        return Err("这个仓库还没有理解卡".to_owned());
    }
    let updated_jobs = tx
        .execute(
            "UPDATE ai_jobs
             SET priority = MIN(priority, ?1),
                 spend_authorized_until = ?2,
                 not_before = NULL,
                 attempts = 0,
                 error_kind = NULL,
                 error_message = NULL,
                 updated_at = ?3
             WHERE job_type = 'repo_card'
               AND target_type = 'repository'
               AND target_id = ?4
               AND status = 'queued'",
            params![REGENERATE_PRIORITY, until, now, repo_id],
        )
        .map_err(|error| format!("理解卡重做入队失败：{error}"))?;
    if updated_jobs == 0 {
        tx.execute(
            "INSERT INTO ai_jobs(
                id, account_id, job_type, target_type, target_id, priority, status, spend_authorized_until
             ) VALUES (
                lower(hex(randomblob(8))), ?1, 'repo_card', 'repository', ?2, ?3, 'queued', ?4
             )",
            params![account_id, repo_id, REGENERATE_PRIORITY, until],
        )
        .map_err(|error| format!("理解卡重做入队失败：{error}"))?;
    }
    tx.commit()
        .map_err(|error| format!("理解卡重做失败：{error}"))?;
    Ok(())
}

/// 只入队，不写预算。`scope` 是 `missing`、`outdated` 或 `readme_changed`。补旧账的优先级是 6。
pub fn enqueue_cards_for_scope(db_path: &Path, scope: &str) -> Result<usize, String> {
    if scope == "readme_changed" {
        return Ok(crate::readme_follow::enqueue_changed(db_path)?.queued);
    }
    let scope = match scope {
        "missing" => CardEnqueueScope::Missing,
        "outdated" => CardEnqueueScope::Outdated,
        _ => return Err("排队范围只能是 missing、outdated 或 readme_changed".to_owned()),
    };
    let mut conn = open_db(db_path)?;
    enqueue_cards(&mut conn, scope, BACKFILL_PRIORITY)
}

/// 全量整理进行中的样子（给界面画进度）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FullScanView {
    pub started_at: String,
    /// 兜底截止（开始后 48 小时），到点自动结束。
    pub until: String,
    /// 开始以来已用 token（含在跑任务的预留）。
    pub tokens_used: i64,
    /// 开始时要整理的个数。
    pub total: i64,
    /// 现在还剩（排队 + 进行中）。
    pub remaining: i64,
    /// 开始以来失败了几个。
    pub failed: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationStatus {
    /// 日常额度开着、或全量整理在进行，并且没有暂停。
    pub enabled: bool,
    pub daily_budget_tokens: u64,
    pub paused: bool,
    pub pause_reason: Option<jobs::PauseReason>,
    pub tokens_24h: i64,
    /// 每日额度的已用，不含一次性全量整理时段里的用量。
    pub daily_tokens_used: i64,
    /// 日常额度还剩多少。全量整理期间它不代表队列会不会停，界面要先看 `unlimited`。
    pub budget_remaining: u64,
    pub queued: i64,
    pub running: i64,
    pub failed: i64,
    pub provider_configured: bool,
    pub model: Option<String>,
    /// 全量整理正在进行（Fox 亲手确认过；这一轮不看每天额度）。
    pub unlimited: bool,
    pub full_scan: Option<FullScanView>,
    pub last_full_scan: Option<jobs::LastFullScan>,
    /// 上次 README 更新检查。没有检查过就是空。
    pub last_readme_check: Option<jobs::LastReadmeCheck>,
    /// 现在还没整理好的有效仓库数（没有理解卡 + 说明过期、没被确认），用来估全量整理要多少。
    pub needs_organizing: i64,
    pub active_repos: i64,
    pub needs_organizing_missing: i64,
    pub needs_organizing_outdated: i64,
    /// 缺失、过期，以及已经排队或进行中的理解卡任务，按仓库去重。
    pub awaiting: i64,
}

/// 与 insert_scoped 的 Missing / Outdated 范围一致；拆分计数来自同一次查询。
fn organizing_counts(conn: &Connection) -> Result<(i64, i64, i64, i64), String> {
    let missing =
        "(repo_ai_documents.repo_id IS NULL OR IFNULL(trim(repo_ai_documents.card_json), '') = '')";
    let outdated = outdated_sql();
    let sql = format!(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN {missing} AND {NOT_CONFIRMED_SQL} THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN ({outdated}) AND {NOT_CONFIRMED_SQL} THEN 1 ELSE 0 END), 0)
         FROM repositories
         LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
         WHERE repositories.sync_status = 'active'"
    );
    let (active, missing_count, outdated_count) = conn
        .query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|error| format!("待整理数读取失败：{error}"))?;
    // 与 start_full_scan 补排 Missing / Outdated 后的 count_open_cards 集合一致；UNION 按仓库去重。
    let awaiting_sql = format!(
        "SELECT COUNT(*) FROM (
           SELECT repositories.id FROM repositories
           LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
           WHERE repositories.sync_status = 'active'
             AND ({missing} OR ({outdated})) AND {NOT_CONFIRMED_SQL}
           UNION
           SELECT target_id FROM ai_jobs
           WHERE job_type = 'repo_card' AND status IN ('queued', 'running')
         )"
    );
    let awaiting = conn
        .query_row(&awaiting_sql, [], |row| row.get(0))
        .map_err(|error| format!("待整理数读取失败：{error}"))?;
    Ok((active, missing_count, outdated_count, awaiting))
}

/// `settings` 是设置文件里的 JSON。不读钥匙串，也不看 `apiKey`。
pub fn automation_status(db_path: &Path, settings: &Value) -> Result<AutomationStatus, String> {
    let control_path = jobs::queue_control_path(db_path);
    let now = jobs::now_db_time();
    let conn = open_db(db_path)?;
    // 全量整理整理完了 / 到时间了：先收尾，界面读到的就是收尾后的样子
    jobs::settle_full_scan(&conn, &control_path, &now)?;
    let control = jobs::read_control(&control_path);
    let queue = jobs::queue_status(db_path)?;
    let (provider_configured, model) = provider_from_settings(settings);
    let unlimited = control.full_scan_active(&now);
    let full_scan = match (&control.full_scan, unlimited) {
        (Some(scan), true) => Some(FullScanView {
            started_at: scan.started_at.clone(),
            until: scan.until.clone(),
            tokens_used: jobs::tokens_used_since(&conn, &scan.started_at)?.max(0),
            total: scan.total,
            remaining: jobs::count_open_cards(&conn)?,
            failed: jobs::count_failed_cards_since(&conn, &scan.started_at)?,
        }),
        _ => None,
    };
    let (active_repos, needs_organizing_missing, needs_organizing_outdated, awaiting) =
        organizing_counts(&conn)?;
    Ok(AutomationStatus {
        enabled: (control.daily_budget_tokens > 0 || unlimited) && !control.paused,
        daily_budget_tokens: control.daily_budget_tokens,
        paused: control.paused,
        pause_reason: control.pause_reason,
        tokens_24h: queue.tokens_24h,
        daily_tokens_used: queue.daily_tokens_used,
        budget_remaining: queue.budget_remaining,
        queued: queue.queued,
        running: queue.running,
        failed: queue.failed,
        provider_configured,
        model,
        unlimited,
        full_scan,
        last_full_scan: control.last_full_scan,
        last_readme_check: control.last_readme_check,
        needs_organizing: needs_organizing_missing + needs_organizing_outdated,
        active_repos,
        needs_organizing_missing,
        needs_organizing_outdated,
        awaiting,
    })
}

/// 开始「全量整理」：这一轮不受每天额度限制，一直整理到全部完成（最长 48 小时），然后自动回到每天额度。
/// 花钱的决定只能由 Fox 本人做：界面必须先弹确认，调用时带 `confirmed = true`，没带就拒绝。
/// 开始时把没有理解卡的、说明过期的都排进队列（已经排着的不重复），并恢复暂停。已经在进行就原样返回。
pub fn start_full_scan(
    db_path: &Path,
    settings: &Value,
    confirmed: bool,
) -> Result<AutomationStatus, String> {
    if !confirmed {
        return Err("「现在整理」会花钱，需要你确认之后才能开始。".to_owned());
    }
    let (provider_configured, _) = provider_from_settings(settings);
    if !provider_configured {
        return Err(
            "还没配置 AI 服务，先到「设置 → AI 设置」里选好服务商和模型、保存密钥。".to_owned(),
        );
    }
    let control_path = jobs::queue_control_path(db_path);
    let now = jobs::now_db_time();
    let mut conn = open_db(db_path)?;
    // 上一轮如果到时间了还没收尾，先收尾（留下记录），再看现在是不是已经在进行
    jobs::settle_full_scan(&conn, &control_path, &now)?;
    if jobs::read_control(&control_path).full_scan_active(&now) {
        return automation_status(db_path, settings);
    }
    enqueue_cards(&mut conn, CardEnqueueScope::Missing, BACKFILL_PRIORITY)?;
    enqueue_cards(&mut conn, CardEnqueueScope::Outdated, BACKFILL_PRIORITY)?;
    let total = jobs::count_open_cards(&conn)?;
    if total == 0 {
        return Err("没有需要整理的仓库了。".to_owned());
    }
    jobs::begin_full_scan(&control_path, &now, total)?;
    automation_status(db_path, settings)
}

/// 停止全量整理（随时可以，不需要确认）。已经在跑的任务会做完，之后按每天额度。没有在进行就什么也不做。
pub fn stop_full_scan(db_path: &Path, settings: &Value) -> Result<AutomationStatus, String> {
    let conn = open_db(db_path)?;
    jobs::stop_full_scan(
        &conn,
        &jobs::queue_control_path(db_path),
        &jobs::now_db_time(),
    )?;
    automation_status(db_path, settings)
}

pub fn set_automation_budget(
    db_path: &Path,
    settings: &Value,
    tokens: u64,
) -> Result<AutomationStatus, String> {
    jobs::set_daily_budget(&jobs::queue_control_path(db_path), tokens)?;
    automation_status(db_path, settings)
}

fn provider_from_settings(settings: &Value) -> (bool, Option<String>) {
    let Some(ai) = settings.get("ai").and_then(Value::as_object) else {
        return (false, None);
    };
    let provider = ai
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let model = ai
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    let configured =
        !provider.is_empty() && !provider.eq_ignore_ascii_case("none") && model.is_some();
    (configured, model)
}

fn config_ready(config: &AiRequestConfig) -> bool {
    let provider = config.provider.trim();
    if provider.is_empty()
        || provider.eq_ignore_ascii_case("none")
        || config.model.trim().is_empty()
    {
        return false;
    }
    if !config.api_key.trim().is_empty() {
        return true;
    }
    // 与设置页一致：本机 OpenAI 兼容接口可以不带密钥。别的情况空密钥一律当成没配好。
    super::can_use_ai_without_api_key(config)
}

fn map_card_error(error: CardRunError) -> JobFailure {
    let message = error.message;
    match error.kind {
        CardRunErrorKind::RateLimit { retry_after_secs } => JobFailure::Retryable {
            kind: ErrorKind::RateLimit,
            message,
            retry_after_secs: error.retry_after_secs.or(retry_after_secs),
        },
        CardRunErrorKind::Timeout => JobFailure::Retryable {
            kind: ErrorKind::Timeout,
            message,
            retry_after_secs: error.retry_after_secs,
        },
        CardRunErrorKind::Network => JobFailure::Retryable {
            kind: ErrorKind::Network,
            message,
            retry_after_secs: error.retry_after_secs,
        },
        CardRunErrorKind::Auth => JobFailure::PauseQueue {
            kind: ErrorKind::Auth,
            message,
        },
        CardRunErrorKind::ContentTooLarge => JobFailure::Fatal {
            kind: ErrorKind::ContentTooLarge,
            message,
        },
        CardRunErrorKind::BadJson => JobFailure::Retryable {
            kind: ErrorKind::BadJson,
            message,
            retry_after_secs: None,
        },
        CardRunErrorKind::Schema => JobFailure::Retryable {
            kind: ErrorKind::Schema,
            message,
            retry_after_secs: None,
        },
        CardRunErrorKind::NotFound => JobFailure::Fatal {
            kind: ErrorKind::Other,
            message,
        },
        // 模型回了空内容：试跑里同一个仓库连着三次都是空的（确定性的），再试只会白花 token，第一次就算失败
        CardRunErrorKind::Other if message.contains("没有可用内容") => JobFailure::Fatal {
            kind: ErrorKind::Other,
            message,
        },
        // 请求被服务商直接拒绝（模型名 / 地址不对、参数不支持、余额不足）：没有计费，也不会因为重试变好。
        // 以前当成普通的「其他」错误：每次尝试记 4k、重试 3 次，8 个任务就把 10 万额度「用完」了，什么也没花。
        // 402（余额不足）和 404（模型或地址配错了）是整条队列都会遇到的：停下来让人去处理，别一个个失败；
        // 别的 4xx 只让这一条失败，不重试、不记用量。（401 / 403 / 408 / 429 有各自的种类，不会走到这里。）
        CardRunErrorKind::Other if matches!(http_status_in(&message), Some(402 | 404)) => {
            JobFailure::PauseQueue {
                kind: ErrorKind::Other,
                message,
            }
        }
        CardRunErrorKind::Other if matches!(http_status_in(&message), Some(400..=499)) => {
            JobFailure::Fatal {
                kind: ErrorKind::ContentTooLarge,
                message,
            }
        }
        CardRunErrorKind::Database | CardRunErrorKind::Other => JobFailure::Retryable {
            kind: ErrorKind::Other,
            message,
            retry_after_secs: error.retry_after_secs,
        },
    }
}

/// 从「AI 接口请求失败（HTTP 404）：…」这种错误信息里取出状态码。
fn http_status_in(message: &str) -> Option<u16> {
    let start = message.find("HTTP ")? + "HTTP ".len();
    let digits: String = message[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.len() == 3 {
        digits.parse().ok()
    } else {
        None
    }
}

fn apply_scenario_tags(
    conn: &mut Connection,
    account_id: &str,
    repo_id: &str,
) -> Result<(), String> {
    let card_json = read_card_json(conn, repo_id)?;
    let Some(raw) = card_json.filter(|text| !text.trim().is_empty()) else {
        return Ok(());
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Ok(());
    };
    let confidence = value
        .get("confidence")
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let slugs = value
        .get("scenario_tags")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    scenarios::apply_ai_scenarios(conn, account_id, repo_id, &slugs, confidence)?;
    Ok(())
}

fn insert_scoped(
    tx: &Transaction<'_>,
    scope: CardEnqueueScope,
    priority: i64,
) -> Result<usize, String> {
    let extra = match scope {
        CardEnqueueScope::Missing => {
            "(repo_ai_documents.repo_id IS NULL OR IFNULL(trim(repo_ai_documents.card_json), '') = '')"
                .to_owned()
        }
        CardEnqueueScope::Outdated => outdated_sql(),
    };
    let sql = format!(
        "INSERT OR IGNORE INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
         SELECT lower(hex(randomblob(8))), repositories.account_id, 'repo_card', 'repository', repositories.id, ?1, 'queued'
         FROM repositories
         LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
         WHERE repositories.sync_status = 'active'
           AND {extra}
           AND {NOT_CONFIRMED_SQL}"
    );
    tx.execute(&sql, params![priority])
        .map_err(|error| format!("理解卡入队失败：{error}"))
}

fn read_card_json(conn: &Connection, repo_id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT card_json FROM repo_ai_documents WHERE repo_id = ?1",
        params![repo_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("理解卡读取失败：{error}"))
}

fn rewrite_review_status(card_json: Option<&str>, status: &str) -> Result<String, String> {
    let raw = card_json.unwrap_or("").trim();
    if raw.is_empty() {
        return Err("这个仓库还没有理解卡".to_owned());
    }
    let mut value: Value =
        serde_json::from_str(raw).map_err(|_| "这个仓库还没有理解卡".to_owned())?;
    let Some(object) = value.as_object_mut() else {
        return Err("这个仓库还没有理解卡".to_owned());
    };
    object.insert("review_status".to_owned(), Value::String(status.to_owned()));
    serde_json::to_string(&value).map_err(|_| "理解卡无法保存".to_owned())
}

fn open_db(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .busy_timeout(Duration::from_millis(5000))
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    Ok(connection)
}

fn now_stamp() -> String {
    let value = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        value.year(),
        u8::from(value.month()),
        value.day(),
        value.hour(),
        value.minute(),
        value.second(),
        value.millisecond()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_model_reply_fails_at_once_instead_of_retrying() {
        let empty = map_card_error(CardRunError {
            kind: CardRunErrorKind::Other,
            message: "OpenAI 响应中没有可用内容（结束原因 stop）".to_owned(),
            retry_after_secs: None,
            spent_tokens: 0,
        });
        assert!(matches!(
            empty,
            JobFailure::Fatal {
                kind: ErrorKind::Other,
                ..
            }
        ));
        // 别的「其他」错误照旧可以重试
        let other = map_card_error(CardRunError {
            kind: CardRunErrorKind::Other,
            message: "别的错误".to_owned(),
            retry_after_secs: None,
            spent_tokens: 0,
        });
        assert!(matches!(
            other,
            JobFailure::Retryable {
                kind: ErrorKind::Other,
                ..
            }
        ));
    }
    use crate::card_pipeline::{LlmError, LlmErrorKind, LlmReply, PROMPT_VERSION};
    use crate::db::migrations::migrate_database;
    use crate::jobs::{QueueControl, Worker, WorkerConfig};
    use rusqlite::Connection;
    use serde_json::{json, Value};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    struct TempDb {
        path: PathBuf,
    }

    impl TempDb {
        fn new(label: &str) -> Self {
            static SEQ: AtomicUsize = AtomicUsize::new(0);
            let n = SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应晚于纪元")
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("fsl-cardh-{label}-{nanos}-{n}"));
            fs::create_dir_all(&dir).expect("临时目录应能创建");
            let path = dir.join("fox-stars-lab.sqlite3");
            migrate_database(&path).expect("测试库应能完成迁移");
            let conn = open_db(&path).expect("测试库应能打开");
            conn.execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .expect("测试账号应能写入");
            drop(conn);
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn open(&self) -> Connection {
            open_db(&self.path).expect("测试库应能打开")
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let Some(parent) = self.path.parent() else {
                return;
            };
            let Some(name) = parent.file_name().and_then(|name| name.to_str()) else {
                return;
            };
            if name.starts_with("fsl-cardh-") {
                let _ = fs::remove_dir_all(parent);
            }
        }
    }

    struct Scripted {
        replies: Arc<Mutex<Vec<Result<LlmReply, LlmError>>>>,
        calls: Arc<AtomicUsize>,
    }

    impl LlmClient for Scripted {
        fn complete_json(
            &self,
            _request: &card_pipeline::LlmRequest,
        ) -> Result<LlmReply, LlmError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut replies = self.replies.lock().expect("剧本锁应可用");
            if replies.is_empty() {
                return Err(LlmError {
                    kind: LlmErrorKind::Other,
                    message: "剧本已用完".to_owned(),
                });
            }
            replies.remove(0)
        }
    }

    fn ok_config() -> AiRequestConfig {
        AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: "test-key".to_owned(),
            base_url: None,
            model: "config-model".to_owned(),
        }
    }

    fn reply(content: &str, model: &str, input: u64, output: u64) -> Result<LlmReply, LlmError> {
        Ok(LlmReply {
            content: content.to_owned(),
            input_tokens: Some(input),
            output_tokens: Some(output),
            model: Some(model.to_owned()),
        })
    }

    fn llm_err(kind: LlmErrorKind, message: &str) -> Result<LlmReply, LlmError> {
        Err(LlmError {
            kind,
            message: message.to_owned(),
        })
    }

    fn scripted(
        config: Result<AiRequestConfig, String>,
        replies: Vec<Result<LlmReply, LlmError>>,
    ) -> (RepoCardHandler, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_factory = Arc::clone(&calls);
        let stored = Arc::new(Mutex::new(replies));
        let stored_for_factory = Arc::clone(&stored);
        let handler = RepoCardHandler::with_client_factory(
            Arc::new(move || config.clone()),
            Arc::new(move || {
                Box::new(Scripted {
                    replies: Arc::clone(&stored_for_factory),
                    calls: Arc::clone(&calls_for_factory),
                })
            }),
        );
        (handler, calls)
    }

    fn job(id: &str, target_type: &str, target_id: &str) -> JobRecord {
        JobRecord {
            id: id.to_owned(),
            account_id: "acct".to_owned(),
            job_type: "repo_card".to_owned(),
            target_type: target_type.to_owned(),
            target_id: target_id.to_owned(),
            attempts: 1,
            max_attempts: 3,
            input_hash: None,
            reserved: 0,
        }
    }

    fn usable_readme() -> String {
        let mut text = String::from(
            "# fox/demo\n\n做小红书封面的命令行工具。\n\n## 安装\n\nnpm install xhs-cover\n\n## 使用\n\n运行后即可生成封面。\n",
        );
        while text.chars().count() < 80 {
            text.push('。');
        }
        text
    }

    fn valid_json() -> String {
        serde_json::to_string(&json!({
            "one_liner_zh": "给小红书做封面的命令行",
            "what_it_is": "根据提示生成小红书封面图。",
            "form": "cli",
            "primary_category": "content-creation",
            "secondary_categories": ["media-gen"],
            "capabilities": [{"name_zh": "生成封面图"}],
            "problems_solved": ["想批量做小红书封面"],
            "how_to_try": "npm install xhs-cover",
            "maturity": "usable",
            "zh_friendly": true,
            "confidence": 0.82,
            "keywords": ["小红书", "封面", "xiaohongshu", "cover", "配图", "出图"],
            "scenario_tags": ["xhs"],
            "scenario_evidence": {"xhs": "做小红书封面的命令行工具"}
        }))
        .expect("样例卡片应能序列化")
    }

    fn insert_repo(conn: &Connection, id: &str, status: &str) {
        conn.execute(
            "INSERT INTO repositories(
                id, account_id, owner, name, full_name, html_url, starred_at, sync_status,
                description, language, topics_json, stars_count, pushed_at
             ) VALUES (
                ?1, 'acct', 'fox', ?1, 'fox/' || ?1, 'https://github.com/fox/' || ?1,
                '2026-09-01T00:00:00Z', ?2, '做小红书封面', 'Rust', '[\"xiaohongshu\",\"cover\"]',
                12, '2026-09-30T08:00:00Z'
             )",
            params![id, status],
        )
        .expect("仓库应能写入");
    }

    fn seed_catalog(conn: &Connection) {
        conn.execute_batch(
            "
            INSERT INTO categories(id, account_id, slug, name_zh, sort_order) VALUES
              ('c1', 'acct', 'content-creation', '内容创作', 1),
              ('c2', 'acct', 'media-gen', '图像生成', 2);
            INSERT INTO scenarios(id, account_id, slug, name_zh) VALUES
              ('s1', 'acct', 'xhs', '小红书');
            ",
        )
        .expect("分类和场景应能写入");
    }

    fn insert_readme(conn: &Connection, repo_id: &str) {
        let text = usable_readme();
        conn.execute(
            "INSERT INTO repo_readmes(repo_id, raw_markdown, content_hash, source_path, fetched_at)
             VALUES (?1, ?2, 'hash', 'README.md', '2026-09-30T00:00:00Z')",
            params![repo_id, text],
        )
        .expect("README 应能写入");
    }

    fn prepare_runnable(db: &TempDb) {
        let conn = db.open();
        insert_repo(&conn, "r1", "active");
        seed_catalog(&conn);
        insert_readme(&conn, "r1");
    }

    fn insert_card(conn: &Connection, repo_id: &str, card_json: &str, schema: i64, hash: &str) {
        conn.execute(
            "INSERT INTO repo_ai_documents(
                repo_id, summary_zh, model, prompt_version, source_hash, generated_at, card_json, card_schema_version
             ) VALUES (?1, '旧摘要', 'kept-model', ?5, ?2, '2026-01-01T00:00:00Z', ?3, ?4)",
            // 第 2 版结构的卡当作「现行」：提示词版本跟当前一致；更老的结构才是 card-v1
            params![
                repo_id,
                hash,
                card_json,
                schema,
                if schema >= 2 { PROMPT_VERSION } else { "card-v1" }
            ],
        )
        .expect("理解卡应能写入");
    }

    fn cell_string(conn: &Connection, sql: &str) -> String {
        conn.query_row(sql, [], |row| row.get(0))
            .expect("查询应返回文字")
    }

    fn cell_i64(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |row| row.get(0))
            .expect("查询应返回整数")
    }

    fn assert_retryable(
        result: Result<JobSuccess, JobFailure>,
        kind: ErrorKind,
        retry_after_secs: Option<u64>,
        message_part: &str,
    ) {
        match result {
            Err(JobFailure::Retryable {
                kind: got,
                message,
                retry_after_secs: got_retry,
            }) => {
                assert_eq!(got, kind);
                assert_eq!(got_retry, retry_after_secs);
                assert!(message.contains(message_part), "{message}");
            }
            other => panic!("应为可重试失败，实际 {other:?}"),
        }
    }

    #[test]
    fn handler_success_writes_card_tokens_scenarios_and_job_row() {
        let db = TempDb::new("success");
        prepare_runnable(&db);
        let conn = db.open();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts)
             VALUES ('job-r1', 'acct', 'repo_card', 'repository', 'r1', 'running', 1)",
            [],
        )
        .expect("运行中的任务应能写入");
        drop(conn);
        let (handler, calls) = scripted(
            Ok(ok_config()),
            vec![reply(&valid_json(), "fake-model", 111, 22)],
        );
        let success = handler
            .run(&job("job-r1", "repository", "r1"), db.path())
            .expect("合法卡应成功");
        assert!(
            handler.is_proven(&config_fingerprint(&ok_config())),
            "真的调过模型并成功，这套配置就算证明过"
        );
        let mut other_model = ok_config();
        other_model.model = "another-model".to_owned();
        assert!(
            !handler.is_proven(&config_fingerprint(&other_model)),
            "换了模型，之前的成功不再证明什么"
        );
        let mut other_address = ok_config();
        other_address.base_url = Some("https://gateway.example.com/v1".to_owned());
        assert!(
            !handler.is_proven(&config_fingerprint(&other_address)),
            "换了地址，之前的成功不再证明什么"
        );
        let mut other_provider = ok_config();
        other_provider.provider = "anthropic".to_owned();
        assert!(
            !handler.is_proven(&config_fingerprint(&other_provider)),
            "换了服务商，之前的成功不再证明什么"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(success.input_tokens, 111);
        assert_eq!(success.output_tokens, 22);
        assert_eq!(success.provider, "openai-compatible");
        assert_eq!(success.model, "fake-model");
        assert_eq!(success.prompt_version, PROMPT_VERSION);
        assert!(!success.skipped);
        assert_eq!(success.input_hash.len(), 64);

        let conn = db.open();
        let card_json = cell_string(
            &conn,
            "SELECT card_json FROM repo_ai_documents WHERE repo_id = 'r1'",
        );
        assert!(!card_json.contains('\n'), "{card_json}");
        assert!(
            card_json.contains("\"review_status\":\"suggested\""),
            "{card_json}"
        );
        assert!(
            card_json.contains("\"scenario_tags\":[\"xhs\"]"),
            "{card_json}"
        );
        assert_eq!(
            cell_i64(
                &conn,
                "SELECT input_tokens FROM repo_ai_documents WHERE repo_id = 'r1'"
            ),
            111
        );
        assert_eq!(
            cell_i64(
                &conn,
                "SELECT output_tokens FROM repo_ai_documents WHERE repo_id = 'r1'"
            ),
            22
        );
        assert_eq!(
            cell_string(
                &conn,
                "SELECT source_hash FROM repo_ai_documents WHERE repo_id = 'r1'"
            ),
            success.input_hash
        );
        let (slug, origin, status, confidence): (String, String, String, f64) = conn
            .query_row(
                "SELECT s.slug, es.origin, es.status, es.confidence
                 FROM entity_scenarios es
                 JOIN scenarios s ON s.id = es.scenario_id
                 WHERE es.entity_type = 'repository' AND es.entity_id = 'r1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("场景归类应已写入");
        assert_eq!(slug, "xhs");
        assert_eq!(origin, "ai");
        assert_eq!(status, "suggested");
        assert!((confidence - 0.82).abs() < 1e-9, "{confidence}");
        drop(conn);

        assert!(jobs::finish_job(
            db.path(),
            &job("job-r1", "repository", "r1"),
            &now_stamp(),
            Ok(success.clone()),
            0.0
        )
        .expect("收尾应成功"));
        let conn = db.open();
        assert_eq!(
            cell_string(&conn, "SELECT status FROM ai_jobs WHERE id = 'job-r1'"),
            "succeeded"
        );
        assert_eq!(
            cell_i64(
                &conn,
                "SELECT input_tokens FROM ai_jobs WHERE id = 'job-r1'"
            ),
            111
        );
        assert_eq!(
            cell_i64(
                &conn,
                "SELECT output_tokens FROM ai_jobs WHERE id = 'job-r1'"
            ),
            22
        );
        assert_eq!(
            cell_string(&conn, "SELECT provider FROM ai_jobs WHERE id = 'job-r1'"),
            "openai-compatible"
        );
        assert_eq!(
            cell_string(&conn, "SELECT model FROM ai_jobs WHERE id = 'job-r1'"),
            "fake-model"
        );
        assert_eq!(
            cell_string(
                &conn,
                "SELECT prompt_version FROM ai_jobs WHERE id = 'job-r1'"
            ),
            PROMPT_VERSION
        );
        assert_eq!(
            cell_string(&conn, "SELECT input_hash FROM ai_jobs WHERE id = 'job-r1'"),
            success.input_hash
        );
    }

    #[test]
    fn handler_skips_when_the_same_content_is_already_current() {
        let db = TempDb::new("skip-same");
        prepare_runnable(&db);
        let (handler, calls) = scripted(
            Ok(ok_config()),
            vec![reply(&valid_json(), "fake-model", 11, 5)],
        );
        let first = handler
            .run(&job("job-1", "repository", "r1"), db.path())
            .expect("第一次应生成");
        assert!(!first.skipped);
        let second = handler
            .run(&job("job-2", "repository", "r1"), db.path())
            .expect("内容没变应跳过");
        assert!(second.skipped);
        assert_eq!(second.input_tokens, 0);
        assert_eq!(second.output_tokens, 0);
        assert_eq!(second.input_hash, first.input_hash);
        assert_eq!(second.model, "fake-model");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "跳过时不应再调用模型");

        let conn = db.open();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts)
             VALUES ('job-2', 'acct', 'repo_card', 'repository', 'r1', 'running', 1)",
            [],
        )
        .expect("运行中的任务应能写入");
        drop(conn);
        assert!(jobs::finish_job(
            db.path(),
            &job("job-2", "repository", "r1"),
            &now_stamp(),
            Ok(second),
            0.0
        )
        .expect("收尾应成功"));
        assert_eq!(
            cell_string(&db.open(), "SELECT status FROM ai_jobs WHERE id = 'job-2'"),
            "skipped"
        );
        assert_eq!(
            cell_i64(
                &db.open(),
                "SELECT input_tokens + output_tokens FROM ai_jobs WHERE id = 'job-2'"
            ),
            0
        );
    }

    #[test]
    fn handler_skips_a_confirmed_card_without_calling_the_model() {
        let db = TempDb::new("skip-confirmed");
        prepare_runnable(&db);
        insert_card(
            &db.open(),
            "r1",
            r#"{"review_status":"confirmed","one_liner_zh":"已确认","confidence":0.2}"#,
            1,
            "other-hash",
        );
        let (handler, calls) = scripted(
            Ok(ok_config()),
            vec![reply(&valid_json(), "fake-model", 11, 5)],
        );
        let success = handler
            .run(&job("job-1", "repository", "r1"), db.path())
            .expect("已确认的卡应跳过");
        assert!(success.skipped);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(success.input_tokens, 0);
        assert_eq!(success.output_tokens, 0);
        assert!(
            !handler.is_proven(&config_fingerprint(&ok_config())),
            "没有调过模型，不能算证明过这套配置"
        );
    }

    #[test]
    fn unusable_config_pauses_without_calling_the_model() {
        let db = TempDb::new("config-bad");
        prepare_runnable(&db);
        let cases = [
            Err("没有密钥".to_owned()),
            Ok(AiRequestConfig {
                provider: "none".to_owned(),
                api_key: String::new(),
                base_url: None,
                model: "m".to_owned(),
            }),
            Ok(AiRequestConfig {
                provider: "openai".to_owned(),
                api_key: "test-key".to_owned(),
                base_url: None,
                model: "  ".to_owned(),
            }),
            Ok(AiRequestConfig {
                provider: "openai".to_owned(),
                api_key: String::new(),
                base_url: Some("https://api.openai.com/v1".to_owned()),
                model: "gpt".to_owned(),
            }),
        ];
        for config in cases {
            let (handler, calls) = scripted(config, vec![reply(&valid_json(), "fake-model", 1, 1)]);
            match handler.run(&job("job-1", "repository", "r1"), db.path()) {
                Err(JobFailure::PauseQueue { kind, message }) => {
                    assert_eq!(kind, ErrorKind::Auth);
                    assert_eq!(message, NOT_CONFIGURED_MESSAGE);
                }
                other => panic!("不可用的配置应暂停队列，实际 {other:?}"),
            }
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn loopback_without_a_key_still_reaches_the_model() {
        let db = TempDb::new("loopback");
        prepare_runnable(&db);
        let config = Ok(AiRequestConfig {
            provider: "openai-compatible".to_owned(),
            api_key: String::new(),
            base_url: Some("http://127.0.0.1:11434/v1".to_owned()),
            model: "local-model".to_owned(),
        });
        let (handler, calls) = scripted(config, vec![llm_err(LlmErrorKind::Timeout, "本地超时")]);
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), db.path()),
            ErrorKind::Timeout,
            None,
            "本地超时",
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn config_failure_pauses_the_queue_and_requeues_the_job() {
        let db = TempDb::new("pause-queue");
        let conn = db.open();
        insert_repo(&conn, "r1", "active");
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts)
             VALUES ('job-r1', 'acct', 'repo_card', 'repository', 'r1', 'queued', 0)",
            [],
        )
        .expect("排队任务应能写入");
        drop(conn);
        // 预算要放得下一条任务的预留（jobs::CLAIM_RESERVE_TOKENS），否则一条也领不到
        jobs::set_daily_budget(&jobs::queue_control_path(db.path()), 50_000)
            .expect("测试预算应能写入");
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_handler = Arc::clone(&calls);
        let handler = RepoCardHandler::new(Arc::new(move || {
            calls_for_handler.fetch_add(1, Ordering::SeqCst);
            Err("钥匙串里没有密钥".to_owned())
        }));
        let worker = Worker::start(
            db.path().to_path_buf(),
            vec![Arc::new(handler)],
            WorkerConfig {
                concurrency: 1,
                idle_poll: Duration::from_millis(20),
            },
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut control = QueueControl::default();
        let mut status = String::new();
        while Instant::now() < deadline {
            control = jobs::read_control(&jobs::queue_control_path(db.path()));
            status = cell_string(&db.open(), "SELECT status FROM ai_jobs WHERE id = 'job-r1'");
            if control.paused && status == "queued" {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        worker.stop();
        assert!(control.paused, "队列应被暂停");
        let reason = control.pause_reason.expect("应写下暂停原因");
        assert_eq!(reason.kind, "auth");
        assert_eq!(reason.message, NOT_CONFIGURED_MESSAGE);
        assert!(!reason.at.is_empty());
        assert_eq!(status, "queued");
        assert_eq!(
            cell_i64(
                &db.open(),
                "SELECT attempts FROM ai_jobs WHERE id = 'job-r1'"
            ),
            0
        );
        assert!(calls.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn rate_limit_maps_to_retryable() {
        let db = TempDb::new("map-rate");
        prepare_runnable(&db);
        let (handler, _) = scripted(
            Ok(ok_config()),
            vec![llm_err(
                LlmErrorKind::RateLimit {
                    retry_after_secs: Some(42),
                },
                "太频繁",
            )],
        );
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), db.path()),
            ErrorKind::RateLimit,
            Some(42),
            "太频繁",
        );
    }

    #[test]
    fn timeout_maps_to_retryable() {
        let db = TempDb::new("map-timeout");
        prepare_runnable(&db);
        let (handler, _) = scripted(
            Ok(ok_config()),
            vec![llm_err(LlmErrorKind::Timeout, "超时了")],
        );
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), db.path()),
            ErrorKind::Timeout,
            None,
            "超时了",
        );
    }

    #[test]
    fn network_maps_to_retryable() {
        let db = TempDb::new("map-network");
        prepare_runnable(&db);
        let (handler, _) = scripted(
            Ok(ok_config()),
            vec![llm_err(LlmErrorKind::Network, "网络断了")],
        );
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), db.path()),
            ErrorKind::Network,
            None,
            "网络断了",
        );
    }

    #[test]
    fn pipeline_auth_maps_to_pause_queue() {
        let db = TempDb::new("map-auth");
        prepare_runnable(&db);
        let (handler, _) = scripted(
            Ok(ok_config()),
            vec![llm_err(LlmErrorKind::Auth, "密钥被拒绝")],
        );
        match handler.run(&job("job-1", "repository", "r1"), db.path()) {
            Err(JobFailure::PauseQueue { kind, message }) => {
                assert_eq!(kind, ErrorKind::Auth);
                assert_eq!(message, "密钥被拒绝");
            }
            other => panic!("认证失败应暂停队列，实际 {other:?}"),
        }
    }

    #[test]
    fn content_too_large_maps_to_fatal() {
        let db = TempDb::new("map-large");
        prepare_runnable(&db);
        let (handler, _) = scripted(
            Ok(ok_config()),
            vec![llm_err(LlmErrorKind::ContentTooLarge, "材料太长")],
        );
        match handler.run(&job("job-1", "repository", "r1"), db.path()) {
            Err(JobFailure::Fatal { kind, message }) => {
                assert_eq!(kind, ErrorKind::ContentTooLarge);
                assert_eq!(message, "材料太长");
            }
            other => panic!("内容过大应直接失败，实际 {other:?}"),
        }
    }

    #[test]
    fn bad_json_maps_to_retryable() {
        let db = TempDb::new("map-json");
        prepare_runnable(&db);
        let (handler, calls) = scripted(
            Ok(ok_config()),
            vec![reply("你好", "m", 1, 1), reply("还是不好", "m", 1, 1)],
        );
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), db.path()),
            ErrorKind::BadJson,
            None,
            "JSON",
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn schema_maps_to_retryable() {
        let db = TempDb::new("map-schema");
        prepare_runnable(&db);
        let (handler, _) = scripted(
            Ok(ok_config()),
            vec![reply("{}", "m", 1, 1), reply("{}", "m", 1, 1)],
        );
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), db.path()),
            ErrorKind::Schema,
            None,
            "校验",
        );
    }

    #[test]
    fn not_found_maps_to_fatal() {
        let db = TempDb::new("map-missing");
        let (handler, calls) = scripted(Ok(ok_config()), vec![reply(&valid_json(), "m", 1, 1)]);
        match handler.run(&job("job-1", "repository", "missing"), db.path()) {
            Err(JobFailure::Fatal { kind, message }) => {
                assert_eq!(kind, ErrorKind::Other);
                assert_eq!(message, "找不到这个仓库");
            }
            other => panic!("找不到仓库应直接失败，实际 {other:?}"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn database_maps_to_retryable_other() {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("fsl-cardh-empty-{n}.sqlite3"));
        let _ = fs::remove_file(&path);
        Connection::open(&path).expect("空库应能创建");
        let (handler, _) = scripted(Ok(ok_config()), vec![reply(&valid_json(), "m", 1, 1)]);
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), &path),
            ErrorKind::Other,
            None,
            "数据库",
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn other_maps_to_retryable_other() {
        let db = TempDb::new("map-other");
        prepare_runnable(&db);
        let (handler, _) = scripted(
            Ok(ok_config()),
            vec![llm_err(LlmErrorKind::Other, "服务暂时不可用")],
        );
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), db.path()),
            ErrorKind::Other,
            None,
            "服务暂时不可用",
        );
    }

    #[test]
    fn non_repository_target_is_fatal() {
        let db = TempDb::new("target");
        let (handler, calls) = scripted(Ok(ok_config()), vec![reply(&valid_json(), "m", 1, 1)]);
        for (target_type, target_id) in [("inbox_item", "x"), ("repository", "  ")] {
            match handler.run(&job("job-1", target_type, target_id), db.path()) {
                Err(JobFailure::Fatal { kind, message }) => {
                    assert_eq!(kind, ErrorKind::Other);
                    assert_eq!(message, "任务目标不是仓库");
                }
                other => panic!("错误目标应直接失败，实际 {other:?}"),
            }
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn worker_leaves_spending_jobs_queued_until_budget_is_set() {
        let db = TempDb::new("worker-budget");
        let conn = db.open();
        insert_repo(&conn, "fresh", "active");
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             VALUES ('canary', 'acct', 'categorize', 'repository', 'canary-repo', 1, 'queued')",
            [],
        )
        .expect("探针任务应能写入");
        drop(conn);
        let mut conn = db.open();
        assert_eq!(
            enqueue_cards(&mut conn, CardEnqueueScope::Missing, 6).expect("应能入队"),
            1
        );
        drop(conn);

        let canary_hits = Arc::new(AtomicUsize::new(0));
        let spend_hits = Arc::new(AtomicUsize::new(0));
        struct Canary(Arc<AtomicUsize>);
        struct Spender(Arc<AtomicUsize>);
        impl JobHandler for Canary {
            fn job_type(&self) -> &'static str {
                "categorize"
            }
            fn spends_tokens(&self) -> bool {
                false
            }
            fn run(&self, _job: &JobRecord, _db_path: &Path) -> Result<JobSuccess, JobFailure> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(JobSuccess {
                    input_tokens: 0,
                    output_tokens: 0,
                    provider: "fake".to_owned(),
                    model: "fake".to_owned(),
                    prompt_version: "none".to_owned(),
                    input_hash: String::new(),
                    skipped: false,
                })
            }
        }
        impl JobHandler for Spender {
            fn job_type(&self) -> &'static str {
                "repo_card"
            }
            fn spends_tokens(&self) -> bool {
                true
            }
            fn run(&self, _job: &JobRecord, _db_path: &Path) -> Result<JobSuccess, JobFailure> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(JobSuccess {
                    input_tokens: 3,
                    output_tokens: 4,
                    provider: "fake".to_owned(),
                    model: "fake".to_owned(),
                    prompt_version: "card-v2".to_owned(),
                    input_hash: "hash".to_owned(),
                    skipped: false,
                })
            }
        }
        let worker = Worker::start(
            db.path().to_path_buf(),
            vec![
                Arc::new(Canary(Arc::clone(&canary_hits))),
                Arc::new(Spender(Arc::clone(&spend_hits))),
            ],
            WorkerConfig {
                concurrency: 1,
                idle_poll: Duration::from_millis(20),
            },
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while canary_hits.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            canary_hits.load(Ordering::SeqCst),
            1,
            "不花钱的探针应在预算为 0 时被领走"
        );
        thread::sleep(Duration::from_millis(150));
        assert_eq!(spend_hits.load(Ordering::SeqCst), 0);
        assert_eq!(
            cell_string(
                &db.open(),
                "SELECT status FROM ai_jobs WHERE target_id = 'fresh'"
            ),
            "queued"
        );
        assert_eq!(
            jobs::read_control(&jobs::queue_control_path(db.path())).daily_budget_tokens,
            0
        );

        jobs::set_daily_budget(&jobs::queue_control_path(db.path()), 50_000)
            .expect("测试预算应能写入");
        let deadline = Instant::now() + Duration::from_secs(3);
        while spend_hits.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        worker.stop();
        assert_eq!(spend_hits.load(Ordering::SeqCst), 1);
        let conn = db.open();
        assert_eq!(
            cell_string(
                &conn,
                "SELECT status FROM ai_jobs WHERE target_id = 'fresh'"
            ),
            "succeeded"
        );
        assert_eq!(
            cell_i64(
                &conn,
                "SELECT input_tokens FROM ai_jobs WHERE target_id = 'fresh'"
            ),
            3
        );
        assert_eq!(
            cell_i64(
                &conn,
                "SELECT output_tokens FROM ai_jobs WHERE target_id = 'fresh'"
            ),
            4
        );
        assert_eq!(
            cell_i64(
                &conn,
                "SELECT priority FROM ai_jobs WHERE target_id = 'fresh'"
            ),
            6
        );
    }

    #[test]
    fn enqueue_missing_skips_confirmed_current_and_duplicates() {
        let db = TempDb::new("missing");
        let conn = db.open();
        insert_repo(&conn, "bare", "active");
        insert_repo(&conn, "empty", "active");
        insert_card(&conn, "empty", "", 0, "");
        insert_repo(&conn, "confirmed", "active");
        insert_card(
            &conn,
            "confirmed",
            r#"{"review_status":"confirmed"}"#,
            2,
            "h",
        );
        insert_repo(&conn, "current", "active");
        insert_card(&conn, "current", r#"{"review_status":"suggested"}"#, 2, "h");
        insert_repo(&conn, "gone", "removed");
        insert_repo(&conn, "queued", "active");
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             VALUES ('keep', 'acct', 'repo_card', 'repository', 'queued', 1, 'queued')",
            [],
        )
        .expect("已有任务应能写入");
        drop(conn);

        let mut conn = db.open();
        assert_eq!(
            enqueue_cards(&mut conn, CardEnqueueScope::Missing, 6).expect("应能入队"),
            2
        );
        assert_eq!(
            enqueue_cards(&mut conn, CardEnqueueScope::Missing, 6).expect("重复入队应为 0"),
            0
        );
        drop(conn);

        let conn = db.open();
        let rows = job_rows(&conn);
        assert_eq!(priority_of(&rows, "bare"), Some((6, "queued")));
        assert_eq!(priority_of(&rows, "empty"), Some((6, "queued")));
        assert_eq!(priority_of(&rows, "queued"), Some((1, "queued")));
        assert!(priority_of(&rows, "confirmed").is_none());
        assert!(priority_of(&rows, "current").is_none());
        assert!(priority_of(&rows, "gone").is_none());
        assert_eq!(rows.iter().filter(|row| row.0 == "queued").count(), 1);
    }

    #[test]
    fn enqueue_outdated_only_old_unconfirmed_cards() {
        let db = TempDb::new("outdated");
        let conn = db.open();
        insert_repo(&conn, "old", "active");
        insert_card(&conn, "old", r#"{"review_status":"suggested"}"#, 1, "h");
        insert_repo(&conn, "pretty", "active");
        insert_card(
            &conn,
            "pretty",
            "{\n  \"review_status\": \"confirmed\",\n  \"one_liner_zh\": \"已确认\"\n}",
            1,
            "h",
        );
        insert_repo(&conn, "current", "active");
        insert_card(&conn, "current", r#"{"review_status":"suggested"}"#, 2, "h");
        insert_repo(&conn, "blank", "active");
        insert_card(&conn, "blank", "", 0, "");
        insert_repo(&conn, "gone", "removed");
        insert_card(&conn, "gone", r#"{"review_status":"suggested"}"#, 1, "h");
        insert_repo(&conn, "bare", "active");
        drop(conn);

        let mut conn = db.open();
        assert_eq!(
            enqueue_cards(&mut conn, CardEnqueueScope::Outdated, 6).expect("应能入队"),
            1
        );
        drop(conn);
        let rows = job_rows(&db.open());
        assert_eq!(rows.len(), 1);
        assert_eq!(priority_of(&rows, "old"), Some((6, "queued")));
    }

    #[test]
    fn enqueue_new_stars_only_listed_repos_at_priority_three() {
        let db = TempDb::new("new-stars");
        let conn = db.open();
        insert_repo(&conn, "fresh", "active");
        insert_repo(&conn, "has-card", "active");
        insert_card(
            &conn,
            "has-card",
            r#"{"review_status":"suggested"}"#,
            2,
            "h",
        );
        insert_repo(&conn, "other", "active");
        insert_repo(&conn, "gone", "removed");
        drop(conn);

        assert_eq!(enqueue_new_stars(db.path(), &[]).expect("空列表应为 0"), 0);
        let ids = vec![
            "fresh".to_owned(),
            "has-card".to_owned(),
            "gone".to_owned(),
            "missing".to_owned(),
        ];
        assert_eq!(enqueue_new_stars(db.path(), &ids).expect("应能入队"), 1);
        assert_eq!(
            enqueue_new_stars(db.path(), &ids).expect("重复入队应为 0"),
            0
        );
        let rows = job_rows(&db.open());
        assert_eq!(rows.len(), 1);
        assert_eq!(
            priority_of(&rows, "fresh"),
            Some((NEW_STAR_PRIORITY, "queued"))
        );
        assert!(priority_of(&rows, "other").is_none());
        assert!(priority_of(&rows, "has-card").is_none());
        assert!(priority_of(&rows, "gone").is_none());
    }

    fn job_rows(conn: &Connection) -> Vec<(String, i64, String)> {
        let mut statement = conn
            .prepare("SELECT target_id, priority, status FROM ai_jobs")
            .expect("任务查询应能准备");
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("任务查询应能执行");
        rows.collect::<Result<Vec<_>, _>>().expect("任务行应能读出")
    }

    fn priority_of<'a>(
        rows: &'a [(String, i64, String)],
        target_id: &str,
    ) -> Option<(i64, &'a str)> {
        rows.iter()
            .find(|row| row.0 == target_id)
            .map(|row| (row.1, row.2.as_str()))
    }

    #[test]
    fn confirm_card_confirms_review_and_primary_category() {
        let db = TempDb::new("confirm");
        let conn = db.open();
        insert_repo(&conn, "r1", "active");
        seed_catalog(&conn);
        conn.execute(
            "INSERT INTO categories(id, account_id, slug, name_zh) VALUES ('c3', 'acct', 'writing', '写作')",
            [],
        )
        .expect("分类应能写入");
        let pretty = serde_json::to_string_pretty(&json!({
            "one_liner_zh": "一行说明",
            "review_status": "suggested",
            "confidence": 0.5
        }))
        .expect("样例应能序列化");
        assert!(pretty.contains('\n'));
        insert_card(&conn, "r1", &pretty, 2, "hash");
        conn.execute(
            "INSERT INTO entity_categories(account_id, entity_type, entity_id, category_id, is_primary, confidence, origin, status)
             VALUES ('acct', 'repository', 'r1', 'c1', 1, 0.8, 'ai', 'suggested'),
                    ('acct', 'repository', 'r1', 'c2', 0, 0.4, 'ai', 'suggested'),
                    ('acct', 'repository', 'r1', 'c3', 1, 0.2, 'ai', 'rejected')",
            [],
        )
        .expect("归类应能写入");
        drop(conn);

        confirm_card(db.path(), "r1").expect("应能确认");
        let conn = db.open();
        let stored = cell_string(
            &conn,
            "SELECT card_json FROM repo_ai_documents WHERE repo_id = 'r1'",
        );
        assert!(!stored.contains('\n'), "{stored}");
        assert!(
            stored.contains("\"review_status\":\"confirmed\""),
            "{stored}"
        );
        let parsed: Value = serde_json::from_str(&stored).expect("确认后仍应是 JSON");
        assert_eq!(parsed["one_liner_zh"], "一行说明");
        assert_eq!(parsed["confidence"], 0.5);
        assert_eq!(
            stored,
            serde_json::to_string(&parsed).expect("紧凑 JSON 应能序列化")
        );
        let (status, origin): (String, String) = conn
            .query_row(
                "SELECT status, origin FROM entity_categories WHERE category_id = 'c1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("主分类应还在");
        assert_eq!(status, "confirmed");
        assert_eq!(origin, "user");
        let (status, origin): (String, String) = conn
            .query_row(
                "SELECT status, origin FROM entity_categories WHERE category_id = 'c2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("次分类应还在");
        assert_eq!(status, "suggested");
        assert_eq!(origin, "ai");
        let (status, origin): (String, String) = conn
            .query_row(
                "SELECT status, origin FROM entity_categories WHERE category_id = 'c3'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("已拒绝的主分类应还在");
        assert_eq!(status, "rejected");
        assert_eq!(origin, "ai");
    }

    #[test]
    fn confirm_card_rejects_a_missing_card() {
        let db = TempDb::new("confirm-missing");
        let conn = db.open();
        insert_repo(&conn, "empty", "active");
        insert_card(&conn, "empty", "   ", 0, "h");
        insert_repo(&conn, "bad", "active");
        insert_card(&conn, "bad", "不是 JSON", 0, "h");
        drop(conn);
        for repo_id in ["empty", "bad", "missing"] {
            let error = confirm_card(db.path(), repo_id).expect_err("没有卡应失败");
            assert_eq!(error, "这个仓库还没有理解卡");
        }
    }

    fn configured_ai() -> Value {
        json!({ "ai": { "provider": "openai", "model": "fake-model" } })
    }

    fn write_paused(db: &TempDb, paused: bool) {
        fs::write(
            jobs::queue_control_path(db.path()),
            format!(r#"{{"paused":{paused},"dailyBudgetTokens":0}}"#),
        )
        .expect("控制文件应能写入");
    }

    fn card_pair(conn: &Connection, repo_id: &str) -> (String, String) {
        (
            cell_string(
                conn,
                &format!("SELECT card_json FROM repo_ai_documents WHERE repo_id = '{repo_id}'"),
            ),
            cell_string(
                conn,
                &format!("SELECT source_hash FROM repo_ai_documents WHERE repo_id = '{repo_id}'"),
            ),
        )
    }

    fn job_digest(conn: &Connection) -> String {
        let mut statement = conn
            .prepare(
                "SELECT id, target_id, priority, status, attempts, ifnull(not_before, ''),
                        ifnull(error_kind, ''), ifnull(error_message, ''), ifnull(spend_authorized_until, '')
                 FROM ai_jobs ORDER BY id",
            )
            .expect("任务摘要应能准备");
        let rows = statement
            .query_map([], |row| {
                Ok(format!(
                    "{}|{}|{}|{}|{}|{}|{}|{}|{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .expect("任务摘要应能查询");
        rows.collect::<Result<Vec<_>, _>>()
            .expect("任务摘要应能读出")
            .join("\n")
    }

    fn auth_window(until: &str, before: &str, after: &str) {
        let low = jobs::shift_db_time(before, 1800 - 5).expect("下限");
        let high = jobs::shift_db_time(after, 1800 + 5).expect("上限");
        assert!(
            until >= low.as_str() && until <= high.as_str(),
            "授权到期 {until} 不在 {low} 和 {high} 之间"
        );
    }

    #[test]
    fn one_off_authorization_lasts_thirty_minutes() {
        // 这是每日额度第二个例外的边界。改它要同时改 AGENTS.md 的说明和任务卡。
        assert_eq!(ONE_OFF_AUTHORIZATION_SECS, 30 * 60);
    }

    fn seed_confirmed(conn: &Connection, repo_id: &str) {
        insert_repo(conn, repo_id, "active");
        insert_card(
            conn,
            repo_id,
            r#"{"review_status":"confirmed","one_liner_zh":"旧的"}"#,
            2,
            "abc",
        );
    }

    #[test]
    fn regenerate_without_confirmation_does_not_touch_the_database() {
        let db = TempDb::new("regen-unconfirmed");
        let conn = db.open();
        seed_confirmed(&conn, "r1");
        let before_card = card_pair(&conn, "r1");
        let before_jobs = job_digest(&conn);
        drop(conn);
        let error =
            request_regenerate(db.path(), &configured_ai(), "r1", false).expect_err("没确认应拒绝");
        assert_eq!(
            error,
            "重新整理这一张会花 token，需要你在界面上确认之后才能开始。"
        );
        let conn = db.open();
        assert_eq!(card_pair(&conn, "r1"), before_card);
        assert_eq!(job_digest(&conn), before_jobs);
        assert_eq!(cell_i64(&conn, "SELECT COUNT(*) FROM ai_jobs"), 0);
    }

    #[test]
    fn regenerate_without_ai_provider_does_not_touch_the_database() {
        let db = TempDb::new("regen-no-ai");
        let conn = db.open();
        seed_confirmed(&conn, "r1");
        let before_card = card_pair(&conn, "r1");
        let before_jobs = job_digest(&conn);
        drop(conn);
        let error =
            request_regenerate(db.path(), &json!({}), "r1", true).expect_err("没配置应拒绝");
        assert_eq!(
            error,
            "还没配置 AI 服务，先到「设置 → AI 设置」里选好服务商和模型、保存密钥。"
        );
        let conn = db.open();
        assert_eq!(card_pair(&conn, "r1"), before_card);
        assert_eq!(job_digest(&conn), before_jobs);
        assert_eq!(cell_i64(&conn, "SELECT COUNT(*) FROM ai_jobs"), 0);
    }

    #[test]
    fn regenerate_while_paused_does_not_touch_the_database() {
        let db = TempDb::new("regen-paused");
        let conn = db.open();
        seed_confirmed(&conn, "r1");
        let before_card = card_pair(&conn, "r1");
        let before_jobs = job_digest(&conn);
        drop(conn);
        write_paused(&db, true);
        let error =
            request_regenerate(db.path(), &configured_ai(), "r1", true).expect_err("暂停应拒绝");
        assert_eq!(
            error,
            "自动整理现在是暂停的，先到「自动整理」里恢复，再重新整理这一张。"
        );
        let conn = db.open();
        assert_eq!(card_pair(&conn, "r1"), before_card);
        assert_eq!(job_digest(&conn), before_jobs);
        assert_eq!(cell_i64(&conn, "SELECT COUNT(*) FROM ai_jobs"), 0);
    }

    #[test]
    fn regenerate_resets_hash_and_enqueues_priority_two() {
        let db = TempDb::new("regen");
        let conn = db.open();
        seed_confirmed(&conn, "r1");
        drop(conn);
        let before = jobs::now_db_time();
        request_regenerate(db.path(), &configured_ai(), "r1", true).expect("应能重做");
        let after = jobs::now_db_time();
        let conn = db.open();
        let stored = cell_string(
            &conn,
            "SELECT card_json FROM repo_ai_documents WHERE repo_id = 'r1'",
        );
        let parsed: Value = serde_json::from_str(&stored).expect("重做后仍应是 JSON");
        assert_eq!(parsed["review_status"], "suggested");
        assert_eq!(parsed["one_liner_zh"], "旧的");
        assert!(!stored.contains('\n'));
        assert_eq!(
            cell_string(
                &conn,
                "SELECT source_hash FROM repo_ai_documents WHERE repo_id = 'r1'"
            ),
            ""
        );
        let rows = job_rows(&conn);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            priority_of(&rows, "r1"),
            Some((REGENERATE_PRIORITY, "queued"))
        );
        let (attempts, max_attempts, until): (i64, i64, String) = conn
            .query_row(
                "SELECT attempts, max_attempts, spend_authorized_until FROM ai_jobs WHERE target_id = 'r1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("任务应能读出");
        assert_eq!(attempts, 0);
        assert_eq!(max_attempts, 3);
        auth_window(&until, &before, &after);
    }

    #[test]
    fn regenerate_updates_an_existing_queued_job_in_place() {
        let db = TempDb::new("regen-queued");
        let conn = db.open();
        insert_repo(&conn, "queued", "active");
        insert_card(
            &conn,
            "queued",
            r#"{"review_status":"confirmed","one_liner_zh":"甲"}"#,
            2,
            "hash",
        );
        conn.execute(
            "INSERT INTO ai_jobs(
                id, account_id, job_type, target_type, target_id, priority, status,
                attempts, max_attempts, not_before, error_kind, error_message
             ) VALUES (
                'backfill', 'acct', 'repo_card', 'repository', 'queued', 6, 'queued',
                2, 3, '2026-10-03T18:00:00.000Z', 'timeout', '旧错误'
             )",
            [],
        )
        .expect("补旧账任务应能写入");
        drop(conn);
        let before = jobs::now_db_time();
        request_regenerate(db.path(), &configured_ai(), "queued", true).expect("应能原地更新");
        let after = jobs::now_db_time();
        let conn = db.open();
        let (
            id,
            priority,
            status,
            attempts,
            max_attempts,
            not_before,
            error_kind,
            error_message,
            until,
        ): (
            String,
            i64,
            String,
            i64,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
        ) = conn
            .query_row(
                "SELECT id, priority, status, attempts, max_attempts, not_before, error_kind, error_message, spend_authorized_until
                 FROM ai_jobs WHERE target_id = 'queued'",
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
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .expect("任务应能读出");
        assert_eq!(id, "backfill");
        assert_eq!(priority, REGENERATE_PRIORITY);
        assert_eq!(status, "queued");
        assert_eq!(attempts, 0);
        assert_eq!(max_attempts, 3);
        assert!(not_before.is_none());
        assert!(error_kind.is_none());
        assert!(error_message.is_none());
        auth_window(&until, &before, &after);
        assert_eq!(
            cell_i64(
                &conn,
                "SELECT COUNT(*) FROM ai_jobs WHERE target_id = 'queued' AND job_type = 'repo_card' AND status IN ('queued', 'running')"
            ),
            1
        );
        let stored = cell_string(
            &conn,
            "SELECT card_json FROM repo_ai_documents WHERE repo_id = 'queued'",
        );
        let parsed: Value = serde_json::from_str(&stored).expect("卡片应能解析");
        assert_eq!(parsed["review_status"], "suggested");
        assert_eq!(
            cell_string(
                &conn,
                "SELECT source_hash FROM repo_ai_documents WHERE repo_id = 'queued'"
            ),
            ""
        );
    }

    #[test]
    fn regenerate_refuses_a_running_job_without_writing() {
        let db = TempDb::new("regen-running");
        let conn = db.open();
        insert_repo(&conn, "running", "active");
        insert_card(
            &conn,
            "running",
            r#"{"review_status":"confirmed","one_liner_zh":"乙"}"#,
            2,
            "hash",
        );
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             VALUES ('r', 'acct', 'repo_card', 'repository', 'running', 4, 'running')",
            [],
        )
        .expect("进行中的任务应能写入");
        let before_card = card_pair(&conn, "running");
        let before_jobs = job_digest(&conn);
        drop(conn);
        let error = request_regenerate(db.path(), &configured_ai(), "running", true)
            .expect_err("进行中应拒绝");
        assert_eq!(error, "这一张正在整理中，等它结束再试。");
        let conn = db.open();
        assert_eq!(card_pair(&conn, "running"), before_card);
        assert_eq!(job_digest(&conn), before_jobs);
        assert_eq!(
            cell_string(
                &conn,
                "SELECT ifnull(spend_authorized_until, '') FROM ai_jobs WHERE id = 'r'"
            ),
            ""
        );
        assert_eq!(
            cell_string(&conn, "SELECT status FROM ai_jobs WHERE id = 'r'"),
            "running"
        );
    }

    #[test]
    fn regenerate_does_not_authorize_another_repos_job() {
        let db = TempDb::new("regen-other");
        let conn = db.open();
        seed_confirmed(&conn, "mine");
        seed_confirmed(&conn, "other");
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             VALUES ('other-job', 'acct', 'repo_card', 'repository', 'other', 0, 'queued')",
            [],
        )
        .expect("另一个仓库的任务应能写入");
        drop(conn);
        request_regenerate(db.path(), &configured_ai(), "mine", true).expect("应能重做这一张");
        let conn = db.open();
        let other_until: Option<String> = conn
            .query_row(
                "SELECT spend_authorized_until FROM ai_jobs WHERE id = 'other-job'",
                [],
                |row| row.get(0),
            )
            .expect("另一个任务应还在");
        assert!(other_until.is_none());
        let mine_until: Option<String> = conn
            .query_row(
                "SELECT spend_authorized_until FROM ai_jobs WHERE target_id = 'mine'",
                [],
                |row| row.get(0),
            )
            .expect("这一张的任务应已写入");
        assert!(mine_until.is_some());
    }

    #[test]
    fn regenerate_missing_card_does_not_enqueue() {
        let db = TempDb::new("regen-no-card");
        let conn = db.open();
        insert_repo(&conn, "bare", "active");
        drop(conn);
        let error = request_regenerate(db.path(), &configured_ai(), "bare", true)
            .expect_err("没有理解卡应失败");
        assert_eq!(error, "这个仓库还没有理解卡");
        assert_eq!(cell_i64(&db.open(), "SELECT COUNT(*) FROM ai_jobs"), 0);
    }

    #[test]
    fn regenerate_missing_repo_is_an_error() {
        let db = TempDb::new("regen-missing");
        let error = request_regenerate(db.path(), &configured_ai(), "nope", true)
            .expect_err("不存在的仓库应失败");
        assert_eq!(error, "找不到这个仓库");
        assert_eq!(cell_i64(&db.open(), "SELECT COUNT(*) FROM ai_jobs"), 0);
    }

    #[test]
    fn regenerate_then_claim_next_takes_only_the_authorized_row() {
        let db = TempDb::new("regen-claim");
        let conn = db.open();
        seed_confirmed(&conn, "mine");
        seed_confirmed(&conn, "other");
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             VALUES ('other-job', 'acct', 'repo_card', 'repository', 'other', 0, 'queued')",
            [],
        )
        .expect("没授权的任务应能写入");
        drop(conn);
        request_regenerate(db.path(), &configured_ai(), "mine", true).expect("应能重做这一张");
        let now = jobs::now_db_time();
        let gate = jobs::BudgetGate {
            paused: false,
            daily_budget_tokens: 0,
            unlimited: false,
            full_scan_periods: Vec::new(),
        };
        let handled = vec![jobs::ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: true,
        }];
        let mut conn = db.open();
        let claimed = jobs::claim_next(&mut conn, &now, &handled, &gate)
            .expect("认领应成功")
            .expect("预算是 0 时也应领到这一张");
        assert_eq!(claimed.target_id, "mine");
        assert_eq!(claimed.reserved, jobs::CLAIM_RESERVE_TOKENS);
        assert!(jobs::claim_next(&mut conn, &now, &handled, &gate)
            .expect("第二次认领应成功返回")
            .is_none());
        assert_eq!(
            cell_string(&conn, "SELECT status FROM ai_jobs WHERE id = 'other-job'"),
            "queued"
        );
    }

    #[test]
    fn automation_status_reports_queue_budget_and_provider() {
        let db = TempDb::new("status");
        let now = now_stamp();
        let conn = db.open();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, created_at)
             VALUES ('q1', 'acct', 'repo_card', 'repository', 'q1', 'queued', ?1),
                    ('q2', 'acct', 'repo_card', 'repository', 'q2', 'queued', ?1),
                    ('run', 'acct', 'repo_card', 'repository', 'run', 'running', ?1)",
            [&now],
        )
        .expect("任务应能写入");
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, output_tokens, finished_at, created_at)
             VALUES ('ok', 'acct', 'repo_card', 'repository', 'ok', 'succeeded', 10, 5, ?1, ?1),
                    ('bad', 'acct', 'repo_card', 'repository', 'bad', 'failed', 1, 0, ?1, ?1)",
            [&now],
        )
        .expect("已结束任务应能写入");
        drop(conn);
        let control = jobs::queue_control_path(db.path());
        fs::write(
            &control,
            r#"{"paused":false,"dailyBudgetTokens":50000,"pauseReason":null}"#,
        )
        .expect("控制文件应能写入");
        let settings = json!({
            "ai": {"provider": "openai", "model": "fake-model", "apiKey": "sk-SHOULD-NOT-LEAK"}
        });
        let status = automation_status(db.path(), &settings).expect("状态应能读出");
        assert!(status.enabled);
        assert_eq!(status.daily_budget_tokens, 50_000);
        assert!(!status.paused);
        assert!(status.pause_reason.is_none());
        assert_eq!(status.tokens_24h, 16);
        assert_eq!(status.daily_tokens_used, 16);
        assert_eq!(status.budget_remaining, 50_000 - 16);
        assert_eq!(status.queued, 2);
        assert_eq!(status.running, 1);
        assert_eq!(status.failed, 1);
        assert!(status.provider_configured);
        assert_eq!(status.model.as_deref(), Some("fake-model"));
        let text = serde_json::to_string(&status).expect("状态应能序列化");
        assert!(!text.contains("sk-SHOULD-NOT-LEAK"));
        assert!(!text.contains("apiKey"));
    }

    #[test]
    fn automation_status_json_keys_match_the_contract() {
        let db = TempDb::new("keys");
        let settings = json!({"ai": {"provider": "openai", "model": "m"}});
        fs::write(
            jobs::queue_control_path(db.path()),
            r#"{"paused":true,"dailyBudgetTokens":10,"pauseReason":{"kind":"auth","message":"密钥无效","at":"2026-10-03T00:00:00.000Z"}}"#,
        )
        .expect("控制文件应能写入");
        let with_reason = automation_status(db.path(), &settings).expect("状态应能读出");
        assert_eq!(json_keys(&with_reason), expected_status_keys());
        let reason = with_reason.pause_reason.expect("应有暂停原因");
        let reason_keys = serde_json::to_value(&reason)
            .expect("暂停原因应能序列化")
            .as_object()
            .expect("暂停原因应是对象")
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            reason_keys,
            BTreeSet::from(["kind".to_owned(), "message".to_owned(), "at".to_owned()])
        );

        let plain = automation_status(&TempDb::new("keys-empty").path(), &json!({}))
            .expect("空状态应能读出");
        let value = serde_json::to_value(&plain).expect("状态应能序列化");
        assert_eq!(json_keys(&plain), expected_status_keys());
        assert!(value.get("pauseReason").expect("键应存在").is_null());
        assert!(value.get("model").expect("键应存在").is_null());
    }

    fn json_keys(status: &AutomationStatus) -> BTreeSet<String> {
        serde_json::to_value(status)
            .expect("状态应能序列化")
            .as_object()
            .expect("状态应是对象")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn automation_status_surfaces_last_readme_check() {
        let db = TempDb::new("readme-status");
        fs::write(
            jobs::queue_control_path(db.path()),
            r#"{"paused":false,"dailyBudgetTokens":1000,"lastReadmeCheck":{"checkedAt":"2026-10-05T10:20:00.000Z","checked":23,"refreshed":8,"material":3,"queued":2}}"#,
        )
        .expect("控制文件应能写入");
        let status = automation_status(db.path(), &json!({})).expect("状态应能读出");
        let check = status.last_readme_check.clone().expect("应带出上次检查");
        assert_eq!(check.checked_at, "2026-10-05T10:20:00.000Z");
        assert_eq!(check.checked, 23);
        assert_eq!(check.refreshed, 8);
        assert_eq!(check.material, 3);
        assert_eq!(check.queued, 2);
        let text = serde_json::to_string(&status).expect("状态应能序列化");
        assert!(text.contains("lastReadmeCheck"));
    }

    #[test]
    fn readme_changed_scope_enqueues_at_priority_five_without_touching_budget() {
        let db = TempDb::new("scope-readme");
        let conn = db.open();
        insert_repo(&conn, "fresh", "active");
        insert_readme(&conn, "fresh");
        insert_card(&conn, "fresh", r#"{"review_status":"suggested"}"#, 2, "h");
        drop(conn);
        assert!(!jobs::queue_control_path(db.path()).exists());
        assert_eq!(
            enqueue_cards_for_scope(db.path(), "readme_changed").expect("应能入队"),
            1
        );
        assert!(!jobs::queue_control_path(db.path()).exists());
        assert_eq!(
            cell_i64(
                &db.open(),
                "SELECT priority FROM ai_jobs WHERE target_id = 'fresh'"
            ),
            5
        );
        assert_eq!(
            enqueue_cards_for_scope(db.path(), "readme_changed").expect("重复入队应为 0"),
            0
        );
    }

    fn expected_status_keys() -> BTreeSet<String> {
        [
            "enabled",
            "dailyBudgetTokens",
            "paused",
            "pauseReason",
            "tokens24h",
            "dailyTokensUsed",
            "budgetRemaining",
            "queued",
            "running",
            "failed",
            "providerConfigured",
            "model",
            "unlimited",
            "fullScan",
            "lastFullScan",
            "lastReadmeCheck",
            "needsOrganizing",
            "activeRepos",
            "needsOrganizingMissing",
            "needsOrganizingOutdated",
            "awaiting",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    #[test]
    fn automation_status_splits_missing_outdated_and_deduplicates_awaiting_repos() {
        let db = TempDb::new("status-split-counts");
        let conn = db.open();
        for id in ["a", "b", "c", "d", "f", "g"] {
            insert_repo(&conn, id, "active");
        }
        insert_repo(&conn, "e", "removed");
        for (id, schema, review) in [
            ("b", 1, "suggested"),
            ("c", 2, "suggested"),
            ("d", 1, "confirmed"),
            ("f", 2, "suggested"),
        ] {
            insert_card(
                &conn,
                id,
                &json!({"review_status": review}).to_string(),
                schema,
                "h",
            );
        }
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status)
             VALUES ('f-queued', 'acct', 'repo_card', 'repository', 'f', 'queued'),
                    ('g-queued', 'acct', 'repo_card', 'repository', 'g', 'queued')",
            [],
        )
        .unwrap();
        // c 已有新版说明，归类任务排队不能把它算进待整理的理解卡。
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status)
             VALUES ('c-categorize', 'acct', 'categorize', 'repository', 'c', 'queued')",
            [],
        )
        .unwrap();
        let status = automation_status(db.path(), &fs_settings()).unwrap();
        assert_eq!(status.active_repos, 6);
        assert_eq!(
            status.needs_organizing_missing, 2,
            "a、g 没有卡；已移除的 e 不算"
        );
        assert_eq!(
            status.needs_organizing_outdated, 1,
            "b 是旧格式；已确认的 d 不算"
        );
        assert_eq!(status.needs_organizing, 3);
        assert_eq!(
            status.needs_organizing,
            status.needs_organizing_missing + status.needs_organizing_outdated
        );
        assert_eq!(status.awaiting, 4, "a、b、f、g；g 既没卡又排队只算一次");
        conn.execute(
            "UPDATE ai_jobs SET status = 'running' WHERE id = 'c-categorize'",
            [],
        )
        .unwrap();
        assert_eq!(
            automation_status(db.path(), &fs_settings())
                .unwrap()
                .awaiting,
            4,
            "c 的归类任务进行中也不算待整理，仍然只有 a、b、f、g"
        );
        conn.execute(
            "UPDATE ai_jobs SET status = 'running' WHERE id = 'f-queued'",
            [],
        )
        .unwrap();
        assert_eq!(
            automation_status(db.path(), &fs_settings())
                .unwrap()
                .awaiting,
            4
        );
        // start_full_scan 也保留原本已经在排队的任务；补排后集合与 awaiting 完全一致。
        let scan = start_full_scan(db.path(), &fs_settings(), true).unwrap();
        assert_eq!(scan.awaiting, 4);
        assert_eq!(scan.full_scan.unwrap().total, 4);
    }

    #[test]
    fn automation_status_reports_daily_usage_separately_from_full_scan_usage() {
        let db = TempDb::new("status-daily-usage");
        let now = jobs::now_db_time();
        let start = jobs::shift_db_time(&now, -7200).unwrap();
        let inside = jobs::shift_db_time(&now, -5400).unwrap();
        let finish = jobs::shift_db_time(&now, -3600).unwrap();
        let conn = db.open();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, finished_at)
             VALUES ('scan', 'acct', 'repo_card', 'repository', 'scan', 'succeeded', 3000000, ?1),
                    ('auto', 'acct', 'repo_card', 'repository', 'auto', 'succeeded', 300, ?2)",
            params![inside, now],
        ).unwrap();
        fs::write(
            jobs::queue_control_path(db.path()),
            json!({
                "dailyBudgetTokens": 50_000,
                "recentFullScans": [{"startedAt": start, "finishedAt": finish}]
            })
            .to_string(),
        )
        .unwrap();
        let status = automation_status(db.path(), &fs_settings()).unwrap();
        assert_eq!(status.tokens_24h, 3_000_300);
        assert_eq!(status.daily_tokens_used, 300);
        assert_eq!(status.budget_remaining, 50_000 - 300);
        assert_eq!(
            (
                status.active_repos,
                status.needs_organizing_missing,
                status.needs_organizing_outdated,
                status.needs_organizing,
                status.awaiting
            ),
            (0, 0, 0, 0, 0)
        );
    }

    // —— 全量整理 ——

    fn fs_settings() -> Value {
        json!({"ai": {"provider": "openai", "model": "fake-model"}})
    }

    fn open_jobs(db: &TempDb) -> i64 {
        cell_i64(
            &db.open(),
            "SELECT COUNT(*) FROM ai_jobs WHERE job_type = 'repo_card' AND status IN ('queued', 'running')",
        )
    }

    /// r1 没有卡；r2 旧版卡（第 1 版）；r3 现行卡；r4 旧版但已确认；r5 已移除；r6 卡是空串。需要整理的是 r1、r2、r6。
    fn seed_mixed_cards(db: &TempDb) {
        prepare_many_runnable(db, 6);
        let conn = db.open();
        insert_card(&conn, "r2", r#"{"review_status":"suggested"}"#, 1, "h2");
        insert_card(&conn, "r3", r#"{"review_status":"suggested"}"#, 2, "h3");
        insert_card(&conn, "r4", r#"{"review_status":"confirmed"}"#, 1, "h4");
        insert_card(&conn, "r6", "", 2, "h6");
        conn.execute(
            "UPDATE repositories SET sync_status = 'removed' WHERE id = 'r5'",
            [],
        )
        .expect("应能标记已移除");
    }

    #[test]
    fn a_full_scan_needs_confirmation_and_a_configured_provider() {
        let db = TempDb::new("fs-guard");
        seed_mixed_cards(&db);
        let error = start_full_scan(db.path(), &fs_settings(), false).expect_err("没确认应拒绝");
        assert!(error.contains("确认"), "{error}");
        let error = start_full_scan(db.path(), &json!({}), true).expect_err("没配置 AI 服务应拒绝");
        assert!(error.contains("还没配置 AI 服务"), "{error}");
        let none = json!({"ai": {"provider": "none", "model": "m"}});
        assert!(start_full_scan(db.path(), &none, true).is_err());
        // 什么也没发生：没排队、没写全量整理
        assert_eq!(open_jobs(&db), 0);
        assert!(jobs::read_control(&jobs::queue_control_path(db.path()))
            .full_scan
            .is_none());
    }

    #[test]
    fn the_full_scan_refusals_use_the_current_names() {
        // 卡 37：不再说「全量整理」「设置 → 引擎」；整句逐字对
        let db = TempDb::new("fs-wording");
        assert_eq!(
            start_full_scan(db.path(), &fs_settings(), false).expect_err("没确认应拒绝"),
            "「现在整理」会花钱，需要你确认之后才能开始。"
        );
        assert_eq!(
            start_full_scan(db.path(), &json!({}), true).expect_err("没配置 AI 服务应拒绝"),
            "还没配置 AI 服务，先到「设置 → AI 设置」里选好服务商和模型、保存密钥。"
        );
        // 确认并配置 AI，但库里没有要整理的：也没有开始一次性全部整理。
        assert_eq!(
            start_full_scan(db.path(), &fs_settings(), true).expect_err("没有要整理的应拒绝"),
            "没有需要整理的仓库了。"
        );
        assert!(jobs::read_control(&jobs::queue_control_path(db.path()))
            .full_scan
            .is_none());
    }

    #[test]
    fn cards_written_by_an_older_prompt_are_outdated_and_current_or_confirmed_ones_are_not() {
        let db = TempDb::new("older-prompt");
        let conn = db.open();
        for id in ["old-prompt", "current", "confirmed-old", "no-card"] {
            insert_repo(&conn, id, "active");
        }
        conn.execute(
            "INSERT INTO repo_ai_documents(repo_id, summary_zh, model, prompt_version, source_hash, generated_at, card_json, card_schema_version)
             VALUES ('old-prompt', 's', 'm', 'card-v2', 'h', '2026-01-01T00:00:00Z', '{\"review_status\":\"suggested\"}', 2),
                    ('current', 's', 'm', ?1, 'h', '2026-01-01T00:00:00Z', '{\"review_status\":\"suggested\"}', 2),
                    ('confirmed-old', 's', 'm', 'card-v2', 'h', '2026-01-01T00:00:00Z', '{\"review_status\":\"confirmed\"}', 2)",
            params![PROMPT_VERSION],
        )
        .expect("理解卡应能写入");
        drop(conn);
        // 「说明过期」= 旧结构，或旧版提示词写的（没被确认）；没有卡的不算过期（那是「缺失」）
        let mut conn = db.open();
        assert_eq!(
            enqueue_cards(&mut conn, CardEnqueueScope::Outdated, 6).expect("应能入队"),
            1
        );
        drop(conn);
        let rows = job_rows(&db.open());
        assert_eq!(rows.len(), 1);
        assert_eq!(priority_of(&rows, "old-prompt"), Some((6, "queued")));
        // 一次性全部整理要数的「还要整理多少」= 没有卡的 + 过期的
        let status = automation_status(db.path(), &fs_settings()).expect("状态应能读出");
        assert_eq!(status.needs_organizing, 2, "old-prompt 和 no-card");
        assert_eq!(PROMPT_VERSION, "card-v3");
    }

    #[test]
    fn a_full_scan_queues_everything_that_needs_a_card_and_reports_progress() {
        let db = TempDb::new("fs-start");
        seed_mixed_cards(&db);
        let control = jobs::queue_control_path(db.path());
        // 日常额度关着（0）、而且被暂停了：点开始就是要它跑
        fs::write(
            &control,
            r#"{"paused":true,"dailyBudgetTokens":0,"pauseReason":{"kind":"auth","message":"旧原因","at":"2026-10-03T00:00:00.000Z"}}"#,
        )
        .expect("控制文件应能写入");
        let before = automation_status(db.path(), &fs_settings()).expect("状态应能读出");
        assert_eq!(
            before.needs_organizing, 3,
            "r1、r2、r6；已确认、现行卡、已移除的不算"
        );
        assert!(!before.unlimited && before.full_scan.is_none() && before.last_full_scan.is_none());
        assert!(!before.enabled);

        let status = start_full_scan(db.path(), &fs_settings(), true).expect("应能开始");
        assert!(status.unlimited);
        assert!(status.enabled, "全量整理期间即使日常额度是 0 也算开着");
        assert!(!status.paused && status.pause_reason.is_none());
        assert_eq!(status.queued, 3);
        let scan = status.full_scan.clone().expect("应有进度");
        assert_eq!((scan.total, scan.remaining, scan.failed), (3, 3, 0));
        assert_eq!(scan.tokens_used, 0);
        assert_eq!(open_jobs(&db), 3);
        assert_eq!(status.daily_budget_tokens, 0, "日常额度原样保留");
        let queued: Vec<String> = {
            let conn = db.open();
            let mut statement = conn
                .prepare("SELECT target_id FROM ai_jobs ORDER BY target_id")
                .expect("应能查询");
            statement
                .query_map([], |row| row.get(0))
                .expect("应能读取")
                .map(|row| row.expect("行应可读"))
                .collect()
        };
        assert_eq!(queued, vec!["r1", "r2", "r6"]);

        // 再点一次：原样返回，不重置、不重复排队
        let again = start_full_scan(db.path(), &fs_settings(), true).expect("重复开始应无害");
        assert_eq!(
            again.full_scan.map(|scan| scan.started_at),
            Some(scan.started_at.clone())
        );
        assert_eq!(open_jobs(&db), 3);

        // 停止：回到日常额度，排队的留着，留下记录
        let stopped = stop_full_scan(db.path(), &fs_settings()).expect("应能停止");
        assert!(!stopped.unlimited && stopped.full_scan.is_none());
        assert!(!stopped.enabled, "日常额度是 0：停止后回到「未开启」");
        let last = stopped.last_full_scan.expect("应留下记录");
        assert_eq!(last.reason, "stopped");
        assert_eq!((last.total, last.remaining), (3, 3));
        assert_eq!(open_jobs(&db), 3);
        // 没有在进行时再停：无害
        assert!(stop_full_scan(db.path(), &fs_settings()).is_ok());
    }

    #[test]
    fn a_full_scan_with_nothing_to_organize_says_so_and_changes_nothing() {
        let db = TempDb::new("fs-nothing");
        prepare_many_runnable(&db, 2);
        let conn = db.open();
        insert_card(&conn, "r1", r#"{"review_status":"suggested"}"#, 2, "h1");
        insert_card(&conn, "r2", r#"{"review_status":"suggested"}"#, 2, "h2");
        drop(conn);
        let error = start_full_scan(db.path(), &fs_settings(), true).expect_err("没有要整理的");
        assert!(error.contains("没有需要整理"), "{error}");
        assert!(jobs::read_control(&jobs::queue_control_path(db.path()))
            .full_scan
            .is_none());
    }

    #[test]
    fn starting_again_first_archives_a_scan_that_ran_out_of_time() {
        let db = TempDb::new("fs-restart");
        seed_mixed_cards(&db);
        let long_ago = jobs::shift_db_time(&jobs::now_db_time(), -49 * 3600).expect("时间应能计算");
        // 49 小时前开始的一轮已经到时间、还没来得及收尾；队列里还有排着的
        db.open()
            .execute(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, created_at)
                 VALUES ('old-q', 'acct', 'repo_card', 'repository', 'r1', 'queued', ?1)",
                [&long_ago],
            )
            .expect("任务应能写入");
        fs::write(
            jobs::queue_control_path(db.path()),
            format!(
                r#"{{"dailyBudgetTokens":0,"fullScan":{{"startedAt":"{long_ago}","until":"x","total":9}}}}"#
            ),
        )
        .expect("控制文件应能写入");
        let status = start_full_scan(db.path(), &fs_settings(), true).expect("应能开始新的一轮");
        let last = status
            .last_full_scan
            .expect("上一轮到时间的要留下记录，不能被新的一轮盖掉");
        assert_eq!(
            (last.reason.as_str(), last.started_at.as_str()),
            ("expired", long_ago.as_str())
        );
        let scan = status.full_scan.expect("新的一轮应在进行");
        assert!(scan.started_at > long_ago);
        assert_eq!(scan.total, 3, "r1（已在排队）、r2、r6");
    }

    #[test]
    fn the_status_finishes_a_full_scan_that_ended_while_nobody_was_looking() {
        let db = TempDb::new("fs-settle");
        prepare_many_runnable(&db, 1);
        let started = jobs::shift_db_time(&jobs::now_db_time(), -3 * 3600).expect("时间应能计算");
        fs::write(
            jobs::queue_control_path(db.path()),
            format!(
                r#"{{"dailyBudgetTokens":100000,"fullScan":{{"startedAt":"{started}","until":"x","total":4}}}}"#
            ),
        )
        .expect("控制文件应能写入");
        // 没有排队的、没有进行的：整理完了
        let status = automation_status(db.path(), &fs_settings()).expect("状态应能读出");
        assert!(!status.unlimited && status.full_scan.is_none());
        let last = status.last_full_scan.expect("读状态时应收尾并留下记录");
        assert_eq!(
            (last.reason.as_str(), last.total, last.remaining),
            ("completed", 4, 0)
        );
    }

    #[test]
    fn provider_configured_reads_settings_only() {
        let db = TempDb::new("provider");
        let configured = automation_status(
            db.path(),
            &json!({"ai": {"provider": "openai-compatible", "model": " fake-model ", "apiKey": ""}}),
        )
        .expect("状态应能读出");
        assert!(configured.provider_configured);
        assert_eq!(configured.model.as_deref(), Some("fake-model"));

        let none = automation_status(
            db.path(),
            &json!({"ai": {"provider": "none", "model": "fake-model", "apiKey": "sk-secret"}}),
        )
        .expect("状态应能读出");
        assert!(!none.provider_configured);
        assert_eq!(none.model.as_deref(), Some("fake-model"));
        assert!(!serde_json::to_string(&none)
            .expect("应能序列化")
            .contains("sk-secret"));

        let no_model = automation_status(
            db.path(),
            &json!({"ai": {"provider": "openai", "model": "  "}}),
        )
        .expect("状态应能读出");
        assert!(!no_model.provider_configured);
        assert!(no_model.model.is_none());

        let missing = automation_status(db.path(), &json!({})).expect("状态应能读出");
        assert!(!missing.provider_configured);
        assert!(missing.model.is_none());
    }

    #[test]
    fn budget_defaults_to_zero() {
        let db = TempDb::new("budget-zero");
        assert!(!jobs::queue_control_path(db.path()).exists());
        let status = automation_status(db.path(), &json!({})).expect("状态应能读出");
        assert_eq!(status.daily_budget_tokens, 0);
        assert!(!status.enabled);
        assert!(!status.paused);
        assert_eq!(status.budget_remaining, 0);
        assert_eq!(status.tokens_24h, 0);
        assert!(status.pause_reason.is_none());
    }

    #[test]
    fn set_budget_caps_and_does_not_clear_pause() {
        let db = TempDb::new("set-budget");
        let settings = json!({"ai": {"provider": "openai", "model": "m"}});
        let closed = set_automation_budget(db.path(), &settings, 0).expect("应能写入 0");
        assert_eq!(closed.daily_budget_tokens, 0);
        assert!(!closed.enabled);
        let opened = set_automation_budget(db.path(), &settings, 150).expect("应能写入额度");
        assert_eq!(opened.daily_budget_tokens, 150);
        assert!(opened.enabled);
        assert!(!opened.paused);
        let capped = set_automation_budget(db.path(), &settings, 9_000_000).expect("超限应被压住");
        assert_eq!(capped.daily_budget_tokens, 2_000_000);

        fs::write(
            jobs::queue_control_path(db.path()),
            r#"{"paused":true,"dailyBudgetTokens":10,"note":"keep","pauseReason":{"kind":"auth","message":"密钥无效","at":"2026-10-03T00:00:00.000Z"}}"#,
        )
        .expect("控制文件应能写入");
        let still = set_automation_budget(db.path(), &settings, 80).expect("改额度不应清掉暂停");
        assert_eq!(still.daily_budget_tokens, 80);
        assert!(still.paused);
        assert!(!still.enabled);
        assert_eq!(
            still.pause_reason.expect("暂停原因应保留").message,
            "密钥无效"
        );
        let raw = fs::read_to_string(jobs::queue_control_path(db.path())).expect("控制文件应能读");
        assert!(raw.contains("keep"));
    }

    #[test]
    fn enqueue_scope_does_not_authorize_spending() {
        let db = TempDb::new("scope-budget");
        let conn = db.open();
        insert_repo(&conn, "bare", "active");
        insert_repo(&conn, "old", "active");
        insert_card(&conn, "old", r#"{"review_status":"suggested"}"#, 0, "h");
        drop(conn);
        assert!(!jobs::queue_control_path(db.path()).exists());
        assert_eq!(
            enqueue_cards_for_scope(db.path(), "missing").expect("应能入队"),
            1
        );
        assert_eq!(
            enqueue_cards_for_scope(db.path(), "outdated").expect("应能入队"),
            1
        );
        assert!(!jobs::queue_control_path(db.path()).exists());
        jobs::set_daily_budget(&jobs::queue_control_path(db.path()), 77).expect("测试预算应能写入");
        assert_eq!(
            enqueue_cards_for_scope(db.path(), "missing").expect("重复入队应为 0"),
            0
        );
        assert_eq!(
            jobs::read_control(&jobs::queue_control_path(db.path())).daily_budget_tokens,
            77
        );
        let error = enqueue_cards_for_scope(db.path(), "all").expect_err("未知范围应失败");
        assert_eq!(error, "排队范围只能是 missing、outdated 或 readme_changed");
        assert_eq!(cell_i64(&db.open(), "SELECT COUNT(*) FROM ai_jobs"), 2);
        assert_eq!(
            cell_i64(
                &db.open(),
                "SELECT priority FROM ai_jobs WHERE target_id = 'bare'"
            ),
            6
        );
        assert_eq!(
            cell_i64(
                &db.open(),
                "SELECT priority FROM ai_jobs WHERE target_id = 'old'"
            ),
            6
        );
    }

    #[test]
    fn provider_rejections_are_not_charged_and_not_retried() {
        let rejected = |status: u16| CardRunError {
            kind: CardRunErrorKind::Other,
            message: format!("AI 接口请求失败（HTTP {status}）：请求被拒绝"),
            retry_after_secs: None,
            spent_tokens: 0,
        };
        // 402（余额不足）/ 404（模型名或请求地址配错了）：停下整条队列让人去处理
        for status in [402, 404] {
            assert!(
                matches!(
                    map_card_error(rejected(status)),
                    JobFailure::PauseQueue {
                        kind: ErrorKind::Other,
                        ..
                    }
                ),
                "HTTP {status}"
            );
        }
        // 别的 4xx（400、405、409、410、413、415、422、451 ……）：只让这一条失败，用「不计费」的种类，不重试
        for status in [400, 405, 409, 410, 413, 415, 422, 451] {
            assert!(
                matches!(
                    map_card_error(rejected(status)),
                    JobFailure::Fatal {
                        kind: ErrorKind::ContentTooLarge,
                        ..
                    }
                ),
                "HTTP {status}"
            );
        }
        // 500 之类仍按原来的方式处理（可重试）
        assert!(matches!(
            map_card_error(rejected(500)),
            JobFailure::Retryable { .. }
        ));
        assert_eq!(http_status_in("没有状态码"), None);
        assert_eq!(http_status_in("（HTTP 4）"), None);
    }

    #[test]
    fn a_failed_repair_call_keeps_the_first_calls_usage_on_the_job() {
        let db = TempDb::new("repair-spent");
        prepare_runnable(&db);
        db.open()
            .execute(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts)
                 VALUES ('job-1', 'acct', 'repo_card', 'repository', 'r1', 'running', 1)",
                [],
            )
            .unwrap();
        // 第一次调用成功（已计费 3000 + 500）但回的不是 JSON，紧接着的修复调用断网了：
        // 「断网」本身记 0，但第一次那笔 3500 不能丢
        let (handler, _) = scripted(
            Ok(ok_config()),
            vec![
                reply("这不是 JSON", "m", 3000, 500),
                llm_err(LlmErrorKind::Network, "断了"),
            ],
        );
        assert_retryable(
            handler.run(&job("job-1", "repository", "r1"), db.path()),
            ErrorKind::Network,
            None,
            "断了",
        );
        assert_eq!(
            cell_i64(
                &db.open(),
                "SELECT input_tokens FROM ai_jobs WHERE id = 'job-1'"
            ),
            3500
        );
    }

    #[test]
    fn five_rejections_in_a_row_pause_the_queue_and_any_other_result_resets_the_count() {
        let (handler, _) = scripted(Ok(ok_config()), vec![]);
        let rejected = || JobFailure::Fatal {
            kind: ErrorKind::ContentTooLarge,
            message: "AI 接口请求失败（HTTP 422）：请求被拒绝".to_owned(),
        };
        // 连着被拒绝：前 4 个只是各自失败
        for _ in 0..4 {
            assert!(matches!(
                handler.track_rejections(rejected(), false),
                JobFailure::Fatal { .. }
            ));
        }
        // 中间夹一个别的结果（比如超时）：清零，重新数
        assert!(matches!(
            handler.track_rejections(
                JobFailure::Retryable {
                    kind: ErrorKind::Timeout,
                    message: "超时".to_owned(),
                    retry_after_secs: None,
                },
                false
            ),
            JobFailure::Retryable { .. }
        ));
        for _ in 0..4 {
            assert!(matches!(
                handler.track_rejections(rejected(), false),
                JobFailure::Fatal { .. }
            ));
        }
        // 第 5 个：升级成暂停整条队列，信息里带着原因
        match handler.track_rejections(rejected(), false) {
            JobFailure::PauseQueue { kind, message } => {
                assert_eq!(kind, ErrorKind::Other);
                assert!(message.contains("连续 5 个"), "{message}");
                assert!(message.contains("HTTP 422"), "{message}");
                assert!(message.contains("继续整理"), "{message}");
            }
            other => panic!("应暂停整条队列，实际 {other:?}"),
        }
        // 暂停之后重新数
        assert!(matches!(
            handler.track_rejections(rejected(), false),
            JobFailure::Fatal { .. }
        ));
        // 「内容超过模型上下文」是这一条自己的问题（一个特别长的 README），不算被拒绝：连着 10 个也不停队列
        for _ in 0..10 {
            assert!(matches!(
                handler.track_rejections(
                    JobFailure::Fatal {
                        kind: ErrorKind::ContentTooLarge,
                        message: "AI 接口请求失败（HTTP 400）：内容超过模型上下文或最大 token 限制，请缩短输入。".to_owned(),
                    },
                    false
                ),
                JobFailure::Fatal { .. }
            ));
        }
    }

    /// 种 `count` 个能跑的仓库 r1..r{count}，每个都有 README。
    fn prepare_many_runnable(db: &TempDb, count: usize) {
        let conn = db.open();
        seed_catalog(&conn);
        for index in 1..=count {
            let id = format!("r{index}");
            insert_repo(&conn, &id, "active");
            insert_readme(&conn, &id);
        }
    }

    fn rejection_reply() -> Result<LlmReply, LlmError> {
        llm_err(
            LlmErrorKind::Other,
            "AI 接口请求失败（HTTP 400）：Content Exists Risk",
        )
    }

    #[test]
    fn run_pauses_after_five_rejections_when_nothing_has_worked_yet() {
        let db = TempDb::new("rejections-unproven");
        prepare_many_runnable(&db, 6);
        let (handler, _) = scripted(Ok(ok_config()), (0..6).map(|_| rejection_reply()).collect());
        for index in 1..=4 {
            match handler.run(
                &job(&format!("j{index}"), "repository", &format!("r{index}")),
                db.path(),
            ) {
                Err(JobFailure::Fatal { .. }) => {}
                other => panic!("第 {index} 个只应各自失败，实际 {other:?}"),
            }
        }
        match handler.run(&job("j5", "repository", "r5"), db.path()) {
            Err(JobFailure::PauseQueue { message, .. }) => {
                assert!(message.contains("连续 5 个"), "{message}");
            }
            other => panic!("第 5 个应暂停整条队列，实际 {other:?}"),
        }
    }

    #[test]
    fn run_widens_the_limit_only_after_a_real_success_with_the_same_config() {
        let db = TempDb::new("rejections-proven");
        prepare_many_runnable(&db, 25);
        let mut replies = vec![reply(&valid_json(), "fake-model", 111, 22)];
        replies.extend((0..22).map(|_| rejection_reply()));
        let (handler, _) = scripted(Ok(ok_config()), replies);
        handler
            .run(&job("j1", "repository", "r1"), db.path())
            .expect("第一张应成功");
        // 成功之后：连着 19 个内容被拒绝，只是各自失败
        for index in 2..=20 {
            match handler.run(
                &job(&format!("j{index}"), "repository", &format!("r{index}")),
                db.path(),
            ) {
                Err(JobFailure::Fatal { .. }) => {}
                other => panic!("第 {index} 个只应各自失败，实际 {other:?}"),
            }
        }
        // 第 20 个还是停下（比如账户余额没了、Anthropic 也回 400）
        match handler.run(&job("j21", "repository", "r21"), db.path()) {
            Err(JobFailure::PauseQueue { message, .. }) => {
                assert!(message.contains("连续 20 个"), "{message}");
            }
            other => panic!("第 20 个应暂停整条队列，实际 {other:?}"),
        }
    }

    #[test]
    fn a_success_with_another_model_does_not_widen_the_limit_for_this_one() {
        // 用模型 A 成功过，之后配置换成模型 B：A 的成功不证明 B 的配置是对的
        let db = TempDb::new("rejections-other-model");
        prepare_many_runnable(&db, 8);
        let model = Arc::new(Mutex::new("model-a".to_owned()));
        let model_for_config = Arc::clone(&model);
        let replies = Arc::new(Mutex::new({
            let mut list = vec![reply(&valid_json(), "fake-model", 111, 22)];
            list.extend((0..6).map(|_| rejection_reply()));
            list
        }));
        let replies_for_factory = Arc::clone(&replies);
        let calls = Arc::new(AtomicUsize::new(0));
        let handler = RepoCardHandler::with_client_factory(
            Arc::new(move || {
                let mut config = ok_config();
                config.model = model_for_config.lock().expect("模型锁应可用").clone();
                Ok(config)
            }),
            Arc::new(move || {
                Box::new(Scripted {
                    replies: Arc::clone(&replies_for_factory),
                    calls: Arc::clone(&calls),
                })
            }),
        );
        handler
            .run(&job("j1", "repository", "r1"), db.path())
            .expect("模型 A 应成功");
        *model.lock().expect("模型锁应可用") = "model-b".to_owned();
        for index in 2..=5 {
            match handler.run(
                &job(&format!("j{index}"), "repository", &format!("r{index}")),
                db.path(),
            ) {
                Err(JobFailure::Fatal { .. }) => {}
                other => panic!("第 {index} 个只应各自失败，实际 {other:?}"),
            }
        }
        match handler.run(&job("j6", "repository", "r6"), db.path()) {
            Err(JobFailure::PauseQueue { message, .. }) => {
                assert!(message.contains("连续 5 个"), "{message}");
            }
            other => panic!("换了模型后，第 5 个被拒绝就该暂停，实际 {other:?}"),
        }
    }

    #[test]
    fn once_the_config_has_worked_the_rejection_limit_is_wider_but_still_there() {
        // 配置已经成功跑过：某几个仓库的内容被服务商拒绝（内容审核的 400）是它们自己的问题，不能 5 个就停整条队列；
        // 但连着 20 个都被拒绝还是不对劲（比如账户余额没了、Anthropic 也回 400），要停
        let (handler, _) = scripted(Ok(ok_config()), vec![]);
        let rejected = || JobFailure::Fatal {
            kind: ErrorKind::ContentTooLarge,
            message: "AI 接口请求失败（HTTP 400）：Content Exists Risk".to_owned(),
        };
        for _ in 0..19 {
            assert!(matches!(
                handler.track_rejections(rejected(), true),
                JobFailure::Fatal { .. }
            ));
        }
        match handler.track_rejections(rejected(), true) {
            JobFailure::PauseQueue { message, .. } => {
                assert!(message.contains("连续 20 个"), "{message}");
            }
            other => panic!("第 20 个应暂停整条队列，实际 {other:?}"),
        }
    }
}
