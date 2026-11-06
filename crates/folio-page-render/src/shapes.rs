//! Shapes: their outlines as paths, and the fills and lines they carry.
//! The reading of a shape is the engine's; this is the drawing of it.

use crate::eng::shapes::{Cmd, CustPath, Shape};
use folio_canvas::Cx;
use folio_math::Det;

/// A closed polygon through the points given.
fn poly(cr: &mut Cx, pts: &[(f64, f64)]) {
    for (i, (x, y)) in pts.iter().enumerate() {
        if i == 0 {
            cr.move_to(*x, *y);
        } else {
            cr.line_to(*x, *y);
        }
    }
    cr.close_path();
}

fn custom_path(cr: &mut Cx, paths: &[CustPath], w: f64, h: f64) -> bool {
    let mut closed = false;
    for p in paths {
        let sx = if p.w > 0.0 { w / p.w } else { 1.0 / 12700.0 };
        let sy = if p.h > 0.0 { h / p.h } else { 1.0 / 12700.0 };
        let (mut cx, mut cy) = (0.0, 0.0);
        let (mut startx, mut starty) = (0.0, 0.0);
        for c in &p.cmds {
            match *c {
                Cmd::Move(x, y) => {
                    cr.move_to(x * sx, y * sy);
                    (cx, cy, startx, starty) = (x, y, x, y);
                }
                Cmd::Line(x, y) => {
                    cr.line_to(x * sx, y * sy);
                    (cx, cy) = (x, y);
                }
                Cmd::Cubic(a, b, c2, d, e, f) => {
                    cr.curve_to(a * sx, b * sy, c2 * sx, d * sy, e * sx, f * sy);
                    (cx, cy) = (e, f);
                }
                Cmd::Quad(a, b, e, f) => {
                    let (x0, y0) = (cx, cy);
                    cr.curve_to((x0 + 2.0 * a) / 3.0 * sx, (y0 + 2.0 * b) / 3.0 * sy, (e + 2.0 * a) / 3.0 * sx,
                                (f + 2.0 * b) / 3.0 * sy, e * sx, f * sy);
                    (cx, cy) = (e, f);
                }
                Cmd::Arc(wr, hr, st, sw) => {
                    // the arc starts at the current point
                    let a0 = st.to_radians();
                    let ocx = cx - wr * a0.dcos();
                    let ocy = cy - hr * a0.dsin();
                    let a1 = (st + sw).to_radians();
                    cr.save();
                    cr.translate(ocx * sx, ocy * sy);
                    cr.scale((wr * sx).max(1e-6), (hr * sy).max(1e-6));
                    if sw >= 0.0 { cr.arc(0.0, 0.0, 1.0, a0, a1) } else { cr.arc_negative(0.0, 0.0, 1.0, a0, a1) }
                    cr.restore();
                    cx = ocx + wr * a1.dcos();
                    cy = ocy + hr * a1.dsin();
                }
                Cmd::Close => {
                    cr.close_path();
                    (cx, cy) = (startx, starty);
                    closed = true;
                }
            }
        }
        if p.fill {
            closed = true;
        }
    }
    closed
}

