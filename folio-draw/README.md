# Folio Draw

A quick, uncluttered picture editor for Zorin OS and Ubuntu, in the spirit of the classic desktop
paint program — and a machine-embroidery workshop. Part of Folio Studio.

Draw is written in Rust on Folio's portable base, which it shares with [Folio Cork](../README.md):
`crates/folio-platform` (the only code that talks to the operating system), `crates/folio-kit` (the
renderer, text and ribbon) and `crates/folio-math` (the deterministic maths every engine uses). Its
own engine is `crates/folio-draw-engine`, which depends on nothing but `folio-math`. It replaced the
GTK version in 1.11.0 and does everything that one did.

## What it does

- **Painting**: pencil, brushes, airbrush, eraser, fill, gradients, shapes, curves, text, and a
  stabilizer for a steadier line.
- **Selections**: rectangle, ellipse, lasso, polygon and magic wand, with tolerance and
  contiguous or not; move, crop, resize, rotate, flip.
- **Magic Select**: drag a box round the subject and it is cut out, ready to drag where you want
  it; Ctrl leaves the picture whole and lifts a copy.
- **From a scanner or camera**: a page from a scanner (through SANE's `scanimage`) or a picture
  from a webcam (through ffmpeg); the window keeps working while it comes in, and Draw says which
  package to install when neither tool is there.
- **Layers**: any number, with opacity, blend modes, groups and real transparency.
- **Pictures**: colour adjustments, remove background, and a history pane.
- **Files**: PNG, JPEG, BMP, GIF, WebP, TIFF, Windows icons, Photoshop PSD and OpenRaster (.ora,
  which keeps the layers). An animated GIF or WebP opens with one frame per layer. Every codec is
  Folio's own: no image library.
- **Printing and pages**: page setup lays a picture across sheets, with a print preview.
- **Embroidery**: a picture becomes editable shapes with fill, satin and running stitches;
  lettering from a font, envelopes, node editing, a density view and a stitch-out simulator. Saves
  for the machine as PES, DST, JEF, EXP, VP3, XXX or PEC, with five hoop sizes and the PEC thread
  chart. Crop and Canvas size work on a design too: the hoop is reframed round the new picture and
  the stitching stays where it was on the fabric.

Everything stays on this computer: no cloud, no sign-in, no telemetry.

## Layout

| Path | What's in it |
| --- | --- |
| `src/app.rs` | the window: ribbon, commands, tools, panes |
| `src/emb.rs`, `src/letters.rs` | embroidery: shapes, lettering, density, stitch-out |
| `src/page.rs`, `src/clip.rs` | page setup and printing; the clipboard relay used by the suite test |
| `src/stabilizer.rs` | the steadier line |
| `src/uitest.rs` | the window test: 50 steps, run in a window or in memory |
| `../crates/folio-draw-engine` | the picture engine: layers, undo, codecs, embroidery planning |

## Commands

| Command | What it does |
|---|---|
| `cargo run -p folio-draw` | run it |
| `cargo test -p folio-draw -p folio-draw-engine` | its unit tests |
| `folio-draw --ui-test DIR [--headless]` | the window test, in a window or in memory |
| `folio-draw --embroider PICTURE OUT.pes` | a picture straight to a machine file |
| `folio-draw --convert IN OUT` | convert between picture formats |
| `folio-draw/build-deb.sh out/` | the package |
| `tools/release-draw.sh` | the release run: tests, the .deb, the three test machines, Wayland |

Needs the fonts in `../fonts` (the `folio-fonts` package installs them to
`/usr/share/folio/fonts`). The portability rules it is held to are in
[docs/PORTABILITY.md](../docs/PORTABILITY.md).
