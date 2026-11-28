use serde::Serialize;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// 从聊天、帖子里抽出来的一条链接。
///
/// `url` 是清理后的原始链接（缺协议的补上 https://，粘在后面的中文和标点已去掉）；
/// `normalized_url` 只用来去重和入库：小写主机、去跟踪参数、twitter 并入 x、GitHub 截到仓库级。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedLink {
    pub url: String,
    pub normalized_url: String,
    /// github_repo / github_skill / github / x_post / x / web
    pub kind: String,
    /// github / x / wechat / xiaohongshu / bilibili / youtube / telegram / discord / web
    pub platform: String,
    pub owner: Option<String>,
    pub name: Option<String>,
    /// 有值说明这条链接不会被收进去（本机或内网地址），预览时就该告诉用户。
    pub blocked_reason: Option<String>,
}

/// 聊天里常见的写法：URL 后面直接跟中文、全角标点，没有空格。
/// 所以只吃 ASCII 的 URL 字符，遇到中文、全角标点、引号、尖括号就停。
fn is_url_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || "-._~:/?#[]@!$&()*+,;=%".contains(ch)
}

/// 一条链接最多这么长。浏览器和主流服务器的上限也是 2K 左右。
const MAX_LINK_CHARS: usize = 2048;

/// 一段文字最多认出这么多条链接。本地接口一次请求可以带 64 KB 文字，最多能塞进六千多条链接，
/// 每一条都会变成一个要联网去读的收集箱条目。
const MAX_LINKS_PER_TEXT: usize = 300;

/// 没写 https:// 也认的几个站（X 和 GitHub 是主要来源，公众号文章偶尔也缺协议）。
const BARE_HOSTS: [&str; 4] = ["github.com/", "x.com/", "twitter.com/", "mp.weixin.qq.com/"];

fn starts_with_ci(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn starts_url(rest: &str, previous: Option<char>) -> bool {
    let after_word = previous.is_some_and(|ch| ch.is_ascii_alphanumeric());
    if starts_with_ci(rest, "https://") || starts_with_ci(rest, "http://") {
        return !after_word;
    }
    let inside_token = previous.is_some_and(|ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '@' | '/' | ':' | '_')
    });
    !inside_token && BARE_HOSTS.iter().any(|host| starts_with_ci(rest, host))
}

/// 句子标点和 Markdown 加粗会粘在链接末尾；括号只在不成对时才剥。
/// 不剥下划线：真实仓库名可以以 `_` 结尾。
fn trim_trailing(raw: &str) -> &str {
    // 括号数只数一次，剥掉一个就减一个：每剥一个字符都重数整段，一串 4 万个 `)` 要 3.6 秒（二次方）。
    let open_paren = raw.matches('(').count();
    let mut close_paren = raw.matches(')').count();
    let open_bracket = raw.matches('[').count();
    let mut close_bracket = raw.matches(']').count();
    let mut end = raw.len();
    while let Some(last) = raw[..end].chars().next_back() {
        let strip = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '*' => true,
            ')' => open_paren < close_paren,
            ']' => open_bracket < close_bracket,
            _ => false,
        };
        if !strip {
            break;
        }
        match last {
            ')' => close_paren -= 1,
            ']' => close_bracket -= 1,
            _ => {}
        }
        end -= last.len_utf8();
    }
    &raw[..end]
}

fn candidates(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut skip_until = 0;
    let mut previous: Option<char> = None;
    for (index, ch) in text.char_indices() {
        if index >= skip_until {
            let rest = &text[index..];
            if starts_url(rest, previous) {
                let length = rest
                    .char_indices()
                    .find(|(_, candidate)| !is_url_char(*candidate))
                    .map(|(offset, _)| offset)
                    .unwrap_or(rest.len());
                let raw = trim_trailing(&rest[..length]);
                // 太长的不是正常链接（一条 5 万字符的「链接」会绕过后面每一处长度限制）
                if (8..=MAX_LINK_CHARS).contains(&raw.len()) {
                    found.push(raw.to_owned());
                }
                skip_until = index + length;
            }
        }
        previous = Some(ch);
    }
    found
}

pub fn extract_links_from_text(text: &str) -> Vec<ExtractedLink> {
    extract_links_limited(text, MAX_LINKS_PER_TEXT)
}

