//! Drawing a page: shapes, text, pictures, ink and connectors, at any zoom. Used for the screen,
//! PNG export and printing.

use crate::geom::{self, Seg};
use crate::model::{Dash, Document, Id, Page, Shape, LANE_BAND};
use crate::route;
use folio_kit::gfx::{self, sk, Color};
use folio_kit::text::{Align, Text, TextStyle, VAlign};
use std::collections::HashMap;

/// Page-to-screen mapping: screen = page * zoom + offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub ox: f32,
    pub oy: f32,
}

impl Default for View {
    fn default() -> Self {
        View { zoom: 1.0, ox: 0.0, oy: 0.0 }
    }
}

impl View {
    pub fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        (x * self.zoom + self.ox, y * self.zoom + self.oy)
    }
    pub fn to_page(&self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.ox) / self.zoom, (y - self.oy) / self.zoom)
    }
    pub fn xf(&self) -> sk::Transform {
        sk::Transform::from_row(self.zoom, 0.0, 0.0, self.zoom, self.ox, self.oy)
    }
}

/// Decoded pictures, kept between frames.
#[derive(Default)]
pub struct Images {
    cache: HashMap<String, Option<sk::Pixmap>>,
}

impl Images {
    pub fn get(&mut self, doc: &Document, key: &str) -> Option<&sk::Pixmap> {
        if !self.cache.contains_key(key) {
            let pm = doc.images.get(key).and_then(|d| gfx::decode_image(&d.bytes)).map(|pm| {
                let max = 2400;
                if pm.width() > max || pm.height() > max {
                    let s = max as f32 / pm.width().max(pm.height()) as f32;
                    let (w, h) = ((pm.width() as f32 * s) as u32, (pm.height() as f32 * s) as u32);
                    let mut small = sk::Pixmap::new(w.max(1), h.max(1)).unwrap();
                    gfx::draw_image(&mut small, &pm, gfx::Rect::new(0.0, 0.0, w as f32, h as f32), 1.0, sk::FilterQuality::Bicubic);
                    small
                } else {
                    pm
                }
            });
            self.cache.insert(key.to_string(), pm);
        }
        self.cache.get(key).and_then(|x| x.as_ref())
    }
}

#[derive(Clone, Debug, Default)]
pub struct DrawOpts {
    /// Printing or exporting: skip layers not marked for printing.
    pub print: bool,
    /// A shape whose text is being edited (its text is drawn by the editor).
    pub hide_text: Option<Id>,
    /// Shapes not to draw at all (being dragged as a preview).
    pub skip: Vec<Id>,
    /// Only what touches this page-space box needs drawing (x0, y0, x1, y1): the screen.
    pub visible: Option<(f32, f32, f32, f32)>,
}

/// Where shapes are drawn: the screen's pixel buffer, or a recorder for SVG and PDF export.
pub trait Surface {
    fn fill(&mut self, p: &sk::Path, c: Color, opacity: f32, xf: sk::Transform);
    fn stroke(&mut self, p: &sk::Path, c: Color, opacity: f32, st: &sk::Stroke, xf: sk::Transform);
    /// A picture: `img_xf` maps picture pixels to local units, `frame` is the visible local box.
    fn image(&mut self, key: &str, img: &sk::Pixmap, frame: (f32, f32, f32, f32), img_xf: sk::Transform, opacity: f32, xf: sk::Transform);
}

