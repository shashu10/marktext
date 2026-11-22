use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
    rgb,
};
use gpui_elements::editable_text::EditableTextState;
use marktext_core::{HostBridge, MemoryHostBridge};
use marktext_ui_kit::{
    action_button, button_disabled, choice_button, pref_field, pref_page, pref_section,
    pref_text_input, select_control, toggle,
};
use serde_json::{Value, json};

use crate::model::{
    Choice, DetectionState, ImagesModel, PersistDecision, PersistGate, UserDataKeys,
    extract_user_data, folder_path_valid, pandoc_switch_state, relative_directory_name_valid,
};

pub type PreferencePublisher = Rc<dyn Fn(String, Value)>;
pub type PersistPublisher = Rc<dyn Fn(u64, String, Value)>;
/// Host-only actions: user-data writes, folder picker, open path/url, probes.
pub type HostActionPublisher = Rc<dyn Fn(u64, String, Value)>;

const INK: u32 = 0x1C1C1F;
const MUTED: u32 = 0x5C5C66;
const ACCENT: u32 = 0x4C9AFF;
const DANGER: u32 = 0xB42318;

#[derive(Clone, Debug)]
struct ProbeState {
    status: DetectionState,
    message: String,
    path: String,
    debug: String,
    last_detection: String,
    last_success: String,
    started: bool,
}

impl Default for ProbeState {
    fn default() -> Self {
        Self {
            status: DetectionState::Checking,
            message: String::new(),
            path: String::new(),
            debug: String::new(),
            last_detection: String::new(),
            last_success: String::new(),
            started: false,
        }
    }
}

pub struct ImagesPane {
    bridge: Arc<MemoryHostBridge>,
    publish: PreferencePublisher,
    persist: PersistPublisher,
    host_action: HostActionPublisher,
    gate: PersistGate,
    user_data: BTreeMap<String, Value>,
    next_action_id: u64,
    pending_action: Option<(u64, String)>,
    pandoc: ProbeState,
    picgo: ProbeState,
    cli_script_error: bool,
    folder_path_error: bool,
    relative_name_error: bool,
    guide_open: bool,
    debug_open: bool,
    folder_draft: Option<Entity<EditableTextState>>,
    relative_draft: Option<Entity<EditableTextState>>,
    cli_draft: Option<Entity<EditableTextState>>,
}

impl ImagesPane {
    pub fn new(
        bridge: Arc<MemoryHostBridge>,
        publish: PreferencePublisher,
        persist: PersistPublisher,
        host_action: HostActionPublisher,
    ) -> Self {
        Self {
            bridge,
            publish,
            persist,
            host_action,
            gate: PersistGate::new(),
            user_data: BTreeMap::new(),
            next_action_id: 0,
            pending_action: None,
            pandoc: ProbeState::default(),
            picgo: ProbeState::default(),
            cli_script_error: false,
            folder_path_error: false,
            relative_name_error: false,
            guide_open: false,
            debug_open: false,
            folder_draft: None,
            relative_draft: None,
            cli_draft: None,
        }
    }

    pub fn sync_user_data(&mut self, snapshot: &BTreeMap<String, Value>, cx: &mut Context<Self>) {
        let next = extract_user_data(snapshot);
        if next != self.user_data {
            self.user_data = next;
            self.sync_draft_fields(cx);
            cx.notify();
        }
    }

    fn sync_draft_fields(&mut self, cx: &mut Context<Self>) {
        let model = self.model();
        if let Some(draft) = self.folder_draft.clone() {
            let current = draft.read(cx).as_str().to_string();
            if current != model.image_folder_path {
                draft.update(cx, |state, cx| state.emplace(&model.image_folder_path, cx));
            }
        }
        if let Some(draft) = self.relative_draft.clone() {
            let current = draft.read(cx).as_str().to_string();
            if current != model.image_relative_directory_name {
                draft.update(cx, |state, cx| {
                    state.emplace(&model.image_relative_directory_name, cx)
                });
            }
        }
        if let Some(draft) = self.cli_draft.clone() {
            let current = draft.read(cx).as_str().to_string();
            if current != model.cli_script {
                draft.update(cx, |state, cx| state.emplace(&model.cli_script, cx));
            }
        }
    }

