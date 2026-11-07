//! Titles, lower thirds and captions, drawn by us.
//!
//! ffmpeg can put words on a picture, but only with the fonts and the build the
//! computer happens to have. Reel draws each title itself, with the suite's own
//! text engine, and hands ffmpeg the picture — so what the preview shows and
//! what the film holds are the same drawing.

use crate::project::{Clip, Kind, Project};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::text::{Align, Text};
use folio_kit::theme::Theme;
use folio_kit::ui::Ctx;
use std::path::{Path, PathBuf};

fn colour(s: &str, fallback: Color) -> Color {
    // the older Reel wrote #aarrggbb; the kit reads #rrggbb and #rrggbbaa
    let t = s.trim();
    if t.len() == 9 && t.starts_with('#') {
        let hex = |r: std::ops::Range<usize>| u8::from_str_radix(&t[r], 16).unwrap_or(0);
        return Color::rgba(hex(3..5), hex(5..7), hex(7..9), hex(1..3));
    }
    Color::parse(t).unwrap_or(fallback)
}

/// How big the words are on a picture this tall, from the clip's own font line.
fn size_of(font: &str, h: u32) -> f32 {
    let pt: f32 = font.split_whitespace().last().and_then(|s| s.parse().ok()).unwrap_or(48.0);
    // the older Reel sized its fonts against a 1080-high picture
    (pt * h as f32 / 1080.0).max(8.0)
}

fn bold_of(font: &str) -> bool {
    font.to_lowercase().contains("bold")
}

/// One title as a picture the size of the film, transparent but for the words.
pub fn draw(c: &Clip, w: u32, h: u32, text: &mut Text) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(w.max(1), h.max(1)).unwrap();
    pm.fill(sk::Color::TRANSPARENT);
    let size = size_of(&c.font, h);
    let lines: Vec<&str> = c.text.lines().collect();
    let line_h = size * 1.3;
    let block = line_h * lines.len() as f32;
    let pad = size * 0.5;
    let top = match c.valign.as_str() {
        "top" => pad,
        "bottom" => h as f32 - block - pad,
        _ => (h as f32 - block) / 2.0,
    };
    let fg = colour(&c.text_color, Color::WHITE);
    let bg = colour(&c.bg_color, Color::TRANSPARENT);
    // the band behind the words: the whole picture for a title, a strip for the rest
    if bg.a > 0 {
        let band = if c.kind == Kind::Title {
            Rect::new(0.0, 0.0, w as f32, h as f32)
        } else {
            Rect::new(0.0, top - pad * 0.6, w as f32, block + pad * 1.2)
        };
        gfx::fill_rect(&mut pm, band, bg);
    }
    let align = match c.halign.as_str() {
        "left" => Align::Left,
        "right" => Align::Right,
        _ => Align::Center,
    };
    let theme = Theme::new(true, fg, fg);
    let mut icons = Icons::new();
    let mut cx = Ctx { pm: &mut pm, text, icons: &mut icons, theme: &theme, s: 1.0 };
    let mut y = top;
    for line in lines {
        let inset = size * 0.8;
        cx.label(line, Rect::new(inset, y, w as f32 - inset * 2.0, line_h), size, fg, bold_of(&c.font), align);
        y += line_h;
    }
    pm
}

/// Draw every title in the film and leave the pictures in `dir`. The export
/// hands these back to ffmpeg as the Titles track.
pub fn write_all(p: &Project, w: u32, h: u32, dir: &Path, text: &mut Text) -> Vec<(u64, PathBuf)> {
    let mut out = Vec::new();
    let _ = std::fs::create_dir_all(dir);
    for (_, c) in p.all_clips() {
        if !c.kind.is_text() || c.text.trim().is_empty() {
            continue;
        }
        let pm = draw(c, w, h, text);
        let path = dir.join(format!("title-{}.png", c.id));
        if std::fs::write(&path, gfx::encode_png(&pm)).is_ok() {
            out.push((c.id, path));
        }
    }
    out
}