impl Surface for sk::Pixmap {
    fn fill(&mut self, p: &sk::Path, c: Color, opacity: f32, xf: sk::Transform) {
        if c.a == 0 {
            return;
        }
        self.fill_path(p, &paint(c, opacity), sk::FillRule::Winding, xf, None);
    }
    fn stroke(&mut self, p: &sk::Path, c: Color, opacity: f32, st: &sk::Stroke, xf: sk::Transform) {
        if c.a == 0 {
            return;
        }
        self.stroke_path(p, &paint(c, opacity), st, xf, None);
    }
    fn image(&mut self, _key: &str, img: &sk::Pixmap, frame: (f32, f32, f32, f32), img_xf: sk::Transform, opacity: f32, xf: sk::Transform) {
        let mut p = sk::Paint::default();
        p.shader = sk::Pattern::new(img.as_ref(), sk::SpreadMode::Pad, sk::FilterQuality::Bicubic, opacity, img_xf);
        p.anti_alias = true;
        let (ix, iy, iw, ih) = frame;
        let r = [Seg::M(ix, iy), Seg::L(ix + iw, iy), Seg::L(ix + iw, iy + ih), Seg::L(ix, iy + ih), Seg::Z];
        if let Some(path) = geom::to_path(&r) {
            self.fill_path(&path, &p, sk::FillRule::Winding, xf, None);
        }
    }
}

pub fn paint(c: Color, opacity: f32) -> sk::Paint<'static> {
    let mut p = sk::Paint::default();
    let a = (c.a as f32 * opacity.clamp(0.0, 1.0)).round() as u8;
    p.set_color(c.with_alpha(a).to_sk());
    p.anti_alias = true;
    p
}

pub fn stroke(width: f32, dash: Dash) -> sk::Stroke {
    let mut st = sk::Stroke { width: width.max(0.1), line_cap: sk::LineCap::Round, line_join: sk::LineJoin::Round, ..Default::default() };
    let w = width.max(1.0);
    st.dash = match dash {
        Dash::Solid => None,
        Dash::Dash => sk::StrokeDash::new(vec![4.0 * w, 3.0 * w], 0.0),
        Dash::Dot => sk::StrokeDash::new(vec![0.1, 2.5 * w], 0.0),
        Dash::DashDot => sk::StrokeDash::new(vec![5.0 * w, 2.5 * w, 0.1, 2.5 * w], 0.0),
    };
    if dash != Dash::Solid {
        st.line_cap = if dash == Dash::Dot { sk::LineCap::Round } else { sk::LineCap::Butt };
    }
    st
}

/// Text style for a shape's text, in page units.
pub fn text_style(s: &Shape, size: f32) -> TextStyle {
    TextStyle {
        family: s.tstyle.family.clone(),
        size,
        bold: s.tstyle.bold,
        italic: s.tstyle.italic,
        underline: s.tstyle.underline,
        color: s.tstyle.color,
        align: s.tstyle.align,
        line: 1.2,
    }
}

/// The local text box and the font size that makes the text fit (stickies shrink their text).
pub fn text_fit(text: &mut Text, s: &Shape) -> ((f32, f32, f32, f32), f32) {
    let g = geom::geometry(s);
    let mut tb = g.text;
    if s.kind == "font" {
        tb = (0.0, 0.0, s.w, s.h);
    }
    let pad = if s.kind == "text" || s.kind == "font" { 2.0 } else { 6.0 };
    let tb = (tb.0 + pad, tb.1 + pad * 0.5, (tb.2 - 2.0 * pad).max(4.0), (tb.3 - pad).max(4.0));
    let mut size = s.tstyle.size;
    if s.kind == "sticky" && !s.text.is_empty() {
        while size > 7.0 {
            let lay = text.layout(&s.text, &text_style(s, size), Some(tb.2));
            if lay.height <= tb.3 && lay.width <= tb.2 + 0.5 {
                break;
            }
            size -= 1.0;
        }
    }
    (tb, size)
}

/// Text block origin (local) for a shape's text.
pub fn text_origin(text: &mut Text, s: &Shape) -> ((f32, f32), (f32, f32, f32, f32), f32, f32) {
    let (tb, size) = text_fit(text, s);
    let st = text_style(s, size);
    let lay = text.layout(&s.text, &st, Some(tb.2));
    let y = match s.tstyle.valign {
        VAlign::Top => tb.1,
        VAlign::Middle => tb.1 + (tb.3 - lay.height) / 2.0,
        VAlign::Bottom => tb.1 + tb.3 - lay.height,
    };
    ((tb.0, y), tb, size, lay.height)
}

