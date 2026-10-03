---
编号: 18-repo-facts
允许改动:
  - apps/desktop/src-tauri/src/repo_facts.rs
  - apps/desktop/src-tauri/src/github.rs
  - apps/desktop/src-tauri/src/storage.rs
  - apps/desktop/src-tauri/src/library_view.rs
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src-tauri/src/db/migrations.rs
  - packages/storage/migrations/fox/032_repo_license.sql
---

# 卡 18 · 仓库的「事实字段」：许可证（来自 GitHub）和形态推断（来自话题）

## 为什么有这张卡
2026-10-03 总控请一个「站在 Codex 角度」的审查员把 Fox 开发库里的 495 个仓库的「单项」和「参考包」都拿到手读了一遍。结论之一：**许可证 495/495 都是「未知」**，Codex 没法判断「能不能商用」「别复制受限许可证的代码」。原因很简单：许可证只出现在 AI 理解卡的 `license` 字段里，而 AI 卡大多没有，真卡里这个字段也几乎是空的；GitHub 的 Stars 接口其实**每个仓库都带 `license`**，同步时没存。
另外，**形态**（skill / mcp / cli / app …）也只来自 AI 卡，318 个没有卡的仓库形态是空的，而 Codex 要靠形态决定「装到哪」（Skill 装进 `~/.agents/skills`，MCP 要登记进配置，App 要下 dmg）。GitHub 话题（`topics_json`，495 个仓库都有）里常有 `mcp-server`、`agent-skill` 这类词，可以推断。

这张卡是**数据底座**：把许可证存下来，并给一个「形态推断」的纯函数。后面的卡（19 参考包、20 找方案）会用。界面不改。

## Fox 能看到的效果
- 点一次「同步」之后，每个仓库都有了许可证（MIT、AGPL-3.0、没有声明的写「未声明」）。
- 详情页「许可证」不再全是空；没有 AI 卡的仓库，形态也能推断出来。
- 参考包、「复制给 Codex」（卡 19）里的「许可证：未知」会换成真实值。

## 背景与先读什么
- 同步的位置：`github.rs` 的 `GitHubRepository`（反序列化 GitHub 响应，没有 `license`）、`StarredRepository`（落库用的结构）、`map_starred_repository`；`storage.rs` 的 `upsert_repositories`（第 798 行附近，用 `sql_text` 拼 SQL 的老写法，**两个 `ON CONFLICT` 分支都要加新列**）；`lib.rs` 约第 3809 行调用它。同步每次都拉全部分页，所以一次同步就能给全部 495 个仓库补上。
- GitHub Stars 接口（`application/vnd.github.star+json`）每个 `repo` 里有 `license`：`{ "key": "mit", "name": "MIT License", "spdx_id": "MIT", "url": "..." }`，没有许可证时是 `null`；无法识别时 `spdx_id` 是 `"NOASSERTION"`，`key` 是 `"other"`。
- 资料库详情的许可证现在来自 AI 卡：`library_view.rs` 约第 345 行 `license: card_string(&card, "license")`。
- 迁移规则：`CLAUDE.md`「Schema 与迁移」（只追加、登记到 `db/migrations.rs` 的 `MIGRATIONS`、补测试；**这张卡用 032**，031 是卡 17 的）。`ALTER TABLE ... ADD COLUMN` 对已有数据安全。
- 测试怎么造库：`storage.rs`、`library_view.rs` 各自 `mod tests` 里已有的夹具；`StarredRepository` 在测试里有多处手写构造（`storage.rs` 约 5347、5366、6140 行附近），加字段后编译器会逐处提醒。

## 要做的事

### 1. 迁移 `packages/storage/migrations/fox/032_repo_license.sql`（并在 `db/migrations.rs` 登记、补测试）
```sql
ALTER TABLE repositories ADD COLUMN license_spdx TEXT;
ALTER TABLE repositories ADD COLUMN license_name TEXT;
```
- 两列都可空，没有默认值（旧行是 `NULL`，下次同步才补）。
- `db/migrations.rs`：按 `MIGRATIONS` 现有写法登记版本 `032`、名称 `repo_license`；补测试：全新库应用全部迁移后 `repositories` 有这两列；旧库（只到 031）升到 032 后旧行的这两列是 `NULL` 且行数不变。

### 2. 同步时存许可证（`github.rs` + `storage.rs`）
- `GitHubRepository` 加 `license: Option<GitHubLicense>`，`GitHubLicense { spdx_id: Option<String>, name: Option<String> }`（只取这两个字段，其余忽略；响应里没有 `license` 键也要能解析）。
- `StarredRepository` 加 `license_spdx: Option<String>`、`license_name: Option<String>`；`map_starred_repository` 填上（`license` 为 `null` → 两个都是 `None`）。
- `upsert_repositories`：`INSERT` 列表、`VALUES`、**两个 `ON CONFLICT ... DO UPDATE SET`** 都带上这两列（用现成的 `sql_optional_text`）。GitHub 这次返回 `null` 就把库里的旧值**清成 `NULL`**（许可证可能被作者删掉）。
- 所有手写 `StarredRepository { ... }` 的地方补 `license_spdx: None, license_name: None`。
- 测试：解析一段带 `license`（MIT、`NOASSERTION`、`null`、没有这个键）的 Stars 响应；`upsert_repositories` 写入后能读回；第二次 upsert 更新为另一个许可证；第二次返回 `None` 时清空。

