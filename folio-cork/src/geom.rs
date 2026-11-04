//! Shape outlines, connection points and hit testing.

use folio_kit::math::Det;
use crate::model::Shape;
use crate::stencils;
use folio_kit::sk;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    M(f32, f32),
    L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

/// One sub-path. `fill` false means it is only stroked (leads, inner lines).
#[derive(Clone, Debug, PartialEq)]
pub struct Sub {
    pub segs: Vec<Seg>,
    pub fill: bool,
    /// Filled with the line colour (dots, arrowheads in symbols).
    pub solid: bool,
}

/// A shape's geometry in local pixels (0,0)-(w,h), before rotation and flipping.
#[derive(Clone, Debug, Default)]
pub struct Geo {
    pub subs: Vec<Sub>,
    /// Connection points in local pixels.
    pub points: Vec<(f32, f32)>,
    /// Text box in local pixels.
    pub text: (f32, f32, f32, f32),
}

const K: f32 = 0.552_284_8;

pub fn ellipse_segs(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<Seg> {
    let (kx, ky) = (rx * K, ry * K);
    vec![
        Seg::M(cx + rx, cy),
        Seg::C(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
        Seg::C(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
        Seg::C(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
        Seg::C(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        Seg::Z,
    ]
}

fn poly(pts: &[(f32, f32)]) -> Vec<Seg> {
    let mut v = vec![Seg::M(pts[0].0, pts[0].1)];
    for p in &pts[1..] {
        v.push(Seg::L(p.0, p.1));
    }
    v.push(Seg::Z);
    v
}

fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<Seg> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = r * K;
    vec![
        Seg::M(x + r, y),
        Seg::L(x + w - r, y),
        Seg::C(x + w - r + k, y, x + w, y + r - k, x + w, y + r),
        Seg::L(x + w, y + h - r),
        Seg::C(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h),
        Seg::L(x + r, y + h),
        Seg::C(x + r - k, y + h, x, y + h - r + k, x, y + h - r),
        Seg::L(x, y + r),
        Seg::C(x, y + r - k, x + r - k, y, x + r, y),
        Seg::Z,
    ]
}

fn sides(w: f32, h: f32) -> Vec<(f32, f32)> {
    vec![(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)]
}

fn filled(segs: Vec<Seg>) -> Sub {
    Sub { segs, fill: true, solid: false }
}

fn stroked(segs: Vec<Seg>) -> Sub {
    Sub { segs, fill: false, solid: false }
}

/// Geometry for any shape kind.
pub fn geometry(s: &Shape) -> Geo {
    let (w, h) = (s.w.max(1.0), s.h.max(1.0));
    let full = (0.0, 0.0, w, h);
    let inset = |f: f32| (w * f, h * f, w * (1.0 - 2.0 * f), h * (1.0 - 2.0 * f));
    match s.kind.as_str() {
        "rect" | "swatch" | "image" | "font" | "text" | "frame" | "table" => Geo {
            subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]))],
            points: sides(w, h),
            text: if s.kind == "swatch" { (0.0, h * 0.72, w, h * 0.28) } else { full },
        },
        "rounded" | "process-alt" | "mind" => Geo { subs: vec![filled(round_rect(0.0, 0.0, w, h, w.min(h) * 0.18))], points: sides(w, h), text: full },
        "sticky" => Geo { subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]))], points: sides(w, h), text: (w * 0.08, h * 0.08, w * 0.84, h * 0.84) },
        "ellipse" | "circle" => Geo {
            subs: vec![filled(ellipse_segs(w / 2.0, h / 2.0, w / 2.0, h / 2.0))],
            points: {
                let mut p = sides(w, h);
                let d = std::f32::consts::FRAC_1_SQRT_2;
                for (sx, sy) in [(1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                    p.push((w / 2.0 + sx * d * w / 2.0, h / 2.0 + sy * d * h / 2.0));
                }
                p
            },
            text: inset(0.15),
        },
        "diamond" => Geo { subs: vec![filled(poly(&[(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)]))], points: sides(w, h), text: inset(0.22) },
        "triangle" => Geo {
            subs: vec![filled(poly(&[(w / 2.0, 0.0), (w, h), (0.0, h)]))],
            points: vec![(w / 2.0, 0.0), (w * 0.75, h / 2.0), (w / 2.0, h), (w * 0.25, h / 2.0)],
            text: (w * 0.25, h * 0.45, w * 0.5, h * 0.5),
        },
        "rtriangle" => Geo {
            subs: vec![filled(poly(&[(0.0, 0.0), (w, h), (0.0, h)]))],
            points: vec![(0.0, h / 2.0), (w / 2.0, h), (w / 2.0, h / 2.0)],
            text: (0.0, h * 0.45, w * 0.6, h * 0.5),
        },
        "parallelogram" | "flow.data" => {
            let o = w.min(h) * 0.35;
            Geo { subs: vec![filled(poly(&[(o, 0.0), (w, 0.0), (w - o, h), (0.0, h)]))], points: vec![(w / 2.0, 0.0), (w - o / 2.0, h / 2.0), (w / 2.0, h), (o / 2.0, h / 2.0)], text: (o, 0.0, w - 2.0 * o, h) }
        }
        "trapezoid" => {
            let o = w * 0.2;
            Geo { subs: vec![filled(poly(&[(o, 0.0), (w - o, 0.0), (w, h), (0.0, h)]))], points: vec![(w / 2.0, 0.0), (w - o / 2.0, h / 2.0), (w / 2.0, h), (o / 2.0, h / 2.0)], text: (o, 0.0, w - 2.0 * o, h) }
        }
        "pentagon" => {
            let pts = regular(5, w, h, -90.0);
            Geo { subs: vec![filled(poly(&pts))], points: pts.clone(), text: inset(0.2) }
        }
        "hexagon" => {
            let o = w * 0.22;
            let pts = [(o, 0.0), (w - o, 0.0), (w, h / 2.0), (w - o, h), (o, h), (0.0, h / 2.0)];
            Geo { subs: vec![filled(poly(&pts))], points: sides(w, h), text: (o * 0.6, 0.0, w - 1.2 * o, h) }
        }
        "octagon" => {
            let (a, b) = (w * 0.29, h * 0.29);
            let pts = [(a, 0.0), (w - a, 0.0), (w, b), (w, h - b), (w - a, h), (a, h), (0.0, h - b), (0.0, b)];
            Geo { subs: vec![filled(poly(&pts))], points: sides(w, h), text: inset(0.1) }
        }
        "star" => {
            let mut pts = vec![];
            for i in 0..10 {
                let a = (-90.0 + i as f32 * 36.0).to_radians();
                let r = if i % 2 == 0 { 0.5 } else { 0.2 };
                pts.push((w / 2.0 + a.dcos() * r * w, h / 2.0 + a.dsin() * r * h * 1.05));
            }
            Geo { subs: vec![filled(poly(&pts))], points: sides(w, h), text: inset(0.32) }
        }
        "cross" => {
            let (a, b) = (w * 0.3, h * 0.3);
            let pts = [(a, 0.0), (w - a, 0.0), (w - a, b), (w, b), (w, h - b), (w - a, h - b), (w - a, h), (a, h), (a, h - b), (0.0, h - b), (0.0, b), (a, b)];
            Geo { subs: vec![filled(poly(&pts))], points: sides(w, h), text: (a, b, w - 2.0 * a, h - 2.0 * b) }
        }
        "arrow-right" | "arrow-left" | "arrow-up" | "arrow-down" => {
            // Built pointing right, then turned.
            let (lw, lh) = if matches!(s.kind.as_str(), "arrow-up" | "arrow-down") { (h, w) } else { (w, h) };
            let head = (lh * 0.9).min(lw * 0.45);
            let t = lh * 0.25;
            let pts = vec![(0.0, t), (lw - head, t), (lw - head, 0.0), (lw, lh / 2.0), (lw - head, lh), (lw - head, lh - t), (0.0, lh - t)];
            let map = |(x, y): (f32, f32)| match s.kind.as_str() {
                "arrow-left" => (lw - x, y),
                "arrow-up" => (y, lw - x),
                "arrow-down" => (y, x),
                _ => (x, y),
            };
            let pts: Vec<(f32, f32)> = pts.into_iter().map(map).collect();
            Geo { subs: vec![filled(poly(&pts))], points: sides(w, h), text: inset(0.2) }
        }
        "arrow-double" => {
            let head = (h * 0.9).min(w * 0.3);
            let t = h * 0.25;
            let pts = [(0.0, h / 2.0), (head, 0.0), (head, t), (w - head, t), (w - head, 0.0), (w, h / 2.0), (w - head, h), (w - head, h - t), (head, h - t), (head, h)];
            Geo { subs: vec![filled(poly(&pts))], points: sides(w, h), text: (head, t, w - 2.0 * head, h - 2.0 * t) }
        }
        "chevron" => {
            let o = w.min(h) * 0.4;
            let pts = [(0.0, 0.0), (w - o, 0.0), (w, h / 2.0), (w - o, h), (0.0, h), (o, h / 2.0)];
            Geo { subs: vec![filled(poly(&pts))], points: sides(w, h), text: (o, 0.0, w - 2.0 * o, h) }
        }
        "cylinder" | "flow.database" => {
            let e = (h * 0.12).min(w * 0.25);
            let mut body = vec![Seg::M(0.0, e)];
            body.extend([
                Seg::C(0.0, e - e * K, w / 2.0 - w / 2.0 * K, 0.0, w / 2.0, 0.0),
                Seg::C(w / 2.0 + w / 2.0 * K, 0.0, w, e - e * K, w, e),
                Seg::L(w, h - e),
                Seg::C(w, h - e + e * K, w / 2.0 + w / 2.0 * K, h, w / 2.0, h),
                Seg::C(w / 2.0 - w / 2.0 * K, h, 0.0, h - e + e * K, 0.0, h - e),
                Seg::Z,
            ]);
            let rim = vec![Seg::M(0.0, e), Seg::C(0.0, e + e * K, w / 2.0 - w / 2.0 * K, 2.0 * e, w / 2.0, 2.0 * e), Seg::C(w / 2.0 + w / 2.0 * K, 2.0 * e, w, e + e * K, w, e)];
            Geo { subs: vec![filled(body), stroked(rim)], points: sides(w, h), text: (0.0, 2.0 * e, w, h - 3.0 * e) }
        }
        "cloud" => {
            // One scalloped outline: bumps bulging out between points on an ellipse.
            let n = 10;
            let (cx, cy, rx, ry) = (w / 2.0, h / 2.0, w * 0.42, h * 0.38);
            let pt = |i: usize| {
                let a = (i as f32 / n as f32) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                (cx + rx * a.dcos(), cy + ry * a.dsin())
            };
            let mut segs = vec![];
            let p0 = pt(0);
            segs.push(Seg::M(p0.0, p0.1));
            for i in 0..n {
                let (a, b) = (pt(i), pt(i + 1));
                let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let out = ((m.0 - cx) / rx, (m.1 - cy) / ry);
                let l = (out.0 * out.0 + out.1 * out.1).sqrt().max(0.001);
                let bulge = (w.min(h) * 0.16, w.min(h) * 0.16);
                let o = (out.0 / l * bulge.0, out.1 / l * bulge.1);
                segs.push(Seg::C(a.0 + o.0 * 1.3, a.1 + o.1 * 1.3, b.0 + o.0 * 1.3, b.1 + o.1 * 1.3, b.0, b.1));
            }
            segs.push(Seg::Z);
            Geo { subs: vec![filled(segs)], points: sides(w, h), text: inset(0.22) }
        }
        "callout" => {
            let bh = h * 0.78;
            let r = w.min(bh) * 0.12;
            let mut segs = round_rect(0.0, 0.0, w, bh, r);
            segs.pop();
            let tail = vec![Seg::M(w * 0.2, bh - 1.0), Seg::L(w * 0.12, h), Seg::L(w * 0.38, bh - 1.0), Seg::Z];
            segs.push(Seg::Z);
            let mut all = segs;
            all.extend(tail);
            Geo { subs: vec![filled(all)], points: sides(w, bh), text: (0.0, 0.0, w, bh) }
        }
        "flow.terminator" => Geo { subs: vec![filled(round_rect(0.0, 0.0, w, h, h / 2.0))], points: sides(w, h), text: (h * 0.3, 0.0, w - h * 0.6, h) },
        "flow.process" => Geo { subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]))], points: sides(w, h), text: full },
        "flow.decision" => Geo { subs: vec![filled(poly(&[(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)]))], points: sides(w, h), text: inset(0.2) },
        "flow.predefined" => {
            let o = w * 0.1;
            Geo {
                subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)])), stroked(vec![Seg::M(o, 0.0), Seg::L(o, h)]), stroked(vec![Seg::M(w - o, 0.0), Seg::L(w - o, h)])],
                points: sides(w, h),
                text: (o, 0.0, w - 2.0 * o, h),
            }
        }
        "flow.document" => {
            let d = h * 0.12;
            let segs = vec![Seg::M(0.0, 0.0), Seg::L(w, 0.0), Seg::L(w, h - d), Seg::C(w * 0.75, h - 3.0 * d, w * 0.5, h - 3.0 * d, w * 0.5, h - d), Seg::C(w * 0.5, h + d, w * 0.25, h + d, 0.0, h - d), Seg::Z];
            Geo { subs: vec![filled(segs)], points: vec![(w / 2.0, 0.0), (w, (h - d) / 2.0), (w / 2.0, h - d), (0.0, (h - d) / 2.0)], text: (0.0, 0.0, w, h - d) }
        }
        "flow.manual-input" => Geo { subs: vec![filled(poly(&[(0.0, h * 0.3), (w, 0.0), (w, h), (0.0, h)]))], points: vec![(w / 2.0, h * 0.15), (w, h / 2.0), (w / 2.0, h), (0.0, h * 0.65)], text: (0.0, h * 0.25, w, h * 0.75) },
        "flow.manual-op" => {
            let o = w * 0.15;
            Geo { subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w - o, h), (o, h)]))], points: sides(w, h), text: (o, 0.0, w - 2.0 * o, h) }
        }
        "flow.preparation" => {
            let o = w * 0.15;
            Geo { subs: vec![filled(poly(&[(o, 0.0), (w - o, 0.0), (w, h / 2.0), (w - o, h), (o, h), (0.0, h / 2.0)]))], points: sides(w, h), text: (o, 0.0, w - 2.0 * o, h) }
        }
        "flow.delay" => {
            let r = (h / 2.0).min(w / 2.0);
            let segs = vec![Seg::M(0.0, 0.0), Seg::L(w - r, 0.0), Seg::C(w - r + r * K, 0.0, w, h / 2.0 - r * K, w, h / 2.0), Seg::C(w, h / 2.0 + r * K, w - r + r * K, h, w - r, h), Seg::L(0.0, h), Seg::Z];
            Geo { subs: vec![filled(segs)], points: sides(w, h), text: (0.0, 0.0, w - r * 0.4, h) }
        }
        "flow.offpage" => Geo {
            subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h * 0.65), (w / 2.0, h), (0.0, h * 0.65)]))],
            points: sides(w, h),
            text: (0.0, 0.0, w, h * 0.65),
        },
        "flow.connector" => Geo { subs: vec![filled(ellipse_segs(w / 2.0, h / 2.0, w / 2.0, h / 2.0))], points: sides(w, h), text: inset(0.15) },
        "flow.merge" => Geo { subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w / 2.0, h)]))], points: vec![(w / 2.0, 0.0), (w * 0.75, h / 2.0), (w / 2.0, h), (w * 0.25, h / 2.0)], text: (w * 0.2, 0.0, w * 0.6, h * 0.5) },
        "flow.extract" => Geo { subs: vec![filled(poly(&[(w / 2.0, 0.0), (w, h), (0.0, h)]))], points: vec![(w / 2.0, 0.0), (w * 0.75, h / 2.0), (w / 2.0, h), (w * 0.25, h / 2.0)], text: (w * 0.2, h * 0.45, w * 0.6, h * 0.5) },
        "flow.stored-data" => {
            let o = w * 0.15;
            let segs = vec![Seg::M(o, 0.0), Seg::L(w, 0.0), Seg::C(w - o, 0.0, w - o, h, w, h), Seg::L(o, h), Seg::C(-o / 3.0, h, -o / 3.0, 0.0, o, 0.0), Seg::Z];
            Geo { subs: vec![filled(segs)], points: sides(w, h), text: (o, 0.0, w - 2.0 * o, h) }
        }
        "flow.internal-storage" => {
            let (a, b) = (w * 0.12, h * 0.2);
            Geo {
                subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)])), stroked(vec![Seg::M(a, 0.0), Seg::L(a, h)]), stroked(vec![Seg::M(0.0, b), Seg::L(w, b)])],
                points: sides(w, h),
                text: (a, b, w - a, h - b),
            }
        }
        "flow.card" => {
            let o = w.min(h) * 0.25;
            Geo { subs: vec![filled(poly(&[(o, 0.0), (w, 0.0), (w, h), (0.0, h), (0.0, o)]))], points: sides(w, h), text: full }
        }
        "flow.display" => {
            let o = w * 0.15;
            let segs = vec![Seg::M(0.0, h / 2.0), Seg::L(o, 0.0), Seg::L(w - o, 0.0), Seg::C(w, 0.0, w, h, w - o, h), Seg::L(o, h), Seg::Z];
            Geo { subs: vec![filled(segs)], points: sides(w, h), text: (o, 0.0, w - 2.0 * o, h) }
        }
        "flow.loop-limit" => {
            let o = w.min(h) * 0.25;
            Geo { subs: vec![filled(poly(&[(o, 0.0), (w - o, 0.0), (w, o), (w, h), (0.0, h), (0.0, o)]))], points: sides(w, h), text: full }
        }
        "flow.or" | "flow.summing" => {
            let mut subs = vec![filled(ellipse_segs(w / 2.0, h / 2.0, w / 2.0, h / 2.0))];
            if s.kind == "flow.or" {
                subs.push(stroked(vec![Seg::M(w / 2.0, 0.0), Seg::L(w / 2.0, h)]));
                subs.push(stroked(vec![Seg::M(0.0, h / 2.0), Seg::L(w, h / 2.0)]));
            } else {
                let d = std::f32::consts::FRAC_1_SQRT_2 * 0.5;
                subs.push(stroked(vec![Seg::M(w * (0.5 - d), h * (0.5 - d)), Seg::L(w * (0.5 + d), h * (0.5 + d))]));
                subs.push(stroked(vec![Seg::M(w * (0.5 + d), h * (0.5 - d)), Seg::L(w * (0.5 - d), h * (0.5 + d))]));
            }
            Geo { subs, points: sides(w, h), text: inset(0.2) }
        }
        "ink" | "group" => Geo { subs: vec![], points: vec![], text: full },
        "custom" => {
            let mut subs = stencils::parse(&s.path, w, h);
            if s.get("lineart") == "1" {
                for x in subs.iter_mut().filter(|x| x.fill) {
                    x.solid = true;
                }
            }
            let points = if s.cpts.is_empty() { sides(w, h) } else { s.cpts.iter().map(|&(x, y)| (x * w, y * h)).collect() };
            let text = match s.data.get("tbox").map(|t| t.split(',').filter_map(|v| v.parse::<f32>().ok()).collect::<Vec<f32>>()) {
                Some(v) if v.len() == 4 => (v[0] * w, v[1] * h, v[2] * w, v[3] * h),
                _ => full,
            };
            Geo { subs, points, text }
        }
        k => match stencils::find(k) {
            Some(st) => stencils::geometry(st, w, h),
            None => Geo { subs: vec![filled(poly(&[(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]))], points: sides(w, h), text: full },
        },
    }
}

