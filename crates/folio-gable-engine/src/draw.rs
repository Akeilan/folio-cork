//! Drawing the model: a z-buffer over the pixels, triangles filled by hand and
//! the edges drawn on top. No GPU, so a drawing looks the same everywhere.

use crate::geom::face_triangles;
use crate::math::V3;
use crate::model::{Definition, Entities, Material, Model, Ref, Kind};
use crate::math::Xf;
use crate::view::Proj;
use folio_kit::gfx::sk;

/// What the viewport shows besides the model.
#[derive(Clone, Debug)]
pub struct Style {
    pub background: u32,
    pub sky: bool,
    pub ground: u32,
    pub face_front: u32,
    pub face_back: u32,
    pub edge: u32,
    pub edge_width: f64,
    /// draw the faces at all (wireframe when false)
    pub faces: bool,
    /// the axes through the origin
    pub axes: bool,
    pub highlight: u32,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            background: 0xF2F2F2,
            sky: true,
            ground: 0xE4E2DD,
            face_front: 0xF5F4F2,
            face_back: 0xC9D3DE,
            edge: 0x2A2A2A,
            edge_width: 1.0,
            faces: true,
            axes: true,
            highlight: 0x1D7FD8,
        }
    }
}

/// A picture of the model, with a depth for every pixel so the next thing drawn
/// knows whether it is in front.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub color: Vec<u32>,
    pub depth: Vec<f32>,
}

impl Frame {
    pub fn new(w: u32, h: u32, background: u32) -> Frame {
        Frame { w, h, color: vec![background | 0xFF00_0000; (w * h) as usize], depth: vec![f32::MAX; (w * h) as usize] }
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, z: f32, c: u32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let i = (y as u32 * self.w + x as u32) as usize;
        if z < self.depth[i] {
            self.depth[i] = z;
            self.color[i] = c | 0xFF00_0000;
        }
    }

    /// The picture as a pixmap the canvas can draw.
    pub fn to_pixmap(&self) -> Option<sk::Pixmap> {
        let mut pm = sk::Pixmap::new(self.w.max(1), self.h.max(1))?;
        let px = pm.pixels_mut();
        for (i, c) in self.color.iter().enumerate() {
            let (r, g, b) = (((c >> 16) & 255) as u8, ((c >> 8) & 255) as u8, (c & 255) as u8);
            px[i] = sk::PremultipliedColorU8::from_rgba(r, g, b, 255).unwrap_or_else(|| sk::PremultipliedColorU8::from_rgba(0, 0, 0, 255).unwrap());
        }
        Some(pm)
    }
}

fn shade(color: u32, light: f64) -> u32 {
    let f = |v: u32| ((v as f64 * light).clamp(0.0, 255.0)) as u32;
    (f((color >> 16) & 255) << 16) | (f((color >> 8) & 255) << 8) | f(color & 255)
}

/// Fill one triangle, with the depth written as it goes.
fn triangle(frame: &mut Frame, p: [(f64, f64, f64); 3], color: u32) {
    let minx = p.iter().map(|v| v.0).fold(f64::MAX, f64::min).floor().max(0.0) as i32;
    let maxx = p.iter().map(|v| v.0).fold(f64::MIN, f64::max).ceil().min(frame.w as f64) as i32;
    let miny = p.iter().map(|v| v.1).fold(f64::MAX, f64::min).floor().max(0.0) as i32;
    let maxy = p.iter().map(|v| v.1).fold(f64::MIN, f64::max).ceil().min(frame.h as f64) as i32;
    if minx >= maxx || miny >= maxy {
        return;
    }
    let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0);
    if area.abs() < 1e-9 {
        return;
    }
    let inv = 1.0 / area;
    for y in miny..maxy {
        for x in minx..maxx {
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let w0 = ((p[1].0 - p[0].0) * (fy - p[0].1) - (p[1].1 - p[0].1) * (fx - p[0].0)) * inv;
            let w1 = ((p[2].0 - p[1].0) * (fy - p[1].1) - (p[2].1 - p[1].1) * (fx - p[1].0)) * inv;
            let w2 = ((p[0].0 - p[2].0) * (fy - p[2].1) - (p[0].1 - p[2].1) * (fx - p[2].0)) * inv;
            if (w0 < 0.0 || w1 < 0.0 || w2 < 0.0) && (w0 > 0.0 || w1 > 0.0 || w2 > 0.0) {
                continue;
            }
            // barycentric depth
            let (a, b, c) = (w1, w2, w0);
            let z = (p[0].2 * a + p[1].2 * b + p[2].2 * c) as f32;
            frame.put(x, y, z, color);
        }
    }
}

/// A line, drawn a pixel wide with its depth, so edges sit on their faces.
fn line(frame: &mut Frame, a: (f64, f64, f64), b: (f64, f64, f64), color: u32, width: f64) {
    let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as i32).max(1);
    let half = ((width - 1.0) / 2.0).max(0.0) as i32;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = a.0 + (b.0 - a.0) * t;
        let y = a.1 + (b.1 - a.1) * t;
        let z = (a.2 + (b.2 - a.2) * t) as f32;
        // a hair towards the eye, so an edge is never hidden by its own face
        let z = z * 0.999;
        for dy in -half..=half {
            for dx in -half..=half {
                frame.put(x.round() as i32 + dx, y.round() as i32 + dy, z, color);
            }
        }
    }
}

