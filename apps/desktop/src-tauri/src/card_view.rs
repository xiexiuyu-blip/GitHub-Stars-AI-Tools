//! 旧版占位理解卡的读取侧清洗。不写数据库，也不改 `v2_ops::import_summary_cards`。
//!
//! 占位卡是旧摘要硬转出来的：半句一句话、固定的废话问题和能力。真 AI 卡不会同时满足判定条件。

use serde_json::{Map, Value};
use std::borrow::Cow;

pub(crate) const LEGACY_PROBLEM: &str = "想知道这个收藏是做什么的";
pub(crate) const LEGACY_CAPABILITY: &str = "查看项目说明";
pub(crate) const LEGACY_HOW_TO_TRY: &str = "打开仓库 README";

/// 占位卡一句话的上限。60 时大半被截成半句，100 能放下多数整句。
const ONE_LINER_MAX: usize = 100;
const LEGACY_WHAT_IT_IS_CHARS: usize = 400;

pub(crate) fn is_legacy_placeholder(card: &Value) -> bool {
    let Some(object) = card.as_object() else {
        return false;
    };
    problems_are_legacy(object) || capabilities_are_legacy(object)
}

pub(crate) fn derive_one_liner(card: &Value) -> Option<String> {
    let text = card.get("what_it_is").and_then(Value::as_str)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let sentence = first_sentence(trimmed).trim();
    if sentence.is_empty() {
        return None;
    }
    Some(fit_one_liner(sentence))
}

pub(crate) fn sanitize(card: &Value) -> Cow<'_, Value> {
    if !is_legacy_placeholder(card) {
        return Cow::Borrowed(card);
    }
    let mut copy = card.clone();
    let Some(object) = copy.as_object_mut() else {
        return Cow::Borrowed(card);
    };
    if let Some(line) = derive_one_liner(card) {
        object.insert("one_liner_zh".to_owned(), Value::String(line));
    }
    object.insert("capabilities".to_owned(), Value::Array(Vec::new()));
    object.insert("problems_solved".to_owned(), Value::Array(Vec::new()));
    clear_how_to_try(object);
    if let Some(text) = card.get("what_it_is").and_then(Value::as_str) {
        object.insert(
            "what_it_is".to_owned(),
            Value::String(trim_legacy_what_it_is(text)),
        );
    }
    Cow::Owned(copy)
}

pub(crate) fn display_one_liner(stored: Option<&str>, card: &Value) -> Option<String> {
    if is_legacy_placeholder(card) {
        if let Some(line) = derive_one_liner(card) {
            return Some(line);
        }
    }
    clean_stored(stored)
}

fn problems_are_legacy(object: &Map<String, Value>) -> bool {
    let Some(items) = object.get("problems_solved").and_then(Value::as_array) else {
        return false;
    };
    let mut count = 0usize;
    for item in items {
        let Some(text) = item.as_str() else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        if text != LEGACY_PROBLEM {
            return false;
        }
        count += 1;
    }
    count >= 1
}

fn capabilities_are_legacy(object: &Map<String, Value>) -> bool {
    let Some(items) = object.get("capabilities").and_then(Value::as_array) else {
        return false;
    };
    if items.is_empty() {
        return false;
    }
    for item in items {
        let Some(text) = capability_label(item) else {
            return false;
        };
        if text != LEGACY_CAPABILITY {
            return false;
        }
    }
    true
}

fn clear_how_to_try(object: &mut Map<String, Value>) {
    if object.get("how_to_try").and_then(Value::as_str) == Some(LEGACY_HOW_TO_TRY) {
        object.insert("how_to_try".to_owned(), Value::Null);
        return;
    }
    object.remove("how_to_try");
}

fn capability_label(item: &Value) -> Option<&str> {
    if let Some(text) = item.as_str() {
        return Some(text);
    }
    item.get("name_zh").and_then(Value::as_str)
}

/// 取到第一个句末（。！？或换行，含这个字符）。换行随后会被 `trim` 去掉。
fn first_sentence(text: &str) -> &str {
    for (index, ch) in text.char_indices() {
        if matches!(ch, '。' | '！' | '？' | '\n') {
            let end = index + ch.len_utf8();
            return &text[..end];
        }
    }
    text
}

fn fit_one_liner(sentence: &str) -> String {
    if sentence.chars().count() <= ONE_LINER_MAX {
        return sentence.to_owned();
    }
    let head: String = sentence.chars().take(ONE_LINER_MAX).collect();
    if let Some(index) = head.rfind(['，', '；', '、']) {
        format!("{}…", &head[..index])
    } else {
        format!("{head}…")
    }
}

fn trim_legacy_what_it_is(text: &str) -> String {
    if text.chars().count() != LEGACY_WHAT_IT_IS_CHARS {
        return text.to_owned();
    }
    let Some(index) = text.rfind('。') else {
        return text.to_owned();
    };
    let end = index + '。'.len_utf8();
    text[..end].to_owned()
}

