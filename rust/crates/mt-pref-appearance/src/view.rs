use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px, rgb,
};
use gpui_elements::editable_text::{EditableTextState, text_area, text_input};
use marktext_core::{HostBridge, MemoryHostBridge};
use marktext_ui_kit::{UI_FONT, select_control, toggle};
use serde_json::{Value, json};

use crate::model::{
    AppearanceModel, TextSave, ThemeMode, ZOOM_STEPS, font_family_ok, format_step, line_width_ok,
    step_by, theme_label, theme_swatch, visible_themes, zoom_selected,
};

pub type PreferencePublisher = Rc<dyn Fn(String, Value)>;
pub type PersistPublisher = Rc<dyn Fn(u64, String, Value)>;

const PAGE: u32 = 0xFFFFFF;
const INK: u32 = 0x1C1C1F;
const MUTED: u32 = 0x5C5C66;
const CHIP: u32 = 0xF4F4F5;
const LINE: u32 = 0xD8D8DE;

pub struct AppearancePane {
    bridge: Arc<MemoryHostBridge>,
    publish: PreferencePublisher,
    persist: PersistPublisher,
    save: TextSave,
    theme_mode: ThemeMode,
    format_error: Option<String>,
    search: Option<Entity<EditableTextState>>,
    css: Option<Entity<EditableTextState>>,
    editor_font: Option<Entity<EditableTextState>>,
    code_font: Option<Entity<EditableTextState>>,
    line_width: Option<Entity<EditableTextState>>,
}

impl AppearancePane {
    pub fn new(
        bridge: Arc<MemoryHostBridge>,
        publish: PreferencePublisher,
        persist: PersistPublisher,
    ) -> Self {
        Self {
            bridge,
            publish,
            persist,
            save: TextSave::new(),
            theme_mode: ThemeMode::All,
            format_error: None,
            search: None,
            css: None,
            editor_font: None,
            code_font: None,
            line_width: None,
        }
    }

    fn write(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.bridge.set_preference(key.to_string(), value.clone());
        (self.publish)(key.to_string(), value);
        cx.notify();
    }

    fn commit_text(&mut self, key: &str, value: String, cx: &mut Context<Self>) {
        let accepted = match key {
            "editorLineWidth" => line_width_ok(&value),
            "editorFontFamily" | "codeFontFamily" => font_family_ok(&value),
            "customCss" => true,
            _ => false,
        };
        if !accepted {
            self.format_error = Some(key.to_string());
            cx.notify();
            return;
        }
        self.format_error = None;
        let id = self
            .save
            .request(key.to_string(), Value::String(value.clone()));
        if let Some(id) = id {
            (self.persist)(id, key.to_string(), Value::String(value));
            cx.notify();
        }
    }

    pub fn accept_persist_result(&mut self, id: u64, persisted: bool, cx: &mut Context<Self>) {
        let applied = self.save.finish(id, persisted);
        if let Some((key, value)) = applied {
            self.bridge.set_preference(key, value);
        }
        cx.notify();
    }
}

impl Render for AppearancePane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = AppearanceModel::from_snapshot(&self.bridge.get_preferences());
        let search = EditableTextState::use_keyed("setting-theme-search", window, cx);
        let css = EditableTextState::use_keyed("setting-customCss", window, cx);
        let editor_font = EditableTextState::use_keyed("setting-editorFontFamily", window, cx);
        let code_font = EditableTextState::use_keyed("setting-codeFontFamily", window, cx);
        let line_width = EditableTextState::use_keyed("setting-editorLineWidth", window, cx);
        self.search = Some(search.clone());
        self.css = Some(css.clone());
        self.editor_font = Some(editor_font.clone());
        self.code_font = Some(code_font.clone());
        self.line_width = Some(line_width.clone());
        let query = search.read(cx).as_str().to_string();
        let saving = self.save.is_busy();
        let save_error = self.save.error_key.clone();
        let format_error = self.format_error.clone();
        let theme_mode = self.theme_mode;
        let view = cx.entity();

        div()
            .id("appearance-pane")
            .font_family(UI_FONT)
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_6()
            .p_2()
            .bg(rgb(PAGE))
            .text_color(rgb(INK))
            .child(div().text_xl().child("Appearance"))
            .child(section(
                "Theme",
                theme_section(&view, &model, &search, &query, theme_mode),
            ))
            .child(section(
                "Custom CSS",
                css_editor(
                    &view,
                    &css,
                    &model.custom_css,
                    saving,
                    save_error.as_deref() == Some("customCss"),
                ),
            ))
            .child(section("Window", window_section(&view, &model)))
            .child(section(
                "Text editor",
                typography_section(
                    &view,
                    &model,
                    &editor_font,
                    &code_font,
                    &line_width,
                    saving,
                    &format_error,
                ),
            ))
    }
}

