# 交回报告 · 卡 31-copy-solve（续作）

## 一句话结论
续作 1–5 条已做完并各补了测试，第 6 条按要求只写说明、没动清单；web-build / web-test / rust-fmt / rust-check / rust-test 全部重跑通过。没有启动 App，界面没在窗口里看过（尤其是新的格式列表），有几处我自己拿了主意，见「没做到的 / 偏离」。

## 验收逐条

### 续作「必须做」
| 验收项（照抄续作） | 做到了吗 | 证据（文件:行） |
|---|---|---|
| 1. 「已替你勾了最对口的 N 个」的 N 要等于界面上当前勾着的个数；手动改过勾选后改写成「已勾 N 个」或不显示；没有默认勾选时沿用现有说明；补 AI 精选前 / 后、手动改勾后三种情况的测试 | 是 | N 的算法 `solve-view.ts:324`（`pickListNote`：数 `orderedPicks(命中, 勾选)`，和右边「已选 N 个」同一个数，不再用后端 `preselected.length`），文案 `solve-view.ts:315`（`defaultPickNote`）；页面接线 `solve.tsx:119`（新增 `picksEdited` state，和原有 `picksTouched` ref 对应）、`:181`（新搜索清零）、`:312`（点勾选 / 设主参考置真）、`:346`（顶上一行）。**二选一的结果：写「已勾 N 个」（勾到 0 个也写「已勾 0 个」），不隐藏**：这一行消失会把列表顶上去一截，用户连续点几行勾选时会点歪。没有默认勾选时：顶上不说，右边沿用 `packEmptyNote` 那句「没有特别对口的，所以没替你默认勾选…」（`solve-view.ts:354`、`solve.tsx:348`、`:477`）。测试：`solve-view.test.ts:627` 起 9 个用例（AI 精选前 2 个 → 精选后 1 / 3 / 0 个、精选失败 → 手动加、减、减到 0；直接模式不说；不在命中里的 id 不算；右边那句的几种情况）；页面级 `solve-page.test.tsx:99`、`:113`、`:122`（从草稿恢复的页面里顶上「已勾 N 个」和右边「已选 N 个」同数） |
| 2. 工作约定第 3 条做成无引用版（前端单项复制和 AGENTS.md 用，与 `codex-prompts.ts` 逐字一致）和有引用版（Markdown 交接单和 JSON `rules` 用，两处带例外）；测试改回前端逐字一致；另断言 Markdown / JSON 是有引用版、AGENTS.md 是无引用版；不改 `codex-prompts.ts` | 是 | 无引用版就是改动前原文（「最多 5 个」）`packs.rs:16-24`（`WORK_RULES`）；有引用版 `packs.rs:28`（`WORK_RULE_3_WITH_ASK_FIRST`：「（最多 6 个；「开工前先问我」里点名的除外）」和「装东西、改配置不用等我点头（除了「开工前先问我」里点名要先问的）」）；取用 `packs.rs:50`（`rules_with_ask_first()`，只换第 3 条）。Markdown `packs.rs:497`、直接 Markdown `:536`、JSON `rules` `:763` 用有引用版；AGENTS.md `:669`、直接 AGENTS.md `:705` 仍用 `WORK_RULES`。测试：`packs.rs:2046`（`prompt_constants_match_the_frontend_copy` 改回原样：关于我 + 七条无引用版与前端逐字一致）；`:2063`（`ask_first_reference_only_where_the_section_exists`：两版只差第 3 条、去掉两处例外后等于无引用版、Markdown 两种 / JSON 两种是有引用版并且有「## 开工前先问我」或 `askFirst`、AGENTS.md 两种是无引用版且全文没有「开工前先问我」）；Markdown 快照 `:2481`、直接 Markdown `:4259`、JSON 快照 `:2566` 同步改成有引用版。`codex-prompts.ts` 一个字没动 |
| 3. 格式选项改成竖排单选列表（名字 + 一句说明，选中有底色，键盘可操作），`value` 不变；测试至少渲染出三个选项且没有不换行按钮 | 是 | 新组件 `components/solve/format-picker.tsx`（真单选框 `type="radio"` + `sr-only`，三项同一个 `name`：Tab 进来、方向键换、空格选；整行 `<label>` 可点；选中 `bg-accent` + 圆点；说明文字 `break-words`，会自己换行；没有 `<button>`、没有 `whitespace-nowrap`）。选项数据 `solve-view.ts:9-27`（`FormatOption` 加 `name` / `note`，`label` 仍是完整的「名字（说明）」，`value` 和顺序没变，现有 `label` 断言原样通过）；页面 `solve.tsx:461` 换掉原来的 `Segmented`。测试：`format-picker.test.tsx:39` 起 7 个用例（三个 radio、value 对应、同 name、名字和说明都显示、没有 button / `whitespace-nowrap`、只有选中那项勾上且有底色、onChange 收到对应 value）；页面级 `solve-page.test.tsx:74` |
| 4. 复制成功的提示补齐：`hit-drawer.tsx` 约 65、106 行改成和 `solve.tsx` 一致，抽成共用常量 | 是（第 106 行有一处判断，见下） | 共用常量 `solve-view.ts:33`（`COPY_FOR_CODEX_TOAST`，就是审读第 5 条那句）；`solve.tsx:307`、`hit-drawer.tsx:66` 用它，原来 `solve.tsx` 里的本地常量删了。第 106 行见「没做到的 / 偏离」。测试 `solve-view.test.ts:783` 起（提示原文、两个文件都引用常量且不再有「已复制，去 Codex 里粘贴」、不再各抄一份原文） |
| 5. 残留旧叫法：`solve.tsx` 「方向」→「场景」；`solve-view.ts` 「方向」「项目」→「场景」「工具」、「还没整理」→「待 AI 整理」；`solve-hit-row.tsx` 「待整理」→「待 AI 整理」；「有 N 个项目已不在库里」→「有 N 个工具已不在资料库里」；「约 12,345 字」→「约 1.2 万字」 | 是 | 空状态那句 `solve-view.ts:125`（`emptyResultHint`，`solve.tsx:629` 用）；`crossNote` `solve-view.ts:117`（「场景」「工具」）；`pendingNote` `:121`（「待 AI 整理」）；行标签 `solve-hit-row.tsx:72`（「待 AI 整理」，上一轮写的「还没有中文说明」换掉了）；缺失提示 `solve-view.ts:342`（`missingToolsNote`，文案「有 N 个工具已不在资料库里（可能你取消了收藏）」，`solve.tsx:349`）；字数 `solve-view.ts:334`（`packSizeText`，不到一万写「约 2,345 字」，一万以上写「约 1.2 万字」，`solve.tsx:449` 用）。测试 `solve-view.test.ts:717` 起（含：这五个文件去掉注释后不含「参考包 / 仓库 / 项目 / 方向 / 待整理 / 还没整理 / 理解卡 / 知识库 / 您」等旧叫法）；行标签 `solve-hit-row.test.tsx:40`；两处原有断言 `crossNote`（`:241` 起两处）、`pendingNote`（`:266` 起）换成新文案 |
| 6. 先问清单条数：只在报告里写清怎么出现 7 条、哪两条可以合并，不改清单 | 是（只写说明） | 见下面「续作第 6 条的说明」。`pack_guard.rs` 本轮一行没动；我在 `packs.rs:2136` 加了一个测试把现状「最多 7 条」钉住 |

