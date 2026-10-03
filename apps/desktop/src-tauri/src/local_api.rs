//! 本机 HTTP 接口，给 CLI、web-search、微信工作台用。
//!
//! 安全模型（默认关闭，开启后也只在本机）：
//! - 只监听 127.0.0.1；
//! - 除 `GET /v1/health` 外都要 `Authorization: Bearer <令牌>`。
//!   完整令牌 256 位随机，存在 0600 权限的控制文件里，能访问全部接口。
//!   `POST /v1/capture` 另外接受一把独立的「收集专用令牌」（同样 256 位、同样 0600）。
//!   这把令牌只能往收集箱里放链接，访问其它接口返回 403。两把令牌互不影响。
//! - 校验 `Host`（防 DNS 重绑定）；带 `Origin` 头的请求一律拒绝（网页脚本发起的请求都会带），
//!   响应也不带任何 CORS 头，浏览器里的页面既写不进来也读不出去。
use crate::v2_ops;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub const HOST: &str = "127.0.0.1";
pub const PORT: u16 = 39217;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_CAPTURE_URLS: usize = 100;
const MAX_CAPTURE_URL_CHARS: usize = 2048;
const MAX_CAPTURE_TEXT_CHARS: usize = 50_000;
const MAX_CAPTURE_CONTEXT_CHARS: usize = 2_000;

struct Runtime {
    stop: std::sync::Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiStatus {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub listening: bool,
    pub has_token: bool,
}

pub fn loopback_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT)
}

pub fn refuse_non_loopback(addr: SocketAddr) -> Result<(), String> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err("本地 API 只能监听 127.0.0.1，不能对局域网或公网开放。".to_owned())
    }
}

fn control_path(database_path: &Path) -> PathBuf {
    database_path.with_file_name("fox-stars-lab.local-api.json")
}

fn read_control(database_path: &Path) -> Value {
    std::fs::read_to_string(control_path(database_path))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

#[cfg(unix)]
fn write_private(path: &Path, content: &str) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(content.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::write(path, content)
}

/// 先写临时文件再改名：新文件一出生就是 0600，旧的宽权限文件被整个替换掉。
fn write_control(database_path: &Path, value: &Value) -> Result<(), String> {
    let path = control_path(database_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("本地 API 配置目录创建失败：{error}"))?;
    }
    let temp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&temp);
    write_private(&temp, &value.to_string())
        .map_err(|error| format!("本地 API 配置写入失败：{error}"))?;
    std::fs::rename(&temp, &path).map_err(|error| format!("本地 API 配置替换失败：{error}"))
}

fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| format!("系统随机数不可用：{error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn token(database_path: &Path) -> Option<String> {
    read_control(database_path)
        .get("token")
        .and_then(Value::as_str)
        .filter(|value| value.len() >= 32)
        .map(str::to_owned)
}

pub fn rotate_token(database_path: &Path) -> Result<String, String> {
    let mut control = read_control(database_path);
    let fresh = generate_token()?;
    control["token"] = json!(fresh);
    write_control(database_path, &control)?;
    Ok(fresh)
}

pub fn capture_token(database_path: &Path) -> Option<String> {
    read_control(database_path)
        .get("captureToken")
        .and_then(Value::as_str)
        .filter(|value| value.len() >= 32)
        .map(str::to_owned)
}

pub fn ensure_capture_token(database_path: &Path) -> Result<String, String> {
    if let Some(existing) = capture_token(database_path) {
        return Ok(existing);
    }
    rotate_capture_token(database_path)
}

pub fn rotate_capture_token(database_path: &Path) -> Result<String, String> {
    let mut control = read_control(database_path);
    let fresh = generate_token()?;
    control["captureToken"] = json!(fresh);
    write_control(database_path, &control)?;
    Ok(fresh)
}

pub fn is_enabled(database_path: &Path) -> bool {
    read_control(database_path)
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn status(database_path: &Path) -> LocalApiStatus {
    let listening = RUNTIME
        .get()
        .and_then(|runtime| runtime.lock().ok())
        .is_some_and(|runtime| runtime.thread.is_some());
    LocalApiStatus {
        enabled: is_enabled(database_path),
        host: HOST.to_owned(),
        port: PORT,
        listening,
        has_token: token(database_path).is_some(),
    }
}

pub fn set_enabled(database_path: &Path, enabled: bool) -> Result<LocalApiStatus, String> {
    let mut control = read_control(database_path);
    control["enabled"] = json!(enabled);
    control["host"] = json!(HOST);
    control["port"] = json!(PORT);
    if enabled && token(database_path).is_none() {
        control["token"] = json!(generate_token()?);
    }
    write_control(database_path, &control)?;
    if enabled {
        start(database_path.to_path_buf())?;
    } else {
        stop();
    }
    Ok(status(database_path))
}

pub fn start_if_enabled(database_path: &Path) -> Result<(), String> {
    if is_enabled(database_path) {
        if token(database_path).is_none() {
            rotate_token(database_path)?;
        }
        start(database_path.to_path_buf())?;
    }
    Ok(())
}

fn runtime() -> &'static Mutex<Runtime> {
    RUNTIME.get_or_init(|| {
        Mutex::new(Runtime {
            stop: std::sync::Arc::new(AtomicBool::new(false)),
            thread: None,
        })
    })
}

fn start(database_path: PathBuf) -> Result<(), String> {
    let addr = loopback_addr();
    refuse_non_loopback(addr)?;
    let mut runtime = runtime()
        .lock()
        .map_err(|_| "本地 API 状态锁损坏".to_owned())?;
    if runtime.thread.is_some() {
        return Ok(());
    }
    let listener = TcpListener::bind(addr)
        .map_err(|error| format!("本地 API 绑定 127.0.0.1:{PORT} 失败：{error}"))?;
    listener
        .set_nonblocking(false)
        .map_err(|error| format!("本地 API 监听设置失败：{error}"))?;
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let stop_for_thread = stop.clone();
    let thread = thread::spawn(move || {
        while !stop_for_thread.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    if stop_for_thread.load(Ordering::Relaxed) {
                        break;
                    }
                    if !peer.ip().is_loopback() {
                        let _ = write_raw(&stream, 403, r#"{"error":"只接受本机连接"}"#);
                        continue;
                    }
                    // 每个连接单独一个线程：慢客户端占着读超时的两秒，也不会卡住别的请求。
                    let database_path = database_path.clone();
                    thread::spawn(move || handle_connection(stream, &database_path));
                }
                Err(_) => break,
            }
        }
    });
    runtime.stop = stop;
    runtime.thread = Some(thread);
    Ok(())
}

