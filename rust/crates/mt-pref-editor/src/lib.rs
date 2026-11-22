//! Editor & spellchecker preferences pane (`prefs.pane.editor`).
//!
//! Covers the Editor category groups `writing` and `code`, plus the embedded
//! spellchecker block from `categoryPage.vue`. Typography and file
//! representation stay on Appearance and Files.

mod model;
mod view;

pub use model::{
    Choice, EditorModel, SpellcheckerHostState, TabSize, WordRemovalDecision, WordRemovalGate,
};
pub use view::{EditorPane, PreferencePublisher, WordRemovalPublisher};
