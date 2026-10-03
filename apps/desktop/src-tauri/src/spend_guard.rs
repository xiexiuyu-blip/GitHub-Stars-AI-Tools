//! 旧版 AI 功能和新版队列共用同一个每日额度。
//!
//! 队列（`jobs.rs`）在认领时就把预留记进数据库，额度是它管的。但旧版功能——整批解析、单个仓库的 AI 摘要、
//! 标签网络、AI 搜索、翻译、推荐——不走队列，以前完全不看额度：顶栏「增量 AI 解析」点一下，
//! 495 个仓库约 160 万到 190 万 token，是 10 万额度的十几倍，而且没有取消按钮，也不记进「最近 24 小时已用」。
//!
//! 这里给它们补同一道闸，守在向模型服务发请求的地方，做法和队列认领一样：
//! - 请求前（`check_before_request`）：在一个 `BEGIN IMMEDIATE` 里查「额度是 0 / 队列暂停 / 24 小时已用 + 本次预计 > 额度」，
//!   通过就**同一个事务里**往账本写一条 `running` 的预留行（用量 = 本次预计）。队列认领看得见这条预留，
//!   所以旧功能的请求在途时，队列不会再认领出去把额度撑破；两个旧功能的请求同时来，也只有放得下的那个过得去；
//! - 请求后（`record_after_request`）：把预留行换成真实用量（服务商回的真实用量；流式没有就按文字估，中文按 1 字 1 token）；
//! - 没记账就结束（出错、panic）：预留行记成已取消，只留「提示词那部分」的估计用量（请求发出去了才可能计费，输出那部分没有）。
//!   程序崩溃时留下的预留行，下次启动由 `jobs::recover_running_jobs` 记成已取消，用量照旧计入窗口。
//!
//! 额度、暂停都用队列的控制文件（工具箱里「AI 自动整理」的设置），所以只有一处开关。
//! 没装闸（单元测试）时什么都不拦；正式运行时没装上就一律拒绝，宁可旧功能暂时不能用，也不在没有额度保护时花钱。
//! 连接测试和自检是一次很小的调用，用 `probe` 包起来：不看额度、不被暂停拦住，但仍然记账。

use crate::jobs;
use rusqlite::{params, Connection, TransactionBehavior};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

/// 旧功能没有给输出设 `max_tokens`，输出长度不可控；按一个宽松的值估：至少 3000，翻译这类输出和输入一样长的取输入的长度。
/// 一篇 README 摘要实测输出约 1k，标签网络一批可达几千。
const MIN_OUTPUT_ALLOWANCE_TOKENS: u64 = 3_000;

/// 单次调用记进账本的上限（每一列）。服务商回一个离谱的用量（几十亿）会让账本求和溢出，
/// 「最近 24 小时已用」读不出来，队列和旧功能都被锁 24 小时。
const MAX_RECORDED_TOKENS: u64 = 5_000_000;

/// 一次调用的真实用量超过这个数就不信（比最大的模型上下文窗口还大）：
/// 一个离谱的 `usage` 会把当天额度整个占满，改按文字估。
const MAX_PLAUSIBLE_CALL_TOKENS: u64 = 300_000;

/// 拒绝时的前缀。调用方用 `is_refusal` 认它。
pub const REFUSAL_PREFIX: &str = "AI 额度：";

static DATABASE: OnceLock<PathBuf> = OnceLock::new();

/// 这个线程当前这次请求在账本里的预留行。
struct Reservation {
    database: PathBuf,
    id: String,
    /// 提示词那部分的估计用量：请求没有记账就结束时，只留这么多。
    input_estimate: u64,
    /// 预留的总量（提示词 + 输出余量）。流式回复没有真实用量时，记账不能比它还少。
    reserved_total: u64,
}

thread_local! {
    static PROBE_DEPTH: Cell<u32> = const { Cell::new(0) };
    static PERMIT_DEPTH: Cell<u32> = const { Cell::new(0) };
    static RESERVATION: RefCell<Option<Reservation>> = const { RefCell::new(None) };
}

/// 启动时装上。只能装一次；数据库路径之后不变。
pub fn install(database_path: &Path) {
    let _ = DATABASE.set(database_path.to_path_buf());
}

