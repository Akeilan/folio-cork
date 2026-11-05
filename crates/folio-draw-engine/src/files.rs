//! Opening and saving pictures by content, in every format the engine codes itself.

use crate::doc::Doc;
use crate::{bmp, gif, ico, jpeg, ora, png, psd, tiff, webp};

/// Formats Draw saves, as the Save As list names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    OpenRaster,
    Bmp24,
    Bmp32,
    Jpeg,
    Gif,
    GifAnimation,
    Webp,
    WebpAnimation,
    Tiff,
    Ico,
    Psd,
}

impl Format {
    /// The format a file name asks for (the last dot), PNG when it says nothing known.
    pub fn from_name(name: &str) -> Format {
        let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "ora" => Format::OpenRaster,
            "bmp" | "dib" => Format::Bmp24,
            "jpg" | "jpeg" | "jpe" | "jfif" => Format::Jpeg,
            "gif" => Format::Gif,
            "webp" => Format::Webp,
            "tif" | "tiff" => Format::Tiff,
            "ico" => Format::Ico,
            "psd" => Format::Psd,
            _ => Format::Png,
        }
    }
    /// Whether this format keeps layers (otherwise the picture is flattened).
    pub fn keeps_layers(self) -> bool {
        matches!(self, Format::OpenRaster | Format::Psd | Format::GifAnimation | Format::WebpAnimation)
    }
}

/// A picture file's bytes as a document: by content, not by name.
pub fn load(data: &[u8]) -> Result<Doc, String> {
    if png::is_png(data) {
        png::decode(data).map(|i| Doc::from_rgba(i.w, i.h, i.px, "Background"))
    } else if bmp::is_bmp(data) {
        bmp::decode(data).map(|i| Doc::from_rgba(i.w, i.h, i.px, "Background"))
    } else if jpeg::is_jpeg(data) {
        jpeg::decode(data).map(|i| Doc::from_rgba(i.w, i.h, i.px, "Background"))
    } else if gif::is_gif(data) {
        gif::decode_frames(data).map(frames_doc)
    } else if webp::is_webp(data) {
        webp::decode_frames(data).map(frames_doc)
    } else if tiff::is_tiff(data) {
        tiff::decode(data).map(|i| Doc::from_rgba(i.w, i.h, i.px, "Background"))
    } else if ico::is_ico(data) {
        ico::decode(data).map(|i| Doc::from_rgba(i.w, i.h, i.px, "Background"))
    } else if data.len() > 4 && &data[..4] == b"PK\x03\x04" {
        ora::load(data)
    } else if psd::is_psd(data) {
        psd::load(data)
    } else {
        Err("Folio Draw can't read this kind of picture.".into())
    }
}

/// A picture file's bytes as one RGBA image (for pasting or inserting a picture).
pub fn decode(data: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let doc = load(data)?;
    Ok((doc.w, doc.h, doc.flatten_rgba()))
}

/// An animation as a document: every frame becomes a layer, bottom = first frame.
fn frames_doc(f: gif::Frames) -> Doc {
    if f.frames.len() == 1 {
        let (px, _) = f.frames.into_iter().next().unwrap();
        return Doc::from_rgba(f.w, f.h, px, "Background");
    }
    let mut doc = Doc::empty(f.w, f.h);
    for (i, (px, delay)) in f.frames.into_iter().enumerate() {
        doc.push_layer(&format!("Frame {}", i + 1), px, 1.0, true, 0);
        doc.layers.last_mut().unwrap().delay_ms = delay;
    }
    doc.active = 0;
    doc.solo = true;
    doc.gif_loop = f.loop_count.map_or(-1, |v| v as i32);
    doc.frames_truncated = f.truncated;
    doc
}

/// Layers as animation frames (bottom first), each with its opacity applied.
fn layer_frames(doc: &Doc) -> Vec<Vec<u8>> {
    doc.layers
        .iter()
        .map(|l| {
            let mut px = l.px.clone();
            if l.opacity < 1.0 {
                for p in px.chunks_exact_mut(4) {
                    p[3] = (p[3] as f32 * l.opacity + 0.5) as u8;
                }
            }
            px
        })
        .collect()
}

/// The document's bytes in a format; flattened formats are put on `matte` where they can't
/// keep see-through pixels.
pub fn save(doc: &mut Doc, format: Format, matte: [u8; 3]) -> Vec<u8> {
    doc.commit_float();
    let anim = |doc: &Doc| {
        let frames = layer_frames(doc);
        let lc = if doc.gif_loop < 0 { None } else { Some(doc.gif_loop.min(65535) as u16) };
        let delays: Vec<u32> = doc.layers.iter().map(|l| if l.delay_ms == 0 { 100 } else { l.delay_ms }).collect();
        (frames, delays, lc)
    };
    match format {
        Format::Png => png::encode(doc.w, doc.h, &doc.flatten_rgba(), 7),
        Format::OpenRaster => ora::save(doc),
        Format::Bmp24 => bmp::encode(doc.w, doc.h, &doc.flatten_rgba(), false, matte),
        Format::Bmp32 => bmp::encode(doc.w, doc.h, &doc.flatten_rgba(), true, [255, 255, 255]),
        Format::Jpeg => {
            let mut px = doc.flatten_rgba();
            for p in px.chunks_exact_mut(4) {
                let a = p[3] as u32;
                for k in 0..3 {
                    p[k] = ((p[k] as u32 * a + matte[k] as u32 * (255 - a) + 127) / 255) as u8;
                }
                p[3] = 255;
            }
            jpeg::encode(doc.w, doc.h, &px, 92)
        }
        Format::Gif => gif::encode(doc.w, doc.h, &doc.flatten_rgba()),
        Format::GifAnimation => {
            let (frames, delays, lc) = anim(doc);
            let refs: Vec<(&[u8], u32)> = frames.iter().zip(delays).map(|(f, d)| (f.as_slice(), d)).collect();
            gif::encode_animation(doc.w, doc.h, &refs, lc)
        }
        Format::Webp => webp::encode(doc.w, doc.h, &doc.flatten_rgba()),
        Format::WebpAnimation => {
            let (frames, delays, lc) = anim(doc);
            let refs: Vec<(&[u8], u32)> = frames.iter().zip(delays).map(|(f, d)| (f.as_slice(), d)).collect();
            webp::encode_animation(doc.w, doc.h, &refs, lc)
        }
        Format::Tiff => tiff::encode(doc.w, doc.h, &doc.flatten_rgba()),
        Format::Ico => ico::encode(doc.w, doc.h, &doc.flatten_rgba()),
        Format::Psd => psd::save(doc),
    }
}
