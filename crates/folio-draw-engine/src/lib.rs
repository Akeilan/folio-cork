//! Folio Draw engine, used by the Rust app directly (the GTK version used a copy through a C ABI).
//! No third-party crates: codecs, rasteriser and compositor are all in this crate.

pub mod adjust;
pub mod bmp;
pub mod deflate;
pub mod doc;
pub mod cutout;
pub mod emb;
pub mod gif;
pub mod ico;
pub mod jpeg;
pub mod ora;
pub mod png;
pub mod psd;
pub mod tiff;
pub mod vp8;
pub mod vp8_tables;
pub mod vp8l;
pub mod webp;
pub mod zip;

pub mod files;
