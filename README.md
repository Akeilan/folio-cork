# Folio Cork

Diagrams, whiteboards and mood boards for Folio Studio, written in Rust on Folio's portable platform
layer and toolkit.

- `crates/folio-platform` — the only code that talks to the operating system (window, input,
  clipboard, printing). Linux X11 and Wayland today; Windows and macOS through the same layer.
- `crates/folio-kit` — the Folio toolkit: software renderer, text, ribbon with KeyTips and
  ScreenTips, title bar in the app colour, menus, dialogs, file browser, light and dark themes.
- `folio-cork` — the app.

Build and test: `cargo test`, then `cargo run -- --ui-test DIR` (window test, X11 or Wayland).
Release: `tools/release.sh` (tests, .deb built on 22.04, installed tests on three machines, Wayland).
