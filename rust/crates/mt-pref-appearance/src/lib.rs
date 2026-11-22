//! Appearance preferences pane (`prefs.pane.appearance`).
//!
//! Covers the theme block plus the window and typography groups the Vue
//! category page mounts beside it. The view writes through `HostBridge`.

mod model;
mod view;

pub use model::{AppearanceModel, ThemeMode, font_family_ok, line_width_ok, theme_label};
pub use view::{AppearancePane, PersistPublisher, PreferencePublisher};
