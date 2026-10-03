//! 不带令牌的联网层。curl 是阻塞的，调用方必须放进 `spawn_blocking`。
//! 第一个参数 `-q` 让 curl 不读用户配置，避免配置文件夹带请求头。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchErrorKind {
    Timeout,
    Network,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchError {
    pub kind: FetchErrorKind,
    pub message: String,
}

impl FetchError {
    fn timeout() -> Self {
        Self {
            kind: FetchErrorKind::Timeout,
            message: "联网超时".to_owned(),
        }
    }

    fn network() -> Self {
        Self {
            kind: FetchErrorKind::Network,
            message: "网络连不上".to_owned(),
        }
    }

    fn other(message: &str) -> Self {
        Self {
            kind: FetchErrorKind::Other,
            message: message.to_owned(),
        }
    }
}

/// 响应头名已转成小写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

pub trait HttpFetcher {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, FetchError>;
}

#[derive(Debug, Default)]
pub struct CurlFetcher;

impl HttpFetcher for CurlFetcher {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, FetchError> {
        let dump = HeaderDump::create()?;
        let args = curl_arguments(url, headers, &dump.path_arg()?);
        let output = std::process::Command::new("curl")
            .args(&args)
            .output()
            .map_err(|_| FetchError::other("联网程序启动失败"))?;
        let code = output
            .status
            .code()
            .ok_or_else(|| FetchError::other("联网失败"))?;
        if code != 0 {
            return Err(fetch_error_for_exit(code));
        }
        let header_text =
            std::fs::read_to_string(&dump.0).map_err(|_| FetchError::other("读不到响应头"))?;
        let (status, response_headers) = parse_header_dump(&header_text)?;
        let body =
            String::from_utf8(output.stdout).map_err(|_| FetchError::other("响应不是 UTF-8"))?;
        Ok(HttpResponse {
            status,
            headers: response_headers,
            body,
        })
    }
}

struct HeaderDump(PathBuf);

impl HeaderDump {
    fn create() -> Result<Self, FetchError> {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        Ok(Self(
            std::env::temp_dir().join(format!("fsl-curl-headers-{nanos}-{seq}.txt")),
        ))
    }

    fn path_arg(&self) -> Result<String, FetchError> {
        self.0
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| FetchError::other("临时文件路径无效"))
    }
}