pub(crate) const MAX_SOURCE_LINKS: usize = 100;
pub(crate) const SOURCE_TEXT_LIMIT_BYTES: usize = 64 * 1024;

/// 新收集流程保留具体 Skill 文件。旧预览与投递仍按仓库去重。
pub(crate) fn extract_source_links(text: &str) -> Vec<ExtractedLink> {
    unique_links(text, MAX_SOURCE_LINKS, build_source_link)
}

fn build_source_link(raw: &str) -> Option<ExtractedLink> {
    let mut link = build_link(raw)?;
    if link.blocked_reason.is_some() {
        return Some(link);
    }
    let parsed = url::Url::parse(&link.url).ok()?;
    let segments: Vec<_> = parsed
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    let host = parsed.host_str()?;
    if host.eq_ignore_ascii_case("raw.githubusercontent.com")
        && segments.len() >= 4
        && valid_github_name(segments[0])
        && valid_github_name(segments[1])
        && segments
            .last()
            .is_some_and(|last| last.eq_ignore_ascii_case("skill.md"))
    {
        link.kind = "github_skill".to_owned();
        link.platform = "github".to_owned();
        link.owner = Some(segments[0].to_owned());
        link.name = Some(segments[1].to_owned());
        link.normalized_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            segments[0].to_ascii_lowercase(),
            segments[1].to_ascii_lowercase(),
            segments[2..].join("/")
        );
    } else if link.kind == "github_repo" && segments.len() >= 5 && !segments[3].is_empty() {
        let file = segments[2] == "blob"
            && segments
                .last()
                .is_some_and(|last| last.eq_ignore_ascii_case("skill.md"));
        let directory = segments[2] == "tree"
            && segments[4..].iter().enumerate().any(|(index, part)| {
                (part.eq_ignore_ascii_case("skill") || part.eq_ignore_ascii_case("skills"))
                    && index + 1 < segments[4..].len()
            });
        if file || directory {
            link.kind = "github_skill".to_owned();
            link.normalized_url.push('/');
            link.normalized_url.push_str(&segments[2..].join("/"));
        }
    }
    Some(link)
}

/// 结构化来源必须是一条完整地址，避免把带正文的字段当成地址截断。
pub(crate) fn source_url_link(url: &str) -> Option<ExtractedLink> {
    let url = url.trim();
    if url.len() > MAX_LINK_CHARS || !url.chars().all(is_url_char) {
        return None;
    }
    if !starts_url(url, None) {
        return None;
    }
    build_source_link(url).filter(|link| link.blocked_reason.is_none())
}

