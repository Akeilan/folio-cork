//! SVG icons rendered and cached per size and colour. Icons use `currentColor` for strokes so
//! one file works in light and dark themes; `var(--accent)` picks up the app's accent.

use crate::gfx::{sk, Color, Rect};
use std::collections::HashMap;

pub struct Icons {
    sources: HashMap<String, String>,
    cache: HashMap<(String, u32, Color), Option<sk::Pixmap>>,
}

impl Icons {
    pub fn new() -> Icons {
        Icons { sources: HashMap::new(), cache: HashMap::new() }
    }

    /// Register an icon's SVG source under a name.
    pub fn add(&mut self, name: &str, svg: &str) {
        self.sources.insert(name.to_string(), svg.to_string());
    }

    pub fn has(&self, name: &str) -> bool {
        self.sources.contains_key(name)
    }

    pub fn get(&mut self, name: &str, px: u32, color: Color, accent: Color) -> Option<&sk::Pixmap> {
        let key = (name.to_string(), px, color);
        if !self.cache.contains_key(&key) {
            let pm = self.sources.get(name).and_then(|src| {
                let src = src.replace("currentColor", &color.to_hex()).replace("var(--accent)", &accent.to_hex());
                render_svg(&src, px, px)
            });
            self.cache.insert(key.clone(), pm);
        }
        self.cache.get(&key).and_then(|x| x.as_ref())
    }

    pub fn draw(&mut self, pm: &mut sk::Pixmap, name: &str, r: Rect, color: Color, accent: Color) {
        let px = r.w.round().max(1.0) as u32;
        if let Some(img) = self.get(name, px, color, accent) {
            let img = img.clone();
            pm.draw_pixmap(r.x.round() as i32, r.y.round() as i32, img.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
        }
    }
}

impl Default for Icons {
    fn default() -> Self {
        Icons::new()
    }
}

/// Render an SVG document into a pixmap of the given size (aspect kept, centred).
pub fn render_svg(src: &str, w: u32, h: u32) -> Option<sk::Pixmap> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(src, &opt).ok()?;
    let size = tree.size();
    let s = (w as f32 / size.width()).min(h as f32 / size.height());
    let mut pm = sk::Pixmap::new(w.max(1), h.max(1))?;
    let tx = (w as f32 - size.width() * s) / 2.0;
    let ty = (h as f32 - size.height() * s) / 2.0;
    resvg::render(&tree, sk::Transform::from_row(s, 0.0, 0.0, s, tx, ty), &mut pm.as_mut());
    Some(pm)
}
