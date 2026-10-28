# 交回报告 · 卡 49c-solve-stale-hint

## 一句话结论

完成：找方案主列表及后备候选带上数据库维护事实，结果行复用维护标签，Markdown / AGENTS.md / JSON 交接单保留准确提醒；排序、勾选和 JSON schema 不变，前端全套及相关后端测试通过，未启动 App。

## 验收逐条

| 验收项（照抄任务卡） | 做到了吗（是/部分/否） | 证据（命令输出摘要，或 文件:行） |
|---|---|---|
| SolveHit 加 pushedAt 与 archived（保留可缺省/未知兼容），全部命中及后备候选保持字段；读取已有 repositories 列，不加迁移、不改同步。 | 是 | solve.rs:430、776；contracts.ts:155。Rust 使用 Option，前端字段可缺省或 null；hits / moreHits 共用构造路径。solve.rs:5764 测试覆盖主/后备及无卡项目。 |
| 找方案行复用维护规则：已归档优先、放缓/疑似停更按已有边界、未知不误报。提醒不改变排序、选择、筛选，不把旧工具自动排除。 | 是 | solve-hit-row.tsx:75 直接复用 maintenanceOf 与 MaintenanceTag；solve-hit-row.test.tsx:42 覆盖 90/365 天整点与相差一秒、未知、未来、归档优先及按钮未禁用。solve.rs:5764 比较维护字段变化前后的完整结果（仅剔除两个新字段），排名、勾选等完全一致。 |
| 交接单三种格式保留维护提醒：沿用现有 cautions/risks 等兼容字段，优先不新增 JSON schema 键；从 DB 事实生成，不相信前端传入。归档优先、不制造更新/安全结论。 | 是 | packs.rs:410 从数据库读取推送时间、归档；506 按现有边界计算。342/350 将维护提醒追加到现有 cautions，三种格式及单项复制均保留。没有新增 JSON 键，维护提示不加入 askFirst 权限门禁。packs.rs:1999 测试前端伪造字段与生成日不会改变判断。 |
| 覆盖活跃/放缓/停更/已归档/未知、近期仍归档、边界时间、moreHits、无理解卡、JSON/Markdown/AGENTS 输出；其他排名与选择不退步。 | 是 | 新增 Rust 2 个、前端 2 个测试。前端全套 900 项通过；solve::tests 124 项通过；packs::tests 76 项通过，包含既有快照/schema/选择回归。solve-view.test.ts:380 验证后备提升、精选排序和已选工具保留维护字段。 |
| 不修改 solve.tsx/App.tsx，无新增依赖、不联网、不真实 AI。 | 是 | 实际仅下述 6 个卡内源/测试文件及本报告；没有迁移、依赖、同步、AI 调用或 App 启动操作。 |

## 改了哪些文件

- apps/desktop/src-tauri/src/solve.rs：读取并输出 pushedAt / archived；扩充契约测试、验证维护事实不改变结果。
- apps/desktop/src-tauri/src/packs.rs：基于后端真实当前时刻和数据库维护事实生成提醒，沿用现有 cautions；补充可控时钟和三格式边界测试。
- apps/desktop/src/lib/contracts.ts：SolveHit 两个兼容可选字段。
- apps/desktop/src/components/solve/solve-hit-row.tsx：复用现有维护规则与标签。
- apps/desktop/src/components/solve/solve-hit-row.test.tsx：边界、归档优先、未知和旧草稿、勾选可用性测试。
- apps/desktop/src/lib/solve-view.test.ts：验证后备候选提升、精选与勾选保留维护事实。

## 验证

- `tools/cards/run.sh rust-fmt-fix` → 通过，只整理卡内 Rust 改动。
- `tools/cards/run.sh web-test solve` → 入口实际运行了前端全套：41 文件、900 项通过。
- `tools/cards/run.sh web-build` → TypeScript 类型检查、2191 模块构建通过。
- `CARGO_NET_OFFLINE=true tools/cards/run.sh rust-check` → 通过；仅既有 dead_code 警告。
- 首次 `CARGO_NET_OFFLINE=true tools/cards/run.sh rust-test solve::tests` → 编译完成，但链接失败：离线 ort-sys 没有定位 ONNX Runtime 的本地静态库。
- 总控提供现有缓存后核验 `libonnxruntime.a` 存在，以进程环境 `ORT_LIB_PATH=/Users/fox/Library/Caches/ort.pyke.io/dfbin/aarch64-apple-darwin/612739f75438dc0a075461e1fb454226b4a1eb175e60a7271ba966bbbb972cd4`、`CARGO_NET_OFFLINE=true`、取消 `ORT_OFFLINE` 后重跑 `tools/cards/run.sh rust-test solve::tests` → 124 通过、0 失败。
- 同一离线环境 `tools/cards/run.sh rust-test packs::tests` → 76 通过、0 失败。
- `tools/cards/run.sh rust-fmt`、`git diff --check` → 通过。
- 依赖处理：未安装或下载新依赖；只复用总控已准备的项目依赖与本地 ONNX Runtime 静态库。没有修改系统环境或配置。

## 没做到的 / 偏离任务卡的地方

- 未启动正式/开发 App，未接触真实库或凭据；因此没有亲手交互验收或截图。界面证据是组件静态渲染测试、前端类型检查与构建。
- 未重复跑完整 Rust 套件；已运行改动相关的 solve 与 packs 全组，共 200 项。全仓集成验收由总控执行。
- 现有 Rust 维护逻辑在 library_view.rs 私有 SQL 宏中；经总控确认，本批不扩范围抽公共模块。packs.rs 使用相同秒级 90/365 天边界与相同 SQLite 日期解析，并使用同一边界表验证；前端直接复用现成函数/组件。

## 需要总控或 Fox 决定的事

无。待总控只读审查、集成与本批验收；未推进 52。

## 给审核者的提示

- `generatedOn` 仍是展示用日期，允许前端传入；维护状态只使用后端 `now_utc()`。`build_pack_at` 是私有函数，仅供确定性测试传时间，不是前端接口。
- 归档优先于推送时间；无有效时间且未归档不警告，未来时间按已有规则视为活跃。活跃不增加维护警告。
- 维护提醒追加到现有 cautions 的普通提醒部分，不抬升为要求用户确认的权限门禁，也不改变主参考与预选规则。
- 修改了通用理解卡测试夹具的推送日期到固定测试时刻前一天，以继续单独验证原有安装/费用提醒；维护行为由专门的 11 组时间/归档夹具覆盖，没有删除或放宽既有断言。
- 数据库测试均使用临时目录自行迁移和造数据。新增测试的“前端伪造字段”是纯 JSON 夹具，没有网络或模型调用。
