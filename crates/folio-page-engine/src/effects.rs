//! Picture and shape effects: shadows (outer and inner), reflection, glow,
//! soft edges, bevel, and the picture's colour adjustments (brightness,
//! contrast, grayscale, black and white, washout, transparency, duotone),
//! read from DrawingML (the values only; the drawing layer applies them).

use folio_math::Det;
use crate::xml::{NodeId, XmlDoc};
use std::collections::HashMap;

const EMU: f64 = 12700.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    /// points
    pub blur: f64,
    pub dist: f64,
    /// degrees, clockwise from the right
    pub dir: f64,
    pub color: u32,
    pub alpha: f64,
    /// scale (1.0 = 100%)
    pub sx: f64,
    pub sy: f64,
    /// where scaling is anchored: "b", "ctr", "tl"...
    pub algn: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reflection {
    pub blur: f64,
    pub start_alpha: f64,
    pub end_alpha: f64,
    /// how far down the picture the fade reaches (0..1)
    pub end_pos: f64,
    pub dist: f64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Fx {
    pub outer: Option<Shadow>,
    pub inner: Option<Shadow>,
    /// radius (points), colour, opacity
    pub glow: Option<(f64, u32, f64)>,
    pub soft_edge: Option<f64>,
    pub reflection: Option<Reflection>,
    /// bevel width, height (points) and preset
    pub bevel: Option<(f64, f64, String)>,
    /// 3-D rotation: latitude, longitude, revolution (degrees)
    pub camera: Option<(f64, f64, f64)>,
    /// the camera's field of view (degrees) for perspective presets: drawn foreshortened
    pub persp: Option<f64>,
    /// -1..1
    pub brightness: f64,
    pub contrast: f64,
    pub grayscale: bool,
    /// black and white at this threshold (0..1)
    pub bilevel: Option<f64>,
    /// the picture's opacity (1.0 opaque)
    pub opacity: f64,
    pub duotone: Option<(u32, u32)>,
}

impl Fx {
    pub fn is_empty(&self) -> bool {
        *self == Fx { opacity: 1.0, ..Default::default() }
    }

    /// Anything to do to the picture's own pixels?
    pub fn recolors(&self) -> bool {
        self.brightness != 0.0 || self.contrast != 0.0 || self.grayscale || self.bilevel.is_some()
            || self.opacity < 1.0 || self.duotone.is_some()
    }

    /// Room the effects take around a w x h picture: l, t, r, b (points).
    pub fn extents(&self, _w: f64, h: f64) -> [f64; 4] {
        let mut e = [0.0f64; 4];
        let mut grow = |l: f64, t: f64, r: f64, b: f64| {
            e[0] = e[0].max(l);
            e[1] = e[1].max(t);
            e[2] = e[2].max(r);
            e[3] = e[3].max(b);
        };
        if let Some(s) = &self.outer {
            let (dx, dy) = (s.dist * s.dir.to_radians().dcos(), s.dist * s.dir.to_radians().dsin());
            let b = s.blur;
            grow((b - dx).max(0.0), (b - dy).max(0.0), (b + dx).max(0.0), (b + dy).max(0.0));
        }
        if let Some((r, _, _)) = self.glow {
            grow(r, r, r, r);
        }
        if let Some(rf) = &self.reflection {
            grow(0.0, 0.0, 0.0, rf.dist + h * rf.end_pos + rf.blur);
        }
        e
    }
}

fn val(dom: &XmlDoc, n: NodeId, k: &str, d: f64) -> f64 {
    dom.attr(n, k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d)
}

/// The rotation a camera preset stands for (latitude, longitude, revolution).
pub fn camera_preset(prst: &str) -> Option<(f64, f64, f64)> {
    Some(match prst {
        "isometricLeftDown" => (35.0, 45.0, 0.0),
        "isometricRightUp" => (-35.0, -45.0, 0.0),
        "isometricTopUp" => (35.0, -45.0, 0.0),
        "isometricBottomDown" => (-35.0, 45.0, 0.0),
        "isometricOffAxis1Left" => (10.0, 64.0, 0.0),
        "isometricOffAxis1Right" => (10.0, -64.0, 0.0),
        "isometricOffAxis2Left" => (-10.0, 64.0, 0.0),
        "isometricOffAxis2Right" => (-10.0, -64.0, 0.0),
        "perspectiveLeft" => (0.0, 20.0, 0.0),
        "perspectiveRight" => (0.0, -20.0, 0.0),
        "perspectiveBelow" => (-20.0, 0.0, 0.0),
        "perspectiveAbove" => (20.0, 0.0, 0.0),
        "perspectiveRelaxed" => (30.0, 0.0, 0.0),
        "perspectiveContrastingLeftFacing" => (10.0, 40.0, 0.0),
        "perspectiveContrastingRightFacing" => (10.0, -40.0, 0.0),
        "obliqueTopLeft" => (20.0, 20.0, 0.0),
        "obliqueTopRight" => (20.0, -20.0, 0.0),
        "obliqueBottomLeft" => (-20.0, 20.0, 0.0),
        "obliqueBottomRight" => (-20.0, -20.0, 0.0),
        _ => return None,
    })
}

/// The flat picture of a 3-D rotation: x' = a x + c y, y' = b x + d y.
pub fn camera_matrix(lat: f64, lon: f64, rev: f64) -> (f64, f64, f64, f64) {
    let (la, lo, rv) = (lat.to_radians(), lon.to_radians(), rev.to_radians());
    // turn about the vertical axis (longitude), then tip (latitude), then spin
    let (a, b, c, d) = (lo.dcos(), lo.dsin() * la.dsin(), 0.0, la.dcos());
    let (cs, sn) = (rv.dcos(), rv.dsin());
    (cs * a - sn * b, sn * a + cs * b, cs * c - sn * d, sn * c + cs * d)
}

fn color_of(dom: &XmlDoc, n: NodeId, colors: &HashMap<String, u32>) -> (u32, f64) {
    dom.elements(n).find_map(|c| crate::shapes::color(dom, c, colors)).unwrap_or((0, 1.0))
}

fn shadow(dom: &XmlDoc, n: NodeId, colors: &HashMap<String, u32>) -> Shadow {
    let (color, alpha) = color_of(dom, n, colors);
    Shadow {
        blur: val(dom, n, "blurRad", 0.0) / EMU,
        dist: val(dom, n, "dist", 0.0) / EMU,
        dir: val(dom, n, "dir", 0.0) / 60000.0,
        color,
        alpha,
        sx: val(dom, n, "sx", 100000.0) / 100000.0,
        sy: val(dom, n, "sy", 100000.0) / 100000.0,
        algn: dom.attr(n, "algn").unwrap_or("b").to_string(),
    }
}

/// The effects of a picture or shape: its shape properties (a:effectLst,
/// a:sp3d) and its picture's a:blip.
pub fn parse(dom: &XmlDoc, sppr: Option<NodeId>, blip: Option<NodeId>, colors: &HashMap<String, u32>) -> Fx {
    let mut fx = Fx { opacity: 1.0, ..Default::default() };
    if let Some(sp) = sppr {
        if let Some(el) = dom.child(sp, "a:effectLst") {
            for c in dom.elements(el) {
                match dom.name(c) {
                    "a:outerShdw" => fx.outer = Some(shadow(dom, c, colors)),
                    "a:prstShdw" => {
                        let mut s = shadow(dom, c, colors);
                        if s.dist == 0.0 {
                            s.dist = 3.0;
                            s.dir = 45.0;
                        }
                        fx.outer = Some(s);
                    }
                    "a:innerShdw" => fx.inner = Some(shadow(dom, c, colors)),
                    "a:glow" => {
                        let (col, a) = color_of(dom, c, colors);
                        fx.glow = Some((val(dom, c, "rad", 0.0) / EMU, col, a));
                    }
                    "a:softEdge" => fx.soft_edge = Some(val(dom, c, "rad", 0.0) / EMU),
                    "a:reflection" => {
                        fx.reflection = Some(Reflection {
                            blur: val(dom, c, "blurRad", 0.0) / EMU,
                            start_alpha: val(dom, c, "stA", 100000.0) / 100000.0,
                            end_alpha: val(dom, c, "endA", 0.0) / 100000.0,
                            end_pos: val(dom, c, "endPos", 100000.0) / 100000.0,
                            dist: val(dom, c, "dist", 0.0) / EMU,
                        })
                    }
                    _ => {}
                }
            }
        }
        if let Some(cam) = dom.child(sp, "a:scene3d").and_then(|s| dom.child(s, "a:camera")) {
            let r = dom.child(cam, "a:rot").map(|r| (val(dom, r, "lat", 0.0) / 60000.0,
                                                     val(dom, r, "lon", 0.0) / 60000.0,
                                                     val(dom, r, "rev", 0.0) / 60000.0));
            let r = r.or_else(|| camera_preset(dom.attr(cam, "prst").unwrap_or("")));
            fx.camera = r.filter(|(a, b, c)| a.abs() + b.abs() + c.abs() > 0.01);
            let prst = dom.attr(cam, "prst").unwrap_or("");
            let fov = val(dom, cam, "fov", -1.0);
            fx.persp = if fov > 0.0 {
                Some((fov / 60000.0).clamp(1.0, 179.0))
            } else if prst.starts_with("perspective") {
                Some(45.0)
            } else {
                None
            };
        }
        if let Some(b) = dom.child(sp, "a:sp3d").and_then(|s| dom.child(s, "a:bevelT")) {
            fx.bevel = Some((val(dom, b, "w", 76200.0) / EMU, val(dom, b, "h", 76200.0) / EMU,
                             dom.attr(b, "prst").unwrap_or("circle").to_string()));
        }
    }
    if let Some(bl) = blip {
        for c in dom.elements(bl) {
            match dom.name(c) {
                "a:lum" => {
                    fx.brightness = val(dom, c, "bright", 0.0) / 100000.0;
                    fx.contrast = val(dom, c, "contrast", 0.0) / 100000.0;
                }
                "a:grayscl" => fx.grayscale = true,
                "a:biLevel" => fx.bilevel = Some(val(dom, c, "thresh", 50000.0) / 100000.0),
                "a:alphaModFix" => fx.opacity = val(dom, c, "amt", 100000.0) / 100000.0,
                "a:duotone" => {
                    let cs: Vec<(u32, f64)> = dom.elements(c).filter_map(|k| crate::shapes::color(dom, k, colors)).collect();
                    if cs.len() == 2 {
                        fx.duotone = Some((cs[0].0, cs[1].0));
                    }
                }
                _ => {}
            }
        }
        // Word's brightness/contrast (Picture Corrections) live in an extension
        for e in dom.elements(bl) {
            if dom.is(e, "a:extLst") {
                for ext in dom.elements(e) {
                    for k in descendants(dom, ext) {
                        if dom.name(k).ends_with(":brightnessContrast") {
                            fx.brightness = val(dom, k, "bright", 0.0) / 100000.0;
                            fx.contrast = val(dom, k, "contrast", 0.0) / 100000.0;
                        }
                    }
                }
            }
        }
    }
    fx
}

fn descendants(dom: &XmlDoc, n: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack = vec![n];
    while let Some(x) = stack.pop() {
        for c in dom.elements(x) {
            out.push(c);
            stack.push(c);
        }
    }
    out
}

// ------------------------------------------------------------- pixel work







/// Where a point (centred coordinates, points) of a flat picture ends up under a 3-D
/// rotation seen through a camera with this field of view.
pub fn project(x: f64, y: f64, lat: f64, lon: f64, rev: f64, dist: f64) -> (f64, f64) {
    let (la, lo, rv) = (lat.to_radians(), lon.to_radians(), rev.to_radians());
    // turn about the vertical axis, tip, then spin (as camera_matrix)
    let x1 = x * lo.dcos();
    let z1 = x * lo.dsin();
    let y2 = y * la.dcos() + z1 * la.dsin();
    let z2 = z1 * la.dcos() - y * la.dsin();
    let (cs, sn) = (rv.dcos(), rv.dsin());
    let x3 = cs * x1 - sn * y2;
    let y3 = sn * x1 + cs * y2;
    let s = dist / (dist + z2).max(dist * 0.05);
    (x3 * s, y3 * s)
}

/// The 3x3 projective map taking the four points `from` to `to` (row major, h33 = 1).
pub fn homography(from: [(f64, f64); 4], to: [(f64, f64); 4]) -> Option<[f64; 9]> {
    let mut a = [[0.0f64; 9]; 8];
    for i in 0..4 {
        let (x, y) = from[i];
        let (u, v) = to[i];
        a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for c in 0..8 {
        let mut piv = c;
        for r in c + 1..8 {
            if a[r][c].abs() > a[piv][c].abs() {
                piv = r;
            }
        }
        if a[piv][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, piv);
        for r in 0..8 {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..9 {
                    a[r][k] -= f * a[c][k];
                }
            }
        }
    }
    let mut h = [0.0; 9];
    for i in 0..8 {
        h[i] = a[i][8] / a[i][i];
    }
    h[8] = 1.0;
    Some(h)
}

