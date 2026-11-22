use marktext_core::PreferencesSnapshot;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub value: &'static str,
    pub label: &'static str,
}

/// Values shown on the Files category: auto-save, startup/recovery, and file
/// representation. Other groups stay on General, Appearance, or Editor.
#[derive(Clone, Debug, PartialEq)]
pub struct FilesModel {
    pub auto_save: bool,
    pub auto_save_delay: f64,
    pub start_up_action: String,
    pub default_directory_to_open: String,
    pub restore_layout_state: bool,
    pub restore_editors: bool,
    pub restore_view_state: bool,
    pub end_of_line: String,
    pub default_encoding: String,
    pub auto_guess_encoding: bool,
    pub auto_normalize_line_endings: bool,
    pub trim_trailing_newline: i64,
}

const STARTUP_ACTIONS: &[Choice] = &[
    Choice {
        value: "restoreAll",
        label: "Restore all windows",
    },
    Choice {
        value: "restoreLastWindow",
        label: "Restore the last active window",
    },
    Choice {
        value: "openLastFolder",
        label: "Open the last workspace",
    },
    Choice {
        value: "folder",
        label: "Open the selected folder",
    },
    Choice {
        value: "blank",
        label: "New blank document",
    },
];

const END_OF_LINE: &[Choice] = &[
    Choice {
        value: "default",
        label: "Default",
    },
    Choice {
        value: "crlf",
        label: "CRLF(Windows)",
    },
    Choice {
        value: "lf",
        label: "LF(Linux/Mac)",
    },
];

const TRIM_TRAILING: &[Choice] = &[
    Choice {
        value: "0",
        label: "Trim All",
    },
    Choice {
        value: "1",
        label: "Ensure One",
    },
    Choice {
        value: "2",
        label: "Preserve",
    },
    Choice {
        value: "3",
        label: "Do Nothing",
    },
];

/// Same order and labels as `common/encoding.ts` `ENCODING_NAME_MAP`.
const ENCODINGS: &[Choice] = &[
    Choice {
        value: "utf8",
        label: "UTF-8",
    },
    Choice {
        value: "utf16be",
        label: "UTF-16 BE",
    },
    Choice {
        value: "utf16le",
        label: "UTF-16 LE",
    },
    Choice {
        value: "utf32be",
        label: "UTF-32 BE",
    },
    Choice {
        value: "utf32le",
        label: "UTF-32 LE",
    },
    Choice {
        value: "ascii",
        label: "Western (ISO 8859-1)",
    },
    Choice {
        value: "latin3",
        label: "Western (ISO 8859-3)",
    },
    Choice {
        value: "iso885915",
        label: "Western (ISO 8859-15)",
    },
    Choice {
        value: "cp1252",
        label: "Western (Windows 1252)",
    },
    Choice {
        value: "arabic",
        label: "Arabic (ISO 8859-6)",
    },
    Choice {
        value: "cp1256",
        label: "Arabic (Windows 1256)",
    },
    Choice {
        value: "latin4",
        label: "Baltic (ISO 8859-4)",
    },
    Choice {
        value: "cp1257",
        label: "Baltic (Windows 1257)",
    },
    Choice {
        value: "iso88592",
        label: "Central European (ISO 8859-2)",
    },
    Choice {
        value: "windows1250",
        label: "Central European (Windows 1250)",
    },
    Choice {
        value: "cp866",
        label: "Cyrillic (CP 866)",
    },
    Choice {
        value: "iso88595",
        label: "Cyrillic (ISO 8859-5)",
    },
    Choice {
        value: "koi8r",
        label: "Cyrillic (KOI8-R)",
    },
    Choice {
        value: "koi8u",
        label: "Cyrillic (KOI8-U)",
    },
    Choice {
        value: "cp1251",
        label: "Cyrillic (Windows 1251)",
    },
    Choice {
        value: "iso885913",
        label: "Estonian (ISO 8859-13)",
    },
    Choice {
        value: "greek",
        label: "Greek (ISO 8859-7)",
    },
    Choice {
        value: "cp1253",
        label: "Greek (Windows 1253)",
    },
    Choice {
        value: "hebrew",
        label: "Hebrew (ISO 8859-8)",
    },
    Choice {
        value: "cp1255",
        label: "Hebrew (Windows 1255)",
    },
    Choice {
        value: "latin5",
        label: "Turkish (ISO 8859-9)",
    },
    Choice {
        value: "cp1254",
        label: "Turkish (Windows 1254)",
    },
    Choice {
        value: "gb2312",
        label: "Simplified Chinese (GB2312)",
    },
    Choice {
        value: "gb18030",
        label: "Simplified Chinese (GB18030)",
    },
    Choice {
        value: "gbk",
        label: "Simplified Chinese (GBK)",
    },
    Choice {
        value: "big5",
        label: "Traditional Chinese (Big5)",
    },
    Choice {
        value: "big5hkscs",
        label: "Traditional Chinese (Big5-HKSCS)",
    },
    Choice {
        value: "shiftjis",
        label: "Japanese (Shift JIS)",
    },
    Choice {
        value: "eucjp",
        label: "Japanese (EUC-JP)",
    },
    Choice {
        value: "euckr",
        label: "Korean (EUC-KR)",
    },
    Choice {
        value: "latin6",
        label: "Nordic (ISO 8859-10)",
    },
];

