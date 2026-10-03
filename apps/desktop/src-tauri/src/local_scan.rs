//! 只读扫描本机已装的 Skill 和 MCP，只返回名字、一句话和路径。
//!
//! 不写文件、不联网。MCP 配置逐行只认 `[mcp_servers.<名字>]`，
//! 其余行立刻丢掉，不进入返回值、错误或日志。

use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufRead, BufReader, ErrorKind, Read},
    path::{Path, PathBuf},
};

const SKILL_PREFIX_BYTES: usize = 4096;
const NAME_MAX_CHARS: usize = 60;
const ONE_LINER_MAX_CHARS: usize = 80;
const RESULT_LIMIT: usize = 200;
const SKILL_FALLBACK: &str = "Skill";
const MCP_ONE_LINER: &str = "MCP 工具";
const MCP_DISPLAY_PATH: &str = "~/.codex/config.toml";

#[derive(Debug, Clone)]
pub struct ScanRoots {
    pub skills_dir: PathBuf,
    pub codex_config: PathBuf,
    pub home: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCapabilityOut {
    pub name: String,
    pub one_liner: String,
    pub path: String,
}

/// 扫描注入的目录。读不到的那一项当作没有，不报错。
pub fn scan(roots: &ScanRoots) -> Vec<LocalCapabilityOut> {
    let mut skills = scan_skills(roots);
    let mut mcps = scan_mcps(&roots.codex_config);
    dedup_exact(&mut skills);
    let mut seen: HashSet<String> = skills.iter().map(|item| item.name.clone()).collect();
    mcps.retain(|item| seen.insert(item.name.clone()));
    sort_by_name(&mut skills);
    sort_by_name(&mut mcps);
    let mut combined = skills;
    combined.append(&mut mcps);
    combined.truncate(RESULT_LIMIT);
    combined
}

/// `home` 为空就返回空列表。Skill 在 `~/.agents/skills`，MCP 在 `~/.codex/config.toml`。
pub fn scan_from_home_str(home: &str) -> Vec<LocalCapabilityOut> {
    let home = home.trim();
    if home.is_empty() {
        return Vec::new();
    }
    let home = PathBuf::from(home);
    scan(&ScanRoots {
        skills_dir: home.join(".agents").join("skills"),
        codex_config: home.join(".codex").join("config.toml"),
        home,
    })
}

fn scan_from_env() -> Vec<LocalCapabilityOut> {
    match std::env::var("HOME") {
        Ok(home) => scan_from_home_str(&home),
        Err(_) => Vec::new(),
    }
}

/// 永远成功：找不到 HOME、目录或文件都是空列表。
#[tauri::command]
pub fn scan_local_capabilities() -> Result<Vec<LocalCapabilityOut>, String> {
    Ok(scan_from_env())
}

fn scan_skills(roots: &ScanRoots) -> Vec<LocalCapabilityOut> {
    let Ok(entries) = fs::read_dir(&roots.skills_dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .collect();
    dirs.sort_by(|left, right| {
        file_name_key(left)
            .cmp(&file_name_key(right))
            .then_with(|| left.cmp(right))
    });
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(meta) = fs::metadata(&dir) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        if let Some(item) = read_skill(&dir, &roots.home) {
            out.push(item);
        }
    }
    out
}

fn read_skill(dir: &Path, home: &Path) -> Option<LocalCapabilityOut> {
    let dir_name = dir.file_name()?.to_string_lossy();
    let prefix = read_prefix(&dir.join("SKILL.md"), SKILL_PREFIX_BYTES)?;
    let text = String::from_utf8_lossy(&prefix);
    let (yaml_name, description) = parse_skill_front_matter(&text);
    let name = resolve_name(yaml_name, &dir_name)?;
    let one_liner = description
        .and_then(|raw| non_empty_line(&raw))
        .map(|line| truncate_with_ellipsis(&line, ONE_LINER_MAX_CHARS))
        .unwrap_or_else(|| SKILL_FALLBACK.to_owned());
    Some(LocalCapabilityOut {
        name,
        one_liner,
        path: tilde_home(dir, home),
    })
}

/// 只读前 `limit` 个字节。打不开就不返回内容。
fn read_prefix(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; limit];
    let mut filled = 0;
    while filled < limit {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return None,
        }
    }
    buf.truncate(filled);
    Some(buf)
}

