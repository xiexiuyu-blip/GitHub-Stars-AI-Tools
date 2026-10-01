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

pub fn is_enabled(database_path: &Path) -> bool {
    std::fs::read_to_string(control_path(database_path))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.get("enabled").and_then(Value::as_bool))
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
    }
}

pub fn set_enabled(database_path: &Path, enabled: bool) -> Result<LocalApiStatus, String> {
    if let Some(parent) = control_path(database_path).parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("本地 API 配置目录创建失败：{error}"))?;
    }
    std::fs::write(
        control_path(database_path),
        json!({ "enabled": enabled, "host": HOST, "port": PORT }).to_string(),
    )
    .map_err(|error| format!("本地 API 配置写入失败：{error}"))?;
    if enabled {
        start(database_path.to_path_buf())?;
    } else {
        stop();
    }
    Ok(status(database_path))
}

pub fn start_if_enabled(database_path: &Path) -> Result<(), String> {
    if is_enabled(database_path) {
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
    let mut runtime = runtime().lock().map_err(|_| "本地 API 状态锁损坏".to_owned())?;
    if runtime.thread.is_some() {
        return Ok(());
    }
    let listener = TcpListener::bind(addr).map_err(|error| format!("本地 API 绑定 127.0.0.1:{PORT} 失败：{error}"))?;
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
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    handle_connection(stream, &database_path);
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
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]);
    let (status, body) = route(database_path, &request);
    let _ = write_raw(&stream, status, &body);
}

pub fn route(database_path: &Path, request: &str) -> (u16, String) {
    if !is_enabled(database_path) {
        return (403, r#"{"error":"本地 API 默认关闭"}"#.to_owned());
    }
    let mut lines = request.lines();
    let Some(start) = lines.next() else {
        return (400, r#"{"error":"请求不完整"}"#.to_owned());
    };
    let mut parts = start.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    match (method, path) {
        ("GET", "/v1/health") => (
            200,
            json!({ "ok": true, "host": HOST, "port": PORT }).to_string(),
        ),
        ("GET", "/v1/search") => match v2_ops::list_workspace(database_path, &query_value(target, "q"), "", "", 20, 0) {
            Ok(page) => (200, serde_json::to_string(&page).unwrap_or_else(|_| r#"{"error":"结果序列化失败"}"#.to_owned())),
            Err(error) => (500, json!({ "error": error }).to_string()),
        },
        ("POST", "/v1/usage") => {
            let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
            match serde_json::from_str::<Value>(body) {
                Ok(value) => {
                    let entity_id = value.get("entityId").and_then(Value::as_str).unwrap_or("");
                    let verdict = value.get("verdict").and_then(Value::as_str).unwrap_or("");
                    let note = value.get("note").and_then(Value::as_str).unwrap_or("");
                    match v2_ops::log_usage(database_path, entity_id, verdict, note, "local_api") {
                        Ok(()) => (200, r#"{"ok":true}"#.to_owned()),
                        Err(error) => (400, json!({ "error": error }).to_string()),
                    }
                }
                Err(_) => (400, r#"{"error":"使用记录必须是 JSON"}"#.to_owned()),
            }
        }
        _ => (404, r#"{"error":"没有这个本地接口"}"#.to_owned()),
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
            if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""), 16) {
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
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Error",
    };
    let payload = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(payload.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_api_refuses_non_loopback_and_defaults_off() {
        assert!(refuse_non_loopback(SocketAddr::from(([0, 0, 0, 0], 80))).is_err());
        assert!(refuse_non_loopback(loopback_addr()).is_ok());
        let path = std::env::temp_dir().join(format!("fsl-api-missing-{}.sqlite3", std::process::id()));
        assert!(!is_enabled(&path));
        let (status, body) = route(&path, "GET /v1/health HTTP/1.1\r\n\r\n");
        assert_eq!(status, 403);
        assert!(body.contains("默认关闭"));
    }
}
