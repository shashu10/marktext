# Migration memory

Lessons from slices where the first implementation was wrong and the cause was not obvious from the code. Read this before implementing a slice. Use an entry when it matches the pane you are touching. Ignore the rest.

After the slice, add a lesson only if all of these are true:

- You had to investigate. A fix that was right on the first attempt does not belong here.
- The cause is not already in this file, `AGENTS.md`, or `RUNBOOK.md`.
- A later pane can hit the same constraint.

Write a few sentences: what you saw, why it happened, what to do. Extend an existing entry when it is the same constraint. Leave out clippy and lint nits, import errors, which controls belong on which pane, measurements, and review chatter.

## The canvas stays black (A1)

The iframe was loaded and the pane-ready attribute was set, but the canvas was black and no control was visible.

`size_full` is a percentage of the parent. A pane returned as the window root has no sized parent, so it lays out at zero. `HostView` has to wrap the active pane in `div().size_full().child(content)`. A child view's own `size_full` is not enough.

## The first pane message never arrives (A1)

`SET_PANE` and the preference snapshot posted from the iframe `load` handler never reached the guest, so the host stayed on the placeholder.

`load` fires before the WASM guest installs its `message` listener. The guest posts `READY` once it is listening. The Vue host must send `SET_PANE` and the snapshot again when `READY` arrives. Sending them only from `load` is not enough.

## READY and HOST_HELLO must not bounce (W2.A)

The native prefs sidecar posts `READY` once at startup. Echoing `HOST_HELLO` from that bootstrap makes `PaneRouter` reply `READY` again, and the host re-pushes forever. On Stage 2, answer the first `READY` with `SET_PANE` / snapshots only. Keep a one-shot bootstrap guard. `HOST_HELLO` is for Stage 1 iframe reconnect, not for the NDJSON launcher.

## Empty accelerator is an unbind (W2.A)

Keybinding user maps store `""` when the user clears a shortcut. Treating empty like a missing key restores the default. When displaying and when writing the next user map, keep `""` as a real value; only delete the entry when the accelerator equals the default.

## The settings page stops scrolling (A1)

The scroll class lives on `<router-view>` and falls through onto `GpuiPaneHost`. That component sets `inheritAttrs: false`. A bare slot drops the class, so the page no longer scrolls. Keep the fallthrough attrs on a real element.

## A saved-looking edit never reached disk (A1)

The exclusion field cleared and the new rule appeared in the pane, but a read-only or rejected write left `preferences.json` unchanged. The failure was only a log line.

`SET_PREFERENCE` is forwarded with `mt::set-user-preference`, which does not report success. An edit that throws away typed text, or otherwise presents the value as saved, has to post `PERSIST_PREFERENCE` and wait for `PREFERENCE_RESULT`. The host answers by invoking `mt::preferences::update`, whose status is `persisted` or `failed`.

Update the snapshot and clear the draft only when the status is `persisted`. On `failed`, leave the draft and show the error. Clear the draft only if the field still contains the submitted text: the user can type the next rule while the round trip is in flight. Ignore a second edit of the same list until that result arrives. A list computed from the pre-save snapshot would replace the save that is already running.

Toggles and choice chips can keep using `SET_PREFERENCE` until a slice needs to show a save failure.

## A text field that replaces the saved value (A2)

The custom CSS box started empty and reported only the length of the saved stylesheet. Saving without typing wrote that empty string over the stylesheet. Font family and line width had the same hole, and a rejected save was shown only under the CSS box.

`EditableTextState::use_keyed` starts empty, and `use_keyed_init` runs on the first frame only. Create the field with the snapshot text. If a later snapshot arrives, copy it in only while the field still equals the last snapshot it showed. Keystrokes move it off that baseline and stay on screen. Do the copy after the frame: replacing the text during render notifies the field while the pane is still drawing. On `failed`, leave the draft and show the error on that field.

## Labels that the bundled font cannot draw (A1)

The canvas loads Liberation Sans only. A language name written in Chinese, Japanese, or Korean is invisible, which looks like a missing control rather than a font gap. Show an English label. The stored value stays the locale code, such as `zh-CN`.

## `HOST_ACTION` payloads arrive as JS `Map` instead of plain objects (A8)

Clicking a category in the Rust sidebar highlighted the sidebar row, but Vue never navigated to the new pane.

`serde_wasm_bindgen::to_value` serializes `serde_json::Value::Object` as a JavaScript `Map`, so property checks like `typeof payload.category === 'string'` in `GpuiPaneHost.vue` silently fail. Always serialize guest-to-host messages with `serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true)` in `marktext-wasm-host`.

## Data-center keys never reach the preference bridge (A6)

`imageFolderPath`, `currentUploader`, and `cliScript` arrive in the `PREFERENCES` snapshot from Pinia, but `MemoryHostBridge` only keeps keys from `schema.json`. The Images pane looked empty for the global folder even though Electron had a path.

Keep those keys on the pane from the raw snapshot (`extract_user_data`). Write them with `HOST_ACTION` (`set-user-data`, `ask-image-folder`, probes, open path/url), not `SET_PREFERENCE`. Seed `EditableTextState` from the pane copy on first paint; an empty draft does not mean the host has no value.