### 原任务卡验收（上一位做的，我复核并按续作调整过）
| 原卡条号 / 验收项 | 状态 | 证据 |
|---|---|---|
| 第 2 条 参考包 → 交接单 | 是 | 面板标题 `solve.tsx:446`；提示 `:407`；副标题 `:362`；复制按钮 `:354`；加入 / 移除 `hit-drawer.tsx:87`、`solve-hit-row.tsx:48-49`；复制出去的标题 `packs.rs:453`、直接模式 `:506`、AGENTS.md `:635`、`:684`；直接模式那句 `:509`；`buildPackRequest` 前缀 `solve-view.ts:287`（直接模式仍是 `:279` 的「直接交给 Codex：」）；「参考包」「参考项目」仍是禁用词（`packs.rs` `assert_forbidden_phrases`） |
| 第 4 条 why 改人话、不改打分 | 是（上一位做的，本轮没动） | `solve.rs:1611`、`:1666-1812`；12 个新用例 `solve.rs:4406-4630`（每种字段、近义、README 次数、省略号、打分不变）；交接单不叠「对上：」`packs.rs:1360-1363`、测试 `:3490`；行里去掉「为什么相关」`solve-hit-row.tsx:88-90` |
| 第 5 条 找方案复制按钮和提示 | 是 | `solve.tsx:354`、`:307`；见续作第 4 条 |
| 第 16 条 工作约定第 3 条加例外、最多 6 个 | 是，**本轮改成两个版本** | 见续作第 2 条（上一位只把例外挂在「只问你确实缺的信息」前面，且把前端旧文案钉死在测试里） |
| 第 29 条 列表顶部一行、格式文案、主参考说明、三个按钮统一 | 是 | 顶部一行见续作第 1 条；格式见续作第 3 条；主参考旁 `solve-hit-row.tsx:70`；`DIRECT_LABEL` / `DIRECT_FROM_PICKS_LABEL` `solve-view.ts:261-263`（同一句「不用这些工具，直接交给 Codex」），`BACK_TO_PICK_LABEL` 仍是「改回挑工具」 |
| 第 41 条 「未确认是否已装」、术语对照 | 是 | `packs.rs:344`（「还没查你电脑上有没有（Codex 会先查）」）；术语行 `packs.rs:12`，写在四种文档的 ABOUT_ME 下一行，不进 JSON |
| 补充 3 三种格式结构不变 | 是 | Markdown 仍 8 个 `## `、无 `### `；JSON 顶层键集合测试原样通过，`rules` 仍 7 条；AGENTS.md 结构未变 |

