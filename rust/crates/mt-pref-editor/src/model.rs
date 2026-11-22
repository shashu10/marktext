use marktext_core::PreferencesSnapshot;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

/// Values shown on the Editor category (`editorGroups.editor = writing + code`)
/// plus the embedded spellchecker block from `categoryPage.vue`.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorModel {
    pub code_block_line_numbers: bool,
    pub trim_unnecessary_code_block_empty_lines: bool,
    pub wrap_code_blocks: bool,
    pub auto_pair_bracket: bool,
    pub auto_pair_markdown_syntax: bool,
    pub auto_pair_quote: bool,
    pub tab_size: TabSize,
    pub text_direction: String,
    pub auto_check: bool,
    pub hide_quick_insert_hint: bool,
    pub hide_link_popup: bool,
    pub source_code_line_numbers: bool,
    pub spellchecker_enabled: bool,
    pub spellchecker_no_underline: bool,
    pub spellchecker_language: String,
}

/// `tabSize` is either a space count or the literal tab character.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabSize {
    Spaces(u64),
    Tab,
}

impl TabSize {
    pub fn from_value(value: Option<&Value>) -> Self {
        match value {
            Some(Value::String(text)) if text == "tab" => Self::Tab,
            Some(Value::Number(number)) => {
                number.as_u64().map(Self::Spaces).unwrap_or(Self::Spaces(4))
            }
            _ => Self::Spaces(4),
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            Self::Tab => Value::String("tab".to_string()),
            Self::Spaces(count) => Value::Number((*count).into()),
        }
    }

    pub fn as_choice_value(&self) -> String {
        match self {
            Self::Tab => "tab".to_string(),
            Self::Spaces(count) => count.to_string(),
        }
    }
}

const TAB_SIZES: &[Choice] = &[
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
    Choice {
        value: "tab",
        label: "Tab",
    },
];

const TEXT_DIRECTIONS: &[Choice] = &[
    Choice {
        value: "ltr",
        label: "Left to Right",
    },
    Choice {
        value: "rtl",
        label: "Right to Left",
    },
];

/// English labels only: Liberation Sans cannot draw native-script names.
const SPELLCHECKER_LANGUAGES: &[Choice] = &[
    Choice {
        value: "en-US",
        label: "English (en-US)",
    },
    Choice {
        value: "en-GB",
        label: "English (en-GB)",
    },
    Choice {
        value: "en-AU",
        label: "English (en-AU)",
    },
    Choice {
        value: "en-CA",
        label: "English (en-CA)",
    },
    Choice {
        value: "de-DE",
        label: "German (de-DE)",
    },
    Choice {
        value: "de-CH",
        label: "German (de-CH)",
    },
    Choice {
        value: "fr-FR",
        label: "French (fr-FR)",
    },
    Choice {
        value: "es-ES",
        label: "Spanish (es-ES)",
    },
    Choice {
        value: "es-419",
        label: "Spanish (es-419)",
    },
    Choice {
        value: "it-IT",
        label: "Italian (it-IT)",
    },
    Choice {
        value: "nl-NL",
        label: "Dutch (nl-NL)",
    },
    Choice {
        value: "pt-BR",
        label: "Portuguese (pt-BR)",
    },
    Choice {
        value: "pt-PT",
        label: "Portuguese (pt-PT)",
    },
    Choice {
        value: "ru-RU",
        label: "Russian (ru-RU)",
    },
    Choice {
        value: "pl-PL",
        label: "Polish (pl-PL)",
    },
    Choice {
        value: "sv-SE",
        label: "Swedish (sv-SE)",
    },
    Choice {
        value: "da-DK",
        label: "Danish (da-DK)",
    },
    Choice {
        value: "nb-NO",
        label: "Norwegian (nb-NO)",
    },
    Choice {
        value: "fi-FI",
        label: "Finnish (fi-FI)",
    },
    Choice {
        value: "tr-TR",
        label: "Turkish (tr-TR)",
    },
    Choice {
        value: "uk-UA",
        label: "Ukrainian (uk-UA)",
    },
    Choice {
        value: "vi-VN",
        label: "Vietnamese (vi-VN)",
    },
    Choice {
        value: "id-ID",
        label: "Indonesian (id-ID)",
    },
    Choice {
        value: "hi-IN",
        label: "Hindi (hi-IN)",
    },
];

impl EditorModel {
    pub fn from_snapshot(snapshot: &PreferencesSnapshot) -> Self {
        Self {
            code_block_line_numbers: bool_pref(snapshot, "codeBlockLineNumbers"),
            trim_unnecessary_code_block_empty_lines: bool_pref(
                snapshot,
                "trimUnnecessaryCodeBlockEmptyLines",
            ),
            wrap_code_blocks: bool_pref(snapshot, "wrapCodeBlocks"),
            auto_pair_bracket: bool_pref(snapshot, "autoPairBracket"),
            auto_pair_markdown_syntax: bool_pref(snapshot, "autoPairMarkdownSyntax"),
            auto_pair_quote: bool_pref(snapshot, "autoPairQuote"),
            tab_size: TabSize::from_value(snapshot.get("tabSize")),
            text_direction: string_pref(snapshot, "textDirection", "ltr"),
            auto_check: bool_pref(snapshot, "autoCheck"),
            hide_quick_insert_hint: bool_pref(snapshot, "hideQuickInsertHint"),
            hide_link_popup: bool_pref(snapshot, "hideLinkPopup"),
            source_code_line_numbers: bool_pref(snapshot, "sourceCodeLineNumbers"),
            spellchecker_enabled: bool_pref(snapshot, "spellcheckerEnabled"),
            spellchecker_no_underline: bool_pref(snapshot, "spellcheckerNoUnderline"),
            spellchecker_language: string_pref(snapshot, "spellcheckerLanguage", "en-US"),
        }
    }

