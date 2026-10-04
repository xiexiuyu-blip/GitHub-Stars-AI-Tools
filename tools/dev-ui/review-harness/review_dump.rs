// 总控的临时审核夹具（不进仓库、不合并）：拿一份「开发库的副本」，
// 跑一遍迁移和所有新后端函数，把真实结果存成 JSON，给假后端渲染界面用。
// 只读源文件的副本；一切写入都落在 FSL_REVIEW_OUT 目录里。
#![cfg(test)]

use crate::{db::migrations::migrate_database, inbox_view, library_view, packs, scenarios, solve};
use rusqlite::Connection;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

fn dump<T: Serialize>(dir: &Path, name: &str, value: &T) {
    fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_string_pretty(value).expect("序列化"),
    )
    .expect("写文件");
}

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = f();
    println!("[计时] {label}: {:?}", start.elapsed());
    value
}

fn filters(json: &str) -> library_view::LibraryFilters {
    serde_json::from_str(json).expect("筛选条件")
}

#[test]
#[ignore]
fn review_dump() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("work.sqlite3");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    fs::copy(&source, &work).expect("复制副本");

    let report = timed("迁移", || migrate_database(&work)).expect("迁移应成功");
    println!("[迁移] 新应用：{:?}；备份：{:?}", report.applied, report.backup_path);

    let seeded = timed("种子", || scenarios::seed_scenarios(&work)).expect("种子");
    println!("[场景] 种子写入 {seeded} 条");
    let stats = timed("规则归类（全部）", || {
        scenarios::classify_repositories(&work, None)
    })
    .expect("归类");
    dump(&out, "classify_stats", &stats);
    let summary = scenarios::list_scenarios_summary(&work).expect("场景列表");
    dump(&out, "list_scenarios", &summary);

    let default_page = timed("资料库默认页", || {
        library_view::list_library_view(&work, &filters("{}"))
    })
    .expect("默认页");
    dump(&out, "library_default", &default_page);
    let all_page = timed("资料库全部（卡片状态=全部）", || {
        library_view::list_library_view(&work, &filters(r#"{"cardState":"all","limit":200}"#))
    })
    .expect("全部页");
    dump(&out, "library_all", &all_page);
    for (name, f) in [
        ("library_sort_stars", r#"{"cardState":"all","sort":"stars"}"#),
        ("library_pending", r#"{"cardState":"pending"}"#),
        ("library_keyword", r#"{"cardState":"all","keyword":"图"}"#),
        ("library_reading_want", r#"{"cardState":"all","reading":"want"}"#),
    ] {
        let page = library_view::list_library_view(&work, &filters(f)).expect(name);
        dump(&out, name, &page);
    }
    if let Some(first) = summary.iter().find(|s| s.count > 0) {
        let f = format!(r#"{{"cardState":"all","scenario":"{}"}}"#, first.id);
        let page = library_view::list_library_view(&work, &filters(&f)).expect("场景筛选");
        dump(&out, "library_scenario_first", &page);
    }

    let today = timed("今日", || {
        library_view::get_today_view(&work, "2026-10-03T12:00:00Z")
    })
    .expect("今日");
    dump(&out, "today", &today);

    // 详情：默认页前 3 个、今日推荐的前 2 个、一个没有卡的
    let mut ids: Vec<String> = default_page.items.iter().take(3).map(|i| i.id.clone()).collect();
    ids.extend(today.picks.iter().take(2).map(|i| i.id.clone()));
    if let Some(no_card) = all_page.items.iter().find(|i| !i.has_card) {
        ids.push(no_card.id.clone());
    }
    ids.dedup();
    let mut details = serde_json::Map::new();
    for id in &ids {
        let detail = timed("详情", || library_view::get_library_detail(&work, id)).expect("详情");
        details.insert(id.clone(), serde_json::to_value(&detail).expect("详情序列化"));
    }
    dump(&out, "details", &details);

    // 「复制给 Codex」的文字
    let connection = Connection::open(&work).expect("打开副本");
    let mut codex = serde_json::Map::new();
    for id in ids.iter().take(3) {
        let text = packs::render_item_for_codex(&connection, id).expect("复制给 Codex");
        codex.insert(id.clone(), serde_json::Value::String(text));
    }
    dump(&out, "copy_for_codex", &codex);
    drop(connection);

    // 找方案
    let mut solved = serde_json::Map::new();
    for query in ["做小红书封面", "给视频加字幕", "抓取网页内容", "公众号文章配图", "xyzzy不存在的东西"] {
        let result = timed(&format!("找方案：{query}"), || solve::solve_query(&work, query))
            .expect("找方案");
        solved.insert(query.to_owned(), serde_json::to_value(&result).expect("序列化"));
    }
    dump(&out, "solve", &solved);

    // 收集
    let text = "看看这个 https://github.com/anthropics/claude-code 还有 https://x.com/someone/status/1234567890 \
                再来一个内网的 http://192.168.1.10:8080/x 和 http://localhost:3000";
    let preview = timed("收集预览", || inbox_view::preview_capture(&work, text)).expect("预览");
    dump(&out, "preview_capture", &preview);
    let inbox = inbox_view::list_inbox(&work, Some("pending"), Some(100), Some(0)).expect("收集箱");
    dump(&out, "list_inbox", &inbox);

    println!("[完成] 输出目录：{}", out.display());
}

// ───────────────────────── 真后端桥（总控临时夹具）─────────────────────────
// 在 127.0.0.1 上起一个很小的 HTTP 服务，把界面发来的 invoke 命令交给**真实的** Rust 模块函数处理，
// 数据库是开发库的副本。界面经假 Tauri 外壳（shim.js）转发到这里，就能用真逻辑、真数据验收界面。
// 没实现的命令返回 {"missing":true}，由 shim 退回假数据。

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

fn arg_str(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("缺少参数 {key}"))
}

fn call(work: &Path, cmd: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    use serde_json::{from_value, to_value, Value};
    let ser = |v: Result<Value, serde_json::Error>| v.map_err(|e| format!("序列化失败：{e}"));
    match cmd {
        "list_scenarios" => {
            scenarios::seed_scenarios(work)?;
            ser(to_value(scenarios::list_scenarios_summary(work)?))
        }
        "list_library_view" => {
            let filters: library_view::LibraryFilters =
                from_value(args.get("filters").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("筛选条件无效：{e}"))?;
            ser(to_value(library_view::list_library_view(work, &filters)?))
        }
        "get_library_detail" => ser(to_value(library_view::get_library_detail(
            work,
            &arg_str(args, "id")?,
        )?)),
        "set_library_reading" => {
            let ids: Vec<String> = from_value(args.get("ids").cloned().unwrap_or(Value::Null))
                .map_err(|e| format!("ids 无效：{e}"))?;
            let changed = library_view::set_library_reading(work, &ids, &arg_str(args, "group")?)?;
            Ok(Value::from(changed as i64))
        }
        "get_today_view" => {
            let now = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|_| "时间格式化失败".to_owned())?;
            ser(to_value(library_view::get_today_view(work, &now)?))
        }
        "commit_capture_batch" => ser(to_value(crate::v2_ops::commit_capture(
            work,
            &arg_str(args, "text")?,
            args.get("sourceContext").and_then(|v| v.as_str()).unwrap_or(""),
        )?)),
        "get_ai_automation" => ser(to_value(crate::card_handler::automation_status(
            work,
            &serde_json::json!({"ai": {"provider": "openai", "model": "LongCat-2.0"}}),
        )?)),
        "set_ai_automation" => ser(to_value(crate::card_handler::set_automation_budget(
            work,
            &serde_json::json!({"ai": {"provider": "openai", "model": "LongCat-2.0"}}),
            args.get("dailyBudgetTokens").and_then(|v| v.as_u64()).unwrap_or(0),
        )?)),
        "start_full_scan" => ser(to_value(crate::card_handler::start_full_scan(
            work,
            &serde_json::json!({"ai": {"provider": "openai", "model": "LongCat-2.0"}}),
            args.get("confirmed").and_then(|v| v.as_bool()).unwrap_or(false),
        )?)),
        "stop_full_scan" => ser(to_value(crate::card_handler::stop_full_scan(
            work,
            &serde_json::json!({"ai": {"provider": "openai", "model": "LongCat-2.0"}}),
        )?)),
        "enqueue_cards_now" => Ok(Value::from(
            crate::card_handler::enqueue_cards_for_scope(work, &arg_str(args, "scope")?)? as i64,
        )),
        "confirm_library_card" => {
            crate::card_handler::confirm_card(work, &arg_str(args, "id")?)?;
            Ok(Value::Null)
        }
        "regenerate_library_card" => {
            crate::card_handler::request_regenerate(work, &arg_str(args, "id")?)?;
            Ok(Value::Null)
        }
        "get_repository_detail" => {
            // 现有命令：详情页读 README 原文。返回形状同 storage::RepositoryDetailView。
            let request = args.get("request").cloned().unwrap_or(Value::Null);
            let repository_id = request
                .get("repositoryId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "缺少 repositoryId".to_owned())?
                .to_owned();
            let connection = Connection::open(work).map_err(|e| e.to_string())?;
            let readme: Option<(String, String, String, String)> = connection
                .query_row(
                    "SELECT raw_markdown, content_hash, source_path, fetched_at FROM repo_readmes WHERE repo_id = ?1",
                    [&repository_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .ok();
            Ok(serde_json::json!({
                "repositoryId": repository_id,
                "accountId": request.get("accountId").cloned().unwrap_or(Value::Null),
                "readme": readme.map(|(raw, hash, path, at)| serde_json::json!({
                    "rawMarkdown": raw, "contentHash": hash, "sourcePath": path, "fetchedAt": at
                })),
                "aiDocument": Value::Null,
            }))
        }
        "open_external_url" => Ok(Value::Null),
        "solve_v2" => ser(to_value(solve::solve_query(work, &arg_str(args, "query")?)?)),
        // 本机已装的 Skill / MCP：读真实的 HOME（只读名字和一句话）
        "scan_local_capabilities" => ser(to_value(crate::local_scan::scan_local_capabilities()?)),
        // 发现 / 社区：这两个会真的联网（GitHub 搜索、Hacker News），不登录；结果写进副本库里的缓存表
        "list_discover" => ser(to_value(crate::discover::list_discover_view(
            work,
            args.get("scenarioId").and_then(|v| v.as_str()),
        )?)),
        "refresh_discover" => ser(to_value(crate::discover::refresh_discover_with(
            &crate::web_fetch::CurlFetcher,
            work,
            &arg_str(args, "scenarioId")?,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )?)),
        "get_repo_community" => ser(to_value(crate::community::get_repo_community_with(
            &crate::web_fetch::CurlFetcher,
            work,
            &arg_str(args, "id")?,
            args.get("refresh").and_then(|v| v.as_bool()).unwrap_or(false),
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )?)),
        "preview_capture" => ser(to_value(inbox_view::preview_capture(
            work,
            &arg_str(args, "text")?,
        )?)),
        "list_inbox" => ser(to_value(inbox_view::list_inbox(
            work,
            args.get("status").and_then(|v| v.as_str()),
            args.get("limit").and_then(|v| v.as_i64()),
            args.get("offset").and_then(|v| v.as_i64()),
        )?)),
        "set_inbox_status" => {
            inbox_view::set_inbox_status(work, &arg_str(args, "id")?, &arg_str(args, "status")?)?;
            Ok(Value::Null)
        }
        "set_inbox_note" => {
            inbox_view::set_inbox_note(work, &arg_str(args, "id")?, &arg_str(args, "note")?)?;
            Ok(Value::Null)
        }
        "fetch_inbox_pending" => {
            let id = args.get("id").and_then(Value::as_str);
            ser(to_value(crate::inbox_fetch::fetch_pending(
                work,
                &crate::inbox_fetch::SafePageFetcher,
                id,
                20,
            )?))
        }
        "copy_item_for_codex" => {
            let connection = Connection::open(work).map_err(|e| e.to_string())?;
            let problem = args.get("problem").and_then(Value::as_str);
            Ok(Value::String(packs::render_item_for_codex_with(
                &connection,
                &arg_str(args, "repoId")?,
                problem,
            )?))
        }
        "build_reference_pack_v2" => {
            let request: packs::PackRequest =
                from_value(args.get("request").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("参考包请求无效：{e}"))?;
            let format: packs::PackFormat =
                from_value(args.get("format").cloned().unwrap_or(Value::Null))
                    .map_err(|e| format!("参考包格式无效：{e}"))?;
            let generated_on = packs::resolve_generated_on(
                args.get("generatedOn").and_then(|v| v.as_str()),
            );
            let connection = Connection::open(work).map_err(|e| e.to_string())?;
            ser(to_value(packs::build_pack(
                &connection,
                &request,
                format,
                &generated_on,
            )?))
        }
        _ => Err(format!("MISSING:{cmd}")),
    }
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let text = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\n\
         Access-Control-Allow-Methods: POST, OPTIONS\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );
    let _ = stream.write_all(text.as_bytes());
}

fn handle(mut stream: TcpStream, work: &Path) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 1 << 20 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let first = head.lines().next().unwrap_or("").to_owned();
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");
    if method == "OPTIONS" {
        respond(&mut stream, "204 No Content", "");
        return;
    }
    let length = head
        .lines()
        .find_map(|l| {
            let lower = l.to_lowercase();
            lower
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
        })
        .unwrap_or(0);
    while buf.len() < header_end + length {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = &buf[header_end..buf.len().min(header_end + length)];
    let args: serde_json::Value = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(body).unwrap_or(serde_json::Value::Null)
    };
    let cmd = path.trim_start_matches("/invoke/");
    let start = Instant::now();
    let result = call(work, cmd, &args);
    let elapsed = start.elapsed();
    let payload = match result {
        Ok(value) => {
            println!("[真后端] {cmd} 成功 {elapsed:?}");
            serde_json::json!({ "ok": value })
        }
        Err(message) if message.starts_with("MISSING:") => {
            serde_json::json!({ "missing": true })
        }
        Err(message) => {
            println!("[真后端] {cmd} 失败：{message}");
            serde_json::json!({ "err": message })
        }
    };
    respond(&mut stream, "200 OK", &payload.to_string());
}

#[test]
#[ignore]
fn review_server() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    let port: u16 = std::env::var("FSL_REVIEW_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(38500);
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("server-work.sqlite3");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    fs::copy(&source, &work).expect("复制副本");
    migrate_database(&work).expect("迁移");
    scenarios::seed_scenarios(&work).expect("种子");
    scenarios::classify_repositories(&work, None).expect("归类");
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("监听端口");
    println!("[真后端] 已就绪：http://127.0.0.1:{port}  数据库副本：{}", work.display());
    for stream in listener.incoming().flatten() {
        handle(stream, &work);
    }
}

// ───────────────── 流水线演练：假的 OpenAI 兼容服务 + 真的队列 / 处理器 / 理解卡流水线 ─────────────────
// 不花钱、不读钥匙串：模型是本机一个小线程，返回固定的合法理解卡和 usage。
// 目的是在真实数据（开发库副本）上把「排队 → 预算 → 领任务 → 调模型 → 校验 → 写库 → 场景标签 → 记账 → 到预算就停」整条线跑一遍。

use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;

fn canned_card() -> String {
    serde_json::json!({
        "one_liner_zh": "演练用的一句话说明",
        "what_it_is": "这是本机假模型返回的演练说明，用来验证整条流水线能走通。",
        "form": "app",
        "primary_category": "dev-infra",
        "capabilities": [{"name_zh": "演练能力"}],
        "problems_solved": ["演练要解决的问题"],
        "how_to_try": "",
        "maturity": "usable",
        "zh_friendly": true,
        "confidence": 0.8,
        "keywords": ["演练", "测试"],
        "scenario_tags": ["xhs"]
    })
    .to_string()
}

/// 假模型的脾气：正常 / 输出额度小于门槛就被截断（只有思考没有正文，和 LongCat 的真实表现一样）/ 输出额度超过上限就 HTTP 400 拒绝。
#[derive(Clone, Copy)]
enum FakeMode {
    Plain,
    CutOffBelow(u64),
    RejectAbove(u64),
}

fn start_fake_llm(served: Arc<AtomicUsize>, delay_ms: u64) -> u16 {
    start_fake_llm_with(served, delay_ms, FakeMode::Plain)
}

fn start_fake_llm_with(served: Arc<AtomicUsize>, delay_ms: u64, mode: FakeMode) -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("监听假模型端口");
    let port = listener.local_addr().expect("端口").port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let served = served.clone();
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                let header_end = loop {
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break pos + 4;
                    }
                };
                let head = String::from_utf8_lossy(&buf[..header_end]).to_lowercase();
                let length = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                    .unwrap_or(0);
                while buf.len() < header_end + length {
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                served.fetch_add(1, AtomicOrdering::SeqCst);
                if delay_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                }
                let request_body = String::from_utf8_lossy(&buf[header_end..]).to_string();
                let max_tokens = serde_json::from_str::<serde_json::Value>(&request_body)
                    .ok()
                    .and_then(|value| value.get("max_tokens").and_then(|v| v.as_u64()))
                    .unwrap_or(0);
                let (status, body) = match mode {
                    FakeMode::RejectAbove(limit) if max_tokens > limit => (
                        "400 Bad Request",
                        serde_json::json!({"error": {"message": format!("max_tokens is too large: {max_tokens}. This model supports at most {limit} completion tokens.")}}).to_string(),
                    ),
                    FakeMode::CutOffBelow(threshold) if max_tokens < threshold => (
                        "200 OK",
                        serde_json::json!({
                            "id": "fake", "model": "fake-model",
                            "choices": [{"finish_reason": "length", "message": {"role": "assistant", "content": "", "reasoning_content": "想了很多很多……"}}],
                            "usage": {"prompt_tokens": 3000, "completion_tokens": max_tokens}
                        })
                        .to_string(),
                    ),
                    _ => (
                        "200 OK",
                        serde_json::json!({
                            "id": "fake",
                            "model": "fake-model",
                            "choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": canned_card()}}],
                            "usage": {"prompt_tokens": 3000, "completion_tokens": 600}
                        })
                        .to_string(),
                    ),
                };
                let text = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.as_bytes().len()
                );
                let _ = stream.write_all(text.as_bytes());
            });
        }
    });
    port
}