pub fn is_refusal(message: &str) -> bool {
    message.starts_with(REFUSAL_PREFIX)
}

struct ProbeScope;

impl Drop for ProbeScope {
    fn drop(&mut self) {
        PROBE_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// 连接测试、运行自检这类一次很小的调用：包在里面的请求不看额度、不被暂停拦住，但仍然记账。
pub fn probe<R>(run: impl FnOnce() -> R) -> R {
    PROBE_DEPTH.with(|depth| depth.set(depth.get() + 1));
    let _scope = ProbeScope;
    run()
}

fn probing() -> bool {
    PROBE_DEPTH.with(|depth| depth.get() > 0)
}

/// 通过了额度检查的凭证，拿着它直到请求结束并记完账。同一个线程里可以嵌套
/// （流式请求失败后会在里面再发一次普通请求）：只有最外层真的在账本里预留，也只有它在结束时收尾。
///
/// 它不能跨线程（`!Send`）：嵌套深度和预留行都记在线程本地，凭证在别的线程上放掉，
/// 原来那个线程的深度就永远是 1，之后的请求会被当成「嵌套」而不检查额度。
#[must_use = "凭证要一直拿到请求结束并记完账；写成 `check_before_request(..)?;` 会立刻放掉，预留当场被取消"]
pub struct SpendPermit {
    outermost: bool,
    _not_send: PhantomData<*const ()>,
}

impl Drop for SpendPermit {
    fn drop(&mut self) {
        PERMIT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        if self.outermost {
            // 到这里还在，说明这次请求没有记账就结束了（出错、panic）
            if let Some(reservation) = RESERVATION.with(|slot| slot.borrow_mut().take()) {
                if let Err(error) = cancel_reservation(&reservation, &jobs::now_db_time()) {
                    eprintln!("旧版 AI 的预留没有收尾：{error}");
                }
            }
        }
    }
}

pub fn check_before_request(prompt: &str) -> Result<SpendPermit, String> {
    begin(DATABASE.get().map(PathBuf::as_path), prompt)
}

fn begin(database: Option<&Path>, prompt: &str) -> Result<SpendPermit, String> {
    let outermost = PERMIT_DEPTH.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        current == 0
    });
    // 提前返回时这个凭证被放掉：深度减回去，最外层没有预留行可收
    let permit = SpendPermit {
        outermost,
        _not_send: PhantomData,
    };
    if !outermost {
        return Ok(permit);
    }
    let Some(database) = database else {
        // 没装闸：单元测试里不拦；正式运行时拒绝
        if cfg!(test) {
            return Ok(permit);
        }
        return Err(format!(
            "{REFUSAL_PREFIX}额度闸门还没有准备好，旧版的 AI 功能先不能用。重新打开应用再试。"
        ));
    };
    if probing() {
        return Ok(permit);
    }
    let input = estimate_tokens(prompt);
    let estimated = input.saturating_add(MIN_OUTPUT_ALLOWANCE_TOKENS.max(input));
    let id = reserve_at(database, &jobs::now_db_time(), estimated)?;
    RESERVATION.with(|slot| {
        *slot.borrow_mut() = Some(Reservation {
            database: database.to_path_buf(),
            id,
            input_estimate: input,
            reserved_total: estimated,
        });
    });
    Ok(permit)
}

/// 请求被服务商直接拒绝（4xx：密钥 / 模型 / 额度 / 参数不对）或根本没连上：没有计费，把预留整个还回去，不留任何用量。
/// 不调用的话，预留行会在凭证放掉时留下提示词那部分的估计用量——一把错的密钥、一个配错的模型，
/// 整批解析连失败 20 多次，就在账本里记下 9 万多 token、什么也没花，把队列和所有旧功能锁 24 小时。
pub fn release_unbilled() {
    if let Some(reservation) = RESERVATION.with(|slot| slot.borrow_mut().take()) {
        if let Err(error) = release_reservation(&reservation, &jobs::now_db_time()) {
            eprintln!("旧版 AI 的预留没有退回：{error}");
        }
    }
}