/// Draw the whole model: the ground, the faces, the edges and the axes.
pub fn render(model: &Model, proj: &Proj, style: &Style, w: u32, h: u32, picked: &[Ref]) -> Frame {
    let mut frame = Frame::new(w, h, style.background);
    if style.sky {
        // a plain horizon: sky above, ground below, from the camera's own level
        for y in 0..h {
            let t = y as f64 / h.max(1) as f64;
            let c = if t < 0.5 { blend(0xDCE8F5, style.background, t * 2.0) } else { blend(style.background, style.ground, (t - 0.5) * 2.0) };
            for x in 0..w {
                let i = (y * w + x) as usize;
                frame.color[i] = c | 0xFF00_0000;
            }
        }
    }
    draw_entities(&mut frame, &model.ents, &model.defs, &model.materials, proj, style, &Xf::identity(), picked, 0);
    if style.axes {
        draw_axes(&mut frame, proj);
    }
    frame
}

fn blend(a: u32, b: u32, t: f64) -> u32 {
    let f = |sa: u32, sb: u32| ((sa as f64 * (1.0 - t) + sb as f64 * t).clamp(0.0, 255.0)) as u32;
    (f((a >> 16) & 255, (b >> 16) & 255) << 16) | (f((a >> 8) & 255, (b >> 8) & 255) << 8) | f(a & 255, b & 255)
}

#[allow(clippy::too_many_arguments)]
fn draw_entities(
    frame: &mut Frame,
    ents: &Entities,
    defs: &[Definition],
    mats: &[Material],
    proj: &Proj,
    style: &Style,
    xf: &Xf,
    picked: &[Ref],
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    let light = V3::new(0.35, -0.5, 0.78).unit();
    if style.faces {
        for f in 0..ents.faces.len() {
            let face = &ents.faces[f];
            if !face.alive || face.hidden {
                continue;
            }
            let tris = face_triangles(ents, f);
            if tris.is_empty() {
                continue;
            }
            let n = xf.dir(face.plane.n).unit();
            let towards = n.dot(proj.cam.forward()) < 0.0;
            let base = if towards {
                mats.get(face.mat_front.max(-1) as usize).map(|m| m.color).unwrap_or(style.face_front)
            } else {
                mats.get(face.mat_back.max(-1) as usize).map(|m| m.color).unwrap_or(style.face_back)
            };
            let base = if face.mat_front < 0 && towards { style.face_front } else if face.mat_back < 0 && !towards { style.face_back } else { base };
            let lam = (n.dot(light).abs() * 0.65 + 0.45).min(1.15);
            let mut color = shade(base, lam);
            if depth == 0 && picked.contains(&Ref::new(Kind::Face, f)) {
                color = blend(color, style.highlight, 0.45);
            }
            for t in tris {
                let s: Vec<(f64, f64, f64)> = t
                    .iter()
                    .map(|p| {
                        let w = xf.point(*p);
                        let sp = proj.project(w);
                        (sp.x, sp.y, sp.depth)
                    })
                    .collect();
                if s.iter().any(|v| !v.2.is_finite() || v.2 <= 0.0) {
                    continue;
                }
                triangle(frame, [s[0], s[1], s[2]], color);
            }
        }
    }
    for (i, e) in ents.edges.iter().enumerate() {
        if !e.alive || e.hidden || (e.soft && style.faces) {
            continue;
        }
        let a = proj.project(xf.point(ents.verts[e.v[0]].p));
        let b = proj.project(xf.point(ents.verts[e.v[1]].p));
        if !a.infront || !b.infront {
            continue;
        }
        let mut c = style.edge;
        let mut width = style.edge_width;
        if depth == 0 && picked.contains(&Ref::new(Kind::Edge, i)) {
            c = style.highlight;
            width = style.edge_width + 2.0;
        }
        line(frame, (a.x, a.y, a.depth), (b.x, b.y, b.depth), c, width);
    }
    for inst in ents.insts.iter().filter(|i| i.alive && !i.hidden) {
        if let Some(d) = defs.get(inst.def.max(0) as usize) {
            let x = inst.xf.then(xf);
            draw_entities(frame, &d.ents, defs, mats, proj, style, &x, picked, depth + 1);
        }
    }
}

/// The three axes through the origin, in red, green and blue.
fn draw_axes(frame: &mut Frame, proj: &Proj) {
    let o = V3::default();
    let len = 100000.0;
    for (dir, color) in [
        (V3::new(1.0, 0.0, 0.0), 0xD03A3Au32),
        (V3::new(0.0, 1.0, 0.0), 0x2E9E4Fu32),
        (V3::new(0.0, 0.0, 1.0), 0x3A6FD0u32),
    ] {
        for s in [-1.0f64, 1.0] {
            let a = proj.project(o);
            let b = proj.project(dir * (len * s));
            if !a.infront || !b.infront {
                continue;
            }
            // the negative half is drawn faint, as the original does
            let c = if s < 0.0 { blend(color, 0xFFFFFF, 0.55) } else { color };
            line(frame, (a.x, a.y, a.depth), (b.x, b.y, b.depth), c, 1.0);
        }
    }
}
