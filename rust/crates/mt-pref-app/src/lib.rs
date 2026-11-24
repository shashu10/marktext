//! Native Preferences window for Stage 2 (`window.preferences=gpui-native`).
//!
//! Reuses the Wave 1A `mt-pref-*` panes. Electron Main speaks the same
//! `HostToGuest` / `GuestToHost` messages as `<GpuiPaneHost>`, framed as NDJSON
//! on the child process stdio (`NativeIpcBridge`).

mod ndjson;
mod view;

pub use ndjson::{
    encode_host_line, is_json_object_line, parse_guest_line, spawn_stdin_reader, write_guest,
};
pub use view::PrefAppRoot;
