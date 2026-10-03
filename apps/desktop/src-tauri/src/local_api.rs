//! 本机 HTTP 接口，给 CLI、web-search、以后的微信工作台用。
//!
//! 安全模型（默认关闭，开启后也只在本机）：
//! - 只监听 127.0.0.1；
//! - 除 `GET /v1/health` 外都要 `Authorization: Bearer <令牌>`，令牌 256 位随机，存在 0600 权限的控制文件里；
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
    let Some(expected) = token(database_path) else {
        return (
            503,
            error_body("令牌还没生成，请在工具箱里重新开启本地 API"),
        );
    };
    let presented = request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    if !constant_time_eq(presented.as_bytes(), expected.as_bytes()) {
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
}
