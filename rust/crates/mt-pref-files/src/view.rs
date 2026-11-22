use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px, rgb,
};
use marktext_core::{HostBridge, MemoryHostBridge};
use marktext_ui_kit::{UI_FONT, select_control, toggle};
use serde_json::Value;

use crate::model::{
    Choice, FilesModel, PersistDecision, PersistGate, folder_selection_changes, persist_wire,
    single_change,
};

/// Called for every fire-and-forget edit. The wasm host posts `SET_PREFERENCE`.
pub type PreferencePublisher = Rc<dyn Fn(String, Value)>;

/// Recovery edits. The host answers with `PREFERENCE_RESULT` before the snapshot moves.
pub type PersistPublisher = Rc<dyn Fn(u64, String, Value)>;

/// Opens the Electron directory dialog. The host answers with `DIRECTORY_CHOSEN`.
pub type ChooseDirectoryPublisher = Rc<dyn Fn(u64)>;

const PAGE: u32 = 0xFFFFFF;
const INK: u32 = 0x1C1C1F;
const MUTED: u32 = 0x5C5C66;
const CHIP: u32 = 0xF4F4F5;
const ACCENT: u32 = 0x4C9AFF;

pub struct FilesPane {
    bridge: Arc<MemoryHostBridge>,
    publish: PreferencePublisher,
    persist: PersistPublisher,
    choose_directory: ChooseDirectoryPublisher,
    gate: PersistGate,
    choose_id: u64,
    choosing: bool,
}

impl FilesPane {
    pub fn new(
        bridge: Arc<MemoryHostBridge>,
        publish: PreferencePublisher,
        persist: PersistPublisher,
        choose_directory: ChooseDirectoryPublisher,
    ) -> Self {
        Self {
            bridge,
            publish,
            persist,
            choose_directory,
            gate: PersistGate::new(),
            choose_id: 0,
            choosing: false,
        }
    }

    fn write(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.bridge.set_preference(key.to_string(), value.clone());
        (self.publish)(key.to_string(), value);
        cx.notify();
    }

    fn request_persist(&mut self, changes: BTreeMap<String, Value>, cx: &mut Context<Self>) {
        let id = self.gate.request(changes.clone());
        if let Some(id) = id {
            let (key, value) = persist_wire(&changes);
            (self.persist)(id, key, value);
            cx.notify();
        }
    }

    pub fn accept_persist_result(&mut self, id: u64, persisted: bool, cx: &mut Context<Self>) {
        let decision = self.gate.finish(id, persisted);
        if let PersistDecision::Applied(pending) = decision {
            for (key, value) in pending.changes {
                self.bridge.set_preference(key, value);
            }
        }
        cx.notify();
    }

    pub fn request_directory(&mut self, cx: &mut Context<Self>) {
        if self.gate.is_busy() || self.choosing {
            return;
        }
        self.choose_id += 1;
        self.choosing = true;
        (self.choose_directory)(self.choose_id);
        cx.notify();
    }

    pub fn accept_directory(&mut self, id: u64, path: Option<String>, cx: &mut Context<Self>) {
        if id != self.choose_id {
            return;
        }
        self.choosing = false;
        if let Some(directory) = path.filter(|value| !value.is_empty()) {
            self.request_persist(folder_selection_changes(&directory), cx);
        } else {
            cx.notify();
        }
    }
}

impl Render for FilesPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = FilesModel::from_snapshot(&self.bridge.get_preferences());
        let save_error = self.gate.save_error;
        let saving = self.gate.is_busy() || self.choosing;
        let view = cx.entity();

        div()
            .id("files-pane")
            .font_family(UI_FONT)
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_6()
            .p_2()
            .bg(rgb(PAGE))
            .text_color(rgb(INK))
            .child(div().text_xl().child("Files & saving"))
            .child(section(
                "Auto Save",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Auto save",
                        "",
                        bool_row(&view, "autoSave", "setting-autoSave", model.auto_save),
                    ))
                    .child(labeled(
                        "Auto save delay",
                        "",
                        delay_row(&view, model.auto_save_delay),
                    )),
            ))
            .child(section(
                "Startup and recovery",
                recovery_section(&view, &model, saving, save_error),
            ))
            .child(section(
                "File Representation",
                file_representation(&view, &model),
            ))
    }
}