/// 作为来源页本身传入的独立地址（如 GitHub Issue / Discussion / PR）：保留子页面路径，避免同仓不同帖互相覆盖。
pub(crate) fn source_origin_link(url: &str) -> Option<ExtractedLink> {
    let mut link = source_url_link(url)?;
    if link.kind == "github_repo" {
        let mut parsed = url::Url::parse(&link.url).ok()?;
        let segments: Vec<String> = parsed
            .path_segments()
            .map(|parts| {
                parts
                    .filter(|part| !part.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if segments.len() > 2 {
            let owner = segments[0].to_ascii_lowercase();
            let repo = segments[1].trim_end_matches(".git").to_ascii_lowercase();
            let rest = segments[2..].join("/");
            parsed.set_path(&format!("/{owner}/{repo}/{rest}"));
            link.normalized_url = normalize_web(&mut parsed, "github.com");
            link.kind = "github".to_owned();
        }
    }
    Some(link)
}

pub(crate) fn normalized_x_post_id(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://x.com/")?;
    let (user, id) = rest.split_once("/status/")?;
    (!user.is_empty()
        && !user.contains('/')
        && !id.is_empty()
        && id.bytes().all(|b| b.is_ascii_digit()))
    .then_some(id)
}

fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

/// 读网页正文用：按出现顺序找 GitHub 仓库链接，`accept` 决定这一条算不算（调用方自己的校验和去重），
/// 够 `max` 条就停。不先把整页的链接都建出来：一个 6 MB 的页面可以有四十多万条链接（实测 110 MB 内存），
/// 而文章里提取仓库只要前 30 个；也不受聊天粘贴的「最多 300 条链接」限制，
/// 排在前面的几百个网站链接不能把后面的仓库挤掉。
pub(crate) fn find_github_repo_links(
    text: &str,
    max: usize,
    mut accept: impl FnMut(&ExtractedLink) -> bool,
) -> Vec<ExtractedLink> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for raw in candidates(text) {
        if found.len() >= max {
            break;
        }
        // 绝大多数链接不是 GitHub 的：不含 github.com 的不必建链接（解析 URL、查表都要钱）
        if !contains_ignore_case(&raw, "github.com") {
            continue;
        }
        let Some(link) = build_link(&raw) else {
            continue;
        };
        if link.kind != "github_repo" || !seen.insert(link.normalized_url.clone()) {
            continue;
        }
        if accept(&link) {
            found.push(link);
        }
    }
    found
}

fn extract_links_limited(text: &str, limit: usize) -> Vec<ExtractedLink> {
    unique_links(text, limit, build_link)
}

fn unique_links(
    text: &str,
    limit: usize,
    build: impl Fn(&str) -> Option<ExtractedLink>,
) -> Vec<ExtractedLink> {
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    for raw in candidates(text) {
        if links.len() >= limit {
            break;
        }
        let Some(link) = build(&raw) else {
            continue;
        };
        if seen.insert(link.normalized_url.clone()) {
            links.push(link);
        }
    }
    links
}

/// GitHub 路径第一段不是用户名的情况，这些不是仓库。
const GITHUB_RESERVED: [&str; 38] = [
    "about",
    "account",
    "apps",
    "blog",
    "codespaces",
    "collections",
    "contact",
    "copilot",
    "customer-stories",
    "discussions",
    "enterprise",
    "explore",
    "features",
    "git-guides",
    "issues",
    "join",
    "login",
    "logout",
    "marketplace",
    "new",
    "notifications",
    "orgs",
    "organizations",
    "password_reset",
    "pricing",
    "pulls",
    "readme",
    "resources",
    "search",
    "security",
    "sessions",
    "settings",
    "signup",
    "sitemap",
    "solutions",
    "sponsors",
    "stars",
    "topics",
];

fn valid_github_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn build_link(raw: &str) -> Option<ExtractedLink> {
    let with_scheme = if starts_with_ci(raw, "http://") || starts_with_ci(raw, "https://") {
        raw.to_owned()
    } else {
        format!("https://{raw}")
    };
    let mut parsed = url::Url::parse(&with_scheme).ok()?;
    let host = canonical_host(parsed.host_str()?);
    let segments: Vec<String> = parsed
        .path_segments()
        .map(|parts| {
            parts
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let blocked_reason = url_is_safe(&with_scheme).err();

    let mut link = ExtractedLink {
        url: with_scheme.clone(),
        normalized_url: String::new(),
        kind: "web".to_owned(),
        platform: platform_for(&host).to_owned(),
        owner: None,
        name: None,
        blocked_reason,
    };

    match host.as_str() {
        "github.com" => {
            let first = segments.first()?;
            let reserved = GITHUB_RESERVED.contains(&first.to_ascii_lowercase().as_str());
            let name = segments
                .get(1)
                .map(|value| value.trim_end_matches(".git").to_owned());
            match name {
                Some(name) if !reserved && valid_github_name(first) && valid_github_name(&name) => {
                    link.normalized_url = format!(
                        "https://github.com/{}/{}",
                        first.to_ascii_lowercase(),
                        name.to_ascii_lowercase()
                    );
                    link.kind = "github_repo".to_owned();
                    link.owner = Some(first.clone());
                    link.name = Some(name);
                }
                _ if !reserved && valid_github_name(first) => {
                    link.normalized_url =
                        format!("https://github.com/{}", first.to_ascii_lowercase());
                    link.kind = "github".to_owned();
                    link.owner = Some(first.clone());
                }
                _ => {
                    link.normalized_url = format!(
                        "https://github.com/{}",
                        segments.join("/").to_ascii_lowercase()
                    );
                    link.kind = "github".to_owned();
                }
            }
        }
        "x.com" => {
            let status_id = segments
                .iter()
                .position(|part| part == "status" || part == "statuses")
                .and_then(|index| segments.get(index + 1))
                .filter(|id| !id.is_empty() && id.chars().all(|ch| ch.is_ascii_digit()));
            match status_id {
                Some(id) => {
                    let user = segments
                        .first()
                        .map(|value| value.to_ascii_lowercase())
                        .unwrap_or_default();
                    link.normalized_url = format!("https://x.com/{user}/status/{id}");
                    link.kind = "x_post".to_owned();
                }
                None => {
                    link.normalized_url = format!("https://x.com/{}", segments.join("/"));
                    link.kind = "x".to_owned();
                }
            }
        }
        _ => {
            link.normalized_url = normalize_web(&mut parsed, &host);
        }
    }
    Some(link)
}

fn canonical_host(host: &str) -> String {
    let lower = host.to_ascii_lowercase();
    let stripped = lower.strip_prefix("www.").unwrap_or(&lower);
    match stripped {
        "twitter.com" | "mobile.twitter.com" | "m.twitter.com" | "mobile.x.com" => {
            "x.com".to_owned()
        }
        other => other.to_owned(),
    }
}

fn platform_for(host: &str) -> &'static str {
    match host {
        "github.com" => "github",
        "x.com" => "x",
        "mp.weixin.qq.com" => "wechat",
        "xiaohongshu.com" | "xhslink.com" => "xiaohongshu",
        "bilibili.com" | "b23.tv" => "bilibili",
        "youtube.com" | "m.youtube.com" | "youtu.be" => "youtube",
        "t.me" => "telegram",
        "discord.com" | "discord.gg" => "discord",
        _ => "web",
    }
}

/// 只去掉不影响内容身份的跟踪参数。公众号长链接里 `__biz/mid/idx/sn` 才是身份，必须留着。
fn keep_query_pair(host: &str, path: &str, key: &str) -> bool {
    if host == "mp.weixin.qq.com" {
        if path.starts_with("/s/") {
            return false;
        }
        return matches!(key, "__biz" | "mid" | "idx" | "sn");
    }
    let lower = key.to_ascii_lowercase();
    !(lower.starts_with("utm_")
        || matches!(
            lower.as_str(),
            "fbclid"
                | "gclid"
                | "igshid"
                | "spm"
                | "mc_cid"
                | "mc_eid"
                | "ref_src"
                | "share_source"
                | "share_medium"
                | "share_plat"
                | "share_session_id"
                | "share_tag"
                | "share_token"
                | "xsec_source"
                | "xsec_token"
        ))
}

fn normalize_web(parsed: &mut url::Url, host: &str) -> String {
    parsed.set_fragment(None);
    let path = parsed.path().to_owned();
    let kept: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| keep_query_pair(host, &path, key))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    parsed.set_query(None);
    if !kept.is_empty() {
        parsed.query_pairs_mut().extend_pairs(kept);
    }
    let trimmed = path.trim_end_matches('/');
    parsed.set_path(if trimmed.is_empty() { "/" } else { trimmed });
    let _ = parsed.set_host(Some(host));
    parsed.as_str().to_owned()
}

/// 链接本身能不能收。只做字面量检查；以后真正去抓网页时，还要把域名解析成 IP 后再查一次。
pub fn url_is_safe(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| "链接格式无效".to_owned())?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err("只允许 http 或 https".to_owned());
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return Err("链接不能带账号密码".to_owned());
    }
    let host = parsed
        .host_str()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    // 没有点的主机名（`nas`、`router`）只会是内网名字：fake-ip DNS 下它们也会被解析成 198.18.x.x 放行，
    // 再由代理软件在它那一侧去解析，可能连到局域网里的设备。公网上的网站一定有点。
    let single_label = !host.contains('.') && bare.parse::<IpAddr>().is_err();
    if host.is_empty()
        || host == "localhost"
        || single_label
        || [
            ".localhost",
            ".local",
            ".internal",
            ".lan",
            ".home.arpa",
            ".localdomain",
            ".corp",
            ".intranet",
            ".private",
            ".home",
            ".svc",
        ]
        .iter()
        .any(|suffix| host.ends_with(suffix))
    {
        return Err("本机或内网地址，不收".to_owned());
    }
    if let Ok(ip) = bare.parse::<IpAddr>() {
        if ip_is_blocked(ip) {
            return Err("本机、内网或云元数据地址，不收".to_owned());
        }
    }
    Ok(())
}

