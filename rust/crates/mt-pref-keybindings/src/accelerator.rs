//! Electron accelerator helpers used by the shortcut recorder.
//!
//! Layout-aware mapping stays in Electron. This module covers the US-layout
//! chords the preferences recorder needs for Stage 1.

use gpui::Keystroke;

/// Human-readable label for a stored Electron accelerator.
pub fn accelerator_label(accelerator: &str, is_mac: bool) -> String {
    if accelerator.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = accelerator
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let mut modifiers: Vec<String> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "cmdorctrl" | "commandorcontrol" | "command" | "cmd" | "meta" | "super" => {
                modifiers.push(if is_mac { "Cmd" } else { "Ctrl" }.to_string());
            }
            "control" | "ctrl" => modifiers.push("Ctrl".to_string()),
            "shift" => modifiers.push("Shift".to_string()),
            "option" | "alt" => {
                modifiers.push(if is_mac { "Option" } else { "Alt" }.to_string())
            }
            "plus" => keys.push("+".to_string()),
            other => {
                if other.len() == 1 {
                    keys.push(other.to_ascii_uppercase());
                } else {
                    keys.push(part.to_string());
                }
            }
        }
    }
    let order = if is_mac {
        ["Ctrl", "Option", "Shift", "Cmd"]
    } else {
        ["Ctrl", "Alt", "Shift", "Win"]
    };
    modifiers.sort_by_key(|name| order.iter().position(|item| item == name).unwrap_or(99));
    modifiers.dedup();
    let mut out = modifiers;
    out.extend(keys);
    if is_mac {
        out.join("")
    } else {
        out.join("+")
    }
}

/// Same rule as the legacy configurator's fallback equality path.
pub fn accelerators_equal(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let left = normalize_parts(a);
    let right = normalize_parts(b);
    if left.is_empty() && right.is_empty() {
        return true;
    }
    if left.len() != right.len() {
        return false;
    }
    let set: std::collections::BTreeSet<_> = left.iter().chain(right.iter()).collect();
    set.len() == right.len()
}

fn normalize_parts(accelerator: &str) -> Vec<String> {
    accelerator
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            match lower.as_str() {
                "commandorcontrol" | "cmdorctrl" | "command" | "cmd" | "meta" => "ctrl".to_string(),
                "control" => "ctrl".to_string(),
                "option" => "alt".to_string(),
                other => other.to_string(),
            }
        })
        .collect()
}

/// Mirrors `isValidAccelerator` for Linux/Windows (Cmd becomes Ctrl).
pub fn is_valid_accelerator(accelerator: &str, is_mac: bool) -> bool {
    if accelerator.is_empty() {
        return true;
    }
    let parts: Vec<&str> = accelerator
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return false;
    }
    let mut seen_key = false;
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    for part in parts {
        let lower = part.to_ascii_lowercase();
        match lower.as_str() {
            "cmd" | "command" => {
                if !is_mac || meta {
                    return false;
                }
                meta = true;
            }
            "super" => {
                if meta {
                    return false;
                }
                meta = true;
            }
            "ctrl" | "control" => {
                if ctrl {
                    return false;
                }
                ctrl = true;
            }
            "cmdorctrl" | "commandorcontrol" => {
                if is_mac {
                    if meta {
                        return false;
                    }
                    meta = true;
                } else {
                    if ctrl {
                        return false;
                    }
                    ctrl = true;
                }
            }
            "option" => {
                if !is_mac || alt {
                    return false;
                }
                alt = true;
            }
            "alt" | "altgr" => {
                if alt {
                    return false;
                }
                alt = true;
            }
            "shift" => {
                if shift {
                    return false;
                }
                shift = true;
            }
            "plus" | "space" | "tab" | "backspace" | "delete" | "insert" | "return" | "enter"
            | "up" | "down" | "left" | "right" | "home" | "end" | "pageup" | "pagedown"
            | "escape" | "esc" => {
                if seen_key {
                    return false;
                }
                seen_key = true;
            }
            other if other.len() == 1 => {
                if seen_key {
                    return false;
                }
                seen_key = true;
            }
            other if other.starts_with('f')
                && other.len() <= 3
                && other[1..].chars().all(|c| c.is_ascii_digit()) =>
            {
                if seen_key {
                    return false;
                }
                seen_key = true;
            }
            other if other.starts_with("num") && other.len() == 4 && other.as_bytes()[3].is_ascii_digit() =>
            {
                if seen_key {
                    return false;
                }
                seen_key = true;
            }
            _ => return false,
        }
    }
    seen_key
}

/// Build an Electron accelerator from a GPUI keystroke. Modifier-only strokes
/// return `None`. Escape with no modifiers is handled by the caller.
pub fn record_keystroke(keystroke: &Keystroke, is_mac: bool) -> Option<(String, bool)> {
    let key = keystroke.key.as_str();
    if matches!(key, "control" | "alt" | "shift" | "meta" | "cmd" | "super") {
        return None;
    }
    let mut parts = Vec::new();
    if keystroke.modifiers.control {
        parts.push(if is_mac { "ctrl" } else { "ctrl" });
    }
    if keystroke.modifiers.alt {
        parts.push("alt");
    }
    if keystroke.modifiers.shift {
        parts.push("shift");
    }
    if keystroke.modifiers.platform {
        parts.push(if is_mac { "cmd" } else { "super" });
    }
    let key_part = match key {
        "+" => "plus",
        " " => "space",
        "escape" => "escape",
        "enter" | "return" => "enter",
        "arrowup" => "up",
        "arrowdown" => "down",
        "arrowleft" => "left",
        "arrowright" => "right",
        "pageup" => "pageup",
        "pagedown" => "pagedown",
        "backspace" => "backspace",
        "delete" => "delete",
        "tab" => "tab",
        "home" => "home",
        "end" => "end",
        "insert" => "insert",
        other if other.len() == 1 => other,
        other => other,
    };
    parts.push(key_part);
    let accelerator = parts.join("+");
    let unmodified = !keystroke.modifiers.control
        && !keystroke.modifiers.alt
        && !keystroke.modifiers.shift
        && !keystroke.modifiers.platform;
    let valid = is_valid_accelerator(&accelerator, is_mac)
        && (key_part.len() > 1 || !unmodified || key_part == "plus");
    // Single unmodified letters are not valid Electron shortcuts for MarkText.
    let valid = if unmodified && key_part.len() == 1 {
        false
    } else {
        valid
    };
    Some((accelerator, valid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_platform_words() {
        assert_eq!(accelerator_label("Ctrl+S", false), "Ctrl+S");
        assert_eq!(accelerator_label("CmdOrCtrl+Shift+S", false), "Ctrl+Shift+S");
        assert_eq!(accelerator_label("", false), "");
    }

    #[test]
    fn equality_ignores_modifier_aliases() {
        assert!(accelerators_equal("Ctrl+S", "ctrl+s"));
        assert!(accelerators_equal("CmdOrCtrl+S", "Ctrl+S"));
        assert!(!accelerators_equal("Ctrl+S", "Ctrl+Shift+S"));
        assert!(accelerators_equal("", ""));
    }

    #[test]
    fn validation_rejects_empty_keys_and_duplicates() {
        assert!(is_valid_accelerator("Ctrl+S", false));
        assert!(is_valid_accelerator("F3", false));
        assert!(is_valid_accelerator("", false));
        assert!(!is_valid_accelerator("Ctrl", false));
        assert!(!is_valid_accelerator("Ctrl+Ctrl+S", false));
        assert!(!is_valid_accelerator("Cmd+S", false));
    }
}
