//! Turning a document's pictures into something to draw: the formats Word
//! documents carry, decoded by the suite's own picture code.

use folio_kit::gfx::{self, sk};
use std::sync::Arc;

/// Decode a picture, no larger than `max_px` on its longest side. SVG
/// drawings are drawn at that size rather than scaled up from a small one.
pub fn decode(bytes: &[u8], max_px: u32) -> Option<Arc<sk::Pixmap>> {
    if looks_like_svg(bytes) {
        let src = String::from_utf8_lossy(bytes);
        let pm = folio_kit::icons::render_svg(&src, max_px.min(1024), max_px.min(1024))?;
        return Some(Arc::new(pm));
    }
    let pm = gfx::decode_image(bytes)?;
    let (w, h) = (pm.width(), pm.height());
    if w == 0 || h == 0 {
        return None;
    }
    if w.max(h) <= max_px {
        return Some(Arc::new(pm));
    }
    let f = max_px as f64 / w.max(h) as f64;
    let (nw, nh) = (((w as f64 * f).round() as u32).max(1), ((h as f64 * f).round() as u32).max(1));
    let mut out = sk::Pixmap::new(nw, nh)?;
    let mut paint = sk::PixmapPaint::default();
    paint.quality = sk::FilterQuality::Bicubic;
    out.draw_pixmap(
        0,
        0,
        pm.as_ref(),
        &paint,
        sk::Transform::from_scale(nw as f32 / w as f32, nh as f32 / h as f32),
        None,
    );
    Some(Arc::new(out))
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(400)];
    let s = String::from_utf8_lossy(head);
    s.contains("<svg") || (s.contains("<?xml") && s.contains("svg"))
}

/// A picture's pixels as straight (not premultiplied) RGBA, for the picture
/// effects to work on.
pub fn to_rgba(pm: &sk::Pixmap) -> Vec<u8> {
    gfx::rgba_from_pixmap(pm)
}

/// The other way round.
pub fn from_rgba(w: u32, h: u32, rgba: &[u8]) -> Option<sk::Pixmap> {
    gfx::pixmap_from_rgba(w, h, rgba)
}
