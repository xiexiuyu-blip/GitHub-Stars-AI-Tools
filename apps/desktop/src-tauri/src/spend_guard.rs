//! 旧版 AI 功能和新版队列共用同一个每日额度。
//!
//! 队列（`jobs.rs`）在认领时就把预留记进数据库，额度是它管的。但旧版功能——整批解析、单个仓库的 AI 摘要、
//! 标签网络、AI 搜索、翻译、推荐——不走队列，以前完全不看额度：顶栏「增量 AI 解析」点一下，
//! 495 个仓库约 160 万到 190 万 token，是 10 万额度的十几倍，而且没有取消按钮，也不记进「最近 24 小时已用」。
//!
//! 这里给它们补同一道闸，守在向模型服务发请求的地方：
//! - 请求前（`check_before_request`）：额度是 0、队列被暂停、或最近 24 小时已用加上这次预计用量超过额度，
//!   就不发请求，返回一句人话（以「AI 额度：」开头，调用方可以据此提前停下，不要把整批都记成失败）；
//! - 请求后（`record_after_request`）：把用量记成一条已完成的任务，「最近 24 小时已用」就看得见。
//!
//! 额度、暂停都用队列的控制文件（工具箱里「AI 自动整理」的设置），所以只有一处开关。
//! 没装闸（单元测试、没有数据库）时什么都不拦。连接测试和自检是一次很小的调用，
//! 用 `probe` 包起来：不看额度，也不被暂停拦住，但仍然记账。

use crate::jobs;
use rusqlite::{params, Connection};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// 旧功能没有给输出设 `max_tokens`，输出长度不可控；按一个宽松的值估：至少 3000，翻译这类输出和输入一样长的取输入的长度。
/// 一篇 README 摘要实测输出约 1k，标签网络一批可达几千。
const MIN_OUTPUT_ALLOWANCE_TOKENS: u64 = 3_000;

/// 单次调用记进账本的上限（每一列）。服务商回一个离谱的用量（几十亿）会让账本求和溢出，
/// 「最近 24 小时已用」读不出来，队列和旧功能都被锁 24 小时。
const MAX_RECORDED_TOKENS: u64 = 5_000_000;

/// 拒绝时的前缀。调用方用 `is_refusal` 认它。
pub const REFUSAL_PREFIX: &str = "AI 额度：";

static DATABASE: OnceLock<PathBuf> = OnceLock::new();

