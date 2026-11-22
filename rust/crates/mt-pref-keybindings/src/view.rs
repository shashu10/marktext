use std::rc::Rc;

use gpui::{
    Context, Entity, FocusHandle, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, prelude::*, px, rgb,
};
use marktext_core::KeybindingEntry;
use marktext_ui_kit::{UI_FONT, toggle};

use crate::accelerator::{accelerator_label, record_keystroke};
use crate::model::{
    PersistDecision, PersistGate, ToggleAction, group_label, KeybindingsModel,
};

/// Posts `SAVE_KEYBINDING` / `RESET_ALL_KEYBINDINGS` / reload / debug dump.
pub type KeybindingPublisher = Rc<dyn Fn(KeybindingHostRequest)>;

#[derive(Clone, Debug)]
pub enum KeybindingHostRequest {
    Reload,
    Save {
        id: u64,
        command_id: String,
        accelerator: String,
    },
    ResetAll {
        id: u64,
    },
    DebugDump,
}

const PAGE: u32 = 0xFFFFFF;
const INK: u32 = 0x1C1C1F;
const MUTED: u32 = 0x5C5C66;
const CHIP: u32 = 0xF4F4F5;
const ACCENT: u32 = 0x4C9AFF;
const DANGER: u32 = 0xB42318;
const ROW_LINE: u32 = 0xE8E8EA;

pub struct KeybindingsPane {
    publish: KeybindingPublisher,
    gate: PersistGate,
    model: KeybindingsModel,
    requested_once: bool,
    recorder_focus: FocusHandle,
}

impl KeybindingsPane {
    pub fn new(publish: KeybindingPublisher, cx: &mut Context<Self>) -> Self {
        Self {
            publish,
            gate: PersistGate::new(),
            model: KeybindingsModel::empty(),
            requested_once: false,
            recorder_focus: cx.focus_handle(),
        }
    }

    pub fn apply_snapshot(
        &mut self,
        commands: &[KeybindingEntry],
        is_mac: bool,
        show_debug: bool,
        loading: bool,
        load_error: bool,
        cx: &mut Context<Self>,
    ) {
        let selected = self.model.selected_id.clone();
        self.model =
            KeybindingsModel::from_host(commands, is_mac, show_debug, loading, load_error);
        self.model.selected_id = selected.filter(|id| {
            self.model.commands.iter().any(|command| &command.id == id)
        });
        self.model.save_error = false;
        cx.notify();
    }

    pub fn accept_result(
        &mut self,
        id: u64,
        status: &str,
        commands: Option<Vec<KeybindingEntry>>,
        conflict_accelerator: Option<String>,
        conflict_command_ids: Option<Vec<String>>,
        cx: &mut Context<Self>,
    ) {
        let decision = self.gate.finish(
            id,
            status,
            commands,
            conflict_accelerator,
            conflict_command_ids,
        );
        match decision {
            PersistDecision::Ignored => {}
            PersistDecision::Failed => {
                self.model.save_error = true;
            }
            PersistDecision::Conflict {
                accelerator,
                command_ids,
            } => {
                let names: Vec<String> = command_ids
                    .iter()
                    .map(|id| {
                        self.model
                            .commands
                            .iter()
                            .find(|command| &command.id == id)
                            .map(|command| command.description.clone())
                            .unwrap_or_else(|| id.clone())
                    })
                    .collect();
                let label = accelerator_label(&accelerator, self.model.is_mac);
                self.model.conflict_message = Some(format!(
                    "{label} is already assigned to {}.",
                    names.join(", ")
                ));
            }
            PersistDecision::Applied { commands } => {
                self.model.save_error = false;
                self.model.conflict_message = None;
                if let Some(commands) = commands {
                    self.model.replace_commands(&commands);
                }
            }
        }
        cx.notify();
    }

    fn ensure_requested(&mut self) {
        if self.requested_once {
            return;
        }
        self.requested_once = true;
        (self.publish)(KeybindingHostRequest::Reload);
    }

    fn begin_recording(
        &mut self,
        command_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model.selected_id = Some(command_id);
        self.model.recorder_label.clear();
        self.model.recorder_invalid = false;
        self.model.conflict_message = None;
        window.focus(&self.recorder_focus, cx);
        cx.notify();
    }

    fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        self.model.selected_id = None;
        self.model.recorder_label.clear();
        self.model.recorder_invalid = false;
        cx.notify();
    }

    fn request_save(&mut self, command_id: String, accelerator: String, cx: &mut Context<Self>) {
        if self.gate.is_busy() || self.model.loading {
            return;
        }
        let local = self.model.conflicts(&command_id, &accelerator);
        if !local.is_empty() {
            let names: Vec<_> = local.iter().map(|command| command.description.clone()).collect();
            let label = accelerator_label(&accelerator, self.model.is_mac);
            self.model.conflict_message = Some(format!(
                "{label} is already assigned to {}.",
                names.join(", ")
            ));
            cx.notify();
            return;
        }
        let Some(pending) = self.gate.request_save(command_id.clone(), accelerator.clone()) else {
            return;
        };
        self.model.conflict_message = None;
        self.model.save_error = false;
        self.model.selected_id = None;
        (self.publish)(KeybindingHostRequest::Save {
            id: pending.id,
            command_id,
            accelerator,
        });
        cx.notify();
    }

    fn request_reset_all(&mut self, cx: &mut Context<Self>) {
        if self.gate.is_busy() || self.model.loading || !self.model.has_custom_bindings() {
            return;
        }
        let Some(pending) = self.gate.request_reset_all() else {
            return;
        };
        self.model.selected_id = None;
        self.model.conflict_message = None;
        self.model.save_error = false;
        (self.publish)(KeybindingHostRequest::ResetAll { id: pending.id });
        cx.notify();
    }
}

impl Render for KeybindingsPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_requested();
        let view = cx.entity();
        let model = self.model.clone();
        let saving = self.gate.is_busy();
        let mut root = div()
            .id("keybindings-pane")
            .font_family(UI_FONT)
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .p_2()
            .bg(rgb(PAGE))
            .text_color(rgb(INK))
            .child(div().text_xl().child("Keybindings"))
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("To change a shortcut, select its key combination, then press the new key combination. Documentation: https://marktext.me/docs/key-bindings"),
            );

        if model.load_error {
            let view = view.clone();
            root = root.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_color(rgb(DANGER)).child("Could not load keyboard shortcuts."))
                    .child(
                        div()
                            .id("keybindings-reload")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(ACCENT))
                            .text_color(rgb(PAGE))
                            .cursor_pointer()
                            .child("Reload")
                            .on_click(move |_event, _window, app| {
                                view.update(app, |pane, cx| {
                                    pane.requested_once = false;
                                    pane.ensure_requested();
                                    cx.notify();
                                });
                            }),
                    ),
            );
        }

        if model.loading && model.commands.is_empty() {
            root = root.child(div().text_color(rgb(MUTED)).child("Loading shortcuts…"));
        }

        if let Some(message) = &model.conflict_message {
            root = root.child(
                div()
                    .text_sm()
                    .text_color(rgb(DANGER))
                    .child(format!("Shortcut in use: {message}")),
            );
        }
        if model.save_error {
            root = root.child(
                div()
                    .text_sm()
                    .text_color(rgb(DANGER))
                    .child("Error saving keybindings. The previous shortcuts have been kept."),
            );
        }

        let recorder_focus = self.recorder_focus.clone();
        for (group, commands) in model.grouped() {
            let mut section = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_color(rgb(MUTED)).child(group_label(group)));
            for command in commands {
                section = section.child(command_row(
                    &view,
                    command,
                    &model,
                    saving,
                    recorder_focus.clone(),
                ));
            }
            root = root.child(section);
        }

        if model.show_debug {
            let view = view.clone();
            root = root.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_color(rgb(MUTED)).child("Debug Options"))
                    .child(
                        div()
                            .id("keybindings-debug-dump")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(CHIP))
                            .cursor_pointer()
                            .child("Dump Keyboard Info")
                            .on_click(move |_event, _window, app| {
                                view.update(app, |pane, _cx| {
                                    (pane.publish)(KeybindingHostRequest::DebugDump);
                                });
                            }),
                    ),
            );
        }

        let can_restore = !model.loading && !saving && model.has_custom_bindings();
        let view_restore = view.clone();
        root.child(
            div()
                .mt_4()
                .pt_3()
                .border_t_1()
                .border_color(rgb(ROW_LINE))
                .flex()
                .justify_end()
                .child(
                    div()
                        .id("keybindings-restore-defaults")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(if can_restore { ACCENT } else { CHIP }))
                        .text_color(rgb(if can_restore { PAGE } else { MUTED }))
                        .when(can_restore, |el| el.cursor_pointer())
                        .child(if saving { "Saving…" } else { "Restore defaults" })
                        .on_click(move |_event, _window, app| {
                            view_restore.update(app, |pane, cx| pane.request_reset_all(cx));
                        }),
                ),
        )
    }
}

