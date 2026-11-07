//! Page's ribbon tabs.

use folio_kit::ribbon::{Group, Item as I, Tab};

pub fn tabs() -> Vec<Tab> {
    vec![
        Tab::new(
            "home",
            "Home",
            "H",
            vec![
                Group::new(
                    "Clipboard",
                    vec![
                        I::large("paste", "Paste", "paste").key("Ctrl+V").desc("Paste what was cut or copied."),
                        I::small("cut", "Cut", "cut").key("Ctrl+X"),
                        I::small("copy", "Copy", "copy").key("Ctrl+C"),
                        I::small("painter", "Format Painter", "painter").desc("Copy the look of the text, then select other text to give it that look."),
                    ],
                ),
                Group::rows(
                    "Font",
                    vec![
                        vec![
                            I::combo("font", "Font", 150).desc("The font of the selected text."),
                            I::combo("size", "Font size", 56),
                            I::icon("font-dialog", "More font settings", "font-dialog"),
                        ],
                        vec![
                            I::icon("bold", "Bold", "bold").key("Ctrl+B"),
                            I::icon("italic", "Italic", "italic").key("Ctrl+I"),
                            I::icon("underline", "Underline", "underline").key("Ctrl+U"),
                            I::icon("strike", "Strikethrough", "strike"),
                            I::icon("font-color", "Font colour", "font-color").menu(),
                            I::icon("highlight", "Highlight", "highlight").menu(),
                            I::icon("case", "Change case", "case").menu(),
                            I::icon("clear-format", "Clear formatting", "clear-format"),
                        ],
                    ],
                ),
                Group::rows(
                    "Paragraph",
                    vec![
                        vec![
                            I::icon("bullets", "Bullets", "bullets"),
                            I::icon("numbering", "Numbering", "numbering"),
                            I::icon("outdent", "Decrease indent", "outdent"),
                            I::icon("indent", "Increase indent", "indent"),
                            I::icon("marks", "Show marks", "marks").desc("Show paragraph marks, spaces and tabs."),
                            I::icon("line-spacing", "Line spacing", "line-spacing").menu(),
                        ],
                        vec![
                            I::icon("align-left", "Align left", "align-left"),
                            I::icon("align-center", "Centre", "align-center"),
                            I::icon("align-right", "Align right", "align-right"),
                            I::icon("align-justify", "Justify", "align-justify"),
                            I::icon("para-dialog", "More paragraph settings", "para-dialog"),
                        ],
                    ],
                ),
                Group::new(
                    "Styles",
                    vec![I::large("styles", "Styles", "styles").menu().desc("Heading and body styles from the document.")],
                ),
                Group::new(
                    "Editing",
                    vec![
                        I::large("find", "Find", "find").key("Ctrl+F").desc("Find words, and replace them."),
                        I::small("select-all", "Select All", "select-all").key("Ctrl+A"),
                        I::small("undo", "Undo", "undo").key("Ctrl+Z"),
                        I::small("redo", "Redo", "redo").key("Ctrl+Y"),
                    ],
                ),
            ],
        ),
        Tab::new(
            "insert",
            "Insert",
            "N",
            vec![
                Group::new(
                    "Pages",
                    vec![
                        I::large("break-page", "Page Break", "break-page").key("Ctrl+Return"),
                        I::small("break-column", "Column Break", "break-column"),
                    ],
                ),
                Group::new("Tables", vec![I::large("table", "Table", "table").desc("A table of three rows and three columns.")]),
                Group::new("Illustrations", vec![I::large("picture", "Picture", "picture").desc("A picture from a file.")]),
                Group::new("Symbols", vec![I::large("symbol", "Symbol", "symbol").menu()]),
                Group::new(
                    "Header & Footer",
                    vec![
                        I::small("header", "Header", "header"),
                        I::small("footer", "Footer", "footer"),
                        I::small("page-number", "Page Number", "page-number"),
                        I::small("body", "Back to the text", "body"),
                    ],
                ),
            ],
        ),
        Tab::new(
            "layout",
            "Layout",
            "P",
            vec![Group::new(
                "Page Setup",
                vec![
                    I::large("page-setup", "Page Setup", "page-setup"),
                    I::large("margins", "Margins", "margins").menu(),
                    I::large("orientation", "Orientation", "orientation").menu(),
                    I::large("paper", "Size", "paper").menu(),
                ],
            )],
        ),
        Tab::new(
            "table",
            "Table",
            "T",
            vec![
                Group::new(
                    "Rows & Columns",
                    vec![
                        I::large("row-below", "Insert Below", "row-below"),
                        I::small("row-above", "Insert Above", "row-above"),
                        I::small("col-right", "Insert Right", "col-right"),
                        I::small("col-left", "Insert Left", "col-left"),
                        I::small("row-delete", "Delete Rows", "row-delete"),
                        I::small("col-delete", "Delete Columns", "col-delete"),
                    ],
                ),
                Group::new(
                    "Merge",
                    vec![I::large("merge", "Merge Cells", "merge"), I::small("table-delete", "Delete Table", "table-delete")],
                ),
                Group::new("Borders", vec![I::large("borders", "Borders", "borders").menu()]),
            ],
        ),
        Tab::new(
            "references",
            "References",
            "S",
            vec![
                Group::new(
                    "Table of Contents",
                    vec![I::large("toc", "Table of Contents", "toc").desc("A contents list made from the headings."),
                         I::small("update-fields", "Update", "update-fields").desc("Work the fields out again.")],
                ),
                Group::new(
                    "Captions",
                    vec![
                        I::large("caption", "Caption", "caption"),
                        I::small("cross-ref", "Cross-reference", "cross-ref"),
                        I::small("bookmark", "Bookmark", "bookmark"),
                    ],
                ),
                Group::new(
                    "Citations",
                    vec![
                        I::large("citation", "Citation", "citation"),
                        I::small("bibliography", "Bibliography", "bibliography"),
                    ],
                ),
                Group::new(
                    "Footnotes",
                    vec![
                        I::large("footnote", "Insert Footnote", "footnote").key("Alt+Ctrl+F"),
                        I::small("endnote", "Insert Endnote", "endnote"),
                    ],
                ),
            ],
        ),
        Tab::new(
            "mailings",
            "Mailings",
            "M",
            vec![
                Group::new(
                    "Recipients",
                    vec![I::large("recipients", "Select Recipients", "recipients")
                        .desc("A comma- or tab-separated file: the first line names the columns.")],
                ),
                Group::new(
                    "Write",
                    vec![I::large("merge-field", "Insert Merge Field", "merge-field").menu()],
                ),
                Group::new(
                    "Preview",
                    vec![
                        I::large("preview-merge", "Preview", "preview-merge"),
                        I::small("merge-prev", "Previous", "merge-prev"),
                        I::small("merge-next", "Next", "merge-next"),
                    ],
                ),
                Group::new("Finish", vec![I::large("finish-merge", "Finish & Merge", "finish-merge")]),
            ],
        ),
        Tab::new(
            "review",
            "Review",
            "R",
            vec![
                Group::new(
                    "Comments",
                    vec![
                        I::large("comment", "New Comment", "comment"),
                        I::small("comment-next", "Next", "comment-next"),
                        I::small("comment-delete", "Delete", "comment-delete"),
                    ],
                ),
                Group::new(
                    "Tracking",
                    vec![I::large("track", "Track Changes", "track").desc("Record what is changed from now on.")],
                ),
                Group::new(
                    "Changes",
                    vec![
                        I::small("accept", "Accept", "accept"),
                        I::small("reject", "Reject", "reject"),
                        I::small("accept-all", "Accept All", "accept-all"),
                        I::small("reject-all", "Reject All", "reject-all"),
                    ],
                ),
            ],
        ),
        Tab::new(
            "view",
            "View",
            "W",
            vec![
                Group::new(
                    "Zoom",
                    vec![
                        I::large("zoom", "Zoom", "zoom").menu(),
                        I::small("zoom-in", "Zoom in", "zoom-in").key("Ctrl++"),
                        I::small("zoom-out", "Zoom out", "zoom-out").key("Ctrl+-"),
                        I::small("zoom-100", "100%", "zoom-100").key("Ctrl+0"),
                    ],
                ),
                Group::new(
                    "Show",
                    vec![
                        I::large("marks", "Marks", "marks"),
                        I::small("ruler", "Ruler", "ruler"),
                        I::small("nav", "Navigation", "nav"),
                    ],
                ),
                Group::new(
                    "Views",
                    vec![
                        I::large("view-print", "Print Layout", "view-print"),
                        I::small("read-mode", "Read Mode", "read-mode"),
                        I::small("view-draft", "Draft", "view-draft"),
                        I::small("view-web", "Web Layout", "view-web"),
                    ],
                ),
            ],
        ),
        Tab::new(
            "file-ops",
            "Document",
            "D",
            vec![Group::new(
                "Document",
                vec![
                    I::large("save", "Save", "save").key("Ctrl+S"),
                    I::small("saveas", "Save As", "save-as"),
                    I::small("pdf", "Export PDF", "pdf").key("Ctrl+P"),
                    I::small("open", "Open", "open").key("Ctrl+O"),
                    I::small("new", "New", "new").key("Ctrl+N"),
                    I::small("properties", "Properties", "properties"),
                    I::small("print", "Print", "print").key("Ctrl+Shift+P"),
                    I::small("accessibility", "Accessibility", "accessibility"),
                ],
            )],
        ),
    ]
}
