//! Light and dark colours shared by every Folio app; the app colour goes on the title bar.

use crate::gfx::Color;

/// The Folio app colours. Title bars carry these; menu bars and ribbons don't.
pub mod app_colors {
    use crate::gfx::Color;
    pub const CHORD: Color = Color::hex(0xC2187A); // fuchsia
    pub const REEL: Color = Color::hex(0x1F3A6B); // navy blue
    pub const CORK: Color = Color::hex(0xF4E3A1); // pale yellow
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    /// The app colour (title bar).
    pub app: Color,
    pub app_text: Color,
    /// Highlights: selection, focus, checked buttons.
    pub accent: Color,
    pub window: Color,
    pub ribbon: Color,
    pub tabs: Color,
    pub panel: Color,
    pub canvas: Color,
    pub text: Color,
    pub muted: Color,
    pub border: Color,
    pub hover: Color,
    pub pressed: Color,
    pub checked: Color,
    pub popup: Color,
    pub field: Color,
    pub selection: Color,
    pub danger: Color,
}

impl Theme {
    pub fn new(dark: bool, app: Color, accent: Color) -> Theme {
        if dark {
            Theme {
                dark,
                app,
                app_text: app.contrast_text(),
                accent,
                window: Color::hex(0x1F1F1F),
                ribbon: Color::hex(0x2B2B2B),
                tabs: Color::hex(0x262626),
                panel: Color::hex(0x252525),
                canvas: Color::hex(0x161616),
                text: Color::hex(0xEDEDED),
                muted: Color::hex(0xA8A8A8),
                border: Color::hex(0x3E3E3E),
                hover: Color::hex(0x3A3A3A),
                pressed: Color::hex(0x4A4A4A),
                checked: accent.with_alpha(90),
                popup: Color::hex(0x2D2D2D),
                field: Color::hex(0x1B1B1B),
                selection: accent.with_alpha(110),
                danger: Color::hex(0xE5534B),
            }
        } else {
            Theme {
                dark,
                app,
                app_text: app.contrast_text(),
                accent,
                window: Color::hex(0xF3F3F3),
                ribbon: Color::hex(0xFAFAFA),
                tabs: Color::hex(0xF3F3F3),
                panel: Color::hex(0xF7F7F7),
                canvas: Color::hex(0xE6E6E6),
                text: Color::hex(0x1E1E1E),
                muted: Color::hex(0x666666),
                border: Color::hex(0xD6D6D6),
                hover: Color::hex(0xE8E8E8),
                pressed: Color::hex(0xD8D8D8),
                checked: accent.with_alpha(70),
                popup: Color::WHITE,
                field: Color::WHITE,
                selection: accent.with_alpha(80),
                danger: Color::hex(0xC42B1C),
            }
        }
    }
}
