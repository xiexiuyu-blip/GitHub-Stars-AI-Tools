//! 收集箱公开链接读取。只读 https 页面和官方 oEmbed，不登录、不带 cookie、不带令牌。
//!
//! 联网必须走 `PageFetcher`。测试注入假实现；真正的 `SafePageFetcher` 每一跳先检查链接，
//! 再把主机解析成 IP，拒绝内网后用 `curl --resolve` 把这次解析结果固定住。

use crate::capture::{find_github_repo_links, ip_is_blocked, url_is_safe};
use crate::web_fetch::url_encode;
use encoding_rs::{Encoding, UTF_8};
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::Read;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 公众号文章单篇就有 3.5 MB 上下（正文不大，内联的脚本和样式很大），所以上限放到 6 MB；
/// 任务卡原来写的 1 MB 会让每一篇公众号都读不了（2026-10-04 真网络演练发现）。
const MAX_BODY_BYTES: usize = 6_000_000;
/// 解压后的上限。只多读 1 个字节用来发现「超了」，不能先把整包解出来再比大小。
const MAX_DECODED_BYTES: usize = 16_000_000;
const META_CHARSET_BYTES: usize = 4096;
const GARBLED_SAMPLE_CHARS: usize = 2000;
/// 替换字符和控制字符占比要超过这个数才判乱码。刚好 20% 仍留下。
const GARBLED_BAD_PERCENT: usize = 20;
/// 可见正文达到这个字数就不再当拦截页，免得长文章里的「安全检测」被误伤。
const CHALLENGE_TEXT_LIMIT: usize = 600;
const MAX_REDIRECTS: usize = 3;
const EXCERPT_CHARS: usize = 2000;
const TITLE_CHARS: usize = 120;
const AUTHOR_CHARS: usize = 200;
const ERROR_CHARS: usize = 500;
const X_TITLE_CHARS: usize = 40;
/// 一篇文章里最多留下的仓库数。再多就丢掉后面的，保持第一次出现的顺序。
const MAX_PAGE_REPOS: usize = 30;
/// 与 `page_repos` 列的 CHECK 一致。超了就从末尾丢元素，不能让写库失败。
const PAGE_REPOS_MAX_CHARS: usize = 8000;

const UA_OEMBED: &str = "fox-stars-lab";
const UA_SAFARI: &str = concat!(
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ",
    "AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15"
);

const WECHAT_BLOCK_REASON: &str = "公众号拦住了自动读取，请在微信里打开这篇";
const EMPTY_PAGE_REASON: &str = "页面里没有可读的内容";
const BAD_ENCODING_REASON: &str = "网站返回了这里读不了的压缩格式";
const CORRUPT_BODY_REASON: &str = "页面压缩数据损坏";
const UNREADABLE_REASON: &str = "页面内容读不出来";
const CHALLENGE_REASON: &str = "网站要求先做人机验证，这里读不了";
const X_MISSING_REASON: &str = "帖子不存在、已删除或需要登录才能看";
const BLOCKED_IP_REASON: &str = "域名指向内网或云元数据地址，不读取";
const EXTRACT_FAILED_REASON: &str = "页面太复杂或有异常，没读出来";
/// 解析一个页面最多等这么久（正常的 6 MB 页面只要零点几秒）。
const EXTRACT_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchProblemKind {
    Skip,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchProblem {
    pub kind: FetchProblemKind,
    pub message: String,
}

impl FetchProblem {
    fn skip(message: impl Into<String>) -> Self {
        Self {
            kind: FetchProblemKind::Skip,
            message: message.into(),
        }
    }

    fn fail(message: impl Into<String>) -> Self {
        Self {
            kind: FetchProblemKind::Fail,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageResponse {
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub final_url: String,
    /// 3xx 的 Location。不跟随重定向的单次响应才有；最终页面为空。
    pub location: Option<String>,
}

pub trait PageFetcher {
    fn get(&self, url: &str) -> Result<PageResponse, FetchProblem>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchSummary {
    pub done: i64,
    pub failed: i64,
    pub skipped: i64,
    pub remaining: i64,
}

#[derive(Debug, Default)]
pub struct SafePageFetcher;

impl PageFetcher for SafePageFetcher {
    fn get(&self, url: &str) -> Result<PageResponse, FetchProblem> {
        fetch_guarded(url, &StdResolver, &|next, ip| curl_once_pinned(next, ip))
    }
}

trait HostResolver {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, String>;
}

struct StdResolver;

impl HostResolver for StdResolver {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<IpAddr>, String> {
        let host = host.trim_matches(|ch| ch == '[' || ch == ']');
        if host.is_empty() {
            return Err("域名解析失败".to_owned());
        }
        let mut ips = Vec::new();
        for addr in (host, port)
            .to_socket_addrs()
            .map_err(|_| "域名解析失败".to_owned())?
        {
            let ip = addr.ip();
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
        if ips.is_empty() {
            Err("域名解析失败".to_owned())
        } else {
            Ok(ips)
        }
    }
}

/// 读库、联网、写库分开。一次最多处理 `limit` 条，一条一条来，不并发。
pub fn fetch_pending(
    db_path: &Path,
    fetcher: &dyn PageFetcher,
    only_id: Option<&str>,
    limit: usize,
) -> Result<FetchSummary, String> {
    let only = only_id.unwrap_or("").trim();
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let rows = {
        let connection = open_read(db_path)?;
        load_pending(&connection, only, limit)?
    };
    let mut done = 0_i64;
    let mut failed = 0_i64;
    let mut skipped = 0_i64;
    for row in rows {
        // 解析出错（panic）也要写成「读失败」：不写的话这一条永远是 pending，每次打开收集页都读它、
        // 每次都出错，排在它后面的链接一个也读不到。
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decide(&row, fetcher)))
                .unwrap_or_else(|_| Outcome::Failed {
                    reason: EXTRACT_FAILED_REASON.to_owned(),
                });
        write_outcome(db_path, &row.id, &outcome)?;
        match outcome {
            Outcome::Done { .. } => done += 1,
            Outcome::Failed { .. } => failed += 1,
            Outcome::Skipped { .. } => skipped += 1,
        }
    }
    let remaining = {
        let connection = open_read(db_path)?;
        count_pending(&connection)?
    };
    Ok(FetchSummary {
        done,
        failed,
        skipped,
        remaining,
    })
}

struct PendingRow {
    id: String,
    original_url: String,
    normalized_url: String,
    item_type: String,
}

#[derive(Debug)]
enum Outcome {
    Done {
        title: String,
        author: String,
        excerpt: String,
        repos: Vec<String>,
    },
    Failed {
        reason: String,
    },
    Skipped {
        reason: String,
    },
}

enum Class {
    Stop(Outcome),
    X { user: String, id: String },
    Youtube,
    Page(String),
}

enum PageKind {
    X,
    Youtube,
    Web,
}

fn decide(row: &PendingRow, fetcher: &dyn PageFetcher) -> Outcome {
    if row.item_type == "github_repository" {
        return Outcome::Skipped {
            reason: "GitHub 仓库不读取网页".to_owned(),
        };
    }
    let url = preferred_url(row);
    match classify(&url) {
        Class::Stop(outcome) => outcome,
        Class::X { user, id } => fetch_x(fetcher, &user, &id),
        Class::Youtube => {
            let target = if youtube_page(&row.original_url) {
                row.original_url.clone()
            } else {
                url
            };
            fetch_youtube(fetcher, &target)
        }
        Class::Page(target) => fetch_html(fetcher, &target),
    }
}

fn preferred_url(row: &PendingRow) -> String {
    if url::Url::parse(&row.normalized_url).is_ok() {
        row.normalized_url.clone()
    } else {
        row.original_url.clone()
    }
}

fn classify(url: &str) -> Class {
    if let Err(problem) = gate_request(url) {
        return Class::Stop(outcome_from(problem));
    }
    let Ok(parsed) = url::Url::parse(url) else {
        return Class::Stop(Outcome::Failed {
            reason: "链接格式无效".to_owned(),
        });
    };
    let host = bare_host(&parsed);
    if is_x_host(&host) {
        return match x_status(&parsed) {
            Some((user, id)) => Class::X { user, id },
            None => Class::Stop(Outcome::Skipped {
                reason: "不是单条帖子".to_owned(),
            }),
        };
    }
    if is_youtube(&parsed) {
        return Class::Youtube;
    }
    Class::Page(url.to_owned())
}

fn fetch_x(fetcher: &dyn PageFetcher, user: &str, id: &str) -> Outcome {
    let status_url = match twitter_status_url(user, id) {
        Ok(url) => url,
        Err(reason) => return Outcome::Failed { reason },
    };
    let endpoint = format!(
        "https://publish.twitter.com/oembed?url={}&omit_script=1&dnt=1",
        url_encode(&status_url)
    );
    let page = match request_page(fetcher, &endpoint) {
        Ok(page) => page,
        Err(outcome) => return outcome,
    };
    if let Some(reason) = failure_for_status(page.status, PageKind::X) {
        return Outcome::Failed { reason };
    }
    read_x_body(&page.body)
}

fn fetch_youtube(fetcher: &dyn PageFetcher, url: &str) -> Outcome {
    let endpoint = format!(
        "https://www.youtube.com/oembed?url={}&format=json",
        url_encode(url)
    );
    let page = match request_page(fetcher, &endpoint) {
        Ok(page) => page,
        Err(outcome) => return outcome,
    };
    if let Some(reason) = failure_for_status(page.status, PageKind::Youtube) {
        return Outcome::Failed { reason };
    }
    read_youtube_body(&page.body)
}

fn fetch_html(fetcher: &dyn PageFetcher, url: &str) -> Outcome {
    let page = match request_page(fetcher, url) {
        Ok(page) => page,
        Err(outcome) => return outcome,
    };
    if let Some(reason) = failure_for_status(page.status, PageKind::Web) {
        return Outcome::Failed { reason };
    }
    // 解析页面是纯计算，放进单独的线程并限时：万一遇到没想到的恶意结构，最多等这么久就放弃这一条，
    // 不让整个读取一直卡在它上面（线程会自己跑完，不再占着调用方）。
    let body = page.body;
    let page_url = url.to_owned();
    let parsed = run_bounded(Duration::from_secs(EXTRACT_TIMEOUT_SECS), move || {
        let extracted = extract_web(&body, &page_url);
        let repos = if extracted.failure.is_none() {
            github_repos_in(&body, &page_url)
        } else {
            Vec::new()
        };
        (extracted, repos)
    });
    let Some((extracted, repos)) = parsed else {
        return Outcome::Failed {
            reason: EXTRACT_FAILED_REASON.to_owned(),
        };
    };
    if let Some(reason) = extracted.failure {
        return Outcome::Failed {
            reason: reason.to_owned(),
        };
    }
    Outcome::Done {
        title: extracted.title,
        author: extracted.author,
        excerpt: extracted.excerpt,
        repos,
    }
}

/// 在新线程里跑，最多等 `limit`。线程 panic 或超时都返回 `None`（panic 时发送端被丢掉，接收端立刻知道）。
fn run_bounded<T: Send + 'static>(
    limit: Duration,
    job: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("inbox-extract".to_owned())
        .spawn(move || {
            let _ = sender.send(job());
        })
        .ok()?;
    receiver.recv_timeout(limit).ok()
}

fn request_page(fetcher: &dyn PageFetcher, url: &str) -> Result<PageResponse, Outcome> {
    match fetcher.get(url) {
        Ok(page) => match accept_body(page) {
            Ok(page) => Ok(page),
            Err(problem) => Err(outcome_from(problem)),
        },
        Err(problem) => Err(outcome_from(problem)),
    }
}

fn outcome_from(problem: FetchProblem) -> Outcome {
    match problem.kind {
        FetchProblemKind::Skip => Outcome::Skipped {
            reason: problem.message,
        },
        FetchProblemKind::Fail => Outcome::Failed {
            reason: problem.message,
        },
    }
}

fn failure_for_status(status: u16, kind: PageKind) -> Option<String> {
    if (200..300).contains(&status) {
        return None;
    }
    let reason = match (kind, status) {
        (PageKind::X, 403 | 404 | 500..=599) => X_MISSING_REASON,
        (PageKind::Youtube, 403 | 404 | 500..=599) => "视频不存在、已删除或暂时打不开",
        (_, 404) => "页面不存在",
        (_, 401 | 403) => "页面拒绝访问",
        (_, 500..=599) => "页面暂时打不开",
        _ => return Some(format!("页面返回了异常状态 {status}")),
    };
    Some(reason.to_owned())
}

/// 每一跳：先做字面量检查，再解析 IP。任何一个地址落在内网就拒绝，通过后把该 IP 交给传输层固定。
fn fetch_guarded(
    start: &str,
    resolver: &dyn HostResolver,
    transport: &dyn Fn(&str, IpAddr) -> Result<PageResponse, FetchProblem>,
) -> Result<PageResponse, FetchProblem> {
    let mut current = start.to_owned();
    for hop in 0..=MAX_REDIRECTS {
        let parsed = gate_request(&current)?;
        let host = parsed
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| FetchProblem::fail("链接没有主机名"))?;
        let ips = resolver.resolve(host, 443).map_err(FetchProblem::fail)?;
        let ip = checked_ip(&ips)?;
        let response = transport(&current, ip)?;
        if is_redirect(response.status) {
            if hop == MAX_REDIRECTS {
                return Err(FetchProblem::fail("重定向超过 3 跳"));
            }
            let location = response
                .location
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| FetchProblem::fail("重定向没有目标地址"))?;
            current = resolve_location(&current, location)?;
            continue;
        }
        let mut response = response;
        response.final_url = current;
        return accept_body(response);
    }
    Err(FetchProblem::fail("重定向超过 3 跳"))
}

fn gate_request(url: &str) -> Result<url::Url, FetchProblem> {
    let parsed = url::Url::parse(url).map_err(|_| FetchProblem::fail("链接格式无效"))?;
    if parsed.scheme() == "http" {
        return Err(FetchProblem::skip("只读取 https 链接"));
    }
    if parsed.scheme() != "https" {
        return Err(FetchProblem::fail("只允许 http 或 https"));
    }
    match parsed.port_or_known_default() {
        Some(443) => {}
        _ => return Err(FetchProblem::skip("不读取非常规端口")),
    }
    if let Err(reason) = url_is_safe(url) {
        return Err(FetchProblem::fail(reason));
    }
    if parsed.host_str().unwrap_or("").is_empty() {
        return Err(FetchProblem::fail("链接没有主机名"));
    }
    Ok(parsed)
}

fn checked_ip(ips: &[IpAddr]) -> Result<IpAddr, FetchProblem> {
    if ips.is_empty() {
        return Err(FetchProblem::fail("域名解析失败"));
    }
    if ips.iter().copied().any(ip_is_blocked) {
        return Err(FetchProblem::fail(BLOCKED_IP_REASON));
    }
    // 多数家用网络没有 IPv6 出口：有 IPv4 就先用它，免得系统把 AAAA 排在前面时连不上。
    ips.iter()
        .copied()
        .find(IpAddr::is_ipv4)
        .or_else(|| ips.first().copied())
        .ok_or_else(|| FetchProblem::fail("域名解析失败"))
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn resolve_location(current: &str, location: &str) -> Result<String, FetchProblem> {
    let base = url::Url::parse(current).map_err(|_| FetchProblem::fail("链接格式无效"))?;
    base.join(location.trim())
        .map(|next| next.to_string())
        .map_err(|_| FetchProblem::fail("重定向地址无效"))
}

fn accept_body(response: PageResponse) -> Result<PageResponse, FetchProblem> {
    if response.body.len() > MAX_BODY_BYTES {
        return Err(too_big_problem());
    }
    if (200..300).contains(&response.status) && !allowed_type(&response.content_type) {
        return Err(FetchProblem::skip("不是网页"));
    }
    Ok(response)
}

fn allowed_type(content_type: &str) -> bool {
    let media = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    matches!(
        media.as_str(),
        "text/html" | "application/xhtml+xml" | "application/json" | "text/plain"
    )
}

fn user_agent_for(url: &str) -> &'static str {
    let Ok(parsed) = url::Url::parse(url) else {
        return UA_SAFARI;
    };
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    let path = parsed.path().to_ascii_lowercase();
    if path.starts_with("/oembed")
        && matches!(
            host.as_str(),
            "publish.twitter.com" | "www.youtube.com" | "youtube.com"
        )
    {
        UA_OEMBED
    } else {
        UA_SAFARI
    }
}

/// 拼 curl 参数。不带 `-L`、cookie 或 `Authorization`。只带 `Accept-Encoding` 这一个请求头。
/// `-g`（关掉网址通配）：curl 默认把 `[1-100]`、`{a,b}` 当成要展开的范围，
/// 一条 `https://x.example/a[1-100000]` 的链接会发出十万次请求，主机名里的 `{a,b}` 还会绕过固定 IP 的检查。
/// `--noproxy *`：环境里有 `https_proxy` 时 curl 会把 `CONNECT 主机:443` 交给代理，`--resolve` 的固定 IP 就失效了。
fn safe_curl_arguments(
    url: &str,
    header_dump: &str,
    resolve: &str,
    user_agent: &str,
) -> Vec<String> {
    let max_filesize = MAX_BODY_BYTES.to_string();
    [
        "-q",
        "-sS",
        "-g",
        "--noproxy",
        "*",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--max-time",
        "10",
        "--connect-timeout",
        "6",
        "--max-filesize",
        max_filesize.as_str(),
        "-A",
        user_agent,
        "-H",
        "Accept-Encoding: gzip, deflate",
        "-D",
        header_dump,
        "--resolve",
        resolve,
        "--",
        url,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn resolve_arg(host: &str, port: u16, ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(address) => format!("{host}:{port}:{address}"),
        IpAddr::V6(address) => format!("{host}:{port}:[{address}]"),
    }
}

fn curl_once_pinned(url: &str, ip: IpAddr) -> Result<PageResponse, FetchProblem> {
    let parsed = gate_request(url)?;
    if ip_is_blocked(ip) {
        return Err(FetchProblem::fail(BLOCKED_IP_REASON));
    }
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or_else(|| FetchProblem::fail("链接没有主机名"))?;
    let pin = resolve_arg(host, 443, ip);
    let dump = HeaderDump::create()?;
    let header_path = dump.path_arg()?;
    let args = safe_curl_arguments(parsed.as_str(), &header_path, &pin, user_agent_for(url));
    let output = std::process::Command::new(curl_program())
        .args(&args)
        .output()
        .map_err(|_| FetchProblem::fail("联网程序启动失败"))?;
    let code = output
        .status
        .code()
        .ok_or_else(|| FetchProblem::fail("联网失败"))?;
    if code != 0 {
        return Err(curl_exit_problem(
            code,
            &String::from_utf8_lossy(&output.stderr),
        ));
    }
    let header_text =
        std::fs::read_to_string(&dump.0).map_err(|_| FetchProblem::fail("读不到响应头"))?;
    let (status, headers) = parse_header_dump(&header_text)?;
    let content_type = header_value(&headers, "content-type")
        .unwrap_or("")
        .to_owned();
    let location = header_value(&headers, "location").map(str::to_owned);
    Ok(PageResponse {
        status,
        content_type,
        body: decode_page(&output.stdout, &headers)?,
        final_url: url.to_owned(),
        location,
    })
}

/// 系统自带的 curl，不按 PATH 找：PATH 里可能有别的同名程序。
fn curl_program() -> &'static str {
    if Path::new("/usr/bin/curl").exists() {
        "/usr/bin/curl"
    } else {
        "curl"
    }
}

fn too_big_problem() -> FetchProblem {
    FetchProblem::skip(format!(
        "页面太大（超过 {} MB），没有读取",
        MAX_BODY_BYTES / 1_000_000
    ))
}

/// curl 的退出码。响应没有 Content-Length 时，超过 `--max-filesize` 是 56 加一句英文提示，不是 63，
/// 要看 stderr 才分得出「页面太大」和「网络真的断了」。
fn curl_exit_problem(code: i32, stderr: &str) -> FetchProblem {
    match code {
        28 => FetchProblem::fail("联网超时"),
        63 => too_big_problem(),
        56 if stderr.contains("maximum allowed file size") => too_big_problem(),
        5 | 6 | 7 | 35 | 52 | 55 | 56 => FetchProblem::fail("网络连不上"),
        _ => FetchProblem::fail("联网失败"),
    }
}

enum BodyCoding {
    Plain,
    Gzip,
    Deflate,
    Unsupported,
}

enum InflateFault {
    TooBig,
    Corrupt { produced: usize },
}

/// 先按响应头解压，再按字符集解码。解压失败或正文明显是乱码时返回失败，不把乱码当读成功。
fn decode_page(bytes: &[u8], headers: &[(String, String)]) -> Result<String, FetchProblem> {
    let plain = inflate_body(bytes, headers)?;
    let text = decode_charset(&plain, headers);
    if text_is_garbled(&text) {
        return Err(FetchProblem::fail(UNREADABLE_REASON));
    }
    Ok(text)
}

fn inflate_body(bytes: &[u8], headers: &[(String, String)]) -> Result<Vec<u8>, FetchProblem> {
    match body_coding(bytes, headers) {
        BodyCoding::Plain => Ok(bytes.to_vec()),
        BodyCoding::Gzip => limited_bytes(GzDecoder::new(bytes)),
        BodyCoding::Deflate => inflate_deflate(bytes),
        BodyCoding::Unsupported => Err(FetchProblem::fail(BAD_ENCODING_REASON)),
    }
}

/// 没有 `Content-Encoding` 时，开头是 gzip 魔数的也按 gzip 解（有的服务器忘了写头）。
/// 头在但值有多个（逗号隔开，或重复头）一律不支持，避免只解一半。
fn body_coding(bytes: &[u8], headers: &[(String, String)]) -> BodyCoding {
    let mut present = false;
    let mut codings = Vec::new();
    for (name, value) in headers {
        if !name.eq_ignore_ascii_case("content-encoding") {
            continue;
        }
        present = true;
        for part in value.split(',') {
            let part = part.trim();
            if !part.is_empty() {
                codings.push(part.to_ascii_lowercase());
            }
        }
    }
    if !present {
        return if looks_like_gzip(bytes) {
            BodyCoding::Gzip
        } else {
            BodyCoding::Plain
        };
    }
    match codings.as_slice() {
        [] => BodyCoding::Plain,
        [one] if one == "identity" => BodyCoding::Plain,
        [one] if one == "gzip" || one == "x-gzip" => BodyCoding::Gzip,
        [one] if one == "deflate" => BodyCoding::Deflate,
        _ => BodyCoding::Unsupported,
    }
}

fn looks_like_gzip(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

fn inflate_deflate(bytes: &[u8]) -> Result<Vec<u8>, FetchProblem> {
    match read_limited(ZlibDecoder::new(bytes)) {
        Ok(data) => Ok(data),
        Err(InflateFault::TooBig) => Err(too_big_problem()),
        Err(InflateFault::Corrupt { produced }) if produced == 0 => {
            limited_bytes(DeflateDecoder::new(bytes))
        }
        Err(InflateFault::Corrupt { .. }) => Err(FetchProblem::fail(CORRUPT_BODY_REASON)),
    }
}

fn limited_bytes<R: Read>(reader: R) -> Result<Vec<u8>, FetchProblem> {
    match read_limited(reader) {
        Ok(data) => Ok(data),
        Err(InflateFault::TooBig) => Err(too_big_problem()),
        Err(InflateFault::Corrupt { .. }) => Err(FetchProblem::fail(CORRUPT_BODY_REASON)),
    }
}

/// 最多读 `MAX_DECODED_BYTES + 1`。多出来的那一个字节只说明超限，解码器不会继续把炸弹解完。
fn read_limited<R: Read>(reader: R) -> Result<Vec<u8>, InflateFault> {
    let mut limited = reader.take((MAX_DECODED_BYTES as u64).saturating_add(1));
    let mut output = Vec::new();
    match limited.read_to_end(&mut output) {
        Ok(_) if output.len() > MAX_DECODED_BYTES => Err(InflateFault::TooBig),
        Ok(_) => Ok(output),
        Err(_) => Err(InflateFault::Corrupt {
            produced: output.len(),
        }),
    }
}

fn decode_charset(bytes: &[u8], headers: &[(String, String)]) -> String {
    text_encoding(bytes, headers).decode(bytes).0.into_owned()
}

/// BOM 优先于响应头，响应头优先于 `<meta>`，都没有或不认识就用 UTF-8。非法字节换成 U+FFFD。
fn text_encoding(bytes: &[u8], headers: &[(String, String)]) -> &'static Encoding {
    if let Some((encoding, _)) = Encoding::for_bom(bytes) {
        return encoding;
    }
    if let Some(label) = content_type_charset(headers) {
        return Encoding::for_label(label.as_bytes()).unwrap_or(UTF_8);
    }
    if let Some(label) = meta_charset(bytes) {
        return Encoding::for_label(label.as_bytes()).unwrap_or(UTF_8);
    }
    UTF_8
}

fn content_type_charset(headers: &[(String, String)]) -> Option<String> {
    charset_label_in(header_last_ci(headers, "content-type")?)
}

fn header_last_ci<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .rev()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn charset_label_in(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut index = 0;
    while index + "charset".len() <= bytes.len() {
        if bytes[index..].starts_with(b"charset") {
            let boundary_ok = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
            if boundary_ok {
                let mut cursor = index + "charset".len();
                while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                if cursor < bytes.len() && bytes[cursor] == b'=' {
                    return take_charset_label(&lower[cursor + 1..]);
                }
            }
        }
        index += 1;
    }
    None
}

fn take_charset_label(rest: &str) -> Option<String> {
    let rest = rest.trim_start();
    let label = if let Some(inner) = rest.strip_prefix('"') {
        inner.split('"').next().unwrap_or("")
    } else if let Some(inner) = rest.strip_prefix('\'') {
        inner.split('\'').next().unwrap_or("")
    } else {
        rest.split(|ch: char| ch == ';' || ch.is_ascii_whitespace())
            .next()
            .unwrap_or("")
    };
    let label = label.trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_owned())
    }
}

