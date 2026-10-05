//! README 变了以后，决定要不要把理解卡重新排队。
//!
//! 只读本地缓存，不联网，也不改额度。明显改动才入队；徽章、版本号、错别字这类小改动只更新记过的哈希。
//! 这个仓库最近 24 小时内整理失败或被取消过，就先不排（见 `insert_job`），免得每次同步都白花一次额度。
//!
//! 写库的规矩：文字处理（摘录、清洗、比较）都在写事务**之外**做完；事务（`IMMEDIATE`）里只复核几个短字段、再照着写。
//! 一份奇怪的超长 README 清洗得再慢，也不会占着写锁挡住别的写库操作（「database is locked」）。
//!
//! 同步里批量抓 README 的循环（`fetch_in_batches`）也在这里：总时限、连续失败就停；抓取和落库用 `ReadmeIo` 注入，测试不联网。

use crate::card_pipeline::build_readme_excerpt;
use crate::github::ReadmeDocument;
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};

/// 规范化后的相似度低于这个数，才算「明显改动」。
pub const MATERIAL_SIMILARITY: f64 = 0.80;
/// 一次同步最多重新抓这么多个 README，剩下的留给下次。
pub const MAX_REFRESH_PER_RUN: usize = 150;
/// README 明显改了以后重新整理的优先级。比新收藏（3）靠后，比补旧账（6）靠前。
pub const README_CHANGED_PRIORITY: i64 = 5;
/// 同步里批量抓老 README 的总时限（秒）：从开始抓起过了这么久，就不再开新的一批，剩下的留给下一次同步。
/// 只在批与批之间看表：已经发出去的这一批（最多 6 个请求，每个请求自己有超时）会做完。
pub const REFRESH_TIME_BUDGET_SECS: u64 = 120;
/// 同步里抓**新收藏**的 README（第二步）的总时限（秒），比老 README 短得多：新收藏要等这一步做完才能排队，
/// 而它们的 id 只在内存里，等得越久，中途关掉应用就越容易让它们永远排不上队。一次同步通常只有几个到几十个新收藏，
/// 抓不完的照样先排队，README 之后由第四步和下一次同步补上。
pub const NEW_STAR_README_BUDGET_SECS: u64 = 30;
/// 连续这么多个抓取失败（断网、被限流、本地库写不进）就停，别把剩下的全试一遍。抓到一份（含「没有 README」）就清零。
pub const REFRESH_STOP_AFTER_FAILURES: usize = 5;
/// 事务外读 README 时的安全上限（字节，按字符边界截，不会切坏多字节字符）。
/// **不是为了省时间**（清洗函数已经是线性的，1 MB 一行的恶意 README 也只要零点几秒），所以放得比 GitHub 能返回的 README
/// （接口最多 1 MB）还大：摘录必须和理解卡流水线写 `readme_basis` 时用的输入——整份 README——一样，
/// 否则大 README 的摘录对不上基准（有一大节「安装 / 常见问题」排在后面时，哪怕只改一个徽章也会被当成明显改动、白花一次重做）。
pub const README_EXCERPT_INPUT_BYTES: usize = 4 * 1024 * 1024;

const MAX_COMPARE_CHARS: usize = 20_000;
const LINK_TEXT_CAP: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoReadmeState {
    pub id: String,
    pub full_name: String,
    pub pushed_at: Option<String>,
    pub readme_fetched_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FollowReport {
    /// 这次真正评估了几份有理解卡、有 README 缓存、且没被确认的仓库。
    pub checked: usize,
    /// `readme_hash` 有值，且和当前 `content_hash` 不同。
    pub changed: usize,
    /// 算作明显改动的份数（含「以前没有 README，现在有了」）。
    pub material: usize,
    /// 这次新插进队列的任务数。已经排着或正在跑的不重复计。
    pub queued: usize,
    /// 明显改动、本来该排队，但这个仓库最近 24 小时内整理失败或被取消过，这次先不排的份数。
    pub held_back: usize,
}

/// 转小写，去掉图片、链接地址、裸网址、HTML、数字和标点，只留字母和汉字。最多 20000 个字符。
/// 没结束的 `<` 或 `[` 就停，已经输出的留下，后面的整段丢掉。
pub fn normalize_for_compare(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut kept = 0usize;
    let mut index = 0usize;
    while index < chars.len() && kept < MAX_COMPARE_CHARS {
        let ch = chars[index];
        if ch == '!' && chars.get(index + 1) == Some(&'[') {
            match skip_image(&chars, index) {
                Some(next) => {
                    index = next;
                    continue;
                }
                None => break,
            }
        }
        if ch == '[' {
            match take_link(&chars, index, &mut out, &mut kept) {
                Some(next) => {
                    index = next;
                    continue;
                }
                None => break,
            }
        }
        if ch == '<' {
            match skip_html(&chars, index) {
                Some(next) => {
                    index = next;
                    continue;
                }
                None => break,
            }
        }
        if is_url_at(&chars, index) {
            index = skip_until_whitespace(&chars, index);
            continue;
        }
        if ch.is_alphabetic() {
            push_letters(ch, &mut out, &mut kept);
        }
        index += 1;
    }
    out
}

/// 两边都是已经规范化的字符串。不足 4 个字符的整串当 1 个片段。
pub fn similarity(left: &str, right: &str) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let left_set = shingles(left);
    let right_set = shingles(right);
    let intersection = left_set.intersection(&right_set).count();
    let union = left_set.union(&right_set).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

/// 现在的摘录空了（README 没了或几乎没内容）→ 不重新整理。
/// 以前没有依据、现在有内容 → 算明显改动。其余看 Jaccard。
pub fn is_material_change(basis: Option<&str>, current_excerpt: Option<&str>) -> bool {
    let current = current_excerpt
        .map(normalize_for_compare)
        .unwrap_or_default();
    if current.is_empty() {
        return false;
    }
    let basis = basis.map(normalize_for_compare).unwrap_or_default();
    if basis.is_empty() {
        return true;
    }
    similarity(&basis, &current) < MATERIAL_SIMILARITY
}

/// 没有缓存的在前；其余按 `readme_fetched_at` 从旧到新。最多 `MAX_REFRESH_PER_RUN` 个。
/// 同步里现在用 `refresh_targets_excluding`（要去掉第二步抓过的）；这个不带名单的写法留着给测试，`cargo check` 看不见测试调用。
#[allow(dead_code)]
pub fn refresh_targets(repos: &[RepoReadmeState]) -> Vec<String> {
    refresh_targets_excluding(repos, &HashSet::new())
}

/// 同 `refresh_targets`，但先去掉 `exclude` 里的仓库（同步里第一步已经抓过的新收藏），再取前 `MAX_REFRESH_PER_RUN` 个：
/// 去掉的不占这 150 个名额。
pub fn refresh_targets_excluding(
    repos: &[RepoReadmeState],
    exclude: &HashSet<String>,
) -> Vec<String> {
    let mut chosen: Vec<&RepoReadmeState> = repos
        .iter()
        .filter(|repo| !exclude.contains(repo.id.as_str()) && needs_refresh(repo))
        .collect();
    chosen.sort_by(|left, right| {
        match (
            left.readme_fetched_at.is_none(),
            right.readme_fetched_at.is_none(),
        ) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => left
                .readme_fetched_at
                .as_deref()
                .unwrap_or("")
                .cmp(right.readme_fetched_at.as_deref().unwrap_or(""))
                .then_with(|| left.id.cmp(&right.id)),
        }
    });
    chosen
        .into_iter()
        .take(MAX_REFRESH_PER_RUN)
        .map(|repo| repo.id.clone())
        .collect()
}

/// 一次批量抓 README 的保护。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchLimits {
    /// 总时限：从开始抓起过了这么久，就不再开新的一批（已经发出去的这一批会做完），剩下的留给下一次。
    pub time_budget: Duration,
    /// 连续这么多个失败就停。
    pub stop_after_failures: usize,
}

impl FetchLimits {
    /// 同步里抓老 README 用的：`REFRESH_TIME_BUDGET_SECS` 秒，连续 `REFRESH_STOP_AFTER_FAILURES` 个失败。
    pub const fn for_sync() -> Self {
        Self {
            time_budget: Duration::from_secs(REFRESH_TIME_BUDGET_SECS),
            stop_after_failures: REFRESH_STOP_AFTER_FAILURES,
        }
    }

    /// 同步里抓新收藏的 README 用的：`NEW_STAR_README_BUDGET_SECS` 秒，连续 `REFRESH_STOP_AFTER_FAILURES` 个失败。
    pub const fn for_new_stars() -> Self {
        Self {
            time_budget: Duration::from_secs(NEW_STAR_README_BUDGET_SECS),
            stop_after_failures: REFRESH_STOP_AFTER_FAILURES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// 过了总时限。
    TimeBudget,
    /// 连续失败太多。
    Failures,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FetchReport {
    /// 抓到并落了库的（含「内容没变、只拨了检查时间」的）。
    pub refreshed: usize,
    /// GitHub 说没有 README 的。
    pub missed: usize,
    /// 抓取失败、线程异常、或抓到了落库失败的。
    pub failed: usize,
    /// 实际发出过抓取的仓库（不论成败）。同步里第二步抓过的，第四步不再抓。
    pub attempted: Vec<String>,
    /// 因为什么提前停的；没有就是全部处理完了。
    pub stopped: Option<StopReason>,
}

/// 抓一批 README 要用到的外部动作。线上是 GitHub + 本地库 + 进度事件；测试换成假的，不联网。
pub trait ReadmeIo: Sync {
    /// 抓一个仓库的 README。`Ok(None)` 是 GitHub 说没有。会在抓取线程里调用。
    fn fetch(&self, repo_id: &str, full_name: &str) -> Result<Option<ReadmeDocument>, String>;
    /// 把抓到的 README 落库（内容没变就只拨检查时间）。在循环所在的线程里依次调用。
    fn store(&self, readme: &ReadmeDocument) -> Result<(), String>;
    /// 现在是几点（单调时钟）。总时限用它量；测试注入假时钟。
    fn now(&self) -> Instant;
    /// 开始抓一批之前报进度：已处理几个、一共几个、这一批是哪几个仓库。
    fn progress(&self, done: usize, total: usize, batch_names: &[String]);
}

/// 分批并发抓 README：每批最多 `batch_size` 个同时发出，一批的结果依次落库，再开下一批。
/// - 开每一批之前看表：从进来起过了 `limits.time_budget` 就停（`StopReason::TimeBudget`）；
/// - 连续 `limits.stop_after_failures` 个失败就停（`StopReason::Failures`）：断网或被限流时，不把剩下的全试一遍。
///   「连续失败」只数「网或本地库有问题」的：抓取出错（断网、超时、限流、服务端错）、抓取线程异常、抓到了落库失败。
///   抓到一份、GitHub 明确说「没有 README」、或者 GitHub 回了但**这一个仓库**的 README 本身不能用（超过 1 MB 没有内容、
///   不是 UTF-8、响应解析不了：`is_repo_problem`）都说明网是通的，计数清零——最后一种仍记进 `failed`，
///   免得几个永远抓不了的仓库（它们没有缓存，总排在最前面）把整个刷新每次都卡在它们那里。
///   触发时同一批里已经抓回来的照常处理完，只是不再开下一批。
pub fn fetch_in_batches(
    targets: Vec<(String, String)>,
    batch_size: usize,
    limits: FetchLimits,
    io: &dyn ReadmeIo,
) -> FetchReport {
    let total = targets.len();
    let started = io.now();
    let mut report = FetchReport::default();
    let mut failures_in_a_row = 0usize;
    let mut pending = targets.into_iter();
    while pending.len() > 0 {
        if io.now().saturating_duration_since(started) >= limits.time_budget {
            report.stopped = Some(StopReason::TimeBudget);
            break;
        }
        let batch: Vec<(String, String)> = pending.by_ref().take(batch_size.max(1)).collect();
        let names: Vec<String> = batch.iter().map(|(_, name)| name.clone()).collect();
        io.progress(report.attempted.len(), total, &names);
        // 线程异常（panic）当成这一个失败，其余照常
        let fetched: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = batch
                .iter()
                .map(|(id, name)| scope.spawn(move || io.fetch(id, name)))
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().ok())
                .collect()
        });
        for ((id, _), outcome) in batch.into_iter().zip(fetched) {
            report.attempted.push(id);
            let verdict = match outcome {
                None => Verdict::Broken,
                Some(Err(error)) if is_repo_problem(&error) => Verdict::RepoProblem,
                Some(Err(_)) => Verdict::Broken,
                Some(Ok(None)) => {
                    report.missed += 1;
                    Verdict::Worked
                }
                Some(Ok(Some(readme))) => match io.store(&readme) {
                    Ok(()) => {
                        report.refreshed += 1;
                        Verdict::Worked
                    }
                    Err(_) => Verdict::Broken,
                },
            };
            match verdict {
                Verdict::Worked => failures_in_a_row = 0,
                Verdict::RepoProblem => {
                    report.failed += 1;
                    failures_in_a_row = 0;
                }
                Verdict::Broken => {
                    report.failed += 1;
                    failures_in_a_row += 1;
                    if failures_in_a_row >= limits.stop_after_failures {
                        report.stopped = Some(StopReason::Failures);
                    }
                }
            }
        }
        if report.stopped.is_some() {
            break;
        }
    }
    report
}

