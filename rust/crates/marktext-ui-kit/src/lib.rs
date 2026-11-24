//! Shared GPUI controls. Pane crates compose these instead of restyling raw
//! divs, so a later visual pass can change every settings row in one place.
//!
//! Controls are elements, not entities. The pane view owns the value and passes
//! it back in on the next frame after `on_change`.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, Context, ElementId, IntoElement, ParentElement, Render, SharedString, Styled,
    Window, div, prelude::*, px, rgb,
};
use gpui_components::button::{Button, ButtonVariants};
use gpui_components::group_box::{GroupBox, GroupBoxVariants};
use gpui_components::label::Label;
use gpui_components::menu::{DropdownMenu, PopupMenuItem};
use gpui_components::switch::Switch;
use gpui_components::{ActiveTheme, Disableable, Sizable, v_flex};
use gpui_elements::editable_text::{EditableTextElement, text_area, text_input};

/// Loads the component theme. Call once at startup, before the first control paints.
pub fn init(cx: &mut App) {
    gpui_components::init(cx);
}

/// Family name in the Liberation Sans name table. The web canvas has no
/// `.SystemUIFont`, so every control names this face explicitly.
pub const UI_FONT: &str = "Liberation Sans";

const INK: u32 = 0xF4_F4_F5;
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
) -> Switch {
    Switch::new(id)
        .checked(checked)
        .on_change(move |next, window, app| on_change(*next, window, app))
}

pub fn choice_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    let button = if selected {
        Button::new(id).primary()
    } else {
        Button::new(id).secondary()
    };
    button
        .small()
        .font_family(UI_FONT)
        .label(label)
        .on_click(on_click)
}

pub fn action_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .small()
        .font_family(UI_FONT)
        .label(label)
        .primary()
        .on_click(on_click)
}

pub fn button_disabled(button: Button, disabled: bool) -> Button {
    button.disabled(disabled)
}

pub fn switch_disabled(switch: Switch, disabled: bool) -> Switch {
    switch.disabled(disabled)
}

pub fn danger_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .small()
        .font_family(UI_FONT)
        .label(label)
        .danger()
        .on_click(on_click)
}

/// Scrollable settings page. The title and background come from the kit theme.
pub fn pref_page(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    cx: &App,
) -> impl IntoElement + ParentElement {
    div()
        .id(id)
        .font_family(UI_FONT)
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_4()
        .p_4()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(Label::new(title).text_xl().font_family(UI_FONT))
}

pub fn pref_section(title: impl Into<SharedString>, body: impl IntoElement) -> GroupBox {
    let title = title.into();
    // Every group on one page needs its own element id. The kit default is shared.
    GroupBox::new()
        .id(SharedString::from(format!("section-{title}")))
        .outline()
        .title(Label::new(title.clone()).font_family(UI_FONT))
        .child(body)
}

pub fn pref_field(
    label: impl Into<SharedString>,
    note: &str,
    control: impl IntoElement,
) -> impl IntoElement {
    let heading = if note.is_empty() {
        Label::new(label).font_family(UI_FONT).into_any_element()
    } else {
        Label::new(label)
            .font_family(UI_FONT)
            .secondary(note)
            .into_any_element()
    };
    v_flex().gap_2().w_full().child(heading).child(control)
}

pub fn pref_note(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .text_sm()
        .font_family(UI_FONT)
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

pub fn pref_error(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .text_sm()
        .font_family(UI_FONT)
        .text_color(cx.theme().danger)
        .child(text.into())
}

pub fn pref_text_input(id: impl Into<ElementId>) -> EditableTextElement {
    text_input(id)
        .font_family(UI_FONT)
        .min_h(px(36.))
        .px_3()
        .rounded_md()
        .border_1()
        .border_color(rgb(0xE4_E4_E7))
        .bg(rgb(0xFF_FF_FF))
        .text_color(rgb(0x18_18_1B))
}

pub fn pref_text_area(id: impl Into<ElementId>) -> EditableTextElement {
    text_area(id)
        .font_family(UI_FONT)
        .w_full()
        .min_h(px(120.))
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(0xE4_E4_E7))
        .bg(rgb(0xFF_FF_FF))
        .text_color(rgb(0x18_18_1B))
}

pub fn text_field(
    id: impl Into<gpui::ElementId>,
    placeholder: impl Into<SharedString>,
) -> impl IntoElement {
    pref_text_input(id).placeholder(placeholder).w(px(280.))
}

