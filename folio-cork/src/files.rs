//! Opening and saving files (K3-05).

use crate::model::*;
use std::path::Path;

pub fn save(doc: &Document, path: &Path) -> Result<(), String> {
    let mut doc = doc.clone();
    doc.prune_images();
    let png = path.extension().map(|e| e.eq_ignore_ascii_case("png")).unwrap_or(false);
    let bytes = if png { board_png(&doc)? } else { crate::vsdx::write(&doc)? };
    // Written beside the target first, so a failed save never damages the old file.
    folio_kit::platform::atomic_write(path, &bytes)
}

/// Files Cork saves back to in place: its own drawings, and diagram pictures from other apps.
pub fn keeps_path(path: &Path) -> bool {
    matches!(path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref(), Some("vsdx") | Some("png"))
}

pub fn load(path: &Path) -> Result<Document, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "vsdx" | "vsdm" => {
            let bytes = std::fs::read(path).map_err(|e| format!("Can't open {}: {e}", path.display()))?;
            crate::vsdx::read(&bytes)
        }
        "vsd" => load_vsd(path),
        "png" => {
            let bytes = std::fs::read(path).map_err(|e| format!("Can't open {}: {e}", path.display()))?;
            let board = board_in_png(&bytes).ok_or("This picture isn't a Cork diagram.")?;
            crate::vsdx::read(&board)
        }
        "csv" | "xlsx" => {
            let mut ed = crate::editor::Editor::new(Document::default());
            let mut text = folio_kit::text::Text::new();
            crate::layout::org_from_file(&mut ed, path, &mut text)?;
            ed.sel.clear();
            Ok(ed.doc)
        }
        _ => Err("Cork opens Visio drawings (.vsdx and .vsd).".into()),
    }
}

/// The private PNG chunk that carries a diagram inside its own picture (K3-08): ancillary,
/// private, and not safe to copy, so an editor that changes the pixels drops it.
pub const BOARD_CHUNK: &[u8; 4] = b"foLK";

/// A diagram as a picture another app can show (the first page's shapes, transparent around
/// them) with the whole drawing inside it, so double-clicking it there opens it in Cork again.
pub fn board_png(doc: &Document) -> Result<Vec<u8>, String> {
    let mut text = folio_kit::text::Text::new();
    let mut images = crate::render::Images::default();
    let shapes = doc.pages.first().map(|p| p.shapes.clone()).unwrap_or_default();
    let pm = crate::export::shapes_image(doc, &shapes, &[], &mut text, &mut images, 2.0)
        .or_else(|| folio_kit::gfx::sk::Pixmap::new(16, 16))
        .ok_or("The diagram is too large to show as a picture.")?;
    let png = folio_kit::gfx::encode_png(&pm);
    Ok(with_chunk(&png, BOARD_CHUNK, &crate::vsdx::write(doc)?))
}

/// PNG bytes with one more chunk just before IEND.
pub fn with_chunk(png: &[u8], name: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let iend = png.len().saturating_sub(12);
    let mut out = png[..iend].to_vec();
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(name);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
    out.extend_from_slice(&png[iend..]);
    out
}

/// The drawing carried inside a picture by `board_png`, if any.
pub fn board_in_png(png: &[u8]) -> Option<Vec<u8>> {
    if png.len() < 8 || &png[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let mut i = 8;
    while i + 12 <= png.len() {
        let n = u32::from_be_bytes(png[i..i + 4].try_into().ok()?) as usize;
        let name = &png[i + 4..i + 8];
        let end = i.checked_add(12)?.checked_add(n)?;
        if end > png.len() {
            return None;
        }
        if name == BOARD_CHUNK {
            let crc = u32::from_be_bytes(png[end - 4..end].try_into().ok()?);
            return (crc32(&png[i + 4..end - 4]) == crc).then(|| png[i + 8..end - 4].to_vec());
        }
        i = end;
    }
    None
}

fn crc32(b: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &x in b {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// Older binary .vsd files go through the system's converter (LibreOffice, when installed),
/// which turns each page into a picture.
fn load_vsd(path: &Path) -> Result<Document, String> {
    let png = folio_kit::platform::convert_with_office(path, "png").map_err(|e| {
        if e.contains("isn't installed") {
            "Older Visio files (.vsd) need LibreOffice to convert them, and it isn't installed. In Visio, save the drawing as .vsdx.".to_string()
        } else {
            "LibreOffice couldn't convert the drawing.".to_string()
        }
    })?;
    let bytes = std::fs::read(&png).map_err(|_| "LibreOffice couldn't convert the drawing.".to_string())?;
    if let Some(d) = png.parent() {
        let _ = std::fs::remove_dir_all(d);
    }
    let mut doc = Document::default();
    let key = doc.add_image(bytes.clone());
    let pm = folio_kit::gfx::decode_image(&bytes).ok_or("The converted drawing can't be read.")?;
    let id = doc.new_id();
    let layer = doc.pages[0].layers[0].id;
    let s = Shape { id, kind: "image".into(), x: 0.0, y: 0.0, w: pm.width() as f32, h: pm.height() as f32, image: key, layer, locked: true, style: Style { fill: folio_kit::Color::TRANSPARENT, width: 0.0, ..Default::default() }, ..Default::default() };
    doc.pages[0].shapes.push(s);
    doc.pages[0].name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("Page-1".into());
    Ok(doc)
}