/// 注意：**不要**把 198.18.0.0/15（基准测试网段）加进来。Fox 的电脑用 Clash / Surge 一类的「fake-ip」DNS，
/// 所有域名在本机都解析成 198.18.x.x，再由代理软件接走；拦了这个段，收集箱就一个网页都读不了。
fn ipv4_is_blocked(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || octets[0] == 0
        // 240.0.0.0/4：保留
        || octets[0] >= 240
        // 100.64.0.0/10：运营商级 NAT，也不是公网
        || (octets[0] == 100 && (octets[1] & 0xc0) == 64)
        // 192.0.0.0/24：IETF 协议分配；192.88.99.0/24：6to4 中继（已废弃）
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
}

fn embedded_ipv4(high: u16, low: u16) -> Ipv4Addr {
    Ipv4Addr::new((high >> 8) as u8, high as u8, (low >> 8) as u8, low as u8)
}

/// IPv6 里藏着 IPv4 的写法很多：映射、兼容、NAT64、6to4。只看 IPv6 前缀会让
/// `64:ff9b::a9fe:a9fe`（NAT64 里的 169.254.169.254）、`2002:7f00:1::`（6to4 里的 127.0.0.1）绕过去。
fn ipv6_is_blocked(v6: Ipv6Addr) -> bool {
    if let Some(mapped) = v6.to_ipv4_mapped() {
        return ipv4_is_blocked(mapped);
    }
    let s = v6.segments();
    // ::/96（含 `::`、`::1` 和已废弃的 IPv4 兼容地址）
    if s[..6].iter().all(|segment| *segment == 0) {
        return true;
    }
    // ::ffff:0:a.b.c.d（IPv4 转换）
    if s[..4].iter().all(|segment| *segment == 0) && s[4] == 0xffff && s[5] == 0 {
        return true;
    }
    // 64:ff9b::/96（NAT64）：按里面的 IPv4 判断，公网的放行（纯 IPv6 网络靠它上网）
    if s[0] == 0x0064 && s[1] == 0xff9b && s[2..6].iter().all(|segment| *segment == 0) {
        return ipv4_is_blocked(embedded_ipv4(s[6], s[7]));
    }
    // 64:ff9b:1::/48（本地用 NAT64）：IPv4 的位置随前缀长度变，不猜，整段拦
    if s[0] == 0x0064 && s[1] == 0xff9b && s[2] == 1 {
        return true;
    }
    // 2002::/16（6to4）：IPv4 在第 16–47 位
    if s[0] == 0x2002 {
        return ipv4_is_blocked(embedded_ipv4(s[1], s[2]));
    }
    // 2001::/32（Teredo）、2001:db8::/32（文档）、100::/64（丢弃专用）
    if s[0] == 0x2001 && (s[1] == 0 || s[1] == 0x0db8) {
        return true;
    }
    if s[0] == 0x0100 && s[1..4].iter().all(|segment| *segment == 0) {
        return true;
    }
    v6.is_loopback()
        || v6.is_unspecified()
        || v6.is_unique_local()
        || v6.is_multicast()
        // fe80::/10 链路本地、fec0::/10 站点本地（已废弃）
        || (s[0] & 0xffc0) == 0xfe80
        || (s[0] & 0xffc0) == 0xfec0
}

