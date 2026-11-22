//! Keybindings preferences pane (`prefs.pane.keybindings`).
//!
//! Loads and saves through the host bridge (`KEYBINDINGS` /
//! `SAVE_KEYBINDING` / `KEYBINDING_RESULT`). It does not write
//! `preferences.json`.

mod accelerator;
mod model;
mod view;

pub use accelerator::{
    accelerator_label, accelerators_equal, is_valid_accelerator, record_keystroke,
};
pub use model::{
    CommandGroup, KeybindingCommand, KeybindingsModel, PendingKeybindingSave, PersistDecision,
    PersistGate, command_group, group_label, groups_in_order,
};
pub use view::{KeybindingHostRequest, KeybindingPublisher, KeybindingsPane};
