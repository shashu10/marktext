use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    #[serde(rename = "SCROLL_TO_HEADER")]
    ScrollToHeader { slug: String },
    #[serde(rename = "TOGGLE_READING_MODE")]
    ToggleReadingMode {
        #[serde(rename = "readOnly")]
        read_only: bool,
    },
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
