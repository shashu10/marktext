use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px,
};
use gpui_elements::editable_text::EditableTextState;
use marktext_core::{HostBridge, MemoryHostBridge};
use marktext_ui_kit::{
    action_button, choice_button, pref_error, pref_field, pref_page, pref_section, pref_text_input,
    select_control, toggle,
};
use serde_json::Value;

use crate::model::{
    Choice, GeneralModel, PersistDecision, PersistGate, draft_still_matches, string_array,
};

/// Called for every user edit. The wasm host posts `SET_PREFERENCE`; tests can record it.
pub type PreferencePublisher = Rc<dyn Fn(String, Value)>;

/// Exclusion edits. The host answers with `PREFERENCE_RESULT` before the draft is cleared.
pub type PersistPublisher = Rc<dyn Fn(u64, String, Value)>;

pub struct GeneralPane {
    bridge: Arc<MemoryHostBridge>,
    publish: PreferencePublisher,
    persist: PersistPublisher,
    gate: PersistGate,
    draft: Option<Entity<EditableTextState>>,
}

impl GeneralPane {
    pub fn new(
        bridge: Arc<MemoryHostBridge>,
        publish: PreferencePublisher,
        persist: PersistPublisher,
    ) -> Self {
        Self {
            bridge,
            publish,
            persist,
            gate: PersistGate::new(),
            draft: None,
        }
    }

    fn write(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.bridge.set_preference(key.to_string(), value.clone());
        (self.publish)(key.to_string(), value);
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

    pub fn accept_persist_result(&mut self, id: u64, persisted: bool, cx: &mut Context<Self>) {
        let decision = self.gate.finish(id, persisted);
        if let PersistDecision::Applied(pending) = decision {
            let submitted = pending.draft_text.clone();
            self.bridge.set_preference(pending.key, pending.value);
            if let Some(draft) = self.draft.clone() {
                let current = draft.read(cx).as_str().to_string();
                if draft_still_matches(submitted.as_deref(), &current) {
                    draft.update(cx, |state, cx| state.emplace("", cx));
                }
            }
        }
        cx.notify();
    }
}

impl Render for GeneralPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = GeneralModel::from_snapshot(&self.bridge.get_preferences());
        let draft =
            EditableTextState::use_keyed("setting-treePathExcludePatterns-draft", window, cx);
        self.draft = Some(draft.clone());
        let save_error = self.gate.save_error;
        let saving = self.gate.is_busy();
        let view = cx.entity();

        pref_page("general-pane", "General", cx)
            .child(section(
                "Language",
                labeled(
                    "Interface language",
                    "",
                    choice_row(&view, "language", "setting-language", GeneralModel::language_choices(), &model.language),
                ),
            ))
            .child(section(
                "Opening mode",
                labeled(
                    "Default mode for opened files",
                    "Applies to existing files opened from now on. New documents remain editable.",
                    choice_row(&view, "defaultOpenMode", "setting-defaultOpenMode", GeneralModel::open_mode_choices(), &model.default_open_mode),
                ),
            ))
            .child(section(
                "Window",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Open files in new window",
                        "When enabled, files open in a new window. When disabled, they open in tabs in the current window.",
                        bool_row(&view, "openFilesInNewWindow", "setting-openFilesInNewWindow", model.open_files_in_new_window),
                    ))
                    .child(labeled(
                        "Open folders in new window",
                        "When enabled, folders open in a new window. When disabled, they replace the folder in the current window.",
                        bool_row(&view, "openFolderInNewWindow", "setting-openFolderInNewWindow", model.open_folder_in_new_window),
                    )),
            ))
            .child(section(
                "Sidebar and table of contents",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Default table of contents position",
                        "",
                        choice_row(&view, "tocPosition", "setting-tocPosition", GeneralModel::toc_choices(), &model.toc_position),
                    ))
                    .child(labeled(
                        "Wrap text in table of contents",
                        "",
                        bool_row(&view, "wordWrapInToc", "setting-wordWrapInToc", model.word_wrap_in_toc),
                    ))
                    .child(labeled(
                        "Show opened files",
                        "",
                        bool_row(&view, "openedFilesInSidebar", "setting-openedFilesInSidebar", model.opened_files_in_sidebar),
                    ))
                    .child(labeled(
                        "Reveal the active file in the sidebar",
                        "",
                        bool_row(&view, "autoRevealInSidebar", "setting-autoRevealInSidebar", model.auto_reveal_in_sidebar),
                    ))
                    .child(labeled(
                        "Open sidebar files in preview tabs. Editing or double-clicking keeps the tab open.",
                        "",
                        bool_row(&view, "openFilesInPreviewMode", "setting-openFilesInPreviewMode", model.open_files_in_preview_mode),
                    ))
                    .child(exclude_editor(&view, &model, draft, save_error, saving, cx))
                    .child(labeled(
                        "File Sort By",
                        "",
                        choice_row(&view, "fileSortBy", "setting-fileSortBy", GeneralModel::sort_by_choices(), &model.file_sort_by),
                    ))
                    .child(labeled(
                        "Sort Order",
                        "",
                        choice_row(&view, "fileSortOrder", "setting-fileSortOrder", &model.sort_order_choices(), &model.file_sort_order),
                    )),
            ))
    }
}