/// `usage` 是服务商回的真实用量（输入，输出）；没有、或不可信就按文字估。
pub fn record_after_request(
    provider: &str,
    model: &str,
    prompt: &str,
    content: &str,
    usage: Option<(u64, u64)>,
) {
    record(
        DATABASE.get().map(PathBuf::as_path),
        provider,
        model,
        prompt,
        content,
        usage,
    );
}

fn record(
    database: Option<&Path>,
    provider: &str,
    model: &str,
    prompt: &str,
    content: &str,
    usage: Option<(u64, u64)>,
) {
    let Some(database) = database else {
        return;
    };
    let real_usage = usage.filter(|(input, output)| {
        input.saturating_add(*output) <= MAX_PLAUSIBLE_CALL_TOKENS
            && input.saturating_add(*output) > 0
    });
    let estimate = (estimate_tokens(prompt), estimate_tokens(content));
    let now = jobs::now_db_time();
    let reservation = RESERVATION.with(|slot| slot.borrow_mut().take());
    let result = match reservation {
        Some(reservation) if reservation.database == database => {
            // 流式回复里没有用量；推理模型的思考 token 又不在文字里，按文字估会比真实少得多：
            // 这种情况记账不能比预留还少（预留本来就是「提示词 + 输出余量」的保守估计）。
            let (input, output) = match real_usage {
                Some(pair) => pair,
                None => {
                    let total = estimate
                        .0
                        .saturating_add(estimate.1)
                        .max(reservation.reserved_total);
                    (estimate.0, total.saturating_sub(estimate.0))
                }
            };
            let mut outcome = Err(String::new());
            for attempt in 0..3 {
                outcome = settle_reservation(&reservation, &now, provider, model, input, output);
                if outcome.is_ok() {
                    break;
                }
                if attempt < 2 {
                    thread::sleep(Duration::from_millis(300));
                }
            }
            match outcome {
                Ok(true) => Ok(()),
                // 预留行已经不在 running（被恢复动作取消了）：退回去另记一条
                Ok(false) => record_at(database, &now, provider, model, input, output),
                Err(error) => {
                    // 记不上：把预留放回去，凭证放掉时会收尾成「已取消 + 提示词那部分」，不会留下一条永远 running 的行
                    RESERVATION.with(|slot| *slot.borrow_mut() = Some(reservation));
                    Err(error)
                }
            }
        }
        _ => {
            let (input, output) = real_usage.unwrap_or(estimate);
            record_at(database, &now, provider, model, input, output)
        }
    };
    if let Err(error) = result {
        // 记不上账不能悄悄吞掉：至少留一条日志（开发版在终端里能看到）。预留行还在，仍占着额度。
        eprintln!("旧版 AI 用量没有记上：{error}");
    }
}

/// 估算 token：中日韩文字约 1 个字 1 个 token，别的字符约 4 个字符 1 个 token。宁多勿少。
pub fn estimate_tokens(text: &str) -> u64 {
    let mut wide = 0_u64;
    let mut narrow = 0_u64;
    for ch in text.chars() {
        if is_wide(ch) {
            wide += 1;
        } else {
            narrow += 1;
        }
    }
    wide + narrow.div_ceil(4)
}

fn is_wide(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3000..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xFF00..=0xFFEF
    )
}

/// 额度的规矩（纯函数）：暂停、额度是 0、放不下。
fn refusal_for(control: &jobs::QueueControl, used: u64, estimated: u64) -> Result<(), String> {
    if control.paused {
        return Err(format!(
            "{REFUSAL_PREFIX}AI 整理现在是暂停状态，旧版的 AI 功能也一起停着。要继续请先在工具箱里点「继续整理」。"
        ));
    }
    if control.daily_budget_tokens == 0 {
        return Err(format!(
            "{REFUSAL_PREFIX}每天的 AI 额度是 0，所有 AI 功能（包括旧版的整批解析、标签网络、AI 搜索）都还不能用。要用请先在工具箱的「AI 自动整理」里开启额度。"
        ));
    }
    if used.saturating_add(estimated) > control.daily_budget_tokens {
        return Err(format!(
            "{REFUSAL_PREFIX}最近 24 小时已经用了约 {}，这一次预计还要约 {}，会超过每天 {} 的额度，所以没有发请求。过一阵额度恢复后再试，或在工具箱里调高额度。",
            readable_tokens(used),
            readable_tokens(estimated),
            readable_tokens(control.daily_budget_tokens),
        ));
    }
    Ok(())
}

