use marktext_core::PreferencesSnapshot;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    All,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppearanceModel {
    pub follow_system_theme: bool,
    pub theme: String,
    pub light_mode_theme: String,
    pub dark_mode_theme: String,
    pub custom_css: String,
    pub title_bar_style: String,
    pub hide_scrollbar: bool,
    pub zoom: f64,
    pub font_size: f64,
    pub code_font_size: f64,
    pub line_height: f64,
    pub editor_font_family: String,
    pub code_font_family: String,
    pub editor_line_width: String,
}

const THEMES: &[&str] = &[
    "ayu-light",
    "light",
    "catppuccin-latte",
    "everforest-light",
    "graphite",
    "gruvbox-light",
    "rose-pine-dawn",
    "solarized-light",
    "tokyo-night-light",
    "ulysses",
    "ayu-dark",
    "ayu-mirage",
    "dark",
    "catppuccin-mocha",
    "cyberdream",
    "dracula",
    "everforest-dark",
    "gruvbox-dark",
    "horizon-dark",
    "kanagawa",
    "material-dark",
    "monokai-pro",
    "nightfox",
    "nord",
    "one-dark",
    "oxocarbon-dark",
    "palenight",
    "rose-pine",
    "rose-pine-moon",
    "solarized-dark",
    "synthwave-84",
    "tokyo-night",
    "tokyo-night-storm",
];

const DARK_THEMES: &[&str] = &[
    "dark",
    "material-dark",
    "dracula",
    "nord",
    "catppuccin-mocha",
    "gruvbox-dark",
    "tokyo-night",
    "tokyo-night-storm",
    "solarized-dark",
    "ayu-dark",
    "ayu-mirage",
    "everforest-dark",
    "rose-pine",
    "rose-pine-moon",
    "monokai-pro",
    "synthwave-84",
    "horizon-dark",
    "palenight",
    "oxocarbon-dark",
    "kanagawa",
    "nightfox",
    "cyberdream",
    "one-dark",
];

const SWATCHES: &[(&str, u32)] = &[
    ("ayu-dark", 0x0A0E14),
    ("ayu-light", 0xFAFAFA),
    ("ayu-mirage", 0x1F2430),
    ("catppuccin-latte", 0xEFF1F5),
    ("catppuccin-mocha", 0x1E1E2E),
    ("cyberdream", 0x16181A),
    ("dark", 0x282828),
    ("dracula", 0x282A36),
    ("everforest-dark", 0x2D353B),
    ("everforest-light", 0xFDF6E3),
    ("graphite", 0xF7F7F7),
    ("gruvbox-dark", 0x282828),
    ("gruvbox-light", 0xFBF1C7),
    ("horizon-dark", 0x1C1E26),
    ("kanagawa", 0x1F1F28),
    ("light", 0xFFFFFF),
    ("material-dark", 0x34393F),
    ("monokai-pro", 0x2D2A2E),
    ("nightfox", 0x192330),
    ("nord", 0x2E3440),
    ("one-dark", 0x282C34),
    ("oxocarbon-dark", 0x161616),
    ("palenight", 0x292D3E),
    ("rose-pine", 0x191724),
    ("rose-pine-dawn", 0xFAF4ED),
    ("rose-pine-moon", 0x232136),
    ("solarized-dark", 0x002B36),
    ("solarized-light", 0xFDF6E3),
    ("synthwave-84", 0x262335),
    ("tokyo-night", 0x1A1B26),
    ("tokyo-night-light", 0xD5D6DB),
    ("tokyo-night-storm", 0x24283B),
    ("ulysses", 0xF3F3F3),
];

pub const ZOOM_STEPS: &[(f64, &str)] = &[
    (0.5, "50%"),
    (0.625, "62.5%"),
    (0.75, "75%"),
    (0.875, "87.5%"),
    (1.0, "100%"),
    (1.125, "112.5%"),
    (1.25, "125%"),
    (1.375, "137.5%"),
    (1.5, "150%"),
    (1.625, "162.5%"),
    (1.75, "175%"),
    (1.875, "187.5%"),
    (2.0, "200%"),
];

