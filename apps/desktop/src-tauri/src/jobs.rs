//! AI 任务队列的消费者。不依赖 Tauri，也不调用任何会花钱的接口。
//!
//! 默认预算是 0：控制文件不存在，或没有 `dailyBudgetTokens` 时，花钱的任务不会被认领。
//! 用量按滚动 24 小时计算（避开时区）。认领是预算的硬门：在一个 `BEGIN IMMEDIATE` 里检查
//! 「已用 + 这一条的预留 ≤ 额度」，并把预留（`CLAIM_RESERVE_TOKENS`）写进这条任务自己的行，
//! 后一次认领一定看得见前一次的预留；任务结束时预留换成真实用量（失败按种类记）。
//! 取消、崩溃、被打断的任务保留预留，账不会消失。旧版 AI 功能不走队列，由 `spend_guard` 记进同一本账。
//!
//! 唯一的例外是「全量整理」（`fullScan`）：Fox 亲手点了确认之后，这一轮不看每天额度，一直整理到全部完成，
//! 最长 `FULL_SCAN_SECS`（48 小时），之后自动回到每天额度。用量照常记账；用量超过「预计 × 4」的刹车线也会自动结束。
//!
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_DAILY_BUDGET: u64 = 2_000_000;

/// 「全量整理」最长运行多久（从开始算）。Fox 点了确认才开，到点自动结束，免得忘了关就一直不限额。
pub(crate) const FULL_SCAN_SECS: i64 = 48 * 60 * 60;
/// 一张理解卡用多少 token（2026-10-04 真实记录：成功的卡平均约 6.3k，含思考；输出额度放宽后偶尔还要被截断重试，按 7.5k 算）。
/// 只用来给全量整理算「远超预计」的刹车线。
const FULL_SCAN_TOKENS_PER_CARD: i64 = 7_500;
/// 全量整理的用量超过「预计 × 4」（至少 300 万）就自动结束：说明哪里不对劲了（死循环、反复重试、提示词暴涨），
/// 比让它一直不限额地花下去强。正常整理远到不了这条线。
const FULL_SCAN_CEILING_FACTOR: i64 = 4;
const FULL_SCAN_MIN_CEILING: i64 = 3_000_000;
/// 全量整理里失败的个数达到「预计个数的五分之一（至少 25 个）」就自动结束：失败的调用多半已经计费（输出被截断、校验不过），
/// 失败这么多说明模型 / 提示词 / 服务商哪里不对，继续整理只是在白花钱，先停下检查。
const FULL_SCAN_MIN_FAILURE_LIMIT: i64 = 25;
pub(crate) const DAY_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Interrupted,
    Timeout,
    RateLimit,
    Auth,
    BadJson,
    Schema,
    Network,
    ContentTooLarge,
    Other,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interrupted => "interrupted",
            Self::Timeout => "timeout",
            Self::RateLimit => "rate_limit",
            Self::Auth => "auth",
            Self::BadJson => "bad_json",
            Self::Schema => "schema",
            Self::Network => "network",
            Self::ContentTooLarge => "content_too_large",
            Self::Other => "other",
        }
    }
}

/// 一次失败的尝试，只要请求已经到了模型那边（回了坏 JSON、字段不对、空内容、超时），服务商就可能已经计费，
/// 而失败时我们读不到真实用量。为了让「每天额度」这句话是真的，按这个估计值记一笔。
/// 估计值取一张理解卡的平均用量（2026-10-04 真模型试跑：每张约 5k token，其中输入约 3k）。
pub const FAILED_ATTEMPT_TOKEN_ESTIMATE: i64 = 4000;

/// 理解卡没通过校验（坏 JSON、字段不对）一定已经调用了两次模型：第一次，加一次带着上次输出的修复调用。
/// 修复调用要把整份提示词和上一次输出再发一遍，合计约 10k–12k，只记 4000 会低估两倍多。
pub const REPAIRED_FAILURE_TOKEN_ESTIMATE: i64 = 10_000;

/// 认领一个花钱的任务时，先在这条任务自己的行上记下这么多（写进 `input_tokens`），结束时换成真实用量。
/// 这样额度用量里一直看得见在跑的任务，取消、崩溃、被打断都不会让这笔账消失，
/// 几个线程同时认领也不会一起冲破额度（认领在一个 `BEGIN IMMEDIATE` 里，后一个一定看得见前一个的预留）。
/// 取一次调用的上限：提示词约 3–4.5k + 输出额度 8192（`card_pipeline::CARD_MAX_OUTPUT_TOKENS`）≈ 12k；一张卡平均约 6k。
/// 被截断重试 / 校验没过再补一次调用时会超过它（最多约再多一次调用），这是已知的小误差，不是额度被冲破：
/// 超出的部分照实记账，下一次认领会看见。
pub const CLAIM_RESERVE_TOKENS: i64 = 12_000;

/// 这一次失败该记多少。网络不通（没连上）、限流、认证失败、内容太大（本地就拦了）、被打断没有真的花 token，
/// 其余说明请求已经到了模型那边。
fn failure_charge(kind: ErrorKind) -> i64 {
    match kind {
        ErrorKind::BadJson | ErrorKind::Schema => REPAIRED_FAILURE_TOKEN_ESTIMATE,
        ErrorKind::Timeout | ErrorKind::Other => FAILED_ATTEMPT_TOKEN_ESTIMATE,
        ErrorKind::Interrupted
        | ErrorKind::RateLimit
        | ErrorKind::Auth
        | ErrorKind::Network
        | ErrorKind::ContentTooLarge => 0,
    }
}

