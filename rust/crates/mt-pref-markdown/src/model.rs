use marktext_core::PreferencesSnapshot;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

/// `listIndentation` is either a keyword (`dfm` / `tab`) or a space count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IndentValue {
    Keyword(&'static str),
    Spaces(u64),
}

impl IndentValue {
    pub fn as_token(&self) -> String {
        match self {
            Self::Keyword(word) => (*word).to_string(),
            Self::Spaces(n) => n.to_string(),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::Keyword(word) => Value::String((*word).to_string()),
            Self::Spaces(n) => Value::Number((*n).into()),
        }
    }

    pub fn from_json(value: Option<&Value>) -> Self {
        match value {
            Some(Value::String(s)) if s == "dfm" => Self::Keyword("dfm"),
            Some(Value::String(s)) if s == "tab" => Self::Keyword("tab"),
            Some(Value::Number(n)) => match n.as_u64() {
                Some(1) => Self::Spaces(1),
                Some(2) => Self::Spaces(2),
                Some(3) => Self::Spaces(3),
                Some(4) => Self::Spaces(4),
                _ => Self::Spaces(1),
            },
            Some(Value::String(s)) => match s.parse::<u64>() {
                Ok(1) => Self::Spaces(1),
                Ok(2) => Self::Spaces(2),
                Ok(3) => Self::Spaces(3),
                Ok(4) => Self::Spaces(4),
                _ => Self::Spaces(1),
            },
            _ => Self::Spaces(1),
        }
    }

    pub fn from_token(token: &str) -> Self {
        match token {
            "dfm" => Self::Keyword("dfm"),
            "tab" => Self::Keyword("tab"),
            "1" => Self::Spaces(1),
            "2" => Self::Spaces(2),
            "3" => Self::Spaces(3),
            "4" => Self::Spaces(4),
            _ => Self::Spaces(1),
        }
    }
}

/// Values shown on the Markdown category in `markdown/index.vue`.
#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownModel {
    pub prefer_loose_list_item: bool,
    pub bullet_list_marker: String,
    pub order_list_delimiter: String,
    pub list_indentation: IndentValue,
    pub frontmatter_type: String,
    pub super_sub_script: bool,
    pub footnote: bool,
    pub tex_math_dollars: bool,
    pub tex_math_gfm: bool,
    pub tex_math_single_backslash: bool,
    pub tex_math_double_backslash: bool,
    pub fancy_lists: bool,
    pub highlight_syntax: bool,
    pub inline_diff: bool,
    pub github_alerts: bool,
    pub multiline_blockquote: bool,
    pub is_html_enabled: bool,
    pub cjk_emphasis: bool,
    pub soft_newline_as_space: bool,
    pub mermaid_light_theme: String,
    pub mermaid_dark_theme: String,
    pub sequence_theme: String,
    pub plantuml_server: String,
    pub prefer_heading_style: String,
}

const BULLET_MARKERS: &[Choice] = &[
    Choice {
        value: "*",
        label: "*",
    },
    Choice {
        value: "-",
        label: "-",
    },
    Choice {
        value: "+",
        label: "+",
    },
];

const ORDER_DELIMITERS: &[Choice] = &[
    Choice {
        value: ".",
        label: ".",
    },
    Choice {
        value: ")",
        label: ")",
    },
];

const LIST_INDENTATION: &[Choice] = &[
    Choice {
        value: "dfm",
        label: "DFM Style",
    },
    Choice {
        value: "tab",
        label: "Tab",
    },
    Choice {
        value: "1",
        label: "One Space",
    },
    Choice {
        value: "2",
        label: "Two Spaces",
    },
    Choice {
        value: "3",
        label: "Three Spaces",
    },
    Choice {
        value: "4",
        label: "Four Spaces",
    },
];

const FRONTMATTER_TYPES: &[Choice] = &[
    Choice {
        value: "-",
        label: "YAML",
    },
    Choice {
        value: "+",
        label: "TOML",
    },
    Choice {
        value: ";",
        label: "JSON with Semicolon",
    },
    Choice {
        value: "{",
        label: "JSON with Braces",
    },
];

const MERMAID_THEMES: &[Choice] = &[
    Choice {
        value: "default",
        label: "default",
    },
    Choice {
        value: "forest",
        label: "forest",
    },
    Choice {
        value: "neutral",
        label: "neutral",
    },
    Choice {
        value: "base",
        label: "base",
    },
    Choice {
        value: "dark",
        label: "dark",
    },
    Choice {
        value: "neo",
        label: "neo",
    },
    Choice {
        value: "neo-dark",
        label: "neo-dark",
    },
    Choice {
        value: "redux",
        label: "redux",
    },
    Choice {
        value: "redux-dark",
        label: "redux-dark",
    },
    Choice {
        value: "redux-color",
        label: "redux-color",
    },
    Choice {
        value: "redux-dark-color",
        label: "redux-dark-color",
    },
];

