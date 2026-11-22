use marktext_core::PreferencesSnapshot;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

/// Schema-backed values shown on the Images category.
#[derive(Clone, Debug, PartialEq)]
pub struct ImagesModel {
    pub image_insert_action: String,
    pub image_prefer_relative_directory: bool,
    pub image_relative_directory_base: String,
    pub image_relative_directory_name: String,
    pub show_pandoc_convert: bool,
    /// Data-center keys. Not in `schema.json`; the host still ships them in PREFERENCES.
    pub image_folder_path: String,
    pub current_uploader: String,
    pub cli_script: String,
}

const INSERT_ACTIONS: &[Choice] = &[
    Choice {
        value: "upload",
        label: "Upload to cloud",
    },
    Choice {
        value: "folder",
        label: "Copy to folder",
    },
    Choice {
        value: "path",
        label: "Use absolute path",
    },
];

const RELATIVE_BASES: &[Choice] = &[
    Choice {
        value: "file",
        label: "Relative to file",
    },
    Choice {
        value: "folder",
        label: "Relative to folder",
    },
];

const UPLOADERS: &[Choice] = &[
    Choice {
        value: "picgo",
        label: "PicGo-Core",
    },
    Choice {
        value: "cliScript",
        label: "CLI Script",
    },
];

/// Keys that live in the Electron data center rather than preferences.json.
pub struct UserDataKeys;

impl UserDataKeys {
    pub const IMAGE_FOLDER_PATH: &'static str = "imageFolderPath";
    pub const CURRENT_UPLOADER: &'static str = "currentUploader";
    pub const CLI_SCRIPT: &'static str = "cliScript";

    pub fn is_user_data(key: &str) -> bool {
        matches!(
            key,
            Self::IMAGE_FOLDER_PATH | Self::CURRENT_UPLOADER | Self::CLI_SCRIPT
        )
    }
}

impl ImagesModel {
    pub fn from_snapshot(
        snapshot: &PreferencesSnapshot,
        user_data: &BTreeMap<String, Value>,
    ) -> Self {
        Self {
            image_insert_action: string_pref(snapshot, "imageInsertAction", "path"),
            image_prefer_relative_directory: bool_pref(snapshot, "imagePreferRelativeDirectory"),
            image_relative_directory_base: string_pref(
                snapshot,
                "imageRelativeDirectoryBase",
                "file",
            ),
            image_relative_directory_name: string_pref(
                snapshot,
                "imageRelativeDirectoryName",
                "assets",
            ),
            show_pandoc_convert: bool_pref(snapshot, "showPandocConvert"),
            image_folder_path: user_string(user_data, UserDataKeys::IMAGE_FOLDER_PATH, ""),
            current_uploader: user_string(user_data, UserDataKeys::CURRENT_UPLOADER, "picgo"),
            cli_script: user_string(user_data, UserDataKeys::CLI_SCRIPT, ""),
        }
    }

