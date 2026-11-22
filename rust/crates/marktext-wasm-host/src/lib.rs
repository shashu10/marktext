//! Single-canvas router. The Electron iframe posts `HostToGuest` messages and
//! this view answers with `GuestToHost`. Pane crates replace `HostView::render`
//! once they own a real preferences screen.

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, rgb};
use marktext_core::{GuestToHost, HostToGuest, PaneRouter};
use marktext_ui_kit::setting_row;

pub struct HostView {
    router: PaneRouter,
}

impl HostView {
    pub fn new(initial_pane: impl Into<String>) -> Self {
        Self {
            router: PaneRouter::new(initial_pane),
        }
    }

    pub fn active_pane(&self) -> &str {
        self.router.active()
    }

    pub fn apply(&mut self, message: &HostToGuest) -> Option<GuestToHost> {
        self.router.handle(message)
    }
}

impl Render for HostView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.router.active().to_string();
        div()
            .font_family("Liberation Sans")
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .bg(rgb(0x141416))
            .child(setting_row(
                "Active pane",
                div().text_color(rgb(0xf4f4f5)).child(pane),
            ))
    }
}