    pub fn tab_size_choices() -> &'static [Choice] {
        TAB_SIZES
    }

    pub fn text_direction_choices() -> &'static [Choice] {
        TEXT_DIRECTIONS
    }

    pub fn spellchecker_language_choices() -> &'static [Choice] {
        SPELLCHECKER_LANGUAGES
    }

    /// Prefer host-reported dictionaries; fall back to the static English list.
    /// Unknown codes keep an ASCII `Language (code)` label for Liberation Sans.
    pub fn language_options_for(&self, available: &[String]) -> Vec<(String, String)> {
        let codes: Vec<String> = if available.is_empty() {
            SPELLCHECKER_LANGUAGES
                .iter()
                .map(|choice| choice.value.to_string())
                .collect()
        } else {
            available.to_vec()
        };
        codes
            .into_iter()
            .map(|code| {
                let label = SPELLCHECKER_LANGUAGES
                    .iter()
                    .find(|choice| choice.value == code)
                    .map(|choice| choice.label.to_string())
                    .unwrap_or_else(|| format!("Language ({code})"));
                (code, label)
            })
            .collect()
    }

    pub fn parse_tab_size_choice(raw: &str) -> Option<TabSize> {
        if raw == "tab" {
            return Some(TabSize::Tab);
        }
        raw.parse::<u64>().ok().map(TabSize::Spaces)
    }
}

/// Host-supplied spellchecker extras that are not preference keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpellcheckerHostState {
    pub is_osx: bool,
    pub dictionaries: Vec<String>,
    pub words: Vec<String>,
}

impl SpellcheckerHostState {
    pub fn without_word(&self, word: &str) -> Vec<String> {
        self.words
            .iter()
            .filter(|existing| existing.as_str() != word)
            .cloned()
            .collect()
    }
}

/// One custom-dictionary removal waiting for the host result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingWordRemoval {
    pub id: u64,
    pub word: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WordRemovalDecision {
    Ignored,
    Failed,
    Applied(PendingWordRemoval),
}

#[derive(Debug)]
pub struct WordRemovalGate {
    next_id: u64,
    pending: Option<PendingWordRemoval>,
    pub save_error: bool,
}

impl Default for WordRemovalGate {
    fn default() -> Self {
        Self::new()
    }
}

impl WordRemovalGate {
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

    pub fn request(&mut self, word: String) -> Option<u64> {
        if self.pending.is_some() || word.is_empty() {
            return None;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.pending = Some(PendingWordRemoval { id, word });
        self.save_error = false;
        Some(id)
    }

    pub fn finish(&mut self, id: u64, ok: bool) -> WordRemovalDecision {
        let Some(pending) = self.pending.take() else {
            return WordRemovalDecision::Ignored;
        };
        if pending.id != id {
            self.pending = Some(pending);
            return WordRemovalDecision::Ignored;
        }
        if !ok {
            self.save_error = true;
            return WordRemovalDecision::Failed;
        }
        self.save_error = false;
        WordRemovalDecision::Applied(pending)
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
    fn snapshot_maps_editor_and_spellchecker_keys() {
        let bridge = MemoryHostBridge::with_defaults();
        bridge.set_preference("autoPairBracket".to_string(), json!(false));
        bridge.set_preference("tabSize".to_string(), json!("tab"));
        bridge.set_preference("textDirection".to_string(), json!("rtl"));
        bridge.set_preference("spellcheckerEnabled".to_string(), json!(true));
        bridge.set_preference("spellcheckerLanguage".to_string(), json!("de-DE"));
        bridge.set_preference("wrapCodeBlocks".to_string(), json!(false));

        let model = EditorModel::from_snapshot(&bridge.get_preferences());
        assert!(!model.auto_pair_bracket);
        assert_eq!(model.tab_size, TabSize::Tab);
        assert_eq!(model.text_direction, "rtl");
        assert!(model.spellchecker_enabled);
        assert_eq!(model.spellchecker_language, "de-DE");
        assert!(!model.wrap_code_blocks);
        assert_eq!(model.tab_size.to_value(), json!("tab"));
        assert_eq!(
            EditorModel::parse_tab_size_choice("2"),
            Some(TabSize::Spaces(2))
        );
    }

    #[test]
    fn language_labels_stay_ascii_for_liberation_sans() {
        for choice in EditorModel::spellchecker_language_choices() {
            assert!(
                choice.label.is_ascii(),
                "label {} must stay in Liberation Sans",
                choice.label
            );
        }
    }

    #[test]
    fn word_removal_gate_serializes_destructive_edits() {
        let mut gate = WordRemovalGate::new();
        let id = gate
            .request("colour".to_string())
            .expect("first removal is accepted");
        assert!(gate.request("colour".to_string()).is_none());
        assert_eq!(gate.finish(id + 1, true), WordRemovalDecision::Ignored);
        assert_eq!(gate.finish(id, false), WordRemovalDecision::Failed);
        assert!(gate.save_error);

        let id = gate
            .request("colour".to_string())
            .expect("failed removal no longer blocks");
        match gate.finish(id, true) {
            WordRemovalDecision::Applied(pending) => assert_eq!(pending.word, "colour"),
            other => panic!("expected applied removal, got {other:?}"),
        }
    }

    #[test]
    fn host_state_drops_removed_words() {
        let state = SpellcheckerHostState {
            is_osx: false,
            dictionaries: vec!["en-US".to_string()],
            words: vec!["colour".to_string(), "favour".to_string()],
        };
        assert_eq!(state.without_word("colour"), vec!["favour".to_string()]);
    }
}
