//! Draw's ribbon icons (24 x 24, stroked in the text colour; the accent fills highlights).

use folio_kit::icons::Icons;

const ICONS: &[(&str, &str)] = &[
    ("paste", "<rect x='6' y='5' width='12' height='16' rx='1.5'/><path d='M9 5V3.5h6V5'/><path d='M9 11h6M9 15h4'/>"),
    ("cut", "<circle cx='7' cy='17' r='2.6'/><circle cx='17' cy='17' r='2.6'/><path d='M8.8 15.2 17 4M15.2 15.2 7 4'/>"),
    ("copy", "<rect x='8' y='8' width='11' height='12' rx='1.5'/><path d='M5 15V5.5A1.5 1.5 0 0 1 6.5 4H15'/>"),
    ("select", "<rect x='4' y='5' width='16' height='13' stroke-dasharray='2.5 2'/><path d='M14 14l5 6 1-3 3-1z' fill='currentColor'/>"),
    ("crop", "<path d='M7 3v14h14M3 7h14v14'/>"),
    ("resize", "<rect x='4' y='9' width='11' height='11'/><path d='M12 4h8v8M20 4l-7 7'/>"),
    ("rotate", "<path d='M5 12a7 7 0 1 0 2.1-5'/><path d='M4 3v5h5'/>"),
    ("pencil", "<path d='M4 20l1.5-5L16 4.5l3.5 3.5L9 18.5z'/><path d='M14 6.5l3.5 3.5'/>"),
    ("fill", "<path d='M5 11l7-7 7 7-7 7z'/><path d='M19 14s2 2.6 2 4a2 2 0 0 1-4 0c0-1.4 2-4 2-4z' fill='currentColor'/>"),
    ("eraser", "<path d='M3 16l9-9 6 6-6 6H7z'/><path d='M9 10l6 6M12 19h9'/>"),
    ("picker", "<path d='M14 4l6 6-2 2-6-6z'/><path d='M13 7l-8 8-1 5 5-1 8-8'/>"),
    ("brush", "<path d='M14 4l6 6-7 7-6-6z'/><path d='M7 11l-2 2c-1.5 1.5-1 4-3 6 3 0 5.5-.5 7-2l2-2'/>"),
    ("line", "<path d='M5 19L19 5'/>"),
    ("rect", "<rect x='4' y='6' width='16' height='12'/>"),
    ("ellipse", "<ellipse cx='12' cy='12' rx='8.5' ry='6.5'/>"),
    ("shape-fill", "<rect x='4' y='6' width='16' height='12' fill='currentColor' fill-opacity='.35'/>"),
    ("size", "<path d='M4 6h16' stroke-width='1'/><path d='M4 11h16' stroke-width='2'/><path d='M4 17h16' stroke-width='3.5'/>"),
    ("color", "<circle cx='12' cy='12' r='8'/>"),
    ("zoom-in", "<circle cx='10.5' cy='10.5' r='6.5'/><path d='M15.5 15.5 20 20M8 10.5h5M10.5 8v5'/>"),
    ("zoom-out", "<circle cx='10.5' cy='10.5' r='6.5'/><path d='M15.5 15.5 20 20M8 10.5h5'/>"),
    ("zoom-100", "<rect x='4' y='4' width='16' height='16'/><path d='M8 9l2-1v8M14 12h0M16 12h0'/>"),
    ("zoom-fit", "<path d='M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5'/>"),
    ("undo", "<path d='M9 14L4 9l5-5'/><path d='M4 9h10a6 6 0 0 1 0 12h-3'/>"),
    ("redo", "<path d='M15 14l5-5-5-5'/><path d='M20 9H10a6 6 0 0 0 0 12h3'/>"),
    ("save", "<path d='M5 4h11l3 3v13H5z'/><path d='M8 4v5h7V4M8 20v-6h8v6'/>"),
    ("layer-add", "<path d='M12 4l8 4-8 4-8-4z'/><path d='M4 12l8 4 8-4M18 16v6M15 19h6'/>"),
    ("layers", "<path d='M12 4l8 4-8 4-8-4z'/><path d='M4 12l8 4 8-4M4 16l8 4 8-4'/>"),
    ("merge", "<path d='M12 4l8 4-8 4-8-4z'/><path d='M12 14v7M9 18l3 3 3-3'/>"),
    ("flatten", "<path d='M4 10h16M4 14h16'/><path d='M12 3v5M9 5l3 3 3-3M12 21v-5M9 19l3-3 3 3'/>"),
    ("text", "<path d='M5 6V4h14v2M12 4v16M9 20h6'/>"),
    ("adjust", "<circle cx='12' cy='12' r='3'/><path d='M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1'/>"),
    ("rulers", "<rect x='3' y='8' width='18' height='8' rx='1'/><path d='M7 8v3M11 8v4M15 8v3M19 8v4'/>"),
    ("grid", "<rect x='4' y='4' width='16' height='16'/><path d='M4 9.3h16M4 14.6h16M9.3 4v16M14.6 4v16'/>"),
    ("curve", "<path d='M4 18C8 4 14 22 20 6'/>"),
    ("polygon", "<path d='M12 4l8 6-3 9H7l-3-9z'/>"),
    ("frames", "<rect x='3' y='6' width='12' height='12'/><path d='M18 6v12M21 8v8'/>"),
    ("frame-prev", "<path d='M15 6l-6 6 6 6'/>"),
    ("frame-next", "<path d='M9 6l6 6-6 6'/>"),
    ("frame-play", "<path d='M8 5l11 7-11 7z'/>"),
    ("invert", "<circle cx='12' cy='12' r='8'/><path d='M12 4a8 8 0 0 1 0 16z' fill='currentColor'/>"),
];

pub fn load(icons: &mut Icons) {
    for (name, body) in ICONS {
        let svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' width='24' height='24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'>{body}</svg>"
        );
        icons.add(name, &svg);
    }
}

/// The app icon in full colour, for the window and the desktop entry.
pub fn app_icon_svg() -> &'static str {
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64' width='64' height='64'>\
<rect x='6' y='6' width='52' height='52' rx='8' fill='#800000'/>\
<rect x='12' y='12' width='40' height='40' rx='3' fill='#FFFFFF'/>\
<path d='M18 44c6-14 12-4 18-16s10-4 10-4' fill='none' stroke='#2E75B6' stroke-width='4' stroke-linecap='round'/>\
<circle cx='42' cy='40' r='6' fill='#F2B800'/></svg>"
}
