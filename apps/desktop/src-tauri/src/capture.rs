use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedLink {
    pub url: String,
    pub kind: String,
    pub owner: Option<String>,
    pub name: Option<String>,
}

pub fn extract_links_from_text(text: &str) -> Vec<ExtractedLink> {
    let mut links: Vec<ExtractedLink> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let rest = &text[index..];
        let start = rest
            .find("https://")
            .or_else(|| rest.find("http://"))
            .map(|offset| index + offset);
        let Some(start) = start else {
            break;
        };
        let slice = &text[start..];
        let end = slice
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '\'' | '）' | '】'))
            .unwrap_or(slice.len());
        let url = slice[..end].trim_end_matches(['.', ',', ';', ':', '!', '?', '，', '。', ')']).to_owned();
        index = start + end.max(1);
        if url.len() < 12 {
            continue;
        }
        if !seen.insert(url.clone()) {
            continue;
        }
        let (kind, owner, name) = classify(&url);
        if kind == "github_repo" {
            if let Some(existing) = links.iter_mut().find(|link| {
                link.kind == "github_repo" && link.owner == owner && link.name == name
            }) {
                if url.len() < existing.url.len() {
                    existing.url = url;
                }
                continue;
            }
        }
        links.push(ExtractedLink {
            url,
            kind,
            owner,
            name,
        });
    }
    links
}

fn classify(url: &str) -> (String, Option<String>, Option<String>) {
    let without_scheme = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host_and_path = without_scheme.split(['?', '#']).next().unwrap_or(without_scheme);
    let mut parts = host_and_path.split('/').filter(|part| !part.is_empty());
    let host = parts.next().unwrap_or("").trim_start_matches("www.");
    if host == "github.com" {
        let owner = parts.next().map(str::to_owned);
        let name = parts.next().map(|name| name.trim_end_matches(".git").to_owned());
        if owner.as_deref().is_some_and(|value| !value.is_empty())
            && name.as_deref().is_some_and(|value| !value.is_empty())
        {
            return ("github_repo".to_owned(), owner, name);
        }
        return ("github".to_owned(), owner, None);
    }
    if host == "x.com" || host == "twitter.com" {
        return ("x_post".to_owned(), None, None);
    }
    ("web".to_owned(), None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_dedupes_github_and_chat_links() {
        let text = "群里说 https://github.com/openai/codex 和 https://github.com/openai/codex/tree/main
再看 https://x.com/fox/status/1 。重复 https://github.com/openai/codex";
        let links = extract_links_from_text(text);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].kind, "github_repo");
        assert_eq!(links[0].owner.as_deref(), Some("openai"));
        assert_eq!(links[0].name.as_deref(), Some("codex"));
        assert_eq!(links[1].kind, "x_post");
    }
}