impl AppearanceModel {
    pub fn from_snapshot(snapshot: &PreferencesSnapshot) -> Self {
        Self {
            follow_system_theme: bool_pref(snapshot, "followSystemTheme"),
            theme: string_pref(snapshot, "theme", "light"),
            light_mode_theme: string_pref(snapshot, "lightModeTheme", "light"),
            dark_mode_theme: string_pref(snapshot, "darkModeTheme", "dark"),
            custom_css: string_pref(snapshot, "customCss", ""),
            title_bar_style: string_pref(snapshot, "titleBarStyle", "custom"),
            hide_scrollbar: bool_pref(snapshot, "hideScrollbar"),
            zoom: number_pref(snapshot, "zoom", 1.0),
            font_size: number_pref(snapshot, "fontSize", 16.0),
            code_font_size: number_pref(snapshot, "codeFontSize", 14.0),
            line_height: number_pref(snapshot, "lineHeight", 1.6),
            editor_font_family: string_pref(snapshot, "editorFontFamily", "Open Sans"),
            code_font_family: string_pref(snapshot, "codeFontFamily", "DejaVu Sans Mono"),
            editor_line_width: string_pref(snapshot, "editorLineWidth", ""),
        }
    }
}

pub fn is_dark_theme(name: &str) -> bool {
    DARK_THEMES.contains(&name)
}

pub fn theme_label(name: &str) -> String {
    name.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn theme_swatch(name: &str) -> u32 {
    SWATCHES
        .iter()
        .find(|(theme, _)| *theme == name)
        .map(|(_, color)| *color)
        .unwrap_or(0xFFFFFF)
}

pub fn visible_themes(query: &str, mode: ThemeMode) -> Vec<&'static str> {
    let query = query.trim().to_lowercase();
    THEMES
        .iter()
        .copied()
        .filter(|name| {
            let dark = is_dark_theme(name);
            let mode_ok = match mode {
                ThemeMode::All => true,
                ThemeMode::Light => !dark,
                ThemeMode::Dark => dark,
            };
            mode_ok && (query.is_empty() || theme_label(name).to_lowercase().contains(&query))
        })
        .collect()
}

pub fn zoom_selected(value: f64, option: f64) -> bool {
    (value - option).abs() < 0.000_1
}

/// Moves a snapped numeric preference by one step and keeps it inside the schema range.
pub fn step_by(value: f64, delta_steps: i32, step: f64, min: f64, max: f64) -> f64 {
    let current = ((value - min) / step).round();
    let next = current + f64::from(delta_steps);
    (min + next * step).clamp(min, max)
}

pub fn format_step(value: f64, step: f64) -> String {
    if step >= 1.0 {
        format!("{}", value.round() as i32)
    } else {
        format!("{value:.1}")
    }
}

pub fn font_family_ok(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let mut chars = value.chars();
    match (chars.next(), value.chars().next_back()) {
        (Some(first), Some(last)) => !first.is_whitespace() && !last.is_whitespace(),
        _ => false,
    }
}

pub fn line_width_ok(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let Some(unit_at) = value.find(|c: char| !c.is_ascii_digit()) else {
        return false;
    };
    unit_at > 0 && matches!(&value[unit_at..], "ch" | "px" | "%")
}

/// A field still showing `baseline` should take a newer snapshot.
/// Text that has moved off `baseline` is an unsaved edit and stays on screen.
pub fn draft_should_adopt(current: &str, baseline: &str, saved: &str) -> bool {
    current == baseline && current != saved
}

#[derive(Clone, Debug, PartialEq)]
struct PendingText {
    id: u64,
    key: String,
    value: Value,
}

/// One typed setting waiting for `mt::preferences::update`.
#[derive(Debug)]
pub struct TextSave {
    next_id: u64,
    pending: Option<PendingText>,
    pub error_key: Option<String>,
}

