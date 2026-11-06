//! Equations: the boxes the engine measured, drawn.

use crate::eng::fonts::Font;
use crate::eng::math::{Draw, MBox};
use folio_canvas::glyphs::FontRef;
use folio_canvas::Cx;

/// Draw an equation box with its top-left corner at (x, y). `face` gives the
/// outlines for a font the engine measured with.
pub fn draw(cr: &mut Cx, b: &MBox, x: f64, y: f64, face: &mut dyn FnMut(&Font) -> Option<FontRef>) {
    for d in &b.draws {
        match d {
            Draw::Glyph { font, gid, x: gx, y: gy, size, color } => {
                let Some(ff) = face(font) else { continue };
                cr.set_rgb(*color);
                cr.show_glyphs(&ff, *size, &[(*gid, x + gx, y + gy)]);
            }
            Draw::Rule { x: rx, y: ry, w, h, color } => {
                cr.set_rgb(*color);
                cr.rectangle(x + rx, y + ry, *w, *h);
                cr.fill();
            }
            Draw::Line { x0, y0, x1, y1, w, color } => {
                cr.set_rgb(*color);
                cr.set_line_width(*w);
                cr.move_to(x + x0, y + y0);
                cr.line_to(x + x1, y + y1);
                cr.stroke();
            }
        }
    }
}
