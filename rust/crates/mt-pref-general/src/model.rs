use marktext_core::PreferencesSnapshot;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

/// Values shown on the General category. Other groups in `general/index.vue`
/// belong to Appearance, Files, or Images and stay on those panes.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralModel {
    pub language: String,
    pub default_open_mode: String,
    pub open_files_in_new_window: bool,
    pub open_folder_in_new_window: bool,
    pub toc_position: String,
    pub word_wrap_in_toc: bool,
    pub opened_files_in_sidebar: bool,
    pub auto_reveal_in_sidebar: bool,
    pub open_files_in_preview_mode: bool,
    pub exclude_patterns: Vec<String>,
    pub file_sort_by: String,
    pub file_sort_order: String,
}

const LANGUAGES: &[Choice] = &[
    Choice {
        value: "en",
        label: "English",
    },
    Choice {
        value: "zh-CN",
        label: "Chinese (Simplified)",
    },
    Choice {
        value: "zh-TW",
        label: "Chinese (Traditional)",
    },
    Choice {
        value: "es",
        label: "Spanish",
    },
    Choice {
        value: "fr",
        label: "French",
    },
    Choice {
        value: "de",
        label: "German",
    },
    Choice {
        value: "ja",
        label: "Japanese",
    },
    Choice {
        value: "ko",
        label: "Korean",
    },
    Choice {
        value: "nl",
        label: "Dutch",
    },
    Choice {
        value: "pt",
        label: "Portuguese",
    },
    Choice {
        value: "tr",
        label: "Turkish",
    },
    Choice {
        value: "ru",
        label: "Russian",
    },
    Choice {
        value: "it",
        label: "Italian",
    },
];

const OPEN_MODES: &[Choice] = &[
    Choice {
        value: "edit",
        label: "Edit Mode",
    },
    Choice {
        value: "read",
        label: "Reading Mode",
    },
];

const TOC_SIDES: &[Choice] = &[
    Choice {
        value: "left",
        label: "Left",
    },
    Choice {
        value: "right",
        label: "Right",
    },
];

const SORT_BY: &[Choice] = &[
    Choice {
        value: "created",
        label: "Creation Time",
    },
    Choice {
        value: "modified",
        label: "Modification Time",
    },
    Choice {
        value: "title",
        label: "File Name",
    },
];

impl GeneralModel {
    pub fn from_snapshot(snapshot: &PreferencesSnapshot) -> Self {
        Self {
            language: string_pref(snapshot, "language", "en"),
            default_open_mode: string_pref(snapshot, "defaultOpenMode", "edit"),
            open_files_in_new_window: bool_pref(snapshot, "openFilesInNewWindow"),
            open_folder_in_new_window: bool_pref(snapshot, "openFolderInNewWindow"),
            toc_position: string_pref(snapshot, "tocPosition", "left"),
            word_wrap_in_toc: bool_pref(snapshot, "wordWrapInToc"),
            opened_files_in_sidebar: bool_pref(snapshot, "openedFilesInSidebar"),
            auto_reveal_in_sidebar: bool_pref(snapshot, "autoRevealInSidebar"),
            open_files_in_preview_mode: bool_pref(snapshot, "openFilesInPreviewMode"),
            exclude_patterns: string_list(snapshot, "treePathExcludePatterns"),
            file_sort_by: string_pref(snapshot, "fileSortBy", "modified"),
            file_sort_order: string_pref(snapshot, "fileSortOrder", "asc"),
        }
    }

