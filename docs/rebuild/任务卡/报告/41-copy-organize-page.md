# 交回报告 · 卡 41-copy-organize-page

## 一句话结论

做完了：按任务卡统一「现在整理」「现有收藏」文案，补上收藏分项行的源码守卫；前端构建通过，34 个测试文件、801 项测试全部通过。

## 验收逐条

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| 1. `tools/cards/run.sh web-build`、`web-test` 通过。 | 是 | 两条命令退出码均为 0；构建完成类型检查及 Vite 打包（2187 个模块）；测试为 `Test Files 34 passed (34)`、`Tests 801 passed (801)`。 |
| 2. `git diff --stat` 只出现「允许改动」里的文件（含新增的测试用例所在文件）。 | 是 | 仅 `apps/desktop/src/lib/automation-view.ts`、`apps/desktop/src/lib/automation-view.test.ts`、`apps/desktop/src/pages/v2/toolbox.test.tsx`，3 files changed, 38 insertions(+), 29 deletions(-)。 |
| 3. 报告写：条号 → 文件:行；改了哪些既有测试的期望（逐条点名）；第 8 条搜索的输出；没把握的地方。 | 是 | 本报告下面分别列出要做的事 1–8、既有测试期望修改、完整搜索输出和审核提示。 |

### 要做的事：条号 → 文件:行

| 条号 | 文件:行 | 完成内容与逐字断言 |
|---|---|---|
| 1 | `apps/desktop/src/lib/automation-view.ts:384` | 状态徽章为 `现在整理中`，detail 保持原文；`apps/desktop/src/lib/automation-view.test.ts:662`、`:716` 逐字断言标签。 |
| 2 | `apps/desktop/src/lib/automation-view.ts:327` | running 说明改为指定的「应用在后台一个个整理现有收藏……」；`apps/desktop/src/lib/automation-view.test.ts:448` 在完整面板对象断言中逐字核对，原有标题、进度和暂停字段断言保留。 |
| 3 | `apps/desktop/src/lib/automation-view.ts:360` | 第一步按钮为 `现在整理全部 ${pending} 个`，之后的按钮原样保留；`apps/desktop/src/lib/automation-view.test.ts:539` 对四组第一步夹具逐字断言 `现在整理全部 12 个`，`:514`、`:556` 保留后续按钮的逐字断言。 |
| 4 | `apps/desktop/src/lib/automation-view.ts:239`、`:240`、`:243`、`:246`、`:249`、`:252` | 五种 reason 前缀统一为 `上次整理现有收藏：`；completed 失败重试、ceiling 继续提示改为「再点一次「现在整理」」。数字和时间处理未改；`apps/desktop/src/lib/automation-view.test.ts:604`、`:607`、`:610`、`:613`、`:619`、`:622`、`:629`、`:637` 逐字核对对应完整结果。 |
| 5 | `apps/desktop/src/lib/automation-view.ts:582` | 小字为 `这是之前整理现有收藏用的，不占每天额度`，出现条件未改；`apps/desktop/src/lib/automation-view.test.ts:1030`、`:1032`、`:1066`、`:1070` 对横幅和指标格、有无整理个数的四种输出逐字断言。 |
| 6 | `apps/desktop/src/pages/v2/toolbox.test.tsx:43` | 新增源码守卫：断言 `{countsLine ? <p` 存在，正则核对该段落实际渲染 `{countsLine}`，并断言状态说明 → 收藏分项行 → FullScanPanelView 的顺序；同时检查顺序两端存在，避免缺失节点的 indexOf(-1) 误通过。现有页面行位于 `apps/desktop/src/pages/v2/toolbox.tsx:487`，无须改页面。 |
| 7 | `apps/desktop/src/lib/automation-view.test.ts:425` 等，见下表 | 更新 12 个既有测试声明中的 19 处期望字符串；断言类型、输入夹具、负向断言均未放宽或删除，未跳过用例。新增文案均已有逐字断言。 |
| 8 | 整个 `apps/desktop/src` | 实际运行 `rg -n '一次性' apps/desktop/src`，剩余 36 行：18 行注释、14 行测试名称、4 行负向断言；没有用户可见的旧词字符串。完整输出见下文。 |

