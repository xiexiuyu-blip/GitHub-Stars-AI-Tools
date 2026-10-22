---
编号: 46-version-and-updater
允许改动:
  - package.json
  - apps/desktop/package.json
  - apps/desktop/src-tauri/tauri.conf.json
  - apps/desktop/src-tauri/Cargo.toml
  - apps/desktop/src-tauri/Cargo.lock
  - apps/desktop/src-tauri/src/lib.rs
  - apps/desktop/src/providers/app-update-provider.tsx
  - apps/desktop/src/providers/app-update-provider.test.ts
  - apps/desktop/src/components/app-update-panel.tsx
  - apps/desktop/src/lib/api.ts
  - apps/desktop/src/lib/api.test.ts
  - tools/dev-ui/mock-host/shim.js
---

# 卡 46 · 版本号写成 2.0.0-fox.1，并关掉「向上游 GitHub 检查更新」（前端 + 配置，只加一条 Rust 测试）

> 执行者：Grok（见分配单）｜不加迁移、不加依赖、不改 JS 依赖和锁文件、**不碰 lib.rs 里除测试以外的任何代码**。

## Fox 能看到的效果
打开「应用更新」（顶栏快捷菜单里的「检查更新」，或设置里的关于页），当前版本显示 **2.0.0-fox.1**；这个窗口里**没有「检查更新」按钮**，只有一句话：「这个版本不会自动检查更新。有新版本时，会由开发者重新安装；你的资料不受影响。」应用启动时**不再偷偷联网**去上游仓库问有没有新版。以后上游发 2.x 也不会弹出「有新版本，立即安装」把 Fox 的版本换掉。

## 为什么有这张卡
2026-10-06 切库时，正式版是临时改了版本号才装上的（见 `docs/rebuild/切库清单.md`），仓库里的版本号还是 1.5.3-fox.2。同时 `tauri.conf.json` 的 `plugins.updater.endpoints` 仍指向上游 `GitHub-Stars-AI-Tools` 的 `latest.json`，`app-update-provider.tsx` 启动时会静默检查。现在上游版本号比我们低，不会提示；上游一旦发布 2.x，Fox 点了「立即安装」，装进来的就是**别人的版本**。要在那之前堵上。

## 背景与先读什么
- 当前版本号在 6 个地方：根 `package.json:3`（1.5.3-fox.2）、`apps/desktop/package.json:3`（1.5.3）、`apps/desktop/src-tauri/tauri.conf.json:4`（1.5.3-fox.2）、`apps/desktop/src-tauri/Cargo.toml:3`（1.5.3-fox.2）、`apps/desktop/src-tauri/Cargo.lock` 里 `name = "gsat-desktop"` 那一项（约 1746–1747 行，1.5.3-fox.2）、`tools/dev-ui/mock-host/shim.js:20`（开发预览用的假身份，1.5.3-fox.2）。
- `apps/desktop/src-tauri/tauri.conf.json`：`plugins.updater` 里有 `pubkey` 和上游的 `endpoints`。**`plugins.updater` 这一整块必须留着、`pubkey` 一个字不改**——`lib.rs` 里注册了 `tauri_plugin_updater`，配置块整个没有会让应用启动失败；`endpoints` 为空数组是合法配置，这时插件的 `check()` 会返回错误 `Updater does not have any endpoints set.`，不会发出任何网络请求。
- `apps/desktop/src/providers/app-update-provider.tsx`：约 100–102 行 `useEffect(() => { void checkForUpdate({ silent: true }); }, [])` 是启动时的静默检查；`runUpdateCheck` 里调用 `check()`。
- `apps/desktop/src/components/app-update-panel.tsx`：`AppUpdatePanel` 里有「当前版本」「最近检查」两行和「检查更新」按钮。
- `apps/desktop/src/lib/api.ts` 约 306–360 行：`describeUpdaterError`、`describeUpdateStatus`（`idle` 那句现在写着「应用会在启动时静默检查更新，也可以在这里手动检查。」）。
- 既有测试：`apps/desktop/src/lib/api.test.ts` 约 395–425 行（`describeUpdateStatus` 的几个用例，421 行断言 `toContain('静默检查更新')`）；`apps/desktop/src/lib/automation-view.test.ts` 约 1686 行有一条文案守卫：`app-update-provider.tsx` 里不能出现 `latest.json`、`GitHub Release`、`更新文件没找到 / 校验没通过`（**这个测试文件你不能改，也不需要改；新文案别写进 provider 里**）。
- 入口：`App.tsx` 里 `<AppUpdateProvider>` 和 `AppUpdateDialog`、`components/app-layout.tsx` 约 372 行的「检查更新」菜单项、`pages/settings.tsx` 的关于页。**这几个文件不改**——它们会自动跟着新行为走。

## 要做的事

### 1. 版本号统一成 `2.0.0-fox.1`
上面 6 个地方全部改成 `2.0.0-fox.1`（`Cargo.lock` 只改 `gsat-desktop` 这一项的 `version`，别的包一个字都不动）。`Cargo.lock` 的做法：改完 `Cargo.toml` 后跑一次 `tools/cards/run.sh rust-check`，cargo 会自己把那一行改对；然后用 `git diff apps/desktop/src-tauri/Cargo.lock` 确认**只有这一行变了**。如果 diff 里还有别的包被动了，立刻还原那些行，把原因写进报告。

### 2. 配置：更新端点清空，公钥保留
`tauri.conf.json` 的 `plugins.updater.endpoints` 改成 `[]`。`pubkey` 原样保留，不要打印、不要复制到报告里。`bundle.createUpdaterArtifacts` 等其它字段**不动**。