impl TextSave {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            pending: None,
            error_key: None,
        }
    }

    pub fn pending_key(&self) -> Option<&str> {
        self.pending.as_ref().map(|pending| pending.key.as_str())
    }

    pub fn request(&mut self, key: String, value: Value) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.pending = Some(PendingText { id, key, value });
        self.error_key = None;
        Some(id)
    }

    pub fn finish(&mut self, id: u64, persisted: bool) -> Option<(String, Value)> {
        let pending = self.pending.take()?;
        if pending.id != id {
            self.pending = Some(pending);
            return None;
        }
        if !persisted {
            self.error_key = Some(pending.key);
            return None;
        }
        self.error_key = None;
        Some((pending.key, pending.value))
    }
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
    snapshot
        .get(key)
        .and_then(Value::as_f64)
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use marktext_core::{HostBridge, MemoryHostBridge};
    use serde_json::json;

    #[test]
    fn theme_catalog_matches_the_vue_picker() {
        assert_eq!(theme_label("tokyo-night-light"), "Tokyo Night Light");
        assert!(is_dark_theme("one-dark"));
        assert!(!is_dark_theme("graphite"));
        assert_eq!(theme_swatch("dracula"), 0x282A36);
        let dark = visible_themes("rose", ThemeMode::Dark);
        assert_eq!(dark, vec!["rose-pine", "rose-pine-moon"]);
        assert!(visible_themes("zzz", ThemeMode::All).is_empty());
        assert!(visible_themes("", ThemeMode::Light).contains(&"ulysses"));
        assert!(!visible_themes("", ThemeMode::Light).contains(&"nord"));
    }

    #[test]
    fn steppers_and_text_rules_follow_the_schema() {
        assert_eq!(step_by(16.0, 1, 1.0, 12.0, 32.0), 17.0);
        assert_eq!(step_by(12.0, -1, 1.0, 12.0, 32.0), 12.0);
        assert_eq!(step_by(1.6, 1, 0.1, 1.2, 2.0), 1.7);
        assert_eq!(step_by(2.0, 1, 0.1, 1.2, 2.0), 2.0);
        assert!(line_width_ok(""));
        assert!(line_width_ok("80ch"));
        assert!(line_width_ok("800px"));
        assert!(line_width_ok("60%"));
        assert!(!line_width_ok("80"));
        assert!(!line_width_ok("px"));
        assert!(font_family_ok(""));
        assert!(font_family_ok("DejaVu Sans Mono"));
        assert!(!font_family_ok(" Open Sans"));
        assert!(zoom_selected(1.125, 1.125));
        assert!(!zoom_selected(1.0, 1.125));
    }

    #[test]
    fn snapshot_reads_appearance_defaults_and_overrides() {
        let bridge = MemoryHostBridge::with_defaults();
        let model = AppearanceModel::from_snapshot(&bridge.get_preferences());
        assert_eq!(model.theme, "light");
        assert!(!model.follow_system_theme);
        assert_eq!(model.font_size, 16.0);
        bridge.set_preference("followSystemTheme".to_string(), json!(true));
        bridge.set_preference("zoom".to_string(), json!(1.5));
        let model = AppearanceModel::from_snapshot(&bridge.get_preferences());
        assert!(model.follow_system_theme);
        assert!(zoom_selected(model.zoom, 1.5));
    }

    #[test]
    fn typed_setting_applies_only_after_persisted() {
        let mut save = TextSave::new();
        let id = save
            .request("editorLineWidth".to_string(), json!("80ch"))
            .expect("first save");
        assert!(save.request("customCss".to_string(), json!("")).is_none());
        assert!(save.finish(id, false).is_none());
        assert_eq!(save.error_key.as_deref(), Some("editorLineWidth"));
        let id = save
            .request("editorLineWidth".to_string(), json!("80ch"))
            .expect("retry after failure");
        assert_eq!(
            save.finish(id, true),
            Some(("editorLineWidth".to_string(), json!("80ch")))
        );
        assert!(save.error_key.is_none());
    }

    #[test]
    fn clean_draft_adopts_saved_text_and_an_edit_stays() {
        assert!(draft_should_adopt("", "", "h1 { color: red }"));
        assert!(draft_should_adopt("Open Sans", "Open Sans", "Iosevka"));
        assert!(!draft_should_adopt(
            "h1 { color: blue }",
            "",
            "h1 { color: red }"
        ));
        assert!(!draft_should_adopt("Iosevka", "Open Sans", "Open Sans"));
        assert!(!draft_should_adopt("80ch", "80ch", "80ch"));
    }
}