### 改了哪些既有测试的期望（逐条点名）

以下均在 `apps/desktop/src/lib/automation-view.test.ts`；测试名称保留原样，只有期望字符串变化。

| 测试组 / 测试名 | 期望行 | 改动 |
|---|---|---|
| fullScanPanel / 没有要整理的是 nothing，有上次结果就带上一行 | 425 | 完成记录前缀改为「上次整理现有收藏：」。 |
| fullScanPanel / 有要整理的是 ready：388 个约 250 万 token | 434 | 停止记录前缀改为「上次整理现有收藏：」。 |
| fullScanPanel / 「现在整理」进行中是 running，带上进度、花费和暂停原因 | 448 | running description 改为指定的后台整理现有收藏说明。 |
| fullScanPanel / 额度用完但有重新排队的收藏，仍可手动整理；没有待整理才显示完成 | 517 | 停止记录前缀改为「上次整理现有收藏：」。 |
| fullScanPanel / 分项中新版说明为 0，或旧后台没有上次记录，显示第一步：%j | 539 | 四组夹具的按钮期望改为「现在整理全部 12 个」。 |
| lastFullScanText / 完成、停止、到时间、花费超了，四句各不相同 | 604、607、610、613 | 四个前缀改为「上次整理现有收藏：」；ceiling 尾句改为「要继续可以再点一次「现在整理」。」。 |
| lastFullScanText / 有失败的，整理好的个数要扣掉失败，并单独说一句；没有失败字段（旧后台）和以前完全一样 | 619、622、629 | 三个前缀改为「上次整理现有收藏：」；有失败和全部失败的 completed 重试提示改为「（可以再点一次「现在整理」重试）」。 |
| lastFullScanText / 失败太多被自动停下：说清原因、让人先检查模型 | 637 | failures 前缀改为「上次整理现有收藏：」，模型检查提示保持原文。 |
| 一次性全部整理期间不说额度用完 / 顶部状态是「一次性全部整理中」，不提额度用完 | 662 | 完整状态对象中的 label 改为「现在整理中」。 |
| 一次性全部整理期间不说额度用完 / 队列是 0、额度也是 0 时，只要 unlimited，详情和顶上提示都不说额度用完 | 716 | label 逐字期望改为「现在整理中」。 |
| recentUsageLine / recentUsageMetric / 关了每天额度、上一轮一次性整理刚结束：标明这是之前一次性整理用的 | 1030、1032 | 有无真实整理个数的完整横幅期望均换用「这是之前整理现有收藏用的，不占每天额度」。 |
| recentUsageLine / recentUsageMetric / 工具箱那一格：关了额度又刚跑完一次性整理时，下面一行说清这笔花费的来历 | 1066、1070 | 有无真实整理个数的完整指标对象均换用新小字。 |

### 第 8 条搜索的输出

命令：`rg -n '一次性' apps/desktop/src`，退出码 0，命中 36 行。以下为实际输出；输出顺序由 rg 决定。

