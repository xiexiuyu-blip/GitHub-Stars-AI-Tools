//! 仓库的许可证文案和形态推断。纯函数，不读库、不联网。

const NAME_MAX_CHARS: usize = 40;
const UNDECLARED: &str = "未声明";
const OTHER_LICENSE: &str = "其他（见仓库）";

const MCP_TOPICS: &[&str] = &["mcp", "mcp-server", "mcp-servers", "model-context-protocol"];
const SKILL_TOPICS: &[&str] = &[
    "skill",
    "skills",
    "agent-skill",
    "agent-skills",
    "claude-skill",
    "claude-skills",
    "claude-code-skills",
    "codex-skill",
    "codex-skills",
];
const CLI_TOPICS: &[&str] = &["cli", "command-line", "command-line-tool", "terminal"];
const APP_TOPICS: &[&str] = &[
    "desktop-app",
    "electron",
    "tauri",
    "macos-app",
    "windows-app",
    "menubar-app",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenseLabel {
    pub text: String,
    pub declared: bool,
}

pub fn license_label(spdx: Option<&str>, name: Option<&str>) -> LicenseLabel {
    let Some(spdx) = trimmed_nonempty(spdx) else {
        return LicenseLabel {
            text: UNDECLARED.to_owned(),
            declared: false,
        };
    };
    if is_unrecognized(&spdx) {
        let text = match trimmed_nonempty(name) {
            Some(name) => truncate_chars(&name, NAME_MAX_CHARS),
            None => OTHER_LICENSE.to_owned(),
        };
        return LicenseLabel {
            text,
            declared: true,
        };
    }
    LicenseLabel {
        text: spdx,
        declared: true,
    }
}

/// 旧版导入的占位卡把形态写成 `other`，等于没说。这种值要当成「没有」，改用话题和仓库名推断
/// （`xxx-skill` 要知道是 Skill，才能告诉 Codex 装到 ~/.agents/skills）。
pub(crate) fn is_unspecific_form(form: &str) -> bool {
    matches!(
        form.trim().to_ascii_lowercase().as_str(),
        "" | "other" | "unknown" | "none" | "n/a"
    )
}

/// 从话题和仓库名推断形态。`description` 先留着不用，避免被简介里的词带偏。
pub fn infer_form(
    topics: &[String],
    full_name: &str,
    description: Option<&str>,
) -> Option<&'static str> {
    let _ = description;
    let name = repository_name(full_name)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if topic_is(topics, MCP_TOPICS) || name_is_mcp(&name) {
        return Some("mcp");
    }
    if topic_is(topics, SKILL_TOPICS) || name_is_skill(&name) {
        return Some("skill");
    }
    if topic_is(topics, CLI_TOPICS) {
        return Some("cli");
    }
    if topic_is(topics, APP_TOPICS) {
        return Some("app");
    }
    None
}

fn trimmed_nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn is_unrecognized(spdx: &str) -> bool {
    spdx.eq_ignore_ascii_case("NOASSERTION") || spdx.eq_ignore_ascii_case("Other")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    text.chars().take(max_chars).collect()
}

