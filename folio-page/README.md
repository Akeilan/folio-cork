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
highlight, case and clearing formatting, with the format painter and a Font
dialog. Alignment, bullets and numbering, indents, line spacing and paragraph
styles, with a Paragraph dialog and a ruler whose markers drag. Page and column
breaks, tables (rows, columns, merging, borders), pictures, shapes, charts,
symbols, headers and footers with page numbers. Margins, orientation, paper
size and a Page Setup dialog. Find and replace. A contents list, fields,
footnotes and endnotes, captions, cross-references, bookmarks, citations and a
bibliography. Comments, Track Changes, accepting or rejecting changes, and
protecting the document. Spelling, with a red wavy line and other spellings on
a right-click. Mail merge from a comma- or tab-separated list, previewed
recipient by recipient and written out one document each. The navigation pane,
Print Layout, Draft, Web Layout and Read Mode, zoom, and the paragraph marks.
Printing, and Export PDF with the fonts embedded, searchable text, bookmarks
and the document's properties. The accessibility check. The last ten
documents, and a question before closing a document with changes in it.

36 window tests drive all of this as a person would, and pass in memory, in an
X window and on Wayland.

## What is not here yet

Macros, and read aloud. The Python version still ships until the Product Owner
says this one takes over.
