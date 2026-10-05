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
//! - 请求后（`record_after_request`）：把预留行换成真实用量（服务商回的真实用量；没有用量——流式、或回复里没写——
//!   就按文字估，中文按 1 字 1 token，而且不低于预留：推理模型的思考 token 不在文字里）；
//! - 没记账就结束（出错、panic）：预留行记成已取消，只留「提示词那部分」的估计用量（请求发出去了才可能计费，输出那部分没有）。
//!   程序崩溃时留下的预留行，下次启动由 `jobs::recover_running_jobs` 记成已取消，用量照旧计入窗口；
//! - 同一次请求里的多次尝试（5xx 后重试、流式失败后回退到普通请求）：每一次可能已经计费的失败尝试，
//!   按提示词的估计用量记一笔（`account_failed_attempt`），最后结算或收尾时一起算进去；
//!   肯定没计费的（被 4xx 拒绝、连接没建起来、请求没拼成、几秒内就回的 5xx 或断线——网关 / 过载保护在模型开始跑之前挡下的，
//!   见 `ai.rs` 的 `FAST_FAILURE_SECONDS`）不记，整笔请求都没计费就把预留整个退回（`release_unbilled`）。
//!
//! 额度、暂停都用队列的控制文件（工具箱里「AI 自动整理」的设置），所以只有一处开关。
//! 没装闸（单元测试）时什么都不拦；正式运行时没装上就一律拒绝，宁可旧功能暂时不能用，也不在没有额度保护时花钱。
//! 连接测试和自检是一次很小的调用，用 `probe` 包起来：不看额度、不被暂停拦住，但仍然记账。
//!
//! 「找方案」的 AI 精选（用户一搜就自动调一次模型，约 4k token）用 `solve_rerank` 包起来：它在同一个账本里记账
//! （`prompt_version = 'solve-rerank'`），但**有自己的每日额度**，不被后台整理（尤其是全量整理）用掉的额度挡住；
//! 这份额度跟着每天额度走（`rerank_cap`）：最多 `SOLVE_RERANK_DAILY_TOKENS`，并且不超过每天额度的一半（全量整理进行时按上限）；
//! 前提是 Fox 开过 AI 额度（每天额度大于 0，或全量整理在进行）、队列没有暂停。
//! AI 精选一次搜索只发一个请求（`ai::AiCallOptions`，不重试）；等了很久才失败、没拿到可用回复的那一次（最终超时等，
//! 见 `account_slow_failure`），账本里至少记 `jobs::TIMEOUT_ATTEMPT_TOKEN_ESTIMATE`。

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

/// 旧功能在账本里的标记（`ai_jobs.prompt_version`）。
const LEGACY_TAG: &str = "legacy";
/// 「找方案」AI 精选在账本里的标记。队列从不认领 `problem_answer`，所以它和旧功能一样只是账本行。
pub const SOLVE_RERANK_TAG: &str = "solve-rerank";
/// 「找方案」AI 精选每 24 小时最多用这么多 token（一次约 4k，够搜 70 次左右）。这是「上限的上限」：
/// 实际能用多少看 `rerank_cap`（每天额度小的时候只给一半，全量整理进行时才给满）。
pub const SOLVE_RERANK_DAILY_TOKENS: u64 = 300_000;

static DATABASE: OnceLock<PathBuf> = OnceLock::new();

/// 这个线程当前这次请求在账本里的预留行。
struct Reservation {
    database: PathBuf,
    id: String,
    /// 提示词那部分的估计用量：请求没有记账就结束时，只留这么多。
    input_estimate: u64,
    /// 预留的总量（提示词 + 输出余量）。流式回复没有真实用量时，记账不能比它还少。
    reserved_total: u64,
    /// 账本里的标记（旧功能 / 找方案 AI 精选）。预留行被取消后要「另记一条」时沿用它。
    tag: &'static str,
}

thread_local! {
    static PROBE_DEPTH: Cell<u32> = const { Cell::new(0) };
    static RERANK_DEPTH: Cell<u32> = const { Cell::new(0) };
    static PERMIT_DEPTH: Cell<u32> = const { Cell::new(0) };
    static RESERVATION: RefCell<Option<Reservation>> = const { RefCell::new(None) };
    /// 最近这次尝试可能已经到了服务商那里（流式请求收到了状态行、读到一半断了），还没记进 `EXTRA_TOKENS`。
    static MAYBE_BILLED: Cell<bool> = const { Cell::new(false) };
    /// 同一次请求里更早的、已经失败但可能计费的尝试（5xx 后重试、流式失败后回退到普通请求）：
    /// 每次按提示词的估计用量记一笔，最后结算（或收尾）时一起算进去。
    static EXTRA_TOKENS: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// 测试里指定这个线程用哪个库（见 `testing::Dir::use_in_this_thread`）；正式运行里没有这一项。
    static TEST_DATABASE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// 启动时装上。只能装一次；数据库路径之后不变。
pub fn install(database_path: &Path) {
    let _ = DATABASE.set(database_path.to_path_buf());
}

fn database_in_use() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(path) = TEST_DATABASE.with(|slot| slot.borrow().clone()) {
        return Some(path);
    }
    DATABASE.get().cloned()
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

struct RerankScope;

impl Drop for RerankScope {
    fn drop(&mut self) {
        RERANK_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// 「找方案」的 AI 精选：包在里面的请求按它自己的每日额度（`rerank_cap`，不超过 `SOLVE_RERANK_DAILY_TOKENS`）算，
/// 不看后台整理用掉了多少；账本行标成 `solve-rerank`。暂停、每天额度是 0（Fox 还没开过 AI 额度，全量整理也没开）照样拦。
pub fn solve_rerank<R>(run: impl FnOnce() -> R) -> R {
    RERANK_DEPTH.with(|depth| depth.set(depth.get() + 1));
    let _scope = RerankScope;
    run()
}

fn reranking() -> bool {
    RERANK_DEPTH.with(|depth| depth.get() > 0)
}

/// 预留 / 记账用哪套规矩。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Policy {
    Legacy,
    SolveRerank,
}

impl Policy {
    fn tag(self) -> &'static str {
        match self {
            Policy::Legacy => LEGACY_TAG,
            Policy::SolveRerank => SOLVE_RERANK_TAG,
        }
    }
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
            // 到这里还在，说明这次请求没有记账就结束了（出错、panic）：
            // 最后一次尝试可能已经发出去了（留提示词那部分），加上更早失败但可能计费的那些
            let earlier = EXTRA_TOKENS.with(Cell::take);
            if let Some(reservation) = RESERVATION.with(|slot| slot.borrow_mut().take()) {
                let kept = reservation.input_estimate.saturating_add(earlier);
                if let Err(error) = cancel_reservation(&reservation, &jobs::now_db_time(), kept) {
                    eprintln!("旧版 AI 的预留没有收尾：{error}");
                }
            }
            MAYBE_BILLED.with(|flag| flag.set(false));
        }
    }
}

pub fn check_before_request(prompt: &str) -> Result<SpendPermit, String> {
    begin(database_in_use().as_deref(), prompt)
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
    MAYBE_BILLED.with(|flag| flag.set(false));
    EXTRA_TOKENS.with(|extra| extra.set(0));
    let Some(database) = database else {
        // 没装闸：单元测试里不拦；正式运行时拒绝
        if cfg!(test) {
            return Ok(permit);
        }
        return Err(format!(
            "{REFUSAL_PREFIX}每天额度的检查还没有准备好，AI 功能先不能用。重新打开应用再试。"
        ));
    };
    if probing() {
        return Ok(permit);
    }
    let input = estimate_tokens(prompt);
    let estimated = input.saturating_add(MIN_OUTPUT_ALLOWANCE_TOKENS.max(input));
    let policy = if reranking() {
        Policy::SolveRerank
    } else {
        Policy::Legacy
    };
    let id = reserve_with(database, &jobs::now_db_time(), estimated, policy)?;
    RESERVATION.with(|slot| {
        *slot.borrow_mut() = Some(Reservation {
            database: database.to_path_buf(),
            id,
            input_estimate: input,
            reserved_total: estimated,
            tag: policy.tag(),
        });
    });
    Ok(permit)
}

