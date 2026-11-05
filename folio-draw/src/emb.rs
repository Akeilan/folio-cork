//! Embroidery documents (D3-06), in the same form the GTK version keeps them: the document's
//! settings (hoop, machine, threads) as JSON beside the picture, and each shape layer's
//! objects (outlines in mm with a thread and stitch settings) as the layer's description, with
//! its pixels drawn in the thread colours. The engine plans the stitches and writes the files.

use folio_draw_engine::doc::{poly_coverage, Doc};
use folio_draw_engine::emb::{formats, formats2, json as ej, plan};
use folio_kit::math::Det;
use serde_json::{json, Value};

pub const SHAPES_KIND: &str = "emb-shapes";
pub const PICTURE_KIND: &str = "emb-picture";
pub const PX_PER_MM: f32 = 10.0;

/// Hoops: name, width and height in mm. The Skitch PP1 takes the 4" × 4" one.
pub const HOOPS: [(&str, f32, f32); 5] = [
    ("4 × 4 in (100 × 100 mm)", 101.6, 101.6),
    ("5 × 7 in (130 × 180 mm)", 127.0, 177.8),
    ("6 × 10 in (160 × 260 mm)", 152.4, 254.0),
    ("8 × 8 in (200 × 200 mm)", 203.2, 203.2),
    ("8 × 12 in (200 × 300 mm)", 203.2, 304.8),
];

/// Stitch file formats: extension, label.
pub const FORMATS: [(&str, &str); 7] = [
    ("pes", "PES — Brother, Artspira"),
    ("dst", "DST — Tajima"),
    ("jef", "JEF — Janome"),
    ("exp", "EXP — Melco, Bernina"),
    ("vp3", "VP3 — Husqvarna Viking, Pfaff"),
    ("xxx", "XXX — Singer"),
    ("pec", "PEC — Brother"),
];

pub fn default_settings(hoop: usize) -> Value {
    let (name, w, h) = HOOPS[hoop.min(HOOPS.len() - 1)];
    json!({
        "hoop": name, "hoop_mm": [w, h], "machine": "Brother Skitch PP1", "fabric": [244, 239, 226],
        "threads": "brother", "trim": 2.0, "ties": true, "min": 0.3, "max": 7.0,
    })
}

/// The embroidery settings of a document, or None for an ordinary picture.
pub fn settings(doc: &Doc) -> Option<Value> {
    if doc.emb.trim().is_empty() {
        return None;
    }
    let v: Value = serde_json::from_str(&doc.emb).ok()?;
    let mut out = default_settings(0);
    if let (Some(o), Some(d)) = (out.as_object_mut(), v.as_object()) {
        for (k, x) in d {
            o.insert(k.clone(), x.clone());
        }
    }
    Some(out)
}

pub fn hoop_mm(s: &Value) -> (f32, f32) {
    let h = &s["hoop_mm"];
    (h[0].as_f64().unwrap_or(101.6) as f32, h[1].as_f64().unwrap_or(101.6) as f32)
}

pub fn canvas_px(s: &Value) -> (usize, usize) {
    let (w, h) = hoop_mm(s);
    ((w * PX_PER_MM).round() as usize, (h * PX_PER_MM).round() as usize)
}

/// mm position of pixel (0, 0): the hoop centre is (0, 0), y down.
pub fn origin(s: &Value) -> (f32, f32) {
    let (w, h) = hoop_mm(s);
    (-w / 2.0, -h / 2.0)
}

/// A new embroidery document: an empty hoop-sized canvas.
pub fn new_doc(hoop: usize) -> Doc {
    let s = default_settings(hoop);
    let (w, h) = canvas_px(&s);
    let mut d = Doc::empty(w, h);
    d.push_layer("Background", vec![0u8; w * h * 4], 1.0, true, 0);
    d.emb = s.to_string();
    d
}

