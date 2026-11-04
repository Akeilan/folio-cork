//! Text: font discovery, shaping, wrapping and drawing, for both the interface (crisp bitmaps)
//! and documents (vector outlines that scale with zoom and export to SVG and PDF).

use crate::gfx::{blend_px, sk, Color, Rect};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap};
use std::collections::HashMap;

/// How text should look.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Color,
    pub align: Align,
    /// Line height as a multiple of the size.
    pub line: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum VAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            family: String::new(),
            size: 14.0,
            bold: false,
            italic: false,
            underline: false,
            color: Color::BLACK,
            align: Align::Left,
            line: 1.25,
        }
    }
}

/// One positioned line of laid-out text.
#[derive(Clone, Debug)]
pub struct LineInfo {
    /// Byte range in the source text.
    pub start: usize,
    pub end: usize,
    pub top: f32,
    pub height: f32,
    pub baseline: f32,
    pub x: f32,
    pub width: f32,
    /// x of each glyph cluster start, with the byte index it starts at.
    pub carets: Vec<(usize, f32)>,
}

/// The result of laying out a block of text.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub lines: Vec<LineInfo>,
}

impl Layout {
    /// Caret position (x, top, height) for byte index `i`.
    pub fn caret(&self, i: usize) -> (f32, f32, f32) {
        for (n, l) in self.lines.iter().enumerate() {
            let last = n + 1 == self.lines.len();
            if i >= l.start && (i < l.end || (i == l.end && (last || self.lines[n + 1].start > i))) {
                let mut x = l.x + l.width;
                for &(b, cx) in &l.carets {
                    if b >= i {
                        x = cx;
                        break;
                    }
                }
                if i == l.end {
                    x = l.x + l.width;
                }
                return (x, l.top, l.height);
            }
        }
        match self.lines.last() {
            Some(l) => (l.x + l.width, l.top, l.height),
            None => (0.0, 0.0, 16.0),
        }
    }

    /// Byte index nearest to (x, y).
    pub fn hit(&self, x: f32, y: f32) -> usize {
        let Some(line) = self
            .lines
            .iter()
            .find(|l| y < l.top + l.height)
            .or(self.lines.last())
        else {
            return 0;
        };
        let mut best = line.end;
        let mut bd = (line.x + line.width - x).abs();
        for &(b, cx) in &line.carets {
            let d = (cx - x).abs();
            if d < bd {
                bd = d;
                best = b;
            }
        }
        best
    }

    /// Line index containing byte index `i`.
    pub fn line_of(&self, i: usize) -> usize {
        for (n, l) in self.lines.iter().enumerate() {
            if i <= l.end && (n + 1 == self.lines.len() || i < self.lines[n + 1].start) {
                return n;
            }
        }
        self.lines.len().saturating_sub(1)
    }
}

type CacheKey = (String, u32, bool, bool, u32, String, u8);

struct Cached {
    buffer: Buffer,
    used: u64,
    /// Glyph outlines, built once per layout (documents redraw them every frame).
    outlines: Option<Vec<sk::Path>>,
}

/// The text engine. One per app.
pub struct Text {
    pub fonts: FontSystem,
    swash: SwashCache,
    cache: HashMap<CacheKey, Cached>,
    tick: u64,
    ui_family: String,
}

/// The fonts that ship with Folio (X3-16): Latin, Greek and Cyrillic.
pub const BUNDLED_SANS: &str = "Carlito";
pub const BUNDLED_SERIF: &str = "Liberation Serif";
pub const BUNDLED_MONO: &str = "Liberation Mono";

fn key_size(v: f32) -> u32 {
    (v * 64.0).round() as u32
}