/// 被打断（崩溃、强退、重新编译）的任务回到队列后，隔这么久才会被再次认领，免得一个一直崩的任务马上又花一次。
const INTERRUPTED_RETRY_DELAY_SECS: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRecord {
    pub id: String,
    pub account_id: String,
    pub job_type: String,
    pub target_type: String,
    pub target_id: String,
    pub attempts: i64,
    pub max_attempts: i64,
    pub input_hash: Option<String>,
    /// 认领时记在这条任务上的预留（`CLAIM_RESERVE_TOKENS`；不花钱的任务是 0）。结束时用它换成真实用量。
    pub reserved: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobFailure {
    /// 还有次数就按延迟重新排队，用尽次数则失败。
    Retryable {
        kind: ErrorKind,
        message: String,
        retry_after_secs: Option<u64>,
    },
    /// 直接失败，不再重试。
    Fatal { kind: ErrorKind, message: String },
    /// 整条队列停下。这个任务回到 queued，且本次不计入 attempts。
    PauseQueue { kind: ErrorKind, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSuccess {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub provider: String,
    pub model: String,
    pub prompt_version: String,
    pub input_hash: String,
    pub skipped: bool,
}

pub trait JobHandler: Send + Sync {
    fn job_type(&self) -> &'static str;
    fn spends_tokens(&self) -> bool;
    fn run(&self, job: &JobRecord, db_path: &Path) -> Result<JobSuccess, JobFailure>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimCandidate {
    pub job_type: String,
    pub spends_tokens: bool,
}

/// 认领时用的预算门闩。用量不在这里，认领时从库里现算。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetGate {
    pub paused: bool,
    pub daily_budget_tokens: u64,
    /// 全量整理进行中：花钱的任务不看每天额度（暂停照样拦）。
    pub unlimited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueueControl {
    pub paused: bool,
    pub daily_budget_tokens: u64,
    pub pause_reason: Option<PauseReason>,
    /// 全量整理正在进行（Fox 亲手确认过）。没有这一项就按每天额度。
    pub full_scan: Option<FullScan>,
    /// 上一轮全量整理怎么结束的，只用来显示。
    pub last_full_scan: Option<LastFullScan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullScan {
    pub started_at: String,
    /// 只用来显示。是否还在进行以 `started_at + FULL_SCAN_SECS` 为准，手改这一项不会让它一直不限额。
    pub until: String,
    /// 开始时要整理的个数（排队 + 进行中）。
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastFullScan {
    pub started_at: String,
    pub finished_at: String,
    pub tokens_used: i64,
    /// `completed` / `stopped` / `expired` / `ceiling`（用量远超预计）/ `failures`（失败太多）
    pub reason: String,
    pub total: i64,
    /// 结束时还剩几个没整理（排队 + 进行中）。
    pub remaining: i64,
    /// 开始以来失败了几个。
    #[serde(default)]
    pub failed: i64,
}

impl QueueControl {
    /// 全量整理现在是不是在进行。以开始时间为准（开始后 `FULL_SCAN_SECS` 内）：文件里的 `until` 只用来显示，
    /// 被手改成很远的未来也不会让它一直不限额；时钟倒退、时间读不懂都按「没在进行」算（保守）。
    pub fn full_scan_active(&self, now: &str) -> bool {
        let Some(scan) = &self.full_scan else {
            return false;
        };
        let Ok(deadline) = shift_db_time(&scan.started_at, FULL_SCAN_SECS) else {
            return false;
        };
        scan.started_at.as_str() <= now && now < deadline.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PauseReason {
    pub kind: String,
    pub message: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatus {
    /// `running` / `idle` / `paused` / `budget_reached` / `stopped`。
    pub state: String,
    pub pause_reason: Option<PauseReason>,
    pub queued: i64,
    pub running: i64,
    pub succeeded_24h: i64,
    pub failed: i64,
    pub tokens_24h: i64,
    pub daily_budget: u64,
    pub budget_remaining: u64,
    /// 全量整理进行中（不看每天额度）。
    pub unlimited: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct WorkerConfig {
    /// 同时跑几条。小于 1 按 1，大于 4 按 4。默认 2。
    pub concurrency: usize,
    /// 没有任务时隔多久再看一眼。传 0 会改用默认的 2 秒，避免空转。
    pub idle_poll: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            concurrency: 2,
            idle_poll: Duration::from_secs(2),
        }
    }
}

/// 测试要看线程有没有停。`cargo check` 不编译测试，所以单独放开。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerStatus {
    pub stopped: bool,
    pub inflight: usize,
}

struct WorkerInner {
    stop: AtomicBool,
    recovered: AtomicBool,
    inflight: AtomicUsize,
    recover_lock: Mutex<()>,
    wake_lock: Mutex<()>,
    wake: Condvar,
}

pub struct Worker {
    inner: Arc<WorkerInner>,
    /// `JoinHandle` 不是 `Sync`。放进锁里，工作线程才能作为 Tauri 托管状态留下来。
    joins: Mutex<Vec<JoinHandle<()>>>,
}

pub fn queue_control_path(database_path: &Path) -> PathBuf {
    database_path.with_file_name("fox-stars-lab.queue.json")
}

pub fn read_control(path: &Path) -> QueueControl {
    match load_object(path) {
        Ok(map) => control_from_map(&map),
        Err(_) => QueueControl::default(),
    }
}

/// 只改 `paused`。恢复（`paused == false`）时清掉 `pauseReason`，其它字段原样保留。
pub fn set_paused(path: &Path, paused: bool) -> Result<(), String> {
    with_control_file(path, |map| {
        map.insert("paused".to_owned(), Value::Bool(paused));
        if !paused {
            map.remove("pauseReason");
        }
        Ok(())
    })
}

/// 允许 0。超过 2_000_000 的值会被压到上限，并返回实际写入的数字。没有 1000 的下限。
pub fn set_daily_budget(path: &Path, tokens: u64) -> Result<u64, String> {
    let tokens = tokens.min(MAX_DAILY_BUDGET);
    with_control_file(path, |map| {
        map.insert("dailyBudgetTokens".to_owned(), Value::from(tokens));
        Ok(tokens)
    })
}

/// 同一个优先级里谁先做（只对仓库类任务有区别）：先做打了业务场景标签的，再做没打标签的；
/// 同一档里最近收藏的先做，最后才按入队顺序。预算一天只够整理十几个仓库，先花在最可能用得上的那些上。
/// 被拒绝的场景标签不算数。别的任务没有对应的仓库，这两项都是常量，不改变原来的顺序。
/// 优先级 5 和 6 都是「后台补课」（旧失败项 6、手动补缺 6、老入口 5），认领时合成一档再按上面的规矩排；
/// 0–4 是用户点的重做、新收藏这类，照旧排在前面。
const ORDER_WITHIN_PRIORITY: &str = "CASE WHEN ai_jobs.target_type = 'repository' AND EXISTS (
        SELECT 1 FROM entity_scenarios es
        WHERE es.entity_type = 'repository' AND es.entity_id = ai_jobs.target_id AND es.status != 'rejected'
    ) THEN 0 ELSE 1 END ASC,
    IFNULL((SELECT r.starred_at FROM repositories r WHERE ai_jobs.target_type = 'repository' AND r.id = ai_jobs.target_id), '') DESC,
    ai_jobs.created_at ASC, ai_jobs.id ASC";

/// 在一个 `BEGIN IMMEDIATE` 里认领下一条任务，两个连接不会领到同一条。
///
/// 暂停时一条都不领，包括不花钱的任务。花钱的任务还要预算大于 0，且最近 24 小时
/// 已结束任务的 token 之和小于预算。窗口含正好 24 小时前的那一刻（`finished_at >= now-24h`），
/// 更早的不计入。
pub fn claim_next(
    conn: &mut Connection,
    now: &str,
    handled_types: &[ClaimCandidate],
    budget_gate: &BudgetGate,
) -> Result<Option<JobRecord>, String> {
    if budget_gate.paused {
        return Ok(None);
    }
    let cutoff = shift_db_time(now, -DAY_SECS)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("任务认领失败：{error}"))?;
    // 在跑任务的预留已经写在它们自己的行里，算在 used 里；再加上这一条自己的预留也放得下才认领。
    let used = tokens_used_since(&tx, &cutoff)?.max(0);
    let spending_open = budget_gate.unlimited
        || (budget_gate.daily_budget_tokens > 0
            && (used as u64).saturating_add(CLAIM_RESERVE_TOKENS as u64)
                <= budget_gate.daily_budget_tokens);
    let mut eligible: Vec<&str> = Vec::new();
    for candidate in handled_types {
        if candidate.job_type.is_empty() {
            continue;
        }
        if candidate.spends_tokens && !spending_open {
            continue;
        }
        if !eligible.contains(&candidate.job_type.as_str()) {
            eligible.push(candidate.job_type.as_str());
        }
    }
    if eligible.is_empty() {
        tx.commit()
            .map_err(|error| format!("任务认领失败：{error}"))?;
        return Ok(None);
    }

    let mut sql = String::from(
        "SELECT id, account_id, job_type, target_type, target_id, attempts, max_attempts, input_hash
         FROM ai_jobs
         WHERE status = 'queued'
           AND (not_before IS NULL OR not_before <= ?)
           AND job_type IN (",
    );
    sql.push_str(
        &std::iter::repeat("?")
            .take(eligible.len())
            .collect::<Vec<_>>()
            .join(", "),
    );
    sql.push_str(") ORDER BY MIN(ai_jobs.priority, 5) ASC, ");
    sql.push_str(ORDER_WITHIN_PRIORITY);
    sql.push_str(" LIMIT 1");

    let mut bound: Vec<String> = Vec::with_capacity(1 + eligible.len());
    bound.push(now.to_owned());
    for job_type in &eligible {
        bound.push((*job_type).to_owned());
    }
    let selected = {
        let mut statement = tx
            .prepare(&sql)
            .map_err(|error| format!("任务认领失败：{error}"))?;
        statement
            .query_row(params_from_iter(bound.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })
            .optional()
            .map_err(|error| format!("任务认领失败：{error}"))?
    };
    let Some((
        id,
        account_id,
        job_type,
        target_type,
        target_id,
        attempts,
        max_attempts,
        input_hash,
    )) = selected
    else {
        tx.commit()
            .map_err(|error| format!("任务认领失败：{error}"))?;
        return Ok(None);
    };
    let reserved = if handled_types
        .iter()
        .any(|candidate| candidate.spends_tokens && candidate.job_type == job_type)
    {
        CLAIM_RESERVE_TOKENS
    } else {
        0
    };
    // 认领会把这条任务的 updated_at 改成现在：如果它身上还留着 24 小时以前的失败用量，
    // 这些旧账会被「重新记到今天」，白白占掉当天的额度（队列里本来就有一批积压的、试跑留下用量的任务）。
    // 所以先让过了 24 小时的旧账过期（归零），再记这次的预留。
    let changed = tx
        .execute(
            "UPDATE ai_jobs
             SET status = 'running', attempts = attempts + 1, started_at = ?1,
                 input_tokens = (CASE WHEN updated_at >= ?4 THEN input_tokens ELSE 0 END) + ?3,
                 output_tokens = CASE WHEN updated_at >= ?4 THEN output_tokens ELSE 0 END,
                 updated_at = ?1
             WHERE id = ?2 AND status = 'queued'",
            params![now, id, reserved, cutoff],
        )
        .map_err(|error| format!("任务认领失败：{error}"))?;
    if changed == 0 {
        tx.commit()
            .map_err(|error| format!("任务认领失败：{error}"))?;
        return Ok(None);
    }
    tx.commit()
        .map_err(|error| format!("任务认领失败：{error}"))?;
    Ok(Some(JobRecord {
        id,
        account_id,
        job_type,
        target_type,
        target_id,
        attempts: attempts + 1,
        max_attempts,
        input_hash,
        reserved,
    }))
}

/// 把处理器的结果写回。更新都带 `status = 'running'`：跑的过程中被取消的任务不会被盖掉。
/// 返回 `false` 表示这条任务已经不在 running（通常是被取消了）。
///
/// `jitter` 取 -1.0 到 1.0，只作用在默认退避上，表示最多 ±10%。`retry_after_secs` 不加抖动，
/// 只夹到 5 到 3600 秒。工作线程传 0。
pub fn finish_job(
    db_path: &Path,
    job: &JobRecord,
    now: &str,
    result: Result<JobSuccess, JobFailure>,
    jitter: f64,
) -> Result<bool, String> {
    let _ = parse_db_time(now)?;
    match result {
        Ok(success) => write_success(db_path, &job.id, now, &success, job.reserved),
        Err(JobFailure::PauseQueue { kind, message }) => {
            pause_queue(db_path, &job.id, now, kind, message, job.reserved)
        }
        Err(JobFailure::Fatal { kind, message }) => {
            write_failure(db_path, &job.id, now, kind, &message, job.reserved)
        }
        Err(JobFailure::Retryable {
            kind,
            message,
            retry_after_secs,
        }) => {
            if job.attempts >= job.max_attempts {
                return write_failure(db_path, &job.id, now, kind, &message, job.reserved);
            }
            let delay = retry_delay_secs(job.attempts, retry_after_secs, jitter);
            let delay_i64 = i64::try_from(delay).map_err(|_| "延迟太长")?;
            let not_before = shift_db_time(now, delay_i64)?;
            requeue_job(
                db_path,
                &job.id,
                now,
                kind,
                &message,
                &not_before,
                job.reserved,
            )
        }
    }
}

fn pause_queue(
    db_path: &Path,
    id: &str,
    now: &str,
    kind: ErrorKind,
    message: String,
    reserved: i64,
) -> Result<bool, String> {
    // 先把队列停住，再把任务放回去，避免别的线程在窗口里又领走它。
    let reason = PauseReason {
        kind: kind.as_str().to_owned(),
        message,
        at: now.to_owned(),
    };
    set_pause_reason(&queue_control_path(db_path), &reason)?;
    let conn = open_db(db_path)?;
    let changed = conn
        .execute(
            "UPDATE ai_jobs
             SET status = 'queued',
                 attempts = CASE WHEN attempts > 0 THEN attempts - 1 ELSE 0 END,
                 started_at = NULL,
                 not_before = NULL,
                 input_tokens = MAX(input_tokens - ?3, 0),
                 updated_at = ?1
             WHERE id = ?2 AND status = 'running'",
            params![now, id, reserved],
        )
        .map_err(|error| format!("任务收尾失败：{error}"))?;
    Ok(changed > 0)
}

/// 失败的尝试也记用量：把认领时记下的预留换成这次失败该记的用量（可能是 0），累加在这条任务上，
/// 后面重试成功时不会被盖掉。和状态变化在同一条 UPDATE 里，只对仍在 running 的任务生效：
/// 跑的过程中被取消的任务保留预留不退，因为那次请求发出去了、收不回来。
fn requeue_job(
    db_path: &Path,
    id: &str,
    now: &str,
    kind: ErrorKind,
    message: &str,
    not_before: &str,
    reserved: i64,
) -> Result<bool, String> {
    let conn = open_db(db_path)?;
    let changed = conn
        .execute(
            "UPDATE ai_jobs
             SET status = 'queued',
                 error_kind = ?1,
                 error_message = ?2,
                 not_before = ?3,
                 started_at = NULL,
                 finished_at = NULL,
                 input_tokens = MAX(input_tokens - ?6, 0) + ?7,
                 updated_at = ?4
             WHERE id = ?5 AND status = 'running'",
            params![
                kind.as_str(),
                message,
                not_before,
                now,
                id,
                reserved,
                failure_charge(kind)
            ],
        )
        .map_err(|error| format!("任务收尾失败：{error}"))?;
    Ok(changed > 0)
}

pub fn queue_status(db_path: &Path) -> Result<QueueStatus, String> {
    let control = read_control(&queue_control_path(db_path));
    let conn = open_db(db_path)?;
    let cutoff = shift_db_time(&now_db_time(), -DAY_SECS)?;
    let queued = count_status(&conn, "queued")?;
    let running = count_status(&conn, "running")?;
    let failed = count_status(&conn, "failed")?;
    let succeeded_24h: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM ai_jobs WHERE status = 'succeeded' AND finished_at IS NOT NULL AND finished_at >= ?1",
            [&cutoff],
            |row| row.get(0),
        )
        .map_err(|error| format!("任务计数失败：{error}"))?;
    let tokens_24h = tokens_used_since(&conn, &cutoff)?.max(0);
    let unlimited = control.full_scan_active(&now_db_time());
    let state = queue_state(
        control.paused,
        control.pause_reason.is_some(),
        running,
        control.daily_budget_tokens,
        tokens_24h,
        unlimited,
    );
    Ok(QueueStatus {
        state: state.to_owned(),
        pause_reason: control.pause_reason,
        queued,
        running,
        succeeded_24h,
        failed,
        tokens_24h,
        daily_budget: control.daily_budget_tokens,
        budget_remaining: budget_remaining(control.daily_budget_tokens, tokens_24h),
        unlimited,
    })
}

impl Worker {
    pub fn start(
        db_path: PathBuf,
        handlers: Vec<Arc<dyn JobHandler>>,
        config: WorkerConfig,
    ) -> Self {
        let concurrency = config.concurrency.clamp(1, 4);
        let idle_poll = if config.idle_poll.is_zero() {
            Duration::from_secs(2)
        } else {
            config.idle_poll
        };
        let recovered = match open_db(&db_path) {
            Ok(conn) => recover_running_jobs(&conn, &now_db_time()).is_ok(),
            Err(_) => false,
        };
        let inner = Arc::new(WorkerInner {
            stop: AtomicBool::new(false),
            recovered: AtomicBool::new(recovered),
            inflight: AtomicUsize::new(0),
            recover_lock: Mutex::new(()),
            wake_lock: Mutex::new(()),
            wake: Condvar::new(),
        });
        let handlers = Arc::new(handlers);
        let mut joins = Vec::with_capacity(concurrency);
        for index in 0..concurrency {
            let db_path = db_path.clone();
            let handlers = Arc::clone(&handlers);
            let inner = Arc::clone(&inner);
            match thread::Builder::new()
                .name(format!("ai-job-{index}"))
                .spawn(move || worker_loop(db_path, handlers, idle_poll, inner))
            {
                Ok(handle) => joins.push(handle),
                Err(_) => break,
            }
        }
        Self {
            inner,
            joins: Mutex::new(joins),
        }
    }

    /// 不再认领新任务，并等正在跑的任务自己结束。处理器的网络超时由处理器自己管。
    /// 应用退出走 `Drop`。这个方法给测试主动停机，`cargo check` 看不见测试调用。
    #[allow(dead_code)]
    pub fn stop(self) {
        self.signal_and_join();
    }

    /// 测试要确认线程已经起来。应用本身不读这个状态。
    #[allow(dead_code)]
    pub fn status(&self) -> WorkerStatus {
        WorkerStatus {
            stopped: self.inner.stop.load(Ordering::SeqCst),
            inflight: self.inner.inflight.load(Ordering::SeqCst),
        }
    }

    fn signal_and_join(&self) {
        self.inner.stop.store(true, Ordering::SeqCst);
        self.inner.wake.notify_all();
        let mut joins = self.joins.lock().unwrap_or_else(|error| error.into_inner());
        for handle in joins.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.signal_and_join();
    }
}

/// 用调用方给的处理器启动工作线程。预算仍只看控制文件，默认是 0，这里不写预算、也不调用处理器。
pub fn start_worker(db_path: &Path, handlers: Vec<Arc<dyn JobHandler>>) -> Worker {
    Worker::start(db_path.to_path_buf(), handlers, WorkerConfig::default())
}

/// 上次没跑完的任务回到队列。认领时记在它们行上的预留原样保留：那次请求可能已经发出去、花了钱，
/// 不能因为程序被关掉、重新编译或崩溃就当没发生过。隔一会儿才再认领，免得一直崩的任务立刻又花一次。
/// `updated_at` 不改：那次请求发生在认领的时候，预留按那个时间进 24 小时窗口。改成现在，
/// 几天前就停在 running 的旧行会把预留「重新记到今天」。
fn recover_running_jobs(conn: &Connection, now: &str) -> Result<usize, String> {
    let not_before = shift_db_time(now, INTERRUPTED_RETRY_DELAY_SECS)?;
    // 两条 UPDATE 在一个事务里，中间不会有别的连接插进一条 running 的预留行
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("中断任务恢复失败：{error}"))?;
    tx.execute(
        "UPDATE ai_jobs SET status = 'cancelled', finished_at = updated_at
         WHERE status = 'running' AND target_id LIKE 'legacy-%'",
        [],
    )
    .map_err(|error| format!("中断任务恢复失败：{error}"))?;
    // 旧功能的预留行绝不放回队列（没有处理器认领，会变成永远「排队中」的幽灵行）
    let changed = tx
        .execute(
            "UPDATE ai_jobs
             SET status = 'queued', error_kind = ?1, started_at = NULL, not_before = ?2
             WHERE status = 'running' AND target_id NOT LIKE 'legacy-%'",
            params![ErrorKind::Interrupted.as_str(), not_before],
        )
        .map_err(|error| format!("中断任务恢复失败：{error}"))?;
    tx.commit()
        .map_err(|error| format!("中断任务恢复失败：{error}"))?;
    Ok(changed)
}

fn worker_loop(
    db_path: PathBuf,
    handlers: Arc<Vec<Arc<dyn JobHandler>>>,
    idle_poll: Duration,
    inner: Arc<WorkerInner>,
) {
    while !inner.stop.load(Ordering::SeqCst) {
        if !ensure_recovered(&db_path, &inner) {
            if inner.stop.load(Ordering::SeqCst) {
                break;
            }
            wait_idle(&inner, idle_poll);
            continue;
        }
        if inner.stop.load(Ordering::SeqCst) {
            break;
        }
        match claim_for_worker(&db_path, handlers.as_slice()) {
            Ok(Some(job)) => {
                inner.inflight.fetch_add(1, Ordering::SeqCst);
                let outcome = run_handler(handlers.as_slice(), &job, &db_path);
                let now = now_db_time();
                // 抖动固定为 0：重试时间可预期。测试要覆盖抖动时，直接把固定值传给 finish_job。
                // 收尾失败（数据库忙超过 5 秒）就再试两次：每条路径都是一条 UPDATE，重试不会重复记账；
                // 三次都不行，这条任务留在 running，它的预留照旧占着额度，下次启动再恢复。
                for attempt in 0..3 {
                    match finish_job(&db_path, &job, &now, outcome.clone(), 0.0) {
                        Ok(_) => break,
                        Err(_) if attempt < 2 => thread::sleep(Duration::from_millis(300)),
                        Err(_) => break,
                    }
                }
                inner.inflight.fetch_sub(1, Ordering::SeqCst);
            }
            Ok(None) | Err(_) => {
                if inner.stop.load(Ordering::SeqCst) {
                    break;
                }
                wait_idle(&inner, idle_poll);
            }
        }
    }
}

fn ensure_recovered(db_path: &Path, inner: &WorkerInner) -> bool {
    if inner.recovered.load(Ordering::SeqCst) {
        return true;
    }
    let _guard = inner
        .recover_lock
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if inner.recovered.load(Ordering::SeqCst) {
        return true;
    }
    let Ok(conn) = open_db(db_path) else {
        return false;
    };
    if recover_running_jobs(&conn, &now_db_time()).is_ok() {
        inner.recovered.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

fn wait_idle(inner: &WorkerInner, idle_poll: Duration) {
    let guard = inner
        .wake_lock
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if inner.stop.load(Ordering::SeqCst) {
        return;
    }
    let _ = inner.wake.wait_timeout(guard, idle_poll);
}

fn claim_for_worker(
    db_path: &Path,
    handlers: &[Arc<dyn JobHandler>],
) -> Result<Option<JobRecord>, String> {
    let control_path = queue_control_path(db_path);
    let now = now_db_time();
    let mut conn = open_db(db_path)?;
    // 全量整理该收尾就先收尾（整理完了 / 到时间 / 用量远超预计），再按收尾后的控制文件认领
    settle_full_scan(&conn, &control_path, &now)?;
    let control = read_control(&control_path);
    let gate = BudgetGate {
        paused: control.paused,
        daily_budget_tokens: control.daily_budget_tokens,
        unlimited: control.full_scan_active(&now),
    };
    let handled = handlers
        .iter()
        .map(|handler| ClaimCandidate {
            job_type: handler.job_type().to_owned(),
            spends_tokens: handler.spends_tokens(),
        })
        .collect::<Vec<_>>();
    claim_next(&mut conn, &now, &handled, &gate)
}

fn run_handler(
    handlers: &[Arc<dyn JobHandler>],
    job: &JobRecord,
    db_path: &Path,
) -> Result<JobSuccess, JobFailure> {
    let Some(handler) = handlers.iter().find(|item| item.job_type() == job.job_type) else {
        return Err(JobFailure::Fatal {
            kind: ErrorKind::Other,
            message: "没有找到任务处理器".to_owned(),
        });
    };
    match std::panic::catch_unwind(AssertUnwindSafe(|| handler.run(job, db_path))) {
        Ok(outcome) => outcome,
        Err(payload) => Err(JobFailure::Fatal {
            kind: ErrorKind::Other,
            message: panic_message(payload),
        }),
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "未知错误".to_owned());
    format!("任务内部出错：{detail}")
}

fn write_success(
    db_path: &Path,
    id: &str,
    now: &str,
    success: &JobSuccess,
    reserved: i64,
) -> Result<bool, String> {
    let input_tokens = i64::try_from(success.input_tokens).map_err(|_| "token 数量超出范围")?;
    let output_tokens = i64::try_from(success.output_tokens).map_err(|_| "token 数量超出范围")?;
    let status = if success.skipped {
        "skipped"
    } else {
        "succeeded"
    };
    let conn = open_db(db_path)?;
    let changed = conn
        .execute(
            "UPDATE ai_jobs
             SET status = ?1,
                 input_tokens = MAX(input_tokens - ?10, 0) + ?2,
                 output_tokens = output_tokens + ?3,
                 provider = ?4,
                 model = ?5,
                 prompt_version = ?6,
                 input_hash = ?7,
                 finished_at = ?8,
                 error_kind = NULL,
                 error_message = NULL,
                 updated_at = ?8
             WHERE id = ?9 AND status = 'running'",
            params![
                status,
                input_tokens,
                output_tokens,
                success.provider,
                success.model,
                success.prompt_version,
                success.input_hash,
                now,
                id,
                reserved,
            ],
        )
        .map_err(|error| format!("任务收尾失败：{error}"))?;
    Ok(changed > 0)
}

fn write_failure(
    db_path: &Path,
    id: &str,
    now: &str,
    kind: ErrorKind,
    message: &str,
    reserved: i64,
) -> Result<bool, String> {
    let conn = open_db(db_path)?;
    let changed = conn
        .execute(
            "UPDATE ai_jobs
             SET status = 'failed',
                 error_kind = ?1,
                 error_message = ?2,
                 finished_at = ?3,
                 input_tokens = MAX(input_tokens - ?5, 0) + ?6,
                 updated_at = ?3
             WHERE id = ?4 AND status = 'running'",
            params![
                kind.as_str(),
                message,
                now,
                id,
                reserved,
                failure_charge(kind)
            ],
        )
        .map_err(|error| format!("任务收尾失败：{error}"))?;
    Ok(changed > 0)
}

fn retry_delay_secs(attempt: i64, retry_after_secs: Option<u64>, jitter: f64) -> u64 {
    if let Some(given) = retry_after_secs {
        return given.clamp(5, 3600);
    }
    let base: u64 = if attempt <= 1 {
        30
    } else if attempt == 2 {
        120
    } else {
        600
    };
    let factor = 1.0 + 0.1 * jitter.clamp(-1.0, 1.0);
    let scaled = (base as f64) * factor;
    if !scaled.is_finite() {
        return base;
    }
    scaled.round().max(0.0) as u64
}

pub(crate) fn tokens_used_since(conn: &Connection, cutoff: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(SUM(input_tokens + output_tokens), 0)
         FROM ai_jobs
         WHERE (status IN ('succeeded', 'skipped', 'failed', 'cancelled')
                AND finished_at IS NOT NULL
                AND finished_at >= ?1)
            OR (status IN ('queued', 'running')
                AND input_tokens + output_tokens > 0
                AND updated_at >= ?1)",
        [cutoff],
        |row| row.get(0),
    )
    .map_err(|error| format!("用量读取失败：{error}"))
}

/// 队列里某个状态的任务数。旧功能（`spend_guard`）的账本行不算：它们不是队列任务，
/// 数进去，一次 AI 搜索在途时界面就会说「正在后台整理」「进行中 1」。用量（`tokens_used_since`）照旧含它们。
/// 全量整理的刹车线：用量超过它就自动结束。按开始时要整理的个数估（每张约 5k，乘 4），至少 300 万。
pub(crate) fn full_scan_ceiling(total: i64) -> i64 {
    total
        .max(0)
        .saturating_mul(FULL_SCAN_TOKENS_PER_CARD)
        .saturating_mul(FULL_SCAN_CEILING_FACTOR)
        .max(FULL_SCAN_MIN_CEILING)
}

/// 全量整理的失败刹车线：开始以来失败的个数达到它就自动结束（见 `FULL_SCAN_MIN_FAILURE_LIMIT`）。
pub(crate) fn full_scan_failure_limit(total: i64) -> i64 {
    (total.max(0) / 5).max(FULL_SCAN_MIN_FAILURE_LIMIT)
}

/// 还没整理完的理解卡任务数（排队 + 进行中）。
pub(crate) fn count_open_cards(conn: &Connection) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM ai_jobs WHERE job_type = 'repo_card' AND status IN ('queued', 'running')",
        [],
        |row| row.get(0),
    )
    .map_err(|error| format!("任务计数失败：{error}"))
}

/// 从 `since` 起失败了几个理解卡任务。
pub(crate) fn count_failed_cards_since(conn: &Connection, since: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM ai_jobs
         WHERE job_type = 'repo_card' AND status = 'failed' AND finished_at IS NOT NULL AND finished_at >= ?1",
        [since],
        |row| row.get(0),
    )
    .map_err(|error| format!("任务计数失败：{error}"))
}

/// 现在能不能不看每天额度：全量整理进行中，且用量没超过刹车线。只读，不收尾；读不出用量就当不能（保守）。
pub(crate) fn full_scan_open(conn: &Connection, control: &QueueControl, now: &str) -> bool {
    if !control.full_scan_active(now) {
        return false;
    }
    let Some(scan) = &control.full_scan else {
        return false;
    };
    match tokens_used_since(conn, &scan.started_at) {
        Ok(used) => used <= full_scan_ceiling(scan.total),
        Err(_) => false,
    }
}

/// 全量整理该收尾就收尾：用量超过刹车线、整理完了、或者到时间了。没有在进行就什么也不做。
/// 每次认领前和读状态时都可以调；多个线程同时调也只会收尾一次。
pub(crate) fn settle_full_scan(
    conn: &Connection,
    control_path: &Path,
    now: &str,
) -> Result<(), String> {
    let control = read_control(control_path);
    let Some(scan) = control.full_scan.clone() else {
        return Ok(());
    };
    let used = tokens_used_since(conn, &scan.started_at)?.max(0);
    let remaining = count_open_cards(conn)?;
    let failed = count_failed_cards_since(conn, &scan.started_at)?;
    // 到时间：开始后满 48 小时（开始时间读不懂也算到时间）。时钟倒退（现在比开始时间还早）不算——
    // 那只是暂时不满足「在进行」，等时钟走回来再说，不能把刚开始的全量整理永久收尾
    let expired = match shift_db_time(&scan.started_at, FULL_SCAN_SECS) {
        Ok(deadline) => now >= deadline.as_str(),
        Err(_) => true,
    };
    let reason = if used > full_scan_ceiling(scan.total) {
        "ceiling"
    } else if remaining == 0 {
        "completed"
    } else if failed >= full_scan_failure_limit(scan.total) {
        "failures"
    } else if expired {
        "expired"
    } else {
        return Ok(());
    };
    finish_full_scan(control_path, &scan, reason, used, remaining, failed, now)
}

/// 停止全量整理（Fox 点了停止）。没有在进行就什么也不做。已经在跑的任务会做完，之后按每天额度。
pub(crate) fn stop_full_scan(
    conn: &Connection,
    control_path: &Path,
    now: &str,
) -> Result<(), String> {
    let control = read_control(control_path);
    let Some(scan) = control.full_scan.clone() else {
        return Ok(());
    };
    let used = tokens_used_since(conn, &scan.started_at)?.max(0);
    let remaining = count_open_cards(conn)?;
    let failed = count_failed_cards_since(conn, &scan.started_at)?;
    finish_full_scan(control_path, &scan, "stopped", used, remaining, failed, now)
}

fn finish_full_scan(
    path: &Path,
    scan: &FullScan,
    reason: &str,
    used: i64,
    remaining: i64,
    failed: i64,
    now: &str,
) -> Result<(), String> {
    let record = serde_json::to_value(LastFullScan {
        started_at: scan.started_at.clone(),
        finished_at: now.to_owned(),
        tokens_used: used,
        reason: reason.to_owned(),
        total: scan.total,
        remaining,
        failed,
    })
    .map_err(|error| format!("全量整理记录序列化失败：{error}"))?;
    with_control_file(path, |map| {
        // 别的线程已经收尾了（或者又开了新的一轮）：什么也不做
        let current = map
            .get("fullScan")
            .and_then(|value| value.get("startedAt"))
            .and_then(Value::as_str);
        if current != Some(scan.started_at.as_str()) {
            return Ok(());
        }
        map.remove("fullScan");
        map.insert("lastFullScan".to_owned(), record.clone());
        Ok(())
    })
}

/// 换了一个数据库（恢复备份、清空本地库）：用量记录跟着库变了，控制文件里的授权要一并归零——
/// 日常额度回到 0（要继续花钱得在工具箱里重新开启），全量整理也一并结束（旧库的用量对不上，「已用」和刹车线都会重置；
/// 记录也对不上新库，不留）。别的键（令牌等）原样保留。
pub fn reset_spending_authorization(path: &Path) -> Result<(), String> {
    with_control_file(path, |map| {
        map.insert("dailyBudgetTokens".to_owned(), Value::from(0_u64));
        map.remove("fullScan");
        map.remove("lastFullScan");
        Ok(())
    })
}

/// 开始全量整理。已经在进行就原样返回（不重置开始时间）。同时恢复暂停：Fox 点了开始，就是要它跑。
pub(crate) fn begin_full_scan(path: &Path, now: &str, total: i64) -> Result<FullScan, String> {
    let existing = read_control(path);
    if existing.full_scan_active(now) {
        if let Some(scan) = existing.full_scan {
            return Ok(scan);
        }
    }
    let scan = FullScan {
        started_at: now.to_owned(),
        until: shift_db_time(now, FULL_SCAN_SECS)?,
        total,
    };
    let value =
        serde_json::to_value(&scan).map_err(|error| format!("全量整理记录序列化失败：{error}"))?;
    with_control_file(path, |map| {
        map.insert("fullScan".to_owned(), value.clone());
        map.insert("paused".to_owned(), Value::Bool(false));
        map.remove("pauseReason");
        Ok(())
    })?;
    Ok(scan)
}

fn count_status(conn: &Connection, status: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM ai_jobs WHERE status = ?1 AND target_id NOT LIKE 'legacy-%'",
        [status],
        |row| row.get(0),
    )
    .map_err(|error| format!("任务计数失败：{error}"))
}

/// 24 小时前的时刻（和库里的时间同一个格式）。过了这个时刻的用量不再计入。
pub(crate) fn day_ago_db_time() -> String {
    shift_db_time(&now_db_time(), -DAY_SECS).unwrap_or_default()
}

/// 处理器在失败前已经确定花掉的用量（比如第一次调用成功、紧接着的修复调用失败）：直接记到这条在跑的任务上。
/// 之后结束时 `MAX(input_tokens - 预留, 0) + 失败该记的` 会把它保留下来。
pub(crate) fn add_charge(db_path: &Path, id: &str, tokens: u64) -> Result<(), String> {
    let tokens = i64::try_from(tokens.min(5_000_000)).unwrap_or(0);
    let conn = open_db(db_path)?;
    conn.execute(
        "UPDATE ai_jobs SET input_tokens = input_tokens + ?1 WHERE id = ?2 AND status = 'running'",
        params![tokens, id],
    )
    .map_err(|error| format!("用量记录失败：{error}"))?;
    Ok(())
}

/// 最近 24 小时已用的 token。和队列认领、状态显示是同一个口径（含在跑任务的预留、已取消的任务）。
/// 只给测试用：生产代码在自己的事务里直接调 `tokens_used_since`。
#[cfg(test)]
pub(crate) fn tokens_used_in_last_day(db_path: &Path, now: &str) -> Result<i64, String> {
    let conn = open_db(db_path)?;
    let cutoff = shift_db_time(now, -DAY_SECS)?;
    tokens_used_since(&conn, &cutoff)
}

/// 预算为 0 表示还没授权花钱，不是额度用完。
/// `stopped` 是系统停的（写了 pauseReason，例如认证失败）；`paused` 是用户暂停。
fn queue_state(
    paused: bool,
    has_reason: bool,
    running: i64,
    budget: u64,
    tokens_24h: i64,
    unlimited: bool,
) -> &'static str {
    if paused && has_reason {
        "stopped"
    } else if paused {
        "paused"
    } else if running > 0 {
        "running"
    } else if !unlimited
        && budget > 0
        && tokens_24h.saturating_add(CLAIM_RESERVE_TOKENS)
            > i64::try_from(budget).unwrap_or(i64::MAX)
    {
        // 剩下的额度放不下再认领一条任务的预留，就等于用完了：队列不会再领，界面也该这么说
        "budget_reached"
    } else {
        "idle"
    }
}

