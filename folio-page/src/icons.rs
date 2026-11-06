//! Page's icons: 24-unit line drawings in the current colour.

use folio_kit::icons::Icons;

const ICONS: &[(&str, &str)] = &[
    ("page", "<path d='M6 3h8l4 4v14H6zM14 3v4h4M9 12h6M9 16h6'/>"),
    ("check", "<path d='M5 12l5 5 9-10' stroke-width='2.2'/>"),
    ("chevron-right", "<path d='M9 6l6 6-6 6' stroke-width='2'/>"),
    ("chevron-down", "<path d='M6 9l6 6 6-6' stroke-width='2'/>"),
    ("chevron-up", "<path d='M6 15l6-6 6 6' stroke-width='2'/>"),
    ("back", "<path d='M15 5l-7 7 7 7' stroke-width='2'/>"),
    ("arrow-up", "<path d='M12 19V5M6 11l6-6 6 6'/>"),
    ("folder", "<path d='M3 6h6l2 2h10v11H3z' fill='var(--accent)' fill-opacity='.35'/>"),
    ("file", "<path d='M6 3h8l4 4v14H6zM14 3v4h4'/>"),
    ("win-min", "<path d='M5 12h14'/>"),
    ("win-max", "<rect x='5' y='5' width='14' height='14'/>"),
    ("win-restore", "<rect x='5' y='8' width='11' height='11'/><path d='M8 8V5h11v11h-3'/>"),
    ("win-close", "<path d='M6 6l12 12M18 6L6 18'/>"),
    ("new", "<path d='M6 3h8l4 4v14H6zM14 3v4h4M12 11v6M9 14h6'/>"),
    ("open", "<path d='M3 7h6l2 2h8v3M3 19V7M3 19l3-7h16l-3 7z'/>"),
    ("save", "<path d='M5 3h11l3 3v15H5zM8 3v5h8V3M8 21v-7h8v7'/>"),
    ("save-as", "<path d='M5 3h11l3 3v6M5 3v18h6M8 3v5h8V3M14 21l1-3 5-5 2 2-5 5z'/>"),
    ("pdf", "<path d='M6 3h8l4 4v14H6zM14 3v4h4'/><path d='M9 17v-5h2a1.5 1.5 0 0 1 0 3H9M14 17v-5h2' stroke-width='1.4'/>"),
    ("undo", "<path d='M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 0 1 0 11H11'/>"),
    ("redo", "<path d='M15 14l5-5-5-5M20 9H9.5a5.5 5.5 0 0 0 0 11H13'/>"),
    ("cut", "<circle cx='6' cy='18' r='2.6'/><circle cx='18' cy='18' r='2.6'/><path d='M8 16L18 4M16 16L6 4'/>"),
    ("copy", "<rect x='8' y='8' width='12' height='12' rx='1.5'/><path d='M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3'/>"),
    ("paste", "<path d='M9 3h6v3H9zM8 5H5v16h14V5h-3'/><path d='M8 11h8M8 15h5'/>"),
    ("bold", "<path d='M7 5h6a3.5 3.5 0 0 1 0 7H7zM7 12h7a3.5 3.5 0 0 1 0 7H7z' stroke-width='2.4'/>"),
    ("italic", "<path d='M10 5h8M6 19h8M15 5L9 19'/>"),
    ("underline", "<path d='M7 4v7a5 5 0 0 0 10 0V4M5 20h14'/>"),
    ("strike", "<path d='M7 6a4 4 0 0 1 8 0M9 18a4 4 0 0 0 8-1c0-1.5-1-2.5-3-3M4 12h16'/>"),
    ("font-color", "<path d='M7 16L12 4l5 12M9 12h6'/><rect x='5' y='19' width='14' height='3' fill='var(--accent)'/>"),
    ("highlight", "<path d='M8 13l7-9 5 5-9 7z' fill='#FFE94D'/><path d='M9 15l-4 5h5l2-2'/>"),
    ("align-left", "<path d='M4 6h16M4 10h10M4 14h16M4 18h10'/>"),
    ("align-center", "<path d='M4 6h16M7 10h10M4 14h16M7 18h10'/>"),
    ("align-right", "<path d='M4 6h16M10 10h10M4 14h16M10 18h10'/>"),
    ("align-justify", "<path d='M4 6h16M4 10h16M4 14h16M4 18h16'/>"),
    ("bullets", "<circle cx='5' cy='7' r='1.4' fill='currentColor'/><circle cx='5' cy='12' r='1.4' fill='currentColor'/><circle cx='5' cy='17' r='1.4' fill='currentColor'/><path d='M9 7h11M9 12h11M9 17h11'/>"),
    ("numbering", "<path d='M4 6h1v4M4 10h2M4 14h2l-2 3h2M9 7h11M9 12h11M9 17h11' stroke-width='1.4'/>"),
    ("indent", "<path d='M4 6h16M10 10h10M10 14h10M4 18h16M4 10l3 2-3 2z' fill='currentColor'/>"),
    ("outdent", "<path d='M4 6h16M10 10h10M10 14h10M4 18h16M7 10l-3 2 3 2z' fill='currentColor'/>"),
    ("marks", "<path d='M14 4v16M10 4v16M14 4H9a4 4 0 0 0 0 8h5' stroke-width='1.6'/>"),
    ("styles", "<path d='M4 6h16M4 11h10M4 16h13' stroke-width='1.6'/><path d='M17 14l2-2 2 2-2 2z' fill='var(--accent)'/>"),
    ("select-all", "<rect x='4' y='4' width='16' height='16' stroke-dasharray='3 2'/><path d='M8 12h8M12 8v8'/>"),
    ("break-page", "<path d='M6 3h8l4 4v5H6zM14 3v4h4M6 16h12M6 21h12' stroke-dasharray='3 2'/>"),
    ("break-column", "<path d='M4 4h7v16H4zM13 4h7v16h-7z' stroke-dasharray='3 2'/>"),
    ("table", "<rect x='3' y='5' width='18' height='14'/><path d='M3 10h18M3 14.5h18M9 5v14M15 5v14'/>"),
    ("picture", "<rect x='3' y='5' width='18' height='14' rx='2'/><circle cx='9' cy='10' r='1.8'/><path d='M21 16l-5-5-9 8'/>"),
    ("header", "<rect x='4' y='3' width='16' height='18'/><path d='M4 8h16' fill='var(--accent)'/><path d='M7 12h10M7 16h7'/>"),
    ("footer", "<rect x='4' y='3' width='16' height='18'/><path d='M4 16h16'/><path d='M7 7h10M7 11h7'/>"),
    ("page-number", "<rect x='4' y='3' width='16' height='18'/><path d='M9 17h6M12 14v3' stroke-width='1.4'/>"),
    ("body", "<path d='M6 3h12v18H6zM9 8h6M9 12h6M9 16h3'/>"),
    ("margins", "<rect x='3' y='3' width='18' height='18'/><rect x='7' y='7' width='10' height='10' stroke-dasharray='3 2'/>"),
    ("orientation", "<rect x='4' y='3' width='9' height='13'/><rect x='11' y='12' width='9' height='7'/><path d='M14 8l3-3 3 3'/>"),
    ("paper", "<rect x='5' y='3' width='14' height='18'/><path d='M8 8h8M8 12h8M8 16h5'/>"),
    ("row-below", "<rect x='3' y='5' width='18' height='14'/><path d='M3 12h18M9 5v14M15 5v14'/><path d='M3 12h18v7H3z' fill='var(--accent)' fill-opacity='.3'/>"),
    ("row-above", "<rect x='3' y='5' width='18' height='14'/><path d='M3 12h18M9 5v14M15 5v14'/><path d='M3 5h18v7H3z' fill='var(--accent)' fill-opacity='.3'/>"),
    ("col-right", "<rect x='3' y='5' width='18' height='14'/><path d='M12 5v14M3 12h18'/><path d='M12 5h9v14h-9z' fill='var(--accent)' fill-opacity='.3'/>"),
    ("col-left", "<rect x='3' y='5' width='18' height='14'/><path d='M12 5v14M3 12h18'/><path d='M3 5h9v14H3z' fill='var(--accent)' fill-opacity='.3'/>"),
    ("row-delete", "<rect x='3' y='5' width='18' height='14'/><path d='M3 12h18M9 5v14M15 5v14'/><path d='M6 15l12-6M6 9l12 6' stroke-width='1.4'/>"),
    ("col-delete", "<rect x='3' y='5' width='18' height='14'/><path d='M12 5v14M3 12h18'/><path d='M6 16l12-8M6 8l12 8' stroke-width='1.4'/>"),
    ("merge", "<rect x='3' y='5' width='18' height='14'/><path d='M12 5v3M12 16v3'/><path d='M7 12h10M9 10l-2 2 2 2M15 10l2 2-2 2' stroke-width='1.4'/>"),
    ("table-delete", "<rect x='3' y='5' width='18' height='14'/><path d='M3 10h18M9 5v14'/><path d='M7 7l10 10M17 7L7 17' stroke-width='1.4'/>"),
    ("borders", "<rect x='3' y='5' width='18' height='14' stroke-width='2'/><path d='M3 12h18M12 5v14' stroke-dasharray='2 2'/>"),
    ("line-spacing", "<path d='M9 5h12M9 12h12M9 19h12'/><path d='M4 7l2-2 2 2M4 17l2 2 2-2M6 5v14' stroke-width='1.3'/>"),
    ("case", "<path d='M3 18l4-10 4 10M4.2 15h5.6M14 18V8h3a3 3 0 0 1 0 6h-3' stroke-width='1.5'/>"),
    ("clear-format", "<path d='M6 5h12M11 5v14M8 19h6'/><path d='M15 14l6 6M21 14l-6 6' stroke-width='1.4'/>"),
    ("painter", "<path d='M4 4h12v5H4zM16 6h3v6h-8v3M10 15h2v6h-2z'/>"),
    ("view-print", "<rect x='5' y='3' width='14' height='18'/><path d='M8 7h8M8 11h8M8 15h5'/>"),
    ("view-draft", "<path d='M4 5h16M4 9h16M4 13h16M4 17h10'/>"),
    ("view-web", "<rect x='3' y='5' width='18' height='14' rx='2'/><path d='M3 9h18M7 7h.01M10 7h.01'/>"),
    ("toc", "<path d='M4 5h16M4 9h11M4 13h16M4 17h9'/><path d='M18 9h2M18 17h2' stroke-width='1.2'/>"),
    ("update-fields", "<path d='M20 11a8 8 0 1 0-2.3 5.6M20 6v5h-5'/>"),
    ("footnote", "<path d='M6 4h12M6 8h12M6 12h8'/><path d='M6 18h8M6 21h5' stroke-width='1.2'/><path d='M4 16h16' stroke-dasharray='2 2'/>"),
    ("endnote", "<path d='M6 4h12M6 8h12M6 12h8M6 20h10'/><path d='M4 16h16' stroke-dasharray='2 2'/>"),
    ("comment", "<path d='M4 5h16v11H9l-5 4z'/>"),
    ("comment-next", "<path d='M4 5h16v11H9l-5 4z'/><path d='M10 10h5M13 8l2 2-2 2' stroke-width='1.4'/>"),
    ("comment-delete", "<path d='M4 5h16v11H9l-5 4z'/><path d='M9 8l6 6M15 8l-6 6' stroke-width='1.4'/>"),
    ("track", "<path d='M4 18l1-4 9-9 3 3-9 9zM13 7l3 3'/><path d='M4 21h16' stroke-width='1.6'/>"),
    ("accept", "<path d='M5 13l4 4 10-11' stroke-width='2'/>"),
    ("reject", "<path d='M6 6l12 12M18 6L6 18' stroke-width='2'/>"),
    ("accept-all", "<path d='M3 13l3 3 7-8M10 15l2 2 9-10' stroke-width='1.8'/>"),
    ("reject-all", "<path d='M4 7l8 8M12 7l-8 8M14 7l8 8M22 7l-8 8' stroke-width='1.6'/>"),
    ("find", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21'/>"),
    ("zoom", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21M8 11h6'/>"),
    ("zoom-in", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21M8 11h6M11 8v6'/>"),
    ("zoom-out", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21M8 11h6'/>"),
    ("zoom-100", "<circle cx='11' cy='11' r='6'/><path d='M15.5 15.5L21 21'/><text x='11' y='14' font-size='7' text-anchor='middle' fill='currentColor' stroke='none'>1</text>"),
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
     <rect x='8' y='4' width='40' height='56' rx='4' fill='#FFFFFF' stroke='#2B579A' stroke-width='3'/>\
     <path d='M38 4v12h12' fill='none' stroke='#2B579A' stroke-width='3'/>\
     <path d='M17 26h22M17 34h22M17 42h14' stroke='#2B579A' stroke-width='3' stroke-linecap='round'/>\
     </svg>"
}
