//! `NativeIpcBridge`: newline-delimited JSON framing for `HostToGuest` /
//! `GuestToHost` over stdio. Same serde shapes as the Stage 1 postMessage
//! bridge — only the transport changes.

use std::io::{BufRead, Write};
use std::sync::Mutex;

use marktext_core::{GuestToHost, HostToGuest};
use serde_json::Value;

static STDOUT: Mutex<()> = Mutex::new(());

/// Write one guest message as a single JSON line on stdout.
pub fn write_guest(message: &GuestToHost) {
    let Ok(line) = serde_json::to_string(message) else {
        return;
    };
    let _guard = STDOUT.lock().expect("stdout lock");
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// Blocking stdin reader. Each non-empty line is one `HostToGuest` message.
pub fn spawn_stdin_reader(tx: async_channel::Sender<HostToGuest>) {
    std::thread::Builder::new()
        .name("mt-pref-ndjson-stdin".into())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut lines = stdin.lock().lines();
            while let Some(Ok(line)) = lines.next() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<HostToGuest>(trimmed) {
                    Ok(message) => {
                        if tx.send_blocking(message).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("mt-pref-app: drop malformed host line: {error}");
                    }
                }
            }
        })
        .expect("stdin reader thread");
}

/// Parse a guest line the Electron host receives on the child stdout.
pub fn parse_guest_line(line: &str) -> Option<GuestToHost> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

/// Encode a host line Electron writes to the child stdin.
pub fn encode_host_line(message: &HostToGuest) -> Option<String> {
    serde_json::to_string(message).ok()
}

/// Loose JSON object check used by unit tests for round-trips.
pub fn is_json_object_line(line: &str) -> bool {
    matches!(serde_json::from_str::<Value>(line.trim()), Ok(Value::Object(_)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use marktext_core::GuestToHost;
    use mt_pref_shell::NAVIGATE_PREFERENCE_ACTION;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn navigate_category(payload: &Value) -> Option<&str> {
        payload
            .get("category")
            .and_then(Value::as_str)
            .filter(|category| !category.is_empty())
    }

    #[test]
    fn navigate_preference_payload_reads_category() {
        assert_eq!(
            navigate_category(&json!({ "category": "editor", "key": "tabSize" })),
            Some("editor")
        );
        assert!(navigate_category(&json!({})).is_none());
        assert_eq!(NAVIGATE_PREFERENCE_ACTION, "navigate-preference");
    }

    #[test]
    fn host_and_guest_lines_round_trip() {
        let host = HostToGuest::SetPane {
            pane: "general".into(),
        };
        let line = encode_host_line(&host).unwrap();
        assert!(is_json_object_line(&line));
        let parsed: HostToGuest = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed, host);

        let guest = GuestToHost::SetPreference {
            key: "autoSave".into(),
            value: json!(true),
        };
        let raw = serde_json::to_string(&guest).unwrap();
        let parsed = parse_guest_line(&raw).unwrap();
        assert_eq!(parsed, guest);
    }

    #[test]
    fn preferences_snapshot_line_keeps_keys() {
        let mut snapshot = BTreeMap::new();
        snapshot.insert("theme".into(), json!("dark"));
        let host = HostToGuest::Preferences { snapshot };
        let line = encode_host_line(&host).unwrap();
        let parsed: HostToGuest = serde_json::from_str(&line).unwrap();
        match parsed {
            HostToGuest::Preferences { snapshot } => {
                assert_eq!(snapshot.get("theme"), Some(&json!("dark")));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