### 3. 新模块 `src-tauri/src/repo_facts.rs`（`lib.rs` 里加一行 `mod repo_facts;`，这是 `lib.rs` 里你唯一能改的地方）
纯函数，不碰数据库、不联网：

```rust
pub struct LicenseLabel { pub text: String, pub declared: bool }
pub fn license_label(spdx: Option<&str>, name: Option<&str>) -> LicenseLabel
```
- `spdx` 为 `None` 或去空白后为空 → `{ text: "未声明", declared: false }`。
- `spdx` 是 `NOASSERTION`（不分大小写）或 `Other`：有 `name`（去空白、不超过 40 个字符，超出截断）就用 `name`，否则写 `"其他（见仓库）"`；`declared: true`。
- 其余 → `text` 就是 `spdx` 去空白；`declared: true`。

```rust
pub fn infer_form(topics: &[String], full_name: &str, description: Option<&str>) -> Option<&'static str>
```
返回 `"mcp"`、`"skill"`、`"cli"`、`"app"` 或 `None`。**不分大小写**，按下面的优先级（先命中先返回）：
1. `mcp`：话题里有 `mcp`、`mcp-server`、`mcp-servers`、`model-context-protocol`，或仓库名（`/` 后面那段）以 `-mcp`、`_mcp` 结尾或以 `mcp-`、`mcp_` 开头，或包含 `mcp-server`。
2. `skill`：话题里有 `skill`、`skills`、`agent-skill`、`agent-skills`、`claude-skill`、`claude-skills`、`claude-code-skills`、`codex-skill`、`codex-skills`，或仓库名以 `-skill`、`-skills`、`_skill`、`_skills` 结尾，或以 `skill-`、`skills-` 开头。
3. `cli`：话题里有 `cli`、`command-line`、`command-line-tool`、`terminal`。
4. `app`：话题里有 `desktop-app`、`electron`、`tauri`、`macos-app`、`windows-app`、`menubar-app`。
5. 都没有 → `None`。**不要**因为描述里出现某个词就推断（描述只作为这个函数的参数预留，现在不用它；这样不会被一句营销文案带偏）。
- 测试至少覆盖：每条规则的正例、优先级（同时有 `mcp` 和 `cli` 话题 → `mcp`）、大小写、`/` 前后的名字别混（`owner` 叫 `mcp-fan` 不算）、空话题、`topics` 里有空串。

### 4. 资料库详情用上它们（`library_view.rs`）
- `get_library_detail`：`license` 的取值顺序改为：仓库的 `license_spdx`/`license_name`（经 `license_label`，`declared == true` 时用 `text`）→ AI 卡里的 `license` → `None`。`declared == false`（GitHub 说「没有声明」）时返回 `Some("未声明")`——要和「完全不知道」（`None`，还没同步过）区分开。
- 详情的 `form`：AI 卡的 `form` 优先；没有时用 `infer_form(topics, full_name, description)`。
- **只改详情**。列表（`into_item`）、筛选、计数、SQL 都**不动**——形态筛选的计数仍按 AI 卡算，这张卡不改这个口径。
- 契约（`contracts.ts`）形状不变（`license: string | null`、`form: string | null`）。
- 测试：详情的许可证三种来源的优先级；`declared == false` 返回「未声明」；无 AI 卡的仓库 `form` 来自话题；列表项的 `form` 行为不变。

## 不要做
- 不改前端、不改契约、不改 `packs.rs` / `solve.rs`（后面的卡用 `repo_facts`）。
- 不联网、不读钥匙串；测试里不要碰真实数据目录。
- 不改 `001`–`031` 的迁移文件。
- 不要为了「更聪明」而扩大 `infer_form` 的规则（宁可 `None`）。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过，新增测试**至少 14 个**（迁移 2、解析 + 落库 4、`license_label` 4、`infer_form` 至少 6、详情 3）。
2. `tools/cards/run.sh rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
3. `git diff --stat` 里只出现「允许改动」里的路径；`001`–`031` 迁移文件没有改动。
4. 报告里给出：`upsert_repositories` 里新列出现的行（两个 `ON CONFLICT` 都有）；`StarredRepository` 手写构造被你改了几处。

## 停止条件
- 要改契约、前端或卡外文件；要新依赖；发现 `repositories` 的列被别的代码按位置读取（`SELECT *`）导致加列会出错——停手并写进报告。
