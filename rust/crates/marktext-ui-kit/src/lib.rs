//! Shared GPUI controls. Pane crates compose these instead of restyling raw
//! divs, so a later visual pass can change every settings row in one place.
//!
//! Controls are elements, not entities. The pane view owns the value and passes
//! it back in on the next frame after `on_change`.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, prelude::*, px, rgb,
};
use gpui_elements::editable_text::text_input;
use gpui_elements::switch::BaseSwitch;

/// Family name in the Liberation Sans name table. The web canvas has no
/// `.SystemUIFont`, so every control names this face explicitly.
pub const UI_FONT: &str = "Liberation Sans";

const TRACK: u32 = 0x2A_2A_2E;
const INK: u32 = 0xF4_F4_F5;
const ACCENT: u32 = 0x4C_9A_FF;
const MUTED: u32 = 0xA1_A1_AA;

pub fn setting_row(label: impl Into<SharedString>, control: impl IntoElement) -> impl IntoElement {
    div()
        .font_family(UI_FONT)
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_2()
        .child(div().text_color(rgb(INK)).child(label.into()))
        .child(control)
}

pub fn toggle(
    id: impl Into<gpui::ElementId>,
    checked: bool,
    on_change: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> BaseSwitch {
    BaseSwitch::new(id)
        .checked(checked)
        .on_change(move |next, window, app| on_change(*next, window, app))
        .with_state(|row, checked, _disabled| {
            row.font_family(UI_FONT)
                .px_3()
                .py_1()
                .rounded_md()
                .bg(rgb(if checked { ACCENT } else { TRACK }))
                .text_color(rgb(INK))
                .child(if checked { "On" } else { "Off" })
        })
}

pub fn text_field(
    id: impl Into<gpui::ElementId>,
    placeholder: impl Into<SharedString>,
) -> impl IntoElement {
    text_input(id)
        .placeholder(placeholder)
        .font_family(UI_FONT)
        .w(px(280.))
        .min_h(px(32.))
        .px_2()
        .bg(rgb(TRACK))
        .text_color(rgb(INK))
}

pub fn select_control(
    id_prefix: impl Into<SharedString>,
    options: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    selected: impl Into<SharedString>,
    on_change: impl Fn(SharedString, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id_prefix = id_prefix.into();
    let selected = selected.into();
    let on_change = Rc::new(on_change);
    let mut row = div()
        .id(id_prefix.clone())
        .font_family(UI_FONT)
        .flex()
        .gap_2();
    for (index, (value, label)) in options.into_iter().enumerate() {
        let value = value.into();
        let label = label.into();
        let active = value == selected;
        let choose = Rc::clone(&on_change);
        let choice = value.clone();
        row = row.child(
            div()
                .id(SharedString::from(format!("{id_prefix}-{index}")))
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(if active { ACCENT } else { TRACK }))
                .text_color(rgb(INK))
                .cursor_pointer()
                .child(label)
                .on_click(move |_event: &ClickEvent, window, app| {
                    choose(choice.clone(), window, app);
                }),
        );
    }
    row
}

pub fn slider_control(
    id: impl Into<SharedString>,
    value: f64,
    min: f64,
    max: f64,
    on_change: impl Fn(f64, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id = id.into();
    let span = (max - min).abs().max(f64::EPSILON);
    let step = span / 20.0;
    let on_change = Rc::new(on_change);
    let decrease = Rc::clone(&on_change);
    let increase = Rc::clone(&on_change);
    div()
        .id(id.clone())
        .font_family(UI_FONT)
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .id(SharedString::from(format!("{id}-dec")))
                .px_2()
                .cursor_pointer()
                .child("−")
                .on_click(move |_event, window, app| {
                    decrease((value - step).clamp(min, max), window, app);
                }),
        )
        .child(
            div()
                .min_w(px(48.))
                .text_color(rgb(INK))
                .child(format!("{value:.2}")),
        )
        .child(
            div()
                .id(SharedString::from(format!("{id}-inc")))
                .px_2()
                .cursor_pointer()
                .child("+")
                .on_click(move |_event, window, app| {
                    increase((value + step).clamp(min, max), window, app);
                }),
        )
}

pub struct TreeNode {
    pub id: SharedString,
    pub label: SharedString,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
}

pub fn tree(
    nodes: &[TreeNode],
    on_toggle: impl Fn(&str, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    render_nodes(nodes, 0, Rc::new(on_toggle))
}

type ToggleHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

fn render_nodes(nodes: &[TreeNode], depth: usize, on_toggle: ToggleHandler) -> gpui::Div {
    let mut column = div().font_family(UI_FONT).flex().flex_col().gap_1();
    for node in nodes {
        let toggle = Rc::clone(&on_toggle);
        let id = node.id.clone();
        column = column.child(
            div()
                .id(id.clone())
                .pl(px((depth as f32) * 12.0))
                .cursor_pointer()
                .text_color(rgb(INK))
                .child(format!(
                    "{} {}",
                    if node.children.is_empty() {
                        "•"
                    } else if node.expanded {
                        "▾"
                    } else {
                        "▸"
                    },
                    node.label
                ))
                .on_click(move |_event, window, app| toggle(id.as_ref(), window, app)),
        );
        if node.expanded && !node.children.is_empty() {
            column = column.child(render_nodes(
                &node.children,
                depth + 1,
                Rc::clone(&on_toggle),
            ));
        }
    }
    column
}

pub fn modal(title: impl Into<SharedString>, body: impl IntoElement) -> impl IntoElement {
    div()
        .font_family(UI_FONT)
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(0x000000))
        .child(
            div()
                .min_w(px(320.))
                .p_4()
                .rounded_lg()
                .bg(rgb(0x18_18_1B))
                .text_color(rgb(INK))
                .flex()
                .flex_col()
                .gap_3()
                .child(div().text_lg().text_color(rgb(INK)).child(title.into()))
                .child(div().text_color(rgb(MUTED)).child(body)),
        )
}

/// Placeholder canvas used until a pane crate registers its own view.
pub struct PanePlaceholder {
    pub pane: SharedString,
}

impl Render for PanePlaceholder {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .font_family(UI_FONT)
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_6()
            .bg(rgb(0x141416))
            .text_color(rgb(INK))
            .child(div().text_xl().child(marktext_core_label(&self.pane)))
            .child(setting_row(
                "GPUI host",
                div().text_color(rgb(MUTED)).child("pane crate not mounted"),
            ))
    }
}

fn marktext_core_label(pane: &str) -> String {
    format!("{pane} preferences")
}