#[test]
#[ignore]
fn review_pipeline_rehearsal() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("rehearsal.sqlite3");
    for suffix in ["", "-wal", "-shm", ".queue.json"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    let _ = fs::remove_file(crate::jobs::queue_control_path(&work));
    fs::copy(&source, &work).expect("复制副本");
    migrate_database(&work).expect("迁移");
    scenarios::ensure_initial_classification(&work).expect("首次归类");

    let served = Arc::new(AtomicUsize::new(0));
    let port = start_fake_llm(served.clone(), 0);
    println!("[演练] 假模型：http://127.0.0.1:{port}/v1");

    // 1) 排队：没有授权预算时不应有任何调用
    let queued_missing =
        crate::card_handler::enqueue_cards_for_scope(&work, "missing").expect("排队 missing");
    println!("[演练] 新排队（没有理解卡的）：{queued_missing}");
    let config_fn: Arc<dyn Fn() -> Result<crate::ai::AiRequestConfig, String> + Send + Sync> =
        Arc::new(move || {
            Ok(crate::ai::AiRequestConfig {
                provider: "openai-compatible".to_owned(),
                api_key: String::new(),
                base_url: Some(format!("http://127.0.0.1:{port}/v1")),
                model: "fake-model".to_owned(),
            })
        });
    let handler = crate::card_handler::RepoCardHandler::new(config_fn);
    let worker = crate::jobs::start_worker(&work, vec![Arc::new(handler)]);
    std::thread::sleep(std::time::Duration::from_secs(5));
    println!(
        "[演练] 预算 0 时 5 秒后：模型被调用 {} 次（应为 0）",
        served.load(AtomicOrdering::SeqCst)
    );
    assert_eq!(served.load(AtomicOrdering::SeqCst), 0, "没有预算不该调用模型");

    // 2) 授权 30000 token：每次假调用记 3600，预计到第 9 张附近停下
    crate::jobs::set_daily_budget(&crate::jobs::queue_control_path(&work), 30_000).expect("设预算");
    let start = Instant::now();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let status = crate::jobs::queue_status(&work).expect("队列状态");
        if status.state == "budget_reached" || start.elapsed().as_secs() > 90 {
            println!(
                "[演练] {} 秒后：状态={} 排队={} 进行中={} 失败={} 24小时已用={} 剩余={}",
                start.elapsed().as_secs(),
                status.state,
                status.queued,
                status.running,
                status.failed,
                status.tokens_24h,
                status.budget_remaining
            );
            break;
        }
    }
    // 再等 3 秒确认真的停了
    std::thread::sleep(std::time::Duration::from_secs(3));
    let calls = served.load(AtomicOrdering::SeqCst);
    println!("[演练] 模型总调用次数：{calls}（预算 30000 / 每次 3600，预期 9 左右）");

    let connection = Connection::open(&work).expect("打开副本");
    let one = |sql: &str| -> i64 { connection.query_row(sql, [], |r| r.get(0)).expect(sql) };
    println!(
        "[演练] ai_jobs：succeeded={} failed={} queued={} running={}",
        one("SELECT COUNT(*) FROM ai_jobs WHERE status='succeeded'"),
        one("SELECT COUNT(*) FROM ai_jobs WHERE status='failed'"),
        one("SELECT COUNT(*) FROM ai_jobs WHERE status='queued'"),
        one("SELECT COUNT(*) FROM ai_jobs WHERE status='running'")
    );
    println!(
        "[演练] 新理解卡（schema 版本 2）：{}；AI 场景建议行：{}",
        one("SELECT COUNT(*) FROM repo_ai_documents WHERE card_schema_version >= 2"),
        one("SELECT COUNT(*) FROM entity_scenarios WHERE origin='ai'")
    );
    let sample: Option<(String, String, i64, i64)> = connection
        .query_row(
            "SELECT target_id, IFNULL(error_message,''), input_tokens, output_tokens FROM ai_jobs WHERE status='failed' LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .ok();
    println!("[演练] 失败样例：{sample:?}");
    worker.stop();
    println!(
        "[演练] 控制文件：{}",
        fs::read_to_string(crate::jobs::queue_control_path(&work)).unwrap_or_default()
    );
}