fn fill_segs(pm: &mut dyn Surface, segs: &[Seg], c: Color, opacity: f32, xf: sk::Transform) {
    if c.a == 0 {
        return;
    }
    if let Some(p) = geom::to_path(segs) {
        pm.fill(&p, c, opacity, xf);
    }
}

fn stroke_segs(pm: &mut dyn Surface, segs: &[Seg], c: Color, opacity: f32, st: &sk::Stroke, xf: sk::Transform) {
    if c.a == 0 || st.width <= 0.0 {
        return;
    }
    if let Some(p) = geom::to_path(segs) {
        pm.stroke(&p, c, opacity, st, xf);
    }
}

/// Draw one shape (not connectors) with the page-to-device transform `view`.
pub fn draw_shape(pm: &mut dyn Surface, doc: &Document, s: &Shape, text: &mut Text, images: &mut Images, view: sk::Transform, hide_text: bool) {
    let xf = geom::shape_xf(s).post_concat(view);
    let op = s.style.opacity;
    match s.kind.as_str() {
        "group" => return,
        "ink" => {
            if s.points.len() < 2 {
                return;
            }
            let mut pb = sk::PathBuilder::new();
            pb.move_to(s.x + s.points[0].0, s.y + s.points[0].1);
            for w in s.points.windows(2).skip(0) {
                let m = ((w[0].0 + w[1].0) / 2.0, (w[0].1 + w[1].1) / 2.0);
                pb.quad_to(s.x + w[0].0, s.y + w[0].1, s.x + m.0, s.y + m.1);
            }
            let last = s.points[s.points.len() - 1];
            pb.line_to(s.x + last.0, s.y + last.1);
            if let Some(p) = pb.finish() {
                let mut st = stroke(s.style.width, Dash::Solid);
                if s.get("hl") == "1" {
                    st.line_cap = sk::LineCap::Square;
                }
                pm.stroke(&p, s.style.line, op, &st, view);
            }
            return;
        }
        "image" => {
            let polaroid = s.get("frame") == "polaroid";
            let rect = [Seg::M(0.0, 0.0), Seg::L(s.w, 0.0), Seg::L(s.w, s.h), Seg::L(0.0, s.h), Seg::Z];
            if s.style.shadow || polaroid {
                let sh = xf.pre_translate(3.0, 4.0);
                fill_segs(pm, &rect, Color::rgba(0, 0, 0, 40), op, sh);
            }
            let (ix, iy, iw, ih) = if polaroid {
                let m = (s.w.min(s.h) * 0.05).max(4.0);
                fill_segs(pm, &rect, Color::WHITE, op, xf);
                (m, m, s.w - 2.0 * m, s.h - 4.0 * m)
            } else {
                (0.0, 0.0, s.w, s.h)
            };
            if let Some(img) = images.get(doc, &s.image).cloned() {
                let (l, t, r, b) = s.crop;
                let cw = ((r - l) * img.width() as f32).max(1.0);
                let ch = ((b - t) * img.height() as f32).max(1.0);
                let sx = iw / cw;
                let sy = ih / ch;
                let pat_xf = sk::Transform::from_row(sx, 0.0, 0.0, sy, ix - l * img.width() as f32 * sx, iy - t * img.height() as f32 * sy);
                let img = img.clone();
                pm.image(&s.image, &img, (ix, iy, iw, ih), pat_xf, op, xf);
            } else {
                fill_segs(pm, &rect, Color::hex(0xDDDDDD), op, xf);
            }
            if s.style.width > 0.0 && s.style.line.a > 0 && !polaroid {
                stroke_segs(pm, &rect, s.style.line, op, &stroke(s.style.width, s.style.dash), xf);
            }
            if polaroid && !s.text.is_empty() && !hide_text {
                let st = TextStyle { size: (s.h * 0.05).clamp(9.0, 28.0), color: s.tstyle.color, align: Align::Center, family: s.tstyle.family.clone(), ..Default::default() };
                let lay = text.layout(&s.text, &st, Some(s.w));
                let y = s.h - (s.h * 0.035).max(3.0) * 2.0 - lay.height / 2.0 - (s.w.min(s.h) * 0.05).max(4.0) * 0.4;
                for p in text.outlines(&s.text, &st, Some(s.w)) {
                    pm.fill(&p, st.color, op, xf.pre_translate(0.0, y));
                }
            }
            draw_votes(pm, s, text, view);
            return;
        }
        "lane" => {
            // the lane: its body, the title band down the left (or along the top), and the title
            let v = s.lane_vertical();
            let rect = [Seg::M(0.0, 0.0), Seg::L(s.w, 0.0), Seg::L(s.w, s.h), Seg::L(0.0, s.h), Seg::Z];
            fill_segs(pm, &rect, s.style.fill, op, xf);
            let band = if v {
                [Seg::M(0.0, 0.0), Seg::L(s.w, 0.0), Seg::L(s.w, LANE_BAND), Seg::L(0.0, LANE_BAND), Seg::Z]
            } else {
                [Seg::M(0.0, 0.0), Seg::L(LANE_BAND, 0.0), Seg::L(LANE_BAND, s.h), Seg::L(0.0, s.h), Seg::Z]
            };
            fill_segs(pm, &band, s.style.fill.mix(s.style.line, 0.35), op, xf);
            let st = stroke(s.style.width.max(1.0), s.style.dash);
            stroke_segs(pm, &band, s.style.line, op, &st, xf);
            stroke_segs(pm, &rect, s.style.line, op, &st, xf);
            if !hide_text && !s.text.is_empty() {
                let ts = TextStyle { size: 13.0, bold: true, color: s.tstyle.color, family: s.tstyle.family.clone(), align: Align::Center, ..Default::default() };
                let along = if v { s.w } else { s.h };
                let lay = text.layout(&s.text, &ts, Some(along));
                // a lane running across the page has its title turned on its side, as Visio draws it
                let inner = if v {
                    xf.pre_translate(0.0, (LANE_BAND - lay.height) / 2.0)
                } else {
                    xf.pre_translate(LANE_BAND / 2.0, s.h / 2.0)
                        .pre_concat(crate::geom::rotation(-90.0))
                        .pre_translate(-along / 2.0, -lay.height / 2.0)
                };
                for p in text.outlines(&s.text, &ts, Some(along)) {
                    pm.fill(&p, ts.color, op, inner);
                }
            }
            draw_votes(pm, s, text, view);
            return;
        }
        "frame" => {
            let rect = [Seg::M(0.0, 0.0), Seg::L(s.w, 0.0), Seg::L(s.w, s.h), Seg::L(0.0, s.h), Seg::Z];
            fill_segs(pm, &rect, s.style.fill, op, xf);
            stroke_segs(pm, &rect, s.style.line, op, &stroke(s.style.width.max(1.0), s.style.dash), xf);
            if !hide_text {
                let st = TextStyle { size: 14.0, bold: true, color: s.tstyle.color, ..Default::default() };
                let title = if s.text.is_empty() { "Frame".to_string() } else { s.text.clone() };
                for p in text.outlines(&title, &st, None) {
                    pm.fill(&p, st.color, op, xf.pre_translate(2.0, -22.0));
                }
            }
            return;
        }
        _ => {}
    }
    let g = geom::geometry(s);
    let sticky = s.kind == "sticky";
    if s.style.shadow || sticky {
        let off = if sticky { (2.0, 4.0) } else { (3.0, 3.0) };
        for sub in g.subs.iter().filter(|x| x.fill && !x.solid) {
            fill_segs(pm, &sub.segs, Color::rgba(0, 0, 0, if sticky { 34 } else { 48 }), op, xf.post_translate(off.0 * view.sx, off.1 * view.sy));
        }
    }
    for sub in &g.subs {
        if sub.solid {
            fill_segs(pm, &sub.segs, s.style.line, op, xf);
        } else if sub.fill {
            fill_segs(pm, &sub.segs, s.style.fill, op, xf);
        }
    }
    if !sticky && s.kind != "text" && s.kind != "font" {
        let st = stroke(s.style.width, s.style.dash);
        for sub in &g.subs {
            if !sub.solid {
                stroke_segs(pm, &sub.segs, s.style.line, op, &st, xf);
            }
        }
    }
    if s.kind == "swatch" {
        let (x, y, w, h) = g.text;
        let band = [Seg::M(x, y), Seg::L(x + w, y), Seg::L(x + w, y + h), Seg::L(x, y + h), Seg::Z];
        fill_segs(pm, &band, Color::WHITE, op, xf);
        let st = TextStyle { size: (h * 0.36).clamp(8.0, 20.0), color: Color::hex(0x333333), align: Align::Center, ..Default::default() };
        let label = if s.text.is_empty() { s.style.fill.to_hex().to_uppercase() } else { s.text.clone() };
        let lay = text.layout(&label, &st, Some(w));
        for p in text.outlines(&label, &st, Some(w)) {
            pm.fill(&p, st.color, op, xf.pre_translate(x, y + (h - lay.height) / 2.0));
        }
        return;
    }
    if !s.text.is_empty() && !hide_text {
        draw_text(pm, s, text, xf, op);
    }
    draw_votes(pm, s, text, view);
}

