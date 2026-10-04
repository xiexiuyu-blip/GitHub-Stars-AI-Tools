---
编号: 28-direct-handoff
允许改动:
  - apps/desktop/src-tauri/src/packs.rs
  - apps/desktop/src-tauri/src/pack_guard.rs
  - apps/desktop/src/lib/api.ts
  - apps/desktop/src/lib/solve-view.ts
  - apps/desktop/src/lib/solve-view.test.ts
  - apps/desktop/src/lib/solve-draft.ts
  - apps/desktop/src/pages/v2/solve.tsx
---

# 卡 28 · 找方案：没有合适的工具时，「直接交给 Codex」

## 为什么有这张卡
2026-10-04 一个站在 Codex 角度的审查员用 6 个真实任务试了「找方案」→「复制给 Codex」。其中「批量改图尺寸和压缩」收藏里确实没有专用工具，找方案要么什么都没有，要么只剩几个不相干的项目（下载器、向量库），**而参考包必须至少选 1 个项目才能复制**——所以这条路是死的。但这件事本机的命令行就能做（macOS 自带 `sips`，常见的还有 `magick`、`ffmpeg`、`cwebp`），Codex 不需要任何收藏里的工具。

Fox 的原则是「用现成工具做成品，不开发新软件」。「没有合适的收藏」时，应该让他一键把这件事交给 Codex，并明确写上：先查本机有什么、够用就直接做、不够就停下来告诉我，**不要自己写一个新软件**。

## Fox 能看到的效果
- 找方案页右边「参考包」面板里，**没勾任何工具时**，多一个文字按钮「没有合适的？直接交给 Codex」。
- 点它：面板预览变成一份没有项目列表的任务交接单（这次要做的事 / 先这样做 / 开工前先问我 / 完成标准 / 工作约定，以及有的话「我本机已经有的」），「复制给 Codex」变成可点。面板顶部显示「不选工具 · 直接交给 Codex」，旁边有文字按钮「改回挑工具」。
- 再勾选任何一个工具、重新找、或点「改回挑工具」，都回到原来的参考包；切到别的页面再回来，状态保持（和已有的草稿一致）。

## 背景与先读什么
- `apps/desktop/src-tauri/src/packs.rs`：`PackRequest`（第 112 行一带）、`build_pack`（约 155 行，空 `items` 时返回 `还没有选任何参考项目`）、`render_markdown` / `render_agents` / `render_json`、`push_markdown_project`、`WORK_RULES`、`ABOUT_ME`、`problem_title`、`neutralize_block`。
- `apps/desktop/src-tauri/src/pack_guard.rs`（总控刚写的）：`task_kinds`（按任务关键词认出视频 / 记录整理 / 批量处理 / 写作 / 图文 / 通用）、`ask_items(problem, flagged)`、`done_items(problem)`。**直接交接单的「开工前先问我」「完成标准」沿用它们**（`ask_items(problem, &[])`）。
- `docs/rebuild/命令契约.md` §9「参考包 JSON v2」：`task.mode` 现在固定 `use_tools`；这张卡要让它可以是 `direct`（契约文档由总控改，你不要动）。
- 前端：`apps/desktop/src/lib/api.ts`（`PackRequest` 类型）、`lib/solve-view.ts`（`buildPackRequest`、`packPlaceholder`、`pickLocalCapabilities`）、`lib/solve-draft.ts`（切页保留状态）、`pages/v2/solve.tsx`（页面：参考包面板、`copyToCodex`、`canCopy`）。
- 参考包文字里不许出现「参考包」「参考项目」这两个词（现有测试 `assert_forbidden_phrases` 在查）；直接交接单也一样，用「工具」「收藏里的工具」。

## 要做的事