fn section(title: &'static str, body: impl IntoElement) -> impl IntoElement {
    pref_section(title, body)
}

fn labeled(label: &'static str, note: &'static str, control: impl IntoElement) -> impl IntoElement {
    pref_field(label, note, control)
}

fn choice_row(
    view: &Entity<GeneralPane>,
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

fn bool_row(
    view: &Entity<GeneralPane>,
    key: &'static str,
    id: &'static str,
    checked: bool,
) -> impl IntoElement {
    let view = view.clone();
    toggle(id, checked, move |next, _window, app| {
        write_back(&view, key, Value::Bool(next), app);
    })
}

fn exclude_editor(
    view: &Entity<GeneralPane>,
    model: &GeneralModel,
    draft: Entity<EditableTextState>,
    save_error: bool,
    saving: bool,
    cx: &App,
) -> impl IntoElement {
    let mut chips = div().flex().flex_wrap().gap_2();
    for (index, pattern) in model.exclude_patterns.iter().enumerate() {
        let view = view.clone();
        chips = chips.child(choice_button(
            SharedString::from(format!("setting-treePathExcludePatterns-remove-{index}")),
            format!("{pattern}  ×"),
            false,
            move |_event, _window, app| {
                let current = GeneralModel::from_snapshot(&view.read(app).bridge.get_preferences());
                let next = current.patterns_without(index);
                if next.len() != current.exclude_patterns.len() {
                    view.update(app, |pane, cx| {
                        pane.request_persist(
                            "treePathExcludePatterns",
                            string_array(&next),
                            None,
                            cx,
                        );
                    });
                }
            },
        ));
    }

    let view = view.clone();
    let draft_for_add = draft.clone();
    let error = if save_error {
        pref_error(
            "Could not save exclusion rules. The saved rules have been kept. Please try again.",
            cx,
        )
        .into_any_element()
    } else {
        div().into_any_element()
    };
    pref_field(
        "File tree exclusion rules",
        "Paths matching any glob rule are excluded from the file tree. Add a rule, or click one to remove it. Reopen the folder to fully apply changes.",
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(chips)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .on_key_down({
                        let view = view.clone();
                        let draft_for_enter = draft_for_add.clone();
                        move |event, _window, app| {
                            let plain_enter = event.keystroke.key == "enter"
                                && !event.keystroke.modifiers.modified();
                            if plain_enter {
                                submit_exclusion(&view, &draft_for_enter, app);
                            }
                        }
                    })
                    .child(
                        pref_text_input("setting-treePathExcludePatterns-draft")
                            .state(draft.downgrade())
                            .placeholder("For example: **/drafts/** or *.tmp")
                            .w(px(360.)),
                    )
                    .child(action_button(
                        "setting-treePathExcludePatterns-add",
                        if saving { "Saving…" } else { "Add" },
                        move |_event, _window, app| {
                            submit_exclusion(&view, &draft_for_add, app);
                        },
                    )),
            )
            .child(error),
    )
}

fn submit_exclusion(view: &Entity<GeneralPane>, draft: &Entity<EditableTextState>, app: &mut App) {
    let pattern = draft.read(app).as_str().to_string();
    let current = GeneralModel::from_snapshot(&view.read(app).bridge.get_preferences());
    let added = current.patterns_after_add(&pattern);
    if let Some(next) = added {
        view.update(app, |pane, cx| {
            pane.request_persist(
                "treePathExcludePatterns",
                string_array(&next),
                Some(pattern),
                cx,
            );
        });
    }
}

fn write_back(view: &Entity<GeneralPane>, key: &'static str, value: Value, app: &mut App) {
    view.update(app, |pane, cx| pane.write(key, value, cx));
}
