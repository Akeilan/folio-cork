//! Graze's icons: 24-unit line drawings in the current colour.

use folio_kit::icons::Icons;

const ICONS: &[(&str, &str)] = &[
    ("win-min", "<path d='M5 12h14'/>"),
    ("win-max", "<rect x='5' y='5' width='14' height='14'/>"),
    ("win-restore", "<rect x='5' y='8' width='11' height='11'/><path d='M8 8V5h11v11h-3'/>"),
    ("win-close", "<path d='M6 6l12 12M18 6L6 18'/>"),
    ("graze", "<circle cx='12' cy='12' r='9'/><path d='M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18'/>"),
    ("back", "<path d='M19 12H5M11 6l-6 6 6 6' stroke-width='1.9'/>"),
    ("forward", "<path d='M5 12h14M13 6l6 6-6 6' stroke-width='1.9'/>"),
    ("reload", "<path d='M19 12a7 7 0 1 1-2.1-5' stroke-width='1.9'/><path d='M19 4v4h-4' stroke-width='1.9'/>"),
    ("stop", "<path d='M6 6l12 12M18 6L6 18' stroke-width='1.9'/>"),
    ("menu", "<path d='M4 7h16M4 12h16M4 17h16' stroke-width='1.9'/>"),
    ("plus", "<path d='M12 5v14M5 12h14' stroke-width='1.9'/>"),
    ("x", "<path d='M6 6l12 12M18 6L6 18' stroke-width='2'/>"),
    ("lock", "<rect x='5' y='11' width='14' height='10' rx='2'/><path d='M8 11V8a4 4 0 0 1 8 0v3'/>"),
    ("search", "<circle cx='11' cy='11' r='6'/><path d='M20 20l-4.5-4.5'/>"),
    ("warn", "<path d='M12 3l10 18H2z'/><path d='M12 10v5M12 18v.5' stroke-width='2'/>"),
    ("page", "<path d='M6 3h8l4 4v14H6zM14 3v4h4'/>"),
    ("loading", "<path d='M12 3a9 9 0 1 0 9 9' stroke-width='2'/>"),
    ("up", "<path d='M6 15l6-6 6 6' stroke-width='2'/>"),
    ("down", "<path d='M6 9l6 6 6-6' stroke-width='2'/>"),
    ("check", "<path d='M5 12l5 5 9-10' stroke-width='2.2'/>"),
    ("chevron-right", "<path d='M9 6l6 6-6 6' stroke-width='2'/>"),
];

pub fn load(icons: &mut Icons) {
    for (name, body) in ICONS {
        let svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' width='24' height='24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'>{body}</svg>"
        );
        icons.add(name, &svg);
    }
}

/// The app icon in full colour: a lime globe with a leaf.
pub fn app_icon_svg() -> &'static str {
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64' width='64' height='64'>\
<circle cx='32' cy='32' r='27' fill='#8BC34A'/>\
<path d='M5 32h54M32 5c9 8 9 46 0 54M32 5c-9 8-9 46 0 54M9 19h46M9 45h46' stroke='#F4FAEC' stroke-width='2.4' fill='none' opacity='.85'/>\
<path d='M38 50c0-12 8-20 20-22-1 13-8 22-20 22z' fill='#33691E'/>\
<path d='M40 48c4-6 9-10 15-13' stroke='#C5E1A5' stroke-width='1.6' fill='none'/></svg>"
}
