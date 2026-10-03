# 交回报告 · 卡 21-local-capabilities

## 一句话结论
做完了：只读命令 `scan_local_capabilities` 会列出本机 Skill 和 MCP 的名字、一句话和路径，不返回配置里的其他内容。

## 验收逐条
| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh rust-test` 全部通过，新增测试至少 14 个（临时目录造夹具，不碰真实的 HOME）：YAML 头解析（普通、带引号、`description: >` 多行块、缺 `name`、缺 `description`、没有 YAML 头）；只读前 4096 字节（造一个 `SKILL.md` 里 4096 字节之后才出现 `description` 的，应当取不到而不是读全文）；符号链接目录能跟随；没有 `SKILL.md` 的目录跳过；MCP 小节名解析（普通、带 `-` 和 `_`、`[mcp_servers.x.env]` 不出现、行里带密钥的内容不会出现在任何返回值里——用一个含 `KEY = "sk-secret"` 的夹具，断言整个序列化后的 JSON 里没有 `sk-secret`）；排序与去重；200 条上限；`~` 替换；缺目录 / 缺文件返回空；`serde` 键集合是 `name` / `oneLiner` / `path`。 | 是 | `tools/cards/run.sh rust-test`：`573 passed; 0 failed; 5 ignored; 0 measured; 1 filtered out`（这 1 个是 `run.sh` 跳过的钥匙串用例）。其中 `local_scan` 23 个全过。测试都在临时目录造夹具，没有调用 `scan_from_env` / `scan_local_capabilities`，因此没有读真实 `HOME`。对应关系：`plain_yaml_front_matter`、`quoted_yaml_values`、`folded_description_block_uses_first_nonempty_line`、`missing_name_uses_directory_name`、`missing_description_uses_skill_fallback`、`no_front_matter_uses_directory_name_and_skill`、`reads_only_first_4096_bytes`、`follows_symlinked_skill_directory`、`skips_directory_without_skill_md`、`mcp_names_with_dash_and_underscore`、`mcp_env_subsection_and_secrets_do_not_leak`（JSON 不含 `sk-secret`）、`skills_sort_before_mcp_and_duplicate_names_keep_first`、`caps_results_at_200`、`replaces_home_prefix_with_tilde`、`missing_directory_and_file_return_empty`、`serde_keys_are_name_one_liner_and_path`。另有 `|` 块、超长截断、空名字、空 home、真实相对布局、权限登记等补充测试。 |
| 2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。 | 是 | 动手前 `rust-check`：`generated 4 warnings`（`migrations.rs` 的 `applied` / `backup_path`，`v2_ops.rs` 的 `ensure_ready` / `repair_model_json` / `restore_database_file`）。做完后再跑 `rust-fmt`（退出码 0）和 `rust-check`，仍是这 4 条，没有新增。 |
| 3. `git diff --stat` 里只出现「允许改动」里的路径。 | 是 | `git diff --stat` 只有 `acl-manifests.json`、`gsat-commands.toml`、`lib.rs`。新建的 `local_scan.rs` 还没被跟踪，不出现在 `--stat` 里；`git status --short` 能看到它。`tools/cards/check-scope.sh 21-local-capabilities`：4 个文件全部 ✓，「全部在允许范围内」。 |
| 4. 报告里给出：命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明没有读 `config.toml` 的其他内容（贴出读取那一段代码）。 | 是 | `generate_handler!`：`apps/desktop/src-tauri/src/lib.rs:7188`（`local_scan::scan_local_capabilities`）。`mod local_scan;` 在 `lib.rs:20`。`gsat-commands.toml:130`。命令本体在 `local_scan.rs:74-77`，只 `Ok(scan_from_env())`。MCP 读取见下方代码：逐行只留下小节名。 |

MCP 读取（`local_scan.rs:334-369`）。对不上的行在循环里丢掉，不放进返回值、错误或日志：

```rust
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
```

`[mcp_servers.x.env]` 在去掉前缀后剩下 `x.env`，含 `.`，匹配失败。返回的 `path` 固定是 `~/.codex/config.toml`，配置正文不会被拼进任何字段。

## 改了哪些文件
- `apps/desktop/src-tauri/src/local_scan.rs`：新建只读扫描模块、命令和 23 个测试。
- `apps/desktop/src-tauri/src/lib.rs`：加 `mod local_scan;`，并在 `generate_handler!` 登记命令。
- `apps/desktop/src-tauri/permissions/gsat-commands.toml`：允许列表加上 `scan_local_capabilities`。
- `apps/desktop/src-tauri/gen/schemas/acl-manifests.json`：`rust-check` 按权限清单重新生成，允许列表末尾多了这条命令。

## 验证
- `tools/cards/run.sh rust-check`（动手前）→ 4 条既有警告，编译通过。
- `tools/cards/run.sh rust-fmt-fix` → 通过。
- `tools/cards/run.sh rust-fmt` → 通过。
- `tools/cards/run.sh rust-check`（做完后）→ 仍是 4 条既有警告，编译通过。
- `tools/cards/run.sh rust-test local_scan` → `23 passed; 0 failed`。
- `tools/cards/run.sh rust-test` → `573 passed; 0 failed; 5 ignored; 1 filtered out`。
- `tools/cards/check-scope.sh 21-local-capabilities` → 4 个改动文件都在允许范围内。

## 没做到的 / 偏离任务卡的地方
- 没有对真实 `HOME` 执行 `scan_local_capabilities`。任务卡要求测试不碰真实 HOME；命令本体只在 App 里调用。
- 命令签名是 `Result<Vec<LocalCapabilityOut>, String>`，函数体只有 `Ok(...)`。任务卡写成直接返回 `Vec`。这和现有 Tauri 命令一致，前端拿到的仍是数组。
- 一句话超过 80 个字符时，先保留 80 个再加「…」，结果是 81 个字符。名字最多 60 个字符，不加省略号。
- 去重按清洗后的字符串精确比较，`Alpha` 和 `alpha` 算两个名字。
- Skill 子目录先按目录名（不分大小写）排序再读。同名时保留目录名更靠前的那条，这样结果不跟着文件系统的遍历顺序变。
- 没有加 `regex` 依赖。MCP 行用与任务卡那条正则等价的判断：前后空白、`[mcp_servers.`、只含字母数字 `_` `-` 的名字、以 `]` 结束。
- `description:` 下面缩进的普通文字不当成多行块。只有值本身是 `>` / `|`（可带 chomp 和缩进数字）才取块里第一个非空行。这样别的键不会被扫进一句话。

## 需要总控或 Fox 决定的事
- 无

## 给审核者的提示
- `lib.rs` 只加了模块声明和 `generate_handler!` 里的一行。命令函数在 `local_scan.rs`。
- 测试没有调用会读环境变量 `HOME` 的 `scan_from_env`。空 home 用 `scan_from_home_str("")` 测。
- 密钥夹具是测试文件里的假字符串 `sk-secret`，用来断言它不会出现在返回的 JSON 里。
- 前端、`packs.rs`、命令契约都没改。参考包要显示「本机已装」，还得由总控把这个命令的结果填进 `local_capabilities`。