pub fn path(cr: &mut Cx, shape: &Shape, w: f64, h: f64) -> bool {
        if shape.geom == "custom" {
            return custom_path(cr, &shape.custom, w, h);
        }
        let adj = |i: usize, d: f64| shape.adj.get(i).copied().unwrap_or(d) / 100000.0;
        let ss = w.min(h);

        match shape.geom.as_str() {
            "ellipse" | "flowChartConnector" => {
                cr.save();
                cr.translate(w / 2.0, h / 2.0);
                cr.scale((w / 2.0).max(1e-6), (h / 2.0).max(1e-6));
                cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
                cr.restore();
            }
            "roundRect" | "flowChartAlternateProcess" => {
                let r = ss * adj(0, 16667.0);
                cr.new_sub_path();
                cr.arc(w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
                cr.arc(w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
                cr.arc(r, h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
                cr.arc(r, r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
                cr.close_path();
            }
            "line" | "straightConnector1" => {
                cr.move_to(0.0, 0.0);
                cr.line_to(w, h);
                return false;
            }
            "bentConnector2" => {
                cr.move_to(0.0, 0.0);
                cr.line_to(w, 0.0);
                cr.line_to(w, h);
                return false;
            }
            "bentConnector3" => {
                let x1 = w * adj(0, 50000.0);
                cr.move_to(0.0, 0.0);
                cr.line_to(x1, 0.0);
                cr.line_to(x1, h);
                cr.line_to(w, h);
                return false;
            }
            "curvedConnector3" => {
                let x1 = w * adj(0, 50000.0);
                cr.move_to(0.0, 0.0);
                cr.curve_to(x1, 0.0, x1, h, w, h);
                return false;
            }
            "triangle" | "flowChartExtract" => poly(cr, &[(w * adj(0, 50000.0), 0.0), (w, h), (0.0, h)]),
            "rtTriangle" => poly(cr, &[(0.0, 0.0), (w, h), (0.0, h)]),
            "diamond" | "flowChartDecision" => poly(cr, &[(w / 2.0, 0.0), (w, h / 2.0), (w / 2.0, h), (0.0, h / 2.0)]),
            "parallelogram" | "flowChartInputOutput" => {
                let a = ss * adj(0, 25000.0);
                poly(cr, &[(a, 0.0), (w, 0.0), (w - a, h), (0.0, h)])
            }
            "trapezoid" => {
                let a = ss * adj(0, 25000.0);
                poly(cr, &[(0.0, h), (a, 0.0), (w - a, 0.0), (w, h)])
            }
            "hexagon" => {
                let a = ss * adj(0, 25000.0);
                poly(cr, &[(0.0, h / 2.0), (a, 0.0), (w - a, 0.0), (w, h / 2.0), (w - a, h), (a, h)])
            }
            "octagon" => {
                let a = ss * adj(0, 29289.0);
                poly(cr, &[(a, 0.0), (w - a, 0.0), (w, a), (w, h - a), (w - a, h), (a, h), (0.0, h - a), (0.0, a)])
            }
            "pentagon" | "homePlate" => {
                if shape.geom == "homePlate" {
                    let a = ss * adj(0, 50000.0);
                    poly(cr, &[(0.0, 0.0), (w - a, 0.0), (w, h / 2.0), (w - a, h), (0.0, h)])
                } else {
                    poly(cr, &[(w / 2.0, 0.0), (w, h * 0.382), (w * 0.809, h), (w * 0.191, h), (0.0, h * 0.382)])
                }
            }
            "chevron" => {
                let a = ss * adj(0, 50000.0);
                poly(cr, &[(0.0, 0.0), (w - a, 0.0), (w, h / 2.0), (w - a, h), (0.0, h), (a, h / 2.0)])
            }
            "rightArrow" | "leftArrow" => {
                let a1 = adj(0, 50000.0);
                let hd = ss * adj(1, 50000.0);
                let (t, b) = (h / 2.0 - h * a1 / 2.0, h / 2.0 + h * a1 / 2.0);
                let pts = [(0.0, t), (w - hd, t), (w - hd, 0.0), (w, h / 2.0), (w - hd, h), (w - hd, b), (0.0, b)];
                if shape.geom == "leftArrow" {
                    let m: Vec<(f64, f64)> = pts.iter().map(|(x, y)| (w - x, *y)).collect();
                    poly(cr, &m);
                } else {
                    poly(cr, &pts);
                }
            }
            "upArrow" | "downArrow" => {
                let a1 = adj(0, 50000.0);
                let hd = ss * adj(1, 50000.0);
                let (l, r) = (w / 2.0 - w * a1 / 2.0, w / 2.0 + w * a1 / 2.0);
                let pts = [(l, h), (l, hd), (0.0, hd), (w / 2.0, 0.0), (w, hd), (r, hd), (r, h)];
                if shape.geom == "downArrow" {
                    let m: Vec<(f64, f64)> = pts.iter().map(|(x, y)| (*x, h - y)).collect();
                    poly(cr, &m);
                } else {
                    poly(cr, &pts);
                }
            }
            "star5" | "star4" | "star6" | "star8" => {
                let n: usize = shape.geom[4..].parse().unwrap_or(5);
                let inner = match n {
                    4 => 0.38,
                    5 => 0.38,
                    6 => 0.58,
                    _ => 0.7,
                };
                let mut pts = Vec::new();
                for i in 0..n * 2 {
                    let a = -std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::PI / n as f64;
                    let r = if i % 2 == 0 { 1.0 } else { inner };
                    pts.push((w / 2.0 + a.dcos() * r * w / 2.0, h / 2.0 + a.dsin() * r * h / 2.0));
                }
                poly(cr, &pts);
            }
            "plus" | "flowChartPredefinedProcess" if shape.geom == "plus" => {
                let a = ss * adj(0, 25000.0);
                poly(cr, &[(a, 0.0), (w - a, 0.0), (w - a, a), (w, a), (w, h - a), (w - a, h - a), (w - a, h), (a, h),
                       (a, h - a), (0.0, h - a), (0.0, a), (a, a)])
            }
            "frame" => {
                let a = ss * adj(0, 12500.0);
                cr.rectangle(0.0, 0.0, w, h);
                cr.new_sub_path();
                cr.move_to(a, a);
                cr.line_to(a, h - a);
                cr.line_to(w - a, h - a);
                cr.line_to(w - a, a);
                cr.close_path();
            }
            "donut" => {
                let a = ss * adj(0, 25000.0);
                ellipse(cr, w / 2.0, h / 2.0, w / 2.0, h / 2.0, false);
                ellipse(cr, w / 2.0, h / 2.0, (w / 2.0 - a).max(0.5), (h / 2.0 - a).max(0.5), true);
            }
            "cube" => {
                let a = ss * adj(0, 25000.0);
                poly(cr, &[(0.0, a), (a, 0.0), (w, 0.0), (w, h - a), (w - a, h), (0.0, h)]);
                cr.move_to(0.0, a);
                cr.line_to(w - a, a);
                cr.line_to(w, 0.0);
                cr.move_to(w - a, a);
                cr.line_to(w - a, h);
            }
            "can" | "flowChartMagneticDisk" => {
                let a = (ss * adj(0, 25000.0)).min(h / 2.0);
                let ry = a / 2.0;
                cr.move_to(0.0, ry);
                elliptic_arc(cr, w / 2.0, ry, w / 2.0, ry, std::f64::consts::PI, std::f64::consts::TAU);
                cr.line_to(w, h - ry);
                elliptic_arc(cr, w / 2.0, h - ry, w / 2.0, ry, 0.0, std::f64::consts::PI);
                cr.close_path();
                cr.new_sub_path();
                cr.move_to(w, ry);
                elliptic_arc(cr, w / 2.0, ry, w / 2.0, ry, 0.0, std::f64::consts::PI);
            }
            "heart" => {
                cr.move_to(w / 2.0, h * 0.25);
                cr.curve_to(w * 0.5, h * -0.1, w * -0.15, h * 0.05, w * 0.05, h * 0.42);
                cr.curve_to(w * 0.15, h * 0.62, w * 0.42, h * 0.8, w / 2.0, h);
                cr.curve_to(w * 0.58, h * 0.8, w * 0.85, h * 0.62, w * 0.95, h * 0.42);
                cr.curve_to(w * 1.15, h * 0.05, w * 0.5, h * -0.1, w / 2.0, h * 0.25);
                cr.close_path();
            }
            "mathPlus" => {
                let t = ss * adj(0, 23520.0) / 2.0;
                let (ex, ey) = (w * 0.3675, h * 0.3675);
                let (cx, cy) = (w / 2.0, h / 2.0);
                poly(cr, &[(cx - t, cy - ey), (cx + t, cy - ey), (cx + t, cy - t), (cx + ex, cy - t), (cx + ex, cy + t),
                       (cx + t, cy + t), (cx + t, cy + ey), (cx - t, cy + ey), (cx - t, cy + t), (cx - ex, cy + t),
                       (cx - ex, cy - t), (cx - t, cy - t)]);
            }
            "mathMinus" => {
                let t = h * adj(0, 23520.0) / 2.0;
                cr.rectangle(w * 0.1325, h / 2.0 - t, w * 0.735, 2.0 * t);
            }
            "mathMultiply" => {
                let t = ss * adj(0, 23520.0) / 2.0;
                let (cx, cy) = (w / 2.0, h / 2.0);
                let (ex, ey) = (w * 0.34, h * 0.34);
                // two crossing bars
                for sgn in [1.0, -1.0] {
                    let (dx, dy) = (ex, sgn * ey);
                    let l = (dx * dx + dy * dy).sqrt().max(0.01);
                    let (nx, ny) = (-dy / l * t, dx / l * t);
                    poly(cr, &[(cx - dx + nx, cy - dy + ny), (cx + dx + nx, cy + dy + ny),
                           (cx + dx - nx, cy + dy - ny), (cx - dx - nx, cy - dy - ny)]);
                    cr.new_sub_path();
                }
            }
            "mathDivide" => {
                let t = h * adj(0, 23520.0) / 2.0;
                let gap = h * adj(1, 5880.0);
                let r = h * adj(2, 11760.0) / 2.0;
                cr.rectangle(w * 0.1325, h / 2.0 - t, w * 0.735, 2.0 * t);
                cr.new_sub_path();
                ellipse(cr, w / 2.0, h / 2.0 - t - gap - r, r, r, false);
                cr.new_sub_path();
                ellipse(cr, w / 2.0, h / 2.0 + t + gap + r, r, r, false);
            }
            "mathEqual" | "mathNotEqual" => {
                let t = h * adj(0, 23520.0);
                let gap = h * adj(if shape.geom == "mathEqual" { 1 } else { 2 }, 11760.0);
                cr.rectangle(w * 0.1325, h / 2.0 - gap / 2.0 - t, w * 0.735, t);
                cr.rectangle(w * 0.1325, h / 2.0 + gap / 2.0, w * 0.735, t);
                if shape.geom == "mathNotEqual" {
                    let (cx, cy, ht) = (w / 2.0, h / 2.0, h * 0.4);
                    let dx = ht * 0.36;
                    let k = t / 2.0;
                    cr.new_sub_path();
                    poly(cr, &[(cx + dx - k, cy - ht), (cx + dx + k, cy - ht), (cx - dx + k, cy + ht), (cx - dx - k, cy + ht)]);
                }
            }
            "lightningBolt" => poly(cr, &[(w * 0.39, 0.0), (w * 0.62, h * 0.29), (w * 0.53, h * 0.34), (w * 0.82, h * 0.6),
                                      (w * 0.72, h * 0.64), (w, h), (w * 0.47, h * 0.72), (w * 0.57, h * 0.67),
                                      (w * 0.2, h * 0.42), (w * 0.33, h * 0.36), (0.0, h * 0.17)]),
            "sun" => {
                let r = ss * 0.25;
                ellipse(cr, w / 2.0, h / 2.0, w * 0.25, h * 0.25, false);
                for i in 0..8 {
                    let a = i as f64 * std::f64::consts::FRAC_PI_4;
                    let (c, s_) = (a.dcos(), a.dsin());
                    let (px, py) = (-s_, c);
                    let (r0, r1) = (0.34, 0.5);
                    cr.new_sub_path();
                    cr.move_to(w / 2.0 + c * w * r1, h / 2.0 + s_ * h * r1);
                    cr.line_to(w / 2.0 + c * w * r0 + px * r * 0.25, h / 2.0 + s_ * h * r0 + py * r * 0.25);
                    cr.line_to(w / 2.0 + c * w * r0 - px * r * 0.25, h / 2.0 + s_ * h * r0 - py * r * 0.25);
                    cr.close_path();
                }
            }
            "moon" => {
                let a = w * adj(0, 50000.0);
                cr.move_to(w, 0.0);
                cr.curve_to(w * -0.33, 0.0, w * -0.33, h, w, h);
                cr.curve_to(a - w * 0.1, h * 0.85, a - w * 0.1, h * 0.15, w, 0.0);
                cr.close_path();
            }
            "smileyFace" => {
                ellipse(cr, w / 2.0, h / 2.0, w / 2.0, h / 2.0, false);
                ellipse(cr, w * 0.35, h * 0.38, w * 0.05, h * 0.05, true);
                ellipse(cr, w * 0.65, h * 0.38, w * 0.05, h * 0.05, true);
                cr.move_to(w * 0.25, h * 0.66);
                cr.curve_to(w * 0.4, h * 0.82, w * 0.6, h * 0.82, w * 0.75, h * 0.66);
            }
            "cloud" | "cloudCallout" => {
                let bumps = [(0.22, 0.32, 0.2, 0.2), (0.45, 0.2, 0.24, 0.2), (0.72, 0.26, 0.22, 0.2), (0.82, 0.55, 0.18, 0.2),
                             (0.62, 0.74, 0.22, 0.2), (0.35, 0.74, 0.22, 0.2), (0.15, 0.56, 0.16, 0.18)];
                for (cx, cy, rx, ry) in bumps {
                    ellipse(cr, w * cx, h * cy, w * rx, h * ry, false);
                }
                if shape.geom == "cloudCallout" {
                    for (cx, cy, r) in [(0.18, 0.92, 0.05), (0.08, 1.02, 0.03)] {
                        ellipse(cr, w * cx, h * cy, w * r, h * r, false);
                    }
                }
            }
            "leftRightArrow" => {
                let a1 = adj(0, 50000.0);
                let hd = (ss * adj(1, 50000.0)).min(w / 2.0);
                let (t, b) = (h / 2.0 - h * a1 / 2.0, h / 2.0 + h * a1 / 2.0);
                poly(cr, &[(0.0, h / 2.0), (hd, 0.0), (hd, t), (w - hd, t), (w - hd, 0.0), (w, h / 2.0), (w - hd, h),
                       (w - hd, b), (hd, b), (hd, h)]);
            }
            "upDownArrow" => {
                let a1 = adj(0, 50000.0);
                let hd = (ss * adj(1, 50000.0)).min(h / 2.0);
                let (l, r) = (w / 2.0 - w * a1 / 2.0, w / 2.0 + w * a1 / 2.0);
                poly(cr, &[(w / 2.0, 0.0), (w, hd), (r, hd), (r, h - hd), (w, h - hd), (w / 2.0, h), (0.0, h - hd),
                       (l, h - hd), (l, hd), (0.0, hd)]);
            }
            "notchedRightArrow" => {
                let a1 = adj(0, 50000.0);
                let hd = ss * adj(1, 50000.0);
                let (t, b) = (h / 2.0 - h * a1 / 2.0, h / 2.0 + h * a1 / 2.0);
                let notch = hd * (h * a1 / 2.0) / (h / 2.0);
                poly(cr, &[(0.0, t), (w - hd, t), (w - hd, 0.0), (w, h / 2.0), (w - hd, h), (w - hd, b), (0.0, b),
                       (notch, h / 2.0)]);
            }
            "circularArrow" => {
                let th = ss * 0.125;
                let (cx, cy) = (w / 2.0, h / 2.0);
                let (rx, ry) = (w / 2.0 - th * 1.5, h / 2.0 - th * 1.5);
                let a0 = std::f64::consts::PI * 1.05;
                let a1 = std::f64::consts::PI * 1.95;
                cr.save();
                cr.translate(cx, cy);
                cr.scale(1.0, ry / rx.max(1e-6));
                cr.arc(0.0, 0.0, rx + th / 2.0, a0, a1);
                cr.arc_negative(0.0, 0.0, (rx - th / 2.0).max(0.5), a1, a0);
                cr.close_path();
                cr.restore();
                let (ex, ey) = (cx + a1.dcos() * rx, cy + a1.dsin() * ry);
                poly(cr, &[(ex - th * 1.5, ey), (ex + th * 1.5, ey), (ex, ey + th * 1.6)]);
            }
            "flowChartTerminator" => {
                let r = (h / 2.0).min(w / 2.0);
                cr.new_sub_path();
                cr.arc(w - r, h / 2.0, r, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
                cr.arc(r, h / 2.0, r, std::f64::consts::FRAC_PI_2, 1.5 * std::f64::consts::PI);
                cr.close_path();
            }
            "flowChartDocument" => {
                cr.move_to(0.0, 0.0);
                cr.line_to(w, 0.0);
                cr.line_to(w, h * 0.83);
                cr.curve_to(w * 0.75, h * 0.7, w * 0.5, h * 0.85, w * 0.35, h * 0.95);
                cr.curve_to(w * 0.2, h * 1.03, w * 0.08, h * 0.95, 0.0, h * 0.9);
                cr.close_path();
            }
            "star24" | "star7" | "star10" | "star12" | "star16" | "star32" => {
                let n: usize = shape.geom[4..].parse().unwrap_or(24);
                let inner = if n >= 16 { 0.8 } else { 0.62 };
                let mut pts = Vec::new();
                for i in 0..n * 2 {
                    let a = -std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::PI / n as f64;
                    let r = if i % 2 == 0 { 1.0 } else { inner };
                    pts.push((w / 2.0 + a.dcos() * r * w / 2.0, h / 2.0 + a.dsin() * r * h / 2.0));
                }
                poly(cr, &pts);
            }
            "horizontalScroll" => {
                let a = ss * adj(0, 12500.0);
                cr.move_to(a, a);
                cr.line_to(w - a, a);
                cr.line_to(w - a, a / 2.0);
                cr.arc(w - a / 2.0, a / 2.0, a / 2.0, std::f64::consts::PI, 0.0);
                cr.line_to(w, h - a);
                cr.line_to(a, h - a);
                cr.line_to(a, h - a / 2.0);
                cr.arc(a / 2.0, h - a / 2.0, a / 2.0, 0.0, std::f64::consts::PI);
                cr.line_to(0.0, a + a / 2.0);
                cr.arc(a / 2.0, a + a / 2.0, a / 2.0, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
                cr.close_path();
            }
            "wedgeRectCallout" | "wedgeRoundRectCallout" | "wedgeEllipseCallout" => {
                // the tail: adj1/adj2 give its tip relative to the centre
                let tx = w / 2.0 + w * adj(0, -20833.0);
                let ty = h / 2.0 + h * adj(1, 62500.0);
                if shape.geom == "wedgeEllipseCallout" {
                    ellipse(cr, w / 2.0, h / 2.0, w / 2.0, h / 2.0, false);
                    poly(cr, &[(w * 0.3, h * 0.8), (tx, ty), (w * 0.45, h * 0.95)]);
                } else {
                    let r = if shape.geom == "wedgeRoundRectCallout" { ss * 0.16667 } else { 0.0 };
                    cr.new_sub_path();
                    if r > 0.0 {
                        cr.arc(r, r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
                        cr.arc(w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
                        cr.arc(w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
                    } else {
                        cr.move_to(0.0, 0.0);
                        cr.line_to(w, 0.0);
                        cr.line_to(w, h);
                    }
                    cr.line_to(w * 0.58, h);
                    cr.line_to(tx, ty);
                    cr.line_to(w * 0.42, h);
                    if r > 0.0 {
                        cr.arc(r, h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
                    } else {
                        cr.line_to(0.0, h);
                    }
                    cr.close_path();
                }
            }
            "flowChartProcess" | "rect" | _ => {
                cr.rectangle(0.0, 0.0, w, h);
            }
        }
        true
    }

pub fn outline(cr: &mut Cx, shape: &Shape, w: f64, h: f64) -> bool {
        path(cr, shape, w, h)
    }

pub fn draw(cr: &mut Cx, shape: &Shape, w: f64, h: f64) {
        if matches!(shape.geom.as_str(), "cloud" | "cloudCallout") {
            // overlapping bumps: one outline round them all
            if let Some((lw, c, _)) = &shape.line {
                path(cr, shape, w, h);
                set(cr, *c, 1.0);
                cr.set_line_width(lw.max(0.25) * 2.0);
                cr.stroke();
            }
            path(cr, shape, w, h);
            let (c, a) = shape.fill.unwrap_or((0xFFFFFF, 1.0));
            set(cr, c, a);
            cr.fill();
            return;
        }
        let closed = path(cr, shape, w, h);
        if closed {
            if let Some((c, a)) = shape.fill {
                set(cr, c, a);
                cr.fill_preserve();
            }
        }
        if let Some((lw, c, dash)) = &shape.line {
            set(cr, *c, 1.0);
            cr.set_line_width(lw.max(0.25));
            let d = lw.max(0.5);
            match dash.as_str() {
                "dash" | "lgDash" | "sysDash" | "dashDot" | "lgDashDot" => cr.set_dash(&[d * 4.0, d * 3.0], 0.0),
                "dot" | "sysDot" | "shortdot" => cr.set_dash(&[d, d * 2.0], 0.0),
                _ => {}
            }
            cr.stroke();
            cr.set_dash(&[], 0.0);
            if shape.ends != (false, false) {
                // arrowheads along the line's first and last segments
                let (a, b) = match shape.geom.as_str() {
                    "bentConnector3" => (((w * 0.5, 0.0), (0.0, 0.0)), ((w * 0.5, h), (w, h))),
                    "bentConnector2" => (((w, 0.0), (0.0, 0.0)), ((w, 0.0), (w, h))),
                    "curvedConnector3" => (((w * 0.5, 0.0), (0.0, 0.0)), ((w * 0.5, h), (w, h))),
                    _ => (((w, h), (0.0, 0.0)), ((0.0, 0.0), (w, h))),
                };
                let size = (lw * 3.0).max(5.0);
                for (on, (from, to)) in [(shape.ends.0, a), (shape.ends.1, b)] {
                    if !on {
                        continue;
                    }
                    let ang = (to.1 - from.1).datan2(to.0 - from.0);
                    let (ca, sa) = (ang.dcos(), ang.dsin());
                    cr.move_to(to.0, to.1);
                    cr.line_to(to.0 - size * ca + size * 0.5 * sa, to.1 - size * sa - size * 0.5 * ca);
                    cr.line_to(to.0 - size * ca - size * 0.5 * sa, to.1 - size * sa + size * 0.5 * ca);
                    cr.close_path();
                    cr.fill();
                }
            }
        } else {
            cr.new_path();
        }
    }

fn ellipse(cr: &mut Cx, cx: f64, cy: f64, rx: f64, ry: f64, reverse: bool) {
    cr.save();
    cr.translate(cx, cy);
    cr.scale(rx.max(1e-6), ry.max(1e-6));
    cr.new_sub_path();
    if reverse {
        cr.arc_negative(0.0, 0.0, 1.0, 0.0, -std::f64::consts::TAU);
    } else {
        cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
    }
    cr.restore();
}

fn elliptic_arc(cr: &mut Cx, cx: f64, cy: f64, rx: f64, ry: f64, a0: f64, a1: f64) {
    cr.save();
    cr.translate(cx, cy);
    cr.scale(rx.max(1e-6), ry.max(1e-6));
    cr.arc(0.0, 0.0, 1.0, a0, a1);
    cr.restore();
}

fn set(cr: &mut Cx, c: u32, a: f64) {
    cr.set_source_rgba(((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0, a);
}


/// A shape's outline as closed polygons in a w by h box: what text flows
/// round when it is set to follow the shape.
pub fn polygons(shape: &Shape, w: f64, h: f64) -> Vec<Vec<(f64, f64)>> {
    let Some(mut pm) = folio_kit::gfx::sk::Pixmap::new(1, 1) else { return Vec::new() };
    let mut glyphs = folio_canvas::Glyphs::new();
    let mut cx = Cx::new(&mut pm, &mut glyphs);
    if !path(&mut cx, shape, w, h) {
        return Vec::new();
    }
    cx.copy_path().flatten(0.25)
}
