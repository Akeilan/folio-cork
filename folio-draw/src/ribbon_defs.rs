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
                        I::small("wallpaper", "Set as background", "wallpaper").desc("Make this picture the desktop's background."),
                        I::small("email", "Send in email", "email").menu().desc("A new message with the picture attached."),
                        I::small("properties", "Properties", "page").key("Ctrl+E").desc("The picture's size, its layers and the file it came from."),
                        I::small("remove-bg", "Remove background", "nobg").desc("Cut the subject out of its background, leaving it transparent. With a selection, only the selected part is cut out."),
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
                        vec![I::icon("tool.line", "Line", "line").key("L"), I::icon("tool.curve", "Curve", "curve").desc("Drag a line, then drag twice to bend it."), I::icon("tool.rect", "Rectangle", "rect").key("R")],
                        vec![I::icon("tool.ellipse", "Oval", "ellipse").key("O"), I::icon("tool.polygon", "Polygon", "polygon").desc("Click each corner; double-click, or click the first corner, to close it."), I::icon("shape.fill", "Fill shapes with colour 2", "shape-fill"), I::icon("shape.outline", "Draw the shape's outline", "shape-outline")],
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
                        I::small("full-screen", "Full screen", "zoom-fit").key("F11").desc("The picture on the whole screen; Esc or F11 comes back."),
                        I::small("print", "Print", "print").key("Ctrl+P").desc("Print the picture, laid out by Page setup."),
                        I::small("page-setup", "Page setup", "page").desc("Paper, orientation, margins and how big the picture goes on the page."),
                    ],
                ),
                Group::new(
                    "Layers",
                    vec![
                        I::large("pane.layers", "Layers", "layers").key("F7").desc("Show or hide the Layers pane."),
                        I::large("layer.add", "New layer", "layer-add").key("Ctrl+Shift+N"),
                        I::small("layer.props", "Layer properties", "layers"),
                        I::small("pane.history", "History", "undo").desc("The steps that can be undone, newest first."),
                        I::small("layer.merge", "Merge down", "merge"),
                        I::small("layer.flatten", "Flatten", "flatten"),
                    ],
                ),
                Group::new(
                    "Frames",
                    vec![
                        I::large("frames", "Frames view", "frames").desc("Show one layer at a time, as the frames of an animated GIF or WebP."),
                        I::small("frame.prev", "Previous frame", "frame-prev").key(","),
                        I::small("frame.next", "Next frame", "frame-next").key("."),
                        I::small("frame.play", "Play", "frame-play"),
                        I::small("frame.loop", "Plays", "frames").menu().desc("How many times a saved animated GIF or WebP plays."),
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
        Tab::new(
            "embroidery",
            "Embroidery",
            "M",
            vec![
                Group::new(
                    "Design",
                    vec![
                        I::large("emb.new", "New embroidery", "hoop").menu().desc("A new design the size of a hoop."),
                        I::small("emb.hoop", "Hoop and machine", "hoop").desc("The hoop size, machine and trims of this design."),
                        I::large("emb.import", "Import stitches", "emb-import").desc("Open a machine file (PES, DST, JEF, EXP, VP3, XXX, PEC) as a stitch object."),
                    ],
                ),
                Group::new(
                    "Picture",
                    vec![
                        I::large("emb.place", "Place picture", "place").desc("Put a picture in the hoop, fitted, on its own layer."),
                        I::large("emb.trace", "Convert to shapes", "trace").desc("Turn the picture into shapes, one layer per thread colour."),
                    ],
                ),
                Group::new(
                    "Shapes",
                    vec![
                        I::large("tool.emb", "Select shapes", "select").desc("Click a shape to select it; drag to move it; Delete takes it away."),
                        I::large("emb.recolour", "Thread colour", "color").menu(),
                        I::small("emb.holes", "Fill holes", "shape-fill").desc("Make the selected shape solid: its holes are stitched too."),
                        I::small("emb.merge", "Merge", "merge").desc("Make the shapes of this colour on this layer one shape where they touch."),
                        I::small("emb.split", "Split", "cut").desc("Break the selected shape into its separate parts, or draw a line across it to cut it in two."),
                        I::small("emb.nodes", "Edit nodes", "nodes").desc("Move, add (double-click an edge) or delete (Delete) the nodes of the selected shape, or the needle points of a stitch design. Double-clicking a shape does this too."),
                        I::large("emb.stitch", "Stitch properties", "stitches").desc("Stitch type, angle, density and underlay of the selected shape."),
                        I::small("emb.envelope", "Envelope", "envelope").desc("Bend the selected shapes or lettering: arch, bulge, pinch, wave or slant."),
                    ],
                ),
                Group::new("Lettering", vec![I::large("emb.letter", "Lettering", "lettering").desc("Letters from a font, straight or on an arc, as shapes that can be stitched.")]),
                Group::new(
                    "Stitches",
                    vec![
                        I::large("emb.view", "Stitch view", "stitches").desc("Show every planned stitch over the picture."),
                        I::large("emb.stitchout", "Stitch out", "frame-play").desc("Sew the design on screen, stitch by stitch, with the needle where the machine would be."),
                        I::large("emb.density", "Density", "density").desc("Colour the design by how many layers of thread cover it: green is fine, amber three layers, red four or more."),
                        I::large("emb.info", "Stitch information", "emb-info"),
                        I::large("emb.export", "Export stitches", "emb-export").menu().desc("Save the design as a file for the embroidery machine."),
                    ],
                ),
            ],
        ),
    ]
}
