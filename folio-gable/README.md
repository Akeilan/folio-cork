# Folio Gable on the portable base

The 3D modeller, rewritten in Rust on the suite's own base: no Qt, no OpenGL.
It draws, it pushes and pulls, and it puts the pixels on the screen itself.

## What it is made of

| Crate | What it does |
| --- | --- |
| `crates/folio-gable-engine` | The maths, the model (vertices, edges, faces, groups, materials), the geometry (triangulation, picking, ray hits), the solids (box, polygon, push/pull, area, volume), the camera, and the renderer. |
| `crates/folio-kit` | The window furniture: the title bar, dialogs, menus, the file pane, text and icons. |
| `folio-gable` | The window: the tools down the side, the viewport, the measurements box and the status line. |

There is no GPU under it. The renderer keeps a depth for every pixel, fills the
triangles by hand with a light on them, and draws the edges a hair in front of
their own faces, so a drawing looks the same on every machine, down to the pixel.

## Running it

    folio-gable [FILE]              open a drawing
    folio-gable --png IN OUT        draw it to a picture
    folio-gable --export OUT.obj IN draw it out for other programs (.obj, .stl)
    folio-gable --ui-test [DIR]     drive the window as a person would
    folio-gable --ui-test DIR --headless    the same, in memory

## What works

Drawing on the ground and on faces: lines that close into faces, rectangles and
circles. Push/pull to give a face depth, with a typed distance in the
measurements box. Select, move, erase and paint. The tape measure. Orbit, pan,
zoom, zoom extents, and the Iso, Top and Front views, in perspective or parallel,
shaded or as wireframe. Undo and redo. Millimetres, centimetres, metres, inches
or feet. The drawing has its own file, and writes .obj and .stl for other
programs.

Not in this version yet: arcs and freehand, follow-me and offset, rotate and
scale, groups and components made in the window (the engine holds them), the
outliner and the materials browser, dimensions and text, scenes, section planes,
shadows, the sandbox tools, and importing from other modellers.

## Testing it

    cargo test -p folio-gable-engine       the model, the solids and the file
    folio-gable --ui-test DIR              the window test, on X or Wayland
    folio-gable --ui-test DIR --headless   the same without a display