fn parse_skill_front_matter(text: &str) -> (Option<String>, Option<String>) {
    let normalized = text.trim_start_matches('\u{feff}');
    let lines: Vec<&str> = normalized
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect();
    if lines.first().map(|line| line.trim()) != Some("---") {
        return (None, None);
    }
    let mut name = None;
    let mut description = None;
    let mut index = 1;
    while index < lines.len() {
        let line = lines[index];
        if line.trim() == "---" {
            break;
        }
        if let Some((key, value)) = top_level_key(line) {
            if key == "name" && name.is_none() {
                name = Some(unquote(value));
            } else if key == "description" && description.is_none() {
                if is_block_header(value) {
                    index += 1;
                    let first = take_block_first_line(&lines, &mut index);
                    description = Some(first.unwrap_or_default());
                    continue;
                }
                description = Some(unquote(value));
            }
        }
        index += 1;
    }
    (name, description)
}

fn top_level_key(line: &str) -> Option<(&str, &str)> {
    if line.starts_with(' ') || line.starts_with('\t') {
        return None;
    }
    let (raw_key, raw_value) = line.split_once(':')?;
    let key = raw_key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return None;
    }
    Some((key, raw_value.trim()))
}

fn take_block_first_line(lines: &[&str], index: &mut usize) -> Option<String> {
    let mut first = None;
    while *index < lines.len() {
        let next = lines[*index];
        if next.trim() == "---" || top_level_key(next).is_some() {
            break;
        }
        if first.is_none() {
            let trimmed = next.trim();
            if !trimmed.is_empty() {
                first = Some(trimmed.to_owned());
            }
        }
        *index += 1;
    }
    first
}

/// `>` / `|` 以及只带 chomp、缩进数字的块头。同一行还有别的文字就不是块。
fn is_block_header(value: &str) -> bool {
    let mut chars = value.chars().peekable();
    match chars.next() {
        Some('>' | '|') => {}
        _ => return false,
    }
    if chars.peek().is_none() {
        return true;
    }
    if matches!(chars.peek(), Some('+' | '-')) {
        chars.next();
        return chars.all(|ch| ch.is_ascii_digit());
    }
    let mut saw_digit = false;
    let mut saw_sign = false;
    for ch in chars {
        if ch.is_ascii_digit() {
            if saw_sign {
                return false;
            }
            saw_digit = true;
        } else if ch == '+' || ch == '-' {
            if saw_sign || !saw_digit {
                return false;
            }
            saw_sign = true;
        } else {
            return false;
        }
    }
    true
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    if (first == '"' || first == '\'') && value.ends_with(first) && value.chars().count() >= 2 {
        let mut inner = value.chars();
        inner.next();
        inner.next_back();
        return inner.collect();
    }
    value.to_owned()
}

fn resolve_name(yaml_name: Option<String>, dir_name: &str) -> Option<String> {
    if let Some(name) = yaml_name.and_then(|raw| clean_name(&raw)) {
        return Some(name);
    }
    clean_name(dir_name)
}

fn clean_name(raw: &str) -> Option<String> {
    let folded = fold_line(raw);
    if folded.is_empty() {
        None
    } else {
        Some(truncate_chars(&folded, NAME_MAX_CHARS))
    }
}

fn non_empty_line(raw: &str) -> Option<String> {
    let folded = fold_line(raw);
    if folded.is_empty() {
        None
    } else {
        Some(folded)
    }
}

fn fold_line(raw: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in raw.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    out
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_owned()
    } else {
        value.chars().take(max_chars).collect()
    }
}

