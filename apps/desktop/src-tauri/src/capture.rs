use serde::Serialize;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};

/// 从聊天、帖子里抽出来的一条链接。
///
/// `url` 是清理后的原始链接（缺协议的补上 https://，粘在后面的中文和标点已去掉）；
/// `normalized_url` 只用来去重和入库：小写主机、去跟踪参数、twitter 并入 x、GitHub 截到仓库级。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedLink {
    pub url: String,
    pub normalized_url: String,
    /// github_repo / github / x_post / x / web
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
    let mut end = raw.len();
    while let Some(last) = raw[..end].chars().next_back() {
        let slice = &raw[..end];
        let strip = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '*' => true,
            ')' => slice.matches('(').count() < slice.matches(')').count(),
            ']' => slice.matches('[').count() < slice.matches(']').count(),
            _ => false,
        };
        if !strip {
            break;
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
                if raw.len() >= 8 {
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
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    for raw in candidates(text) {
        let Some(link) = build_link(&raw) else {
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
    if host.is_empty()
        || host == "localhost"
        || [".localhost", ".local", ".internal", ".lan"]
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
        || octets[0] == 0
        // 100.64.0.0/10：运营商级 NAT，也不是公网
        || (octets[0] == 100 && (octets[1] & 0xc0) == 64)
}

pub(crate) fn ip_is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_blocked(v4),
        IpAddr::V6(v6) => {
            // ::ffff:127.0.0.1 这类「IPv4 映射」写法要按它背后的 IPv4 判断
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return ipv4_is_blocked(mapped);
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
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
}
