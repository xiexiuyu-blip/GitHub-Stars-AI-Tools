//! 阻塞联网层，调用方必须放进 `spawn_blocking`。
//! curl 仅用于无认证请求，`-q` 禁止读取用户配置。

use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};

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

const MAX_BODY_BYTES: u64 = 2_000_000;
const REQUEST_TIMEOUT: StdDuration = StdDuration::from_secs(20);

/// 令牌通过请求头传递，不出现在进程参数中。
pub struct ReqwestFetcher;

fn build_client() -> Result<reqwest::blocking::Client, FetchError> {
    reqwest::blocking::Client::builder()
        .connect_timeout(StdDuration::from_secs(8))
        .timeout(REQUEST_TIMEOUT)
        .user_agent("fox-stars-lab")
        .https_only(true)
        .redirect(redirect_policy())
        .build()
        .map_err(|_| FetchError::other("联网初始化失败"))
}

fn timed_get(
    client: &reqwest::blocking::Client,
    url: impl reqwest::IntoUrl,
    timeout: StdDuration,
) -> reqwest::blocking::RequestBuilder {
    // blocking 客户端的 builder 超时分别作用于等待响应和每次读取；
    // 请求级超时还会传到内部异步请求，整个连接、重定向和响应体共享同一截止时间。
    client.get(url).timeout(timeout)
}

fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        match redirect_error(attempt.url(), attempt.previous().len()) {
            Some(message) => attempt.error(message),
            None => attempt.follow(),
        }
    })
}

fn redirect_error(url: &reqwest::Url, previous_count: usize) -> Option<&'static str> {
    if !safe_redirect_target(url) {
        Some("重定向地址无效")
    } else if previous_count > 5 {
        Some("重定向次数过多")
    } else {
        None
    }
}

fn safe_redirect_target(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
}

fn validate_request(url: &str, headers: &[(&str, &str)]) -> Result<reqwest::Url, FetchError> {
    let url = reqwest::Url::parse(url).map_err(|_| FetchError::other("联网地址无效"))?;
    if !safe_redirect_target(&url) {
        return Err(FetchError::other("联网地址无效"));
    }
    for (name, _) in headers {
        if name.eq_ignore_ascii_case("proxy-authorization")
            || (name.eq_ignore_ascii_case("authorization")
                && (url.host_str() != Some("api.github.com")
                    || url.port_or_known_default() != Some(443)))
        {
            return Err(FetchError::other("不允许向此地址发送认证信息"));
        }
    }
    Ok(url)
}

fn fetch_error_for_reqwest(error: &reqwest::Error) -> FetchError {
    if error.is_timeout() {
        FetchError::timeout()
    } else if error.is_connect() {
        FetchError::network()
    } else {
        FetchError::other("联网失败")
    }
}

fn read_body(reader: impl Read) -> Result<String, FetchError> {
    let mut body = Vec::new();
    reader
        .take(MAX_BODY_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::TimedOut
                || error
                    .get_ref()
                    .and_then(|source| source.downcast_ref::<reqwest::Error>())
                    .is_some_and(reqwest::Error::is_timeout)
            {
                FetchError::timeout()
            } else {
                FetchError::network()
            }
        })?;
    if body.len() as u64 > MAX_BODY_BYTES {
        return Err(FetchError::other("响应内容过大"));
    }
    String::from_utf8(body).map_err(|_| FetchError::other("响应不是 UTF-8"))
}

