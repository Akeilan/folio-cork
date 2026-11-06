# Folio Page on the portable base

The word processor, rewritten on the suite's own base: no GTK, no cairo, no
Python. It opens and writes .docx documents, lays them out the way Word does,
draws them, and writes PDFs with the text still in them.

## What it is made of

| Crate | What it does |
| --- | --- |
| `crates/folio-page-engine` | The .docx model, the layout and the pagination. No drawing, no windows. |
| `crates/folio-canvas` | A drawing surface: paths, gradients, clipping and glyph outlines, the same pixels everywhere. |
| `crates/folio-page-render` | Draws a laid-out page: text, tables, pictures, shapes, equations, charts, metafiles. Writes the PDF. |
| `crates/folio-page-editor` | The caret, selections, the commands, undo, tables, comments, fields, references, review. |
| `folio-page` | The window: title bar, ribbon, the pages, the status bar. |

Each layer only knows the one below it. The engine measures with the same font
files the renderer draws from, so what is drawn is what was measured, and a
page breaks the same way on every machine (deterministic maths throughout).

## Running it

    folio-page [FILE]            open a document
    folio-page --pdf IN OUT      write a PDF without opening a window
    folio-page --ui-test [DIR]   drive the window as a person would
    folio-page --ui-test DIR --headless    the same, in memory

## What works

Opening and saving .docx. Typing, selecting with the mouse and the keyboard,
undo and redo. Fonts, size, bold, italic, underline, strikethrough, colour,
highlight, case and clearing formatting, with the format painter. Alignment,
bullets and numbering, indents, line spacing and paragraph styles. Page and
column breaks, tables (rows, columns, merging, borders), pictures, headers and
footers with page numbers. Margins, orientation and paper size. Find and
replace. A contents list, fields, footnotes and endnotes. Comments, Track
Changes, and accepting or rejecting changes. Print Layout, Draft and Web
Layout, zoom, and the paragraph marks. Export PDF with the fonts embedded,
searchable text and bookmarks. The last ten documents, and a question before
closing a document with changes in it.

22 window tests drive all of this as a person would, and pass in memory, in an
X window and on Wayland.

## What is not here yet

The Python version still ships, and still has what this one has not: the ruler
and the navigation pane, mail merge, citations and the bibliography, captions
and cross-reference dialogs, charts and drawings made in the document, the
accessibility checker, read mode and read aloud, macros, document protection,
and the dialogs behind many of the ribbon's launchers. This one takes over when
it has them.