/// 全量整理演练：假模型 + 真队列 / 真处理器 / 真工作线程，在开发库副本上跑。不花钱、不读钥匙串。
/// 日常额度到线就停 → 开始全量整理（必须确认）一路做 → 中途停止后回到每天额度 → 再开始，整理完自动结束。
#[test]
#[ignore]
fn review_full_scan_rehearsal() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("fullscan.sqlite3");
    for suffix in ["", "-wal", "-shm", ".queue.json"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    let _ = fs::remove_file(crate::jobs::queue_control_path(&work));
    fs::copy(&source, &work).expect("复制副本");
    migrate_database(&work).expect("迁移");
    scenarios::ensure_initial_classification(&work).expect("首次归类");

    let served = Arc::new(AtomicUsize::new(0));
    let port = start_fake_llm(served.clone(), 120);
    let settings = serde_json::json!({"ai": {"provider": "openai-compatible", "model": "fake-model"}});
    let config_fn: Arc<dyn Fn() -> Result<crate::ai::AiRequestConfig, String> + Send + Sync> =
        Arc::new(move || {
            Ok(crate::ai::AiRequestConfig {
                provider: "openai-compatible".to_owned(),
                api_key: String::new(),
                base_url: Some(format!("http://127.0.0.1:{port}/v1")),
                model: "fake-model".to_owned(),
            })
        });
    let handler = crate::card_handler::RepoCardHandler::new(config_fn);
    let baseline = {
        let connection = Connection::open(&work).expect("打开副本");
        let one = |sql: &str| -> i64 { connection.query_row(sql, [], |r| r.get(0)).expect(sql) };
        (
            one("SELECT COUNT(*) FROM ai_jobs WHERE status='succeeded' AND input_tokens > 0"),
            one("SELECT COUNT(*) FROM ai_jobs WHERE status='failed'"),
        )
    };
    let worker = crate::jobs::start_worker(&work, vec![Arc::new(handler)]);
    let control = crate::jobs::queue_control_path(&work);
    let calls = || served.load(AtomicOrdering::SeqCst);

    // 0) 没有授权时：全量整理要确认、也不会悄悄开
    let refused = crate::card_handler::start_full_scan(&work, &settings, false);
    println!("[演练] 没确认就开始：{refused:?}");
    assert!(refused.is_err());

    // 1) 日常额度 3 万：到线就停（每次假调用记 3600）
    let queued = crate::card_handler::enqueue_cards_for_scope(&work, "missing").expect("排队 missing")
        + crate::card_handler::enqueue_cards_for_scope(&work, "outdated").expect("排队 outdated");
    crate::jobs::set_daily_budget(&control, 30_000).expect("设预算");
    let begun = Instant::now();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let status = crate::jobs::queue_status(&work).expect("队列状态");
        if status.state == "budget_reached" || begun.elapsed().as_secs() > 120 {
            break;
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(2));
    let at_cap = calls();
    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("[演练] 排队 {queued} 个；日常额度到线时调用 {at_cap} 次，再等 3 秒 {} 次", calls());
    assert_eq!(calls(), at_cap, "到线之后不该再调用模型");
    let capped = crate::card_handler::automation_status(&work, &settings).expect("状态");
    println!(
        "[演练] 到线时：enabled={} unlimited={} 已用={} 剩余额度={} 待整理={}",
        capped.enabled, capped.unlimited, capped.tokens_24h, capped.budget_remaining, capped.needs_organizing
    );
    assert!(!capped.unlimited);

    // 2) 开始全量整理（确认）：不看每天额度一路做；中途停止
    let started = crate::card_handler::start_full_scan(&work, &settings, true).expect("开始全量整理");
    println!(
        "[演练] 开始：unlimited={} enabled={} total={:?}",
        started.unlimited,
        started.enabled,
        started.full_scan.as_ref().map(|scan| scan.total)
    );
    assert!(started.unlimited && started.enabled);
    std::thread::sleep(std::time::Duration::from_secs(5));
    let mid = crate::card_handler::automation_status(&work, &settings).expect("状态");
    let mid_scan = mid.full_scan.clone().expect("进行中应有进度");
    println!(
        "[演练] 5 秒后：已用 {} 剩余 {} / {}，调用 {}",
        mid_scan.tokens_used,
        mid_scan.remaining,
        mid_scan.total,
        calls()
    );
    assert!(calls() > at_cap, "全量整理应当继续调用");
    assert!(mid.tokens_24h > 30_000, "用量应当超过每天额度");
    let stopped = crate::card_handler::stop_full_scan(&work, &settings).expect("停止");
    assert!(!stopped.unlimited && stopped.full_scan.is_none());
    println!("[演练] 停止：{:?}", stopped.last_full_scan);
    std::thread::sleep(std::time::Duration::from_secs(2)); // 在跑的做完
    let after_stop = calls();
    std::thread::sleep(std::time::Duration::from_secs(4));
    println!("[演练] 停止后调用 {after_stop} → 再等 4 秒 {}", calls());
    assert_eq!(calls(), after_stop, "停止之后回到每天额度，用量早已超线，不该再调用模型");

    // 3) 再开始，一路做完，整理完自动结束
    crate::card_handler::start_full_scan(&work, &settings, true).expect("再次开始");
    let begun = Instant::now();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let status = crate::card_handler::automation_status(&work, &settings).expect("状态");
        let progress = status.full_scan.as_ref().map(|scan| (scan.remaining, scan.total, scan.tokens_used));
        println!("[演练] {} 秒：unlimited={} 进度={progress:?} 调用={}", begun.elapsed().as_secs(), status.unlimited, calls());
        if !status.unlimited || begun.elapsed().as_secs() > 600 {
            break;
        }
    }
    let done = crate::card_handler::automation_status(&work, &settings).expect("状态");
    let last = done.last_full_scan.clone().expect("整理完应留下记录");
    println!("[演练] 完成记录：{last:?}；24 小时已用 {}；待整理 {}", done.tokens_24h, done.needs_organizing);
    assert_eq!(last.reason, "completed");
    assert_eq!(last.remaining, 0);
    assert!(!done.unlimited && done.full_scan.is_none());
    assert_eq!(done.queued + done.running, 0);

    // 4) 账：调用次数 = 成功的任务数（没有白花的调用）；回到每天额度
    let connection = Connection::open(&work).expect("打开副本");
    let one = |sql: &str| -> i64 { connection.query_row(sql, [], |r| r.get(0)).expect(sql) };
    let succeeded = one("SELECT COUNT(*) FROM ai_jobs WHERE status='succeeded' AND input_tokens > 0") - baseline.0;
    let failed = one("SELECT COUNT(*) FROM ai_jobs WHERE status='failed'") - baseline.1;
    println!("[演练] 这一轮新增：成功 {succeeded}，失败 {failed}，模型调用 {}", calls());
    assert_eq!(calls() as i64, succeeded + failed, "每次调用都应有对应的任务记账");
    worker.stop();
    println!(
        "[演练] 控制文件：{}",
        fs::read_to_string(crate::jobs::queue_control_path(&work)).unwrap_or_default()
    );
}

