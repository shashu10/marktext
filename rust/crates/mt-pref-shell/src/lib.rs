//! Preferences shell (`prefs.sidebar`): category sidebar, global search,
//! and the English preference registry used for deep-links.
//!
//! Shared Vue form controls under `prefComponents/common/` stay in
//! `marktext-ui-kit` for Stage 1; this crate owns navigation + search parity.

mod categories;
mod model;
mod registry;
mod search;
mod view;

pub use categories::{
    PREFERENCE_CATEGORIES, PreferenceCategory, category_by_id, category_en, category_path,
};
pub use model::{
    MAX_SUGGESTIONS, NAVIGATE_PREFERENCE_ACTION, NavigatePreference, PrefShellModel,
    SEARCH_PLACEHOLDER, SIDEBAR_TITLE,
};
pub use registry::{
    PREFERENCE_DEFINITIONS, PlatformFilter, PreferenceDefinition, setting_target_id,
};
pub use search::{
    SearchEntry, SearchKind, accelerators_equal_normalized, filter_entries,
    keybinding_search_entries, matches_search, setting_search_entries,
};
pub use view::{HostActionPublisher, KeybindingsRequester, PrefShellPane};
