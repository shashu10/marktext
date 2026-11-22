use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*, px, rgb,
};
use gpui_elements::editable_text::{EditableTextState, StringStorage};
use marktext_core::{HostBridge, MemoryHostBridge};
use marktext_ui_kit::{
    action_button, choice_button, pref_field, pref_page, pref_section, pref_text_area,
    pref_text_input, select_control, toggle,
};
use serde_json::{Value, json};

use crate::model::{
    AppearanceModel, TextSave, ThemeMode, ZOOM_STEPS, draft_should_adopt, font_family_ok,
    format_step, line_width_ok, step_by, theme_label, theme_swatch, visible_themes, zoom_selected,
};

pub type PreferencePublisher = Rc<dyn Fn(String, Value)>;
pub type PersistPublisher = Rc<dyn Fn(u64, String, Value)>;

const INK: u32 = 0x1C1C1F;
const MUTED: u32 = 0x5C5C66;
const LINE: u32 = 0xD8D8DE;
const SAVE_FAILED: &str = "Could not save this setting. The previous value was kept.";

/// Saved text last copied into a field. A draft that still equals `baseline`
/// can follow a newer snapshot. Keystrokes move the draft off `baseline`.
struct BoundText {
    entity: Entity<EditableTextState>,
    baseline: String,
}

pub struct AppearancePane {
    bridge: Arc<MemoryHostBridge>,
    publish: PreferencePublisher,
    persist: PersistPublisher,
    save: TextSave,
    theme_mode: ThemeMode,
    format_error: Option<String>,
    search: Option<Entity<EditableTextState>>,
    css: Option<BoundText>,
    editor_font: Option<BoundText>,
    code_font: Option<BoundText>,
    line_width: Option<BoundText>,
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
            if let Some(text) = value.as_str() {
                self.confirm_saved_text(&key, text);
            }
            self.bridge.set_preference(key, value);
        }
        cx.notify();
    }

    fn confirm_saved_text(&mut self, key: &str, saved: &str) {
        let Some(field) = self.bound_text(key) else {
            return;
        };
        field.baseline = saved.to_string();
    }

    fn adopt_saved_text(&mut self, key: &'static str, saved: String, cx: &mut Context<Self>) {
        let Some(field) = self.bound_text(key) else {
            return;
        };
        let current = field.entity.read(cx).as_str();
        if !draft_should_adopt(current, &field.baseline, &saved) {
            return;
        }
        field.baseline = saved.clone();
        field
            .entity
            .update(cx, |state, cx| state.emplace(&saved, cx));
    }

    fn bound_text(&mut self, key: &str) -> Option<&mut BoundText> {
        match key {
            "customCss" => self.css.as_mut(),
            "editorFontFamily" => self.editor_font.as_mut(),
            "codeFontFamily" => self.code_font.as_mut(),
            "editorLineWidth" => self.line_width.as_mut(),
            _ => None,
        }
    }
}

/// The keyed state is created with the snapshot. A later snapshot is copied
/// in after this frame, because replacing the text notifies that field while
/// this pane is still drawing.
fn bind_saved_text(
    bound: &mut Option<BoundText>,
    key: &'static str,
    element_id: &'static str,
    saved: &str,
    window: &mut Window,
    cx: &mut Context<AppearancePane>,
) -> Entity<EditableTextState> {
    let entity = EditableTextState::use_keyed_init(element_id, window, cx, {
        let initial = saved.to_string();
        move |_window, _cx| StringStorage::from(initial)
    });
    let adopt = match bound {
        Some(field) => draft_should_adopt(field.entity.read(cx).as_str(), &field.baseline, saved),
        None => {
            let current = entity.read(cx).as_str().to_string();
            *bound = Some(BoundText {
                entity: entity.clone(),
                baseline: current,
            });
            false
        }
    };
    if adopt {
        let saved = saved.to_string();
        cx.defer_in(window, move |pane, _window, cx| {
            pane.adopt_saved_text(key, saved, cx);
        });
    }
    entity
}