/// 输出额度演练：真队列 / 真处理器 / 真流水线 / 真 HTTP，假服务商。看三种服务商脾气下，调用次数和记账对不对：
/// 1) 正常：每张卡 1 次调用，记 3600；2) 额度小于 16384 就被截断（真实记录里的 LongCat）：每张卡 2 次调用，
/// 记账 = 重试那次的 3600 + 被截断那次（输出 8192 + 提示词估计）；3) 输出上限 4096（8192 被 400 拒绝）：每张卡 2 次调用，
/// 被拒绝的那次不计费，记账 3600。不花钱、不读钥匙串。
#[test]
#[ignore]
fn review_output_room_rehearsal() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    fs::create_dir_all(&out).expect("建输出目录");
    for (label, mode) in [
        ("正常", FakeMode::Plain),
        ("被截断（16384 才够）", FakeMode::CutOffBelow(16_000)),
        ("上限 4096", FakeMode::RejectAbove(4_096)),
    ] {
        let work = out.join("room.sqlite3");
        for suffix in ["", "-wal", "-shm", ".queue.json"] {
            let _ = fs::remove_file(format!("{}{suffix}", work.display()));
        }
        let _ = fs::remove_file(crate::jobs::queue_control_path(&work));
        fs::copy(&source, &work).expect("复制副本");
        migrate_database(&work).expect("迁移");
        scenarios::ensure_initial_classification(&work).expect("首次归类");
        crate::card_handler::enqueue_cards_for_scope(&work, "missing").expect("排队");
        // 只留 10 个排队的，演练快一点
        {
            let connection = Connection::open(&work).expect("打开副本");
            connection
                .execute(
                    "DELETE FROM ai_jobs WHERE status = 'queued' AND id NOT IN (SELECT id FROM ai_jobs WHERE status = 'queued' LIMIT 10)",
                    [],
                )
                .expect("只留 10 个");
            connection.execute("DELETE FROM ai_jobs WHERE status <> 'queued'", []).expect("清掉别的");
        }
        // 副本里排队的任务可能带着以前失败尝试记的账：用「增量」比，不比绝对值
        let baseline_tokens: i64 = {
            let connection = Connection::open(&work).expect("打开副本");
            connection
                .query_row(
                    "SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM ai_jobs WHERE status = 'queued'",
                    [],
                    |r| r.get(0),
                )
                .expect("基线")
        };
        let served = Arc::new(AtomicUsize::new(0));
        let port = start_fake_llm_with(served.clone(), 0, mode);
        let config_fn: Arc<dyn Fn() -> Result<crate::ai::AiRequestConfig, String> + Send + Sync> =
            Arc::new(move || {
                Ok(crate::ai::AiRequestConfig {
                    provider: "openai-compatible".to_owned(),
                    api_key: String::new(),
                    base_url: Some(format!("http://127.0.0.1:{port}/v1")),
                    model: "fake-model".to_owned(),
                })
            });
        let handler = crate::card_handler::RepoCardHandler::new(config_fn);
        let worker = crate::jobs::start_worker(&work, vec![Arc::new(handler)]);
        crate::jobs::set_daily_budget(&crate::jobs::queue_control_path(&work), 2_000_000).expect("设预算");
        let begun = Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let status = crate::jobs::queue_status(&work).expect("队列状态");
            if status.queued + status.running == 0 || begun.elapsed().as_secs() > 120 {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        worker.stop();
        let connection = Connection::open(&work).expect("打开副本");
        let one = |sql: &str| -> i64 { connection.query_row(sql, [], |r| r.get(0)).expect(sql) };
        let succeeded = one("SELECT COUNT(*) FROM ai_jobs WHERE status='succeeded'");
        let failed = one("SELECT COUNT(*) FROM ai_jobs WHERE status='failed'");
        let tokens = one("SELECT COALESCE(SUM(input_tokens + output_tokens), 0) FROM ai_jobs WHERE status='succeeded'");
        let calls = served.load(AtomicOrdering::SeqCst) as i64;
        let per_job = if succeeded > 0 { (tokens - baseline_tokens) / succeeded } else { 0 };
        println!("[演练·{label}] 成功 {succeeded} 失败 {failed}；模型被调用 {calls} 次；成功任务平均记账 {per_job} token（总 {tokens}）");
        assert_eq!(succeeded, 10, "{label}：10 个都应成功");
        assert_eq!(failed, 0);
        match mode {
            FakeMode::Plain => {
                assert_eq!(calls, 10);
                assert_eq!(per_job, 3_600);
            }
            FakeMode::CutOffBelow(_) => {
                assert_eq!(calls, 20, "每张卡：被截断一次 + 放大重试一次");
                assert!(per_job >= 3_600 + 8_192, "被截断那次也要记账：{per_job}");
            }
            FakeMode::RejectAbove(_) => {
                assert_eq!(calls, 20, "每张卡：8192 被拒绝一次 + 4096 成功一次");
                assert_eq!(per_job, 3_600, "被拒绝的请求不计费");
            }
        }
    }
}

