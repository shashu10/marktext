//! Markdown and diagrams preferences pane (`prefs.pane.markdown`).
//!
//! The view reads and writes the shared [`marktext_core::MemoryHostBridge`].
//! It does not touch Electron IPC. The wasm host publishes each write.

mod model;
mod view;

pub use model::{Choice, IndentValue, MarkdownModel, PersistDecision, PersistGate, plantuml_url_ok};
pub use view::{MarkdownPane, PreferencePublisher, PersistPublisher};
