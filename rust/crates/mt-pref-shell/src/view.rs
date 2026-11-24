use std::rc::Rc;

use gpui::{
    Context, Entity, FocusHandle, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, prelude::*, px, rgb,
};
use gpui_elements::editable_text::{EditableTextState, text_input};
use marktext_core::KeybindingEntry;
use marktext_ui_kit::UI_FONT;
use serde_json::Value;

use crate::categories::PREFERENCE_CATEGORIES;
use crate::model::{
    NAVIGATE_PREFERENCE_ACTION, NavigatePreference, PrefShellModel, SEARCH_PLACEHOLDER,
    SIDEBAR_TITLE,
};
use crate::search::SearchEntry;

/// Posts `HOST_ACTION` with `navigate-preference` so Vue can `router.push`.
pub type HostActionPublisher = Rc<dyn Fn(u64, String, Value)>;

/// Asks the host to (re)load keybindings for search.
pub type KeybindingsRequester = Rc<dyn Fn()>;

const PAGE: u32 = 0xF5F5F7;
const INK: u32 = 0x1C1C1F;
const MUTED: u32 = 0x5C5C66;
const ACTIVE: u32 = 0x111113;
const HOVER: u32 = 0xE8E8EC;
const ACCENT: u32 = 0x4C9AFF;
const CHIP: u32 = 0xFFFFFF;

pub struct PrefShellPane {
    model: PrefShellModel,
    publish_action: HostActionPublisher,
    request_keybindings: KeybindingsRequester,
    next_action_id: u64,
    query_field: Option<Entity<EditableTextState>>,
    last_query_snapshot: String,
    /// Tab stops for the seven category buttons (after the search field).
    category_focus: Vec<FocusHandle>,
}

impl PrefShellPane {
    pub fn new(
        publish_action: HostActionPublisher,
        request_keybindings: KeybindingsRequester,
        is_mac: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let category_focus = PREFERENCE_CATEGORIES
            .iter()
            .enumerate()
            .map(|(index, _)| {
                // Search field owns tab index 0 via EditableTextState.
                cx.focus_handle()
                    .tab_stop(true)
                    .tab_index((index as isize) + 1)
            })
            .collect();
        Self {
            model: PrefShellModel::new(is_mac),
            publish_action,
            request_keybindings,
            next_action_id: 1,
            query_field: None,
            last_query_snapshot: String::new(),
            category_focus,
        }
    }

    pub fn set_active_category(&mut self, category: &str, cx: &mut Context<Self>) {
        self.model.set_active_category(category);
        cx.notify();
    }

    pub fn apply_keybindings(
        &mut self,
        commands: Vec<KeybindingEntry>,
        is_mac: bool,
        loading: bool,
        cx: &mut Context<Self>,
    ) {
        self.model.apply_keybindings(commands, is_mac, loading);
        cx.notify();
    }

    pub fn request_keybindings_load(&self) {
        (self.request_keybindings)();
    }

    fn navigate(&mut self, target: NavigatePreference, cx: &mut Context<Self>) {
        if target.key.is_none() {
            self.model.set_active_category(&target.category);
        }
        let id = self.next_action_id;
        self.next_action_id = self.next_action_id.saturating_add(1);
        (self.publish_action)(id, NAVIGATE_PREFERENCE_ACTION.to_string(), target.to_payload());
        cx.notify();
    }

    fn sync_query_from_field(&mut self, cx: &mut Context<Self>) {
        let Some(field) = self.query_field.clone() else {
            return;
        };
        let current = field.read(cx).as_str().to_string();
        if current == self.last_query_snapshot {
            return;
        }
        self.last_query_snapshot = current.clone();
        self.model.set_query(current);
    }

    fn clear_query(&mut self, cx: &mut Context<Self>) {
        self.model.set_query(String::new());
        self.last_query_snapshot.clear();
        if let Some(field) = self.query_field.clone() {
            field.update(cx, |state, cx| state.emplace("", cx));
        }
    }
}

impl Render for PrefShellPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query_field =
            EditableTextState::use_keyed("pref-shell-search-query", window, cx);
        self.query_field = Some(query_field.clone());
        self.sync_query_from_field(cx);

        let view = cx.entity();
        let active = self.model.active_category.clone();
        let suggestions = self.model.suggestions.clone();
        let category_focus = self.category_focus.clone();

        div()
            .id("pref-shell-sidebar")
            .font_family(UI_FONT)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(PAGE))
            .text_color(rgb(INK))
            .pt(px(24.))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_xl()
                    .child(SIDEBAR_TITLE),
            )
            .child(search_block(&view, query_field, &suggestions))
            .child(category_list(&view, &active, &category_focus))
    }
}

