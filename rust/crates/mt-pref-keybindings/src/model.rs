use marktext_core::KeybindingEntry;

use crate::accelerator::accelerators_equal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandGroup {
    File,
    Edit,
    Format,
    Paragraph,
    View,
    Window,
}

pub fn groups_in_order() -> &'static [CommandGroup] {
    &[
        CommandGroup::File,
        CommandGroup::Edit,
        CommandGroup::Format,
        CommandGroup::Paragraph,
        CommandGroup::View,
        CommandGroup::Window,
    ]
}

pub fn group_label(group: CommandGroup) -> &'static str {
    match group {
        CommandGroup::File => "File",
        CommandGroup::Edit => "Edit",
        CommandGroup::Format => "Format",
        CommandGroup::Paragraph => "Paragraph",
        CommandGroup::View => "View",
        CommandGroup::Window => "Window and navigation",
    }
}

/// Same grouping as `prefComponents/keybindings/groups.ts`.
pub fn command_group(id: &str) -> CommandGroup {
    if matches!(id, "file.new-window" | "file.close-window" | "file.quit") {
        return CommandGroup::Window;
    }
    if matches!(id, "edit.create-paragraph" | "edit.delete-paragraph") {
        return CommandGroup::Paragraph;
    }
    let prefix = id.split('.').next().unwrap_or("");
    if prefix == "spellchecker" {
        return CommandGroup::Edit;
    }
    match prefix {
        "file" => CommandGroup::File,
        "edit" => CommandGroup::Edit,
        "format" => CommandGroup::Format,
        "paragraph" => CommandGroup::Paragraph,
        "view" => CommandGroup::View,
        _ => CommandGroup::Window,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeybindingCommand {
    pub id: String,
    pub description: String,
    pub accelerator: String,
    pub default_accelerator: String,
}

impl KeybindingCommand {
    pub fn from_entry(entry: &KeybindingEntry) -> Self {
        Self {
            id: entry.id.clone(),
            description: entry.description.clone(),
            accelerator: entry.accelerator.clone(),
            default_accelerator: entry.default_accelerator.clone(),
        }
    }

    pub fn is_bound(&self) -> bool {
        !self.accelerator.is_empty()
    }

    pub fn is_custom(&self) -> bool {
        !accelerators_equal(&self.accelerator, &self.default_accelerator)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeybindingsModel {
    pub commands: Vec<KeybindingCommand>,
    pub is_mac: bool,
    pub show_debug: bool,
    pub loading: bool,
    pub load_error: bool,
    pub selected_id: Option<String>,
    pub recorder_label: String,
    pub recorder_invalid: bool,
    pub conflict_message: Option<String>,
    pub save_error: bool,
}

impl KeybindingsModel {
    pub fn empty() -> Self {
        Self {
            commands: Vec::new(),
            is_mac: false,
            show_debug: false,
            loading: true,
            load_error: false,
            selected_id: None,
            recorder_label: String::new(),
            recorder_invalid: false,
            conflict_message: None,
            save_error: false,
        }
    }

    pub fn from_host(
        commands: &[KeybindingEntry],
        is_mac: bool,
        show_debug: bool,
        loading: bool,
        load_error: bool,
    ) -> Self {
        let mut model = Self::empty();
        model.replace_commands(commands);
        model.is_mac = is_mac;
        model.show_debug = show_debug;
        model.loading = loading;
        model.load_error = load_error;
        model
    }

    pub fn replace_commands(&mut self, commands: &[KeybindingEntry]) {
        self.commands = commands.iter().map(KeybindingCommand::from_entry).collect();
        self.commands
            .sort_by_key(|command| command.description.to_lowercase());
    }

    pub fn has_custom_bindings(&self) -> bool {
        self.commands.iter().any(KeybindingCommand::is_custom)
    }

    pub fn grouped(&self) -> Vec<(CommandGroup, Vec<&KeybindingCommand>)> {
        let mut out = Vec::new();
        for group in groups_in_order() {
            let commands: Vec<&KeybindingCommand> = self
                .commands
                .iter()
                .filter(|command| command_group(&command.id) == *group)
                .collect();
            if !commands.is_empty() {
                out.push((*group, commands));
            }
        }
        out
    }

    pub fn conflicts(&self, id: &str, accelerator: &str) -> Vec<&KeybindingCommand> {
        if accelerator.is_empty() {
            return Vec::new();
        }
        self.commands
            .iter()
            .filter(|command| {
                command.id != id && accelerators_equal(&command.accelerator, accelerator)
            })
            .collect()
    }

    /// Toggle on restores the default when one exists; otherwise recording starts.
    /// Toggle off unbinds.
    pub fn toggle_action(&self, id: &str, enabled: bool) -> ToggleAction {
        let Some(command) = self.commands.iter().find(|command| command.id == id) else {
            return ToggleAction::Ignore;
        };
        if enabled {
            if !command.default_accelerator.is_empty() {
                ToggleAction::Save {
                    command_id: id.to_string(),
                    accelerator: command.default_accelerator.clone(),
                }
            } else {
                ToggleAction::StartRecording {
                    command_id: id.to_string(),
                }
            }
        } else {
            ToggleAction::Save {
                command_id: id.to_string(),
                accelerator: String::new(),
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToggleAction {
    Ignore,
    StartRecording { command_id: String },
    Save { command_id: String, accelerator: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingKeybindingSave {
    pub id: u64,
    pub command_id: String,
    pub accelerator: String,
    pub kind: SaveKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveKind {
    Single,
    ResetAll,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PersistDecision {
    Ignored,
    Failed,
    Conflict {
        accelerator: String,
        command_ids: Vec<String>,
    },
    Applied {
        commands: Option<Vec<KeybindingEntry>>,
    },
}

/// At most one keybinding write is in flight.
#[derive(Debug)]
pub struct PersistGate {
    next_id: u64,
    pending: Option<PendingKeybindingSave>,
}

impl Default for PersistGate {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistGate {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            pending: None,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    pub fn request_save(
        &mut self,
        command_id: String,
        accelerator: String,
    ) -> Option<PendingKeybindingSave> {
        if self.pending.is_some() {
            return None;
        }
        self.next_id += 1;
        let pending = PendingKeybindingSave {
            id: self.next_id,
            command_id,
            accelerator,
            kind: SaveKind::Single,
        };
        self.pending = Some(pending.clone());
        Some(pending)
    }

    pub fn request_reset_all(&mut self) -> Option<PendingKeybindingSave> {
        if self.pending.is_some() {
            return None;
        }
        self.next_id += 1;
        let pending = PendingKeybindingSave {
            id: self.next_id,
            command_id: String::new(),
            accelerator: String::new(),
            kind: SaveKind::ResetAll,
        };
        self.pending = Some(pending.clone());
        Some(pending)
    }

    pub fn finish(
        &mut self,
        id: u64,
        status: &str,
        commands: Option<Vec<KeybindingEntry>>,
        conflict_accelerator: Option<String>,
        conflict_command_ids: Option<Vec<String>>,
    ) -> PersistDecision {
        let Some(pending) = self.pending.take() else {
            return PersistDecision::Ignored;
        };
        if pending.id != id {
            self.pending = Some(pending);
            return PersistDecision::Ignored;
        }
        match status {
            "persisted" => PersistDecision::Applied { commands },
            "conflict" => PersistDecision::Conflict {
                accelerator: conflict_accelerator.unwrap_or_default(),
                command_ids: conflict_command_ids.unwrap_or_default(),
            },
            _ => PersistDecision::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marktext_core::KeybindingEntry;

    fn entry(id: &str, accel: &str, default: &str) -> KeybindingEntry {
        KeybindingEntry {
            id: id.to_string(),
            description: id.to_string(),
            accelerator: accel.to_string(),
            default_accelerator: default.to_string(),
        }
    }

    #[test]
    fn groups_match_the_vue_helper() {
        assert_eq!(command_group("file.save"), CommandGroup::File);
        assert_eq!(command_group("file.quit"), CommandGroup::Window);
        assert_eq!(command_group("edit.create-paragraph"), CommandGroup::Paragraph);
        assert_eq!(command_group("spellchecker.switch-language"), CommandGroup::Edit);
        assert_eq!(command_group("tabs.cycleForward"), CommandGroup::Window);
        assert_eq!(group_label(CommandGroup::Window), "Window and navigation");
    }

    #[test]
    fn conflicts_and_toggle_follow_the_configurator() {
        let model = KeybindingsModel::from_host(
            &[
                entry("file.save", "Ctrl+S", "Ctrl+S"),
                entry("file.save-as", "Ctrl+Shift+S", "Ctrl+Shift+S"),
                entry("edit.find", "", ""),
            ],
            false,
            false,
            false,
            false,
        );
        assert!(!model.has_custom_bindings());
        assert_eq!(model.conflicts("file.save-as", "Ctrl+S").len(), 1);
        assert_eq!(
            model.toggle_action("file.save", false),
            ToggleAction::Save {
                command_id: "file.save".to_string(),
                accelerator: String::new(),
            }
        );
        assert_eq!(
            model.toggle_action("file.save", true),
            ToggleAction::Save {
                command_id: "file.save".to_string(),
                accelerator: "Ctrl+S".to_string(),
            }
        );
        assert_eq!(
            model.toggle_action("edit.find", true),
            ToggleAction::StartRecording {
                command_id: "edit.find".to_string(),
            }
        );
    }

    #[test]
    fn persist_gate_serializes_saves_and_surfaces_conflict() {
        let mut gate = PersistGate::new();
        let first = gate
            .request_save("file.save".to_string(), "Ctrl+Alt+S".to_string())
            .expect("accepted");
        assert!(
            gate.request_save("file.save-as".to_string(), "Ctrl+Q".to_string())
                .is_none()
        );
        match gate.finish(
            first.id,
            "conflict",
            None,
            Some("Ctrl+Alt+S".to_string()),
            Some(vec!["file.save-as".to_string()]),
        ) {
            PersistDecision::Conflict {
                accelerator,
                command_ids,
            } => {
                assert_eq!(accelerator, "Ctrl+Alt+S");
                assert_eq!(command_ids, vec!["file.save-as".to_string()]);
            }
            other => panic!("expected conflict, got {other:?}"),
        }
        let reset = gate.request_reset_all().expect("ready again");
        match gate.finish(reset.id, "persisted", Some(vec![entry("file.save", "Ctrl+S", "Ctrl+S")]), None, None)
        {
            PersistDecision::Applied { commands } => {
                assert_eq!(commands.unwrap().len(), 1);
            }
            other => panic!("expected applied, got {other:?}"),
        }
    }
}