const SEQUENCE_THEMES: &[Choice] = &[
    Choice {
        value: "hand",
        label: "Hand Drawn",
    },
    Choice {
        value: "simple",
        label: "Simple",
    },
];

const HEADING_STYLES: &[Choice] = &[
    Choice {
        value: "atx",
        label: "ATX Style",
    },
    Choice {
        value: "setext",
        label: "Setext Style",
    },
];

impl MarkdownModel {
    pub fn from_snapshot(snapshot: &PreferencesSnapshot) -> Self {
        Self {
            prefer_loose_list_item: bool_pref(snapshot, "preferLooseListItem"),
            bullet_list_marker: string_pref(snapshot, "bulletListMarker", "-"),
            order_list_delimiter: string_pref(snapshot, "orderListDelimiter", "."),
            list_indentation: IndentValue::from_json(snapshot.get("listIndentation")),
            frontmatter_type: string_pref(snapshot, "frontmatterType", "-"),
            super_sub_script: bool_pref(snapshot, "superSubScript"),
            footnote: bool_pref(snapshot, "footnote"),
            tex_math_dollars: bool_pref(snapshot, "texMathDollars"),
            tex_math_gfm: bool_pref(snapshot, "texMathGfm"),
            tex_math_single_backslash: bool_pref(snapshot, "texMathSingleBackslash"),
            tex_math_double_backslash: bool_pref(snapshot, "texMathDoubleBackslash"),
            fancy_lists: bool_pref(snapshot, "fancyLists"),
            highlight_syntax: bool_pref(snapshot, "highlightSyntax"),
            inline_diff: bool_pref(snapshot, "inlineDiff"),
            github_alerts: bool_pref(snapshot, "githubAlerts"),
            multiline_blockquote: bool_pref(snapshot, "multilineBlockquote"),
            is_html_enabled: bool_pref(snapshot, "isHtmlEnabled"),
            cjk_emphasis: bool_pref(snapshot, "cjkEmphasis"),
            soft_newline_as_space: bool_pref(snapshot, "softNewlineAsSpace"),
            mermaid_light_theme: string_pref(snapshot, "mermaidLightTheme", "default"),
            mermaid_dark_theme: string_pref(snapshot, "mermaidDarkTheme", "dark"),
            sequence_theme: string_pref(snapshot, "sequenceTheme", "hand"),
            plantuml_server: string_pref(
                snapshot,
                "plantumlServer",
                "https://www.plantuml.com/plantuml",
            ),
            prefer_heading_style: string_pref(snapshot, "preferHeadingStyle", "atx"),
        }
    }

    pub fn bullet_marker_choices() -> &'static [Choice] {
        BULLET_MARKERS
    }

    pub fn order_delimiter_choices() -> &'static [Choice] {
        ORDER_DELIMITERS
    }

    pub fn list_indentation_choices() -> &'static [Choice] {
        LIST_INDENTATION
    }

    pub fn frontmatter_type_choices() -> &'static [Choice] {
        FRONTMATTER_TYPES
    }

    pub fn mermaid_theme_choices() -> &'static [Choice] {
        MERMAID_THEMES
    }

    pub fn sequence_theme_choices() -> &'static [Choice] {
        SEQUENCE_THEMES
    }

    pub fn heading_style_choices() -> &'static [Choice] {
        HEADING_STYLES
    }
}

/// Schema pattern `^(https?://|$)` for `plantumlServer`.
pub fn plantuml_url_ok(value: &str) -> bool {
    value.is_empty() || value.starts_with("http://") || value.starts_with("https://")
}

/// One PlantUML URL edit waiting for `mt::preferences::update` to answer.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingPersist {
    pub id: u64,
    pub key: String,
    pub value: Value,
    pub draft_text: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PersistDecision {
    Ignored,
    Failed,
    Applied(PendingPersist),
}

/// At most one PlantUML write is in flight. Another save is ignored until it finishes.
#[derive(Debug)]
pub struct PersistGate {
    next_id: u64,
    pending: Option<PendingPersist>,
    pub save_error: bool,
}

impl Default for PersistGate {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistGate {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            pending: None,
            save_error: false,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn request(
        &mut self,
        key: String,
        value: Value,
        draft_text: Option<String>,
    ) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.pending = Some(PendingPersist {
            id,
            key,
            value,
            draft_text,
        });
        self.save_error = false;
        Some(id)
    }

