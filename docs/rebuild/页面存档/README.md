# 页面存档

发到 claude.ai 的页面（Artifact）在这里留一份源文件。线上页面是私有的，会话临时目录（scratchpad）又会被系统清空——2026-10-19 发布的页面源文件，到 10-22 已经没了，是从线上下载回来的——所以**以这里为准**。

| 页面 | 线上地址 | 版本 | 本地文件 |
|---|---|---|---|
| Fox Stars Lab 进度与规划 | https://claude.ai/artifact/4AXBvrtrr31TWoJ7yVx2u1 | 第 4 版（2026-10-19，后续开发计划重排） | `进度与规划-V4.html` |
| Starcat 对照 | https://claude.ai/artifact/5R8SBFb9CihAXrSUcMrjZr | 第 4 版（2026-10-19，知识库问答改判为要做） | `Starcat对照-V4/index.html` + `img/`（35 张截图） |

另有 Fox Stars Lab 2.0 原型（https://claude.ai/artifact/VSzNzWtqmYQixqMz8WezwS），源文件在 `../原型/`。

## 怎么看

- 对照页：浏览器直接打开 `Starcat对照-V4/index.html`，`img/` 要在它旁边。
- 计划页：是「只有内容」的写法（平台发布时才套外壳），本地直接打开能看，排版可能和线上略有差别，要看原样就开线上。
- 计划页的文字以 `../后续开发计划.md` 为准，页面是它的展示版。

## 怎么更新线上

用 Artifact 工具，带线上地址（`url`）和本地文件路径重新发布；对照页只发 `index.html`，`img/` 里没改的会保留。**先改这里的文件，再发布**，别反过来；发布后把版本号补进上面的表。

## 注意

- 对照页里：我们的截图是假数据（mock-host），Starcat 的是 Fox 本机截图。**API Key、许可证、令牌类截图一律不放**（AI 与 MCP 两组截图已逐张看过，没有明文密钥）。
- 版本号以线上为准：计划页 version id `1792414765-5ea0`，对照页 `1792415009-249d`。