impl Render for AppearancePane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = AppearanceModel::from_snapshot(&self.bridge.get_preferences());
        let search = EditableTextState::use_keyed("setting-theme-search", window, cx);
        let css = bind_saved_text(
            &mut self.css,
            "customCss",
            "setting-customCss",
            &model.custom_css,
            window,
            cx,
        );
        let editor_font = bind_saved_text(
            &mut self.editor_font,
            "editorFontFamily",
            "setting-editorFontFamily",
            &model.editor_font_family,
            window,
            cx,
        );
        let code_font = bind_saved_text(
            &mut self.code_font,
            "codeFontFamily",
            "setting-codeFontFamily",
            &model.code_font_family,
            window,
            cx,
        );
        let line_width = bind_saved_text(
            &mut self.line_width,
            "editorLineWidth",
            "setting-editorLineWidth",
            &model.editor_line_width,
            window,
            cx,
        );
        self.search = Some(search.clone());
        let query = search.read(cx).as_str().to_string();
        let pending = self.save.pending_key().map(str::to_string);
        let save_error = self.save.error_key.clone();
        let format_error = self.format_error.clone();
        let theme_mode = self.theme_mode;
        let view = cx.entity();

        pref_page("appearance-pane", "Appearance", cx)
            .child(section(
                "Theme",
                theme_section(&view, &model, &search, &query, theme_mode),
            ))
            .child(section(
                "Custom CSS",
                css_editor(
                    &view,
                    &css,
                    pending.as_deref() == Some("customCss"),
                    save_error.as_deref() == Some("customCss"),
                ),
            ))
            .child(section("Window", window_section(&view, &model)))
            .child(section(
                "Text editor",
                typography_section(
                    &view,
                    &model,
                    TypographyFields {
                        editor_font: &editor_font,
                        code_font: &code_font,
                        line_width: &line_width,
                        pending: pending.as_deref(),
                        save_error: save_error.as_deref(),
                        format_error: &format_error,
                    },
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
            pref_text_input("setting-theme-search")
                .state(search.downgrade())
                .placeholder("Search themes")
                .w(px(280.)),
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
        row = row.child(choice_button(
            SharedString::from(format!("setting-theme-filter-{label}")),
            label,
            active,
            move |_event, _window, app| {
                view.update(app, |pane, cx| {
                    pane.theme_mode = mode;
                    cx.notify();
                });
            },
        ));
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
            choice_button(
                SharedString::from(format!("setting-{key}-{name}")),
                theme_label(name),
                active,
                move |_event, _window, app| {
                    let chosen = name.to_string();
                    view.update(app, |pane, cx| {
                        pane.write(key, Value::String(chosen), cx);
                    });
                },
            )
            .child(
                div()
                    .size(px(14.))
                    .rounded_sm()
                    .bg(rgb(swatch))
                    .border_1()
                    .border_color(rgb(LINE)),
            ),
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
    saving: bool,
    save_error: bool,
) -> impl IntoElement {
    let view = view.clone();
    let draft_for_save = draft.clone();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            pref_text_area("setting-customCss")
                .state(draft.downgrade())
                .placeholder("Custom CSS"),
        )
        .child(save_button(
            "setting-customCss-save",
            saving,
            move |_event, _window, app| {
                let value = draft_for_save.read(app).as_str().to_string();
                view.update(app, |pane, cx| pane.commit_text("customCss", value, cx));
            },
        ))
        .child(error_line(save_error.then_some(SAVE_FAILED)))
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

struct TypographyFields<'a> {
    editor_font: &'a Entity<EditableTextState>,
    code_font: &'a Entity<EditableTextState>,
    line_width: &'a Entity<EditableTextState>,
    pending: Option<&'a str>,
    save_error: Option<&'a str>,
    format_error: &'a Option<String>,
}

fn typography_section(
    view: &Entity<AppearancePane>,
    model: &AppearanceModel,
    fields: TypographyFields<'_>,
) -> impl IntoElement {
    let TypographyFields {
        editor_font,
        code_font,
        line_width,
        pending,
        save_error,
        format_error,
    } = fields;
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
            draft: editor_font,
            saving: pending == Some("editorFontFamily"),
            invalid: format_error.as_deref() == Some("editorFontFamily"),
            rejected: save_error == Some("editorFontFamily"),
        }))
        .child(font_field(FontField {
            view,
            key: "codeFontFamily",
            id: "setting-codeFontFamily",
            title: "Code font family",
            note: "Applies to inline code and code blocks. Empty uses the built-in fallback fonts.",
            draft: code_font,
            saving: pending == Some("codeFontFamily"),
            invalid: format_error.as_deref() == Some("codeFontFamily"),
            rejected: save_error == Some("codeFontFamily"),
        }))
        .child(font_field(FontField {
            view,
            key: "editorLineWidth",
            id: "setting-editorLineWidth",
            title: "Maximum editor content width",
            note: "A number followed by px, ch, or %, such as 800px. Empty uses the default width.",
            draft: line_width,
            saving: pending == Some("editorLineWidth"),
            invalid: format_error.as_deref() == Some("editorLineWidth"),
            rejected: save_error == Some("editorLineWidth"),
        }))
}

