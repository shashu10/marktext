use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px, rgb,
};
use gpui_elements::editable_text::{EditableTextState, text_input};
use marktext_core::{HostBridge, MemoryHostBridge};
use marktext_ui_kit::{UI_FONT, select_control, toggle};
use serde_json::Value;

use crate::model::{Choice, GeneralModel, string_array};

/// Called for every user edit. The wasm host posts `SET_PREFERENCE`; tests can record it.
pub type PreferencePublisher = Rc<dyn Fn(String, Value)>;

const PAGE: u32 = 0xFFFFFF;
const INK: u32 = 0x1C1C1F;
const MUTED: u32 = 0x5C5C66;
const CHIP: u32 = 0xF4F4F5;

pub struct GeneralPane {
    bridge: Arc<MemoryHostBridge>,
    publish: PreferencePublisher,
}

impl GeneralPane {
    pub fn new(bridge: Arc<MemoryHostBridge>, publish: PreferencePublisher) -> Self {
        Self { bridge, publish }
    }

    fn write(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.bridge.set_preference(key.to_string(), value.clone());
        (self.publish)(key.to_string(), value);
        cx.notify();
    }
}

impl Render for GeneralPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = GeneralModel::from_snapshot(&self.bridge.get_preferences());
        let draft =
            EditableTextState::use_keyed("setting-treePathExcludePatterns-draft", window, cx);
        let view = cx.entity();

        div()
            .id("general-pane")
            .font_family(UI_FONT)
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_6()
            .p_2()
            .bg(rgb(PAGE))
            .text_color(rgb(INK))
            .child(div().text_xl().child("General"))
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
                    .child(exclude_editor(&view, &model, draft))
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
) -> impl IntoElement {
    let mut chips = div().flex().flex_wrap().gap_2();
    for (index, pattern) in model.exclude_patterns.iter().enumerate() {
        let view = view.clone();
        chips = chips.child(
            div()
                .id(SharedString::from(format!(
                    "setting-treePathExcludePatterns-remove-{index}"
                )))
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(CHIP))
                .cursor_pointer()
                .child(format!("{pattern}  ×"))
                .on_click(move |_event, _window, app| {
                    let view = view.clone();
                    view.update(app, |pane, cx| {
                        let current = GeneralModel::from_snapshot(&pane.bridge.get_preferences());
                        let next = current.patterns_without(index);
                        pane.write("treePathExcludePatterns", string_array(&next), cx);
                    });
                }),
        );
    }

    let view = view.clone();
    let draft_for_add = draft.clone();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(rgb(INK)).child("File tree exclusion rules"))
        .child(
            div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child("Paths matching any glob rule are excluded from the file tree. Add a rule, or click one to remove it. Reopen the folder to fully apply changes."),
        )
        .child(chips)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    text_input("setting-treePathExcludePatterns-draft")
                        .state(draft.downgrade())
                        .placeholder("For example: **/drafts/** or *.tmp")
                        .font_family(UI_FONT)
                        .w(px(360.))
                        .min_h(px(32.))
                        .px_2()
                        .bg(rgb(CHIP))
                        .text_color(rgb(INK)),
                )
                .child(
                    div()
                        .id("setting-treePathExcludePatterns-add")
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(0x4C9AFF))
                        .text_color(rgb(PAGE))
                        .cursor_pointer()
                        .child("Add")
                        .on_click(move |_event, _window, app| {
                            let pattern = draft_for_add.read(app).as_str().to_string();
                            let saved = view.update(app, |pane, cx| {
                                let current = GeneralModel::from_snapshot(&pane.bridge.get_preferences());
                                let added = current.patterns_after_add(&pattern);
                                let Some(next) = added else {
                                    return false;
                                };
                                pane.write("treePathExcludePatterns", string_array(&next), cx);
                                true
                            });
                            if saved {
                                draft_for_add.update(app, |state, cx| state.emplace("", cx));
                            }
                        }),
                ),
        )
}

fn write_back(view: &Entity<GeneralPane>, key: &'static str, value: Value, app: &mut App) {
    view.update(app, |pane, cx| pane.write(key, value, cx));
}