```text
apps/desktop/src/pages/v2/toolbox.test.tsx:75:    expect(SOURCE).not.toContain('一次性全部整理</');
apps/desktop/src/pages/v2/toolbox.tsx:302:  /** 设置额度、开始 / 停止一次性全部整理后拿到的状态里没有「最近 24 小时整理了几个」：沿用上一次读到的（这几个动作不会改它）。 */
apps/desktop/src/pages/v2/toolbox.tsx:420:  // 一次性全部整理进行中：页面开着时每 5 秒刷新一次进度（整理完 / 被停下后 unlimited 变回 false，轮询自己停）
apps/desktop/src/lib/item-format.ts:391: * 自动整理没开（服务商配好了、没暂停、也不是一次性全部整理，只是开关关着）时，说清「不会重写」，并保留「去开启」；
apps/desktop/src/lib/automation-view.test.ts:650:describe('一次性全部整理期间不说额度用完', () => {
apps/desktop/src/lib/automation-view.test.ts:659:  it('顶部状态是「一次性全部整理中」，不提额度用完', () => {
apps/desktop/src/lib/automation-view.test.ts:669:  it('今日 / 资料库提示说还在一次性全部整理，按钮是「看整理进度」，个数用传入的 pending', () => {
apps/desktop/src/lib/automation-view.test.ts:720:  it('重新整理的提示说会在一次性全部整理里重做，不说额度用完', () => {
apps/desktop/src/lib/automation-view.test.ts:741:  it('不是一次性全部整理时，提示去整理，并说明可以现在整理', () => {
apps/desktop/src/lib/automation-view.test.ts:970:describe('spentByEarlierFullScan：「这是之前一次性整理用的」只在有证据时才说', () => {
apps/desktop/src/lib/automation-view.test.ts:975:  it('每天额度是 0、没有一次性整理在跑、上一轮在 24 小时内结束：说', () => {
apps/desktop/src/lib/automation-view.test.ts:980:  it('暂停但没有一次性整理（暂停时 enabled 也是 false）：不说', () => {
apps/desktop/src/lib/automation-view.test.ts:1022:  it('关了每天额度、上一轮一次性整理刚结束：标明这是之前一次性整理用的', () => {
apps/desktop/src/lib/automation-view.test.ts:1036:  it('暂停了、没有一次性整理：不能出现「之前一次性整理用的」', () => {
apps/desktop/src/lib/automation-view.test.ts:1040:    expect(recentUsageLine(pausedNoBudget, now)).not.toContain('一次性');
apps/desktop/src/lib/automation-view.test.ts:1041:    expect(recentUsageLine({ ...pausedNoBudget, succeeded24h: 5 }, now)).not.toContain('一次性');
apps/desktop/src/lib/automation-view.test.ts:1042:    expect(recentUsageMetric(pausedNoBudget, now).sub).not.toContain('一次性');
apps/desktop/src/lib/automation-view.test.ts:1045:  it('没花过就不提一次性整理', () => {
apps/desktop/src/lib/automation-view.test.ts:1064:  it('工具箱那一格：关了额度又刚跑完一次性整理时，下面一行说清这笔花费的来历', () => {
apps/desktop/src/lib/automation-view.test.ts:1080:describe('一次性全部整理的横幅：「已整理 N 个」读后台给的开始个数和剩余个数', () => {
apps/desktop/src/lib/automation-view.ts:2: * 「自动整理与备份」「设置」「顶栏」「今日 / 资料库横幅」里用到的纯函数：额度档位、一次性全部整理、状态文案、
apps/desktop/src/lib/automation-view.ts:8:/** 正在进行的一轮一次性全部整理。后台还没带上这个字段时，整个值是 null。 */
apps/desktop/src/lib/automation-view.ts:24:/** 上一轮一次性全部整理的结果。reason 认不出就当没有。 */
apps/desktop/src/lib/automation-view.ts:51:   * 一次性全部整理正在进行。字段缺失（旧后台）当作 false：用 `=== true` 判断，不要把缺失当成开着。
apps/desktop/src/lib/automation-view.ts:95:/** 一次性全部整理的估算：2026-10-04 真实记录里成功的卡平均约 5.7–6.3 千 token（含思考），偶尔被截断要重试，取 6500（略高于平均）。只用于这一轮的预计花费，不用于每天额度折算。 */
apps/desktop/src/lib/automation-view.ts:147:/** 一次性全部整理预计要多少 token。个数不是有限数或 ≤ 0 时不估。 */
apps/desktop/src/lib/automation-view.ts:219:/** 这一轮一次性全部整理已经整理好几个（处理过的减去失败的）。没有这一轮、或开始个数不是正数，就是 null。 */
apps/desktop/src/lib/automation-view.ts:227:/** 「自动整理与备份」里「上次一次性全部整理」那一行。没有、或 reason 认不出，就什么都不写。 */
apps/desktop/src/lib/automation-view.ts:314: * 「一次性全部整理」面板该画哪一种。顺序不能换：
apps/desktop/src/lib/automation-view.ts:461:  // 走到这里一定不是一次性全部整理（上面已经返回了），所以「额度用完」只指每天额度
apps/desktop/src/lib/automation-view.ts:492:  // 一次性全部整理时不要说「额度用完」。每天自动整理（unlimited 不是 true）仍用下面原来的「已经在整理队列里了」，原有测试靠这句。
apps/desktop/src/lib/automation-view.ts:561:/** 一次性全部整理的确认框，固定三句。estimate 是已经取整的 token 文字，例如「250 万」。 */
apps/desktop/src/lib/automation-view.ts:600: * 最近 24 小时的花费能不能说成「这是之前一次性整理用的」。只在有证据时才说：
apps/desktop/src/lib/automation-view.ts:601: * 现在没有一次性整理在跑、没开每天额度，而且上一轮一次性整理在最近 24 小时内结束过。
apps/desktop/src/lib/automation-view.ts:602: * 不能看 `enabled`：暂停时它也是 false，但暂停不代表用过一次性整理。
apps/desktop/src/lib/item-format.test.ts:616:  it('没配置服务 / 暂停 / 一次性全部整理，开关关着也各说各的，不套用「没开」那句', () => {
```

