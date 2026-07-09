# Product

## Register

fox-stars-lab

## Personal Edition

Fox Stars Lab 是 Fox 长期使用的个人定制版，基于 `fox-product-lab` 分支维护。该版本优先服务 Fox 的本机工作流，应用显示名、Bundle ID、本地数据目录、配置目录、凭据服务名和安装包名称都与原版 GitHub-Stars-AI-Tools 隔离。

- Display name / window title: `Fox Stars Lab`
- Bundle ID: `com.foxwork.fox-stars-lab`
- Data/config namespace: `com.foxwork.fox-stars-lab`
- Database file: `fox-stars-lab.sqlite3`
- Settings file: `fox-stars-lab.settings.json`
- Credential service: `fox-stars-lab`
- macOS app bundle: `Fox Stars Lab.app`
- DMG artifact: `Fox-Stars-Lab_<version>.dmg`

设置页的通用设置区必须把这些身份和路径展示给用户，作为个人版不覆盖原版的可见验收入口。

这个分支不需要为保持上游兼容而牺牲个人版体验；通用 bug 修复可以后续再拆 PR 回上游。

## Users

主要用户是 Fox 本人，以及需要长期整理 GitHub Stars 的开发者和 AI 工具研究者。他们在本地桌面环境中筛选项目、阅读 README、记录笔记、维护标签，并希望把散落的收藏逐步变成可检索、可理解、可复用的个人开源项目情报库。

## Product Purpose

Fox Stars Lab 的长期定位不是“GitHub Stars 管理器”，而是本地优先的开源项目情报库 + AI 工具选型工作台。它将 GitHub Stars 同步到本地 SQLite，缓存 README，承载标签、笔记和选型状态，并为 AI 中文摘要、语义检索、项目对比、方案推荐和技术雷达提供工作台。成功的界面应让用户快速找回项目、判断是否值得用、比较同类工具、沉淀使用结论，并信任数据和密钥都留在本地优先的控制范围内。

## Roadmap Focus

- v1.1 可靠性升级：AI 请求诊断、AI 批处理任务中心、长 README 分块摘要、OpenAI 兼容模型适配、标签网络稳定性、本地数据备份/恢复。
- v1.2 知识质量升级：项目知识卡、来源引用、用户可编辑 AI 摘要、锁定标签、标签治理、项目选型状态。
- v1.3 搜索和选型升级：混合检索、真实问题搜索、对比模式、方案模式、保存搜索。
- v1.4 项目雷达：Release 跟踪、活跃度/风险评分、替代品发现、每周技术雷达。
- v1.5 生态连接：MCP Server、Markdown/Obsidian 导出、Raycast/Alfred 快捷搜索、浏览器伴侣插件、分享报告。

## Brand Personality

简洁、可靠、简约。整体应像专业知识管理工具，而不是营销页或演示样机。

## Anti-references

不要 AI 模板感，不要大标题英雄区，不要渐变背景，不要玻璃拟态，不要花哨装饰，不要空泛的内部开发阶段文案。避免过度圆角、彩色卡片堆叠和看起来像生成式落地页的视觉套路。

## Design Principles

- 工作台优先：首屏直接呈现搜索、筛选、仓库列表和知识面板。
- 黑白克制：用灰阶、线条、密度和留白建立秩序，颜色只用于必要状态。
- 信息可信：同步、缓存、注解和 AI 派生层要明确区分来源与状态。
- 选型导向：项目状态、笔记、标签和 AI 解析都要服务“是否值得试、是否适合用、后续怎么跟踪”。
- 操作靠近上下文：筛选靠近列表，笔记和标签靠近当前仓库，账号与同步保持可见。
- 少说多做：界面文案直接说明当前状态和下一步，不使用宣传腔。

## Accessibility & Inclusion

默认目标为 WCAG AA。正文和控件文字需满足对比度要求；焦点状态清晰可见；交互控件保留语义标签；动效只用于状态反馈，并尊重系统减少动态效果设置。
