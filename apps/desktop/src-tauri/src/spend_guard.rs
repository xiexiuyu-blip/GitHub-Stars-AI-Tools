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
use std::sync::OnceLock;

/// 旧功能没有给输出设 `max_tokens`，输出长度不可控；按一个宽松的值估。
/// 一篇 README 摘要实测输出约 1k，标签网络一批可达几千。
const OUTPUT_ALLOWANCE_TOKENS: u64 = 3_000;

/// 拒绝时的前缀。调用方用 `is_refusal` 认它。
pub const REFUSAL_PREFIX: &str = "AI 额度：";

static DATABASE: OnceLock<PathBuf> = OnceLock::new();

thread_local! {
    static PROBE_DEPTH: Cell<u32> = const { Cell::new(0) };
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

pub fn check_before_request(prompt: &str) -> Result<(), String> {
    let Some(database) = DATABASE.get() else {
        return Ok(());
    };
    if probing() {
        return Ok(());
    }
    let estimated = estimate_tokens(prompt).saturating_add(OUTPUT_ALLOWANCE_TOKENS);
    check_at(database, &jobs::now_db_time(), estimated)
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
    let _ = record_at(
        database,
        &jobs::now_db_time(),
        provider,
        model,
        input,
        output,
    );
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
    let input = i64::try_from(input_tokens).unwrap_or(i64::MAX / 4);
    let output = i64::try_from(output_tokens).unwrap_or(i64::MAX / 4);
    connection
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
        assert!(check_before_request("任意提示词").is_ok());
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
}