/// 在一个 `BEGIN IMMEDIATE` 里查额度并写一条 `running` 的预留行，返回这一行的 id。
/// 队列认领（`jobs::claim_next`）也是在 `BEGIN IMMEDIATE` 里算用量，后一个一定看得见前一个的预留。
fn reserve_at(database_path: &Path, now: &str, estimated: u64) -> Result<String, String> {
    let control = jobs::read_control(&jobs::queue_control_path(database_path));
    let refuse = |error: String| format!("{REFUSAL_PREFIX}额度检查失败：{error}");
    let mut connection = jobs::open_db(database_path).map_err(refuse)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| refuse(error.to_string()))?;
    let cutoff = jobs::shift_db_time(now, -jobs::DAY_SECS).map_err(refuse)?;
    let used = jobs::tokens_used_since(&transaction, &cutoff)
        .map_err(refuse)?
        .max(0) as u64;
    refusal_for(&control, used, estimated)?;
    let id: String = transaction
        .query_row("SELECT lower(hex(randomblob(8)))", [], |row| row.get(0))
        .map_err(|error| format!("{REFUSAL_PREFIX}额度预留失败：{error}"))?;
    let reserved = i64::try_from(estimated.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    let inserted = transaction
        .execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, attempts, max_attempts,
                                 prompt_version, input_tokens, output_tokens, created_at, started_at, updated_at)
             SELECT ?1, github_accounts.id, 'problem_answer', 'query', 'legacy-' || ?1, 9, 'running', 1, 1,
                    'legacy', ?2, 0, ?3, ?3, ?3
             FROM github_accounts
             ORDER BY github_accounts.updated_at DESC
             LIMIT 1",
            params![id, reserved, now],
        )
        .map_err(|error| format!("{REFUSAL_PREFIX}额度预留失败：{error}"))?;
    if inserted == 0 {
        return Err(format!(
            "{REFUSAL_PREFIX}还没有连接 GitHub 账号，用量没法记账，所以旧版的 AI 功能先不能用。"
        ));
    }
    transaction
        .commit()
        .map_err(|error| format!("{REFUSAL_PREFIX}额度预留失败：{error}"))?;
    Ok(id)
}