/// 真任务演练：对 Fox 的几件真实业务（小红书封面、公众号配图、短视频字幕、电商详情页……）跑「找方案」，
/// 取前几名做参考包（Markdown 任务交接单）和单项「复制给 Codex」，全部写进 FSL_REVIEW_OUT，
/// 给「站在 Codex 角度」的审读员看：这份东西能不能让 Codex 直接开工。只读副本，不花钱。
#[test]
#[ignore]
fn review_tasks_dump() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("tasks.sqlite3");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    fs::copy(&source, &work).expect("复制副本");
    migrate_database(&work).expect("迁移");
    scenarios::ensure_initial_classification(&work).expect("首次归类");

    // 默认是 6 个真实任务；FSL_REVIEW_TASKS 指向一个文本文件（每行一句）时用它
    let default_tasks = [
        "做小红书封面",
        "公众号文章配图",
        "给短视频加字幕",
        "做电商商品详情页",
        "把群聊记录整理成日报",
        "批量改图尺寸和压缩",
    ];
    let from_file: Option<Vec<String>> = std::env::var("FSL_REVIEW_TASKS").ok().map(|path| {
        fs::read_to_string(path)
            .expect("读任务文件")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    });
    let tasks: Vec<&str> = match &from_file {
        Some(lines) => lines.iter().map(String::as_str).collect(),
        None => default_tasks.to_vec(),
    };
    let connection = Connection::open(&work).expect("打开副本");
    let mut index = serde_json::Map::new();
    for (number, task) in tasks.iter().enumerate() {
        let result = solve::solve_query(&work, task).expect("找方案");
        let name = format!("task{}", number + 1);
        dump(&out, &format!("{name}_solve"), &result);
        // 界面默认勾选的（preselected）；一个都没有时才退回前 3 个有理解卡的命中，看「如果 Fox 自己勾了」会得到什么
        let preselected: Vec<&solve::SolveHit> = result
            .preselected
            .iter()
            .filter_map(|id| result.hits.iter().find(|hit| &hit.id == id))
            .collect();
        let picks: Vec<&solve::SolveHit> = if preselected.is_empty() {
            result.hits.iter().filter(|hit| hit.has_card).take(3).collect()
        } else {
            preselected
        };
        if picks.is_empty() {
            index.insert((*task).to_owned(), serde_json::json!({"files": [], "note": "没有命中"}));
            continue;
        }
        let request = packs::PackRequest {
            title: (*task).to_owned(),
            problem: (*task).to_owned(),
            architecture_notes: None,
            items: picks
                .iter()
                .enumerate()
                .map(|(position, hit)| packs::PackItemInput {
                    repo_id: hit.id.clone(),
                    role: if position == 0 { packs::PackRole::Main } else { packs::PackRole::Alternative },
                    why: Some(hit.why.clone()),
                })
                .collect(),
            local_capabilities: vec![],
            mode: packs::PackMode::UseTools,
        };
        let generated_on = packs::resolve_generated_on(None);
        let markdown = packs::build_pack(&connection, &request, packs::PackFormat::Markdown, &generated_on)
            .expect("参考包 Markdown");
        fs::write(out.join(format!("{name}_pack.md")), &markdown.content).expect("写参考包");
        // 没有默认勾选的任务：界面上有「没有合适的？直接交给 Codex」，一并生成那份直接交接单
        if result.preselected.is_empty() {
            let direct = packs::PackRequest {
                title: format!("直接交给 Codex：{task}"),
                problem: (*task).to_owned(),
                architecture_notes: None,
                items: vec![],
                local_capabilities: vec![],
                mode: packs::PackMode::Direct,
            };
            let direct_pack = packs::build_pack(&connection, &direct, packs::PackFormat::Markdown, &generated_on)
                .expect("直接交接单");
            fs::write(out.join(format!("{name}_direct.md")), &direct_pack.content).expect("写直接交接单");
        }
        let single = packs::render_item_for_codex_with(&connection, &picks[0].id, Some(task)).expect("单项复制");
        fs::write(out.join(format!("{name}_single.txt")), &single).expect("写单项");
        index.insert(
            (*task).to_owned(),
            serde_json::json!({
                "files": [format!("{name}_solve.json"), format!("{name}_pack.md"), format!("{name}_single.txt")],
                "top": picks.iter().map(|hit| format!("{} · {} · {}", hit.full_name, hit.one_liner.clone().unwrap_or_default(), hit.why)).collect::<Vec<_>>(),
            }),
        );
    }
    dump(&out, "index", &index);
    println!("[完成] 输出目录：{}", out.display());
}

