//! Reel's icons: 24-unit line drawings in the current colour.

use folio_kit::icons::Icons;

const ICONS: &[(&str, &str)] = &[
    ("add-media", "<path d='M3 5h12v10H3z'/><path d='M15 9l6-3v10l-6-3'/><path d='M9 7v6M6 10h6' stroke-width='1.4'/>"),
    ("track-video", "<rect x='3' y='6' width='18' height='12' rx='1.5'/><path d='M10 9l5 3-5 3z' fill='currentColor' stroke='none'/>"),
    ("track-audio", "<path d='M4 12h3l4-6v12l-4-6H4z'/><path d='M16 9a4 4 0 0 1 0 6'/>"),
    ("split", "<path d='M12 3v18' stroke-dasharray='2 2'/><path d='M4 7h6v10H4zM14 7h6v10h-6z'/>"),
    ("delete", "<path d='M5 7h14M9 7V4h6v3M7 7l1 14h8l1-14'/>"),
    ("ripple", "<path d='M4 7h5v10H4z'/><path d='M12 12h8M16 8l4 4-4 4'/>"),
    ("trim-start", "<path d='M8 4v16' stroke-width='2'/><path d='M11 7h9v10h-9z'/>"),
    ("trim-end", "<path d='M16 4v16' stroke-width='2'/><path d='M4 7h9v10H4z'/>"),
    ("group", "<rect x='3' y='4' width='8' height='7'/><rect x='13' y='13' width='8' height='7'/><path d='M11 8h2v7' stroke-dasharray='2 2'/>"),
    ("title", "<path d='M4 6h16M12 6v13' stroke-width='1.8'/><path d='M9 19h6'/>"),
    ("lower-third", "<rect x='3' y='5' width='18' height='14' rx='1.5'/><path d='M5 15h8' stroke-width='2'/><path d='M5 17.5h5' stroke-width='1.2'/>"),
    ("caption", "<rect x='3' y='5' width='18' height='14' rx='1.5'/><path d='M7 16h10' stroke-width='2'/>"),
    ("subtitles", "<rect x='3' y='5' width='18' height='14' rx='1.5'/><path d='M6 13h5M13 13h5M6 16h8' stroke-width='1.3'/>"),
    ("play", "<path d='M7 4l13 8-13 8z' fill='currentColor' stroke='none'/>"),
    ("pause", "<path d='M7 4h4v16H7zM13 4h4v16h-4z' fill='currentColor' stroke='none'/>"),
    ("to-start", "<path d='M6 4v16' stroke-width='2'/><path d='M20 4L9 12l11 8z' fill='currentColor' stroke='none'/>"),
    ("to-end", "<path d='M18 4v16' stroke-width='2'/><path d='M4 4l11 8L4 20z' fill='currentColor' stroke='none'/>"),
    ("marker", "<path d='M6 3v18' stroke-width='1.6'/><path d='M6 4h12l-3 4 3 4H6z' fill='currentColor' stroke='none'/>"),
    ("next-marker", "<path d='M5 4v16'/><path d='M10 12h9M15 8l4 4-4 4'/>"),
    ("prev-marker", "<path d='M19 4v16'/><path d='M14 12H5M9 8l-4 4 4 4'/>"),
    ("mute", "<path d='M4 12h3l4-6v12l-4-6H4z'/><path d='M15 9l5 6M20 9l-5 6'/>"),
    ("music-bed", "<path d='M9 17V6l9-2v11'/><circle cx='7' cy='17' r='2'/><circle cx='16' cy='15' r='2'/>"),
    ("panzoom", "<rect x='3' y='5' width='18' height='14' rx='1.5'/><rect x='8' y='9' width='8' height='6' stroke-dasharray='2 2'/><path d='M12 9V5M12 19v-4' stroke-width='1.2'/>"),
    ("chroma", "<rect x='3' y='5' width='18' height='14' rx='1.5'/><path d='M12 9a3 3 0 1 0 0 6' /><path d='M15 8l4 8' stroke-width='1.2'/>"),
    ("export", "<path d='M12 3v11M8 10l4 4 4-4'/><path d='M4 17v3h16v-3'/>"),
    ("poster", "<rect x='3' y='5' width='18' height='14' rx='1.5'/><circle cx='9' cy='10' r='2'/><path d='M3 17l5-4 4 3 3-2 6 5'/>"),
    ("size", "<rect x='3' y='6' width='18' height='12'/><path d='M7 10h4v4H7z' stroke-dasharray='2 2'/>"),
    ("zoom-in", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21M8 11h6M11 8v6'/>"),
    ("zoom-out", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21M8 11h6'/>"),
    ("zoom-fit", "<path d='M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5'/>"),
    ("snap", "<path d='M12 3v18' stroke-dasharray='3 3'/><path d='M4 8h6v8H4z'/><path d='M14 8h6v8h-6z'/>"),
    ("wave", "<path d='M3 12h2M7 7v10M11 9v6M15 5v14M19 10v4'/>"),
    ("bin", "<rect x='3' y='4' width='18' height='16' rx='1.5'/><path d='M9 4v16M3 9h6M3 14h6'/>"),
    ("props", "<path d='M4 6h16M4 12h16M4 18h16'/><circle cx='9' cy='6' r='2' fill='currentColor' stroke='none'/><circle cx='15' cy='12' r='2' fill='currentColor' stroke='none'/><circle cx='8' cy='18' r='2' fill='currentColor' stroke='none'/>"),
    ("dark", "<path d='M20 14a8 8 0 1 1-10-10 7 7 0 0 0 10 10z'/>"),
    ("paste", "<path d='M8 4h8v3H8z'/><path d='M6 6h2M16 6h2v14H6V6'/><path d='M9 12h6M9 16h6' stroke-width='1.2'/>"),
    ("cut", "<circle cx='6' cy='18' r='2.5'/><circle cx='18' cy='18' r='2.5'/><path d='M8 16L18 4M16 16L6 4'/>"),
    ("copy", "<rect x='4' y='4' width='11' height='11'/><path d='M9 20h11V9' stroke-dasharray='2 2'/>"),
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
     <rect x='4' y='4' width='56' height='56' rx='12' fill='#F0F0EE' stroke='#1F3864' stroke-width='3'/>\
     <rect x='12' y='18' width='40' height='28' rx='3' fill='#1F3864'/>\
     <path d='M12 24h40M12 40h40' stroke='#F0F0EE' stroke-width='2'/>\
     <path d='M27 28l12 6-12 6z' fill='#F0F0EE'/>\
     <path d='M18 18v6M30 18v6M42 18v6M18 40v6M30 40v6M42 40v6' stroke='#F0F0EE' stroke-width='2'/>\
     </svg>"
}
