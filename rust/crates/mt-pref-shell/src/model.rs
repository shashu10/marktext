//! Shell navigation payloads and sidebar view state.

use marktext_core::KeybindingEntry;
use serde_json::{Value, json};

use crate::categories::PREFERENCE_CATEGORIES;
use crate::search::{
    SearchEntry, filter_entries, keybinding_search_entries, setting_search_entries,
};

pub const SIDEBAR_TITLE: &str = "Preferences";
pub const SEARCH_PLACEHOLDER: &str = "Search preferences...";
pub const MAX_SUGGESTIONS: usize = 12;

/// Ask Electron/Vue to open a preference category (and optional setting key).
pub const NAVIGATE_PREFERENCE_ACTION: &str = "navigate-preference";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavigatePreference {
    pub category: String,
    pub key: Option<String>,
}

impl NavigatePreference {
    pub fn category(category: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            key: None,
        }
    }

    pub fn setting(category: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            category: category.into(),
            key: Some(key.into()),
        }
    }

    pub fn to_payload(&self) -> Value {
        match &self.key {
            Some(key) => json!({ "category": self.category, "key": key }),
            None => json!({ "category": self.category }),
        }
    }

    pub fn from_payload(payload: &Value) -> Option<Self> {
        let category = payload.get("category")?.as_str()?.to_string();
        if category.is_empty() {
            return None;
        }
        let key = payload
            .get("key")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        Some(Self { category, key })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrefShellModel {
    pub active_category: String,
    pub is_mac: bool,
    pub query: String,
    pub suggestions: Vec<SearchEntry>,
    pub keybindings: Vec<KeybindingEntry>,
    pub keybindings_loading: bool,
}

impl PrefShellModel {
    pub fn new(is_mac: bool) -> Self {
        Self {
            active_category: "general".to_string(),
            is_mac,
            query: String::new(),
            suggestions: Vec::new(),
            keybindings: Vec::new(),
            keybindings_loading: false,
        }
    }

    pub fn set_active_category(&mut self, category: &str) {
        if PREFERENCE_CATEGORIES
            .iter()
            .any(|entry| entry.id == category)
        {
            self.active_category = category.to_string();
        }
    }

    pub fn apply_keybindings(
        &mut self,
        commands: Vec<KeybindingEntry>,
        is_mac: bool,
        loading: bool,
    ) {
        self.is_mac = is_mac;
        self.keybindings = commands;
        self.keybindings_loading = loading;
        self.refresh_suggestions();
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.refresh_suggestions();
    }

    pub fn all_entries(&self) -> Vec<SearchEntry> {
        let mut entries = setting_search_entries(self.is_mac);
        entries.extend(keybinding_search_entries(&self.keybindings));
        entries
    }

    pub fn refresh_suggestions(&mut self) {
        let filtered = filter_entries(&self.all_entries(), &self.query, self.is_mac);
        self.suggestions = filtered.into_iter().take(MAX_SUGGESTIONS).collect();
    }

    pub fn navigate_for_category(&self, category_id: &str) -> Option<NavigatePreference> {
        PREFERENCE_CATEGORIES
            .iter()
            .find(|category| category.id == category_id)
            .map(|category| NavigatePreference::category(category.id))
    }

    pub fn navigate_for_entry(&self, entry: &SearchEntry) -> NavigatePreference {
        NavigatePreference::setting(entry.route_category.clone(), entry.key.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigate_payload_round_trips() {
        let category = NavigatePreference::category("files");
        assert_eq!(
            NavigatePreference::from_payload(&category.to_payload()),
            Some(category)
        );
        let setting = NavigatePreference::setting("editor", "tabSize");
        let payload = setting.to_payload();
        assert_eq!(payload["key"], json!("tabSize"));
        assert_eq!(NavigatePreference::from_payload(&payload), Some(setting));
        assert!(NavigatePreference::from_payload(&json!({ "category": "" })).is_none());
    }

    #[test]
    fn query_drives_suggestions() {
        let mut model = PrefShellModel::new(false);
        model.set_query("zoom".into());
        assert!(!model.suggestions.is_empty());
        assert!(
            model
                .suggestions
                .iter()
                .any(|entry| entry.key == "zoom" || entry.preference_en.to_lowercase().contains("zoom"))
        );
        model.set_query(String::new());
        assert!(model.suggestions.is_empty());
    }

    #[test]
    fn active_category_ignores_unknown_ids() {
        let mut model = PrefShellModel::new(false);
        model.set_active_category("markdown");
        assert_eq!(model.active_category, "markdown");
        model.set_active_category("nope");
        assert_eq!(model.active_category, "markdown");
    }
}
