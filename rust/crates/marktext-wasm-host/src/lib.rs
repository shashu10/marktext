//! Single-canvas router. The Electron iframe posts `HostToGuest` messages and
//! this view answers with `GuestToHost`. Preference panes mount here as their
//! crates land; unmigrated panes still use the placeholder.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, rgb};
use marktext_core::{GuestSession, GuestToHost, HostBridge, HostToGuest};
use marktext_ui_kit::setting_row;
use mt_pref_appearance::AppearancePane;
use mt_pref_editor::EditorPane;
use mt_pref_files::FilesPane;
use mt_pref_general::GeneralPane;
use mt_pref_keybindings::{KeybindingHostRequest, KeybindingsPane};
use mt_pref_markdown::MarkdownPane;
use serde_json::Value;

/// Each pane numbers its own saves from 1. Appearance ids are shifted so a
/// result for one pane cannot complete another pane's in-flight save.
const APPEARANCE_SAVE_ID_BASE: u64 = 1 << 32;

pub struct HostView {
    session: GuestSession,
    general: gpui::Entity<GeneralPane>,
    appearance: gpui::Entity<AppearancePane>,
    editor: gpui::Entity<EditorPane>,
    files: gpui::Entity<FilesPane>,
    markdown: gpui::Entity<MarkdownPane>,
    keybindings: gpui::Entity<KeybindingsPane>,
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
        let appearance = cx.new(|_cx| {
            AppearancePane::new(
                Arc::clone(&bridge),
                Rc::new(publish_preference),
                Rc::new(publish_appearance_persist),
            )
        });
        let editor = cx.new(|_cx| {
            EditorPane::new(
                Arc::clone(&bridge),
                Rc::new(publish_preference),
                Rc::new(publish_word_removal),
            )
        });
        let files = cx.new(|_cx| {
            FilesPane::new(
                Arc::clone(&bridge),
                Rc::new(publish_preference),
                Rc::new(publish_persist),
                Rc::new(publish_choose_directory),
            )
        });
        let markdown = cx.new(|_cx| {
            MarkdownPane::new(
                bridge,
                Rc::new(publish_preference),
                Rc::new(publish_persist),
            )
        });
        let keybindings = cx.new(|cx| KeybindingsPane::new(Rc::new(publish_keybinding), cx));
        Self {
            session,
            general,
            appearance,
            editor,
            files,
            markdown,
            keybindings,
        }
    }

    pub fn active_pane(&self) -> &str {
        self.session.active_pane()
    }

    pub fn apply(&mut self, message: &HostToGuest, cx: &mut Context<Self>) -> Option<GuestToHost> {
        match message {
            HostToGuest::PreferenceResult { id, status } => {
                let persisted = status == "persisted";
                let (pane, request_id) = save_result_target(*id);
                if pane == "appearance" {
                    self.appearance.update(cx, |pane, cx| {
                        pane.accept_persist_result(request_id, persisted, cx);
                    });
                } else {
                    self.general.update(cx, |pane, cx| {
                        pane.accept_persist_result(request_id, persisted, cx);
                    });
                    self.files.update(cx, |pane, cx| {
                        pane.accept_persist_result(request_id, persisted, cx);
                    });
                    self.markdown.update(cx, |pane, cx| {
                        pane.accept_persist_result(request_id, persisted, cx);
                    });
                }
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
            HostToGuest::SpellcheckerInfo {
                is_osx,
                dictionaries,
                words,
            } => {
                let is_osx = *is_osx;
                let dictionaries = dictionaries.clone();
                let words = words.clone();
                self.editor.update(cx, |pane, cx| {
                    pane.apply_spellchecker_info(is_osx, dictionaries, words, cx);
                });
                return None;
            }
            HostToGuest::SpellcheckerRemoveResult { id, ok } => {
                let request_id = *id;
                let ok = *ok;
                self.editor.update(cx, |pane, cx| {
                    pane.accept_word_removal(request_id, ok, cx);
                });
                return None;
            }
            HostToGuest::Keybindings {
                commands,
                is_mac,
                show_debug,
                loading,
                load_error,
            } => {
                self.keybindings.update(cx, |pane, cx| {
                    pane.apply_snapshot(
                        commands,
                        *is_mac,
                        *show_debug,
                        *loading,
                        *load_error,
                        cx,
                    );
                });
                return None;
            }
            HostToGuest::KeybindingResult {
                id,
                status,
                commands,
                conflict_accelerator,
                conflict_command_ids,
            } => {
                self.keybindings.update(cx, |pane, cx| {
                    pane.accept_result(
                        *id,
                        status,
                        commands.clone(),
                        conflict_accelerator.clone(),
                        conflict_command_ids.clone(),
                        cx,
                    );
                });
                return None;
            }
            _ => {}
        }
        let reply = self.session.apply(message);
        self.general.update(cx, |_pane, cx| cx.notify());
        self.appearance.update(cx, |_pane, cx| cx.notify());
        self.editor.update(cx, |_pane, cx| cx.notify());
        self.files.update(cx, |_pane, cx| cx.notify());
        self.markdown.update(cx, |_pane, cx| cx.notify());
        self.keybindings.update(cx, |_pane, cx| cx.notify());
        reply
    }
}

impl Render for HostView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // The window sizes this root. A child view's `size_full` is a percentage of it.
        let content: AnyElement = match self.session.active_pane() {
            "general" => self.general.clone().into_any_element(),
            "appearance" => self.appearance.clone().into_any_element(),
            "editor" => self.editor.clone().into_any_element(),
            "files" => self.files.clone().into_any_element(),
            "markdown" => self.markdown.clone().into_any_element(),
            "keybindings" => self.keybindings.clone().into_any_element(),
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

fn save_result_target(id: u64) -> (&'static str, u64) {
    if id >= APPEARANCE_SAVE_ID_BASE {
        ("appearance", id - APPEARANCE_SAVE_ID_BASE)
    } else {
        ("general", id)
    }
}

fn publish_appearance_persist(id: u64, key: String, value: Value) {
    publish_persist(id.saturating_add(APPEARANCE_SAVE_ID_BASE), key, value);
}

fn publish_keybinding(request: KeybindingHostRequest) {
    #[cfg(target_family = "wasm")]
    {
        let message = match request {
            KeybindingHostRequest::Reload => GuestToHost::RequestKeybindings,
            KeybindingHostRequest::Save {
                id,
                command_id,
                accelerator,
            } => GuestToHost::SaveKeybinding {
                id,
                command_id,
                accelerator,
            },
            KeybindingHostRequest::ResetAll { id } => GuestToHost::ResetAllKeybindings { id },
            KeybindingHostRequest::DebugDump => GuestToHost::KeybindingDebugDump,
        };
        post_to_parent(&message);
    }
    #[cfg(not(target_family = "wasm"))]
    {
        let _ = request;
    }
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

pub fn publish_word_removal(id: u64, word: String) {
    #[cfg(target_family = "wasm")]
    post_to_parent(&GuestToHost::SpellcheckerRemoveWord { id, word });
    #[cfg(not(target_family = "wasm"))]
    {
        let _ = (id, word);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_results_stay_on_the_pane_that_asked() {
        assert_eq!(save_result_target(1), ("general", 1));
        assert_eq!(
            save_result_target(APPEARANCE_SAVE_ID_BASE + 1),
            ("appearance", 1)
        );
    }
}