/// `<meta charset>` 写在字节里，而且页面可能还不是 UTF-8，所以只扫前 4096 字节、按 ASCII 忽略大小写。
fn meta_charset(bytes: &[u8]) -> Option<String> {
    let end = bytes.len().min(META_CHARSET_BYTES);
    let lower: Vec<u8> = bytes[..end]
        .iter()
        .copied()
        .map(|byte| byte.to_ascii_lowercase())
        .collect();
    let mut from = 0;
    while from < lower.len() {
        let Some(relative) = find_bytes(&lower[from..], b"<meta") else {
            return None;
        };
        let start = from + relative;
        let after_name = start + 5;
        if after_name < lower.len() {
            let next = lower[after_name];
            if !next.is_ascii_whitespace() && next != b'/' && next != b'>' {
                from = after_name;
                continue;
            }
        }
        let Some(gt) = lower[after_name..].iter().position(|byte| *byte == b'>') else {
            return None;
        };
        if let Some(label) = charset_from_meta_bytes(&lower[after_name..after_name + gt]) {
            return Some(label);
        }
        from = after_name + gt + 1;
    }
    None
}

fn charset_from_meta_bytes(tag_body: &[u8]) -> Option<String> {
    let attrs = byte_attrs(tag_body);
    if let Some((_, value)) = attrs.iter().find(|(name, _)| name == "charset") {
        let label = value.trim();
        if !label.is_empty() {
            return Some(label.to_owned());
        }
    }
    let content_type = attrs.iter().any(|(name, value)| {
        name == "http-equiv" && value.trim().eq_ignore_ascii_case("content-type")
    });
    if !content_type {
        return None;
    }
    let (_, content) = attrs.iter().find(|(name, _)| name == "content")?;
    charset_label_in(content)
}

fn byte_attrs(source: &[u8]) -> Vec<(String, String)> {
    let mut index = 0;
    let mut attrs = Vec::new();
    while index < source.len() {
        while index < source.len() && source[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= source.len() || source[index] == b'/' {
            break;
        }
        let name_start = index;
        while index < source.len() && (is_name_byte(source[index]) || source[index] == b'_') {
            index += 1;
        }
        if index == name_start {
            break;
        }
        let name = String::from_utf8_lossy(&source[name_start..index]).into_owned();
        while index < source.len() && source[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= source.len() || source[index] != b'=' {
            attrs.push((name, String::new()));
            continue;
        }
        index += 1;
        while index < source.len() && source[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= source.len() {
            break;
        }
        let value = if source[index] == b'"' || source[index] == b'\'' {
            let quote = source[index];
            index += 1;
            let value_start = index;
            while index < source.len() && source[index] != quote {
                index += 1;
            }
            let value = String::from_utf8_lossy(&source[value_start..index]).into_owned();
            if index < source.len() {
                index += 1;
            }
            value
        } else {
            let value_start = index;
            while index < source.len()
                && !source[index].is_ascii_whitespace()
                && source[index] != b'>'
                && source[index] != b'/'
            {
                index += 1;
            }
            String::from_utf8_lossy(&source[value_start..index]).into_owned()
        };
        attrs.push((name, value));
    }
    attrs
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn text_is_garbled(text: &str) -> bool {
    let mut total = 0usize;
    let mut bad = 0usize;
    for ch in text.chars().take(GARBLED_SAMPLE_CHARS) {
        total += 1;
        if ch == '\u{FFFD}' || is_disallowed_control(ch) {
            bad += 1;
        }
    }
    if total == 0 {
        return false;
    }
    bad.saturating_mul(100) > total.saturating_mul(GARBLED_BAD_PERCENT)
}

fn is_disallowed_control(ch: char) -> bool {
    ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t'
}

struct HeaderDump(PathBuf);

impl HeaderDump {
    fn create() -> Result<Self, FetchProblem> {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        Ok(Self(
            std::env::temp_dir().join(format!("fsl-inbox-headers-{nanos}-{seq}.txt")),
        ))
    }

    fn path_arg(&self) -> Result<String, FetchProblem> {
        self.0
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| FetchProblem::fail("临时文件路径无效"))
    }
}

impl Drop for HeaderDump {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn parse_header_dump(dump: &str) -> Result<(u16, Vec<(String, String)>), FetchProblem> {
    let normalized = dump.replace("\r\n", "\n");
    let block = normalized
        .split("\n\n")
        .map(str::trim)
        .filter(|block| block.starts_with("HTTP/"))
        .last()
        .ok_or_else(|| FetchProblem::fail("读不到响应状态"))?;
    let mut lines = block.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| FetchProblem::fail("读不到响应状态"))?;
    let status = parse_status_line(status_line)?;
    let mut headers = Vec::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        headers.push((name, value.trim().to_owned()));
    }
    Ok((status, headers))
}

fn parse_status_line(line: &str) -> Result<u16, FetchProblem> {
    let mut parts = line.split_whitespace();
    let version = parts.next().unwrap_or("");
    if !version.starts_with("HTTP/") {
        return Err(FetchProblem::fail("读不到响应状态"));
    }
    parts
        .next()
        .and_then(|code| code.parse().ok())
        .filter(|code: &u16| (100..600).contains(code))
        .ok_or_else(|| FetchProblem::fail("读不到响应状态"))
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .rev()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn read_x_body(body: &str) -> Outcome {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return Outcome::Failed {
                reason: "帖子内容读不出来".to_owned(),
            }
        }
    };
    let author = plain_text(
        value
            .get("author_name")
            .and_then(Value::as_str)
            .unwrap_or(""),
        AUTHOR_CHARS,
    );
    let html = value.get("html").and_then(Value::as_str).unwrap_or("");
    let Some(text) = first_paragraph_text(html) else {
        return Outcome::Failed {
            reason: "帖子里没有正文".to_owned(),
        };
    };
    let excerpt = truncate_chars(&text, EXCERPT_CHARS);
    let title = x_display_title(&author, &text);
    Outcome::Done {
        title,
        author,
        excerpt,
        repos: Vec::new(),
    }
}

fn read_youtube_body(body: &str) -> Outcome {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return Outcome::Failed {
                reason: "视频信息读不出来".to_owned(),
            }
        }
    };
    let title = plain_text(
        value.get("title").and_then(Value::as_str).unwrap_or(""),
        300,
    );
    if title.is_empty() {
        return Outcome::Failed {
            reason: "视频没有标题".to_owned(),
        };
    }
    let author = plain_text(
        value
            .get("author_name")
            .and_then(Value::as_str)
            .unwrap_or(""),
        AUTHOR_CHARS,
    );
    Outcome::Done {
        title,
        author,
        excerpt: String::new(),
        repos: Vec::new(),
    }
}

struct WebPage {
    title: String,
    author: String,
    excerpt: String,
    failure: Option<&'static str>,
}

fn extract_web(raw: &str, page_url: &str) -> WebPage {
    let cleaned = remove_hidden(raw);
    let metas = collect_metas(&cleaned);
    let title = pick_title(&cleaned, &metas);
    let summary = pick_summary(&metas);
    let body = pick_body(&cleaned);
    let excerpt = compose_excerpt(&summary, &body);
    let meta_author = meta_content(&metas, "author")
        .map(|value| plain_text(&value, AUTHOR_CHARS))
        .unwrap_or_default();
    let site = meta_content(&metas, "og:site_name")
        .map(|value| plain_text(&value, AUTHOR_CHARS))
        .unwrap_or_default();
    let nick = if is_wechat_url(page_url) {
        wechat_author(raw, &cleaned)
    } else {
        String::new()
    };
    // 公众号：账号名优先（meta author 常是「点击关注→」这类引导语）；别的网站：meta author 优先
    let author = if !nick.is_empty() {
        nick
    } else if !meta_author.is_empty() {
        meta_author
    } else {
        site
    };
    if is_wechat_url(page_url) && wechat_blocked(&title, &excerpt, &body) {
        return WebPage {
            title,
            author,
            excerpt,
            failure: Some(WECHAT_BLOCK_REASON),
        };
    }
    // 用可见正文 body，不用摘要：摘要会拼上 meta description，也会被截到 2000 字，600 字的门槛就失真。
    if let Some(reason) = challenge_reason(&title, &body) {
        return WebPage {
            title,
            author,
            excerpt,
            failure: Some(reason),
        };
    }
    if title.is_empty() && excerpt.is_empty() {
        return WebPage {
            title,
            author,
            excerpt,
            failure: Some(EMPTY_PAGE_REASON),
        };
    }
    WebPage {
        title,
        author,
        excerpt,
        failure: None,
    }
}

/// 从正文里找出 GitHub 仓库，输出 `"owner/name"`。
/// 解析不了就返回空数组，不报错。页面本身在 github.com 上时也不抽。
fn github_repos_in(raw_html: &str, page_url: &str) -> Vec<String> {
    if page_host_is_github(page_url) {
        return Vec::new();
    }
    let cleaned = remove_hidden(raw_html);
    let region = content_html_for_repos(&cleaned, page_url);
    repos_from_text(&repo_scan_text(&region))
}

fn page_host_is_github(page_url: &str) -> bool {
    match url::Url::parse(page_url) {
        Ok(parsed) => bare_host(&parsed) == "github.com",
        Err(_) => false,
    }
}

/// 公众号只看 `js_content`。别的页面依次用 article、main，再兜底到 body、整页。
/// 兜底时先去掉导航、页眉、页脚、侧栏：站点自己的 GitHub 链接（主题、模板仓库）多在这些地方，
/// 不是文章在讲的项目。找到 article / main / `js_content` 就整块用，里面自带的页眉页脚不去。
fn content_html_for_repos<'a>(cleaned: &'a str, page_url: &str) -> Cow<'a, str> {
    let wechat = is_wechat_url(page_url);
    let focused = if wechat {
        element_by_id(cleaned, "js_content")
    } else {
        element_inner(cleaned, "article").or_else(|| element_inner(cleaned, "main"))
    };
    match focused {
        Some(inner) => Cow::Borrowed(inner),
        None => {
            let fallback = if wechat {
                cleaned
            } else {
                element_inner(cleaned, "body").unwrap_or(cleaned)
            };
            Cow::Owned(without_page_chrome(fallback))
        }
    }
}

/// 去掉 nav / header / footer / aside 整块。没有配对的结束标签就不去（免得一个没关的标签吞掉整页）。
fn without_page_chrome(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index] != b'<' {
                index += 1;
            }
            out.push_str(&html[start..index]);
            continue;
        }
        match scan_tag_at(html, index) {
            TagScan::Tag(tag) => {
                if !tag.closing
                    && !tag.self_closing
                    && matches!(tag.name.as_str(), "nav" | "header" | "footer" | "aside")
                {
                    let end = skip_element(html, tag.end, &tag.name);
                    if ends_with_close_tag(html, end, &tag.name) {
                        index = end;
                        continue;
                    }
                    // 第一个没有配对结束标签的就到此为止：skip_element 已经扫到了末尾，
                    // 后面每个没关的 <nav> 再扫一遍就是二次方（2 万个 <nav> 要 7 秒）。剩下的原样带上。
                    out.push_str(&html[index..]);
                    break;
                }
                out.push_str(&html[index..tag.end]);
                index = tag.end;
                continue;
            }
            TagScan::NotATag => {}
            TagScan::Unterminated => {
                out.push_str(&html[index..]);
                break;
            }
        }
        out.push('<');
        index += 1;
    }
    out
}

/// `skip_element` 找不到配对的结束标签时会一路吃到末尾；用这个分辨「真的关上了」和「吃到头了」。
fn ends_with_close_tag(html: &str, end: usize, name: &str) -> bool {
    let needle = format!("</{name}").into_bytes();
    let tail_start = end.saturating_sub(needle.len() + 16);
    html.as_bytes()[tail_start..end]
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(&needle))
}

/// 按文档顺序拼出 href（先留着实体）和可见文字，最后统一做一次实体解码。
fn repo_scan_text(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut raw = String::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index] != b'<' {
                index += 1;
            }
            raw.push_str(&html[start..index]);
            continue;
        }
        match scan_tag_at(html, index) {
            TagScan::Tag(tag) => {
                if let Some(href) = attr(&tag, "href") {
                    if !href.is_empty() {
                        raw.push('\n');
                        raw.push_str(href);
                        raw.push('\n');
                    }
                }
                // 行内标签（span、strong……）不打断文字：编辑器会把一个网址拆进几个 span。
                // 其余标签（段落、列表、表格、br、不认识的标签）都当换行，
                // 否则相邻两段的网址会粘成一个，两个都认不出来。
                if !is_inline_tag(&tag.name) {
                    raw.push('\n');
                }
                index = tag.end;
                continue;
            }
            TagScan::NotATag => {}
            TagScan::Unterminated => {
                raw.push_str(&html[index..]);
                break;
            }
        }
        raw.push('<');
        index += 1;
    }
    decode_entities(&raw)
}