/// `/` 后面才是仓库名。没有 `/` 时不拿整段去对名字规则，避免把 owner 当成仓库。
fn repository_name(full_name: &str) -> Option<&str> {
    let (_, name) = full_name.trim().split_once('/')?;
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn topic_is(topics: &[String], expected: &[&str]) -> bool {
    topics.iter().any(|topic| {
        let normalized = topic.trim().to_ascii_lowercase();
        expected.iter().any(|item| normalized == *item)
    })
}

fn name_is_mcp(name: &str) -> bool {
    name.ends_with("-mcp")
        || name.ends_with("_mcp")
        || name.starts_with("mcp-")
        || name.starts_with("mcp_")
        || name.contains("mcp-server")
}

fn name_is_skill(name: &str) -> bool {
    name.ends_with("-skills")
        || name.ends_with("-skill")
        || name.ends_with("_skills")
        || name.ends_with("_skill")
        || name.starts_with("skills-")
        || name.starts_with("skill-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_label_missing_or_blank_is_undeclared() {
        for spdx in [None, Some(""), Some("   "), Some("\n\t")] {
            let label = license_label(spdx, Some("MIT License"));
            assert_eq!(label.text, "未声明");
            assert!(!label.declared);
        }
    }

    #[test]
    fn license_label_noassertion_uses_name_or_fallback() {
        let named = license_label(Some(" noassertion "), Some("  自定义许可证  "));
        assert_eq!(named.text, "自定义许可证");
        assert!(named.declared);

        let blank_name = license_label(Some("NOASSERTION"), Some("  \t"));
        assert_eq!(blank_name.text, "其他（见仓库）");
        assert!(blank_name.declared);

        let missing_name = license_label(Some("NoAssertion"), None);
        assert_eq!(missing_name.text, "其他（见仓库）");

        let long_name = format!("  {}  ", "字".repeat(41));
        let truncated = license_label(Some("NOASSERTION"), Some(&long_name));
        assert_eq!(truncated.text, "字".repeat(40));
        assert_eq!(truncated.text.chars().count(), 40);
        assert!(truncated.declared);
    }

    #[test]
    fn license_label_other_uses_name_or_fallback() {
        let named = license_label(Some(" OTHER "), Some("见仓库根目录"));
        assert_eq!(named.text, "见仓库根目录");
        assert!(named.declared);

        let exact = license_label(Some("Other"), None);
        assert_eq!(exact.text, "其他（见仓库）");
        assert!(exact.declared);

        let exactly_40 = "名".repeat(40);
        let kept = license_label(Some("other"), Some(&exactly_40));
        assert_eq!(kept.text, exactly_40);
    }

    #[test]
    fn license_label_uses_trimmed_spdx() {
        let mit = license_label(Some("  MIT  "), Some("MIT License"));
        assert_eq!(mit.text, "MIT");
        assert!(mit.declared);

        let agpl = license_label(Some("AGPL-3.0"), None);
        assert_eq!(agpl.text, "AGPL-3.0");
        assert!(agpl.declared);

        let another = license_label(Some("Another"), Some("不要用这个名字"));
        assert_eq!(another.text, "Another");
        assert!(another.declared);
    }

    #[test]
    fn infer_form_detects_mcp() {
        assert_eq!(infer_form(&topics(&["mcp"]), "o/plain", None), Some("mcp"));
        assert_eq!(
            infer_form(&topics(&["mcp-server"]), "o/plain", None),
            Some("mcp")
        );
        assert_eq!(
            infer_form(&topics(&["mcp-servers"]), "o/plain", None),
            Some("mcp")
        );
        assert_eq!(
            infer_form(&topics(&["model-context-protocol"]), "o/plain", None),
            Some("mcp")
        );
        assert_eq!(infer_form(&[], "o/foo-mcp", None), Some("mcp"));
        assert_eq!(infer_form(&[], "o/foo_mcp", None), Some("mcp"));
        assert_eq!(infer_form(&[], "o/mcp-foo", None), Some("mcp"));
        assert_eq!(infer_form(&[], "o/mcp_foo", None), Some("mcp"));
        assert_eq!(infer_form(&[], "o/foo-mcp-server-bar", None), Some("mcp"));
        assert_eq!(infer_form(&[], "o/mcp", None), None);
    }

    #[test]
    fn infer_form_detects_skill() {
        for topic in [
            "skill",
            "skills",
            "agent-skill",
            "agent-skills",
            "claude-skill",
            "claude-skills",
            "claude-code-skills",
            "codex-skill",
            "codex-skills",
        ] {
            assert_eq!(
                infer_form(&topics(&[topic]), "o/plain", None),
                Some("skill"),
                "{topic}"
            );
        }
        assert_eq!(infer_form(&[], "o/foo-skill", None), Some("skill"));
        assert_eq!(infer_form(&[], "o/foo-skills", None), Some("skill"));
        assert_eq!(infer_form(&[], "o/foo_skill", None), Some("skill"));
        assert_eq!(infer_form(&[], "o/foo_skills", None), Some("skill"));
        assert_eq!(infer_form(&[], "o/skill-foo", None), Some("skill"));
        assert_eq!(infer_form(&[], "o/skills-foo", None), Some("skill"));
        assert_eq!(infer_form(&[], "o/skill", None), None);
    }

    #[test]
    fn infer_form_detects_cli_only_from_topics() {
        for topic in ["cli", "command-line", "command-line-tool", "terminal"] {
            assert_eq!(
                infer_form(&topics(&[topic]), "o/plain", None),
                Some("cli"),
                "{topic}"
            );
        }
        assert_eq!(infer_form(&[], "o/foo-cli", None), None);
        assert_eq!(
            infer_form(&topics(&["command-line-tools"]), "o/plain", None),
            None
        );
        assert_eq!(
            infer_form(&[], "o/plain", Some("最好的 command-line 工具")),
            None
        );
    }

    #[test]
    fn infer_form_detects_app_only_from_topics() {
        for topic in [
            "desktop-app",
            "electron",
            "tauri",
            "macos-app",
            "windows-app",
            "menubar-app",
        ] {
            assert_eq!(
                infer_form(&topics(&[topic]), "o/plain", None),
                Some("app"),
                "{topic}"
            );
        }
        assert_eq!(infer_form(&[], "o/tauri-app", None), None);
        assert_eq!(
            infer_form(&[], "o/plain", Some("一个 electron 桌面应用")),
            None
        );
    }

    #[test]
    fn infer_form_prefers_earlier_rule() {
        assert_eq!(
            infer_form(&topics(&["cli", "mcp"]), "o/plain", None),
            Some("mcp")
        );
        assert_eq!(
            infer_form(&topics(&["app", "terminal"]), "o/plain", None),
            Some("cli")
        );
        assert_eq!(
            infer_form(&topics(&["electron"]), "o/foo-skill", None),
            Some("skill")
        );
        assert_eq!(
            infer_form(&topics(&["cli"]), "o/foo-mcp", None),
            Some("mcp")
        );
        assert_eq!(
            infer_form(&topics(&["cli"]), "o/skills-mcp", None),
            Some("mcp")
        );
        assert_eq!(
            infer_form(
                &topics(&["mcp", "skill", "cli", "electron"]),
                "o/plain",
                None
            ),
            Some("mcp")
        );
    }

    #[test]
    fn infer_form_ignores_case_owner_empty_topics_and_description() {
        assert_eq!(
            infer_form(&topics(&["MCP-Server"]), "o/plain", None),
            Some("mcp")
        );
        assert_eq!(infer_form(&[], " MCP-Fan/Widget-MCP ", None), Some("mcp"));
        assert_eq!(infer_form(&[], "mcp-fan/notes", None), None);
        assert_eq!(infer_form(&topics(&[""]), "mcp-fan/notes", None), None);
        assert_eq!(
            infer_form(
                &topics(&["", "  "]),
                "mcp-fan/plain",
                Some("mcp skill cli electron tauri")
            ),
            None
        );
        assert_eq!(
            infer_form(&topics(&["", "  ", "Terminal"]), "o/plain", None),
            Some("cli")
        );
        assert_eq!(infer_form(&[], "foo-mcp", None), None);
        assert_eq!(
            infer_form(&topics(&["CLI"]), "Owner/Plain", None),
            Some("cli")
        );
    }

    fn topics(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn unspecific_forms_are_treated_as_missing() {
        for raw in [
            "", "  ", "other", "Other", " OTHER ", "unknown", "none", "n/a",
        ] {
            assert!(is_unspecific_form(raw), "{raw:?}");
        }
        for raw in [
            "skill",
            "mcp",
            "cli",
            "app",
            "library",
            "service",
            "collection",
        ] {
            assert!(!is_unspecific_form(raw), "{raw:?}");
        }
    }
}
