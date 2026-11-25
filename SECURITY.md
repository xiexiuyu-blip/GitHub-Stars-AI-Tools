# 安全策略（Security Policy）

## 支持的版本

| 版本分支 | 状态 | 说明 |
|---|---|---|
| `2.1.x`（`main`） | 活跃维护 | 接收安全修复与功能更新 |
| `2.0.x` / `1.x` | 停止维护 | 请升级至最新 `2.1.x` 版本 |

---

## 本项目的核心安全边界

Fox Stars Lab 在设计上围绕以下安全原则构建：

1. **凭据与本地数据隔离**：GitHub Token 与 AI 服务密钥保存在系统钥匙串（macOS Keychain / Windows Credential / Secret Service），不写入仓库源码、`localStorage` 或明文日志；开发模式（`.dev`）与正式应用目录严格隔离。
2. **本机 HTTP API 访问控制**：`local_api.rs` 默认关闭；启用后仅允许绑定 `127.0.0.1`，除 `/v1/health` 外均需 256 位 Bearer 令牌，严格校验 `Host` 并拒绝携带 `Origin` 头的浏览器请求。
3. **AI 预算门禁**：默认 `dailyBudgetTokens = 0`，后台队列与对话调用统一受 `spend_guard` 约束，避免异常死循环或未授权触发高额花费。
4. **不可信内容与 SSRF 防护**：来自 GitHub README、第三方网页及粘贴文本的内容在提示词与解析器中均被视为不可信输入；链接抓取前会检查并拦截私网、回环与重绑定地址。

---

## 如何报告安全漏洞

**请勿在公开 GitHub Issue 或公开讨论区披露尚未修复的安全漏洞细节。**

请通过 GitHub 仓库自带的私密漏洞通道提交报告：

1. 进入本仓库在 GitHub 的 **Security** 标签页；
2. 点击 **Advisories → Report a vulnerability**（Private Vulnerability Reporting）；
3. 尽量附带复现步骤、影响模块（如 `capture.rs`、`local_api.rs`、`spend_guard.rs`、`auth.rs` 或前端 IPC）及最小复现载荷。

> 报告时请务必先移除真实 GitHub Token、AI API Key 或个人数据库文件。团队内同事也可先通过内部安全沟通渠道同步给仓库维护者核查处理。