impl Text {
    pub fn new() -> Text {
        if folio_platform::setting("FOLIO_FONTS_ONLY").is_some() {
            return Text::bundled();
        }
        let mut fonts = FontSystem::new();
        // Fonts shipped next to the app (X3-16) come first.
        for dir in folio_platform::font_dirs() {
            fonts.db_mut().load_fonts_dir(&dir);
        }
        let have = |f: &FontSystem, name: &str| f.db().faces().any(|x| x.families.iter().any(|(n, _)| n == name));
        let ui_family = ["Ubuntu", "Segoe UI", "SF Pro Text", "Cantarell", "Noto Sans", "DejaVu Sans", "Liberation Sans", BUNDLED_SANS]
            .iter()
            .find(|n| have(&fonts, n))
            .map(|s| s.to_string())
            .unwrap_or_default();
        Text { fonts, swash: SwashCache::new(), cache: HashMap::new(), tick: 0, ui_family }
    }

    /// Only the fonts that ship with Folio, never the system's (X3-16): what the pixel tests
    /// use, so a page draws the same on every machine.
    pub fn bundled() -> Text {
        let mut db = cosmic_text::fontdb::Database::new();
        let mut dirs = folio_platform::font_dirs();
        dirs.push(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fonts")));
        for d in dirs.iter().filter(|d| d.is_dir()) {
            db.load_fonts_dir(d);
        }
        db.set_sans_serif_family(BUNDLED_SANS);
        db.set_serif_family(BUNDLED_SERIF);
        db.set_monospace_family(BUNDLED_MONO);
        let fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
        Text { fonts, swash: SwashCache::new(), cache: HashMap::new(), tick: 0, ui_family: BUNDLED_SANS.into() }
    }

    pub fn ui_family(&self) -> &str {
        &self.ui_family
    }

    /// Installed font families, sorted, without duplicates.
    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self.fonts.db().faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
        v.sort_by_key(|s| s.to_lowercase());
        v.dedup();
        v
    }

    fn ensure(&mut self, text: &str, st: &TextStyle, width: Option<f32>) -> CacheKey {
        self.tick += 1;
        let fam = if st.family.is_empty() { self.ui_family.clone() } else { st.family.clone() };
        let key = (
            fam.clone(),
            key_size(st.size),
            st.bold,
            st.italic,
            width.map(key_size).unwrap_or(u32::MAX),
            text.to_string(),
            match st.align {
                Align::Left => 0u8,
                Align::Center => 1,
                Align::Right => 2,
            }
            .wrapping_add((key_size(st.line) as u8).wrapping_mul(4)),
        );
        if self.cache.len() > 3000 {
            let cut = self.tick.saturating_sub(1500);
            self.cache.retain(|_, c| c.used >= cut);
        }
        let tick = self.tick;
        if let Some(c) = self.cache.get_mut(&key) {
            c.used = tick;
            return key;
        }
        let mut b = Buffer::new(&mut self.fonts, Metrics::new(st.size.max(0.5), (st.size * st.line).max(1.0)));
        b.set_wrap(if width.is_some() { Wrap::WordOrGlyph } else { Wrap::None });
        b.set_size(width, None);
        let attrs = Attrs::new()
            .family(if fam.is_empty() { Family::SansSerif } else { Family::Name(&fam) })
            .weight(if st.bold { Weight::BOLD } else { Weight::NORMAL })
            .style(if st.italic { Style::Italic } else { Style::Normal });
        let align = Some(match st.align {
            Align::Left => cosmic_text::Align::Left,
            Align::Center => cosmic_text::Align::Center,
            Align::Right => cosmic_text::Align::Right,
        });
        b.set_text(text, &attrs, Shaping::Advanced, align);
        b.shape_until_scroll(&mut self.fonts, false);
        self.cache.insert(key.clone(), Cached { buffer: b, used: tick, outlines: None });
        key
    }

