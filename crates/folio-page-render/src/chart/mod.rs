//! Charts in a document, drawn. The reading of the part is the engine's.

pub mod draw;
pub mod ex;

use folio_canvas::Cx;
use std::sync::Arc;

/// Draw a chart part's chart into (0, 0, w, h) points.
pub fn draw(cr: &mut Cx, bytes: &Arc<Vec<u8>>, w: f64, h: f64, theme: &[u32; 12], font: &str) -> bool {
    let Some(ch) = crate::eng::chart::chart_of(bytes, theme) else { return false };
    let p = draw::Painter { theme: *theme, font: font.to_string(), text: std::cell::Cell::new(0x595959) };
    let mut pen = draw::Pen::new();
    p.paint_chart(&ch, cr, 0.0, 0.0, w, h, 1.0, &mut pen);
    true
}

/// Draw an Office 2016 chart part (treemap, sunburst) into (0, 0, w, h).
pub fn draw_ex(cr: &mut Cx, bytes: &Arc<Vec<u8>>, w: f64, h: f64, theme: &[u32; 12], font: &str) -> bool {
    let text = String::from_utf8_lossy(bytes);
    let Some(ch) = ex::parse(&text, theme) else { return false };
    ex::draw(cr, &ch, w, h, font);
    true
}