pub const AUTO_SAVE_DELAY_MIN: f64 = 1000.0;
pub const AUTO_SAVE_DELAY_MAX: f64 = 10000.0;
pub const AUTO_SAVE_DELAY_STEP: f64 = 100.0;

impl FilesModel {
    pub fn from_snapshot(snapshot: &PreferencesSnapshot) -> Self {
        Self {
            auto_save: bool_pref(snapshot, "autoSave"),
            auto_save_delay: number_pref(snapshot, "autoSaveDelay", 5000.0),
            start_up_action: string_pref(snapshot, "startUpAction", "restoreAll"),
            default_directory_to_open: string_pref(snapshot, "defaultDirectoryToOpen", ""),
            restore_layout_state: bool_pref(snapshot, "restoreLayoutState"),
            restore_editors: bool_pref(snapshot, "restoreEditors"),
            restore_view_state: bool_pref(snapshot, "restoreViewState"),
            end_of_line: string_pref(snapshot, "endOfLine", "default"),
            default_encoding: string_pref(snapshot, "defaultEncoding", "utf8"),
            auto_guess_encoding: bool_pref(snapshot, "autoGuessEncoding"),
            auto_normalize_line_endings: bool_pref(snapshot, "autoNormalizeLineEndings"),
            trim_trailing_newline: int_pref(snapshot, "trimTrailingNewline", 2),
        }
    }

    pub fn startup_choices() -> &'static [Choice] {
        STARTUP_ACTIONS
    }

    pub fn end_of_line_choices() -> &'static [Choice] {
        END_OF_LINE
    }

    pub fn encoding_choices() -> &'static [Choice] {
        ENCODINGS
    }

    pub fn trim_trailing_choices() -> &'static [Choice] {
        TRIM_TRAILING
    }

    pub fn shows_directory_picker(&self) -> bool {
        self.start_up_action == "folder"
    }

    /// Same clamp and step as the Vue range control (1000–10000, step 100).
    pub fn delay_after_step(&self, delta_steps: i32) -> f64 {
        let next = self.auto_save_delay + f64::from(delta_steps) * AUTO_SAVE_DELAY_STEP;
        snap_delay(next)
    }
}

pub fn snap_delay(value: f64) -> f64 {
    let clamped = value.clamp(AUTO_SAVE_DELAY_MIN, AUTO_SAVE_DELAY_MAX);
    (clamped / AUTO_SAVE_DELAY_STEP).round() * AUTO_SAVE_DELAY_STEP
}

/// One recovery write waiting for `mt::preferences::update` to answer.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingPersist {
    pub id: u64,
    pub changes: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PersistDecision {
    Ignored,
    Failed,
    Applied(PendingPersist),
}