/// 和资料库列表原来的 `nonempty_spaces` 一样，只去掉两侧普通空格。
fn clean_stored(stored: Option<&str>) -> Option<String> {
    let text = stored?;
    let trimmed = text.trim_matches(' ');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SUMMARY_AWESOME: &str = "Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。每个模板均为原创、自包含的完整源代码，支持3条命令即可运行。覆盖AI Agents、多智能体团队、MCP、语音AI、RAG、微调等现代AI栈，支持Claude、Gemini、GPT、Llama等多种模型提供商。采用Apache-2.0许可证，可自由商用。";
    const SUMMARY_CHATCUT: &str = "ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件，支持通过 AI 辅助编辑 ChatCut 视频项目，具体功能包括导入媒体、修改时间线、创建动态图形、生成素材、音频转录、添加字幕、导出视频以及验证编辑可见性。";
    const LINE_AWESOME: &str =
        "Awesome LLM Apps 是一个可运行的 LLM 应用模板集合，包含100+ AI Agent和RAG应用。";
    const LINE_CHATCUT: &str = "ChatCut Agent Plugin 是一个连接 Codex 与 ChatCut 的插件，支持通过 AI 辅助编辑 ChatCut 视频项目，具体功能包括导入媒体、修改时间线、创建动态图形…";

    fn repeat(ch: char, count: usize) -> String {
        std::iter::repeat(ch).take(count).collect()
    }

    fn legacy_with(problems: Value, capabilities: Value, what_it_is: Value) -> Value {
        json!({
            "one_liner_zh": "半句话",
            "what_it_is": what_it_is,
            "form": "other",
            "primary_category": "dev-infra",
            "capabilities": capabilities,
            "problems_solved": problems,
            "how_to_try": LEGACY_HOW_TO_TRY,
            "maturity": "usable",
            "zh_friendly": true,
            "confidence": 0.4,
            "review_status": "suggested",
            "license": "MIT"
        })
    }

    fn both_legacy(what_it_is: &str) -> Value {
        legacy_with(
            json!([LEGACY_PROBLEM]),
            json!([{"name_zh": LEGACY_CAPABILITY}]),
            json!(what_it_is),
        )
    }

    #[test]
    fn legacy_problem_alone_is_placeholder() {
        let card = legacy_with(
            json!([LEGACY_PROBLEM]),
            json!([{"name_zh": "真能力"}]),
            json!("一个工具。"),
        );
        assert!(is_legacy_placeholder(&card));
    }

    #[test]
    fn legacy_capability_object_alone_is_placeholder() {
        let card = legacy_with(
            json!(["真问题"]),
            json!([{"name_zh": LEGACY_CAPABILITY, "evidence_line": 1}]),
            json!("一个工具。"),
        );
        assert!(is_legacy_placeholder(&card));
    }

    #[test]
    fn legacy_capability_string_is_placeholder() {
        let card = legacy_with(json!([]), json!([LEGACY_CAPABILITY]), json!("一个工具。"));
        assert!(is_legacy_placeholder(&card));
        let mixed_shape = legacy_with(
            json!([]),
            json!([LEGACY_CAPABILITY, {"name_zh": LEGACY_CAPABILITY}]),
            json!("一个工具。"),
        );
        assert!(is_legacy_placeholder(&mixed_shape));
    }

    #[test]
    fn mixed_real_content_is_not_placeholder() {
        let card = legacy_with(
            json!([LEGACY_PROBLEM, "真实问题"]),
            json!([{"name_zh": LEGACY_CAPABILITY}, {"name_zh": "真实能力"}]),
            json!("一个工具。"),
        );
        assert!(!is_legacy_placeholder(&card));
    }

    #[test]
    fn legacy_capability_stays_placeholder_when_problems_are_mixed() {
        let card = legacy_with(
            json!([LEGACY_PROBLEM, "真实问题"]),
            json!([{"name_zh": LEGACY_CAPABILITY}]),
            json!("一个工具。"),
        );
        assert!(is_legacy_placeholder(&card));
    }

    #[test]
    fn empty_arrays_are_not_placeholder() {
        let card = legacy_with(json!([]), json!([]), json!("一个工具。"));
        assert!(!is_legacy_placeholder(&card));
        let missing = json!({"one_liner_zh": "真的", "confidence": 0.9});
        assert!(!is_legacy_placeholder(&missing));
    }

    #[test]
    fn null_is_not_placeholder() {
        assert!(!is_legacy_placeholder(&Value::Null));
    }

    #[test]
    fn non_object_is_not_placeholder() {
        assert!(!is_legacy_placeholder(&json!([])));
        assert!(!is_legacy_placeholder(&json!("占位")));
        assert!(!is_legacy_placeholder(&json!(1)));
        assert!(!is_legacy_placeholder(&json!(true)));
    }

    #[test]
    fn blank_and_non_string_problems_still_count_as_placeholder() {
        let card = legacy_with(
            json!(["", LEGACY_PROBLEM, 1, null]),
            json!([]),
            json!("一个工具。"),
        );
        assert!(is_legacy_placeholder(&card));
        let only_blanks = legacy_with(json!(["", " "]), json!([]), json!("一个工具。"));
        assert!(!is_legacy_placeholder(&only_blanks));
    }

    #[test]
    fn derive_keeps_short_first_sentence_with_punctuation() {
        let card = both_legacy(SUMMARY_AWESOME);
        assert_eq!(derive_one_liner(&card).as_deref(), Some(LINE_AWESOME));
        let padded = both_legacy("  你好 世界。 后面不要 ");
        assert_eq!(derive_one_liner(&padded).as_deref(), Some("你好 世界。"));
    }

    #[test]
    fn derive_cuts_long_sentence_before_last_break() {
        let card = both_legacy(SUMMARY_CHATCUT);
        assert_eq!(derive_one_liner(&card).as_deref(), Some(LINE_CHATCUT));
        let sentence = format!(
            "{}，{}，{}，{}。",
            repeat('甲', 10),
            repeat('乙', 19),
            repeat('丙', 19),
            repeat('丁', 60)
        );
        let expected = format!(
            "{}，{}，{}…",
            repeat('甲', 10),
            repeat('乙', 19),
            repeat('丙', 19)
        );
        assert_eq!(
            derive_one_liner(&both_legacy(&sentence)).as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn derive_hard_cuts_when_head_has_no_break() {
        let sentence = format!("{}。", repeat('甲', 120));
        let expected = format!("{}…", repeat('甲', 100));
        assert_eq!(
            derive_one_liner(&both_legacy(&sentence)).as_deref(),
            Some(expected.as_str())
        );
        let exact = format!("{}。", repeat('乙', 99));
        assert_eq!(
            derive_one_liner(&both_legacy(&exact)).as_deref(),
            Some(exact.as_str())
        );
        let formerly_cut = format!("{}。", repeat('丙', 80));
        assert_eq!(
            derive_one_liner(&both_legacy(&formerly_cut)).as_deref(),
            Some(formerly_cut.as_str())
        );
    }

    #[test]
    fn derive_uses_whole_text_without_sentence_end() {
        assert_eq!(
            derive_one_liner(&both_legacy("没有句号的短句")).as_deref(),
            Some("没有句号的短句")
        );
        let medium = repeat('丁', 70);
        assert_eq!(
            derive_one_liner(&both_legacy(&medium)).as_deref(),
            Some(medium.as_str())
        );
        let long = repeat('丙', 130);
        let expected = format!("{}…", repeat('丙', 100));
        assert_eq!(
            derive_one_liner(&both_legacy(&long)).as_deref(),
            Some(expected.as_str())
        );
    }

    #[test]
    fn derive_rejects_empty_missing_and_non_string() {
        assert_eq!(derive_one_liner(&both_legacy("")), None);
        assert_eq!(derive_one_liner(&both_legacy("   \n\t")), None);
        let missing = legacy_with(
            json!([LEGACY_PROBLEM]),
            json!([{"name_zh": LEGACY_CAPABILITY}]),
            Value::Null,
        );
        assert_eq!(derive_one_liner(&missing), None);
        let number = legacy_with(
            json!([LEGACY_PROBLEM]),
            json!([{"name_zh": LEGACY_CAPABILITY}]),
            json!(400),
        );
        assert_eq!(derive_one_liner(&number), None);
    }

    #[test]
    fn derive_stops_at_newline_exclamation_and_question() {
        assert_eq!(
            derive_one_liner(&both_legacy("第一句\n第二句很长")).as_deref(),
            Some("第一句")
        );
        assert_eq!(
            derive_one_liner(&both_legacy("好！后面还有很多字。")).as_deref(),
            Some("好！")
        );
        assert_eq!(
            derive_one_liner(&both_legacy("真的吗？后面。")).as_deref(),
            Some("真的吗？")
        );
    }

    #[test]
    fn sanitize_clears_placeholder_fields_and_keeps_confidence() {
        let card = both_legacy(SUMMARY_AWESOME);
        let clean = sanitize(&card);
        assert!(matches!(clean, Cow::Owned(_)));
        assert!(!std::ptr::eq(clean.as_ref(), &card));
        assert_eq!(card["problems_solved"][0], LEGACY_PROBLEM);
        assert_eq!(card["how_to_try"], LEGACY_HOW_TO_TRY);
        assert_eq!(clean["one_liner_zh"], LINE_AWESOME);
        assert!(clean["capabilities"].as_array().unwrap().is_empty());
        assert!(clean["problems_solved"].as_array().unwrap().is_empty());
        assert!(clean["how_to_try"].is_null());
        assert_eq!(clean["what_it_is"], SUMMARY_AWESOME);
        assert_eq!(clean["confidence"], card["confidence"]);
        assert_eq!(clean["confidence"], 0.4);
        assert_eq!(clean["review_status"], "suggested");
        assert_eq!(clean["form"], "other");
        assert_eq!(clean["license"], "MIT");
        assert_eq!(clean["maturity"], "usable");
        assert_eq!(clean["primary_category"], "dev-infra");
        assert_eq!(clean["zh_friendly"], true);
    }

    #[test]
    fn sanitize_borrows_real_card_without_copying() {
        let card = json!({
            "one_liner_zh": "真的一句话",
            "what_it_is": "这是完整介绍。",
            "problems_solved": ["真实问题"],
            "capabilities": [{"name_zh": "真实能力"}],
            "how_to_try": "运行测试",
            "confidence": 0.8,
            "review_status": "confirmed"
        });
        let before = card.clone();
        let clean = sanitize(&card);
        assert!(matches!(clean, Cow::Borrowed(_)));
        assert!(std::ptr::eq(clean.as_ref(), &card));
        assert_eq!(clean.as_ref(), &before);
    }

    #[test]
    fn sanitize_cuts_exact_four_hundred_chars_at_last_period() {
        let body = format!(
            "{}。{}。{}",
            repeat('甲', 10),
            repeat('乙', 10),
            repeat('丙', 378)
        );
        assert_eq!(body.chars().count(), 400);
        let card = both_legacy(&body);
        let clean = sanitize(&card);
        let expected = format!("{}。{}。", repeat('甲', 10), repeat('乙', 10));
        assert_eq!(clean["what_it_is"], expected);
        assert!(clean["what_it_is"].as_str().unwrap().ends_with('。'));
        assert_eq!(clean["confidence"], 0.4);
    }

    #[test]
    fn sanitize_keeps_what_it_is_unless_it_is_exactly_four_hundred() {
        let no_period = repeat('甲', 400);
        let no_period_card = both_legacy(&no_period);
        let clean = sanitize(&no_period_card);
        assert_eq!(clean["what_it_is"], no_period);
        let shorter = format!("{}。{}", repeat('甲', 10), repeat('乙', 20));
        let shorter_card = both_legacy(&shorter);
        let clean = sanitize(&shorter_card);
        assert_eq!(clean["what_it_is"], shorter);
        let longer = format!("{}。{}", repeat('甲', 400), repeat('乙', 5));
        let longer_card = both_legacy(&longer);
        let clean = sanitize(&longer_card);
        assert_eq!(clean["what_it_is"], longer);
    }

    #[test]
    fn sanitize_keeps_original_one_liner_when_derive_is_none() {
        let card = legacy_with(
            json!([LEGACY_PROBLEM]),
            json!([{"name_zh": LEGACY_CAPABILITY}]),
            Value::Null,
        );
        let clean = sanitize(&card);
        assert_eq!(clean["one_liner_zh"], "半句话");
        assert!(clean["how_to_try"].is_null());
        assert!(clean.get("what_it_is").unwrap().is_null());
    }

    #[test]
    fn display_one_liner_uses_derived_line_for_placeholder() {
        let card = both_legacy(SUMMARY_CHATCUT);
        assert_eq!(
            display_one_liner(Some("半句话被截断"), &card).as_deref(),
            Some(LINE_CHATCUT)
        );
    }

    #[test]
    fn display_one_liner_falls_back_to_stored_when_derive_is_none() {
        let card = legacy_with(
            json!([LEGACY_PROBLEM]),
            json!([{"name_zh": LEGACY_CAPABILITY}]),
            json!(""),
        );
        assert_eq!(
            display_one_liner(Some("  库存的句子  "), &card).as_deref(),
            Some("库存的句子")
        );
        assert_eq!(display_one_liner(Some("   "), &card), None);
        assert_eq!(display_one_liner(None, &card), None);
    }

    #[test]
    fn display_one_liner_uses_stored_text_for_real_card() {
        let card = json!({
            "one_liner_zh": "卡里的句子",
            "problems_solved": ["真实问题"],
            "capabilities": [{"name_zh": "真实能力"}]
        });
        assert_eq!(
            display_one_liner(Some("  列里的  "), &card).as_deref(),
            Some("列里的")
        );
        assert_eq!(
            display_one_liner(Some("\t一句话"), &card).as_deref(),
            Some("\t一句话")
        );
        assert_eq!(display_one_liner(Some(""), &card), None);
        assert_eq!(display_one_liner(Some("   "), &card), None);
        assert_eq!(display_one_liner(None, &card), None);
    }
}