fn stop() {
    let Ok(mut runtime) = runtime().lock() else {
        return;
    };
    runtime.stop.store(true, Ordering::Relaxed);
    let _ = TcpStream::connect_timeout(&loopback_addr(), Duration::from_millis(200));
    if let Some(thread) = runtime.thread.take() {
        drop(runtime);
        let _ = thread.join();
    }
}

fn handle_connection(mut stream: TcpStream, database_path: &Path) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let (status, body) = match read_request(&mut stream) {
        Ok(raw) => route(database_path, &raw),
        Err((code, message)) => (code, error_body(message)),
    };
    let _ = write_raw(&stream, status, &body);
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// 读完整个请求：先读到空行，再按 Content-Length 把正文读全（正文可能在后面的数据包里）。
fn read_request(stream: &mut impl Read) -> Result<Vec<u8>, (u16, &'static str)> {
    let mut buffer = Vec::with_capacity(2048);
    let mut chunk = [0_u8; 4096];
    let (header_end, content_length) = loop {
        if let Some(end) = find_subslice(&buffer, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..end]).into_owned();
            let length = head
                .lines()
                .find_map(|line| {
                    let (key, value) = line.split_once(':')?;
                    key.trim()
                        .eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            break (end + 4, length);
        }
        if buffer.len() > MAX_HEADER_BYTES {
            return Err((431, "请求头过大"));
        }
        let read = stream.read(&mut chunk).map_err(|_| (408, "读取请求超时"))?;
        if read == 0 {
            return Err((400, "请求不完整"));
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    if content_length > MAX_BODY_BYTES {
        return Err((413, "请求体过大"));
    }
    while buffer.len() < header_end + content_length {
        let read = stream.read(&mut chunk).map_err(|_| (408, "读取请求超时"))?;
        if read == 0 {
            return Err((400, "请求正文不完整"));
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    Ok(buffer)
}

struct Request<'a> {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: &'a [u8],
}

impl Request<'_> {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

fn parse_request(raw: &[u8]) -> Result<Request<'_>, (u16, &'static str)> {
    let split = find_subslice(raw, b"\r\n\r\n").ok_or((400, "请求不完整"))?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let mut start = lines.next().unwrap_or("").split_whitespace();
    let method = start.next().unwrap_or("").to_ascii_uppercase();
    let target = start.next().unwrap_or("/").to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Ok(Request {
        method,
        target,
        headers,
        body: &raw[split + 4..],
    })
}

fn host_allowed(host: Option<&str>) -> bool {
    let Some(host) = host else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    host == format!("{HOST}:{PORT}") || host == format!("localhost:{PORT}")
}

/// 比较令牌时不因第一个不同字节提前返回，避免通过响应时间猜令牌。
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

/// 控制文件里没有令牌时不能把空字符串当成匹配。
fn token_matches(presented: &str, expected: &str) -> bool {
    !expected.is_empty() && constant_time_eq(presented.as_bytes(), expected.as_bytes())
}

fn error_body(message: &str) -> String {
    json!({ "error": message }).to_string()
}

pub fn route(database_path: &Path, raw: &[u8]) -> (u16, String) {
    if !is_enabled(database_path) {
        return (403, error_body("本地 API 默认关闭"));
    }
    let request = match parse_request(raw) {
        Ok(request) => request,
        Err((code, message)) => return (code, error_body(message)),
    };
    if !host_allowed(request.header("host")) {
        return (403, error_body("Host 不被允许"));
    }
    if request.header("origin").is_some() {
        return (403, error_body("本地 API 不接受来自网页的跨站请求"));
    }
    let path = request.target.split('?').next().unwrap_or("/");
    if request.method == "GET" && path == "/v1/health" {
        return (
            200,
            json!({ "ok": true, "host": HOST, "port": PORT }).to_string(),
        );
    }
    let presented = request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    let full_token = token(database_path);
    let capture_only = capture_token(database_path);
    let full_ok = full_token
        .as_deref()
        .is_some_and(|expected| token_matches(presented, expected));
    // 没有 captureToken 时 capture_only 是 None，空 Bearer 不会被当成收集专用令牌。
    let capture_ok = capture_only
        .as_deref()
        .is_some_and(|expected| token_matches(presented, expected));
    if request.method == "POST" && path == "/v1/capture" {
        if !full_ok && !capture_ok {
            return (401, error_body("缺少或错误的令牌"));
        }
        return handle_capture(database_path, &request);
    }
    // 收集专用令牌碰到别的接口：明确 403，不要落到后面的 401。
    if capture_ok && !full_ok {
        return (403, error_body("收集专用令牌只能用于 /v1/capture"));
    }
    if full_token.is_none() {
        return (
            503,
            error_body("令牌还没生成，请在工具箱里重新开启本地 API"),
        );
    }
    if !full_ok {
        return (401, error_body("缺少或错误的令牌"));
    }
    match (request.method.as_str(), path) {
        ("GET", "/v1/search") => match v2_ops::list_workspace(
            database_path,
            &query_value(&request.target, "q"),
            "",
            "",
            20,
            0,
        ) {
            Ok(page) => (
                200,
                serde_json::to_string(&page).unwrap_or_else(|_| error_body("结果序列化失败")),
            ),
            Err(error) => (500, error_body(&error)),
        },
        ("POST", "/v1/usage") => {
            let is_json = request
                .header("content-type")
                .is_some_and(|value| value.to_ascii_lowercase().starts_with("application/json"));
            if !is_json {
                return (415, error_body("请求必须是 application/json"));
            }
            let Ok(value) = serde_json::from_slice::<Value>(request.body) else {
                return (400, error_body("使用记录必须是 JSON"));
            };
            let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or("");
            match v2_ops::log_usage(
                database_path,
                text("entityId"),
                text("verdict"),
                text("note"),
                "local_api",
            ) {
                Ok(()) => (200, r#"{"ok":true}"#.to_owned()),
                Err(error) => (400, error_body(&error)),
            }
        }
        _ => (404, error_body("没有这个本地接口")),
    }
}

fn handle_capture(database_path: &Path, request: &Request<'_>) -> (u16, String) {
    let is_json = request
        .header("content-type")
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("application/json"));
    if !is_json {
        return (415, error_body("请求必须是 application/json"));
    }
    let Ok(value) = serde_json::from_slice::<Value>(request.body) else {
        return (400, error_body("收集内容必须是 JSON"));
    };
    let Some(object) = value.as_object() else {
        return (400, error_body("字段类型不对"));
    };
    let text = match object.get("text") {
        None => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(_) => return (400, error_body("字段类型不对")),
    };
    let urls = match object.get("urls") {
        None => Vec::new(),
        Some(Value::Array(items)) => {
            let mut urls = Vec::with_capacity(items.len());
            for item in items {
                let Some(url) = item.as_str() else {
                    return (400, error_body("字段类型不对"));
                };
                urls.push(url.to_owned());
            }
            urls
        }
        Some(_) => return (400, error_body("字段类型不对")),
    };
    let platform = match object.get("sourcePlatform") {
        None => "other".to_owned(),
        Some(Value::String(platform)) => platform.clone(),
        Some(_) => return (400, error_body("字段类型不对")),
    };
    let context = match object.get("sourceContext") {
        None => String::new(),
        Some(Value::String(context)) => context.chars().take(MAX_CAPTURE_CONTEXT_CHARS).collect(),
        Some(_) => return (400, error_body("字段类型不对")),
    };
    if urls.len() > MAX_CAPTURE_URLS {
        return (400, error_body("链接最多 100 条"));
    }
    if urls
        .iter()
        .any(|url| url.chars().count() > MAX_CAPTURE_URL_CHARS)
    {
        return (400, error_body("单条链接最多 2048 个字符"));
    }
    if text.chars().count() > MAX_CAPTURE_TEXT_CHARS {
        return (400, error_body("原文最多 50000 个字符"));
    }
    let has_text = !text.is_empty();
    let has_url = urls.iter().any(|url| !url.is_empty());
    if !has_text && !has_url {
        return (400, error_body("没有可收集的内容"));
    }
    let composed = if urls.is_empty() {
        text.clone()
    } else if text.is_empty() {
        urls.join("\n")
    } else {
        format!("{}\n\n{text}", urls.join("\n"))
    };
    let source = v2_ops::CaptureSource {
        platform: &platform,
        via: "local_api",
        original_text: has_text.then_some(text.as_str()),
    };
    match v2_ops::commit_capture_with(database_path, &composed, &context, &source) {
        Ok(commit) => capture_response(&commit),
        Err(_) => (500, error_body("收集写入失败")),
    }
}

fn capture_response(commit: &v2_ops::CaptureCommit) -> (u16, String) {
    let Ok(mut value) = serde_json::to_value(commit) else {
        return (500, error_body("收集写入失败"));
    };
    let Some(object) = value.as_object_mut() else {
        return (500, error_body("收集写入失败"));
    };
    object.insert("ok".to_owned(), json!(true));
    match serde_json::to_string(&value) {
        Ok(body) => (200, body),
        Err(_) => (500, error_body("收集写入失败")),
    }
}

fn query_value(target: &str, key: &str) -> String {
    let Some(query) = target.split('?').nth(1) else {
        return String::new();
    };
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next() == Some(key) {
            return percent_decode(parts.next().unwrap_or(""));
        }
    }
    String::new()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.replace('+', " ").into_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).chars().take(200).collect()
}

