//! Files & Startup/Recovery preferences pane (`prefs.pane.files`).
//!
//! The view reads and writes the shared [`marktext_core::MemoryHostBridge`].
//! It does not touch Electron IPC. The wasm host publishes each write.

mod model;
mod view;

pub use model::{
    Choice, FilesModel, PersistDecision, PersistGate, folder_selection_changes, persist_wire,
    single_change, snap_delay,
};
pub use view::{ChooseDirectoryPublisher, FilesPane, PersistPublisher, PreferencePublisher};