/// Closed dropdown for a legacy `el-select`. The button shows the selected
/// label. Choosing a menu item calls `on_change` with that option's value.
pub fn select_control(
    id_prefix: impl Into<SharedString>,
    options: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    selected: impl Into<SharedString>,
    on_change: impl Fn(SharedString, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    dropdown_select(id_prefix, options, selected, false, on_change)
}

/// Same choices as [`select_control`], drawn disabled. The menu does not open.
pub fn locked_select(
    id_prefix: impl Into<SharedString>,
    options: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    selected: impl Into<SharedString>,
) -> impl IntoElement {
    dropdown_select(id_prefix, options, selected, true, |_, _, _| {})
}

fn dropdown_select(
    id_prefix: impl Into<SharedString>,
    options: impl IntoIterator<Item = (impl Into<SharedString>, impl Into<SharedString>)>,
    selected: impl Into<SharedString>,
    disabled: bool,
    on_change: impl Fn(SharedString, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id = id_prefix.into();
    let selected = selected.into();
    let options: Vec<(SharedString, SharedString)> = options
        .into_iter()
        .map(|(value, label)| (value.into(), label.into()))
        .collect();
    let current = options
        .iter()
        .find(|(value, _)| value == &selected)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| selected.clone());
    let on_change = Rc::new(on_change);
    let mut button = Button::new(id)
        .outline()
        .small()
        .font_family(UI_FONT)
        .min_w(px(180.))
        .label(current)
        .dropdown_caret(true);
    if disabled {
        button = button.disabled(true);
    }
    let menu_options = options;
    let menu_selected = selected;
    button.dropdown_menu(move |menu, _, _| {
        let on_change = Rc::clone(&on_change);
        let mut menu = menu.scrollable(true).max_h(px(280.)).min_w(px(180.));
        for (value, label) in &menu_options {
            let choose = Rc::clone(&on_change);
            let value = value.clone();
            menu = menu.item(
                PopupMenuItem::new(label.clone())
                    .checked(value == menu_selected)
                    .on_click(move |_event, window, app| {
                        choose(value.clone(), window, app);
                    }),
            );
        }
        menu
    })
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
        .child(action_button(
            SharedString::from(format!("{id}-dec")),
            "−",
            move |_event, window, app| {
                decrease((value - step).clamp(min, max), window, app);
            },
        ))
        .child(
            div()
                .min_w(px(48.))
                .text_color(rgb(INK))
                .child(format!("{value:.2}")),
        )
        .child(action_button(
            SharedString::from(format!("{id}-inc")),
            "+",
            move |_event, window, app| {
                increase((value + step).clamp(min, max), window, app);
            },
        ))
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

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{MouseButton, Render, TestApp, point, px};

    use super::*;

    struct Harness {
        chosen: Rc<RefCell<String>>,
        disabled: bool,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let chosen = Rc::clone(&self.chosen);
            let selected = chosen.borrow().clone();
            div().size_full().child(if self.disabled {
                locked_select("probe", [("a", "Alpha"), ("b", "Beta")], selected).into_any_element()
            } else {
                select_control(
                    "probe",
                    [("a", "Alpha"), ("b", "Beta")],
                    selected,
                    move |value, _, _| {
                        *chosen.borrow_mut() = value.to_string();
                    },
                )
                .into_any_element()
            })
        }
    }

    fn open(disabled: bool) -> (TestApp, gpui::TestAppWindow<Harness>, Rc<RefCell<String>>) {
        let mut app = TestApp::new();
        app.update(init);
        let chosen = Rc::new(RefCell::new("a".to_string()));
        let chosen_for_view = Rc::clone(&chosen);
        let window = app.open_window(move |_window, _cx| Harness {
            chosen: chosen_for_view,
            disabled,
        });
        (app, window, chosen)
    }

    #[test]
    fn choosing_a_menu_item_reports_its_value() {
        let (_app, mut window, chosen) = open(false);
        window.draw();
        window.simulate_click(point(px(24.), px(12.)), MouseButton::Left);
        window.draw();
        window.simulate_click(point(px(40.), px(80.)), MouseButton::Left);
        assert_eq!(chosen.borrow().as_str(), "b");
    }

    #[test]
    fn a_locked_select_ignores_the_menu() {
        let (_app, mut window, chosen) = open(true);
        window.draw();
        window.simulate_click(point(px(24.), px(12.)), MouseButton::Left);
        window.draw();
        window.simulate_click(point(px(40.), px(80.)), MouseButton::Left);
        assert_eq!(chosen.borrow().as_str(), "a");
    }
}
