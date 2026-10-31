//! Mood boards (K3-11): pictures, palettes picked from them, tidy layouts, sections; and voting
//! results for whiteboards.

use crate::editor::{bbox_of, Editor};
use crate::geom;
use crate::model::*;
use crate::render::Images;
use folio_kit::gfx;
use folio_kit::Color;

/// Add a picture centred on `at`, scaled to a sensible size.
pub fn insert_picture(ed: &mut Editor, bytes: Vec<u8>, at: (f32, f32), images: &mut Images) -> Option<Id> {
    let pm = gfx::decode_image(&bytes)?;
    let (iw, ih) = (pm.width() as f32, pm.height() as f32);
    let key = ed.doc.add_image(bytes);
    let _ = images.get(&ed.doc, &key);
    let max = 360.0;
    let sc = (max / iw.max(ih)).min(1.0);
    let mut s = ed.make_shape("image", at.0, at.1);
    s.w = (iw * sc).round().max(8.0);
    s.h = (ih * sc).round().max(8.0);
    s.x = at.0 - s.w / 2.0;
    s.y = at.1 - s.h / 2.0;
    s.image = key;
    s.style.fill = Color::TRANSPARENT;
    s.style.line = Color::TRANSPARENT;
    s.style.width = 0.0;
    if ed.pg().mood {
        s.style.shadow = true;
    }
    Some(ed.add_shape(s, "Insert picture"))
}

