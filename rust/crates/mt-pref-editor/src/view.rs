use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*,
};
use marktext_core::{HostBridge, MemoryHostBridge};
use marktext_ui_kit::{
    danger_button, locked_select, pref_error, pref_field, pref_note, pref_page, pref_section,
    select_control, switch_disabled, toggle,
};
use serde_json::Value;

use crate::model::{
    Choice, EditorModel, SpellcheckerHostState, TabSize, WordRemovalDecision, WordRemovalGate,
};

/// Called for every preference edit. The wasm host posts `SET_PREFERENCE`.
pub type PreferencePublisher = Rc<dyn Fn(String, Value)>;

/// Custom-dictionary removals. The host answers before the word disappears.
pub type WordRemovalPublisher = Rc<dyn Fn(u64, String)>;

pub struct EditorPane {
    bridge: Arc<MemoryHostBridge>,
    publish: PreferencePublisher,
    remove_word: WordRemovalPublisher,
    spellchecker: SpellcheckerHostState,
    gate: WordRemovalGate,
}

impl EditorPane {
    pub fn new(
        bridge: Arc<MemoryHostBridge>,
        publish: PreferencePublisher,
        remove_word: WordRemovalPublisher,
    ) -> Self {
        Self {
            bridge,
            publish,
            remove_word,
            spellchecker: SpellcheckerHostState::default(),
            gate: WordRemovalGate::new(),
        }
    }

    fn write(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.bridge.set_preference(key.to_string(), value.clone());
        (self.publish)(key.to_string(), value);
        cx.notify();
    }

    pub fn apply_spellchecker_info(
        &mut self,
        is_osx: bool,
        dictionaries: Vec<String>,
        words: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        self.spellchecker = SpellcheckerHostState {
            is_osx,
            dictionaries,
            words,
        };
        cx.notify();
    }

    pub fn accept_word_removal(&mut self, id: u64, ok: bool, cx: &mut Context<Self>) {
        if let WordRemovalDecision::Applied(pending) = self.gate.finish(id, ok) {
            self.spellchecker.words = self.spellchecker.without_word(&pending.word);
        }
        cx.notify();
    }

    fn request_word_removal(&mut self, word: &str, cx: &mut Context<Self>) {
        if let Some(id) = self.gate.request(word.to_string()) {
            (self.remove_word)(id, word.to_string());
            cx.notify();
        }
    }
}

impl Render for EditorPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = EditorModel::from_snapshot(&self.bridge.get_preferences());
        let spellchecker = self.spellchecker.clone();
        let save_error = self.gate.save_error;
        let saving = self.gate.is_busy();
        let view = cx.entity();

        pref_page("editor-pane", "Editing", cx)
            .child(section(
                "Code Block",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Show line numbers",
                        "",
                        bool_row(
                            &view,
                            "codeBlockLineNumbers",
                            "setting-codeBlockLineNumbers",
                            model.code_block_line_numbers,
                        ),
                    ))
                    .child(labeled(
                        "Remove empty lines",
                        "",
                        bool_row(
                            &view,
                            "trimUnnecessaryCodeBlockEmptyLines",
                            "setting-trimUnnecessaryCodeBlockEmptyLines",
                            model.trim_unnecessary_code_block_empty_lines,
                        ),
                    ))
                    .child(labeled(
                        "Wrap code blocks",
                        "",
                        bool_row(
                            &view,
                            "wrapCodeBlocks",
                            "setting-wrapCodeBlocks",
                            model.wrap_code_blocks,
                        ),
                    )),
            ))
            .child(section(
                "Writing preferences",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Auto close brackets",
                        "",
                        bool_row(
                            &view,
                            "autoPairBracket",
                            "setting-autoPairBracket",
                            model.auto_pair_bracket,
                        ),
                    ))
                    .child(labeled(
                        "Auto-pair Markdown markers",
                        "Complete paired Markdown markers such as asterisks, underscores and backticks as you type.",
                        bool_row(
                            &view,
                            "autoPairMarkdownSyntax",
                            "setting-autoPairMarkdownSyntax",
                            model.auto_pair_markdown_syntax,
                        ),
                    ))
                    .child(labeled(
                        "Auto close quotes",
                        "",
                        bool_row(
                            &view,
                            "autoPairQuote",
                            "setting-autoPairQuote",
                            model.auto_pair_quote,
                        ),
                    ))
                    .child(labeled(
                        "Tab key input",
                        "Choose spaces or a real tab character. Existing tabs use four-column tab stops. In lists and tables, Tab adjusts nesting or moves between cells.",
                        tab_size_row(&view, &model.tab_size),
                    ))
                    .child(labeled(
                        "Text Direction",
                        "",
                        choice_row(
                            &view,
                            "textDirection",
                            "setting-textDirection",
                            EditorModel::text_direction_choices(),
                            &model.text_direction,
                        ),
                    ))
                    .child(labeled(
                        "Link parent and child tasks",
                        "Checking a parent updates its subtasks; changing subtasks updates their parent.",
                        bool_row(&view, "autoCheck", "setting-autoCheck", model.auto_check),
                    )),
            ))
            .child(spellchecker_section(
                &view,
                &model,
                &spellchecker,
                cx,
                save_error,
                saving,
            ))
            .child(section(
                "Miscellaneous",
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(labeled(
                        "Hide quick insert hint",
                        "",
                        bool_row(
                            &view,
                            "hideQuickInsertHint",
                            "setting-hideQuickInsertHint",
                            model.hide_quick_insert_hint,
                        ),
                    ))
                    .child(labeled(
                        "Hide link popup",
                        "",
                        bool_row(
                            &view,
                            "hideLinkPopup",
                            "setting-hideLinkPopup",
                            model.hide_link_popup,
                        ),
                    ))
                    .child(labeled(
                        "Show line numbers in source code mode",
                        "",
                        bool_row(
                            &view,
                            "sourceCodeLineNumbers",
                            "setting-sourceCodeLineNumbers",
                            model.source_code_line_numbers,
                        ),
                    )),
            ))
    }
}