pub fn draw_text(pm: &mut dyn Surface, s: &Shape, text: &mut Text, xf: sk::Transform, op: f32) {
    let ((x, y), tb, size, _) = text_origin(text, s);
    let st = text_style(s, size);
    let txf = xf.pre_translate(x, y);
    for p in text.outlines(&s.text, &st, Some(tb.2)) {
        pm.fill(&p, st.color, op, txf);
    }
}

fn draw_votes(pm: &mut dyn Surface, s: &Shape, text: &mut Text, view: sk::Transform) {
    if s.votes == 0 {
        return;
    }
    let (bx, by, bw, _) = geom::bounds(s);
    let (cx, cy) = (bx + bw - 2.0, by + 2.0);
    let r = 11.0;
    if let Some(p) = geom::to_path(&geom::ellipse_segs(cx, cy, r, r)) {
        pm.fill(&p, Color::hex(0xD13438), 1.0, view);
    }
    let st = TextStyle { size: 12.0, bold: true, color: Color::WHITE, align: Align::Center, ..Default::default() };
    let n = s.votes.to_string();
    let lay = text.layout(&n, &st, Some(2.0 * r));
    for p in text.outlines(&n, &st, Some(2.0 * r)) {
        pm.fill(&p, Color::WHITE, 1.0, view.pre_translate(cx - r, cy - lay.height / 2.0));
    }
}