/// The main colours of an image: a small k-means over sampled pixels.
pub fn palette(pm: &folio_kit::sk::Pixmap, crop: (f32, f32, f32, f32), k: usize) -> Vec<Color> {
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    let d = pm.data();
    let (x0, y0) = ((crop.0 * w as f32) as usize, (crop.1 * h as f32) as usize);
    let (x1, y1) = ((crop.2 * w as f32) as usize, (crop.3 * h as f32) as usize);
    let step = (((x1 - x0) * (y1 - y0)) as f32 / 6000.0).sqrt().max(1.0) as usize;
    let mut px: Vec<[f32; 3]> = vec![];
    let mut y = y0;
    while y < y1.min(h) {
        let mut x = x0;
        while x < x1.min(w) {
            let i = (y * w + x) * 4;
            let a = d[i + 3] as f32;
            if a > 128.0 {
                px.push([d[i] as f32 * 255.0 / a, d[i + 1] as f32 * 255.0 / a, d[i + 2] as f32 * 255.0 / a]);
            }
            x += step;
        }
        y += step;
    }
    if px.is_empty() {
        return vec![];
    }
    // Seed with k-means++ style spread: farthest-point picks, deterministic.
    let mut cent: Vec<[f32; 3]> = vec![px[px.len() / 2]];
    while cent.len() < k.min(px.len()) {
        let far = px
            .iter()
            .max_by(|a, b| {
                let da = cent.iter().map(|c| dist2(a, c)).fold(f32::MAX, f32::min);
                let db = cent.iter().map(|c| dist2(b, c)).fold(f32::MAX, f32::min);
                da.partial_cmp(&db).unwrap()
            })
            .copied()
            .unwrap();
        cent.push(far);
    }
    let mut counts = vec![0usize; cent.len()];
    for _ in 0..12 {
        let mut sums = vec![[0f32; 3]; cent.len()];
        counts = vec![0; cent.len()];
        for p in &px {
            let (ci, _) = cent.iter().enumerate().map(|(i, c)| (i, dist2(p, c))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
            for j in 0..3 {
                sums[ci][j] += p[j];
            }
            counts[ci] += 1;
        }
        for (i, c) in cent.iter_mut().enumerate() {
            if counts[i] > 0 {
                for j in 0..3 {
                    c[j] = sums[i][j] / counts[i] as f32;
                }
            }
        }
    }
    let mut out: Vec<(usize, Color)> = cent.iter().zip(counts).map(|(c, n)| (n, Color::rgb(c[0].round() as u8, c[1].round() as u8, c[2].round() as u8))).collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().filter(|x| x.0 > 0).map(|x| x.1).collect()
}

fn dist2(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Add swatches for the main colours of each selected picture. Returns how many pictures.
pub fn extract_palette(ed: &mut Editor, k: usize) -> usize {
    let pics: Vec<Shape> = ed.selected().into_iter().filter(|s| s.kind == "image").cloned().collect();
    if pics.is_empty() {
        return 0;
    }
    ed.checkpoint("Extract palette", "");
    let mut new_ids = vec![];
    for p in &pics {
        let Some(pm) = ed.doc.images.get(&p.image).and_then(|d| gfx::decode_image(&d.bytes)) else { continue };
        let cols = palette(&pm, p.crop, k);
        let (bx, by, _, bh) = geom::bounds(p);
        let sw = 70.0;
        for (i, c) in cols.iter().enumerate() {
            let mut s = ed.make_shape("swatch", 0.0, 0.0);
            s.w = sw;
            s.h = 86.0;
            s.x = bx + i as f32 * (sw + 8.0);
            s.y = by + bh + 14.0;
            s.style.fill = *c;
            s.data.insert("from".into(), p.id.to_string());
            new_ids.push(s.id);
            ed.pgm().shapes.push(s);
        }
    }
    ed.sel = new_ids;
    ed.changed();
    pics.len()
}

/// Hex codes of the selected swatches (or every swatch on the page).
pub fn palette_text(ed: &Editor) -> String {
    let sel: Vec<&Shape> = ed.selected().into_iter().filter(|s| s.kind == "swatch").collect();
    let all: Vec<&Shape> = if sel.is_empty() { ed.pg().shapes.iter().filter(|s| s.kind == "swatch").collect() } else { sel };
    all.iter().map(|s| s.style.fill.to_hex().to_uppercase()).collect::<Vec<_>>().join("\n")
}

/// Arrange the selection (or the page's loose items) in a grid or in masonry columns.
pub fn tidy(ed: &mut Editor, masonry: bool) {
    let page = ed.pg();
    let mut ids: Vec<Id> = if ed.sel.len() > 1 {
        ed.sel.clone()
    } else if ed.sel.len() == 1 && page.get(ed.sel[0]).map(|s| s.kind == "frame").unwrap_or(false) {
        // Tidy what's inside a selected frame.
        let f = page.get(ed.sel[0]).unwrap().clone();
        page.shapes.iter().filter(|s| s.group.is_none() && !s.is_connector() && s.kind != "frame" && inside(s, &f)).map(|s| s.id).collect()
    } else {
        page.shapes.iter().filter(|s| s.group.is_none() && !s.is_connector() && s.kind != "frame").map(|s| s.id).collect()
    };
    ids.retain(|id| ed.editable(*id) && ed.pg().get(*id).map(|s| !s.is_connector() && s.kind != "frame").unwrap_or(false));
    if ids.len() < 2 {
        return;
    }
    let Some((x0, y0, bw, _)) = bbox_of(ed.pg(), &ids) else { return };
    ed.checkpoint(if masonry { "Tidy masonry" } else { "Tidy grid" }, "");
    // Keep reading order: top-to-bottom, then left-to-right.
    ids.sort_by(|a, b| {
        let sa = ed.pg().get(*a).unwrap();
        let sb = ed.pg().get(*b).unwrap();
        ((sa.y / 40.0).round(), sa.x).partial_cmp(&((sb.y / 40.0).round(), sb.x)).unwrap()
    });
    let gap = 16.0;
    let n = ids.len();
    let cols = ((n as f32).sqrt().ceil() as usize).max(1).min(n);
    if masonry {
        let colw = ((bw - gap * (cols as f32 - 1.0)) / cols as f32).max(80.0).min(320.0);
        let mut heights = vec![y0; cols];
        for id in ids {
            let c = heights.iter().enumerate().min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|x| x.0).unwrap();
            let (w, h) = ed.pg().get(id).map(|s| (s.w, s.h)).unwrap();
            let nh = h * colw / w.max(1.0);
            let nx = x0 + c as f32 * (colw + gap);
            let ny = heights[c];
            place(ed, id, nx, ny, colw, nh);
            heights[c] += nh + gap;
        }
    } else {
        let cw = ids.iter().filter_map(|id| ed.pg().get(*id).map(|s| s.w)).fold(0.0, f32::max);
        let ch = ids.iter().filter_map(|id| ed.pg().get(*id).map(|s| s.h)).fold(0.0, f32::max);
        for (i, id) in ids.into_iter().enumerate() {
            let (c, r) = (i % cols, i / cols);
            let (w, h) = ed.pg().get(id).map(|s| (s.w, s.h)).unwrap();
            let nx = x0 + c as f32 * (cw + gap) + (cw - w) / 2.0;
            let ny = y0 + r as f32 * (ch + gap) + (ch - h) / 2.0;
            place(ed, id, nx, ny, w, h);
        }
    }
    ed.changed();
}

fn inside(s: &Shape, f: &Shape) -> bool {
    let (x, y, w, h) = geom::bounds(s);
    x >= f.x && y >= f.y && x + w <= f.x + f.w && y + h <= f.y + f.h
}

fn place(ed: &mut Editor, id: Id, x: f32, y: f32, w: f32, h: f32) {
    let is_group = ed.pg().get(id).map(|s| s.is_group()).unwrap_or(false);
    if is_group {
        let (ox, oy) = ed.pg().get(id).map(|s| (s.x, s.y)).unwrap();
        ed.shift(id, x - ox, y - oy);
        return;
    }
    if let Some(s) = ed.pgm().get_mut(id) {
        let sx = w / s.w.max(1.0);
        let sy = h / s.h.max(1.0);
        s.x = x;
        s.y = y;
        if s.kind == "ink" {
            s.points = s.points.iter().map(|&(px, py)| (px * sx, py * sy)).collect();
        }
        s.w = w;
        s.h = h;
    }
}

/// Put a titled frame around the selection.
pub fn section(ed: &mut Editor) {
    let Some((x, y, w, h)) = ed.sel_bounds() else { return };
    let pad = 24.0;
    let mut f = ed.make_shape("frame", 0.0, 0.0);
    f.x = x - pad;
    f.y = y - pad;
    f.w = w + 2.0 * pad;
    f.h = h + 2.0 * pad;
    f.text = "Section".into();
    f.style.fill = Color::rgba(255, 255, 255, 140);
    let id = ed.add_shape(f, "Add section");
    ed.begin_edit(id);
}

/// Shapes with votes, most first.
pub fn vote_results(ed: &Editor) -> String {
    let mut v: Vec<(&Shape, u32)> = ed.pg().shapes.iter().filter(|s| s.votes > 0).map(|s| (s, s.votes)).collect();
    if v.is_empty() {
        return "No votes on this page yet. Use the Vote tool and click shapes to add votes.".into();
    }
    v.sort_by(|a, b| b.1.cmp(&a.1));
    let total: u32 = v.iter().map(|x| x.1).sum();
    let mut out = format!("{total} votes\n\n");
    for (i, (s, n)) in v.iter().enumerate().take(12) {
        let name = if s.text.trim().is_empty() { format!("({})", crate::stencils::find(&s.kind).map(|x| x.name).unwrap_or(&s.kind)) } else { s.text.lines().next().unwrap_or("").to_string() };
        out.push_str(&format!("{}. {}  —  {} vote{}\n", i + 1, name, n, if *n == 1 { "" } else { "s" }));
    }
    out
}