fn recovery_section(
    view: &Entity<FilesPane>,
    model: &FilesModel,
    saving: bool,
    save_error: bool,
) -> impl IntoElement {
    let mut body = div()
        .flex()
        .flex_col()
        .gap_4()
        .child(labeled(
            "On startup",
            "",
            persist_choice_row(
                view,
                "startUpAction",
                "setting-startUpAction",
                FilesModel::startup_choices(),
                &model.start_up_action,
                saving,
            ),
        ));

    if model.shows_directory_picker() {
        body = body.child(directory_row(view, model, saving));
    }

    body = body
        .child(labeled(
            "Restore window layout",
            "Restore window size, position, and layouts such as the sidebar on restart.",
            persist_bool_row(
                view,
                "restoreLayoutState",
                "setting-restoreLayoutState",
                model.restore_layout_state,
                saving,
            ),
        ))
        .child(labeled(
            "Restore opened files",
            "Reopen the previous file tabs on restart.",
            persist_bool_row(
                view,
                "restoreEditors",
                "setting-restoreEditors",
                model.restore_editors,
                saving,
            ),
        ))
        .child(labeled(
            "Restore reading positions and selections",
            "Return to the previous reading position and selection when restoring a file.",
            persist_bool_row(
                view,
                "restoreViewState",
                "setting-restoreViewState",
                model.restore_view_state,
                saving,
            ),
        ))
        .child(if save_error {
            div()
                .text_sm()
                .text_color(rgb(0xB42318))
                .child(
                    "The settings could not be saved. Your previous settings are still in use.",
                )
        } else {
            div()
        });

    body
}

fn file_representation(view: &Entity<FilesPane>, model: &FilesModel) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(labeled(
            "Default file line ending",
            "Used for new files and files with mixed or undetected line endings. Existing LF or CRLF files keep their detected format.",
            choice_row(
                view,
                "endOfLine",
                "setting-endOfLine",
                FilesModel::end_of_line_choices(),
                &model.end_of_line,
            ),
        ))
        .child(labeled(
            "Default encoding",
            "",
            choice_row(
                view,
                "defaultEncoding",
                "setting-defaultEncoding",
                FilesModel::encoding_choices(),
                &model.default_encoding,
            ),
        ))
        .child(labeled(
            "Auto detect encoding",
            "",
            bool_row(
                view,
                "autoGuessEncoding",
                "setting-autoGuessEncoding",
                model.auto_guess_encoding,
            ),
        ))
        .child(labeled(
            "Auto-normalize line endings to LF on open",
            "",
            bool_row(
                view,
                "autoNormalizeLineEndings",
                "setting-autoNormalizeLineEndings",
                model.auto_normalize_line_endings,
            ),
        ))
        .child(labeled(
            "Trailing Newlines",
            "",
            number_choice_row(
                view,
                "trimTrailingNewline",
                "setting-trimTrailingNewline",
                FilesModel::trim_trailing_choices(),
                model.trim_trailing_newline,
            ),
        ))
}

fn directory_row(
    view: &Entity<FilesPane>,
    model: &FilesModel,
    saving: bool,
) -> impl IntoElement {
    let view = view.clone();
    let disabled = saving;
    div()
        .id("setting-defaultDirectoryToOpen")
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .id("setting-defaultDirectoryToOpen-select")
                .px_3()
                .py_1()
                .rounded_md()
                .bg(rgb(if disabled { CHIP } else { ACCENT }))
                .text_color(rgb(if disabled { MUTED } else { PAGE }))
                .cursor_pointer()
                .child(if disabled { "Saving…" } else { "Select Folder" })
                .on_click(move |_event, _window, app| {
                    if !disabled {
                        view.update(app, |pane, cx| pane.request_directory(cx));
                    }
                }),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child(if model.default_directory_to_open.is_empty() {
                    SharedString::from("No folder selected")
                } else {
                    SharedString::from(model.default_directory_to_open.clone())
                }),
        )
}

