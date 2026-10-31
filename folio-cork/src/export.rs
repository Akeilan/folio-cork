//! Export (K3-05): PNG pictures, SVG drawings, PDF documents (also used for printing), and the
//! picture put on the clipboard for other apps.

use crate::model::*;
use crate::render::{self, DrawOpts, Images, Surface};
use folio_kit::gfx::{self, sk, Color};
use folio_kit::text::Text;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MARGIN: f32 = 24.0;

fn page_box(page: &Page) -> (f32, f32, f32, f32) {
    match render::content_bounds(page) {
        Some((x, y, w, h)) => (x - MARGIN, y - MARGIN, w + 2.0 * MARGIN, h + 2.0 * MARGIN),
        None => {
            let (w, h) = page.paper.px();
            (0.0, 0.0, w, h)
        }
    }
}

/// One output path per page: "name.png", or "name-1.png", "name-2.png"... for several pages.
fn numbered(path: &Path, i: usize, n: usize) -> PathBuf {
    if n <= 1 {
        return path.to_path_buf();
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = path.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    path.with_file_name(format!("{stem}-{}.{ext}", i + 1))
}

pub fn render_page(doc: &Document, page: &Page, text: &mut Text, images: &mut Images, scale: f32, bg: bool) -> Option<sk::Pixmap> {
    let (x, y, w, h) = page_box(page);
    let (pw, ph) = ((w * scale).ceil().max(1.0) as u32, (h * scale).ceil().max(1.0) as u32);
    if pw as u64 * ph as u64 > 400_000_000 {
        return None;
    }
    let mut pm = sk::Pixmap::new(pw, ph)?;
    if bg {
        pm.fill(page.background.to_sk());
    }
    let xf = sk::Transform::from_row(scale, 0.0, 0.0, scale, -x * scale, -y * scale);
    render::draw_page(&mut pm, doc, page, text, images, xf, &DrawOpts { print: true, ..Default::default() });
    Some(pm)
}

pub fn png(doc: &Document, pages: &[usize], path: &Path, text: &mut Text, images: &mut Images, scale: f32) -> Result<(), String> {
    for (i, &pi) in pages.iter().enumerate() {
        let page = &doc.pages[pi];
        let pm = render_page(doc, page, text, images, scale, true).ok_or("The page is too large to export at this resolution.")?;
        std::fs::write(numbered(path, i, pages.len()), gfx::encode_png(&pm)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A picture of some shapes (clipboard, thumbnails), transparent around them.
pub fn shapes_image(doc: &Document, shapes: &[Shape], imgs: &[(String, ImageData)], text: &mut Text, images: &mut Images, scale: f32) -> Option<sk::Pixmap> {
    let mut d = Document { pages: vec![], images: doc.images.clone(), next_id: doc.next_id };
    for (k, v) in imgs {
        d.images.insert(k.clone(), v.clone());
    }
    let page = Page { shapes: shapes.to_vec(), background: Color::TRANSPARENT, ..Default::default() };
    let mut p = page.clone();
    p.layers = vec![Layer { id: 0, ..Default::default() }];
    for s in p.shapes.iter_mut() {
        s.layer = 0;
    }
    let (x, y, w, h) = render::content_bounds(&p)?;
    let pad = 4.0;
    let (pw, ph) = (((w + 2.0 * pad) * scale).ceil() as u32, ((h + 2.0 * pad) * scale).ceil() as u32);
    if pw as u64 * ph as u64 > 64_000_000 {
        return None;
    }
    let mut pm = sk::Pixmap::new(pw.max(1), ph.max(1))?;
    let xf = sk::Transform::from_row(scale, 0.0, 0.0, scale, (pad - x) * scale, (pad - y) * scale);
    d.pages.push(p.clone());
    render::draw_page(&mut pm, &d, &p, text, images, xf, &DrawOpts::default());
    Some(pm)
}

// ---------------------------------------------------------------- vector recording

#[derive(Clone)]
enum Op {
    Fill { path: sk::Path, color: Color, alpha: f32 },
    Stroke { path: sk::Path, color: Color, alpha: f32, width: f32, dash: Option<Vec<f32>>, cap: sk::LineCap, join: sk::LineJoin },
    Image { key: String, w: u32, h: u32, frame: sk::Path, xf: sk::Transform, alpha: f32 },
}

/// Records drawing as page-space vector operations.
#[derive(Default)]
struct Rec {
    ops: Vec<Op>,
    imgs: HashMap<String, sk::Pixmap>,
}

fn scale_of(xf: &sk::Transform) -> f32 {
    ((xf.sx * xf.sx + xf.ky * xf.ky).sqrt() * (xf.kx * xf.kx + xf.sy * xf.sy).sqrt()).sqrt()
}

impl Surface for Rec {
    fn fill(&mut self, p: &sk::Path, c: Color, opacity: f32, xf: sk::Transform) {
        if c.a == 0 {
            return;
        }
        if let Some(path) = p.clone().transform(xf) {
            self.ops.push(Op::Fill { path, color: c, alpha: c.a as f32 / 255.0 * opacity });
        }
    }
    fn stroke(&mut self, p: &sk::Path, c: Color, opacity: f32, st: &sk::Stroke, xf: sk::Transform) {
        if c.a == 0 || st.width <= 0.0 {
            return;
        }
        let s = scale_of(&xf);
        if let Some(path) = p.clone().transform(xf) {
            let dash = st.dash.as_ref().map(|_| dash_of(st.width * s));
            self.ops.push(Op::Stroke { path, color: c, alpha: c.a as f32 / 255.0 * opacity, width: st.width * s, dash, cap: st.line_cap, join: st.line_join });
        }
    }
    fn image(&mut self, key: &str, img: &sk::Pixmap, frame: (f32, f32, f32, f32), img_xf: sk::Transform, opacity: f32, xf: sk::Transform) {
        let (x, y, w, h) = frame;
        let Some(r) = sk::Rect::from_xywh(x, y, w.max(0.01), h.max(0.01)) else { return };
        let Some(fp) = sk::PathBuilder::from_rect(r).transform(xf) else { return };
        self.imgs.entry(key.to_string()).or_insert_with(|| img.clone());
        self.ops.push(Op::Image { key: key.to_string(), w: img.width(), h: img.height(), frame: fp, xf: img_xf.post_concat(xf), alpha: opacity });
    }
}

/// Dashes can't be read back from tiny-skia, so vector output uses a standard pattern.
fn dash_of(w: f32) -> Vec<f32> {
    vec![4.0 * w.max(1.0), 3.0 * w.max(1.0)]
}

fn record(doc: &Document, page: &Page, text: &mut Text) -> Rec {
    let mut rec = Rec::default();
    let mut images = Images::default();
    render::draw_page(&mut rec, doc, page, text, &mut images, sk::Transform::identity(), &DrawOpts { print: true, ..Default::default() });
    rec
}

fn path_d(p: &sk::Path, dx: f32, dy: f32) -> String {
    let mut d = String::new();
    for seg in p.segments() {
        use sk::PathSegment as S;
        match seg {
            S::MoveTo(a) => d += &format!("M{:.2} {:.2}", a.x + dx, a.y + dy),
            S::LineTo(a) => d += &format!("L{:.2} {:.2}", a.x + dx, a.y + dy),
            S::QuadTo(a, b) => d += &format!("Q{:.2} {:.2} {:.2} {:.2}", a.x + dx, a.y + dy, b.x + dx, b.y + dy),
            S::CubicTo(a, b, c) => d += &format!("C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}", a.x + dx, a.y + dy, b.x + dx, b.y + dy, c.x + dx, c.y + dy),
            S::Close => d += "Z",
        }
    }
    d
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

pub fn svg_string(doc: &Document, page: &Page, text: &mut Text) -> String {
    let rec = record(doc, page, text);
    let (x, y, w, h) = page_box(page);
    let (dx, dy) = (-x, -y);
    let mut o = format!(
        "<?xml version='1.0' encoding='UTF-8'?>\n<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' width='{w:.0}' height='{h:.0}' viewBox='0 0 {w:.2} {h:.2}'>\n"
    );
    if page.background.a > 0 {
        o += &format!("<rect width='100%' height='100%' fill='{}'/>\n", hex(page.background));
    }
    let mut defs = String::new();
    let mut used_imgs: HashMap<String, String> = HashMap::new();
    for (i, op) in rec.ops.iter().enumerate() {
        match op {
            Op::Fill { path, color, alpha } => {
                o += &format!("<path d='{}' fill='{}'{}/>\n", path_d(path, dx, dy), hex(*color), if *alpha < 0.999 { format!(" fill-opacity='{alpha:.3}'") } else { String::new() });
            }
            Op::Stroke { path, color, alpha, width, dash, cap, join } => {
                let cap = match cap {
                    sk::LineCap::Round => "round",
                    sk::LineCap::Square => "square",
                    _ => "butt",
                };
                let join = match join {
                    sk::LineJoin::Round => "round",
                    sk::LineJoin::Bevel => "bevel",
                    _ => "miter",
                };
                o += &format!(
                    "<path d='{}' fill='none' stroke='{}' stroke-width='{width:.2}' stroke-linecap='{cap}' stroke-linejoin='{join}'{}{}/>\n",
                    path_d(path, dx, dy),
                    hex(*color),
                    if *alpha < 0.999 { format!(" stroke-opacity='{alpha:.3}'") } else { String::new() },
                    dash.as_ref().map(|d| format!(" stroke-dasharray='{:.2} {:.2}'", d[0], d[1])).unwrap_or_default()
                );
            }
            Op::Image { key, w, h, frame, xf, alpha } => {
                let id = match used_imgs.get(key) {
                    Some(id) => id.clone(),
                    None => {
                        let id = format!("img{}", used_imgs.len());
                        let bytes = doc.images.get(key).map(|d| (d.bytes.to_vec(), d.mime.clone())).unwrap_or_else(|| (gfx::encode_png(&rec.imgs[key]), "image/png".into()));
                        defs += &format!("<image id='{id}' width='{w}' height='{h}' preserveAspectRatio='none' xlink:href='data:{};base64,{}'/>\n", bytes.1, crate::model::b64_encode(&bytes.0));
                        used_imgs.insert(key.clone(), id.clone());
                        id
                    }
                };
                // Picture pixels -> page, with the page shifted to the export box.
                let full = xf.post_translate(dx, dy);
                defs += &format!("<clipPath id='clip{i}'><path d='{}'/></clipPath>\n", path_d(frame, dx, dy));
                o += &format!(
                    "<use xlink:href='#{id}' clip-path='url(#clip{i})' transform='matrix({:.5} {:.5} {:.5} {:.5} {:.3} {:.3})'{}/>\n",
                    full.sx,
                    full.ky,
                    full.kx,
                    full.sy,
                    full.tx,
                    full.ty,
                    if *alpha < 0.999 { format!(" opacity='{alpha:.3}'") } else { String::new() }
                );
                let _ = (w, h);
            }
        }
    }
    if !defs.is_empty() {
        o = o.replacen("'>\n", &format!("'>\n<defs>\n{defs}</defs>\n"), 1);
    }
    o += "</svg>\n";
    o
}

pub fn svg(doc: &Document, pages: &[usize], path: &Path, text: &mut Text) -> Result<(), String> {
    for (i, &pi) in pages.iter().enumerate() {
        let s = svg_string(doc, &doc.pages[pi], text);
        std::fs::write(numbered(path, i, pages.len()), s).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// PDF of the given pages. `fit`: each page scaled onto one sheet; otherwise actual size tiled
/// over as many sheets as needed. Sheets use each page's paper size and orientation.
pub fn pdf(doc: &Document, pages: &[usize], path: &Path, text: &mut Text, _images: &mut Images, fit: bool, _print: bool) -> Result<(), String> {
    let bytes = pdf_bytes(doc, pages, text, fit)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

pub fn pdf_bytes(doc: &Document, pages: &[usize], text: &mut Text, fit: bool) -> Result<Vec<u8>, String> {
    use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect as PRect, Ref};
    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog = alloc();
    let tree = alloc();
    let mut page_refs = vec![];
    // Images and alpha states are shared across pages.
    let mut img_refs: HashMap<String, (Ref, String)> = HashMap::new();
    let mut alphas: HashMap<u32, (Ref, String)> = HashMap::new();
    struct Sheet {
        ops: Vec<Op>,
        size: (f32, f32),
        xf: sk::Transform,
    }
    let mut sheets: Vec<Sheet> = vec![];
    let mut all_imgs: HashMap<String, sk::Pixmap> = HashMap::new();
    for &pi in pages {
        let page = &doc.pages[pi];
        let rec = record(doc, page, text);
        for (k, v) in &rec.imgs {
            all_imgs.entry(k.clone()).or_insert_with(|| v.clone());
        }
        let (pw, ph) = page.paper.px();
        let (sw, sh) = (pw * 0.75, ph * 0.75);
        let margin = 36.0;
        let b = page_box(page);
        if fit {
            let s = ((sw - 2.0 * margin) / (b.2 * 0.75)).min((sh - 2.0 * margin) / (b.3 * 0.75)).min(1.0) * 0.75;
            let ox = (sw - b.2 * s) / 2.0 - b.0 * s;
            let oy = (sh - b.3 * s) / 2.0 - b.1 * s;
            // PDF y points up: flip.
            let xf = sk::Transform::from_row(s, 0.0, 0.0, -s, ox, sh - oy);
            sheets.push(Sheet { ops: rec.ops.clone(), size: (sw, sh), xf });
        } else {
            let (aw, ah) = (sw - 2.0 * margin, sh - 2.0 * margin);
            let (cw, ch) = (b.2 * 0.75, b.3 * 0.75);
            let nx = (cw / aw).ceil().max(1.0) as usize;
            let ny = (ch / ah).ceil().max(1.0) as usize;
            for j in 0..ny.min(40) {
                for i in 0..nx.min(40) {
                    let ox = margin - b.0 * 0.75 - i as f32 * aw;
                    let oy = margin - b.1 * 0.75 - j as f32 * ah;
                    let xf = sk::Transform::from_row(0.75, 0.0, 0.0, -0.75, ox, sh - oy);
                    sheets.push(Sheet { ops: rec.ops.clone(), size: (sw, sh), xf });
                }
            }
        }
    }
    for (k, img) in &all_imgs {
        let r = alloc();
        let mask = alloc();
        let name = format!("Im{}", img_refs.len());
        let (w, h) = (img.width(), img.height());
        let rgba = gfx::rgba_from_pixmap(img);
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        let mut a = Vec::with_capacity((w * h) as usize);
        for p in rgba.chunks_exact(4) {
            rgb.extend_from_slice(&p[..3]);
            a.push(p[3]);
        }
        let rgb_z = miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6);
        let a_z = miniz_oxide::deflate::compress_to_vec_zlib(&a, 6);
        let mut x = pdf.image_xobject(r, &rgb_z);
        x.filter(Filter::FlateDecode);
        x.width(w as i32);
        x.height(h as i32);
        x.color_space().device_rgb();
        x.bits_per_component(8);
        x.s_mask(mask);
        x.finish();
        let mut m = pdf.image_xobject(mask, &a_z);
        m.filter(Filter::FlateDecode);
        m.width(w as i32);
        m.height(h as i32);
        m.color_space().device_gray();
        m.bits_per_component(8);
        m.finish();
        img_refs.insert(k.clone(), (r, name));
    }
    for sheet in &sheets {
        let mut c = Content::new();
        let xf = sheet.xf;
        let tp = |p: &sk::Path| p.clone().transform(xf);
        let emit = |c: &mut Content, p: &sk::Path| {
            let mut last = (0.0f32, 0.0f32);
            for seg in p.segments() {
                use sk::PathSegment as S;
                match seg {
                    S::MoveTo(a) => {
                        c.move_to(a.x, a.y);
                        last = (a.x, a.y);
                    }
                    S::LineTo(a) => {
                        c.line_to(a.x, a.y);
                        last = (a.x, a.y);
                    }
                    S::QuadTo(q, b) => {
                        // Quadratics become cubics; PDF has no quadratic operator.
                        let c1 = (last.0 + 2.0 / 3.0 * (q.x - last.0), last.1 + 2.0 / 3.0 * (q.y - last.1));
                        let c2 = (b.x + 2.0 / 3.0 * (q.x - b.x), b.y + 2.0 / 3.0 * (q.y - b.y));
                        c.cubic_to(c1.0, c1.1, c2.0, c2.1, b.x, b.y);
                        last = (b.x, b.y);
                    }
                    S::CubicTo(a, b, d) => {
                        c.cubic_to(a.x, a.y, b.x, b.y, d.x, d.y);
                        last = (d.x, d.y);
                    }
                    S::Close => {
                        c.close_path();
                    }
                }
            }
        };
        let mut alpha_name = |a: f32, alloc: &mut dyn FnMut() -> Ref| -> String {
            let k = (a * 1000.0).round() as u32;
            alphas.entry(k).or_insert_with(|| (alloc(), format!("Gs{k}"))).1.clone()
        };
        for op in &sheet.ops {
            match op {
                Op::Fill { path, color, alpha } => {
                    let Some(p) = tp(path) else { continue };
                    c.save_state();
                    if *alpha < 0.999 {
                        let n = alpha_name(*alpha, &mut alloc);
                        c.set_parameters(Name(n.as_bytes()));
                    }
                    c.set_fill_rgb(color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0);
                    emit(&mut c, &p);
                    c.fill_nonzero();
                    c.restore_state();
                }
                Op::Stroke { path, color, alpha, width, dash, cap, join } => {
                    let Some(p) = tp(path) else { continue };
                    c.save_state();
                    if *alpha < 0.999 {
                        let n = alpha_name(*alpha, &mut alloc);
                        c.set_parameters(Name(n.as_bytes()));
                    }
                    let s = scale_of(&xf);
                    c.set_stroke_rgb(color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0);
                    c.set_line_width(width * s);
                    c.set_line_cap(match cap {
                        sk::LineCap::Round => pdf_writer::types::LineCapStyle::RoundCap,
                        sk::LineCap::Square => pdf_writer::types::LineCapStyle::ProjectingSquareCap,
                        _ => pdf_writer::types::LineCapStyle::ButtCap,
                    });
                    c.set_line_join(match join {
                        sk::LineJoin::Round => pdf_writer::types::LineJoinStyle::RoundJoin,
                        sk::LineJoin::Bevel => pdf_writer::types::LineJoinStyle::BevelJoin,
                        _ => pdf_writer::types::LineJoinStyle::MiterJoin,
                    });
                    if let Some(d) = dash {
                        c.set_dash_pattern(d.iter().map(|v| v * s), 0.0);
                    }
                    emit(&mut c, &p);
                    c.stroke();
                    c.restore_state();
                }
                Op::Image { key, w, h, frame, xf: ixf, alpha } => {
                    let Some((_, name)) = img_refs.get(key) else { continue };
                    let Some(fp) = tp(frame) else { continue };
                    c.save_state();
                    if *alpha < 0.999 {
                        let n = alpha_name(*alpha, &mut alloc);
                        c.set_parameters(Name(n.as_bytes()));
                    }
                    emit(&mut c, &fp);
                    c.clip_nonzero();
                    c.end_path();
                    // Unit square -> picture pixels (flipped, since PDF images draw bottom-up) -> page -> sheet.
                    let unit = sk::Transform::from_row(*w as f32, 0.0, 0.0, -(*h as f32), 0.0, *h as f32);
                    let m = unit.post_concat(*ixf).post_concat(xf);
                    c.transform([m.sx, m.ky, m.kx, m.sy, m.tx, m.ty]);
                    c.x_object(Name(name.as_bytes()));
                    c.restore_state();
                }
            }
        }
        let content = alloc();
        let pref = alloc();
        let data = c.finish();
        let data = miniz_oxide::deflate::compress_to_vec_zlib(&data, 6);
        pdf.stream(content, &data).filter(Filter::FlateDecode);
        let mut page = pdf.page(pref);
        page.media_box(PRect::new(0.0, 0.0, sheet.size.0, sheet.size.1));
        page.parent(tree);
        page.contents(content);
        let mut res = page.resources();
        {
            let mut xo = res.x_objects();
            for (r, n) in img_refs.values() {
                xo.pair(Name(n.as_bytes()), *r);
            }
        }
        {
            let mut gs = res.ext_g_states();
            for (r, n) in alphas.values() {
                gs.pair(Name(n.as_bytes()), *r);
            }
        }
        res.finish();
        page.finish();
        page_refs.push(pref);
    }
    for (k, (r, _)) in &alphas {
        let a = *k as f32 / 1000.0;
        pdf.ext_graphics(*r).non_stroking_alpha(a).stroking_alpha(a);
    }
    pdf.pages(tree).kids(page_refs.iter().copied()).count(page_refs.len() as i32);
    pdf.catalog(catalog).pages(tree);
    Ok(pdf.finish())
}
