//! Folio toolkit (X3-14): the shared look and controls of every Folio app, drawn with its own
//! software renderer on top of the platform layer.

pub mod chrome;
pub mod files;
pub mod gfx;
pub mod icons;
pub mod ribbon;
pub mod text;
pub mod theme;
pub mod ui;

pub use folio_platform as platform;
pub use gfx::{sk, Color, Rect};