/// 预留行换成真实用量。返回这一行是不是真的还在 running。
fn settle_reservation(
    reservation: &Reservation,
    now: &str,
    provider: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> Result<bool, String> {
    let connection = jobs::open_db(&reservation.database)?;
    let input = i64::try_from(input_tokens.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    let output = i64::try_from(output_tokens.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    let changed = connection
        .execute(
            "UPDATE ai_jobs
             SET status = 'succeeded', provider = ?1, model = ?2, input_tokens = ?3, output_tokens = ?4,
                 finished_at = ?5, updated_at = ?5
             WHERE id = ?6 AND status = 'running'",
            params![provider, model, input, output, now, reservation.id],
        )
        .map_err(|error| format!("用量记账失败：{error}"))?;
    Ok(changed > 0)
}

/// 没有计费：预留整个还回去（记成已取消、用量清零）。
fn release_reservation(reservation: &Reservation, now: &str) -> Result<(), String> {
    let connection = jobs::open_db(&reservation.database)?;
    connection
        .execute(
            "UPDATE ai_jobs
             SET status = 'cancelled', input_tokens = 0, output_tokens = 0, finished_at = ?1, updated_at = ?1
             WHERE id = ?2 AND status = 'running'",
            params![now, reservation.id],
        )
        .map_err(|error| format!("预留退回失败：{error}"))?;
    Ok(())
}

/// 没记账就结束：记成已取消，只留提示词那部分的估计用量。
fn cancel_reservation(reservation: &Reservation, now: &str) -> Result<(), String> {
    let connection = jobs::open_db(&reservation.database)?;
    let kept = i64::try_from(reservation.input_estimate.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    connection
        .execute(
            "UPDATE ai_jobs
             SET status = 'cancelled', input_tokens = ?1, output_tokens = 0, finished_at = ?2, updated_at = ?2
             WHERE id = ?3 AND status = 'running'",
            params![kept, now, reservation.id],
        )
        .map_err(|error| format!("预留收尾失败：{error}"))?;
    Ok(())
}

/// 没有预留行可换时（连接测试、自检，或预留行被恢复动作取消了）：直接记成一条已完成的任务。
/// `problem_answer` 这个类型队列从不认领，`target_id` 每次不同，不会撞唯一索引。
fn record_at(
    database_path: &Path,
    now: &str,
    provider: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> Result<(), String> {
    let connection = jobs::open_db(database_path)?;
    insert_ledger_row(
        &connection,
        now,
        provider,
        model,
        input_tokens,
        output_tokens,
    )
}

fn insert_ledger_row(
    connection: &Connection,
    now: &str,
    provider: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
) -> Result<(), String> {
    let input = i64::try_from(input_tokens.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    let output = i64::try_from(output_tokens.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    let inserted = connection
        .execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, attempts, max_attempts,
                                 provider, model, prompt_version, input_tokens, output_tokens, created_at, started_at, finished_at, updated_at)
             SELECT lower(hex(randomblob(8))), github_accounts.id, 'problem_answer', 'query', 'legacy-' || lower(hex(randomblob(8))),
                    9, 'succeeded', 1, 1, ?1, ?2, 'legacy', ?3, ?4, ?5, ?5, ?5, ?5
             FROM github_accounts
             ORDER BY github_accounts.updated_at DESC
             LIMIT 1",
            params![provider, model, input, output, now],
        )
        .map_err(|error| format!("用量记账失败：{error}"))?;
    if inserted == 0 {
        return Err("还没有 GitHub 账号，这笔用量记不到任何人名下".to_owned());
    }
    Ok(())
}

/// 一万以上写成「x.x 万」，不到一万写成整数。
fn readable_tokens(tokens: u64) -> String {
    if tokens >= 10_000 {
        let tenths = (tokens + 500) / 1_000;
        format!("{}.{} 万 token", tenths / 10, tenths % 10)
    } else {
        format!("{tokens} token")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fsl-spend-guard-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            let database = path.join("fox-stars-lab.sqlite3");
            // 用真的迁移建库：CHECK 约束、外键、唯一索引都和正式库一样
            crate::db::migrations::migrate_database(&database).unwrap();
            let connection = Connection::open(&database).unwrap();
            connection
                .execute_batch(
                    "INSERT INTO github_accounts(id, login, token_ref, updated_at) VALUES
                       ('old', 'olduser', 'test', '2020-01-01T00:00:00.000Z'),
                       ('acct', 'fox', 'test', '2026-10-01T00:00:00.000Z');",
                )
                .unwrap();
            Self(database)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn control(&self, text: &str) {
            fs::write(jobs::queue_control_path(&self.0), text).unwrap();
        }

        fn used(&self) -> i64 {
            jobs::tokens_used_in_last_day(&self.0, &jobs::now_db_time()).unwrap()
        }

        fn row(&self, sql: &str) -> (String, i64, i64) {
            Connection::open(&self.0)
                .unwrap()
                .query_row(sql, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            if let Some(parent) = self.0.parent() {
                let _ = fs::remove_dir_all(parent);
            }
        }
    }

    const BUDGET_100K: &str = r#"{"dailyBudgetTokens":100000,"paused":false}"#;
    const LEGACY_ROW: &str =
        "SELECT status, input_tokens, output_tokens FROM ai_jobs WHERE target_id LIKE 'legacy-%'";
    const LEGACY_ROW_LATEST: &str = "SELECT status, input_tokens, output_tokens FROM ai_jobs WHERE target_id LIKE 'legacy-%' ORDER BY created_at DESC, rowid DESC LIMIT 1";

    #[test]
    fn nothing_is_blocked_without_a_guard_installed() {
        // 单元测试里没有 install：旧代码的测试不该被额度拦住
        let permit = check_before_request("任意提示词");
        assert!(permit.is_ok());
        drop(permit);
        record_after_request("openai", "m", "p", "c", None);
    }

    #[test]
    fn zero_budget_refuses_with_a_plain_message() {
        let dir = Dir::new();
        let error = begin(Some(dir.path()), "提示词")
            .err()
            .expect("额度是 0 应拒绝");
        assert!(is_refusal(&error), "{error}");
        assert!(error.contains("额度是 0"), "{error}");
        assert_eq!(dir.used(), 0, "被拒绝不能留下预留");
    }

    #[test]
    fn paused_queue_refuses_even_with_budget() {
        let dir = Dir::new();
        dir.control(r#"{"dailyBudgetTokens":100000,"paused":true}"#);
        let error = begin(Some(dir.path()), "提示词").err().expect("暂停应拒绝");
        assert!(is_refusal(&error));
        assert!(error.contains("暂停"), "{error}");
    }

    #[test]
    fn a_reservation_is_written_and_the_queue_can_see_it() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        // 1000 个汉字 = 1000 输入 + max(3000, 1000) 输出余量 = 4000
        assert_eq!(dir.used(), 4000);
        let status = jobs::queue_status(dir.path()).unwrap();
        // 旧功能的预留行占额度（队列认领看得见），但不算队列任务：界面不该因此说「正在后台整理」
        assert_eq!(
            (status.running, status.queued, status.tokens_24h),
            (0, 0, 4000)
        );
        drop(permit);
    }

    #[test]
    fn two_requests_cannot_both_fit_when_only_one_does() {
        // 修复前「问额度」和「记账」是两步，两个请求同时问到「还有 10 万」、各花 6 万就是 12 万。
        // 要真正证明是 BEGIN IMMEDIATE 在串行化：输的一方必须是被正式拒绝（看到了对方的预留），
        // 而不是撞上 database is locked（用延迟事务也会「恰好一个成功」，但输的一方报的是锁错误）。循环多次，避免碰巧。
        for round in 0..8 {
            let dir = Dir::new();
            dir.control(BUDGET_100K);
            let now = jobs::now_db_time();
            let barrier = Arc::new(Barrier::new(2));
            let path = dir.path().to_path_buf();
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let barrier = Arc::clone(&barrier);
                    let path = path.clone();
                    let now = now.clone();
                    thread::spawn(move || {
                        barrier.wait();
                        reserve_at(&path, &now, 60_000)
                    })
                })
                .collect();
            let outcomes: Vec<Result<String, String>> = handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();
            let wins = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
            assert_eq!(wins, 1, "第 {round} 轮：{outcomes:?}");
            let loser = outcomes
                .iter()
                .find_map(|outcome| outcome.as_ref().err())
                .unwrap();
            assert!(
                loser.contains("所以没有发请求"),
                "第 {round} 轮：输的一方应该是被正式拒绝，不是锁错误：{loser}"
            );
            assert_eq!(dir.used(), 60_000);
        }
    }

    #[test]
    fn headroom_decides_and_the_message_says_why() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let now = jobs::now_db_time();
        reserve_at(dir.path(), &now, 60_000).unwrap();
        reserve_at(dir.path(), &now, 40_000).unwrap();
        let error = reserve_at(dir.path(), &now, 1).unwrap_err();
        assert!(is_refusal(&error));
        assert!(error.contains("10.0 万"), "{error}");
        assert!(error.contains("所以没有发请求"), "{error}");
    }

    #[test]
    fn recording_replaces_the_estimate_with_the_real_usage() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(500)).expect("应通过");
        assert_eq!(dir.used(), 500 + 3000);
        record(
            Some(dir.path()),
            "openai",
            "gpt",
            "p",
            "c",
            Some((1200, 345)),
        );
        assert_eq!(dir.used(), 1545, "预估换成真实用量，不叠加");
        let (status, input, output) = dir.row(LEGACY_ROW);
        assert_eq!((status.as_str(), input, output), ("succeeded", 1200, 345));
        // 结束时预留行已经换掉了，放掉凭证不能再改它
        drop(permit);
        assert_eq!(dir.used(), 1545);
    }

    #[test]
    fn a_request_that_never_records_keeps_only_the_prompt_estimate() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(2000)).expect("应通过");
        assert_eq!(dir.used(), 2000 + 3000);
        drop(permit);
        // 出错 / panic：请求发出去了才可能计费，输出那部分没有，只留提示词那部分
        assert_eq!(dir.used(), 2000);
        let (status, _, _) = dir.row(LEGACY_ROW);
        assert_eq!(status, "cancelled");
    }

    #[test]
    fn nested_permits_reserve_once_and_the_fallback_settles_the_outer_row() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let outer = begin(Some(dir.path()), "外层").expect("应通过");
        let inner =
            begin(Some(dir.path()), "内层（流式失败后的普通请求）").expect("嵌套不能被拒绝");
        let rows: i64 = Connection::open(dir.path())
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE target_id LIKE 'legacy-%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1, "只有最外层预留");
        record(Some(dir.path()), "openai", "m", "p", "c", Some((100, 20)));
        drop(inner);
        drop(outer);
        assert_eq!(dir.used(), 120);
    }

    #[test]
    fn probe_skips_the_check_but_still_records() {
        let dir = Dir::new();
        // 额度是 0，探测照样放行；用量照样记账（没有预留行，直接记一条）
        let permit = probe(|| begin(Some(dir.path()), "测试连接")).expect("探测不看额度");
        record(
            Some(dir.path()),
            "openai",
            "m",
            "测试连接",
            "OK",
            Some((50, 5)),
        );
        drop(permit);
        assert_eq!(dir.used(), 55);
        // 探测之外仍然拦
        assert!(begin(Some(dir.path()), "别的请求").is_err());
        assert!(!probing());
    }

    #[test]
    fn an_implausible_usage_is_replaced_by_the_estimate() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let _permit = begin(Some(dir.path()), &"说".repeat(100)).expect("应通过");
        // 服务商回了一个离谱的用量（几十亿）：不能让它把当天额度整个占满，改按文字估；
        // 文字只有 150，但有预留在（100 + 3000 = 3100），记账不低于预留
        record(
            Some(dir.path()),
            "openai",
            "m",
            &"说".repeat(100),
            &"答".repeat(50),
            Some((4_000_000_000, 9)),
        );
        assert_eq!(dir.used(), 3100);
    }

    #[test]
    fn without_any_account_the_request_is_refused_and_recording_errors() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        Connection::open(dir.path())
            .unwrap()
            .execute("DELETE FROM github_accounts", [])
            .unwrap();
        let error = begin(Some(dir.path()), "提示词")
            .err()
            .expect("没有账号应拒绝");
        assert!(is_refusal(&error));
        assert!(error.contains("账号"), "{error}");
        let error =
            record_at(dir.path(), &jobs::now_db_time(), "openai", "m", 100, 10).unwrap_err();
        assert!(error.contains("账号"), "{error}");
    }

    #[test]
    fn absurd_usage_is_clamped_so_the_ledger_sum_cannot_overflow() {
        let dir = Dir::new();
        dir.control(r#"{"dailyBudgetTokens":100000}"#);
        for _ in 0..3 {
            record_at(
                dir.path(),
                &jobs::now_db_time(),
                "openai",
                "m",
                u64::MAX,
                u64::MAX,
            )
            .unwrap();
        }
        assert_eq!(dir.used(), 3 * 2 * MAX_RECORDED_TOKENS as i64);
    }

    #[test]
    fn a_crash_leaves_a_reservation_that_still_counts_in_the_window() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        // 模拟崩溃：不走 Drop，预留行留在 running（用量照旧计入窗口，队列认领看得见它）
        std::mem::forget(permit);
        RESERVATION.with(|slot| slot.borrow_mut().take());
        PERMIT_DEPTH.with(|depth| depth.set(0));
        assert_eq!(dir.used(), 4000);
        let (status, _, _) = dir.row(LEGACY_ROW);
        assert_eq!(status, "running");
    }

    #[test]
    fn estimate_counts_chinese_as_one_token_per_character() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("中文说明"), 4);
        assert_eq!(estimate_tokens("中文 ab"), 2 + 1);
        // 旧估法是字符数 / 4：一段 1000 字的中文只估 250，低估四倍
        let chinese: String = "说".repeat(1000);
        assert_eq!(estimate_tokens(&chinese), 1000);
    }

    #[test]
    fn readable_tokens_are_short_and_plain() {
        assert_eq!(readable_tokens(0), "0 token");
        assert_eq!(readable_tokens(9_999), "9999 token");
        assert_eq!(readable_tokens(10_000), "1.0 万 token");
        assert_eq!(readable_tokens(138_651), "13.9 万 token");
        assert_eq!(readable_tokens(100_000), "10.0 万 token");
    }

    #[test]
    fn a_rejected_request_gives_the_whole_reservation_back() {
        // 一把错的密钥、一个配错的模型：整批解析连失败 20 多次，不能在账本里留下 9 万多 token、什么也没花
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        for _ in 0..30 {
            let permit = begin(Some(dir.path()), &"说".repeat(2000)).expect("每次都应通过");
            release_unbilled();
            drop(permit);
        }
        assert_eq!(dir.used(), 0, "被拒绝的请求没有计费，一分都不该留");
        let (status, input, output) = dir.row(LEGACY_ROW_LATEST);
        assert_eq!((status.as_str(), input, output), ("cancelled", 0, 0));
        // 没退回的对照（超时 / 5xx，不确定是否计费）：留提示词那部分
        let permit = begin(Some(dir.path()), &"说".repeat(2000)).expect("应通过");
        drop(permit);
        assert_eq!(dir.used(), 2000);
    }

    #[test]
    fn a_stream_without_usage_never_settles_below_the_reservation() {
        // 流式回复没有用量，推理模型的思考 token 又不在文字里：只按文字估会比真实少得多，
        // 甚至把预留压低（6k 预留结算成 1.5k）。这种情况记账不能比预留少。
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        assert_eq!(dir.used(), 4000);
        record(
            Some(dir.path()),
            "openai",
            "m",
            &"说".repeat(1000),
            "短",
            None,
        );
        assert_eq!(dir.used(), 4000, "没有用量：不低于预留 4000");
        drop(permit);
        // 文字估得比预留还多（输出很长）：按文字估
        let permit = begin(Some(dir.path()), "短").expect("应通过");
        record(
            Some(dir.path()),
            "openai",
            "m",
            "短",
            &"答".repeat(9000),
            None,
        );
        drop(permit);
        assert_eq!(dir.used(), 4000 + 1 + 9000);
    }

    #[test]
    fn a_failed_settle_puts_the_reservation_back_so_nothing_stays_running() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        // 把预留指向一个打不开的路径，让结算失败（重试 3 次后放弃）
        let bogus = dir
            .path()
            .with_file_name("not-a-dir")
            .join("x")
            .join("y.sqlite3");
        RESERVATION.with(|slot| {
            if let Some(reservation) = slot.borrow_mut().as_mut() {
                reservation.database = bogus.clone();
            }
        });
        record(Some(&bogus), "openai", "m", "p", "c", Some((10, 5)));
        let restored = RESERVATION.with(|slot| slot.borrow().is_some());
        assert!(restored, "结算失败要把预留放回去，凭证放掉时才有人收尾");
        // 收尾：指回真的库，让凭证放掉时能把它记成已取消
        RESERVATION.with(|slot| {
            if let Some(reservation) = slot.borrow_mut().as_mut() {
                reservation.database = dir.path().to_path_buf();
            }
        });
        drop(permit);
        let (status, _, _) = dir.row(LEGACY_ROW);
        assert_eq!(status, "cancelled", "不能留下一条永远 running 的行");
    }

    #[test]
    fn a_permit_cannot_cross_threads() {
        // 编译期性质：SpendPermit 含 PhantomData<*const ()>，不是 Send。
        // 用「自动引用」的小技巧在运行期读出来：只有 T: Send 时 Probe::<T>::IS_SEND 才取到 true，否则退到 trait 里的 false。
        trait NotSend {
            const IS_SEND: bool = false;
        }
        impl<T: ?Sized> NotSend for T {}
        struct Probe<T: ?Sized>(PhantomData<T>);
        #[allow(dead_code)]
        impl<T: ?Sized + Send> Probe<T> {
            const IS_SEND: bool = true;
        }
        assert!(
            !<Probe<SpendPermit>>::IS_SEND,
            "凭证不能是 Send：在别的线程放掉会让原线程的嵌套深度卡在 1，之后的请求不再检查额度"
        );
        assert!(<Probe<String>>::IS_SEND, "探针本身要能认出 Send 的类型");
    }
}
