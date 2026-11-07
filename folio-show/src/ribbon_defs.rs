//! Folio Show's ribbon tabs.

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
                Group::new(
                    "Slides",
                    vec![
                        I::large("slide-new", "New Slide", "slide-add").desc("Add a slide after this one.").key("Ctrl+M"),
                        I::small("slide-duplicate", "Duplicate", "slide"),
                        I::small("slide-delete", "Delete", "slide-delete"),
                        I::small("layout-menu", "Layout", "layout").menu(),
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
                            I::icon("font-color", "Text colour", "font-color"),
                        ],
                    ],
                ),
                Group::rows(
                    "Paragraph",
                    vec![
                        vec![
                            I::icon("align-left", "Align left", "align-left"),
                            I::icon("align-center", "Centre", "align-center"),
                            I::icon("align-right", "Align right", "align-right"),
                        ],
                        vec![I::icon("bullets", "Bullets", "bullets")],
                    ],
                ),
                Group::new(
                    "Drawing",
                    vec![
                        I::large("shape-menu", "Shapes", "shapes").menu().desc("Draw a shape on the slide."),
                        I::small("textbox", "Text Box", "textbox"),
                        I::small("picture", "Picture", "picture"),
                        I::small("shape-delete", "Delete", "clear"),
                    ],
                ),
                Group::new("Editing", vec![I::large("find", "Find", "find").key("Ctrl+F")]),
            ],
        ),
        Tab::new(
            "insert",
            "Insert",
            "N",
            vec![
                Group::new(
                    "Slides",
                    vec![I::large("slide-new", "New Slide", "slide-add").key("Ctrl+M")],
                ),
                Group::new(
                    "Illustrations",
                    vec![
                        I::large("picture", "Picture", "picture").desc("Put a picture on the slide."),
                        I::small("shape-menu", "Shapes", "shapes").menu(),
                        I::small("table", "Table", "table"),
                    ],
                ),
                Group::new("Text", vec![I::large("textbox", "Text Box", "textbox")]),
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
                        vec![I::small("pane", "Slide pane", "sorter").desc("The slides down the side.")],
                        vec![I::small("notes", "Notes", "notes").desc("The notes under the slide.")],
                    ],
                ),
                Group::new(
                    "Start",
                    vec![I::large("present", "Present", "present").desc("Show the slides full screen (F5).").key("F5")],
                ),
                Group::new(
                    "Zoom",
                    vec![I::large("zoom-menu", "Zoom", "zoom").menu().desc("How large the slide is drawn.")],
                ),
            ],
        ),
    ]
}