struct FontField<'a> {
    view: &'a Entity<AppearancePane>,
    key: &'static str,
    id: &'static str,
    title: &'static str,
    note: &'static str,
    draft: &'a Entity<EditableTextState>,
    saving: bool,
    invalid: bool,
    rejected: bool,
}

fn font_field(field: FontField<'_>) -> impl IntoElement {
    let FontField {
        view,
        key,
        id,
        title,
        note,
        draft,
        saving,
        invalid,
        rejected,
    } = field;
    let view = view.clone();
    let draft_for_save = draft.clone();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_color(rgb(INK)).child(title))
        .child(div().text_sm().text_color(rgb(MUTED)).child(note))
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
                    pref_text_input(id)
                        .state(draft.downgrade())
                        .placeholder(title)
                        .w(px(280.)),
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
        .child(error_line(text_field_error(key, invalid, rejected)))
}

fn text_field_error(key: &str, invalid: bool, rejected: bool) -> Option<&'static str> {
    if invalid {
        return Some(match key {
            "editorLineWidth" => "Enter a number followed by px, ch, or %, such as 800px.",
            _ => "The font family can't start or end with a space.",
        });
    }
    if rejected {
        return Some(SAVE_FAILED);
    }
    None
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
    action_button(
        SharedString::from(id),
        label,
        move |_event, _window, app| {
            let next = step_by(value, delta, step, min, max);
            write_back(&view, key, json!(next), app);
        },
    )
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
    id: impl Into<gpui::ElementId>,
    saving: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    action_button(id, if saving { "Saving…" } else { "Save" }, on_click)
}

fn error_line(message: Option<&'static str>) -> impl IntoElement {
    match message {
        Some(text) => div().text_sm().text_color(rgb(0xB42318)).child(text),
        None => div(),
    }
}

fn section(title: &'static str, body: impl IntoElement) -> impl IntoElement {
    pref_section(title, body)
}

fn labeled(label: &'static str, note: &'static str, control: impl IntoElement) -> impl IntoElement {
    pref_field(label, note, control)
}

