//! Folio Page's document engine (W3-12): the .docx model, Word-compatible layout and pagination.
//!
//! This is the portable half of the engine Page has used since 2.x — no system calls, no drawing
//! and no fonts of its own. Text measurements come in through [`Metrics`], which the app fills in
//! from `folio-kit`, so the same layout runs under the window, in the pixel tests and on any
//! machine the platform layer reaches.
pub mod art;
pub mod chart;
pub mod document;
pub mod effects;
pub mod shapes;
pub mod fonts;
pub mod math;
pub mod pictures;
pub mod hyph;
pub mod layout;
pub mod numbering;
pub mod package;
pub mod paginate;
pub mod props;
pub mod styles;
pub mod text;
pub mod xml;

/// What the engine needs to know about a font to lay text out. The app answers with folio-kit's
/// text layer; a test can answer with fixed numbers.
pub trait Metrics {
    /// The width of this text in the given font, in points.
    fn width(&mut self, text: &str, family: &str, size: f32, bold: bool, italic: bool) -> f32;
    /// Ascent, descent and line gap of the font, in points.
    fn vmetrics(&mut self, family: &str, size: f32, bold: bool, italic: bool) -> (f32, f32, f32);
    /// Whether the family is there at all (a missing one falls back).
    fn has_family(&mut self, family: &str) -> bool {
        let _ = family;
        true
    }
}
