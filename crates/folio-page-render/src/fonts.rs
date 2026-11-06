//! The bridge between the engine's fonts and the surface's glyph outlines:
//! the same font file, so what is drawn is what was measured.

use crate::eng::fonts::Font;
use folio_canvas::glyphs::FontRef;
use std::collections::HashMap;
use std::sync::Arc;

/// Font files already read, by the engine's font number.
#[derive(Default)]
pub struct Faces {
    files: HashMap<usize, Option<Arc<Vec<u8>>>>,
}

impl Faces {
    pub fn new() -> Faces {
        Faces::default()
    }

    /// A font to take outlines from. `italic` and `bold` say whether this run
    /// is asking for a slant or a weight the file itself has not got.
    pub fn get(&mut self, f: &Font, bold: bool, italic: bool) -> Option<FontRef> {
        let data = match self.files.get(&f.id) {
            Some(d) => d.clone(),
            None => {
                let d = Some(Arc::new(f.data().to_vec()));
                self.files.insert(f.id, d.clone());
                d
            }
        }?;
        Some(FontRef {
            key: f.id,
            data,
            index: f.index,
            upem: f.upem,
            synth_bold: bold && f.synth_bold,
            synth_italic: italic && f.synth_italic,
        })
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }
}