/// 还能用多少。剩下的不够认领一条任务的预留（`CLAIM_RESERVE_TOKENS`）就当 0：
/// 队列不会再领，界面也不该说「还能整理几个」。
fn budget_remaining(budget: u64, tokens_24h: i64) -> u64 {
    let Ok(budget_i64) = i64::try_from(budget) else {
        return 0;
    };
    if budget_i64 == 0 || tokens_24h >= budget_i64 {
        return 0;
    }
    let remaining = (budget_i64 - tokens_24h.max(0)) as u64;
    if remaining < CLAIM_RESERVE_TOKENS as u64 {
        0
    } else {
        remaining
    }
}

fn set_pause_reason(path: &Path, reason: &PauseReason) -> Result<(), String> {
    let value =
        serde_json::to_value(reason).map_err(|error| format!("暂停原因序列化失败：{error}"))?;
    with_control_file(path, |map| {
        map.insert("paused".to_owned(), Value::Bool(true));
        map.insert("pauseReason".to_owned(), value.clone());
        Ok(())
    })
}

fn with_control_file<T>(
    path: &Path,
    update: impl FnOnce(&mut Map<String, Value>) -> Result<T, String>,
) -> Result<T, String> {
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut map = load_object(path)?;
    let result = update(&mut map)?;
    write_object(path, &map)?;
    Ok(result)
}

fn load_object(path: &Path) -> Result<Map<String, Value>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(format!("队列控制文件读取失败：{error}")),
    };
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("队列控制文件解析失败：{error}"))?;
    match value {
        Value::Object(map) => Ok(map),
        _ => Err("队列控制文件格式不对".to_owned()),
    }
}

fn control_from_map(map: &Map<String, Value>) -> QueueControl {
    let paused = map.get("paused").and_then(Value::as_bool).unwrap_or(false);
    let daily_budget_tokens = map
        .get("dailyBudgetTokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let pause_reason = map.get("pauseReason").and_then(|value| {
        if value.is_null() {
            None
        } else {
            serde_json::from_value(value.clone()).ok()
        }
    });
    // 读不懂的 fullScan 当作没有（保守：不会因为文件坏了就不限额）
    let full_scan = map
        .get("fullScan")
        .and_then(|value| serde_json::from_value::<FullScan>(value.clone()).ok());
    let last_full_scan = map
        .get("lastFullScan")
        .and_then(|value| serde_json::from_value::<LastFullScan>(value.clone()).ok());
    QueueControl {
        paused,
        daily_budget_tokens,
        pause_reason,
        full_scan,
        last_full_scan,
    }
}

fn write_object(path: &Path, map: &Map<String, Value>) -> Result<(), String> {
    let text = serde_json::to_string(&Value::Object(map.clone()))
        .map_err(|error| format!("队列控制文件序列化失败：{error}"))?;
    let mut tmp_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "queue.json".into());
    tmp_name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    let tmp = path.with_file_name(tmp_name);
    if let Err(error) = fs::write(&tmp, text) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("队列控制文件写入失败：{error}"));
    }
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("队列控制文件写入失败：{error}"));
    }
    Ok(())
}

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn open_db(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .busy_timeout(Duration::from_millis(5000))
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    Ok(connection)
}

pub(crate) fn now_db_time() -> String {
    format_db_time(time::OffsetDateTime::now_utc())
}