    pub fn language_choices() -> &'static [Choice] {
        LANGUAGES
    }

    pub fn open_mode_choices() -> &'static [Choice] {
        OPEN_MODES
    }

    pub fn toc_choices() -> &'static [Choice] {
        TOC_SIDES
    }

    pub fn sort_by_choices() -> &'static [Choice] {
        SORT_BY
    }

    /// Filename sort uses alphabetical labels. Time sort uses oldest/newest.
    pub fn sort_order_choices(&self) -> [Choice; 2] {
        if self.file_sort_by == "title" {
            [
                Choice {
                    value: "asc",
                    label: "A → Z",
                },
                Choice {
                    value: "desc",
                    label: "Z → A",
                },
            ]
        } else {
            [
                Choice {
                    value: "asc",
                    label: "Oldest First",
                },
                Choice {
                    value: "desc",
                    label: "Newest First",
                },
            ]
        }
    }

    /// Blank text and a rule that is already present do not change the list.
    /// Surrounding whitespace is kept: a glob can legally contain it.
    pub fn patterns_after_add(&self, pattern: &str) -> Option<Vec<String>> {
        if pattern.is_empty()
            || self
                .exclude_patterns
                .iter()
                .any(|existing| existing == pattern)
        {
            return None;
        }
        let mut next = self.exclude_patterns.clone();
        next.push(pattern.to_string());
        Some(next)
    }

    pub fn patterns_without(&self, index: usize) -> Vec<String> {
        let mut next = self.exclude_patterns.clone();
        if index < next.len() {
            next.remove(index);
        }
        next
    }
}

pub fn string_array(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

/// Same rule as `general/pandoc.ts`: an unanswered probe disables nothing,
/// a missing binary disables the switch unless it is already on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PandocSwitch {
    pub note: &'static str,
    pub path: String,
    pub disabled: bool,
}

pub fn pandoc_switch_state(command: Option<&str>, switch_on: bool) -> PandocSwitch {
    let Some(path) = command else {
        return PandocSwitch {
            note: "",
            path: String::new(),
            disabled: false,
        };
    };
    if path.is_empty() {
        return PandocSwitch {
            note: "preferences.general.pandoc.notFound",
            path: String::new(),
            disabled: !switch_on,
        };
    }
    PandocSwitch {
        note: "preferences.general.pandoc.found",
        path: path.to_string(),
        disabled: false,
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

fn string_list(snapshot: &PreferencesSnapshot, key: &str) -> Vec<String> {
    let Some(Value::Array(items)) = snapshot.get(key) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use marktext_core::{HostBridge, MemoryHostBridge};
    use serde_json::json;

    #[test]
    fn snapshot_values_and_sort_labels_follow_the_general_page() {
        let bridge = MemoryHostBridge::with_defaults();
        bridge.set_preference("language".to_string(), json!("ja"));
        bridge.set_preference("fileSortBy".to_string(), json!("title"));
        bridge.set_preference("openFilesInNewWindow".to_string(), json!(true));
        bridge.set_preference(
            "treePathExcludePatterns".to_string(),
            json!(["*.tmp", "**/drafts/**"]),
        );

        let model = GeneralModel::from_snapshot(&bridge.get_preferences());
        assert_eq!(model.language, "ja");
        assert!(model.open_files_in_new_window);
        assert_eq!(model.exclude_patterns, vec!["*.tmp", "**/drafts/**"]);
        assert_eq!(model.sort_order_choices()[0].label, "A → Z");

        bridge.set_preference("fileSortBy".to_string(), json!("modified"));
        let model = GeneralModel::from_snapshot(&bridge.get_preferences());
        assert_eq!(model.sort_order_choices()[1].label, "Newest First");
    }

    #[test]
    fn exclude_rules_reject_blanks_and_duplicates() {
        let bridge = MemoryHostBridge::with_defaults();
        let model = GeneralModel::from_snapshot(&bridge.get_preferences());
        assert!(model.patterns_after_add("").is_none());
        let added = model
            .patterns_after_add(" *.tmp")
            .expect("whitespace is part of the glob");
        bridge.set_preference("treePathExcludePatterns".to_string(), string_array(&added));
        let model = GeneralModel::from_snapshot(&bridge.get_preferences());
        assert!(model.patterns_after_add(" *.tmp").is_none());
        assert_eq!(model.patterns_without(0), Vec::<String>::new());
    }

    #[test]
    fn pandoc_switch_matches_the_vue_helper() {
        assert!(!pandoc_switch_state(None, false).disabled);
        assert!(pandoc_switch_state(Some(""), false).disabled);
        assert!(!pandoc_switch_state(Some(""), true).disabled);
        let found = pandoc_switch_state(Some("/usr/bin/pandoc"), false);
        assert_eq!(found.path, "/usr/bin/pandoc");
        assert!(!found.disabled);
    }
}
