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

## The settings page stops scrolling (A1)

The scroll class lives on `<router-view>` and falls through onto `GpuiPaneHost`. That component sets `inheritAttrs: false`. A bare slot drops the class, so the page no longer scrolls. Keep the fallthrough attrs on a real element.

## A saved-looking edit never reached disk (A1)

The exclusion field cleared and the new rule appeared in the pane, but a read-only or rejected write left `preferences.json` unchanged. The failure was only a log line.

`SET_PREFERENCE` is forwarded with `mt::set-user-preference`, which does not report success. An edit that throws away typed text, or otherwise presents the value as saved, has to post `PERSIST_PREFERENCE` and wait for `PREFERENCE_RESULT`. The host answers by invoking `mt::preferences::update`, whose status is `persisted` or `failed`.

Update the snapshot and clear the draft only when the status is `persisted`. On `failed`, leave the draft and show the error. Clear the draft only if the field still contains the submitted text: the user can type the next rule while the round trip is in flight. Ignore a second edit of the same list until that result arrives. A list computed from the pre-save snapshot would replace the save that is already running.

Toggles and choice chips can keep using `SET_PREFERENCE` until a slice needs to show a save failure.

## Labels that the bundled font cannot draw (A1)

The canvas loads Liberation Sans only. A language name written in Chinese, Japanese, or Korean is invisible, which looks like a missing control rather than a font gap. Show an English label. The stored value stays the locale code, such as `zh-CN`.