fn section(title: &'static str, body: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_color(rgb(MUTED)).child(title))
        .child(body)
}

fn labeled(label: &'static str, note: &'static str, control: impl IntoElement) -> impl IntoElement {
    let mut column = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(rgb(INK)).child(label))
        .child(control);
    if !note.is_empty() {
        column = column.child(div().text_sm().text_color(rgb(MUTED)).child(note));
    }
    column
}

fn choice_row(
    view: &Entity<FilesPane>,
    key: &'static str,
    id: &'static str,
    choices: &[Choice],
    selected: &str,
) -> impl IntoElement {
    let view = view.clone();
    let options = choices.iter().map(|choice| (choice.value, choice.label));
    select_control(id, options, selected, move |chosen, _window, app| {
        write_back(&view, key, Value::String(chosen.to_string()), app);
    })
}

fn number_choice_row(
    view: &Entity<FilesPane>,
    key: &'static str,
    id: &'static str,
    choices: &[Choice],
    selected: i64,
) -> impl IntoElement {
    let view = view.clone();
    let selected = selected.to_string();
    let options = choices.iter().map(|choice| (choice.value, choice.label));
    select_control(id, options, selected, move |chosen, _window, app| {
        if let Ok(number) = chosen.parse::<i64>() {
            write_back(&view, key, Value::Number(number.into()), app);
        }
    })
}

fn persist_choice_row(
    view: &Entity<FilesPane>,
    key: &'static str,
    id: &'static str,
    choices: &[Choice],
    selected: &str,
    saving: bool,
) -> impl IntoElement {
    let view = view.clone();
    let options = choices.iter().map(|choice| (choice.value, choice.label));
    select_control(id, options, selected, move |chosen, _window, app| {
        if saving {
            return;
        }
        view.update(app, |pane, cx| {
            pane.request_persist(single_change(key, Value::String(chosen.to_string())), cx);
        });
    })
}

fn bool_row(
    view: &Entity<FilesPane>,
    key: &'static str,
    id: &'static str,
    checked: bool,
) -> impl IntoElement {
    let view = view.clone();
    toggle(id, checked, move |next, _window, app| {
        write_back(&view, key, Value::Bool(next), app);
    })
}

fn persist_bool_row(
    view: &Entity<FilesPane>,
    key: &'static str,
    id: &'static str,
    checked: bool,
    saving: bool,
) -> impl IntoElement {
    let view = view.clone();
    toggle(id, checked, move |next, _window, app| {
        if saving {
            return;
        }
        view.update(app, |pane, cx| {
            pane.request_persist(single_change(key, Value::Bool(next)), cx);
        });
    })
}

fn delay_row(view: &Entity<FilesPane>, value: f64) -> impl IntoElement {
    let decrease = view.clone();
    let increase = view.clone();
    div()
        .id("setting-autoSaveDelay")
        .font_family(UI_FONT)
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .id("setting-autoSaveDelay-dec")
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(CHIP))
                .cursor_pointer()
                .child("−")
                .on_click(move |_event, _window, app| {
                    let next = FilesModel::from_snapshot(&decrease.read(app).bridge.get_preferences())
                        .delay_after_step(-1);
                    write_back(&decrease, "autoSaveDelay", json_number(next), app);
                }),
        )
        .child(
            div()
                .min_w(px(72.))
                .text_color(rgb(INK))
                .child(format!("{value:.0} ms")),
        )
        .child(
            div()
                .id("setting-autoSaveDelay-inc")
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(CHIP))
                .cursor_pointer()
                .child("+")
                .on_click(move |_event, _window, app| {
                    let next = FilesModel::from_snapshot(&increase.read(app).bridge.get_preferences())
                        .delay_after_step(1);
                    write_back(&increase, "autoSaveDelay", json_number(next), app);
                }),
        )
}

fn json_number(value: f64) -> Value {
    Value::Number(serde_json::Number::from_f64(value).unwrap_or_else(|| 5000.into()))
}

fn write_back(view: &Entity<FilesPane>, key: &'static str, value: Value, app: &mut App) {
    view.update(app, |pane, cx| pane.write(key, value, cx));
}
