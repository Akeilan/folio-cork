//! Opening and saving files (K3-05).

use crate::model::*;
use std::path::Path;

pub fn save(doc: &Document, path: &Path) -> Result<(), String> {
    let mut doc = doc.clone();
    doc.prune_images();
    let bytes = crate::vsdx::write(&doc)?;
    // Write next to the target first so a failed save never damages the old file.
    let tmp = path.with_extension("vsdx.saving");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("Can't save {}: {e}", path.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Can't save {}: {e}", path.display())
    })
}

pub fn load(path: &Path) -> Result<Document, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "vsdx" | "vsdm" => {
            let bytes = std::fs::read(path).map_err(|e| format!("Can't open {}: {e}", path.display()))?;
            crate::vsdx::read(&bytes)
        }
        "vsd" => load_vsd(path),
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

/// Older binary .vsd files go through the system's converter (LibreOffice, when installed),
/// which turns each page into a picture.
fn load_vsd(path: &Path) -> Result<Document, String> {
    let out = std::env::temp_dir().join(format!("folio-cork-vsd-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&out);
    let soffice = ["soffice", "libreoffice"].iter().find(|c| std::process::Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)).copied();
    let Some(cmd) = soffice else {
        return Err("Older Visio files (.vsd) need LibreOffice to convert them, and it isn't installed. In Visio, save the drawing as .vsdx.".into());
    };
    let r = std::process::Command::new(cmd).args(["--headless", "--convert-to", "png", "--outdir"]).arg(&out).arg(path).output().map_err(|e| e.to_string())?;
    if !r.status.success() {
        return Err("LibreOffice couldn't convert the drawing.".into());
    }
    let png = out.join(path.with_extension("png").file_name().unwrap());
    let bytes = std::fs::read(&png).map_err(|_| "LibreOffice couldn't convert the drawing.".to_string())?;
    let _ = std::fs::remove_dir_all(&out);
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