fn write_raw(mut stream: &TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    // 故意不发任何 CORS 头：网页里的脚本既不能写入，也读不到响应。
    let payload = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(payload.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 每个测试一个独立目录：控制文件固定叫同一个名字，放在一起并行跑会互相覆盖。
    fn temp_db(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("fsl-api-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("fox-stars-lab.sqlite3")
    }

    /// 直接写控制文件开启，不调用 set_enabled（它会真的占用 39217 端口）。
    fn enable_for_test(path: &Path) -> String {
        let token = generate_token().unwrap();
        write_control(
            path,
            &json!({ "enabled": true, "host": HOST, "port": PORT, "token": token }),
        )
        .unwrap();
        token
    }

    fn request(method: &str, target: &str, headers: &[(&str, &str)], body: &str) -> Vec<u8> {
        let mut text = format!("{method} {target} HTTP/1.1\r\n");
        for (key, value) in headers {
            text.push_str(&format!("{key}: {value}\r\n"));
        }
        text.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
        text.into_bytes()
    }

    fn good_host() -> (&'static str, &'static str) {
        ("Host", "127.0.0.1:39217")
    }

    #[test]
    fn local_api_refuses_non_loopback_and_defaults_off() {
        assert!(refuse_non_loopback(SocketAddr::from(([0, 0, 0, 0], 80))).is_err());
        assert!(refuse_non_loopback(loopback_addr()).is_ok());
        let path = temp_db("off");
        assert!(!is_enabled(&path));
        let (status, body) = route(&path, b"GET /v1/health HTTP/1.1\r\n\r\n");
        assert_eq!(status, 403);
        assert!(body.contains("默认关闭"));
    }

    #[test]
    fn health_is_open_but_only_for_the_right_host_and_never_for_web_pages() {
        let path = temp_db("host");
        enable_for_test(&path);
        let ok = route(&path, &request("GET", "/v1/health", &[good_host()], ""));
        assert_eq!(ok.0, 200);
        for host in ["evil.example:39217", "127.0.0.1:80", "127.0.0.1"] {
            let denied = route(&path, &request("GET", "/v1/health", &[("Host", host)], ""));
            assert_eq!(denied.0, 403, "Host {host} 应被拒绝");
        }
        // HTTP/1.0 不带 Host：无法校验，拒绝
        assert_eq!(route(&path, b"GET /v1/health HTTP/1.0\r\n\r\n").0, 403);
        // 网页脚本发起的请求必带 Origin，即使令牌正确也拒绝
        let token = token(&path).unwrap();
        let from_web = route(
            &path,
            &request(
                "POST",
                "/v1/usage",
                &[
                    good_host(),
                    ("Origin", "https://evil.example"),
                    ("Authorization", &format!("Bearer {token}")),
                    ("Content-Type", "application/json"),
                ],
                "{}",
            ),
        );
        assert_eq!(from_web.0, 403);
    }

    #[test]
    fn everything_except_health_needs_the_right_token() {
        let path = temp_db("auth");
        v2_ops::ensure_ready(&path).unwrap();
        let token = enable_for_test(&path);
        let bearer = format!("Bearer {token}");

        let none = route(&path, &request("GET", "/v1/search?q=x", &[good_host()], ""));
        assert_eq!(none.0, 401);
        let wrong = route(
            &path,
            &request(
                "GET",
                "/v1/search?q=x",
                &[good_host(), ("Authorization", "Bearer nope")],
                "",
            ),
        );
        assert_eq!(wrong.0, 401);
        // 同样长度但内容不同的令牌
        let same_len = format!("Bearer {}", "0".repeat(token.len()));
        let forged = route(
            &path,
            &request(
                "GET",
                "/v1/search?q=x",
                &[good_host(), ("Authorization", &same_len)],
                "",
            ),
        );
        assert_eq!(forged.0, 401);

        let ok = route(
            &path,
            &request(
                "GET",
                "/v1/search?q=x",
                &[good_host(), ("Authorization", &bearer)],
                "",
            ),
        );
        assert_eq!(ok.0, 200, "{}", ok.1);
        assert!(ok.1.contains("totalCount"));

        let unknown = route(
            &path,
            &request(
                "GET",
                "/v1/nothing",
                &[good_host(), ("Authorization", &bearer)],
                "",
            ),
        );
        assert_eq!(unknown.0, 404);
    }

    #[test]
    fn writes_need_json_and_a_valid_body() {
        let path = temp_db("write");
        v2_ops::ensure_ready(&path).unwrap();
        let token = enable_for_test(&path);
        let bearer = format!("Bearer {token}");
        let plain = route(
            &path,
            &request(
                "POST",
                "/v1/usage",
                &[
                    good_host(),
                    ("Authorization", &bearer),
                    ("Content-Type", "text/plain"),
                ],
                "{}",
            ),
        );
        assert_eq!(plain.0, 415);
        let broken = route(
            &path,
            &request(
                "POST",
                "/v1/usage",
                &[
                    good_host(),
                    ("Authorization", &bearer),
                    ("Content-Type", "application/json"),
                ],
                "not json",
            ),
        );
        assert_eq!(broken.0, 400);
    }

    #[test]
    fn rotating_the_token_invalidates_the_old_one_and_the_file_is_private() {
        let path = temp_db("rotate");
        let old = enable_for_test(&path);
        let fresh = rotate_token(&path).unwrap();
        assert_ne!(old, fresh);
        assert_eq!(fresh.len(), 64);
        let with_old = route(
            &path,
            &request(
                "GET",
                "/v1/search",
                &[good_host(), ("Authorization", &format!("Bearer {old}"))],
                "",
            ),
        );
        assert_eq!(with_old.0, 401);
        // 旋转令牌不应把「已开启」状态弄丢
        assert!(is_enabled(&path));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(control_path(&path))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "控制文件里有令牌，必须只有自己能读");
        }
    }

    #[test]
    fn token_comparison_checks_length_and_content() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"a"));
    }

    #[test]
    fn body_arriving_in_a_later_packet_is_still_read() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let body: &'static [u8] = b"{\"a\":\"b\"}";
        let writer = thread::spawn(move || {
            let mut client = TcpStream::connect(address).unwrap();
            let head = format!(
                "POST /v1/usage HTTP/1.1\r\nHost: 127.0.0.1:39217\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            client.write_all(head.as_bytes()).unwrap();
            client.flush().unwrap();
            // 正文故意晚一会儿才到，模拟分成两个数据包
            thread::sleep(Duration::from_millis(80));
            client.write_all(body).unwrap();
        });
        let (mut server_side, _) = listener.accept().unwrap();
        server_side
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let raw = read_request(&mut server_side).unwrap();
        writer.join().unwrap();
        let parsed = parse_request(&raw).unwrap();
        assert_eq!(parsed.method, "POST");
        assert_eq!(parsed.body, body);
    }

    #[test]
    fn a_body_shorter_than_content_length_is_rejected_not_half_read() {
        let truncated = b"POST /v1/usage HTTP/1.1\r\nContent-Length: 20\r\n\r\n{\"a\":1}";
        assert_eq!(read_request(&mut truncated.as_slice()).unwrap_err().0, 400);
    }

    #[test]
    fn oversized_requests_are_refused() {
        let mut huge_header = b"GET / HTTP/1.1\r\nX: ".to_vec();
        huge_header.extend(std::iter::repeat_n(b'a', MAX_HEADER_BYTES + 10));
        assert_eq!(
            read_request(&mut huge_header.as_slice()).unwrap_err().0,
            431
        );
        let big_body = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert_eq!(read_request(&mut big_body.as_bytes()).unwrap_err().0, 413);
    }

    fn insert_account(path: &Path) {
        let connection = v2_ops::open_db(path).unwrap();
        connection
            .execute(
                "INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test')",
                [],
            )
            .unwrap();
    }

    fn insert_starred_tool(path: &Path) {
        let connection = v2_ops::open_db(path).unwrap();
        connection.execute(
            "INSERT INTO repositories(id, account_id, owner, name, full_name, description, language, topics_json, html_url, sync_status, starred_at) VALUES ('repo1', 'acct', 'fox', 'tool', 'fox/tool', '批量整理群聊链接', 'TypeScript', '[]', 'https://github.com/fox/tool', 'active', '2026-10-01T00:00:00Z')",
            [],
        ).unwrap();
    }

    fn opened_db(label: &str) -> (PathBuf, String) {
        let path = temp_db(label);
        v2_ops::ensure_ready(&path).unwrap();
        insert_account(&path);
        let full = enable_for_test(&path);
        (path, full)
    }

    fn capture_db(label: &str) -> (PathBuf, String, String) {
        let (path, full) = opened_db(label);
        let capture = rotate_capture_token(&path).unwrap();
        (path, full, capture)
    }

    fn authed(
        method: &str,
        target: &str,
        bearer: &str,
        extra: &[(&str, &str)],
        body: &str,
    ) -> Vec<u8> {
        let header = format!("Bearer {bearer}");
        let mut headers = vec![good_host(), ("Authorization", header.as_str())];
        headers.extend_from_slice(extra);
        request(method, target, &headers, body)
    }

    fn post_capture(path: &Path, bearer: &str, body: &str) -> (u16, String) {
        route(
            path,
            &authed(
                "POST",
                "/v1/capture",
                bearer,
                &[("Content-Type", "application/json")],
                body,
            ),
        )
    }

    fn assert_commit_keys(body: &str) -> Value {
        let value: Value = serde_json::from_str(body).unwrap();
        let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "alreadyInInbox",
                "alreadyStarred",
                "blocked",
                "created",
                "duplicateBatch",
                "linksFound",
                "ok",
                "skipped",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
        );
        assert_eq!(value["ok"], json!(true));
        for key in [
            "created",
            "skipped",
            "alreadyStarred",
            "alreadyInInbox",
            "blocked",
            "linksFound",
        ] {
            assert!(value[key].is_number(), "{key} 应该是数字");
        }
        assert!(value["duplicateBatch"].is_boolean());
        value
    }

    #[test]
    fn capture_without_a_token_is_unauthorized() {
        let (path, _) = opened_db("no-token");
        let response = route(
            &path,
            &request(
                "POST",
                "/v1/capture",
                &[good_host(), ("Content-Type", "application/json")],
                r#"{"urls":["https://github.com/acme/no-token"]}"#,
            ),
        );
        assert_eq!(response.0, 401);
        assert!(response.1.contains("令牌"));
    }

    #[test]
    fn capture_with_a_wrong_token_is_unauthorized() {
        let (path, full) = opened_db("wrong-token");
        let wrong = post_capture(
            &path,
            "nope",
            r#"{"urls":["https://github.com/acme/wrong"]}"#,
        );
        assert_eq!(wrong.0, 401);
        let forged = "0".repeat(full.len());
        let same_len = post_capture(
            &path,
            &forged,
            r#"{"urls":["https://github.com/acme/wrong"]}"#,
        );
        assert_eq!(same_len.0, 401);
        assert!(same_len.1.contains("令牌"));
    }

    #[test]
    fn full_token_can_capture() {
        let (path, full) = opened_db("full-capture");
        assert!(capture_token(&path).is_none());
        let (status, body) = post_capture(
            &path,
            &full,
            r#"{"urls":["https://github.com/acme/by-full"]}"#,
        );
        assert_eq!(status, 200, "{body}");
        let value = assert_commit_keys(&body);
        assert_eq!(value["created"], json!(1));
        assert_eq!(value["linksFound"], json!(1));
    }

    #[test]
    fn capture_token_can_capture() {
        let (path, full, capture) = capture_db("cap-token");
        assert_ne!(full, capture);
        let (status, body) = post_capture(
            &path,
            &capture,
            r#"{"urls":["https://github.com/acme/by-capture"]}"#,
        );
        assert_eq!(status, 200, "{body}");
        assert_eq!(assert_commit_keys(&body)["created"], json!(1));
    }

    #[test]
    fn capture_token_cannot_search() {
        let (path, _, capture) = capture_db("cap-search");
        let search = route(&path, &authed("GET", "/v1/search?q=x", &capture, &[], ""));
        assert_eq!(search.0, 403);
        assert!(search.1.contains("收集专用令牌"));
        let unknown = route(&path, &authed("GET", "/v1/nothing", &capture, &[], ""));
        assert_eq!(unknown.0, 403);
        let get_capture = route(&path, &authed("GET", "/v1/capture", &capture, &[], ""));
        assert_eq!(get_capture.0, 403);
    }

    #[test]
    fn capture_token_cannot_record_usage() {
        let (path, _, capture) = capture_db("cap-usage");
        let usage = route(
            &path,
            &authed(
                "POST",
                "/v1/usage",
                &capture,
                &[("Content-Type", "application/json")],
                r#"{"entityId":"repo1","verdict":"useful","note":""}"#,
            ),
        );
        assert_eq!(usage.0, 403);
        assert!(usage.1.contains("收集专用令牌只能用于 /v1/capture"));
    }

    #[test]
    fn full_token_still_opens_the_other_routes() {
        let (path, full, capture) = capture_db("full-routes");
        let search = route(&path, &authed("GET", "/v1/search?q=x", &full, &[], ""));
        assert_eq!(search.0, 200, "{}", search.1);
        assert!(search.1.contains("totalCount"));
        let usage = route(
            &path,
            &authed(
                "POST",
                "/v1/usage",
                &full,
                &[("Content-Type", "application/json")],
                r#"{"entityId":"repo1","verdict":"useful","note":"测"}"#,
            ),
        );
        assert_eq!(usage.0, 200, "{}", usage.1);
        let unknown = route(&path, &authed("GET", "/v1/nothing", &full, &[], ""));
        assert_eq!(unknown.0, 404);
        let health = route(&path, &request("GET", "/v1/health", &[good_host()], ""));
        assert_eq!(health.0, 200);
        // 健康检查在认证之前，本来就不看令牌。收集专用令牌走到这里也仍是 200。
        let health_with_capture = route(&path, &authed("GET", "/v1/health", &capture, &[], ""));
        assert_eq!(health_with_capture.0, 200);
        let get_capture = route(&path, &authed("GET", "/v1/capture", &full, &[], ""));
        assert_eq!(get_capture.0, 404);
    }

    #[test]
    fn empty_bearer_is_rejected_when_capture_token_is_absent() {
        let (path, full) = opened_db("empty-bearer");
        assert!(read_control(&path).get("captureToken").is_none());
        let empty = route(
            &path,
            &request(
                "POST",
                "/v1/capture",
                &[
                    good_host(),
                    ("Authorization", "Bearer "),
                    ("Content-Type", "application/json"),
                ],
                r#"{"urls":["https://github.com/acme/empty-bearer"]}"#,
            ),
        );
        assert_eq!(empty.0, 401);
        assert!(empty.1.contains("令牌"));
        let ok = post_capture(
            &path,
            &full,
            r#"{"urls":["https://github.com/acme/empty-bearer"]}"#,
        );
        assert_eq!(ok.0, 200, "{}", ok.1);
    }

    #[test]
    fn rotating_either_token_leaves_the_other_alone() {
        let (path, full, capture) = capture_db("rotate-both");
        let new_full = rotate_token(&path).unwrap();
        assert_ne!(new_full, full);
        assert_eq!(capture_token(&path).as_deref(), Some(capture.as_str()));
        let old_full = route(&path, &authed("GET", "/v1/search?q=x", &full, &[], ""));
        assert_eq!(old_full.0, 401);
        let fresh_full = route(&path, &authed("GET", "/v1/search?q=x", &new_full, &[], ""));
        assert_eq!(fresh_full.0, 200, "{}", fresh_full.1);
        let still_capture = post_capture(
            &path,
            &capture,
            r#"{"urls":["https://github.com/acme/rotate-full"]}"#,
        );
        assert_eq!(still_capture.0, 200, "{}", still_capture.1);

        let new_capture = rotate_capture_token(&path).unwrap();
        assert_ne!(new_capture, capture);
        assert_eq!(token(&path).as_deref(), Some(new_full.as_str()));
        let old_capture = post_capture(
            &path,
            &capture,
            r#"{"urls":["https://github.com/acme/rotate-capture"]}"#,
        );
        assert_eq!(old_capture.0, 401);
        let fresh_capture = post_capture(
            &path,
            &new_capture,
            r#"{"urls":["https://github.com/acme/rotate-capture"]}"#,
        );
        assert_eq!(fresh_capture.0, 200, "{}", fresh_capture.1);
        assert_eq!(assert_commit_keys(&fresh_capture.1)["created"], json!(1));
        let full_still = route(&path, &authed("GET", "/v1/search?q=x", &new_full, &[], ""));
        assert_eq!(full_still.0, 200, "{}", full_still.1);
        let full_can_capture = post_capture(
            &path,
            &new_full,
            r#"{"urls":["https://github.com/acme/rotate-full-still"]}"#,
        );
        assert_eq!(full_can_capture.0, 200, "{}", full_can_capture.1);
    }

    #[test]
    fn capture_rejects_requests_that_carry_origin() {
        let (path, _, capture) = capture_db("origin");
        let bearer = format!("Bearer {capture}");
        let response = route(
            &path,
            &request(
                "POST",
                "/v1/capture",
                &[
                    good_host(),
                    ("Origin", "https://evil.example"),
                    ("Authorization", &bearer),
                    ("Content-Type", "application/json"),
                ],
                r#"{"urls":["https://github.com/acme/origin"]}"#,
            ),
        );
        assert_eq!(response.0, 403);
        assert!(response.1.contains("跨站"));
    }

    #[test]
    fn capture_rejects_a_foreign_host() {
        let (path, _, capture) = capture_db("host");
        let bearer = format!("Bearer {capture}");
        let response = route(
            &path,
            &request(
                "POST",
                "/v1/capture",
                &[
                    ("Host", "evil.example:39217"),
                    ("Authorization", &bearer),
                    ("Content-Type", "application/json"),
                ],
                r#"{"urls":["https://github.com/acme/host"]}"#,
            ),
        );
        assert_eq!(response.0, 403);
        assert!(response.1.contains("Host"));
        let missing_host = route(
            &path,
            &request(
                "POST",
                "/v1/capture",
                &[
                    ("Authorization", &bearer),
                    ("Content-Type", "application/json"),
                ],
                r#"{"urls":["https://github.com/acme/host"]}"#,
            ),
        );
        assert_eq!(missing_host.0, 403);
    }

    #[test]
    fn capture_requires_json_content_type() {
        let (path, full, _) = capture_db("ctype");
        let missing = route(
            &path,
            &authed(
                "POST",
                "/v1/capture",
                &full,
                &[],
                r#"{"urls":["https://github.com/acme/ctype"]}"#,
            ),
        );
        assert_eq!(missing.0, 415);
        assert!(missing.1.contains("application/json"));
        let plain = route(
            &path,
            &authed(
                "POST",
                "/v1/capture",
                &full,
                &[("Content-Type", "text/plain")],
                r#"{"urls":["https://github.com/acme/ctype"]}"#,
            ),
        );
        assert_eq!(plain.0, 415);
    }

    #[test]
    fn capture_rejects_a_body_that_is_not_json() {
        let (path, full, _) = capture_db("not-json");
        for body in ["not json", "", "{", "ok"] {
            let (status, message) = post_capture(&path, &full, body);
            assert_eq!(status, 400, "{body} -> {message}");
            assert!(
                message.contains("收集内容必须是 JSON"),
                "{body} -> {message}"
            );
        }
    }

    #[test]
    fn capture_rejects_empty_text_and_urls() {
        let (path, full, _) = capture_db("empty");
        for body in [
            "{}",
            r#"{"text":""}"#,
            r#"{"urls":[]}"#,
            r#"{"text":"","urls":[]}"#,
            r#"{"urls":[""]}"#,
            r#"{"text":"","urls":[""]}"#,
        ] {
            let (status, message) = post_capture(&path, &full, body);
            assert_eq!(status, 400, "{body} -> {message}");
            assert!(message.contains("没有可收集的内容"), "{body} -> {message}");
        }
    }

    #[test]
    fn capture_rejects_more_than_one_hundred_urls() {
        let (path, full, _) = capture_db("url-count");
        let too_many = json!({ "urls": vec!["https://example.com/same"; 101] }).to_string();
        let (status, message) = post_capture(&path, &full, &too_many);
        assert_eq!(status, 400, "{message}");
        assert!(message.contains("100"), "{message}");
        let enough = json!({ "urls": vec!["https://example.com/same"; 100] }).to_string();
        let (status, body) = post_capture(&path, &full, &enough);
        assert_eq!(status, 200, "{body}");
        assert_eq!(assert_commit_keys(&body)["created"], json!(1));
    }

    #[test]
    fn capture_rejects_a_url_over_2048_chars() {
        let (path, full, _) = capture_db("url-len");
        let too_long = json!({ "urls": ["u".repeat(2049)] }).to_string();
        let (status, message) = post_capture(&path, &full, &too_long);
        assert_eq!(status, 400, "{message}");
        assert!(message.contains("2048"), "{message}");
        let fits = json!({ "urls": ["u".repeat(2048)] }).to_string();
        let (status, body) = post_capture(&path, &full, &fits);
        assert_eq!(status, 200, "{body}");
        assert_eq!(assert_commit_keys(&body)["linksFound"], json!(0));
    }

    #[test]
    fn capture_rejects_text_over_50000_chars() {
        let (path, full, _) = capture_db("text-len");
        let too_long = json!({ "text": "a".repeat(50_001) }).to_string();
        let (status, message) = post_capture(&path, &full, &too_long);
        assert_eq!(status, 400, "{message}");
        assert!(message.contains("50000"), "{message}");
        let fits = json!({ "text": "b".repeat(50_000) }).to_string();
        let (status, body) = post_capture(&path, &full, &fits);
        assert_eq!(status, 200, "{body}");
        assert_eq!(assert_commit_keys(&body)["linksFound"], json!(0));
        // 2 万个汉字超过 5 万字节，但仍少于 5 万个字符，应按字符放行。
        let chinese = json!({ "text": "字".repeat(20_000) }).to_string();
        let (status, body) = post_capture(&path, &full, &chinese);
        assert_eq!(status, 200, "{body}");
        assert_eq!(assert_commit_keys(&body)["linksFound"], json!(0));
    }

    #[test]
    fn capture_truncates_source_context_instead_of_failing() {
        let (path, full, _) = capture_db("context");
        let context = "群".repeat(2001);
        let body = json!({
            "urls": ["https://github.com/acme/ctx"],
            "sourcePlatform": "wechat",
            "sourceContext": context,
        })
        .to_string();
        let (status, response) = post_capture(&path, &full, &body);
        assert_eq!(status, 200, "{response}");
        let connection = v2_ops::open_db(&path).unwrap();
        let (batch_context, item_context): (String, String) = connection
            .query_row(
                "SELECT capture_batches.source_context, inbox_items.source_context FROM inbox_items JOIN capture_batches ON capture_batches.id = inbox_items.capture_batch_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(batch_context.chars().count(), 2000);
        assert_eq!(batch_context, "群".repeat(2000));
        assert_eq!(item_context, batch_context);
    }

    #[test]
    fn capture_rejects_mistyped_fields() {
        let (path, full, _) = capture_db("types");
        for body in [
            r#"{"text":1}"#,
            r#"{"urls":"https://github.com/acme/types"}"#,
            r#"{"urls":[1]}"#,
            r#"{"urls":[null]}"#,
            r#"{"sourcePlatform":true,"urls":["https://github.com/acme/types"]}"#,
            r#"{"sourceContext":["群"],"text":"https://github.com/acme/types"}"#,
            r#"{"text":null}"#,
            r#"{"urls":{"a":"b"}}"#,
            "[]",
            "\"https://github.com/acme/types\"",
            "1",
            "null",
        ] {
            let (status, message) = post_capture(&path, &full, body);
            assert_eq!(status, 400, "{body} -> {message}");
            assert!(message.contains("字段类型不对"), "{body} -> {message}");
        }
    }

    #[test]
    fn capture_with_only_urls_creates_items() {
        let (path, full, _) = capture_db("urls-only");
        let (status, body) = post_capture(
            &path,
            &full,
            r#"{"urls":["https://github.com/acme/only-url"]}"#,
        );
        assert_eq!(status, 200, "{body}");
        assert_eq!(assert_commit_keys(&body)["created"], json!(1));
        let connection = v2_ops::open_db(&path).unwrap();
        let (excerpt, via, batch_platform): (String, String, String) = connection
            .query_row(
                "SELECT inbox_items.raw_excerpt, inbox_items.captured_via, capture_batches.source_platform FROM inbox_items JOIN capture_batches ON capture_batches.id = inbox_items.capture_batch_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(excerpt, "");
        assert_eq!(via, "local_api");
        assert_eq!(batch_platform, "other");
    }

    #[test]
    fn capture_with_only_text_creates_items() {
        let (path, full, _) = capture_db("text-only");
        let text = "请看 https://github.com/acme/only-text";
        let body = json!({ "text": text }).to_string();
        let (status, response) = post_capture(&path, &full, &body);
        assert_eq!(status, 200, "{response}");
        let value = assert_commit_keys(&response);
        assert_eq!(value["created"], json!(1));
        assert_eq!(value["linksFound"], json!(1));
        let connection = v2_ops::open_db(&path).unwrap();
        let excerpt: String = connection
            .query_row("SELECT raw_excerpt FROM inbox_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(excerpt, text);
    }

    #[test]
    fn capture_puts_urls_before_text() {
        let (path, full, _) = capture_db("order");
        let text = "再看 https://github.com/acme/two";
        let body = json!({
            "urls": ["https://github.com/acme/one"],
            "text": text,
        })
        .to_string();
        let (status, response) = post_capture(&path, &full, &body);
        assert_eq!(status, 200, "{response}");
        assert_eq!(assert_commit_keys(&response)["created"], json!(2));
        let connection = v2_ops::open_db(&path).unwrap();
        let mut statement = connection
            .prepare("SELECT raw_excerpt FROM inbox_items")
            .unwrap();
        let excerpts: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        assert_eq!(excerpts.len(), 2);
        assert!(excerpts.iter().all(|excerpt| excerpt == text));
        drop(statement);
        drop(connection);
        let composed = "https://github.com/acme/one\n\n再看 https://github.com/acme/two";
        assert!(
            v2_ops::commit_capture(&path, composed, "")
                .unwrap()
                .duplicate_batch
        );
        let reversed = "再看 https://github.com/acme/two\n\nhttps://github.com/acme/one";
        let reversed = v2_ops::commit_capture(&path, reversed, "").unwrap();
        assert!(!reversed.duplicate_batch);
        assert_eq!(reversed.already_in_inbox, 2);
    }

    #[test]
    fn capture_stores_a_known_source_platform_on_the_batch() {
        let (path, full, _) = capture_db("platform");
        let context = "群：小红书运营交流 · 张三 · 2026-10-04 09:30";
        let body = json!({
            "urls": ["https://github.com/acme/platform"],
            "sourcePlatform": "wechat",
            "sourceContext": context,
        })
        .to_string();
        let (status, response) = post_capture(&path, &full, &body);
        assert_eq!(status, 200, "{response}");
        let connection = v2_ops::open_db(&path).unwrap();
        let (batch_platform, item_platform, via, stored_context): (String, String, String, String) = connection
            .query_row(
                "SELECT capture_batches.source_platform, inbox_items.source_platform, inbox_items.captured_via, capture_batches.source_context FROM inbox_items JOIN capture_batches ON capture_batches.id = inbox_items.capture_batch_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(batch_platform, "wechat");
        assert_eq!(item_platform, "github");
        assert_eq!(via, "local_api");
        assert_eq!(stored_context, context);
    }

    #[test]
    fn capture_turns_an_unknown_platform_into_other() {
        let (path, full, _) = capture_db("bad-platform");
        let body = json!({
            "urls": ["https://example.com/not-a-platform"],
            "sourcePlatform": "weixin",
        })
        .to_string();
        let (status, response) = post_capture(&path, &full, &body);
        assert_eq!(status, 200, "{response}");
        let connection = v2_ops::open_db(&path).unwrap();
        let (batch_platform, item_platform): (String, String) = connection
            .query_row(
                "SELECT capture_batches.source_platform, inbox_items.source_platform FROM inbox_items JOIN capture_batches ON capture_batches.id = inbox_items.capture_batch_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(batch_platform, "other");
        assert_eq!(item_platform, "web");
    }

    #[test]
    fn capture_marks_via_local_api() {
        let (path, _, capture) = capture_db("via");
        let (status, response) = post_capture(
            &path,
            &capture,
            r#"{"urls":["https://github.com/acme/via"]}"#,
        );
        assert_eq!(status, 200, "{response}");
        let connection = v2_ops::open_db(&path).unwrap();
        let via: String = connection
            .query_row("SELECT captured_via FROM inbox_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(via, "local_api");
    }

    #[test]
    fn capture_keeps_the_link_platform_on_the_item() {
        let (path, full, _) = capture_db("link-platform");
        let body = json!({
            "urls": ["https://github.com/acme/link-platform", "https://x.com/u/status/1"],
            "sourcePlatform": "wechat",
        })
        .to_string();
        let (status, response) = post_capture(&path, &full, &body);
        assert_eq!(status, 200, "{response}");
        assert_eq!(assert_commit_keys(&response)["created"], json!(2));
        let connection = v2_ops::open_db(&path).unwrap();
        let mut statement = connection
            .prepare("SELECT normalized_url, source_platform FROM inbox_items")
            .unwrap();
        let rows: Vec<(String, String)> = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        let github = rows
            .iter()
            .find(|(url, _)| url.contains("github.com"))
            .unwrap();
        let post = rows.iter().find(|(url, _)| url.contains("x.com")).unwrap();
        assert_eq!(github.1, "github");
        assert_eq!(post.1, "x");
        drop(statement);
        let batch_platform: String = connection
            .query_row("SELECT source_platform FROM capture_batches", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(batch_platform, "wechat");
    }

    #[test]
    fn capture_stores_the_original_words_trimmed_to_2000_chars() {
        let (path, full, _) = capture_db("excerpt");
        let words = format!("  {}  ", "字".repeat(2005));
        let body = json!({
            "urls": ["https://github.com/acme/excerpt"],
            "text": words,
        })
        .to_string();
        let (status, response) = post_capture(&path, &full, &body);
        assert_eq!(status, 200, "{response}");
        let connection = v2_ops::open_db(&path).unwrap();
        let excerpt: String = connection
            .query_row("SELECT raw_excerpt FROM inbox_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(excerpt.chars().count(), 2000);
        assert_eq!(excerpt, "字".repeat(2000));
        assert!(!excerpt.contains("github.com"));
    }

    #[test]
    fn sending_the_same_capture_again_is_a_duplicate_batch() {
        let (path, full, _) = capture_db("dup");
        let body = r#"{"text":"再发一次 https://github.com/acme/twice","sourceContext":"第一次"}"#;
        let (status, first_body) = post_capture(&path, &full, body);
        assert_eq!(status, 200, "{first_body}");
        let first = assert_commit_keys(&first_body);
        assert_eq!(first["duplicateBatch"], json!(false));
        assert_eq!(first["created"], json!(1));
        let again = r#"{"text":"再发一次 https://github.com/acme/twice","sourceContext":"第二次，上下文不同也不重新收"}"#;
        let (status, second_body) = post_capture(&path, &full, again);
        assert_eq!(status, 200, "{second_body}");
        let second = assert_commit_keys(&second_body);
        assert_eq!(second["duplicateBatch"], json!(true));
        assert_eq!(second["created"], json!(0));
        assert_eq!(second["linksFound"], json!(1));
        assert_eq!(second["skipped"], json!(1));
    }

    #[test]
    fn capture_counts_repositories_already_starred() {
        let (path, full, _) = capture_db("starred");
        insert_starred_tool(&path);
        let (status, body) =
            post_capture(&path, &full, r#"{"urls":["https://github.com/Fox/Tool"]}"#);
        assert_eq!(status, 200, "{body}");
        let value = assert_commit_keys(&body);
        assert_eq!(value["alreadyStarred"], json!(1));
        assert_eq!(value["created"], json!(0));
        assert_eq!(value["blocked"], json!(0));
        assert_eq!(value["linksFound"], json!(1));
        assert_eq!(value["skipped"], json!(1));
    }

    #[test]
    fn capture_counts_blocked_private_addresses() {
        let (path, full, _) = capture_db("blocked");
        let (status, body) = post_capture(
            &path,
            &full,
            r#"{"urls":["http://192.168.1.10/admin","https://github.com/acme/public"]}"#,
        );
        assert_eq!(status, 200, "{body}");
        let value = assert_commit_keys(&body);
        assert_eq!(value["blocked"], json!(1));
        assert_eq!(value["created"], json!(1));
        assert_eq!(value["linksFound"], json!(2));
        assert_eq!(value["skipped"], json!(1));
    }

    #[test]
    fn capture_with_no_link_is_still_ok() {
        let (path, full, _) = capture_db("no-link");
        let (status, body) = post_capture(&path, &full, r#"{"text":"只有文字，没有链接。"}"#);
        assert_eq!(status, 200, "{body}");
        let value = assert_commit_keys(&body);
        assert_eq!(value["linksFound"], json!(0));
        assert_eq!(value["created"], json!(0));
        assert_eq!(value["ok"], json!(true));
        let connection = v2_ops::open_db(&path).unwrap();
        let batches: i64 = connection
            .query_row("SELECT COUNT(*) FROM capture_batches", [], |row| row.get(0))
            .unwrap();
        assert_eq!(batches, 0);
    }

    #[test]
    fn capture_response_json_has_only_the_commit_keys() {
        let (path, full, _) = capture_db("json-keys");
        let (status, body) = post_capture(
            &path,
            &full,
            r#"{"urls":["https://github.com/acme/json-keys"]}"#,
        );
        assert_eq!(status, 200, "{body}");
        let value = assert_commit_keys(&body);
        assert_eq!(value["created"], json!(1));
        assert_eq!(value["duplicateBatch"], json!(false));
    }

    #[test]
    fn a_new_capture_token_file_is_private() {
        let path = temp_db("mode");
        let fresh = ensure_capture_token(&path).unwrap();
        assert_eq!(fresh.len(), 64);
        let again = ensure_capture_token(&path).unwrap();
        assert_eq!(again, fresh);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(control_path(&path))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn saving_the_capture_token_keeps_the_other_control_keys() {
        let path = temp_db("keep-keys");
        let full = generate_token().unwrap();
        write_control(
            &path,
            &json!({
                "enabled": true,
                "host": HOST,
                "port": PORT,
                "token": full,
                "note": "keep"
            }),
        )
        .unwrap();
        let capture = rotate_capture_token(&path).unwrap();
        let control = read_control(&path);
        assert_eq!(control["enabled"], json!(true));
        assert_eq!(control["host"].as_str(), Some(HOST));
        assert_eq!(control["port"], json!(PORT));
        assert_eq!(control["token"].as_str(), Some(full.as_str()));
        assert_eq!(control["captureToken"].as_str(), Some(capture.as_str()));
        assert_eq!(control["note"].as_str(), Some("keep"));
        let new_full = rotate_token(&path).unwrap();
        let control = read_control(&path);
        assert_eq!(control["token"].as_str(), Some(new_full.as_str()));
        assert_eq!(control["captureToken"].as_str(), Some(capture.as_str()));
        assert_eq!(control["enabled"], json!(true));
        assert_eq!(control["note"].as_str(), Some("keep"));
        let new_capture = rotate_capture_token(&path).unwrap();
        let control = read_control(&path);
        assert_eq!(control["captureToken"].as_str(), Some(new_capture.as_str()));
        assert_eq!(control["token"].as_str(), Some(new_full.as_str()));
        assert_eq!(control["enabled"], json!(true));
        assert_eq!(control["host"].as_str(), Some(HOST));
        assert_eq!(control["note"].as_str(), Some("keep"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(control_path(&path))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn capture_database_errors_stay_generic() {
        let (path, full, _) = capture_db("leak");
        let connection = v2_ops::open_db(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER fail_capture BEFORE INSERT ON inbox_items BEGIN SELECT RAISE(ABORT, 'boom-internal'); END;",
            )
            .unwrap();
        drop(connection);
        let (status, body) = post_capture(
            &path,
            &full,
            r#"{"urls":["https://example.com/secret-path"]}"#,
        );
        assert_eq!(status, 500);
        assert_eq!(body, r#"{"error":"收集写入失败"}"#);
        assert!(!body.contains("boom"));
        assert!(!body.contains("secret"));
        assert!(!body.contains(&path.display().to_string()));
    }
}
