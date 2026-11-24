//! Full preferences window: fixed sidebar + active category pane.
//!
//! Stage 1 hosts each surface in its own iframe. Stage 2 draws both in one
//! native window and routes `navigate-preference` locally.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{AnyElement, Context, IntoElement, Render, Window, div, prelude::*, px};
use marktext_core::{GuestSession, GuestToHost, HostToGuest};
use mt_pref_appearance::AppearancePane;
use mt_pref_editor::EditorPane;
use mt_pref_files::FilesPane;
use mt_pref_general::GeneralPane;
use mt_pref_images::ImagesPane;
use mt_pref_keybindings::{KeybindingHostRequest, KeybindingsPane};
use mt_pref_markdown::MarkdownPane;
use mt_pref_shell::PrefShellPane;
use serde_json::Value;

use crate::ndjson::write_guest;

const APPEARANCE_SAVE_ID_BASE: u64 = 1 << 32;
const SIDEBAR_WIDTH: f32 = 220.0;

pub struct PrefAppRoot {
    session: GuestSession,
    general: gpui::Entity<GeneralPane>,
    appearance: gpui::Entity<AppearancePane>,
    editor: gpui::Entity<EditorPane>,
    files: gpui::Entity<FilesPane>,
    markdown: gpui::Entity<MarkdownPane>,
    images: gpui::Entity<ImagesPane>,
    keybindings: gpui::Entity<KeybindingsPane>,
    shell: gpui::Entity<PrefShellPane>,
}

impl PrefAppRoot {
    pub fn new(initial_pane: impl Into<String>, cx: &mut Context<Self>) -> Self {
        let session = GuestSession::new(initial_pane);
        let bridge = session.bridge_handle();
        let is_mac = cfg!(target_os = "macos");
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
                Arc::clone(&bridge),
                Rc::new(publish_preference),
                Rc::new(publish_persist),
            )
        });
        let images = cx.new(|_cx| {
            ImagesPane::new(
                bridge,
                Rc::new(publish_preference),
                Rc::new(publish_persist),
                Rc::new(publish_host_action),
            )
        });
        let keybindings = cx.new(|cx| KeybindingsPane::new(Rc::new(publish_keybinding), cx));
        let shell = cx.new(|cx| {
            PrefShellPane::new(
                Rc::new(publish_host_action),
                Rc::new(request_keybindings),
                is_mac,
                cx,
            )
        });
        Self {
            session,
            general,
            appearance,
            editor,
            files,
            markdown,
            images,
            keybindings,
            shell,
        }
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
                    self.images.update(cx, |pane, cx| {
                        pane.accept_persist_result(request_id, persisted, cx);
                    });
                }
                return None;
            }
            HostToGuest::HostActionResult {
                id,
                status,
                payload,
            } => {
                let request_id = *id;
                self.images.update(cx, |pane, cx| {
                    pane.accept_host_result(request_id, status, payload, cx);
                });
                return None;
            }
            HostToGuest::Preferences { snapshot } => {
                self.sync_images_user_data(snapshot, cx);
            }
            HostToGuest::PreferencePatch { changes } => {
                self.sync_images_user_data(changes, cx);
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
                    pane.apply_snapshot(commands, *is_mac, *show_debug, *loading, *load_error, cx);
                });
                let commands = commands.clone();
                let is_mac = *is_mac;
                let loading = *loading;
                self.shell.update(cx, |pane, cx| {
                    pane.apply_keybindings(commands, is_mac, loading, cx);
                });
                return None;
            }
            HostToGuest::ActiveCategory { category } => {
                let category = category.clone();
                self.shell.update(cx, |pane, cx| {
                    pane.set_active_category(&category, cx);
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
            HostToGuest::SetPane { pane } => {
                let category = pane.clone();
                self.shell.update(cx, |shell, cx| {
                    shell.set_active_category(&category, cx);
                });
            }
            _ => {}
        }
        let reply = self.session.apply(message);
        self.notify_panes(cx);
        reply
    }

    /// Sidebar search / category clicks stay inside this process.
    pub fn navigate_preference(&mut self, category: &str, cx: &mut Context<Self>) {
        let set = HostToGuest::SetPane {
            pane: category.to_string(),
        };
        let _ = self.apply(&set, cx);
        self.shell.update(cx, |pane, cx| {
            pane.set_active_category(category, cx);
        });
    }

    fn sync_images_user_data(
        &mut self,
        snapshot: &std::collections::BTreeMap<String, Value>,
        cx: &mut Context<Self>,
    ) {
        self.images.update(cx, |pane, cx| {
            pane.sync_user_data(snapshot, cx);
        });
    }

    fn notify_panes(&mut self, cx: &mut Context<Self>) {
        self.general.update(cx, |_pane, cx| cx.notify());
        self.appearance.update(cx, |_pane, cx| cx.notify());
        self.editor.update(cx, |_pane, cx| cx.notify());
        self.files.update(cx, |_pane, cx| cx.notify());
        self.markdown.update(cx, |_pane, cx| cx.notify());
        self.images.update(cx, |_pane, cx| cx.notify());
        self.keybindings.update(cx, |_pane, cx| cx.notify());
        self.shell.update(cx, |_pane, cx| cx.notify());
    }
}

impl Render for PrefAppRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match self.session.active_pane() {
            "appearance" => self.appearance.clone().into_any_element(),
            "editor" => self.editor.clone().into_any_element(),
            "files" => self.files.clone().into_any_element(),
            "markdown" => self.markdown.clone().into_any_element(),
            "images" => self.images.clone().into_any_element(),
            "keybindings" => self.keybindings.clone().into_any_element(),
            _ => self.general.clone().into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_row()
            .child(
                div()
                    .w(px(SIDEBAR_WIDTH))
                    .h_full()
                    .flex_shrink_0()
                    .child(self.shell.clone()),
            )
            .child(div().flex_1().h_full().min_w_0().child(content))
    }
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
    write_guest(&message);
}

fn request_keybindings() {
    write_guest(&GuestToHost::RequestKeybindings);
}

fn publish_persist(id: u64, key: String, value: Value) {
    write_guest(&GuestToHost::PersistPreference { id, key, value });
}

fn publish_preference(key: String, value: Value) {
    write_guest(&GuestToHost::SetPreference { key, value });
}

fn publish_choose_directory(id: u64) {
    write_guest(&GuestToHost::ChooseDirectory { id });
}

fn publish_word_removal(id: u64, word: String) {
    write_guest(&GuestToHost::SpellcheckerRemoveWord { id, word });
}

fn publish_host_action(id: u64, action: String, payload: Value) {
    write_guest(&GuestToHost::HostAction {
        id,
        action,
        payload,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_save_ids_stay_disjoint() {
        assert_eq!(save_result_target(1), ("general", 1));
        assert_eq!(
            save_result_target(APPEARANCE_SAVE_ID_BASE + 3),
            ("appearance", 3)
        );
    }
}
