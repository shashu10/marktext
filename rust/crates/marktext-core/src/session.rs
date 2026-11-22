use crate::bridge::MemoryHostBridge;
use crate::messages::{GuestToHost, HostToGuest};
use crate::router::PaneRouter;

/// One GPUI canvas: the active pane plus the preference snapshot Electron posted.
///
/// `PREFERENCES_APPLIED` is sent only after the snapshot or patch is stored.
/// `PaneRouter` does not acknowledge those messages on its own.
pub struct GuestSession {
    router: PaneRouter,
    bridge: MemoryHostBridge,
}

impl GuestSession {
    pub fn new(initial_pane: impl Into<String>) -> Self {
        Self {
            router: PaneRouter::new(initial_pane),
            bridge: MemoryHostBridge::with_defaults(),
        }
    }

    pub fn active_pane(&self) -> &str {
        self.router.active()
    }

    pub fn bridge(&self) -> &MemoryHostBridge {
        &self.bridge
    }

    pub fn apply(&mut self, message: &HostToGuest) -> Option<GuestToHost> {
        match message {
            HostToGuest::Preferences { snapshot } => {
                self.bridge.replace_known(snapshot);
                Some(GuestToHost::PreferencesApplied)
            }
            HostToGuest::PreferencePatch { changes } => {
                self.bridge.apply_changes(changes);
                Some(GuestToHost::PreferencesApplied)
            }
            other => self.router.handle(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::HostBridge;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    fn map(value: serde_json::Value) -> BTreeMap<String, Value> {
        serde_json::from_value(value).expect("object")
    }

    #[test]
    fn preference_messages_are_stored_before_they_are_acknowledged() {
        let session = GuestSession::new("general");
        let seen = Arc::new(Mutex::new(0_usize));
        let seen_listener = Arc::clone(&seen);
        session.bridge().subscribe_preferences(Box::new(move |_| {
            *seen_listener.lock().expect("seen lock") += 1;
        }));

        let mut session = session;
        let reply = session.apply(&HostToGuest::Preferences {
            snapshot: map(json!({ "autoSave": true, "nope": true })),
        });
        assert_eq!(reply, Some(GuestToHost::PreferencesApplied));
        assert_eq!(
            session.bridge().get_preferences().get("autoSave"),
            Some(&Value::Bool(true))
        );
        assert!(session.bridge().get_preferences().get("nope").is_none());
        assert_eq!(*seen.lock().expect("seen lock"), 1);

        let reply = session.apply(&HostToGuest::PreferencePatch {
            changes: map(json!({ "autoSave": false, "also-nope": 1 })),
        });
        assert_eq!(reply, Some(GuestToHost::PreferencesApplied));
        assert_eq!(
            session.bridge().get_preferences().get("autoSave"),
            Some(&Value::Bool(false))
        );
        assert_eq!(*seen.lock().expect("seen lock"), 2);
        assert_eq!(session.active_pane(), "general");
    }
}