fn is_inline_tag(name: &str) -> bool {
    matches!(
        name,
        "a" | "abbr"
            | "b"
            | "bdi"
            | "bdo"
            | "big"
            | "cite"
            | "code"
            | "data"
            | "del"
            | "dfn"
            | "em"
            | "font"
            | "i"
            | "ins"
            | "kbd"
            | "label"
            | "mark"
            | "q"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strike"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "tt"
            | "u"
            | "var"
            | "wbr"
    )
}

fn repos_from_text(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    // 文章里的链接数不设上限、找到 MAX_PAGE_REPOS 个仓库就停（见 `find_github_repo_links`）。
    find_github_repo_links(text, MAX_PAGE_REPOS, |link| {
        let Some(owner) = link.owner.as_deref().filter(|value| !value.is_empty()) else {
            return false;
        };
        let Some(name) = link.name.as_deref().filter(|value| !value.is_empty()) else {
            return false;
        };
        if !is_github_owner(owner) || !is_github_repo_name(name) || is_hex_32(name) {
            return false;
        }
        let key = format!(
            "{}/{}",
            owner.to_ascii_lowercase(),
            name.to_ascii_lowercase()
        );
        if !seen.insert(key) {
            return false;
        }
        found.push(format!("{owner}/{name}"));
        true
    });
    found
}

/// GitHub 用户名只有字母、数字和连字符。仓库名另外允许 `.` 和 `_`。
fn is_github_owner(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn is_github_repo_name(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

/// 公众号页面里有 `repository/<32 位哈希>` 这种内部链接，不是仓库。
fn is_hex_32(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn pick_title(cleaned: &str, metas: &[(String, String)]) -> String {
    for key in ["og:title", "twitter:title"] {
        if let Some(value) = meta_content(metas, key) {
            let text = plain_text(&value, TITLE_CHARS);
            if !text.is_empty() {
                return text;
            }
        }
    }
    element_inner(cleaned, "title")
        .map(|inner| truncate_chars(&html_fragment_to_text(inner, false), TITLE_CHARS))
        .unwrap_or_default()
}

fn pick_summary(metas: &[(String, String)]) -> String {
    for key in ["og:description", "description"] {
        if let Some(value) = meta_content(metas, key) {
            let text = plain_text(&value, EXCERPT_CHARS);
            if !text.is_empty() {
                return text;
            }
        }
    }
    String::new()
}

fn pick_body(cleaned: &str) -> String {
    let inner = element_inner(cleaned, "article")
        .or_else(|| element_by_id(cleaned, "js_content"))
        .or_else(|| element_inner(cleaned, "main"))
        .or_else(|| element_inner(cleaned, "body"));
    match inner {
        Some(inner) => html_fragment_to_text(inner, true),
        None => html_fragment_to_text(cleaned, true),
    }
}

fn compose_excerpt(summary: &str, body: &str) -> String {
    let text = match (summary.is_empty(), body.is_empty()) {
        (true, true) => String::new(),
        (false, true) => summary.to_owned(),
        (true, false) => body.to_owned(),
        (false, false) => format!("{summary}\n{body}"),
    };
    truncate_chars(&text, EXCERPT_CHARS)
}

/// 正文短，并且标题或正文里出现人机验证话术，就当成被拦住。长文里偶然出现这些词不算。
fn challenge_reason(title: &str, text: &str) -> Option<&'static str> {
    if text.chars().count() >= CHALLENGE_TEXT_LIMIT {
        return None;
    }
    const PHRASES: [&str; 16] = [
        "正在进行安全检测",
        "安全检测",
        "人机验证",
        "请完成验证",
        "滑动验证",
        "访问验证",
        "检测到异常流量",
        "请开启 javascript",
        "请启用 javascript",
        "just a moment",
        "checking your browser",
        "verify you are human",
        "attention required",
        "enable javascript and cookies",
        "are you a robot",
        "captcha",
    ];
    let title = title.to_ascii_lowercase();
    let text = text.to_ascii_lowercase();
    if PHRASES
        .iter()
        .any(|phrase| title.contains(phrase) || text.contains(phrase))
    {
        Some(CHALLENGE_REASON)
    } else {
        None
    }
}

fn wechat_blocked(title: &str, excerpt: &str, body: &str) -> bool {
    const PHRASES: [&str; 3] = ["环境异常", "完成验证后即可继续访问", "请在微信客户端打开"];
    if PHRASES
        .iter()
        .any(|phrase| title.contains(phrase) || excerpt.contains(phrase) || body.contains(phrase))
    {
        return true;
    }
    // 公众号的文章页一定有标题（og:title）。没有标题的多半是错误页或拦截页，哪怕里面有几句按钮文字
    // （真网络演练里有一页只读出了「视频 小程序 赞 在看」）
    title.is_empty()
}

/// 公众号名字。真实页面里：`var nickname = htmlDecode("名字")`、`nick_name: '名字'` 都是它；
/// 页面里还有 `nickname: "data-miniprogram-nickname"`（一张属性名对照表）和后面的 `nick_name: '微信广告'`，
/// 所以每种写法只认固定的赋值符号，并且忽略「看起来是属性名」的值。
fn wechat_author(raw: &str, cleaned: &str) -> String {
    // 只在 <script> 里找：正文是谁都能写的，往文章里写一句 `nickname = '官方账号'` 不能变成显示的作者名。
    let scripts = script_sections(raw);
    for (ident, separators) in [
        ("nickname", b"=".as_slice()),
        ("nick_name", b":".as_slice()),
        ("profile_nickname", b"=:".as_slice()),
    ] {
        for script in &scripts {
            if let Some(name) = js_assigned_string_with(script, ident, separators) {
                let text = plain_text(&name, AUTHOR_CHARS);
                if !text.is_empty() && !looks_like_attribute_name(&text) {
                    return text;
                }
            }
        }
    }
    if let Some(inner) = element_by_class_or_id(cleaned, "profile_nickname") {
        let text = truncate_chars(&html_fragment_to_text(inner, false), AUTHOR_CHARS);
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

/// 页面里所有 `<script>` 的内容（原样文本，不当 HTML 解析）。
fn script_sections(raw: &str) -> Vec<&str> {
    let mut sections = Vec::new();
    let mut index = 0;
    while let Some(tag) = next_tag(raw, index) {
        if !tag.closing && !tag.self_closing && tag.name == "script" {
            let end = skip_raw_text_element(raw, tag.end, "script");
            sections.push(&raw[tag.end..end]);
            index = end;
        } else {
            index = tag.end;
        }
    }
    sections
}

/// `data-miniprogram-nickname` 这种全小写加连字符、没有汉字的值是代码里的名字，不是账号名。
fn looks_like_attribute_name(text: &str) -> bool {
    text.contains('-')
        && text
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

fn x_display_title(author: &str, body: &str) -> String {
    let flat = collapse_ws(&strip_controls(body), false);
    let total = flat.chars().count();
    let mut prefix: String = flat.chars().take(X_TITLE_CHARS).collect();
    if total > X_TITLE_CHARS {
        prefix.push('…');
    }
    if author.is_empty() {
        format!("「{prefix}」")
    } else {
        format!("「{author}：{prefix}」")
    }
}

fn first_paragraph_text(html: &str) -> Option<String> {
    let text = html_fragment_to_text(element_inner(html, "p")?, true);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn twitter_status_url(user: &str, id: &str) -> Result<String, String> {
    if user.is_empty()
        || id.is_empty()
        || !id.chars().all(|ch| ch.is_ascii_digit())
        || user
            .chars()
            .any(|ch| matches!(ch, '/' | '?' | '#' | '\\' | ' ' | '@'))
    {
        return Err("帖子地址无效".to_owned());
    }
    let mut base =
        url::Url::parse("https://twitter.com/").map_err(|_| "帖子地址无效".to_owned())?;
    {
        let mut segments = base
            .path_segments_mut()
            .map_err(|_| "帖子地址无效".to_owned())?;
        segments.push(user).push("status").push(id);
    }
    Ok(base.to_string())
}

fn bare_host(parsed: &url::Url) -> String {
    let host = parsed
        .host_str()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_owned()
}

fn is_x_host(host: &str) -> bool {
    matches!(
        host,
        "x.com" | "twitter.com" | "mobile.twitter.com" | "mobile.x.com" | "m.twitter.com"
    )
}

fn x_status(parsed: &url::Url) -> Option<(String, String)> {
    let segments: Vec<String> = parsed
        .path_segments()
        .map(|parts| {
            parts
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if segments.len() >= 3
        && (segments[1] == "status" || segments[1] == "statuses")
        && !segments[2].is_empty()
        && segments[2].chars().all(|ch| ch.is_ascii_digit())
    {
        Some((segments[0].clone(), segments[2].clone()))
    } else {
        None
    }
}

fn is_youtube(parsed: &url::Url) -> bool {
    let host = bare_host(parsed);
    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|parts| parts.filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    match host.as_str() {
        "youtu.be" => segments.first().is_some_and(|id| !id.is_empty()),
        "youtube.com" | "m.youtube.com" => match segments.first().copied() {
            Some("shorts") => segments.get(1).is_some_and(|id| !id.is_empty()),
            Some("watch") => parsed
                .query_pairs()
                .any(|(key, value)| key == "v" && !value.is_empty()),
            _ => false,
        },
        _ => false,
    }
}

fn youtube_page(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .is_some_and(|parsed| parsed.scheme() == "https" && is_youtube(&parsed))
}

fn is_wechat_url(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .is_some_and(|parsed| bare_host(&parsed) == "mp.weixin.qq.com")
}

fn plain_text(text: &str, max_chars: usize) -> String {
    truncate_chars(
        &collapse_ws(&strip_controls(&decode_entities(text)), false),
        max_chars,
    )
}

fn decode_entities(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'&' {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index] != b'&' {
                index += 1;
            }
            out.push_str(&input[start..index]);
            continue;
        }
        if let Some((ch, next)) = parse_entity(input, index) {
            out.push(ch);
            index = next;
        } else {
            out.push('&');
            index += 1;
        }
    }
    out
}

fn parse_entity(input: &str, start: usize) -> Option<(char, usize)> {
    let rest = input.get(start + 1..)?;
    // 实体最长 32 个字节：只在这个窗口里找分号。整段找的话，80 万个 `&` 要 10 秒。
    let window = rest.len().min(33);
    let end = rest.as_bytes()[..window]
        .iter()
        .position(|byte| *byte == b';')?;
    if end == 0 || end > 32 {
        return None;
    }
    let body = rest.get(..end)?;
    let next = start + 1 + end + 1;
    let ch = if let Some(numeric) = body.strip_prefix('#') {
        let code = if let Some(hex) = numeric
            .strip_prefix('x')
            .or_else(|| numeric.strip_prefix('X'))
        {
            u32::from_str_radix(hex, 16).ok()?
        } else if !numeric.is_empty() && numeric.bytes().all(|byte| byte.is_ascii_digit()) {
            numeric.parse().ok()?
        } else {
            return None;
        };
        char::from_u32(code)?
    } else {
        match body {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            "nbsp" => '\u{00a0}',
            _ => return None,
        }
    };
    Some((ch, next))
}

fn strip_controls(text: &str) -> String {
    text.chars()
        .filter(|ch| (!ch.is_control() || ch.is_whitespace()) && !is_invisible_format(*ch))
        .collect()
}

/// 看不见、只会用来伪装的格式字符：双向文字控制符（把标题显示得前后颠倒）、零宽空格、隐形的词连接符、标签字符。
/// 不去掉零宽连接符 U+200C / U+200D：表情符号组合和印度文字要用它。
fn is_invisible_format(ch: char) -> bool {
    matches!(
        ch as u32,
        0x00AD
            | 0x061C
            | 0x180E
            | 0x200B
            | 0x200E..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x2069
            | 0xFEFF
            | 0xE0000..=0xE007F
    )
}

fn collapse_ws(text: &str, keep_newlines: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut newline = false;
    let mut space = false;
    for ch in text.chars() {
        let is_break = ch == '\n' || ch == '\r';
        if is_break && keep_newlines {
            if !out.is_empty() {
                newline = true;
            }
            space = false;
            continue;
        }
        if ch.is_whitespace() {
            if !out.is_empty() && !newline {
                space = true;
            }
            continue;
        }
        if newline {
            out.push('\n');
            newline = false;
            space = false;
        } else if space {
            out.push(' ');
            space = false;
        }
        out.push(ch);
    }
    out
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        text.chars().take(max_chars).collect()
    }
}

struct Tag {
    name: String,
    closing: bool,
    self_closing: bool,
    attrs: Vec<(String, String)>,
    start: usize,
    end: usize,
}

fn html_fragment_to_text(fragment: &str, keep_newlines: bool) -> String {
    let cleaned = remove_hidden(fragment);
    let bytes = cleaned.as_bytes();
    let mut raw = String::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index] != b'<' {
                index += 1;
            }
            raw.push_str(&cleaned[start..index]);
            continue;
        }
        match scan_tag_at(&cleaned, index) {
            TagScan::Tag(tag) => {
                if tag.name == "br" || (tag.closing && is_block_tag(&tag.name)) {
                    raw.push('\n');
                }
                index = tag.end;
                continue;
            }
            TagScan::NotATag => {}
            TagScan::Unterminated => {
                raw.push_str(&cleaned[index..]);
                break;
            }
        }
        raw.push('<');
        index += 1;
    }
    collapse_ws(&strip_controls(&decode_entities(&raw)), keep_newlines)
}

fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "li"
            | "tr"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "blockquote"
            | "main"
    )
}

fn remove_hidden(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut index = 0;
    while index < bytes.len() {
        if html.is_char_boundary(index) && html[index..].starts_with("<!--") {
            if let Some(relative) = html[index + 4..].find("-->") {
                index = index + 4 + relative + 3;
                continue;
            }
            break;
        }
        if bytes[index] != b'<' {
            let start = index;
            index += 1;
            while index < bytes.len() && bytes[index] != b'<' {
                index += 1;
            }
            out.push_str(&html[start..index]);
            continue;
        }
        match scan_tag_at(html, index) {
            TagScan::Tag(tag) => {
                if !tag.closing
                    && !tag.self_closing
                    && matches!(tag.name.as_str(), "script" | "style" | "noscript" | "svg")
                {
                    index = if tag.name == "svg" {
                        skip_element(html, tag.end, &tag.name)
                    } else {
                        skip_raw_text_element(html, tag.end, &tag.name)
                    };
                    continue;
                }
                out.push_str(&html[index..tag.end]);
                index = tag.end;
                continue;
            }
            TagScan::NotATag => {}
            // 没结束的标签吞掉了剩下的整页：丢掉。浏览器也不会显示它，
            // 而推给后面的话，里面的 <script> / <style> 源码会漏进摘要。
            TagScan::Unterminated => break,
        }
        out.push('<');
        index += 1;
    }
    out
}

/// `script`、`style`、`noscript` 的内容是原样文本，不是 HTML：到第一个 `</名字` 就结束，
/// 里面出现的 `<script>`、`<div>` 之类（脚本字符串里的模板）都不算标签。
/// 以前按「标签嵌套深度」数，脚本里一个没配对的 `<script>` 就让整页后面的内容全被吞掉
/// （一篇公众号文章 3.6 MB 只剩 1 KB，标题、正文全丢）。
fn skip_raw_text_element(html: &str, content_start: usize, name: &str) -> usize {
    let bytes = html.as_bytes();
    let needle = format!("</{name}").into_bytes();
    let mut index = content_start;
    while index + needle.len() <= bytes.len() {
        if bytes[index] == b'<'
            && bytes[index..index + needle.len()].eq_ignore_ascii_case(&needle)
            && bytes
                .get(index + needle.len())
                .is_none_or(|next| next.is_ascii_whitespace() || *next == b'>' || *next == b'/')
        {
            return match html[index..].find('>') {
                Some(relative) => index + relative + 1,
                None => html.len(),
            };
        }
        index += 1;
    }
    html.len()
}

fn skip_element(html: &str, content_start: usize, name: &str) -> usize {
    let mut depth = 1;
    let mut index = content_start;
    while let Some(tag) = next_tag(html, index) {
        if tag.name == name {
            if tag.closing {
                depth -= 1;
                if depth == 0 {
                    return tag.end;
                }
            } else if !tag.self_closing {
                depth += 1;
            }
        }
        index = tag.end;
    }
    html.len()
}

fn element_inner<'a>(html: &'a str, name: &str) -> Option<&'a str> {
    element_inner_if(html, |tag| !tag.closing && tag.name == name)
}

fn element_by_id<'a>(html: &'a str, id: &str) -> Option<&'a str> {
    element_inner_if(html, |tag| {
        !tag.closing && attr(tag, "id").is_some_and(|value| value == id)
    })
}

fn element_by_class_or_id<'a>(html: &'a str, token: &str) -> Option<&'a str> {
    element_inner_if(html, |tag| {
        if tag.closing {
            return false;
        }
        attr(tag, "id").is_some_and(|value| value == token)
            || attr(tag, "class")
                .is_some_and(|value| value.split_whitespace().any(|part| part == token))
    })
}

fn element_inner_if<'a>(html: &'a str, mut pred: impl FnMut(&Tag) -> bool) -> Option<&'a str> {
    let mut index = 0;
    while let Some(tag) = next_tag(html, index) {
        if pred(&tag) {
            if tag.self_closing {
                return Some("");
            }
            return Some(inner_until_close(html, &tag));
        }
        index = tag.end;
    }
    None
}

fn inner_until_close<'a>(html: &'a str, open: &Tag) -> &'a str {
    let mut depth = 1;
    let mut index = open.end;
    while let Some(tag) = next_tag(html, index) {
        if tag.name == open.name {
            if tag.closing {
                depth -= 1;
                if depth == 0 {
                    return &html[open.end..tag.start];
                }
            } else if !tag.self_closing {
                depth += 1;
            }
        }
        index = tag.end;
    }
    &html[open.end..]
}

fn next_tag(html: &str, from: usize) -> Option<Tag> {
    let bytes = html.as_bytes();
    let mut index = from;
    while index < bytes.len() {
        if bytes[index] == b'<' {
            if html.is_char_boundary(index) && html[index..].starts_with("<!--") {
                if let Some(relative) = html[index + 4..].find("-->") {
                    index = index + 4 + relative + 3;
                    continue;
                }
                return None;
            }
            match scan_tag_at(html, index) {
                TagScan::Tag(tag) => return Some(tag),
                TagScan::NotATag => {}
                // 没结束的标签吞掉了剩下的整页：后面不会再有标签
                TagScan::Unterminated => return None,
            }
        }
        index += 1;
    }
    None
}

/// 一个 `<` 开头的东西是什么。
enum TagScan {
    Tag(Tag),
    /// 不是标签（`<` 后面不是标签名，或是声明）。
    NotATag,
    /// 是标签的开头，但一直找不到结束的 `>`（或引号没配对）直到文档末尾。浏览器会把剩下的整页都当成这个标签的内容。
    /// 调用方碰到它就必须停止找标签、把剩下的当文字：从下一个 `<` 重新扫会又扫到末尾，
    /// 一页 10 万个没结束的 `<a` 就要 12 秒（二次方），6 MB 的页面要十几个小时。
    Unterminated,
}

