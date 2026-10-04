//! 任务交接单里「先问我」「装到哪」「按任务换题」的规则。
//!
//! 只做字符串判断：不联网、不读库。输入是理解卡里的装法和注意事项（第三方文字，只当资料看），
//! 输出是几行固定措辞的提醒。认不出的不猜：宁可少标，也不要每个项目都标「先问我」，
//! 标多了谁都不会再看。

/// 开工前要先问 Fox 的几类事。和 `packs.rs` 里工作约定第 3 条「只有这几种情况先停下来问我」对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Caution {
    /// 要管理员权限（sudo、.pkg 安装包）
    Admin,
    /// 要 API Key、付费或订阅
    Money,
    /// 要登录账号、扫码、Cookie，或有账号被封的风险
    Account,
    /// 内容可能发到外部服务，或涉及隐私数据
    DataOut,
    /// 会往真实账号发布、评论、回复、私信
    Publish,
    /// 理解卡写明它不支持 macOS
    Platform,
}

impl Caution {
    /// 一句话里的短说法（在「开工前先问我」的汇总那条里用）。
    pub(crate) fn short(self) -> &'static str {
        match self {
            Caution::Admin => "要管理员权限",
            Caution::Money => "要 API Key 或付费",
            Caution::Account => "要登录账号 / 扫码 / Cookie",
            Caution::DataOut => "内容可能外发或涉及隐私数据",
            Caution::Publish => "会往真实账号发内容",
            Caution::Platform => "不支持 macOS",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Caution::Admin => "要管理员权限（sudo / .pkg 安装包）",
            Caution::Money => "要 API Key 或付费 / 订阅（先告诉我预算和用哪家）",
            Caution::Account => "要登录账号 / 扫码 / Cookie，或有账号被封的风险",
            Caution::DataOut => {
                "内容可能发到外部服务，或涉及隐私数据（有本机 / 本地模型的模式就优先用，不要默认外发）"
            }
            Caution::Publish => {
                "会往我的真实账号发布 / 评论 / 回复 / 私信（先只演练，发出去之前每一条都要我确认）"
            }
            Caution::Platform => {
                "卡片写它不支持 macOS（只有 Windows / Linux 版），这台是 Mac：先别装，问我换哪个"
            }
        }
    }
}

pub(crate) const ONE_LINE_INSTALL_NOTE: &str =
    "装法里有「curl … | bash」这类一行安装：不要直接运行，先把脚本下载下来通读，没问题再装";
const UNSIGNED_NOTE: &str = "未签名的 Mac 应用：第一次打开会被系统拦下，要在「系统设置 → 隐私与安全性」里点「仍要打开」；不要用 sudo 或 xattr 命令绕过，先告诉我";
const LINUX_NOTE: &str =
    "装法写的是 Linux 命令（apt / dnf / .deb / .rpm），这台是 Mac，别照抄：先看有没有 macOS 的装法，没有就告诉我";
const WINDOWS_NOTE: &str =
    "装法写的是 Windows 命令（winget / .exe / .msi），这台是 Mac，别照抄：先看有没有 macOS 的装法，没有就告诉我";
const SUDO_DOCKER_NOTE: &str =
    "装法里 Docker 命令前的 sudo 去掉：Mac 上的 Docker（Docker Desktop / OrbStack）不需要 sudo";

/// 装法写的是别的系统的命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForeignOs {
    Linux,
    Windows,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Cautions {
    /// 要先问 Fox 的事。
    pub(crate) ask: Vec<Caution>,
    /// 下面几项是「照着做就行」的提醒，不用先问。
    pub(crate) one_line_install: bool,
    pub(crate) unsigned: bool,
    pub(crate) foreign: Option<ForeignOs>,
    pub(crate) sudo_docker: bool,
}

impl Cautions {
    /// 「先问我再动手」后面接的那段话；没有要问的返回 `None`。
    pub(crate) fn ask_text(&self) -> Option<String> {
        if self.ask.is_empty() {
            return None;
        }
        Some(
            self.ask
                .iter()
                .map(|caution| caution.label())
                .collect::<Vec<_>>()
                .join("；"),
        )
    }

    /// 汇总那条用的短说法。
    fn ask_short_text(&self) -> Option<String> {
        if self.ask.is_empty() {
            return None;
        }
        Some(
            self.ask
                .iter()
                .map(|caution| caution.short())
                .collect::<Vec<_>>()
                .join("、"),
        )
    }

    /// 不用先问、照着做就行的提醒，每条一句，按重要程度排。
    pub(crate) fn notes(&self) -> Vec<&'static str> {
        let mut notes = Vec::new();
        if self.one_line_install {
            notes.push(ONE_LINE_INSTALL_NOTE);
        }
        match self.foreign {
            Some(ForeignOs::Linux) => notes.push(LINUX_NOTE),
            Some(ForeignOs::Windows) => notes.push(WINDOWS_NOTE),
            None => {}
        }
        if self.sudo_docker {
            notes.push(SUDO_DOCKER_NOTE);
        }
        if self.unsigned {
            notes.push(UNSIGNED_NOTE);
        }
        notes
    }

    /// JSON 里 `cautions` 数组的内容：要先问的，加上不用先问的提醒。
    pub(crate) fn labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self
            .ask
            .iter()
            .map(|caution| caution.label().to_owned())
            .collect();
        labels.extend(self.notes().into_iter().map(str::to_owned));
        labels
    }

    /// 理解卡写了它支持哪些系统，而里面没有 macOS（也不是跨平台 / 网页）：这台是 Mac，要先问。
    pub(crate) fn with_platforms(mut self, platforms: &[String]) -> Self {
        let lowered: Vec<String> = platforms
            .iter()
            .map(|platform| platform.trim().to_lowercase())
            .filter(|platform| !platform.is_empty())
            .collect();
        let usable = lowered.iter().any(|platform| {
            matches!(
                platform.as_str(),
                "macos" | "mac" | "darwin" | "cross" | "web"
            )
        });
        if !lowered.is_empty() && !usable {
            self.ask.push(Caution::Platform);
        }
        self
    }
}

const ADMIN_TOKENS: &[&str] = &["sudo", "管理员", ".pkg", "root 权限", "root权限"];
const MONEY_TOKENS: &[&str] = &[
    "api key",
    "api_key",
    "apikey",
    "api-key",
    "api 密钥",
    "api密钥",
    "付费",
    "订阅",
    "收费",
    "计费",
    "费用",
    "按量",
    "paid plan",
    "调用成本",
    "成本不可控",
];
const ACCOUNT_TOKENS: &[&str] = &[
    "注册登录",
    "登录账号",
    "登录账户",
    "扫码",
    "cookie",
    "登录态",
    "账号密码",
    "access token",
    "oauth",
    "账号风险",
    "封号",
    "封禁",
    "风控",
    "凭证",
    "数据库密钥",
    "提取密钥",
];
const DATA_OUT_TOKENS: &[&str] = &[
    "发送至",
    "发送到",
    "发送给",
    "中转",
    "离开本地",
    "外发",
    "上传到",
    "上传至",
    "上传头像",
    "上传声音",
    "数据处理风险",
    "数据隐私",
    "隐私风险",
    "隐私泄露",
    "敏感信息",
    "托管 ocr",
    "云端 ocr",
];
const PUBLISH_TOKENS: &[&str] = &[
    "误发",
    "草稿箱",
    "推送草稿",
    "自动发布",
    "自动评论",
    "自动回复",
    "群发",
];
const UNSIGNED_TOKENS: &[&str] = &[
    "未签名",
    "gatekeeper",
    "手动放行",
    "手动解除",
    "无法验证开发者",
    "身份不明",
];
/// 只有这些系统的命令、又没有任何跨平台写法时，才说「装法是别的系统的」。
const LINUX_MARKERS: &[&str] = &[
    "apt install",
    "apt-get install",
    "dnf install",
    "yum install",
    "pacman -s",
    "snap install",
    ".deb",
    ".rpm",
    "systemctl ",
];
const WINDOWS_MARKERS: &[&str] = &[
    "winget ",
    "choco install",
    "scoop ",
    ".exe",
    ".msi",
    "windows 安装包",
    "windows 安装器",
    "windows 安装程序",
    "powershell",
];
/// 跨平台或 macOS 上能用的写法：出现任何一个，就不说「装法是别的系统的」。
const SAFE_MARKERS: &[&str] = &[
    "git clone",
    "npm ",
    "pnpm ",
    "yarn ",
    "npx ",
    "bun ",
    "pip ",
    "pip3 ",
    "pipx ",
    "uv ",
    "cargo ",
    "go install",
    "docker",
    "brew ",
    "curl ",
    "wget ",
    "dmg",
    ".app",
    "macos",
    "mac os",
    ".zip",
    "zip 包",
    ".tar.gz",
];
/// 这些词紧挨在前面，说明后面那个词是被否定的（「无需 API Key」不算要 API Key）。
const NEGATIONS: [&str; 7] = ["无需", "不需要", "不用", "无须", "不必", "避免", "without"];
/// 看前面几个字符找否定词。
const NEGATION_WINDOW: usize = 8;
/// Linux 写法的 sudo：在 Mac 上不是「要管理员权限」。
const LINUX_SUDO: [&str; 8] = [
    "sudo apt-get",
    "sudo apt",
    "sudo dnf",
    "sudo yum",
    "sudo snap",
    "sudo pacman",
    "sudo systemctl",
    "sudo service",
];