pub(crate) fn ip_is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_blocked(v4),
        IpAddr::V6(v6) => ipv6_is_blocked(v6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(String, String)> {
        extract_links_from_text(text)
            .into_iter()
            .map(|link| (link.kind, link.normalized_url))
            .collect()
    }

    #[test]
    fn source_skill_variants_keep_paths_and_normalize_owner_repo() {
        let text = "https://github.com/Acme/Repo/blob/Main/a/SKILL.md?x=1 https://github.com/Acme/Repo/blob/Main/b/skill.md https://raw.githubusercontent.com/Acme/Repo/Main/c/SKILL.md?x=1#part https://github.com/Acme/Repo/tree/Main/tools/Skills/one https://github.com/Acme/Repo/tree/Main/src/utils";
        let links = extract_source_links(text);
        assert_eq!(links.len(), 5);
        assert_eq!(
            links
                .iter()
                .map(|link| link.kind.as_str())
                .collect::<Vec<_>>(),
            [
                "github_skill",
                "github_skill",
                "github_skill",
                "github_skill",
                "github_repo"
            ]
        );
        assert_eq!(
            links[0].normalized_url,
            "https://github.com/acme/repo/blob/Main/a/SKILL.md"
        );
        assert_eq!(
            links[1].normalized_url,
            "https://github.com/acme/repo/blob/Main/b/skill.md"
        );
        assert_eq!(
            links[2].normalized_url,
            "https://raw.githubusercontent.com/acme/repo/Main/c/SKILL.md"
        );
        assert_eq!(
            links[3].normalized_url,
            "https://github.com/acme/repo/tree/Main/tools/Skills/one"
        );
        assert_eq!(links[4].normalized_url, "https://github.com/acme/repo");
        assert_eq!(links[2].platform, "github");
        assert_eq!(links[2].owner.as_deref(), Some("Acme"));
        assert_eq!(links[2].name.as_deref(), Some("Repo"));
        assert_eq!(
            extract_source_links("https://github.com/a/b/tree/main/skills")[0].kind,
            "github_repo"
        );
    }

    #[test]
    fn extracts_and_dedupes_github_and_chat_links() {
        let text =
            "群里说 https://github.com/openai/codex 和 https://github.com/openai/codex/tree/main
再看 https://x.com/fox/status/1 。重复 https://github.com/openai/codex";
        let links = extract_links_from_text(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].kind, "github_repo");
        assert_eq!(links[0].owner.as_deref(), Some("openai"));
        assert_eq!(links[0].name.as_deref(), Some("codex"));
        assert_eq!(links[1].kind, "x_post");
    }

    #[test]
    fn thirty_chat_links_extract_within_a_second() {
        let text = (1..=30)
            .map(|index| format!("https://github.com/fox/tool-{index}"))
            .collect::<Vec<_>>()
            .join(" 群里还有 ");
        let started = std::time::Instant::now();
        let links = extract_links_from_text(&text);
        assert!(started.elapsed().as_secs_f64() < 1.0);
        assert_eq!(links.len(), 30);
        assert!(links.iter().all(|link| link.kind == "github_repo"));
    }

    #[test]
    fn chinese_text_glued_to_links_is_not_swallowed() {
        let text = "看这个 https://github.com/openai/codex，大家可以试试\n\
                    推荐https://github.com/obra/superpowers这个插件\n\
                    链接：https://github.com/a/b。后面还有\n\
                    （https://github.com/c/d）好用";
        let links = extract_links_from_text(text);
        let names: Vec<_> = links
            .iter()
            .map(|link| {
                format!(
                    "{}/{}",
                    link.owner.clone().unwrap(),
                    link.name.clone().unwrap()
                )
            })
            .collect();
        assert_eq!(names, ["openai/codex", "obra/superpowers", "a/b", "c/d"]);
        assert_eq!(links[0].url, "https://github.com/openai/codex");
    }

    #[test]
    fn links_without_scheme_are_found_for_x_and_github() {
        let found =
            kinds("赵六：x.com/karpathy/status/1234567890 这条讲得很好，还有 github.com/a/b");
        assert_eq!(
            found,
            [
                (
                    "x_post".to_owned(),
                    "https://x.com/karpathy/status/1234567890".to_owned()
                ),
                (
                    "github_repo".to_owned(),
                    "https://github.com/a/b".to_owned()
                ),
            ]
        );
        // 域名的一部分、邮箱里的 github.com 不算链接
        assert!(extract_links_from_text("mygithub.com/a/b 和 foo@github.com/x").is_empty());
    }

    #[test]
    fn case_and_tracking_params_do_not_create_duplicates() {
        let text = "https://github.com/OpenAI/Codex 和 https://github.com/openai/codex.git \
                    https://example.com/post?utm_source=wechat&id=7#top https://www.example.com/post/?id=7&fbclid=x \
                    https://twitter.com/u/status/9?s=20&t=abc https://x.com/U/status/9";
        let found = kinds(text);
        assert_eq!(found.len(), 3, "{found:?}");
        assert_eq!(found[0].1, "https://github.com/openai/codex");
        assert_eq!(found[1].1, "https://example.com/post?id=7");
        assert_eq!(found[2].1, "https://x.com/u/status/9");
        // 保留第一次出现的写法，方便界面显示
        let links = extract_links_from_text(text);
        assert_eq!(links[0].owner.as_deref(), Some("OpenAI"));
    }

    #[test]
    fn github_non_repo_paths_are_not_repositories() {
        let found = kinds("https://github.com/topics/ai https://github.com/orgs/x/people https://github.com/someone https://github.com/settings/profile https://github.com/");
        let repos: Vec<_> = found
            .iter()
            .filter(|(kind, _)| kind == "github_repo")
            .collect();
        assert!(repos.is_empty(), "{found:?}");
        assert!(found
            .iter()
            .any(|(_, url)| url == "https://github.com/someone"));
    }

    #[test]
    fn repo_names_may_end_with_underscore_and_urls_with_parentheses_survive() {
        let links = extract_links_from_text(
            "https://github.com/chenjin-cmd/agent-skills-launch-pack_ 以及 (见 https://en.wikipedia.org/wiki/Foo_(bar)) **https://example.com/a**",
        );
        assert_eq!(links[0].name.as_deref(), Some("agent-skills-launch-pack_"));
        assert_eq!(links[1].url, "https://en.wikipedia.org/wiki/Foo_(bar)");
        assert_eq!(links[2].url, "https://example.com/a");
    }

    #[test]
    fn wechat_article_identity_is_kept_and_share_noise_dropped() {
        let short = kinds("https://mp.weixin.qq.com/s/abcdEFG?chksm=xyz&scene=1");
        assert_eq!(short[0].1, "https://mp.weixin.qq.com/s/abcdEFG");
        let long = extract_links_from_text(
            "https://mp.weixin.qq.com/s?__biz=MzA%3D&mid=1&idx=2&sn=ab&chksm=zz&pass_ticket=SECRET",
        );
        assert!(long[0].normalized_url.contains("__biz="));
        assert!(!long[0].normalized_url.contains("pass_ticket"));
        assert!(!long[0].normalized_url.contains("chksm"));
        assert_eq!(long[0].platform, "wechat");
    }

    #[test]
    fn internal_links_are_flagged_before_commit() {
        let links = extract_links_from_text(
            "内网 http://192.168.1.10:8080/admin 本机 http://localhost:3000 映射 http://[::ffff:127.0.0.1]/x 元数据 http://169.254.169.254/latest 公网 https://example.com",
        );
        assert_eq!(links.len(), 5);
        assert!(
            links[..4].iter().all(|link| link.blocked_reason.is_some()),
            "{links:?}"
        );
        assert!(links[4].blocked_reason.is_none());
    }

    #[test]
    fn url_safety_rejects_internal_hosts_and_credentials() {
        for bad in [
            "http://127.0.0.1/secret",
            "http://169.254.169.254/latest",
            "http://10.0.0.5",
            "http://100.64.0.1",
            "http://[::1]/",
            "http://[::ffff:10.0.0.1]/",
            "http://[fe80::1]/",
            "http://[fd00::1]/",
            "http://localhost./x",
            "http://printer.local/",
            "http://2130706433/",
            "https://user:pw@example.com/",
            "ftp://example.com/",
        ] {
            assert!(url_is_safe(bad).is_err(), "{bad} 应被拒绝");
        }
        for good in [
            "https://github.com/fox/tool",
            "http://example.com:8080/a?b=1",
        ] {
            assert!(url_is_safe(good).is_ok(), "{good} 应放行");
        }
    }

    fn blocked(text: &str) -> bool {
        ip_is_blocked(text.parse().expect("测试里的 IP 应能解析"))
    }

    #[test]
    fn ipv6_forms_that_hide_a_private_ipv4_are_blocked() {
        // 修复前这些都能绕过
        for text in [
            "::127.0.0.1",
            "::a9fe:a9fe",
            "64:ff9b::a9fe:a9fe", // NAT64 里的 169.254.169.254（云元数据）
            "64:ff9b::7f00:1",
            "64:ff9b::a00:1",
            "64:ff9b:1::1",
            "2002:7f00:1::1", // 6to4 里的 127.0.0.1
            "2002:a00:1::1",
            "2001::1", // Teredo
            "2001:db8::1",
            "100::1",
            "fec0::1",
            "ff02::1",
            "::ffff:0:7f00:1",
            "::ffff:127.0.0.1",
        ] {
            assert!(blocked(text), "{text} 应被拦");
        }
    }

    #[test]
    fn extra_ipv4_ranges_are_blocked() {
        for text in [
            "192.0.0.8",
            "192.88.99.1",
            "224.0.0.1",
            "239.255.255.250",
            "240.0.0.1",
            "255.255.255.254",
        ] {
            assert!(blocked(text), "{text} 应被拦");
        }
    }

    #[test]
    fn public_addresses_and_the_fake_ip_range_stay_allowed() {
        for text in [
            "198.18.0.1", // Fox 的 fake-ip DNS 把所有域名解析到这里，绝不能拦
            "198.19.255.254",
            "93.184.216.34",
            "8.8.8.8",
            "2606:4700:4700::1111",
            "64:ff9b::808:808", // NAT64 里的 8.8.8.8：纯 IPv6 网络靠它上网
            "2002:808:808::1",  // 6to4 里的 8.8.8.8
            "2a00:1450:4001:81b::200e",
        ] {
            assert!(!blocked(text), "{text} 不该被拦");
        }
    }

    #[test]
    fn trailing_punctuation_runs_are_trimmed_in_linear_time() {
        // 修复前：4 万个 `)` 要 3.6 秒（每剥一个字符都重数整段的括号）
        let url = format!("https://github.com/x/y{}", ")".repeat(60_000));
        let started = std::time::Instant::now();
        let trimmed = trim_trailing(&url);
        assert!(
            started.elapsed().as_secs_f64() < 1.0,
            "{:?}",
            started.elapsed()
        );
        assert_eq!(trimmed, "https://github.com/x/y");
        // 成对的括号仍然保留
        assert_eq!(
            trim_trailing("https://en.wikipedia.org/wiki/Rust_(language)"),
            "https://en.wikipedia.org/wiki/Rust_(language)"
        );
        assert_eq!(
            trim_trailing("https://example.com/a)),"),
            "https://example.com/a"
        );
    }

    #[test]
    fn absurdly_long_links_and_huge_link_lists_are_capped() {
        let long = format!("https://github.com/fox/{}", "a".repeat(5000));
        assert!(
            extract_links_from_text(&long).is_empty(),
            "5000 字符的链接不收"
        );
        let ok = format!("https://example.com/{}", "b".repeat(1900));
        assert_eq!(extract_links_from_text(&ok).len(), 1);
        let many = (0..1000)
            .map(|index| format!("https://example.com/p{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let started = std::time::Instant::now();
        let links = extract_links_from_text(&many);
        assert_eq!(links.len(), MAX_LINKS_PER_TEXT);
        assert!(started.elapsed().as_secs_f64() < 2.0);
    }

    #[test]
    fn single_label_and_intranet_suffix_hosts_are_not_collected() {
        for url in [
            "https://nas/",
            "https://router/admin",
            "https://printer.local/",
            "https://x.home.arpa/",
            "https://box.localdomain/",
            "https://wiki.corp/",
            "https://git.intranet/",
            "https://kubernetes.default.svc/",
            "https://[::1]/",
        ] {
            assert!(url_is_safe(url).is_err(), "{url} 应被拦");
        }
        for url in [
            "https://github.com/openai/codex",
            "https://mp.weixin.qq.com/s/abc",
            "https://example.com/",
            "https://[2606:4700:4700::1111]/",
        ] {
            assert!(url_is_safe(url).is_ok(), "{url} 不该被拦");
        }
    }

    #[test]
    fn the_repo_finder_stops_early_and_ignores_links_without_github() {
        let mut text = String::new();
        for index in 0..5_000 {
            text.push_str(&format!("https://example.com/p{index} "));
        }
        text.push_str("HTTPS://GITHUB.COM/Late/One github.com/late/two ");
        for index in 0..40 {
            text.push_str(&format!("https://github.com/owner/repo-{index} "));
        }
        let mut seen = 0;
        let found = find_github_repo_links(&text, 5, |_| {
            seen += 1;
            true
        });
        assert_eq!(found.len(), 5, "够 5 个就停");
        assert_eq!(seen, 5, "停了之后不再看后面的");
        assert_eq!(
            found[0].owner.as_deref(),
            Some("Late"),
            "大小写不同的 GitHub 主机名也认"
        );
        assert_eq!(found[1].name.as_deref(), Some("two"));
        // 校验不过的不算数，也不占名额
        let rejected_first =
            find_github_repo_links(&text, 3, |link| link.owner.as_deref() != Some("Late"));
        assert_eq!(rejected_first.len(), 3);
        assert!(rejected_first
            .iter()
            .all(|link| link.owner.as_deref() != Some("Late")));
    }
}