fn theme_section(
    view: &Entity<AppearancePane>,
    model: &AppearanceModel,
    search: &Entity<EditableTextState>,
    query: &str,
    theme_mode: ThemeMode,
) -> impl IntoElement {
    let follow = bool_row(
        view,
        "followSystemTheme",
        "setting-followSystemTheme",
        model.follow_system_theme,
    );
    let mut column = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(labeled(
            "Follow system appearance",
            if model.follow_system_theme {
                "Choose separate themes for dark and light modes below."
            } else {
                ""
            },
            follow,
        ))
        .child(
            text_input("setting-theme-search")
                .state(search.downgrade())
                .placeholder("Search themes")
                .font_family(UI_FONT)
                .w(px(280.))
                .min_h(px(32.))
                .px_2()
                .bg(rgb(CHIP))
                .text_color(rgb(INK)),
        );
    if model.follow_system_theme {
        column = column
            .child(theme_list(
                view,
                "lightModeTheme",
                "Light mode theme",
                &model.light_mode_theme,
                query,
                ThemeMode::Light,
            ))
            .child(theme_list(
                view,
                "darkModeTheme",
                "Dark mode theme",
                &model.dark_mode_theme,
                query,
                ThemeMode::Dark,
            ));
    } else {
        column = column.child(mode_chips(view, theme_mode)).child(theme_list(
            view,
            "theme",
            "Theme",
            &model.theme,
            query,
            theme_mode,
        ));
    }
    column
}

fn mode_chips(view: &Entity<AppearancePane>, selected: ThemeMode) -> impl IntoElement {
    let modes = [
        (ThemeMode::All, "All"),
        (ThemeMode::Light, "Light"),
        (ThemeMode::Dark, "Dark"),
    ];
    let mut row = div().flex().gap_2();
    for (mode, label) in modes {
        let view = view.clone();
        let active = mode == selected;
        row = row.child(
            div()
                .id(SharedString::from(format!("setting-theme-filter-{label}")))
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(if active { 0x4C9AFF } else { CHIP }))
                .cursor_pointer()
                .child(label)
                .on_click(move |_event, _window, app| {
                    view.update(app, |pane, cx| {
                        pane.theme_mode = mode;
                        cx.notify();
                    });
                }),
        );
    }
    row
}

fn theme_list(
    view: &Entity<AppearancePane>,
    key: &'static str,
    title: &'static str,
    selected: &str,
    query: &str,
    mode: ThemeMode,
) -> impl IntoElement {
    let names = visible_themes(query, mode);
    let mut chips = div().flex().flex_wrap().gap_2();
    if names.is_empty() {
        chips = chips.child(
            div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child("No themes match your search."),
        );
    }
    for name in names {
        let view = view.clone();
        let active = name == selected;
        let swatch = theme_swatch(name);
        chips = chips.child(
            div()
                .id(SharedString::from(format!("setting-{key}-{name}")))
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(if active { 0x4C9AFF } else { CHIP }))
                .text_color(rgb(if active { PAGE } else { INK }))
                .cursor_pointer()
                .child(
                    div()
                        .size(px(14.))
                        .rounded_sm()
                        .bg(rgb(swatch))
                        .border_1()
                        .border_color(rgb(LINE)),
                )
                .child(theme_label(name))
                .on_click(move |_event, _window, app| {
                    let chosen = name.to_string();
                    view.update(app, |pane, cx| {
                        pane.write(key, Value::String(chosen), cx);
                    });
                }),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(rgb(INK)).child(title))
        .child(chips)
}