    /// Lay out text, optionally wrapped to `width`. Positions are relative to the block's top left.
    pub fn layout(&mut self, text: &str, st: &TextStyle, width: Option<f32>) -> Layout {
        let k = self.ensure(text, st, width);
        let b = &self.cache[&k].buffer;
        let mut lay = Layout::default();
        // Map line indices to byte offsets in the original text.
        let mut line_starts = vec![0usize];
        for (i, ch) in text.char_indices() {
            if ch == '\n' {
                line_starts.push(i + 1);
            }
        }
        for run in b.layout_runs() {
            let base = line_starts.get(run.line_i).copied().unwrap_or(0);
            let (mut x0, mut x1) = (f32::MAX, f32::MIN);
            let mut carets = Vec::new();
            let (mut s, mut e) = (usize::MAX, 0usize);
            for g in run.glyphs {
                x0 = x0.min(g.x);
                x1 = x1.max(g.x + g.w);
                carets.push((base + g.start, g.x));
                s = s.min(base + g.start);
                e = e.max(base + g.end);
            }
            if run.glyphs.is_empty() {
                x0 = match st.align {
                    Align::Left => 0.0,
                    Align::Center => width.unwrap_or(0.0) / 2.0,
                    Align::Right => width.unwrap_or(0.0),
                };
                x1 = x0;
                s = base;
                e = base;
            }
            carets.sort_by(|a, b| a.0.cmp(&b.0));
            lay.width = lay.width.max(x1 - x0);
            lay.lines.push(LineInfo {
                start: s,
                end: e,
                top: run.line_top,
                height: run.line_height,
                baseline: run.line_y,
                x: x0,
                width: x1 - x0,
                carets,
            });
            lay.height = run.line_top + run.line_height;
        }
        if lay.lines.is_empty() {
            let h = st.size * st.line;
            let x = match st.align {
                Align::Left => 0.0,
                Align::Center => width.unwrap_or(0.0) / 2.0,
                Align::Right => width.unwrap_or(0.0),
            };
            lay.lines.push(LineInfo { start: 0, end: 0, top: 0.0, height: h, baseline: st.size, x, width: 0.0, carets: vec![] });
            lay.height = h;
        }
        lay
    }

    pub fn measure(&mut self, text: &str, st: &TextStyle) -> (f32, f32) {
        let l = self.layout(text, st, None);
        (l.width, l.height)
    }

