//! The fonts Folio carries inside itself (X3-16).
//!
//! One executable means one file: a program that needs a folder of fonts beside it is two things,
//! and the one that goes missing is always the folder. These faces are compiled in, so the binary
//! draws the same text on a machine with no fonts installed at all as on a full desktop — which is
//! also what makes the pixel tests mean anything.
//!
//! Carlito (Łukasz Dziedzic, SIL Open Font License) and Liberation (Steve Matteson / Google,
//! SIL Open Font License) — see fonts/LICENSE-Carlito.txt and fonts/LICENSE-Liberation.txt.

macro_rules! face {
    ($f:literal) => {
        ($f, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fonts/", $f)) as &[u8])
    };
}

/// Every face that ships inside the program, as (file name, bytes).
pub fn bundled() -> Vec<(&'static str, &'static [u8])> {
    vec![
        face!("Carlito-Regular.ttf"),
        face!("Carlito-Bold.ttf"),
        face!("Carlito-Italic.ttf"),
        face!("Carlito-BoldItalic.ttf"),
        face!("LiberationSerif-Regular.ttf"),
        face!("LiberationSerif-Bold.ttf"),
        face!("LiberationSerif-Italic.ttf"),
        face!("LiberationSerif-BoldItalic.ttf"),
        face!("LiberationMono-Regular.ttf"),
        face!("LiberationMono-Bold.ttf"),
        face!("LiberationMono-Italic.ttf"),
        face!("LiberationMono-BoldItalic.ttf"),
    ]
}

/// How much of the program is fonts.
pub fn bytes() -> usize {
    bundled().iter().map(|(_, b)| b.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_is_really_in_the_program() {
        let all = bundled();
        assert_eq!(all.len(), 12);
        for (name, data) in &all {
            assert!(data.len() > 10_000, "{} is too small to be a font", name);
            // a TrueType face starts with 0x00010000, or "true", or "OTTO"
            let tag = &data[..4];
            assert!(tag == [0, 1, 0, 0] || tag == b"true" || tag == b"OTTO", "{} is not a font", name);
        }
        assert!(bytes() > 4_000_000, "the faces should weigh something: {}", bytes());
    }
}