impl Drop for HeaderDump {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// 拼 curl 参数。每个参数单独一项，不经过 shell，也不真的执行。
pub fn curl_arguments(url: &str, headers: &[(&str, &str)], header_dump: &str) -> Vec<String> {
    let mut args = vec![
        "-q".to_owned(),
        "-sS".to_owned(),
        "-L".to_owned(),
        // 只走 https：重定向也不许跳到别的协议（ftp、gopher 等）
        "--proto".to_owned(),
        "=https".to_owned(),
        "--proto-redir".to_owned(),
        "=https".to_owned(),
        "--max-redirs".to_owned(),
        "5".to_owned(),
        "--max-time".to_owned(),
        "20".to_owned(),
        "--connect-timeout".to_owned(),
        "8".to_owned(),
        "--max-filesize".to_owned(),
        "2000000".to_owned(),
        "-A".to_owned(),
        "fox-stars-lab".to_owned(),
        "-D".to_owned(),
        header_dump.to_owned(),
    ];
    for (name, value) in headers {
        args.push("-H".to_owned());
        args.push(format!("{name}: {value}"));
    }
    args.push("--".to_owned());
    args.push(url.to_owned());
    args
}

fn fetch_error_for_exit(code: i32) -> FetchError {
    match classify_curl_exit(code) {
        FetchErrorKind::Timeout => FetchError::timeout(),
        FetchErrorKind::Network => FetchError::network(),
        FetchErrorKind::Other => FetchError::other("联网失败"),
    }
}

fn classify_curl_exit(code: i32) -> FetchErrorKind {
    match code {
        28 => FetchErrorKind::Timeout,
        // 6 解析不了主机，7 连不上，35 TLS 握手失败；5/52/55/56 是同类的网络失败。
        5 | 6 | 7 | 35 | 52 | 55 | 56 => FetchErrorKind::Network,
        _ => FetchErrorKind::Other,
    }
}

/// 跟随重定向时，头文件里会有多段响应，只取最后一段的状态和头。
fn parse_header_dump(dump: &str) -> Result<(u16, Vec<(String, String)>), FetchError> {
    let normalized = dump.replace("\r\n", "\n");
    let block = normalized
        .split("\n\n")
        .map(str::trim)
        .filter(|block| block.starts_with("HTTP/"))
        .last()
        .ok_or_else(|| FetchError::other("读不到响应状态"))?;
    let mut lines = block.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| FetchError::other("读不到响应状态"))?;
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

fn parse_status_line(line: &str) -> Result<u16, FetchError> {
    let mut parts = line.split_whitespace();
    let version = parts.next().unwrap_or("");
    if !version.starts_with("HTTP/") {
        return Err(FetchError::other("读不到响应状态"));
    }
    parts
        .next()
        .and_then(|code| code.parse::<u16>().ok())
        .filter(|code| (100..600).contains(code))
        .ok_or_else(|| FetchError::other("读不到响应状态"))
}

/// 只放行字母数字和 `-_.~`，其余按 UTF-8 字节百分号编码（大写）。
pub fn url_encode(input: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push('%');
                encoded.push(HEX[(byte >> 4) as usize] as char);
                encoded.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    encoded
}

/// 403/429 且剩余次数为 0 时，用重置时间减 `now_epoch`。
/// 没有重置时间但有 `retry-after`（或 429 没给剩余次数）时用等待秒数。
pub fn rate_limit_wait_secs(response: &HttpResponse, now_epoch: i64) -> Option<u64> {
    if response.status != 403 && response.status != 429 {
        return None;
    }
    let remaining = header_value(response, "x-ratelimit-remaining");
    let reset = header_value(response, "x-ratelimit-reset");
    let retry_after = header_value(response, "retry-after");
    if remaining.map(str::trim) == Some("0") {
        if let Some(wait) = reset_wait(reset, now_epoch) {
            return Some(wait);
        }
        if let Some(wait) = delay_seconds(retry_after) {
            return Some(wait);
        }
        return None;
    }
    if remaining.is_none() {
        return delay_seconds(retry_after);
    }
    None
}

fn header_value<'a>(response: &'a HttpResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .rev()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn reset_wait(reset: Option<&str>, now_epoch: i64) -> Option<u64> {
    let reset = reset?.trim().parse::<i64>().ok()?;
    Some(reset.saturating_sub(now_epoch).max(0) as u64)
}

fn delay_seconds(value: Option<&str>) -> Option<u64> {
    let value = value?.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<u64>().ok()
}

pub fn format_unix_time(epoch: i64) -> Result<String, String> {
    let time = OffsetDateTime::from_unix_timestamp(epoch).map_err(|_| "时间无效".to_owned())?;
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        time.year(),
        u8::from(time.month()),
        time.day(),
        time.hour(),
        time.minute(),
        time.second()
    ))
}

pub fn since_date(now_epoch: i64, days: i64) -> Result<String, String> {
    if days < 0 {
        return Err("搜索词配置的天数不对".to_owned());
    }
    let now = OffsetDateTime::from_unix_timestamp(now_epoch).map_err(|_| "时间无效".to_owned())?;
    let date = now
        .date()
        .checked_sub(Duration::days(days))
        .ok_or_else(|| "日期超出范围".to_owned())?;
    Ok(format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    ))
}

