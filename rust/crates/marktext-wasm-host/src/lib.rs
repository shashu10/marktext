//! Single-canvas router. The Electron iframe posts `HostToGuest` messages and
//! this view answers with `GuestToHost`. Preference panes mount here as their
//! crates land; unmigrated panes still use the placeholder.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, rgb};
use marktext_core::{GuestSession, GuestToHost, HostBridge, HostToGuest};
use marktext_ui_kit::setting_row;
use mt_pref_files::FilesPane;
use mt_pref_general::GeneralPane;
use serde_json::Value;

pub struct HostView {
    session: GuestSession,
    general: gpui::Entity<GeneralPane>,
    files: gpui::Entity<FilesPane>,
}

impl HostView {
    pub fn new(initial_pane: impl Into<String>, cx: &mut Context<Self>) -> Self {
        let session = GuestSession::new(initial_pane);
        let bridge = session.bridge_handle();
        let general = cx.new(|_cx| {
            GeneralPane::new(
                Arc::clone(&bridge),
                Rc::new(publish_preference),
                Rc::new(publish_persist),
            )
        });
        let files = cx.new(|_cx| {
            FilesPane::new(
                bridge,
                Rc::new(publish_preference),
                Rc::new(publish_persist),
                Rc::new(publish_choose_directory),
            )
        });
        Self {
            session,
            general,
            files,
        }
    }

    pub fn active_pane(&self) -> &str {
        self.session.active_pane()
    }

    pub fn apply(&mut self, message: &HostToGuest, cx: &mut Context<Self>) -> Option<GuestToHost> {
        match message {
            HostToGuest::PreferenceResult { id, status } => {
                let persisted = status == "persisted";
                let request_id = *id;
                self.general.update(cx, |pane, cx| {
                    pane.accept_persist_result(request_id, persisted, cx);
                });
                self.files.update(cx, |pane, cx| {
                    pane.accept_persist_result(request_id, persisted, cx);
                });
                return None;
            }
            HostToGuest::DirectoryChosen { id, path } => {
                let request_id = *id;
                let chosen = path.clone();
                self.files.update(cx, |pane, cx| {
                    pane.accept_directory(request_id, chosen, cx);
                });
                return None;
            }
            _ => {}
        }
        let reply = self.session.apply(message);
        self.general.update(cx, |_pane, cx| cx.notify());
        self.files.update(cx, |_pane, cx| cx.notify());
        reply
    }
}

impl Render for HostView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // The window sizes this root. A child view's `size_full` is a percentage of it.
        let content: AnyElement = match self.session.active_pane() {
            "general" => self.general.clone().into_any_element(),
            "files" => self.files.clone().into_any_element(),
            _ => placeholder(self).into_any_element(),
        };
        div().size_full().child(content)
    }
}

fn placeholder(view: &HostView) -> impl IntoElement {
    let pane = view.session.active_pane().to_string();
    let theme = view
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

/// Stage 1 transport for a pane write. Native builds keep the value in the bridge only.
pub fn publish_persist(id: u64, key: String, value: Value) {
    #[cfg(target_family = "wasm")]
    post_to_parent(&GuestToHost::PersistPreference { id, key, value });
    #[cfg(not(target_family = "wasm"))]
    {
        let _ = (id, key, value);
    }
}

pub fn publish_preference(key: String, value: Value) {
    #[cfg(target_family = "wasm")]
    post_to_parent(&GuestToHost::SetPreference { key, value });
    #[cfg(not(target_family = "wasm"))]
    {
        let _ = (key, value);
    }
}

pub fn publish_choose_directory(id: u64) {
    #[cfg(target_family = "wasm")]
    post_to_parent(&GuestToHost::ChooseDirectory { id });
    #[cfg(not(target_family = "wasm"))]
    {
        let _ = id;
    }
}

#[cfg(target_family = "wasm")]
pub fn post_to_parent(reply: &GuestToHost) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(parent)) = window.parent() else {
        return;
    };
    let Ok(value) = serde_wasm_bindgen::to_value(reply) else {
        return;
    };
    let _ = parent.post_message(&value, "*");
}
