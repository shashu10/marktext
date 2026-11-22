use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::{Map, Value};

/// Preference values keyed by the desktop `schema.json` property names.
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesSnapshot {
    values: BTreeMap<String, Value>,
}

/// A partial preference write. Listeners receive only the keys that changed.
#[derive(Debug, Clone, PartialEq)]
pub struct PreferencesPatch {
    pub changes: BTreeMap<String, Value>,
}

impl PreferencesSnapshot {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    /// Inserts `value` when `key` is declared in the bundled schema.
    /// Unknown keys are ignored so a pane cannot invent preferences the
    /// Electron main process would reject.
    pub fn insert_known(&mut self, key: String, value: Value) -> Option<PreferencesPatch> {
        if !is_known_preference_key(&key) {
            return None;
        }
        if self.values.get(&key) == Some(&value) {
            return None;
        }
        self.values.insert(key.clone(), value.clone());
        let mut changes = BTreeMap::new();
        changes.insert(key, value);
        Some(PreferencesPatch { changes })
    }
}

fn schema() -> &'static Map<String, Value> {
    static SCHEMA: OnceLock<Map<String, Value>> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../packages/desktop/src/shared/preferences/schema.json"
        ))
        .expect("packages/desktop preferences schema.json is a JSON object")
    })
}

pub fn is_known_preference_key(key: &str) -> bool {
    schema().contains_key(key)
}

/// Defaults declared on the Electron preference schema.
pub fn preference_defaults() -> PreferencesSnapshot {
    let mut values = BTreeMap::new();
    for (key, spec) in schema() {
        let Some(default) = spec.get("default") else {
            continue;
        };
        values.insert(key.clone(), default.clone());
    }
    PreferencesSnapshot { values }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_schema_booleans() {
        let defaults = preference_defaults();
        assert_eq!(defaults.get("autoSave"), Some(&Value::Bool(false)));
        assert!(is_known_preference_key("theme"));
        assert!(!is_known_preference_key("not-a-preference"));
    }

    #[test]
    fn unknown_keys_do_not_patch() {
        let mut snapshot = preference_defaults();
        assert!(
            snapshot
                .insert_known("nope".to_string(), Value::Bool(true))
                .is_none()
        );
        let patch = snapshot
            .insert_known("autoSave".to_string(), Value::Bool(true))
            .expect("autoSave is in the schema");
        assert_eq!(patch.changes.get("autoSave"), Some(&Value::Bool(true)));
        assert!(
            snapshot
                .insert_known("autoSave".to_string(), Value::Bool(true))
                .is_none()
        );
    }
}