### 3. 前端：不再自己去问，面板里说清楚
- `lib/api.ts` 新增导出常量 `export const IN_APP_UPDATES_ENABLED: boolean = false;`，上面写一行注释：这个版本由开发者手动安装，应用内更新关着；要重新打开更新，必须同时给 `tauri.conf.json` 配上自己的端点和签名，而不是只改这个开关。
- `lib/api.ts` 的 `describeUpdateStatus`：`idle` 那一句改成「这个版本不会自动检查更新。有新版本时，会由开发者重新安装；你的资料不受影响。」其它状态的文案不动。
- `providers/app-update-provider.tsx`：
  - **删掉**启动时静默检查的那个 `useEffect`（整个 effect，不是只改参数）。读取当前版本号的那个 effect 保留。
  - `checkForUpdate` 的最开头加：`if (!IN_APP_UPDATES_ENABLED) return;`，这样就算别处误调用也**不会调到 `check()`**。其余逻辑（下载、安装、重启）原样保留，将来重新打开只改一个开关。
  - 别在这个文件里写 `latest.json`、`GitHub Release`、`更新文件没找到 / 校验没通过` 这些字（有文案守卫）。
- `components/app-update-panel.tsx`：`IN_APP_UPDATES_ENABLED` 为 false 时，不画「最近检查」那一行和「检查更新」按钮（其余照旧）；为 true 时和现在完全一样。`statusMessage` 仍由 `describeUpdateStatus` 给出。

### 4. 测试
- `lib/api.test.ts`：把 421 行那条断言改成新句子的关键片段（`toContain('不会自动检查更新')`），并加一条：`IN_APP_UPDATES_ENABLED` 是 `false`。
- 新建 `providers/app-update-provider.test.ts`（照 `pages/v2/toolbox.test.tsx` 的写法：`import SOURCE from '@/providers/app-update-provider.tsx?raw'`），至少这几条，**每条改坏就失败**：
  1. 源码里没有 `silent: true`（没有任何地方还在启动时静默检查）；
  2. `checkForUpdate` 函数体里，`IN_APP_UPDATES_ENABLED` 的判断出现在 `runUpdateCheck(` 之前；
  3. 读取当前版本的 `getVersion()` 还在。
- `src-tauri/src/lib.rs` 里已有的 `#[cfg(test)] mod tests` 末尾加**一条** Rust 测试（只加测试，不碰别的代码）：用 `include_str!("../tauri.conf.json")` 解析成 JSON，断言：① `plugins.updater.endpoints` 是**空数组**；② `plugins.updater.pubkey` 是非空字符串（断言失败时的消息里**不要带出公钥**）；③ 顶层 `version` 等于 `env!("CARGO_PKG_VERSION")`；④ `version` 等于 `"2.0.0-fox.1"`（以后升版本时这一条跟着改，是故意的）。

## 不要做
- 不改 `pubkey`，不删 `plugins.updater` 这一整块，不改 `bundle` 里任何字段，不改 `capabilities/default.json`。
- 不删 `lib.rs` 里注册 `tauri_plugin_updater` / `tauri_plugin_process` 的那几行，不改 `lib.rs` 的其它任何非测试代码。
- 不改 `App.tsx`、`app-layout.tsx`、`pages/settings.tsx`、`automation-view.test.ts`。
- 不运行 `pnpm install` / `cargo update`；不改 `pnpm-lock.yaml`；不碰 JS 依赖。
- 不改 `docs/` 下任何文件（文档由总控改）。

## 验收
1. `grep -rn "2.0.0-fox.1" package.json apps/desktop/package.json apps/desktop/src-tauri/tauri.conf.json apps/desktop/src-tauri/Cargo.toml tools/dev-ui/mock-host/shim.js` 六处都在（Cargo.lock 的那一项另给 `grep -n -A1 'name = "gsat-desktop"' apps/desktop/src-tauri/Cargo.lock`）。仓库里（除 `docs/` 和 `node_modules`）不再有 `1.5.3-fox.2`。
2. `git diff apps/desktop/src-tauri/Cargo.lock` 只有一行变化。
3. `grep -n "endpoints" apps/desktop/src-tauri/tauri.conf.json` 显示 `[]`；`grep -c "GitHub-Stars-AI-Tools" apps/desktop/src-tauri/tauri.conf.json` 为 0。`pubkey` 那一行 `git diff` 里**看不到**（没改）。
4. `grep -n "silent: true" apps/desktop/src/providers/app-update-provider.tsx` 没有输出；`checkForUpdate` 的第一条语句是 `IN_APP_UPDATES_ENABLED` 的判断。
5. `tools/cards/run.sh web-build`、`web-test`、`rust-fmt`、`rust-check`、`rust-test` 全部通过；报告里写前端测试文件数 / 项数、Rust 通过 / 失败 / 忽略数，**和合并前的基线逐项对比**（基线：前端 853 项左右、Rust 1378 通过 5 忽略——以你在改动前先跑一次的数字为准）。
6. 改坏检查（每条都实际做一遍，报告里写「改了什么 → 哪几条测试失败」）：① `endpoints` 改回一个 https 地址；② `IN_APP_UPDATES_ENABLED` 改成 `true`；③ provider 里把启动的 `useEffect` 加回来；④ 把 `tauri.conf.json` 的 `version` 改成别的。每条改完要还原。

## 停止条件
- `cargo check` 之后 `Cargo.lock` 里除 `gsat-desktop` 外还有别的包变化，且还原后 cargo 又改回去 → 停手，写进报告。
- 发现必须改「不要做」里的文件才能完成 → 停手写进报告，不要硬改。
- 发现 Tauri 对 `endpoints: []` 报配置错误（`rust-check` 或测试里出现与 updater 配置相关的失败）→ 停手写进报告，不要换成别的配置写法。
- 需要新依赖 → 停手写进报告。
