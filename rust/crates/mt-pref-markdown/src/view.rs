use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, prelude::*, px,
};
use gpui_elements::editable_text::EditableTextState;
use marktext_core::{HostBridge, MemoryHostBridge};
use marktext_ui_kit::{
    action_button, locked_select, pref_error, pref_field, pref_page, pref_section, pref_text_input,
    select_control, toggle,
};
use serde_json::Value;

use crate::model::{
    Choice, IndentValue, MarkdownModel, PersistDecision, PersistGate, plantuml_url_ok,
};

/// Called for every user edit. The wasm host posts `SET_PREFERENCE`; tests can record it.
pub type PreferencePublisher = Rc<dyn Fn(String, Value)>;

/// PlantUML URL edits. The host answers with `PREFERENCE_RESULT` before the value is trusted.
pub type PersistPublisher = Rc<dyn Fn(u64, String, Value)>;

pub struct MarkdownPane {
    bridge: Arc<MemoryHostBridge>,
    publish: PreferencePublisher,
    persist: PersistPublisher,
    gate: PersistGate,
    plantuml_draft: Option<Entity<EditableTextState>>,
    plantuml_invalid: bool,
    plantuml_seeded: bool,
}

impl MarkdownPane {
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
            plantuml_draft: None,
            plantuml_invalid: false,
            plantuml_seeded: false,
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
            self.bridge.set_preference(pending.key, pending.value);
            self.plantuml_invalid = false;
        }
        cx.notify();
    }
}