fn scan_tag_at(html: &str, start: usize) -> TagScan {
    let Some(rest) = html.get(start..) else {
        return TagScan::NotATag;
    };
    let bytes = rest.as_bytes();
    if !rest.starts_with('<') {
        return TagScan::NotATag;
    }
    let mut index = 1;
    if index < bytes.len() && (bytes[index] == b'!' || bytes[index] == b'?') {
        return TagScan::NotATag;
    }
    let closing = if index < bytes.len() && bytes[index] == b'/' {
        index += 1;
        true
    } else {
        false
    };
    // 按 HTML 规范：`<` 或 `</` 后面必须紧跟一个字母才是标签。`I <3 you`、`a < b`、`<1>` 都只是文字：
    // 以前把它们当成标签，一路扫到下一个 `>`，中间的内容（比如一个仓库网址）就丢了。
    if index >= bytes.len() || !bytes[index].is_ascii_alphabetic() {
        return TagScan::NotATag;
    }
    let name_start = index;
    while index < bytes.len() && is_name_byte(bytes[index]) {
        index += 1;
    }
    if index == name_start {
        return TagScan::NotATag;
    }
    let name = rest[name_start..index].to_ascii_lowercase();
    let attr_start = index;
    // 按 HTML 规范，引号只在「属性值的开头」才算引号：`=` 后面（可以隔着空白）紧跟的 `"` 或 `'`。
    // 别处的引号（属性名里、没加引号的值里、`app-argument="" />` 这种多出来的）都只是普通字符。
    // 以前遇到任何引号都翻转「在引号里」，一个多出来的引号就让后面每个 `>` 都被当成在引号里，
    // 一路扫到文末，这个标签和后面一大片都读不出来（少数派首页的摘要因此变成了一堆标签源码）。
    #[derive(Clone, Copy, PartialEq)]
    enum Scan {
        Plain,
        AfterEquals,
        DoubleQuoted,
        SingleQuoted,
    }
    let mut state = Scan::Plain;
    while index < bytes.len() {
        let byte = bytes[index];
        match state {
            Scan::DoubleQuoted => {
                if byte == b'"' {
                    state = Scan::Plain;
                }
                index += 1;
                continue;
            }
            Scan::SingleQuoted => {
                if byte == b'\'' {
                    state = Scan::Plain;
                }
                index += 1;
                continue;
            }
            Scan::AfterEquals => {
                if byte.is_ascii_whitespace() {
                    index += 1;
                    continue;
                }
                if byte == b'"' {
                    state = Scan::DoubleQuoted;
                    index += 1;
                    continue;
                }
                if byte == b'\'' {
                    state = Scan::SingleQuoted;
                    index += 1;
                    continue;
                }
                // 没加引号的值（或者 `<a href=>`）：这个字符交给下面按普通字符处理
                state = Scan::Plain;
            }
            Scan::Plain => {}
        }
        if byte == b'=' {
            state = Scan::AfterEquals;
            index += 1;
            continue;
        }
        if byte == b'>' {
            let raw_attrs = &rest[attr_start..index];
            let self_closing = raw_attrs.trim_end().ends_with('/');
            let attrs = if closing {
                Vec::new()
            } else {
                parse_attrs(raw_attrs.trim_end().trim_end_matches('/'))
            };
            return TagScan::Tag(Tag {
                name,
                closing,
                self_closing,
                attrs,
                start,
                end: start + index + 1,
            });
        }
        index += 1;
    }
    TagScan::Unterminated
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b':'
}

fn parse_attrs(source: &str) -> Vec<(String, String)> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut attrs = Vec::new();
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] == b'/' {
            break;
        }
        let name_start = index;
        while index < bytes.len() && (is_name_byte(bytes[index]) || bytes[index] == b'_') {
            index += 1;
        }
        if index == name_start {
            // 不认识的字节（`@click`、多余的引号）：跳过它，后面的属性（比如 href）还要读。
            // 按字符边界走，免得落在多字节字符中间。
            index = next_boundary(source, index);
            continue;
        }
        let name = source[name_start..index].to_ascii_lowercase();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            attrs.push((name, String::new()));
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let value = if bytes[index] == b'"' || bytes[index] == b'\'' {
            let quote = bytes[index];
            index += 1;
            let value_start = index;
            while index < bytes.len() && bytes[index] != quote {
                index += 1;
            }
            let value = source[value_start..index].to_owned();
            if index < bytes.len() {
                index += 1;
            }
            value
        } else {
            let value_start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'>'
            {
                index += 1;
            }
            source[value_start..index].to_owned()
        };
        attrs.push((name, value));
    }
    attrs
}

fn attr<'a>(tag: &'a Tag, name: &str) -> Option<&'a str> {
    tag.attrs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

fn collect_metas(html: &str) -> Vec<(String, String)> {
    let mut metas = Vec::new();
    let mut index = 0;
    while let Some(tag) = next_tag(html, index) {
        if !tag.closing && tag.name == "meta" {
            let content = attr(&tag, "content").unwrap_or("").to_owned();
            if let Some(property) = attr(&tag, "property") {
                metas.push((property.to_ascii_lowercase(), content.clone()));
            }
            if let Some(name) = attr(&tag, "name") {
                metas.push((name.to_ascii_lowercase(), content));
            }
        }
        index = tag.end;
    }
    metas
}

fn meta_content(metas: &[(String, String)], key: &str) -> Option<String> {
    metas
        .iter()
        .find(|(name, value)| name == key && !value.trim().is_empty())
        .map(|(_, value)| value.clone())
}

/// 两种赋值符号都认的简便写法，只有测试在用（生产代码用 `js_assigned_string_with` 指定符号）。
#[cfg(test)]
fn js_assigned_string(html: &str, ident: &str) -> Option<String> {
    js_assigned_string_with(html, ident, b"=:")
}

/// `separators` 是变量名后面允许的赋值符号：`=`（`var nickname = …`）或 `:`（`nick_name: '…'`）。
fn js_assigned_string_with(html: &str, ident: &str, separators: &[u8]) -> Option<String> {
    let bytes = html.as_bytes();
    let needle = ident.as_bytes();
    let mut index = 0;
    while index + needle.len() <= bytes.len() {
        if !html.is_char_boundary(index) {
            index += 1;
            continue;
        }
        if html[index..].starts_with(ident) {
            // 前面不能紧贴字母数字、_、$，也不能是「-」：`data-miniprogram-nickname="…"` 是 HTML 属性，不是 JS 变量
            let before_ok =
                index == 0 || !(is_ident_byte(bytes[index - 1]) || bytes[index - 1] == b'-');
            let after = index + needle.len();
            let after_ok = after >= bytes.len() || !is_ident_byte(bytes[after]);
            if before_ok && after_ok {
                if let Some(value) = assignment_after(html, after, separators) {
                    if !value.is_empty() {
                        return Some(value);
                    }
                }
            }
        }
        index += 1;
    }
    None
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn assignment_after(html: &str, mut index: usize, separators: &[u8]) -> Option<String> {
    let bytes = html.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index >= bytes.len() || !separators.contains(&bytes[index]) {
        return None;
    }
    // `===`、`==` 是比较，不是赋值
    if bytes[index] == b'=' && bytes.get(index + 1) == Some(&b'=') {
        return None;
    }
    index += 1;
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    // 公众号页面里是 `var nickname = htmlDecode("名字")`：允许值外面包一层函数调用
    if index < bytes.len() && (bytes[index].is_ascii_alphabetic() || bytes[index] == b'_') {
        let mut call = index;
        while call < bytes.len() && (is_ident_byte(bytes[call]) || bytes[call] == b'.') {
            call += 1;
        }
        while call < bytes.len() && bytes[call].is_ascii_whitespace() {
            call += 1;
        }
        if call < bytes.len() && bytes[call] == b'(' {
            index = call + 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
        }
    }
    if index >= bytes.len() || (bytes[index] != b'"' && bytes[index] != b'\'') {
        return None;
    }
    let quote = bytes[index];
    index += 1;
    let mut out = String::new();
    while index < bytes.len() {
        if !html.is_char_boundary(index) {
            // 不该走到这里（下面的转义都按字符前进）；万一落在字符中间，跳过去，不能从中间切片
            index = next_boundary(html, index);
            continue;
        }
        if bytes[index] == quote {
            return Some(out);
        }
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            // 转义符后面可能是多字节字符（`\é`）：按字符取，再按它的字节数前进。
            // 以前只取一个字节再 index += 2，落在字符中间，下一次切片就 panic，整个收集箱读取卡死。
            let Some(escaped_char) = html[index + 1..].chars().next() else {
                break;
            };
            let escaped = if escaped_char.is_ascii() {
                escaped_char as u8
            } else {
                0
            };
            if escaped == b'u' && index + 6 <= bytes.len() && html.is_char_boundary(index + 6) {
                let hex = &html[index + 2..index + 6];
                if let Ok(code) = u32::from_str_radix(hex, 16) {
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                        index += 6;
                        continue;
                    }
                }
            }
            match escaped {
                b'\\' => out.push('\\'),
                b'"' => out.push('"'),
                b'\'' => out.push('\''),
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                b'/' => out.push('/'),
                _ => {
                    out.push('\\');
                    out.push(escaped_char);
                }
            }
            index += 1 + escaped_char.len_utf8();
            continue;
        }
        let next = next_boundary(html, index);
        out.push_str(&html[index..next]);
        index = next;
    }
    None
}

fn next_boundary(text: &str, index: usize) -> usize {
    (index + 1..=text.len())
        .find(|next| text.is_char_boundary(*next))
        .unwrap_or(text.len())
}

fn open_read(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .busy_timeout(Duration::from_millis(5000))
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    Ok(connection)
}

fn open_write(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open(path).map_err(|error| format!("数据库打开失败：{error}"))?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")
        .map_err(|error| format!("数据库参数设置失败：{error}"))?;
    Ok(connection)
}

fn load_pending(
    connection: &Connection,
    only_id: &str,
    limit: i64,
) -> Result<Vec<PendingRow>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, original_url, normalized_url, item_type
             FROM inbox_items
             WHERE status <> 'archived'
               AND (
                 (?1 = '' AND fetch_status = 'pending')
                 OR (?1 <> '' AND id = ?1 AND fetch_status IN ('pending', 'failed'))
               )
             ORDER BY created_at ASC, id ASC
             LIMIT ?2",
        )
        .map_err(|error| format!("收集箱读取失败：{error}"))?;
    let rows = statement
        .query_map(params![only_id, limit], |row| {
            Ok(PendingRow {
                id: row.get(0)?,
                original_url: row.get(1)?,
                normalized_url: row.get(2)?,
                item_type: row.get(3)?,
            })
        })
        .map_err(|error| format!("收集箱读取失败：{error}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| format!("收集箱行读取失败：{error}"))?);
    }
    Ok(items)
}

fn count_pending(connection: &Connection) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM inbox_items WHERE fetch_status = 'pending' AND status <> 'archived'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("收集箱计数失败：{error}"))
}

/// 超长就从末尾丢仓库，直到 JSON 放得进列。空数组一定放得下。
fn page_repos_json(repos: &[String]) -> String {
    let mut kept = repos.to_vec();
    loop {
        match serde_json::to_string(&kept) {
            Ok(json) if json.chars().count() <= PAGE_REPOS_MAX_CHARS => return json,
            Ok(_) => {
                if kept.pop().is_none() {
                    return "[]".to_owned();
                }
            }
            Err(_) => return "[]".to_owned(),
        }
    }
}