/// 批量问「找方案」：FSL_REVIEW_QUERIES 是一个文本文件，每行一句话。每句输出
/// 意图词 / 额外词 / 前 8 名（名称、形态、命中、缺口、分数、是否全对上），写到 FSL_REVIEW_OUT/solve_batch.json。
/// 用来在改词表 / 分词 / 打分前后对比。只读副本，不花钱。
#[test]
#[ignore]
fn review_solve_batch() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    let queries = fs::read_to_string(std::env::var("FSL_REVIEW_QUERIES").expect("需要 FSL_REVIEW_QUERIES"))
        .expect("读问题文件");
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("solve_batch.sqlite3");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    fs::copy(&source, &work).expect("复制副本");
    migrate_database(&work).expect("迁移");
    let mut rows = Vec::new();
    for query in queries.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let result = solve::solve_query(&work, query).expect("找方案");
        rows.push(serde_json::json!({
            "query": query,
            "intents": result.intents,
            "extras": result.extras,
            "fullMatchCount": result.full_match_count,
            "preselected": result.preselected.iter().map(|id| {
                result.hits.iter().find(|hit| &hit.id == id).map(|hit| hit.full_name.clone()).unwrap_or_default()
            }).collect::<Vec<_>>(),
            "gapIntents": result.gap_intents,
            "hits": result.hits.iter().map(|hit| serde_json::json!({
                "name": hit.full_name,
                "form": hit.form,
                "matched": hit.matched,
                "missing": hit.missing,
                "score": (hit.score * 10.0).round() / 10.0,
                "full": hit.full_match,
            })).collect::<Vec<_>>(),
        }));
    }
    dump(&out, "solve_batch", &rows);
    println!("[完成] {} 句 → {}", rows.len(), out.display());
}

