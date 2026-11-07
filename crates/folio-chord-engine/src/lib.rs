//! Folio Chord's engine: the library of songs, the playlists, and playing them.
//!
//! Nothing here opens a window or talks to the sound card: the app hands the
//! player's samples to the platform layer.
pub mod db;
pub mod library;
pub mod player;
pub mod smart;
pub mod tags;