/// 最后一次尝试被服务商直接拒绝（4xx：密钥 / 模型 / 额度 / 参数不对、限流）或根本没连上：这一次没有计费，
/// 把预留还回去，只留更早失败但可能计费的尝试（`account_failed_attempt` 记下的），没有的话不留任何用量。
/// 不调用的话，预留行会在凭证放掉时留下提示词那部分的估计用量——一把错的密钥、一个配错的模型，
/// 整批解析连失败 20 多次，就在账本里记下 9 万多 token、什么也没花，把队列和所有旧功能锁 24 小时。
/// 最近这次尝试如果可能已经到了服务商那里（`mark_possibly_billed` 之后），就什么也不退：留给凭证放掉时按「可能计费」收尾。
pub fn release_unbilled() {
    if MAYBE_BILLED.with(Cell::get) {
        return;
    }
    let Some(reservation) = RESERVATION.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    let earlier = EXTRA_TOKENS.with(Cell::get);
    match cancel_reservation(&reservation, &jobs::now_db_time(), earlier) {
        Ok(()) => EXTRA_TOKENS.with(|extra| extra.set(0)),
        Err(error) => {
            eprintln!("旧版 AI 的预留没有退回：{error}");
            // 没退成：把预留放回去，凭证放掉时会收尾成「已取消 + 提示词那部分」，不会留下一条永远 running 的行
            RESERVATION.with(|slot| *slot.borrow_mut() = Some(reservation));
        }
    }
}

/// 最近这次尝试可能已经到了服务商那里（流式请求收到了状态行；超时或读到一半断了）。
/// 之后如果请求还要继续（回退到普通请求），再调 `account_failed_stream` 把它记下来。
pub fn mark_possibly_billed() {
    MAYBE_BILLED.with(|flag| flag.set(true));
}

/// 一次可能已计费的尝试失败了，但请求还会继续（5xx 后重试、流式失败后回退到普通请求）：
/// 把这次尝试提示词的估计用量记下，最后结算（或收尾）时一起算进去——否则只有最后一次的用量进账本。
pub fn account_failed_attempt() {
    let input = RESERVATION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |reservation| reservation.input_estimate)
    });
    EXTRA_TOKENS.with(|extra| extra.set(extra.get().saturating_add(input)));
    MAYBE_BILLED.with(|flag| flag.set(false));
}

/// 流式请求失败、马上要回退到普通请求：流式那次如果可能已经计费，就把它记下。
pub fn account_failed_stream() {
    if MAYBE_BILLED.with(Cell::get) {
        account_failed_attempt();
    }
}

/// 等了很久才失败、没拿到可用回复，但服务商多半已经在算的这一次（最终超时：请求发出去了，一直等到时限也没等到回应；
/// 等了一阵才回的 5xx；等了一阵才断开的连接；响应体一直读不完）。已经写出来的会计费，只留「提示词那部分」会明显偏低。
/// 这一次在账本里记 **max(`jobs::TIMEOUT_ATTEMPT_TOKEN_ESTIMATE`，提示词估计 + 输出上限)**：
/// 不少于队列对超时的记法（1 万），提示词很长时再按最坏情形（把输出额度写满）往上补；`max_output_tokens` 没有上限就传 0。
/// 做法：把「提示词估计之外还要补的部分」（上面那个数减去提示词估计，不会是负数）加进 `EXTRA_TOKENS`，
/// 凭证放掉时（`SpendPermit::drop`，没改）和提示词那部分一起结算，所以这一行 = 上面那个数，再加上更早失败的尝试。
/// 这次尝试之后也标成「可能已计费」，之后不会被 `release_unbilled` 退回。
/// 每次失败只调用一次；没有预留行（探测、没装闸的测试）时什么也不做。
pub fn account_slow_failure(max_output_tokens: u64) {
    let Some(input) = RESERVATION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|reservation| reservation.input_estimate)
    }) else {
        return;
    };
    let timeout_floor = u64::try_from(jobs::TIMEOUT_ATTEMPT_TOKEN_ESTIMATE).unwrap_or(0);
    let floor = timeout_floor.max(input.saturating_add(max_output_tokens));
    let top_up = floor.saturating_sub(input);
    EXTRA_TOKENS.with(|extra| extra.set(extra.get().saturating_add(top_up)));
    MAYBE_BILLED.with(|flag| flag.set(true));
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
        database_in_use().as_deref(),
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
    // 更早失败但可能计费的尝试（见 `account_failed_attempt`）：算进输入那一列
    let earlier = EXTRA_TOKENS.with(Cell::take);
    MAYBE_BILLED.with(|flag| flag.set(false));
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
            let input = input.saturating_add(earlier);
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
                Ok(false) => record_at_tagged(
                    database,
                    &now,
                    provider,
                    model,
                    input,
                    output,
                    reservation.tag,
                ),
                Err(error) => {
                    // 记不上：把预留放回去，凭证放掉时会收尾成「已取消 + 提示词那部分」，不会留下一条永远 running 的行
                    RESERVATION.with(|slot| *slot.borrow_mut() = Some(reservation));
                    EXTRA_TOKENS.with(|extra| extra.set(earlier));
                    Err(error)
                }
            }
        }
        _ => {
            let (input, output) = real_usage.unwrap_or(estimate);
            record_at(
                database,
                &now,
                provider,
                model,
                input.saturating_add(earlier),
                output,
            )
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

/// 额度的规矩（纯函数）：暂停、额度是 0、放不下。`unlimited`：Fox 亲手开了「全量整理」，这一轮不看每天额度（暂停照样拦）。
fn refusal_for(
    control: &jobs::QueueControl,
    used: u64,
    estimated: u64,
    unlimited: bool,
) -> Result<(), String> {
    if control.paused {
        return Err(format!(
            "{REFUSAL_PREFIX}AI 整理现在是暂停状态，其它 AI 功能也一起停着。要继续请先到「设置 → 自动整理与备份」里点「继续整理」。"
        ));
    }
    if unlimited {
        return Ok(());
    }
    if control.daily_budget_tokens == 0 {
        return Err(format!(
            "{REFUSAL_PREFIX}每天额度是 0，所有 AI 功能都还不能用。要用请先到「设置 → 自动整理与备份」的「每天自动整理」里开启每天额度，或在同一页点「现在整理」。"
        ));
    }
    if used.saturating_add(estimated) > control.daily_budget_tokens {
        return Err(format!(
            "{REFUSAL_PREFIX}最近 24 小时已经用了约 {}（整理现有收藏用的不算），这一次预计还要约 {}，会超过每天 {} 的额度，所以没有发请求。过一阵额度恢复后再试，或到「设置 → 自动整理与备份」里调高每天额度；想马上整理完，可以点「现在整理」。",
            readable_tokens(used),
            readable_tokens(estimated),
            readable_tokens(control.daily_budget_tokens),
        ));
    }
    Ok(())
}

/// 「找方案」AI 精选现在每 24 小时最多能用多少（有效上限）：
/// - 全量整理在进行（Fox 亲手开的）：`SOLVE_RERANK_DAILY_TOKENS`；
/// - 否则：每天额度的一半，但不超过 `SOLVE_RERANK_DAILY_TOKENS`。每天额度调小（试跑 10 万），AI 精选也跟着少花，
///   不会出现「额度只有 5 万、AI 精选却能花 30 万、后台整理被挡一整天」（同一本账，它用掉的队列也看得见）。
/// 每天额度是 0 又没有全量整理时这里给 0；调用方（`refusal_for_rerank`）在那之前就已经拒绝了。
fn rerank_cap(control: &jobs::QueueControl, scan_open: bool) -> u64 {
    if scan_open {
        SOLVE_RERANK_DAILY_TOKENS
    } else {
        SOLVE_RERANK_DAILY_TOKENS.min(control.daily_budget_tokens / 2)
    }
}

/// 「找方案」AI 精选的规矩（纯函数）：暂停、每天额度是 0（又没有全量整理在进行）、自己这份额度（`rerank_cap`）放不下。
/// 不看后台整理已经用了多少：后台用满了，AI 精选照样能用自己的那份。
fn refusal_for_rerank(
    control: &jobs::QueueControl,
    rerank_used: u64,
    estimated: u64,
    scan_open: bool,
) -> Result<(), String> {
    if control.paused {
        return Err(format!(
            "{REFUSAL_PREFIX}AI 整理现在是暂停状态，「找方案」的 AI 精选也先停着，这次按本地排序。"
        ));
    }
    if control.daily_budget_tokens == 0 && !scan_open {
        return Err(format!(
            "{REFUSAL_PREFIX}每天额度是 0，「找方案」的 AI 精选还不能用，这次按本地排序。"
        ));
    }
    let cap = rerank_cap(control, scan_open);
    if rerank_used.saturating_add(estimated) > cap {
        return Err(format!(
            "{REFUSAL_PREFIX}「找方案」的 AI 精选最近 24 小时已经用了约 {}，这一次预计还要约 {}，会超过它每天 {} 的额度，所以没有发请求；这次按本地排序，过一阵额度恢复后再试。",
            readable_tokens(rerank_used),
            readable_tokens(estimated),
            readable_tokens(cap),
        ));
    }
    Ok(())
}

/// 最近 24 小时里「找方案」AI 精选用掉的（含在途预留）。口径和 `jobs::tokens_used_since` 一样，只是限定标记。
fn rerank_tokens_used_since(connection: &Connection, cutoff: &str) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COALESCE(SUM(input_tokens + output_tokens), 0)
             FROM ai_jobs
             WHERE prompt_version = ?2
               AND ((status IN ('succeeded', 'skipped', 'failed', 'cancelled')
                     AND finished_at IS NOT NULL
                     AND finished_at >= ?1)
                 OR (status IN ('queued', 'running')
                     AND input_tokens + output_tokens > 0
                     AND updated_at >= ?1))",
            params![cutoff, SOLVE_RERANK_TAG],
            |row| row.get(0),
        )
        .map_err(|error| format!("用量读取失败：{error}"))
}