保留项分类：

- 注释：`apps/desktop/src/lib/automation-view.ts:2,8,24,51,95,147,219,227,314,461,492,561,600,601,602`；`apps/desktop/src/pages/v2/toolbox.tsx:302,420`；`apps/desktop/src/lib/item-format.ts:391`。
- 测试名称：`apps/desktop/src/lib/automation-view.test.ts:650,659,669,720,741,970,975,980,1022,1036,1045,1064,1080`；`apps/desktop/src/lib/item-format.test.ts:616`。
- 禁止旧词出现的负向断言：`apps/desktop/src/lib/automation-view.test.ts:1040,1041,1042`；`apps/desktop/src/pages/v2/toolbox.test.tsx:75`。它们是测试检查条件，不会显示给用户，按要求原样保留。

## 改了哪些文件

- `apps/desktop/src/lib/automation-view.ts`：只改任务卡指定的字符串，未改额度、数字格式、时间格式或逻辑判断。
- `apps/desktop/src/lib/automation-view.test.ts`：替换既有逐字期望字符串，保留断言、夹具和测试名。
- `apps/desktop/src/pages/v2/toolbox.test.tsx`：新增收藏分项行内容及顺序守卫。
- `.card-report.md`：按命令契约 §6 写交回报告（已被 git 忽略）。

## 验证

- 起手 `git status --short`、`git diff --stat` → 无输出，工作区无上一位开发者留下的改动。
- `tools/cards/run.sh web-build` → 退出码 0；共享包构建、前端类型检查、Vite 打包通过，2187 个模块，Vite 显示 `built in 762ms`。
- `tools/cards/run.sh web-test` → 退出码 0；34 个文件、801 项测试通过，0 失败，时长 1.17 秒。
- `git diff --check` → 退出码 0，无空白错误。
- `git diff --stat` → 仅允许的 3 个文件：automation-view.test.ts 38 行变化、automation-view.ts 20 行变化、toolbox.test.tsx 新增 9 行；合计 38 行新增、29 行删除。
- `git diff -- apps/desktop/src/lib/automation-view.ts apps/desktop/src/lib/automation-view.test.ts apps/desktop/src/pages/v2/toolbox.test.tsx` → 逐项检查：生产代码仅指定文案替换，既有测试仅期望字符串替换，页面测试增加 1 个用例。
- `rg -n '一次性' apps/desktop/src` → 36 行，完整输出及分类见上文。
- `git status --short` → 仅上述 3 个允许文件为修改状态，没有新增依赖或卡外代码文件。
- 报告核对命令 `python3 - <<'PY' …` → 退出码 1；zsh 创建 here-document 临时文件时被沙盒拒绝，Python 未执行，未改动代码或报告。
- 改用不需要临时文件的 `python3 -c …`（只读核对报告、rg 输出和 git diff --name-only）→ 退出码 0；搜索命中 36 行、报告遗漏 0 行、越界文件 0 个、要求的报告栏目遗漏 0 个。