## 改了哪些文件
- `apps/desktop/src-tauri/src/packs.rs`：工作约定第 3 条拆成无引用版 / 有引用版，Markdown 和 JSON 用有引用版；测试改回前端逐字一致，新增 2 个测试（上一位的交接单标题、术语行、安装态文案等保留）。
- `apps/desktop/src/lib/solve-view.ts`：列表顶部一行（`defaultPickNote` / `pickListNote`）、`packEmptyNote` 加 `edited`、格式选项加名字和说明、共用提示常量和 `copyPromptToast`、`packSizeText`、`missingToolsNote`、`emptyResultHint`，`crossNote` / `pendingNote` 改成统一用词。
- `apps/desktop/src/lib/solve-view.test.ts`：见上，新增 18 个用例；原有断言只换了期望字符串（`crossNote` 两处、`pendingNote` 一处），一个没删、没放宽。
- `apps/desktop/src/pages/v2/solve.tsx`：`picksEdited`、顶部一行、格式列表换成 `FormatPicker`、字数 / 缺失 / 空状态提示改用上面的函数、窄屏时右边面板最小高度 280 → 460 px（见下）。
- `apps/desktop/src/components/solve/format-picker.tsx`（新）：竖排单选列表。
- `apps/desktop/src/components/solve/format-picker.test.tsx`、`solve-hit-row.test.tsx`、`solve-page.test.tsx`（新）：渲染测试（`react-dom/server`，不加依赖）。
- `apps/desktop/src/components/solve/hit-drawer.tsx`、`solve-hit-row.tsx`：复制提示、行标签。
- `apps/desktop/src-tauri/src/solve.rs`、`pack_guard.rs`：上一位的改动，本轮没碰。
- `git status` 里只有以上路径，没有卡外文件。

## 验证
- `tools/cards/run.sh rust-fmt-fix` → 退出码 0（Rust 改完跑过一次）；`tools/cards/run.sh rust-fmt` → 退出码 0。
- `tools/cards/run.sh rust-check` → 通过，6 条警告，全在没改的文件里（`db/migrations.rs`、`scenarios.rs`、`spend_guard.rs`、`v2_ops.rs` ×3），和上一轮一样，警告数没增。
- `tools/cards/run.sh rust-test` → `1150 passed; 0 failed; 5 ignored; 1 filtered out`（上一轮 1148，本轮 +2；filtered 的是沙盒里必然失败的钥匙串用例）。测试配置的警告只有已有的 `scenarios.rs:115`。
- `tools/cards/run.sh web-test` → 19 个文件、411 个测试通过（上一轮 16 个文件 376 个，本轮 +35）。
- `tools/cards/run.sh web-build` → 退出码 0（`tsc --noEmit` 和 vite 构建都过；新用的 Tailwind 类我在构建出的 CSS 里搜过，都生成了）。
- 让测试变红的检查（改坏实现、跑、再还原）：① 让 `defaultPickNote` 不看 `edited`、`pickListNote` 数 `picked.length` → 3 个前端用例变红；② 把 `picksEdited` 的初值改成 `false` → 2 个页面用例变红；③ 让 AGENTS.md 渲染改用有引用版 → `ask_first_reference_only_where_the_section_exists` 变红。三处都已还原，最后一轮五项检查是还原后跑的。
- `git diff --stat` → 8 个已有文件（都在允许路径里）+ 4 个新文件（都在 `components/solve/` 下，续作允许）。没有启动 App，没动红线里的任何东西。
- 两处小的越规，如实写：① 做「改坏再还原」检查时，前两次把备份文件 `cp` 到了会话临时目录、有一次把构建日志用重定向写到了那里（不在工作区），都已删除，之后改用工作区里的 `.tmp-card/`（也已删）；② 有一次用 `python3` 对工作区里的一个文件做文本替换（不联网、没有写别处）。

