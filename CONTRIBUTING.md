# 贡献指南（Contributing Guide）

欢迎团队同事与开源社区贡献者参与 **Fox Stars Lab** 的开发与维护。提交代码或文档前，请先阅读 [README.md](README.md)、[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) 与 [AGENTS.md](AGENTS.md)。

---

## 一、协作分支与提交流程

1. **禁止直接向 `main` 提交或强推（force push）**：所有改动必须从最新 `main` 切出独立分支，通过 Pull Request 经审阅及自动检查后再合并。
2. **分支命名建议**：
   - `feat/<简短主题>`：新功能（例如 `feat/mcp-readonly-bridge`）
   - `fix/<简短主题>`：缺陷修复（例如 `fix/discover-feed-fallback`）
   - `refactor/<简短主题>`：结构重构
   - `docs/<简短主题>`：文档与说明完善
   - `test/<简短主题>`：测试补充与用例加固
3. **Commit 信息规范**：采用 Conventional Commits 风格，正文可使用简体中文：
   - `feat(discover): ...` / `fix(solve): ...` / `docs: ...` / `test: ...` / `chore: ...`
4. **保持 PR 聚焦**：一次 PR 只解决一个清晰目标；若同时修改 Rust 后端与前端契约，确保类型与单测同步提交。

---

## 二、高风险改动必查规范

涉及以下目录或逻辑时，须由对应模块维护者（见 [.github/CODEOWNERS](.github/CODEOWNERS)）审阅通过：

### 1. SQLite 数据库迁移（`packages/storage/migrations/fox/`）
- **严禁修改 `001–039` 已有迁移文件**，也禁止改名或删除 `repositories`、`repo_ai_documents`、`repo_tags`、`annotations` 与 `v_public_*` 既有字段。
- 新增表或列须同步最新 `main` 后按顺序追加编号（如 `040_<name>.sql`），并在 `apps/desktop/src-tauri/src/db/migrations.rs` 的 `MIGRATIONS` 中登记与补充迁移测试。
- **并发编号冲突处理**：若两名协作者在同一时间基于 `main` 追加了相同编号的迁移文件，后合并的 PR 须先 `rebase main` 并顺延迁移编号后再重跑测试。

### 2. AI 额度闸门与费用安全（`spend_guard.rs` / `jobs.rs`）
- 任何调用对话/生成模型的路径都必须走后台队列（`jobs.rs`）或经过 `spend_guard.rs`，不得新增绕过 `dailyBudgetTokens` 的自动调用。
- 新增付费能力必须默认关闭或显式标明代价，并在额度为 `0`、耗尽或离线时提供可用降级方案。

### 3. 凭据、本机 HTTP API 与内容安全
- 禁止把任何 GitHub Token、AI API Key、个人主机名、本机绝对路径（如用户主目录路径）或内部 IP 写入源码、测试夹具或文档。
- 本机 HTTP API（`local_api.rs`）必须保持仅监听 `127.0.0.1`、严格校验 `Host` 与拒绝携带 `Origin` 的浏览器请求。
- 解析外部 HTML、README 或粘贴文本时须采用线性有限状态逻辑与字符边界安全切分，并附带恶意构造输入测试。

### 4. 界面文案与内嵌资源路径
- 新增或修改界面文字、设置项前，请核对 [apps/desktop/DESIGN.md](apps/desktop/DESIGN.md)「文案与设置页写法」与 [docs/rebuild/文案审读.md](docs/rebuild/文案审读.md) 统一用词表。
- **禁止移动** Rust 通过 `include_str!` 编译引用的路径：`docs/rebuild/spec/*.json` 与 `docs/quality/embedding-golden.json`。

---

## 三、发起 Pull Request 前本地必跑验证

请至少确保以下本地检查全部通过（开发环境须使用隔离 `dev` 身份，勿触碰正式数据目录）：

```bash
tools/cards/run.sh web-build     # 共享包与桌面前端编译检查
tools/cards/run.sh web-test      # 前端 Vitest 单测（含文案守卫 old-terms.test.ts）
tools/cards/run.sh rust-fmt      # Rust 格式检查（可用 rust-fmt-fix 自动整理）
tools/cards/run.sh rust-test     # Rust 后端全量单测
tools/cards/run.sh script-test   # 门禁脚本测试
```

涉及核心交互流程或路由改动时，建议同时参照 [tools/dev-ui/README.md](tools/dev-ui/README.md) 补充运行隔离端到端验收 `tools/cards/run.sh ui-test`。

---

## 四、团队内部协同与开源许可说明

- **公司内部仓与 GitHub 同步原则**：公司内部开发与 GitHub 开源共享统一 `main` 主线。内部开发同样禁止在文档、测试与提交记录中残留内网地址、私人账号路径或敏感配置。
- **许可证与贡献授权**：本仓库采用 [PolyForm Noncommercial License 1.0.0](LICENSE)。向本仓库提交 Pull Request，即表示你确认所提交内容为本人原创或具备合法公开授权（不含未授权专有代码或商业机密），并同意你的贡献以仓库现行许可证条款发布。