/// 抓一个仓库的结果，对「连续失败」计数意味着什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// 抓到了并落了库，或 GitHub 明确说没有 README：网是通的。计数清零。
    Worked,
    /// GitHub 回了，但这一个仓库的 README 本身不能用。记一次失败，但网是通的：计数清零。
    RepoProblem,
    /// 断网、超时、限流、服务端错、线程异常、落库失败：计数加一。
    Broken,
}

/// `github::fetch_readme` 里「GitHub 回了、但内容不能用」的几种错误都以这个开头（响应解析失败、不支持的编码、解码失败、不是 UTF-8）。
/// 网络和 HTTP 错误是别的开头（「GitHub API …」「Token …」），认不出来的一律当作「网有问题」，宁可早停。
fn is_repo_problem(error: &str) -> bool {
    error.starts_with("GitHub README ")
}

/// 只看缓存里的 README，不联网。每个仓库一个小的 `IMMEDIATE` 事务，文字处理都在事务外。
pub fn enqueue_changed(db_path: &Path) -> Result<FollowReport, String> {
    enqueue_changed_at(db_path, &crate::jobs::now_db_time(), &mut |_, _| {})
}

/// 评估一个仓库时依次经过的几处。线上不观察（传空闭包）；测试在这里插一脚：
/// 看此刻有没有占着写锁，或者模拟「读完之后库里的东西被别人改了」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FollowStage {
    /// 刚读出这个仓库、摘录和比较还没开始。此时不能占着写锁。
    Evaluating,
    /// 摘录和比较都算完了，写事务还没开。
    BeforeWrite,
    /// 写事务（`IMMEDIATE`）已经开了，还没复核。
    InTransaction,
}

/// `now`：库里用的时间格式（`jobs::now_db_time`），「最近 24 小时」从它往前数。
pub(crate) fn enqueue_changed_at(
    db_path: &Path,
    now: &str,
    observe: &mut dyn FnMut(FollowStage, &str),
) -> Result<FollowReport, String> {
    enqueue_changed_with_cap(db_path, now, README_EXCERPT_INPUT_BYTES, observe)
}

/// 同 `enqueue_changed_at`，另外可以指定读 README 的字节上限（测试用小的上限造夹具；线上永远是 `README_EXCERPT_INPUT_BYTES`）。
pub(crate) fn enqueue_changed_with_cap(
    db_path: &Path,
    now: &str,
    head_cap: usize,
    observe: &mut dyn FnMut(FollowStage, &str),
) -> Result<FollowReport, String> {
    let mut conn = crate::jobs::open_db(db_path)?;
    let hold_back_since = crate::jobs::shift_db_time(now, -crate::jobs::DAY_SECS)?;
    let ids = conn
        .prepare(&candidate_sql())
        .map_err(|error| format!("README 跟进读取失败：{error}"))?
        .query_map([], |row| row.get(0))
        .map_err(|error| format!("README 跟进读取失败：{error}"))?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|error| format!("README 跟进读取失败：{error}"))?;
    let mut report = FollowReport::default();
    for repo_id in ids {
        // ① 事务外：读出来（README 超过安全上限的部分不要）
        let Some(candidate) = read_candidate(&conn, &repo_id, head_cap)? else {
            continue;
        };
        // ② 事务外：摘录、清洗、比较，慢也不碍别人
        observe(FollowStage::Evaluating, &repo_id);
        let evaluation = evaluate(&candidate);
        observe(FollowStage::BeforeWrite, &repo_id);
        // ③ 事务里：只复核几个短字段，再照着算好的写
        write_evaluation(
            &mut conn,
            &repo_id,
            &candidate,
            &evaluation,
            &hold_back_since,
            &mut report,
            observe,
        )?;
    }
    Ok(report)
}

fn needs_refresh(repo: &RepoReadmeState) -> bool {
    let Some(fetched_at) = repo.readme_fetched_at.as_deref() else {
        return true;
    };
    let Some(pushed_at) = repo.pushed_at.as_deref() else {
        return false;
    };
    stamp19(pushed_at) > stamp19(fetched_at)
}

/// 只取 `YYYY-MM-DDTHH:MM:SS` 这 19 个字符，按字符边界切，避免把多字节字符切坏。
fn stamp19(value: &str) -> String {
    value.chars().take(19).collect()
}

fn candidate_sql() -> String {
    format!(
        "SELECT repositories.id
         FROM repositories
         JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
         JOIN repo_readmes ON repo_readmes.repo_id = repositories.id
         WHERE repositories.sync_status = 'active'
           AND IFNULL(trim(repo_ai_documents.card_json), '') <> ''
           AND {}
         ORDER BY repositories.id",
        crate::card_handler::NOT_CONFIRMED_SQL
    )
}

/// 一个仓库的评估条件（和 `candidate_sql` 一致）：`columns` 是要读的列。
fn detail_sql(columns: &str) -> String {
    format!(
        "SELECT {columns}
         FROM repositories
         JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
         JOIN repo_readmes ON repo_readmes.repo_id = repositories.id
         WHERE repositories.id = ?1
           AND repositories.sync_status = 'active'
           AND IFNULL(trim(repo_ai_documents.card_json), '') <> ''
           AND {}",
        crate::card_handler::NOT_CONFIRMED_SQL
    )
}

/// 事务外读出来的一个仓库。`head` 是 README 的前 `head_cap` 字节（线上是 `README_EXCERPT_INPUT_BYTES`，比任何真实 README 都大）。
struct Candidate {
    account_id: String,
    readme_hash: Option<String>,
    readme_basis: Option<String>,
    content_hash: String,
    head: String,
}

fn read_candidate(
    conn: &rusqlite::Connection,
    repo_id: &str,
    head_cap: usize,
) -> Result<Option<Candidate>, String> {
    let sql = detail_sql(
        "repositories.account_id,
                repo_ai_documents.readme_hash,
                repo_ai_documents.readme_basis,
                repo_readmes.content_hash,
                repo_readmes.raw_markdown",
    );
    conn.query_row(&sql, params![repo_id], |row| {
        let mut head: String = row.get(4)?;
        head.truncate(char_boundary_at_most(&head, head_cap));
        Ok(Candidate {
            account_id: row.get(0)?,
            readme_hash: row.get(1)?,
            readme_basis: row.get(2)?,
            content_hash: row.get(3)?,
            head,
        })
    })
    .optional()
    .map_err(|error| format!("README 跟进读取失败：{error}"))
}

/// 不超过 `max_bytes` 的最大字符边界（`text[..返回值]` 一定不会切坏多字节字符）。
fn char_boundary_at_most(text: &str, max_bytes: usize) -> usize {
    let mut end = max_bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

/// 在事务外算好的决定：事务里只照着写，不再碰任何和文字长度成比例的计算。
struct Evaluation {
    /// 现在的摘录；README 没了或几乎没内容就是空。
    current: Option<String>,
    plan: Plan,
}

enum Plan {
    /// 还没记过 README 哈希（卡是在有 README 之前或不记哈希的版本里生成的）：明显改动才排队。
    NoRecordedHash { material: bool },
    /// 哈希没变：只在还没有基准时补上基准。
    HashUnchanged,
    /// 哈希变了：没有基准就把现在的当基准（不排队）；有基准就比较，明显改动才排队，否则只更新哈希。
    HashChanged { has_basis: bool, material: bool },
}

fn evaluate(candidate: &Candidate) -> Evaluation {
    let excerpt = build_readme_excerpt(&candidate.head);
    let current = if excerpt.is_absent() || excerpt.text.is_empty() {
        None
    } else {
        Some(excerpt.text)
    };
    let plan = match candidate.readme_hash.as_deref() {
        None => Plan::NoRecordedHash {
            material: is_material_change(None, current.as_deref()),
        },
        Some(hash) if hash == candidate.content_hash => Plan::HashUnchanged,
        Some(_) => match candidate.readme_basis.as_deref() {
            None => Plan::HashChanged {
                has_basis: false,
                material: false,
            },
            Some(basis) => Plan::HashChanged {
                has_basis: true,
                material: is_material_change(Some(basis), current.as_deref()),
            },
        },
    };
    Evaluation { current, plan }
}

/// 开 `IMMEDIATE` 事务，复核这个仓库「读出来之后」没变（还是同一份 README、同一个记过的哈希、基准有没有没变），
/// 再照着 `evaluation` 写。变了就整个跳过（不写、不计数），下次同步再说。
fn write_evaluation(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    candidate: &Candidate,
    evaluation: &Evaluation,
    hold_back_since: &str,
    report: &mut FollowReport,
    observe: &mut dyn FnMut(FollowStage, &str),
) -> Result<(), String> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("README 跟进入队失败：{error}"))?;
    observe(FollowStage::InTransaction, repo_id);
    let fresh = tx
        .query_row(
            &detail_sql(
                "repositories.account_id,
                repo_ai_documents.readme_hash,
                repo_ai_documents.readme_basis IS NULL,
                repo_readmes.content_hash",
            ),
            params![repo_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("README 跟进读取失败：{error}"))?;
    let unchanged = fresh
        .as_ref()
        .is_some_and(|(account, hash, no_basis, content)| {
            *account == candidate.account_id
                && *hash == candidate.readme_hash
                && *no_basis == candidate.readme_basis.is_none()
                && *content == candidate.content_hash
        });
    if !unchanged {
        tx.commit()
            .map_err(|error| format!("README 跟进入队失败：{error}"))?;
        return Ok(());
    }
    let account_id = candidate.account_id.as_str();
    let content_hash = candidate.content_hash.as_str();
    let current = evaluation.current.as_deref();
    report.checked += 1;
    match &evaluation.plan {
        Plan::NoRecordedHash { material } => {
            if *material {
                report.material += 1;
                count_job(
                    report,
                    insert_job(&tx, account_id, repo_id, hold_back_since)?,
                );
            }
        }
        Plan::HashUnchanged => {
            if candidate.readme_basis.is_none() {
                tx.execute(
                    "UPDATE repo_ai_documents SET readme_basis = ?1 WHERE repo_id = ?2",
                    params![current, repo_id],
                )
                .map_err(|error| format!("README 基准补写失败：{error}"))?;
            }
        }
        Plan::HashChanged {
            has_basis: false, ..
        } => {
            report.changed += 1;
            tx.execute(
                "UPDATE repo_ai_documents SET readme_hash = ?1, readme_basis = ?2 WHERE repo_id = ?3",
                params![content_hash, current, repo_id],
            )
            .map_err(|error| format!("README 基准补写失败：{error}"))?;
        }
        Plan::HashChanged {
            has_basis: true,
            material: true,
        } => {
            report.changed += 1;
            report.material += 1;
            count_job(
                report,
                insert_job(&tx, account_id, repo_id, hold_back_since)?,
            );
        }
        Plan::HashChanged {
            has_basis: true,
            material: false,
        } => {
            report.changed += 1;
            tx.execute(
                "UPDATE repo_ai_documents SET readme_hash = ?1 WHERE repo_id = ?2",
                params![content_hash, repo_id],
            )
            .map_err(|error| format!("README 哈希更新失败：{error}"))?;
        }
    }
    tx.commit()
        .map_err(|error| format!("README 跟进入队失败：{error}"))?;
    Ok(())
}

/// 想给一个仓库排队时的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobOutcome {
    /// 新排上了。
    Queued,
    /// 已经有一条排着或正在跑的，不重复排。
    AlreadyActive,
    /// 最近 24 小时内失败或被取消过，这次先不排。
    HeldBack,
}

fn count_job(report: &mut FollowReport, outcome: JobOutcome) {
    match outcome {
        JobOutcome::Queued => report.queued += 1,
        JobOutcome::HeldBack => report.held_back += 1,
        JobOutcome::AlreadyActive => {}
    }
}