## 没做到的 / 偏离任务卡的地方
- **看不到界面**：红线不让启动 App，所以格式列表的实际样子、右边面板的高度只靠读代码和渲染测试核对，没在窗口里看过。需要总控合并后在 dev 版里点一遍。
- **窄屏（宽度 < 900 px）右边面板最小高度 280 → 460 px**（`solve.tsx:401`）：格式列表占 100 多 px，不加高的话 280 px 放不下「标题 + 格式 + 预览（最小 180）+ 底部复制按钮」，复制按钮会被裁掉。这是布局改动，卡里说不改布局，但新列表本身就是续作要求的布局改动；觉得不妥可以只回退这一个数字。宽屏没改。
- **续作第 4 条第 106 行没有照字面全换**：`onCopyPrompt` 这个回调在详情里同时接两段话——「复制一段话，粘给 Codex（帮我装好）」和「让 Codex 去查别人怎么用」（`detail-panels.tsx:142`、`:293`）。后者只读资料、不装东西，如果也提示「它会先查你电脑上有没有，没有就装进独立文件夹…」，是说错了（审读第 5 条对「只评估」的那一类写的就是「它只会读资料、给你结论，不会安装」）。所以我加了 `copyPromptToast(text)`（`solve-view.ts:43`）：认得出是「去查别人怎么用」那段话（含「请用 web-search 调查」，测试里用真正的 `buildCommunityPrompt` / `buildInstallPrompt` 对过）才用 `COPY_FOR_RESEARCH_TOAST`（「已复制。粘到 Codex 后，它只会读资料、给你结论，不会安装。」，用的是审读第 5 条的原话）；**认不出的一律按会装东西的那句说**，宁可多提醒。第 65 行（单个工具的复制）和装东西那段话用共用常量，和续作一致。想严格照字面：把 `hit-drawer.tsx:107` 的 `copyPromptToast(text)` 换成 `COPY_FOR_CODEX_TOAST` 即可。资料库 / 今日页里同样的回调（卡 32 的范围，不在我的允许路径里）有同样的问题，我没碰，建议总控转告卡 32。
- **多做了一小处（右边「没替你默认勾选」那句）**：AI 精选说「都不合适」、把后端默认勾的清空之后，原来的 `packEmptyNote` 只看后端 `preselected` 是不是空的，会什么都不说。续作第 1 条点名了「AI 精选后可能 0 个」，所以给它加了第二个参数 `edited`（`solve-view.ts:354`，`solve.tsx:348`）：用户没动过勾选、应用没勾时也说那一句；用户自己取消的不说；不传参数时行为和原来完全一样（原有 4 个用例没动）。
- **从草稿恢复的页面**（切到别的页再切回来）把勾选当作「改过」（原来的 `picksTouched` 就是这么处理的），所以这时顶上写「已勾 N 个」，不会说成「替你勾了」。
- **工作约定第 3 条里「只有这几种情况先停下来问我：要管理员权限、…」没动**。续作只点名了两处例外，我照做。这句仍是「只有这几种」，而「开工前先问我」里还有「内容外发 / 往真实账号发内容 / 不支持 macOS」几类，严格读的话还有一点出入，要不要也加例外由总控定。
- **JSON 里没有叫「开工前先问我」的一节**，对应的键是 `askFirst`（键名不能改）。按续作 JSON 的 `rules` 用了有引用版，文字里仍写「开工前先问我」；Codex 读 JSON 时要自己对上 `askFirst`。想让它更直白，可以改成「`askFirst`（开工前先问我）」，我没改。
- **`docs/rebuild/命令契约.md` §9**「`rules` … 与前端 `WORK_RULES` 逐字相同」现在对 JSON 的第 3 条不成立了（Markdown 和 JSON 是有引用版）。文档不在我的允许路径里，请总控改一句「第 3 条除外」。
- 没动、也不属于这张卡的旧叫法：交接单里的小标题「装法」、类型名（技能（Skill）、命令行工具…）是审读第 34 条（现有测试按「装法」逐字断言）；`rerankStatusText` 里的「本次约 4.2 千 token」是卡 30 的文案。`packs.rs` / `solve.rs` 注释里的「仓库」按续作「注释不动」没改。

## 需要总控或 Fox 决定的事
- 续作第 6 条：清单要不要收到 6 条（方案见下）。
- 上面「偏离」里的三处：窄屏面板高度、第 106 行的两种提示、工作约定第 3 条里「只有这几种情况」要不要也加例外。
- 命令契约 §9 的 `rules` 一行要改一句。
- 卡 32 的资料库 / 今日页有和抽屉一样的「同一个回调接了会装 / 只读两种话」的问题。

