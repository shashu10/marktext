use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One row on the Keybindings preferences pane (`prefs.pane.keybindings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeybindingEntry {
    pub id: String,
    pub description: String,
    pub accelerator: String,
    #[serde(rename = "defaultAccelerator")]
    pub default_accelerator: String,
}

/// Messages the Electron `<GpuiPaneHost>` posts into the iframe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HostToGuest {
    #[serde(rename = "HOST_HELLO")]
    HostHello,
    #[serde(rename = "SET_PANE")]
    SetPane { pane: String },
    #[serde(rename = "PREFERENCES")]
    Preferences { snapshot: BTreeMap<String, Value> },
    #[serde(rename = "PREFERENCE_PATCH")]
    PreferencePatch { changes: BTreeMap<String, Value> },
    /// Answer to `PERSIST_PREFERENCE`. `status` is `persisted` or `failed`.
    #[serde(rename = "PREFERENCE_RESULT")]
    PreferenceResult { id: u64, status: String },
    /// Answer to `CHOOSE_DIRECTORY`. `path` is null when the dialog was cancelled.
    #[serde(rename = "DIRECTORY_CHOSEN")]
    DirectoryChosen { id: u64, path: Option<String> },
    /// Spellchecker dictionaries and personal words for the Editor pane.
    #[serde(rename = "SPELLCHECKER_INFO")]
    SpellcheckerInfo {
        #[serde(rename = "isOsx")]
        is_osx: bool,
        dictionaries: Vec<String>,
        words: Vec<String>,
    },
    /// Answer to `SPELLCHECKER_REMOVE_WORD`.
    #[serde(rename = "SPELLCHECKER_REMOVE_RESULT")]
    SpellcheckerRemoveResult { id: u64, ok: bool },
    /// Full keybinding list for the Keybindings pane. Additive for A7.
    #[serde(rename = "KEYBINDINGS")]
    Keybindings {
        commands: Vec<KeybindingEntry>,
        #[serde(rename = "isMac", default)]
        is_mac: bool,
        #[serde(rename = "showDebug", default)]
        show_debug: bool,
        #[serde(default)]
        loading: bool,
        #[serde(rename = "loadError", default)]
        load_error: bool,
    },
    /// Answer to `SAVE_KEYBINDING` / `RESET_ALL_KEYBINDINGS`.
    /// `status` is `persisted`, `failed`, or `conflict`.
    #[serde(rename = "KEYBINDING_RESULT")]
    KeybindingResult {
        id: u64,
        status: String,
        #[serde(default)]
        commands: Option<Vec<KeybindingEntry>>,
        #[serde(rename = "conflictAccelerator", default)]
        conflict_accelerator: Option<String>,
        #[serde(rename = "conflictCommandIds", default)]
        conflict_command_ids: Option<Vec<String>>,
    },
}

/// Messages the GPUI guest posts back to the Vue host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GuestToHost {
    #[serde(rename = "READY")]
    Ready { pane: Option<String> },
    #[serde(rename = "PANE_CHANGED")]
    PaneChanged { pane: String },
    #[serde(rename = "PREFERENCES_APPLIED")]
    PreferencesApplied,
    #[serde(rename = "SET_PREFERENCE")]
    SetPreference { key: String, value: Value },
    /// Ask Electron to run `mt::preferences::update` and answer with `PREFERENCE_RESULT`.
    #[serde(rename = "PERSIST_PREFERENCE")]
    PersistPreference { id: u64, key: String, value: Value },
    /// Ask Electron to open a directory picker and answer with `DIRECTORY_CHOSEN`.
    #[serde(rename = "CHOOSE_DIRECTORY")]
    ChooseDirectory { id: u64 },
    #[serde(rename = "SCROLL_TO_HEADER")]
    ScrollToHeader { slug: String },
    #[serde(rename = "TOGGLE_READING_MODE")]
    ToggleReadingMode {
        #[serde(rename = "readOnly")]
        read_only: bool,
    },
    /// Ask Electron to remove a personal dictionary word.
    #[serde(rename = "SPELLCHECKER_REMOVE_WORD")]
    SpellcheckerRemoveWord { id: u64, word: String },
    /// Ask the Vue host to load (or reload) keybindings and post `KEYBINDINGS`.
    #[serde(rename = "REQUEST_KEYBINDINGS")]
    RequestKeybindings,
    /// Persist one binding. Empty `accelerator` unbinds. Round-trips via `KEYBINDING_RESULT`.
    #[serde(rename = "SAVE_KEYBINDING")]
    SaveKeybinding {
        id: u64,
        #[serde(rename = "commandId")]
        command_id: String,
        accelerator: String,
    },
    #[serde(rename = "RESET_ALL_KEYBINDINGS")]
    ResetAllKeybindings { id: u64 },
    #[serde(rename = "KEYBINDING_DEBUG_DUMP")]
    KeybindingDebugDump,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn host_and_guest_messages_round_trip() {
        let host = HostToGuest::SetPane {
            pane: "general".to_string(),
        };
        let raw = serde_json::to_value(&host).unwrap();
        assert_eq!(raw, json!({ "type": "SET_PANE", "pane": "general" }));
        let parsed: HostToGuest = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed, host);

        let guest = GuestToHost::ToggleReadingMode { read_only: true };
        let raw = serde_json::to_value(&guest).unwrap();
        assert_eq!(raw["readOnly"], json!(true));
        let parsed: GuestToHost = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed, guest);
    }
}
