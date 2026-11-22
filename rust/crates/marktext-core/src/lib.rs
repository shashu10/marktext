//! Contracts every migrated pane uses to talk to Electron, a native window, or
//! the in-process core. UI crates depend on this module and not on a transport.

mod bridge;
mod messages;
mod preferences;
mod router;
mod session;
mod tokens;

pub use bridge::{HostBridge, MemoryHostBridge};
pub use messages::{GuestToHost, HostToGuest, KeybindingEntry};
pub use preferences::{
    PreferencesPatch, PreferencesSnapshot, is_known_preference_key, preference_defaults,
};
pub use router::PaneRouter;
pub use session::GuestSession;
pub use tokens::{ThemeTokens, category_label, theme_tokens};
