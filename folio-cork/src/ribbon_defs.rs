//! Cork's ribbon tabs.

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
                        I::large("paste", "Paste", "paste").key("Ctrl+V").desc("Paste shapes or a picture from the clipboard."),
                        I::small("cut", "Cut", "cut").key("Ctrl+X"),
                        I::small("copy", "Copy", "copy").key("Ctrl+C").desc("Copy the selection. Other Folio apps can paste it as a picture."),
                        I::small("painter", "Format Painter", "painter").desc("Copy the look of the selected shape, then click another shape to apply it."),
                    ],
                ),
                Group::rows(
                    "Font",
                    vec![
                        vec![I::combo("font", "Font", 130).desc("Font of the selected shapes' text."), I::combo("size", "Font size", 54), I::icon("grow", "Grow font", "grow"), I::icon("shrink", "Shrink font", "shrink")],
                        vec![
                            I::icon("bold", "Bold", "bold").key("Ctrl+B"),
                            I::icon("italic", "Italic", "italic").key("Ctrl+I"),
                            I::icon("underline", "Underline", "underline").key("Ctrl+U"),
                            I::icon("font-color", "Font colour", "font-color").menu(),
                        ],
                    ],
                ),
                Group::rows(
                    "Paragraph",
                    vec![
                        vec![I::icon("align-left", "Align left", "align-left"), I::icon("align-center", "Centre", "align-center"), I::icon("align-right", "Align right", "align-right")],
                        vec![I::icon("valign-top", "Top", "valign-top"), I::icon("valign-middle", "Middle", "valign-middle"), I::icon("valign-bottom", "Bottom", "valign-bottom")],
                    ],
                ),
                Group::new(
                    "Tools",
                    vec![
                        I::large("tool.pointer", "Pointer", "pointer").key("V").desc("Select, move and resize shapes."),
                        I::large("tool.connector", "Connector", "connector").key("C").desc("Draw lines that stay glued to shapes. Drag from one shape to another."),
                        I::large("tool.text", "Text", "text").key("T").desc("Click on the board to add a text box."),
                        I::small("tool.hand", "Pan", "hand").key("H, or hold Space"),
                    ],
                ),
                Group::new(
                    "Shape Styles",
                    vec![
                        I::large("quick", "Quick Styles", "quick").menu().desc("Colour sets for shapes and lines."),
                        I::small("fill", "Fill", "fill").split().desc("Fill colour of the selected shapes."),
                        I::small("line", "Line", "line").split().desc("Line colour of the selected shapes and connectors."),
                        I::small("weight", "Weight", "weight").menu(),
                        I::small("dash", "Dashes", "dash").menu(),
                        I::small("shadow", "Shadow", "shadow"),
                    ],
                ),
                Group::new(
                    "Arrange",
                    vec![
                        I::large("align-menu", "Align", "obj-center").menu().desc("Line the selected shapes up with each other."),
                        I::large("order-menu", "Position", "front").menu().desc("Bring shapes forward or send them back."),
                        I::small("group", "Group", "group").key("Ctrl+G"),
                        I::small("ungroup", "Ungroup", "ungroup").key("Ctrl+Shift+G"),
                        I::small("lock", "Lock", "lock").key("Ctrl+L").desc("Locked shapes can be selected but not moved, resized or deleted."),
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
                    "Shapes",
                    vec![
                        I::large("shapes-menu", "Shapes", "shapes").menu().desc("Insert a basic, flowchart, network, UML, floor plan or schematic shape."),
                        I::small("tool.shape.rect", "Rectangle", "rect").key("R"),
                        I::small("tool.shape.ellipse", "Ellipse", "ellipse").key("O"),
                        I::small("tool.shape.diamond", "Diamond", "diamond"),
                    ],
                ),
                Group::new(
                    "Board",
                    vec![
                        I::large("tool.sticky", "Sticky Note", "sticky").key("N").desc("Click to place a sticky note and type on it."),
                        I::large("tool.frame", "Frame", "frame").key("F").desc("Drag out a frame to group an area. Moving a frame moves what's inside."),
                        I::large("picture", "Picture", "picture").desc("Insert a picture from a file. You can also drop or paste pictures."),
                        I::small("tool.text", "Text Box", "text"),
                        I::small("tool.connector", "Connector", "connector"),
                    ],
                ),
                Group::new(
                    "Draw",
                    vec![
                        I::large("tool.pen", "Pen", "pen").split().key("P").desc("Draw freehand. The arrow picks colour and thickness."),
                        I::large("tool.highlighter", "Highlighter", "highlighter"),
                        I::large("tool.eraser", "Eraser", "eraser").key("E").desc("Erase pen and highlighter strokes."),
                    ],
                ),
                Group::new(
                    "Comments",
                    vec![I::large("tool.comment", "Comment", "comment").desc("Pin a comment to the board or a shape.")],
                ),
                Group::new(
                    "Pages",
                    vec![
                        I::large("page-new", "New Page", "page-add"),
                        I::small("page-dup", "Duplicate Page", "page-dup"),
                        I::small("page-mood", "Mood Board Page", "palette"),
                    ],
                ),
            ],
        ),
        Tab::new(
            "design",
            "Design",
            "G",
            vec![
                Group::new(
                    "Page Setup",
                    vec![
                        I::large("paper-menu", "Size", "page-size").menu().desc("Paper size used for printing and PDF export."),
                        I::large("orient-menu", "Orientation", "landscape").menu(),
                        I::large("background", "Background", "background").menu().desc("Board colour of this page."),
                    ],
                ),
                Group::new(
                    "Connectors",
                    vec![
                        I::small("route.elbow", "Right-angle", "route-elbow"),
                        I::small("route.straight", "Straight", "route-straight"),
                        I::small("route.curved", "Curved", "route-curved"),
                        I::small("arrow-b", "End arrow", "arrow-end").menu(),
                        I::small("arrow-a", "Start arrow", "arrow-start").menu(),
                        I::small("jumps", "Line jumps", "jumps").desc("Draw small hops where lines cross."),
                    ],
                ),
                Group::new(
                    "Layout",
                    vec![
                        I::large("layout-menu", "Re-Layout", "tree").menu().desc("Lay out a flowchart, tree or mind map in one click."),
                        I::large("orgchart", "Org Chart from File", "org").desc("Build an org chart from a CSV file or a spreadsheet with Name, Title and Reports To columns."),
                    ],
                ),
            ],
        ),
        Tab::new(
            "arrange",
            "Arrange",
            "A",
            vec![
                Group::rows(
                    "Align",
                    vec![
                        vec![I::icon("align.left", "Align left", "obj-left"), I::icon("align.center", "Align centre", "obj-center"), I::icon("align.right", "Align right", "obj-right")],
                        vec![I::icon("align.top", "Align top", "obj-top"), I::icon("align.middle", "Align middle", "obj-middle"), I::icon("align.bottom", "Align bottom", "obj-bottom")],
                        vec![I::icon("dist.h", "Distribute horizontally", "dist-h"), I::icon("dist.v", "Distribute vertically", "dist-v")],
                    ],
                ),
                Group::new(
                    "Order",
                    vec![
                        I::large("order.front", "Bring to Front", "front").key("Ctrl+Shift+]"),
                        I::large("order.back", "Send to Back", "back").key("Ctrl+Shift+["),
                        I::small("order.forward", "Bring Forward", "forward").key("Ctrl+]"),
                        I::small("order.backward", "Send Backward", "backward").key("Ctrl+["),
                    ],
                ),
                Group::new(
                    "Group",
                    vec![
                        I::large("group", "Group", "group").key("Ctrl+G"),
                        I::large("ungroup", "Ungroup", "ungroup").key("Ctrl+Shift+G"),
                    ],
                ),
                Group::new(
                    "Rotate",
                    vec![
                        I::small("rotate.right", "Rotate right 90°", "rotate"),
                        I::small("rotate.left", "Rotate left 90°", "rotate"),
                        I::small("flip.h", "Flip horizontal", "flip"),
                        I::small("flip.v", "Flip vertical", "flip"),
                    ],
                ),
                Group::new(
                    "Protect",
                    vec![
                        I::large("lock", "Lock", "lock").key("Ctrl+L"),
                        I::large("unlock", "Unlock", "unlock"),
                    ],
                ),
                Group::new(
                    "Layers",
                    vec![
                        I::large("layers-menu", "Layers", "layers").menu().desc("Put shapes on layers you can hide, lock or leave out of printing."),
                        I::small("layer-new", "New Layer", "plus"),
                    ],
                ),
            ],
        ),
        Tab::new(
            "board",
            "Board",
            "B",
            vec![
                Group::new(
                    "Templates",
                    vec![I::large("template-menu", "Templates", "template").menu().desc("Start a brainstorm, mind map, kanban, retrospective, timeline, user story map or mood board on this page.")],
                ),
                Group::new(
                    "Voting",
                    vec![
                        I::large("tool.vote", "Vote", "vote").desc("Click shapes to add a vote dot. Shift+click removes one."),
                        I::small("vote.results", "Show Results", "results"),
                        I::small("vote.reset", "Clear Votes", "rotate"),
                    ],
                ),
                Group::new(
                    "Mood Board",
                    vec![
                        I::large("mood.add", "Add Pictures", "picture"),
                        I::large("mood.palette", "Extract Palette", "palette").desc("Pick the main colours of the selected pictures as swatches, with their codes."),
                        I::small("tool.eyedropper", "Pick Colour", "eyedropper").desc("Click a picture to make a swatch of that colour."),
                        I::small("mood.font", "Font Sample", "font-sample"),
                        I::small("mood.swatch", "Swatch", "swatch"),
                        I::small("mood.polaroid", "Polaroid Frame", "polaroid"),
                        I::small("mood.crop", "Crop", "crop"),
                        I::small("mood.section", "Section", "section").desc("Put a frame around the selection."),
                        I::large("mood.grid", "Tidy Grid", "tidy-grid").desc("Arrange the selection, or the whole page, in a neat grid."),
                        I::large("mood.masonry", "Tidy Masonry", "tidy-masonry").desc("Arrange pictures in columns like a pinboard."),
                        I::small("mood.copy-palette", "Copy Palette", "copy").desc("Copy the hex codes of the selected swatches (or all on the page)."),
                    ],
                ),
                Group::new(
                    "Working Together",
                    vec![
                        I::large("share-host", "Share Board", "together").desc("Let others on your network join this board with a code. No cloud: computers connect directly."),
                        I::large("share-join", "Join Board", "share"),
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
                    "Show",
                    vec![
                        I::small("view.rulers", "Rulers", "rulers"),
                        I::small("view.grid", "Grid", "grid"),
                        I::small("view.snap", "Snap to Grid", "snap"),
                        I::small("view.guides", "Smart Guides", "guides"),
                        I::small("view.minimap", "Mini-map", "minimap"),
                        I::small("view.pagebreaks", "Page Breaks", "page-size"),
                    ],
                ),
                Group::new(
                    "Zoom",
                    vec![
                        I::large("zoom.fit", "Fit Page", "fit").key("Ctrl+0"),
                        I::large("zoom.sel", "Selection", "fit-sel"),
                        I::large("zoom.100", "100%", "zoom-100").key("Ctrl+1"),
                        I::small("zoom.in", "Zoom In", "zoom-in").key("Ctrl++"),
                        I::small("zoom.out", "Zoom Out", "zoom-out").key("Ctrl+-"),
                    ],
                ),
                Group::new(
                    "Panes",
                    vec![
                        I::large("pane.shapes", "Shapes", "stencil"),
                        I::large("pane.layers", "Layers", "layers"),
                        I::large("pane.comments", "Comments", "comments"),
                    ],
                ),
                Group::new("Theme", vec![I::large("theme-menu", "Light or Dark", "dark").menu()]),
            ],
        ),
    ]
}
