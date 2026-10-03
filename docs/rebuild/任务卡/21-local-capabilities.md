---
编号: 21-local-capabilities
允许改动:
  - apps/desktop/src-tauri/src/local_scan.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/permissions/gsat-commands.toml
  - apps/desktop/src-tauri/gen/schemas/acl-manifests.json
---

# 卡 21 · 扫描本机已装的 Skill 和 MCP（只读名字，不读密钥）

## 为什么有这张卡
参考包里有一节「我本机已经有的相关能力」（`PackRequest.local_capabilities`），但前端写死传 `[]`。「站在 Codex 角度」的审查说：本机已经装了 `ai-draw`（出图）、`gpt-image`（MCP）、`agent-reach` 等，参考包却一句不提，Codex 会**重复安装**，或者装到它根本不读的目录（README 里常写 `~/.claude/skills`，Codex 读的是 `~/.agents/skills`）。
这张卡做一个**只读**的扫描命令，把本机已有的 Skill 和 MCP 的**名字和一句话**列出来，前端（总控另做）据此在参考包里标「本机已装」，并列出和任务相关的几个。

## Fox 能看到的效果
- 参考包里，已经装过的工具标「本机已装」，Codex 不再重复安装；任务相关的本机能力（比如「ai-draw：AI 绘图」）会列在「我本机已经有的」里。

## 背景与先读什么
- `LocalCapability`（`packs.rs` 第 73 行）：`{ name, one_liner, path }`，三个都是字符串。**这张卡返回同样形状**（`serde(rename_all = "camelCase")`：`name`、`oneLiner`、`path`），前端直接放进参考包请求。**不许改 `packs.rs`。**
- 本机的样子（总控看过目录结构）：`~/.agents/skills/<名字>/SKILL.md`（75 个，有的是指向别处的符号链接），`SKILL.md` 开头是 YAML 头 `---\nname: ...\ndescription: ...\n---`；`~/.codex/config.toml` 里 MCP 是 `[mcp_servers.<名字>]` 小节，还有 `[mcp_servers.<名字>.env]` 之类的子小节（**里面是密钥，绝对不能读出来、不能返回、不能进日志**）。
- 命令怎么注册：看 `lib.rs` 里 `generate_handler!` 的写法和 `permissions/gsat-commands.toml`、`gen/schemas/acl-manifests.json`（改了命令后跑一次 `rust-check` 会重新生成 `acl-manifests.json`，把生成的结果一起留下）。
- 只读、同步、很快：这个命令不联网，读几百个小文件，可以是普通的同步命令（不需要 `async`）。

## 要做的事

### 新模块 `src-tauri/src/local_scan.rs`（`lib.rs` 里加 `mod local_scan;`、命令注册——这是 `lib.rs` 里你能改的全部）
1. 纯函数 + 可注入路径，方便测试：
   ```rust
   pub struct ScanRoots { pub skills_dir: PathBuf, pub codex_config: PathBuf, pub home: PathBuf }
   pub fn scan(roots: &ScanRoots) -> Vec<LocalCapabilityOut>
   ```
   `LocalCapabilityOut { name, one_liner, path }`（`serde` 驼峰，键与 `packs::LocalCapability` 一致）。
2. **Skill**：遍历 `skills_dir` 下一层子目录（符号链接要跟随；读不了的跳过，不报错）；每个目录读 `SKILL.md`，**最多读前 4096 个字节**，解析开头 `---` … `---` 之间的 `name:` 和 `description:`（简单按行解析，值可以带引号；`description` 可能是 `>`、`|` 开头的多行块——取块里第一个非空行；找不到 `name` 就用目录名）。`one_liner` = `description` 折成一行、最多 80 个字符（超出截断加「…」；没有 `description` 写「Skill」）；`path` = 目录路径里把 `home` 前缀换成 `~`（如 `~/.agents/skills/ai-draw`）。没有 `SKILL.md` 的目录跳过。
3. **MCP**：读 `codex_config`，**只匹配这一种行**：`^\s*\[mcp_servers\.([A-Za-z0-9_\-]+)\]\s*$`（带点的子小节如 `.env` **不匹配**，所以不会出现）。每个名字一条：`one_liner` = `MCP 工具`，`path` = `~/.codex/config.toml`。**不要读取、解析、返回这个文件的任何其他内容**——实现上就是逐行用上面的正则，匹配到才取名字，其余行丢弃，不放进任何日志、错误信息或返回值。
4. 排序：先 Skill 后 MCP，各自按名字（不分大小写）排序；去重（同名取先出现的）；**总数最多 200 条**。
5. 名字清洗：`name` 去掉首尾空白，折成一行，最多 60 个字符；空名字丢弃。
6. 命令 `scan_local_capabilities() -> Vec<LocalCapabilityOut>`：`home` 取环境变量 `HOME`（取不到返回空列表，不报错）；`skills_dir = ~/.agents/skills`；`codex_config = ~/.codex/config.toml`。任何读取失败都是「这一项当作没有」，**命令本身永远返回 `Ok`**。
7. 注册命令：`generate_handler!`、`gsat-commands.toml`、`acl-manifests.json`；在 `docs/rebuild/命令契约.md` 里**不要改**（总控另补）。

## 不要做
- 不写任何文件、不联网、不读钥匙串；**不读 `~/.codex/auth.json`、`config.toml` 里除小节名外的任何内容、各 Skill 目录里除 `SKILL.md` 开头 4096 字节以外的任何文件**。
- 不改前端、契约、`packs.rs`。
- 不要为了「更完整」扫描别的目录（`~/.claude/skills` 等不扫，Fox 的 Skill 体系以 `~/.agents/skills` 为准）。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过，新增测试**至少 14 个**（临时目录造夹具，**不碰真实的 HOME**）：YAML 头解析（普通、带引号、`description: >` 多行块、缺 `name`、缺 `description`、没有 YAML 头）；只读前 4096 字节（造一个 `SKILL.md` 里 4096 字节之后才出现 `description` 的，应当取不到而不是读全文）；符号链接目录能跟随；没有 `SKILL.md` 的目录跳过；MCP 小节名解析（普通、带 `-` 和 `_`、**`[mcp_servers.x.env]` 不出现**、行里带密钥的内容不会出现在任何返回值里——用一个含 `KEY = "sk-secret"` 的夹具，断言整个序列化后的 JSON 里没有 `sk-secret`）；排序与去重；200 条上限；`~` 替换；缺目录 / 缺文件返回空；`serde` 键集合是 `name` / `oneLiner` / `path`。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. `git diff --stat` 里只出现「允许改动」里的路径。
4. 报告里给出：命令在 `generate_handler!` 和 `gsat-commands.toml` 里的行号；证明没有读 `config.toml` 的其他内容（贴出读取那一段代码）。

## 停止条件
- 要改契约、前端、`packs.rs` 或卡外文件；要新依赖；需要读 `config.toml` 的非小节名内容才能做——停手并写进报告。
