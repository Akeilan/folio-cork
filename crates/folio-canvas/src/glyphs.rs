//! Glyph outlines, from the same font files the engine measured with.

use crate::{Cmd, Path};
use std::collections::HashMap;
use std::sync::Arc;

/// A font file to take outlines from. `key` tells one font from another in the
/// cache; `upem` is the font's own unit of measurement.
#[derive(Clone)]
pub struct FontRef {
    pub key: usize,
    pub data: Arc<Vec<u8>>,
    pub index: u32,
    pub upem: f64,
    /// a font being used in place of a bold or an italic it has not got: the
    /// outline is slanted, or drawn a little thicker
    pub synth_bold: bool,
    pub synth_italic: bool,
}

/// Outlines already taken, kept for the next line that uses them.
#[derive(Default)]
pub struct Glyphs {
    paths: HashMap<(usize, u16), Option<Arc<Path>>>,
}

struct Build {
    cmds: Vec<Cmd>,
    slant: f64,
}

impl ttf_parser::OutlineBuilder for Build {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = (x as f64 + self.slant * y as f64, y as f64);
        self.cmds.push(Cmd::Move(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = (x as f64 + self.slant * y as f64, y as f64);
        self.cmds.push(Cmd::Line(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        // a quadratic curve as the cubic the surface draws
        let (cx, cy) = (x1 as f64 + self.slant * y1 as f64, y1 as f64);
        let (px, py) = (x as f64 + self.slant * y as f64, y as f64);
        let (fx, fy) = match self.cmds.last() {
            Some(Cmd::Move(a, b)) | Some(Cmd::Line(a, b)) => (*a, *b),
            Some(Cmd::Curve(.., a, b)) => (*a, *b),
            _ => (cx, cy),
        };
        self.cmds.push(Cmd::Curve(
            fx + 2.0 / 3.0 * (cx - fx),
            fy + 2.0 / 3.0 * (cy - fy),
            px + 2.0 / 3.0 * (cx - px),
            py + 2.0 / 3.0 * (cy - py),
            px,
            py,
        ));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cmds.push(Cmd::Curve(
            x1 as f64 + self.slant * y1 as f64,
            y1 as f64,
            x2 as f64 + self.slant * y2 as f64,
            y2 as f64,
            x as f64 + self.slant * y as f64,
            y as f64,
        ));
    }

    fn close(&mut self) {
        self.cmds.push(Cmd::Close);
    }
}

impl Glyphs {
    pub fn new() -> Glyphs {
        Glyphs::default()
    }

    /// One glyph's outline, in font units with y upwards. None when the font
    /// has no outline for it (a space, or a glyph the file does not hold).
    pub fn outline(&mut self, f: &FontRef, gid: u16) -> Option<Arc<Path>> {
        if let Some(p) = self.paths.get(&(f.key, gid)) {
            return p.clone();
        }
        let p = Self::build(f, gid).map(Arc::new);
        self.paths.insert((f.key, gid), p.clone());
        p
    }

    fn build(f: &FontRef, gid: u16) -> Option<Path> {
        let face = ttf_parser::Face::parse(&f.data, f.index).ok()?;
        let mut b = Build { cmds: Vec::new(), slant: if f.synth_italic { 0.2 } else { 0.0 } };
        face.outline_glyph(ttf_parser::GlyphId(gid), &mut b)?;
        if b.cmds.is_empty() {
            return None;
        }
        Some(Path { cmds: b.cmds })
    }

    /// Forget everything: for when a document's fonts have changed.
    pub fn clear(&mut self) {
        self.paths.clear();
    }
}