fn regular(n: usize, w: f32, h: f32, start: f32) -> Vec<(f32, f32)> {
    let mut pts: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let a = (start + i as f32 * 360.0 / n as f32).to_radians();
            (a.dcos(), a.dsin())
        })
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in &pts {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    for p in pts.iter_mut() {
        *p = ((p.0 - x0) / (x1 - x0) * w, (p.1 - y0) / (y1 - y0) * h);
    }
    pts
}

/// Local-to-page transform of a shape: flip, then rotate about the centre, then place.
pub fn shape_xf(s: &Shape) -> sk::Transform {
    let (cx, cy) = (s.w / 2.0, s.h / 2.0);
    let fx = if s.flip_h { -1.0 } else { 1.0 };
    let fy = if s.flip_v { -1.0 } else { 1.0 };
    sk::Transform::from_translate(-cx, -cy)
        .post_scale(fx, fy)
        .post_concat(rotation(s.rot))
        .post_translate(s.x + cx, s.y + cy)
}

/// A rotation in degrees from Folio's own sine and cosine (X3-12), not the system's.
pub fn rotation(deg: f32) -> sk::Transform {
    let r = deg.to_radians();
    let (c, s) = (r.dcos(), r.dsin());
    sk::Transform::from_row(c, s, -s, c, 0.0, 0.0)
}