    fn model(&self) -> ImagesModel {
        ImagesModel::from_snapshot(&self.bridge.get_preferences(), &self.user_data)
    }

    fn write_pref(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.bridge.set_preference(key.to_string(), value.clone());
        (self.publish)(key.to_string(), value);
        cx.notify();
    }

    fn write_user_data(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.user_data.insert(key.to_string(), value.clone());
        self.request_action("set-user-data", json!({ "key": key, "value": value }));
        cx.notify();
    }

    fn request_persist(
        &mut self,
        key: &str,
        value: Value,
        draft_text: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let id = self
            .gate
            .request(key.to_string(), value.clone(), draft_text);
        if let Some(id) = id {
            (self.persist)(id, key.to_string(), value);
            cx.notify();
        }
    }

    fn request_action(&mut self, kind: &str, payload: Value) -> u64 {
        self.next_action_id += 1;
        let id = self.next_action_id;
        self.pending_action = Some((id, kind.to_string()));
        (self.host_action)(id, kind.to_string(), payload);
        id
    }

    pub fn accept_persist_result(&mut self, id: u64, persisted: bool, cx: &mut Context<Self>) {
        let decision = self.gate.finish(id, persisted);
        if let PersistDecision::Applied(pending) = decision {
            if UserDataKeys::is_user_data(&pending.key) {
                self.user_data
                    .insert(pending.key.clone(), pending.value.clone());
            } else {
                self.bridge
                    .set_preference(pending.key.clone(), pending.value.clone());
            }
            let _ = pending;
        }
        cx.notify();
    }

