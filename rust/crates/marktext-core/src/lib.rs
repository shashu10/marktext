//! Contracts every migrated pane uses to talk to Electron, a native window, or
//! the in-process core. UI crates depend on this module and not on a transport.

mod bridge;
mod messages;
mod preferences;
mod router;
mod tokens;

pub use bridge::{HostBridge, MemoryHostBridge};
pub use messages::{GuestToHost, HostToGuest};
pub use preferences::{
    PreferencesPatch, PreferencesSnapshot, is_known_preference_key, preference_defaults,
};
pub use router::PaneRouter;
pub use tokens::{ThemeTokens, category_label, theme_tokens};
