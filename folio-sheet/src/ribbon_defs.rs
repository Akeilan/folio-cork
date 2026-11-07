//! Folio Sheet's ribbon tabs.

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
                        I::large("paste", "Paste", "paste").key("Ctrl+V"),
                        I::small("cut", "Cut", "cut").key("Ctrl+X"),
                        I::small("copy", "Copy", "copy").key("Ctrl+C"),
                    ],
                ),
                Group::rows(
                    "Font",
                    vec![
                        vec![I::combo("font", "Font", 130), I::combo("size", "Font size", 54)],
                        vec![
                            I::icon("bold", "Bold", "bold").key("Ctrl+B"),
                            I::icon("italic", "Italic", "italic").key("Ctrl+I"),
                            I::icon("underline", "Underline", "underline").key("Ctrl+U"),
                        ],
                    ],
                ),
                Group::rows(
                    "Alignment",
                    vec![
                        vec![I::icon("align-left", "Align left", "align-left"), I::icon("align-center", "Centre", "align-center"), I::icon("align-right", "Align right", "align-right")],
                        vec![I::icon("wrap", "Wrap Text", "wrap")],
                    ],
                ),
                Group::rows(
                    "Number",
                    vec![
                        vec![I::combo("numfmt-menu", "Number format", 140)],
                        vec![I::icon("dec-less", "Fewer decimals", "dec-less"), I::icon("dec-more", "More decimals", "dec-more")],
                    ],
                ),
                Group::rows(
                    "Cells",
                    vec![
                        vec![
                            I::icon("row-above", "Insert rows above", "row-above"),
                            I::icon("row-delete", "Delete the rows", "row-delete"),
                            I::icon("col-left", "Insert columns to the left", "col-left"),
                            I::icon("col-delete", "Delete the columns", "col-delete"),
                        ],
                        vec![
                            I::icon("sheet-new", "New sheet", "sheet-add"),
                            I::icon("sheet-delete", "Delete this sheet", "sheet-delete"),
                            I::icon("delete", "Clear the cells", "clear").key("Delete"),
                        ],
                    ],
                ),
                Group::new(
                    "Editing",
                    vec![
                        I::large("autosum", "AutoSum", "sum").desc("Add up the cells above or to the left."),
                        I::small("find", "Find", "find").key("Ctrl+F"),
                        I::small("replace", "Replace", "case").key("Ctrl+H"),
                    ],
                ),
            ],
        ),
        Tab::new(
            "view",
            "View",
            "W",
            vec![
                Group::rows(
                    "Show",
                    vec![
                        vec![I::small("gridlines", "Gridlines", "table").desc("Draw the lines between the cells.")],
                        vec![I::small("headings", "Headings", "ruler").desc("Show the row numbers and column letters.")],
                        vec![I::small("show-formulas", "Formulas", "symbol").desc("Show what was typed instead of the answers.")],
                    ],
                ),
                Group::new(
                    "Window",
                    vec![I::large("freeze", "Freeze Panes", "freeze").desc("Keep the rows above and the columns left of the cursor in view.")],
                ),
                Group::new(
                    "Zoom",
                    vec![I::large("zoom-menu", "Zoom", "zoom").menu().desc("How large the sheet is drawn.")],
                ),
            ],
        ),
    ]
}