    pub fn accept_host_result(
        &mut self,
        id: u64,
        status: &str,
        payload: &Value,
        cx: &mut Context<Self>,
    ) {
        let Some((pending_id, kind)) = self.pending_action.clone() else {
            return;
        };
        if pending_id != id {
            return;
        }
        self.pending_action = None;

        match kind.as_str() {
            "probe-pandoc" => self.apply_pandoc_result(status, payload),
            "probe-picgo" => self.apply_picgo_result(status, payload),
            "is-executable" => {
                let ok = status == "ok"
                    && payload.get("executable").and_then(Value::as_bool) == Some(true);
                self.cli_script_error = !ok;
                if ok && let Some(path) = payload.get("path").and_then(Value::as_str) {
                    self.user_data.insert(
                        UserDataKeys::CLI_SCRIPT.to_string(),
                        Value::String(path.to_string()),
                    );
                    self.request_action(
                        "set-user-data",
                        json!({ "key": UserDataKeys::CLI_SCRIPT, "value": path }),
                    );
                }
            }
            "set-user-data" | "ask-image-folder" => {
                if status == "ok"
                    && let Some(map) = payload.as_object()
                {
                    for (key, value) in map {
                        if UserDataKeys::is_user_data(key) {
                            self.user_data.insert(key.clone(), value.clone());
                        }
                    }
                    self.sync_draft_fields(cx);
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn apply_pandoc_result(&mut self, status: &str, payload: &Value) {
        let path = payload
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match status {
            "ok" if !path.is_empty() => {
                self.pandoc.status = DetectionState::Found;
                self.pandoc.path = path.clone();
                self.pandoc.message = format!("Found: {path}");
            }
            "ok" => {
                self.pandoc.status = DetectionState::NotFound;
                self.pandoc.path.clear();
                self.pandoc.message = "Not found".to_string();
            }
            _ => {
                self.pandoc.status = DetectionState::Failed;
                self.pandoc.path.clear();
                self.pandoc.message = "Detection failed. Please check again.".to_string();
            }
        }
    }

    fn apply_picgo_result(&mut self, status: &str, payload: &Value) {
        let debug = payload
            .get("debug")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let detected_at = payload
            .get("detectedAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !detected_at.is_empty() {
            self.picgo.last_detection = detected_at;
        }
        self.picgo.debug = debug;
        match status {
            "ok" if payload.get("exists").and_then(Value::as_bool) == Some(true) => {
                self.picgo.status = DetectionState::Found;
                self.picgo.message = "PicGo-Core is installed".to_string();
                if let Some(at) = payload.get("successAt").and_then(Value::as_str) {
                    self.picgo.last_success = at.to_string();
                }
            }
            "ok" => {
                self.picgo.status = DetectionState::NotFound;
                self.picgo.message = "PicGo-Core is not installed".to_string();
            }
            _ => {
                self.picgo.status = DetectionState::Failed;
                self.picgo.message = "PicGo-Core detection failed".to_string();
            }
        }
    }

    fn ensure_probes_started(&mut self, cx: &mut Context<Self>) {
        if !self.pandoc.started {
            self.pandoc.started = true;
            self.pandoc.status = DetectionState::Checking;
            self.pandoc.message = "Checking…".to_string();
            self.request_action("probe-pandoc", json!({}));
            cx.notify();
        }
        let show_picgo = self.model().show_picgo();
        if show_picgo && !self.picgo.started {
            self.picgo.started = true;
            self.picgo.status = DetectionState::Checking;
            self.picgo.message = "Checking…".to_string();
            self.request_action("probe-picgo", json!({}));
            cx.notify();
        }
    }
}

impl Render for ImagesPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_probes_started(cx);
        let model = self.model();
        let folder_draft =
            EditableTextState::use_keyed("setting-imageFolderPath-draft", window, cx);
        let relative_draft =
            EditableTextState::use_keyed("setting-imageRelativeDirectoryName-draft", window, cx);
        let cli_draft = EditableTextState::use_keyed("setting-cliScript-draft", window, cx);
        // First paint starts empty; fill from the host snapshot so the field
        // matches the legacy Vue input that shows the saved path.
        if folder_draft.read(cx).as_str().is_empty() && !model.image_folder_path.is_empty() {
            let path = model.image_folder_path.clone();
            folder_draft.update(cx, |state, cx| state.emplace(&path, cx));
        }
        if relative_draft.read(cx).as_str().is_empty()
            && !model.image_relative_directory_name.is_empty()
        {
            let name = model.image_relative_directory_name.clone();
            relative_draft.update(cx, |state, cx| state.emplace(&name, cx));
        }
        if cli_draft.read(cx).as_str().is_empty() && !model.cli_script.is_empty() {
            let script = model.cli_script.clone();
            cli_draft.update(cx, |state, cx| state.emplace(&script, cx));
        }
        self.folder_draft = Some(folder_draft.clone());
        self.relative_draft = Some(relative_draft.clone());
        self.cli_draft = Some(cli_draft.clone());
        let view = cx.entity();

        let unanswered =
            self.pandoc.status == DetectionState::Checking && self.pandoc.path.is_empty();
        let probe_path = if unanswered {
            None
        } else if self.pandoc.status == DetectionState::Found {
            Some(self.pandoc.path.as_str())
        } else {
            Some("")
        };
        let pandoc = pandoc_switch_state(probe_path, model.show_pandoc_convert);
        let pandoc_disabled = if unanswered { false } else { pandoc.disabled };

        let mut root = pref_page("images-pane", "Images & export", cx)
            .child(pandoc_section(
                &view,
                model.show_pandoc_convert,
                pandoc_disabled,
                &self.pandoc,
            ))
            .child(section(
                "Image",
                labeled(
                    "The default behavior after pasting or dragging the image to MarkText",
                    "",
                    choice_row(
                        &view,
                        "imageInsertAction",
                        "setting-imageInsertAction",
                        ImagesModel::insert_action_choices(),
                        &model.image_insert_action,
                        false,
                    ),
                ),
            ));

        if model.show_folder_settings() {
            root = root.child(folder_section(
                &view,
                &model,
                folder_draft,
                relative_draft,
                self.folder_path_error,
                self.relative_name_error,
                self.gate.save_error,
            ));
        }

        if model.show_uploader() {
            root = root.child(uploader_section(
                &view,
                &model,
                cli_draft,
                &self.picgo,
                self.cli_script_error,
                self.guide_open,
                self.debug_open,
            ));
        }

        root
    }
}

fn pandoc_section(
    view: &Entity<ImagesPane>,
    switch_on: bool,
    disabled: bool,
    probe: &ProbeState,
) -> impl IntoElement {
    let view_toggle = view.clone();
    let view_check = view.clone();
    let status_text = if probe.status == DetectionState::Checking {
        "Checking…".to_string()
    } else {
        probe.message.clone()
    };
    let toggle_control = if disabled {
        div()
            .opacity(0.5)
            .child(toggle("setting-showPandocConvert", switch_on, |_, _, _| {}))
            .into_any_element()
    } else {
        toggle(
            "setting-showPandocConvert",
            switch_on,
            move |next, _window, app| {
                write_pref_back(&view_toggle, "showPandocConvert", Value::Bool(next), app);
            },
        )
        .into_any_element()
    };

    section(
        "Pandoc Conversion",
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(labeled(
                "Enable Pandoc conversion",
                "Show format conversion commands in the menu. Requires Pandoc to be installed.",
                toggle_control,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(if probe.status == DetectionState::Failed {
                                DANGER
                            } else {
                                MUTED
                            }))
                            .child(status_text),
                    )
                    .child(chip_button(
                        "setting-pandoc-recheck",
                        "Check again",
                        move |app| {
                            view_check.update(app, |pane, cx| {
                                pane.pandoc.status = DetectionState::Checking;
                                pane.pandoc.message = "Checking…".to_string();
                                pane.request_action("probe-pandoc", json!({}));
                                cx.notify();
                            });
                        },
                    )),
            ),
    )
}

fn folder_section(
    view: &Entity<ImagesPane>,
    model: &ImagesModel,
    folder_draft: Entity<EditableTextState>,
    relative_draft: Entity<EditableTextState>,
    folder_error: bool,
    relative_error: bool,
    save_error: bool,
) -> impl IntoElement {
    let view_choose = view.clone();
    let view_show = view.clone();
    let view_enter = view.clone();
    let view_save_relative = view.clone();
    let draft_for_enter = folder_draft.clone();
    let draft_for_choose = folder_draft.clone();
    let relative_for_save = relative_draft.clone();
    let relative_disabled = !model.image_prefer_relative_directory;

    section(
        "Folder Setting",
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_color(rgb(INK)).child("Global image folder"))
                    .child(
                        div()
                            .on_key_down(move |event, _window, app| {
                                let plain_enter = event.keystroke.key == "enter"
                                    && !event.keystroke.modifiers.modified();
                                if !plain_enter {
                                    return;
                                }
                                let typed = draft_for_enter.read(app).as_str().to_string();
                                view_enter.update(app, |pane, cx| {
                                    commit_folder_path(pane, typed, cx);
                                });
                            })
                            .child(
                                pref_text_input("setting-imageFolderPath")
                                    .state(folder_draft.downgrade())
                                    .placeholder("/path/to/images")
                                    .w(px(420.)),
                            ),
                    )
                    .child(error_line(folder_error, "Enter an absolute folder path."))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(chip_button(
                                "setting-imageFolderPath-choose",
                                "Choose folder…",
                                move |app| {
                                    let typed = draft_for_choose.read(app).as_str().to_string();
                                    view_choose.update(app, |pane, cx| {
                                        if typed.is_empty() {
                                            pane.folder_path_error = false;
                                            pane.request_action(
                                                "ask-image-folder",
                                                json!({ "path": Value::Null }),
                                            );
                                        } else {
                                            commit_folder_path(pane, typed, cx);
                                        }
                                        cx.notify();
                                    });
                                },
                            ))
                            .child(chip_button(
                                "setting-imageFolderPath-show",
                                "Show in Folder",
                                move |app| {
                                    let path = view_show.read(app).model().image_folder_path;
                                    if path.is_empty() {
                                        return;
                                    }
                                    view_show.update(app, |pane, cx| {
                                        pane.request_action("open-path", json!({ "path": path }));
                                        cx.notify();
                                    });
                                },
                            )),
                    ),
            )
            .child(labeled(
                "Prefer relative directory",
                "",
                bool_row(
                    view,
                    "imagePreferRelativeDirectory",
                    "setting-imagePreferRelativeDirectory",
                    model.image_prefer_relative_directory,
                ),
            ))
            .child({
                let body = div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(labeled(
                        "Copy image relative to",
                        "",
                        choice_row(
                            view,
                            "imageRelativeDirectoryBase",
                            "setting-imageRelativeDirectoryBase",
                            ImagesModel::relative_base_choices(),
                            &model.image_relative_directory_base,
                            relative_disabled,
                        ),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_color(rgb(INK)).child("Relative folder name"))
                            .child(
                                pref_text_input("setting-imageRelativeDirectoryName")
                                    .state(relative_draft.downgrade())
                                    .placeholder("assets")
                                    .w(px(280.)),
                            )
                            .child(error_line(relative_error, "Enter a relative folder name."))
                            .child(accent_button(
                                "setting-imageRelativeDirectoryName-save",
                                "Save",
                                {
                                    let draft = relative_for_save;
                                    let view = view_save_relative;
                                    let disabled = relative_disabled;
                                    move |app| {
                                        if disabled {
                                            return;
                                        }
                                        let typed = draft.read(app).as_str().to_string();
                                        view.update(app, |pane, cx| {
                                            if relative_directory_name_valid(&typed) {
                                                pane.relative_name_error = false;
                                                pane.request_persist(
                                                    "imageRelativeDirectoryName",
                                                    Value::String(typed.clone()),
                                                    Some(typed),
                                                    cx,
                                                );
                                            } else {
                                                pane.relative_name_error = true;
                                                cx.notify();
                                            }
                                        });
                                    }
                                },
                            )),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child("The filename will be automatically generated."),
                    )
                    .child(error_line(save_error, "Could not save. Please try again."));
                if relative_disabled {
                    body.opacity(0.5)
                } else {
                    body
                }
            }),
    )
}