/// 超出时尽量在一个完整的分句后面截断，不留半句话：
/// 1. 前 `max_chars` 个字符的后半段里，最后一个「，。；、！？」或「.;,」（后面跟空格）之前截断，不加「…」；
/// 2. 英文没有这些标点时，在最后一个空格之前截断，加「…」；
/// 3. 都没有才硬截 `max_chars` 个字符，加「…」。
fn truncate_with_ellipsis(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let head: Vec<char> = value.chars().take(max_chars).collect();
    let floor = max_chars / 2;
    let is_break = |index: usize| {
        let ch = head[index];
        matches!(ch, '，' | '。' | '；' | '、' | '！' | '？')
            || (matches!(ch, '.' | ';' | ',') && head.get(index + 1) == Some(&' '))
            || (matches!(ch, '.' | ';' | ',')
                && index + 1 == head.len()
                && value.chars().nth(max_chars) == Some(' '))
    };
    if let Some(index) = (floor..head.len()).rev().find(|index| is_break(*index)) {
        let cut: String = head[..index].iter().collect();
        let cut = cut.trim_end().to_owned();
        if !cut.is_empty() {
            return cut;
        }
    }
    if let Some(index) = (floor..head.len()).rev().find(|index| head[*index] == ' ') {
        let cut: String = head[..index].iter().collect();
        let cut = cut.trim_end().to_owned();
        if !cut.is_empty() {
            return format!("{cut}…");
        }
    }
    let kept: String = head.iter().collect();
    format!("{kept}…")
}

fn tilde_home(path: &Path, home: &Path) -> String {
    if home.as_os_str().is_empty() {
        return path.to_string_lossy().into_owned();
    }
    match path.strip_prefix(home) {
        Ok(rest) => {
            let rest = rest.to_string_lossy();
            if rest.is_empty() {
                "~".to_owned()
            } else {
                format!("~/{rest}")
            }
        }
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

/// 逐行匹配 `^\s*\[mcp_servers\.([A-Za-z0-9_\-]+)\]\s*$`。
/// 对不上的行（含 `.env` 子小节和密钥）马上丢掉，不保存原文。
fn scan_mcps(config_path: &Path) -> Vec<LocalCapabilityOut> {
    let Ok(file) = File::open(config_path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    let mut out = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        let Some(name) = mcp_section_name(&line).and_then(|raw| clean_name(raw)) else {
            continue;
        };
        out.push(LocalCapabilityOut {
            name,
            one_liner: MCP_ONE_LINER.to_owned(),
            path: MCP_DISPLAY_PATH.to_owned(),
        });
    }
    out
}

fn mcp_section_name(line: &str) -> Option<&str> {
    let line = line.trim();
    let name = line.strip_prefix("[mcp_servers.")?.strip_suffix(']')?;
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        Some(name)
    } else {
        None
    }
}

fn dedup_exact(items: &mut Vec<LocalCapabilityOut>) {
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.name.clone()));
}

fn sort_by_name(items: &mut [LocalCapabilityOut]) {
    items.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.one_liner.cmp(&right.one_liner))
    });
}