    /// Draw text as crisp bitmaps at (x, y) = top left. For interface text.
    pub fn draw(&mut self, pm: &mut sk::Pixmap, text: &str, st: &TextStyle, x: f32, y: f32, width: Option<f32>) {
        let color = st.color;
        let (w, h) = (pm.width(), pm.height());
        let ox = x.round() as i32;
        let oy = y.round() as i32;
        let k = self.ensure(text, st, width);
        let Some(c) = self.cache.get(&k) else { return };
        let data = pm.data_mut();
        let mut underline: Vec<(f32, f32, f32)> = Vec::new();
        for run in c.buffer.layout_runs() {
            if st.underline && !run.glyphs.is_empty() {
                let x0 = run.glyphs.first().map(|g| g.x).unwrap_or(0.0);
                let x1 = run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0);
                underline.push((x0, x1, run.line_y + st.size * 0.12));
            }
            for g in run.glyphs.iter() {
                let pg = g.physical((0.0, 0.0), 1.0);
                if let Some(img) = self.swash.get_image(&mut self.fonts, pg.cache_key) {
                    let gx = ox + pg.x + img.placement.left;
                    let gy = oy + run.line_y as i32 + pg.y - img.placement.top;
                    let iw = img.placement.width as i32;
                    let ih = img.placement.height as i32;
                    match img.content {
                        cosmic_text::SwashContent::Mask => {
                            let mut i = 0usize;
                            for yy in 0..ih {
                                for xx in 0..iw {
                                    let a = img.data[i];
                                    i += 1;
                                    if a > 0 {
                                        blend_px(data, w, h, gx + xx, gy + yy, color, a);
                                    }
                                }
                            }
                        }
                        cosmic_text::SwashContent::Color => {
                            let mut i = 0usize;
                            for yy in 0..ih {
                                for xx in 0..iw {
                                    let px = &img.data[i..i + 4];
                                    i += 4;
                                    if px[3] > 0 {
                                        blend_px(data, w, h, gx + xx, gy + yy, Color::rgb(px[0], px[1], px[2]), px[3]);
                                    }
                                }
                            }
                        }
                        cosmic_text::SwashContent::SubpixelMask => {}
                    }
                }
            }
        }
        for (x0, x1, yy) in underline {
            crate::gfx::fill_rect(pm, Rect::new(ox as f32 + x0, oy as f32 + yy, x1 - x0, (st.size / 14.0).max(1.0)), color);
        }
    }

    /// Text as vector paths (one per glyph run line), top left at (0, 0), in the style's units.
    /// Used for document text so it scales with zoom and exports exactly.
    pub fn outlines(&mut self, text: &str, st: &TextStyle, width: Option<f32>) -> Vec<sk::Path> {
        let key = self.ensure(text, st, width);
        let Some(c) = self.cache.get(&key) else { return vec![] };
        if let Some(o) = &c.outlines {
            return o.clone();
        }
        let mut out = Vec::new();
        let runs: Vec<(f32, Vec<(f32, f32, cosmic_text::CacheKey)>, f32, f32)> = c
            .buffer
            .layout_runs()
            .map(|run| {
                let gl: Vec<(f32, f32, cosmic_text::CacheKey)> = run
                    .glyphs
                    .iter()
                    .map(|g| {
                        let pg = g.physical((0.0, 0.0), 1.0);
                        (g.x + g.x_offset * g.font_size, g.y_offset * g.font_size, pg.cache_key)
                    })
                    .collect();
                let x0 = run.glyphs.first().map(|g| g.x).unwrap_or(0.0);
                let x1 = run.glyphs.last().map(|g| g.x + g.w).unwrap_or(0.0);
                (run.line_y, gl, x0, x1)
            })
            .collect();
        for (baseline, glyphs, x0, x1) in runs {
            let mut pb = sk::PathBuilder::new();
            for (gx, gy, key) in glyphs {
                let Some(cmds) = self.swash.get_outline_commands(&mut self.fonts, key) else { continue };
                // Outline units are pixels at the style size with y pointing up from the baseline.
                let tx = |x: f32, y: f32| (gx + x, baseline + gy - y);
                for cmd in cmds {
                    use cosmic_text::Command as C;
                    match *cmd {
                        C::MoveTo(p) => {
                            let (x, y) = tx(p.x, p.y);
                            pb.move_to(x, y)
                        }
                        C::LineTo(p) => {
                            let (x, y) = tx(p.x, p.y);
                            pb.line_to(x, y)
                        }
                        C::QuadTo(c1, p) => {
                            let (cx, cy) = tx(c1.x, c1.y);
                            let (x, y) = tx(p.x, p.y);
                            pb.quad_to(cx, cy, x, y)
                        }
                        C::CurveTo(c1, c2, p) => {
                            let (ax, ay) = tx(c1.x, c1.y);
                            let (bx, by) = tx(c2.x, c2.y);
                            let (x, y) = tx(p.x, p.y);
                            pb.cubic_to(ax, ay, bx, by, x, y)
                        }
                        C::Close => pb.close(),
                    }
                }
            }
            if st.underline && x1 > x0 {
                let t = (st.size / 14.0).max(0.6);
                if let Some(r) = sk::Rect::from_xywh(x0, baseline + st.size * 0.12, x1 - x0, t) {
                    pb.push_rect(r);
                }
            }
            if let Some(p) = pb.finish() {
                out.push(p);
            }
        }
        if let Some(c) = self.cache.get_mut(&key) {
            c.outlines = Some(out.clone());
        }
        out
    }
}

impl Default for Text {
    fn default() -> Self {
        Text::new()
    }
}
