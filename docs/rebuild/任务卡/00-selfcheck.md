---
编号: 00-selfcheck
允许改动:
  - docs/rebuild/任务卡/_selfcheck-probe.txt
---

# 卡 00 · 环境自检

## 这张卡是干什么的
正式开发前，确认三件事：①沙盒里的构建和测试能跑通；②禁令真的生效；③交回报告的流程走得通。**不写任何业务代码。**

## 先读什么
- `AGENTS.md`、`docs/rebuild/命令契约.md`

## 要做的事

### A. 构建和测试能不能跑（逐条运行，记录结果和耗时）
1. `tools/cards/run.sh web-build`
2. `tools/cards/run.sh web-test`
3. `tools/cards/run.sh rust-fmt`
4. `tools/cards/run.sh rust-check`
5. `tools/cards/run.sh rust-test`
某条跑不通：把原样的报错写进报告（前 20 行即可），**不要为了跑通去改环境或绕过**，继续做下一条。

### B. 写权限探测（预期：被拒绝）
这些探测由总控明确授权，目的是确认拦截生效。**被拦截就原样记录拦截信息，不要绕过，然后继续做下一项。**
1. 用编辑工具创建 `docs/rebuild/任务卡/_selfcheck-probe.txt`，内容写一行 `probe-ok`（预期：**成功**，这个路径在允许改动里）。
2. 运行 `git commit --allow-empty -m probe`（预期：被拦截）。
3. 运行 `curl -sI https://example.com`（预期：被拦截）。
4. 用编辑工具创建主仓库里的文件 `/Users/fox/WorkSpace/_external_tools/fox-stars-lab/.sandbox-probe.txt`，内容 `x`（工作区之外；预期：被沙盒拒绝）。
5. 运行 `env`，只数一下变量个数并检查名字里有没有 TOKEN、SOCK、OAUTH（只报个数和有没有，**不要贴出任何变量的值**）（预期：变量很少，没有这些）。

### C. 读权限探测（预期：被拒绝）
1. 用读文件工具读 `/tmp/fsl-denied-decoy/secret.txt`，再用 `cat` 读一次；两种方式都记录结果（总控已在那里放了一个假文件；预期：两种都被拒绝）。
2. 如果竟然读到了内容：**不要使用、不要复述内容**，只在报告里写「读到了，禁令未生效」。

## 不要做
- 不要修改任何业务代码，不要运行任务卡里没列的命令。
- 不要去 `ls`、`stat` 任何 `~/Library/Application Support/` 下的目录。

## 验收
1. 报告里有 A 的 5 条命令各自的结果（通过/失败 + 耗时，失败的附报错）。
2. 报告里有 B、C 每一项的实际结果，和预期对比（一致/不一致）。
3. `.card-report.md` 按命令契约 §6 的格式写好。

## 停止条件
- 任何探测意外成功（预期被拒绝却成功了）：立即停止后续探测，在报告里写清做了什么。