pub fn to_path(segs: &[Seg]) -> Option<sk::Path> {
    let mut pb = sk::PathBuilder::new();
    for s in segs {
        match *s {
            Seg::M(x, y) => pb.move_to(x, y),
            Seg::L(x, y) => pb.line_to(x, y),
            Seg::C(a, b, c, d, e, f) => pb.cubic_to(a, b, c, d, e, f),
            Seg::Z => pb.close(),
        }
    }
    pb.finish()
}

pub fn map_pt(xf: &sk::Transform, x: f32, y: f32) -> (f32, f32) {
    let mut p = [sk::Point::from_xy(x, y)];
    xf.map_points(&mut p);
    (p[0].x, p[0].y)
}

/// Flatten segments into polylines (one per sub-path start).
pub fn flatten(segs: &[Seg], tol_steps: usize) -> Vec<Vec<(f32, f32)>> {
    let mut out: Vec<Vec<(f32, f32)>> = vec![];
    let mut cur: Vec<(f32, f32)> = vec![];
    let mut last = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    for s in segs {
        match *s {
            Seg::M(x, y) => {
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                cur.push((x, y));
                last = (x, y);
                start = (x, y);
            }
            Seg::L(x, y) => {
                cur.push((x, y));
                last = (x, y);
            }
            Seg::C(a, b, c, d, e, f) => {
                for i in 1..=tol_steps {
                    let t = i as f32 / tol_steps as f32;
                    let mt = 1.0 - t;
                    let x = mt * mt * mt * last.0 + 3.0 * mt * mt * t * a + 3.0 * mt * t * t * c + t * t * t * e;
                    let y = mt * mt * mt * last.1 + 3.0 * mt * mt * t * b + 3.0 * mt * t * t * d + t * t * t * f;
                    cur.push((x, y));
                }
                last = (e, f);
            }
            Seg::Z => {
                cur.push(start);
                last = start;
            }
        }
    }
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

pub fn point_in_poly(p: (f32, f32), poly: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > p.1) != (yj > p.1) && p.0 < (xj - xi) * (p.1 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn dist_seg(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - qx).sq() + (p.1 - qy).sq()).sqrt()
}

