# 看真实界面的小工具（macOS）

**为什么有它**：开发版（`pnpm dev:desktop`）是没有 bundle id 的裸可执行文件，computer-use 控制不了它；而「页面能打开、测试通过」不等于「界面能用」。2026-10-02 之前，这个项目从没人真正看过自己做出来的界面。这里的工具只用系统自带能力：窗口截图，加辅助功能接口按名称点按钮、给输入框赋值。

**前提**：运行它们的应用（Claude 桌面版或终端）已在「系统设置 → 隐私与安全性」里获得**屏幕录制**和**辅助功能**权限。

## 编译（一次）

```bash
cd /Users/fox/WorkSpace/_external_tools/fox-stars-lab
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
- 命令面板（⌘K）打开后，`Escape` 经这条路径关不掉；直接退出开发版即可。
