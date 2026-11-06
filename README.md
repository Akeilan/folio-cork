# Folio Cork

Diagrams, whiteboards and mood boards for Folio Studio. Cork is written in Rust on Folio's portable platform layer and toolkit, the base every Folio app moves to. Draw is the next app being moved onto it.

- `crates/folio-platform`: the only code that talks to the operating system.
  - Covers the window, input, clipboard, sound, files, sockets, threads and printing.
  - Linux (X11 and Wayland) today. Windows and macOS come through the same layer.
  - Its contract is in [docs/PLATFORM.md](docs/PLATFORM.md).
- `crates/folio-kit`: the Folio toolkit.
  - A software renderer, text, and the ribbon with KeyTips and ScreenTips.
  - A title bar in the app colour, menus, dialogs, a file browser, light and dark themes.
  - Deterministic maths.
- `folio-cork`: the app.
  - Swimlane diagrams (K4-01): Insert > Swimlane adds a lane for a team or a stage; moving a lane carries what's in it, Turn Lanes runs them down the page instead of across, and the Cross-functional flowchart template starts you off.
  - Shape data (K4-02): named properties on a shape — cost, owner, part number — in the Shape Data pane (View > Shape Data), saved in the drawing and written into the .vsdx the way Visio writes them.
  - Diagrams in Page, Sheet and Show: Copy in Cork and paste there. The picture carries the drawing, and double-clicking it opens it in Cork again. The format is in [docs/FORMATS.md](docs/FORMATS.md).
- `folio-draw`: Folio Draw, on this base since 1.11.0 (D3-06). It replaced the GTK version, which it matches. Its own page is [folio-draw/README.md](folio-draw/README.md).
  - Its engine is `crates/folio-draw-engine`, with no dependencies beyond `crates/folio-math` (the deterministic maths every engine shares).
  - `folio-draw/build-deb.sh` builds its package; `tools/release-draw.sh` is its release run.
- `fonts`: the fonts that ship with Folio (Carlito, Liberation Serif, Liberation Mono; Latin, Greek and Cyrillic).
- `docs`: [portability rules](docs/PORTABILITY.md), [file and wire formats](docs/FORMATS.md), [platform contract](docs/PLATFORM.md).

## Commands

| Command | What it does |
|---|---|
| `cargo test --workspace` | Unit tests, pixel tests, the platform conformance tests, and the full window test without a display. |
| `tools/check-portability.sh` | The portability rules that a machine can check. |
| `tools/conformance.sh` | Window conformance on a real display (X11, or Wayland through Weston). |
| `folio-cork --ui-test DIR [--headless]` | The window test, in a window or in memory. |
| `folio-cork --bench` | The Raspberry Pi 4B budgets. |
| `folio-draw --ui-test DIR [--headless]` | Draw's window test. |
| `folio-draw --embroider PICTURE -o DESIGN.pes` | A picture turned into a machine file, without a window. |
| `tools/release.sh` | All of the above, the .deb built on 22.04, the installed tests on three machines, and the Wayland pass. |
| `tools/release-draw.sh` | The same for Folio Draw. |