/// 对全部有效仓库各出一份 JSON 参考包，统计每个项目被标了哪些「先问我」：看有没有标太多 / 太少。
/// 只读副本，不花钱。输出到 FSL_REVIEW_OUT/cautions_stats.json（每类的数量和前几个例子）。
#[test]
#[ignore]
fn review_cautions_stats() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("cautions.sqlite3");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    fs::copy(&source, &work).expect("复制副本");
    migrate_database(&work).expect("迁移");
    let connection = Connection::open(&work).expect("打开副本");
    let ids: Vec<String> = connection
        .prepare("SELECT id FROM repositories WHERE sync_status = 'active'")
        .expect("查 id")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("读 id")
        .filter_map(Result::ok)
        .collect();
    let mut flagged = serde_json::Map::new();
    let mut installs = serde_json::Map::new();
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    let mut pipes = 0usize;
    let mut any = 0usize;
    for chunk in ids.chunks(50) {
        let request = packs::PackRequest {
            title: "x".to_owned(),
            problem: "x".to_owned(),
            architecture_notes: None,
            items: chunk
                .iter()
                .map(|id| packs::PackItemInput {
                    repo_id: id.clone(),
                    role: packs::PackRole::Alternative,
                    why: None,
                })
                .collect(),
            local_capabilities: vec![],
            mode: packs::PackMode::UseTools,
        };
        let json = packs::build_pack(&connection, &request, packs::PackFormat::Json, "2026-10-05")
            .expect("JSON 参考包");
        let parsed: serde_json::Value = serde_json::from_str(&json.content).expect("解析");
        for item in parsed["items"].as_array().expect("items") {
            installs.insert(
                item["fullName"].as_str().unwrap_or_default().to_owned(),
                serde_json::json!({"kind": item["kind"], "howToTry": item["howToTry"], "installTarget": item["installTarget"]}),
            );
            let labels: Vec<String> = item["cautions"]
                .as_array()
                .expect("cautions")
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect();
            if labels.is_empty() {
                continue;
            }
            any += 1;
            for label in &labels {
                let key: String = label.chars().take(12).collect();
                *counts.entry(key).or_default() += 1;
                if label.contains("curl") {
                    pipes += 1;
                }
            }
            flagged.insert(
                item["fullName"].as_str().unwrap_or_default().to_owned(),
                serde_json::json!({"labels": labels, "howToTry": item["howToTry"], "risks": item["risks"]}),
            );
        }
    }
    println!("[统计] 有效仓库 {}，被标的 {any}，其中一行安装 {pipes}", ids.len());
    for (key, count) in &counts {
        println!("[统计] {key}… × {count}");
    }
    dump(&out, "cautions_stats", &serde_json::json!({"total": ids.len(), "flagged": any, "counts": counts, "items": flagged, "installs": installs}));
}

