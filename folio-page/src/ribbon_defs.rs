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
                    ],
                ),
                Group::rows(
                    "Font",
                    vec![
                        vec![
                            I::combo("font", "Font", 150).desc("The font of the selected text."),
                            I::combo("size", "Font size", 56),
                        ],
                        vec![
                            I::icon("bold", "Bold", "bold").key("Ctrl+B"),
                            I::icon("italic", "Italic", "italic").key("Ctrl+I"),
                            I::icon("underline", "Underline", "underline").key("Ctrl+U"),
                            I::icon("strike", "Strikethrough", "strike"),
                            I::icon("font-color", "Font colour", "font-color").menu(),
                            I::icon("highlight", "Highlight", "highlight").menu(),
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
                        ],
                        vec![
                            I::icon("align-left", "Align left", "align-left"),
                            I::icon("align-center", "Centre", "align-center"),
                            I::icon("align-right", "Align right", "align-right"),
                            I::icon("align-justify", "Justify", "align-justify"),
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
                    I::large("margins", "Margins", "margins").menu(),
                    I::large("orientation", "Orientation", "orientation").menu(),
                    I::large("paper", "Size", "paper").menu(),
                ],
            )],
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
                Group::new("Show", vec![I::large("marks", "Marks", "marks")]),
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
                ],
            )],
        ),
    ]
}