/// 从装法和注意事项里认出要先问 Fox 的几类事，和几条「照着做就行」的提醒。
/// `how_to_try` 和 `risks` 都是未截断的原文。内容外发只看注意事项：装法里的「上传到」多半是在说把成品传到哪儿。
pub(crate) fn detect_cautions(how_to_try: &str, risks: &str) -> Cautions {
    let how = how_to_try.to_lowercase();
    let risk = risks.to_lowercase();
    // `sudo docker …` 和 `sudo apt …` 是 Linux 的写法，Mac 上的 Docker 不需要 sudo、apt 根本没有：
    // 不算「要管理员权限」，去掉 sudo 再认，另外各写一条提醒。
    let (how_for_admin, sudo_docker, sudo_linux) = strip_foreign_sudo(&how);
    let both = format!("{how_for_admin}；{risk}");
    let mut ask = Vec::new();
    if mentions_any(&both, ADMIN_TOKENS) {
        ask.push(Caution::Admin);
    }
    if mentions_any(&both, MONEY_TOKENS) {
        ask.push(Caution::Money);
    }
    if mentions_any(&both, ACCOUNT_TOKENS) {
        ask.push(Caution::Account);
    }
    if mentions_any(&risk, DATA_OUT_TOKENS) {
        ask.push(Caution::DataOut);
    }
    if mentions_any(&both, PUBLISH_TOKENS) {
        ask.push(Caution::Publish);
    }
    Cautions {
        ask,
        one_line_install: pipes_to_shell(&how),
        unsigned: UNSIGNED_TOKENS.iter().any(|token| both.contains(token)),
        foreign: foreign_install(&how_for_admin, sudo_linux),
        sudo_docker,
    }
}

/// 去掉 Docker / Linux 包管理器命令前的 sudo。返回 (处理后的文本, 有没有 sudo docker, 有没有 Linux 的 sudo 命令)。
fn strip_foreign_sudo(how: &str) -> (String, bool, bool) {
    let mut text = how.to_owned();
    let docker = text.contains("sudo docker");
    if docker {
        text = text.replace("sudo docker", "docker");
    }
    let mut linux = false;
    for pattern in LINUX_SUDO {
        if text.contains(pattern) {
            linux = true;
            text = text.replace(pattern, &pattern["sudo ".len()..]);
        }
    }
    (text, docker, linux)
}

/// 装法只有别的系统的写法（Linux 的 apt / dnf / .deb / .rpm，Windows 的 winget / .exe / .msi），
/// 没有任何跨平台或 macOS 的写法。
fn foreign_install(how: &str, sudo_linux: bool) -> Option<ForeignOs> {
    if SAFE_MARKERS.iter().any(|marker| how.contains(marker)) {
        return None;
    }
    if sudo_linux || LINUX_MARKERS.iter().any(|marker| how.contains(marker)) {
        return Some(ForeignOs::Linux);
    }
    if WINDOWS_MARKERS.iter().any(|marker| how.contains(marker)) {
        return Some(ForeignOs::Windows);
    }
    None
}

fn mentions_any(text: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|token| mentions(text, token))
}