fn format_db_time(value: time::OffsetDateTime) -> String {
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

fn parse_db_time(text: &str) -> Result<time::OffsetDateTime, String> {
    let bytes = text.as_bytes();
    if bytes.len() != 24
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'.'
        || bytes[23] != b'Z'
    {
        return Err("时间格式不对".to_owned());
    }
    let year: i32 = text[0..4].parse().map_err(|_| "时间格式不对")?;
    let month_num: u8 = text[5..7].parse().map_err(|_| "时间格式不对")?;
    let day: u8 = text[8..10].parse().map_err(|_| "时间格式不对")?;
    let hour: u8 = text[11..13].parse().map_err(|_| "时间格式不对")?;
    let minute: u8 = text[14..16].parse().map_err(|_| "时间格式不对")?;
    let second: u8 = text[17..19].parse().map_err(|_| "时间格式不对")?;
    let millis: u16 = text[20..23].parse().map_err(|_| "时间格式不对")?;
    let month = time::Month::try_from(month_num).map_err(|_| "时间格式不对")?;
    let date = time::Date::from_calendar_date(year, month, day).map_err(|_| "时间格式不对")?;
    let clock =
        time::Time::from_hms_milli(hour, minute, second, millis).map_err(|_| "时间格式不对")?;
    Ok(time::PrimitiveDateTime::new(date, clock).assume_utc())
}

pub(crate) fn shift_db_time(now: &str, seconds: i64) -> Result<String, String> {
    let parsed = parse_db_time(now)?;
    let next = parsed
        .checked_add(time::Duration::seconds(seconds))
        .ok_or_else(|| "时间超出范围".to_owned())?;
    Ok(format_db_time(next))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    const NOW: &str = "2026-10-03T12:00:00.000Z";

    struct TestDb {
        path: PathBuf,
    }

    impl TestDb {
        fn new(label: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let n = SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("fsl-jobs-{label}-{nanos}-{n}"));
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join("fox-stars-lab.sqlite3");
            crate::db::migrations::migrate_database(&path).unwrap();
            let conn = open_db(&path).unwrap();
            conn.execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .unwrap();
            drop(conn);
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn conn(&self) -> Connection {
            open_db(&self.path).unwrap()
        }
    }

    impl Drop for TestDb {
        fn drop(&mut self) {
            let Some(parent) = self.path.parent() else {
                return;
            };
            let Some(name) = parent.file_name().and_then(|name| name.to_str()) else {
                return;
            };
            if name.starts_with("fsl-jobs-") {
                let _ = fs::remove_dir_all(parent);
            }
        }
    }

    struct JobSnap {
        status: String,
        attempts: i64,
        error_kind: Option<String>,
        error_message: Option<String>,
        not_before: Option<String>,
        started_at: Option<String>,
        finished_at: Option<String>,
        input_tokens: i64,
        output_tokens: i64,
        provider: Option<String>,
        model: Option<String>,
        prompt_version: Option<String>,
        input_hash: Option<String>,
    }

    fn insert_job(
        conn: &Connection,
        id: &str,
        job_type: &str,
        priority: i64,
        created_at: &str,
        not_before: Option<&str>,
        status: &str,
    ) {
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, attempts, max_attempts, created_at, not_before)
             VALUES (?1, 'acct', ?2, 'repository', ?1, ?3, ?4, 0, 3, ?5, ?6)",
            params![id, job_type, priority, status, created_at, not_before],
        )
        .unwrap();
    }

    fn insert_running(conn: &Connection, id: &str, attempts: i64, max_attempts: i64) {
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, max_attempts, error_kind, error_message, started_at, created_at)
             VALUES (?1, 'acct', 'repo_card', 'repository', ?1, 'running', ?2, ?3, 'timeout', '旧错误', '2026-10-03T11:00:00.000Z', '2026-10-03T10:00:00.000Z')",
            params![id, attempts, max_attempts],
        )
        .unwrap();
    }

    fn snap(conn: &Connection, id: &str) -> JobSnap {
        conn.query_row(
            "SELECT status, attempts, error_kind, error_message, not_before, started_at, finished_at, input_tokens, output_tokens, provider, model, prompt_version, input_hash
             FROM ai_jobs WHERE id = ?1",
            [id],
            |row| {
                Ok(JobSnap {
                    status: row.get(0)?,
                    attempts: row.get(1)?,
                    error_kind: row.get(2)?,
                    error_message: row.get(3)?,
                    not_before: row.get(4)?,
                    started_at: row.get(5)?,
                    finished_at: row.get(6)?,
                    input_tokens: row.get(7)?,
                    output_tokens: row.get(8)?,
                    provider: row.get(9)?,
                    model: row.get(10)?,
                    prompt_version: row.get(11)?,
                    input_hash: row.get(12)?,
                })
            },
        )
        .unwrap()
    }

    fn record(id: &str, attempts: i64, max_attempts: i64) -> JobRecord {
        JobRecord {
            id: id.to_owned(),
            account_id: "acct".to_owned(),
            job_type: "repo_card".to_owned(),
            target_type: "repository".to_owned(),
            target_id: id.to_owned(),
            attempts,
            max_attempts,
            input_hash: None,
            reserved: 0,
        }
    }

    fn ok_success(skipped: bool) -> JobSuccess {
        JobSuccess {
            input_tokens: 11,
            output_tokens: 7,
            provider: "fake".to_owned(),
            model: "fake-model".to_owned(),
            prompt_version: "card-v1".to_owned(),
            input_hash: "hash-1".to_owned(),
            skipped,
        }
    }

    fn free_and_spend() -> Vec<ClaimCandidate> {
        vec![
            ClaimCandidate {
                job_type: "repo_card".to_owned(),
                spends_tokens: true,
            },
            ClaimCandidate {
                job_type: "categorize".to_owned(),
                spends_tokens: false,
            },
        ]
    }

    fn gate(paused: bool, budget: u64) -> BudgetGate {
        BudgetGate {
            paused,
            daily_budget_tokens: budget,
            unlimited: false,
        }
    }

    fn claim_one(
        conn: &mut Connection,
        handled: &[ClaimCandidate],
        budget_gate: &BudgetGate,
    ) -> Option<JobRecord> {
        claim_next(conn, NOW, handled, budget_gate).unwrap()
    }

    struct ReleaseOnDrop(Arc<(Mutex<bool>, Condvar)>);

    impl ReleaseOnDrop {
        fn notify(flag: &Arc<(Mutex<bool>, Condvar)>) {
            let (lock, condvar) = flag.as_ref();
            let mut go = lock.lock().unwrap();
            *go = true;
            condvar.notify_all();
        }
    }

    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            Self::notify(&self.0);
        }
    }

    #[test]
    fn error_kind_strings_match_the_check_constraint() {
        let pairs = [
            (ErrorKind::Interrupted, "interrupted"),
            (ErrorKind::Timeout, "timeout"),
            (ErrorKind::RateLimit, "rate_limit"),
            (ErrorKind::Auth, "auth"),
            (ErrorKind::BadJson, "bad_json"),
            (ErrorKind::Schema, "schema"),
            (ErrorKind::Network, "network"),
            (ErrorKind::ContentTooLarge, "content_too_large"),
            (ErrorKind::Other, "other"),
        ];
        for (kind, text) in pairs {
            assert_eq!(kind.as_str(), text);
        }
    }

    #[test]
    fn claim_orders_by_priority_then_created_at() {
        let db = TestDb::new("order");
        let conn = db.conn();
        insert_job(
            &conn,
            "late-high",
            "repo_card",
            5,
            "2026-10-03T09:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "early-low",
            "repo_card",
            1,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "later-low",
            "repo_card",
            1,
            "2026-10-03T08:30:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: false,
        }];
        let mut conn = db.conn();
        let first = claim_one(&mut conn, &handled, &gate(false, 0)).unwrap();
        let second = claim_one(&mut conn, &handled, &gate(false, 0)).unwrap();
        let third = claim_one(&mut conn, &handled, &gate(false, 0)).unwrap();
        assert!(claim_one(&mut conn, &handled, &gate(false, 0)).is_none());
        assert_eq!(first.id, "early-low");
        assert_eq!(second.id, "later-low");
        assert_eq!(third.id, "late-high");
        assert_eq!(first.attempts, 1);
        let row = snap(&conn, "early-low");
        assert_eq!(row.status, "running");
        assert_eq!(row.started_at.as_deref(), Some(NOW));
        assert_eq!(row.attempts, 1);
    }

    #[test]
    fn claim_prefers_tagged_then_recent_repos_within_a_priority() {
        let db = TestDb::new("order-relevance");
        let conn = db.conn();
        conn.execute(
            "INSERT INTO scenarios(id, account_id, slug, name_zh) VALUES ('sc-xhs', 'acct', 'xhs', '小红书')",
            [],
        )
        .unwrap();
        // (仓库 id 同时也是任务 id, 收藏时间, 场景标签状态, 优先级)
        let repos = [
            ("old-untagged", "2026-06-01T00:00:00Z", None, 6),
            ("new-untagged", "2026-10-01T00:00:00Z", None, 6),
            ("old-tagged", "2026-07-01T00:00:00Z", Some("suggested"), 5),
            ("new-tagged", "2026-09-20T00:00:00Z", Some("confirmed"), 6),
            ("rejected-tag", "2026-09-30T00:00:00Z", Some("rejected"), 6),
            ("urgent", "2026-05-01T00:00:00Z", None, 2),
        ];
        for (id, starred_at, tag, priority) in repos {
            conn.execute(
                "INSERT INTO repositories(id, account_id, owner, name, full_name, html_url, starred_at, sync_status)
                 VALUES (?1, 'acct', 'fox', ?1, 'fox/' || ?1, 'https://github.com/fox/' || ?1, ?2, 'active')",
                params![id, starred_at],
            )
            .unwrap();
            insert_job(
                &conn,
                id,
                "repo_card",
                priority,
                "2026-10-03T08:00:00.000Z",
                None,
                "queued",
            );
            if let Some(status) = tag {
                conn.execute(
                    "INSERT INTO entity_scenarios(account_id, entity_type, entity_id, scenario_id, origin, status)
                     VALUES ('acct', 'repository', ?1, 'sc-xhs', 'rule', ?2)",
                    params![id, status],
                )
                .unwrap();
            }
        }
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: false,
        }];
        let mut conn = db.conn();
        let mut order = Vec::new();
        while let Some(job) = claim_one(&mut conn, &handled, &gate(false, 0)) {
            order.push(job.id);
        }
        assert_eq!(
            order,
            [
                "urgent",
                "new-tagged",
                "old-tagged",
                "new-untagged",
                "rejected-tag",
                "old-untagged"
            ]
        );
    }

    #[test]
    fn running_jobs_reserve_budget_before_the_next_claim() {
        let db = TestDb::new("reserve");
        let conn = db.conn();
        for id in ["q1", "q2", "q3"] {
            insert_job(
                &conn,
                id,
                "repo_card",
                5,
                "2026-10-03T08:00:00.000Z",
                None,
                "queued",
            );
        }
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: true,
        }];
        // 预算正好放得下两条的预留：认领时预留就写在任务自己的行里，
        // 后一次认领一定看得见前一次，第三条领不到，额度不会被冲破。
        let mut conn = db.conn();
        let gate = gate(false, 2 * CLAIM_RESERVE_TOKENS as u64);
        let first = claim_one(&mut conn, &handled, &gate).unwrap();
        assert_eq!(first.reserved, CLAIM_RESERVE_TOKENS);
        assert_eq!(snap(&conn, &first.id).input_tokens, CLAIM_RESERVE_TOKENS);
        assert!(claim_one(&mut conn, &handled, &gate).is_some());
        assert!(
            claim_one(&mut conn, &handled, &gate).is_none(),
            "两个在跑的任务已预留满额度，不能再领"
        );
        // 不花钱的任务不受影响，也不记预留
        let free = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: false,
        }];
        let third = claim_one(&mut conn, &free, &gate).unwrap();
        assert_eq!(third.reserved, 0);
        assert_eq!(snap(&conn, &third.id).input_tokens, 0);
    }

    #[test]
    fn success_replaces_the_reserve_with_the_real_usage() {
        let db = TestDb::new("reserve-success");
        let conn = db.conn();
        insert_job(
            &conn,
            "job",
            "repo_card",
            5,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: true,
        }];
        let mut conn = db.conn();
        let claimed = claim_one(&mut conn, &handled, &gate(false, 100_000)).unwrap();
        drop(conn);
        assert!(finish_job(db.path(), &claimed, NOW, Ok(ok_success(false)), 0.0).unwrap());
        let row = snap(&db.conn(), "job");
        assert_eq!(
            (row.input_tokens, row.output_tokens),
            (11, 7),
            "预留要换成真实用量，不能叠在上面"
        );
    }

    #[test]
    fn cancelled_running_job_keeps_its_reserve_in_the_daily_usage() {
        let db = TestDb::new("reserve-cancel");
        let conn = db.conn();
        insert_job(
            &conn,
            "job",
            "repo_card",
            5,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: true,
        }];
        let mut conn = db.conn();
        let claimed = claim_one(&mut conn, &handled, &gate(false, 100_000)).unwrap();
        drop(conn);
        // 跑的过程中被取消：请求已经发出去，取不回来，预留保留；
        // 之后处理器返回成功也不能再改这条任务（update 都带 status = 'running'）。
        crate::v2_ops::cancel_job(db.path(), "job").unwrap();
        assert!(!finish_job(db.path(), &claimed, NOW, Ok(ok_success(false)), 0.0).unwrap());
        let row = snap(&db.conn(), "job");
        assert_eq!(row.status, "cancelled");
        assert_eq!(row.input_tokens, CLAIM_RESERVE_TOKENS);
        let status = queue_status(db.path()).unwrap();
        assert_eq!(status.tokens_24h, CLAIM_RESERVE_TOKENS);
    }

    #[test]
    fn cancelling_a_failed_job_does_not_erase_its_charge() {
        let db = TestDb::new("cancel-failed");
        let conn = db.conn();
        let now = now_db_time();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, finished_at, created_at, updated_at)
             VALUES ('failed-job', 'acct', 'repo_card', 'repository', 'r', 'failed', 10000, ?1, ?1, ?1)",
            [&now],
        )
        .unwrap();
        drop(conn);
        assert_eq!(queue_status(db.path()).unwrap().tokens_24h, 10_000);
        crate::v2_ops::cancel_job(db.path(), "failed-job").unwrap();
        assert_eq!(
            queue_status(db.path()).unwrap().tokens_24h,
            10_000,
            "取消一条失败的任务不能让它记过的用量消失"
        );
    }

    #[test]
    fn interrupted_attempt_stays_charged_and_waits_before_the_next_claim() {
        let db = TestDb::new("reserve-recover");
        let conn = db.conn();
        insert_job(
            &conn,
            "job",
            "repo_card",
            5,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: true,
        }];
        let mut conn = db.conn();
        claim_one(&mut conn, &handled, &gate(false, 100_000)).unwrap();
        // 程序被关掉 / 崩溃 / 重新编译：下次启动把在跑的任务放回队列
        assert_eq!(recover_running_jobs(&conn, NOW).unwrap(), 1);
        let row = snap(&conn, "job");
        assert_eq!(row.status, "queued");
        assert_eq!(
            row.input_tokens, CLAIM_RESERVE_TOKENS,
            "那次请求可能已经发出去，预留要留着，不能当没发生过"
        );
        assert_eq!(row.not_before.as_deref(), Some("2026-10-03T12:01:00.000Z"));
        assert!(claim_one(&mut conn, &handled, &gate(false, 100_000)).is_none());
        // 等够了再领，真实用量只替换这一次的预留，上一次中断的那笔照旧留着
        let again = claim_next(
            &mut conn,
            "2026-10-03T12:01:00.000Z",
            &handled,
            &gate(false, 100_000),
        )
        .unwrap()
        .unwrap();
        drop(conn);
        assert!(finish_job(
            db.path(),
            &again,
            "2026-10-03T12:01:05.000Z",
            Ok(ok_success(false)),
            0.0
        )
        .unwrap());
        let row = snap(&db.conn(), "job");
        assert_eq!(row.input_tokens, CLAIM_RESERVE_TOKENS + 11);
    }

    #[test]
    fn failure_after_claim_is_charged_by_kind_and_the_reserve_is_refunded() {
        for (kind, expected) in [
            (ErrorKind::BadJson, REPAIRED_FAILURE_TOKEN_ESTIMATE),
            (ErrorKind::Schema, REPAIRED_FAILURE_TOKEN_ESTIMATE),
            (ErrorKind::Timeout, FAILED_ATTEMPT_TOKEN_ESTIMATE),
            (ErrorKind::Other, FAILED_ATTEMPT_TOKEN_ESTIMATE),
            (ErrorKind::Network, 0),
            (ErrorKind::RateLimit, 0),
            (ErrorKind::ContentTooLarge, 0),
        ] {
            let db = TestDb::new(&format!("claim-fail-{}", kind.as_str()));
            let conn = db.conn();
            insert_job(
                &conn,
                "job",
                "repo_card",
                5,
                "2026-10-03T08:00:00.000Z",
                None,
                "queued",
            );
            drop(conn);
            let handled = vec![ClaimCandidate {
                job_type: "repo_card".to_owned(),
                spends_tokens: true,
            }];
            let mut conn = db.conn();
            let claimed = claim_one(&mut conn, &handled, &gate(false, 100_000)).unwrap();
            drop(conn);
            let failure = JobFailure::Retryable {
                kind,
                message: "失败".to_owned(),
                retry_after_secs: None,
            };
            assert!(finish_job(db.path(), &claimed, NOW, Err(failure), 0.0).unwrap());
            let row = snap(&db.conn(), "job");
            assert_eq!(row.status, "queued", "{kind:?}");
            assert_eq!(row.input_tokens, expected, "{kind:?}");
        }
    }

    #[test]
    fn claim_skips_not_before_in_the_future() {
        let db = TestDb::new("not-before");
        let conn = db.conn();
        insert_job(
            &conn,
            "future",
            "repo_card",
            0,
            "2026-10-03T08:00:00.000Z",
            Some("2026-10-03T12:00:00.001Z"),
            "queued",
        );
        insert_job(
            &conn,
            "exact",
            "repo_card",
            2,
            "2026-10-03T08:00:00.000Z",
            Some(NOW),
            "queued",
        );
        insert_job(
            &conn,
            "past",
            "repo_card",
            3,
            "2026-10-03T07:00:00.000Z",
            Some("2026-10-03T11:00:00.000Z"),
            "queued",
        );
        insert_job(
            &conn,
            "empty",
            "repo_card",
            4,
            "2026-10-03T06:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: false,
        }];
        let mut conn = db.conn();
        let ids = [
            claim_one(&mut conn, &handled, &gate(false, 0)).unwrap().id,
            claim_one(&mut conn, &handled, &gate(false, 0)).unwrap().id,
            claim_one(&mut conn, &handled, &gate(false, 0)).unwrap().id,
        ];
        assert!(claim_one(&mut conn, &handled, &gate(false, 0)).is_none());
        assert_eq!(ids, ["exact", "past", "empty"]);
        assert_eq!(snap(&conn, "future").status, "queued");
        let later = claim_next(
            &mut conn,
            "2026-10-03T12:00:00.001Z",
            &handled,
            &gate(false, 0),
        )
        .unwrap()
        .unwrap();
        assert_eq!(later.id, "future");
    }

    #[test]
    fn claim_skips_job_types_without_a_handler() {
        let db = TestDb::new("type");
        let conn = db.conn();
        insert_job(
            &conn,
            "other",
            "translate_readme",
            0,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "known",
            "repo_card",
            5,
            "2026-10-03T09:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: false,
        }];
        let mut conn = db.conn();
        let claimed = claim_one(&mut conn, &handled, &gate(false, 0)).unwrap();
        assert_eq!(claimed.id, "known");
        assert!(claim_one(&mut conn, &handled, &gate(false, 0)).is_none());
        assert_eq!(snap(&conn, "other").status, "queued");
    }

    #[test]
    fn four_threads_do_not_claim_the_same_job_twice() {
        let db = TestDb::new("race");
        let conn = db.conn();
        for index in 0..50 {
            insert_job(
                &conn,
                &format!("job-{index:02}"),
                "categorize",
                5,
                "2026-10-03T08:00:00.000Z",
                None,
                "queued",
            );
        }
        drop(conn);
        let claimed = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let path = db.path().to_path_buf();
            let claimed = Arc::clone(&claimed);
            handles.push(thread::spawn(move || {
                let mut conn = open_db(&path).unwrap();
                let handled = vec![ClaimCandidate {
                    job_type: "categorize".to_owned(),
                    spends_tokens: false,
                }];
                loop {
                    match claim_next(&mut conn, NOW, &handled, &gate(false, 0)).unwrap() {
                        Some(job) => claimed.lock().unwrap().push(job.id),
                        None => break,
                    }
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let mut ids = claimed.lock().unwrap().clone();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 50, "有重复认领或漏领");
        let conn = db.conn();
        let running: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE status = 'running'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let queued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE status = 'queued'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((running, queued), (50, 0));
    }

    #[test]
    fn zero_budget_claims_free_jobs_and_skips_spending_jobs() {
        let db = TestDb::new("zero-budget");
        let conn = db.conn();
        insert_job(
            &conn,
            "spend",
            "repo_card",
            0,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "free",
            "categorize",
            5,
            "2026-10-03T09:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let handled = free_and_spend();
        let mut conn = db.conn();
        let first = claim_one(&mut conn, &handled, &gate(false, 0)).unwrap();
        assert_eq!(first.id, "free");
        assert!(claim_one(&mut conn, &handled, &gate(false, 0)).is_none());
        assert_eq!(snap(&conn, "spend").status, "queued");
        // 预算要放得下这一条自己的预留才认领：预算比预留小，等于没有额度
        assert!(claim_one(&mut conn, &handled, &gate(false, 10)).is_none());
        let spend = claim_one(
            &mut conn,
            &handled,
            &gate(false, CLAIM_RESERVE_TOKENS as u64),
        )
        .unwrap();
        assert_eq!(spend.id, "spend");
        assert_eq!(spend.reserved, CLAIM_RESERVE_TOKENS);
    }

    #[test]
    fn rolling_24h_usage_stops_spending_claims() {
        let db = TestDb::new("budget-window");
        let handled = free_and_spend();
        let budget = 100_000_u64;
        let conn = db.conn();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, output_tokens, finished_at, created_at)
             VALUES ('old', 'acct', 'repo_card', 'repository', 'old', 'succeeded', 95000, 0, '2020-01-01T00:00:00.000Z', '2020-01-01T00:00:00.000Z')",
            [],
        )
        .unwrap();
        insert_job(
            &conn,
            "spend-old",
            "repo_card",
            1,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let mut conn = db.conn();
        // 24 小时前结束的用量不计入，预算仍然可以领花钱任务。
        assert_eq!(
            claim_one(&mut conn, &handled, &gate(false, budget))
                .unwrap()
                .id,
            "spend-old"
        );
        conn.execute("DELETE FROM ai_jobs WHERE id = 'spend-old'", [])
            .unwrap();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, output_tokens, finished_at, created_at)
             VALUES ('recent', 'acct', 'repo_card', 'repository', 'recent', 'succeeded', 80000, 20000, '2026-10-03T11:00:00.000Z', '2026-10-03T11:00:00.000Z')",
            [],
        )
        .unwrap();
        insert_job(
            &conn,
            "spend-full",
            "repo_card",
            1,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "free-full",
            "categorize",
            2,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        assert!(
            claim_one(&mut conn, &handled, &gate(false, budget))
                .unwrap()
                .id
                == "free-full"
        );
        assert!(claim_one(&mut conn, &handled, &gate(false, budget)).is_none());
        assert_eq!(snap(&conn, "spend-full").status, "queued");
        // 已用加上这一条的预留超过预算一个 token，就不领：额度是上限，不是「差一点也可以再领一条」。
        conn.execute(
            "UPDATE ai_jobs SET output_tokens = 8001 WHERE id = 'recent'",
            [],
        )
        .unwrap();
        assert!(claim_one(&mut conn, &handled, &gate(false, budget)).is_none());
        // 刚好放得下（88000 + 12000 预留 = 100000）才领，并且预留已经写在这条任务上。
        conn.execute(
            "UPDATE ai_jobs SET output_tokens = 8000 WHERE id = 'recent'",
            [],
        )
        .unwrap();
        let fits = claim_one(&mut conn, &handled, &gate(false, budget)).unwrap();
        assert_eq!(fits.id, "spend-full");
        assert_eq!(snap(&conn, "spend-full").input_tokens, CLAIM_RESERVE_TOKENS);
    }

    #[test]
    fn paused_gate_claims_nothing() {
        let db = TestDb::new("paused");
        let conn = db.conn();
        insert_job(
            &conn,
            "spend",
            "repo_card",
            0,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "free",
            "categorize",
            0,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let mut conn = db.conn();
        assert!(claim_one(&mut conn, &free_and_spend(), &gate(true, 1_000_000)).is_none());
        assert_eq!(snap(&conn, "spend").status, "queued");
        assert_eq!(snap(&conn, "free").status, "queued");
    }

    #[test]
    fn success_writes_result_fields_and_clears_errors() {
        let db = TestDb::new("success");
        let conn = db.conn();
        insert_running(&conn, "job", 1, 3);
        drop(conn);
        let updated = finish_job(
            db.path(),
            &record("job", 1, 3),
            NOW,
            Ok(ok_success(false)),
            0.0,
        )
        .unwrap();
        assert!(updated);
        let row = snap(&db.conn(), "job");
        assert_eq!(row.status, "succeeded");
        assert_eq!((row.input_tokens, row.output_tokens), (11, 7));
        assert_eq!(row.provider.as_deref(), Some("fake"));
        assert_eq!(row.model.as_deref(), Some("fake-model"));
        assert_eq!(row.prompt_version.as_deref(), Some("card-v1"));
        assert_eq!(row.input_hash.as_deref(), Some("hash-1"));
        assert_eq!(row.finished_at.as_deref(), Some(NOW));
        assert!(row.error_kind.is_none());
        assert!(row.error_message.is_none());
        assert_eq!(row.attempts, 1);
    }

    #[test]
    fn skipped_marks_skipped_status() {
        let db = TestDb::new("skipped");
        let conn = db.conn();
        insert_running(&conn, "job", 1, 3);
        drop(conn);
        assert!(finish_job(
            db.path(),
            &record("job", 1, 3),
            NOW,
            Ok(ok_success(true)),
            0.0
        )
        .unwrap());
        let row = snap(&db.conn(), "job");
        assert_eq!(row.status, "skipped");
        assert_eq!((row.input_tokens, row.output_tokens), (11, 7));
        assert!(row.error_kind.is_none());
        assert_eq!(row.finished_at.as_deref(), Some(NOW));
    }

    #[test]
    fn retryable_uses_backoff_table_clamp_and_jitter() {
        assert_eq!(shift_db_time(NOW, 30).unwrap(), "2026-10-03T12:00:30.000Z");
        assert_eq!(shift_db_time(NOW, 27).unwrap(), "2026-10-03T12:00:27.000Z");
        assert_eq!(shift_db_time(NOW, 33).unwrap(), "2026-10-03T12:00:33.000Z");
        assert_eq!(shift_db_time(NOW, 120).unwrap(), "2026-10-03T12:02:00.000Z");
        assert_eq!(shift_db_time(NOW, 126).unwrap(), "2026-10-03T12:02:06.000Z");
        assert_eq!(shift_db_time(NOW, 600).unwrap(), "2026-10-03T12:10:00.000Z");
        assert_eq!(shift_db_time(NOW, 660).unwrap(), "2026-10-03T12:11:00.000Z");
        assert_eq!(shift_db_time(NOW, 5).unwrap(), "2026-10-03T12:00:05.000Z");
        assert_eq!(
            shift_db_time(NOW, 3600).unwrap(),
            "2026-10-03T13:00:00.000Z"
        );
        assert_eq!(
            shift_db_time("2026-10-03T23:59:50.000Z", 30).unwrap(),
            "2026-10-04T00:00:20.000Z"
        );

        let cases = [
            (1, None, 0.0, "2026-10-03T12:00:30.000Z"),
            (2, None, 0.0, "2026-10-03T12:02:00.000Z"),
            (3, None, 0.0, "2026-10-03T12:10:00.000Z"),
            (1, None, 1.0, "2026-10-03T12:00:33.000Z"),
            (1, None, -1.0, "2026-10-03T12:00:27.000Z"),
            (2, None, 0.5, "2026-10-03T12:02:06.000Z"),
            (4, None, 1.0, "2026-10-03T12:11:00.000Z"),
            (1, Some(1), 1.0, "2026-10-03T12:00:05.000Z"),
            (1, Some(10), -1.0, "2026-10-03T12:00:10.000Z"),
            (1, Some(9_000), 0.0, "2026-10-03T13:00:00.000Z"),
        ];
        for (index, (attempt, retry_after, jitter, expect)) in cases.iter().enumerate() {
            let db = TestDb::new(&format!("backoff-{index}"));
            let conn = db.conn();
            insert_running(&conn, "job", *attempt, 9);
            drop(conn);
            let failure = JobFailure::Retryable {
                kind: ErrorKind::RateLimit,
                message: "稍后重试".to_owned(),
                retry_after_secs: *retry_after,
            };
            assert!(finish_job(
                db.path(),
                &record("job", *attempt, 9),
                NOW,
                Err(failure),
                *jitter
            )
            .unwrap());
            let row = snap(&db.conn(), "job");
            assert_eq!(row.status, "queued", "attempt {attempt}");
            assert_eq!(row.error_kind.as_deref(), Some("rate_limit"));
            assert_eq!(row.error_message.as_deref(), Some("稍后重试"));
            assert_eq!(
                row.not_before.as_deref(),
                Some(*expect),
                "attempt {attempt} jitter {jitter}"
            );
            assert!(row.started_at.is_none());
            assert!(row.finished_at.is_none());
            assert_eq!(row.attempts, *attempt);
        }
    }

    #[test]
    fn retryable_fails_when_attempts_are_exhausted() {
        let db = TestDb::new("exhausted");
        let conn = db.conn();
        insert_running(&conn, "job", 3, 3);
        drop(conn);
        let failure = JobFailure::Retryable {
            kind: ErrorKind::Network,
            message: "网络断了".to_owned(),
            retry_after_secs: Some(10),
        };
        assert!(finish_job(db.path(), &record("job", 3, 3), NOW, Err(failure), 0.0).unwrap());
        let row = snap(&db.conn(), "job");
        assert_eq!(row.status, "failed");
        assert_eq!(row.error_kind.as_deref(), Some("network"));
        assert_eq!(row.error_message.as_deref(), Some("网络断了"));
        assert_eq!(row.finished_at.as_deref(), Some(NOW));
        assert_eq!(row.attempts, 3);
    }

    #[test]
    fn fatal_fails_immediately() {
        let db = TestDb::new("fatal");
        let conn = db.conn();
        insert_running(&conn, "job", 1, 3);
        drop(conn);
        let failure = JobFailure::Fatal {
            kind: ErrorKind::Schema,
            message: "字段不对".to_owned(),
        };
        assert!(finish_job(db.path(), &record("job", 1, 3), NOW, Err(failure), 0.0).unwrap());
        let row = snap(&db.conn(), "job");
        assert_eq!(row.status, "failed");
        assert_eq!(row.error_kind.as_deref(), Some("schema"));
        assert_eq!(row.error_message.as_deref(), Some("字段不对"));
        assert_eq!(row.finished_at.as_deref(), Some(NOW));
        assert_eq!(row.attempts, 1);
    }

    #[test]
    fn failed_attempts_that_reached_the_model_are_charged_an_estimate() {
        for kind in [
            ErrorKind::BadJson,
            ErrorKind::Schema,
            ErrorKind::Timeout,
            ErrorKind::Other,
        ] {
            let db = TestDb::new(&format!("charge-{}", kind.as_str()));
            let conn = db.conn();
            insert_running(&conn, "job", 1, 3);
            drop(conn);
            let failure = JobFailure::Retryable {
                kind,
                message: "模型回了不能用的内容".to_owned(),
                retry_after_secs: None,
            };
            assert!(finish_job(db.path(), &record("job", 1, 3), NOW, Err(failure), 0.0).unwrap());
            let row = snap(&db.conn(), "job");
            assert_eq!(row.status, "queued", "{kind:?}");
            assert_eq!(row.input_tokens, failure_charge(kind), "{kind:?}");
        }
    }

    #[test]
    fn failures_that_never_reached_the_model_are_not_charged() {
        for kind in [
            ErrorKind::Network,
            ErrorKind::RateLimit,
            ErrorKind::ContentTooLarge,
            ErrorKind::Interrupted,
        ] {
            let db = TestDb::new(&format!("nocharge-{}", kind.as_str()));
            let conn = db.conn();
            insert_running(&conn, "job", 1, 3);
            drop(conn);
            let failure = JobFailure::Fatal {
                kind,
                message: "没有到模型".to_owned(),
            };
            assert!(finish_job(db.path(), &record("job", 1, 3), NOW, Err(failure), 0.0).unwrap());
            assert_eq!(snap(&db.conn(), "job").input_tokens, 0, "{kind:?}");
        }
    }

    #[test]
    fn charged_failures_count_toward_the_daily_usage_even_while_waiting_to_retry() {
        let db = TestDb::new("charge-usage");
        let conn = db.conn();
        insert_running(&conn, "job", 1, 3);
        drop(conn);
        let failure = JobFailure::Retryable {
            kind: ErrorKind::BadJson,
            message: "坏 JSON".to_owned(),
            retry_after_secs: None,
        };
        let now = now_db_time();
        assert!(finish_job(db.path(), &record("job", 1, 3), &now, Err(failure), 0.0).unwrap());
        let status = queue_status(db.path()).unwrap();
        assert_eq!(status.queued, 1);
        assert_eq!(status.tokens_24h, REPAIRED_FAILURE_TOKEN_ESTIMATE);
    }

    #[test]
    fn success_after_a_failed_attempt_keeps_the_earlier_charge() {
        let db = TestDb::new("charge-then-success");
        let conn = db.conn();
        insert_running(&conn, "job", 1, 3);
        drop(conn);
        let failure = JobFailure::Retryable {
            kind: ErrorKind::Schema,
            message: "字段不对".to_owned(),
            retry_after_secs: None,
        };
        assert!(finish_job(db.path(), &record("job", 1, 3), NOW, Err(failure), 0.0).unwrap());
        // 再领一次（模拟重试）：直接把它改回 running
        db.conn()
            .execute(
                "UPDATE ai_jobs SET status = 'running', attempts = 2 WHERE id = 'job'",
                [],
            )
            .unwrap();
        let success = JobSuccess {
            input_tokens: 3000,
            output_tokens: 1000,
            provider: "p".to_owned(),
            model: "m".to_owned(),
            prompt_version: "v".to_owned(),
            input_hash: "h".to_owned(),
            skipped: false,
        };
        assert!(finish_job(db.path(), &record("job", 2, 3), NOW, Ok(success), 0.0).unwrap());
        let row = snap(&db.conn(), "job");
        assert_eq!(row.status, "succeeded");
        assert_eq!(row.input_tokens, REPAIRED_FAILURE_TOKEN_ESTIMATE + 3000);
        assert_eq!(row.output_tokens, 1000);
    }

    #[test]
    fn pause_queue_requeues_without_counting_the_attempt() {
        let db = TestDb::new("pause-queue");
        let control = queue_control_path(db.path());
        set_daily_budget(&control, 50_321).unwrap();
        let conn = db.conn();
        insert_job(
            &conn,
            "job",
            "repo_card",
            1,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "next",
            "categorize",
            2,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let mut conn = db.conn();
        let claimed = claim_one(&mut conn, &free_and_spend(), &gate(false, 50_321)).unwrap();
        assert_eq!(claimed.attempts, 1);
        assert_eq!(snap(&conn, "job").input_tokens, CLAIM_RESERVE_TOKENS);
        drop(conn);
        let failure = JobFailure::PauseQueue {
            kind: ErrorKind::Auth,
            message: "密钥无效".to_owned(),
        };
        assert!(finish_job(db.path(), &claimed, NOW, Err(failure), 0.0).unwrap());
        let row = snap(&db.conn(), "job");
        assert_eq!(row.status, "queued");
        assert_eq!(row.attempts, 0, "这次认证失败不应计入 attempts");
        assert_eq!(row.input_tokens, 0, "认证失败没有花钱，预留要退回");
        assert!(row.started_at.is_none());
        let stored = read_control(&control);
        assert!(stored.paused);
        assert_eq!(stored.daily_budget_tokens, 50_321);
        let reason = stored.pause_reason.expect("应写入暂停原因");
        assert_eq!(reason.kind, "auth");
        assert_eq!(reason.message, "密钥无效");
        assert_eq!(reason.at, NOW);
        let mut conn = db.conn();
        let file_gate = gate(
            read_control(&control).paused,
            read_control(&control).daily_budget_tokens,
        );
        assert!(claim_one(&mut conn, &free_and_spend(), &file_gate).is_none());
        set_paused(&control, false).unwrap();
        let resumed = read_control(&control);
        assert!(!resumed.paused);
        assert!(resumed.pause_reason.is_none());
        assert_eq!(resumed.daily_budget_tokens, 50_321);
        let again = claim_one(&mut conn, &free_and_spend(), &gate(false, 50_321)).unwrap();
        assert_eq!(again.id, "job");
    }

    struct PanicHandler {
        calls: AtomicUsize,
    }

    impl JobHandler for PanicHandler {
        fn job_type(&self) -> &'static str {
            "categorize"
        }
        fn spends_tokens(&self) -> bool {
            false
        }
        fn run(&self, job: &JobRecord, _db_path: &Path) -> Result<JobSuccess, JobFailure> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if job.target_id == "boom" {
                panic!("故意崩溃");
            }
            Ok(ok_success(false))
        }
    }

    #[test]
    fn handler_panic_fails_the_job_and_the_worker_continues() {
        let db = TestDb::new("panic");
        let conn = db.conn();
        insert_job(
            &conn,
            "boom",
            "categorize",
            1,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "ok",
            "categorize",
            2,
            "2026-10-03T08:00:01.000Z",
            None,
            "queued",
        );
        drop(conn);
        let handler = Arc::new(PanicHandler {
            calls: AtomicUsize::new(0),
        });
        let worker = Worker::start(
            db.path().to_path_buf(),
            vec![handler.clone()],
            WorkerConfig {
                concurrency: 1,
                idle_poll: Duration::from_millis(20),
            },
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            let conn = db.conn();
            let boom = snap(&conn, "boom");
            let ok = snap(&conn, "ok");
            if boom.status == "failed" && ok.status == "succeeded" {
                break;
            }
            drop(conn);
            thread::sleep(Duration::from_millis(20));
        }
        worker.stop();
        let boom = snap(&db.conn(), "boom");
        let ok = snap(&db.conn(), "ok");
        assert_eq!(boom.status, "failed");
        assert_eq!(boom.error_kind.as_deref(), Some("other"));
        assert_eq!(
            boom.error_message.as_deref(),
            Some("任务内部出错：故意崩溃")
        );
        assert_eq!(ok.status, "succeeded");
        assert_eq!(handler.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn cancelled_job_is_not_overwritten() {
        let db = TestDb::new("cancel");
        let conn = db.conn();
        insert_running(&conn, "ok-path", 1, 3);
        insert_running(&conn, "err-path", 1, 3);
        conn.execute(
            "UPDATE ai_jobs SET status = 'cancelled', input_tokens = 0, output_tokens = 0 WHERE id IN ('ok-path', 'err-path')",
            [],
        )
        .unwrap();
        drop(conn);
        let success = finish_job(
            db.path(),
            &record("ok-path", 1, 3),
            NOW,
            Ok(ok_success(false)),
            0.0,
        )
        .unwrap();
        let failure = finish_job(
            db.path(),
            &record("err-path", 1, 3),
            NOW,
            Err(JobFailure::Fatal {
                kind: ErrorKind::Other,
                message: "不应写上".to_owned(),
            }),
            0.0,
        )
        .unwrap();
        assert!(!success);
        assert!(!failure);
        let ok_row = snap(&db.conn(), "ok-path");
        let err_row = snap(&db.conn(), "err-path");
        assert_eq!(ok_row.status, "cancelled");
        assert_eq!((ok_row.input_tokens, ok_row.output_tokens), (0, 0));
        assert_eq!(ok_row.error_kind.as_deref(), Some("timeout"));
        assert_eq!(err_row.status, "cancelled");
        assert_eq!(err_row.error_message.as_deref(), Some("旧错误"));
        assert_ne!(err_row.error_message.as_deref(), Some("不应写上"));
    }

    #[test]
    fn recover_running_jobs_marks_them_interrupted() {
        let db = TestDb::new("recover-direct");
        let conn = db.conn();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, started_at)
             VALUES ('run-a', 'acct', 'repo_card', 'repository', 'run-a', 'running', 2, '2026-10-03T11:00:00.000Z'),
                    ('run-b', 'acct', 'categorize', 'repository', 'run-b', 'running', 0, '2026-10-03T11:00:00.000Z')",
            [],
        )
        .unwrap();
        let changed = recover_running_jobs(&conn, NOW).unwrap();
        assert_eq!(changed, 2);
        assert_eq!(recover_running_jobs(&conn, NOW).unwrap(), 0);
        for id in ["run-a", "run-b"] {
            let row = snap(&conn, id);
            assert_eq!(row.status, "queued");
            assert_eq!(row.error_kind.as_deref(), Some("interrupted"));
            assert!(row.started_at.is_none());
        }
        assert_eq!(snap(&conn, "run-a").attempts, 2);
    }

    struct ObserveHandler {
        attempts: Mutex<Option<i64>>,
        kind: Mutex<Option<String>>,
    }

    impl JobHandler for ObserveHandler {
        fn job_type(&self) -> &'static str {
            "repo_card"
        }
        fn spends_tokens(&self) -> bool {
            false
        }
        fn run(&self, job: &JobRecord, db_path: &Path) -> Result<JobSuccess, JobFailure> {
            let conn = open_db(db_path).map_err(|error| JobFailure::Fatal {
                kind: ErrorKind::Other,
                message: error,
            })?;
            let kind: Option<String> = conn
                .query_row(
                    "SELECT error_kind FROM ai_jobs WHERE id = ?1",
                    [&job.id],
                    |row| row.get(0),
                )
                .map_err(|error| JobFailure::Fatal {
                    kind: ErrorKind::Other,
                    message: error.to_string(),
                })?;
            *self.attempts.lock().unwrap() = Some(job.attempts);
            *self.kind.lock().unwrap() = kind;
            Ok(ok_success(false))
        }
    }

    #[test]
    fn worker_start_returns_running_jobs_to_queue_before_claim() {
        let db = TestDb::new("recover-worker");
        let conn = db.conn();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, max_attempts, started_at)
             VALUES ('legacy', 'acct', 'repo_card', 'repository', 'legacy', 'running', 4, 6, '2026-10-03T11:00:00.000Z')",
            [],
        )
        .unwrap();
        drop(conn);
        let handler = Arc::new(ObserveHandler {
            attempts: Mutex::new(None),
            kind: Mutex::new(None),
        });
        let worker = Worker::start(
            db.path().to_path_buf(),
            vec![handler.clone()],
            WorkerConfig {
                concurrency: 1,
                idle_poll: Duration::from_millis(20),
            },
        );
        // 启动时先恢复：放回队列，并且要隔一会儿才能再领，免得一直崩的任务立刻又花一次
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if snap(&db.conn(), "legacy").not_before.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let waiting = snap(&db.conn(), "legacy");
        assert_eq!(waiting.status, "queued");
        assert!(waiting.not_before.is_some(), "恢复的任务要等一会儿才能再领");
        assert!(handler.attempts.lock().unwrap().is_none());
        // 等够了（这里直接清掉等待时间）才会被领走
        db.conn()
            .execute(
                "UPDATE ai_jobs SET not_before = NULL WHERE id = 'legacy'",
                [],
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if snap(&db.conn(), "legacy").status == "succeeded" {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        worker.stop();
        assert_eq!(snap(&db.conn(), "legacy").status, "succeeded");
        assert_eq!(*handler.attempts.lock().unwrap(), Some(5));
        assert_eq!(handler.kind.lock().unwrap().as_deref(), Some("interrupted"));
    }

    #[test]
    fn control_file_defaults_to_zero_and_pause_keeps_budget() {
        let db = TestDb::new("control");
        let path = db.path();
        assert_eq!(crate::v2_ops::daily_budget(path), 0);
        assert!(!crate::v2_ops::queue_is_paused(path));
        assert!(!crate::v2_ops::queue_control_path(path).exists());

        let control = queue_control_path(path);
        fs::write(
            &control,
            r#"{"paused":true,"note":"keep","pauseReason":{"kind":"auth","message":"x","at":"2026-10-03T00:00:00.000Z"}}"#,
        )
        .unwrap();
        let missing_budget = read_control(&control);
        assert!(missing_budget.paused);
        assert_eq!(
            missing_budget.daily_budget_tokens, 0,
            "没有预算字段时必须是 0，不能是 200000"
        );

        assert_eq!(
            crate::v2_ops::set_daily_budget(path, 80_000).unwrap(),
            80_000
        );
        crate::v2_ops::set_queue_paused(path, true).unwrap();
        let paused = serde_json::from_str::<Value>(&fs::read_to_string(&control).unwrap()).unwrap();
        assert_eq!(paused["dailyBudgetTokens"], 80_000);
        assert_eq!(paused["paused"], true);
        assert_eq!(paused["note"], "keep");
        assert!(!paused.to_string().contains("200000"));
        crate::v2_ops::set_queue_paused(path, false).unwrap();
        let resumed =
            serde_json::from_str::<Value>(&fs::read_to_string(&control).unwrap()).unwrap();
        assert_eq!(resumed["dailyBudgetTokens"], 80_000);
        assert_eq!(resumed["paused"], false);
        assert_eq!(resumed["note"], "keep");
        assert!(resumed.get("pauseReason").is_none());
        assert_eq!(crate::v2_ops::daily_budget(path), 80_000);
        assert!(!crate::v2_ops::queue_is_paused(path));
    }

    #[test]
    fn set_daily_budget_accepts_zero_and_enforces_cap() {
        let db = TestDb::new("cap");
        let control = queue_control_path(db.path());
        fs::write(&control, r#"{"paused":true,"note":"keep"}"#).unwrap();
        assert_eq!(set_daily_budget(&control, 0).unwrap(), 0);
        assert_eq!(read_control(&control).daily_budget_tokens, 0);
        assert!(read_control(&control).paused);
        let text = fs::read_to_string(&control).unwrap();
        assert!(text.contains("keep"));
        assert_eq!(set_daily_budget(&control, 2_000_000).unwrap(), 2_000_000);
        assert_eq!(set_daily_budget(&control, 2_000_001).unwrap(), 2_000_000);
        assert_eq!(read_control(&control).daily_budget_tokens, 2_000_000);
        assert_eq!(
            crate::v2_ops::set_daily_budget(db.path(), 9_000_000).unwrap(),
            2_000_000
        );
    }

    struct GateHandler {
        current: Arc<AtomicUsize>,
        max_seen: Arc<AtomicUsize>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }

    impl JobHandler for GateHandler {
        fn job_type(&self) -> &'static str {
            "categorize"
        }
        fn spends_tokens(&self) -> bool {
            false
        }
        fn run(&self, _job: &JobRecord, _db_path: &Path) -> Result<JobSuccess, JobFailure> {
            let now = self.current.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_seen.fetch_max(now, Ordering::SeqCst);
            let (lock, condvar) = self.release.as_ref();
            let guard = lock.lock().unwrap();
            let _ = condvar.wait_timeout_while(guard, Duration::from_secs(3), |go| !*go);
            self.current.fetch_sub(1, Ordering::SeqCst);
            if self.max_seen.load(Ordering::SeqCst) >= 2 {
                Ok(ok_success(false))
            } else {
                Err(JobFailure::Fatal {
                    kind: ErrorKind::Other,
                    message: "没有并行".to_owned(),
                })
            }
        }
    }

    #[test]
    fn two_workers_run_jobs_at_the_same_time() {
        let db = TestDb::new("parallel");
        let conn = db.conn();
        insert_job(
            &conn,
            "a",
            "categorize",
            1,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "b",
            "categorize",
            1,
            "2026-10-03T08:00:01.000Z",
            None,
            "queued",
        );
        drop(conn);
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(GateHandler {
            current: Arc::new(AtomicUsize::new(0)),
            max_seen: Arc::clone(&max_seen),
            release: Arc::clone(&release),
        });
        let worker = Worker::start(
            db.path().to_path_buf(),
            vec![handler],
            WorkerConfig {
                concurrency: 2,
                idle_poll: Duration::from_millis(20),
            },
        );
        let _guard = ReleaseOnDrop(Arc::clone(&release));
        let deadline = Instant::now() + Duration::from_secs(3);
        while max_seen.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            max_seen.load(Ordering::SeqCst) >= 2,
            "两条任务没有同时处于处理器里"
        );
        ReleaseOnDrop::notify(&release);
        worker.stop();
        assert_eq!(snap(&db.conn(), "a").status, "succeeded");
        assert_eq!(snap(&db.conn(), "b").status, "succeeded");
    }

    struct BlockHandler {
        entered: Arc<(Mutex<usize>, Condvar)>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }

    impl JobHandler for BlockHandler {
        fn job_type(&self) -> &'static str {
            "categorize"
        }
        fn spends_tokens(&self) -> bool {
            false
        }
        fn run(&self, _job: &JobRecord, _db_path: &Path) -> Result<JobSuccess, JobFailure> {
            {
                let (lock, condvar) = self.entered.as_ref();
                let mut count = lock.lock().unwrap();
                *count += 1;
                condvar.notify_all();
            }
            let (lock, condvar) = self.release.as_ref();
            let guard = lock.lock().unwrap();
            let _ = condvar.wait_timeout_while(guard, Duration::from_secs(3), |go| !*go);
            Ok(ok_success(false))
        }
    }

    #[test]
    fn stop_waits_for_the_running_job_and_claims_nothing_else() {
        let db = TestDb::new("stop");
        let conn = db.conn();
        insert_job(
            &conn,
            "first",
            "categorize",
            1,
            "2026-10-03T08:00:00.000Z",
            None,
            "queued",
        );
        insert_job(
            &conn,
            "second",
            "categorize",
            2,
            "2026-10-03T08:00:01.000Z",
            None,
            "queued",
        );
        drop(conn);
        let entered = Arc::new((Mutex::new(0_usize), Condvar::new()));
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let handler = Arc::new(BlockHandler {
            entered: Arc::clone(&entered),
            release: Arc::clone(&release),
        });
        let worker = Worker::start(
            db.path().to_path_buf(),
            vec![handler],
            WorkerConfig {
                concurrency: 1,
                idle_poll: Duration::from_millis(20),
            },
        );
        assert!(!worker.status().stopped);
        let _guard = ReleaseOnDrop(Arc::clone(&release));
        let (lock, condvar) = entered.as_ref();
        let guard = lock.lock().unwrap();
        let (guard, timeout) = condvar
            .wait_timeout_while(guard, Duration::from_secs(3), |count| *count == 0)
            .unwrap();
        assert!(!timeout.timed_out());
        assert_eq!(*guard, 1);
        drop(guard);
        let release_later = Arc::clone(&release);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(250));
            ReleaseOnDrop::notify(&release_later);
        });
        let started = Instant::now();
        worker.stop();
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert_eq!(snap(&db.conn(), "first").status, "succeeded");
        assert_eq!(snap(&db.conn(), "second").status, "queued");
        assert_eq!(*entered.as_ref().0.lock().unwrap(), 1);
    }

    #[test]
    fn queue_status_counts_match_the_rows() {
        let db = TestDb::new("status");
        let now = now_db_time();
        let recent = shift_db_time(&now, -3600).unwrap();
        let old = shift_db_time(&now, -48 * 3600).unwrap();
        let conn = db.conn();
        insert_job(&conn, "q1", "repo_card", 1, &old, None, "queued");
        insert_job(&conn, "q2", "repo_card", 1, &old, None, "queued");
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, created_at)
             VALUES ('running-1', 'acct', 'repo_card', 'repository', 'running-1', 'running', ?1)",
            [&old],
        )
        .unwrap();
        for (id, input, output) in [("s1", 10, 5), ("s2", 20, 0)] {
            conn.execute(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, output_tokens, finished_at, created_at)
                 VALUES (?1, 'acct', 'repo_card', 'repository', ?1, 'succeeded', ?2, ?3, ?4, ?4)",
                params![id, input, output, recent],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, output_tokens, finished_at, created_at)
             VALUES ('old-ok', 'acct', 'repo_card', 'repository', 'old-ok', 'succeeded', 999, 0, ?1, ?1)",
            [&old],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, output_tokens, finished_at, created_at)
             VALUES ('skip', 'acct', 'repo_card', 'repository', 'skip', 'skipped', 3, 4, ?1, ?1)",
            [&recent],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, output_tokens, finished_at, created_at)
             VALUES ('fail-new', 'acct', 'repo_card', 'repository', 'fail-new', 'failed', 7, 1, ?1, ?1),
                    ('fail-old', 'acct', 'categorize', 'repository', 'fail-old', 'failed', 50, 0, ?2, ?2)",
            params![recent, old],
        )
        .unwrap();
        drop(conn);
        // 额度要比认领一条任务的预留多一点，才能看出「还没用完」
        let budget = CLAIM_RESERVE_TOKENS as u64 + 100;
        set_daily_budget(&queue_control_path(db.path()), budget).unwrap();
        let status = queue_status(db.path()).unwrap();
        assert_eq!(status.queued, 2);
        assert_eq!(status.running, 1);
        assert_eq!(status.succeeded_24h, 2);
        assert_eq!(status.failed, 2);
        // 15 + 20 + skipped 7 + 最近失败 8 = 50。24 小时前的 999 和 50 不计入。
        assert_eq!(status.tokens_24h, 50);
        assert_eq!(status.daily_budget, budget);
        assert_eq!(status.budget_remaining, budget - 50);
        assert_eq!(status.state, "running");
        assert!(status.pause_reason.is_none());

        let conn = db.conn();
        conn.execute(
            "UPDATE ai_jobs SET status = 'cancelled' WHERE id = 'running-1'",
            [],
        )
        .unwrap();
        drop(conn);
        assert_eq!(queue_status(db.path()).unwrap().state, "idle");

        // 已用 50，再加一条的预留 12000 就是 12050：额度只有 12049 时，剩下的放不下一条，队列不会再领，状态就是「用完了」
        set_daily_budget(
            &queue_control_path(db.path()),
            CLAIM_RESERVE_TOKENS as u64 + 49,
        )
        .unwrap();
        let reached = queue_status(db.path()).unwrap();
        assert_eq!(reached.state, "budget_reached");
        assert_eq!(
            reached.budget_remaining, 0,
            "剩下的不够再领一条，界面不该说「还能整理几个」"
        );
        assert_eq!(reached.tokens_24h, 50);

        set_paused(&queue_control_path(db.path()), true).unwrap();
        assert_eq!(queue_status(db.path()).unwrap().state, "paused");
        set_pause_reason(
            &queue_control_path(db.path()),
            &PauseReason {
                kind: "auth".to_owned(),
                message: "密钥无效".to_owned(),
                at: now,
            },
        )
        .unwrap();
        let stopped = queue_status(db.path()).unwrap();
        assert_eq!(stopped.state, "stopped");
        assert_eq!(stopped.pause_reason.unwrap().kind, "auth");

        set_paused(&queue_control_path(db.path()), false).unwrap();
        set_daily_budget(&queue_control_path(db.path()), 0).unwrap();
        let idle = queue_status(db.path()).unwrap();
        assert_eq!(idle.state, "idle");
        assert_eq!(idle.daily_budget, 0);
        assert_eq!(idle.budget_remaining, 0);
    }

    #[test]
    fn enqueue_missing_cards_does_not_change_the_budget() {
        let db = TestDb::new("enqueue");
        let conn = db.conn();
        conn.execute(
            "INSERT INTO repositories(id, account_id, owner, name, full_name, html_url, starred_at, sync_status)
             VALUES ('repo-missing', 'acct', 'fox', 'tool', 'fox/tool', 'https://github.com/fox/tool', '2026-10-01T00:00:00.000Z', 'active')",
            [],
        )
        .unwrap();
        drop(conn);
        assert_eq!(
            crate::v2_ops::set_daily_budget(db.path(), 777).unwrap(),
            777
        );
        let queued = crate::v2_ops::enqueue_missing_cards(db.path()).unwrap();
        assert!(queued >= 1);
        assert_eq!(crate::v2_ops::daily_budget(db.path()), 777);
        assert_eq!(
            read_control(&queue_control_path(db.path())).daily_budget_tokens,
            777
        );
        let again = crate::v2_ops::enqueue_missing_cards(db.path()).unwrap();
        assert_eq!(again, 0);
        assert_eq!(crate::v2_ops::daily_budget(db.path()), 777);
    }

    #[test]
    fn start_worker_starts_threads_for_the_given_handlers() {
        let db = TestDb::new("start-worker");
        let worker = start_worker(db.path(), Vec::new());
        assert!(!worker.status().stopped);
        worker.stop();
    }

    #[test]
    fn claim_expires_charges_older_than_a_day_instead_of_redating_them() {
        let db = TestDb::new("claim-expire");
        let conn = db.conn();
        // 三天前失败过一次、身上还留着 3 万用量的排队任务；和一条一小时前才失败过（用量还在窗口里）的
        for (id, updated_at, tokens) in [
            ("stale", "2026-10-01T08:00:00.000Z", 30_000),
            ("fresh", "2026-10-03T11:00:00.000Z", 4_000),
        ] {
            conn.execute(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, attempts, input_tokens, output_tokens, created_at, updated_at)
                 VALUES (?1, 'acct', 'repo_card', 'repository', ?1, ?4, 'queued', 1, ?2, 10, '2026-10-01T00:00:00.000Z', ?3)",
                params![id, tokens, updated_at, if id == "stale" { 1 } else { 2 }],
            )
            .unwrap();
        }
        drop(conn);
        let handled = vec![ClaimCandidate {
            job_type: "repo_card".to_owned(),
            spends_tokens: true,
        }];
        let mut conn = db.conn();
        // 窗口里只有 fresh 的 4000；stale 的 3 万早就过期了，不算
        assert_eq!(
            tokens_used_since(&conn, "2026-10-02T12:00:00.000Z").unwrap(),
            4_010
        );
        let stale = claim_one(&mut conn, &handled, &gate(false, 100_000)).unwrap();
        assert_eq!(stale.id, "stale");
        let row = snap(&conn, "stale");
        assert_eq!(
            (row.input_tokens, row.output_tokens),
            (CLAIM_RESERVE_TOKENS, 0),
            "认领会把 updated_at 改成现在：过期的旧用量要先清零，不能被重新记到今天"
        );
        let fresh = claim_one(&mut conn, &handled, &gate(false, 100_000)).unwrap();
        assert_eq!(fresh.id, "fresh");
        let row = snap(&conn, "fresh");
        assert_eq!(
            (row.input_tokens, row.output_tokens),
            (4_000 + CLAIM_RESERVE_TOKENS, 10),
            "窗口里的旧用量照旧保留"
        );
    }

    #[test]
    fn recovering_an_old_running_row_does_not_redate_its_reserve_into_today() {
        let db = TestDb::new("recover-old");
        let conn = db.conn();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, input_tokens, started_at, created_at, updated_at)
             VALUES ('old-run', 'acct', 'repo_card', 'repository', 'old-run', 'running', 1, 12000, '2026-10-01T08:00:00.000Z', '2026-10-01T07:00:00.000Z', '2026-10-01T08:00:00.000Z')",
            [],
        )
        .unwrap();
        assert_eq!(recover_running_jobs(&conn, NOW).unwrap(), 1);
        let row = snap(&conn, "old-run");
        assert_eq!(row.status, "queued");
        assert_eq!(row.input_tokens, 12_000, "账本不改");
        drop(conn);
        // 那次请求发生在三天前：预留按当时的时间算，不计入今天的窗口
        let status = queue_status(db.path()).unwrap();
        assert_eq!(
            status.tokens_24h, 0,
            "几天前的预留不能被恢复动作重新记到今天"
        );
    }

    #[test]
    fn recovery_cancels_orphaned_legacy_reservations_instead_of_queueing_them() {
        let db = TestDb::new("recover-legacy");
        let conn = db.conn();
        let now = now_db_time();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, input_tokens, started_at, created_at, updated_at)
             VALUES ('legacy-run', 'acct', 'problem_answer', 'query', 'legacy-abc', 'running', 1, 4000, ?1, ?1, ?1)",
            [&now],
        )
        .unwrap();
        insert_running(&conn, "card-run", 1, 3);
        assert_eq!(
            recover_running_jobs(&conn, NOW).unwrap(),
            1,
            "只有队列任务被放回队列"
        );
        let legacy = snap(&conn, "legacy-run");
        assert_eq!(
            legacy.status, "cancelled",
            "旧功能的预留行没有处理器认领，放回队列会一直占着「排队中」"
        );
        assert_eq!(legacy.input_tokens, 4000);
        assert_eq!(snap(&conn, "card-run").status, "queued");
        drop(conn);
        // 预留的用量照旧计入窗口
        assert_eq!(queue_status(db.path()).unwrap().tokens_24h, 4000);
    }

    #[test]
    fn legacy_ledger_rows_use_up_budget_but_are_not_queue_work() {
        let db = TestDb::new("legacy-not-queue");
        let conn = db.conn();
        let now = now_db_time();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, input_tokens, created_at, updated_at)
             VALUES ('legacy-run', 'acct', 'problem_answer', 'query', 'legacy-abc', 'running', 1, 4000, ?1, ?1)",
            [&now],
        )
        .unwrap();
        drop(conn);
        set_daily_budget(&queue_control_path(db.path()), 100_000).unwrap();
        let status = queue_status(db.path()).unwrap();
        // 一次 AI 搜索在途：占额度，但界面不能因此说「正在后台整理」「进行中 1」
        assert_eq!((status.running, status.queued, status.failed), (0, 0, 0));
        assert_eq!(status.tokens_24h, 4000);
        assert_eq!(status.state, "idle");
    }

    #[test]
    fn a_late_recovery_never_turns_a_legacy_reservation_into_a_queued_ghost() {
        let db = TestDb::new("recover-ghost");
        let conn = db.conn();
        let now = now_db_time();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, attempts, input_tokens, created_at, updated_at)
             VALUES ('legacy-run', 'acct', 'problem_answer', 'query', 'legacy-abc', 'running', 1, 4000, ?1, ?1)",
            [&now],
        )
        .unwrap();
        recover_running_jobs(&conn, &now).unwrap();
        let queued: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE status = 'queued'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            queued, 0,
            "旧功能的预留行没有处理器认领，变成排队中就是永远的幽灵行"
        );
        assert_eq!(snap(&conn, "legacy-run").status, "cancelled");
    }

    // —— 全量整理：Fox 亲手确认后，这一轮不看每天额度 ——

    fn full_scan_control(started_at: &str, total: i64) -> QueueControl {
        QueueControl {
            full_scan: Some(FullScan {
                started_at: started_at.to_owned(),
                until: "不重要，只用来显示".to_owned(),
                total,
            }),
            ..QueueControl::default()
        }
    }

    fn unlimited_gate(budget: u64) -> BudgetGate {
        BudgetGate {
            paused: false,
            daily_budget_tokens: budget,
            unlimited: true,
        }
    }

    fn at(seconds_after_now: i64) -> String {
        shift_db_time(NOW, seconds_after_now).unwrap()
    }

    #[test]
    fn a_full_scan_claims_past_the_daily_budget_but_never_past_a_pause() {
        let db = TestDb::new("full-scan-gate");
        let mut conn = db.conn();
        insert_job(&conn, "a", "repo_card", 6, NOW, None, "queued");
        insert_job(&conn, "b", "repo_card", 6, NOW, None, "queued");
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, output_tokens, finished_at, created_at)
             VALUES ('spent', 'acct', 'repo_card', 'repository', 'spent', 'succeeded', 70000, 20000, ?1, ?1)",
            [NOW],
        )
        .unwrap();
        let handled = free_and_spend();
        // 额度 5 万、24 小时已用 9 万：每天额度模式领不到
        assert!(claim_one(&mut conn, &handled, &gate(false, 50_000)).is_none());
        // 全量整理：领得到，而且额度是 0（日常额度关着）也一样
        assert!(claim_one(&mut conn, &handled, &unlimited_gate(50_000)).is_some());
        assert!(claim_one(&mut conn, &handled, &unlimited_gate(0)).is_some());
        // 暂停照样拦
        insert_job(&conn, "c", "repo_card", 6, NOW, None, "queued");
        let paused = BudgetGate {
            paused: true,
            daily_budget_tokens: 50_000,
            unlimited: true,
        };
        assert!(claim_one(&mut conn, &handled, &paused).is_none());
    }

    #[test]
    fn a_full_scan_lasts_48_hours_from_its_start_and_a_broken_record_is_ignored() {
        let control = full_scan_control(NOW, 10);
        assert!(control.full_scan_active(NOW));
        assert!(control.full_scan_active(&at(48 * 3600 - 1)));
        assert!(!control.full_scan_active(&at(48 * 3600)));
        assert!(!control.full_scan_active(&at(-1)), "时钟倒退：按没在进行算");
        // 文件里的 until 只用来显示：改到很远的未来也不会让它一直不限额
        let mut edited = full_scan_control(NOW, 10);
        edited.full_scan.as_mut().unwrap().until = "9999-01-01T00:00:00.000Z".to_owned();
        assert!(!edited.full_scan_active(&at(49 * 3600)));
        // 开始时间读不懂：按没在进行算
        assert!(!full_scan_control("昨天", 10).full_scan_active(NOW));
        assert!(!QueueControl::default().full_scan_active(NOW));

        let db = TestDb::new("full-scan-broken");
        let path = queue_control_path(db.path());
        for text in [
            r#"{"fullScan":"yes"}"#,
            r#"{"fullScan":{"startedAt":5,"until":"x","total":1}}"#,
            r#"{"fullScan":{"startedAt":"2026-10-03T12:00:00.000Z"}}"#,
            r#"{"fullScan":null}"#,
        ] {
            fs::write(&path, text).unwrap();
            assert!(read_control(&path).full_scan.is_none(), "{text}");
        }
        fs::write(
            &path,
            r#"{"dailyBudgetTokens":7,"fullScan":{"startedAt":"2026-10-03T12:00:00.000Z","until":"2026-10-05T12:00:00.000Z","total":3}}"#,
        )
        .unwrap();
        let read = read_control(&path);
        assert_eq!(read.daily_budget_tokens, 7);
        assert_eq!(read.full_scan.as_ref().map(|scan| scan.total), Some(3));
    }

    #[test]
    fn beginning_a_full_scan_resumes_the_queue_keeps_the_rest_and_never_restarts_one_in_progress() {
        let db = TestDb::new("full-scan-begin");
        let path = queue_control_path(db.path());
        fs::write(
            &path,
            r#"{"paused":true,"dailyBudgetTokens":50000,"captureToken":"keep-me","pauseReason":{"kind":"auth","message":"密钥无效","at":"2026-10-03T00:00:00.000Z"}}"#,
        )
        .unwrap();
        let first = begin_full_scan(&path, NOW, 388).unwrap();
        assert_eq!(first.started_at, NOW);
        assert_eq!(first.until, at(48 * 3600));
        assert_eq!(first.total, 388);
        let control = read_control(&path);
        assert!(!control.paused, "点了开始就是要它跑：恢复暂停");
        assert!(control.pause_reason.is_none());
        assert_eq!(control.daily_budget_tokens, 50_000, "日常额度原样保留");
        let raw: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["captureToken"], "keep-me", "控制文件里别的键不能丢");
        // 已经在进行：原样返回，不重置开始时间、不改个数
        let again = begin_full_scan(&path, &at(3600), 5).unwrap();
        assert_eq!(again, first);
        assert_eq!(read_control(&path).full_scan, Some(first));
    }

    fn open_job_row(
        conn: &Connection,
        id: &str,
        status: &str,
        tokens: i64,
        finished: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, finished_at, created_at)
             VALUES (?1, 'acct', 'repo_card', 'repository', ?1, ?2, ?3, ?4, ?5)",
            params![id, status, tokens, finished, NOW],
        )
        .unwrap();
    }

    #[test]
    fn a_full_scan_ends_when_nothing_is_left_and_remembers_how_it_went() {
        let db = TestDb::new("full-scan-complete");
        let path = queue_control_path(db.path());
        begin_full_scan(&path, NOW, 3).unwrap();
        let conn = db.conn();
        open_job_row(&conn, "q", "queued", 0, None);
        // 还有排队的：继续
        settle_full_scan(&conn, &path, &at(60)).unwrap();
        assert!(read_control(&path).full_scan.is_some());
        conn.execute("DELETE FROM ai_jobs WHERE id = 'q'", [])
            .unwrap();
        open_job_row(&conn, "a", "succeeded", 5_000, Some(&at(10)));
        open_job_row(&conn, "b", "succeeded", 6_000, Some(&at(20)));
        open_job_row(&conn, "c", "failed", 1_000, Some(&at(30)));
        // 开始之前就结束的任务不算这一轮的
        open_job_row(&conn, "old", "succeeded", 99_000, Some(&at(-3600)));
        settle_full_scan(&conn, &path, &at(60)).unwrap();
        let control = read_control(&path);
        assert!(control.full_scan.is_none());
        let last = control.last_full_scan.expect("应留下记录");
        assert_eq!(last.reason, "completed");
        assert_eq!(last.tokens_used, 12_000);
        assert_eq!((last.total, last.remaining, last.failed), (3, 0, 1));
        assert_eq!(last.started_at, NOW);
        assert_eq!(last.finished_at, at(60));
        // 再调一次什么也不会变（多个线程同时调也只收尾一次）
        settle_full_scan(&conn, &path, &at(120)).unwrap();
        assert_eq!(read_control(&path).last_full_scan, Some(last));
    }

    #[test]
    fn a_full_scan_expires_after_48_hours_even_with_jobs_left() {
        let db = TestDb::new("full-scan-expire");
        let path = queue_control_path(db.path());
        begin_full_scan(&path, NOW, 9).unwrap();
        let conn = db.conn();
        open_job_row(&conn, "q1", "queued", 0, None);
        open_job_row(&conn, "q2", "queued", 0, None);
        settle_full_scan(&conn, &path, &at(48 * 3600 - 1)).unwrap();
        assert!(read_control(&path).full_scan.is_some());
        settle_full_scan(&conn, &path, &at(48 * 3600)).unwrap();
        let last = read_control(&path).last_full_scan.expect("应留下记录");
        assert_eq!(last.reason, "expired");
        assert_eq!(last.remaining, 2);
    }

    #[test]
    fn a_full_scan_stops_itself_far_beyond_its_estimate() {
        // 开始时只有 1 个要整理：刹车线是至少 300 万。用量超过它说明哪里不对劲，自动结束，而不是一直不限额地花
        assert_eq!(full_scan_ceiling(1), 3_000_000);
        assert_eq!(full_scan_ceiling(0), 3_000_000);
        assert_eq!(full_scan_ceiling(388), 388 * 7_500 * 4);
        let db = TestDb::new("full-scan-ceiling");
        let path = queue_control_path(db.path());
        begin_full_scan(&path, NOW, 1).unwrap();
        let conn = db.conn();
        open_job_row(&conn, "q", "queued", 0, None);
        open_job_row(&conn, "big", "succeeded", 3_000_000, Some(&at(10)));
        let control = read_control(&path);
        assert!(full_scan_open(&conn, &control, &at(20)), "刚好到线还算没超");
        open_job_row(&conn, "more", "failed", 1, Some(&at(15)));
        assert!(!full_scan_open(&conn, &control, &at(20)));
        settle_full_scan(&conn, &path, &at(20)).unwrap();
        let last = read_control(&path).last_full_scan.expect("应留下记录");
        assert_eq!(last.reason, "ceiling");
        assert_eq!(last.remaining, 1, "排队的还在，之后按每天额度");
        assert!(read_control(&path).full_scan.is_none());
    }

    #[test]
    fn stopping_a_full_scan_hands_the_queue_back_to_the_daily_budget() {
        let db = TestDb::new("full-scan-stop");
        let path = queue_control_path(db.path());
        set_daily_budget(&path, 20_000).unwrap();
        begin_full_scan(&path, NOW, 3).unwrap();
        let mut conn = db.conn();
        for id in ["a", "b", "c"] {
            open_job_row(&conn, id, "queued", 0, None);
        }
        open_job_row(&conn, "done", "succeeded", 30_000, Some(&at(5)));
        stop_full_scan(&conn, &path, &at(10)).unwrap();
        let control = read_control(&path);
        assert!(control.full_scan.is_none());
        let last = control.last_full_scan.clone().expect("应留下记录");
        assert_eq!(last.reason, "stopped");
        assert_eq!(last.remaining, 3);
        assert_eq!(last.tokens_used, 30_000);
        assert_eq!(control.daily_budget_tokens, 20_000, "日常额度原样保留");
        // 没有在进行时再停一次：什么也不做，记录不变
        stop_full_scan(&conn, &path, &at(20)).unwrap();
        assert_eq!(read_control(&path).last_full_scan, Some(last));
        // 之后认领按每天额度：已用 3 万超过 2 万，领不到
        let gate = BudgetGate {
            paused: false,
            daily_budget_tokens: control.daily_budget_tokens,
            unlimited: control.full_scan_active(&at(20)),
        };
        assert!(claim_one(&mut conn, &free_and_spend(), &gate).is_none());
    }

    struct BigSpender;

    impl JobHandler for BigSpender {
        fn job_type(&self) -> &'static str {
            "repo_card"
        }
        fn spends_tokens(&self) -> bool {
            true
        }
        fn run(&self, _job: &JobRecord, _db_path: &Path) -> Result<JobSuccess, JobFailure> {
            Ok(JobSuccess {
                input_tokens: 30_000,
                output_tokens: 10_000,
                ..ok_success(false)
            })
        }
    }

    #[test]
    fn a_full_scan_works_through_the_whole_queue_past_the_budget_then_the_budget_is_back() {
        let db = TestDb::new("full-scan-e2e");
        let conn = db.conn();
        for index in 0..5 {
            insert_job(
                &conn,
                &format!("job-{index}"),
                "repo_card",
                6,
                "2026-10-03T10:00:00.000Z",
                None,
                "queued",
            );
        }
        drop(conn);
        let path = queue_control_path(db.path());
        set_daily_budget(&path, 50_000).unwrap();
        let handlers: Vec<Arc<dyn JobHandler>> = vec![Arc::new(BigSpender)];
        let drain = |expected: usize| {
            let mut done = 0;
            while let Some(job) = claim_for_worker(db.path(), &handlers).unwrap() {
                let outcome = run_handler(&handlers, &job, db.path());
                finish_job(db.path(), &job, &now_db_time(), outcome, 0.0).unwrap();
                done += 1;
                assert!(done <= expected + 1, "不该领到这么多");
            }
            done
        };
        // 每天额度模式：领第一个（预留 1.2 万 ≤ 5 万），花 4 万之后第二个领不到
        assert_eq!(drain(1), 1);
        // 全量整理：剩下 4 个一口气做完，用量远超每天额度也不停
        let remaining = count_open_cards(&db.conn()).unwrap();
        assert_eq!(remaining, 4);
        begin_full_scan(&path, &now_db_time(), remaining).unwrap();
        assert_eq!(drain(4), 4);
        let status = queue_status(db.path()).unwrap();
        assert_eq!(status.tokens_24h, 5 * 40_000);
        // 最后一次认领时发现没有要整理的了：自动收尾，留下记录
        let control = read_control(&path);
        assert!(control.full_scan.is_none());
        let last = control.last_full_scan.expect("应留下记录");
        assert_eq!(
            (last.reason.as_str(), last.remaining, last.failed),
            ("completed", 0, 0)
        );
        assert_eq!(last.tokens_used, 4 * 40_000);
        assert!(!status.unlimited, "收尾之后界面读到的是回到日常额度");
        // 新来的任务又按每天额度：已用 20 万，领不到
        insert_job(&db.conn(), "late", "repo_card", 6, NOW, None, "queued");
        assert!(claim_for_worker(db.path(), &handlers).unwrap().is_none());
    }

    #[test]
    fn a_clock_slightly_behind_the_start_does_not_end_the_scan_for_good() {
        // 开始时间比收尾检查的「现在」晚 1 秒（时钟回拨、或检查的 now 是更早取的）：只是暂时不算在进行，不能永久收尾
        let db = TestDb::new("full-scan-skew");
        let path = queue_control_path(db.path());
        begin_full_scan(&path, NOW, 3).unwrap();
        let conn = db.conn();
        open_job_row(&conn, "q", "queued", 0, None);
        settle_full_scan(&conn, &path, &at(-1)).unwrap();
        let control = read_control(&path);
        assert!(control.full_scan.is_some(), "{:?}", control.last_full_scan);
        assert!(!control.full_scan_active(&at(-1)), "时钟倒退期间不算在进行");
        assert!(control.full_scan_active(&at(1)), "时钟走回来之后继续");
    }

    #[test]
    fn a_late_finisher_never_clobbers_a_stop_record_or_a_newer_scan() {
        let db = TestDb::new("full-scan-cas");
        let path = queue_control_path(db.path());
        let first = begin_full_scan(&path, NOW, 3).unwrap();
        let conn = db.conn();
        open_job_row(&conn, "q", "queued", 0, None);
        stop_full_scan(&conn, &path, &at(10)).unwrap();
        let stopped = read_control(&path).last_full_scan.clone();
        // 拿着旧的一轮的收尾者晚到：不能改写已经写好的记录
        finish_full_scan(&path, &first, "completed", 0, 0, 0, &at(20)).unwrap();
        assert_eq!(read_control(&path).last_full_scan, stopped);
        // 新一轮开始之后，旧的一轮的收尾者晚到：不能把新一轮删掉
        let second = begin_full_scan(&path, &at(30), 5).unwrap();
        finish_full_scan(&path, &first, "completed", 0, 0, 0, &at(40)).unwrap();
        assert_eq!(read_control(&path).full_scan, Some(second));
    }

    #[test]
    fn claiming_stops_at_the_ceiling_even_though_the_scan_is_still_inside_its_48_hours() {
        let db = TestDb::new("full-scan-claim-ceiling");
        let conn = db.conn();
        insert_job(
            &conn,
            "a",
            "repo_card",
            6,
            "2026-10-03T10:00:00.000Z",
            None,
            "queued",
        );
        let path = queue_control_path(db.path());
        begin_full_scan(&path, &now_db_time(), 1).unwrap();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, finished_at, created_at)
             VALUES ('big', 'acct', 'repo_card', 'repository', 'big', 'succeeded', 3100000, ?1, ?1)",
            [now_db_time()],
        )
        .unwrap();
        drop(conn);
        let handlers: Vec<Arc<dyn JobHandler>> = vec![Arc::new(BigSpender)];
        assert!(
            claim_for_worker(db.path(), &handlers).unwrap().is_none(),
            "超过刹车线不能再领"
        );
        let last = read_control(&path).last_full_scan.expect("认领前应先收尾");
        assert_eq!(last.reason, "ceiling");
    }

    #[test]
    fn the_reasons_for_ending_a_scan_are_checked_in_a_fixed_order() {
        // 用量超线 > 整理完了 > 失败太多 > 到时间
        let cases: [(&str, i64, i64, &str); 4] = [
            // (名字, 用量, 排队数, 期望)
            ("ceiling", 3_100_000, 0, "ceiling"),
            ("completed-even-if-late", 0, 0, "completed"),
            ("failures", 0, 2, "failures"),
            ("expired", 0, 2, "expired"),
        ];
        for (name, used, queued, expected) in cases {
            let db = TestDb::new(&format!("full-scan-order-{name}"));
            let path = queue_control_path(db.path());
            begin_full_scan(&path, NOW, 1).unwrap();
            let conn = db.conn();
            if used > 0 {
                open_job_row(&conn, "big", "succeeded", used, Some(&at(10)));
            }
            for index in 0..queued {
                open_job_row(&conn, &format!("q{index}"), "queued", 0, None);
            }
            let mut settle_at = at(60);
            if name == "failures" {
                for index in 0..25 {
                    open_job_row(&conn, &format!("f{index}"), "failed", 10, Some(&at(20)));
                }
            }
            if name == "expired" || name == "completed-even-if-late" {
                settle_at = at(48 * 3600 + 5);
            }
            settle_full_scan(&conn, &path, &settle_at).unwrap();
            let last = read_control(&path).last_full_scan.expect("应收尾");
            assert_eq!(last.reason, expected, "{name}");
        }
    }

    #[test]
    fn too_many_failures_end_the_scan_instead_of_burning_money() {
        // 预计 388 个：失败达到 max(25, 388/5 = 77) 就停；差一个不停
        assert_eq!(full_scan_failure_limit(388), 77);
        assert_eq!(full_scan_failure_limit(10), 25);
        let db = TestDb::new("full-scan-failures");
        let path = queue_control_path(db.path());
        begin_full_scan(&path, NOW, 388).unwrap();
        let conn = db.conn();
        open_job_row(&conn, "q", "queued", 0, None);
        for index in 0..76 {
            open_job_row(&conn, &format!("f{index}"), "failed", 5_000, Some(&at(10)));
        }
        settle_full_scan(&conn, &path, &at(60)).unwrap();
        assert!(read_control(&path).full_scan.is_some(), "76 个失败还没到线");
        open_job_row(&conn, "f76", "failed", 5_000, Some(&at(11)));
        settle_full_scan(&conn, &path, &at(61)).unwrap();
        let last = read_control(&path).last_full_scan.expect("应收尾");
        assert_eq!(last.reason, "failures");
        assert_eq!((last.failed, last.remaining), (77, 1));
        // 失败只数这一轮开始之后的
        let db = TestDb::new("full-scan-failed-since");
        let conn = db.conn();
        open_job_row(&conn, "old", "failed", 100, Some(&at(-3600)));
        open_job_row(&conn, "new", "failed", 100, Some(&at(10)));
        assert_eq!(count_failed_cards_since(&conn, NOW).unwrap(), 1);
    }

    #[test]
    fn a_scan_where_everything_failed_still_ends_as_completed_with_the_failures_counted() {
        let db = TestDb::new("full-scan-all-failed");
        let path = queue_control_path(db.path());
        begin_full_scan(&path, NOW, 3).unwrap();
        let conn = db.conn();
        for id in ["a", "b", "c"] {
            open_job_row(&conn, id, "failed", 10_000, Some(&at(10)));
        }
        settle_full_scan(&conn, &path, &at(60)).unwrap();
        let last = read_control(&path).last_full_scan.expect("应收尾");
        assert_eq!(
            (last.reason.as_str(), last.failed, last.remaining),
            ("completed", 3, 0)
        );
    }

    #[test]
    fn switching_to_another_database_ends_the_scan_and_zeroes_the_daily_budget() {
        // 恢复备份 / 清空本地库：用量记录跟着库变了，授权要一并归零，不然旧库里排队的任务在剩下的 48 小时里继续不限额
        let db = TestDb::new("full-scan-reset");
        let conn = db.conn();
        insert_job(
            &conn,
            "a",
            "repo_card",
            6,
            "2026-10-03T10:00:00.000Z",
            None,
            "queued",
        );
        drop(conn);
        let path = queue_control_path(db.path());
        set_daily_budget(&path, 100_000).unwrap();
        begin_full_scan(&path, &now_db_time(), 1).unwrap();
        let mut raw: Map<String, Value> = load_object(&path).unwrap();
        raw.insert("captureToken".to_owned(), Value::from("keep-me"));
        write_object(&path, &raw).unwrap();
        reset_spending_authorization(&path).unwrap();
        let control = read_control(&path);
        assert_eq!(control.daily_budget_tokens, 0);
        assert!(control.full_scan.is_none() && control.last_full_scan.is_none());
        assert_eq!(load_object(&path).unwrap()["captureToken"], "keep-me");
        let handlers: Vec<Arc<dyn JobHandler>> = vec![Arc::new(BigSpender)];
        assert!(claim_for_worker(db.path(), &handlers).unwrap().is_none());
    }

    #[test]
    fn queue_status_says_unlimited_during_a_full_scan_and_never_budget_reached() {
        let db = TestDb::new("full-scan-state");
        let path = queue_control_path(db.path());
        set_daily_budget(&path, 20_000).unwrap();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, input_tokens, finished_at, created_at)
             VALUES ('spent', 'acct', 'repo_card', 'repository', 'spent', 'succeeded', 90000, ?1, ?1)",
            [now_db_time()],
        )
        .unwrap();
        drop(conn);
        assert_eq!(queue_status(db.path()).unwrap().state, "budget_reached");
        begin_full_scan(&path, &now_db_time(), 5).unwrap();
        let status = queue_status(db.path()).unwrap();
        assert!(status.unlimited);
        assert_eq!(status.state, "idle");
    }

    #[test]
    fn a_recovery_that_runs_while_a_legacy_request_is_in_flight_leaves_a_consistent_ledger() {
        // 真实顺序：旧功能的请求先预留 → 启动恢复（比如第一次打不开库、之后才成功）跑了一遍 → 请求结束记账。
        // 不能留下排队中 / 进行中的幽灵行，也不能少记：窗口里的用量至少是真实用量
        let dir = crate::spend_guard::testing::Dir::new();
        dir.control(r#"{"dailyBudgetTokens":100000,"paused":false}"#);
        let _scope = dir.use_in_this_thread();
        let prompt = "说".repeat(1000);
        let permit = crate::spend_guard::check_before_request(&prompt).expect("应通过额度检查");
        let conn = open_db(dir.path()).unwrap();
        recover_running_jobs(&conn, &now_db_time()).unwrap();
        // 真实用量（1.5 万）比预留（4000）还大：恢复动作取消了预留行，之后的记账一定要另记一条，不能丢
        crate::spend_guard::record_after_request(
            "openai",
            "m",
            &prompt,
            "答",
            Some((9_000, 6_000)),
        );
        drop(permit);
        let open: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE status IN ('queued', 'running')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(open, 0, "不能留下排队中或一直进行中的幽灵行");
        drop(conn);
        let status = queue_status(dir.path()).unwrap();
        assert_eq!((status.running, status.queued), (0, 0));
        assert!(
            status.tokens_24h >= 15_000,
            "真实用量 1.5 万一定要在窗口里，实际 {}",
            status.tokens_24h
        );
    }
}