/// 旧功能的预留（见 `reserve_with`）。
fn reserve_at(database_path: &Path, now: &str, estimated: u64) -> Result<String, String> {
    reserve_with(database_path, now, estimated, Policy::Legacy)
}

/// 在一个 `BEGIN IMMEDIATE` 里查额度并写一条 `running` 的预留行，返回这一行的 id。
/// 队列认领（`jobs::claim_next`）也是在 `BEGIN IMMEDIATE` 里算用量，后一个一定看得见前一个的预留。
/// 找方案 AI 精选的预留行同样占「最近 24 小时已用」（队列和旧功能都看得见），只是它自己的额度另算。
fn reserve_with(
    database_path: &Path,
    now: &str,
    estimated: u64,
    policy: Policy,
) -> Result<String, String> {
    let control = jobs::read_control(&jobs::queue_control_path(database_path));
    let refuse = |error: String| format!("{REFUSAL_PREFIX}额度检查失败：{error}");
    let mut connection = jobs::open_db(database_path).map_err(refuse)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| refuse(error.to_string()))?;
    let cutoff = jobs::shift_db_time(now, -jobs::DAY_SECS).map_err(refuse)?;
    let used = jobs::daily_tokens_used(&transaction, &control, now)
        .map_err(refuse)?
        .max(0) as u64;
    // 全量整理进行中（Fox 亲手确认过、没超时、用量没超过刹车线）：这一轮不看每天额度。预留照常写，用量照常记账
    let unlimited = jobs::full_scan_open(&transaction, &control, now);
    match policy {
        Policy::Legacy => refusal_for(&control, used, estimated, unlimited)?,
        Policy::SolveRerank => {
            let rerank_used = rerank_tokens_used_since(&transaction, &cutoff)?.max(0) as u64;
            refusal_for_rerank(&control, rerank_used, estimated, unlimited)?
        }
    }
    let id: String = transaction
        .query_row("SELECT lower(hex(randomblob(8)))", [], |row| row.get(0))
        .map_err(|error| format!("{REFUSAL_PREFIX}额度预留失败：{error}"))?;
    let reserved = i64::try_from(estimated.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    let inserted = transaction
        .execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, attempts, max_attempts,
                                 prompt_version, input_tokens, output_tokens, created_at, started_at, updated_at)
             SELECT ?1, github_accounts.id, 'problem_answer', 'query', 'legacy-' || ?1, 9, 'running', 1, 1,
                    ?4, ?2, 0, ?3, ?3, ?3
             FROM github_accounts
             ORDER BY github_accounts.updated_at DESC
             LIMIT 1",
            params![id, reserved, now, policy.tag()],
        )
        .map_err(|error| format!("{REFUSAL_PREFIX}额度预留失败：{error}"))?;
    if inserted == 0 {
        return Err(format!(
            "{REFUSAL_PREFIX}还没有连接 GitHub 账号，用量没法记账，所以 AI 功能先不能用。"
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

/// 预留行记成已取消，只留 `kept_tokens`（0 就是整个退回）：
/// - 没记账就结束：提示词那部分的估计用量，加上更早失败但可能计费的尝试；
/// - 最后一次尝试没有计费（`release_unbilled`）：只留更早失败但可能计费的那些。
fn cancel_reservation(
    reservation: &Reservation,
    now: &str,
    kept_tokens: u64,
) -> Result<(), String> {
    let connection = jobs::open_db(&reservation.database)?;
    let kept = i64::try_from(kept_tokens.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
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
    record_at_tagged(
        database_path,
        now,
        provider,
        model,
        input_tokens,
        output_tokens,
        LEGACY_TAG,
    )
}

fn record_at_tagged(
    database_path: &Path,
    now: &str,
    provider: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    tag: &str,
) -> Result<(), String> {
    let connection = jobs::open_db(database_path)?;
    insert_ledger_row(
        &connection,
        now,
        provider,
        model,
        input_tokens,
        output_tokens,
        tag,
    )
}

fn insert_ledger_row(
    connection: &Connection,
    now: &str,
    provider: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    tag: &str,
) -> Result<(), String> {
    let input = i64::try_from(input_tokens.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    let output = i64::try_from(output_tokens.min(MAX_RECORDED_TOKENS)).unwrap_or(0);
    let inserted = connection
        .execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, attempts, max_attempts,
                                 provider, model, prompt_version, input_tokens, output_tokens, created_at, started_at, finished_at, updated_at)
             SELECT lower(hex(randomblob(8))), github_accounts.id, 'problem_answer', 'query', 'legacy-' || lower(hex(randomblob(8))),
                    9, 'succeeded', 1, 1, ?1, ?2, ?6, ?3, ?4, ?5, ?5, ?5, ?5
             FROM github_accounts
             ORDER BY github_accounts.updated_at DESC
             LIMIT 1",
            params![provider, model, input, output, now, tag],
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
        let tenths = tokens.saturating_add(500) / 1_000;
        format!("{}.{} 万 token", tenths / 10, tenths % 10)
    } else {
        format!("{tokens} token")
    }
}

/// 账本夹具：别的模块的测试（走真请求函数的记账测试）也用它。
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    pub(crate) struct Dir(PathBuf);

    impl Dir {
        pub(crate) fn new() -> Self {
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

        pub(crate) fn path(&self) -> &Path {
            &self.0
        }

        /// 往账本里记一笔「后台整理」的用量（队列任务，不是旧功能的行）：测「队列额度被用光」时用。
        pub(crate) fn fill_background_usage_row(&self, tokens: i64) {
            let now = jobs::now_db_time();
            Connection::open(&self.0)
                .unwrap()
                .execute(
                    "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, attempts, max_attempts,
                                         input_tokens, output_tokens, created_at, started_at, finished_at, updated_at)
                     VALUES (lower(hex(randomblob(8))), 'acct', 'repo_card', 'repository', lower(hex(randomblob(8))), 3, 'succeeded', 1, 1,
                             ?1, 0, ?2, ?2, ?2, ?2)",
                    params![tokens, now],
                )
                .unwrap();
        }

        pub(crate) fn control(&self, text: &str) {
            fs::write(jobs::queue_control_path(&self.0), text).unwrap();
        }

        pub(crate) fn used(&self) -> i64 {
            jobs::tokens_used_in_last_day(&self.0, &jobs::now_db_time()).unwrap()
        }

        pub(crate) fn row(&self, sql: &str) -> (String, i64, i64) {
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

    impl Dir {
        /// 这个线程里的请求都记到这个库（没装全局闸门时的测试用；每个测试自己的线程，互不影响）。
        pub(crate) fn use_in_this_thread(&self) -> DatabaseScope {
            TEST_DATABASE.with(|slot| *slot.borrow_mut() = Some(self.0.clone()));
            DatabaseScope
        }
    }

    pub(crate) fn fill_background_usage(dir: &Dir, tokens: i64) {
        dir.fill_background_usage_row(tokens);
    }

    pub(crate) struct DatabaseScope;

    impl Drop for DatabaseScope {
        fn drop(&mut self) {
            TEST_DATABASE.with(|slot| *slot.borrow_mut() = None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Dir;
    use super::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    const BUDGET_100K: &str = r#"{"dailyBudgetTokens":100000,"paused":false}"#;
    const BUDGET_600K: &str = r#"{"dailyBudgetTokens":600000,"paused":false}"#;
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
        account_failed_attempt();
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
        let (status, input, _) = dir.row(LEGACY_ROW);
        assert_eq!(status, "cancelled", "不能留下一条永远 running 的行");
        assert_eq!(
            input,
            1000 + 1000,
            "结算失败时，更早失败的那次尝试（1000）也要留着，和最后一次的提示词估计（1000）一起收尾"
        );
    }

    #[test]
    fn an_earlier_possibly_billed_attempt_survives_a_later_unbilled_one() {
        // 第一次尝试 5xx（可能已计费）、重试时被限流 / 被拒（肯定没计费）：不能把整笔预留退光，
        // 否则第一次那笔可能的花费就从账本里消失了
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        account_failed_attempt();
        release_unbilled();
        drop(permit);
        assert_eq!(dir.used(), 1000, "只留更早那次的提示词估计，最后一次没计费");
        let (status, input, output) = dir.row(LEGACY_ROW_LATEST);
        assert_eq!((status.as_str(), input, output), ("cancelled", 1000, 0));
    }

    #[test]
    fn earlier_failed_attempts_are_added_to_the_settled_usage() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        account_failed_attempt();
        account_failed_attempt();
        record(
            Some(dir.path()),
            "openai",
            "m",
            &"说".repeat(1000),
            "答",
            Some((500, 100)),
        );
        drop(permit);
        let (status, input, output) = dir.row(LEGACY_ROW_LATEST);
        assert_eq!(
            (status.as_str(), input, output),
            ("succeeded", 500 + 2 * 1000, 100),
            "两次失败的尝试各按提示词估计记一笔，加上最后成功那次的真实用量"
        );
        assert_eq!(dir.used(), 2600);
    }

    #[test]
    fn a_possibly_billed_attempt_is_never_released() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        mark_possibly_billed();
        release_unbilled();
        drop(permit);
        assert_eq!(
            dir.used(),
            1000,
            "这次尝试可能已经计费，不能退，只留提示词那部分"
        );
    }

    #[test]
    fn a_stream_that_never_reached_the_provider_leaves_nothing_behind() {
        // 流式请求连接失败 / 被 4xx 拒绝：没有 mark，回退的普通请求也被拒——整笔退回，一个字都不留
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        account_failed_stream();
        release_unbilled();
        drop(permit);
        assert_eq!(dir.used(), 0);
    }

    #[test]
    fn a_stream_that_may_have_been_billed_is_counted_before_the_fallback() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        mark_possibly_billed();
        account_failed_stream();
        assert!(
            !MAYBE_BILLED.with(Cell::get),
            "记下之后标记要清掉，回退的普通请求自己重新判断"
        );
        // 回退的普通请求被 4xx 拒了：退回它自己那部分，流式那次留着
        release_unbilled();
        drop(permit);
        assert_eq!(dir.used(), 1000);
    }

    #[test]
    fn a_failed_release_puts_the_reservation_back_so_nothing_stays_running() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        RESERVATION.with(|slot| {
            if let Some(reservation) = slot.borrow_mut().as_mut() {
                reservation.database = dir
                    .path()
                    .with_file_name("not-a-dir")
                    .join("x")
                    .join("y.sqlite3");
            }
        });
        release_unbilled();
        assert!(
            RESERVATION.with(|slot| slot.borrow().is_some()),
            "没退成要把预留放回去，凭证放掉时才有人收尾"
        );
        RESERVATION.with(|slot| {
            if let Some(reservation) = slot.borrow_mut().as_mut() {
                reservation.database = dir.path().to_path_buf();
            }
        });
        drop(permit);
        let (status, input, _) = dir.row(LEGACY_ROW);
        assert_eq!(status, "cancelled", "不能留下一条永远 running 的行");
        assert_eq!(input, 1000);
    }

    #[test]
    fn accounting_state_does_not_leak_into_the_next_request() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        account_failed_attempt();
        mark_possibly_billed();
        drop(permit);
        let before = dir.used();
        assert_eq!(before, 2000, "这一次：更早 1000 + 最后一次 1000");
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("应通过");
        release_unbilled();
        drop(permit);
        assert_eq!(
            dir.used(),
            before,
            "上一次的「可能计费」标记和累计不能带进下一次请求：这一次被拒绝，应当整笔退回"
        );
    }

    // —— 全量整理：Fox 亲手确认后，旧版功能这一轮也不看每天额度 ——

    /// 控制文件：每天额度 `budget`，并且全量整理从 `started_at` 开始、预计整理 `total` 个。
    fn control_with_full_scan(budget: u64, paused: bool, started_at: &str, total: i64) -> String {
        format!(
            r#"{{"dailyBudgetTokens":{budget},"paused":{paused},"fullScan":{{"startedAt":"{started_at}","until":"{}","total":{total}}}}}"#,
            jobs::shift_db_time(started_at, jobs::FULL_SCAN_SECS).unwrap()
        )
    }

    #[test]
    fn a_full_scan_lets_legacy_requests_through_past_the_daily_budget() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        // 额度 10 万，24 小时已经用了 13 万
        dir.control(BUDGET_100K);
        record_at(dir.path(), &now, "p", "m", 90_000, 40_000).unwrap();
        let error = begin(Some(dir.path()), &"说".repeat(1000))
            .err()
            .expect("每天额度模式下应拒绝");
        assert!(is_refusal(&error), "{error}");
        assert!(
            error.contains("现在整理"),
            "拒绝的话要告诉他还有这条路：{error}"
        );
        // 全量整理进行中：放行，预留照常写、用量照常记
        dir.control(&control_with_full_scan(100_000, false, &now, 300));
        let permit = begin(Some(dir.path()), &"说".repeat(1000)).expect("全量整理期间应放行");
        assert_eq!(dir.used(), 130_000 + 4_000);
        drop(permit);
        // 日常额度关着（0）也一样
        dir.control(&control_with_full_scan(0, false, &now, 300));
        assert!(begin(Some(dir.path()), "提示词").is_ok());
    }

    #[test]
    fn a_full_scan_does_not_override_a_pause_or_outlive_its_48_hours() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        record_at(dir.path(), &now, "p", "m", 90_000, 40_000).unwrap();
        dir.control(&control_with_full_scan(100_000, true, &now, 300));
        let error = begin(Some(dir.path()), "提示词")
            .err()
            .expect("暂停时应拒绝");
        assert!(error.contains("暂停"), "{error}");
        // 开始于 49 小时前：已经到时间，回到每天额度
        let long_ago = jobs::shift_db_time(&now, -49 * 3600).unwrap();
        dir.control(&control_with_full_scan(100_000, false, &long_ago, 300));
        let error = begin(Some(dir.path()), "提示词")
            .err()
            .expect("到时间之后应按每天额度拒绝");
        assert!(error.contains("额度"), "{error}");
    }

    #[test]
    fn a_full_scan_past_its_ceiling_uses_the_daily_budget_without_unlimited_access() {
        // 预计只有 1 个：刹车线 300 万。超过后取消不限额，整理用量仍不占每日额度。
        let dir = Dir::new();
        let now = jobs::now_db_time();
        dir.control(&control_with_full_scan(100_000, false, &now, 1));
        record_at(dir.path(), &now, "p", "m", 3_000_000, 1).unwrap();
        let conn = Connection::open(dir.path()).unwrap();
        let control = jobs::read_control(&jobs::queue_control_path(dir.path()));
        assert!(!jobs::full_scan_open(&conn, &control, &now));
        assert_eq!(jobs::daily_tokens_used(&conn, &control, &now).unwrap(), 0);
        assert!(reserve_with(dir.path(), &now, 4_000, Policy::Legacy).is_ok());
        assert_eq!(dir.used(), 3_004_001, "按每天额度放行，预留照常记账");

        // 开始前的自动用量超过 10 万：同一个请求必须按每天额度拒绝。
        let before = jobs::shift_db_time(&now, -1).unwrap();
        record_at(dir.path(), &before, "p", "m", 100_001, 0).unwrap();
        assert_eq!(
            jobs::daily_tokens_used(&conn, &control, &now).unwrap(),
            100_001
        );
        let used_before = dir.used();
        let error = reserve_with(dir.path(), &now, 4_000, Policy::Legacy).unwrap_err();
        assert!(is_refusal(&error), "{error}");
        assert_eq!(
            error,
            refusal_for(&control, 100_001, 4_000, false).unwrap_err()
        );
        assert_eq!(dir.used(), used_before, "被拒绝不能留下预留");
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

    // ───────────── 「找方案」AI 精选：自己的额度，同一本账 ─────────────

    const RERANK_ROW: &str = "SELECT status, input_tokens, output_tokens FROM ai_jobs WHERE prompt_version = 'solve-rerank' ORDER BY created_at DESC, rowid DESC LIMIT 1";

    fn rerank_begin(dir: &Dir, prompt: &str) -> Result<SpendPermit, String> {
        solve_rerank(|| begin(Some(dir.path()), prompt))
    }

    #[test]
    fn legacy_and_rerank_reservations_work_immediately_after_a_large_full_scan() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        let start = jobs::shift_db_time(&now, -7200).unwrap();
        let inside = jobs::shift_db_time(&now, -5400).unwrap();
        let finish = jobs::shift_db_time(&now, -3600).unwrap();
        dir.control(r#"{"dailyBudgetTokens":50000,"paused":false}"#);
        jobs::begin_full_scan(&jobs::queue_control_path(dir.path()), &start, 400).unwrap();
        record_at(dir.path(), &inside, "fake", "fake", 2_000_000, 1_000_000).unwrap();
        let conn = Connection::open(dir.path()).unwrap();
        jobs::stop_full_scan(&conn, &jobs::queue_control_path(dir.path()), &finish).unwrap();
        record_at(dir.path(), &now, "fake", "fake", 40_000, 0).unwrap();
        let control = jobs::read_control(&jobs::queue_control_path(dir.path()));
        assert!(
            control.full_scan.is_none(),
            "已经收尾，不能靠不限额绕过每日闸门"
        );
        assert_eq!(
            jobs::daily_tokens_used(&conn, &control, &now).unwrap(),
            40_000
        );
        assert!(reserve_with(dir.path(), &now, 5_000, Policy::Legacy).is_ok());
        assert!(reserve_with(dir.path(), &now, 5_000, Policy::SolveRerank).is_ok());
        assert_eq!(
            jobs::daily_tokens_used(&conn, &control, &now).unwrap(),
            50_000
        );
        assert_eq!(dir.used(), 3_050_000, "全部真实用量和预留照常记账");
        let error = reserve_with(dir.path(), &now, 1, Policy::Legacy).unwrap_err();
        assert_eq!(
            error,
            refusal_for(&control, 50_000, 1, false).unwrap_err(),
            "自动用量超额度时原拒绝文案保持不变"
        );
    }

    #[test]
    fn daily_exclusions_leave_rerank_own_allowance_and_automatic_refusals_unchanged() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        let start = jobs::shift_db_time(&now, -7200).unwrap();
        let inside = jobs::shift_db_time(&now, -5400).unwrap();
        let finish = jobs::shift_db_time(&now, -3600).unwrap();
        dir.control(
            &serde_json::json!({
                "dailyBudgetTokens": 50_000, "paused": false,
                "recentFullScans": [{"startedAt": start, "finishedAt": finish}]
            })
            .to_string(),
        );
        // AI 精选即使落在全量整理时段里，自己的这份额度也照样计入，不改原口径。
        record_at_tagged(
            dir.path(),
            &inside,
            "fake",
            "fake",
            25_000,
            0,
            SOLVE_RERANK_TAG,
        )
        .unwrap();
        record_at(dir.path(), &now, "fake", "fake", 50_001, 0).unwrap();
        let control = jobs::read_control(&jobs::queue_control_path(dir.path()));
        let conn = Connection::open(dir.path()).unwrap();
        assert_eq!(
            jobs::daily_tokens_used(&conn, &control, &now).unwrap(),
            50_001
        );
        assert_eq!(
            reserve_with(dir.path(), &now, 1, Policy::Legacy).unwrap_err(),
            refusal_for(&control, 50_001, 1, false).unwrap_err()
        );
        assert_eq!(
            reserve_with(dir.path(), &now, 1, Policy::SolveRerank).unwrap_err(),
            refusal_for_rerank(&control, 25_000, 1, false).unwrap_err()
        );
        assert_eq!(dir.used(), 75_001, "被拒绝不能留下预留");
    }

    #[test]
    fn rerank_reservations_are_ledger_rows_tagged_for_it() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = rerank_begin(&dir, &"说".repeat(1000)).expect("应通过");
        // 和旧功能一样：账本行、队列看得见、不算队列任务；只是标记不同
        assert_eq!(dir.row(RERANK_ROW), ("running".to_owned(), 4000, 0));
        assert_eq!(dir.used(), 4000);
        let legacy: i64 = Connection::open(dir.path())
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE prompt_version = 'legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy, 0, "AI 精选的行不能标成旧功能");
        let status = jobs::queue_status(dir.path()).unwrap();
        assert_eq!((status.running, status.queued), (0, 0));
        drop(permit);
    }

    #[test]
    fn rerank_has_its_own_allowance_and_ignores_what_the_queue_has_used() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        // 队列额度 10 万，后台整理已经用了 50 万：旧功能被拒，AI 精选照样能用自己的那份
        dir.control(BUDGET_100K);
        record_at(dir.path(), &now, "p", "m", 400_000, 100_000).unwrap();
        let error = begin(Some(dir.path()), "提示词")
            .err()
            .expect("旧功能应被拒");
        assert!(is_refusal(&error), "{error}");
        let permit = rerank_begin(&dir, "提示词").expect("AI 精选有自己的额度");
        drop(permit);
    }

    #[test]
    fn rerank_usage_is_counted_only_against_itself_but_everyone_sees_it() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        dir.control(BUDGET_100K);
        // 旧功能用了 29 万：不占 AI 精选的额度
        record_at(dir.path(), &now, "p", "m", 250_000, 40_000).unwrap();
        let first = reserve_with(dir.path(), &now, 5_000, Policy::SolveRerank);
        assert!(first.is_ok(), "{first:?}");
        // AI 精选自己的用量记进同一本账：队列和旧功能的「最近 24 小时已用」都含它
        assert_eq!(dir.used(), 295_000);
    }

    #[test]
    fn rerank_stops_at_its_own_daily_allowance_and_says_so() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        // 卡 34：AI 精选的上限 = min(30 万, 每天额度的一半)。这条测的是「30 万」这个上限本身，
        // 所以每天额度要给到 60 万（原来是 10 万，那样上限只有 5 万）；下面的断言没有改
        dir.control(BUDGET_600K);
        record_at_tagged(
            dir.path(),
            &now,
            "p",
            "m",
            200_000,
            95_000,
            SOLVE_RERANK_TAG,
        )
        .unwrap();
        // 29.5 万已经用了：再预留 5001 会超过 30 万
        let error = reserve_with(dir.path(), &now, 5_001, Policy::SolveRerank)
            .expect_err("超过自己的额度应拒绝");
        assert!(is_refusal(&error), "{error}");
        assert!(error.contains("AI 精选"), "{error}");
        assert!(error.contains("所以没有发请求"), "{error}");
        assert!(error.contains("本地排序"), "{error}");
        // 刚好放得下（正好 30 万）的不拦
        assert!(reserve_with(dir.path(), &now, 5_000, Policy::SolveRerank).is_ok());
    }

    #[test]
    fn rerank_usage_older_than_a_day_is_forgotten() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        dir.control(BUDGET_100K);
        let long_ago = jobs::shift_db_time(&now, -25 * 3600).unwrap();
        record_at_tagged(
            dir.path(),
            &long_ago,
            "p",
            "m",
            295_000,
            0,
            SOLVE_RERANK_TAG,
        )
        .unwrap();
        // 25 小时前的 29.5 万不算：要是还算着，这次 1 万就超过上限了（每天额度 10 万，上限 5 万）
        assert!(reserve_with(dir.path(), &now, 10_000, Policy::SolveRerank).is_ok());
    }

    #[test]
    fn rerank_follows_the_pause_and_the_zero_budget_gates() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        // 每天额度是 0：Fox 还没开过 AI 额度，AI 精选也不能花钱
        let error = rerank_begin(&dir, "提示词").err().expect("额度 0 应拒绝");
        assert!(is_refusal(&error), "{error}");
        assert!(error.contains("额度是 0"), "{error}");
        assert_eq!(dir.used(), 0, "被拒绝不能留下预留");
        // 暂停
        dir.control(r#"{"dailyBudgetTokens":100000,"paused":true}"#);
        let error = rerank_begin(&dir, "提示词").err().expect("暂停应拒绝");
        assert!(error.contains("暂停"), "{error}");
        // 全量整理进行中（Fox 亲手开的）：每天额度是 0 也放行
        dir.control(&control_with_full_scan(0, false, &now, 300));
        assert!(rerank_begin(&dir, "提示词").is_ok());
        // 全量整理 + 暂停：照样拦
        dir.control(&control_with_full_scan(0, true, &now, 300));
        assert!(rerank_begin(&dir, "提示词").is_err());
    }

    #[test]
    fn recording_settles_the_rerank_row_with_real_usage() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let _scope = dir.use_in_this_thread();
        let permit = solve_rerank(|| check_before_request(&"说".repeat(500))).expect("应通过");
        assert_eq!(dir.row(RERANK_ROW).0, "running");
        record_after_request("openai", "m", "p", "c", Some((2_800, 650)));
        assert_eq!(dir.row(RERANK_ROW), ("succeeded".to_owned(), 2_800, 650));
        drop(permit);
        // 记完账之后凭证放掉，不会再改这一行
        assert_eq!(dir.row(RERANK_ROW), ("succeeded".to_owned(), 2_800, 650));
    }

    #[test]
    fn a_rerank_that_ends_without_recording_keeps_only_the_prompt_part() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let _scope = dir.use_in_this_thread();
        let prompt = "说".repeat(800);
        let permit = solve_rerank(|| check_before_request(&prompt)).expect("应通过");
        drop(permit);
        assert_eq!(dir.row(RERANK_ROW), ("cancelled".to_owned(), 800, 0));
    }

    #[test]
    fn a_cancelled_rerank_reservation_is_replaced_by_a_row_with_the_same_tag() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let _scope = dir.use_in_this_thread();
        let permit = solve_rerank(|| check_before_request("提示词")).expect("应通过");
        // 恢复动作把预留行取消了
        Connection::open(dir.path())
            .unwrap()
            .execute(
                "UPDATE ai_jobs SET status = 'cancelled' WHERE prompt_version = 'solve-rerank'",
                [],
            )
            .unwrap();
        record_after_request("openai", "m", "p", "c", Some((1_000, 200)));
        let rows: i64 = Connection::open(dir.path())
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE prompt_version = 'solve-rerank' AND status = 'succeeded' AND input_tokens = 1000",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1, "另记的那一行也要标成 AI 精选");
        drop(permit);
    }

    #[test]
    fn the_rerank_scope_ends_with_its_closure_even_when_it_panics() {
        assert!(!reranking());
        let nested = solve_rerank(|| solve_rerank(reranking));
        assert!(nested);
        assert!(!reranking(), "嵌套结束后要回到没在精选");
        let result = std::panic::catch_unwind(|| solve_rerank(|| panic!("出事了")));
        assert!(result.is_err());
        assert!(!reranking(), "panic 也不能让后面的旧功能请求被当成 AI 精选");
    }

    #[test]
    fn plain_requests_are_never_tagged_as_rerank() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = begin(Some(dir.path()), "提示词").expect("应通过");
        let rerank_rows: i64 = Connection::open(dir.path())
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM ai_jobs WHERE prompt_version = 'solve-rerank'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rerank_rows, 0);
        drop(permit);
    }

    #[test]
    fn refusal_for_rerank_is_a_plain_table() {
        let control = |budget: u64, paused: bool| jobs::QueueControl {
            paused,
            daily_budget_tokens: budget,
            ..jobs::QueueControl::default()
        };
        // 卡 34：上限跟着每天额度走（最多 30 万、不超过每天额度的一半），原来表里「有额度」写的是 1，
        // 现在它的上限是 0；换成 60 万（上限正好 30 万）。每一行的期望结果都没有改
        const AMPLE: u64 = 600_000;
        assert!(refusal_for_rerank(&control(AMPLE, false), 0, 5_000, false).is_ok());
        assert!(refusal_for_rerank(&control(0, false), 0, 5_000, true).is_ok());
        assert!(refusal_for_rerank(&control(0, false), 0, 5_000, false).is_err());
        assert!(refusal_for_rerank(&control(1, true), 0, 5_000, true).is_err());
        assert!(refusal_for_rerank(&control(AMPLE, false), 295_000, 5_000, false).is_ok());
        assert!(refusal_for_rerank(&control(AMPLE, false), 295_001, 5_000, false).is_err());
        assert!(refusal_for_rerank(&control(AMPLE, false), u64::MAX, 1, false).is_err());
    }

    // ───────────── 卡 34：AI 精选的额度跟着每天额度走 ─────────────

    fn control_of(budget: u64, paused: bool) -> jobs::QueueControl {
        jobs::QueueControl {
            paused,
            daily_budget_tokens: budget,
            ..jobs::QueueControl::default()
        }
    }

    #[test]
    fn the_rerank_cap_is_half_the_daily_budget_but_never_above_the_ceiling() {
        for (budget, expected) in [
            (1, 0),
            (2, 1),
            (99_999, 49_999),
            (100_000, 50_000),
            (100_001, 50_000),
            (599_999, 299_999),
            (600_000, 300_000),
            (600_001, 300_000),
            (10_000_000, 300_000),
            (u64::MAX, 300_000),
        ] {
            assert_eq!(
                rerank_cap(&control_of(budget, false), false),
                expected,
                "每天额度 {budget}"
            );
        }
        // 全量整理进行中（Fox 亲手开的）：每天额度是多少（包括 0）都按上限
        for budget in [0, 1, 100_000, 10_000_000] {
            assert_eq!(
                rerank_cap(&control_of(budget, false), true),
                SOLVE_RERANK_DAILY_TOKENS,
                "全量整理 + 每天额度 {budget}"
            );
        }
        assert_eq!(SOLVE_RERANK_DAILY_TOKENS, 300_000, "上限的上限不改");
    }

    #[test]
    fn rerank_edges_follow_the_effective_cap() {
        // （每天额度，有没有全量整理，有效上限）：用量 + 预计正好等于上限放行，再多 1 个拒绝
        for (budget, scan_open, cap) in [
            (0, true, 300_000_u64),
            (100_000, false, 50_000),
            (100_000, true, 300_000),
            (600_000, false, 300_000),
            (10_000_000, false, 300_000),
        ] {
            let control = control_of(budget, false);
            let label = format!("每天额度 {budget}，全量整理 {scan_open}，上限 {cap}");
            assert!(
                refusal_for_rerank(&control, cap - 5_000, 5_000, scan_open).is_ok(),
                "{label}：已用 + 预计 == 上限应放行"
            );
            assert!(
                refusal_for_rerank(&control, cap - 5_000, 5_001, scan_open).is_err(),
                "{label}：多 1 个应拒绝"
            );
            assert!(
                refusal_for_rerank(&control, 0, cap, scan_open).is_ok(),
                "{label}"
            );
            assert!(
                refusal_for_rerank(&control, 0, cap + 1, scan_open).is_err(),
                "{label}"
            );
        }
    }

    #[test]
    fn rerank_zero_budget_and_pause_still_refuse_with_the_same_wording() {
        let zero = refusal_for_rerank(&control_of(0, false), 0, 1, false).unwrap_err();
        assert_eq!(
            zero,
            format!("{REFUSAL_PREFIX}每天额度是 0，「找方案」的 AI 精选还不能用，这次按本地排序。")
        );
        // 暂停照旧拒绝；全量整理也越不过暂停
        for scan_open in [false, true] {
            let paused =
                refusal_for_rerank(&control_of(100_000, true), 0, 1, scan_open).unwrap_err();
            assert_eq!(
                paused,
                format!("{REFUSAL_PREFIX}AI 整理现在是暂停状态，「找方案」的 AI 精选也先停着，这次按本地排序。")
            );
        }
        // 每天额度 1：一半取整是 0，没有任何空间（不是「还能用 30 万」）
        assert!(refusal_for_rerank(&control_of(1, false), 0, 1, false).is_err());
    }

    #[test]
    fn the_refusal_names_the_effective_cap_not_the_ceiling() {
        for (budget, scan_open, shown) in [
            (100_000, false, "5.0 万 token"),
            (40_000, false, "2.0 万 token"),
            (10_000, false, "5000 token"),
            (600_000, false, "30.0 万 token"),
            (10_000_000, false, "30.0 万 token"),
            (0, true, "30.0 万 token"),
        ] {
            let error = refusal_for_rerank(&control_of(budget, false), 0, u64::MAX / 2, scan_open)
                .unwrap_err();
            assert!(
                error.contains(&format!("会超过它每天 {shown} 的额度")),
                "每天额度 {budget}：{error}"
            );
            assert!(is_refusal(&error), "{error}");
        }
    }

    #[test]
    fn a_small_daily_budget_shrinks_rerank_through_reserve_with() {
        // 经过 reserve_with 的集成：10 万额度、AI 精选已经用了 4.5 万，这次预计 1 万 → 4.5 + 1 = 5.5 万，超过 5 万，拒绝
        let dir = Dir::new();
        let now = jobs::now_db_time();
        dir.control(BUDGET_100K);
        record_at_tagged(dir.path(), &now, "p", "m", 40_000, 5_000, SOLVE_RERANK_TAG).unwrap();
        let error = reserve_with(dir.path(), &now, 10_000, Policy::SolveRerank)
            .expect_err("4.5 万 + 1 万超过有效上限 5 万");
        assert!(is_refusal(&error), "{error}");
        assert!(
            error.contains("会超过它每天 5.0 万 token 的额度"),
            "{error}"
        );
        assert!(error.contains("所以没有发请求"), "{error}");
        assert_eq!(dir.used(), 45_000, "被拒绝不能留下预留");
        // 4.5 万 + 5 千 = 5 万，正好放行；再多 1 个拒绝（刚放行的预留也算在用量里）
        assert!(reserve_with(dir.path(), &now, 5_000, Policy::SolveRerank).is_ok());
        assert!(reserve_with(dir.path(), &now, 1, Policy::SolveRerank).is_err());
        assert_eq!(dir.used(), 50_000);
    }

    #[test]
    fn a_full_scan_lifts_rerank_to_the_ceiling_even_with_a_small_daily_budget() {
        let dir = Dir::new();
        let now = jobs::now_db_time();
        // 每天额度只有 10 万（平时 AI 精选只有 5 万），但全量整理在进行：上限 30 万
        dir.control(&control_with_full_scan(100_000, false, &now, 300));
        record_at_tagged(dir.path(), &now, "p", "m", 250_000, 0, SOLVE_RERANK_TAG).unwrap();
        assert!(reserve_with(dir.path(), &now, 50_000, Policy::SolveRerank).is_ok());
        let error = reserve_with(dir.path(), &now, 1, Policy::SolveRerank)
            .expect_err("30 万用满了，再多 1 个也拒绝");
        assert!(
            error.contains("会超过它每天 30.0 万 token 的额度"),
            "{error}"
        );
    }

    #[test]
    fn a_slow_failure_books_at_least_the_timeout_estimate() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = rerank_begin(&dir, &"说".repeat(1000)).expect("应通过");
        account_slow_failure(4096);
        drop(permit);
        // 这一笔 = max(1 万, 提示词估计 1000 + 输出上限 4096) = 1 万：提示词那部分 1000 + 补的 9000
        assert_eq!(dir.row(RERANK_ROW), ("cancelled".to_owned(), 10_000, 0));
        assert_eq!(dir.used(), 10_000);
        assert!(
            dir.row(RERANK_ROW).1 >= jobs::TIMEOUT_ATTEMPT_TOKEN_ESTIMATE,
            "不少于队列对超时的记法"
        );
    }

    #[test]
    fn a_slow_failure_with_a_long_prompt_is_booked_at_the_worst_case() {
        // 提示词估计 6500（AI 精选提示词的上限）：6500 + 输出上限 4096 = 10596，比 1 万多，按最坏情形记
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = rerank_begin(&dir, &"说".repeat(6_500)).expect("应通过");
        account_slow_failure(4096);
        drop(permit);
        assert_eq!(dir.row(RERANK_ROW), ("cancelled".to_owned(), 10_596, 0));
    }

    #[test]
    fn a_slow_failure_without_an_output_limit_is_neither_topped_up_nor_reduced_past_the_floor() {
        // 没有输出上限（传 0）：提示词估计超过 1 万就是提示词估计，一分不补，也不会比它少
        let dir = Dir::new();
        dir.control(BUDGET_600K);
        let permit = rerank_begin(&dir, &"说".repeat(12_000)).expect("应通过");
        account_slow_failure(0);
        drop(permit);
        assert_eq!(dir.row(RERANK_ROW), ("cancelled".to_owned(), 12_000, 0));
        // 提示词估计不到 1 万：补到 1 万
        let permit = rerank_begin(&dir, &"说".repeat(1_000)).expect("应通过");
        account_slow_failure(0);
        drop(permit);
        assert_eq!(dir.row(RERANK_ROW), ("cancelled".to_owned(), 10_000, 0));
    }

    #[test]
    fn a_slow_failure_is_never_refunded_and_keeps_the_earlier_attempts() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = rerank_begin(&dir, &"说".repeat(1000)).expect("应通过");
        account_failed_attempt(); // 更早失败但可能计费的一次：提示词估计 1000
        account_slow_failure(4096); // 最后这一次等了很久才失败：这一次至少 1 万，补 9000
        release_unbilled(); // 这次可能已经计费，退不了
        drop(permit);
        assert_eq!(
            dir.row(RERANK_ROW),
            ("cancelled".to_owned(), 11_000, 0),
            "更早那次 1000 + 这一次 max(1 万, 1000 + 4096)"
        );
    }

    #[test]
    fn a_slow_failure_without_a_reservation_does_nothing() {
        // 探测（不看额度、没有预留行）：没有账可补，不能 panic，也不能把状态带给下一次请求
        let dir = Dir::new();
        let permit = probe(|| begin(Some(dir.path()), "测试连接")).expect("探测不看额度");
        account_slow_failure(4096);
        drop(permit);
        assert_eq!(dir.used(), 0);
        dir.control(BUDGET_100K);
        let permit = rerank_begin(&dir, &"说".repeat(1000)).expect("应通过");
        release_unbilled();
        drop(permit);
        assert_eq!(
            dir.used(),
            0,
            "上一次的补账和「可能计费」标记不能带进下一次"
        );
    }

    #[test]
    fn the_slow_failure_top_up_does_not_leak_into_the_next_request() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        let permit = rerank_begin(&dir, &"说".repeat(1000)).expect("应通过");
        account_slow_failure(4096);
        drop(permit);
        assert_eq!(dir.used(), 10_000);
        let permit = rerank_begin(&dir, &"说".repeat(1000)).expect("应通过");
        release_unbilled(); // 这一次被服务商直接拒绝：整笔退回，不能带着上一次补的 9000
        drop(permit);
        assert_eq!(dir.used(), 10_000);
    }

    // ───────────── 卡 37：提示正文换成现在的叫法（前缀和判断逻辑一个字没动） ─────────────

    /// 旧叫法：卡 37 之后，每一条额度拒绝提示的正文里都不该再出现。
    const OLD_WORDING: [&str; 10] = [
        "工具箱",
        "AI 自动整理",
        "AI 额度",
        "全量整理",
        "不限额",
        "标签网络",
        "整批解析",
        "AI 搜索",
        "旧版",
        "闸门",
    ];

    /// 是额度拒绝（前缀没换、`is_refusal` 认得），正文里没有旧叫法。
    fn assert_refusal_with_current_names(message: &str) {
        assert!(is_refusal(message), "{message}");
        let body = message
            .strip_prefix(REFUSAL_PREFIX)
            .expect("前缀不能换，不然前端和找方案认不出额度拒绝");
        for old in OLD_WORDING {
            assert!(!body.contains(old), "还在说旧叫法「{old}」：{message}");
        }
    }

    #[test]
    fn the_prefix_stays_and_the_daily_refusals_use_the_current_names() {
        assert_eq!(
            REFUSAL_PREFIX, "AI 额度：",
            "前缀不换：前端和「找方案」靠它认额度拒绝"
        );
        let paused = refusal_for(&control_of(100_000, true), 0, 1, false).unwrap_err();
        assert_eq!(
            paused,
            format!("{REFUSAL_PREFIX}AI 整理现在是暂停状态，其它 AI 功能也一起停着。要继续请先到「设置 → 自动整理与备份」里点「继续整理」。")
        );
        let zero = refusal_for(&control_of(0, false), 0, 1, false).unwrap_err();
        assert_eq!(
            zero,
            format!("{REFUSAL_PREFIX}每天额度是 0，所有 AI 功能都还不能用。要用请先到「设置 → 自动整理与备份」的「每天自动整理」里开启每天额度，或在同一页点「现在整理」。")
        );
        // 24 小时已经用了 13 万，这一次预计 4000，每天额度 10 万
        let over = refusal_for(&control_of(100_000, false), 130_000, 4_000, false).unwrap_err();
        assert_eq!(
            over,
            format!("{REFUSAL_PREFIX}最近 24 小时已经用了约 13.0 万 token（整理现有收藏用的不算），这一次预计还要约 4000 token，会超过每天 10.0 万 token 的额度，所以没有发请求。过一阵额度恢复后再试，或到「设置 → 自动整理与备份」里调高每天额度；想马上整理完，可以点「现在整理」。")
        );
        for message in [&paused, &zero, &over] {
            assert_refusal_with_current_names(message);
        }
        // 判断逻辑没动：一次性全部整理进行中放行额度，暂停照样拦
        assert!(refusal_for(&control_of(0, false), 130_000, 4_000, true).is_ok());
        assert!(refusal_for(&control_of(0, true), 0, 1, true).is_err());
    }

    #[test]
    fn the_rerank_refusals_use_the_current_names_too() {
        let zero = refusal_for_rerank(&control_of(0, false), 0, 1, false).unwrap_err();
        assert_eq!(
            zero,
            format!("{REFUSAL_PREFIX}每天额度是 0，「找方案」的 AI 精选还不能用，这次按本地排序。")
        );
        let paused = refusal_for_rerank(&control_of(100_000, true), 0, 1, false).unwrap_err();
        assert_eq!(
            paused,
            format!("{REFUSAL_PREFIX}AI 整理现在是暂停状态，「找方案」的 AI 精选也先停着，这次按本地排序。")
        );
        // 每天额度 10 万，AI 精选的有效上限 5 万，已经用了 4.5 万，这一次预计 1 万
        let over =
            refusal_for_rerank(&control_of(100_000, false), 45_000, 10_000, false).unwrap_err();
        assert_eq!(
            over,
            format!("{REFUSAL_PREFIX}「找方案」的 AI 精选最近 24 小时已经用了约 4.5 万 token，这一次预计还要约 1.0 万 token，会超过它每天 5.0 万 token 的额度，所以没有发请求；这次按本地排序，过一阵额度恢复后再试。")
        );
        for message in [&zero, &paused, &over] {
            assert_refusal_with_current_names(message);
        }
    }

    #[test]
    fn the_no_account_refusal_says_ai_features_not_old_version() {
        let dir = Dir::new();
        dir.control(BUDGET_100K);
        Connection::open(dir.path())
            .unwrap()
            .execute("DELETE FROM github_accounts", [])
            .unwrap();
        let error = begin(Some(dir.path()), "提示词")
            .err()
            .expect("没有账号应拒绝");
        assert_eq!(
            error,
            format!("{REFUSAL_PREFIX}还没有连接 GitHub 账号，用量没法记账，所以 AI 功能先不能用。")
        );
        assert_refusal_with_current_names(&error);
    }

    #[test]
    fn the_guard_not_installed_refusal_wording_is_plain() {
        // 正式运行时没装上额度检查会走到这一句，单元测试里 `begin` 对「没装」放行，所以读源码确认。
        // 只看测试模块之前的正式代码；分隔符用 concat! 拼出来，免得这条测试自己把自己写进被检查的文字里。
        let source = include_str!("spend_guard.rs");
        let production = source
            .split(concat!("#[cfg(test)]\n", "mod tests {"))
            .next()
            .unwrap_or(source);
        assert!(production.contains(
            "{REFUSAL_PREFIX}每天额度的检查还没有准备好，AI 功能先不能用。重新打开应用再试。"
        ));
    }
}
