//! What a music file says about itself: the tags, the length and the format.
//!
//! Symphonia reads the containers; this turns what it finds into the fields the
//! library keeps, under the names iTunes used.

use std::path::Path;
use symphonia::core::codecs::CODEC_TYPE_NULL;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey, Value};
use symphonia::core::probe::Hint;

/// What one file says about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub genre: String,
    pub composer: String,
    pub grouping: String,
    pub comments: String,
    pub year: i32,
    pub track_no: u32,
    pub track_count: u32,
    pub disc_no: u32,
    pub disc_count: u32,
    pub bpm: u32,
    pub compilation: bool,
    /// ReplayGain, in decibels
    pub rg_track: Option<f64>,
    pub rg_album: Option<f64>,
    /// seconds
    pub duration: f64,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub channels: u16,
    /// "MP3", "AAC", "Apple Lossless", "FLAC", "Ogg Vorbis", "WAV", "AIFF"
    pub kind: String,
    pub has_art: bool,
}

fn number(s: &str) -> (u32, u32) {
    let mut it = s.split(['/', '-']);
    let a = it.next().unwrap_or("").trim().parse().unwrap_or(0);
    let b = it.next().unwrap_or("").trim().parse().unwrap_or(0);
    (a, b)
}

fn db_value(s: &str) -> Option<f64> {
    s.trim().trim_end_matches("dB").trim().parse::<f64>().ok()
}

/// The name people know the format by, from the file's extension and codec.
fn kind_of(ext: &str, codec: &str) -> String {
    match ext.to_ascii_lowercase().as_str() {
        "mp3" => "MP3".into(),
        "flac" => "FLAC".into(),
        "wav" | "wave" => "WAV".into(),
        "aif" | "aiff" | "aifc" => "AIFF".into(),
        "ogg" | "oga" => "Ogg Vorbis".into(),
        "opus" => "Opus".into(),
        "m4a" | "m4b" | "mp4" | "aac" => {
            if codec.contains("ALAC") || codec.contains("Apple Lossless") {
                "Apple Lossless".into()
            } else {
                "AAC".into()
            }
        }
        other => other.to_ascii_uppercase(),
    }
}

/// Read what the file says about itself. Returns None when nothing can read it.
pub fn read(path: &Path) -> Option<Tags> {
    let file = std::fs::File::open(path).ok()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if !ext.is_empty() {
        hint.with_extension(&ext);
    }
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
        .ok()?;
    let mut t = Tags::default();
    let mut format = probed.format;
    let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL)?;
    let p = &track.codec_params;
    t.sample_rate = p.sample_rate.unwrap_or(44100);
    t.channels = p.channels.map(|c| c.count() as u16).unwrap_or(2);
    if let (Some(n), Some(rate)) = (p.n_frames, p.sample_rate) {
        t.duration = n as f64 / rate as f64;
    }
    let codec_name = symphonia::default::get_codecs()
        .get_codec(p.codec)
        .map(|d| d.short_name.to_string())
        .unwrap_or_default();
    t.kind = kind_of(&ext, &codec_name);
    if let Ok(meta) = std::fs::metadata(path) {
        if t.duration > 0.1 {
            t.bitrate = ((meta.len() as f64 * 8.0 / t.duration) / 1000.0).round() as u32;
        }
    }
    let mut revs = Vec::new();
    if let Some(m) = probed.metadata.get().and_then(|mut m| m.skip_to_latest().cloned()) {
        revs.push(m);
    }
    if let Some(m) = format.metadata().skip_to_latest() {
        revs.push(m.clone());
    }
    for rev in revs {
        for tag in rev.tags() {
            let v = match &tag.value {
                Value::String(s) => s.clone(),
                Value::UnsignedInt(u) => u.to_string(),
                Value::SignedInt(i) => i.to_string(),
                Value::Float(f) => f.to_string(),
                Value::Boolean(b) => b.to_string(),
                _ => continue,
            };
            let v = v.trim().to_string();
            if v.is_empty() {
                continue;
            }
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) => t.title = v,
                Some(StandardTagKey::Artist) => {
                    if t.artist.is_empty() {
                        t.artist = v
                    }
                }
                Some(StandardTagKey::AlbumArtist) => t.album_artist = v,
                Some(StandardTagKey::Album) => t.album = v,
                Some(StandardTagKey::Genre) => t.genre = v,
                Some(StandardTagKey::Composer) => t.composer = v,
                Some(StandardTagKey::ContentGroup) => t.grouping = v,
                Some(StandardTagKey::Comment) => t.comments = v,
                Some(StandardTagKey::Date) | Some(StandardTagKey::OriginalDate) => {
                    if t.year == 0 {
                        t.year = v.get(..4).and_then(|y| y.parse().ok()).unwrap_or(0);
                    }
                }
                Some(StandardTagKey::TrackNumber) => {
                    let (a, b) = number(&v);
                    t.track_no = a;
                    if b > 0 {
                        t.track_count = b;
                    }
                }
                Some(StandardTagKey::TrackTotal) => t.track_count = v.parse().unwrap_or(0),
                Some(StandardTagKey::DiscNumber) => {
                    let (a, b) = number(&v);
                    t.disc_no = a;
                    if b > 0 {
                        t.disc_count = b;
                    }
                }
                Some(StandardTagKey::DiscTotal) => t.disc_count = v.parse().unwrap_or(0),
                Some(StandardTagKey::Bpm) => t.bpm = v.parse().unwrap_or(0),
                Some(StandardTagKey::Compilation) => t.compilation = v != "0" && !v.eq_ignore_ascii_case("false"),
                Some(StandardTagKey::ReplayGainTrackGain) => t.rg_track = db_value(&v),
                Some(StandardTagKey::ReplayGainAlbumGain) => t.rg_album = db_value(&v),
                _ => {}
            }
        }
        if !rev.visuals().is_empty() {
            t.has_art = true;
        }
    }
    if t.title.is_empty() {
        t.title = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    }
    if t.album_artist.is_empty() {
        t.album_artist = t.artist.clone();
    }
    Some(t)
}

/// The cover art in the file, if it carries one.
pub fn art(path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if !ext.is_empty() {
        hint.with_extension(&ext);
    }
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .ok()?;
    let mut format = probed.format;
    let mut out = None;
    if let Some(m) = probed.metadata.get().and_then(|mut m| m.skip_to_latest().cloned()) {
        if let Some(v) = m.visuals().first() {
            out = Some(v.data.to_vec());
        }
    }
    if out.is_none() {
        if let Some(m) = format.metadata().skip_to_latest() {
            if let Some(v) = m.visuals().first() {
                out = Some(v.data.to_vec());
            }
        }
    }
    out
}

/// The file types the library takes in.
pub const EXTS: &[&str] = &["mp3", "m4a", "m4b", "aac", "mp4", "flac", "ogg", "oga", "opus", "wav", "wave", "aif", "aiff", "aifc"];

pub fn is_music(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}
