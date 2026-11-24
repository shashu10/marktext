use std::borrow::Cow;

use gpui::{
    App, AppContext as _, Bounds, Entity, QuitMode, TitlebarOptions, WindowBounds, WindowHandle,
    WindowOptions, px, size,
};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use marktext_core::{GuestToHost, HostToGuest};
use mt_pref_app::{PrefAppRoot, spawn_stdin_reader, write_guest};

fn main() {
    let initial = std::env::args()
        .nth(1)
        .filter(|pane| !pane.is_empty())
        .unwrap_or_else(|| "general".to_string());

    gpui_platform::application()
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx| {
            // Defer initialization onto the next foreground turn so `gpui_ce_macos`'s
            // `did_finish_launching` releases its `platform.0` mutex before
            // `marktext_ui_kit::init` / `open_window` re-enter `MacPlatform`.
            cx.spawn(async move |cx| {
                cx.update(|cx| {
                    load_fonts(cx);
                    marktext_ui_kit::init(cx);
                    cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));

                    let root = cx.new(|cx| PrefAppRoot::new(initial.clone(), cx));
                    let bounds = Bounds::centered(None, size(px(950.), px(650.)), cx);
                    let window = cx
                        .open_window(
                            WindowOptions::default()
                                .window_bounds(WindowBounds::Windowed(bounds))
                                .titlebar(Some(TitlebarOptions {
                                    title: Some("Preferences".into()),
                                    ..Default::default()
                                })),
                            {
                                let root = root.clone();
                                move |_window, _cx| root.clone()
                            },
                        )
                        .expect("open preferences window");
                    install_stdio_bridge(root, window, cx);
                    cx.activate(true);
                    write_guest(&GuestToHost::Ready {
                        pane: Some(initial),
                    });
                });
            })
            .detach();
        });
}

fn load_fonts(cx: &mut App) {
    let regular = include_bytes!("../../../assets/fonts/LiberationSans-Regular.ttf");
    let bold = include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf");
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(regular.as_slice()),
            Cow::Borrowed(bold.as_slice()),
        ])
        .expect("bundled Liberation Sans fonts are valid");
}

fn install_stdio_bridge(
    root: Entity<PrefAppRoot>,
    window: WindowHandle<PrefAppRoot>,
    cx: &mut App,
) {
    let (tx, rx) = async_channel::unbounded::<HostToGuest>();
    spawn_stdin_reader(tx);

    cx.spawn(async move |cx| {
        while let Ok(message) = rx.recv().await {
            let reply = cx.update(|app| {
                if matches!(message, HostToGuest::Activate) {
                    app.activate(true);
                    let _ = window.update(app, |_view, window, _cx| {
                        window.activate();
                    });
                    return None;
                }
                root.update(app, |view, cx| {
                    let reply = view.apply(&message, cx);
                    cx.notify();
                    reply
                })
            });
            if let Some(reply) = reply {
                write_guest(&reply);
            }
        }
    })
    .detach();
}
