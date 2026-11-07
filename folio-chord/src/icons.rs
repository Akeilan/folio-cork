//! Chord's icons: 24-unit line drawings in the current colour.

use folio_kit::icons::Icons;

const ICONS: &[(&str, &str)] = &[
    ("play", "<path d='M8 5l12 7-12 7z' fill='currentColor' stroke='none'/>"),
    ("pause", "<path d='M8 5h4v14H8zM14 5h4v14h-4z' fill='currentColor' stroke='none'/>"),
    ("prev", "<path d='M8 12l10-7v14z' fill='currentColor' stroke='none'/><path d='M6 5v14' stroke-width='2'/>"),
    ("next", "<path d='M16 12L6 5v14z' fill='currentColor' stroke='none'/><path d='M18 5v14' stroke-width='2'/>"),
    ("shuffle", "<path d='M3 7h4l10 10h4M3 17h4l3-3M14 10l3-3h4'/><path d='M18 4l3 3-3 3M18 14l3 3-3 3'/>"),
    ("repeat", "<path d='M4 12V9a3 3 0 0 1 3-3h10M20 12v3a3 3 0 0 1-3 3H7'/><path d='M14 3l3 3-3 3M10 15l-3 3 3 3'/>"),
    ("volume", "<path d='M4 9h4l5-4v14l-5-4H4z' fill='currentColor' stroke='none'/><path d='M16 9a4 4 0 0 1 0 6M19 6a8 8 0 0 1 0 12'/>"),
    ("mute", "<path d='M4 9h4l5-4v14l-5-4H4z' fill='currentColor' stroke='none'/><path d='M16 9l6 6M22 9l-6 6' stroke-width='2'/>"),
    ("music", "<path d='M9 18V5l10-2v13'/><circle cx='6.5' cy='18' r='2.6'/><circle cx='16.5' cy='16' r='2.6'/>"),
    ("playlist", "<path d='M4 7h10M4 12h10M4 17h6'/><circle cx='17' cy='17' r='2.4'/><path d='M19.4 17V9'/>"),
    ("playlist-add", "<path d='M4 7h10M4 12h10M4 17h6'/><path d='M17 12v8M13 16h8' stroke-width='2'/>"),
    ("smart", "<path d='M12 3l2.2 5.3L20 9l-4 3.8L17 19l-5-2.8L7 19l1-6.2L4 9l5.8-.7z'/>"),
    ("star", "<path d='M12 3l2.6 6.2 6.4.6-4.8 4.5 1.4 6.7L12 17.6 6.4 21l1.4-6.7L3 9.8l6.4-.6z' fill='currentColor' stroke='none'/>"),
    ("star-empty", "<path d='M12 3l2.6 6.2 6.4.6-4.8 4.5 1.4 6.7L12 17.6 6.4 21l1.4-6.7L3 9.8l6.4-.6z'/>"),
    ("search", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21'/>"),
    ("folder", "<path d='M3 6h6l2 2h10v11H3z'/>"),
    ("info", "<circle cx='12' cy='12' r='9'/><path d='M12 11v6M12 7.5v.01' stroke-width='2'/>"),
    ("check", "<path d='M5 12l5 5 9-10' stroke-width='2.2'/>"),
    ("chevron-down", "<path d='M6 9l6 6 6-6' stroke-width='2'/>"),
    ("chevron-right", "<path d='M9 6l6 6-6 6' stroke-width='2'/>"),
    ("win-min", "<path d='M5 12h14'/>"),
    ("win-max", "<rect x='5' y='5' width='14' height='14'/>"),
    ("win-restore", "<rect x='5' y='8' width='11' height='11'/><path d='M8 8V5h11v11h-3'/>"),
    ("win-close", "<path d='M6 6l12 12M18 6L6 18'/>"),
    ("trash", "<path d='M5 7h14M10 7V5h4v2M7 7l1 13h8l1-13'/>"),
    ("rescan", "<path d='M4 12a8 8 0 0 1 13.7-5.7M20 12a8 8 0 0 1-13.7 5.7'/><path d='M17 3v4h-4M7 21v-4h4'/>"),
    ("eq", "<path d='M6 20V10M12 20V4M18 20v-7'/><circle cx='6' cy='7' r='2'/><circle cx='12' cy='15' r='2'/><circle cx='18' cy='10' r='2'/>"),
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
     <rect x='4' y='4' width='56' height='56' rx='12' fill='#C7007F'/>\
     <path d='M26 44V20l18-4v22' fill='none' stroke='#FFFFFF' stroke-width='4' stroke-linejoin='round'/>\
     <circle cx='22' cy='44' r='6' fill='#FFFFFF'/>\
     <circle cx='40' cy='40' r='6' fill='#FFFFFF'/>\
     </svg>"
}
