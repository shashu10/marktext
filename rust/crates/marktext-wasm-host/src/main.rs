use std::borrow::Cow;

#[cfg(target_family = "wasm")]
use gpui::Entity;
use gpui::{App, AppContext as _, WindowOptions};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use marktext_wasm_host::HostView;

fn main() {
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();

    gpui_platform::application().run(|cx| {
        load_fonts(cx);
        cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));

        let host = cx.new(|_cx| HostView::new("general"));
        #[cfg(target_family = "wasm")]
        install_host_messages(host.clone(), cx);

        cx.open_window(WindowOptions::default(), move |_window, _cx| host.clone())
            .expect("gpui_web allows one canvas in this iframe");
        cx.activate(true);
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

#[cfg(target_family = "wasm")]
fn install_host_messages(host: Entity<HostView>, cx: &mut App) {
    use marktext_core::HostToGuest;
    use wasm_bindgen::JsCast as _;
    use wasm_bindgen::closure::Closure;

    let (tx, rx) = async_channel::unbounded::<String>();
    let listener = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
        let Ok(encoded) = js_sys::JSON::stringify(&event.data()) else {
            return;
        };
        if let Some(text) = encoded.as_string() {
            let _ = tx.try_send(text);
        }
    }) as Box<dyn FnMut(web_sys::MessageEvent)>);

    if let Some(window) = web_sys::window() {
        let _ =
            window.add_event_listener_with_callback("message", listener.as_ref().unchecked_ref());
        listener.forget();
    }

    cx.spawn(async move |cx| {
        while let Ok(raw) = rx.recv().await {
            let Ok(message) = serde_json::from_str::<HostToGuest>(&raw) else {
                continue;
            };
            let reply = cx.update(|app| {
                host.update(app, |view, cx| {
                    let reply = view.apply(&message);
                    cx.notify();
                    reply
                })
            });
            if let Some(reply) = reply {
                post_to_parent(&reply);
            }
        }
    })
    .detach();
}

#[cfg(target_family = "wasm")]
fn post_to_parent(reply: &marktext_core::GuestToHost) {
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