/// 场景标签重跑：对副本做「更新种子 → 重做规则归类 → 去掉没有证据的标签」，输出每个仓库最后带哪些场景。
/// 只读副本、不花钱。输出 FSL_REVIEW_OUT/scenarios_after.json。
#[test]
#[ignore]
fn review_scenarios_rerun() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("scenarios.sqlite3");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    fs::copy(&source, &work).expect("复制副本");
    migrate_database(&work).expect("迁移");
    let before = scenarios::list_scenarios_summary(&work).expect("场景摘要");
    let stats = scenarios::ensure_initial_classification(&work).expect("重跑");
    println!("[重跑] 规则归类：{stats:?}");
    let audit = scenarios::audit_scenario_tags(&work).expect("复核");
    println!("[重跑] 复核：{audit:?}");
    let after = scenarios::list_scenarios_summary(&work).expect("场景摘要");
    for (old, new) in before.iter().zip(after.iter()) {
        println!("[场景] {}：{} → {}", new.name_zh, old.count, new.count);
    }
    let connection = Connection::open(&work).expect("打开副本");
    let mut statement = connection
        .prepare(
            "SELECT r.full_name, s.name_zh, e.origin, e.confidence FROM entity_scenarios e
             JOIN repositories r ON r.id = e.entity_id JOIN scenarios s ON s.id = e.scenario_id
             WHERE e.entity_type = 'repository' AND e.status <> 'rejected' AND r.sync_status = 'active'
             ORDER BY r.full_name, s.sort_order",
        )
        .expect("查询");
    let rows: Vec<(String, String, String, f64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .expect("读取")
        .filter_map(Result::ok)
        .collect();
    let mut by_repo: std::collections::BTreeMap<String, Vec<serde_json::Value>> = Default::default();
    for (repo, scenario, origin, confidence) in &rows {
        by_repo
            .entry(repo.clone())
            .or_default()
            .push(serde_json::json!({"scenario": scenario, "origin": origin, "confidence": confidence}));
    }
    let total: i64 = connection
        .query_row("SELECT COUNT(*) FROM repositories WHERE sync_status = 'active'", [], |row| row.get(0))
        .expect("计数");
    println!("[重跑] 有效仓库 {total}，带场景的 {}，标签共 {}", by_repo.len(), rows.len());
    dump(&out, "scenarios_after", &by_repo);
}

/// 把理解卡的提示词（系统 + 用户）按真实输入拼出来，写成文件，给「替身模型」试跑用。
/// FSL_REVIEW_REPOS 是一个文本文件，每行一个 full_name。只读副本、不联网、不花钱。
/// 输出 FSL_REVIEW_OUT/prompt_<序号>.txt 和 index.json。
#[test]
#[ignore]
fn review_card_prompts() {
    let source = PathBuf::from(std::env::var("FSL_REVIEW_DB").expect("需要 FSL_REVIEW_DB"));
    let out = PathBuf::from(std::env::var("FSL_REVIEW_OUT").expect("需要 FSL_REVIEW_OUT"));
    let repos = fs::read_to_string(std::env::var("FSL_REVIEW_REPOS").expect("需要 FSL_REVIEW_REPOS"))
        .expect("读仓库清单");
    fs::create_dir_all(&out).expect("建输出目录");
    let work = out.join("prompts.sqlite3");
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{suffix}", work.display()));
    }
    fs::copy(&source, &work).expect("复制副本");
    migrate_database(&work).expect("迁移");
    scenarios::seed_scenarios(&work).expect("种子");
    let connection = Connection::open(&work).expect("打开副本");
    let mut index = Vec::new();
    for (number, name) in repos.lines().map(str::trim).filter(|line| !line.is_empty()).enumerate() {
        let id: String = connection
            .query_row("SELECT id FROM repositories WHERE full_name = ?1", [name], |row| row.get(0))
            .unwrap_or_else(|error| panic!("找不到 {name}：{error}"));
        let input = crate::card_pipeline::load_card_input(&connection, &id)
            .unwrap_or_else(|error| panic!("读 {name} 失败：{}", error.message));
        let request = crate::card_pipeline::build_prompt(&input);
        let text = format!("【系统提示词】\n{}\n\n【用户消息】\n{}\n", request.system, request.user);
        fs::write(out.join(format!("prompt_{}.txt", number + 1)), &text).expect("写提示词");
        println!("[提示词] {} → prompt_{}.txt（{} 字）", name, number + 1, text.chars().count());
        index.push(serde_json::json!({"n": number + 1, "fullName": name, "chars": text.chars().count(), "hasReadme": input.readme.is_some()}));
    }
    dump(&out, "index", &index);
}
