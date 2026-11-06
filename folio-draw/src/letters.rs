//! Lettering for embroidery: glyph outlines from the bundled fonts (vector, not traced pixels),
//! set straight or on an arc, in millimetres, and envelopes that bend them.
//!
//! Outlines come from `folio_kit::text`, so a word is built from the same curves on every machine.
//! Curves are flattened here with plain arithmetic and square roots only (portability rule 3).

use folio_kit::gfx::sk;
use folio_kit::math::Det;
use folio_kit::text::{Text, TextStyle};
use serde_json::{json, Value};

/// Outlines are taken at this size in pixels and scaled down to millimetres.
const REF: f32 = 200.0;
/// A flattened curve step, in reference pixels.
const FLAT: f32 = 0.6;

pub const ENVELOPES: [(&str, &str); 7] = [
    ("none", "None"),
    ("arch_up", "Arch up"),
    ("arch_down", "Arch down"),
    ("bulge", "Bulge"),
    ("pinch", "Pinch"),
    ("wave", "Wave"),
    ("slant", "Slant"),
];

type Ring = Vec<[f32; 2]>;
/// An outline ring and its holes.
type Shape = Vec<Ring>;

fn style(font: &str, size: f32) -> TextStyle {
    let bold = font.ends_with(" Bold");
    let family = if bold { font[..font.len() - 5].to_string() } else { font.to_string() };
    TextStyle { family, size, bold, italic: false, ..TextStyle::default() }
}

/// The fonts lettering can be set in: the ones Folio ships, so a design stitches the same anywhere.
pub fn fonts(text: &mut Text) -> Vec<String> {
    let mut out = vec![];
    for f in text.families() {
        out.push(f.clone());
        out.push(format!("{f} Bold"));
    }
    out
}

fn flatten(p: &sk::Path) -> Vec<Ring> {
    let mut rings: Vec<Ring> = vec![];
    let mut cur: Ring = vec![];
    let mut at = [0.0f32, 0.0];
    let steps = |a: [f32; 2], b: [f32; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        (((dx * dx + dy * dy).sqrt() / FLAT).ceil() as usize).clamp(1, 64)
    };
    for seg in p.segments() {
        use sk::PathSegment as S;
        match seg {
            S::MoveTo(a) => {
                if cur.len() >= 3 {
                    rings.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
                at = [a.x, a.y];
                cur.push(at);
            }
            S::LineTo(a) => {
                at = [a.x, a.y];
                cur.push(at);
            }
            S::QuadTo(c, b) => {
                let (c, b) = ([c.x, c.y], [b.x, b.y]);
                let n = steps(at, b).max(steps(at, c));
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    cur.push([
                        u * u * at[0] + 2.0 * u * t * c[0] + t * t * b[0],
                        u * u * at[1] + 2.0 * u * t * c[1] + t * t * b[1],
                    ]);
                }
                at = b;
            }
            S::CubicTo(c1, c2, b) => {
                let (c1, c2, b) = ([c1.x, c1.y], [c2.x, c2.y], [b.x, b.y]);
                let n = steps(at, b).max(steps(at, c1)).max(steps(c2, b));
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    cur.push([
                        u * u * u * at[0] + 3.0 * u * u * t * c1[0] + 3.0 * u * t * t * c2[0] + t * t * t * b[0],
                        u * u * u * at[1] + 3.0 * u * u * t * c1[1] + 3.0 * u * t * t * c2[1] + t * t * t * b[1],
                    ]);
                }
                at = b;
            }
            S::Close => {
                if cur.len() >= 3 {
                    rings.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
        }
    }
    if cur.len() >= 3 {
        rings.push(cur);
    }
    rings
}

fn area(r: &Ring) -> f32 {
    let n = r.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = r[i];
        let q = r[(i + 1) % n];
        a += p[0] * q[1] - q[0] * p[1];
    }
    a / 2.0
}