fn search_block(
    view: &Entity<PrefShellPane>,
    query_field: Entity<EditableTextState>,
    suggestions: &[SearchEntry],
) -> impl IntoElement {
    let view_for_input = view.clone();
    let mut column = div()
        .id("pref-shell-search")
        .flex()
        .flex_col()
        .gap_2()
        .px(px(16.))
        .py(px(18.))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .min_h(px(32.))
                .rounded_md()
                .bg(rgb(CHIP))
                .border_1()
                .border_color(rgb(HOVER))
                .on_key_down({
                    let view = view_for_input.clone();
                    move |event, _window, app| {
                        let plain_enter =
                            event.keystroke.key == "enter" && !event.keystroke.modifiers.modified();
                        if plain_enter {
                            view.update(app, |pane, cx| {
                                pane.sync_query_from_field(cx);
                                if let Some(entry) = pane.model.suggestions.first().cloned() {
                                    let target = pane.model.navigate_for_entry(&entry);
                                    pane.clear_query(cx);
                                    pane.navigate(target, cx);
                                }
                            });
                        }
                    }
                })
                .child(
                    text_input("pref-shell-search-input")
                        .state(query_field.downgrade())
                        .placeholder(SEARCH_PLACEHOLDER)
                        .font_family(UI_FONT)
                        .w_full()
                        .min_h(px(28.))
                        .text_color(rgb(INK)),
                ),
        );

    if !suggestions.is_empty() {
        let mut list = div()
            .id("pref-shell-suggestions")
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(280.))
            .overflow_y_scroll()
            .rounded_md()
            .border_1()
            .border_color(rgb(HOVER))
            .bg(rgb(CHIP))
            .p_1();
        for (index, entry) in suggestions.iter().enumerate() {
            list = list.child(suggestion_row(view, entry, index));
        }
        column = column.child(list);
    }
    column
}

fn suggestion_row(
    view: &Entity<PrefShellPane>,
    entry: &SearchEntry,
    index: usize,
) -> impl IntoElement {
    let view = view.clone();
    let entry = entry.clone();
    let accelerator = entry
        .accelerator
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(|value| format!(" · {value}"))
        .unwrap_or_default();
    let title = format!("{}{accelerator}", entry.category);
    let subtitle = entry.preference.clone();
    div()
        .id(SharedString::from(format!("pref-shell-suggestion-{index}")))
        .flex()
        .flex_col()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .hover(|style| style.bg(rgb(HOVER)))
        .on_click(move |_event, _window, app| {
            let entry = entry.clone();
            view.update(app, |pane, cx| {
                let target = pane.model.navigate_for_entry(&entry);
                pane.clear_query(cx);
                pane.navigate(target, cx);
            });
        })
        .child(div().text_sm().text_color(rgb(INK)).child(title))
        .child(div().text_xs().text_color(rgb(MUTED)).child(subtitle))
}

fn category_list(
    view: &Entity<PrefShellPane>,
    active: &str,
    category_focus: &[FocusHandle],
) -> impl IntoElement {
    let mut list = div()
        .id("pref-shell-categories")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scroll();
    for (index, category) in PREFERENCE_CATEGORIES.iter().enumerate() {
        let is_active = category.id == active;
        let view = view.clone();
        let category_id = category.id;
        let focus = category_focus
            .get(index)
            .cloned()
            .expect("one focus handle per category");
        list = list.child(
            div()
                .id(SharedString::from(format!("pref-shell-category-{category_id}")))
                .track_focus(&focus)
                .focus_visible(|style| style.border_color(rgb(ACCENT)))
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(38.))
                .pl(px(16.))
                .border_1()
                .border_color(rgb(PAGE))
                .cursor_pointer()
                .text_color(rgb(if is_active { ACTIVE } else { INK }))
                .when(is_active, |style| style.bg(rgb(HOVER)))
                .hover(|style| style.bg(rgb(HOVER)))
                .on_click({
                    let view = view.clone();
                    move |_event, _window, app| {
                        view.update(app, |pane, cx| {
                            if let Some(target) = pane.model.navigate_for_category(category_id) {
                                pane.navigate(target, cx);
                            }
                        });
                    }
                })
                .on_key_down({
                    let view = view.clone();
                    move |event, _window, app| {
                        let plain = !event.keystroke.modifiers.modified();
                        let key = event.keystroke.key.as_str();
                        if plain && (key == "enter" || key == "space") {
                            view.update(app, |pane, cx| {
                                if let Some(target) = pane.model.navigate_for_category(category_id)
                                {
                                    pane.navigate(target, cx);
                                }
                            });
                        }
                    }
                })
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(px(4.))
                        .h_full()
                        .rounded_r_md()
                        .bg(rgb(if is_active { ACCENT } else { PAGE })),
                )
                .child(
                    div()
                        .w(px(18.))
                        .h(px(18.))
                        .mr(px(12.))
                        .rounded_md()
                        .bg(rgb(if is_active { ACCENT } else { HOVER }))
                        .text_color(rgb(if is_active { CHIP } else { MUTED }))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .child(category.glyph),
                )
                .child(div().child(category.name)),
        );
    }
    list
}