## 没做到的 / 偏离任务卡的地方

- 无任务条目遗漏、无越界修改。
- 按红线没有启动 App，因此没有桌面实机截图或交互验收；本卡的源码检查、前端构建及全量前端测试均已完成。
- 未改 Rust，因此未运行 Rust 格式、构建或测试；任务卡明确仅改前端。

## 需要总控或 Fox 决定的事

无。

## 给审核者的提示

- 没有实现层面的未决判断。任务卡指定的新文案均逐字替换，已有逻辑和其它文案保持原样。
- 页面守卫沿用 `?raw` 源码检查，是结构与顺序检查；实际桌面显示由总控合并后点测。
- `apps/desktop/src/pages/v2/toolbox.tsx` 已有正确的收藏分项行，`apps/desktop/src/lib/item-format.test.ts` 的相关期望已使用「现在整理」，两者没有必要改动。
- 新小字常量同时被 `recentUsageLine` 和 `recentUsageMetric` 使用；两处既有测试期望都已更新。
- 为不删除或放宽原断言，4 处包含「一次性」的负向测试检查保留；其它剩余旧词只在注释和测试名中。

## 总控审核（2026-10-05，Claude）

- **执行者**：Codex（`gpt-6.1-sol`，xhigh），14:27:16 派出、14:34 交回（约 7 分钟，16 条命令）。
- **范围**：改动 3 个文件，都在任务卡「允许改动」那一行里（`automation-view.ts`、`automation-view.test.ts`、`toolbox.test.tsx`）。`check-scope.sh` 这次读出「0 条规则」：**是我的疏漏**——这张卡我只在正文里写了允许改动的文件名，没写模板要求的文件头 `允许改动:` 清单，脚本读不到。我改为人工逐个核对，没有越界；下次的卡从模板写起。
- **日志审计**：16 条命令全是读文件、`git status / diff`、`rg` 搜索、`tools/cards/run.sh web-test / web-build` 和 `python3 -c` 只读核对；没有碰数据目录、钥匙串，没有 git 写命令，没有联网。报告里写的「here-document 建临时文件被沙盒拒绝」是沙盒正常现象。
- **diff**：三个文件逐行读过，和任务卡 8 条一一对上——徽章「现在整理中」、整理中说明、第一步按钮「现在整理全部 N 个」（之后仍是「现在整理 N 个」）、五种「上次整理现有收藏：」前缀 + 两处「再点一次「现在整理」」、24 小时用量小字「…不占每天额度」、页面守卫。既有测试只换了期望字符串（19 处），没有删、没有放宽、没有跳过；断言形状（`toEqual` 整对象）都在。
- **「改坏实现」检查**（我自己在草稿目录里写的脚本，不进仓库）：18 种改法**全部被测试抓住**——徽章 / 整理中说明 / 第一步按钮分别改回旧名字（C41-01/02/03），「之后」的按钮也写成「全部」（C41-04），五种前缀各自改回旧名字（C41-05～09），重试提示和花费超了的结尾改回旧句子（C41-10/11），24 小时用量小字改回旧句子或去掉「不占每天额度」（C41-12/13），把重试提示加到 stopped 上（C41-14），以及页面守卫的四种：删掉分项行、分项行改画别的、挪到整理面板后面、挪到状态说明前面（TB01～04）。卡 40 里活下来的 TB06（整行删掉分项行没人发现）这次被守住了。
- **用户看得见的旧词**：前端 `rg 一次性` 剩 36 行，18 行注释、14 行测试名、4 行「不许出现旧词」的反向断言，没有用户可见的字符串。Rust 里剩下的三处提示由卡 39 的第二轮处理。
- **没做到 / 遗留**：实际界面（滚动、焦点、折叠、新文案）红线不让 Codex 启动 App，没点过；我在发布后用真数据复验。
- **合并后要做的**：和卡 39、卡 40 一起发布（卡 41 的小字「不占每天额度」依赖卡 39 的记账口径，单发会说错话）。
