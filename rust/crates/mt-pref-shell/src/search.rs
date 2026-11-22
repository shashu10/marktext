//! Global preference search — mirrors `sideBar/config.ts` + `keybindingSearch.ts`.

use marktext_core::KeybindingEntry;

use crate::categories::{category_en, category_by_id};
use crate::registry::{PREFERENCE_DEFINITIONS, PreferenceDefinition};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchKind {
    Setting,
    Keybinding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchEntry {
    pub key: String,
    pub category: String,
    pub category_en: String,
    pub preference: String,
    pub preference_en: String,
    pub route_category: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub kind: SearchKind,
    pub accelerator: Option<String>,
}

/// Fallback accelerator compare used when the Electron keyboard mapper is
/// unavailable (same path as `isEqualAccelerator` after its try/catch).
pub fn accelerators_equal_normalized(a: &str, b: &str, is_mac: bool) -> bool {
    let a = normalize_accelerator(a, is_mac);
    let b = normalize_accelerator(b, is_mac);
    if !a.contains('+') && !b.contains('+') {
        return a == b;
    }
    if a.contains('+') != b.contains('+') {
        return false;
    }
    let parts_a: Vec<&str> = a.split('+').collect();
    let parts_b: Vec<&str> = b.split('+').collect();
    if parts_a.len() != parts_b.len() {
        return false;
    }
    let mut union: Vec<&str> = parts_a.iter().chain(parts_b.iter()).copied().collect();
    union.sort_unstable();
    union.dedup();
    union.len() == parts_b.len()
}

fn normalize_accelerator(accelerator: &str, is_mac: bool) -> String {
    let mut value = accelerator.to_lowercase();
    let command_or_control = if is_mac { "cmd" } else { "ctrl" };
    value = value.replace("commandorcontrol", command_or_control);
    value = value.replace("cmdorctrl", command_or_control);
    value = value.replace("control", "ctrl");
    value = value.replace("meta", "cmd");
    value = value.replace("command", "cmd");
    value = value.replace("option", "alt");
    value
}

fn rewrite_accelerator_query(query: &str) -> String {
    query
        .trim()
        .replace('⌘', "Command+")
        .replace('⌃', "Ctrl+")
        .replace('⇧', "Shift+")
        .replace('⌥', "Alt+")
}

pub fn matches_search(entry: &SearchEntry, query: &str, is_mac: bool) -> bool {
    let accelerator_query = rewrite_accelerator_query(query);
    if let Some(accelerator) = entry.accelerator.as_deref()
        && accelerators_equal_normalized(accelerator, &accelerator_query, is_mac)
    {
        return true;
    }
    let content = [
        entry.preference.as_str(),
        entry.category.as_str(),
        entry.preference_en.as_str(),
        entry.category_en.as_str(),
    ]
    .into_iter()
    .chain(entry.keywords.iter().map(String::as_str))
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase();
    accelerator_query
        .trim()
        .to_lowercase()
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .all(|word| content.contains(word))
}

pub fn setting_search_entries(is_mac: bool) -> Vec<SearchEntry> {
    PREFERENCE_DEFINITIONS
        .iter()
        .filter(|definition| definition.available_on(is_mac))
        .map(setting_entry)
        .collect()
}

fn setting_entry(definition: &PreferenceDefinition) -> SearchEntry {
    let category_name = category_by_id(definition.category)
        .map(|category| category.name)
        .unwrap_or(definition.category);
    let mut keywords = vec![definition.key.to_string()];
    keywords.extend(definition.aliases.iter().map(|alias| (*alias).to_string()));
    if !definition.notes.is_empty() {
        keywords.push(definition.notes.to_string());
    }
    if !definition.description.is_empty() {
        keywords.push(definition.description.to_string());
    }
    SearchEntry {
        key: definition.key.to_string(),
        category: category_name.to_string(),
        category_en: category_en(definition.category),
        preference: definition.label.to_string(),
        preference_en: definition.label.to_string(),
        route_category: definition.category.to_string(),
        description: definition.description.to_string(),
        keywords,
        kind: SearchKind::Setting,
        accelerator: None,
    }
}

fn keybinding_group_label(id: &str) -> &'static str {
    let group = command_group_name(id);
    match group {
        "file" => "File",
        "edit" => "Edit",
        "format" => "Format",
        "paragraph" => "Paragraph",
        "view" => "View",
        _ => "Window and navigation",
    }
}

fn command_group_name(id: &str) -> &'static str {
    if matches!(id, "file.new-window" | "file.close-window" | "file.quit") {
        return "window";
    }
    if matches!(id, "edit.create-paragraph" | "edit.delete-paragraph") {
        return "paragraph";
    }
    let prefix = id.split('.').next().unwrap_or("");
    if prefix == "spellchecker" {
        return "edit";
    }
    match prefix {
        "file" => "file",
        "edit" => "edit",
        "format" => "format",
        "paragraph" => "paragraph",
        "view" => "view",
        _ => "window",
    }
}

pub fn keybinding_search_entries(commands: &[KeybindingEntry]) -> Vec<SearchEntry> {
    commands
        .iter()
        .map(|command| {
            let group = command_group_name(&command.id);
            let group_label = keybinding_group_label(&command.id);
            SearchEntry {
                key: format!("keybinding-{}", command.id),
                category: format!("Keyboard shortcuts › {group_label}"),
                category_en: format!("Keybindings {group}"),
                preference: command.description.clone(),
                preference_en: command.description.clone(),
                route_category: "keybindings".to_string(),
                description: command.description.clone(),
                keywords: vec![command.id.clone(), command.accelerator.clone()],
                kind: SearchKind::Keybinding,
                accelerator: Some(command.accelerator.clone()),
            }
        })
        .collect()
}

pub fn filter_entries(entries: &[SearchEntry], query: &str, is_mac: bool) -> Vec<SearchEntry> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    entries
        .iter()
        .filter(|entry| matches_search(entry, query, is_mac))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use marktext_core::KeybindingEntry;

    #[test]
    fn empty_query_yields_no_suggestions() {
        let entries = setting_search_entries(false);
        assert!(filter_entries(&entries, "   ", false).is_empty());
        assert!(!entries.is_empty());
    }

    #[test]
    fn setting_search_finds_language_by_label_and_key() {
        let entries = setting_search_entries(false);
        let by_label = filter_entries(&entries, "Interface language", false);
        assert!(by_label.iter().any(|entry| entry.key == "language"));
        let by_key = filter_entries(&entries, "autoPairMarkdownSyntax", false);
        assert_eq!(by_key.len(), 1);
        assert_eq!(by_key[0].route_category, "editor");
    }

    #[test]
    fn mac_only_entries_hidden_on_other_platforms() {
        let other = setting_search_entries(false);
        assert!(!other.iter().any(|entry| entry.key == "spellcheckerAutoDetectLanguage"));
        let mac = setting_search_entries(true);
        assert!(mac.iter().any(|entry| entry.key == "spellcheckerAutoDetectLanguage"));
        assert!(!mac.iter().any(|entry| entry.key == "titleBarStyle"));
    }

    #[test]
    fn keybinding_search_matches_accelerator() {
        let commands = vec![KeybindingEntry {
            id: "file.save".into(),
            description: "Save".into(),
            accelerator: "Ctrl+S".into(),
            default_accelerator: "Ctrl+S".into(),
        }];
        let entries = keybinding_search_entries(&commands);
        let hits = filter_entries(&entries, "Ctrl+S", false);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, SearchKind::Keybinding);
        assert_eq!(hits[0].route_category, "keybindings");
    }
}