pub fn layer_spec(doc: &Doc, i: usize) -> Option<Value> {
    let t = &doc.layers.get(i)?.text;
    if !t.starts_with('{') {
        return None;
    }
    let v: Value = serde_json::from_str(t).ok()?;
    (v["kind"] == SHAPES_KIND).then_some(v)
}

/// An imported design's blocks with its move, turn, mirror and scale applied.
fn transformed_stitches(o: &Value) -> Value {
    let tr = &o["transform"];
    let f = |k: &str, d: f64| tr[k].as_f64().unwrap_or(d) as f32;
    let (dx, dy) = (f("dx", 0.0), f("dy", 0.0));
    let ang = f("rotate", 0.0).to_radians();
    let sc = f("scale", 1.0);
    let sx = sc * if tr["mirror"].as_bool().unwrap_or(false) { -1.0 } else { 1.0 };
    let (c, s) = (ang.dcos(), ang.dsin());
    let blocks: Vec<Value> = o["blocks"]
        .as_array()
        .map(|bs| {
            bs.iter()
                .map(|b| {
                    let pts: Vec<Value> = b["pts"]
                        .as_array()
                        .map(|ps| {
                            ps.iter()
                                .map(|p| {
                                    let (x, y) = (p[0].as_f64().unwrap_or(0.0) as f32 * sx, p[1].as_f64().unwrap_or(0.0) as f32 * sc);
                                    json!([x * c - y * s + dx, x * s + y * c + dy, p[2]])
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    json!({"thread": b["thread"], "stop": b["stop"].as_bool().unwrap_or(false), "pts": pts})
                })
                .collect()
        })
        .unwrap_or_default();
    json!({"kind": "stitches", "blocks": blocks})
}

/// The shapes ([outline, holes...] each) an object stitches as.
fn object_shapes(o: &Value) -> Vec<Value> {
    match o["kind"].as_str() {
        Some("lettering") => o["glyphs"].as_array().map(|g| g.iter().filter(|x| x.as_array().map(|a| !a.is_empty()).unwrap_or(false)).cloned().collect()).unwrap_or_default(),
        Some("stitches") => vec![],
        _ => o.get("rings").filter(|r| r.as_array().map(|a| !a.is_empty()).unwrap_or(false)).cloned().into_iter().collect(),
    }
}

/// The planner's design: every visible shape layer, bottom first (the GTK version's
/// design_json, so both plan the same stitches).
pub fn design(doc: &Doc, s: &Value) -> Value {
    let defaults = json!({"type": "auto", "spacing": 0.35, "length": 3.5, "pull": 0.20, "underlay": true,
        "satin_spacing": 0.40, "min_width": 1.0, "run_length": 2.0, "bean": true, "cover": 3.0});
    let mut layers = vec![];
    for (i, l) in doc.layers.iter().enumerate() {
        if !l.visible {
            continue;
        }
        let Some(spec) = layer_spec(doc, i) else { continue };
        let mut objs = vec![];
        for o in spec["objects"].as_array().cloned().unwrap_or_default() {
            if o["hidden"].as_bool().unwrap_or(false) {
                continue;
            }
            if o["kind"] == "stitches" {
                objs.push(transformed_stitches(&o));
                continue;
            }
            let mut st = defaults.clone();
            if o["kind"] == "lettering" {
                let cap = o["cap"].as_f64().unwrap_or(10.0);
                st["type"] = json!(if cap < 8.0 { "fill" } else { "satin" });
            }
            if let (Some(m), Some(extra)) = (st.as_object_mut(), o["stitch"].as_object()) {
                for (k, v) in extra {
                    m.insert(k.clone(), v.clone());
                }
            }
            if st["angle"].is_null() {
                if let Some(m) = st.as_object_mut() {
                    m.remove("angle");
                }
            }
            let parts = object_shapes(&o);
            let n = parts.len();
            for (k, rings) in parts.into_iter().enumerate() {
                let mut d = json!({
                    "rings": rings,
                    "thread": if o["thread"].is_null() { json!({"rgb": [0, 0, 0]}) } else { o["thread"].clone() },
                    "stop": o["stop"].as_bool().unwrap_or(false) && k == n - 1,
                    "group": if o["kind"] != "lettering" { o["group"].clone() } else { json!("") },
                });
                if d["group"].is_null() {
                    d["group"] = json!("");
                }
                if let (Some(m), Some(x)) = (d.as_object_mut(), st.as_object()) {
                    for (k, v) in x {
                        m.insert(k.clone(), v.clone());
                    }
                }
                objs.push(d);
            }
        }
        layers.push(json!({"objects": objs}));
    }
    let (hw, hh) = hoop_mm(s);
    json!({"settings": {"hoop": [hw, hh], "trim": s["trim"], "ties": s["ties"], "min": s["min"], "max": s["max"]}, "layers": layers})
}

/// The stitches the engine plans for the document.
pub fn make_plan(doc: &Doc) -> Option<plan::Plan> {
    let s = settings(doc)?;
    let d = design(doc, &s);
    let j = ej::parse(&d.to_string()).ok()?;
    Some(plan::plan(&j))
}

/// A stitch file in the format its name asks for, checked by reading it back.
pub fn write(p: &plan::Plan, ext: &str, name: &str, date: &str) -> Result<Vec<u8>, String> {
    if p.stitch_count() == 0 {
        return Err("There are no stitches to save.".into());
    }
    let bytes = match ext {
        "dst" => formats::write_dst(p, name),
        "pec" => formats::write_pec_file(p, name),
        "jef" => formats2::write_jef(p, date),
        "exp" => formats2::write_exp(p),
        "vp3" => formats2::write_vp3(p, name),
        "xxx" => formats2::write_xxx(p),
        _ => formats::write_pes(p, name),
    };
    let back = formats::read_named(&bytes, ext)?;
    if back.stitch_count() + 2 < p.stitch_count() {
        return Err(format!("The {ext} file reads back with {} of {} stitches.", back.stitch_count(), p.stitch_count()));
    }
    Ok(bytes)
}

/// Pixels for a shape layer: every shape filled in its thread colour (straight RGBA, canvas size).
pub fn render_layer(spec: &Value, s: &Value) -> Vec<u8> {
    let (w, h) = canvas_px(s);
    let (ox, oy) = origin(s);
    let mut px = vec![0u8; w * h * 4];
    for o in spec["objects"].as_array().cloned().unwrap_or_default() {
        if o["hidden"].as_bool().unwrap_or(false) || o["kind"] == "stitches" {
            continue;
        }
        let rgb = &o["thread"]["rgb"];
        let c = [rgb[0].as_u64().unwrap_or(0) as u8, rgb[1].as_u64().unwrap_or(0) as u8, rgb[2].as_u64().unwrap_or(0) as u8];
        for rings in object_shapes(&o) {
            let contours: Vec<Vec<(f32, f32)>> = rings
                .as_array()
                .map(|rs| {
                    rs.iter()
                        .map(|r| {
                            r.as_array()
                                .map(|pts| pts.iter().map(|p| ((p[0].as_f64().unwrap_or(0.0) as f32 - ox) * PX_PER_MM, (p[1].as_f64().unwrap_or(0.0) as f32 - oy) * PX_PER_MM)).collect())
                                .unwrap_or_default()
                        })
                        .filter(|r: &Vec<(f32, f32)>| r.len() >= 3)
                        .collect()
                })
                .unwrap_or_default();
            if contours.is_empty() {
                continue;
            }
            let cov = poly_coverage(&contours, 0, 0, w, h, true);
            for (i, a) in cov.iter().enumerate() {
                if *a > 0.0 {
                    let o = i * 4;
                    let sa = a.min(1.0);
                    let da = px[o + 3] as f32 / 255.0;
                    let oa = sa + da * (1.0 - sa);
                    for k in 0..3 {
                        let d = px[o + k] as f32;
                        px[o + k] = ((c[k] as f32 * sa + d * da * (1.0 - sa)) / oa.max(1e-6)).round() as u8;
                    }
                    px[o + 3] = (oa * 255.0).round() as u8;
                }
            }
        }
    }
    px
}

/// Add a shape layer to an embroidery document.
pub fn add_shape_layer(doc: &mut Doc, name: &str, spec: &Value) -> usize {
    let Some(s) = settings(doc) else { return doc.active };
    let mut spec = spec.clone();
    spec["kind"] = json!(SHAPES_KIND);
    let px = render_layer(&spec, &s);
    let (w, h) = canvas_px(&s);
    // the engine takes premultiplied BGRA
    let mut block = vec![0u8; w * h * 4];
    for (d, p) in block.chunks_exact_mut(4).zip(px.chunks_exact(4)) {
        let a = p[3] as u32;
        d[0] = (p[2] as u32 * a / 255) as u8;
        d[1] = (p[1] as u32 * a / 255) as u8;
        d[2] = (p[0] as u32 * a / 255) as u8;
        d[3] = p[3];
    }
    doc.text_layer_add(name, &spec.to_string(), 0, 0, w, h, &block, w * 4)
}

/// Every stitch as a line in picture pixels, with its thread colour (for the stitch view).
pub fn stitch_lines(doc: &Doc, p: &plan::Plan) -> Vec<([u8; 3], Vec<(f32, f32)>)> {
    let Some(s) = settings(doc) else { return vec![] };
    let (ox, oy) = origin(&s);
    let mut out = vec![];
    for b in &p.blocks {
        let rgb = b.thread.rgb;
        let mut run = vec![];
        for &(x, y, c) in &b.cmds {
            let q = ((x - ox) * PX_PER_MM, (y - oy) * PX_PER_MM);
            if c == plan::ST {
                run.push(q);
            } else {
                if run.len() > 1 {
                    out.push((rgb, std::mem::take(&mut run)));
                }
                run.clear();
                run.push(q);
            }
        }
        if run.len() > 1 {
            out.push((rgb, run));
        }
    }
    out
}

/// A thread's name for a layer: its name, or its colour.
pub fn thread_label(t: &Value) -> String {
    if let Some(n) = t["name"].as_str().filter(|n| !n.is_empty()) {
        return n.to_string();
    }
    let c = &t["rgb"];
    format!("#{:02X}{:02X}{:02X}", c[0].as_u64().unwrap_or(0), c[1].as_u64().unwrap_or(0), c[2].as_u64().unwrap_or(0))
}

/// Put a picture on a new layer, fitted to the hoop, ready to convert (the GTK version's
/// Place picture). Returns the layer.
pub fn place_picture(doc: &mut Doc, w: usize, h: usize, rgba: Vec<u8>, name: &str) -> Option<usize> {
    use folio_draw_engine::emb::{convert, raster::Img};
    let s = settings(doc)?;
    let (cw, ch) = canvas_px(&s);
    let pl = convert::place(&Img { w, h, px: rgba }, cw, ch, PX_PER_MM, 2.5, false);
    doc.layer_add(name);
    doc.float_from_rgba(cw, ch, pl.img.px, 0, 0);
    doc.commit_float();
    let mut spec = json!({"kind": PICTURE_KIND, "upscale": pl.upscale, "source": name, "fitted": false});
    if let Some(b) = pl.bg {
        spec["bg"] = json!(b);
    }
    let i = doc.active;
    doc.text_layer_set(i, &spec.to_string(), None);
    Some(i)
}

/// The active layer's picture as shapes, one new layer per thread colour above it, the biggest
/// colour at the bottom (sewn first). Returns how many shapes and colours came out.
pub fn convert_layer(doc: &mut Doc) -> Result<(usize, usize), String> {
    use folio_draw_engine::emb::{convert, raster::Img};
    let s = settings(doc).ok_or("This picture isn't an embroidery design.")?;
    let mut li = doc.active;
    if layer_spec(doc, li).is_some() {
        // from a shape layer: the picture beneath it
        li = (0..li).rev().find(|&i| layer_spec(doc, i).is_none()).ok_or("Select the layer with the picture first.")?;
        doc.set_active(li);
    }
    let pspec: Value = serde_json::from_str(&doc.layers[li].text).unwrap_or(json!({}));
    let (cw, ch) = canvas_px(&s);
    let rgba = doc.layers[li].px.clone();
    if !rgba.chunks_exact(4).any(|p| p[3] > 0) {
        return Err("There is nothing on this layer to convert.".into());
    }
    let (ox, oy) = origin(&s);
    let mut tp = json!({"colours": 0, "background": "remove", "smoothing": 1, "min_area": 1.5, "edge_radius": 2,
        "upscale": pspec["upscale"].as_f64().unwrap_or(1.0), "threads": s["threads"].as_str().unwrap_or("brother")});
    if !pspec["bg"].is_null() {
        tp["bg"] = pspec["bg"].clone();
    }
    let pj = ej::parse(&tp.to_string())?;
    let c = convert::convert(&Img { w: cw, h: ch, px: rgba }, PX_PER_MM, ox, oy, &convert::Params::from_json(&pj));
    let shapes: Vec<Value> = c.shapes.iter().filter_map(|j| serde_json::from_str(&j.dump()).ok()).collect();
    if shapes.is_empty() {
        return Err("No shapes came out: the picture may be all background.".into());
    }
    let src = doc.layers[li].id;
    let group = format!("pic{src}");
    let mut by: Vec<(String, Vec<Value>)> = vec![];
    for sh in shapes {
        let key = sh["thread"].to_string();
        match by.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(sh),
            None => by.push((key, vec![sh])),
        }
    }
    let area = |v: &Vec<Value>| -> f64 { v.iter().map(|x| x["area"].as_f64().unwrap_or(0.0)).sum() };
    by.sort_by(|a, b| area(&b.1).partial_cmp(&area(&a.1)).unwrap_or(std::cmp::Ordering::Equal));
    let n: usize = by.iter().map(|(_, v)| v.len()).sum();
    let colours = by.len();
    doc.set_active(li);
    let mark = doc.undo_mark();
    for (_, list) in &by {
        let objects: Vec<Value> = list.iter().enumerate().map(|(k, sh)| json!({"id": k + 1, "rings": sh["rings"], "thread": sh["thread"], "group": group})).collect();
        let spec = json!({"kind": SHAPES_KIND, "source": src, "objects": objects, "next_id": list.len() + 1, "edited": false});
        let label = thread_label(&list[0]["thread"]);
        add_shape_layer(doc, &label, &spec);
    }
    doc.group_since(mark, "Convert to shapes");
    Ok((n, colours))
}

/// A shape layer's description and fresh pixels (one undo step).
pub fn store_layer(doc: &mut Doc, i: usize, spec: &Value) {
    let Some(s) = settings(doc) else { return };
    let px = render_layer(spec, &s);
    let (w, h) = canvas_px(&s);
    let mut block = vec![0u8; w * h * 4];
    for (d, p) in block.chunks_exact_mut(4).zip(px.chunks_exact(4)) {
        let a = p[3] as u32;
        d[0] = (p[2] as u32 * a / 255) as u8;
        d[1] = (p[1] as u32 * a / 255) as u8;
        d[2] = (p[0] as u32 * a / 255) as u8;
        d[3] = p[3];
    }
    doc.text_layer_set(i, &spec.to_string(), Some((0, 0, w, h, &block, w * 4)));
}

/// Picture pixels to mm and back.
pub fn px_to_mm(s: &Value, x: f32, y: f32) -> (f32, f32) {
    let (ox, oy) = origin(s);
    (x / PX_PER_MM + ox, y / PX_PER_MM + oy)
}

pub fn mm_to_px(s: &Value, x: f32, y: f32) -> (f32, f32) {
    let (ox, oy) = origin(s);
    ((x - ox) * PX_PER_MM, (y - oy) * PX_PER_MM)
}

/// (x0, y0, x1, y1) in mm of an object.
pub fn object_bounds(o: &Value) -> (f32, f32, f32, f32) {
    let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut add = |x: f32, y: f32| {
        b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
    };
    if o["kind"] == "stitches" {
        for bl in transformed_stitches(o)["blocks"].as_array().cloned().unwrap_or_default() {
            for p in bl["pts"].as_array().cloned().unwrap_or_default() {
                add(p[0].as_f64().unwrap_or(0.0) as f32, p[1].as_f64().unwrap_or(0.0) as f32);
            }
        }
    } else {
        for rings in object_shapes(o) {
            for r in rings.as_array().cloned().unwrap_or_default() {
                for p in r.as_array().cloned().unwrap_or_default() {
                    add(p[0].as_f64().unwrap_or(0.0) as f32, p[1].as_f64().unwrap_or(0.0) as f32);
                }
            }
        }
    }
    if b.0 > b.2 {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        b
    }
}

fn inside_rings(rings: &Value, x: f32, y: f32) -> bool {
    // even-odd over the outline and its holes
    let mut inside = false;
    for r in rings.as_array().cloned().unwrap_or_default() {
        let pts: Vec<(f32, f32)> = r.as_array().cloned().unwrap_or_default().iter().map(|p| (p[0].as_f64().unwrap_or(0.0) as f32, p[1].as_f64().unwrap_or(0.0) as f32)).collect();
        let n = pts.len();
        if n < 3 {
            continue;
        }
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = pts[i];
            let (xj, yj) = pts[j];
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

/// The topmost visible object under a point (mm): (layer, object id).
pub fn object_at(doc: &Doc, x: f32, y: f32) -> Option<(usize, i64)> {
    for i in (0..doc.layers.len()).rev() {
        if !doc.layers[i].visible {
            continue;
        }
        let Some(spec) = layer_spec(doc, i) else { continue };
        for o in spec["objects"].as_array().cloned().unwrap_or_default().iter().rev() {
            if o["hidden"].as_bool().unwrap_or(false) {
                continue;
            }
            let hit = if o["kind"] == "stitches" {
                let b = object_bounds(o);
                x >= b.0 - 0.6 && x <= b.2 + 0.6 && y >= b.1 - 0.6 && y <= b.3 + 0.6
            } else {
                object_shapes(o).iter().any(|r| inside_rings(r, x, y))
            };
            if hit {
                return Some((i, o["id"].as_i64().unwrap_or(0)));
            }
        }
    }
    None
}

/// Move an object by (dx, dy) mm.
pub fn move_object(o: &mut Value, dx: f32, dy: f32) {
    if o["kind"] == "stitches" {
        let t = &mut o["transform"];
        if !t.is_object() {
            *t = json!({});
        }
        t["dx"] = json!(t["dx"].as_f64().unwrap_or(0.0) + dx as f64);
        t["dy"] = json!(t["dy"].as_f64().unwrap_or(0.0) + dy as f64);
        return;
    }
    let shift = |rings: &mut Value| {
        if let Some(rs) = rings.as_array_mut() {
            for r in rs {
                if let Some(pts) = r.as_array_mut() {
                    for p in pts {
                        p[0] = json!(p[0].as_f64().unwrap_or(0.0) + dx as f64);
                        p[1] = json!(p[1].as_f64().unwrap_or(0.0) + dy as f64);
                    }
                }
            }
        }
    };
    if o["kind"] == "lettering" {
        if let Some(gs) = o["glyphs"].as_array_mut() {
            for g in gs {
                shift(g);
            }
        }
    } else {
        shift(&mut o["rings"]);
    }
}