fn uploader_section(
    view: &Entity<ImagesPane>,
    model: &ImagesModel,
    cli_draft: Entity<EditableTextState>,
    picgo: &ProbeState,
    cli_error: bool,
    guide_open: bool,
    debug_open: bool,
) -> impl IntoElement {
    let view_uploader = view.clone();
    let mut body = div()
        .id("setting-imageUploader")
        .flex()
        .flex_col()
        .gap_4()
        .child(labeled("Image Uploader", "", {
            let options = ImagesModel::uploader_choices();
            let selected = model.current_uploader.clone();
            select_control(
                "setting-currentUploader",
                options.iter().map(|c| (c.value, c.label)),
                selected,
                move |chosen, _window, app| {
                    view_uploader.update(app, |pane, cx| {
                        pane.write_user_data(
                            UserDataKeys::CURRENT_UPLOADER,
                            Value::String(chosen.to_string()),
                            cx,
                        );
                        if chosen.as_ref() == "picgo" {
                            pane.picgo.started = false;
                        }
                    });
                },
            )
        }));

    if model.show_picgo() {
        body = body.child(picgo_block(view, picgo, guide_open, debug_open));
    }
    if model.show_cli_script() {
        body = body.child(cli_script_block(view, cli_draft, cli_error));
    }

    section("Image Uploader", body)
}