fn inside(r: &Ring, x: f32, y: f32) -> bool {
    let n = r.len();
    let mut c = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (r[i][0], r[i][1]);
        let (xj, yj) = (r[j][0], r[j][1]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi + 1e-12) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

/// Outline rings and their holes, grouped by how deeply each sits inside the others.
pub fn group_rings(rings: Vec<Ring>) -> Vec<Shape> {
    let rings: Vec<Ring> = rings.into_iter().filter(|r| area(r).abs() > 1e-6).collect();
    let depth: Vec<usize> = rings
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let (px, py) = (r[0][0], r[0][1]);
            rings.iter().enumerate().filter(|(j, o)| *j != i && area(o).abs() > area(r).abs() && inside(o, px, py)).count()
        })
        .collect();
    let mut out = vec![];
    for (i, r) in rings.iter().enumerate() {
        if depth[i] % 2 == 1 {
            continue;
        }
        let mut shape = vec![r.clone()];
        for (j, h) in rings.iter().enumerate() {
            if depth[j] == depth[i] + 1 && inside(r, h[0][0], h[0][1]) {
                shape.push(h.clone());
            }
        }
        out.push(shape);
    }
    out
}

/// A capital H measured in reference pixels: how tall it is, and where its baseline sits in the
/// outlines' own coordinates (they are laid out from the text's top left, not the baseline).
fn cap_px(text: &mut Text, st: &TextStyle) -> (f32, f32) {
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for p in text.outlines("H", st, None) {
        for r in flatten(&p) {
            for q in r {
                lo = lo.min(q[1]);
                hi = hi.max(q[1]);
            }
        }
    }
    if hi > lo {
        (hi - lo, hi)
    } else {
        (st.size * 0.7, st.size * 0.8)
    }
}

/// Shapes (each an outline and its holes) for a lettering object, in millimetres from the hoop's
/// centre: text, font, cap (mm), spacing (mm), arc ("none", "top", "bottom"), radius (mm), x, y.
pub fn lettering_shapes(text: &mut Text, o: &Value) -> Vec<Shape> {
    let s = o["text"].as_str().unwrap_or("");
    let font = o["font"].as_str().unwrap_or("Carlito Bold");
    let cap = o["cap"].as_f64().unwrap_or(10.0) as f32;
    let spacing = o["spacing"].as_f64().unwrap_or(0.0) as f32;
    let (cx, cy) = (o["x"].as_f64().unwrap_or(0.0) as f32, o["y"].as_f64().unwrap_or(0.0) as f32);
    let arc = o["arc"].as_str().unwrap_or("none");
    let radius = (o["radius"].as_f64().unwrap_or(40.0) as f32).max(1.0);
    if s.is_empty() || cap <= 0.0 {
        return vec![];
    }
    let st = style(font, REF);
    let (caph, base) = cap_px(text, &st);
    let k = cap / caph.max(1e-3);
    // one glyph at a time: the advances place them, so an arc can turn each one on its own
    let chars: Vec<char> = s.chars().collect();
    let adv: Vec<f32> = chars.iter().map(|c| text.measure(&c.to_string(), &st).0 * k).collect();
    let total: f32 = adv.iter().sum::<f32>() + spacing * (chars.len().max(1) - 1) as f32;
    let mut glyphs: Vec<Vec<Ring>> = vec![];
    let mut pos = 0.0f32;
    for (i, ch) in chars.iter().enumerate() {
        let a = adv[i];
        let mid = pos + a / 2.0;
        if !ch.is_whitespace() {
            let raw: Vec<Ring> = text.outlines(&ch.to_string(), &st, None).iter().flat_map(flatten).collect();
            // the outline sits with its baseline on y = 0; put the glyph's middle at the origin
            let (bx, by, angle) = match arc {
                "top" => {
                    let th = (mid - total / 2.0) / radius;
                    (cx + radius * th.dsin(), cy - radius * th.dcos(), th)
                }
                "bottom" => {
                    let rb = radius + cap;
                    let th = (mid - total / 2.0) / radius;
                    (cx + rb * th.dsin(), cy + rb * th.dcos(), -th)
                }
                _ => (cx - total / 2.0 + mid, cy + cap / 2.0, 0.0),
            };
            let (ca, sa) = (angle.dcos(), angle.dsin());
            let mut rings = vec![];
            for r in raw {
                let ring: Ring = r
                    .iter()
                    .map(|p| {
                        let (gx, gy) = (p[0] * k - a / 2.0, (p[1] - base) * k);
                        [bx + gx * ca - gy * sa, by + gx * sa + gy * ca]
                    })
                    .collect();
                rings.push(ring);
            }
            glyphs.push(rings);
        }
        pos += a + spacing;
    }
    // each glyph is grouped on its own: one letter's hole never belongs to its neighbour
    glyphs.into_iter().flat_map(group_rings).collect()
}

