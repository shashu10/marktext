//! General preferences pane (`prefs.pane.general`).
//!
//! The view reads and writes the shared [`marktext_core::MemoryHostBridge`].
//! It does not touch Electron IPC. The wasm host publishes each write.

mod model;
mod view;

pub use model::{Choice, GeneralModel, PandocSwitch, pandoc_switch_state, string_array};
pub use view::{GeneralPane, PreferencePublisher};