/// Draw a connector with its arrowheads, label and line jumps.
pub fn draw_connector(pm: &mut dyn Surface, page: &Page, s: &Shape, text: &mut Text, view: sk::Transform, earlier: &[Vec<(f32, f32)>], hide_text: bool) -> Vec<(f32, f32)> {
    let Some(c) = &s.conn else { return vec![] };
    let r = route::route(page, s);
    if r.pts.len() < 2 {
        return vec![];
    }
    let op = s.style.opacity;
    let mut pts = r.pts.clone();
    // Pull the line back from filled arrowheads.
    let n = pts.len();
    if !r.curve {
        let ia = route::arrow_inset(c.arrow_a, s.style.width);
        let ib = route::arrow_inset(c.arrow_b, s.style.width);
        if ia > 0.0 {
            pts[0] = (pts[0].0 - r.dir_a.0 * ia, pts[0].1 - r.dir_a.1 * ia);
        }
        if ib > 0.0 {
            pts[n - 1] = (pts[n - 1].0 - r.dir_b.0 * ib, pts[n - 1].1 - r.dir_b.1 * ib);
        }
    }
    let segs = if r.curve {
        route::Routed { pts: pts.clone(), ..r.clone() }.segs()
    } else if c.jumps {
        route::with_jumps(&pts, earlier)
    } else {
        route::Routed { pts: pts.clone(), ..r.clone() }.segs()
    };
    let st = stroke(s.style.width, s.style.dash);
    if let Some(p) = geom::to_path(&segs) {
        pm.stroke(&p, s.style.line, op, &st, view);
    }
    for (kind, tip, dir) in [(c.arrow_a, r.pts[0], r.dir_a), (c.arrow_b, r.pts[r.pts.len() - 1], r.dir_b)] {
        if let Some((segs, filled)) = route::arrowhead(kind, tip, dir, s.style.width) {
            if let Some(p) = geom::to_path(&segs) {
                if filled {
                    pm.fill(&p, s.style.line, op, view);
                }
                let mut st2 = stroke(s.style.width, Dash::Solid);
                st2.line_join = sk::LineJoin::Miter;
                pm.stroke(&p, s.style.line, op, &st2, view);
            }
        }
    }
    if !s.text.is_empty() && !hide_text {
        let (mx, my) = r.midpoint();
        let st = text_style(s, s.tstyle.size);
        let lay = text.layout(&s.text, &st, None);
        let (w, h) = (lay.width + 8.0, lay.height + 2.0);
        let bg = [Seg::M(mx - w / 2.0, my - h / 2.0), Seg::L(mx + w / 2.0, my - h / 2.0), Seg::L(mx + w / 2.0, my + h / 2.0), Seg::L(mx - w / 2.0, my + h / 2.0), Seg::Z];
        fill_segs(pm, &bg, page.background, 1.0, view);
        let st = TextStyle { align: Align::Left, ..st };
        for p in text.outlines(&s.text, &st, None) {
            pm.fill(&p, st.color, op, view.pre_translate(mx - lay.width / 2.0, my - lay.height / 2.0));
        }
    }
    r.polyline()
}