/// At most one recovery write is in flight. Another click is ignored until it finishes.
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

    pub fn request(&mut self, changes: BTreeMap<String, Value>) -> Option<u64> {
        if self.pending.is_some() || changes.is_empty() {
            return None;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.pending = Some(PendingPersist { id, changes });
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

pub fn single_change(key: &str, value: Value) -> BTreeMap<String, Value> {
    let mut changes = BTreeMap::new();
    changes.insert(key.to_string(), value);
    changes
}

pub fn folder_selection_changes(directory: &str) -> BTreeMap<String, Value> {
    let mut changes = BTreeMap::new();
    changes.insert(
        "startUpAction".to_string(),
        Value::String("folder".to_string()),
    );
    changes.insert(
        "defaultDirectoryToOpen".to_string(),
        Value::String(directory.to_string()),
    );
    changes
}

/// Host `PERSIST_PREFERENCE` carries one key. Multi-key recovery patches use a
/// JSON object value with an empty key; the Vue host forwards the object as the
/// `mt::preferences::update` patch.
pub fn persist_wire(changes: &BTreeMap<String, Value>) -> (String, Value) {
    if changes.len() == 1 {
        let (key, value) = changes.iter().next().expect("len checked");
        return (key.clone(), value.clone());
    }
    let mut object = Map::new();
    for (key, value) in changes {
        object.insert(key.clone(), value.clone());
    }
    (String::new(), Value::Object(object))
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

fn number_pref(snapshot: &PreferencesSnapshot, key: &str, fallback: f64) -> f64 {
    match snapshot.get(key) {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(fallback),
        _ => fallback,
    }
}

fn int_pref(snapshot: &PreferencesSnapshot, key: &str, fallback: i64) -> i64 {
    match snapshot.get(key) {
        Some(Value::Number(number)) => number.as_i64().unwrap_or(fallback),
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marktext_core::{HostBridge, MemoryHostBridge};
    use serde_json::json;

    #[test]
    fn snapshot_values_follow_the_files_page() {
        let bridge = MemoryHostBridge::with_defaults();
        bridge.set_preference("autoSave".to_string(), json!(true));
        bridge.set_preference("autoSaveDelay".to_string(), json!(2500));
        bridge.set_preference("startUpAction".to_string(), json!("folder"));
        bridge.set_preference("defaultDirectoryToOpen".to_string(), json!("/tmp/notes"));
        bridge.set_preference("endOfLine".to_string(), json!("lf"));
        bridge.set_preference("trimTrailingNewline".to_string(), json!(0));
        bridge.set_preference("defaultEncoding".to_string(), json!("utf16le"));

        let model = FilesModel::from_snapshot(&bridge.get_preferences());
        assert!(model.auto_save);
        assert_eq!(model.auto_save_delay, 2500.0);
        assert!(model.shows_directory_picker());
        assert_eq!(model.default_directory_to_open, "/tmp/notes");
        assert_eq!(model.end_of_line, "lf");
        assert_eq!(model.trim_trailing_newline, 0);
        assert_eq!(model.default_encoding, "utf16le");
        assert_eq!(FilesModel::encoding_choices()[0].value, "utf8");
        assert_eq!(FilesModel::startup_choices()[4].value, "blank");
    }

    #[test]
    fn auto_save_delay_snaps_to_the_vue_range() {
        let bridge = MemoryHostBridge::with_defaults();
        let model = FilesModel::from_snapshot(&bridge.get_preferences());
        assert_eq!(model.delay_after_step(-100), 1000.0);
        assert_eq!(model.delay_after_step(100), 10000.0);
        assert_eq!(snap_delay(2349.0), 2300.0);
        assert_eq!(snap_delay(2350.0), 2400.0);
    }

    #[test]
    fn recovery_persist_gate_blocks_overlap_and_keeps_failed_drafts() {
        let mut gate = PersistGate::new();
        let id = gate
            .request(single_change("restoreEditors", json!(false)))
            .expect("first edit is accepted");
        assert!(
            gate.request(single_change("restoreViewState", json!(false)))
                .is_none()
        );
        assert_eq!(gate.finish(id + 1, true), PersistDecision::Ignored);
        assert_eq!(gate.finish(id, false), PersistDecision::Failed);
        assert!(gate.save_error);

        let id = gate
            .request(folder_selection_changes("/home/notes"))
            .expect("the failed edit no longer blocks");
        assert!(!gate.save_error);
        match gate.finish(id, true) {
            PersistDecision::Applied(pending) => {
                assert_eq!(pending.changes.get("startUpAction"), Some(&json!("folder")));
                assert_eq!(
                    pending.changes.get("defaultDirectoryToOpen"),
                    Some(&json!("/home/notes"))
                );
                let (key, value) = persist_wire(&pending.changes);
                assert_eq!(key, "");
                assert_eq!(
                    value,
                    json!({
                        "defaultDirectoryToOpen": "/home/notes",
                        "startUpAction": "folder"
                    })
                );
            }
            other => panic!("expected the saved folder patch, got {other:?}"),
        }
        let (key, value) = persist_wire(&single_change("restoreLayoutState", json!(true)));
        assert_eq!(key, "restoreLayoutState");
        assert_eq!(value, json!(true));
    }
}