fn write_outcome(path: &Path, id: &str, outcome: &Outcome) -> Result<(), String> {
    let mut connection = open_write(path)?;
    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("收集读取事务开启失败：{error}"))?;
    let changed = match outcome {
        Outcome::Done {
            title,
            author,
            excerpt,
            repos,
        } => {
            let author = truncate_chars(&strip_controls(author), AUTHOR_CHARS);
            let excerpt = truncate_chars(&strip_controls(excerpt), EXCERPT_CHARS);
            let title = truncate_chars(&strip_controls(title), 500);
            let page_repos = page_repos_json(repos);
            if title.is_empty() {
                tx.execute(
                    "UPDATE inbox_items
                     SET fetch_status = 'done',
                         fetch_error = '',
                         fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         page_author = ?1,
                         page_excerpt = ?2,
                         page_repos = ?3
                     WHERE id = ?4",
                    params![author, excerpt, page_repos, id],
                )
            } else {
                tx.execute(
                    "UPDATE inbox_items
                     SET fetch_status = 'done',
                         fetch_error = '',
                         fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         page_author = ?1,
                         page_excerpt = ?2,
                         page_repos = ?3,
                         title = CASE
                             WHEN title = original_url OR title = normalized_url THEN ?4
                             ELSE title
                         END
                     WHERE id = ?5",
                    params![author, excerpt, page_repos, title, id],
                )
            }
        }
        Outcome::Failed { reason } | Outcome::Skipped { reason } => {
            let status = match outcome {
                Outcome::Failed { .. } => "failed",
                _ => "skipped",
            };
            let reason = truncate_chars(&strip_controls(reason), ERROR_CHARS);
            tx.execute(
                "UPDATE inbox_items
                 SET fetch_status = ?1,
                     fetch_error = ?2,
                     fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?3",
                params![status, reason, id],
            )
        }
    }
    .map_err(|error| format!("收集读取写入失败：{error}"))?;
    if changed != 1 {
        return Err("找不到这条收集".to_owned());
    }
    tx.commit()
        .map_err(|error| format!("收集读取提交失败：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::io::Write;
    use std::net::IpAddr;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn page(status: u16, content_type: &str, body: &str, location: Option<&str>) -> PageResponse {
        PageResponse {
            status,
            content_type: content_type.to_owned(),
            body: body.to_owned(),
            final_url: String::new(),
            location: location.map(str::to_owned),
        }
    }

    fn html_page(body: &str) -> PageResponse {
        page(200, "text/html; charset=utf-8", body, None)
    }

    struct SeqFetcher {
        calls: RefCell<Vec<String>>,
        steps: RefCell<VecDeque<Result<PageResponse, FetchProblem>>>,
    }

    impl SeqFetcher {
        fn new(steps: Vec<Result<PageResponse, FetchProblem>>) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                steps: RefCell::new(steps.into()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    impl PageFetcher for SeqFetcher {
        fn get(&self, url: &str) -> Result<PageResponse, FetchProblem> {
            self.calls.borrow_mut().push(url.to_owned());
            self.steps
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err(FetchProblem::fail("测试夹具没有更多响应")))
        }
    }

    struct MapResolver {
        map: HashMap<String, Vec<IpAddr>>,
        default_ip: Option<IpAddr>,
        calls: RefCell<Vec<String>>,
    }

    impl MapResolver {
        fn public() -> Self {
            Self {
                map: HashMap::new(),
                default_ip: Some(ip("1.1.1.1")),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn with(host: &str, address: &str) -> Self {
            let mut resolver = Self::public();
            resolver
                .map
                .insert(host.to_ascii_lowercase(), vec![ip(address)]);
            resolver
        }
    }

    impl HostResolver for MapResolver {
        fn resolve(&self, host: &str, _port: u16) -> Result<Vec<IpAddr>, String> {
            let key = host
                .trim_matches(|ch| ch == '[' || ch == ']')
                .to_ascii_lowercase();
            self.calls.borrow_mut().push(key.clone());
            if let Some(found) = self.map.get(&key) {
                return Ok(found.clone());
            }
            self.default_ip
                .map(|address| vec![address])
                .ok_or_else(|| format!("测试没有配置 {key} 的解析"))
        }
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().expect("测试 IP 应能解析")
    }

    fn guarded(
        url: &str,
        resolver: &MapResolver,
        fetcher: &SeqFetcher,
    ) -> Result<PageResponse, FetchProblem> {
        fetch_guarded(url, resolver, &|next, _ip| fetcher.get(next))
    }

    struct TempDb {
        path: PathBuf,
    }

    impl TempDb {
        fn new(label: &str) -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let seq = SEQ.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("系统时间应可用")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("fsl-fetch-{label}-{nanos}-{seq}.sqlite3"));
            crate::db::migrations::migrate_database(&path).expect("测试库应能迁移");
            let connection = Connection::open(&path).expect("应能打开测试库");
            connection
                .execute(
                    "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                    [],
                )
                .expect("应能写入测试账号");
            drop(connection);
            Self { path }
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
            let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
        }
    }

    struct Stored {
        title: String,
        status: String,
        user_note: String,
        source_context: String,
        raw_excerpt: String,
        updated_at: String,
        fetch_status: String,
        fetch_error: String,
        page_author: String,
        page_excerpt: String,
        fetched_at: String,
        page_repos: String,
    }

    fn insert_item(
        db: &TempDb,
        id: &str,
        url: &str,
        title: &str,
        item_type: &str,
        created_at: &str,
    ) {
        let connection = Connection::open(&db.path).expect("应能打开测试库");
        connection
            .execute(
                "INSERT INTO inbox_items(id, account_id, original_url, normalized_url, title, item_type, user_note, status, source_context, raw_excerpt, source_platform, created_at, updated_at)
                 VALUES (?1, 'acct', ?2, ?2, ?3, ?4, '用户备注', 'ready', '群聊原语境', '用户原话', 'web', ?5, '2000-01-01T00:00:00.000Z')",
                params![id, url, title, item_type, created_at],
            )
            .expect("应能插入收集条目");
    }

    fn stored(db: &TempDb, id: &str) -> Stored {
        let connection = Connection::open(&db.path).expect("应能打开测试库");
        connection
            .query_row(
                "SELECT title, status, user_note, source_context, raw_excerpt, updated_at, fetch_status, fetch_error, page_author, page_excerpt, ifnull(fetched_at, ''), page_repos
                 FROM inbox_items WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Stored {
                        title: row.get(0)?,
                        status: row.get(1)?,
                        user_note: row.get(2)?,
                        source_context: row.get(3)?,
                        raw_excerpt: row.get(4)?,
                        updated_at: row.get(5)?,
                        fetch_status: row.get(6)?,
                        fetch_error: row.get(7)?,
                        page_author: row.get(8)?,
                        page_excerpt: row.get(9)?,
                        fetched_at: row.get(10)?,
                        page_repos: row.get(11)?,
                    })
                },
            )
            .expect("应能读回收集条目")
    }

    fn assert_user_fields_untouched(row: &Stored) {
        assert_eq!(row.status, "ready");
        assert_eq!(row.user_note, "用户备注");
        assert_eq!(row.source_context, "群聊原语境");
        assert_eq!(row.raw_excerpt, "用户原话");
        assert_eq!(row.updated_at, "2000-01-01T00:00:00.000Z");
    }

    fn x_json(author: &str, html: &str) -> String {
        json!({ "author_name": author, "html": html }).to_string()
    }

    #[test]
    fn x_oembed_plain_text() {
        let outcome = read_x_body(&x_json(
            "Fox",
            "<blockquote><p lang=\"zh\">今天天气不错</p>&mdash; Fox</blockquote>",
        ));
        match outcome {
            Outcome::Done {
                title,
                author,
                excerpt,
                ..
            } => {
                assert_eq!(author, "Fox");
                assert_eq!(excerpt, "今天天气不错");
                assert_eq!(title, "「Fox：今天天气不错」");
            }
            other => panic!("应成功，得到 {other:?}"),
        }
    }

    #[test]
    fn x_oembed_br_becomes_newline_and_keeps_pic_link() {
        let outcome = read_x_body(&x_json(
            "Fox",
            "<p>第一行<br>第二行 <a href=\"https://t.co/abc\">pic.twitter.com/abc</a></p>",
        ));
        match outcome {
            Outcome::Done { excerpt, title, .. } => {
                assert_eq!(excerpt, "第一行\n第二行 pic.twitter.com/abc");
                assert!(excerpt.contains("pic.twitter.com/abc"));
                assert!(title.contains("第一行 第二行"));
            }
            other => panic!("应成功，得到 {other:?}"),
        }
    }

    #[test]
    fn x_oembed_decodes_entities() {
        let outcome = read_x_body(&x_json(
            "A &amp; B",
            "<p>A &amp; B &lt;C&gt; &#39; &quot;中&quot;</p>",
        ));
        match outcome {
            Outcome::Done {
                author,
                excerpt,
                title,
                ..
            } => {
                assert_eq!(author, "A & B");
                assert_eq!(excerpt, "A & B <C> ' \"中\"");
                assert!(title.starts_with("「A & B："));
            }
            other => panic!("应成功，得到 {other:?}"),
        }
    }

    #[test]
    fn x_oembed_chinese_title_counts_chars() {
        let body = "文".repeat(20);
        let outcome = read_x_body(&x_json("作者", &format!("<p>{body}</p>")));
        match outcome {
            Outcome::Done { title, excerpt, .. } => {
                assert_eq!(excerpt, body);
                assert_eq!(title, format!("「作者：{body}」"));
                assert!(!title.contains('…'));
            }
            other => panic!("应成功，得到 {other:?}"),
        }
    }

    #[test]
    fn x_oembed_truncates_title_at_40_chars_and_excerpt_at_2000() {
        let body = "字".repeat(2100);
        let outcome = read_x_body(&x_json("作者", &format!("<p>{body}</p>")));
        match outcome {
            Outcome::Done {
                title,
                excerpt,
                author,
                ..
            } => {
                assert_eq!(author, "作者");
                assert_eq!(excerpt.chars().count(), 2000);
                assert_eq!(excerpt, "字".repeat(2000));
                let prefix: String = "字".repeat(40);
                assert_eq!(title, format!("「作者：{prefix}…」"));
            }
            other => panic!("应成功，得到 {other:?}"),
        }
    }

    #[test]
    fn x_error_status_is_the_login_or_deleted_reason() {
        for status in [403, 404, 500, 503] {
            let fetcher = SeqFetcher::new(vec![Ok(page(status, "application/json", "{}", None))]);
            let outcome = fetch_x(&fetcher, "fox", "1");
            match outcome {
                Outcome::Failed { reason } => assert_eq!(reason, X_MISSING_REASON, "{status}"),
                other => panic!("{status} 应失败，得到 {other:?}"),
            }
        }
    }

    #[test]
    fn x_profile_is_skipped_without_fetch() {
        let db = TempDb::new("x-profile");
        insert_item(
            &db,
            "p",
            "https://x.com/fox",
            "https://x.com/fox",
            "webpage",
            "2026-10-01T00:00:00.000Z",
        );
        let fetcher = SeqFetcher::new(vec![]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能处理");
        assert_eq!(summary.skipped, 1);
        assert!(fetcher.calls().is_empty());
        let row = stored(&db, "p");
        assert_eq!(row.fetch_status, "skipped");
        assert_eq!(row.fetch_error, "不是单条帖子");
        assert_user_fields_untouched(&row);
    }

    #[test]
    fn youtube_oembed_keeps_title_and_author_and_empty_excerpt() {
        let body = json!({ "title": "演示视频", "author_name": "频道甲" }).to_string();
        let fetcher = SeqFetcher::new(vec![Ok(page(
            200,
            "application/json; charset=utf-8",
            &body,
            None,
        ))]);
        let outcome = fetch_youtube(&fetcher, "https://www.youtube.com/watch?v=abc");
        match outcome {
            Outcome::Done {
                title,
                author,
                excerpt,
                ..
            } => {
                assert_eq!(title, "演示视频");
                assert_eq!(author, "频道甲");
                assert_eq!(excerpt, "");
            }
            other => panic!("应成功，得到 {other:?}"),
        }
        let called = fetcher.calls();
        assert_eq!(called.len(), 1);
        assert!(called[0].starts_with("https://www.youtube.com/oembed?url="));
        assert!(called[0].contains("format=json"));
        assert!(called[0].contains(&url_encode("https://www.youtube.com/watch?v=abc")));
    }

    #[test]
    fn youtube_shorts_and_youtu_be_are_videos() {
        for url in [
            "https://www.youtube.com/shorts/abc123",
            "https://youtu.be/abc123",
            "https://m.youtube.com/watch?v=abc123",
        ] {
            let parsed = url::Url::parse(url).expect("链接应能解析");
            assert!(is_youtube(&parsed), "{url}");
            let fetcher = SeqFetcher::new(vec![Ok(page(
                200,
                "application/json",
                r#"{"title":"短视频","author_name":"甲"}"#,
                None,
            ))]);
            assert!(
                matches!(fetch_youtube(&fetcher, url), Outcome::Done { .. }),
                "{url}"
            );
            let called = fetcher.calls();
            assert_eq!(called.len(), 1, "{url}");
            assert!(
                called[0].starts_with("https://www.youtube.com/oembed?url="),
                "{url}"
            );
            assert!(called[0].contains(&url_encode(url)), "{url}");
            assert!(called[0].contains("format=json"), "{url}");
        }
        let channel =
            url::Url::parse("https://www.youtube.com/feed/subscriptions").expect("应能解析");
        assert!(!is_youtube(&channel));
        let missing = url::Url::parse("https://www.youtube.com/watch?list=1").expect("应能解析");
        assert!(!is_youtube(&missing));
    }

    #[test]
    fn html_og_fields_win_and_article_beats_main() {
        let html = "\
<meta property=\"og:title\" content=\"OG 标题\">
<meta name=\"twitter:title\" content=\"TW 标题\">
<meta property=\"og:description\" content=\"OG 摘要\">
<meta name=\"description\" content=\"META 摘要\">
<meta name=\"author\" content=\"作者甲\">
<meta property=\"og:site_name\" content=\"站点乙\">
<article><p>文章正文</p></article>
<main>不该用主栏</main>";
        let page = extract_web(html, "https://example.com/post");
        assert!(page.failure.is_none(), "{:?}", page.failure);
        assert_eq!(page.title, "OG 标题");
        assert_eq!(page.author, "作者甲");
        assert!(page.excerpt.contains("OG 摘要"));
        assert!(page.excerpt.contains("文章正文"));
        assert!(!page.excerpt.contains("不该用主栏"));
        assert!(!page.excerpt.contains("META 摘要"));
        assert!(!page.excerpt.contains("TW 标题"));
    }

    #[test]
    fn html_title_only_is_success_with_empty_excerpt() {
        let page = extract_web(
            "<html><head><title>只有标题</title></head><body></body></html>",
            "https://example.com/only",
        );
        assert!(page.failure.is_none());
        assert_eq!(page.title, "只有标题");
        assert_eq!(page.excerpt, "");
    }

    #[test]
    fn html_title_entities_collapse_to_one_line() {
        let page = extract_web(
            "<title>A &amp; B &#x4e2d;\n下一行</title><body>正文</body>",
            "https://example.com/t",
        );
        assert_eq!(page.title, "A & B 中 下一行");
        assert!(page.excerpt.contains("正文"));
    }

    #[test]
    fn html_script_style_comment_and_svg_stay_out_of_excerpt() {
        let html = "\
<title>可见标题</title>
<body>
<script>SECRET_SCRIPT</script>
<style>SECRET_STYLE</style>
<noscript>SECRET_NOSCRIPT</noscript>
<svg>SECRET_SVG</svg>
<!-- SECRET_COMMENT -->
<article>可见正文</article>
</body>";
        let page = extract_web(html, "https://example.com/clean");
        assert_eq!(page.title, "可见标题");
        assert!(page.excerpt.contains("可见正文"));
        for secret in [
            "SECRET_SCRIPT",
            "SECRET_STYLE",
            "SECRET_NOSCRIPT",
            "SECRET_SVG",
            "SECRET_COMMENT",
        ] {
            assert!(!page.excerpt.contains(secret), "{secret} 不应进摘要");
            assert!(!page.title.contains(secret), "{secret} 不应进标题");
        }
    }

    #[test]
    fn html_uses_body_when_no_container() {
        let page = extract_web(
            "<html><head><title>标题</title></head><body><p>身体里的字</p><div>还有</div></body></html>",
            "https://blog.example/p",
        );
        assert!(page.excerpt.contains("身体里的字"));
        assert!(page.excerpt.contains("还有"));
    }

    #[test]
    fn wechat_reads_js_content_and_nickname_not_the_script() {
        let html = "\
<meta property=\"og:title\" content=\"文章标题\">
<meta property=\"og:description\" content=\"摘要一句\">
<meta property=\"og:site_name\" content=\"微信公众平台\">
<script>var nickname = \"测试号\"; SECRET_SCRIPT</script>
<body><div id=\"js_content\"><p>正文第一段</p><p>第二段</p></div><main>不是正文</main></body>";
        let page = extract_web(html, "https://mp.weixin.qq.com/s/abc");
        assert!(page.failure.is_none(), "{:?}", page.failure);
        assert_eq!(page.title, "文章标题");
        assert_eq!(page.author, "测试号");
        assert!(page.excerpt.starts_with("摘要一句"));
        assert!(page.excerpt.contains("正文第一段"));
        assert!(page.excerpt.contains("第二段"));
        assert!(!page.excerpt.contains("不是正文"));
        assert!(!page.excerpt.contains("SECRET_SCRIPT"));
        assert!(!page.excerpt.contains("var nickname"));
    }

    #[test]
    fn wechat_profile_nickname_element_is_author_when_js_missing() {
        let html = "<title>标题</title><body><strong class=\"profile_nickname\">号名</strong><div id=\"js_content\">正文足够长的一段话</div></body>";
        let page = extract_web(html, "https://mp.weixin.qq.com/s?__biz=1");
        assert_eq!(page.author, "号名");
        assert!(page.failure.is_none());
    }

    #[test]
    fn wechat_block_page_and_too_short_page_fail() {
        for html in [
            "<title>环境异常</title><body>请稍后</body>",
            "<title>验证</title><body>完成验证后即可继续访问</body>",
            "<title></title><body>请在微信客户端打开</body>",
            "<html><head></head><body>太短</body></html>",
            "<html><body><div>：</div><div>，</div><div>视频</div><div>小程序</div><div>赞，轻点两下取消赞</div><div>在看，轻点两下取消在看</div></body></html>",
        ] {
            let page = extract_web(html, "https://mp.weixin.qq.com/s/blocked");
            assert_eq!(page.failure, Some(WECHAT_BLOCK_REASON), "{html}");
        }
    }

    #[test]
    fn empty_page_fails_and_short_non_wechat_page_does_not() {
        let empty = extract_web(
            "<html><head></head><body>   </body></html>",
            "https://example.com/empty",
        );
        assert_eq!(empty.failure, Some(EMPTY_PAGE_REASON));
        let short = extract_web("<body>太短</body>", "https://example.com/short");
        assert!(short.failure.is_none(), "非公众号的短正文仍可留下");
        assert_eq!(short.excerpt, "太短");
        let quoted = extract_web(
            "<title>笔记</title><body>文中提到请在微信客户端打开链接</body>",
            "https://blog.example/note",
        );
        assert!(quoted.failure.is_none(), "普通网页引用这句话不算被拦");
    }

    #[test]
    fn non_utf8_body_decodes_without_panic() {
        let bytes =
            b"<!DOCTYPE html><title>\xff\xfeOK</title><body><article>A\x80B</article></body>";
        let text = decode_page(bytes, &[]).expect("少量非法字节不应整页判失败");
        let page = extract_web(&text, "https://example.com/bin");
        assert!(page.title.contains("OK"), "{}", page.title);
        assert!(page.excerpt.contains('A') || page.excerpt.contains('B') || page.failure.is_none());
    }

    #[test]
    fn entities_cover_named_decimal_and_hex_once() {
        assert_eq!(
            decode_entities("&amp; &lt; &gt; &quot; &#39; &apos; &nbsp;"),
            "& < > \" ' ' \u{00a0}"
        );
        assert_eq!(decode_entities("&#20013;"), "中");
        assert_eq!(decode_entities("&#x4e2d;"), "中");
        assert_eq!(decode_entities("&#X4E2D;"), "中");
        assert_eq!(decode_entities("&amp;lt;"), "&lt;");
        assert_eq!(decode_entities("&#x110000;"), "&#x110000;");
        assert_eq!(decode_entities("&amp"), "&amp");
        assert_eq!(plain_text("A&nbsp;B", 20), "A B");
    }

    #[test]
    fn http_is_skipped_before_fetch_and_resolve() {
        let resolver = MapResolver::public();
        let fetcher = SeqFetcher::new(vec![]);
        let error = guarded("http://example.com/a", &resolver, &fetcher).expect_err("http 应跳过");
        assert_eq!(error.kind, FetchProblemKind::Skip);
        assert_eq!(error.message, "只读取 https 链接");
        assert!(fetcher.calls().is_empty());
        assert!(resolver.calls.borrow().is_empty());
    }

    #[test]
    fn non_443_port_is_skipped_and_443_is_allowed() {
        let resolver = MapResolver::public();
        let fetcher = SeqFetcher::new(vec![]);
        let error = guarded("https://example.com:8443/a", &resolver, &fetcher).expect_err("应跳过");
        assert_eq!(error.message, "不读取非常规端口");
        assert!(fetcher.calls().is_empty());
        assert!(resolver.calls.borrow().is_empty());

        let ok = SeqFetcher::new(vec![Ok(html_page("<title>端口</title>"))]);
        let response = guarded("https://example.com:443/a", &resolver, &ok).expect("443 应放行");
        assert_eq!(response.status, 200);
        assert_eq!(ok.calls(), vec!["https://example.com:443/a".to_owned()]);
    }

    #[test]
    fn literal_private_loopback_metadata_and_credentials_are_rejected() {
        let resolver = MapResolver::public();
        for url in [
            "https://localhost/a",
            "https://10.0.0.1/a",
            "https://169.254.169.254/latest",
            "https://[::1]/",
            "https://[::ffff:127.0.0.1]/x",
            "https://user:secret@example.com/a",
        ] {
            let fetcher = SeqFetcher::new(vec![]);
            let error = guarded(url, &resolver, &fetcher).expect_err(url);
            assert_eq!(error.kind, FetchProblemKind::Fail, "{url}");
            assert!(fetcher.calls().is_empty(), "{url}");
            assert!(resolver.calls.borrow().is_empty(), "{url} 不应去解析");
            resolver.calls.borrow_mut().clear();
        }
    }

    #[test]
    fn resolver_private_loopback_metadata_and_mapped_are_rejected() {
        for address in [
            "10.0.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "::1",
            "::ffff:127.0.0.1",
        ] {
            let resolver = MapResolver::with("evil.example", address);
            let fetcher = SeqFetcher::new(vec![Ok(html_page("<title>不该读到</title>"))]);
            let error = guarded("https://evil.example/a", &resolver, &fetcher).expect_err(address);
            assert_eq!(error.message, BLOCKED_IP_REASON, "{address}");
            assert!(fetcher.calls().is_empty(), "{address}");
        }
        let mut mixed = MapResolver::public();
        mixed.map.insert(
            "mixed.example".to_owned(),
            vec![ip("1.1.1.1"), ip("10.0.0.1")],
        );
        let fetcher = SeqFetcher::new(vec![]);
        let error = guarded("https://mixed.example/", &mixed, &fetcher).expect_err("混有内网地址");
        assert_eq!(error.message, BLOCKED_IP_REASON);
        assert!(fetcher.calls().is_empty());
    }

    #[test]
    fn redirect_to_intranet_is_rejected_without_a_second_fetch() {
        let mut resolver = MapResolver::public();
        resolver
            .map
            .insert("hidden.example".to_owned(), vec![ip("10.0.0.1")]);
        let fetcher = SeqFetcher::new(vec![
            Ok(page(
                302,
                "text/html",
                "",
                Some("https://hidden.example/secret"),
            )),
            Ok(html_page("<title>不该读到</title>")),
        ]);
        let error = guarded("https://example.com/start", &resolver, &fetcher).expect_err("应拒绝");
        assert_eq!(error.message, BLOCKED_IP_REASON);
        assert_eq!(
            fetcher.calls(),
            vec!["https://example.com/start".to_owned()]
        );

        let literal = SeqFetcher::new(vec![
            Ok(page(302, "text/html", "", Some("https://127.0.0.1/admin"))),
            Ok(html_page("<title>不该读到</title>")),
        ]);
        let error = guarded("https://example.com/go", &MapResolver::public(), &literal)
            .expect_err("字面内网应拒绝");
        assert_eq!(error.kind, FetchProblemKind::Fail);
        assert_eq!(literal.calls().len(), 1);
    }

    #[test]
    fn redirect_over_three_hops_is_rejected() {
        let hops = [
            "https://b.example/a",
            "https://c.example/a",
            "https://d.example/a",
            "https://e.example/a",
        ];
        let mut steps = Vec::new();
        for hop in hops {
            steps.push(Ok(page(302, "text/html", "", Some(hop))));
        }
        let fetcher = SeqFetcher::new(steps);
        let error = guarded("https://a.example/start", &MapResolver::public(), &fetcher)
            .expect_err("超过 3 跳应拒绝");
        assert_eq!(error.message, "重定向超过 3 跳");
        assert_eq!(
            fetcher.calls(),
            vec![
                "https://a.example/start".to_owned(),
                "https://b.example/a".to_owned(),
                "https://c.example/a".to_owned(),
                "https://d.example/a".to_owned(),
            ]
        );
    }

    #[test]
    fn relative_redirect_is_followed_and_three_hops_can_succeed() {
        let fetcher = SeqFetcher::new(vec![
            Ok(page(302, "text/html", "", Some("/next"))),
            Ok(page(301, "text/html", "", Some("done"))),
            Ok(html_page("<title>到了</title><body>正文</body>")),
        ]);
        let response = guarded(
            "https://example.com/dir/start",
            &MapResolver::public(),
            &fetcher,
        )
        .expect("三跳以内应成功");
        // `/next` 是站内绝对路径；`done` 再相对到该路径的最后一段，结果是 /done。
        assert_eq!(response.final_url, "https://example.com/done");
        assert_eq!(
            fetcher.calls(),
            vec![
                "https://example.com/dir/start".to_owned(),
                "https://example.com/next".to_owned(),
                "https://example.com/done".to_owned(),
            ]
        );
    }

    #[test]
    fn image_and_pdf_are_skipped() {
        for content_type in ["image/png", "application/pdf", "application/octet-stream"] {
            let fetcher = SeqFetcher::new(vec![Ok(page(200, content_type, "not-html", None))]);
            let error = guarded("https://example.com/file", &MapResolver::public(), &fetcher)
                .expect_err(content_type);
            assert_eq!(error.kind, FetchProblemKind::Skip);
            assert_eq!(error.message, "不是网页");
        }
        let html = SeqFetcher::new(vec![Ok(page(
            200,
            "application/xhtml+xml; charset=utf-8",
            "<title>页面</title>",
            None,
        ))]);
        assert!(guarded("https://example.com/x", &MapResolver::public(), &html).is_ok());
    }

    #[test]
    fn body_over_the_limit_is_skipped_and_exact_limit_is_kept() {
        let exact = "a".repeat(MAX_BODY_BYTES);
        let ok = SeqFetcher::new(vec![Ok(page(200, "text/plain", &exact, None))]);
        let response = guarded("https://example.com/exact", &MapResolver::public(), &ok)
            .expect("刚好等于上限应留下");
        assert_eq!(response.body.len(), MAX_BODY_BYTES);

        let mut too_big = "a".repeat(MAX_BODY_BYTES);
        too_big.push('b');
        let fetcher = SeqFetcher::new(vec![Ok(page(200, "text/html", &too_big, None))]);
        let error = guarded("https://example.com/big", &MapResolver::public(), &fetcher)
            .expect_err("超过上限应跳过");
        assert_eq!(error.kind, FetchProblemKind::Skip);
        assert_eq!(error.message, "页面太大（超过 6 MB），没有读取");
    }

    #[test]
    fn curl_size_limit_errors_are_skips_not_network_failures() {
        let big = "curl: (56) Exceeded the maximum allowed file size (6000000) with 6000000 bytes";
        for (code, stderr) in [(63, ""), (56, big)] {
            let problem = curl_exit_problem(code, stderr);
            assert_eq!(problem.kind, FetchProblemKind::Skip, "{code}");
            assert_eq!(problem.message, "页面太大（超过 6 MB），没有读取");
        }
        // 真的断了：56 但没有大小提示
        let reset = curl_exit_problem(56, "curl: (56) Recv failure: Connection reset by peer");
        assert_eq!(reset.kind, FetchProblemKind::Fail);
        assert_eq!(reset.message, "网络连不上");
        assert_eq!(curl_exit_problem(28, "").message, "联网超时");
        assert_eq!(curl_exit_problem(6, "").message, "网络连不上");
        assert_eq!(curl_exit_problem(99, "").message, "联网失败");
    }

    #[test]
    fn transport_receives_the_resolved_public_ip() {
        let resolver = MapResolver::with("example.com", "1.1.1.1");
        let seen = RefCell::new(Vec::new());
        let response = fetch_guarded("https://example.com/a", &resolver, &|url, address| {
            seen.borrow_mut().push((url.to_owned(), address));
            Ok(html_page("<title>好</title>"))
        })
        .expect("应成功");
        assert_eq!(response.final_url, "https://example.com/a");
        assert_eq!(seen.borrow().len(), 1);
        assert_eq!(seen.borrow()[0].1, ip("1.1.1.1"));
    }

    #[test]
    fn safe_curl_arguments_pin_ip_and_carry_no_credentials() {
        let args = safe_curl_arguments(
            "https://example.com/a?q=1",
            "/tmp/fsl-inbox-headers.txt",
            "example.com:443:1.1.1.1",
            UA_SAFARI,
        );
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--resolve" && pair[1] == "example.com:443:1.1.1.1"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--max-time" && pair[1] == "10"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--connect-timeout" && pair[1] == "6"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--max-filesize" && pair[1] == "6000000"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--proto" && pair[1] == "=https"));
        assert_eq!(
            args.last().map(String::as_str),
            Some("https://example.com/a?q=1")
        );
        assert!(args
            .windows(2)
            .any(|pair| { pair[0] == "-H" && pair[1] == "Accept-Encoding: gzip, deflate" }));
        assert_eq!(args.iter().filter(|arg| arg.as_str() == "-H").count(), 1);
        for forbidden in [
            "-L",
            "-b",
            "--cookie",
            "-u",
            "--oauth2-bearer",
            "--compressed",
        ] {
            assert!(!args.iter().any(|arg| arg == forbidden), "{forbidden}");
        }
        for arg in &args {
            let lower = arg.to_ascii_lowercase();
            assert!(!lower.contains("cookie"), "{arg}");
            assert!(!lower.contains("authorization"), "{arg}");
            assert!(!lower.contains("bearer"), "{arg}");
            assert!(!lower.contains("token"), "{arg}");
        }
        assert_eq!(
            resolve_arg("example.com", 443, ip("2001:db8::1")),
            "example.com:443:[2001:db8::1]"
        );
    }

    #[test]
    fn user_agent_is_oembed_for_x_and_youtube_and_safari_for_pages() {
        assert_eq!(
            user_agent_for("https://publish.twitter.com/oembed?url=https%3A%2F%2Ftwitter.com"),
            UA_OEMBED
        );
        assert_eq!(
            user_agent_for("https://www.youtube.com/oembed?url=https%3A%2F%2Fyoutu.be%2Fabc"),
            UA_OEMBED
        );
        assert_eq!(user_agent_for("https://mp.weixin.qq.com/s/abc"), UA_SAFARI);
        assert_eq!(user_agent_for("https://example.com/post"), UA_SAFARI);
        assert!(UA_SAFARI.contains("Safari"));
        assert!(!UA_SAFARI.to_ascii_lowercase().contains("cookie"));
    }

    #[test]
    fn header_dump_reads_location_and_keeps_the_last_block() {
        let dump = "\
HTTP/1.1 100 Continue\r\n\
\r\n\
HTTP/1.1 302 Found\r\n\
Location: /next\r\n\
Content-Type: text/html; charset=utf-8\r\n\
\r\n";
        let (status, headers) = parse_header_dump(dump).expect("应能解析");
        assert_eq!(status, 302);
        assert_eq!(header_value(&headers, "location"), Some("/next"));
        assert_eq!(
            header_value(&headers, "content-type"),
            Some("text/html; charset=utf-8")
        );
    }

    #[test]
    fn pending_page_becomes_done_and_replaces_url_title() {
        let db = TempDb::new("done");
        let url = "https://example.com/post";
        insert_item(&db, "w", url, url, "webpage", "2026-10-01T00:00:00.000Z");
        let fetcher = SeqFetcher::new(vec![Ok(html_page(
            "<meta property=\"og:title\" content=\"真正的标题\"><meta name=\"author\" content=\"写的人\"><meta property=\"og:description\" content=\"一段摘要\"><article>正文</article>",
        ))]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能读取");
        assert_eq!(summary.done, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.remaining, 0);
        assert_eq!(fetcher.calls(), vec![url.to_owned()]);
        let row = stored(&db, "w");
        assert_eq!(row.fetch_status, "done");
        assert_eq!(row.fetch_error, "");
        assert_eq!(row.title, "真正的标题");
        assert_eq!(row.page_author, "写的人");
        assert!(row.page_excerpt.contains("一段摘要"));
        assert!(row.page_excerpt.contains("正文"));
        assert!(row.fetched_at.contains('T'));
        assert_user_fields_untouched(&row);
    }

    #[test]
    fn fetch_failure_records_reason_and_leaves_user_fields() {
        let db = TempDb::new("fail");
        let url = "https://example.com/missing";
        insert_item(&db, "w", url, url, "webpage", "2026-10-01T00:00:00.000Z");
        let fetcher = SeqFetcher::new(vec![Ok(page(404, "text/html", "gone", None))]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能记录失败");
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.remaining, 0);
        let row = stored(&db, "w");
        assert_eq!(row.fetch_status, "failed");
        assert_eq!(row.fetch_error, "页面不存在");
        assert_eq!(row.title, url);
        assert_eq!(row.page_author, "");
        assert_eq!(row.page_excerpt, "");
        assert_user_fields_untouched(&row);
    }

    #[test]
    fn http_row_is_skipped_without_calling_fetcher() {
        let db = TempDb::new("http");
        insert_item(
            &db,
            "h",
            "http://example.com/a",
            "http://example.com/a",
            "webpage",
            "2026-10-01T00:00:00.000Z",
        );
        let fetcher = SeqFetcher::new(vec![]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能跳过");
        assert_eq!(summary.skipped, 1);
        assert!(fetcher.calls().is_empty());
        let row = stored(&db, "h");
        assert_eq!(row.fetch_status, "skipped");
        assert_eq!(row.fetch_error, "只读取 https 链接");
        assert_user_fields_untouched(&row);
    }

    #[test]
    fn github_repository_is_skipped_without_network() {
        let db = TempDb::new("gh");
        insert_item(
            &db,
            "g",
            "https://github.com/fox/tool",
            "fox/tool",
            "github_repository",
            "2026-10-01T00:00:00.000Z",
        );
        insert_item(
            &db,
            "w",
            "https://example.com/a",
            "https://example.com/a",
            "webpage",
            "2026-10-02T00:00:00.000Z",
        );
        let fetcher = SeqFetcher::new(vec![Ok(html_page("<title>网页</title>"))]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能处理");
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.done, 1);
        assert_eq!(fetcher.calls(), vec!["https://example.com/a".to_owned()]);
        let github = stored(&db, "g");
        assert_eq!(github.fetch_status, "skipped");
        assert_eq!(github.fetch_error, "GitHub 仓库不读取网页");
        assert_eq!(github.title, "fox/tool");
        assert_user_fields_untouched(&github);
    }

    #[test]
    fn custom_title_is_not_overwritten() {
        let db = TempDb::new("title");
        let url = "https://example.com/a";
        insert_item(
            &db,
            "w",
            url,
            "我改过的标题",
            "webpage",
            "2026-10-01T00:00:00.000Z",
        );
        let fetcher = SeqFetcher::new(vec![Ok(html_page(
            "<title>抓到的标题</title><body>摘要</body>",
        ))]);
        fetch_pending(&db.path, &fetcher, None, 20).expect("应能读取");
        let row = stored(&db, "w");
        assert_eq!(row.fetch_status, "done");
        assert_eq!(row.title, "我改过的标题");
        assert!(row.page_excerpt.contains("摘要"));
        assert_user_fields_untouched(&row);
    }

    #[test]
    fn only_id_retries_failed_and_limit_keeps_the_rest_pending() {
        let db = TempDb::new("retry");
        insert_item(
            &db,
            "old",
            "https://example.com/old",
            "https://example.com/old",
            "webpage",
            "2026-10-01T00:00:00.000Z",
        );
        insert_item(
            &db,
            "mid",
            "https://example.com/mid",
            "https://example.com/mid",
            "webpage",
            "2026-10-02T00:00:00.000Z",
        );
        insert_item(
            &db,
            "new",
            "https://example.com/new",
            "https://example.com/new",
            "webpage",
            "2026-10-03T00:00:00.000Z",
        );
        let connection = Connection::open(&db.path).expect("应能打开测试库");
        connection
            .execute(
                "UPDATE inbox_items SET fetch_status = 'failed', fetch_error = '上次失败' WHERE id = 'old'",
                [],
            )
            .expect("应能标成失败");
        drop(connection);

        let untouched = SeqFetcher::new(vec![Ok(html_page("<title>不该抓失败项</title>"))]);
        let first = fetch_pending(&db.path, &untouched, None, 1).expect("limit 应生效");
        assert_eq!(first.done, 1);
        assert_eq!(first.remaining, 1);
        assert_eq!(
            untouched.calls(),
            vec!["https://example.com/mid".to_owned()]
        );
        assert_eq!(stored(&db, "old").fetch_status, "failed");
        assert_eq!(stored(&db, "new").fetch_status, "pending");

        let retry = SeqFetcher::new(vec![Ok(html_page("<title>重试成功</title>"))]);
        let second = fetch_pending(&db.path, &retry, Some("old"), 20).expect("应能重试");
        assert_eq!(second.done, 1);
        assert_eq!(second.remaining, 1);
        assert_eq!(retry.calls().len(), 1);
        assert!(retry.calls()[0].starts_with("https://example.com/old"));
        let old = stored(&db, "old");
        assert_eq!(old.fetch_status, "done");
        assert_eq!(old.fetch_error, "");
        assert_eq!(old.title, "重试成功");
        assert_eq!(stored(&db, "new").fetch_status, "pending");
        assert_user_fields_untouched(&old);
    }

    #[test]
    fn x_status_row_uses_twitter_oembed_without_cookies() {
        let db = TempDb::new("x");
        insert_item(
            &db,
            "x",
            "https://x.com/fox/status/42",
            "https://x.com/fox/status/42",
            "webpage",
            "2026-10-01T00:00:00.000Z",
        );
        let body = x_json("Fox", "<p>正文&amp;实体</p>");
        let fetcher = SeqFetcher::new(vec![Ok(page(200, "application/json", &body, None))]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能读帖");
        assert_eq!(summary.done, 1);
        let calls = fetcher.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].starts_with("https://publish.twitter.com/oembed?url="));
        assert!(calls[0].contains("omit_script=1"));
        assert!(calls[0].contains("dnt=1"));
        assert!(calls[0].contains(&url_encode("https://twitter.com/fox/status/42")));
        let row = stored(&db, "x");
        assert_eq!(row.page_author, "Fox");
        assert_eq!(row.page_excerpt, "正文&实体");
        assert_eq!(row.title, "「Fox：正文&实体」");
        assert_user_fields_untouched(&row);
    }

    #[test]
    fn fetch_summary_serde_keys_are_camel_case_words() {
        let summary = FetchSummary {
            done: 1,
            failed: 2,
            skipped: 3,
            remaining: 4,
        };
        let value = serde_json::to_value(&summary).expect("应能序列化");
        let mut keys: Vec<_> = value
            .as_object()
            .expect("应是对象")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        assert_eq!(keys, vec!["done", "failed", "remaining", "skipped"]);
        assert_eq!(value["done"], 1);
        assert_eq!(value["failed"], 2);
        assert_eq!(value["skipped"], 3);
        assert_eq!(value["remaining"], 4);
    }

    #[test]
    fn fetch_command_is_registered_and_runs_in_spawn_blocking() {
        let source = include_str!("lib.rs");
        let permissions = include_str!("../permissions/gsat-commands.toml");
        let manifest = include_str!("../gen/schemas/acl-manifests.json");
        assert!(permissions.contains("\"fetch_inbox_pending\""));
        assert!(manifest.contains("\"fetch_inbox_pending\""));
        let start = source
            .find("async fn fetch_inbox_pending(")
            .expect("应有命令");
        let rest = &source[start..];
        let end = rest.find("\n#[tauri::command]").unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(body.contains("spawn_blocking"));
        assert!(body.contains("SafePageFetcher"));
        assert!(body.contains("20"));
        let handler = source
            .find("generate_handler![")
            .expect("应有 generate_handler");
        let handler_end = source[handler..]
            .find("])")
            .map(|index| handler + index)
            .unwrap_or(source.len());
        assert!(source[handler..handler_end].contains("fetch_inbox_pending"));
    }

    #[test]
    fn checked_ip_prefers_ipv4_when_both_families_resolve() {
        let v6: IpAddr = "2606:4700::6810:84e5".parse().expect("v6");
        let v4: IpAddr = "93.184.216.34".parse().expect("v4");
        assert_eq!(checked_ip(&[v6, v4]).expect("应通过"), v4);
        assert_eq!(checked_ip(&[v4, v6]).expect("应通过"), v4);
        assert_eq!(checked_ip(&[v6]).expect("只有 v6 时用 v6"), v6);
        // 只要有一个落在内网，不管排在哪都拒绝
        let private: IpAddr = "10.0.0.1".parse().expect("private");
        assert!(checked_ip(&[v6, v4, private]).is_err());
    }

    #[test]
    fn archived_items_are_not_fetched_or_counted_as_remaining() {
        let db = TempDb::new("archived");
        insert_item(
            &db,
            "kept",
            "https://example.com/kept",
            "https://example.com/kept",
            "webpage",
            "2026-10-01T00:00:00.000Z",
        );
        insert_item(
            &db,
            "gone",
            "https://example.com/gone",
            "https://example.com/gone",
            "webpage",
            "2026-10-02T00:00:00.000Z",
        );
        let connection = Connection::open(&db.path).expect("应能打开测试库");
        connection
            .execute(
                "UPDATE inbox_items SET status = 'archived' WHERE id = 'gone'",
                [],
            )
            .expect("应能归档");
        drop(connection);
        let fetcher = SeqFetcher::new(vec![Ok(html_page("<title>留着的</title>"))]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应成功");
        assert_eq!(summary.done, 1);
        assert_eq!(summary.remaining, 0);
        assert_eq!(fetcher.calls(), vec!["https://example.com/kept".to_owned()]);
        assert_eq!(stored(&db, "gone").fetch_status, "pending");
        // 指定 id 也不读已归档的
        let again = SeqFetcher::new(Vec::new());
        let summary = fetch_pending(&db.path, &again, Some("gone"), 20).expect("应成功");
        assert_eq!(summary.done + summary.failed + summary.skipped, 0);
        assert!(again.calls().is_empty());
    }

    /// 真实公众号页面的形状（2026-10-04 真网络演练里 3.6 MB 的一篇，裁剪后）：
    /// 脚本字符串里有没配对的 `<script>`、空的 `<title>`、`data-miniprogram-nickname` 属性、
    /// `var nickname = htmlDecode("名字")`、正文在 `visibility: hidden` 的 `js_content` 里。
    #[test]
    fn real_wechat_page_shape_is_read_in_full() {
        let html = concat!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\">",
            "<meta name=\"description\" content=\"全球顶级AI开源项目大赏！\" />",
            "<meta name=\"author\" content=\"点击关注→\" />",
            "<meta property=\"og:title\" content=\"吵着要学AI的小伙伴，可以看看这些牛批的开源项目\" />",
            "<meta property=\"og:description\" content=\"全球顶级AI开源项目大赏！\" />",
            "<meta property=\"og:site_name\" content=\"微信公众平台\" />",
            "<title></title>",
            "<script>var tpl = '<script src=\"x.js\"><\\/script>'; document.write(\"<script>\"); var a = 1 < 2;</script>",
            "<style>.a{color:red}</style></head><body>",
            "<div id=\"js_article\"><h1 class=\"rich_media_title\"><span class=\"js_title_inner\">吵着要学AI</span></h1>",
            "<a data-miniprogram-nickname=\"小程序名\" href=\"#\">x</a>",
            "<div class=\"rich_media_content\" id=\"js_content\" style=\"visibility: hidden;\">",
            "<p>小伙伴们 大家好</p><p>TensorFlow 项目名称</p></div></div>",
            "<script>var attrs = {\n  nickname: \"data-miniprogram-nickname\",\n};</script>",
            "<script>if (nickname === '') {}; var nickname = htmlDecode(\"CodeSheep\"); var profile_nickname = \"\";</script>",
            "<script>window.cgiData = {nick_name: 'CodeSheep', ads: {nick_name: '微信广告'}}</script>",
            "</body></html>"
        );
        let page = extract_web(html, "https://mp.weixin.qq.com/s/abc");
        assert_eq!(page.failure, None);
        assert_eq!(page.title, "吵着要学AI的小伙伴，可以看看这些牛批的开源项目");
        assert_eq!(page.author, "CodeSheep");
        assert!(
            page.excerpt.starts_with("全球顶级AI开源项目大赏！"),
            "{}",
            page.excerpt
        );
        assert!(page.excerpt.contains("小伙伴们 大家好"));
        assert!(page.excerpt.contains("TensorFlow 项目名称"));
        assert!(!page.excerpt.contains("document.write"));
        assert!(!page.excerpt.contains("htmlDecode"));
    }

    #[test]
    fn script_content_ends_at_the_first_closing_tag_even_with_unpaired_opens_inside() {
        let html = "<p>前</p><script>var s = '<script>'; if (a < b) {}</script><p>后</p><style>p{}</style><p>尾</p>";
        assert_eq!(remove_hidden(html), "<p>前</p><p>后</p><p>尾</p>");
        // 大小写和空白不影响结束标签；没有结束标签就吃到文末
        assert_eq!(remove_hidden("a<SCRIPT>x</SCRIPT >b"), "ab");
        assert_eq!(remove_hidden("a<script>没有结束"), "a");
        // 名字只是前缀的标签不算结束：</scripts> 不是 </script>
        assert_eq!(remove_hidden("a<script>x</scripts>y</script>b"), "ab");
    }

    #[test]
    fn attribute_named_like_a_variable_is_not_an_author() {
        let html = "<a data-miniprogram-nickname=\"属性里的\">x</a><script>var nickname = 'JS里的';</script>";
        assert_eq!(
            js_assigned_string(html, "nickname").as_deref(),
            Some("JS里的")
        );
        assert_eq!(
            js_assigned_string(
                "var n = decodeURIComponent('甲'); var nickname = htmlDecode( \"乙\" );",
                "nickname"
            )
            .as_deref(),
            Some("乙")
        );
        assert_eq!(
            js_assigned_string("<a data-x-nickname=\"丙\">", "nickname"),
            None
        );
    }

    fn set_page_repos(db: &TempDb, id: &str, value: &str) {
        let connection = Connection::open(&db.path).expect("应能打开测试库");
        connection
            .execute(
                "UPDATE inbox_items SET page_repos = ?1 WHERE id = ?2",
                params![value, id],
            )
            .expect("应能写入 page_repos");
    }

    #[test]
    fn wechat_js_content_collects_href_text_and_ignores_chrome() {
        let html = concat!(
            "<script>var tpl = '<script src=\"x.js\"><\\/script>'; document.write(\"<script>\"); var hidden = \"https://github.com/hidden/script-repo\";</script>",
            "<nav><a href=\"https://github.com/site/nav-link\">导航</a></nav>",
            "<aside>侧栏 github.com/site/side-link</aside>",
            "<div class=\"rich_media_content\" id=\"js_content\" style=\"visibility: hidden;\">",
            "<p>推荐 <a href=\"https://github.com/a/b\">项目 A</a></p>",
            "<p>纯文字 github.com/c/d 看看</p>",
            "<p>项目地址：https://github.com/e/f。</p>",
            "<script>var inside = \"https://github.com/hidden/inside-script\";</script>",
            "</div>",
            "<footer>页脚 https://github.com/site/footer-link</footer>",
        );
        let found = github_repos_in(html, "https://mp.weixin.qq.com/s/abc");
        assert_eq!(
            serde_json::to_string(&found).expect("应能序列化"),
            r#"["a/b","c/d","e/f"]"#
        );
    }

    #[test]
    fn href_and_text_entities_are_decoded() {
        let html = "\
<article>
  <a href=\"https://github.com/amp-user/amp-repo&#63;x=1&amp;y=2\">看</a>
  <p>地址 github.com&#47;text-owner&#47;text-repo</p>
</article>";
        assert_eq!(
            github_repos_in(html, "https://example.com/amp"),
            vec![
                "amp-user/amp-repo".to_owned(),
                "text-owner/text-repo".to_owned(),
            ]
        );
    }

    #[test]
    fn git_tree_fragment_and_punctuation_still_name_the_repo() {
        let html = "\
<article>
https://github.com/o/n.git
https://github.com/p/q/tree/main
https://github.com/r/s#readme
https://github.com/t/u。
（https://github.com/v/w）
</article>";
        assert_eq!(
            github_repos_in(html, "https://example.com/forms"),
            vec![
                "o/n".to_owned(),
                "p/q".to_owned(),
                "r/s".to_owned(),
                "t/u".to_owned(),
                "v/w".to_owned(),
            ]
        );
    }

    #[test]
    fn same_repo_keeps_the_first_spelling_only() {
        let html = "<article>https://github.com/OpenAI/Codex https://github.com/openai/codex.git https://github.com/OPENAI/Codex/tree/main</article>";
        assert_eq!(
            github_repos_in(html, "https://example.com/case"),
            vec!["OpenAI/Codex".to_owned()]
        );
    }

    #[test]
    fn repos_keep_first_seen_order() {
        let html = "\
<article>
github.com/first/one
<a href=\"https://github.com/second/two\">x</a>
https://github.com/third/three
</article>";
        assert_eq!(
            github_repos_in(html, "https://example.com/order"),
            vec![
                "first/one".to_owned(),
                "second/two".to_owned(),
                "third/three".to_owned(),
            ]
        );
    }

    #[test]
    fn repos_keep_only_the_first_thirty() {
        let mut html = String::from("<article>");
        for index in 0..31 {
            html.push_str(&format!("https://github.com/owner/repo-{index:02} "));
        }
        html.push_str("</article>");
        let found = github_repos_in(&html, "https://example.com/many");
        assert_eq!(found.len(), 30);
        assert_eq!(found[0], "owner/repo-00");
        assert_eq!(found[29], "owner/repo-29");
        assert!(!found.iter().any(|name| name == "owner/repo-30"));
    }

    #[test]
    fn hex_names_and_illegal_owners_are_dropped() {
        let hash = "0123456789abcdef".repeat(2);
        assert_eq!(hash.len(), 32);
        let letters = "z".repeat(32);
        let html = format!(
            "<article>
https://github.com/bad_owner/ok
https://github.com/ok.owner/repo
https://github.com/good/my.repo_name
https://github.com/repository/{hash}
https://github.com/good/{letters}
</article>"
        );
        assert_eq!(
            github_repos_in(&html, "https://mp.weixin.qq.com/s/hash"),
            vec!["good/my.repo_name".to_owned(), format!("good/{letters}"),]
        );
    }

    #[test]
    fn feature_and_topic_paths_are_not_repos() {
        let html = "\
<article>
https://github.com/features
https://github.com/features/copilot
https://github.com/topics/x
https://github.com/topics/rust
https://github.com/real/repo
</article>";
        assert_eq!(
            github_repos_in(html, "https://example.com/reserved"),
            vec!["real/repo".to_owned()]
        );
    }

    #[test]
    fn github_page_returns_empty() {
        let html = "<article><a href=\"https://github.com/a/b\">a</a> github.com/c/d</article>";
        for url in [
            "https://github.com/someone/else",
            "https://www.github.com/someone/else",
            "https://GITHUB.com/topics/rust",
        ] {
            assert!(
                github_repos_in(html, url).is_empty(),
                "{url} 是 GitHub 页面，不应抽取"
            );
        }
        assert_eq!(
            github_repos_in(html, "https://example.com/p"),
            vec!["a/b".to_owned(), "c/d".to_owned()]
        );
    }

    #[test]
    fn ordinary_pages_prefer_article_then_main_then_body() {
        let article = "\
<nav><a href=\"https://github.com/nav/only\">n</a></nav>
<article><a href=\"https://github.com/in/article\">a</a></article>
<footer>https://github.com/foot/only</footer>";
        assert_eq!(
            github_repos_in(article, "https://blog.example/p"),
            vec!["in/article".to_owned()]
        );
        let main = "<nav>https://github.com/nav/only</nav><main>https://github.com/in/main</main>";
        assert_eq!(
            github_repos_in(main, "https://blog.example/m"),
            vec!["in/main".to_owned()]
        );
        let body =
            "<nav>https://github.com/out/nav</nav><body><p>https://github.com/in/body</p></body>";
        assert_eq!(
            github_repos_in(body, "https://blog.example/b"),
            vec!["in/body".to_owned()]
        );
        let whole = "<div>https://github.com/whole/page</div>";
        assert_eq!(
            github_repos_in(whole, "https://blog.example/w"),
            vec!["whole/page".to_owned()]
        );
    }

    #[test]
    fn page_without_github_repos_returns_empty() {
        assert!(github_repos_in(
            "<article><p>今天没有项目</p></article>",
            "https://example.com/p"
        )
        .is_empty());
        assert!(github_repos_in("", "https://example.com/p").is_empty());
        assert!(github_repos_in("<<<<不是标签", ":::").is_empty());
    }

    #[test]
    fn successful_fetch_writes_page_repos_without_touching_user_fields() {
        let db = TempDb::new("repos-ok");
        let url = "https://example.com/post";
        insert_item(&db, "w", url, url, "webpage", "2026-10-01T00:00:00.000Z");
        let fetcher = SeqFetcher::new(vec![Ok(html_page(
            "<title>真正的标题</title><article><a href=\"https://github.com/a/b\">A</a> github.com/c/d</article>",
        ))]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能读取");
        assert_eq!(summary.done, 1);
        let row = stored(&db, "w");
        assert_eq!(row.fetch_status, "done");
        assert_eq!(row.page_repos, r#"["a/b","c/d"]"#);
        assert_user_fields_untouched(&row);
    }

    #[test]
    fn failed_or_skipped_fetch_does_not_change_page_repos() {
        let db = TempDb::new("repos-keep");
        insert_item(
            &db,
            "f",
            "https://example.com/missing",
            "https://example.com/missing",
            "webpage",
            "2026-10-01T00:00:00.000Z",
        );
        insert_item(
            &db,
            "s",
            "http://example.com/a",
            "http://example.com/a",
            "webpage",
            "2026-10-02T00:00:00.000Z",
        );
        set_page_repos(&db, "f", r#"["keep/me"]"#);
        set_page_repos(&db, "s", r#"["keep/me"]"#);
        let fetcher = SeqFetcher::new(vec![Ok(page(404, "text/html", "gone", None))]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能处理");
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        let failed = stored(&db, "f");
        assert_eq!(failed.fetch_status, "failed");
        assert_eq!(failed.page_repos, r#"["keep/me"]"#);
        assert_user_fields_untouched(&failed);
        let skipped = stored(&db, "s");
        assert_eq!(skipped.fetch_status, "skipped");
        assert_eq!(skipped.page_repos, r#"["keep/me"]"#);
        assert_user_fields_untouched(&skipped);
    }

    #[test]
    fn oversized_repo_list_drops_tail_and_still_writes() {
        let db = TempDb::new("repos-long");
        let url = "https://example.com/long";
        insert_item(&db, "w", url, url, "webpage", "2026-10-01T00:00:00.000Z");
        let repos: Vec<String> = (0..120)
            .map(|index| format!("owner/r{index:04}{}", "x".repeat(80)))
            .collect();
        write_outcome(
            &db.path,
            "w",
            &Outcome::Done {
                title: "标题".to_owned(),
                author: "作者".to_owned(),
                excerpt: "摘要".to_owned(),
                repos: repos.clone(),
            },
        )
        .expect("超长也应写成功");
        let row = stored(&db, "w");
        assert_user_fields_untouched(&row);
        assert!(row.page_repos.chars().count() <= 8000);
        let parsed: Vec<String> =
            serde_json::from_str(&row.page_repos).expect("page_repos 应是 JSON 数组");
        assert!(!parsed.is_empty());
        assert!(parsed.len() < repos.len());
        assert_eq!(parsed.as_slice(), &repos[..parsed.len()]);
        let mut one_more = parsed.clone();
        one_more.push(repos[parsed.len()].clone());
        let overflow = serde_json::to_string(&one_more).expect("应能序列化");
        assert!(overflow.chars().count() > 8000);

        write_outcome(
            &db.path,
            "w",
            &Outcome::Done {
                title: "标题".to_owned(),
                author: String::new(),
                excerpt: String::new(),
                repos: vec![format!("owner/{}", "y".repeat(9000))],
            },
        )
        .expect("单条超长也不应让写库失败");
        let again = stored(&db, "w");
        assert_eq!(again.page_repos, "[]");
        assert_user_fields_untouched(&again);
    }

    #[test]
    fn oembed_success_stores_empty_repo_list() {
        let db = TempDb::new("repos-oembed");
        insert_item(
            &db,
            "x",
            "https://x.com/fox/status/42",
            "https://x.com/fox/status/42",
            "webpage",
            "2026-10-01T00:00:00.000Z",
        );
        insert_item(
            &db,
            "y",
            "https://www.youtube.com/watch?v=abc123",
            "https://www.youtube.com/watch?v=abc123",
            "webpage",
            "2026-10-02T00:00:00.000Z",
        );
        set_page_repos(&db, "x", r#"["old/one"]"#);
        set_page_repos(&db, "y", r#"["old/two"]"#);
        let x_body = x_json("Fox", "<p>正文里有 https://github.com/should/ignore</p>");
        let y_body = json!({ "title": "演示", "author_name": "频道" }).to_string();
        let fetcher = SeqFetcher::new(vec![
            Ok(page(200, "application/json", &x_body, None)),
            Ok(page(200, "application/json", &y_body, None)),
        ]);
        let summary = fetch_pending(&db.path, &fetcher, None, 20).expect("应能读取");
        assert_eq!(summary.done, 2);
        let x_row = stored(&db, "x");
        assert_eq!(x_row.fetch_status, "done");
        assert_eq!(x_row.page_repos, "[]");
        assert_user_fields_untouched(&x_row);
        let y_row = stored(&db, "y");
        assert_eq!(y_row.fetch_status, "done");
        assert_eq!(y_row.page_repos, "[]");
        assert_user_fields_untouched(&y_row);
    }

    #[test]
    fn body_and_whole_page_fallbacks_skip_nav_header_footer_and_aside() {
        let body = concat!(
            "<body>",
            "<header><a href=\"https://github.com/theme/header-link\">h</a></header>",
            "<nav>https://github.com/site/nav</nav>",
            "<div><p>推荐 https://github.com/real/one</p></div>",
            "<aside>https://github.com/side/bar</aside>",
            "<footer>Powered by https://github.com/hexojs/hexo</footer>",
            "</body>",
        );
        assert_eq!(
            github_repos_in(body, "https://blog.example/b"),
            vec!["real/one".to_owned()]
        );
        let whole = concat!(
            "<nav>https://github.com/site/nav</nav>",
            "<div>https://github.com/real/two</div>",
            "<footer>https://github.com/foot/er</footer>",
        );
        assert_eq!(
            github_repos_in(whole, "https://blog.example/w"),
            vec!["real/two".to_owned()]
        );
        assert_eq!(
            github_repos_in(whole, "https://mp.weixin.qq.com/s/no-js-content"),
            vec!["real/two".to_owned()]
        );
    }

    #[test]
    fn article_region_keeps_its_own_header_and_footer() {
        let html = concat!(
            "<nav>https://github.com/site/nav</nav>",
            "<article><header>https://github.com/in/header</header>",
            "<p>https://github.com/in/body</p>",
            "<footer>https://github.com/in/footer</footer></article>",
        );
        assert_eq!(
            github_repos_in(html, "https://blog.example/a"),
            vec![
                "in/header".to_owned(),
                "in/body".to_owned(),
                "in/footer".to_owned(),
            ]
        );
    }

    #[test]
    fn unclosed_chrome_tag_does_not_swallow_the_page() {
        let html = "<div><nav>菜单<p>https://github.com/real/after-nav</p></div>";
        assert_eq!(
            github_repos_in(html, "https://blog.example/u"),
            vec!["real/after-nav".to_owned()]
        );
        let nested = concat!(
            "<div><footer><footer>https://github.com/foot/nested</footer></footer>",
            "<p>https://github.com/real/kept</p></div>",
        );
        assert_eq!(
            github_repos_in(nested, "https://blog.example/n"),
            vec!["real/kept".to_owned()]
        );
    }

    #[test]
    fn adjacent_blocks_do_not_glue_their_urls_together() {
        let html = concat!(
            "<article>",
            "<p>https://github.com/a/b</p><p>https://github.com/c/d</p>",
            "<ul><li>github.com/e/f</li><li>github.com/g/h</li></ul>",
            "<table><tr><td>github.com/i/j</td><td>github.com/k/l</td></tr></table>",
            "<div>github.com/m/n<br>github.com/o/p</div>",
            "</article>",
        );
        assert_eq!(
            github_repos_in(html, "https://blog.example/glue"),
            ["a/b", "c/d", "e/f", "g/h", "i/j", "k/l", "m/n", "o/p"]
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn url_split_across_inline_tags_is_still_found() {
        let html = concat!(
            "<div id=\"js_content\"><p><span>https://github.com/</span>",
            "<strong>owner</strong><span style=\"color:red\">/repo</span>。</p></div>",
        );
        assert_eq!(
            github_repos_in(html, "https://mp.weixin.qq.com/s/split"),
            vec!["owner/repo".to_owned()]
        );
    }

    fn header_pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn gzip_bytes(data: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).expect("gzip 夹具应能写入");
        encoder.finish().expect("gzip 夹具应能结束")
    }

    fn zlib_bytes(data: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).expect("zlib 夹具应能写入");
        encoder.finish().expect("zlib 夹具应能结束")
    }

    fn raw_deflate_bytes(data: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).expect("deflate 夹具应能写入");
        encoder.finish().expect("deflate 夹具应能结束")
    }

    fn gbk_bytes(text: &str) -> Vec<u8> {
        let (bytes, _, failed) = encoding_rs::GBK.encode(text);
        assert!(!failed, "夹具应能用 GBK 编码");
        bytes.into_owned()
    }

    /// encoding_rs 的 UTF-16 只有解码器，`encode` 会原样返回 UTF-8。夹具得自己按码元拼小端字节。
    fn utf16le_with_bom(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    fn assert_decodes_chinese(bytes: &[u8], headers: &[(&str, &str)], needle: &str) {
        let text = decode_page(bytes, &header_pairs(headers)).expect("应能解码");
        assert!(text.contains(needle), "{text}");
        assert!(!text.contains('\u{FFFD}'), "{text}");
    }

    fn assert_decode_fail(bytes: &[u8], headers: &[(&str, &str)], message: &str) {
        let problem = decode_page(bytes, &header_pairs(headers)).expect_err("应失败");
        assert_eq!(problem.kind, FetchProblemKind::Fail);
        assert_eq!(problem.message, message);
    }

    #[test]
    fn gzip_decodes_chinese_page() {
        let html = "<title>视频标题</title><body>这是一段中文简介</body>";
        assert_decodes_chinese(
            &gzip_bytes(html.as_bytes()),
            &[("content-encoding", "gzip")],
            "视频标题",
        );
        let text = decode_page(
            &gzip_bytes(html.as_bytes()),
            &header_pairs(&[("content-encoding", "gzip")]),
        )
        .expect("gzip 中文页应能解开");
        assert!(text.contains("这是一段中文简介"), "{text}");
    }

    #[test]
    fn x_gzip_and_mixed_case_header_name_decode() {
        let html = "<title>视频标题</title><body>简介</body>";
        let bytes = gzip_bytes(html.as_bytes());
        assert_decodes_chinese(&bytes, &[("Content-Encoding", "x-gzip")], "视频标题");
        assert_decodes_chinese(&bytes, &[("CONTENT-ENCODING", "X-GZIP")], "视频标题");
    }

    #[test]
    fn gzip_magic_without_content_encoding_header_decodes() {
        let html = "<title>忘了写头</title><body>中文简介</body>";
        let bytes = gzip_bytes(html.as_bytes());
        assert!(bytes.starts_with(&[0x1f, 0x8b]), "夹具应以 gzip 魔数开头");
        assert_decodes_chinese(&bytes, &[], "忘了写头");
        assert_decodes_chinese(&bytes, &[], "中文简介");
    }

    #[test]
    fn zlib_wrapped_deflate_decodes_chinese() {
        let html = "<title>压缩标题</title><body>中文正文</body>";
        assert_decodes_chinese(
            &zlib_bytes(html.as_bytes()),
            &[("content-encoding", "deflate")],
            "压缩标题",
        );
    }

    #[test]
    fn raw_deflate_decodes_chinese() {
        let html = "<title>裸压缩</title><body>中文正文</body>";
        assert_decodes_chinese(
            &raw_deflate_bytes(html.as_bytes()),
            &[("Content-Encoding", "Deflate")],
            "裸压缩",
        );
    }

    #[test]
    fn brotli_encoding_is_unsupported() {
        assert_decode_fail(
            "<title>不应解开</title>".as_bytes(),
            &[("content-encoding", "br")],
            BAD_ENCODING_REASON,
        );
    }

    #[test]
    fn zstd_encoding_is_unsupported() {
        assert_decode_fail(
            "<title>不应解开</title>".as_bytes(),
            &[("content-encoding", "zstd")],
            BAD_ENCODING_REASON,
        );
    }

    #[test]
    fn corrupt_gzip_is_rejected() {
        let mut bytes = gzip_bytes("<title>好</title>".as_bytes());
        bytes.truncate(8);
        assert_decode_fail(&bytes, &[("content-encoding", "gzip")], CORRUPT_BODY_REASON);
    }

    #[test]
    fn gzip_bomb_is_too_big_quickly() {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let chunk = [0u8; 64 * 1024];
        let mut left = 20_000_000usize;
        while left > 0 {
            let size = left.min(chunk.len());
            encoder.write_all(&chunk[..size]).expect("应能写入零");
            left -= size;
        }
        let compressed = encoder.finish().expect("应能压完 20 MB 的零");
        let started = std::time::Instant::now();
        let problem = decode_page(&compressed, &header_pairs(&[("content-encoding", "gzip")]))
            .expect_err("压缩炸弹应被拒绝");
        let elapsed = started.elapsed();
        assert_eq!(problem, too_big_problem());
        assert!(
            elapsed < Duration::from_secs(5),
            "压缩炸弹应在几秒内被拒绝，实际 {elapsed:?}"
        );
    }

    #[test]
    fn multiple_content_encodings_are_unsupported() {
        let html = "<title>不应解开</title>";
        assert_decode_fail(
            &gzip_bytes(html.as_bytes()),
            &[("content-encoding", "gzip, br")],
            BAD_ENCODING_REASON,
        );
    }

    #[test]
    fn content_type_gbk_decodes_chinese() {
        let html = "<title>中文标题</title><body>中文正文</body>";
        let bytes = gbk_bytes(html);
        for content_type in [
            "text/html; charset=gbk",
            "text/html; charset=\"GBK\"",
            "text/html; charset='gbk'",
        ] {
            assert_decodes_chinese(&bytes, &[("Content-Type", content_type)], "中文标题");
            assert_decodes_chinese(&bytes, &[("Content-Type", content_type)], "中文正文");
        }
    }

    #[test]
    fn meta_charset_gb2312_decodes_chinese() {
        let html = "<html><head><meta charset=\"gb2312\"></head><title>中文标题</title><body>中文正文</body></html>";
        let bytes = gbk_bytes(html);
        assert_decodes_chinese(&bytes, &[], "中文标题");
        assert_decodes_chinese(&bytes, &[], "中文正文");
    }

    #[test]
    fn meta_http_equiv_gbk_decodes_chinese() {
        let html = "<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=GBK\"></head><title>门户标题</title><body>门户正文</body></html>";
        assert_decodes_chinese(&gbk_bytes(html), &[], "门户标题");
        assert_decodes_chinese(&gbk_bytes(html), &[], "门户正文");
    }

    #[test]
    fn utf8_bom_wins_over_a_wrong_charset() {
        let mut bytes = b"\xEF\xBB\xBF".to_vec();
        bytes.extend_from_slice("<title>中文标题</title><body>正文</body>".as_bytes());
        assert_decodes_chinese(
            &bytes,
            &[("content-type", "text/html; charset=gbk")],
            "中文标题",
        );
        let text = decode_page(
            &bytes,
            &header_pairs(&[("content-type", "text/html; charset=gbk")]),
        )
        .expect("UTF-8 BOM 应能解码");
        assert!(!text.starts_with('\u{FEFF}'), "BOM 不应留在正文里");
    }

    #[test]
    fn utf16le_bom_decodes_chinese() {
        let bytes = utf16le_with_bom("<title>中文标题</title><body>正文</body>");
        assert!(bytes.starts_with(&[0xFF, 0xFE]), "夹具应带 UTF-16LE BOM");
        assert_decodes_chinese(&bytes, &[], "中文标题");
    }

    #[test]
    fn unknown_charset_falls_back_to_utf8() {
        let html = "<title>中文标题</title><body>正文</body>";
        assert_decodes_chinese(
            html.as_bytes(),
            &[("content-type", "text/html; charset=foo")],
            "中文标题",
        );
    }

    #[test]
    fn binary_noise_is_unreadable() {
        let bytes: Vec<u8> = (0..3000).map(|index| 0x80 + (index % 128) as u8).collect();
        assert!(!bytes.starts_with(&[0x1f, 0x8b]));
        assert_decode_fail(&bytes, &[], UNREADABLE_REASON);
    }

    #[test]
    fn normal_chinese_page_is_not_flagged_as_garbled() {
        let html = "<title>正常中文标题</title><body>这是正常的一段中文，用来确认乱码保护不会误伤。</body>";
        assert_decodes_chinese(html.as_bytes(), &[], "正常中文标题");
        assert_decodes_chinese(html.as_bytes(), &[], "不会误伤");
    }

    #[test]
    fn security_check_interstitial_fails() {
        let html = "<html><head></head><body><div>火山引擎 正在进行安全检测，请耐心等待</div></body></html>";
        let fetcher = SeqFetcher::new(vec![Ok(html_page(html))]);
        match fetch_html(&fetcher, "https://www.36kr.com/p/1") {
            Outcome::Failed { reason } => assert_eq!(reason, CHALLENGE_REASON),
            other => panic!("人机验证页应失败：{other:?}"),
        }
    }

    #[test]
    fn cloudflare_just_a_moment_fails() {
        let html = "<html><head><title>Just a moment...</title></head><body><p>Please wait</p></body></html>";
        let fetcher = SeqFetcher::new(vec![Ok(html_page(html))]);
        match fetch_html(&fetcher, "https://example.com/cdn-cgi") {
            Outcome::Failed { reason } => assert_eq!(reason, CHALLENGE_REASON),
            other => panic!("Cloudflare 拦截页应失败：{other:?}"),
        }
    }

    #[test]
    fn long_article_mentioning_captcha_succeeds() {
        let paragraph = "这是一篇正常的技术文章。";
        let mut body = paragraph.repeat(80);
        body.push_str("文中顺便提到验证码和安全检测，但页面本身不是拦截页。");
        assert!(body.chars().count() > 600, "{}", body.chars().count());
        let html = format!("<title>正常文章</title><article>{body}</article>");
        let fetcher = SeqFetcher::new(vec![Ok(html_page(&html))]);
        match fetch_html(&fetcher, "https://blog.example/long") {
            Outcome::Done { title, excerpt, .. } => {
                assert_eq!(title, "正常文章");
                assert!(excerpt.contains("验证码"), "{excerpt}");
                assert!(excerpt.contains("安全检测"), "{excerpt}");
            }
            other => panic!("长文提到验证码仍应读成功：{other:?}"),
        }
    }

    #[test]
    fn short_page_with_normal_title_succeeds() {
        let html = "<title>今天的笔记</title><body>只有一句短话</body>";
        let fetcher = SeqFetcher::new(vec![Ok(html_page(html))]);
        match fetch_html(&fetcher, "https://example.com/note") {
            Outcome::Done { title, excerpt, .. } => {
                assert_eq!(title, "今天的笔记");
                assert_eq!(excerpt, "只有一句短话");
            }
            other => panic!("正常短页面应读成功：{other:?}"),
        }
    }

    #[test]
    fn accept_encoding_requests_gzip_and_deflate_only() {
        let args = safe_curl_arguments(
            "https://example.com/a",
            "/tmp/fsl-inbox-headers.txt",
            "example.com:443:1.1.1.1",
            UA_SAFARI,
        );
        assert!(args
            .windows(2)
            .any(|pair| { pair[0] == "-H" && pair[1] == "Accept-Encoding: gzip, deflate" }));
        assert!(!args.iter().any(|arg| arg == "-L"));
        assert!(!args.iter().any(|arg| arg == "--compressed"));
        for arg in &args {
            let lower = arg.to_ascii_lowercase();
            assert!(!lower.contains("cookie"), "{arg}");
            assert!(!lower.contains("authorization"), "{arg}");
        }
    }

    // ───────── 安全审查（2026-10-04）后补的测试 ─────────

    #[test]
    fn curl_arguments_turn_off_globbing_and_proxies() {
        let args = safe_curl_arguments(
            "https://evil.example/a[1-100000]",
            "/tmp/h.txt",
            "evil.example:443:93.184.216.34",
            UA_SAFARI,
        );
        // 没有 -g，curl 会把 [1-100000] 展开成十万次请求
        assert!(args.iter().any(|arg| arg == "-g"), "{args:?}");
        let proxy = args
            .iter()
            .position(|arg| arg == "--noproxy")
            .expect("应带 --noproxy");
        assert_eq!(args[proxy + 1], "*");
        // 网址仍然在 `--` 之后，当作一个普通参数
        let dashes = args.iter().position(|arg| arg == "--").expect("应有 --");
        assert_eq!(args[dashes + 1], "https://evil.example/a[1-100000]");
    }

    fn elapsed_under(limit_secs: u64, run: impl FnOnce()) {
        let started = std::time::Instant::now();
        run();
        assert!(
            started.elapsed() < Duration::from_secs(limit_secs),
            "解析用了 {:?}，应该是线性的",
            started.elapsed()
        );
    }

    #[test]
    fn unterminated_tags_are_parsed_in_linear_time() {
        // 修复前：10 万字节的 `<a` 重复要 12 秒（每个都扫到文末），6 MB 要十几个小时
        let html = "<a".repeat(300_000);
        elapsed_under(5, || {
            let page = extract_web(&html, "https://example.com/bomb");
            let _ = page.title;
            assert!(github_repos_in(&html, "https://example.com/bomb").is_empty());
            // 没结束的标签吞掉了剩下的整页：丢掉（浏览器也不显示），不再推给后面
            assert!(remove_hidden(&html).len() <= html.len());
        });
        // 引号没配对：同样是一直扫到末尾
        let quoted = "<a title=\"x ".repeat(100_000);
        elapsed_under(5, || {
            let _ = extract_web(&quoted, "https://example.com/quote");
        });
    }

    #[test]
    fn unclosed_chrome_tags_do_not_make_the_stripper_quadratic() {
        let mut html = "<nav>".repeat(50_000);
        html.push_str("<p>推荐 https://github.com/a/b</p>");
        elapsed_under(5, || {
            assert_eq!(
                github_repos_in(&html, "https://blog.example/nav"),
                vec!["a/b".to_owned()]
            );
        });
    }

    #[test]
    fn entity_decoding_only_looks_a_few_bytes_ahead() {
        let text = "&".repeat(800_000);
        elapsed_under(5, || {
            assert_eq!(decode_entities(&text).len(), 800_000);
        });
        assert_eq!(decode_entities("a &amp; b &#x4e2d; c"), "a & b 中 c");
        assert_eq!(decode_entities("&amp"), "&amp", "没有分号不算实体");
    }

    #[test]
    fn backslash_before_a_multibyte_char_does_not_panic_and_keeps_the_text() {
        // 修复前：`\é` 让下标落在字符中间，下一次切片 panic，收集箱读取整个卡死
        let html = r#"<script>var nickname = "a\é b\字 c";</script>"#;
        let name = js_assigned_string_with(html, "nickname", b"=").expect("应读到值");
        assert_eq!(name, "a\\é b\\字 c");
        assert_eq!(
            js_assigned_string_with(r#"nickname='\é'"#, "nickname", b"="),
            Some("\\é".to_owned())
        );
        // 引号还没结束就到了末尾：不 panic，返回 None
        assert_eq!(
            js_assigned_string_with("nickname = 'abc\\", "nickname", b"="),
            None
        );
    }

    #[test]
    fn wechat_author_comes_from_scripts_only() {
        let html = concat!(
            "<html><head><title>文章</title></head><body>",
            "<div id=\"js_content\"><p>nickname = 'Tencent Official'</p>",
            "<p>nickname='\\é'</p></div>",
            "<script>var nickname = htmlDecode(\"真账号\");</script>",
            "</body></html>",
        );
        let page = extract_web(html, "https://mp.weixin.qq.com/s/abc");
        assert_eq!(page.author, "真账号", "正文里写的 nickname 不能当作者");
        // 页面里没有 script 给出账号名时，不能从正文里捡
        let spoofed = concat!(
            "<html><head><title>文章</title></head><body>",
            "<div id=\"js_content\"><p>nickname = 'Tencent Official'</p></div>",
            "</body></html>",
        );
        let page = extract_web(spoofed, "https://mp.weixin.qq.com/s/abc");
        assert_ne!(page.author, "Tencent Official");
    }

    #[test]
    fn invisible_format_characters_are_removed_but_emoji_joiners_stay() {
        let text = "a\u{202E}b\u{200B}c\u{FEFF}d\u{2066}e\u{E0041}f\u{00AD}g";
        assert_eq!(strip_controls(text), "abcdefg");
        // 零宽连接符是表情组合的一部分，去掉会拆散表情
        let family = "👨\u{200D}👩\u{200D}👧";
        assert_eq!(strip_controls(family), family);
        assert_eq!(
            strip_controls("换行\n制表\t保留\u{0007}响铃去掉"),
            "换行\n制表\t保留响铃去掉"
        );
    }

    struct PanicFetcher;

    impl PageFetcher for PanicFetcher {
        fn get(&self, _url: &str) -> Result<PageResponse, FetchProblem> {
            panic!("故意崩溃");
        }
    }

    #[test]
    fn a_panic_while_reading_becomes_a_failed_row_not_a_stuck_pending_one() {
        let db = TempDb::new("panic-row");
        let url = "https://example.com/boom";
        insert_item(&db, "boom", url, url, "webpage", "2026-10-01T00:00:00.000Z");
        let summary =
            fetch_pending(&db.path, &PanicFetcher, None, 20).expect("不能把 panic 抛出来");
        assert_eq!(summary.failed, 1);
        assert_eq!(
            summary.remaining, 0,
            "不能一直 pending，否则每次打开都再崩一次"
        );
        let row = stored(&db, "boom");
        assert_eq!(row.fetch_status, "failed");
        assert!(row.fetch_error.contains("没读出来"), "{}", row.fetch_error);
    }

    #[test]
    fn bounded_runs_give_up_on_slow_or_panicking_jobs() {
        assert_eq!(run_bounded(Duration::from_secs(2), || 7), Some(7));
        let started = std::time::Instant::now();
        assert_eq!(
            run_bounded(Duration::from_millis(50), || {
                std::thread::sleep(Duration::from_millis(600));
                1
            }),
            None
        );
        assert!(started.elapsed() < Duration::from_millis(400));
        assert_eq!(
            run_bounded(Duration::from_secs(2), || -> i32 { panic!("故意崩溃") }),
            None
        );
    }

    #[test]
    fn a_stray_quote_inside_a_tag_does_not_swallow_the_rest_of_the_page() {
        // 少数派首页真实写法：content="…app-argument="" /> 里多出一个引号。
        // 修复前整页被当成引号里的内容，摘要变成一堆标签源码。
        let html = concat!(
            "<!DOCTYPE html><html><head><title>少数派</title>",
            "<meta name=\"apple-itunes-app\" content=\"app-id=1191720421, app-argument=\"\" />",
            "<meta name=\"description\" content=\"高效工作，品质生活\" />",
            "</head><body><article><p>正文第一段</p></article></body></html>",
        );
        let page = extract_web(html, "https://sspai.example/");
        assert_eq!(page.title, "少数派");
        assert!(page.excerpt.contains("高效工作"), "{}", page.excerpt);
        assert!(!page.excerpt.contains("<meta"), "{}", page.excerpt);
        // 没加引号的值里的撇号（it's）也不能开一个引号
        let apostrophe = "<a title=it's href=\"https://github.com/o/n\">x</a><p>后面的正文</p>";
        assert_eq!(
            github_repos_in(apostrophe, "https://blog.example/q"),
            vec!["o/n".to_owned()]
        );
        let text = html_fragment_to_text(apostrophe, false);
        assert!(text.contains("后面的正文"), "{text}");
        assert!(!text.contains('<'), "标签不能漏成文字：{text}");
    }

    #[test]
    fn a_really_unterminated_quoted_value_still_ends_the_scan_once() {
        // 引号值开了头却一直没有结束：和浏览器一样，剩下的都算这个标签的内容，扫一次就停
        let html = format!("<p>前文</p><a href=\"{}", "x".repeat(200_000));
        elapsed_under(5, || {
            let text = html_fragment_to_text(&html, false);
            assert!(text.starts_with("前文"), "{}", &text[..text.len().min(80)]);
        });
    }

    #[test]
    fn less_than_signs_in_text_are_not_tags() {
        // 修复前：`<3`、`< b` 被当成标签，一路吞到下一个 `>`，中间的仓库网址丢了
        let html = "<article><p>I <3 you https://github.com/o/n</p></article>";
        assert_eq!(
            github_repos_in(html, "https://blog.example/lt"),
            vec!["o/n".to_owned()]
        );
        let spaced = "<article><p>a < b and github.com/p/q</p></article>";
        assert_eq!(
            github_repos_in(spaced, "https://blog.example/lt2"),
            vec!["p/q".to_owned()]
        );
        assert!(html_fragment_to_text(html, false).contains("I <3 you"));
    }

    #[test]
    fn attribute_parsing_skips_junk_bytes_and_still_reads_later_attributes() {
        let attrs = parse_attrs(r#"@click="go()" class="x" "stray" href="https://github.com/o/n""#);
        assert!(
            attrs
                .iter()
                .any(|(name, value)| name == "href" && value == "https://github.com/o/n"),
            "{attrs:?}"
        );
        let html = r#"<article><a @click="go()" href="https://github.com/o/n">项目</a></article>"#;
        assert_eq!(
            github_repos_in(html, "https://blog.example/attrs"),
            vec!["o/n".to_owned()]
        );
    }

    #[test]
    fn an_unterminated_tag_does_not_leak_page_source_into_the_excerpt() {
        let html = concat!(
            "<html><head><title>标题</title></head><body><p>正常的第一段</p>",
            "<a href=\"foo>被吞掉的内容</a><style>.secret{color:red}</style>",
            "<script>var token = 'abc';</script></body></html>",
        );
        let page = extract_web(html, "https://blog.example/leak");
        assert!(page.excerpt.contains("正常的第一段"), "{}", page.excerpt);
        assert!(!page.excerpt.contains("secret"), "{}", page.excerpt);
        assert!(!page.excerpt.contains("var token"), "{}", page.excerpt);
        assert!(!page.excerpt.contains('<'), "{}", page.excerpt);
    }

    #[test]
    fn many_site_links_do_not_crowd_out_the_repos_in_an_article() {
        // 400 个别的链接排在前面，仓库在后面：「一段文字最多 300 条链接」只管聊天粘贴，不管文章里提取仓库
        let mut html = String::from("<article>");
        for index in 0..400 {
            html.push_str(&format!("<a href=\"https://example.com/p{index}\">x</a>"));
        }
        html.push_str("<p>https://github.com/late/one https://github.com/late/two</p></article>");
        assert_eq!(
            github_repos_in(&html, "https://blog.example/many"),
            vec!["late/one".to_owned(), "late/two".to_owned()]
        );
    }
}