fn command_row(
    view: &Entity<KeybindingsPane>,
    command: &crate::model::KeybindingCommand,
    model: &KeybindingsModel,
    saving: bool,
    recorder_focus: FocusHandle,
) -> impl IntoElement {
    let bound = command.is_bound();
    let recording = model.selected_id.as_deref() == Some(command.id.as_str());
    let disabled = saving || model.loading;
    let view_toggle = view.clone();
    let command_id = command.id.clone();
    let is_mac = model.is_mac;
    let label = if command.accelerator.is_empty() {
        if command.default_accelerator.is_empty() {
            "Not assigned".to_string()
        } else {
            accelerator_label(&command.default_accelerator, is_mac)
        }
    } else {
        accelerator_label(&command.accelerator, is_mac)
    };

    let mut right = div().flex().items_center().gap_2();
    if recording {
        right = right.child(recorder(view, model, recorder_focus));
    } else {
        let view_edit = view.clone();
        let edit_id = command.id.clone();
        let can_edit = !disabled && bound;
        right = right.child(
            div()
                .id(SharedString::from(format!("keybinding-edit-{}", command.id)))
                .px_3()
                .py_1()
                .rounded_full()
                .bg(rgb(CHIP))
                .text_color(rgb(if bound { INK } else { MUTED }))
                .when(can_edit, |el| el.cursor_pointer())
                .child(label)
                .on_click(move |_event, window, app| {
                    if !can_edit {
                        return;
                    }
                    view_edit.update(app, |pane, cx| {
                        pane.begin_recording(edit_id.clone(), window, cx);
                    });
                }),
        );
    }

    div()
        .id(SharedString::from(format!("keybinding-row-{}", command.id)))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .py_2()
        .border_b_1()
        .border_color(rgb(ROW_LINE))
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .min_w(px(0.))
                .child(toggle(
                    SharedString::from(format!("keybinding-toggle-{}", command.id)),
                    bound,
                    move |enabled, window, app| {
                        if disabled {
                            return;
                        }
                        view_toggle.update(app, |pane, cx| {
                            pane.cancel_recording(cx);
                            match pane.model.toggle_action(&command_id, enabled) {
                                ToggleAction::Ignore => {}
                                ToggleAction::StartRecording { command_id } => {
                                    pane.begin_recording(command_id, window, cx);
                                }
                                ToggleAction::Save {
                                    command_id,
                                    accelerator,
                                } => pane.request_save(command_id, accelerator, cx),
                            }
                        });
                    },
                ))
                .child(
                    div()
                        .id(SharedString::from(format!("setting-keybinding-{}", command.id)))
                        .text_color(rgb(INK))
                        .child(command.description.clone()),
                ),
        )
        .child(right)
}

fn recorder(
    view: &Entity<KeybindingsPane>,
    model: &KeybindingsModel,
    focus: FocusHandle,
) -> impl IntoElement {
    let view = view.clone();
    let is_mac = model.is_mac;
    let label = if model.recorder_label.is_empty() {
        "Press key combination".to_string()
    } else {
        model.recorder_label.clone()
    };
    let invalid = model.recorder_invalid;
    div()
        .flex()
        .flex_col()
        .gap_1()
        .track_focus(&focus)
        .on_key_down(move |event, _window, app| {
            let keystroke = &event.keystroke;
            let unmodified = !keystroke.modifiers.modified();
            if unmodified && keystroke.key == "escape" {
                view.update(app, |pane, cx| pane.cancel_recording(cx));
                return;
            }
            let Some((accelerator, valid)) = record_keystroke(keystroke, is_mac) else {
                return;
            };
            view.update(app, |pane, cx| {
                pane.model.recorder_label = accelerator_label(&accelerator, pane.model.is_mac);
                pane.model.recorder_invalid = !valid;
                if !valid {
                    cx.notify();
                    return;
                }
                let Some(command_id) = pane.model.selected_id.clone() else {
                    return;
                };
                pane.request_save(command_id, accelerator, cx);
            });
        })
        .child(
            div()
                .id("keybinding-recorder")
                .w(px(180.))
                .min_h(px(36.))
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(rgb(ACCENT))
                .bg(rgb(PAGE))
                .text_color(rgb(if model.recorder_label.is_empty() {
                    MUTED
                } else {
                    INK
                }))
                .child(label),
        )
        .child(if invalid {
            div()
                .text_sm()
                .text_color(rgb(DANGER))
                .child("Invalid key combination")
        } else {
            div()
        })
}
