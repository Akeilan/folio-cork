# Folio Sheet on the portable base

The spreadsheet, rewritten on the suite's own base: no GTK, no cairo, no
Python. It opens and writes .xlsx workbooks, works the formulas out itself,
and draws the grid with the same pixels on every machine.

## What it is made of

| Crate | What it does |
| --- | --- |
| `crates/folio-sheet-engine` | The workbook model, the formulas and their answers, the number formats, the .xlsx reader and writer, and the drawing of the grid. No windows. |
| `crates/folio-canvas` | A drawing surface: paths, gradients, clipping and glyph outlines. |
| `crates/folio-kit` | The ribbon, dialogs, menus, the file pane, text and icons. |
| `folio-sheet` | The window: title bar, ribbon, formula bar, grid, sheet tabs, status bar. |

The engine draws through one small layer (`engine/src/ffi.rs`) that keeps the
old cairo and pango names but paints on folio-canvas, so the 51,000 lines that
already knew how to draw a sheet carried over whole.

## Running it

    folio-sheet [FILE]            open a workbook
    folio-sheet --png IN OUT      draw the first sheet to a PNG
    folio-sheet --ui-test [DIR]   drive the window as a person would
    folio-sheet --ui-test DIR --headless    the same, in memory

## What works

Opening and saving .xlsx. Typing into cells, in the cell or the formula bar,
with undo and redo. Formulas, with the engine's own functions, recalculated as
you go. Moving with the arrow keys, the mouse, and selections that extend with
Shift. Cut, copy and paste, inside the window and to and from other programs
as tab-separated text. Font, size, bold, italic, underline, alignment and text
wrapping. Number formats, with more and fewer decimal places. AutoSum. Rows and
columns inserted and deleted. Several sheets to a workbook, added and deleted,
with tabs along the bottom. Find, Find Next and Replace All. Frozen panes,
gridlines and headings off and on, and formulas shown instead of answers.
Zoom. The File pages open, save, save as, start a new workbook and write the
sheet out as CSV. Everything stays on this computer.

## Testing it

    cargo test -p folio-sheet-engine        the engine's own tests
    folio-sheet --ui-test DIR               the window test, on X or Wayland
    folio-sheet --ui-test DIR --headless    the same without a display

The window test drives the real window: it types, clicks the ribbon, opens the
File pages, saves the workbook and opens it again.