    pub fn finish(&mut self, id: u64, persisted: bool) -> PersistDecision {
        let Some(pending) = self.pending.take() else {
            return PersistDecision::Ignored;
        };
        if pending.id != id {
            self.pending = Some(pending);
            return PersistDecision::Ignored;
        }
        if !persisted {
            self.save_error = true;
            return PersistDecision::Failed;
        }
        self.save_error = false;
        PersistDecision::Applied(pending)
    }
}

fn string_pref(snapshot: &PreferencesSnapshot, key: &str, fallback: &str) -> String {
    snapshot.text(key).unwrap_or(fallback).to_string()
}

fn bool_pref(snapshot: &PreferencesSnapshot, key: &str) -> bool {
    match snapshot.get(key) {
        Some(Value::Bool(value)) => *value,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marktext_core::{HostBridge, MemoryHostBridge};
    use serde_json::json;

    #[test]
    fn snapshot_maps_markdown_defaults_and_mixed_indentation() {
        let bridge = MemoryHostBridge::with_defaults();
        let model = MarkdownModel::from_snapshot(&bridge.get_preferences());
        assert!(model.prefer_loose_list_item);
        assert_eq!(model.bullet_list_marker, "-");
        assert_eq!(model.list_indentation, IndentValue::Spaces(1));
        assert_eq!(model.frontmatter_type, "-");
        assert!(model.tex_math_dollars);
        assert!(!model.fancy_lists);
        assert_eq!(model.mermaid_light_theme, "default");
        assert_eq!(model.mermaid_dark_theme, "dark");
        assert_eq!(model.sequence_theme, "hand");
        assert_eq!(
            model.plantuml_server,
            "https://www.plantuml.com/plantuml"
        );
        assert_eq!(model.prefer_heading_style, "atx");

        bridge.set_preference("listIndentation".to_string(), json!("dfm"));
        bridge.set_preference("bulletListMarker".to_string(), json!("*"));
        bridge.set_preference("githubAlerts".to_string(), json!(true));
        let model = MarkdownModel::from_snapshot(&bridge.get_preferences());
        assert_eq!(model.list_indentation, IndentValue::Keyword("dfm"));
        assert_eq!(model.list_indentation.to_json(), json!("dfm"));
        assert_eq!(IndentValue::Spaces(2).to_json(), json!(2));
        assert_eq!(model.bullet_list_marker, "*");
        assert!(model.github_alerts);
        assert_eq!(
            IndentValue::from_token("3").to_json(),
            IndentValue::Spaces(3).to_json()
        );
    }

    #[test]
    fn plantuml_url_follows_schema_pattern() {
        assert!(plantuml_url_ok(""));
        assert!(plantuml_url_ok("https://www.plantuml.com/plantuml"));
        assert!(plantuml_url_ok("http://localhost:8080"));
        assert!(!plantuml_url_ok("ftp://example.com"));
        assert!(!plantuml_url_ok("not-a-url"));
    }

    #[test]
    fn plantuml_persist_gate_matches_exclusion_rules() {
        let mut gate = PersistGate::new();
        let id = gate
            .request(
                "plantumlServer".to_string(),
                json!("https://example.com"),
                Some("https://example.com".to_string()),
            )
            .expect("first edit is accepted");
        assert!(
            gate.request(
                "plantumlServer".to_string(),
                json!("https://other.example"),
                None
            )
            .is_none()
        );
        assert_eq!(gate.finish(id + 1, true), PersistDecision::Ignored);
        assert_eq!(gate.finish(id, false), PersistDecision::Failed);
        assert!(gate.save_error);

        let id = gate
            .request(
                "plantumlServer".to_string(),
                json!("https://example.com"),
                Some("https://example.com".to_string()),
            )
            .expect("the failed edit no longer blocks");
        assert!(!gate.save_error);
        match gate.finish(id, true) {
            PersistDecision::Applied(pending) => {
                assert_eq!(pending.draft_text.as_deref(), Some("https://example.com"));
                assert_eq!(pending.key, "plantumlServer");
                assert_eq!(pending.value, json!("https://example.com"));
            }
            other => panic!("expected the saved url, got {other:?}"),
        }
    }

    #[test]
    fn choice_tables_match_legacy_config() {
        assert_eq!(MarkdownModel::bullet_marker_choices().len(), 3);
        assert_eq!(MarkdownModel::order_delimiter_choices().len(), 2);
        assert_eq!(MarkdownModel::list_indentation_choices().len(), 6);
        assert_eq!(MarkdownModel::frontmatter_type_choices().len(), 4);
        assert_eq!(MarkdownModel::mermaid_theme_choices().len(), 11);
        assert_eq!(MarkdownModel::sequence_theme_choices()[0].value, "hand");
        assert_eq!(MarkdownModel::heading_style_choices()[1].label, "Setext Style");
    }
}