fn css_editor(
    view: &Entity<AppearancePane>,
    draft: &Entity<EditableTextState>,
    current: &str,
    saving: bool,
    save_error: bool,
) -> impl IntoElement {
    let view = view.clone();
    let draft_for_save = draft.clone();
    let current_note = if current.is_empty() {
        "No custom CSS is saved.".to_string()
    } else {
        format!(
            "Saved CSS is kept until you replace it. Current length: {} characters.",
            current.len()
        )
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_sm().text_color(rgb(MUTED)).child(current_note))
        .child(
            text_area("setting-customCss")
                .state(draft.downgrade())
                .placeholder("Custom CSS")
                .font_family("Liberation Sans")
                .w_full()
                .min_h(px(120.))
                .px_2()
                .py_2()
                .bg(rgb(CHIP))
                .text_color(rgb(INK)),
        )
        .child(save_button(
            "setting-customCss-save",
            saving,
            move |_event, _window, app| {
                let value = draft_for_save.read(app).as_str().to_string();
                view.update(app, |pane, cx| pane.commit_text("customCss", value, cx));
            },
        ))
        .child(error_line(save_error.then_some(
            "Could not save this setting. The previous value was kept.",
        )))
}

fn window_section(view: &Entity<AppearancePane>, model: &AppearanceModel) -> impl IntoElement {
    let zoom_options = ZOOM_STEPS.iter().map(|(value, label)| {
        (
            SharedString::from(format!("{value}")),
            SharedString::from(*label),
        )
    });
    let selected_zoom = ZOOM_STEPS
        .iter()
        .find(|(value, _)| zoom_selected(model.zoom, *value))
        .map(|(value, _)| format!("{value}"))
        .unwrap_or_else(|| "1".to_string());
    let view_for_zoom = view.clone();
    let mut column = div().flex().flex_col().gap_4();
    if !cfg!(target_os = "macos") {
        column = column.child(labeled(
            "Title bar style",
            "Requires restart",
            select_control(
                "setting-titleBarStyle",
                [("custom", "Custom"), ("native", "Native")],
                model.title_bar_style.as_str(),
                {
                    let view = view.clone();
                    move |chosen, _window, app| {
                        write_back(
                            &view,
                            "titleBarStyle",
                            Value::String(chosen.to_string()),
                            app,
                        );
                    }
                },
            ),
        ));
    }
    column
        .child(labeled(
            "Hide scrollbars",
            "",
            bool_row(
                view,
                "hideScrollbar",
                "setting-hideScrollbar",
                model.hide_scrollbar,
            ),
        ))
        .child(labeled(
            "Zoom",
            "",
            select_control(
                "setting-zoom",
                zoom_options,
                selected_zoom.as_str(),
                move |chosen, _window, app| {
                    let parsed = chosen.parse::<f64>().ok();
                    if let Some(zoom) = parsed {
                        write_back(&view_for_zoom, "zoom", json!(zoom), app);
                    }
                },
            ),
        ))
}

fn typography_section(
    view: &Entity<AppearancePane>,
    model: &AppearanceModel,
    editor_font: &Entity<EditableTextState>,
    code_font: &Entity<EditableTextState>,
    line_width: &Entity<EditableTextState>,
    saving: bool,
    format_error: &Option<String>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(labeled(
            "Font size",
            "",
            stepper(
                view,
                "fontSize",
                "setting-fontSize",
                model.font_size,
                1.0,
                12.0,
                32.0,
            ),
        ))
        .child(labeled(
            "Code block font size",
            "",
            stepper(
                view,
                "codeFontSize",
                "setting-codeFontSize",
                model.code_font_size,
                1.0,
                12.0,
                28.0,
            ),
        ))
        .child(labeled(
            "Line height",
            "",
            stepper(
                view,
                "lineHeight",
                "setting-lineHeight",
                model.line_height,
                0.1,
                1.2,
                2.0,
            ),
        ))
        .child(font_field(FontField {
            view,
            key: "editorFontFamily",
            id: "setting-editorFontFamily",
            title: "Font family",
            note: "Empty uses the built-in fallback fonts.",
            current: &model.editor_font_family,
            draft: editor_font,
            saving,
            invalid: format_error.as_deref() == Some("editorFontFamily"),
        }))
        .child(font_field(FontField {
            view,
            key: "codeFontFamily",
            id: "setting-codeFontFamily",
            title: "Code font family",
            note: "Applies to inline code and code blocks. Empty uses the built-in fallback fonts.",
            current: &model.code_font_family,
            draft: code_font,
            saving,
            invalid: format_error.as_deref() == Some("codeFontFamily"),
        }))
        .child(font_field(FontField {
            view,
            key: "editorLineWidth",
            id: "setting-editorLineWidth",
            title: "Maximum editor content width",
            note: "A number followed by px, ch, or %, such as 800px. Empty uses the default width.",
            current: if model.editor_line_width.is_empty() {
                "default"
            } else {
                model.editor_line_width.as_str()
            },
            draft: line_width,
            saving,
            invalid: format_error.as_deref() == Some("editorLineWidth"),
        }))
}