### 1. Rust：`PackRequest.mode`
- `PackRequest` 新增字段 `#[serde(default)] pub mode: PackMode`；`enum PackMode { UseTools, Direct }`（`#[serde(rename_all = "camelCase")]`，`Default` 是 `UseTools`；前端传 `"useTools"` 或 `"direct"`，不传就是 `UseTools`）。
- `build_pack`：
  - `UseTools`：行为**完全不变**（空 `items` 仍报 `还没有选任何参考项目`）。
  - `Direct`：`items` 必须为空，否则返回 `Err("直接交给 Codex 时不能同时选工具")`；`problem` 去掉空白后为空返回 `Err("先写一句你想做的事")`。成功时 `PackOutput.item_count == 0`，`missing` / `warnings` 为空。不读库里的仓库。
- 注意：`build_pack` 现在先判断空 `items`、再 `load_repo`；`Direct` 要在判断空 `items` **之前**分支。

### 2. Rust：直接交接单的三种格式
先在 `pack_guard.rs` 加 `pub(crate) fn direct_steps(problem: &str) -> [String; 3]`，按 `task_kinds(problem).first()` 选一句工具提示 `HINT`：
- 批量处理 / 图文 / （认不出）→ `macOS 自带的 sips 可以改图片尺寸和格式；常见的还有 magick（ImageMagick）、cwebp`
- 视频 → `ffmpeg（剪辑、转码、烧字幕），macOS 自带的 afconvert 处理音频`
- 记录整理 / 写作 → `macOS 自带的 textutil 和 pandoc 可以转文档格式`

三步（逐字照抄，`{HINT}` 替换）：
1. `先查这台电脑上已经有什么：已装的 Skill 和 MCP（只看名字，不要读里面的密钥），以及命令行里现成的工具（{HINT}）；用 which 查有没有。`
2. `现成的够用就直接做：可以把几条现成命令串起来完成这一次的活，但不要开发一个新软件，也不要为这件事装来路不明的东西。`
3. `不够用就停下来，告诉我缺什么、已经试过什么，我来决定换办法，还是去找新工具。`

**Markdown**（`render_direct_markdown`）：
```
# 任务交接：{problem_title(problem)}
> Fox Stars Lab 生成于 {generated_on}。{ABOUT_ME}
> 我的收藏里没有找到合适的现成工具，所以这份交接单没有列工具。

## 这次要做的事
{neutralize_block(problem)}

## 先这样做
1. …（direct_steps 三条，带编号）
2. …
3. …

## 我本机已经有的        ← 只有 local_capabilities 非空才写，格式和现有完全一样
- name：one_liner（path）

## 开工前先问我
- …（ask_items(problem, &[])）

## 完成标准
- …（done_items(problem)）

## 工作约定
1. …（WORK_RULES 全部 7 条，带编号）
```
**AGENTS.md**（`render_direct_agents`）：沿用现有 `render_agents` 的开头两行和「工作规则」「验收」，把「可用的工具（按顺序）」换成：`用现成工具出成品，不开发新软件。这次我的收藏里没有找到合适的现成工具。` + `## 先这样做`（三条）+（有本机能力才有）`## 本机已经有的`。
**JSON**（`render_json` 里 `Direct` 分支）：顶层键 = 现有那些键 **加** `directSteps`（3 条字符串，放在 `askFirst` 后面、`verify` 前面）；`task.mode` 写 `"direct"`；`items` 是 `[]`；`local` 照旧。

### 3. 前端
- `lib/api.ts`：`PackRequest` 加 `mode?: 'useTools' | 'direct'`。
- `lib/solve-view.ts`：`buildPackRequest(query, hits, local = [], direct = false)`：`direct` 为真时 `items: []`、`mode: 'direct'`、`title: \`直接交给 Codex：${query}\``；为假时和现在一模一样（`mode` 不写）。导出常量 `DIRECT_LABEL = '没有合适的？直接交给 Codex'` 和 `BACK_TO_PICK_LABEL = '改回挑工具'`。
- `lib/solve-draft.ts`：`SolveDraft` 加 `direct: boolean`（旧调用没传时按 `false` 处理，别让类型报错）。
- `pages/v2/solve.tsx`：
  - 新状态 `direct`（初值取草稿）。`runSearch`（重新找）、勾选 / 取消勾选 / 设主参考任何一个工具（`setPicked`）时 `setDirect(false)`。
  - `direct` 为真且 `result` 存在时：生成参考包的 effect 用 `buildPackRequest(query, [], pickLocalCapabilities(localCapabilities, [], result.intents), true)`；`packKey` 要把 `direct` 带进去，免得和空选择混在一起；`canCopy = direct ? Boolean(pack?.content) : selectedHits.length > 0 && Boolean(pack?.content)`；`loadingPack` 同理；复制成功提示里「N 个参考项目」在直接模式下写 `直接交给 Codex`。
  - 面板头部：`direct` 时显示「不选工具 · 直接交给 Codex」代替「已选 N 个」，并在「看原文」旁边放文字按钮「改回挑工具」。
  - 面板没勾工具、`result` 存在、不在 `direct` 时：占位文字下面放一个 `Button variant="text"`：「没有合适的？直接交给 Codex」，点击 `setDirect(true)`。**没有 `result`（还没搜）时不显示这个按钮。**
  - 其余布局、配色、间距不改；不新增任何其他入口。