fn spellchecker_section(
    view: &Entity<EditorPane>,
    model: &EditorModel,
    host: &SpellcheckerHostState,
    cx: &App,
    save_error: bool,
    saving: bool,
) -> impl IntoElement {
    let enabled = model.spellchecker_enabled;
    let mut body = div()
        .flex()
        .flex_col()
        .gap_4()
        .child(labeled(
            "Enable spell checking",
            "",
            bool_row(
                view,
                "spellcheckerEnabled",
                "setting-spellcheckerEnabled",
                enabled,
            ),
        ))
        .child(labeled(
            "Hide marks for spelling errors",
            "",
            bool_row_maybe_disabled(
                view,
                "spellcheckerNoUnderline",
                "setting-spellcheckerNoUnderline",
                model.spellchecker_no_underline,
                !enabled,
            ),
        ));

    if host.is_osx {
        body = body.child(labeled(
            "Auto-detect language",
            "",
            disabled_toggle("setting-spellcheckerAutoDetectLanguage", true),
        ));
        if enabled {
            body = body.child(pref_note("Automatically detect document language", cx));
        }
    } else {
        let options = model.language_options_for(&host.dictionaries);
        body = body.child(labeled(
            "Default language",
            "",
            language_row(view, &options, &model.spellchecker_language, !enabled),
        ));
        if enabled {
            body = body.child(custom_dictionary(view, host, save_error, saving, cx));
        }
    }

    section("Spell Checker", body)
}

fn custom_dictionary(
    view: &Entity<EditorPane>,
    host: &SpellcheckerHostState,
    save_error: bool,
    saving: bool,
    cx: &App,
) -> impl IntoElement {
    let mut rows = div().flex().flex_col().gap_2();
    if host.words.is_empty() {
        rows = rows.child(pref_note("No words available", cx));
    } else {
        for word in &host.words {
            let view = view.clone();
            let word_owned = word.clone();
            rows = rows.child(
                div()
                    .id(SharedString::from(format!(
                        "setting-customDictionary-remove-{word}"
                    )))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(word.clone())
                    .child(danger_button(
                        SharedString::from(format!("setting-customDictionary-delete-{word}")),
                        if saving { "…" } else { "Delete" },
                        move |_event, _window, app| {
                            view.update(app, |pane, cx| {
                                pane.request_word_removal(&word_owned, cx);
                            });
                        },
                    )),
            );
        }
    }

    let error = if save_error {
        pref_error("Could not remove the word. Please try again.", cx).into_any_element()
    } else {
        div().into_any_element()
    };
    pref_field(
        "Custom Dictionary",
        "Add custom words to your personal dictionary",
        div()
            .id("setting-customDictionary")
            .flex()
            .flex_col()
            .gap_2()
            .child(rows)
            .child(error),
    )
}

fn section(title: &'static str, body: impl IntoElement) -> impl IntoElement {
    pref_section(title, body)
}

fn labeled(label: &'static str, note: &'static str, control: impl IntoElement) -> impl IntoElement {
    pref_field(label, note, control)
}

fn choice_row(
    view: &Entity<EditorPane>,
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

fn tab_size_row(view: &Entity<EditorPane>, selected: &TabSize) -> impl IntoElement {
    let view = view.clone();
    let current = selected.as_choice_value();
    let options = EditorModel::tab_size_choices()
        .iter()
        .map(|choice| (choice.value, choice.label));
    select_control(
        "setting-tabSize",
        options,
        current,
        move |chosen, _window, app| {
            if let Some(tab_size) = EditorModel::parse_tab_size_choice(chosen.as_ref()) {
                write_back(&view, "tabSize", tab_size.to_value(), app);
            }
        },
    )
}

fn language_row(
    view: &Entity<EditorPane>,
    options: &[(String, String)],
    selected: &str,
    disabled: bool,
) -> impl IntoElement {
    if disabled {
        return locked_select(
            "setting-spellcheckerLanguage",
            options
                .iter()
                .map(|(value, label)| (value.clone(), label.clone())),
            selected,
        )
        .into_any_element();
    }
    let view = view.clone();
    select_control(
        "setting-spellcheckerLanguage",
        options
            .iter()
            .map(|(value, label)| (value.clone(), label.clone())),
        selected,
        move |chosen, _window, app| {
            write_back(
                &view,
                "spellcheckerLanguage",
                Value::String(chosen.to_string()),
                app,
            );
        },
    )
    .into_any_element()
}

fn bool_row(
    view: &Entity<EditorPane>,
    key: &'static str,
    id: &'static str,
    checked: bool,
) -> impl IntoElement {
    bool_row_maybe_disabled(view, key, id, checked, false)
}

fn bool_row_maybe_disabled(
    view: &Entity<EditorPane>,
    key: &'static str,
    id: &'static str,
    checked: bool,
    disabled: bool,
) -> impl IntoElement {
    let view = view.clone();
    switch_disabled(
        toggle(id, checked, move |next, _window, app| {
            if disabled {
                return;
            }
            write_back(&view, key, Value::Bool(next), app);
        }),
        disabled,
    )
    .into_any_element()
}

fn disabled_toggle(id: &'static str, checked: bool) -> impl IntoElement {
    switch_disabled(toggle(id, checked, |_next, _window, _app| {}), true)
}

fn write_back(view: &Entity<EditorPane>, key: &'static str, value: Value, app: &mut App) {
    view.update(app, |pane, cx| pane.write(key, value, cx));
}
