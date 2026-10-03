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
        "solve_v2" => ser(to_value(solve::solve_query(work, &arg_str(args, "query")?)?)),
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
        "copy_item_for_codex" => {
            let connection = Connection::open(work).map_err(|e| e.to_string())?;
            Ok(Value::String(packs::render_item_for_codex(
                &connection,
                &arg_str(args, "repoId")?,
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

fn start_fake_llm(served: Arc<AtomicUsize>) -> u16 {
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
                let body = serde_json::json!({
                    "id": "fake",
                    "model": "fake-model",
                    "choices": [{"message": {"role": "assistant", "content": canned_card()}}],
                    "usage": {"prompt_tokens": 3000, "completion_tokens": 600}
                })
                .to_string();
                let text = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
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
    let port = start_fake_llm(served.clone());
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