fn file_name_key(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("时钟应可用")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "fsl-local-scan-{}-{}-{}",
                std::process::id(),
                nanos,
                tick
            ));
            fs::create_dir_all(&path).expect("临时目录应能创建");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_skill(skills: &Path, dir: &str, body: &str) {
        let path = skills.join(dir);
        fs::create_dir_all(&path).expect("技能目录应能创建");
        fs::write(path.join("SKILL.md"), body).expect("SKILL.md 应能写入");
    }

    fn scan_at(home: &Path, skills: &Path, config: &Path) -> Vec<LocalCapabilityOut> {
        scan(&ScanRoots {
            skills_dir: skills.to_path_buf(),
            codex_config: config.to_path_buf(),
            home: home.to_path_buf(),
        })
    }

    fn only(items: Vec<LocalCapabilityOut>) -> LocalCapabilityOut {
        assert_eq!(items.len(), 1, "应只有一条：{items:?}");
        items.into_iter().next().expect("刚检查过长度")
    }

    #[test]
    fn plain_yaml_front_matter() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        write_skill(
            &skills,
            "ai-draw",
            "---\n# comment\ndescription: AI 绘图\nname: ai-draw\n---\n正文不要读进来\n",
        );
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name, "ai-draw");
        assert_eq!(item.one_liner, "AI 绘图");
        assert_eq!(item.path, "~/skills/ai-draw");
    }

    #[test]
    fn quoted_yaml_values() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        write_skill(
            &skills,
            "quoted",
            "---\nname: \"  My Skill  \"\ndescription: 'Draws pictures'\n---\n",
        );
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name, "My Skill");
        assert_eq!(item.one_liner, "Draws pictures");
    }

    #[test]
    fn folded_description_block_uses_first_nonempty_line() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        write_skill(
            &skills,
            "blocky",
            "---\nname: blocky\ndescription: >\n\n  First real line\n  second should stay out\n---\n",
        );
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name, "blocky");
        assert_eq!(item.one_liner, "First real line");
        assert!(!item.one_liner.contains("second"));
    }

    #[test]
    fn literal_description_block_uses_first_nonempty_line() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        write_skill(
            &skills,
            "literal",
            "---\nname: literal\ndescription: |\n\n  pipe line\n  later line\n---\n",
        );
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.one_liner, "pipe line");
        assert!(!item.one_liner.contains("later"));
    }

    #[test]
    fn missing_name_uses_directory_name() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        write_skill(
            &skills,
            "dir-fallback",
            "---\nname: \"   \"\ndescription: Only a description\n---\n",
        );
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name, "dir-fallback");
        assert_eq!(item.one_liner, "Only a description");
    }

    #[test]
    fn missing_description_uses_skill_fallback() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        write_skill(&skills, "no-desc", "---\nname: no-desc\n---\n");
        write_skill(
            &skills,
            "empty-block",
            "---\nname: empty-block\ndescription: >\n\n---\n",
        );
        write_skill(
            &skills,
            "indented-only",
            "---\nname: indented-only\ndescription:\n  hidden-not-a-block\n---\n",
        );
        let items = scan_at(tmp.path(), &skills, &tmp.path().join("no.toml"));
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| item.one_liner == "Skill"));
        let json = serde_json::to_string(&items).expect("应能序列化");
        assert!(!json.contains("hidden-not-a-block"));
    }

    #[test]
    fn no_front_matter_uses_directory_name_and_skill() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        write_skill(
            &skills,
            "plain-dir",
            "hello\n---\nname: nope\ndescription: nope\n---\n",
        );
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name, "plain-dir");
        assert_eq!(item.one_liner, "Skill");
        let json = serde_json::to_string(&item).expect("应能序列化");
        assert!(!json.contains("nope"));
    }

    #[test]
    fn reads_only_first_4096_bytes() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        let mut body = String::from("---\nname: clipped\n");
        body.push_str(&"\n".repeat(SKILL_PREFIX_BYTES - body.len()));
        assert_eq!(body.len(), SKILL_PREFIX_BYTES);
        body.push_str("description: hidden-after-limit\nname: other\n---\n");
        write_skill(&skills, "clipped", &body);
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name, "clipped");
        assert_eq!(item.one_liner, "Skill");
        let json = serde_json::to_string(&item).expect("应能序列化");
        assert!(!json.contains("hidden-after-limit"));
        assert!(!json.contains("other"));
    }

    #[test]
    fn follows_symlinked_skill_directory() {
        let tmp = TempDir::new();
        let home = tmp.path().join("home");
        let real = tmp.path().join("target-dir");
        fs::create_dir_all(&real).expect("目标目录应能创建");
        fs::write(
            real.join("SKILL.md"),
            "---\nname: linked-skill\ndescription: via link\n---\n",
        )
        .expect("目标说明应能写入");
        let skills = home.join(".agents").join("skills");
        fs::create_dir_all(&skills).expect("技能目录应能创建");
        std::os::unix::fs::symlink(&real, skills.join("ai-draw")).expect("符号链接应能创建");
        let item = only(scan_at(&home, &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name, "linked-skill");
        assert_eq!(item.one_liner, "via link");
        assert_eq!(item.path, "~/.agents/skills/ai-draw");
        assert!(!item.path.contains("target-dir"));
    }

    #[test]
    fn skips_directory_without_skill_md() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        fs::create_dir_all(skills.join("bare")).expect("空目录应能创建");
        fs::create_dir_all(&skills).expect("技能目录应能创建");
        fs::write(skills.join("notes.txt"), "not a skill").expect("文件应能写入");
        std::os::unix::fs::symlink(skills.join("missing-target"), skills.join("broken"))
            .expect("坏链接应能创建");
        write_skill(&skills, "kept", "---\nname: kept\ndescription: yes\n---\n");
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name, "kept");
    }

    #[test]
    fn mcp_names_with_dash_and_underscore() {
        let tmp = TempDir::new();
        let config = tmp.path().join("config.toml");
        fs::write(
            &config,
            "\nmodel = \"gpt\"\n\n[mcp_servers.plain]\ncommand = \"a\"\n\n  [mcp_servers.gpt-image]  \ncommand = \"b\"\n\n[mcp_servers.agent_reach]\ncommand = \"c\"\n",
        )
        .expect("配置应能写入");
        let items = scan_at(tmp.path(), &tmp.path().join("no-skills"), &config);
        let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, vec!["agent_reach", "gpt-image", "plain"]);
        assert!(items.iter().all(|item| item.one_liner == "MCP 工具"));
        assert!(items.iter().all(|item| item.path == "~/.codex/config.toml"));
    }

    #[test]
    fn mcp_env_subsection_and_secrets_do_not_leak() {
        let tmp = TempDir::new();
        let config = tmp.path().join("config.toml");
        fs::write(
            &config,
            "[mcp_servers.gpt-image]\ncommand = \"npx\"\nKEY = \"sk-secret\"\n\n[mcp_servers.x.env]\nKEY = \"sk-secret\"\nTOKEN = \"SHOULD_NOT_LEAK_TOKEN\"\n\n[mcp_servers.ok_tool]\nurl = \"http://example\"\n",
        )
        .expect("配置应能写入");
        let items = scan_at(tmp.path(), &tmp.path().join("no-skills"), &config);
        let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, vec!["gpt-image", "ok_tool"]);
        assert!(items.iter().all(|item| item.name != "x"));
        assert!(items.iter().all(|item| !item.name.contains("env")));
        let json = serde_json::to_string(&items).expect("应能序列化");
        assert!(!json.contains("sk-secret"));
        assert!(!json.contains("SHOULD_NOT_LEAK_TOKEN"));
        assert!(!json.contains("KEY"));
        assert!(!json.contains("http://example"));
    }

    #[test]
    fn skills_sort_before_mcp_and_duplicate_names_keep_first() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        write_skill(
            &skills,
            "b-dir",
            "---\nname: shared\ndescription: from-b\n---\n",
        );
        write_skill(
            &skills,
            "a-dir",
            "---\nname: shared\ndescription: from-a\n---\n",
        );
        write_skill(&skills, "zeta", "---\nname: zeta\ndescription: Z\n---\n");
        write_skill(&skills, "mid", "---\nname: Alpha\ndescription: A\n---\n");
        let config = tmp.path().join("config.toml");
        fs::write(
            &config,
            "[mcp_servers.shared]\nKEY = \"skip\"\n[mcp_servers.beta]\n[mcp_servers.alpha-mcp]\n[mcp_servers.beta]\n",
        )
        .expect("配置应能写入");
        let items = scan_at(tmp.path(), &skills, &config);
        let view: Vec<(&str, &str)> = items
            .iter()
            .map(|item| (item.name.as_str(), item.one_liner.as_str()))
            .collect();
        assert_eq!(
            view,
            vec![
                ("Alpha", "A"),
                ("shared", "from-a"),
                ("zeta", "Z"),
                ("alpha-mcp", "MCP 工具"),
                ("beta", "MCP 工具"),
            ]
        );
    }

    #[test]
    fn caps_results_at_200() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        for index in 0..=200 {
            write_skill(
                &skills,
                &format!("dir{index:03}"),
                &format!("---\nname: n{index:03}\n---\n"),
            );
        }
        let config = tmp.path().join("config.toml");
        fs::write(&config, "[mcp_servers.zzz-mcp]\n").expect("配置应能写入");
        let items = scan_at(tmp.path(), &skills, &config);
        assert_eq!(items.len(), 200);
        assert_eq!(items[0].name, "n000");
        assert_eq!(items[199].name, "n199");
        assert!(items.iter().all(|item| item.name != "n200"));
        assert!(items.iter().all(|item| item.name != "zzz-mcp"));
        assert!(items.iter().all(|item| item.one_liner == "Skill"));
    }

    #[test]
    fn replaces_home_prefix_with_tilde() {
        let tmp = TempDir::new();
        let home = tmp.path().join("home");
        let skills = home.join(".agents").join("skills");
        write_skill(
            &skills,
            "ai-draw",
            "---\nname: ai-draw\ndescription: AI 绘图\n---\n",
        );
        let item = only(scan_at(&home, &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.path, "~/.agents/skills/ai-draw");
    }

    #[test]
    fn does_not_tilde_string_prefix_that_is_not_the_home_path() {
        let tmp = TempDir::new();
        let home = tmp.path().join("home");
        let skills = tmp.path().join("home2").join("skills");
        write_skill(&skills, "tool", "---\nname: tool\ndescription: T\n---\n");
        let item = only(scan_at(&home, &skills, &tmp.path().join("no.toml")));
        assert!(!item.path.starts_with('~'));
        assert!(item.path.contains("home2"));
        assert!(item.path.ends_with("tool") || item.path.ends_with("tool/"));
    }

    #[test]
    fn missing_directory_and_file_return_empty() {
        let tmp = TempDir::new();
        let empty = scan_at(
            tmp.path(),
            &tmp.path().join("missing-skills"),
            &tmp.path().join("missing.toml"),
        );
        assert!(empty.is_empty());

        let skills = tmp.path().join("skills");
        write_skill(&skills, "only-skill", "---\nname: only-skill\n---\n");
        let skill_only = scan_at(tmp.path(), &skills, &tmp.path().join("missing.toml"));
        assert_eq!(only(skill_only).name, "only-skill");

        let config = tmp.path().join("config.toml");
        fs::write(&config, "[mcp_servers.only-mcp]\n").expect("配置应能写入");
        let mcp_only = scan_at(tmp.path(), &tmp.path().join("missing-skills"), &config);
        assert_eq!(only(mcp_only).name, "only-mcp");
    }

    #[test]
    fn serde_keys_are_name_one_liner_and_path() {
        let item = LocalCapabilityOut {
            name: "ai-draw".to_owned(),
            one_liner: "AI 绘图".to_owned(),
            path: "~/.agents/skills/ai-draw".to_owned(),
        };
        let value = serde_json::to_value(&item).expect("应能序列化");
        let object = value.as_object().expect("应是对象");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["name", "oneLiner", "path"]);
        assert!(!object.contains_key("one_liner"));
        let parsed: LocalCapabilityOut =
            serde_json::from_value(value).expect("应能按驼峰键反序列化");
        assert_eq!(parsed, item);
    }

    #[test]
    fn long_description_truncates_with_ellipsis_and_name_stops_at_60() {
        let tmp = TempDir::new();
        let skills = tmp.path().join("skills");
        let name = "名".repeat(70);
        let description = "描".repeat(81);
        write_skill(
            &skills,
            "long",
            &format!("---\nname: {name}\ndescription: {description}\n---\n"),
        );
        let item = only(scan_at(tmp.path(), &skills, &tmp.path().join("no.toml")));
        assert_eq!(item.name.chars().count(), 60);
        assert_eq!(item.name, "名".repeat(60));
        assert!(!item.name.contains('…'));
        assert_eq!(item.one_liner.chars().count(), 81);
        assert_eq!(
            item.one_liner,
            format!("{}…", "描".repeat(ONE_LINER_MAX_CHARS))
        );
    }

    #[test]
    fn empty_name_is_dropped() {
        assert_eq!(resolve_name(Some("   \n\t  ".to_owned()), "  \n"), None);
        assert_eq!(resolve_name(None, "dir"), Some("dir".to_owned()));
    }

    #[test]
    fn blank_home_returns_empty_without_using_env() {
        assert!(scan_from_home_str("").is_empty());
        assert!(scan_from_home_str("   ").is_empty());
    }

    #[test]
    fn home_layout_reads_agents_skills_and_codex_config() {
        let tmp = TempDir::new();
        let home = tmp.path().join("home");
        write_skill(
            &home.join(".agents").join("skills"),
            "ai-draw",
            "---\nname: ai-draw\ndescription: AI 绘图\napi_key: sk-secret-skill\n---\n",
        );
        fs::create_dir_all(home.join(".codex")).expect("配置目录应能创建");
        fs::write(
            home.join(".codex").join("config.toml"),
            "[mcp_servers.gpt-image]\nKEY = \"sk-secret\"\n",
        )
        .expect("配置应能写入");
        let home_str = home.to_str().expect("临时路径应是 UTF-8");
        let items = scan_from_home_str(home_str);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "ai-draw");
        assert_eq!(items[0].one_liner, "AI 绘图");
        assert_eq!(items[0].path, "~/.agents/skills/ai-draw");
        assert_eq!(items[1].name, "gpt-image");
        assert_eq!(items[1].one_liner, "MCP 工具");
        assert_eq!(items[1].path, "~/.codex/config.toml");
        let json = serde_json::to_string(&items).expect("应能序列化");
        assert!(!json.contains("sk-secret"));
    }

    #[test]
    fn command_is_registered_in_permissions_and_manifest() {
        let permissions = include_str!("../permissions/gsat-commands.toml");
        let manifest = include_str!("../gen/schemas/acl-manifests.json");
        assert!(permissions.contains("\"scan_local_capabilities\""));
        assert!(manifest.contains("\"scan_local_capabilities\""));
    }

    #[test]
    fn truncation_prefers_a_clause_boundary_over_a_half_sentence() {
        // 中文：后半段里最后一个逗号前截断，不加「…」
        let text = format!(
            "{}，{}，{}",
            "甲".repeat(50),
            "乙".repeat(20),
            "丙".repeat(30)
        );
        assert_eq!(
            truncate_with_ellipsis(&text, 80),
            format!("{}，{}", "甲".repeat(50), "乙".repeat(20))
        );
        // 逗号太靠前（前半段）不算，宁可硬截
        let early = format!("{}，{}", "甲".repeat(10), "乙".repeat(90));
        assert_eq!(
            truncate_with_ellipsis(&early, 80),
            format!("{}，{}…", "甲".repeat(10), "乙".repeat(69))
        );
        // 英文：没有标点就在最后一个空格前截断并加「…」
        let english = "Route generate edit save and verify AI visual assets for brand campaigns across channels and formats today";
        let cut = truncate_with_ellipsis(english, 60);
        assert!(cut.ends_with('…'));
        assert!(!cut.trim_end_matches('…').ends_with(' '));
        assert!(english.starts_with(cut.trim_end_matches('…')));
        // 英文句内逗号（后面跟空格）也算分句
        let comma = format!("{}, {}", "a".repeat(45), "b ".repeat(30));
        assert_eq!(truncate_with_ellipsis(&comma, 60), "a".repeat(45));
        // 没超长原样返回
        assert_eq!(truncate_with_ellipsis("短句，不用截", 80), "短句，不用截");
    }
}