/// `text` 里有没有 `token`：英文词左边不能紧贴字母或数字（`visudo` 不算 `sudo`），前面紧挨着否定词的不算。
fn mentions(text: &str, token: &str) -> bool {
    let mut from = 0;
    while let Some(found) = text[from..].find(token) {
        let at = from + found;
        if left_boundary_ok(text, at, token) && !negated(text, at) {
            return true;
        }
        from = at + token.chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn left_boundary_ok(text: &str, at: usize, token: &str) -> bool {
    let starts_alnum = token
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric());
    if !starts_alnum {
        return true;
    }
    !text[..at]
        .chars()
        .next_back()
        .is_some_and(|ch| ch.is_ascii_alphanumeric())
}

/// 前面最多 `NEGATION_WINDOW` 个字符、且不越过上一个标点，里面有否定词。
fn negated(text: &str, at: usize) -> bool {
    let mut window: Vec<char> = Vec::new();
    for ch in text[..at].chars().rev().take(NEGATION_WINDOW) {
        if matches!(
            ch,
            '；' | ';' | '，' | ',' | '。' | '.' | '、' | '\n' | '（' | '('
        ) {
            break;
        }
        window.push(ch);
    }
    let window: String = window.into_iter().rev().collect();
    NEGATIONS.iter().any(|negation| window.contains(negation))
}

/// 装法里有没有「下载一段脚本直接交给 shell 运行」：`curl … | bash`、`wget -qO- … | sh`、
/// `bash -c "$(curl …)"`。一条命令以「；」「;」「&&」或换行为界。
fn pipes_to_shell(how: &str) -> bool {
    for part in how.split(['；', ';', '\n']) {
        for segment in part.split("&&") {
            if segment_pipes_to_shell(segment.trim()) {
                return true;
            }
        }
    }
    false
}

fn segment_pipes_to_shell(segment: &str) -> bool {
    let fetches = contains_word(segment, "curl")
        || contains_word(segment, "wget")
        || contains_word(segment, "irm")
        || contains_word(segment, "iwr");
    if !fetches {
        return false;
    }
    if ["$(curl", "$(wget", "<(curl", "<(wget", "`curl", "`wget"]
        .iter()
        .any(|needle| segment.contains(needle))
    {
        return true;
    }
    segment.split('|').skip(1).any(runs_stdin_as_script)
}

/// 管道后面这一段是不是「把收到的内容当脚本运行」：shell 一律算；python / node 这类解释器只有
/// 不带参数或只带 `-`（从标准输入读脚本）才算，`| python3 -m json.tool` 只是在排版，不算。
fn runs_stdin_as_script(part: &str) -> bool {
    let mut tokens = part.split_whitespace().peekable();
    if tokens.peek() == Some(&"sudo") {
        tokens.next();
        while tokens.peek().is_some_and(|token| token.starts_with('-')) {
            tokens.next();
        }
    }
    let Some(command) = tokens.next() else {
        return false;
    };
    let command = command.trim_matches(|ch: char| !ch.is_ascii_alphanumeric());
    match command {
        "sh" | "bash" | "zsh" | "iex" => true,
        "python" | "python3" | "ruby" | "perl" | "node" => {
            let rest: Vec<&str> = tokens.collect();
            rest.is_empty() || rest[0] == "-"
        }
        _ => false,
    }
}

fn contains_word(text: &str, word: &str) -> bool {
    let mut from = 0;
    while let Some(found) = text[from..].find(word) {
        let at = from + found;
        let end = at + word.len();
        let before_ok = !text[..at]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric());
        let after_ok = !text[end..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

const DOCKER_TARGET: &str = "用 Docker 跑：先确认本机装了 Docker 并已启动（没有就先问我）；端口和目录按我本机 AGENTS.md 的规则登记";
const APP_NOTE: &str = "（桌面应用：你点不了它的界面。先看它有没有命令行 / MCP 入口，没有就告诉我，由我自己操作或换备选）";
const APP_NOTE_WEB: &str = "（装好后是个网页界面：你点不了它的界面。先看它有没有 API / 命令行入口，没有就告诉我，由我自己操作或换备选）";
const APP_UNKNOWN: &str = "按装法里写的方式装；装法没写清楚就先读它的 README，别猜安装包";
const NODE_TARGET: &str = "Node 工具：在这次任务的目录里用 npm / npx 本地装，不要 -g 全局装";
const PYTHON_TARGET: &str =
    "Python 工具：用 pipx 或 uv tool 装，或者放进项目自己的 venv，不要装进系统 Python";
const RUST_TARGET: &str = "按装法里的 cargo / go 命令装在用户目录里，不要用 sudo";
const CLI_DEFAULT: &str = "优先用 brew、pipx 或 uv tool，装在独立环境里";
const SCRIPT_TARGET: &str =
    "装法是一行安装脚本：先把脚本下载下来通读，没问题再运行；装在用户目录里，不要用 sudo";
/// 装法里说的是「去下载一个现成安装包」：英文词要整词（`ytdownloader` 里的 download 不算）；
/// 中文只认「安装包」「发行包」「官网」，不认单独的「下载」（「下载打包结果后运行」「下载视频」都不是装安装包）。
fn mentions_release(how: &str) -> bool {
    ["release", "releases", "download", "dmg", "homebrew"]
        .iter()
        .any(|word| contains_word(how, word))
        || [".app", "brew install --cask", "安装包", "发行包", "官网"]
            .iter()
            .any(|word| how.contains(word))
}

/// 装法里的安装包是 Windows 的（`.exe` / `.msi` / 「Windows 安装包」），而且没提 macOS 的包。
fn windows_installer_only(how: &str) -> bool {
    let windows = ["windows 安装包", "windows 安装器", ".exe", ".msi"]
        .iter()
        .any(|word| how.contains(word));
    windows && !(how.contains("dmg") || how.contains("macos") || how.contains("mac os"))
}
const SOURCE_COMMANDS: [&str; 10] = [
    "git clone",
    "npm ",
    "pnpm ",
    "yarn ",
    "pip ",
    "uv ",
    "cargo ",
    "go install",
    "npx ",
    "bun ",
];
const NODE_COMMANDS: [&str; 5] = ["npm ", "npx ", "pnpm ", "yarn ", "bun "];
const PYTHON_COMMANDS: [&str; 4] = ["pip ", "pip3 ", "pipx ", "uv "];

/// 装到哪里。先按形态给默认写法，再看装法里实际写了什么：写的是 Docker 就别让人去拖 .dmg，
/// 写的是 npm / pip 就按「Node 用项目本地、Python 用 venv」的约定写，装法里根本没写安装包就不编一个 .dmg。
pub(crate) fn install_target(
    form: Option<&str>,
    repo_name: &str,
    how_to_try: Option<&str>,
) -> String {
    let how = how_to_try.map(str::to_lowercase).unwrap_or_default();
    let uses_docker = contains_word(&how, "docker");
    let global_flag = how.contains(" -g") || how.contains("--global");
    match form.map(str::trim) {
        Some("skill") => {
            let mut text = format!(
                "~/.agents/skills/{repo_name}（Codex 从这里读取 Skill；README 里写的是 ~/.claude/skills 的话，以这里为准）"
            );
            if how.contains("npx skills add") && !global_flag {
                text.push_str(
                    "；装法里的 `npx skills add` 默认只装进当前目录，要装给 Codex 全局用就加 `-g -y`，不确定先 `npx skills add --help` 看一眼",
                );
            }
            text
        }
        Some("mcp") => {
            if uses_docker {
                "先按装法用 Docker 把它跑起来，再用 codex mcp add 登记它的地址；改 ~/.codex/config.toml 之前先备份".to_owned()
            } else {
                "用 codex mcp add 登记，或者改 ~/.codex/config.toml（改之前先备份）".to_owned()
            }
        }
        Some("app") => {
            if uses_docker {
                format!("{DOCKER_TARGET}{APP_NOTE_WEB}")
            } else {
                let base = if how.contains("brew install") {
                    "按装法里的 brew 命令装；brew 要输入密码或管理员权限时先停下来问我"
                } else if mentions_release(&how)
                    && !(windows_installer_only(&how)
                        && SOURCE_COMMANDS.iter().any(|command| how.contains(command)))
                {
                    "从 Releases 下载 .dmg，拖进「应用程序」；只有 .pkg 的话先告诉我（安装包可能要管理员权限）"
                } else if SOURCE_COMMANDS.iter().any(|command| how.contains(command)) {
                    "按装法里的命令，装在这次任务的目录里，不要装进系统；先查本机有没有它要的运行环境（Node / Python），缺了先告诉我"
                } else {
                    APP_UNKNOWN
                };
                format!("{base}{APP_NOTE}")
            }
        }
        Some("cli") => {
            if uses_docker {
                DOCKER_TARGET.to_owned()
            } else if how.contains("brew install") {
                "按装法里的 brew 命令装；brew 要输入密码或管理员权限时先停下来问我".to_owned()
            } else if pipes_to_shell(&how) {
                SCRIPT_TARGET.to_owned()
            } else if NODE_COMMANDS.iter().any(|command| how.contains(command)) {
                NODE_TARGET.to_owned()
            } else if PYTHON_COMMANDS.iter().any(|command| how.contains(command)) {
                PYTHON_TARGET.to_owned()
            } else if how.contains("cargo ") || how.contains("go install") {
                RUST_TARGET.to_owned()
            } else {
                CLI_DEFAULT.to_owned()
            }
        }
        Some("library") => {
            if NODE_COMMANDS.iter().any(|command| how.contains(command)) {
                NODE_TARGET.to_owned()
            } else if how.contains("<script") || how.contains("script 标签") || how.contains("cdn")
            {
                "前端库：只在这次任务的页面里引入，不要装进系统".to_owned()
            } else if PYTHON_COMMANDS.iter().any(|command| how.contains(command))
                || how.contains("conda")
                || how.is_empty()
            {
                "项目自己的虚拟环境（venv），不要装进系统 Python".to_owned()
            } else {
                "按装法里写的方式，在这次任务的目录里用（Python 用 venv、Node 用项目本地），不要装进系统全局".to_owned()
            }
        }
        Some("service") => {
            if uses_docker || how.is_empty() {
                "用 Docker 跑，并按我本机 AGENTS.md 的规则登记端口和目录".to_owned()
            } else {
                "按装法里的命令，装在这次任务的目录里；要长期跑的话，按我本机 AGENTS.md 的规则登记端口和目录".to_owned()
            }
        }
        _ => {
            if uses_docker {
                DOCKER_TARGET.to_owned()
            } else {
                "README 推荐的方式，装进一个独立的新目录".to_owned()
            }
        }
    }
}

// ---------- 按任务换题 ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskKind {
    Video,
    Records,
    Batch,
    Writing,
    Visual,
    /// 抓取 / 采集 / 下载 / 监控
    Scrape,
    /// 文件格式转换（PDF、Word、Excel、图片格式……）
    Convert,
    /// 自动发布 / 回复 / 私信 / 涨粉：会动真实账号
    Operate,
    /// 分析、调研、竞品、看板、画像
    Analysis,
    General,
}

const VIDEO_WORDS: [&str; 12] = [
    "视频", "字幕", "剪辑", "配音", "口播", "直播", "剪映", "转场", "旁白", "bgm", "录屏", "混剪",
];
const RECORDS_WORDS: [&str; 14] = [
    "日报",
    "周报",
    "月报",
    "群聊",
    "聊天记录",
    "会议",
    "纪要",
    "录音",
    "转写",
    "整理",
    "汇总",
    "摘要",
    "记录",
    "总结",
];
const BATCH_WORDS: [&str; 11] = [
    "批量",
    "压缩",
    "尺寸",
    "缩放",
    "裁剪",
    "格式转换",
    "水印",
    "改图",
    "抠图",
    "修图",
    "转格式",
];
const WRITING_WORDS: [&str; 12] = [
    "文章",
    "公众号",
    "文案",
    "标题",
    "写作",
    "翻译",
    "seo",
    "爆款",
    "脚本",
    "选题",
    "邮件",
    "推文",
];
const VISUAL_WORDS: [&str; 15] = [
    "封面",
    "海报",
    "配图",
    "插图",
    "主图",
    "详情页",
    "头像",
    "logo",
    "信息图",
    "设计",
    "图片",
    "小红书",
    "品牌",
    "图文",
    "插画",
];

const SCRAPE_WORDS: [&str; 6] = ["抓取", "采集", "爬取", "爬虫", "下载", "监控"];
const CONVERT_WORDS: [&str; 14] = [
    "格式转换",
    "转格式",
    "转换格式",
    "pdf",
    "word",
    "excel",
    "markdown",
    "docx",
    "xlsx",
    "png",
    "jpg",
    "webp",
    "mp3",
    "mp4",
];
const OPERATE_WORDS: [&str; 9] = [
    "发布",
    "自动回复",
    "自动评论",
    "私信",
    "群发",
    "涨粉",
    "养号",
    "排期",
    "定时发布",
];
const ANALYSIS_WORDS: [&str; 10] = [
    "分析",
    "调研",
    "竞品",
    "看板",
    "画像",
    "统计",
    "可视化",
    "趋势",
    "数据",
    "报表",
];

fn kind_words(kind: TaskKind) -> &'static [&'static str] {
    match kind {
        TaskKind::Video => &VIDEO_WORDS,
        TaskKind::Records => &RECORDS_WORDS,
        TaskKind::Batch => &BATCH_WORDS,
        TaskKind::Writing => &WRITING_WORDS,
        TaskKind::Visual => &VISUAL_WORDS,
        TaskKind::Scrape => &SCRAPE_WORDS,
        TaskKind::Convert => &CONVERT_WORDS,
        TaskKind::Operate => &OPERATE_WORDS,
        TaskKind::Analysis => &ANALYSIS_WORDS,
        TaskKind::General => &[],
    }
}

/// 这句话是哪类任务，最多两类。先看「要做什么动作」：抓取 > 转格式 > 自动发布 > 分析，认出来的排最前
/// （「抓取抖音热门视频」是抓取，不是视频）；再按「被做的东西」：关键词出现得越靠后的越靠前
/// （中文里被做的东西在句子末尾：「给短视频加字幕」是视频，「公众号文章配图」是配图），位置相同按固定顺序。
/// 一类都认不出返回空。
pub(crate) fn task_kinds(problem: &str) -> Vec<TaskKind> {
    let lower = problem.to_lowercase();
    let mentions = |kind: TaskKind| {
        kind_words(kind)
            .iter()
            .filter_map(|word| lower.rfind(word))
            .max()
    };
    let mut kinds: Vec<TaskKind> = [
        TaskKind::Scrape,
        TaskKind::Convert,
        TaskKind::Operate,
        TaskKind::Analysis,
    ]
    .into_iter()
    .filter(|kind| mentions(*kind).is_some())
    .collect();
    let mut objects: Vec<(usize, usize, TaskKind)> = Vec::new();
    for (order, kind) in [
        TaskKind::Video,
        TaskKind::Records,
        TaskKind::Batch,
        TaskKind::Writing,
        TaskKind::Visual,
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(position) = mentions(kind) {
            objects.push((position, order, kind));
        }
    }
    objects.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
    kinds.extend(objects.into_iter().map(|(_, _, kind)| kind));
    kinds.truncate(2);
    kinds
}

pub(crate) const ASK_VISUAL: [&str; 5] = [
    "要做的具体内容：标题和文案原文、品牌名的写法；要处理现成素材（视频、图片、文章）的话，告诉我文件在哪、有几个、什么语言",
    "品牌素材放在哪：logo、色板、字体、产品图",
    "成品的尺寸或格式、单张还是一组",
    "风格偏好（给我看一两个参考也行）",
    "能不能用网络上找的图、AI 生成的图，或者要付费 / 要 API Key 的工具（预算多少）",
];
const ASK_VIDEO: [&str; 5] = [
    "要处理的视频：文件在哪、有几个、多长、说什么语言（有没有口播、配乐、现成字幕）",
    "成品要什么：字幕是「烧进画面」还是单独的字幕文件（.srt）、只要中文还是中英双语；画面比例（竖屏 9:16 / 横屏 16:9）和时长上限",
    "字幕或配音怎么做：只用本机离线的办法，还是可以用要 API Key / 付费的在线服务（预算多少）；视频内容能不能发给第三方服务",
    "品牌和素材：有没有固定的片头片尾、字体、颜色、背景音乐（版权能不能用）",
    "发到哪个平台（抖音 / 小红书 / 视频号），有没有对应的规格要求",
];
const ASK_RECORDS: [&str; 5] = [
    "原始记录从哪来：哪个群 / 哪几天 / 导出的文件放在哪（你不要替我登录任何账号，也不要自己去取密钥）",
    "这些记录能不能发给第三方 AI 服务（含聊天内容、客户信息）；有没有只在本机跑的办法",
    "要整理成什么样：日报还是周报、给谁看、多久出一次、大概多长；有没有现成的模板或往期样例",
    "哪些内容要保留、哪些要去掉（比如闲聊、隐私、客户姓名）",
    "能不能用要付费 / 要 API Key 的工具（预算多少）",
];
const ASK_BATCH: [&str; 5] = [
    "要处理的文件：在哪个文件夹、一共多少个、什么格式",
    "目标是什么：尺寸（宽 × 高 / 比例）、格式、体积上限（比如每张不超过 200KB）；宽高比和原图不一样时怎么办（等比缩放 / 裁剪 / 留白）",
    "原文件怎么处理：默认保留原文件、结果放到新文件夹，还是直接覆盖",
    "文件名怎么命名；有没有要特别处理的地方（水印、边框、透明背景、拍摄信息要不要保留）",
    "能不能用要付费 / 要 API Key 的在线工具（预算多少）；文件能不能上传到第三方网站",
];
const ASK_WRITING: [&str; 5] = [
    "主题、读者是谁、发在哪个平台（公众号 / 小红书 / 知乎……）、大概多少字",
    "语气和风格：有没有品牌口吻、禁用词、往期爆款可以参考",
    "有没有必须用的资料、数据、产品卖点；哪些信息不能写",
    "要不要配图、要不要排成可以直接发布的格式（Markdown / HTML / Word）",
    "能不能用要付费 / 要 API Key 的工具（预算多少）",
];
const ASK_SCRAPE: [&str; 5] = [
    "要采集什么：哪个平台、哪些账号 / 关键词 / 话题 / 链接；大概要多少条、哪段时间的",
    "结果怎么给我：存成表格（CSV / Excel）还是 Markdown 汇总；要不要带链接、点赞评论数、发布时间",
    "需要登录才能看的内容：你不要替我登录，也不要读我的 Cookie；要用账号的话由我自己登录后告诉你怎么接上",
    "频率和数量要克制，别一口气大量请求；遇到验证码或被平台限制就停下来告诉我",
    "能不能用要付费 / 要 API Key / 要代理的工具（预算多少）；采到的内容能不能发给第三方 AI 服务分析",
];
const ASK_CONVERT: [&str; 5] = [
    "要转换的文件：在哪、有几个、什么格式、大概多大",
    "要转成什么格式；要保留什么（排版、表格、图片、批注、字体）",
    "原文件怎么处理：默认保留原文件，结果放到新文件夹；文件名有没有要求",
    "文件里有没有敏感内容：能不能用在线转换服务（会把文件传上去），还是只用本机工具",
    "能不能用要付费 / 要 API Key 的工具（预算多少）",
];
const ASK_OPERATE: [&str; 5] = [
    "要在哪个平台、用哪个账号（你不要替我登录，也不要读我的 Cookie / 密钥；要登录时由我自己登录）",
    "发什么 / 回什么：内容从哪来（我给你，还是你按要求写）；发出去之前要不要我逐条确认（默认要）",
    "频率和数量：每天最多几条、什么时间段；有没有不能碰的内容和对象",
    "出问题怎么办：被平台限制或要验证码就停下来告诉我；先用「只演练、不真正发出」的方式试一遍",
    "能不能用要付费 / 要 API Key 的工具（预算多少）",
];
const ASK_ANALYSIS: [&str; 5] = [
    "要回答什么问题、给谁看、用来做什么决定",
    "分析的对象和范围：哪些竞品 / 渠道 / 产品、哪段时间、哪些指标",
    "数据从哪来：我给文件 / 表格，还是去公开网页上查；有没有要登录后台才能看的数据（你不要替我登录）",
    "要交付什么：表格、图表，还是一页结论；细到什么程度（按天 / 按周 / 按渠道）",
    "数据里有没有敏感内容（客户信息、销售数据）：能不能发给第三方 AI 服务；能不能用要付费的工具",
];
const ASK_GENERAL: [&str; 5] = [
    "要做的具体内容：主题、文案原文、品牌名的写法；要处理现成素材的话，告诉我文件在哪、有几个、什么格式",
    "成品的格式、数量和尺寸（或页数 / 时长）",
    "品牌素材或参考样例放在哪（logo、色板、字体、往期成品）",
    "风格和语气偏好（给我看一两个参考也行）",
    "能不能用要付费 / 要 API Key 的工具（预算多少）；内容能不能发给第三方服务",
];

fn ask_set(kind: TaskKind) -> &'static [&'static str; 5] {
    match kind {
        TaskKind::Video => &ASK_VIDEO,
        TaskKind::Records => &ASK_RECORDS,
        TaskKind::Batch => &ASK_BATCH,
        TaskKind::Writing => &ASK_WRITING,
        TaskKind::Visual => &ASK_VISUAL,
        TaskKind::Scrape => &ASK_SCRAPE,
        TaskKind::Convert => &ASK_CONVERT,
        TaskKind::Operate => &ASK_OPERATE,
        TaskKind::Analysis => &ASK_ANALYSIS,
        TaskKind::General => &ASK_GENERAL,
    }
}