/// 给这个仓库排一条理解卡任务（在写事务里调用）。
/// 已经有 `queued` / `running` 的不重复排（也不算被拦下）；最近 24 小时内（含正好 24 小时前那一刻，
/// 和 `jobs::claim_next` 的窗口一样）有过 `failed` / `cancelled` 的同一个仓库的理解卡任务，就不再排：
/// 重做总失败、或被取消过的，每次同步都再排一遍只会白花额度。24 小时之后照排。
///
/// 「什么时候失败 / 被取消的」：
/// - 失败的看结束时间 `finished_at`（和队列里别处的「最近 24 小时」一个口径，见 `jobs::count_failed_cards_since`），没有就退回 `updated_at`；
/// - 被取消的看 `updated_at`：`v2_ops::cancel_job` 取消时只把 `updated_at` 改成现在，`finished_at` 沿用原来的记账时间
///   （一条在队列里排了几天的任务，取消时它已经是几天前）——看 `finished_at` 的话，用户刚取消的任务下次同步又排上了。
fn insert_job(
    tx: &Transaction<'_>,
    account_id: &str,
    repo_id: &str,
    hold_back_since: &str,
) -> Result<JobOutcome, String> {
    let active: bool = tx
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM ai_jobs
                 WHERE job_type = 'repo_card' AND target_type = 'repository' AND target_id = ?1
                   AND status IN ('queued', 'running'))",
            params![repo_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("README 跟进入队失败：{error}"))?;
    if active {
        return Ok(JobOutcome::AlreadyActive);
    }
    let recently_failed: bool = tx
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM ai_jobs
                 WHERE job_type = 'repo_card' AND target_type = 'repository' AND target_id = ?1
                   AND status IN ('failed', 'cancelled')
                   AND CASE status
                         WHEN 'cancelled' THEN updated_at
                         ELSE COALESCE(finished_at, updated_at)
                       END >= ?2)",
            params![repo_id, hold_back_since],
            |row| row.get(0),
        )
        .map_err(|error| format!("README 跟进入队失败：{error}"))?;
    if recently_failed {
        return Ok(JobOutcome::HeldBack);
    }
    let inserted = tx
        .execute(
            "INSERT OR IGNORE INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             VALUES (lower(hex(randomblob(8))), ?1, 'repo_card', 'repository', ?2, ?3, 'queued')",
            params![account_id, repo_id, README_CHANGED_PRIORITY],
        )
        .map_err(|error| format!("README 跟进入队失败：{error}"))?;
    Ok(if inserted > 0 {
        JobOutcome::Queued
    } else {
        JobOutcome::AlreadyActive
    })
}

fn shingles(text: &str) -> HashSet<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut set = HashSet::new();
    if chars.len() < 4 {
        set.insert(chars.into_iter().collect());
        return set;
    }
    for index in 0..=chars.len() - 4 {
        set.insert(chars[index..index + 4].iter().collect());
    }
    set
}

fn push_letters(ch: char, out: &mut String, kept: &mut usize) {
    for lower in ch.to_lowercase() {
        if *kept >= MAX_COMPARE_CHARS {
            return;
        }
        out.push(lower);
        *kept += 1;
    }
}

fn skip_image(chars: &[char], start: usize) -> Option<usize> {
    let mut index = start + 2;
    while index < chars.len() {
        if chars[index] == ']' && chars.get(index + 1) == Some(&'(') {
            index += 2;
            while index < chars.len() {
                if chars[index] == ')' {
                    return Some(index + 1);
                }
                index += 1;
            }
            return None;
        }
        index += 1;
    }
    None
}

fn take_link(chars: &[char], start: usize, out: &mut String, kept: &mut usize) -> Option<usize> {
    let mut index = start + 1;
    let mut buf: Vec<char> = Vec::new();
    while index < chars.len() {
        if chars[index] == ']' && chars.get(index + 1) == Some(&'(') {
            let mut end = index + 2;
            while end < chars.len() {
                if chars[end] == ')' {
                    for ch in &buf {
                        if ch.is_alphabetic() {
                            push_letters(*ch, out, kept);
                        }
                    }
                    return Some(end + 1);
                }
                end += 1;
            }
            return None;
        }
        if buf.len() >= LINK_TEXT_CAP {
            return None;
        }
        buf.push(chars[index]);
        index += 1;
    }
    None
}