/// A least-squares circle through points: (centre x, centre y, radius, how far off they are on
/// average). None when they are too few or lie on a line.
pub fn fit_circle(pts: &[(f32, f32)]) -> Option<(f32, f32, f32, f32)> {
    let n = pts.len();
    if n < 3 {
        return None;
    }
    let nf = n as f64;
    let mx = pts.iter().map(|p| p.0 as f64).sum::<f64>() / nf;
    let my = pts.iter().map(|p| p.1 as f64).sum::<f64>() / nf;
    let (mut suu, mut svv, mut suv, mut suuu, mut svvv, mut suvv, mut svuu) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
    for p in pts {
        let (u, v) = (p.0 as f64 - mx, p.1 as f64 - my);
        suu += u * u;
        svv += v * v;
        suv += u * v;
        suuu += u * u * u;
        svvv += v * v * v;
        suvv += u * v * v;
        svuu += v * u * u;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-9 {
        return None;
    }
    let a = 0.5 * (suuu + suvv);
    let b = 0.5 * (svvv + svuu);
    let uc = (a * svv - b * suv) / det;
    let vc = (b * suu - a * suv) / det;
    let (cx, cy) = (uc + mx, vc + my);
    let r = (uc * uc + vc * vc + (suu + svv) / nf).sqrt();
    let rms = (pts.iter().map(|p| {
        let (dx, dy) = (p.0 as f64 - cx, p.1 as f64 - cy);
        let d = (dx * dx + dy * dy).sqrt() - r;
        d * d
    }).sum::<f64>() / nf).sqrt();
    Some((cx as f32, cy as f32, r as f32, rms as f32))
}

/// How far lettering spans: across, or along its arc.
pub fn span_of(shapes: &[Shape], o: &Value) -> f32 {
    if shapes.is_empty() {
        return 0.0;
    }
    let arc = o["arc"].as_str().unwrap_or("none");
    let b = bounds(shapes);
    if arc == "none" {
        return b.2 - b.0;
    }
    let (cx, cy) = (o["x"].as_f64().unwrap_or(0.0) as f32, o["y"].as_f64().unwrap_or(0.0) as f32);
    let radius = (o["radius"].as_f64().unwrap_or(40.0) as f32).max(1.0);
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for g in shapes {
        for r in g.iter().take(1) {
            for p in r {
                let dy = if arc == "top" { -(p[1] - cy) } else { p[1] - cy };
                let a = (p[0] - cx).datan2(dy);
                lo = lo.min(a);
                hi = hi.max(a);
            }
        }
    }
    if hi > lo {
        (hi - lo) * radius
    } else {
        b.2 - b.0
    }
}

/// Fill beats satin for lettering under 8 mm cap height, built from outlines.
pub fn default_type(cap: f32) -> &'static str {
    if cap < 8.0 {
        "fill"
    } else {
        "satin"
    }
}

fn bounds(shapes: &[Shape]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for g in shapes {
        for r in g {
            for p in r {
                x0 = x0.min(p[0]);
                y0 = y0.min(p[1]);
                x1 = x1.max(p[0]);
                y1 = y1.max(p[1]);
            }
        }
    }
    if x1 <= x0 {
        (0.0, 0.0, 1.0, 1.0)
    } else {
        (x0, y0, x1, y1)
    }
}

fn dense(r: &Ring, step: f32) -> Ring {
    let n = r.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let a = r[i];
        let b = r[(i + 1) % n];
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let k = (((dx * dx + dy * dy).sqrt() / step).floor() as usize).clamp(1, 64);
        for j in 0..k {
            let t = j as f32 / k as f32;
            out.push([a[0] + dx * t, a[1] + dy * t]);
        }
    }
    out
}

