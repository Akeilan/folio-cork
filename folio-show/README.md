# Folio Show on the portable base

The presentations app, rewritten on the suite's own base: no GTK, no cairo, no
Python. It opens and writes .pptx decks, lays the slides out and draws them with
the same pixels on every machine.

## What it is made of

| Crate | What it does |
| --- | --- |
| `crates/folio-show-engine` | The .pptx model, the slide and text layout, the drawing and the editing. No windows. |
| `crates/folio-page-engine` | The OOXML package, the XML, shapes, pictures and fonts, shared with Page. |
| `crates/folio-page-render` | Draws shapes and charts on the canvas, shared with Page. |
| `crates/folio-canvas` | A drawing surface: paths, gradients, clipping and glyph outlines. |
| `folio-show` | The window: title bar, ribbon, slide pane, the slide itself, notes and status bar. |

The engine used to draw through cairo and FreeType; that layer is now
folio-canvas and folio-kit, so the slides draw the same everywhere and nothing
below the app comes from the system.

## Running it

    folio-show [FILE]             open a presentation
    folio-show --png IN OUT       draw the first slide to a PNG
    folio-show --ui-test [DIR]    drive the window as a person would
    folio-show --ui-test DIR --headless    the same, in memory

## What works

Opening and saving .pptx. The slide pane with a live picture of every slide;
adding, duplicating and deleting slides, and picking a layout. Shapes and text
boxes added, picked out, moved and deleted, with cut, copy and paste. Typing in
a shape, with the caret placed by a click, Backspace, Enter for a new paragraph,
and the arrow keys. Bold, italic and underline, alignment and bullets. Notes for
a slide. Presenting full screen, with the arrow keys, Space and Escape. Zoom and
the slide pane off and on. The File pages open, save, save as, start a new
presentation and write the slide out as a PNG. A changed presentation asks before
it closes. Everything stays on this computer.

## Testing it

    cargo test -p folio-show-engine         the engine's own tests
    folio-show --ui-test DIR                the window test, on X or Wayland
    folio-show --ui-test DIR --headless     the same without a display