thread_local! {
    static PROBE_DEPTH: Cell<u32> = const { Cell::new(0) };
    static PERMIT_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// 旧功能的请求一次只放一个过去：「问额度 → 发请求 → 记账」是一整段，
/// 两个请求同时问到「还有 10 万」、各自花 6 万，就是 12 万。队列有自己的预留，不走这把锁。
static LEGACY_LOCK: Mutex<()> = Mutex::new(());

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

/// 通过了额度检查的凭证。拿着它直到请求结束并记完账。同一个线程里可以重入
/// （流式请求失败后会在里面再发一次普通请求），只有最外层真的持有那把锁。
pub struct SpendPermit {
    _lock: Option<MutexGuard<'static, ()>>,
}

impl Drop for SpendPermit {
    fn drop(&mut self) {
        PERMIT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

fn acquire_permit() -> SpendPermit {
    let outermost = PERMIT_DEPTH.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        current == 0
    });
    let lock = outermost.then(|| {
        LEGACY_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    });
    SpendPermit { _lock: lock }
}

pub fn check_before_request(prompt: &str) -> Result<SpendPermit, String> {
    let permit = acquire_permit();
    let Some(database) = DATABASE.get() else {
        // 没装闸：单元测试里不拦；正式运行时拒绝（宁可旧功能暂时不能用，也不能在没有额度保护的情况下花钱）
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
    check_at(database, &jobs::now_db_time(), estimated)?;
    Ok(permit)
}

/// `usage` 是服务商回的真实用量（输入，输出）；没有就按文字估。
pub fn record_after_request(
    provider: &str,
    model: &str,
    prompt: &str,
    content: &str,
    usage: Option<(u64, u64)>,
) {
    let Some(database) = DATABASE.get() else {
        return;
    };
    let (input, output) =
        usage.unwrap_or_else(|| (estimate_tokens(prompt), estimate_tokens(content)));
    if let Err(error) = record_at(
        database,
        &jobs::now_db_time(),
        provider,
        model,
        input,
        output,
    ) {
        // 记不上账不能悄悄吞掉：至少留一条日志（开发版在终端里能看到）
        eprintln!("旧版 AI 用量没有记上（{input} + {output} token）：{error}");
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

fn check_at(database_path: &Path, now: &str, estimated: u64) -> Result<(), String> {
    let control = jobs::read_control(&jobs::queue_control_path(database_path));
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
    let used = jobs::tokens_used_in_last_day(database_path, now)?.max(0) as u64;
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

/// 记成一条已完成的任务：`problem_answer` 这个类型队列从不认领，`target_id` 每次不同，不会撞唯一索引。
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
    use std::thread;
    use std::time::Duration;

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
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            if let Some(parent) = self.0.parent() {
                let _ = fs::remove_dir_all(parent);
            }
        }
    }

    const NOW: &str = "2026-10-04T12:00:00.000Z";

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
        let error = check_at(dir.path(), NOW, 1000).unwrap_err();
        assert!(is_refusal(&error), "{error}");
        assert!(error.contains("额度是 0"), "{error}");
        dir.control(r#"{"dailyBudgetTokens":0}"#);
        assert!(check_at(dir.path(), NOW, 1).is_err());
    }

    #[test]
    fn paused_queue_refuses_even_with_budget() {
        let dir = Dir::new();
        dir.control(r#"{"dailyBudgetTokens":100000,"paused":true}"#);
        let error = check_at(dir.path(), NOW, 100).unwrap_err();
        assert!(is_refusal(&error));
        assert!(error.contains("暂停"), "{error}");
    }

    #[test]
    fn headroom_decides_and_the_message_says_why() {
        let dir = Dir::new();
        dir.control(r#"{"dailyBudgetTokens":100000,"paused":false}"#);
        assert!(check_at(dir.path(), NOW, 100_000).is_ok());
        assert!(check_at(dir.path(), NOW, 100_001).is_err());
        record_at(dir.path(), NOW, "openai", "m", 60_000, 30_000).unwrap();
        // 已用 9 万：再要 1 万刚好放得下，10001 放不下
        assert!(check_at(dir.path(), NOW, 10_000).is_ok());
        let error = check_at(dir.path(), NOW, 10_001).unwrap_err();
        assert!(is_refusal(&error));
        assert!(error.contains("9.0 万"), "{error}");
        assert!(error.contains("10.0 万"), "{error}");
    }

    #[test]
    fn recorded_usage_lands_in_the_queue_ledger_as_a_finished_job() {
        let dir = Dir::new();
        dir.control(r#"{"dailyBudgetTokens":100000}"#);
        record_at(dir.path(), NOW, "openai", "gpt", 1200, 345).unwrap();
        record_at(dir.path(), NOW, "openai", "gpt", 10, 5).unwrap();
        let connection = Connection::open(dir.path()).unwrap();
        let (rows, input, output, account, kind, status): (i64, i64, i64, String, String, String) = connection
            .query_row(
                "SELECT COUNT(*), SUM(input_tokens), SUM(output_tokens), MIN(account_id), MIN(job_type), MIN(status) FROM ai_jobs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();
        assert_eq!((rows, input, output), (2, 1210, 350));
        assert_eq!(account, "acct", "记在最近用过的账号上");
        assert_eq!(kind, "problem_answer", "队列从不认领这个类型");
        assert_eq!(status, "succeeded");
        assert_eq!(
            jobs::tokens_used_in_last_day(dir.path(), NOW).unwrap(),
            1560,
            "「最近 24 小时已用」要看得见旧功能的用量"
        );
    }

    #[test]
    fn probe_skips_the_check_but_not_the_ledger() {
        let dir = Dir::new();
        // 额度是 0 也不拦探测；但探测之外仍然拦
        assert!(check_at(dir.path(), NOW, 10).is_err());
        let inside = probe(|| {
            assert!(probing());
            probing()
        });
        assert!(inside);
        assert!(!probing());
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
    fn permits_serialize_requests_but_are_reentrant_on_one_thread() {
        // 同一线程里再取一次（流式失败后回退到普通请求）不能死锁
        let outer = check_before_request("外层").expect("应通过");
        let inner = check_before_request("内层").expect("同一线程重入不能死锁");
        drop(inner);
        drop(outer);
        // 别的线程要等外层放掉才拿得到
        let outer = check_before_request("外层").expect("应通过");
        let (sender, receiver) = std::sync::mpsc::channel();
        let waiter = thread::spawn(move || {
            let _permit = check_before_request("别的线程").expect("应通过");
            let _ = sender.send(());
        });
        assert!(
            receiver.recv_timeout(Duration::from_millis(300)).is_err(),
            "外层还拿着，别的线程不该拿到"
        );
        drop(outer);
        receiver
            .recv_timeout(Duration::from_secs(3))
            .expect("放掉之后别的线程应该拿到");
        waiter.join().unwrap();
    }

    #[test]
    fn recording_without_any_account_is_an_error_not_a_silent_success() {
        let dir = Dir::new();
        Connection::open(dir.path())
            .unwrap()
            .execute("DELETE FROM github_accounts", [])
            .unwrap();
        let error = record_at(dir.path(), NOW, "openai", "m", 100, 10).unwrap_err();
        assert!(error.contains("账号"), "{error}");
    }

    #[test]
    fn absurd_usage_is_clamped_so_the_ledger_sum_cannot_overflow() {
        let dir = Dir::new();
        dir.control(r#"{"dailyBudgetTokens":100000}"#);
        for _ in 0..3 {
            record_at(dir.path(), NOW, "openai", "m", u64::MAX, u64::MAX).unwrap();
        }
        let used = jobs::tokens_used_in_last_day(dir.path(), NOW).expect("求和不能溢出");
        assert_eq!(used, 3 * 2 * MAX_RECORDED_TOKENS as i64);
    }

    #[test]
    fn long_inputs_get_a_long_output_allowance() {
        // 翻译这类输出和输入一样长：2 万字的提示词，检查要看 4 万 token 的余量
        let dir = Dir::new();
        dir.control(r#"{"dailyBudgetTokens":100000}"#);
        record_at(dir.path(), NOW, "openai", "m", 70_000, 0).unwrap();
        let long_prompt = "说".repeat(20_000);
        let estimated = estimate_tokens(&long_prompt)
            .saturating_add(MIN_OUTPUT_ALLOWANCE_TOKENS.max(estimate_tokens(&long_prompt)));
        assert_eq!(estimated, 40_000);
        assert!(check_at(dir.path(), NOW, estimated).is_err());
    }
}
