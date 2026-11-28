# 界面验收工具

## 当前可重复的隔离验收（2026-10-28）

`acceptance.mjs` 验收生产构建的页面，`acceptance-fixture.js` 替换 Tauri、剪贴板和数据。无需启动开发 App，不访问真实库、账号、预算或 AI。使用独立 Chrome 上下文，仅允许随机本地端口；结束时关闭浏览器与临时 HTTP 服务。

```bash
cd <repo-root>
tools/cards/run.sh web-build
FSL_BROWSER_CHANNEL=chrome \
FSL_PLAYWRIGHT_PATH=<playwright-path> \
FSL_ACCEPTANCE_OUT=<output-dir> \
tools/cards/run.sh ui-test
```

可设置 `FSL_BUILD_DIR` 指向另一份已构建的 `apps/desktop/dist` 来作原版对照。默认查找现有 `playwright` 包与浏览器。输出 `results.json` 以及截图，任一操作失败返回非零退出码。当前覆盖 25 条主流程和失败恢复，包括 X 帖文补贴正文、候选勾选、跨来源去重和纯文本保存。

发布脚本故障演练：`tools/cards/run.sh script-test`。只在临时目录替换 Git、构建和预算文件；7 个测试方法、23 个场景，不执行实际发布。

以下保留旧工具说明作参考；旧 App/真实数据操作仍需对应授权，不能因这里有命令就直接执行。

---

# 看真实界面的小工具（macOS）

**为什么有它**：开发版（`pnpm dev:desktop`）是没有 bundle id 的裸可执行文件，computer-use 控制不了它；而「页面能打开、测试通过」不等于「界面能用」。2026-10-02 之前，这个项目从没人真正看过自己做出来的界面。这里的工具只用系统自带能力：窗口截图，加辅助功能接口按名称点按钮、给输入框赋值。

**前提**：运行它们的应用（Claude 桌面版或终端）已在「系统设置 → 隐私与安全性」里获得**屏幕录制**和**辅助功能**权限。

## 编译（一次）

```bash
cd <repo-root>
swiftc -O -o /tmp/winlist tools/dev-ui/winlist.swift
swiftc -O -o /tmp/axui tools/dev-ui/axui.swift
```

## 用法

```bash
PID=$(pgrep -f "target/debug/gsat-desktop" | head -1)

/tmp/winlist                         # 找到窗口 id 和位置（标题应为 Fox Stars Lab Dev）
/tmp/axui move  $PID 0 40            # 把窗口挪到屏幕空白处（被盖住时 WebKit 会停止渲染，截图是旧的）
/tmp/axui front $PID                 # 置前
/tmp/axui dump  $PID                 # 列出可点的按钮、链接、输入框
/tmp/axui dump  $PID --all           # 列出所有有文字的元素
/tmp/axui press $PID 资料库           # 按名称包含「资料库」的第一个按钮；末尾可加序号，如 press $PID 资料库 1
/tmp/axui settext $PID "搜索" "小红书" # 给占位符或名称为「搜索」的输入框赋值
/tmp/axui text  $PID                 # 按顺序打印页面上的文字（读结果用）

screencapture -x -o -l <窗口id> /tmp/shot.png   # 只截这个窗口
```

## 注意

- `axui` 只认标题含 `Fox Stars Lab Dev` 的窗口，不会操作正式版。
- 只在 dev 版上用。会写库的按钮（把当前页标为想试、加入收集箱、生成建议卡和分类、重试失败任务、备份、导出）不要乱点；点之前先 `.backup` 一份 dev 库。
- 旧版曾有命令面板（⌘K）无法用 `Escape` 关闭的问题；批次 A 已修复并通过隔离页面验证，原生窗口仍需发布后验证。

## 假后端看界面（mock-host）

不碰开发版、开发库和钥匙串，在浏览器里看新界面、读页面报错、换窗口宽度：

```bash
# 前提：开发版已经在跑（vite 在 127.0.0.1:1420）
python3 tools/dev-ui/mock-host/serve.py 38413
# 然后用浏览器打开 http://127.0.0.1:38413/
```

- `serve.py` 把 vite 原样转发，只在首页里先塞一段 `shim.js`，让页面以为自己跑在 Tauri 里。
- 所有后端命令由 `shim.js` 里的假数据回答；没写的命令 `list_*` 返回 `[]`，其余返回 `null`，并在控制台打 `[mock]` 提示。要看某个页面的真实数据形状，就在 `HANDLERS` 里补一个。
- 不转发 websocket，热更新不起作用，改完代码刷新页面即可。
- 真正的 WKWebView 里的渲染（字体、窗口被盖住时停止绘制等）仍要用上面的窗口截图方法看一眼开发版。
- 注意：开发窗口被别的窗口完全盖住时，WebKit 会停止绘制，截图会是一片空白或旧画面（看起来像「白屏」），先用 `axui front <pid>` 把它拉到最前面再截图。

### 真后端桥：假外壳 + 真 Rust 代码 + 开发库副本

`serve.py` 带第三个参数时，页面里的命令先交给「真后端桥」回答（桥在 `review-harness/review_dump.rs` 里，跑真的 `library_view`、`solve`、`inbox_view`、`packs`、`card_handler` 等函数，数据库是**开发库的副本**），桥不认识的命令才退回 `shim.js` 的假数据：

```bash
python3 tools/dev-ui/mock-host/serve.py 38415 http://127.0.0.1:1422 http://127.0.0.1:38500
```

## 用真数据验收（review-harness）

`tools/dev-ui/review-harness/review_dump.rs` 是总控的临时审核夹具，**不编译进仓库**。用法：

1. 建一个临时工作区（例如 `git worktree add -b review/tmp ../fox-stars-lab-wt/_review fox-integration`），克隆编译缓存（`cp -c -R <集成区>/apps/desktop/src-tauri/target <临时区>/...`）并把 `apps/desktop/dist` 拷过去（否则 `tauri::generate_context!` 编译失败）。
2. 把 `review_dump.rs` 放进 `apps/desktop/src-tauri/src/`，在 `lib.rs` 里加 `#[cfg(test)] mod review_dump;`（这两处都**不要提交**）。
3. 取开发库副本：`cp` 开发库的 `.sqlite3`（和 `-wal`）到草稿目录，不要碰开发目录里的原文件；正式库目录只能 `stat`。
4. 三个 `#[ignore]` 测试，都要环境变量 `FSL_REVIEW_DB`（副本路径）和 `FSL_REVIEW_OUT`（输出目录），**测试名要写全**（`review_dump::review_dump` 只是 dump；过滤词 `review_dump` 会连 `review_server` 一起启动，它会一直阻塞）：
   - `review_dump::review_dump`：对副本跑迁移、种子、规则归类和所有新后端函数，把真实结果存成 JSON，并打印耗时。
   - `review_dump::review_server`：在 `127.0.0.1:38500`（`FSL_REVIEW_PORT` 可改）起 HTTP 桥，供上面的 `serve.py` 第三个参数使用。
   - `review_dump::review_pipeline_rehearsal`：**流水线演练**。起一个本机假的 OpenAI 兼容服务（返回固定合法理解卡，每次记 3600 token），用真队列、真处理器、真流水线在副本上跑：预算 0 时不调模型；授权 30000 后到预算就停；检查「调用次数 = 成功任务数」（曾靠它发现并发时 `database is locked` 白花模型费用）。不花钱、不读钥匙串。
   ```bash
   cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib review_dump::review_pipeline_rehearsal -- --ignored --nocapture --test-threads=1
   ```

