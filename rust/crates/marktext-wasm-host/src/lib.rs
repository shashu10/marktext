//! Single-canvas router. The Electron iframe posts `HostToGuest` messages and
//! this view answers with `GuestToHost`. Pane crates replace `HostView::render`
//! once they own a real preferences screen.

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, rgb};
use marktext_core::{GuestSession, GuestToHost, HostBridge, HostToGuest};
use marktext_ui_kit::setting_row;

pub struct HostView {
    session: GuestSession,
}

impl HostView {
    pub fn new(initial_pane: impl Into<String>) -> Self {
        Self {
            session: GuestSession::new(initial_pane),
        }
    }

    pub fn active_pane(&self) -> &str {
        self.session.active_pane()
    }

    pub fn apply(&mut self, message: &HostToGuest) -> Option<GuestToHost> {
        self.session.apply(message)
    }
}

impl Render for HostView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.session.active_pane().to_string();
        let theme = self
            .session
            .bridge()
            .get_preferences()
            .text("theme")
            .unwrap_or("")
            .to_string();
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
            .child(setting_row(
                "Theme",
                div().text_color(rgb(0xf4f4f5)).child(theme),
            ))
    }
}
