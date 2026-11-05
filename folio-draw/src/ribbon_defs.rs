//! Draw's ribbon tabs.

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
                        I::large("paste", "Paste", "paste").split().key("Ctrl+V").desc("Paste a picture from the clipboard; drag it into place."),
                        I::small("cut", "Cut", "cut").key("Ctrl+X"),
                        I::small("copy", "Copy", "copy").key("Ctrl+C").desc("Copy the selection, or the whole picture."),
                    ],
                ),
                Group::new(
                    "Image",
                    vec![
                        I::large("tool.select", "Select", "select").split().key("S").desc("Drag out a rectangle. Drag inside it to move what it holds. Shift adds, Alt takes away."),
                        I::small("crop", "Crop", "crop").key("Ctrl+Shift+X").desc("Trim the picture to the selection."),
                        I::small("resize", "Resize", "resize").key("Ctrl+W"),
                        I::small("rotate", "Rotate", "rotate").menu(),
                    ],
                ),
                Group::rows(
                    "Tools",
                    vec![
                        vec![I::icon("tool.pencil", "Pencil", "pencil").key("P"), I::icon("tool.fill", "Fill with colour", "fill").key("F"), I::icon("tool.zoom", "Magnifier", "zoom-in").key("M").desc("Click to zoom in, right-click to zoom out.")],
                        vec![I::icon("tool.eraser", "Eraser", "eraser").key("E"), I::icon("tool.picker", "Colour picker", "picker").key("I"), I::icon("tool.text", "Text", "text").key("T")],
                    ],
                ),
                Group::new("Brushes", vec![I::large("tool.brush", "Brushes", "brush").split().key("B").desc("Paint with soft edges, a calligraphy nib, an airbrush or a marker.")]),
                Group::rows(
                    "Shapes",
                    vec![
                        vec![I::icon("tool.line", "Line", "line").key("L"), I::icon("tool.rect", "Rectangle", "rect").key("R"), I::icon("tool.ellipse", "Oval", "ellipse").key("O")],
                        vec![I::icon("shape.fill", "Fill shapes with colour 2", "shape-fill")],
                    ],
                ),
                Group::new("Size", vec![I::large("size", "Size", "size").menu()]),
                Group::new(
                    "Colours",
                    vec![
                        I::large("color1", "Colour 1", "color").menu().desc("The colour pencils, brushes and outlines use (left button)."),
                        I::large("color2", "Colour 2", "color").menu().desc("The colour the right button paints with, and shapes are filled with."),
                    ],
                ),
            ],
        ),
        Tab::new(
            "view",
            "View",
            "V",
            vec![
                Group::new(
                    "Zoom",
                    vec![
                        I::large("zoom.in", "Zoom in", "zoom-in").key("Ctrl+Plus"),
                        I::large("zoom.out", "Zoom out", "zoom-out").key("Ctrl+Minus"),
                        I::large("zoom.100", "100%", "zoom-100").key("Ctrl+1"),
                        I::large("zoom.fit", "Fit to window", "zoom-fit").key("Ctrl+0"),
                    ],
                ),
                Group::new(
                    "Show",
                    vec![
                        I::small("view.rulers", "Rulers", "rulers").key("Ctrl+R"),
                        I::small("view.grid", "Gridlines", "grid").key("Ctrl+G").desc("A line round every pixel, from 200% up."),
                    ],
                ),
                Group::new(
                    "Layers",
                    vec![
                        I::large("pane.layers", "Layers", "layers").key("F7").desc("Show or hide the Layers pane."),
                        I::large("layer.add", "New layer", "layer-add").key("Ctrl+Shift+N"),
                        I::small("layer.props", "Layer properties", "layers"),
                        I::small("layer.merge", "Merge down", "merge"),
                        I::small("layer.flatten", "Flatten", "flatten"),
                    ],
                ),
                Group::new(
                    "Adjust",
                    vec![
                        I::large("adjust", "Adjustments", "adjust").menu().desc("Brightness and contrast, hue and saturation, greyscale, sepia, blur and sharpen; on the selection when there is one."),
                        I::large("invert", "Invert colours", "invert").key("Ctrl+Shift+I"),
                    ],
                ),
            ],
        ),
    ]
}