/// 第二类任务的第一条问题能不能插进来：只有「内容类」任务之间（图文 / 写作 / 记录整理 / 视频）和「批量处理」
/// 可以混；批量处理、抓取、转格式、自动发布、分析自己不混别的（「批量去除图片水印」不该被问标题和品牌名）。
fn may_mix(primary: TaskKind, secondary: TaskKind) -> bool {
    use TaskKind::{Batch, Records, Video, Visual, Writing};
    primary != secondary
        && matches!(primary, Visual | Writing | Records | Video)
        && matches!(secondary, Visual | Writing | Records | Video | Batch)
}

/// 一个项目要先问的事，用来在「开工前先问我」里列一条。
pub(crate) struct Flagged<'a> {
    pub(crate) name: &'a str,
    pub(crate) cautions: &'a Cautions,
}

/// 「开工前先问我」：先是按任务类型选的 5 条；有第二类就把它的第一条插在第 2 位；
/// 选中的项目里有要先问的，最后再加一条把它们列出来。
pub(crate) fn ask_items(problem: &str, flagged: &[Flagged<'_>]) -> Vec<String> {
    let kinds = task_kinds(problem);
    let primary = kinds.first().copied().unwrap_or(TaskKind::General);
    let mut items: Vec<String> = ask_set(primary)
        .iter()
        .map(|item| (*item).to_owned())
        .collect();
    if let Some(second) = kinds.get(1).filter(|second| may_mix(primary, **second)) {
        let extra = ask_set(*second)[0];
        if !items.iter().any(|item| item == extra) {
            items.insert(1, extra.to_owned());
        }
    }
    let listed: Vec<String> = flagged
        .iter()
        .filter_map(|entry| {
            entry
                .cautions
                .ask_short_text()
                .map(|text| format!("{}（{text}）", entry.name))
        })
        .collect();
    if !listed.is_empty() {
        items.push(format!(
            "这几个工具装之前要先问我：{}。我点头了再装，没点头的先别装，告诉我有没有不用它们的办法",
            listed.join("；")
        ));
    }
    items
}

pub(crate) const DONE_VISUAL: [&str; 3] = [
    "成品能直接打开使用，尺寸 / 格式符合要求，并用绝对路径告诉我放在哪",
    "文字逐字核对过（尤其标题、品牌名），没有错别字",
    "「说明.md」写清了用的工具、参数和素材来源",
];
const DONE_VIDEO: [&str; 3] = [
    "成品视频能正常播放，画面比例和时长符合要求，字幕和声音对得上、没有错字，并用绝对路径告诉我放在哪",
    "原视频没有被改动或覆盖（成品另存为新文件）",
    "「说明.md」写清了用的工具、参数（比如转写模型、字幕样式）和素材来源",
];
const DONE_RECORDS: [&str; 3] = [
    "整理结果里的每条信息都能对回原始记录，没有漏掉的日期，也没有编造的内容",
    "涉及隐私的内容（聊天、姓名、客户信息）没有发给任何第三方服务；做了什么处理写清楚",
    "「说明.md」写清了数据从哪来、用的工具和参数、怎么重新跑一遍",
];
const DONE_BATCH: [&str; 3] = [
    "处理后的文件数量和处理前对得上（输入多少个、输出多少个），尺寸 / 格式 / 体积都达标，并用绝对路径告诉我放在哪",
    "原文件没有被覆盖（结果放在新文件夹里）",
    "「说明.md」写清了用的命令或工具、参数和处理了哪些文件",
];
const DONE_WRITING: [&str; 3] = [
    "成稿能直接发布：格式正确，字数和平台要求符合，并用绝对路径告诉我放在哪",
    "文字逐字核对过（尤其标题、品牌名、数字和引用），没有错别字，也没有编造的数据",
    "「说明.md」写清了用的工具、参数和资料来源",
];

const DONE_SCRAPE: [&str; 3] = [
    "采集到的条数和字段符合要求，抽查几条能点开原链接确认是真的，并用绝对路径告诉我结果放在哪",
    "没有用我的账号登录，也没有触发平台的限制或验证码（如果有，写清楚发生了什么）",
    "「说明.md」写清了采集范围、时间、用的工具和参数、怎么重新跑一遍",
];
const DONE_CONVERT: [&str; 3] = [
    "转换后的文件能正常打开，页数、表格、图片没有丢，抽查几处和原文件对得上，并用绝对路径告诉我放在哪",
    "原文件没有被覆盖或改动",
    "「说明.md」写清了用的工具、参数和转换了哪些文件",
];
const DONE_OPERATE: [&str; 3] = [
    "没有我的确认，就没有真的发出或回复任何东西；演练的结果（会发什么、发给谁、什么时候）写清楚给我看",
    "没有触发平台的限制或风控（如果有，写清楚发生了什么）",
    "「说明.md」写清了用的工具、参数、发布规则和怎么停掉它",
];
const DONE_ANALYSIS: [&str; 3] = [
    "每个结论都能对回数据（写明数据位置或来源链接），没有编造的数字",
    "表格 / 图表能直接打开，口径（时间范围、指标定义）写清楚，并用绝对路径告诉我放在哪",
    "「说明.md」写清了数据来源、处理步骤、用的工具和怎么更新",
];

/// 「完成标准」：永远 3 条，按任务类型换。
pub(crate) fn done_items(problem: &str) -> [&'static str; 3] {
    match task_kinds(problem).first() {
        Some(TaskKind::Video) => DONE_VIDEO,
        Some(TaskKind::Records) => DONE_RECORDS,
        Some(TaskKind::Batch) => DONE_BATCH,
        Some(TaskKind::Writing) => DONE_WRITING,
        Some(TaskKind::Scrape) => DONE_SCRAPE,
        Some(TaskKind::Convert) => DONE_CONVERT,
        Some(TaskKind::Operate) => DONE_OPERATE,
        Some(TaskKind::Analysis) => DONE_ANALYSIS,
        Some(TaskKind::Visual) | Some(TaskKind::General) | None => DONE_VISUAL,
    }
}

/// 没有合适的收藏、直接交给 Codex 时的三步。工具提示只看第一类任务，不写具体项目。
pub(crate) fn direct_steps(problem: &str) -> [String; 3] {
    let hint = match task_kinds(problem).first() {
        Some(TaskKind::Video) => "ffmpeg（剪辑、转码、烧字幕），macOS 自带的 afconvert 处理音频",
        Some(TaskKind::Records) | Some(TaskKind::Writing) => {
            "macOS 自带的 textutil 和 pandoc 可以转文档格式"
        }
        Some(TaskKind::Convert) => {
            "macOS 自带 textutil（文档）、sips（图片）、afconvert（音频），常见的还有 pandoc、soffice（LibreOffice）、ffmpeg"
        }
        Some(TaskKind::Scrape) => {
            "curl 和 python3 能抓公开网页；要做网页自动化，先看本机有没有 playwright"
        }
        Some(TaskKind::Analysis) => "macOS 自带 python3 和 sqlite3，处理表格和做统计够用",
        Some(TaskKind::Operate) => {
            "这类事会动你的账号，先别真的发：只写出「准备发什么、发给谁、什么时候」给我看"
        }
        Some(TaskKind::Batch) | Some(TaskKind::Visual) | Some(TaskKind::General) | None => {
            "macOS 自带的 sips 可以改图片尺寸和格式；常见的还有 magick（ImageMagick）、cwebp"
        }
    };
    [
        format!(
            "先查这台电脑上已经有什么：已装的 Skill 和 MCP（只看名字，不要读里面的密钥），以及命令行里现成的工具（{hint}）；用 which 查有没有。"
        ),
        "现成的够用就直接做：可以把几条现成命令串起来完成这一次的活，但不要开发一个新软件，也不要为这件事装来路不明的东西。".to_owned(),
        "不够用就停下来，告诉我缺什么、已经试过什么，我来决定换办法，还是去找新工具。".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_money_account_and_data_out_are_recognised() {
        let c = detect_cautions("sudo wx init", "");
        assert_eq!(c.ask, vec![Caution::Admin]);
        let c = detect_cautions("下载 .pkg 安装包", "");
        assert_eq!(c.ask, vec![Caution::Admin]);
        let c = detect_cautions("pip install x，配置 LLM API_KEY", "");
        assert_eq!(c.ask, vec![Caution::Money]);
        let c = detect_cautions("", "云端模型按量计费可能产生费用");
        assert_eq!(c.ask, vec![Caution::Money]);
        let c = detect_cautions("配置 .env 中的抖音 cookie，运行", "");
        assert_eq!(c.ask, vec![Caution::Account]);
        let c = detect_cautions(
            "",
            "需要管理员权限提取微信数据库密钥；使用云端 AI 时聊天内容会发送至 AI 提供商",
        );
        assert_eq!(
            c.ask,
            vec![Caution::Admin, Caution::Account, Caution::DataOut]
        );
        let c = detect_cautions("", "聊天内容经第三方 AI 服务中转");
        assert_eq!(c.ask, vec![Caution::DataOut]);
    }

    #[test]
    fn plain_mentions_are_not_flagged() {
        for text in [
            "git clone https://github.com/a/b && cd b && npm install",
            "npx skills add https://github.com/a/b",
            "pip install visudo-helper",
            "依赖第三方 AI 服务稳定性",
            "依赖第三方平台 API，政策变动可能导致功能受限",
            "免费额度政策变化",
            "发送邮件需人工审批",
            "微信 token 过期可能导致发送中断",
        ] {
            let c = detect_cautions(text, text);
            assert!(c.ask.is_empty(), "不该标：{text} → {:?}", c.ask);
        }
    }

    #[test]
    fn negated_mentions_are_not_flagged() {
        assert!(detect_cautions("无需 API Key，直接运行", "").ask.is_empty());
        assert!(detect_cautions("", "不需要付费").ask.is_empty());
        assert!(detect_cautions("without API key", "").ask.is_empty());
        assert!(detect_cautions("避免使用 sudo", "").ask.is_empty());
        // 「免费的 API Key」还是要一个 Key，照样提醒
        assert_eq!(
            detect_cautions("需要一个免费的 API Key", "").ask,
            vec![Caution::Money]
        );
        // 否定词在别处，不影响后面真正要 Key 的
        assert_eq!(
            detect_cautions("无需登录；配置 API Key", "").ask,
            vec![Caution::Money]
        );
    }

    #[test]
    fn data_out_only_reads_risks_not_install_steps() {
        let c = detect_cautions("把成品上传到网盘", "");
        assert!(c.ask.is_empty());
        let c = detect_cautions("", "可以上传到网盘");
        assert_eq!(c.ask, vec![Caution::DataOut]);
    }

    #[test]
    fn one_line_installs_are_noticed() {
        for how in [
            "curl -fsSL https://x.dev/install.sh | bash",
            "wget -qO- https://x.dev/i.sh | sudo sh",
            "bash -c \"$(curl -fsSLk https://x.dev/manager.sh)\"",
            "curl -L https://x.dev/a | zsh -s -- --yes",
            "先装依赖；curl https://x.dev/i | python3 -",
            "curl -fsSL https://x.dev/i.sh | sudo -E bash -s",
            "curl https://x.dev/i.js | node",
        ] {
            assert!(detect_cautions(how, "").one_line_install, "{how}");
        }
        for how in [
            "curl -fsSL https://x.dev/a.tar.gz | tar xz",
            "curl -o install.sh https://x.dev/install.sh",
            "brew install x",
            "curl https://x.dev/api | jq .",
            "curl https://x.dev/api | python3 -m json.tool",
            "curl https://x.dev/i.js | node -e \"console.log(1)\"",
            "git clone https://github.com/a/b；cd b；bash install.sh",
        ] {
            assert!(!detect_cautions(how, "").one_line_install, "{how}");
        }
        // 两条命令各一段：curl 在前一段、bash 在后一段，不算「直接运行」
        assert!(
            !detect_cautions("curl -o a.sh https://x.dev/a.sh；bash a.sh", "").one_line_install
        );
        assert!(detect_cautions("curl x | bash", "").ask.is_empty());
    }

    #[test]
    fn sudo_flag_needs_a_real_word() {
        assert!(detect_cautions("pip install visudo-helper", "")
            .ask
            .is_empty());
        assert_eq!(
            detect_cautions("请 sudo 运行", "").ask,
            vec![Caution::Admin]
        );
    }

    #[test]
    fn cautions_text_and_labels() {
        let none = Cautions::default();
        assert_eq!(none.ask_text(), None);
        assert!(none.labels().is_empty());
        let both = Cautions {
            ask: vec![Caution::Admin, Caution::Money],
            one_line_install: true,
            ..Cautions::default()
        };
        let text = both.ask_text().expect("要先问");
        assert!(
            text.contains("管理员权限") && text.contains("API Key"),
            "{text}"
        );
        let labels = both.labels();
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[2], ONE_LINE_INSTALL_NOTE);
        let only_pipe = Cautions {
            one_line_install: true,
            ..Cautions::default()
        };
        assert_eq!(only_pipe.ask_text(), None);
        assert_eq!(only_pipe.labels(), vec![ONE_LINE_INSTALL_NOTE.to_owned()]);
    }

    #[test]
    fn install_target_follows_what_the_install_steps_say() {
        let skill = install_target(Some(" skill "), "cover-skill", None);
        assert!(skill.contains("~/.agents/skills/cover-skill"));
        assert!(skill.contains("~/.claude/skills"));
        assert!(!skill.contains("npx skills add"));
        let npx = install_target(
            Some("skill"),
            "x",
            Some("npx skills add https://github.com/a/b"),
        );
        assert!(npx.contains("-g -y") && npx.contains("--help"), "{npx}");
        assert!(install_target(Some("mcp"), "x", None).contains("codex mcp add"));
        // 装法没写安装包就不编一个 .dmg
        let unknown_app = install_target(Some("app"), "x", None);
        assert!(
            unknown_app.contains("别猜安装包") && unknown_app.contains("点不了"),
            "{unknown_app}"
        );
        assert!(!unknown_app.contains(".dmg"), "{unknown_app}");
        let windows_only = install_target(Some("app"), "x", Some("winget install a.b"));
        assert!(!windows_only.contains(".dmg"), "{windows_only}");
        // 包名里带 download 不算「去下载安装包」
        let named_downloader = install_target(
            Some("app"),
            "x",
            Some("winget install aandrew-me.ytDownloader"),
        );
        assert!(!named_downloader.contains(".dmg"), "{named_downloader}");
        // 单独的「下载」不算装安装包：Fork 仓库、用 Actions 构建、下载打包结果
        let actions = install_target(
            Some("app"),
            "x",
            Some("Fork 本仓库，启用 GitHub Actions，手动触发构建工作流，下载打包结果后运行。"),
        );
        assert!(
            !actions.contains(".dmg") && actions.contains("别猜安装包"),
            "{actions}"
        );
        // Windows 安装包 + 源码装法：按源码装，不让人去拖 .dmg
        let windows_or_source = install_target(
            Some("app"),
            "x",
            Some("下载 Windows 安装包 (https://github.com/a/b/releases/latest) 安装，或 git clone https://github.com/a/b.git && ./start.sh"),
        );
        assert!(
            windows_or_source.contains("任务的目录") && !windows_or_source.contains(".dmg"),
            "{windows_or_source}"
        );
        // 同时给了 macOS 的包：仍然是 .dmg
        let both = install_target(
            Some("app"),
            "x",
            Some("从 Releases 下载桌面版安装包（macOS 选 .dmg，Windows 选 -setup.exe）"),
        );
        assert!(both.contains(".dmg"), "{both}");
        let real_download = install_target(Some("app"), "x", Some("Download the latest release"));
        assert!(real_download.contains(".dmg"), "{real_download}");
        assert!(install_target(Some("app"), "x", Some("下载安装包")).contains(".dmg"));
        assert!(!install_target(Some("app"), "x", Some("下载视频后运行")).contains(".dmg"));
        let released = install_target(Some("app"), "x", Some("从 Releases 下载 dmg 安装"));
        assert!(released.contains("应用程序"), "{released}");
        let docker_app = install_target(
            Some("app"),
            "x",
            Some("Docker 部署：git clone … && docker compose up"),
        );
        assert!(
            docker_app.contains("Docker") && !docker_app.contains(".dmg"),
            "{docker_app}"
        );
        assert!(
            docker_app.contains("网页界面") && docker_app.contains("点不了"),
            "{docker_app}"
        );
        assert!(!docker_app.contains("sudo docker"), "{docker_app}");
        let source_app = install_target(
            Some("app"),
            "x",
            Some("git clone https://x/y && cd y && pnpm install"),
        );
        assert!(
            source_app.contains("任务的目录") && !source_app.contains(".dmg"),
            "{source_app}"
        );
        let cli = install_target(Some("cli"), "x", None);
        assert!(cli.contains("独立环境") && !cli.contains("全局"));
        let node = install_target(
            Some("cli"),
            "x",
            Some("npm i -g cover-kit && cover-kit init"),
        );
        assert!(
            node.starts_with("Node 工具") && node.contains("不要 -g 全局装"),
            "{node}"
        );
        let python = install_target(Some("cli"), "x", Some("pip install foo && foo run"));
        assert!(
            python.starts_with("Python 工具") && python.contains("pipx"),
            "{python}"
        );
        let rust = install_target(Some("cli"), "x", Some("cargo install foo"));
        assert!(
            rust.contains("cargo") && rust.contains("不要用 sudo"),
            "{rust}"
        );
        let script_cli = install_target(
            Some("cli"),
            "x",
            Some("curl -fsSL https://x.dev/install.sh | sh && x run"),
        );
        assert!(script_cli.starts_with("装法是一行安装脚本"), "{script_cli}");
        assert!(!script_cli.contains("brew"), "{script_cli}");
        let docker_plain = install_target(Some("app"), "x", Some("docker compose up -d"));
        assert!(
            !docker_plain.contains("sudo"),
            "没有 sudo 就不提 sudo：{docker_plain}"
        );
        let brew_cli = install_target(Some("cli"), "x", Some("brew install foo"));
        assert!(brew_cli.starts_with("按装法里的 brew 命令装"), "{brew_cli}");
        let mcp_docker = install_target(Some("mcp"), "x", Some("docker pull a/b"));
        assert!(
            mcp_docker.contains("Docker") && mcp_docker.contains("codex mcp add"),
            "{mcp_docker}"
        );
        let already_global = install_target(
            Some("skill"),
            "x",
            Some("npx skills add https://github.com/a/b -g -y"),
        );
        assert!(
            !already_global.contains("npx skills add"),
            "{already_global}"
        );
        assert!(install_target(Some("cli"), "x", Some("docker run x")).contains("Docker"));
        let cask = install_target(Some("app"), "x", Some("brew install --cask a/tap/x"));
        assert!(
            cask.contains("brew 命令") && !cask.contains(".dmg"),
            "{cask}"
        );
        assert!(cask.contains("点不了"), "{cask}");
        assert!(install_target(Some("library"), "x", None).contains("venv"));
        assert!(install_target(Some("library"), "x", Some("pip install foo")).contains("venv"));
        assert!(install_target(
            Some("library"),
            "x",
            Some("conda create -n a && pip install b")
        )
        .contains("venv"));
        assert!(
            install_target(Some("library"), "x", Some("npx @a/b report.docx"))
                .starts_with("Node 工具")
        );
        assert!(install_target(
            Some("library"),
            "x",
            Some("通过两个 script 标签引入页面使用")
        )
        .starts_with("前端库"));
        let hosted = install_target(
            Some("library"),
            "x",
            Some("访问 https://x.dev 在线体验，或通过某个 Skills 集成"),
        );
        assert!(
            hosted.contains("这次任务的目录") && !hosted.contains("系统 Python"),
            "{hosted}"
        );
        assert!(install_target(Some("service"), "x", None).contains("Docker"));
        assert!(install_target(Some("service"), "x", Some("docker compose up")).contains("Docker"));
        assert!(install_target(Some("service"), "x", Some("npm start")).contains("任务的目录"));
        assert!(install_target(None, "x", None).contains("独立的新目录"));
        assert!(install_target(Some("weird"), "x", None).contains("独立的新目录"));
        assert!(install_target(Some("Skill"), "x", None).contains("独立的新目录"));
        assert!(install_target(None, "x", Some("docker run x")).contains("Docker"));
    }

    #[test]
    fn task_kind_follows_the_object_being_made() {
        assert_eq!(task_kinds("给短视频加字幕"), vec![TaskKind::Video]);
        assert_eq!(task_kinds("把群聊记录整理成日报"), vec![TaskKind::Records]);
        assert_eq!(task_kinds("批量改图尺寸和压缩"), vec![TaskKind::Batch]);
        assert_eq!(task_kinds("做小红书封面"), vec![TaskKind::Visual]);
        assert_eq!(task_kinds("做电商商品详情页"), vec![TaskKind::Visual]);
        assert_eq!(
            task_kinds("公众号文章配图"),
            vec![TaskKind::Visual, TaskKind::Writing]
        );
        assert_eq!(task_kinds("随便做点什么"), Vec::<TaskKind>::new());
        assert_eq!(task_kinds(""), Vec::<TaskKind>::new());
        assert_eq!(task_kinds("Make a LOGO"), vec![TaskKind::Visual]);
        assert_eq!(task_kinds("把会议录音转成文字稿"), vec![TaskKind::Records]);
        assert_eq!(task_kinds("写周报"), vec![TaskKind::Records]);
    }

    #[test]
    fn what_to_do_beats_what_it_is_done_to() {
        // 动作优先：抓取视频不是视频任务，发布笔记不是图文任务
        assert_eq!(
            task_kinds("抓取抖音热门视频"),
            vec![TaskKind::Scrape, TaskKind::Video]
        );
        assert_eq!(
            task_kinds("批量发布小红书笔记"),
            vec![TaskKind::Operate, TaskKind::Visual]
        );
        assert_eq!(task_kinds("自动回复私信"), vec![TaskKind::Operate]);
        assert_eq!(task_kinds("把PDF转成Word"), vec![TaskKind::Convert]);
        assert_eq!(
            task_kinds("把图片转成webp"),
            vec![TaskKind::Convert, TaskKind::Visual]
        );
        assert_eq!(task_kinds("竞品分析报告"), vec![TaskKind::Analysis]);
        assert_eq!(task_kinds("做数据看板"), vec![TaskKind::Analysis]);
        assert_eq!(task_kinds("做用户画像"), vec![TaskKind::Analysis]);
        assert_eq!(
            task_kinds("监控竞品价格"),
            vec![TaskKind::Scrape, TaskKind::Analysis]
        );
        assert_eq!(
            task_kinds("采集竞品评论"),
            vec![TaskKind::Scrape, TaskKind::Analysis]
        );
        // 「批量」类任务自己是动作，不被「图片」带成图文
        assert_eq!(
            task_kinds("批量去除图片水印"),
            vec![TaskKind::Batch, TaskKind::Visual]
        );
    }

    #[test]
    fn new_task_kinds_get_their_own_questions_and_done_lists() {
        let scrape = ask_items("抓取抖音热门视频", &[]);
        assert_eq!(scrape.len(), 5, "抓取不混视频的问题：{scrape:?}");
        assert!(scrape[0].contains("要采集什么"), "{scrape:?}");
        assert!(
            scrape.iter().any(|item| item.contains("Cookie")),
            "{scrape:?}"
        );
        assert!(
            !scrape.iter().any(|item| item.contains("字幕")),
            "{scrape:?}"
        );
        assert!(done_items("抓取抖音热门视频")[0].contains("抽查"));
        let convert = ask_items("把PDF转成Word", &[]);
        assert!(convert[0].contains("要转换的文件"), "{convert:?}");
        assert!(
            !convert.iter().any(|item| item.contains("品牌")),
            "{convert:?}"
        );
        assert!(done_items("把PDF转成Word")[1].contains("原文件没有被覆盖"));
        let operate = ask_items("批量发布小红书笔记", &[]);
        assert_eq!(operate.len(), 5, "{operate:?}");
        assert!(operate[0].contains("哪个平台"), "{operate:?}");
        assert!(
            operate.iter().any(|item| item.contains("逐条确认")),
            "{operate:?}"
        );
        assert!(done_items("批量发布小红书笔记")[0].contains("没有我的确认"));
        let analysis = ask_items("竞品分析报告", &[]);
        assert!(analysis[0].contains("要回答什么问题"), "{analysis:?}");
        assert!(done_items("竞品分析报告")[0].contains("编造"));
        // 批量处理不被图文问题打扰；水印不是「要加的」
        let batch = ask_items("批量去除图片水印", &[]);
        assert_eq!(batch.len(), 5, "{batch:?}");
        assert!(
            !batch.iter().any(|item| item.contains("品牌名的写法")),
            "{batch:?}"
        );
        assert!(
            !batch.iter().any(|item| item.contains("要加的水印")),
            "{batch:?}"
        );
        assert!(
            batch.iter().any(|item| item.contains("宽高比")),
            "{batch:?}"
        );
    }

    #[test]
    fn only_content_tasks_mix_their_questions() {
        assert!(may_mix(TaskKind::Visual, TaskKind::Writing));
        assert!(may_mix(TaskKind::Writing, TaskKind::Records));
        assert!(may_mix(TaskKind::Visual, TaskKind::Batch));
        assert!(may_mix(TaskKind::Video, TaskKind::Visual));
        assert!(!may_mix(TaskKind::Batch, TaskKind::Visual));
        assert!(!may_mix(TaskKind::Scrape, TaskKind::Video));
        assert!(!may_mix(TaskKind::Operate, TaskKind::Visual));
        assert!(!may_mix(TaskKind::Analysis, TaskKind::Records));
        assert!(!may_mix(TaskKind::Visual, TaskKind::Visual));
        assert!(!may_mix(TaskKind::Visual, TaskKind::Analysis));
    }

    #[test]
    fn ask_items_switch_with_the_task_and_stay_visual_for_covers() {
        let visual = ask_items("做小红书封面", &[]);
        assert_eq!(
            visual,
            ASK_VISUAL.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );
        let video = ask_items("给短视频加字幕", &[]);
        assert_eq!(video.len(), 5);
        assert!(video[0].contains("视频"), "{video:?}");
        assert!(video.iter().any(|item| item.contains("srt")), "{video:?}");
        assert!(!video.iter().any(|item| item.contains("色板")), "{video:?}");
        let records = ask_items("把群聊记录整理成日报", &[]);
        assert!(
            records.iter().any(|item| item.contains("第三方 AI 服务")),
            "{records:?}"
        );
        assert!(
            !records.iter().any(|item| item.contains("logo")),
            "{records:?}"
        );
        let batch = ask_items("批量改图尺寸和压缩", &[]);
        assert!(batch.iter().any(|item| item.contains("体积")), "{batch:?}");
        let general = ask_items("随便做点什么", &[]);
        assert_eq!(general.len(), 5);
        assert!(general[0].contains("主题"));
    }

    #[test]
    fn ask_items_mix_in_the_second_kind_and_list_flagged_tools() {
        let mixed = ask_items("公众号文章配图", &[]);
        assert_eq!(mixed.len(), 6);
        assert_eq!(mixed[0], ASK_VISUAL[0]);
        assert_eq!(mixed[1], ASK_WRITING[0]);
        let cautions = Cautions {
            ask: vec![Caution::Admin, Caution::DataOut],
            ..Cautions::default()
        };
        let quiet = Cautions::default();
        let listed = ask_items(
            "做小红书封面",
            &[
                Flagged {
                    name: "a/one",
                    cautions: &cautions,
                },
                Flagged {
                    name: "b/two",
                    cautions: &quiet,
                },
            ],
        );
        assert_eq!(listed.len(), 6);
        let last = listed.last().expect("最后一条");
        assert!(
            last.contains("a/one（要管理员权限、内容可能外发或涉及隐私数据）"),
            "{last}"
        );
        assert!(!last.contains("b/two"), "{last}");
    }

    #[test]
    fn done_items_are_always_three_and_follow_the_task() {
        for problem in [
            "做小红书封面",
            "给短视频加字幕",
            "把群聊记录整理成日报",
            "批量改图尺寸和压缩",
            "写爆款标题",
            "随便",
        ] {
            assert_eq!(done_items(problem).len(), 3);
        }
        assert_eq!(done_items("做小红书封面"), DONE_VISUAL);
        assert!(done_items("给短视频加字幕")[1].contains("原视频"));
        assert!(done_items("把群聊记录整理成日报")[1].contains("隐私"));
        assert!(done_items("批量改图尺寸和压缩")[0].contains("数量"));
        assert!(done_items("写爆款标题")[1].contains("逐字核对"));
    }

    #[test]
    fn direct_steps_pick_a_hint_and_keep_the_other_two_lines() {
        let batch = direct_steps("批量改图尺寸和压缩");
        assert!(batch[0].contains("sips"), "{}", batch[0]);
        assert!(batch[0].contains("magick"), "{}", batch[0]);
        assert!(batch[0].contains("cwebp"), "{}", batch[0]);
        let video = direct_steps("给短视频加字幕");
        assert!(video[0].contains("ffmpeg"), "{}", video[0]);
        assert!(video[0].contains("afconvert"), "{}", video[0]);
        assert!(!video[0].contains("sips"), "{}", video[0]);
        let records = direct_steps("把群聊记录整理成日报");
        assert!(records[0].contains("textutil"), "{}", records[0]);
        assert!(records[0].contains("pandoc"), "{}", records[0]);
        let writing = direct_steps("写爆款标题");
        assert!(writing[0].contains("pandoc"), "{}", writing[0]);
        let visual = direct_steps("做小红书封面");
        assert!(visual[0].contains("sips"), "{}", visual[0]);
        let unknown = direct_steps("随便做点什么");
        assert!(unknown[0].contains("sips"), "{}", unknown[0]);
        let convert = direct_steps("把PDF转成Word");
        assert!(
            convert[0].contains("soffice") && convert[0].contains("textutil"),
            "{}",
            convert[0]
        );
        let scrape = direct_steps("抓取抖音热门视频");
        assert!(
            scrape[0].contains("curl") && scrape[0].contains("playwright"),
            "{}",
            scrape[0]
        );
        let analysis = direct_steps("竞品分析报告");
        assert!(analysis[0].contains("sqlite3"), "{}", analysis[0]);
        let operate = direct_steps("批量发布小红书笔记");
        assert!(operate[0].contains("先别真的发"), "{}", operate[0]);
        for steps in [
            &batch, &video, &records, &writing, &visual, &unknown, &convert, &scrape, &analysis,
            &operate,
        ] {
            assert_eq!(
                steps[1],
                "现成的够用就直接做：可以把几条现成命令串起来完成这一次的活，但不要开发一个新软件，也不要为这件事装来路不明的东西。"
            );
            assert_eq!(
                steps[2],
                "不够用就停下来，告诉我缺什么、已经试过什么，我来决定换办法，还是去找新工具。"
            );
            assert!(steps[0].starts_with("先查这台电脑上已经有什么"));
            assert!(steps[0].contains("用 which 查有没有。"));
        }
    }

    #[test]
    fn sudo_in_front_of_docker_or_apt_is_not_a_mac_admin_request() {
        let c = detect_cautions("sudo docker run -d --name crawler -p 8080:8080 x/y", "");
        assert!(c.ask.is_empty(), "{:?}", c.ask);
        assert!(c.sudo_docker);
        assert_eq!(c.foreign, None, "docker 是跨平台写法");
        assert!(c.notes().contains(&SUDO_DOCKER_NOTE));
        let c = detect_cautions("sudo apt install ./hermesoffice_1.0_amd64.deb", "");
        assert!(c.ask.is_empty(), "{:?}", c.ask);
        assert_eq!(c.foreign, Some(ForeignOs::Linux));
        assert!(c.notes().contains(&LINUX_NOTE));
        let c = detect_cautions("sudo dnf install ./x.rpm", "");
        assert!(c.ask.is_empty());
        assert_eq!(c.foreign, Some(ForeignOs::Linux));
        // 真正要管理员权限的 sudo 仍然要问
        let c = detect_cautions(
            "git clone https://x/y && cd y && cargo build && sudo wx init",
            "",
        );
        assert_eq!(c.ask, vec![Caution::Admin]);
        assert_eq!(c.foreign, None);
        let c = detect_cautions("sudo docker run x；sudo ./install.sh", "");
        assert_eq!(c.ask, vec![Caution::Admin]);
        assert!(c.sudo_docker);
    }

    #[test]
    fn instructions_for_another_system_are_noticed_only_without_a_cross_platform_way() {
        let c = detect_cautions("winget install aandrew-me.ytDownloader", "");
        assert_eq!(c.foreign, Some(ForeignOs::Windows));
        assert!(c.notes().contains(&WINDOWS_NOTE));
        assert_eq!(
            detect_cautions("下载 Windows 安装包安装", "").foreign,
            Some(ForeignOs::Windows)
        );
        assert_eq!(
            detect_cautions("scoop bucket add extras && scoop install navop", "").foreign,
            Some(ForeignOs::Windows)
        );
        for how in [
            "下载 Windows 安装包 (https://x/releases/latest) 安装，或 git clone https://x/y.git && ./start.sh",
            "从 Releases 下载桌面版安装包（macOS 选 .dmg，Windows 选 -setup.exe）",
            "brew install x；或 winget install x",
            "docker compose up -d",
            "下载 Release 中的 zip 包解压；或 Windows 下执行 choco install x",
            "npm i -g x",
            "pip install x；Windows 也可以用 .exe",
            "",
        ] {
            assert_eq!(detect_cautions(how, "").foreign, None, "{how}");
        }
    }

    #[test]
    fn unsigned_mac_apps_get_a_note_not_a_question() {
        for text in [
            "应用未签名，macOS 首次打开需手动解除限制",
            "macOS 版本未签名，需手动绕过 Gatekeeper",
            "首次打开需手动放行",
        ] {
            let c = detect_cautions("", text);
            assert!(c.unsigned, "{text}");
            assert!(c.ask.is_empty(), "{text} → {:?}", c.ask);
            assert!(c.notes().contains(&UNSIGNED_NOTE), "{text}");
        }
        assert!(!detect_cautions("brew install x", "依赖外部 yt-dlp 更新").unsigned);
    }

    #[test]
    fn cost_account_ban_and_hosted_ocr_are_recognised_from_real_card_wording() {
        let c = detect_cautions("", "依赖第三方 AI API 的可用性和调用成本");
        assert_eq!(c.ask, vec![Caution::Money]);
        let c = detect_cautions("", "依赖外部图片 API 和写作模型，成本不可控");
        assert_eq!(c.ask, vec![Caution::Money]);
        let c = detect_cautions("", "闲鱼账号可能因频繁访问被风控");
        assert_eq!(c.ask, vec![Caution::Account]);
        let c = detect_cautions("", "账号或IP被封禁风险");
        assert_eq!(c.ask, vec![Caution::Account]);
        let c = detect_cautions("", "自托管需自行维护 OAuth 凭证安全");
        assert_eq!(c.ask, vec![Caution::Account]);
        let c = detect_cautions("", "扫描版 PDF 需要依赖 Firecrawl 的托管 OCR 服务");
        assert_eq!(c.ask, vec![Caution::DataOut]);
        // 「自托管」不是外发
        assert!(detect_cautions("", "需要自托管服务器").ask.is_empty());
    }

    #[test]
    fn publishing_to_real_accounts_is_a_separate_question() {
        let c = detect_cautions("", "推送草稿箱需要明确授权，存在误发风险");
        assert_eq!(c.ask, vec![Caution::Publish]);
        let c = detect_cautions("", "自动评论和私信可能触发平台风控");
        assert_eq!(c.ask, vec![Caution::Account, Caution::Publish]);
        let text = c.ask_text().expect("要先问");
        assert!(
            text.contains("只演练") && text.contains("每一条都要我确认"),
            "{text}"
        );
        assert!(detect_cautions("", "发送邮件需人工审批").ask.is_empty());
    }

    #[test]
    fn platforms_without_macos_are_a_question() {
        let platforms =
            |items: &[&str]| -> Vec<String> { items.iter().map(|s| (*s).to_owned()).collect() };
        for unsupported in [
            vec!["windows"],
            vec!["windows", "linux"],
            vec!["Linux"],
            vec!["android", "ios"],
        ] {
            let c = Cautions::default().with_platforms(&platforms(&unsupported));
            assert_eq!(c.ask, vec![Caution::Platform], "{unsupported:?}");
        }
        for fine in [
            vec![],
            vec!["cross"],
            vec!["macos"],
            vec!["macos", "windows"],
            vec!["web"],
            vec!["linux", "web"],
            vec!["Cross", " MacOS "],
            vec!["", "  "],
        ] {
            let c = Cautions::default().with_platforms(&platforms(&fine));
            assert!(c.ask.is_empty(), "{fine:?}");
        }
        // 不重复，也不吃掉别的
        let c = detect_cautions("", "需要付费").with_platforms(&platforms(&["windows"]));
        assert_eq!(c.ask, vec![Caution::Money, Caution::Platform]);
        let text = c.ask_text().expect("要先问");
        assert!(
            text.contains("不支持 macOS") && text.contains("先别装"),
            "{text}"
        );
    }

    #[test]
    fn notes_come_in_a_fixed_order_and_join_the_json_labels() {
        let all = Cautions {
            ask: vec![Caution::Admin],
            one_line_install: true,
            unsigned: true,
            foreign: Some(ForeignOs::Windows),
            sudo_docker: true,
        };
        assert_eq!(
            all.notes(),
            vec![
                ONE_LINE_INSTALL_NOTE,
                WINDOWS_NOTE,
                SUDO_DOCKER_NOTE,
                UNSIGNED_NOTE
            ]
        );
        let labels = all.labels();
        assert_eq!(labels.len(), 5);
        assert_eq!(labels[0], Caution::Admin.label());
        assert_eq!(labels[4], UNSIGNED_NOTE);
        assert!(Cautions::default().notes().is_empty());
        // 只有提醒、没有要问的：汇总那条不列它
        let only_notes = Cautions {
            unsigned: true,
            ..Cautions::default()
        };
        assert_eq!(only_notes.ask_text(), None);
        assert_eq!(only_notes.ask_short_text(), None);
    }

    #[test]
    fn pack_text_avoids_the_words_the_pack_tests_forbid() {
        let mut all: Vec<String> = Vec::new();
        for set in [
            ASK_VISUAL,
            ASK_VIDEO,
            ASK_RECORDS,
            ASK_BATCH,
            ASK_WRITING,
            ASK_SCRAPE,
            ASK_CONVERT,
            ASK_OPERATE,
            ASK_ANALYSIS,
            ASK_GENERAL,
        ] {
            all.extend(set.iter().map(|s| (*s).to_owned()));
        }
        for set in [
            DONE_VISUAL,
            DONE_VIDEO,
            DONE_RECORDS,
            DONE_BATCH,
            DONE_WRITING,
            DONE_SCRAPE,
            DONE_CONVERT,
            DONE_OPERATE,
            DONE_ANALYSIS,
        ] {
            all.extend(set.iter().map(|s| (*s).to_owned()));
        }
        for note in [
            ONE_LINE_INSTALL_NOTE,
            UNSIGNED_NOTE,
            LINUX_NOTE,
            WINDOWS_NOTE,
            SUDO_DOCKER_NOTE,
            DOCKER_TARGET,
            APP_NOTE,
            APP_NOTE_WEB,
            APP_UNKNOWN,
            NODE_TARGET,
            PYTHON_TARGET,
            RUST_TARGET,
        ] {
            all.push(note.to_owned());
        }
        for caution in [
            Caution::Admin,
            Caution::Money,
            Caution::Account,
            Caution::DataOut,
            Caution::Publish,
            Caution::Platform,
        ] {
            all.push(caution.label().to_owned());
            all.push(caution.short().to_owned());
        }
        for text in all {
            assert!(
                !text.contains("参考包") && !text.contains("参考项目"),
                "{text}"
            );
        }
    }
}