## 不要做
- 不改 `lib.rs`、`命令契约.md`、其他页面、其他 Rust 文件；不加依赖。
- 不要给直接交接单编造工具名或链接；不要在里面写任何具体项目。
- 不要改 `UseTools` 路径的任何输出（现有 70 多个参考包测试一个字都不许改断言，除非是 `PackRequest` 构造处要补 `mode: PackMode::UseTools`）。
- 不要联网，不要读库里的仓库。

## 验收
1. `tools/cards/run.sh rust-test` 全部通过；新增 Rust 测试**至少 14 个**，覆盖：
   - `Direct` + 空 `items` 成功，`item_count == 0`；`Direct` + 非空 `items` 报错；`Direct` + 空白 `problem` 报错；`UseTools` + 空 `items` 仍报原来的错；请求 JSON 里没有 `mode` 时按 `UseTools` 解析；`"mode":"direct"` 能解析。
   - Markdown：含「## 这次要做的事」「## 先这样做」「## 开工前先问我」「## 完成标准」「## 工作约定」，**不含**「## 怎么选」「## 1. 」「（主参考）」；三步编号；`## ` 标题数（无本机能力时 5 个；有本机能力时 6 个）；WORK_RULES 7 条逐字在；`assert_forbidden_phrases` 通过。
   - HINT 按任务换：「批量改图尺寸和压缩」含 `sips`、「给短视频加字幕」含 `ffmpeg`、「把群聊记录整理成日报」含 `pandoc`、认不出的任务含 `sips`。
   - 「开工前先问我」「完成标准」随任务类型换（拿一个视频任务和一个记录整理任务各断言一条）；没有「这几个工具装之前要先问我」那一条。
   - 本机能力非空时有「## 我本机已经有的」，空时没有。
   - AGENTS：含「这次我的收藏里没有找到合适的现成工具」「## 先这样做」「## 工作规则」「## 验收」，不含「## 可用的工具（按顺序）」。
   - JSON：能解析；顶层键 = 现有键 + `directSteps`；`task.mode == "direct"`；`items == []`；`directSteps` 3 条；`askFirst` 5 条、`verify` 3 条。
   - 注入防护：`problem` 里的 `## 伪造标题`、代码围栏、`<!--` 和 `UseTools` 路径一样被中和。
2. `tools/cards/run.sh web-test` 全部通过；`solve-view.test.ts` 新增至少 5 个测试：`buildPackRequest` 直接模式（`items` 为空、`mode == 'direct'`、标题）、非直接模式不带 `mode`、带本机能力、两个常量的文字。
3. `tools/cards/run.sh web-build`、`rust-fmt`、`rust-check` 通过，警告数不比卡开始前多。
4. `git diff --stat` 里只出现「允许改动」里的文件。
5. 报告里写：直接模式下 `solve.tsx` 里每处改动的位置（文件:行）；以及对「批量改图尺寸和压缩」这句，Markdown 直接交接单的全文。

## 停止条件
- 要改 `lib.rs`、契约文档、其他页面或卡外文件；要新依赖；发现 `UseTools` 的现有测试必须改断言才能过——停手并写进报告。