fn write_back(view: &Entity<AppearancePane>, key: &'static str, value: Value, app: &mut App) {
    view.update(app, |pane, cx| pane.write(key, value, cx));
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use gpui::TestApp;

    #[test]
    fn a_rejected_typography_save_is_shown_on_that_field() {
        assert_eq!(
            text_field_error("editorFontFamily", false, true),
            Some(SAVE_FAILED)
        );
        assert_eq!(
            text_field_error("editorLineWidth", false, true),
            Some(SAVE_FAILED)
        );
        assert_eq!(
            text_field_error("codeFontFamily", false, true),
            Some(SAVE_FAILED)
        );
        assert!(
            text_field_error("editorLineWidth", true, true)
                .is_some_and(|message| message.contains("px"))
        );
        assert!(text_field_error("editorFontFamily", false, false).is_none());
    }

    fn field_text(field: &Option<BoundText>, cx: &gpui::App) -> String {
        field
            .as_ref()
            .expect("text field")
            .entity
            .read(cx)
            .as_str()
            .to_string()
    }

    #[test]
    fn saved_stylesheet_fills_the_field_without_replacing_an_edit() {
        let mut app = TestApp::new();
        app.update(marktext_ui_kit::init);
        let bridge = Arc::new(MemoryHostBridge::with_defaults());
        bridge.set_preference("customCss".to_string(), json!("h1 { color: red }"));
        bridge.set_preference("editorFontFamily".to_string(), json!("Iosevka"));
        let calls = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&calls);
        let bridge_for_view = Arc::clone(&bridge);
        let mut window = app.open_window(move |_window, _cx| {
            AppearancePane::new(
                bridge_for_view,
                Rc::new(|_, _| {}),
                Rc::new(move |id, key, value| recorded.borrow_mut().push((id, key, value))),
            )
        });

        window.draw();
        window.read(|pane, cx| {
            assert_eq!(field_text(&pane.css, cx), "h1 { color: red }");
            assert_eq!(field_text(&pane.editor_font, cx), "Iosevka");
        });

        window.update(|pane, _window, cx| {
            let value = field_text(&pane.css, cx);
            pane.commit_text("customCss", value, cx);
        });
        assert_eq!(calls.borrow()[0].2, json!("h1 { color: red }"));

        window.update(|pane, _window, cx| {
            pane.css
                .as_ref()
                .expect("css")
                .entity
                .update(cx, |state, cx| state.emplace("h1 { color: blue }", cx));
        });
        bridge.set_preference("customCss".to_string(), json!("h1 { color: green }"));
        window.update(|_pane, _window, cx| cx.notify());
        window.draw();
        window.read(|pane, cx| assert_eq!(field_text(&pane.css, cx), "h1 { color: blue }"));

        let css_id = calls.borrow()[0].0;
        window.update(|pane, _window, cx| pane.accept_persist_result(css_id, true, cx));
        window.update(|pane, _window, cx| {
            pane.line_width
                .as_ref()
                .expect("width")
                .entity
                .update(cx, |state, cx| state.emplace("90ch", cx));
            pane.commit_text("editorLineWidth", "90ch".to_string(), cx);
        });
        let width_id = calls
            .borrow()
            .iter()
            .find(|(_, key, _)| key == "editorLineWidth")
            .expect("width save")
            .0;
        window.update(|pane, _window, cx| pane.accept_persist_result(width_id, false, cx));
        window.read(|pane, cx| {
            assert_eq!(pane.save.error_key.as_deref(), Some("editorLineWidth"));
            assert_eq!(field_text(&pane.line_width, cx), "90ch");
        });
    }

    #[test]
    fn a_later_snapshot_fills_a_clean_stylesheet_field() {
        let mut app = TestApp::new();
        app.update(marktext_ui_kit::init);
        let bridge = Arc::new(MemoryHostBridge::with_defaults());
        let bridge_for_view = Arc::clone(&bridge);
        let mut window = app.open_window(move |_window, _cx| {
            AppearancePane::new(bridge_for_view, Rc::new(|_, _| {}), Rc::new(|_, _, _| {}))
        });

        window.draw();
        window.read(|pane, cx| assert_eq!(field_text(&pane.css, cx), ""));

        bridge.set_preference("customCss".to_string(), json!("p { margin: 0 }"));
        window.update(|_pane, _window, cx| cx.notify());
        window.draw();
        window.read(|pane, cx| assert_eq!(field_text(&pane.css, cx), "p { margin: 0 }"));
    }
}