pub fn dist_polyline(p: (f32, f32), pts: &[(f32, f32)]) -> f32 {
    pts.windows(2).map(|w| dist_seg(p, w[0], w[1])).fold(f32::MAX, f32::min)
}

/// Page-space bounding box of a rotated shape.
pub fn bounds(s: &Shape) -> (f32, f32, f32, f32) {
    if s.rot.abs() < 0.01 {
        return (s.x, s.y, s.w, s.h);
    }
    let xf = shape_xf(s);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (x, y) in [(0.0, 0.0), (s.w, 0.0), (s.w, s.h), (0.0, s.h)] {
        let (px, py) = map_pt(&xf, x, y);
        x0 = x0.min(px);
        y0 = y0.min(py);
        x1 = x1.max(px);
        y1 = y1.max(py);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

/// Page position of a shape's connection point.
pub fn conn_point(s: &Shape, i: usize) -> Option<(f32, f32)> {
    let g = geometry(s);
    g.points.get(i).map(|&(x, y)| map_pt(&shape_xf(s), x, y))
}

pub fn conn_points(s: &Shape) -> Vec<(f32, f32)> {
    let g = geometry(s);
    let xf = shape_xf(s);
    g.points.iter().map(|&(x, y)| map_pt(&xf, x, y)).collect()
}

/// The page-space outline polygons of a shape's filled parts.
pub fn outline(s: &Shape) -> Vec<Vec<(f32, f32)>> {
    let g = geometry(s);
    let xf = shape_xf(s);
    let mut v = vec![];
    for sub in g.subs.iter().filter(|s| s.fill) {
        for poly in flatten(&sub.segs, 8) {
            v.push(poly.into_iter().map(|(x, y)| map_pt(&xf, x, y)).collect());
        }
    }
    if v.is_empty() {
        let (x, y, w, h) = (0.0, 0.0, s.w, s.h);
        v.push([(x, y), (w, y), (w, h), (x, h), (x, y)].iter().map(|&(a, b)| map_pt(&xf, a, b)).collect());
    }
    v
}

/// Where a line from the shape's centre toward `toward` leaves its outline.
pub fn boundary_point(s: &Shape, toward: (f32, f32)) -> (f32, f32) {
    let c = (s.cx(), s.cy());
    let polys = outline(s);
    let mut best: Option<((f32, f32), f32)> = None;
    let d = (toward.0 - c.0, toward.1 - c.1);
    let far = (c.0 + d.0 * 1000.0, c.1 + d.1 * 1000.0);
    for poly in &polys {
        for w in poly.windows(2) {
            if let Some(p) = seg_intersect(c, far, w[0], w[1]) {
                let dd = (p.0 - c.0).sq() + (p.1 - c.1).sq();
                if best.map(|b| dd > b.1).unwrap_or(true) {
                    best = Some((p, dd));
                }
            }
        }
    }
    best.map(|b| b.0).unwrap_or(c)
}

pub fn seg_intersect(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> Option<(f32, f32)> {
    let r = (b.0 - a.0, b.1 - a.1);
    let s = (d.0 - c.0, d.1 - c.1);
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-9 {
        return None;
    }
    let t = ((c.0 - a.0) * s.1 - (c.1 - a.1) * s.0) / den;
    let u = ((c.0 - a.0) * r.1 - (c.1 - a.1) * r.0) / den;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some((a.0 + t * r.0, a.1 + t * r.1))
    } else {
        None
    }
}

/// Does a page point hit the shape? `tol` is in page pixels.
pub fn hit_shape(s: &Shape, p: (f32, f32), tol: f32) -> bool {
    if s.kind == "ink" {
        let pts: Vec<(f32, f32)> = s.points.iter().map(|&(x, y)| (s.x + x, s.y + y)).collect();
        return dist_polyline(p, &pts) <= tol + s.style.width / 2.0;
    }
    let inv = shape_xf(s).invert().unwrap_or_default();
    let (lx, ly) = map_pt(&inv, p.0, p.1);
    if s.kind == "frame" {
        // Frames are picked by their border and title, so shapes inside stay clickable.
        let t = tol + 4.0;
        let on_edge = (lx.abs() < t || (lx - s.w).abs() < t) && ly > -t && ly < s.h + t || (ly.abs() < t || (ly - s.h).abs() < t) && lx > -t && lx < s.w + t;
        let title = ly < 0.0 && ly > -26.0 && lx > 0.0 && lx < s.w.min(260.0);
        return on_edge || title;
    }
    if lx < -tol || ly < -tol || lx > s.w + tol || ly > s.h + tol {
        return false;
    }
    let g = geometry(s);
    let has_fill = g.subs.iter().any(|x| x.fill);
    if !has_fill || matches!(s.kind.as_str(), "text" | "font" | "image" | "swatch") || s.kind.contains('.') && !stencils::closed(&s.kind) {
        return true;
    }
    for sub in &g.subs {
        for poly in flatten(&sub.segs, 8) {
            if sub.fill && point_in_poly((lx, ly), &poly) {
                return true;
            }
            if dist_polyline((lx, ly), &poly) <= tol + s.style.width {
                return true;
            }
        }
    }
    false
}

/// Distance from a point to a polyline route (connectors).
pub fn hit_route(route: &[(f32, f32)], p: (f32, f32), tol: f32) -> bool {
    dist_polyline(p, route) <= tol
}