/// Draw every visible shape of a page.
pub fn draw_page(pm: &mut dyn Surface, doc: &Document, page: &Page, text: &mut Text, images: &mut Images, view: sk::Transform, opts: &DrawOpts) {
    let mut earlier: Vec<Vec<(f32, f32)>> = vec![];
    for s in &page.shapes {
        if opts.skip.contains(&s.id) {
            continue;
        }
        let Some(layer) = page.layer(s.layer) else { continue };
        if !layer.visible || (opts.print && !layer.print) {
            continue;
        }
        let hide = opts.hide_text == Some(s.id);
        if s.is_connector() {
            let off = opts.visible.map(|v| route::rough_box(page, s).map(|b| b.2 < v.0 || b.0 > v.2 || b.3 < v.1 || b.1 > v.3).unwrap_or(true) || draw_box(page, s).map(|b| b.2 < v.0 || b.0 > v.2 || b.3 < v.1 || b.1 > v.3).unwrap_or(true)).unwrap_or(false);
            if off && !s.conn.as_ref().map(|c| c.jumps).unwrap_or(false) {
                continue;
            }
            let pl = draw_connector(pm, page, s, text, view, &earlier, hide);
            earlier.push(pl);
        } else if opts.visible.map(|v| !touches(v, s)).unwrap_or(false) {
            continue;
        } else {
            draw_shape(pm, doc, s, text, images, view, hide);
        }
    }
}

/// A page as an overview (the minimap): shapes as blocks of their colour, lines as lines, no
/// text. At that size text can't be read, and this stays fast while typing on a big board.
pub fn draw_overview(pm: &mut sk::Pixmap, page: &Page, view: sk::Transform) {
    for s in &page.shapes {
        if !page.visible(s) || s.is_group() {
            continue;
        }
        if s.is_connector() {
            let pl = route::route(page, s).polyline();
            let mut pb = sk::PathBuilder::new();
            for (i, p) in pl.iter().enumerate() {
                if i == 0 { pb.move_to(p.0, p.1) } else { pb.line_to(p.0, p.1) }
            }
            if let Some(path) = pb.finish() {
                let st = sk::Stroke { width: (1.0 / view.sx).max(s.style.width), ..Default::default() };
                pm.stroke_path(&path, &paint(s.style.line, s.style.opacity), &st, view, None);
            }
            continue;
        }
        let (x, y, w, h) = geom::bounds(s);
        let c = if s.style.fill.a > 0 { s.style.fill } else { s.style.line };
        if let Some(r) = sk::Rect::from_xywh(x, y, w.max(1.0), h.max(1.0)) {
            pm.fill_rect(r, &paint(c, s.style.opacity), view, None);
        }
    }
}