    pub fn insert_action_choices() -> &'static [Choice] {
        INSERT_ACTIONS
    }

    pub fn relative_base_choices() -> &'static [Choice] {
        RELATIVE_BASES
    }

    pub fn uploader_choices() -> &'static [Choice] {
        UPLOADERS
    }

    /// Folder settings appear for `folder` and `path` insert actions.
    pub fn show_folder_settings(&self) -> bool {
        self.image_insert_action == "folder" || self.image_insert_action == "path"
    }

    /// Uploader settings appear only when the insert action is `upload`.
    pub fn show_uploader(&self) -> bool {
        self.image_insert_action == "upload"
    }

    pub fn show_picgo(&self) -> bool {
        self.show_uploader() && self.current_uploader == "picgo"
    }

    pub fn show_cli_script(&self) -> bool {
        self.show_uploader() && self.current_uploader == "cliScript"
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectionState {
    Checking,
    Found,
    NotFound,
    Failed,
}

/// Absolute path or empty — mirrors `/^(?:$|([a-zA-Z]:)?[\/\\].*$)/`.
pub fn folder_path_valid(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let bytes = value.as_bytes();
    // Drive letter: `C:\` or `C:/`
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
    {
        return true;
    }
    bytes[0] == b'/' || bytes[0] == b'\\'
}

/// Relative name or empty — mirrors `/^(?:$|(?![a-zA-Z]:)[^\/\\].*$)/`.
pub fn relative_directory_name_valid(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    !value.contains('/') && !value.contains('\\')
}

/// One text-field edit waiting for `mt::preferences::update` to answer.
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
            self.pending.replace(pending);
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

pub fn draft_still_matches(submitted: Option<&str>, current: &str) -> bool {
    submitted.is_some_and(|text| text == current)
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

fn user_string(user_data: &BTreeMap<String, Value>, key: &str, fallback: &str) -> String {
    match user_data.get(key) {
        Some(Value::String(value)) => value.clone(),
        _ => fallback.to_string(),
    }
}

/// Pull data-center keys out of a full host `PREFERENCES` map.
pub fn extract_user_data(snapshot: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for key in [
        UserDataKeys::IMAGE_FOLDER_PATH,
        UserDataKeys::CURRENT_UPLOADER,
        UserDataKeys::CLI_SCRIPT,
    ] {
        if let Some(value) = snapshot.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use marktext_core::{HostBridge, MemoryHostBridge};
    use serde_json::json;

    #[test]
    fn snapshot_and_user_data_drive_section_visibility() {
        let bridge = MemoryHostBridge::with_defaults();
        let mut user = BTreeMap::new();
        user.insert(
            UserDataKeys::IMAGE_FOLDER_PATH.to_string(),
            json!("/tmp/images"),
        );
        user.insert(
            UserDataKeys::CURRENT_UPLOADER.to_string(),
            json!("cliScript"),
        );
        user.insert(UserDataKeys::CLI_SCRIPT.to_string(), json!("/bin/upload"));

        let model = ImagesModel::from_snapshot(&bridge.get_preferences(), &user);
        assert_eq!(model.image_insert_action, "path");
        assert!(model.show_folder_settings());
        assert!(!model.show_uploader());
        assert_eq!(model.image_folder_path, "/tmp/images");

        bridge.set_preference("imageInsertAction".to_string(), json!("upload"));
        let model = ImagesModel::from_snapshot(&bridge.get_preferences(), &user);
        assert!(model.show_uploader());
        assert!(model.show_cli_script());
        assert!(!model.show_picgo());
        assert!(!model.show_folder_settings());
    }

    #[test]
    fn path_validators_match_legacy_regexes() {
        assert!(folder_path_valid(""));
        assert!(folder_path_valid("/tmp/images"));
        assert!(folder_path_valid(r"C:\Users\me\images"));
        assert!(!folder_path_valid("relative/path"));
        assert!(!folder_path_valid("no-slash"));

        assert!(relative_directory_name_valid(""));
        assert!(relative_directory_name_valid("assets"));
        assert!(!relative_directory_name_valid("C:assets"));
        assert!(!relative_directory_name_valid("foo/bar"));
        assert!(!relative_directory_name_valid(r"foo\bar"));
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

    #[test]
    fn persist_gate_clears_draft_only_after_persisted_result() {
        let mut gate = PersistGate::new();
        let id = gate
            .request(
                "imageRelativeDirectoryName".to_string(),
                json!("assets"),
                Some("assets".to_string()),
            )
            .expect("first edit is accepted");
        assert!(
            gate.request("imageRelativeDirectoryName".to_string(), json!("x"), None)
                .is_none()
        );
        assert_eq!(gate.finish(id + 1, true), PersistDecision::Ignored);
        assert_eq!(gate.finish(id, false), PersistDecision::Failed);
        assert!(gate.save_error);

        let id = gate
            .request(
                "imageRelativeDirectoryName".to_string(),
                json!("assets"),
                Some("assets".to_string()),
            )
            .expect("failed edit no longer blocks");
        match gate.finish(id, true) {
            PersistDecision::Applied(pending) => {
                assert!(draft_still_matches(pending.draft_text.as_deref(), "assets"));
                assert!(!draft_still_matches(pending.draft_text.as_deref(), "other"));
            }
            other => panic!("expected applied, got {other:?}"),
        }
    }

    #[test]
    fn extract_user_data_keeps_only_data_center_keys() {
        let mut raw = BTreeMap::new();
        raw.insert("theme".to_string(), json!("dark"));
        raw.insert("imageFolderPath".to_string(), json!("/a"));
        raw.insert("currentUploader".to_string(), json!("picgo"));
        raw.insert("cliScript".to_string(), json!(""));
        let extracted = extract_user_data(&raw);
        assert_eq!(extracted.len(), 3);
        assert!(!extracted.contains_key("theme"));
    }
}