/// Bend a point by an envelope over the box: arch, bulge, pinch, wave or slant; amount in per cent.
fn warp_point(x: f32, y: f32, kind: &str, amount: f32, b: (f32, f32, f32, f32)) -> [f32; 2] {
    let (x0, y0, x1, y1) = b;
    let (w, h) = ((x1 - x0).max(1e-6), (y1 - y0).max(1e-6));
    let u = (x - x0) / w;
    let cy = (y0 + y1) / 2.0;
    let a = amount / 100.0;
    let s = (std::f32::consts::PI * u).dsin();
    match kind {
        "arch_up" => [x, y - a * h * s],
        "arch_down" => [x, y + a * h * s],
        "bulge" => [x, cy + (y - cy) * (1.0 + a * s)],
        "pinch" => [x, cy + (y - cy) * (1.0 - a * s).max(0.2)],
        "wave" => [x, y + 0.5 * a * h * (2.0 * std::f32::consts::PI * u).dsin()],
        "slant" => [x + a * (cy - y), y],
        _ => [x, y],
    }
}

/// Bend shapes by an envelope ({"kind": ..., "amount": ...}).
pub fn warp(shapes: Vec<Shape>, env: &Value) -> Vec<Shape> {
    let kind = env["kind"].as_str().unwrap_or("none");
    if kind == "none" {
        return shapes;
    }
    let amount = env["amount"].as_f64().unwrap_or(30.0) as f32;
    let b = bounds(&shapes);
    shapes
        .into_iter()
        .map(|g| g.iter().map(|r| dense(r, 0.4).iter().map(|p| warp_point(p[0], p[1], kind, amount, b)).collect()).collect())
        .collect()
}

