# Fox Stars Lab Roadmap

Updated: 2026-07-09

## 一句话定位

Fox Stars Lab 不是“GitHub Stars 管理器”，而是 Fox 长期使用的本地优先开源项目情报库 + AI 工具选型工作台。

它服务的不是单纯收藏，而是把 GitHub Stars 转成可以找回、判断、比较、沉淀、跟踪、复用和分享的个人研发资产库。

## 当前分支原则

`fox-product-lab` 是 Fox 的个人定制分支，不是中立的上游功能分支。这个分支可以优先满足 Fox 的本机工作流和个人判断；通用 bug 修复后续再单独拆 PR 回上游。

Phase 1 已完成应用身份隔离：

| Area | Fox Stars Lab value |
| --- | --- |
| Display name | `Fox Stars Lab` |
| Bundle ID | `com.foxwork.fox-stars-lab` |
| App bundle | `Fox Stars Lab.app` |
| DMG artifact | `Fox-Stars-Lab_<version>.dmg` |
| Local database | `fox-stars-lab.sqlite3` |
| Settings file | `fox-stars-lab.settings.json` |
| Credential service | `fox-stars-lab` |

## 真实用户需求

1. 找回：Star 过很多项目，但想不起来哪个能解决当前问题。
2. 判断：一个项目是否值得用、是否活跃、是否适合 Fox 的系统。
3. 比较：同类工具太多，需要快速比较优缺点、部署难度和维护状态。
4. 沉淀：把 README、标签、笔记、使用体验变成可复用知识。
5. 跟踪：项目更新、替代品、新趋势，不靠刷 GitHub。
6. 协作：能把整理结果导出、分享给朋友、团队或反馈给上游。

## 当前机会点

现有产品已经有本地 Stars、README、AI 摘要、标签网络、AI 搜索和相似项目推荐，基础方向是对的。下一步真正要进阶，重点不是继续堆功能，而是把“AI 生成结果是否可信、能否复用、能否持续更新”做好。

Fox 的 Stars 数据高度集中在 AI 编码代理、Agent Skills、多智能体、知识库、MCP、RAG、桌面应用等方向，所以产品应围绕“工具研究、选型、复用、跟踪”优化，而不是做大而全 GitHub 控制台。

## v1.1 可靠性升级

目标：让 AI 和批处理结果可诊断、可追踪、可恢复。

- AI 请求诊断：显示模型、Base URL、超时、批量大小、失败原因。
- AI 批处理任务中心：每个仓库成功、跳过、失败可追踪，可单独重试。
- 长 README 分块摘要：不要只截前 18k，改成章节分块 + 汇总。
- OpenAI 兼容模型适配：允许非标准 JSON、字段别名、流式异常恢复。
- 标签网络分批、可配置超时、失败保留部分结果。
- 本地数据备份/恢复：明确备份 Stars、标签、笔记、AI 结果。

已落地：

- AI 设置页已增加请求诊断面板，展示服务、协议、Base URL、实际 Endpoint、模型、Key 状态、超时、重试、README 截断上限、标签网络批量上限、Prompt 版本和失败原因。
- 后端新增 `diagnose_ai_request` 本地命令，诊断只返回配置状态，不暴露 API Key 明文；本机 Ollama / LM Studio 等 OpenAI 兼容服务可正确识别为 Key 可选。
- AI 配置测试已升级为弹窗流程，可查看请求阶段、首字响应、总耗时、输出 token 和估算速度。

## v1.2 知识质量升级

目标：让每个项目变成可信、可编辑、可持续积累的知识卡。

- 项目知识卡升级为：解决什么问题、适合谁、安装难度、核心能力、限制、维护风险、替代品。
- AI 摘要增加来源引用：标明来自 README 哪一段，减少“看起来像编的”。
- 用户可编辑 AI 摘要，锁定标签，避免下次 AI 覆盖人工判断。
- 标签治理：合并标签、重命名、父子标签、锁定核心分类。
- 项目选型状态：想试、已试、在用、弃用、观察中。

已落地：

- 选型状态已进入仓库详情、列表徽标、仓库筛选、SQLite 校验和 Gist 注解兼容链路。
- 旧状态 `unread` / `read` / `later` 继续兼容，新状态为 `want_to_try` / `tried` / `in_use` / `watching` / `deprecated`。

## v1.3 搜索和选型升级

目标：让搜索从“找仓库”升级为“回答工具选型问题”。

- 混合检索：关键词 + AI 摘要 + 向量语义搜索。
- 搜索支持真实问题，例如“找一个本地优先、支持 MCP、能做知识库的工具”。
- 对比模式：选 2-5 个仓库，生成对比表。
- 方案模式：用户输入场景，输出推荐组合，而不是只给仓库列表。
- 保存搜索：把常用选型问题保存成动态视图。

## v1.4 项目雷达

目标：把 Fox Stars Lab 变成个人 AI 工具情报台。

- Release 跟踪：我关心的工具最近更新了什么。
- 活跃度/风险评分：最近提交、issue 状态、release 频率、license。
- 替代品发现：基于当前工具自动找同类新项目。
- 每周技术雷达：新增 Stars、热门更新、值得试用、可能弃用。

## v1.5 生态连接

目标：让外部 AI 助手和个人知识库工具可以调用 Fox 的本地 Stars 知识库。

- MCP Server：让 Claude、Codex、Cursor 能直接查询本地 Stars 知识库。
- Markdown/Obsidian 导出：把项目知识卡导出成笔记库。
- Raycast/Alfred 快捷搜索。
- 浏览器伴侣插件：在 GitHub 页面直接显示本地标签、笔记、AI 摘要。
- 分享能力：导出 awesome list、项目选型报告、朋友可读的公开页面。

## 优先 Issue / PR

1. AI 任务中心：批处理结果可追踪、可重试、可恢复。
2. 长 README 分块摘要，避免只读取开头导致结论不完整。
3. AI Provider 高级设置：超时、批量大小、上下文上限、响应格式兼容。
4. 项目知识卡增加维护状态、license、安装方式、适用场景和限制。
5. 标签治理：合并、锁定、层级、AI 不覆盖人工标签。
6. MCP/CLI 查询接口，让外部 AI 助手调用本地知识库。

## 可借鉴产品

- GitHub Lists：官方能力偏公开列表整理，适合作为最低基线。
- Astral：标签、筛选、笔记、README 阅读体验值得学习。
- AmintaCCCP/GithubStarsManager：功能覆盖 release、fork、gist、下载、代理、诊断日志等，可作为重功能参考。
- Karakeep：AI 自动标签、全文搜索、LLM 摘要、Agent/CLI 连接方向值得借鉴。
- Bookmark Lens：local-first、语义搜索、本地模型、MCP native 的方向非常贴近 Fox Stars Lab 的长期生态连接。

参考来源：

- [GitHub Stars 官方 Lists 文档](https://docs.github.com/en/get-started/exploring-projects-on-github/saving-repositories-with-stars)
- [Astral](https://astralapp.com/)
- [AmintaCCCP/GithubStarsManager](https://github.com/AmintaCCCP/GithubStarsManager)
- [Karakeep](https://github.com/karakeep-app/karakeep)
- [Bookmark Lens](https://github.com/cornelcroi/bookmark-lens)