/// 接受 `YYYY-MM-DDTHH:MM:SSZ`，小数秒会被忽略。
pub fn parse_unix_time(text: &str) -> Option<i64> {
    let text = text.trim().trim_end_matches('Z');
    let (date, time) = text.split_once('T')?;
    let mut date_parts = date.split('-');
    let year: i32 = date_parts.next()?.parse().ok()?;
    let month: u8 = date_parts.next()?.parse().ok()?;
    let day: u8 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() {
        return None;
    }
    let time = time.split('.').next().unwrap_or(time);
    let mut time_parts = time.split(':');
    let hour: u8 = time_parts.next()?.parse().ok()?;
    let minute: u8 = time_parts.next()?.parse().ok()?;
    let second: u8 = time_parts.next()?.parse().ok()?;
    if time_parts.next().is_some() {
        return None;
    }
    let month = time::Month::try_from(month).ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    let time = time::Time::from_hms(hour, minute, second).ok()?;
    Some(
        time::PrimitiveDateTime::new(date, time)
            .assume_utc()
            .unix_timestamp(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_encodes_cjk_space_plus_and_colon() {
        assert_eq!(url_encode("a b"), "a%20b");
        assert_eq!(url_encode("a+b"), "a%2Bb");
        assert_eq!(url_encode("a:b"), "a%3Ab");
        assert_eq!(url_encode("a b+c:d"), "a%20b%2Bc%3Ad");
        assert_eq!(url_encode("abcXYZ-_.~09"), "abcXYZ-_.~09");
        assert_eq!(url_encode("小红书"), "%E5%B0%8F%E7%BA%A2%E4%B9%A6");
        assert!(!url_encode("a b").contains('+'));
    }

    #[test]
    fn rate_limit_wait_secs_on_403_remaining_zero_uses_reset() {
        let now = 1_700_000_000;
        let response = HttpResponse {
            status: 403,
            headers: vec![
                ("X-RateLimit-Remaining".to_owned(), "0".to_owned()),
                ("X-RateLimit-Reset".to_owned(), (now + 15).to_string()),
                ("retry-after".to_owned(), "99".to_owned()),
            ],
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&response, now), Some(15));

        let already = HttpResponse {
            status: 403,
            headers: vec![
                ("x-ratelimit-remaining".to_owned(), "0".to_owned()),
                ("x-ratelimit-reset".to_owned(), now.to_string()),
            ],
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&already, now), Some(0));

        let retry_only = HttpResponse {
            status: 403,
            headers: vec![
                ("x-ratelimit-remaining".to_owned(), "0".to_owned()),
                ("retry-after".to_owned(), "8".to_owned()),
            ],
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&retry_only, now), Some(8));
    }

    #[test]
    fn rate_limit_wait_secs_on_429_uses_retry_after() {
        let now = 1_700_000_000;
        let missing_remaining = HttpResponse {
            status: 429,
            headers: vec![("Retry-After".to_owned(), "20".to_owned())],
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&missing_remaining, now), Some(20));

        let exhausted = HttpResponse {
            status: 429,
            headers: vec![
                ("x-ratelimit-remaining".to_owned(), "0".to_owned()),
                ("retry-after".to_owned(), " 8 ".to_owned()),
            ],
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&exhausted, now), Some(8));
    }

    #[test]
    fn rate_limit_wait_secs_none_when_not_limited() {
        let now = 1_700_000_000;
        let ok = HttpResponse {
            status: 200,
            headers: vec![
                ("x-ratelimit-remaining".to_owned(), "0".to_owned()),
                ("x-ratelimit-reset".to_owned(), (now + 10).to_string()),
            ],
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&ok, now), None);

        let still_left = HttpResponse {
            status: 403,
            headers: vec![
                ("x-ratelimit-remaining".to_owned(), "5".to_owned()),
                ("x-ratelimit-reset".to_owned(), (now + 10).to_string()),
                ("retry-after".to_owned(), "3".to_owned()),
            ],
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&still_left, now), None);

        let bare = HttpResponse {
            status: 429,
            headers: Vec::new(),
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&bare, now), None);

        let no_clock = HttpResponse {
            status: 403,
            headers: vec![("x-ratelimit-remaining".to_owned(), "0".to_owned())],
            body: String::new(),
        };
        assert_eq!(rate_limit_wait_secs(&no_clock, now), None);
    }

    #[test]
    fn curl_arguments_are_separate_and_include_limits() {
        let args = curl_arguments(
            "https://api.github.com/search/repositories?q=a",
            &[
                ("Accept", "application/vnd.github+json"),
                ("X-GitHub-Api-Version", "2022-11-28"),
            ],
            "/tmp/fsl-headers.txt",
        );
        assert_eq!(
            args,
            vec![
                "-q".to_owned(),
                "-sS".to_owned(),
                "-L".to_owned(),
                "--proto".to_owned(),
                "=https".to_owned(),
                "--proto-redir".to_owned(),
                "=https".to_owned(),
                "--max-redirs".to_owned(),
                "5".to_owned(),
                "--max-time".to_owned(),
                "20".to_owned(),
                "--connect-timeout".to_owned(),
                "8".to_owned(),
                "--max-filesize".to_owned(),
                "2000000".to_owned(),
                "-A".to_owned(),
                "fox-stars-lab".to_owned(),
                "-D".to_owned(),
                "/tmp/fsl-headers.txt".to_owned(),
                "-H".to_owned(),
                "Accept: application/vnd.github+json".to_owned(),
                "-H".to_owned(),
                "X-GitHub-Api-Version: 2022-11-28".to_owned(),
                "--".to_owned(),
                "https://api.github.com/search/repositories?q=a".to_owned(),
            ]
        );
        assert!(args
            .iter()
            .all(|arg| !arg.contains("&&") && !arg.contains(';')));
        assert_eq!(args.iter().filter(|arg| *arg == "-H").count(), 2);
    }

    #[test]
    fn header_dump_keeps_last_hop_only() {
        let dump = "\
HTTP/1.1 302 Found\r\n\
location: https://example.com/next\r\n\
\r\n\
HTTP/2 200\r\n\
content-type: application/json\r\n\
x-ratelimit-remaining: 9\r\n\
\r\n";
        let (status, headers) = parse_header_dump(dump).expect("应能切开最后一段");
        assert_eq!(status, 200);
        assert_eq!(
            headers,
            vec![
                ("content-type".to_owned(), "application/json".to_owned()),
                ("x-ratelimit-remaining".to_owned(), "9".to_owned()),
            ]
        );
        assert!(headers.iter().all(|(name, _)| name != "location"));
    }

    #[test]
    fn curl_exit_timeout_network_and_other_hide_the_command() {
        let timeout = fetch_error_for_exit(28);
        assert_eq!(timeout.kind, FetchErrorKind::Timeout);
        assert_eq!(timeout.message, "联网超时");
        for code in [5, 6, 7, 35, 52, 55, 56] {
            let error = fetch_error_for_exit(code);
            assert_eq!(error.kind, FetchErrorKind::Network, "{code}");
            assert_eq!(error.message, "网络连不上");
        }
        let other = fetch_error_for_exit(22);
        assert_eq!(other.kind, FetchErrorKind::Other);
        assert_eq!(other.message, "联网失败");
        for error in [timeout, fetch_error_for_exit(6), other] {
            assert!(!error.message.contains('-'));
            assert!(!error.message.contains("http"));
        }
    }

    #[test]
    fn unix_time_round_trip_ignores_fraction() {
        let epoch = time::Date::from_calendar_date(2026, time::Month::October, 3)
            .expect("日期")
            .with_hms(0, 0, 0)
            .expect("时间")
            .assume_utc()
            .unix_timestamp();
        let text = format_unix_time(epoch).expect("应能格式化");
        assert_eq!(text, "2026-10-03T00:00:00Z");
        assert_eq!(parse_unix_time(&text), Some(epoch));
        assert_eq!(parse_unix_time("2026-10-03T00:00:00.123Z"), Some(epoch));
        assert_eq!(since_date(epoch, 120).expect("应能回推"), "2026-06-05");
    }
}