impl HttpFetcher for ReqwestFetcher {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, FetchError> {
        let url = validate_request(url, headers)?;
        let mut request = timed_get(&build_client()?, url, REQUEST_TIMEOUT);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        // reqwest 在跨 host、端口或协议重定向时会删除 Authorization 等敏感头。
        let response = request
            .send()
            .map_err(|error| fetch_error_for_reqwest(&error))?;
        let status = response.status().as_u16();
        let response_headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                value
                    .to_str()
                    .map(|value| (name.as_str().to_owned(), value.to_owned()))
                    .map_err(|_| FetchError::other("读不到响应头"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let body = read_body(response)?;
        Ok(HttpResponse {
            status,
            headers: response_headers,
            body,
        })
    }
}

#[derive(Debug, Default)]
pub struct CurlFetcher;

impl HttpFetcher for CurlFetcher {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, FetchError> {
        if headers.iter().any(|(name, _)| {
            name.eq_ignore_ascii_case("authorization")
                || name.eq_ignore_ascii_case("proxy-authorization")
        }) {
            return Err(FetchError::other("禁止通过 curl 进程参数传递认证头"));
        }
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
    fn authentication_is_restricted_before_network_access() {
        let auth = [("Authorization", "Bearer example")];
        assert!(validate_request("https://api.github.com/search", &auth).is_ok());
        for url in [
            "https://example.com/",
            "https://api.github.com.example.com/",
            "https://api.github.com:8443/",
            "http://api.github.com/",
            "https://user:pass@api.github.com/",
        ] {
            assert!(validate_request(url, &auth).is_err(), "{url}");
        }
        assert!(validate_request(
            "https://api.github.com/",
            &[("pRoXy-aUtHoRiZaTiOn", "secret")]
        )
        .is_err());
        for name in ["Authorization", "authorization", "PROXY-AUTHORIZATION"] {
            let error = CurlFetcher
                .get("https://api.github.com/", &[(name, "secret")])
                .expect_err("不得执行 curl");
            assert_eq!(error.message, "禁止通过 curl 进程参数传递认证头");
            assert!(!error.message.contains("secret"));
        }
    }

    #[test]
    fn redirect_target_must_be_https_without_embedded_credentials() {
        for url in ["http://example.com/", "https://user:pass@example.com/"] {
            assert!(!safe_redirect_target(
                &reqwest::Url::parse(url).expect("URL")
            ));
        }
        assert!(safe_redirect_target(
            &reqwest::Url::parse("https://example.com/path").expect("URL")
        ));
        let target = reqwest::Url::parse("https://example.com/path").expect("URL");
        assert_eq!(redirect_error(&target, 5), None);
        assert_eq!(redirect_error(&target, 6), Some("重定向次数过多"));
    }

    #[test]
    fn actual_redirect_policy_blocks_http_target() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("本地端口");
        let address = listener.local_addr().expect("地址");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("连接");
            stream
                .set_read_timeout(Some(StdDuration::from_secs(2)))
                .expect("超时");
            let mut request = [0; 1024];
            let _ = stream.read(&mut request).expect("请求");
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://example.com/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").expect("响应");
        });
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(redirect_policy())
            .timeout(StdDuration::from_secs(2))
            .build()
            .expect("客户端");
        let error = client
            .get(format!("http://{address}/"))
            .send()
            .expect_err("禁止跳转 HTTP");
        assert!(error.is_redirect());
        server.join().expect("服务结束");
    }