/// FNV-1a over text written into it (`write!`): a quick, exact-enough fingerprint.
#[derive(Default)]
pub struct Fnv(pub u64);
impl Fnv {
    pub fn bits(&mut self, v: f32) {
        use std::fmt::Write;
        let _ = self.write_str("");
        self.0 = (self.0 ^ v.to_bits() as u64).wrapping_mul(0x100000001b3);
    }
}
impl std::io::Write for Fnv {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if self.0 == 0 {
            self.0 = 0xcbf29ce484222325;
        }
        for x in b {
            self.0 = (self.0 ^ *x as u64).wrapping_mul(0x100000001b3);
        }
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl std::fmt::Write for Fnv {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        if self.0 == 0 {
            self.0 = 0xcbf29ce484222325;
        }
        for b in s.bytes() {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100000001b3);
        }
        Ok(())
    }
}

/// The page-space box a shape may draw into: its outline with room for lines, shadows and text.
pub fn draw_box(page: &Page, s: &Shape) -> Option<(f32, f32, f32, f32)> {
    if s.is_connector() {
        let pl = route::route(page, s).polyline();
        if pl.is_empty() {
            return None;
        }
        let (mut a, mut b, mut c, mut d) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in pl {
            a = a.min(p.0);
            b = b.min(p.1);
            c = c.max(p.0);
            d = d.max(p.1);
        }
        // Arrowheads and the label in the middle.
        let m = 16.0 + s.style.width * 4.0 + if s.text.is_empty() { 0.0 } else { s.tstyle.size * 2.0 + s.text.chars().count() as f32 * s.tstyle.size * 0.4 };
        return Some((a - m, b - m, c + m, d + m));
    }
    let (x, y, w, h) = geom::bounds(s);
    let m = 40.0 + s.style.width * 2.0 + s.tstyle.size * 2.0;
    Some((x - m, y - m, x + w + m, y + h + m))
}

/// A fingerprint and drawing box for each shape that touches the visible area: when either
/// changes between two frames, that part of the screen needs drawing again.
pub fn shape_prints(page: &Page, vis: (f32, f32, f32, f32)) -> std::collections::HashMap<Id, (u64, (f32, f32, f32, f32))> {
    use std::fmt::Write;
    let mut out = std::collections::HashMap::new();
    for s in &page.shapes {
        if s.is_connector() && route::rough_box(page, s).map(|b| b.2 < vis.0 || b.0 > vis.2 || b.3 < vis.1 || b.1 > vis.3).unwrap_or(true) {
            continue;
        }
        let Some(b) = draw_box(page, s) else { continue };
        if b.2 < vis.0 || b.0 > vis.2 || b.3 < vis.1 || b.1 > vis.3 {
            continue;
        }
        let mut h = Fnv::default();
        let _ = serde_json::to_writer(&mut h, s);
        let _ = write!(h, "{}", page.visible(s));
        if s.is_connector() {
            for p in route::route(page, s).pts {
                h.bits(p.0);
                h.bits(p.1);
            }
        }
        if let Some(g) = s.group {
            let _ = write!(h, "{g}");
        }
        out.insert(s.id, (h.0, b));
    }
    out
}

/// The page-space boxes of shapes whose fingerprint changed, that appeared or that went.
pub fn changed_boxes(old: &std::collections::HashMap<Id, (u64, (f32, f32, f32, f32))>, new: &std::collections::HashMap<Id, (u64, (f32, f32, f32, f32))>) -> Vec<(f32, f32, f32, f32)> {
    let mut v = vec![];
    for (id, (h, b)) in new {
        match old.get(id) {
            Some((oh, ob)) if oh == h && ob == b => {}
            Some((_, ob)) => {
                v.push(*ob);
                v.push(*b);
            }
            None => v.push(*b),
        }
    }
    for (id, (_, ob)) in old {
        if !new.contains_key(id) {
            v.push(*ob);
        }
    }
    v
}

