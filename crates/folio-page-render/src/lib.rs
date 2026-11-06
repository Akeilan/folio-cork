//! Drawing Folio Page's documents: a laid-out page onto a surface.
//!
//! The engine next door says where every glyph, rule and picture goes; this
//! puts them there. Nothing here measures anything, so what is drawn is what
//! the engine laid out.

pub use folio_page_engine as eng;

pub mod art;
pub mod chart;
pub mod effects;
pub mod fonts;
pub mod math;
pub mod metafile;
pub mod page;
pub mod pdf;
pub mod pictures;
pub mod shapes;

pub use folio_canvas::{Cx, Glyphs};
pub use pdf::{export_pdf, export_pdf_with, pdf_bytes, PdfOptions};
pub use page::{enter_frame, frame_to_page, page_to_frame, render_page_image, stroke_border, Overlay,
               Renderer, RevMarks};
pub use folio_canvas;