    #[test]
    fn actual_request_timeout_is_classified_without_leaking_url() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("本地端口");
        let address = listener.local_addr().expect("地址");
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("连接");
            std::thread::sleep(StdDuration::from_millis(250));
        });
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(StdDuration::from_millis(50))
            .build()
            .expect("客户端");
        let error = client
            .get(format!("http://{address}/secret"))
            .send()
            .expect_err("请求应超时");
        let mapped = fetch_error_for_reqwest(&error);
        assert_eq!(mapped.kind, FetchErrorKind::Timeout);
        assert!(!mapped.message.contains("secret"));
        server.join().expect("服务结束");
    }

    #[test]
    fn total_timeout_covers_redirect_and_slow_streaming_body() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("本地端口");
        let address = listener.local_addr().expect("地址");
        let server = std::thread::spawn(move || {
            let (mut redirect, _) = listener.accept().expect("初始请求");
            redirect
                .set_read_timeout(Some(StdDuration::from_secs(2)))
                .expect("超时");
            let mut request = [0; 1024];
            let _ = redirect.read(&mut request).expect("请求");
            std::thread::sleep(StdDuration::from_millis(90));
            redirect.write_all(b"HTTP/1.1 302 Found\r\nLocation: /stream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").expect("重定向");
            let (mut stream, _) = listener.accept().expect("重定向请求");
            stream
                .set_read_timeout(Some(StdDuration::from_secs(2)))
                .expect("超时");
            let _ = stream.read(&mut request).expect("请求");
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n")
                .expect("响应头");
            for _ in 0..100 {
                if stream.write_all(b"x").is_err() {
                    break;
                }
                std::thread::sleep(StdDuration::from_millis(25));
            }
        });
        // 本地 HTTP 只用于验证同一个请求级超时；生产客户端只允许 HTTPS。
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::limited(5))
            .timeout(StdDuration::from_secs(2))
            .build()
            .expect("客户端");
        let started = std::time::Instant::now();
        let response = timed_get(
            &client,
            format!("http://{address}/"),
            StdDuration::from_millis(300),
        )
        .send()
        .expect("应先收到响应头");
        let error = read_body(response).expect_err("持续送数据也应在总时限内中止");
        let elapsed = started.elapsed();
        assert_eq!(error.kind, FetchErrorKind::Timeout);
        assert!(elapsed < StdDuration::from_secs(2), "实际耗时：{elapsed:?}");
        server.join().expect("服务结束");
    }

    #[test]
    fn reqwest_strips_auth_on_cross_origin_redirect_and_does_not_restore_it() {
        use std::io::{Read as _, Write as _};
        fn read_headers(stream: &mut std::net::TcpStream) -> String {
            stream
                .set_read_timeout(Some(StdDuration::from_secs(2)))
                .expect("超时");
            let mut data = Vec::new();
            let mut byte = [0; 1];
            while !data.ends_with(b"\r\n\r\n") && data.len() < 8192 {
                stream.read_exact(&mut byte).expect("请求头");
                data.push(byte[0]);
            }
            String::from_utf8(data)
                .expect("请求头编码")
                .to_ascii_lowercase()
        }
        let first = std::net::TcpListener::bind("127.0.0.1:0").expect("端口");
        let second = std::net::TcpListener::bind("127.0.0.1:0").expect("端口");
        let first_addr = first.local_addr().expect("地址");
        let second_addr = second.local_addr().expect("地址");
        let server = std::thread::spawn(move || {
            let (mut initial, _) = first.accept().expect("初始请求");
            let initial_headers = read_headers(&mut initial);
            initial.write_all(b"HTTP/1.1 302 Found\r\nLocation: /same\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").expect("同源跳转");
            let (mut same, _) = first.accept().expect("同源请求");
            let same_headers = read_headers(&mut same);
            write!(same, "HTTP/1.1 302 Found\r\nLocation: http://{second_addr}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").expect("跨源跳转");
            let (mut other, _) = second.accept().expect("跨源请求");
            let other_headers = read_headers(&mut other);
            write!(other, "HTTP/1.1 302 Found\r\nLocation: http://{first_addr}/return\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").expect("跳回");
            let (mut returned, _) = first.accept().expect("返回请求");
            let returned_headers = read_headers(&mut returned);
            returned
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .expect("响应");
            (
                initial_headers,
                same_headers,
                other_headers,
                returned_headers,
            )
        });
        // 本地 HTTP 仅用于测试 reqwest 的跨源头清理，生产策略只放行 HTTPS。
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::limited(5))
            .timeout(StdDuration::from_secs(2))
            .build()
            .expect("客户端");
        client
            .get(format!("http://{first_addr}/"))
            .header("Authorization", "Bearer test-only")
            .send()
            .expect("请求完成");
        let (initial, same, other, returned) = server.join().expect("服务结束");
        assert!(initial.contains("authorization: bearer test-only"));
        assert!(same.contains("authorization: bearer test-only"));
        assert!(!other.contains("authorization:"));
        assert!(!returned.contains("authorization:"));
    }

    #[test]
    fn body_size_and_utf8_are_checked_while_reading() {
        assert_eq!(
            read_body(std::io::Cursor::new(vec![b'a'; MAX_BODY_BYTES as usize]))
                .expect("边界长度")
                .len(),
            MAX_BODY_BYTES as usize
        );
        assert_eq!(
            read_body(std::io::Cursor::new(vec![
                b'a';
                MAX_BODY_BYTES as usize + 1
            ]))
            .expect_err("超限")
            .message,
            "响应内容过大"
        );
        assert_eq!(
            read_body(std::io::Cursor::new(vec![0xff]))
                .expect_err("非 UTF-8")
                .message,
            "响应不是 UTF-8"
        );
        let timeout = std::io::Error::new(std::io::ErrorKind::TimedOut, "secret");
        struct Broken(std::io::Error);
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(self.0.kind(), "secret"))
            }
        }
        let error = read_body(Broken(timeout)).expect_err("超时");
        assert_eq!(error.kind, FetchErrorKind::Timeout);
        assert!(!error.message.contains("secret"));
    }

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
