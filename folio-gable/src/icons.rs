//! Gable's icons: 24-unit line drawings in the current colour.

use folio_kit::icons::Icons;

const ICONS: &[(&str, &str)] = &[
    ("select", "<path d='M5 3l14 8-6 1.5L10 19z' fill='currentColor' stroke='none'/>"),
    ("line", "<path d='M4 20L20 4' stroke-width='1.8'/><circle cx='4' cy='20' r='2'/><circle cx='20' cy='4' r='2'/>"),
    ("rect", "<rect x='4' y='6' width='16' height='12'/><circle cx='4' cy='6' r='1.8'/><circle cx='20' cy='18' r='1.8'/>"),
    ("circle", "<circle cx='12' cy='12' r='8'/><circle cx='12' cy='12' r='1.6' fill='currentColor'/>"),
    ("arc", "<path d='M4 18a10 10 0 0 1 16 0'/><circle cx='4' cy='18' r='1.8'/><circle cx='20' cy='18' r='1.8'/>"),
    ("pushpull", "<path d='M4 14l8-4 8 4-8 4z'/><path d='M12 10V3M9 6l3-3 3 3' stroke-width='1.8'/>"),
    ("move", "<path d='M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3'/>"),
    ("rotate", "<path d='M20 12a8 8 0 1 1-3-6.2'/><path d='M20 4v5h-5'/>"),
    ("scale", "<rect x='4' y='4' width='11' height='11'/><path d='M9 20h11V9' stroke-dasharray='2 2'/><path d='M15 15l5 5' stroke-width='1.8'/>"),
    ("eraser", "<path d='M6 18l-3-3 9-9 6 6-6 6z'/><path d='M9 21h12'/>"),
    ("paint", "<path d='M5 10V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v4H5z'/><path d='M11 10v4h4v4a2 2 0 1 1-4 0'/>"),
    ("tape", "<path d='M3 8h18v8H3z'/><path d='M7 8v4M11 8v3M15 8v4M19 8v3'/>"),
    ("orbit", "<circle cx='12' cy='12' r='4'/><ellipse cx='12' cy='12' rx='10' ry='4'/>"),
    ("pan", "<path d='M12 3v18M3 12h18' stroke-dasharray='3 3'/><circle cx='12' cy='12' r='3'/>"),
    ("zoom", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21M8 11h6M11 8v6'/>"),
    ("zoom-extents", "<circle cx='11' cy='11' r='5'/><path d='M15 15l6 6'/><path d='M3 7V3h4M21 7V3h-4' stroke-width='1.4'/>"),
    ("iso", "<path d='M12 3l8 4.5v9L12 21l-8-4.5v-9z'/><path d='M12 12l8-4.5M12 12v9M12 12L4 7.5'/>"),
    ("top", "<rect x='4' y='4' width='16' height='16'/><path d='M4 9h16M9 4v16' stroke-width='1'/>"),
    ("front", "<rect x='4' y='8' width='16' height='10'/><path d='M4 8l4-4h16l-4 4M20 8v10l-4 4' stroke-width='1'/>"),
    ("wire", "<path d='M12 3l8 4.5v9L12 21l-8-4.5v-9z'/><path d='M4 7.5l8 4.5 8-4.5M12 12v9'/>"),
    ("outliner", "<path d='M4 6h16M8 11h12M8 16h12'/><circle cx='5' cy='11' r='1.3' fill='currentColor'/><circle cx='5' cy='16' r='1.3' fill='currentColor'/>"),
    ("group", "<rect x='3' y='3' width='9' height='9'/><rect x='12' y='12' width='9' height='9'/>"),
    ("undo", "<path d='M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11'/>"),
    ("redo", "<path d='M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 0 0 0 11H13'/>"),
    ("save", "<path d='M5 3h11l3 3v15H5zM8 3v5h8V3M8 21v-7h8v7'/>"),
    ("open", "<path d='M3 7h6l2 2h8v3M3 19V7M3 19l3-7h16l-3 7z'/>"),
    ("new", "<path d='M6 3h8l4 4v14H6zM14 3v4h4M12 11v6M9 14h6'/>"),
    ("win-min", "<path d='M5 12h14'/>"),
    ("win-max", "<rect x='5' y='5' width='14' height='14'/>"),
    ("win-restore", "<rect x='5' y='8' width='11' height='11'/><path d='M8 8V5h11v11h-3'/>"),
    ("win-close", "<path d='M6 6l12 12M18 6L6 18'/>"),
];

pub fn install(icons: &mut Icons) {
    for (name, body) in ICONS {
        icons.add(name, &wrap(body));
    }
}

fn wrap(body: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' \
         stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'>{body}</svg>"
    )
}

/// The app's own icon, for the window and the desktop entry.
pub fn app_icon_svg() -> &'static str {
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'>\
     <rect x='4' y='4' width='56' height='56' rx='12' fill='#F0F0EE' stroke='#8C6A3F' stroke-width='3'/>\
     <path d='M14 34l18-14 18 14v18H14z' fill='#C8A06A' stroke='#5E4324' stroke-width='3' stroke-linejoin='round'/>\
     <path d='M32 20v32M14 34h36' stroke='#5E4324' stroke-width='2'/>\
     </svg>"
}