fn picgo_block(
    view: &Entity<ImagesPane>,
    picgo: &ProbeState,
    guide_open: bool,
    debug_open: bool,
) -> impl IntoElement {
    let view_check = view.clone();
    let view_guide = view.clone();
    let view_debug = view.clone();
    let view_docs = view.clone();
    let view_core = view.clone();
    let status = if picgo.status == DetectionState::Checking {
        "Checking…".to_string()
    } else {
        picgo.message.clone()
    };
    let show_install =
        picgo.status == DetectionState::NotFound || picgo.status == DetectionState::Failed;

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(if picgo.status == DetectionState::Failed {
                            DANGER
                        } else {
                            MUTED
                        }))
                        .child(status),
                )
                .child(chip_button(
                    "setting-picgo-recheck",
                    "Check again",
                    move |app| {
                        view_check.update(app, |pane, cx| {
                            pane.picgo.status = DetectionState::Checking;
                            pane.picgo.message = "Checking…".to_string();
                            pane.request_action("probe-picgo", json!({}));
                            cx.notify();
                        });
                    },
                )),
        )
        .child(if picgo.last_detection.is_empty() {
            div()
        } else {
            div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child(format!("Last Detection Time: {}", picgo.last_detection))
        })
        .child(if picgo.last_success.is_empty() {
            div()
        } else {
            div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child(format!("Last success time: {}", picgo.last_success))
        })
        .child(
            div()
                .id("setting-picgo-guide-toggle")
                .cursor_pointer()
                .text_sm()
                .text_color(rgb(INK))
                .child(if guide_open {
                    "▾ PicGo-Core Basic Usage Guide"
                } else {
                    "▸ PicGo-Core Basic Usage Guide"
                })
                .on_click(move |_event, _window, app| {
                    view_guide.update(app, |pane, cx| {
                        pane.guide_open = !pane.guide_open;
                        cx.notify();
                    });
                }),
        )
        .child(if guide_open {
            guide_content(show_install, view_docs, view_core).into_any_element()
        } else {
            div().into_any_element()
        })
        .child(
            if picgo.status == DetectionState::Failed && !picgo.debug.is_empty() {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .id("setting-picgo-debug-toggle")
                            .cursor_pointer()
                            .text_sm()
                            .child(if debug_open {
                                "▾ Debug Info"
                            } else {
                                "▸ Debug Info"
                            })
                            .on_click(move |_event, _window, app| {
                                view_debug.update(app, |pane, cx| {
                                    pane.debug_open = !pane.debug_open;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(if debug_open {
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(picgo.debug.clone())
                    } else {
                        div()
                    })
                    .into_any_element()
            } else {
                div().into_any_element()
            },
        )
}

fn guide_content(
    show_install: bool,
    view_docs: Entity<ImagesPane>,
    view_core: Entity<ImagesPane>,
) -> impl IntoElement {
    let mut column = div()
        .flex()
        .flex_col()
        .gap_2()
        .text_sm()
        .text_color(rgb(MUTED));
    if show_install {
        column = column
            .child(div().child("Choose installation method:"))
            .child(div().child("npm: npm install picgo -g"))
            .child(div().child("yarn: yarn global add picgo"))
            .child(div().child("pnpm: pnpm add -g picgo"))
            .child(
                div()
                    .id("setting-picgo-core-link")
                    .cursor_pointer()
                    .text_color(rgb(ACCENT))
                    .child("PicGo-Core — Please install")
                    .on_click(move |_event, _window, app| {
                        view_core.update(app, |pane, cx| {
                            pane.request_action(
                                "open-external",
                                json!({ "url": "https://github.com/PicGo/PicGo-Core" }),
                            );
                            cx.notify();
                        });
                    }),
            );
    }
    column
        .child(div().child("1. Configure Uploader"))
        .child(div().child("Use the following command to configure PicGo-Core uploader:"))
        .child(code_chip("picgo set uploader"))
        .child(div().child("2. Upload Images"))
        .child(div().child("Use the following command to upload images:"))
        .child(code_chip("picgo upload /path/to/image.png"))
        .child(div().child("3. View Configuration"))
        .child(div().child("Use the following command to view current configuration:"))
        .child(code_chip("picgo config"))
        .child(
            div()
                .id("setting-picgo-docs-link")
                .cursor_pointer()
                .text_color(rgb(ACCENT))
                .child("View full documentation")
                .on_click(move |_event, _window, app| {
                    view_docs.update(app, |pane, cx| {
                        pane.request_action(
                            "open-external",
                            json!({ "url": "https://picgo.github.io/PicGo-Core-Doc/" }),
                        );
                        cx.notify();
                    });
                }),
        )
}

fn cli_script_block(
    view: &Entity<ImagesPane>,
    cli_draft: Entity<EditableTextState>,
    cli_error: bool,
) -> impl IntoElement {
    let view_save = view.clone();
    let draft_for_save = cli_draft.clone();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child("Script Description"),
        )
        .child(div().text_color(rgb(INK)).child("Script Location"))
        .child(
            pref_text_input("setting-cliScript")
                .state(cli_draft.downgrade())
                .placeholder("Script path")
                .w(px(420.)),
        )
        .child(error_line(
            cli_error,
            "Enter the path to an existing executable script.",
        ))
        .child(accent_button(
            "setting-cliScript-save",
            "Save",
            move |app| {
                let typed = draft_for_save.read(app).as_str().to_string();
                view_save.update(app, |pane, cx| {
                    if typed.is_empty() {
                        pane.cli_script_error = true;
                        cx.notify();
                        return;
                    }
                    pane.cli_script_error = false;
                    pane.request_action("is-executable", json!({ "path": typed }));
                    cx.notify();
                });
            },
        ))
}

fn commit_folder_path(pane: &mut ImagesPane, typed: String, cx: &mut Context<ImagesPane>) {
    if folder_path_valid(&typed) {
        pane.folder_path_error = false;
        pane.request_action("ask-image-folder", json!({ "path": typed }));
    } else {
        pane.folder_path_error = true;
    }
    cx.notify();
}

fn section(title: &'static str, body: impl IntoElement) -> impl IntoElement {
    pref_section(title, body)
}

fn labeled(label: &'static str, note: &'static str, control: impl IntoElement) -> impl IntoElement {
    pref_field(label, note, control)
}

fn choice_row(
    view: &Entity<ImagesPane>,
    key: &'static str,
    id: &'static str,
    choices: &[Choice],
    selected: &str,
    disabled: bool,
) -> impl IntoElement {
    let view = view.clone();
    let options = choices.iter().map(|choice| (choice.value, choice.label));
    let control = select_control(id, options, selected, move |chosen, _window, app| {
        if disabled {
            return;
        }
        write_pref_back(&view, key, Value::String(chosen.to_string()), app);
    });
    if disabled {
        div().opacity(0.5).child(control)
    } else {
        div().child(control)
    }
}

fn bool_row(
    view: &Entity<ImagesPane>,
    key: &'static str,
    id: &'static str,
    checked: bool,
) -> impl IntoElement {
    let view = view.clone();
    toggle(id, checked, move |next, _window, app| {
        write_pref_back(&view, key, Value::Bool(next), app);
    })
}

fn write_pref_back(view: &Entity<ImagesPane>, key: &'static str, value: Value, app: &mut App) {
    view.update(app, |pane, cx| pane.write_pref(key, value, cx));
}

fn error_line(show: bool, message: &'static str) -> impl IntoElement {
    if show {
        div().text_sm().text_color(rgb(DANGER)).child(message)
    } else {
        div()
    }
}

fn chip_button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    choice_button(id, label, false, move |_event, _window, app| on_click(app))
}

fn accent_button(
    id: &'static str,
    label: &'static str,
    on_click: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    action_button(id, label, move |_event, _window, app| on_click(app))
}

fn code_chip(text: &'static str) -> impl IntoElement {
    button_disabled(
        choice_button(text, text, false, |_event, _window, _app| {}),
        true,
    )
}