/// Bend one shape's rings (JSON in, JSON out), for shapes that aren't lettering.
pub fn warp_json(rings: &Value, env: &Value) -> Value {
    let shape: Shape = rings
        .as_array()
        .map(|rs| {
            rs.iter()
                .map(|r| {
                    r.as_array()
                        .map(|pts| pts.iter().map(|p| [p[0].as_f64().unwrap_or(0.0) as f32, p[1].as_f64().unwrap_or(0.0) as f32]).collect())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    let out = warp(vec![shape], env);
    out.first().map(|g| json!(g.iter().map(|r| r.iter().map(|p| json!([round4(p[0]), round4(p[1])])).collect::<Vec<_>>()).collect::<Vec<_>>())).unwrap_or(json!([]))
}

fn to_json(shapes: &[Shape]) -> Value {
    json!(shapes
        .iter()
        .map(|g| g.iter().map(|r| r.iter().map(|p| json!([round4(p[0]), round4(p[1])])).collect::<Vec<_>>()).collect::<Vec<_>>())
        .collect::<Vec<_>>())
}

fn round4(v: f32) -> f64 {
    (v as f64 * 10_000.0).round() / 10_000.0
}

/// Work out a lettering object's glyphs again from its text, font and envelope.
pub fn regen(text: &mut Text, o: &mut Value) {
    let mut shapes = lettering_shapes(text, o);
    if let Some(env) = o.get("envelope").cloned() {
        shapes = warp(shapes, &env);
    }
    o["glyphs"] = to_json(&shapes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text() -> Text {
        Text::bundled()
    }

    #[test]
    fn a_word_is_as_tall_as_its_cap_height() {
        let mut t = text();
        let o = json!({"text": "HI", "font": "Carlito Bold", "cap": 10.0, "x": 0.0, "y": 0.0});
        let shapes = lettering_shapes(&mut t, &o);
        assert!(!shapes.is_empty(), "no shapes");
        let (_, y0, _, y1) = bounds(&shapes);
        assert!((y1 - y0 - 10.0).abs() < 0.6, "cap height {} mm", y1 - y0);
    }

    #[test]
    fn a_letter_with_a_hole_keeps_it() {
        let mut t = text();
        let o = json!({"text": "O", "font": "Carlito Bold", "cap": 12.0});
        let shapes = lettering_shapes(&mut t, &o);
        assert_eq!(shapes.len(), 1, "one shape for one letter");
        assert_eq!(shapes[0].len(), 2, "an O is an outline and a hole");
    }

    #[test]
    fn two_letters_are_two_shapes_side_by_side() {
        let mut t = text();
        let o = json!({"text": "IT", "font": "Carlito", "cap": 10.0});
        let shapes = lettering_shapes(&mut t, &o);
        assert_eq!(shapes.len(), 2);
        let b0 = bounds(&shapes[0..1]);
        let b1 = bounds(&shapes[1..2]);
        assert!(b0.2 <= b1.0 + 0.5, "the I is left of the T: {b0:?} {b1:?}");
    }

    #[test]
    fn an_arc_curves_the_word_upwards() {
        let mut t = text();
        let flat = lettering_shapes(&mut t, &json!({"text": "ARCH", "cap": 8.0, "font": "Carlito Bold"}));
        let arc = lettering_shapes(&mut t, &json!({"text": "ARCH", "cap": 8.0, "font": "Carlito Bold", "arc": "top", "radius": 25.0}));
        assert_eq!(flat.len(), arc.len());
        let fb = bounds(&flat);
        let ab = bounds(&arc);
        assert!(ab.3 - ab.1 > (fb.3 - fb.1) * 1.4, "the arc isn't taller: {ab:?} vs {fb:?}");
    }

    #[test]
    fn an_envelope_bends_the_shapes_and_none_leaves_them_alone() {
        let mut t = text();
        let shapes = lettering_shapes(&mut t, &json!({"text": "WAVE", "cap": 10.0, "font": "Carlito Bold"}));
        let b = bounds(&shapes);
        let same = warp(shapes.clone(), &json!({"kind": "none"}));
        assert_eq!(bounds(&same), b);
        let arched = warp(shapes.clone(), &json!({"kind": "arch_up", "amount": 50.0}));
        let ab = bounds(&arched);
        assert!(ab.1 < b.1 - 1.0, "arch up didn't lift the middle: {ab:?} vs {b:?}");
        let slanted = warp(shapes, &json!({"kind": "slant", "amount": 30.0}));
        let sb = bounds(&slanted);
        assert!(sb.2 - sb.0 > b.2 - b.0, "slant didn't lean it over");
    }

    #[test]
    fn a_circle_is_found_through_points_on_one() {
        let (cx, cy, r) = (12.0f32, -5.0f32, 30.0f32);
        let pts: Vec<(f32, f32)> = (0..12)
            .map(|i| {
                let a = i as f32 / 12.0 * 2.0 * std::f32::consts::PI;
                (cx + r * a.dcos(), cy + r * a.dsin())
            })
            .collect();
        let (fx, fy, fr, rms) = fit_circle(&pts).expect("no circle");
        assert!((fx - cx).abs() < 0.01 && (fy - cy).abs() < 0.01, "centre {fx}, {fy}");
        assert!((fr - r).abs() < 0.01, "radius {fr}");
        assert!(rms < 0.01, "rms {rms}");
    }

    #[test]
    fn points_on_a_line_are_no_circle() {
        let pts: Vec<(f32, f32)> = (0..6).map(|i| (i as f32 * 5.0, 3.0)).collect();
        match fit_circle(&pts) {
            None => {}
            Some((_, _, r, rms)) => assert!(r > 1e4 || rms > 1.0, "a line came out as a circle of {r} mm"),
        }
        assert_eq!(fit_circle(&[(0.0, 0.0), (1.0, 0.0)]), None);
    }

    #[test]
    fn small_lettering_is_filled_not_satin() {
        assert_eq!(default_type(5.0), "fill");
        assert_eq!(default_type(12.0), "satin");
    }

    #[test]
    fn regen_writes_the_glyphs_into_the_object() {
        let mut t = text();
        let mut o = json!({"text": "A", "cap": 10.0, "font": "Carlito Bold", "envelope": {"kind": "bulge", "amount": 40.0}});
        regen(&mut t, &mut o);
        let g = o["glyphs"].as_array().expect("no glyphs");
        assert_eq!(g.len(), 1);
        assert!(g[0].as_array().map(|r| !r.is_empty()).unwrap_or(false));
    }
}