impl Render for MarkdownPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = MarkdownModel::from_snapshot(&self.bridge.get_preferences());
        let plantuml = EditableTextState::use_keyed("setting-plantumlServer-draft", window, cx);
        if !self.plantuml_seeded {
            plantuml.update(cx, |state, cx| {
                state.emplace(&model.plantuml_server, cx);
            });
            self.plantuml_seeded = true;
        }
        self.plantuml_draft = Some(plantuml.clone());
        let save_error = self.gate.save_error;
        let saving = self.gate.is_busy();
        let invalid = self.plantuml_invalid;
        let view = cx.entity();

        pref_page("markdown-pane", "Markdown", cx)
            .child(section(
                "Lists",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Prefer loose list items",
                        "",
                        bool_row(
                            &view,
                            "preferLooseListItem",
                            "setting-preferLooseListItem",
                            model.prefer_loose_list_item,
                        ),
                    ))
                    .child(labeled(
                        "Bullet list marker",
                        "",
                        choice_row(
                            &view,
                            "bulletListMarker",
                            "setting-bulletListMarker",
                            MarkdownModel::bullet_marker_choices(),
                            &model.bullet_list_marker,
                        ),
                    ))
                    .child(labeled(
                        "Ordered list delimiter",
                        "",
                        choice_row(
                            &view,
                            "orderListDelimiter",
                            "setting-orderListDelimiter",
                            MarkdownModel::order_delimiter_choices(),
                            &model.order_list_delimiter,
                        ),
                    ))
                    .child(labeled(
                        "List Indentation",
                        "",
                        indent_row(
                            &view,
                            &model.list_indentation.as_token(),
                        ),
                    )),
            ))
            .child(section(
                "Extensions",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Frontmatter Type",
                        "",
                        choice_row(
                            &view,
                            "frontmatterType",
                            "setting-frontmatterType",
                            MarkdownModel::frontmatter_type_choices(),
                            &model.frontmatter_type,
                        ),
                    ))
                    .child(labeled(
                        "Super/Sub script",
                        "",
                        bool_row(
                            &view,
                            "superSubScript",
                            "setting-superSubScript",
                            model.super_sub_script,
                        ),
                    ))
                    .child(labeled(
                        "Footnote",
                        "",
                        bool_row(&view, "footnote", "setting-footnote", model.footnote),
                    ))
                    .child(labeled(
                        "TeX math with $ and $$",
                        "",
                        bool_row(
                            &view,
                            "texMathDollars",
                            "setting-texMathDollars",
                            model.tex_math_dollars,
                        ),
                    ))
                    .child(labeled(
                        "GitHub TeX math ($`…`$ and ```math)",
                        "",
                        bool_row(
                            &view,
                            "texMathGfm",
                            "setting-texMathGfm",
                            model.tex_math_gfm,
                        ),
                    ))
                    .child(labeled(
                        "TeX math with single-backslash delimiters",
                        "",
                        bool_row(
                            &view,
                            "texMathSingleBackslash",
                            "setting-texMathSingleBackslash",
                            model.tex_math_single_backslash,
                        ),
                    ))
                    .child(labeled(
                        "TeX math with double-backslash delimiters",
                        "",
                        bool_row(
                            &view,
                            "texMathDoubleBackslash",
                            "setting-texMathDoubleBackslash",
                            model.tex_math_double_backslash,
                        ),
                    ))
                    .child(labeled(
                        "Letter and Roman numeral lists",
                        "",
                        bool_row(
                            &view,
                            "fancyLists",
                            "setting-fancyLists",
                            model.fancy_lists,
                        ),
                    ))
                    .child(labeled(
                        "Highlight (==text==)",
                        "",
                        bool_row(
                            &view,
                            "highlightSyntax",
                            "setting-highlightSyntax",
                            model.highlight_syntax,
                        ),
                    ))
                    .child(labeled(
                        "Inline diff ({+ … +})",
                        "",
                        bool_row(
                            &view,
                            "inlineDiff",
                            "setting-inlineDiff",
                            model.inline_diff,
                        ),
                    ))
                    .child(labeled(
                        "Enable GitHub Alerts",
                        "",
                        bool_row(
                            &view,
                            "githubAlerts",
                            "setting-githubAlerts",
                            model.github_alerts,
                        ),
                    ))
                    .child(labeled(
                        "Multiline blockquote (>>>)",
                        "",
                        bool_row(
                            &view,
                            "multilineBlockquote",
                            "setting-multilineBlockquote",
                            model.multiline_blockquote,
                        ),
                    )),
            ))
            .child(section(
                "Compatibility",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Enable HTML support",
                        "",
                        bool_row(
                            &view,
                            "isHtmlEnabled",
                            "setting-isHtmlEnabled",
                            model.is_html_enabled,
                        ),
                    ))
                    .child(labeled(
                        "CJK emphasis compatibility",
                        "Relax emphasis boundaries for Chinese, Japanese and Korean text and bold labels ending in a fullwidth colon. This is a CommonMark syntax extension.",
                        bool_row(
                            &view,
                            "cjkEmphasis",
                            "setting-cjkEmphasis",
                            model.cjk_emphasis,
                        ),
                    ))
                    .child(labeled(
                        "Treat soft newlines as spaces",
                        "",
                        bool_row(
                            &view,
                            "softNewlineAsSpace",
                            "setting-softNewlineAsSpace",
                            model.soft_newline_as_space,
                        ),
                    )),
            ))
            .child(section(
                "Diagrams",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Mermaid theme for light MarkText themes",
                        "",
                        choice_row(
                            &view,
                            "mermaidLightTheme",
                            "setting-mermaidLightTheme",
                            MarkdownModel::mermaid_theme_choices(),
                            &model.mermaid_light_theme,
                        ),
                    ))
                    .child(labeled(
                        "Mermaid theme for dark MarkText themes",
                        "",
                        choice_row(
                            &view,
                            "mermaidDarkTheme",
                            "setting-mermaidDarkTheme",
                            MarkdownModel::mermaid_theme_choices(),
                            &model.mermaid_dark_theme,
                        ),
                    ))
                    .child(labeled(
                        "Sequence Theme",
                        "",
                        choice_row(
                            &view,
                            "sequenceTheme",
                            "setting-sequenceTheme",
                            MarkdownModel::sequence_theme_choices(),
                            &model.sequence_theme,
                        ),
                    ))
                    .child(plantuml_editor(
                        &view,
                        plantuml,
                        save_error,
                        saving,
                        invalid,
                        cx,
                    )),
            ))
            .child(section(
                "Miscellaneous",
                labeled(
                    "Prefer Heading Style",
                    "This setting is currently unavailable.",
                    disabled_choice_row(
                        "setting-preferHeadingStyle",
                        MarkdownModel::heading_style_choices(),
                        &model.prefer_heading_style,
                    ),
                ),
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
    view: &Entity<MarkdownPane>,
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

fn indent_row(view: &Entity<MarkdownPane>, selected: &str) -> impl IntoElement {
    let view = view.clone();
    let options = MarkdownModel::list_indentation_choices()
        .iter()
        .map(|choice| (choice.value, choice.label));
    select_control(
        "setting-listIndentation",
        options,
        selected,
        move |chosen, _window, app| {
            let indent = IndentValue::from_token(chosen.as_ref());
            write_back(&view, "listIndentation", indent.to_json(), app);
        },
    )
}

fn bool_row(
    view: &Entity<MarkdownPane>,
    key: &'static str,
    id: &'static str,
    checked: bool,
) -> impl IntoElement {
    let view = view.clone();
    toggle(id, checked, move |next, _window, app| {
        write_back(&view, key, Value::Bool(next), app);
    })
}

fn disabled_choice_row(id: &'static str, choices: &[Choice], selected: &str) -> impl IntoElement {
    locked_select(
        id,
        choices.iter().map(|choice| (choice.value, choice.label)),
        selected,
    )
}

fn plantuml_editor(
    view: &Entity<MarkdownPane>,
    draft: Entity<EditableTextState>,
    save_error: bool,
    saving: bool,
    invalid: bool,
    cx: &App,
) -> impl IntoElement {
    let view = view.clone();
    let draft_for_save = draft.clone();
    let message = if invalid {
        Some("Enter an http:// or https:// URL, or leave the field empty.")
    } else if save_error {
        Some("Could not save the PlantUML server URL. Please try again.")
    } else {
        None
    };
    let error = match message {
        Some(text) => pref_error(text, cx).into_any_element(),
        None => div().into_any_element(),
    };
    pref_field(
        "PlantUML Server URL",
        "",
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .on_key_down({
                        let view = view.clone();
                        let draft_for_enter = draft_for_save.clone();
                        move |event, _window, app| {
                            let plain_enter = event.keystroke.key == "enter"
                                && !event.keystroke.modifiers.modified();
                            if plain_enter {
                                submit_plantuml(&view, &draft_for_enter, app);
                            }
                        }
                    })
                    .child(
                        pref_text_input("setting-plantumlServer")
                            .state(draft.downgrade())
                            .placeholder("https://www.plantuml.com/plantuml")
                            .w(px(420.)),
                    )
                    .child(action_button(
                        "setting-plantumlServer-save",
                        if saving { "Saving…" } else { "Save" },
                        move |_event, _window, app| {
                            submit_plantuml(&view, &draft_for_save, app);
                        },
                    )),
            )
            .child(error),
    )
}

fn submit_plantuml(view: &Entity<MarkdownPane>, draft: &Entity<EditableTextState>, app: &mut App) {
    let text = draft.read(app).as_str().to_string();
    if !plantuml_url_ok(&text) {
        view.update(app, |pane, cx| {
            pane.plantuml_invalid = true;
            cx.notify();
        });
        return;
    }
    view.update(app, |pane, cx| {
        pane.plantuml_invalid = false;
        pane.request_persist(
            "plantumlServer",
            Value::String(text.clone()),
            Some(text),
            cx,
        );
    });
}

fn write_back(view: &Entity<MarkdownPane>, key: &'static str, value: Value, app: &mut App) {
    view.update(app, |pane, cx| pane.write(key, value, cx));
}
