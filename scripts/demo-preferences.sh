#!/usr/bin/env bash
set -euo pipefail

mode="${1:-gpui-native}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo_root/rust/target}"

case "$mode" in
  gpui-native)
    overrides='window.preferences=gpui-native'
    pnpm migration:build-native-pref
    pnpm build:unpack
    export MARKTEXT_PREF_APP="$target_dir/release/mt-pref-app"
    if [ ! -x "$MARKTEXT_PREF_APP" ]; then
      echo "Expected native binary at $MARKTEXT_PREF_APP" >&2
      exit 1
    fi
    if [ "$(uname -s)" = "Linux" ] && [ -z "${VK_ICD_FILENAMES:-}" ] && [ -f /usr/share/vulkan/icd.d/lvp_icd.json ]; then
      export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
    fi
    ;;
  rust-wasm)
    overrides='prefs.sidebar=rust-wasm'
    for pane in general appearance editor files markdown images keybindings; do
      overrides+=",prefs.pane.${pane}=rust-wasm"
    done
    pnpm migration:build-wasm
    pnpm build:unpack
    ;;
  legacy)
    overrides=''
    pnpm build:unpack
    ;;
  *)
    echo 'Usage: pnpm migration:demo [gpui-native|rust-wasm|legacy]' >&2
    exit 2
    ;;
esac

profile_dir=$(mktemp -d "${TMPDIR:-/tmp}/marktext-demo.XXXXXX")
trap 'rm -rf -- "$profile_dir"' EXIT

export MARKTEXT_MIGRATION_ENV=prod
export MARKTEXT_MIGRATION_OVERRIDES="$overrides"

echo "Launching MarkText with Preferences mode=${mode} (disposable profile: ${profile_dir})."
echo "Open Preferences (Cmd+, on macOS / Ctrl+, on Linux) to inspect; quit to discard the demo profile."
pnpm -C packages/desktop exec electron . --user-data-dir "$profile_dir"
