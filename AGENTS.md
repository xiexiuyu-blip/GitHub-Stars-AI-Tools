# AGENTS.md：协作者与 AI 开发守则

> 适用范围：所有在本仓库参与开发、审查与维护的团队成员及 AI 编码助手。  
> 开始改动前建议先读 [README.md](README.md)、[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 与 [docs/HANDOFF.md](docs/HANDOFF.md)。

---

## 一、不可突破的工程红线

1. **严禁触碰正式数据目录与真实凭据**：
   - 开发与自动化测试不得读写正式数据目录（macOS 下为 `~/Library/Application Support/com.foxwork.fox-stars-lab/`）。
   - 不读取真实钥匙串、`.env`、本地 API 令牌文件；密钥、Token、主机个人目录严禁进代码、测试夹具或文档提交。
   - 测试一律使用临时目录、内存数据库或确定性 `mock-host` 替身；本地启动仅使用 `COREPACK_HOME="$PWD/.corepack" pnpm dev:desktop`（dev 隔离身份）。
2. **每日 AI 额度（`spend_guard`）是硬闸门**：
   - 任何向 AI 服务发请求的新代码，必须走后台队列（`jobs.rs`）或经过 `spend_guard.rs`（`check_before_request` / `record_after_request`），不得新增绕过 `dailyBudgetTokens` 限制的路径。
   - 仅保留两处经用户在界面显式点击 `confirmed: true` 触发的例外：**「一次性全部整理」**（`fullScan` 48 小时 / 4 倍熔断）与 **「单张重做」**（单行 `ai_jobs.spend_authorized_until` 30 分钟授权）；除这两处外不得再开旁路。
3. **不可信内容线性安全解析**：
   - 解析网页 HTML、README、用户粘贴文本与外部 API 返回数据时，必须采用线性时间逻辑（禁止回溯正则）、按 Unicode 字符边界切片（非测试 Rust 代码禁用 `unwrap()` / `expect()`）、设大小与超时上限，并配畸形/恶意压缩行单测夹具。
   - 对私网/回环/重绑定地址继续沿用 `capture.rs` 的 `url_is_safe` 校验；GitHub 令牌仅允许发往 `api.github.com`。
4. **数据库向后兼容与编译期内嵌路径稳定**：
   - 不修改既有迁移 `001–040`，只做顺序追加（下一个为 `041_*.sql`）并在 `apps/desktop/src-tauri/src/db/migrations.rs` 登记。
   - 不改名、不删除 `repositories`、`repo_ai_documents`、`repo_tags`、`annotations` 与 `v_public_*` 既有字段。
   - **禁止移动** Rust 通过 `include_str!` 编译引用的路径：`docs/rebuild/spec/*.json` 与 `docs/quality/embedding-golden.json`。
5. **诚实交付与不放水验收**：
   - 不得为了通过测试而删用例、放宽断言或随意加 `#[ignore]`。
   - 代码改动后须实跑 `tools/cards/run.sh` 对应校验（`web-build`、`web-test`、`rust-fmt`、`rust-test`、`script-test` 等），并在交付说明中如实区分已验证与未验证项。

---

## 二、代码与界面风格约定

- **语言**：界面展示文案、面向用户的错误提示、文档与非显而易见原因的注释使用简体中文；代码标识符、命令键名使用英文。
- **前端界面**：React 19 + Tailwind CSS 4，遵循 [apps/desktop/DESIGN.md](apps/desktop/DESIGN.md) 设计令牌与「文案与设置页写法」，遵守 [docs/rebuild/文案审读.md](docs/rebuild/文案审读.md) 统一用词表，并保证 `apps/desktop/src/lib/old-terms.test.ts` 通过。
- **Rust 后端**：提交前执行 `tools/cards/run.sh rust-fmt-fix`，数据库写入走事务，长任务保持可中断、可重试与幂等。
- **历史隔离任务卡会话说明**：仅当通过 `tools/cards/dispatch*.sh` 启动隔离沙盒任务卡会话时，才适用 [docs/rebuild/命令契约.md](docs/rebuild/命令契约.md) 中的卡片范围限制（如禁用 `pnpm dev:desktop`、禁用 `git` 写命令并交回 `.card-report.md`）。日常团队协作以本文与 [docs/HANDOFF.md](docs/HANDOFF.md) 为准。

---

## 三、PR 自动代码审查必查规则（Review Guidelines）

Codex 自动代码审查与人工/AI 复核 PR 时，除遵循上文工程红线外，重点检查以下高风险项（仅报告可证实的 P0/P1 缺陷或真实回归）：

1. **Tauri 命令与 ACL 权限同步**：凡在 `apps/desktop/src-tauri/src/lib.rs` 注册的新 `#[tauri::command]`，必须同步加入 `apps/desktop/src-tauri/permissions/gsat-commands.toml`（并更新 `gen/schemas/acl-manifests.json`）且附权限断言单测；避免出现 `mock-host` 通过但原生桌面端被 ACL 拦截的假绿。
2. **Rust 运行时与数据库安全**：非测试 Rust 代码严禁 `.unwrap()` / `.expect()`；SQLite 写操作包在事务内；SQL `LIKE` 前缀匹配必须正确转义 `%` / `_` 或在 Rust 侧二次精确校验；恢复、迁移与跨平台路径（如空设备路径 `NUL` / `/dev/null`、目录 `sync_all`）需兼顾 macOS / Linux / Windows。
3. **联网安全与凭据防泄**：`Authorization` / 凭据头禁止经由子进程参数传递，仅可发往 `api.github.com:443` 且不得在跨站重定向中转发；外部页面与短链抓取/展开必须逐跳做 HTTPS、443 端口、公网 IP SSRF 拦截与响应体积/超时上限。
4. **AI 额度与账本守恒**：任何消耗 AI 额度的调用（含截断重试、JSON 修复与新能力）必须在发起请求前校验剩余额度与未结算保护，且模型已计费后即使本地写入或结算失败也不得丢账。
5. **前端文案与脱敏守卫**：界面可见文字不得触发 `apps/desktop/src/lib/old-terms.test.ts` 禁用词；任何代码、配置与文档不得泄露本机目录、内部主机名/内网 IP 或真实密钥。