## 续作第 6 条的说明（只写说明，没改清单）
**7 条是怎么出现的**（`pack_guard.rs:1082` 的 `ask_items`）：
1. 先取任务类型对应的 5 条（`ASK_*` 每类固定 5 条）。
2. 任务里认出第二类、而且两类能混（`may_mix`，`pack_guard.rs:1064`：主类是图文 / 写作 / 记录整理 / 视频 / 录音转写，第二类是这五类或批量处理）时，把第二类的第一条插在第 2 位（`:1092`）→ 6 条。
3. 选中的工具里有要先问的事（要管理员权限、要 API Key 或付费、要登录、内容外发、往真实账号发内容、不支持 macOS 之一）时，最后再加 1 条「这几个工具装之前要先问我：…」（`:1105`）→ 6 或 7 条。
两个条件同时满足就是 7 条。例：任务「公众号文章配图」（图文 + 写作）、选中的一个工具要管理员权限 → 5 + 1 + 1。现有测试 `pack_guard.rs:1771` 分别证明了「混类 = 6」「列工具 = 6」；我在 `packs.rs:2136` 加了叠加后的用例（JSON `askFirst` 长度 7，且 `rules` 第 3 条带着「点名的除外」）。抓取 / 转格式 / 自动发布 / 分析 / 去水印 / 批量处理自己不混别的，最多 6 条。命令契约 §9 本来就写「5 到 7 条」。

**哪两条可以合并**：最合适的是**第 1 条和第 2 条**，也就是混类时「主类的第一条」和「插进来的第二类的第一条」。每一类的第一条问的都是同一件事：要做 / 要处理的具体内容（主题、文案原文、素材在哪、有几个、什么格式……），混类任务里这两条经常在问同一批东西（比如图文的「标题和文案原文、品牌名、素材文件在哪」和写作的「主题、读者、平台、字数」）。合并后上限是 5 + 1 = 6，和「最多 6 个」一致。代码改动很小：把 `items.insert(1, extra)` 改成把 `extra` 接在 `items[0]` 后面（用「；另外：」连起来）。
备选：把第 7 条（列工具那条）并进各类里「能不能用要付费 / 要 API Key 的工具（预算多少）」那条。不推荐：这条在各类里位置和措辞不统一（视频、录音转写在第 3 条，其余多数在第 5 条，记录整理另有第 2 条问「能不能发给第三方」），要逐类改。
另外：我写的有引用版已经把「点名的除外」写进去了，所以**现在不合并也不矛盾**。若以后合并成 6 条，可以把有引用版里「（最多 6 个；「开工前先问我」里点名的除外）」改回「（最多 6 个）」，装东西那处的例外仍要留。

## 给审核者的提示
- 前端测试里有几个「读源码」的用例（`solve-view.test.ts` 的「找方案页的接线」「界面源码（注释除外）里不再出现旧叫法」「找方案页和详情抽屉都用这个常量」）：用 Vite 的 `?raw` 读进来，去掉注释后查旧叫法和接线，不用 node 的类型（这个工程的 TypeScript 6 默认不带 `@types/node`，用 `node:fs` 会过不了 `tsc`）。去注释是粗糙的正则，只会漏查、不会误报。它们会拦住以后有人在这几个文件里又写回「方向 / 项目 / 参考包」。
- `solve-page.test.tsx` 是整页用 `react-dom/server` 渲染一次，靠 `saveSolveDraft` 让页面从草稿恢复出结果和勾选。服务端渲染不跑副作用，所以「实时搜索 → AI 精选改勾 → 手动改勾」这条链只在函数层测了（`pickListNote` + `initialPicks` / `rerankPicks` / `togglePick`），页面层测的是「从草稿恢复」这一种状态。页面里 `picksTouched`（ref，给异步回调读）和 `picksEdited`（state，给渲染用）是两份，同步点只有三处：初始值、`runSearch`、`updatePicked`。
- `FormatPicker` 的单选框用 `peer-focus-visible` 画键盘焦点圈（构建出的 CSS 里有）；选中状态由 props 决定，不靠 `peer-checked`。
- 无引用版 / 有引用版：`WORK_RULES`（无引用版）和 `codex-prompts.ts` 又逐字一致了；上一轮那条「钉死前端必须还是旧文案」的断言已删（那条断言本来就会在卡 32 同步前端时变红）。
- 「已勾 N 个」和右边「已选 N 个」是同一个数的两个叫法，是续作给的字；如果想统一成「已选」，改 `solve-view.ts:315` 一处。
