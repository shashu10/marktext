use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::preferences::{PreferencesPatch, PreferencesSnapshot, preference_defaults};

/// Transport-agnostic host used by every GPUI view.
///
/// Stage 1 implements this with `postMessage` inside the Electron iframe.
/// Stage 2 swaps in a native IPC adapter. Stage 3 calls `marktext-core` directly.
/// Views never read `preferences.json` or Electron IPC themselves.
pub trait HostBridge: Send {
    fn get_preferences(&self) -> PreferencesSnapshot;
    fn set_preference(&self, key: String, value: Value);
    fn subscribe_preferences(&self, on_patch: Box<dyn Fn(PreferencesPatch) + Send>);
    fn scroll_to_header(&self, slug: String);
    fn toggle_reading_mode(&self, read_only: bool);
}

type PreferenceListener = Box<dyn Fn(PreferencesPatch) + Send>;

/// In-memory bridge for pane unit tests and the native `cargo test` host.
pub struct MemoryHostBridge {
    snapshot: Mutex<PreferencesSnapshot>,
    listeners: Mutex<Vec<PreferenceListener>>,
    scrolls: Mutex<Vec<String>>,
    reading_mode: Mutex<Vec<bool>>,
}

impl MemoryHostBridge {
    pub fn with_defaults() -> Self {
        Self {
            snapshot: Mutex::new(preference_defaults()),
            listeners: Mutex::new(Vec::new()),
            scrolls: Mutex::new(Vec::new()),
            reading_mode: Mutex::new(Vec::new()),
        }
    }

    pub fn scrolls(&self) -> Vec<String> {
        self.scrolls.lock().expect("scroll lock").clone()
    }

    pub fn reading_mode(&self) -> Vec<bool> {
        self.reading_mode.lock().expect("reading-mode lock").clone()
    }

    /// Host transport: install a full snapshot, then tell subscribers what changed.
    pub fn replace_known(&self, incoming: &BTreeMap<String, Value>) {
        let patch = self
            .snapshot
            .lock()
            .expect("preference lock")
            .replace_known(incoming);
        self.notify(patch);
    }

    /// Host transport: apply a partial update from `PREFERENCE_PATCH`.
    pub fn apply_changes(&self, changes: &BTreeMap<String, Value>) {
        let patch = self
            .snapshot
            .lock()
            .expect("preference lock")
            .apply_known(changes);
        self.notify(patch);
    }

    fn notify(&self, patch: PreferencesPatch) {
        if patch.changes.is_empty() {
            return;
        }
        for listener in self.listeners.lock().expect("listener lock").iter() {
            listener(patch.clone());
        }
    }
}

impl HostBridge for MemoryHostBridge {
    fn get_preferences(&self) -> PreferencesSnapshot {
        self.snapshot.lock().expect("preference lock").clone()
    }

    fn set_preference(&self, key: String, value: Value) {
        let Some(patch) = self
            .snapshot
            .lock()
            .expect("preference lock")
            .insert_known(key, value)
        else {
            return;
        };
        self.notify(patch);
    }

    fn subscribe_preferences(&self, on_patch: Box<dyn Fn(PreferencesPatch) + Send>) {
        self.listeners.lock().expect("listener lock").push(on_patch);
    }

    fn scroll_to_header(&self, slug: String) {
        self.scrolls.lock().expect("scroll lock").push(slug);
    }

    fn toggle_reading_mode(&self, read_only: bool) {
        self.reading_mode
            .lock()
            .expect("reading-mode lock")
            .push(read_only);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    #[test]
    fn writes_notify_subscribers_and_commands_are_recorded() {
        let bridge = MemoryHostBridge::with_defaults();
        let seen = Arc::new(StdMutex::new(Vec::new()));
        let seen_listener = Arc::clone(&seen);
        bridge.subscribe_preferences(Box::new(move |patch| {
            seen_listener.lock().expect("seen lock").push(patch.changes);
        }));

        bridge.set_preference("autoSave".to_string(), Value::Bool(true));
        bridge.set_preference("not-real".to_string(), Value::Bool(true));
        bridge.scroll_to_header("intro".to_string());
        bridge.toggle_reading_mode(true);

        assert_eq!(
            bridge.get_preferences().get("autoSave"),
            Some(&Value::Bool(true))
        );
        let patches = seen.lock().expect("seen lock");
        assert_eq!(patches.len(), 1);
        assert_eq!(bridge.scrolls(), vec!["intro".to_string()]);
        assert_eq!(bridge.reading_mode(), vec![true]);
    }
}