/// Line jumps depend on every earlier connector: when a connector changed on a page that uses
/// them, the whole drawing is redrawn rather than worked out piece by piece.
pub fn jumps_changed(page: &Page, old: &std::collections::HashMap<Id, (u64, (f32, f32, f32, f32))>, new: &std::collections::HashMap<Id, (u64, (f32, f32, f32, f32))>) -> bool {
    if !page.shapes.iter().any(|s| s.conn.as_ref().map(|c| c.jumps).unwrap_or(false)) {
        return false;
    }
    page.shapes.iter().filter(|s| s.is_connector()).any(|s| old.get(&s.id).map(|x| x.0) != new.get(&s.id).map(|x| x.0))
}

/// Whether a shape (with room for its line, shadow and text spilling out) touches a box.
fn touches(v: (f32, f32, f32, f32), s: &Shape) -> bool {
    let (x, y, w, h) = geom::bounds(s);
    let m = 40.0 + s.style.width * 2.0 + s.tstyle.size * 2.0;
    !(x + w + m < v.0 || x - m > v.2 || y + h + m < v.1 || y - m > v.3)
}

/// The page-space box around everything drawn on a page.
pub fn content_bounds(page: &Page) -> Option<(f32, f32, f32, f32)> {
    // Asked for on every frame (minimap, page breaks): keep the answer while nothing moved.
    let fp = page_fingerprint(page);
    if let Some(b) = BOUNDS.with(|c| c.borrow().get(&fp).copied()) {
        return b;
    }
    let b = content_bounds_now(page);
    BOUNDS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 64 {
            c.clear();
        }
        c.insert(fp, b);
    });
    b
}

thread_local! {
    static BOUNDS: std::cell::RefCell<std::collections::HashMap<u64, Option<(f32, f32, f32, f32)>>> = Default::default();
}

/// Everything about a page's shapes that their outlines depend on, as one number.
pub fn page_fingerprint(page: &Page) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut f = |v: u64| h = (h ^ v).wrapping_mul(0x100000001b3);
    for l in &page.layers {
        f(l.id ^ (l.visible as u64) << 63);
    }
    for s in &page.shapes {
        f(s.id);
        f(s.layer);
        for v in [s.x, s.y, s.w, s.h, s.rot, s.style.width] {
            f(v.to_bits() as u64);
        }
        f(s.group.unwrap_or(0));
        if let Some(c) = &s.conn {
            for e in [&c.a, &c.b] {
                f(e.shape.unwrap_or(u64::MAX));
                f(e.point.map(|p| p as u64).unwrap_or(u64::MAX));
                f(e.x.to_bits() as u64 | (e.y.to_bits() as u64) << 32);
            }
            f(c.route as u64);
        }
        f(s.kind.len() as u64);
        f(s.points.len() as u64);
    }
    h
}

fn content_bounds_now(page: &Page) -> Option<(f32, f32, f32, f32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut any = false;
    for s in &page.shapes {
        if !page.visible(s) || s.is_group() {
            continue;
        }
        let (x, y, w, h) = if s.is_connector() {
            let pl = route::route(page, s).polyline();
            if pl.is_empty() {
                continue;
            }
            let (mut a, mut b, mut c, mut d) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for p in pl {
                a = a.min(p.0);
                b = b.min(p.1);
                c = c.max(p.0);
                d = d.max(p.1);
            }
            (a - 8.0, b - 8.0, c - a + 16.0, d - b + 16.0)
        } else if s.kind == "frame" {
            let (x, y, w, h) = geom::bounds(s);
            (x, y - 24.0, w, h + 24.0)
        } else {
            geom::bounds(s)
        };
        any = true;
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + w);
        y1 = y1.max(y + h);
    }
    if any {
        Some((x0, y0, x1 - x0, y1 - y0))
    } else {
        None
    }
}