fn skip_html(chars: &[char], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < chars.len() {
        if chars[index] == '>' {
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

fn is_url_at(chars: &[char], index: usize) -> bool {
    starts_with_ascii_ci(chars, index, &['h', 't', 't', 'p', 's', ':', '/', '/'])
        || starts_with_ascii_ci(chars, index, &['h', 't', 't', 'p', ':', '/', '/'])
}

fn starts_with_ascii_ci(chars: &[char], index: usize, needle: &[char]) -> bool {
    if index + needle.len() > chars.len() {
        return false;
    }
    needle
        .iter()
        .enumerate()
        .all(|(offset, expected)| chars[index + offset].to_ascii_lowercase() == *expected)
}

fn skip_until_whitespace(chars: &[char], index: usize) -> usize {
    let mut next = index;
    while next < chars.len() && !chars[next].is_whitespace() {
        next += 1;
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::migrate_database;
    use rusqlite::{params, Connection};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn prose(len: usize, start: u32) -> String {
        (0..len)
            .map(|index| {
                char::from_u32(0x4E00 + start + index as u32).expect("测试用汉字应在范围内")
            })
            .collect()
    }

    #[test]
    fn normalize_drops_badge_link_url_html_digits_case_and_keeps_han() {
        let raw = "Hello, WORLD! ![badge](https://img.shields.io/badge/v1-blue) See [文档](https://example.com/docs) and https://example.com/raw <b>粗体</b> v1.2.3 汉字保留";
        assert_eq!(
            normalize_for_compare(raw),
            "helloworldsee文档and粗体v汉字保留"
        );
        assert_eq!(normalize_for_compare("甲1乙， 丙"), "甲乙丙");
        assert_eq!(normalize_for_compare("AbC"), "abc");
    }

    #[test]
    fn normalize_truncates_output_at_20000_chars() {
        let raw: String = std::iter::repeat('A').take(20_001).collect();
        let out = normalize_for_compare(&raw);
        assert_eq!(out.chars().count(), 20_000);
        assert!(out.chars().all(|ch| ch == 'a'));
        let han = prose(20_001, 0);
        assert_eq!(normalize_for_compare(&han).chars().count(), 20_000);
    }

    #[test]
    fn normalize_unclosed_html_stops_and_keeps_prefix() {
        assert_eq!(normalize_for_compare("前面<没结束"), "前面");
        assert_eq!(normalize_for_compare("<b>粗体</b>"), "粗体");
    }

    #[test]
    fn normalize_unclosed_link_stops_and_keeps_prefix() {
        assert_eq!(normalize_for_compare("前面[没结束"), "前面");
        assert_eq!(
            normalize_for_compare("见[说明v2](https://example.com/a)。"),
            "见说明v"
        );
    }

    #[test]
    fn normalize_unclosed_image_stops() {
        assert_eq!(normalize_for_compare("前面![没结束"), "前面");
        assert_eq!(
            normalize_for_compare("留![x](https://img.example/a.png)下"),
            "留下"
        );
    }

    #[test]
    fn normalize_url_is_case_insensitive() {
        assert_eq!(normalize_for_compare("看 HTTP://EXAMPLE.com/A 完"), "看完");
        assert_eq!(normalize_for_compare("看 Https://foo.bar/x 完"), "看完");
    }

    #[test]
    fn normalize_malicious_unclosed_markup_stays_fast() {
        let mut tags = String::from("开头");
        tags.push('<');
        tags.extend(std::iter::repeat('a').take(100_000));
        let started = Instant::now();
        let out = normalize_for_compare(&tags);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "未闭合标签扫太慢：{:?}",
            started.elapsed()
        );
        assert_eq!(out, "开头");

        let mut brackets = String::from("前");
        brackets.push('[');
        brackets.extend(std::iter::repeat('字').take(100_000));
        let started = Instant::now();
        let out = normalize_for_compare(&brackets);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "未闭合链接扫太慢：{:?}",
            started.elapsed()
        );
        assert_eq!(out, "前");

        let mut packed = String::new();
        for index in 0..10_000 {
            packed.push(char::from_u32(0x4E00 + (index % 80)).expect("汉字"));
        }
        packed.push_str("https://example.com/x");
        let started = Instant::now();
        let out = normalize_for_compare(&packed);
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(!out.contains("http"));
        assert!(out.chars().count() >= 80);
    }

    #[test]
    fn similarity_identical_is_exactly_one() {
        let text = prose(40, 0);
        assert_eq!(similarity(&text, &text), 1.0);
        assert_eq!(similarity("abcd", "abcd"), 1.0);
    }

    #[test]
    fn similarity_disjoint_is_zero() {
        assert_eq!(similarity(&prose(12, 0), &prose(12, 100)), 0.0);
        assert_eq!(similarity("abcd", "efgh"), 0.0);
    }

    #[test]
    fn similarity_both_empty_is_one() {
        assert_eq!(similarity("", ""), 1.0);
    }

    #[test]
    fn similarity_one_side_empty_is_zero() {
        assert_eq!(similarity("", "abcd"), 0.0);
        assert_eq!(similarity("abcd", ""), 0.0);
    }

    #[test]
    fn similarity_is_symmetric() {
        let left = prose(30, 0);
        let mut right = prose(20, 0);
        right.push_str(&prose(10, 80));
        let forward = similarity(&left, &right);
        let backward = similarity(&right, &left);
        assert_eq!(forward, backward);
        assert!(forward > 0.0 && forward < 1.0, "{forward}");
    }

    #[test]
    fn similarity_shorter_than_four_is_one_shingle() {
        assert_eq!(similarity("ab", "ab"), 1.0);
        assert_eq!(similarity("ab", "cd"), 0.0);
        assert_eq!(similarity("你", "你"), 1.0);
        assert_eq!(similarity("你", "好"), 0.0);
    }

    #[test]
    fn material_version_badge_and_typo_are_not_material() {
        let body = prose(180, 0);
        let left = format!("![b](https://img.shields.io/badge/v1.2.3-blue)\n{body}\nv1.2.3");
        let right = format!("![b](https://img.shields.io/badge/v9.0.0-red)\n{body}\nv9.9.9");
        assert!(!is_material_change(Some(&left), Some(&right)));

        let mut chars: Vec<char> = prose(200, 0).chars().collect();
        let original: String = chars.iter().collect();
        chars[100] = '\u{9FA5}';
        let edited: String = chars.iter().collect();
        assert!(!is_material_change(Some(&original), Some(&edited)));
    }

    #[test]
    fn material_append_about_thirty_percent_is_material() {
        let base = prose(400, 0);
        let mut grown = base.clone();
        grown.push_str(&prose(120, 400));
        assert!(is_material_change(Some(&base), Some(&grown)));
    }

    #[test]
    fn material_append_about_ten_percent_is_not() {
        let base = prose(400, 0);
        let mut grown = base.clone();
        grown.push_str(&prose(40, 400));
        assert!(!is_material_change(Some(&base), Some(&grown)));
    }

    #[test]
    fn material_rewriting_half_is_material() {
        let base = prose(400, 0);
        let mut rewritten = prose(200, 0);
        rewritten.push_str(&prose(200, 2_000));
        assert!(is_material_change(Some(&base), Some(&rewritten)));
    }

    #[test]
    fn material_missing_basis_with_content_is_material() {
        let current = prose(80, 0);
        assert!(is_material_change(None, Some(&current)));
        assert!(is_material_change(Some(""), Some(&current)));
        assert!(is_material_change(Some("123 !!!"), Some(&current)));
    }

    #[test]
    fn material_empty_current_is_not() {
        let basis = prose(80, 0);
        assert!(!is_material_change(Some(&basis), None));
        assert!(!is_material_change(Some(&basis), Some("")));
        assert!(!is_material_change(Some(&basis), Some("1.2.3 !!!")));
        assert!(!is_material_change(None, None));
        assert!(!is_material_change(None, Some("")));
    }

    #[test]
    fn material_one_han_sentence_in_a_long_text_is_not() {
        let mut chars: Vec<char> = prose(400, 0).chars().collect();
        let original: String = chars.iter().collect();
        for index in 120..128 {
            chars[index] = char::from_u32(0x9000 + (index as u32)).expect("替换字");
        }
        let edited: String = chars.iter().collect();
        assert!(!is_material_change(Some(&original), Some(&edited)));
    }

    fn state(
        id: &str,
        pushed_at: Option<&str>,
        readme_fetched_at: Option<&str>,
    ) -> RepoReadmeState {
        RepoReadmeState {
            id: id.to_owned(),
            full_name: format!("fox/{id}"),
            pushed_at: pushed_at.map(str::to_owned),
            readme_fetched_at: readme_fetched_at.map(str::to_owned),
        }
    }

    #[test]
    fn refresh_puts_missing_cache_first_and_skips_unchanged_pushes() {
        let repos = vec![
            state(
                "old",
                Some("2026-01-01T00:00:00Z"),
                Some("2020-01-01T00:00:00Z"),
            ),
            state(
                "fresh",
                Some("2026-01-01T00:00:00Z"),
                Some("2026-06-01T00:00:00Z"),
            ),
            state(
                "equal",
                Some("2026-06-01T00:00:00.123Z"),
                Some("2026-06-01T00:00:00Z"),
            ),
            state("none-push", None, Some("2020-01-01T00:00:00Z")),
            state("no-cache", None, None),
            state("no-cache-b", Some("2026-01-01T00:00:00Z"), None),
            state(
                "newer",
                Some("2026-08-01T00:00:01Z"),
                Some("2026-08-01T00:00:00.999Z"),
            ),
            state(
                "same-second",
                Some("2026-08-01T00:00:00.999Z"),
                Some("2026-08-01T00:00:00.000Z"),
            ),
        ];
        assert_eq!(
            refresh_targets(&repos),
            vec![
                "no-cache".to_owned(),
                "no-cache-b".to_owned(),
                "old".to_owned(),
                "newer".to_owned(),
            ]
        );
    }

    #[test]
    fn refresh_caps_at_150_and_keeps_the_oldest() {
        let mut missing = Vec::new();
        for index in 0..151 {
            missing.push(state(&format!("m{index:03}"), None, None));
        }
        let ids = refresh_targets(&missing);
        assert_eq!(ids.len(), 150);
        assert_eq!(ids[0], "m000");
        assert_eq!(ids[149], "m149");
        assert!(!ids.iter().any(|id| id == "m150"));

        let mut cached = Vec::new();
        for index in 0..160 {
            let fetched = format!("2020-01-01T00:{:02}:{:02}Z", index / 60, index % 60);
            cached.push(state(
                &format!("c{index:03}"),
                Some("2026-10-01T00:00:00Z"),
                Some(&fetched),
            ));
        }
        let ids = refresh_targets(&cached);
        assert_eq!(ids.len(), 150);
        assert_eq!(ids[0], "c000");
        assert_eq!(ids[149], "c149");
        assert!(!ids.iter().any(|id| id == "c159"));
    }

    struct TempDb {
        path: PathBuf,
    }

    impl TempDb {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应晚于纪元")
                .as_nanos();
            // 同一纳秒里两个线程各建一个库也不会撞名
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let counter = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("fsl-follow-{nanos}-{counter}.sqlite3"));
            migrate_database(&path).expect("测试库应能完成迁移");
            let conn = Connection::open(&path).expect("应能打开测试库");
            conn.execute_batch("PRAGMA foreign_keys = ON;")
                .expect("应能打开外键");
            conn.execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .expect("测试账号应能写入");
            drop(conn);
            Self { path }
        }

        fn open(&self) -> Connection {
            let conn = Connection::open(&self.path).expect("应能打开测试库");
            conn.execute_batch("PRAGMA foreign_keys = ON;")
                .expect("应能打开外键");
            conn
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            for suffix in ["-wal", "-shm"] {
                let mut name = self.path.as_os_str().to_os_string();
                name.push(suffix);
                let _ = fs::remove_file(name);
            }
        }
    }

    fn insert_repo(conn: &Connection, id: &str, status: &str) {
        conn.execute(
            "INSERT INTO repositories(
                id, account_id, owner, name, full_name, html_url, starred_at, sync_status, topics_json
             ) VALUES (
                ?1, 'acct', 'fox', ?1, 'fox/' || ?1, 'https://github.com/fox/' || ?1,
                '2026-09-01T00:00:00Z', ?2, '[]'
             )",
            params![id, status],
        )
        .expect("仓库应能写入");
    }

    fn insert_readme(conn: &Connection, id: &str, raw: &str, hash: &str) {
        conn.execute(
            "INSERT INTO repo_readmes(repo_id, raw_markdown, content_hash, source_path, fetched_at)
             VALUES (?1, ?2, ?3, 'README.md', '2026-09-01T00:00:00Z')",
            params![id, raw, hash],
        )
        .expect("README 应能写入");
    }

    fn insert_card(
        conn: &Connection,
        id: &str,
        card_json: &str,
        readme_hash: Option<&str>,
        readme_basis: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO repo_ai_documents(
                repo_id, summary_zh, model, prompt_version, source_hash, generated_at, card_json,
                readme_hash, readme_basis
             ) VALUES (?1, '摘要', 'm', 'card-v2', 'src', '2026-06-01T00:00:00Z', ?2, ?3, ?4)",
            params![id, card_json, readme_hash, readme_basis],
        )
        .expect("理解卡应能写入");
    }

    fn optional_text(conn: &Connection, sql: &str) -> Option<String> {
        conn.query_row(sql, [], |row| row.get(0))
            .expect("查询应返回")
    }

    fn job_count(conn: &Connection, repo_id: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM ai_jobs WHERE target_id = ?1 AND job_type = 'repo_card'",
            params![repo_id],
            |row| row.get(0),
        )
        .expect("任务数应能读出")
    }

    fn job_priority(conn: &Connection, repo_id: &str) -> i64 {
        conn.query_row(
            "SELECT priority FROM ai_jobs WHERE target_id = ?1",
            params![repo_id],
            |row| row.get(0),
        )
        .expect("优先级应能读出")
    }

    #[test]
    fn enqueue_changed_reports_the_combined_fixture_and_second_run_queues_zero() {
        let db = TempDb::new();
        let same = prose(120, 0);
        let typo_old = prose(200, 200);
        let mut typo_chars: Vec<char> = typo_old.chars().collect();
        typo_chars[80] = '\u{9FA5}';
        let typo_new: String = typo_chars.iter().collect();
        let rewrite_basis = prose(160, 500);
        let rewrite_now = prose(160, 2_000);
        let fill = prose(100, 800);
        let adopt = prose(100, 1_000);
        let fresh = prose(100, 1_200);
        let confirmed_old = prose(100, 1_400);
        let confirmed_now = prose(100, 1_600);
        let removed_old = prose(100, 1_800);
        let removed_now = prose(100, 2_200);

        let conn = db.open();
        insert_repo(&conn, "same", "active");
        insert_readme(&conn, "same", &same, "hash-same");
        insert_card(
            &conn,
            "same",
            r#"{"review_status":"suggested"}"#,
            Some("hash-same"),
            Some(&same),
        );

        insert_repo(&conn, "typo", "active");
        insert_readme(&conn, "typo", &typo_new, "hash-typo-new");
        insert_card(
            &conn,
            "typo",
            r#"{"review_status":"suggested"}"#,
            Some("hash-typo-old"),
            Some(&typo_old),
        );

        insert_repo(&conn, "rewrite", "active");
        insert_readme(&conn, "rewrite", &rewrite_now, "hash-rewrite-new");
        insert_card(
            &conn,
            "rewrite",
            r#"{"review_status":"suggested"}"#,
            Some("hash-rewrite-old"),
            Some(&rewrite_basis),
        );

        insert_repo(&conn, "confirmed", "active");
        insert_readme(&conn, "confirmed", &confirmed_now, "hash-confirmed-new");
        insert_card(
            &conn,
            "confirmed",
            r#"{"review_status":"confirmed"}"#,
            Some("hash-confirmed-old"),
            Some(&confirmed_old),
        );

        insert_repo(&conn, "nocard", "active");
        insert_readme(&conn, "nocard", &fresh, "hash-nocard");

        insert_repo(&conn, "removed", "removed");
        insert_readme(&conn, "removed", &removed_now, "hash-removed-new");
        insert_card(
            &conn,
            "removed",
            r#"{"review_status":"suggested"}"#,
            Some("hash-removed-old"),
            Some(&removed_old),
        );

        insert_repo(&conn, "fill", "active");
        insert_readme(&conn, "fill", &fill, "hash-fill");
        insert_card(
            &conn,
            "fill",
            r#"{"review_status":"suggested"}"#,
            Some("hash-fill"),
            None,
        );

        insert_repo(&conn, "adopt", "active");
        insert_readme(&conn, "adopt", &adopt, "hash-adopt-new");
        insert_card(
            &conn,
            "adopt",
            r#"{"review_status":"suggested"}"#,
            Some("hash-adopt-old"),
            None,
        );

        insert_repo(&conn, "fresh", "active");
        insert_readme(&conn, "fresh", &fresh, "hash-fresh");
        insert_card(
            &conn,
            "fresh",
            r#"{"review_status":"suggested"}"#,
            None,
            None,
        );
        drop(conn);

        let first = enqueue_changed(&db.path).expect("第一次评估应成功");
        assert_eq!(
            (first.checked, first.changed, first.material, first.queued),
            (6, 3, 2, 2)
        );
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'same'"
            )
            .as_deref(),
            Some("hash-same")
        );
        assert_eq!(job_count(&conn, "same"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'typo'"
            )
            .as_deref(),
            Some("hash-typo-new")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'typo'"
            )
            .as_deref(),
            Some(typo_old.as_str())
        );
        assert_eq!(job_count(&conn, "typo"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'rewrite'"
            )
            .as_deref(),
            Some("hash-rewrite-old")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'rewrite'"
            )
            .as_deref(),
            Some(rewrite_basis.as_str())
        );
        assert_eq!(job_count(&conn, "rewrite"), 1);
        assert_eq!(job_priority(&conn, "rewrite"), 5);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'confirmed'"
            )
            .as_deref(),
            Some("hash-confirmed-old")
        );
        assert_eq!(job_count(&conn, "confirmed"), 0);
        assert_eq!(job_count(&conn, "nocard"), 0);
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'removed'"
            )
            .as_deref(),
            Some("hash-removed-old")
        );
        assert_eq!(job_count(&conn, "removed"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'fill'"
            )
            .as_deref(),
            Some(fill.as_str())
        );
        assert_eq!(job_count(&conn, "fill"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'adopt'"
            )
            .as_deref(),
            Some("hash-adopt-new")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'adopt'"
            )
            .as_deref(),
            Some(adopt.as_str())
        );
        assert_eq!(job_count(&conn, "adopt"), 0);

        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'fresh'"
            ),
            None
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'fresh'"
            ),
            None
        );
        assert_eq!(job_count(&conn, "fresh"), 1);
        assert_eq!(job_priority(&conn, "fresh"), 5);
        drop(conn);

        let second = enqueue_changed(&db.path).expect("第二次评估应成功");
        assert_eq!(
            (
                second.checked,
                second.changed,
                second.material,
                second.queued
            ),
            (6, 1, 2, 0)
        );
        let conn = db.open();
        assert_eq!(job_count(&conn, "rewrite") + job_count(&conn, "fresh"), 2);
    }

    #[test]
    fn enqueue_changed_small_edit_updates_hash_only() {
        let db = TempDb::new();
        let old = prose(200, 0);
        let mut chars: Vec<char> = old.chars().collect();
        chars[40] = '\u{9FA5}';
        let now: String = chars.iter().collect();
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &now, "hash-new");
        insert_card(
            &conn,
            "r",
            r#"{"review_status":"suggested"}"#,
            Some("hash-old"),
            Some(&old),
        );
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued
            ),
            (1, 1, 0, 0)
        );
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some("hash-new")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some(old.as_str())
        );
    }

    #[test]
    fn enqueue_changed_null_basis_with_new_hash_adopts_current() {
        let db = TempDb::new();
        let now = prose(90, 0);
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &now, "hash-new");
        insert_card(
            &conn,
            "r",
            r#"{"review_status":"suggested"}"#,
            Some("hash-old"),
            None,
        );
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!((report.changed, report.material, report.queued), (1, 0, 0));
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some("hash-new")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some(now.as_str())
        );
    }

    #[test]
    fn enqueue_changed_null_hash_queues_and_leaves_columns() {
        let db = TempDb::new();
        let now = prose(90, 0);
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &now, "hash-now");
        insert_card(&conn, "r", r#"{"review_status":"suggested"}"#, None, None);
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued
            ),
            (1, 0, 1, 1)
        );
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            ),
            None
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            ),
            None
        );
        assert_eq!(job_priority(&conn, "r"), README_CHANGED_PRIORITY);
    }

    #[test]
    fn enqueue_changed_fills_null_basis_when_hash_matches() {
        let db = TempDb::new();
        let now = prose(90, 10);
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &now, "hash-now");
        insert_card(
            &conn,
            "r",
            r#"{"review_status":"suggested"}"#,
            Some("hash-now"),
            None,
        );
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued
            ),
            (1, 0, 0, 0)
        );
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some(now.as_str())
        );
        assert_eq!(job_count(&conn, "r"), 0);
    }

    #[test]
    fn enqueue_changed_does_not_duplicate_an_active_job() {
        let db = TempDb::new();
        let now = prose(90, 0);
        let conn = db.open();
        insert_repo(&conn, "queued", "active");
        insert_readme(&conn, "queued", &now, "hash-now");
        insert_card(
            &conn,
            "queued",
            r#"{"review_status":"suggested"}"#,
            None,
            None,
        );
        insert_repo(&conn, "running", "active");
        insert_readme(&conn, "running", &now, "hash-run");
        insert_card(
            &conn,
            "running",
            r#"{"review_status":"suggested"}"#,
            None,
            None,
        );
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status)
             VALUES ('job-q', 'acct', 'repo_card', 'repository', 'queued', 3, 'queued'),
                    ('job-r', 'acct', 'repo_card', 'repository', 'running', 3, 'running')",
            [],
        )
        .expect("已有任务应能写入");
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!((report.material, report.queued), (2, 0));
        let conn = db.open();
        assert_eq!(job_count(&conn, "queued"), 1);
        assert_eq!(job_count(&conn, "running"), 1);
        assert_eq!(job_priority(&conn, "queued"), 3);
    }

    #[test]
    fn enqueue_changed_ignores_confirmed_removed_and_cardless() {
        let db = TempDb::new();
        let old = prose(80, 0);
        let now = prose(80, 400);
        let conn = db.open();
        insert_repo(&conn, "confirmed", "active");
        insert_readme(&conn, "confirmed", &now, "new");
        insert_card(
            &conn,
            "confirmed",
            r#"{"review_status":"confirmed"}"#,
            Some("old"),
            Some(&old),
        );
        insert_repo(&conn, "removed", "removed");
        insert_readme(&conn, "removed", &now, "new");
        insert_card(
            &conn,
            "removed",
            r#"{"review_status":"suggested"}"#,
            Some("old"),
            Some(&old),
        );
        insert_repo(&conn, "nocard", "active");
        insert_readme(&conn, "nocard", &now, "new");
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!(report, FollowReport::default());
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'confirmed'"
            )
            .as_deref(),
            Some("old")
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'removed'"
            )
            .as_deref(),
            Some("old")
        );
        assert_eq!(
            job_count(&conn, "confirmed")
                + job_count(&conn, "removed")
                + job_count(&conn, "nocard"),
            0
        );
    }

    #[test]
    fn enqueue_changed_short_readme_with_null_hash_does_not_queue() {
        let db = TempDb::new();
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", "太短", "hash-short");
        insert_card(&conn, "r", r#"{"review_status":"suggested"}"#, None, None);
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!((report.checked, report.material, report.queued), (1, 0, 0));
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            ),
            None
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            ),
            None
        );
    }

    #[test]
    fn constants_match_the_card() {
        assert_eq!(MATERIAL_SIMILARITY, 0.80);
        assert_eq!(MAX_REFRESH_PER_RUN, 150);
        assert_eq!(README_CHANGED_PRIORITY, 5);
    }

    // ———————— 新增：新常量 ————————

    #[test]
    fn the_new_constants_have_the_documented_values() {
        assert_eq!(REFRESH_TIME_BUDGET_SECS, 120);
        assert_eq!(NEW_STAR_README_BUDGET_SECS, 30);
        assert_eq!(REFRESH_STOP_AFTER_FAILURES, 5);
        // 卡里写的是 200 KB；放大到 4 MiB 的原因见常量上的说明和
        // `a_trivial_edit_to_a_big_readme_is_not_a_material_change_because_all_of_it_is_read`
        assert_eq!(README_EXCERPT_INPUT_BYTES, 4 * 1024 * 1024);
        assert!(
            README_EXCERPT_INPUT_BYTES > 1024 * 1024,
            "要比 GitHub 接口能返回的 README（最多 1 MB）大：正常的 README 一个字节都不截"
        );
        assert_eq!(MAX_REFRESH_PER_RUN, 150, "150 的上限保持不变");
        let old = FetchLimits::for_sync();
        assert_eq!(old.time_budget, Duration::from_secs(120));
        assert_eq!(old.stop_after_failures, 5);
        let new = FetchLimits::for_new_stars();
        assert_eq!(new.time_budget, Duration::from_secs(30));
        assert_eq!(new.stop_after_failures, 5);
        assert!(new.time_budget < old.time_budget, "排队在等新收藏这一步");
    }

    // ———————— 新增：最近 24 小时失败 / 被取消过的仓库不再排队 ————————

    const NOW: &str = "2026-10-05T12:00:00.000Z";

    fn before(secs: i64) -> String {
        crate::jobs::shift_db_time(NOW, -secs).expect("时间应能往前推")
    }

    const HOUR: i64 = 3_600;

    /// 一个重写过一半的 README：`enqueue_changed` 会判成明显改动并排队。
    fn insert_material_repo(conn: &Connection, id: &str) {
        insert_repo(conn, id, "active");
        insert_readme(conn, id, &prose(160, 2_000), "hash-new");
        insert_card(
            conn,
            id,
            r#"{"review_status":"suggested"}"#,
            Some("hash-old"),
            Some(&prose(160, 500)),
        );
    }

    fn insert_job_row(
        conn: &Connection,
        id: &str,
        repo_id: &str,
        job_type: &str,
        status: &str,
        finished_at: Option<&str>,
        updated_at: &str,
    ) {
        conn.execute(
            "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, priority, status, finished_at, created_at, updated_at)
             VALUES (?1, 'acct', ?2, 'repository', ?3, 5, ?4, ?5, ?6, ?6)",
            params![id, job_type, repo_id, status, finished_at, updated_at],
        )
        .expect("任务行应能写入");
    }

    fn run_at(db: &TempDb, now: &str) -> FollowReport {
        enqueue_changed_at(&db.path, now, &mut |_, _| {}).expect("评估应成功")
    }

    fn statuses(conn: &Connection, repo_id: &str) -> Vec<String> {
        let mut statement = conn
            .prepare("SELECT status FROM ai_jobs WHERE target_id = ?1 ORDER BY status")
            .expect("应能准备");
        statement
            .query_map(params![repo_id], |row| row.get(0))
            .expect("应能查")
            .collect::<Result<Vec<String>, _>>()
            .expect("应能读")
    }

    #[test]
    fn a_repo_whose_card_job_failed_in_the_last_24_hours_is_not_queued_again() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "r");
        let failed_at = before(2 * HOUR);
        insert_job_row(
            &conn,
            "f",
            "r",
            "repo_card",
            "failed",
            Some(&failed_at),
            &failed_at,
        );
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued,
                report.held_back
            ),
            (1, 1, 1, 0, 1)
        );
        assert_eq!(statuses(&db.open(), "r"), vec!["failed"], "不新排一条");
    }

    #[test]
    fn a_cancelled_card_job_holds_the_repo_back_too() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "r");
        let at = before(HOUR);
        insert_job_row(&conn, "c", "r", "repo_card", "cancelled", Some(&at), &at);
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!((report.queued, report.held_back), (0, 1));
        assert_eq!(statuses(&db.open(), "r"), vec!["cancelled"]);
    }

    #[test]
    fn a_failure_older_than_24_hours_does_not_hold_the_repo_back() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "r");
        let old = before(25 * HOUR);
        insert_job_row(&conn, "f", "r", "repo_card", "failed", Some(&old), &old);
        insert_material_repo(&conn, "r2");
        insert_job_row(&conn, "c", "r2", "repo_card", "cancelled", Some(&old), &old);
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!(
            (report.material, report.queued, report.held_back),
            (2, 2, 0)
        );
        let conn = db.open();
        assert_eq!(statuses(&conn, "r"), vec!["failed", "queued"]);
        assert_eq!(statuses(&conn, "r2"), vec!["cancelled", "queued"]);
        assert_eq!(job_priority_of_queued(&conn, "r"), README_CHANGED_PRIORITY);
    }

    fn job_priority_of_queued(conn: &Connection, repo_id: &str) -> i64 {
        conn.query_row(
            "SELECT priority FROM ai_jobs WHERE target_id = ?1 AND status = 'queued'",
            params![repo_id],
            |row| row.get(0),
        )
        .expect("排队的任务应能读到")
    }

    #[test]
    fn the_window_includes_exactly_24_hours_ago_and_ends_one_millisecond_later() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "edge");
        insert_material_repo(&conn, "past");
        let exactly = before(24 * HOUR);
        assert_eq!(exactly, "2026-10-04T12:00:00.000Z");
        insert_job_row(
            &conn,
            "e",
            "edge",
            "repo_card",
            "failed",
            Some(&exactly),
            &exactly,
        );
        // 比正好 24 小时前早 1 毫秒：窗口外
        let past = "2026-10-04T11:59:59.999Z";
        insert_job_row(&conn, "p", "past", "repo_card", "failed", Some(past), past);
        drop(conn);
        let report = run_at(&db, NOW);
        let conn = db.open();
        assert_eq!(
            statuses(&conn, "edge"),
            vec!["failed"],
            "正好 24 小时前：还在窗口里（和队列认领的窗口一样含这一刻）"
        );
        assert_eq!(statuses(&conn, "past"), vec!["failed", "queued"]);
        assert_eq!((report.queued, report.held_back), (1, 1));
    }

    #[test]
    fn the_hold_back_ends_by_itself_after_24_hours() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "r");
        let failed_at = before(2 * HOUR);
        insert_job_row(
            &conn,
            "f",
            "r",
            "repo_card",
            "failed",
            Some(&failed_at),
            &failed_at,
        );
        drop(conn);
        assert_eq!(run_at(&db, NOW).held_back, 1);
        assert_eq!(run_at(&db, NOW).held_back, 1, "没过 24 小时：每次都是拦下");
        // 22 小时后（失败发生在 24 小时窗口的最后 1 小时里）还在；过了 24 小时就放行
        let later = crate::jobs::shift_db_time(NOW, 22 * HOUR).unwrap();
        assert_eq!(run_at(&db, &later).held_back, 1);
        let later = crate::jobs::shift_db_time(NOW, 22 * HOUR + 1).unwrap();
        let report = run_at(&db, &later);
        assert_eq!((report.queued, report.held_back), (1, 0));
        assert_eq!(statuses(&db.open(), "r"), vec!["failed", "queued"]);
    }

    #[test]
    fn a_failure_counts_from_its_finish_time_and_a_cancel_from_the_moment_of_cancelling() {
        let db = TempDb::new();
        let conn = db.open();
        let repos = [
            // 失败：结束时间决定，updated_at 不算
            (
                "failed-finished-long-ago",
                "failed",
                Some(30 * HOUR),
                HOUR / 2,
            ),
            ("failed-finished-just-now", "failed", Some(HOUR), 40 * HOUR),
            // 失败但没有结束时间：退回看 updated_at
            ("failed-unfinished-recent", "failed", None, HOUR),
            ("failed-unfinished-old", "failed", None, 30 * HOUR),
            // 被取消：`v2_ops::cancel_job` 只把 updated_at 改成取消的那一刻，finished_at 沿用原来的记账时间——
            // 一条在队列里排了三天的任务，取消的时候 finished_at 已经是三天前，但刚取消就该算「最近」
            (
                "cancelled-after-a-long-wait",
                "cancelled",
                Some(72 * HOUR),
                HOUR / 2,
            ),
            (
                "cancelled-long-ago",
                "cancelled",
                Some(72 * HOUR),
                30 * HOUR,
            ),
            ("cancelled-unfinished", "cancelled", None, HOUR),
        ];
        for (index, (repo, status, finished_ago, updated_ago)) in repos.iter().enumerate() {
            insert_material_repo(&conn, repo);
            let finished = finished_ago.map(before);
            insert_job_row(
                &conn,
                &format!("job-{index}"),
                repo,
                "repo_card",
                status,
                finished.as_deref(),
                &before(*updated_ago),
            );
        }
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!(
            (report.material, report.queued, report.held_back),
            (7, 3, 4)
        );
        let conn = db.open();
        for held in [
            "failed-finished-just-now",
            "failed-unfinished-recent",
            "cancelled-after-a-long-wait",
            "cancelled-unfinished",
        ] {
            assert_eq!(job_count(&conn, held), 1, "{held} 该被拦下（不新排一条）");
        }
        for queued in [
            "failed-finished-long-ago",
            "failed-unfinished-old",
            "cancelled-long-ago",
        ] {
            assert_eq!(
                job_count(&conn, queued),
                2,
                "{queued} 该照排（旧的那条还在）"
            );
        }
    }

    #[test]
    fn failed_rows_written_the_way_the_other_job_tests_write_them_are_judged_by_their_finish_time()
    {
        // 其它任务测试的写法：只写 finished_at / created_at，updated_at 用库里的默认值（插入的那一刻）。
        // 那一刻是「现在」，不能因此把 25 小时前结束的失败当成最近的
        let db = TempDb::new();
        let conn = db.open();
        let now = crate::jobs::now_db_time();
        let recent = crate::jobs::shift_db_time(&now, -HOUR).unwrap();
        let old = crate::jobs::shift_db_time(&now, -25 * HOUR).unwrap();
        for (repo, finished) in [("failed-recent", &recent), ("failed-old", &old)] {
            insert_material_repo(&conn, repo);
            conn.execute(
                "INSERT INTO ai_jobs(id, account_id, job_type, target_type, target_id, status, finished_at, created_at)
                 VALUES (?1, 'acct', 'repo_card', 'repository', ?2, 'failed', ?3, ?3)",
                params![format!("job-{repo}"), repo, finished],
            )
            .expect("任务行应能写入");
        }
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!((report.queued, report.held_back), (1, 1));
        let conn = db.open();
        assert_eq!(statuses(&conn, "failed-old"), vec!["failed", "queued"]);
        assert_eq!(statuses(&conn, "failed-recent"), vec!["failed"]);
    }

    #[test]
    fn active_jobs_are_still_not_duplicated_and_are_not_counted_as_held_back() {
        let db = TempDb::new();
        let conn = db.open();
        for id in ["queued", "running"] {
            insert_material_repo(&conn, id);
            // 同一个仓库最近还失败过一条：已经有一条排着 / 跑着，既不重复排，也不算「被拦下」
            let at = before(HOUR);
            insert_job_row(
                &conn,
                &format!("f-{id}"),
                id,
                "repo_card",
                "failed",
                Some(&at),
                &at,
            );
        }
        insert_job_row(
            &conn,
            "q",
            "queued",
            "repo_card",
            "queued",
            None,
            &before(10),
        );
        insert_job_row(
            &conn,
            "r",
            "running",
            "repo_card",
            "running",
            None,
            &before(10),
        );
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!(
            (report.material, report.queued, report.held_back),
            (2, 0, 0)
        );
        let conn = db.open();
        assert_eq!(statuses(&conn, "queued"), vec!["failed", "queued"]);
        assert_eq!(statuses(&conn, "running"), vec!["failed", "running"]);
    }

    #[test]
    fn another_repos_failure_does_not_hold_this_repo_back() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "unlucky");
        insert_material_repo(&conn, "fine");
        let at = before(HOUR);
        insert_job_row(&conn, "f", "unlucky", "repo_card", "failed", Some(&at), &at);
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!(
            (report.material, report.queued, report.held_back),
            (2, 1, 1)
        );
        let conn = db.open();
        assert_eq!(statuses(&conn, "fine"), vec!["queued"]);
        assert_eq!(statuses(&conn, "unlucky"), vec!["failed"]);
    }

    #[test]
    fn only_failed_or_cancelled_card_jobs_hold_a_repo_back() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "r");
        let at = before(HOUR);
        // 成功、跳过的不算；别的任务类型（同一个仓库）失败的也不算
        insert_job_row(&conn, "ok", "r", "repo_card", "succeeded", Some(&at), &at);
        insert_job_row(&conn, "skip", "r", "repo_card", "skipped", Some(&at), &at);
        insert_job_row(&conn, "other", "r", "categorize", "failed", Some(&at), &at);
        insert_job_row(
            &conn,
            "other2",
            "r",
            "categorize",
            "cancelled",
            Some(&at),
            &at,
        );
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!((report.queued, report.held_back), (1, 0));
    }

    #[test]
    fn a_failure_row_with_no_finish_time_still_counts_by_its_update_time() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "fresh");
        insert_material_repo(&conn, "stale");
        insert_job_row(
            &conn,
            "a",
            "fresh",
            "repo_card",
            "cancelled",
            None,
            &before(HOUR),
        );
        insert_job_row(
            &conn,
            "b",
            "stale",
            "repo_card",
            "cancelled",
            None,
            &before(30 * HOUR),
        );
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!((report.queued, report.held_back), (1, 1));
        assert_eq!(statuses(&db.open(), "stale"), vec!["cancelled", "queued"]);
    }

    #[test]
    fn a_repo_with_no_recorded_hash_is_held_back_the_same_way() {
        let db = TempDb::new();
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &prose(90, 0), "hash-now");
        insert_card(&conn, "r", r#"{"review_status":"suggested"}"#, None, None);
        let at = before(HOUR);
        insert_job_row(&conn, "f", "r", "repo_card", "failed", Some(&at), &at);
        drop(conn);
        let report = run_at(&db, NOW);
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued,
                report.held_back
            ),
            (1, 0, 1, 0, 1)
        );
    }

    #[test]
    fn holding_back_does_not_touch_the_hash_or_the_basis() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "r");
        let at = before(HOUR);
        insert_job_row(&conn, "f", "r", "repo_card", "failed", Some(&at), &at);
        drop(conn);
        run_at(&db, NOW);
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some("hash-old"),
            "没排队就不能把哈希更新掉：24 小时后还要能看出它改过"
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some(prose(160, 500).as_str())
        );
    }

    #[test]
    fn the_public_entry_point_uses_the_real_clock_for_the_window() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "recent");
        insert_material_repo(&conn, "old");
        let now = crate::jobs::now_db_time();
        let recent = crate::jobs::shift_db_time(&now, -HOUR).unwrap();
        let old = crate::jobs::shift_db_time(&now, -30 * HOUR).unwrap();
        insert_job_row(
            &conn,
            "a",
            "recent",
            "repo_card",
            "failed",
            Some(&recent),
            &recent,
        );
        insert_job_row(&conn, "b", "old", "repo_card", "failed", Some(&old), &old);
        drop(conn);
        let report = enqueue_changed(&db.path).expect("评估应成功");
        assert_eq!((report.queued, report.held_back), (1, 1));
    }

    // ———————— 新增：摘录在写事务之外算 ————————

    /// 此刻别的连接能不能拿到写锁（不等：`busy_timeout` 是 0）。
    fn write_lock_is_free(path: &Path) -> bool {
        let conn = Connection::open(path).expect("应能再开一个连接");
        conn.busy_timeout(Duration::ZERO).expect("应能设超时");
        match conn.execute_batch("BEGIN IMMEDIATE; ROLLBACK;") {
            Ok(()) => true,
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == rusqlite::ErrorCode::DatabaseBusy =>
            {
                false
            }
            Err(other) => panic!("意外的错误：{other}"),
        }
    }

    #[test]
    fn the_excerpt_is_computed_before_the_write_transaction_opens() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "a");
        insert_material_repo(&conn, "b");
        drop(conn);
        let mut seen: Vec<(FollowStage, String, bool)> = Vec::new();
        let path = db.path.clone();
        let report = enqueue_changed_at(&db.path, NOW, &mut |stage, repo| {
            seen.push((stage, repo.to_owned(), write_lock_is_free(&path)));
        })
        .expect("评估应成功");
        assert_eq!(report.queued, 2);
        let expected = |repo: &str| {
            vec![
                (FollowStage::Evaluating, repo.to_owned(), true),
                (FollowStage::BeforeWrite, repo.to_owned(), true),
                // 事务开了（IMMEDIATE）：别的连接拿不到写锁
                (FollowStage::InTransaction, repo.to_owned(), false),
            ]
        };
        let mut want = expected("a");
        want.extend(expected("b"));
        assert_eq!(
            seen, want,
            "摘录和比较（Evaluating → BeforeWrite 之间）不占写锁；写事务一开就是 IMMEDIATE，一仓库一个小事务"
        );
        // 事务提交之后写锁又空出来了
        assert!(write_lock_is_free(&db.path));
    }

    /// 压缩成一整行的恶意页面：每种没写完的标记塞满 1 MB。
    fn hostile_readme(pattern: &str) -> String {
        pattern.repeat(1_000_000 / pattern.len() + 1)
    }

    #[test]
    fn a_one_megabyte_line_of_unfinished_markup_does_not_stall_enqueue_changed() {
        for pattern in ["![", "[](", "&lt;span", "<span", "http://", "<a", "![a]("] {
            let db = TempDb::new();
            let conn = db.open();
            insert_repo(&conn, "r", "active");
            let raw = hostile_readme(pattern);
            assert!(raw.len() >= 1_000_000);
            insert_readme(&conn, "r", &raw, "hash-new");
            insert_card(
                &conn,
                "r",
                r#"{"review_status":"suggested"}"#,
                Some("hash-old"),
                Some(&prose(200, 0)),
            );
            drop(conn);
            let started = Instant::now();
            let report = enqueue_changed(&db.path).expect("评估应成功");
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "{pattern}：1 MB 一行评估了 {:?}",
                started.elapsed()
            );
            assert_eq!((report.checked, report.changed), (1, 1), "{pattern}");
            // 评估完整地走完了。只有 `&lt;span` 这一种的清洗结果里还剩下字母（`ltspan…`），比对起来和基准完全不同、判成明显改动并排队；
            // 别的都是没写完的标记，规范化时整段丢掉，摘录里没有可比的文字：不算明显改动，只更新哈希
            let queued = usize::from(pattern == "&lt;span");
            assert_eq!(report.queued, queued, "{pattern}");
            assert_eq!(report.material, queued, "{pattern}");
            let hash = optional_text(
                &db.open(),
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'",
            );
            assert_eq!(
                hash.as_deref(),
                Some(if queued == 1 { "hash-old" } else { "hash-new" }),
                "{pattern}"
            );
        }
    }

    #[test]
    fn while_a_hostile_readme_is_evaluated_other_writers_are_not_blocked() {
        let db = TempDb::new();
        let conn = db.open();
        for (index, pattern) in ["![", "[](", "<span", "http://", "<a"].iter().enumerate() {
            let id = format!("h{index}");
            insert_repo(&conn, &id, "active");
            insert_readme(&conn, &id, &hostile_readme(pattern), "hash-new");
            insert_card(
                &conn,
                &id,
                r#"{"review_status":"suggested"}"#,
                Some("hash-old"),
                Some(&prose(200, 0)),
            );
        }
        drop(conn);
        let path = db.path.clone();
        let mut longest = Duration::ZERO;
        let mut evaluating_started: Option<Instant> = None;
        let mut free_while_evaluating = true;
        enqueue_changed_at(&db.path, NOW, &mut |stage, _| match stage {
            FollowStage::Evaluating => {
                free_while_evaluating &= write_lock_is_free(&path);
                evaluating_started = Some(Instant::now());
            }
            FollowStage::BeforeWrite => {
                if let Some(started) = evaluating_started.take() {
                    longest = longest.max(started.elapsed());
                }
                free_while_evaluating &= write_lock_is_free(&path);
            }
            FollowStage::InTransaction => {}
        })
        .expect("评估应成功");
        assert!(free_while_evaluating, "算摘录期间写锁一直是空的");
        assert!(
            longest < Duration::from_secs(3),
            "最慢的一份摘录算了 {longest:?}"
        );
    }

    #[test]
    fn a_readme_that_changed_between_reading_and_writing_is_skipped_until_next_time() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "a");
        insert_material_repo(&conn, "b");
        drop(conn);
        let path = db.path.clone();
        let report = enqueue_changed_at(&db.path, NOW, &mut |stage, repo| {
            if stage == FollowStage::BeforeWrite && repo == "a" {
                // 读完之后、写之前，同步把 a 的 README 换了一份
                Connection::open(&path)
                    .expect("应能再开一个连接")
                    .execute(
                        "UPDATE repo_readmes SET content_hash = 'changed-meanwhile' WHERE repo_id = 'a'",
                        [],
                    )
                    .expect("应能改");
            }
        })
        .expect("评估应成功");
        assert_eq!(
            (report.checked, report.material, report.queued),
            (1, 1, 1),
            "只有 b 评估完了；a 被跳过：不计数、不写"
        );
        let conn = db.open();
        assert_eq!(job_count(&conn, "a"), 0);
        assert_eq!(job_count(&conn, "b"), 1);
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'a'"
            )
            .as_deref(),
            Some("hash-old")
        );
        drop(conn);
        // 下次再说：这回 a 照常评估、照常排队
        let report = run_at(&db, NOW);
        assert_eq!((report.checked, report.queued), (2, 1));
        assert_eq!(job_count(&db.open(), "a"), 1);
    }

    #[test]
    fn a_card_regenerated_between_reading_and_writing_is_skipped() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "a");
        drop(conn);
        let path = db.path.clone();
        let report = enqueue_changed_at(&db.path, NOW, &mut |stage, _| {
            if stage == FollowStage::BeforeWrite {
                // 队列刚好把这张卡重新整理完：记过的哈希变成了现在的
                Connection::open(&path)
                    .expect("应能再开一个连接")
                    .execute(
                        "UPDATE repo_ai_documents SET readme_hash = 'hash-new' WHERE repo_id = 'a'",
                        [],
                    )
                    .expect("应能改");
            }
        })
        .expect("评估应成功");
        assert_eq!((report.checked, report.queued), (0, 0));
        assert_eq!(job_count(&db.open(), "a"), 0, "已经整理好的卡不能再排一遍");
    }

    #[test]
    fn a_basis_filled_in_or_an_account_moved_between_reading_and_writing_is_skipped_too() {
        let db = TempDb::new();
        let conn = db.open();
        // 基准是空的、哈希变了：会「采用现在的当基准」。读完之后别人把基准补上了：不能再按旧的判断去写
        insert_repo(&conn, "basis", "active");
        insert_readme(&conn, "basis", &prose(90, 0), "hash-new");
        insert_card(
            &conn,
            "basis",
            r#"{"review_status":"suggested"}"#,
            Some("hash-old"),
            None,
        );
        insert_material_repo(&conn, "moved");
        conn.execute(
            "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct2', 'bar', 'test')",
            [],
        )
        .expect("第二个账号应能写入");
        drop(conn);
        let path = db.path.clone();
        let report = enqueue_changed_at(&db.path, NOW, &mut |stage, repo| {
            if stage != FollowStage::BeforeWrite {
                return;
            }
            let conn = Connection::open(&path).expect("应能再开一个连接");
            match repo {
                "basis" => conn.execute(
                    "UPDATE repo_ai_documents SET readme_basis = '别人刚补上的基准' WHERE repo_id = 'basis'",
                    [],
                ),
                _ => conn.execute(
                    "UPDATE repositories SET account_id = 'acct2' WHERE id = 'moved'",
                    [],
                ),
            }
            .expect("应能改");
        })
        .expect("评估应成功");
        assert_eq!((report.checked, report.queued), (0, 0));
        let conn = db.open();
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'basis'"
            )
            .as_deref(),
            Some("hash-old"),
            "没有按旧的判断去写"
        );
        assert_eq!(
            optional_text(
                &conn,
                "SELECT readme_basis FROM repo_ai_documents WHERE repo_id = 'basis'"
            )
            .as_deref(),
            Some("别人刚补上的基准")
        );
        assert_eq!(job_count(&conn, "moved"), 0);
    }

    #[test]
    fn a_card_confirmed_or_a_repo_removed_between_reading_and_writing_is_left_alone() {
        let db = TempDb::new();
        let conn = db.open();
        insert_material_repo(&conn, "confirmed-meanwhile");
        insert_material_repo(&conn, "removed-meanwhile");
        insert_material_repo(&conn, "plain");
        drop(conn);
        let path = db.path.clone();
        let report = enqueue_changed_at(&db.path, NOW, &mut |stage, repo| {
            if stage != FollowStage::BeforeWrite {
                return;
            }
            let conn = Connection::open(&path).expect("应能再开一个连接");
            match repo {
                "confirmed-meanwhile" => conn.execute(
                    "UPDATE repo_ai_documents SET card_json = '{\"review_status\":\"confirmed\"}' WHERE repo_id = ?1",
                    params![repo],
                ),
                "removed-meanwhile" => conn.execute(
                    "UPDATE repositories SET sync_status = 'removed' WHERE id = ?1",
                    params![repo],
                ),
                _ => Ok(0),
            }
            .expect("应能改");
        })
        .expect("评估应成功");
        assert_eq!((report.checked, report.queued), (1, 1), "只有 plain 排上了");
        let conn = db.open();
        assert_eq!(job_count(&conn, "confirmed-meanwhile"), 0, "已确认的卡不碰");
        assert_eq!(job_count(&conn, "removed-meanwhile"), 0);
        assert_eq!(job_count(&conn, "plain"), 1);
    }

    #[test]
    fn the_head_is_cut_at_a_character_boundary_for_every_kind_of_character() {
        for unit in ["a", "é", "字", "😀"] {
            let text = unit.repeat(20);
            for max in 0..=text.len() + 3 {
                let end = char_boundary_at_most(&text, max);
                assert!(end <= max, "{unit:?} {max}");
                assert!(text.is_char_boundary(end), "{unit:?} {max}");
                assert!(
                    (end + 1..=max.min(text.len())).all(|more| !text.is_char_boundary(more)),
                    "{unit:?} {max}：还能多保留一些"
                );
                // 切出来的是合法的 UTF-8 前缀
                assert!(text.starts_with(&text[..end]));
            }
        }
        assert_eq!(char_boundary_at_most("", 5), 0);
    }

    #[test]
    fn a_huge_multibyte_readme_is_read_as_a_valid_head_cut_at_the_cap() {
        let db = TempDb::new();
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        // 3 字节一个字：上限 204800 这一刀正好落在字的中间（204800 不是 3 的倍数）
        let cap = 200 * 1024;
        let raw = "字".repeat(400_000);
        assert!(!raw.is_char_boundary(cap));
        insert_readme(&conn, "r", &raw, "hash-new");
        insert_card(&conn, "r", r#"{"review_status":"suggested"}"#, None, None);
        let candidate = read_candidate(&conn, "r", cap)
            .expect("应能读")
            .expect("应是候选");
        assert!(candidate.head.len() <= cap);
        assert!(candidate.head.len() > cap - 4);
        assert_eq!(candidate.head.len() % 3, 0, "整字整字地留");
        assert_eq!(candidate.content_hash, "hash-new");
        // 短的 README 原样读出来
        conn.execute(
            "UPDATE repo_readmes SET raw_markdown = '很短' WHERE repo_id = 'r'",
            [],
        )
        .expect("应能改");
        assert_eq!(
            read_candidate(&conn, "r", cap).unwrap().unwrap().head,
            "很短"
        );
        assert!(read_candidate(&conn, "nobody", cap).unwrap().is_none());
    }

    #[test]
    fn a_readme_longer_than_the_real_cap_is_still_read_only_up_to_the_cap() {
        let db = TempDb::new();
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        // 450 万字节的多字节文字：比线上的上限（4 MiB）还大，切在字的中间
        let raw = "字".repeat(1_500_000);
        assert!(raw.len() > README_EXCERPT_INPUT_BYTES);
        assert!(!raw.is_char_boundary(README_EXCERPT_INPUT_BYTES));
        insert_readme(&conn, "r", &raw, "hash-new");
        insert_card(&conn, "r", r#"{"review_status":"suggested"}"#, None, None);
        let candidate = read_candidate(&conn, "r", README_EXCERPT_INPUT_BYTES)
            .expect("应能读")
            .expect("应是候选");
        assert!(candidate.head.len() <= README_EXCERPT_INPUT_BYTES);
        assert!(candidate.head.len() > README_EXCERPT_INPUT_BYTES - 4);
        assert_eq!(candidate.head.len() % 3, 0);
    }

    /// 一份 460 KB 的长 README（awesome 列表那样）：开头一段说明、很长的闲话、「安装」一节排在 200 KB 之后。
    fn big_readme() -> String {
        let head = format!("# 项目\n\n{}\n\n", prose(1_500, 0));
        let filler = "这是中间很长的闲话，不是标题也没有关键词。\n".repeat(7_000);
        let tail = format!("\n## 安装\n\n{}\n", prose(3_000, 3_000));
        format!("{head}{filler}{tail}")
    }

    #[test]
    fn what_lies_beyond_the_cap_is_not_looked_at() {
        // 上限是个参数：用 200 KB 的小上限造夹具，看超出的部分真的不读
        const CAP: usize = 200 * 1024;
        let full = big_readme();
        assert!(full.len() > CAP + 9_000);
        let cut = &full[..char_boundary_at_most(&full, CAP)];
        assert!(!cut.contains("## 安装"), "「安装」一节在上限之外");
        let basis = build_readme_excerpt(cut).text;
        let whole = build_readme_excerpt(&full).text;
        assert!(
            is_material_change(Some(&basis), Some(&whole)),
            "要是整份都读，这一大节会让摘录明显不同"
        );

        let db = TempDb::new();
        let conn = db.open();
        insert_repo(&conn, "r", "active");
        insert_readme(&conn, "r", &full, "hash-new");
        insert_card(
            &conn,
            "r",
            r#"{"review_status":"suggested"}"#,
            Some("hash-old"),
            Some(&basis),
        );
        drop(conn);
        let report =
            enqueue_changed_with_cap(&db.path, NOW, CAP, &mut |_, _| {}).expect("评估应成功");
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued
            ),
            (1, 1, 0, 0),
            "只读上限以内的：摘录没变，不算明显改动，只更新哈希"
        );
        assert_eq!(
            optional_text(
                &db.open(),
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some("hash-new")
        );
    }

    #[test]
    fn a_trivial_edit_to_a_big_readme_is_not_a_material_change_because_all_of_it_is_read() {
        // 卡流水线写 `readme_basis` 时用的是整份 README 的摘录（「安装」一节在 200 KB 之后，也在里面）。
        // 比较时也得读整份：要是只读前 200 KB，摘录里少了一大节，相似度掉到 0.34，只加一行徽章也会被当成明显改动、白花一次重做
        let full = big_readme();
        let basis = build_readme_excerpt(&full).text;
        let edited = format!("![build](https://img.shields.io/badge/build-passing-green)\n{full}");
        let seed = |db: &TempDb| {
            let conn = db.open();
            insert_repo(&conn, "r", "active");
            insert_readme(&conn, "r", &edited, "hash-new");
            insert_card(
                &conn,
                "r",
                r#"{"review_status":"suggested"}"#,
                Some("hash-old"),
                Some(&basis),
            );
        };
        let db = TempDb::new();
        seed(&db);
        let report = run_at(&db, NOW);
        assert_eq!(
            (
                report.checked,
                report.changed,
                report.material,
                report.queued
            ),
            (1, 1, 0, 0),
            "线上的读取上限比任何真实 README 都大：只加一行徽章，只更新哈希"
        );
        assert_eq!(
            optional_text(
                &db.open(),
                "SELECT readme_hash FROM repo_ai_documents WHERE repo_id = 'r'"
            )
            .as_deref(),
            Some("hash-new")
        );
        // 对照：只读前 200 KB 的话，同一份 README 会被判成明显改动并排队——这就是上限要放得这么大的原因
        let small = TempDb::new();
        seed(&small);
        let report = enqueue_changed_with_cap(&small.path, NOW, 200 * 1024, &mut |_, _| {})
            .expect("评估应成功");
        assert_eq!((report.material, report.queued), (1, 1));
    }

    // ———————— 新增：新收藏之后的老 README 刷新要去掉第二步抓过的 ————————

    fn many_states(count: usize, fetched: Option<&str>) -> Vec<RepoReadmeState> {
        (0..count)
            .map(|index| {
                state(
                    &format!("s{index:03}"),
                    Some("2026-10-01T00:00:00Z"),
                    fetched,
                )
            })
            .collect()
    }

    #[test]
    fn excluded_repos_are_removed_before_the_150_cap_not_after() {
        let repos = many_states(200, None);
        let exclude: HashSet<String> = (0..50).map(|index| format!("s{index:03}")).collect();
        let ids = refresh_targets_excluding(&repos, &exclude);
        assert_eq!(ids.len(), 150, "去掉的不占名额：还是满 150 个");
        assert_eq!(ids[0], "s050");
        assert_eq!(ids[149], "s199");
        assert!(ids.iter().all(|id| !exclude.contains(id)));
        // 不带名单的写法和以前一样
        assert_eq!(refresh_targets(&repos)[0], "s000");
        assert_eq!(
            refresh_targets_excluding(&repos, &HashSet::new()),
            refresh_targets(&repos)
        );
    }

    #[test]
    fn exclusion_keeps_the_missing_cache_first_order_and_ignores_unknown_ids() {
        let repos = vec![
            state(
                "old",
                Some("2026-10-01T00:00:00Z"),
                Some("2020-01-01T00:00:00Z"),
            ),
            state("no-cache", None, None),
            state("also-no-cache", Some("2026-01-01T00:00:00Z"), None),
            state(
                "fresh",
                Some("2026-01-01T00:00:00Z"),
                Some("2026-06-01T00:00:00Z"),
            ),
        ];
        let exclude: HashSet<String> = ["also-no-cache", "not-even-listed"]
            .iter()
            .map(|id| (*id).to_owned())
            .collect();
        assert_eq!(
            refresh_targets_excluding(&repos, &exclude),
            vec!["no-cache".to_owned(), "old".to_owned()]
        );
    }

    // ———————— 新增：分批抓 README（总时限、连续失败熔断）————————

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Reply {
        Readme,
        Missing,
        /// 断网、超时、限流这类：网有问题。
        Failure,
        /// GitHub 回了，但这一个仓库的 README 本身不能用（不是 UTF-8 / 编码不支持 / 解析不了）。
        BadReadme,
        Panic,
    }

    struct FakeIo {
        replies: std::collections::HashMap<String, Reply>,
        store_fails: HashSet<String>,
        /// 每抓一个，假时钟往前走这么多毫秒。
        fetch_cost_ms: u64,
        clock_ms: std::sync::atomic::AtomicU64,
        base: Instant,
        log: std::sync::Mutex<Vec<String>>,
        progress: std::sync::Mutex<Vec<(usize, usize, Vec<String>)>>,
    }

    impl FakeIo {
        fn new() -> Self {
            Self {
                replies: std::collections::HashMap::new(),
                store_fails: HashSet::new(),
                fetch_cost_ms: 0,
                clock_ms: std::sync::atomic::AtomicU64::new(0),
                base: Instant::now(),
                log: std::sync::Mutex::new(Vec::new()),
                progress: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn replying(mut self, id: &str, reply: Reply) -> Self {
            self.replies.insert(id.to_owned(), reply);
            self
        }

        fn costing(mut self, millis: u64) -> Self {
            self.fetch_cost_ms = millis;
            self
        }

        fn fetched(&self) -> Vec<String> {
            let mut ids: Vec<String> = self
                .log
                .lock()
                .unwrap()
                .iter()
                .filter_map(|line| line.strip_prefix("fetch:").map(str::to_owned))
                .collect();
            ids.sort();
            ids
        }

        fn stored(&self) -> Vec<String> {
            self.log
                .lock()
                .unwrap()
                .iter()
                .filter_map(|line| line.strip_prefix("store:").map(str::to_owned))
                .collect()
        }
    }

    impl ReadmeIo for FakeIo {
        fn fetch(&self, repo_id: &str, _full_name: &str) -> Result<Option<ReadmeDocument>, String> {
            self.log.lock().unwrap().push(format!("fetch:{repo_id}"));
            self.clock_ms
                .fetch_add(self.fetch_cost_ms, std::sync::atomic::Ordering::SeqCst);
            match self.replies.get(repo_id).copied().unwrap_or(Reply::Readme) {
                Reply::Readme => Ok(Some(ReadmeDocument {
                    repo_id: repo_id.to_owned(),
                    raw_markdown: format!("# {repo_id}"),
                    content_hash: format!("hash-{repo_id}"),
                    source_path: "README.md".to_owned(),
                    fetched_at: "2026-10-05T00:00:00Z".to_owned(),
                })),
                Reply::Missing => Ok(None),
                Reply::Failure => Err("GitHub API 请求超时".to_owned()),
                Reply::BadReadme => Err("GitHub README 不是有效 UTF-8 文本".to_owned()),
                Reply::Panic => panic!("测试用：抓取线程异常退出"),
            }
        }

        fn store(&self, readme: &ReadmeDocument) -> Result<(), String> {
            self.log
                .lock()
                .unwrap()
                .push(format!("store:{}", readme.repo_id));
            if self.store_fails.contains(&readme.repo_id) {
                Err("README 写入失败".to_owned())
            } else {
                Ok(())
            }
        }

        fn now(&self) -> Instant {
            self.base
                + Duration::from_millis(self.clock_ms.load(std::sync::atomic::Ordering::SeqCst))
        }

        fn progress(&self, done: usize, total: usize, batch_names: &[String]) {
            self.progress
                .lock()
                .unwrap()
                .push((done, total, batch_names.to_vec()));
        }
    }

    fn targets(count: usize) -> Vec<(String, String)> {
        (0..count)
            .map(|index| (format!("t{index:02}"), format!("fox/t{index:02}")))
            .collect()
    }

    fn unlimited() -> FetchLimits {
        FetchLimits {
            time_budget: Duration::from_secs(3_600),
            stop_after_failures: REFRESH_STOP_AFTER_FAILURES,
        }
    }

    #[test]
    fn a_clean_run_fetches_stores_and_counts_everything() {
        let io = FakeIo::new()
            .replying("t01", Reply::Missing)
            .replying("t02", Reply::Failure);
        let report = fetch_in_batches(targets(8), 3, unlimited(), &io);
        assert_eq!(
            (
                report.refreshed,
                report.missed,
                report.failed,
                report.stopped
            ),
            (6, 1, 1, None)
        );
        assert_eq!(
            report.attempted,
            (0..8)
                .map(|index| format!("t{index:02}"))
                .collect::<Vec<_>>(),
            "按顺序，成败都算抓过"
        );
        assert_eq!(io.fetched().len(), 8);
        assert_eq!(io.stored().len(), 6, "没有 README 的和抓取失败的不落库");
        assert!(!io.stored().contains(&"t01".to_owned()));
    }

    #[test]
    fn nothing_to_fetch_means_nothing_happens() {
        let io = FakeIo::new();
        let report = fetch_in_batches(Vec::new(), 6, unlimited(), &io);
        assert_eq!(report, FetchReport::default());
        assert!(io.progress.lock().unwrap().is_empty());
    }

    #[test]
    fn progress_is_reported_before_each_batch_with_the_running_total() {
        let io = FakeIo::new();
        fetch_in_batches(targets(7), 3, unlimited(), &io);
        let progress = io.progress.lock().unwrap().clone();
        assert_eq!(
            progress,
            vec![
                (
                    0,
                    7,
                    vec![
                        "fox/t00".to_owned(),
                        "fox/t01".to_owned(),
                        "fox/t02".to_owned()
                    ]
                ),
                (
                    3,
                    7,
                    vec![
                        "fox/t03".to_owned(),
                        "fox/t04".to_owned(),
                        "fox/t05".to_owned()
                    ]
                ),
                (6, 7, vec!["fox/t06".to_owned()]),
            ]
        );
    }

    #[test]
    fn the_run_stops_when_the_total_time_budget_is_used_up_on_an_injected_clock() {
        // 每抓一个假时钟走 10 秒，每批 2 个：一批 20 秒；总时限 120 秒
        let io = FakeIo::new().costing(10_000);
        let limits = FetchLimits {
            time_budget: Duration::from_secs(120),
            stop_after_failures: 5,
        };
        let report = fetch_in_batches(targets(30), 2, limits, &io);
        assert_eq!(report.stopped, Some(StopReason::TimeBudget));
        assert_eq!(
            report.attempted.len(),
            12,
            "第 0、20、…、100 秒各开一批，第 120 秒正好用完，不再开新的一批"
        );
        assert_eq!(report.refreshed, 12);
        assert_eq!(
            io.fetched().len(),
            12,
            "剩下的 18 个一个都没抓：留给下一次同步"
        );
    }

    #[test]
    fn a_batch_already_in_flight_when_time_runs_out_is_finished() {
        // 一批 6 个、每个 30 秒：开第一批时还没到点，做完已经 180 秒；不再开第二批
        let io = FakeIo::new().costing(30_000);
        let limits = FetchLimits {
            time_budget: Duration::from_secs(120),
            stop_after_failures: 5,
        };
        let report = fetch_in_batches(targets(20), 6, limits, &io);
        assert_eq!(report.stopped, Some(StopReason::TimeBudget));
        assert_eq!(report.attempted.len(), 6);
        assert_eq!(report.refreshed, 6, "这一批的结果照常落库");
        assert_eq!(io.stored().len(), 6);
    }

    #[test]
    fn a_zero_budget_fetches_nothing_and_an_unspent_budget_does_not_stop_anything() {
        let io = FakeIo::new();
        let none = FetchLimits {
            time_budget: Duration::ZERO,
            stop_after_failures: 5,
        };
        let report = fetch_in_batches(targets(5), 2, none, &io);
        assert_eq!(report.stopped, Some(StopReason::TimeBudget));
        assert!(report.attempted.is_empty());
        assert!(io.fetched().is_empty());
        // 时钟没走：全部抓完，没有提前停
        let io = FakeIo::new();
        let report = fetch_in_batches(targets(5), 2, FetchLimits::for_sync(), &io);
        assert_eq!((report.stopped, report.refreshed), (None, 5));
    }

    fn failing(ids: &[&str]) -> FakeIo {
        ids.iter()
            .fold(FakeIo::new(), |io, id| io.replying(id, Reply::Failure))
    }

    #[test]
    fn five_failures_in_a_row_stop_the_run() {
        // 一批一个：恰好第 5 个失败之后停，第 6 个没有抓
        let io = failing(&["t00", "t01", "t02", "t03", "t04", "t05", "t06", "t07"]);
        let report = fetch_in_batches(targets(20), 1, unlimited(), &io);
        assert_eq!(report.stopped, Some(StopReason::Failures));
        assert_eq!(report.attempted.len(), 5);
        assert_eq!((report.failed, report.refreshed), (5, 0));
        assert_eq!(io.fetched().len(), 5);
    }

    #[test]
    fn four_failures_in_a_row_are_not_enough() {
        let io = failing(&["t00", "t01", "t02", "t03"]);
        let report = fetch_in_batches(targets(10), 1, unlimited(), &io);
        assert_eq!(report.stopped, None);
        assert_eq!(report.attempted.len(), 10);
        assert_eq!((report.failed, report.refreshed), (4, 6));
    }

    #[test]
    fn a_success_resets_the_failure_count() {
        // 失败 4 个、成功 1 个、再失败 4 个、成功 1 个……从来没有连着 5 个
        let io = failing(&["t00", "t01", "t02", "t03", "t05", "t06", "t07", "t08"]);
        let report = fetch_in_batches(targets(10), 1, unlimited(), &io);
        assert_eq!(report.stopped, None, "{report:?}");
        assert_eq!((report.failed, report.refreshed), (8, 2));
        // 成功之后再连着 5 个失败：停
        let io = failing(&[
            "t00", "t01", "t02", "t03", "t05", "t06", "t07", "t08", "t09",
        ]);
        let report = fetch_in_batches(targets(20), 1, unlimited(), &io);
        assert_eq!(report.stopped, Some(StopReason::Failures));
        assert_eq!(report.attempted.len(), 10, "t05–t09 连着 5 个失败");
    }

    #[test]
    fn a_reply_of_no_readme_also_resets_the_failure_count() {
        // GitHub 回了「没有 README」说明网是通的，不算失败，也让失败的计数重新数
        let io = failing(&["t00", "t01", "t02", "t03", "t05", "t06", "t07", "t08"])
            .replying("t04", Reply::Missing);
        let report = fetch_in_batches(targets(9), 1, unlimited(), &io);
        assert_eq!(report.stopped, None, "{report:?}");
        assert_eq!((report.failed, report.missed), (8, 1));
    }

    fn bad_readmes(ids: &[&str]) -> FakeIo {
        ids.iter()
            .fold(FakeIo::new(), |io, id| io.replying(id, Reply::BadReadme))
    }

    #[test]
    fn repos_whose_own_readme_is_unusable_never_trip_the_breaker_but_are_still_counted_as_failed() {
        // 20 个仓库的 README 都不是 UTF-8（或超过 1 MB）：GitHub 回得好好的，不是网的问题，不能把后面的全拦住
        let ids: Vec<String> = (0..20).map(|index| format!("t{index:02}")).collect();
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let io = bad_readmes(&id_refs);
        let report = fetch_in_batches(targets(25), 1, unlimited(), &io);
        assert_eq!(report.stopped, None, "{report:?}");
        assert_eq!(report.attempted.len(), 25);
        assert_eq!(
            (report.failed, report.refreshed),
            (20, 5),
            "后面 5 个照常抓到"
        );
    }

    #[test]
    fn an_unusable_readme_resets_the_count_of_network_failures_in_a_row() {
        // 网络失败 4 个、一个「README 不能用」（GitHub 回了）、再网络失败 4 个：从来没有连着 5 个
        let io = failing(&["t00", "t01", "t02", "t03", "t05", "t06", "t07", "t08"])
            .replying("t04", Reply::BadReadme);
        let report = fetch_in_batches(targets(9), 1, unlimited(), &io);
        assert_eq!(report.stopped, None, "{report:?}");
        assert_eq!(report.failed, 9);
        // 之后再连着 5 个网络失败：照样停
        let io = failing(&["t05", "t06", "t07", "t08", "t09"]).replying("t04", Reply::BadReadme);
        let report = fetch_in_batches(targets(20), 1, unlimited(), &io);
        assert_eq!(report.stopped, Some(StopReason::Failures));
        assert_eq!(report.attempted.len(), 10);
    }

    #[test]
    fn only_the_known_readme_content_errors_are_told_apart_from_network_ones() {
        for content in [
            "GitHub README 不是有效 UTF-8 文本",
            "GitHub README 使用了不支持的编码：none",
            "GitHub README 解码失败：Invalid byte 61, offset 0.",
            "GitHub README 响应解析失败：expected value at line 1 column 1",
        ] {
            assert!(is_repo_problem(content), "{content}");
        }
        for network in [
            "GitHub API 请求超时，请检查网络连接或稍后重试（已等待 45 秒）。",
            "GitHub API 请求失败：error sending request",
            "GitHub API 请求过于频繁，请稍后再试（HTTP 429）",
            "Token 无效或权限不足，请重新检查 GitHub Personal Access Token（HTTP 401）",
            "GitHub 服务暂时不可用，请稍后重试（HTTP 503）",
            "README 写入失败",
            "",
        ] {
            assert!(!is_repo_problem(network), "{network}");
        }
    }

    #[test]
    fn failures_to_store_count_like_failures_to_fetch() {
        let mut io = FakeIo::new();
        io.store_fails = (0..5).map(|index| format!("t{index:02}")).collect();
        let report = fetch_in_batches(targets(20), 1, unlimited(), &io);
        assert_eq!(report.stopped, Some(StopReason::Failures));
        assert_eq!((report.failed, report.refreshed), (5, 0));
        assert_eq!(io.stored().len(), 5);
    }

    #[test]
    fn when_the_fifth_failure_lands_mid_batch_the_rest_of_that_batch_is_still_stored() {
        // 一批 6 个：前 5 个失败、第 6 个抓到了。5 个连着失败：停；但第 6 个已经抓回来，照常落库，不再开下一批
        let io = failing(&["t00", "t01", "t02", "t03", "t04"]);
        let report = fetch_in_batches(targets(20), 6, unlimited(), &io);
        assert_eq!(report.stopped, Some(StopReason::Failures));
        assert_eq!(report.attempted.len(), 6);
        assert_eq!((report.failed, report.refreshed), (5, 1));
        assert_eq!(io.stored(), vec!["t05".to_owned()]);
        assert_eq!(io.fetched().len(), 6, "没有第二批");
    }

    #[test]
    fn a_fetch_thread_that_panics_is_one_failure_and_the_others_carry_on() {
        let io = FakeIo::new().replying("t01", Reply::Panic);
        let report = fetch_in_batches(targets(6), 3, unlimited(), &io);
        assert_eq!(report.stopped, None);
        assert_eq!((report.refreshed, report.failed), (5, 1));
        assert_eq!(report.attempted.len(), 6);
        assert!(!io.stored().contains(&"t01".to_owned()));
    }

    #[test]
    fn a_batch_size_of_zero_still_makes_progress() {
        let io = FakeIo::new();
        let report = fetch_in_batches(targets(3), 0, unlimited(), &io);
        assert_eq!(report.refreshed, 3);
    }

    #[test]
    fn a_zero_failure_limit_stops_at_the_first_failure_and_never_on_success() {
        let limits = FetchLimits {
            time_budget: Duration::from_secs(60),
            stop_after_failures: 0,
        };
        let io = FakeIo::new();
        let report = fetch_in_batches(targets(4), 1, limits, &io);
        assert_eq!((report.stopped, report.refreshed), (None, 4), "成功不触发");
        let io = failing(&["t02"]);
        let report = fetch_in_batches(targets(6), 1, limits, &io);
        assert_eq!(report.stopped, Some(StopReason::Failures));
        assert_eq!(report.attempted.len(), 3);
    }
}