struct FontField<'a> {
    view: &'a Entity<AppearancePane>,
    key: &'static str,
    id: &'static str,
    title: &'static str,
    note: &'static str,
    current: &'a str,
    draft: &'a Entity<EditableTextState>,
    saving: bool,
    invalid: bool,
}

fn font_field(field: FontField<'_>) -> impl IntoElement {
    let FontField {
        view,
        key,
        id,
        title,
        note,
        current,
        draft,
        saving,
        invalid,
    } = field;
    let view = view.clone();
    let draft_for_save = draft.clone();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(rgb(INK)).child(title))
        .child(
            div()
                .text_sm()
                .text_color(rgb(MUTED))
                .child(format!("Current: {current}. {note}")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .on_key_down({
                    let view = view.clone();
                    let draft_for_enter = draft_for_save.clone();
                    move |event, _window, app| {
                        let plain_enter =
                            event.keystroke.key == "enter" && !event.keystroke.modifiers.modified();
                        if plain_enter {
                            let value = draft_for_enter.read(app).as_str().to_string();
                            view.update(app, |pane, cx| pane.commit_text(key, value, cx));
                        }
                    }
                })
                .child(
                    text_input(id)
                        .state(draft.downgrade())
                        .placeholder(title)
                        .font_family(UI_FONT)
                        .w(px(280.))
                        .min_h(px(32.))
                        .px_2()
                        .bg(rgb(CHIP))
                        .text_color(rgb(INK)),
                )
                .child(save_button(
                    SharedString::from(format!("{id}-save")),
                    saving,
                    move |_event, _window, app| {
                        let value = draft_for_save.read(app).as_str().to_string();
                        view.update(app, |pane, cx| pane.commit_text(key, value, cx));
                    },
                )),
        )
        .child(error_line(invalid.then_some(match key {
            "editorLineWidth" => "Enter a number followed by px, ch, or %, such as 800px.",
            _ => "The font family can't start or end with a space.",
        })))
}

fn stepper(
    view: &Entity<AppearancePane>,
    key: &'static str,
    id: &'static str,
    value: f64,
    step: f64,
    min: f64,
    max: f64,
) -> impl IntoElement {
    let shown = format_step(value, step);
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(step_button(
            view,
            key,
            format!("{id}-dec"),
            StepChange {
                value,
                delta: -1,
                step,
                min,
                max,
            },
            "−",
        ))
        .child(div().min_w(px(48.)).child(shown))
        .child(step_button(
            view,
            key,
            format!("{id}-inc"),
            StepChange {
                value,
                delta: 1,
                step,
                min,
                max,
            },
            "+",
        ))
}

struct StepChange {
    value: f64,
    delta: i32,
    step: f64,
    min: f64,
    max: f64,
}

fn step_button(
    view: &Entity<AppearancePane>,
    key: &'static str,
    id: String,
    change: StepChange,
    label: &'static str,
) -> impl IntoElement {
    let StepChange {
        value,
        delta,
        step,
        min,
        max,
    } = change;
    let view = view.clone();
    div()
        .id(SharedString::from(id))
        .px_2()
        .py_1()
        .rounded_md()
        .bg(rgb(CHIP))
        .cursor_pointer()
        .child(label)
        .on_click(move |_event, _window, app| {
            let next = step_by(value, delta, step, min, max);
            write_back(&view, key, json!(next), app);
        })
}

fn bool_row(
    view: &Entity<AppearancePane>,
    key: &'static str,
    id: &'static str,
    checked: bool,
) -> impl IntoElement {
    let view = view.clone();
    toggle(id, checked, move |next, _window, app| {
        write_back(&view, key, Value::Bool(next), app);
    })
}

fn save_button(
    id: impl Into<SharedString>,
    saving: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(0x4C9AFF))
        .text_color(rgb(PAGE))
        .cursor_pointer()
        .child(if saving { "Saving…" } else { "Save" })
        .on_click(on_click)
}

fn error_line(message: Option<&'static str>) -> impl IntoElement {
    match message {
        Some(text) => div().text_sm().text_color(rgb(0xB42318)).child(text),
        None => div(),
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

fn write_back(view: &Entity<AppearancePane>, key: &'static str, value: Value, app: &mut App) {
    view.update(app, |pane, cx| pane.write(key, value, cx));
}
