//! Geometry over the model: cutting a loop into triangles, telling inside from
//! outside, and what a ray hits.

use crate::math::{Plane, V2, V3, TOL};
use crate::model::{Entities, Kind, Ref};
use crate::view::Proj;

/// A loop of points as 2D, in the plane's own axes.
pub fn flatten(pts: &[V3], plane: &Plane) -> Vec<V2> {
    let (u, v) = plane.axes();
    let o = pts.first().copied().unwrap_or_default();
    pts.iter().map(|p| V2::new((*p - o).dot(u), (*p - o).dot(v))).collect()
}

fn area2(p: &[V2]) -> f64 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let b = p[(i + 1) % p.len()];
        a += p[i].cross(b);
    }
    a
}

fn inside(a: V2, b: V2, c: V2, p: V2) -> bool {
    let d1 = (b - a).cross(p - a);
    let d2 = (c - b).cross(p - b);
    let d3 = (a - c).cross(p - c);
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// Cut a flat loop into triangles by clipping ears. Returns index triples into
/// the loop. Holes are left to the caller to bridge in first.
pub fn triangulate(poly: &[V2]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if area2(poly) < 0.0 {
        idx.reverse();
    }
    let mut out = Vec::new();
    let mut guard = 0;
    while idx.len() > 2 && guard < n * n + 16 {
        guard += 1;
        let m = idx.len();
        let mut cut = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if (b - a).cross(c - a) <= 1e-12 {
                continue;
            }
            let mut ok = true;
            for &j in &idx {
                if j == ia || j == ib || j == ic {
                    continue;
                }
                if inside(a, b, c, poly[j]) {
                    ok = false;
                    break;
                }
            }
            if ok {
                out.push([ia, ib, ic]);
                idx.remove(i);
                cut = true;
                break;
            }
        }
        if !cut {
            break;
        }
    }
    out
}

/// A face as triangles of world points, holes bridged into the outer loop.
pub fn face_triangles(ents: &Entities, f: usize) -> Vec<[V3; 3]> {
    let Some(face) = ents.faces.get(f) else { return Vec::new() };
    if !face.alive {
        return Vec::new();
    }
    let mut pts: Vec<V3> = face.outer.iter().map(|i| ents.verts[*i].p).collect();
    for h in &face.holes {
        // bridge: join the hole to the outer loop at the nearest pair of points
        let hole: Vec<V3> = h.iter().map(|i| ents.verts[*i].p).collect();
        if hole.len() < 3 {
            continue;
        }
        let mut best = (f64::MAX, 0usize, 0usize);
        for (i, a) in pts.iter().enumerate() {
            for (j, b) in hole.iter().enumerate() {
                let d = (*a - *b).len2();
                if d < best.0 {
                    best = (d, i, j);
                }
            }
        }
        let (_, i, j) = best;
        let mut joined = pts[..=i].to_vec();
        for k in 0..hole.len() {
            joined.push(hole[(j + k) % hole.len()]);
        }
        joined.push(hole[j]);
        joined.extend_from_slice(&pts[i..]);
        pts = joined;
    }
    let Some(plane) = crate::model::plane_of(&pts) else { return Vec::new() };
    let flat = flatten(&pts, &plane);
    triangulate(&flat).iter().map(|t| [pts[t[0]], pts[t[1]], pts[t[2]]]).collect()
}

/// Where a ray meets a triangle (Möller–Trumbore), as a distance along it.
pub fn ray_triangle(o: V3, d: V3, a: V3, b: V3, c: V3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let h = d.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(h) * inv;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-6).then_some(t)
}

/// What is under a point on the window: the nearest edge or vertex if one is
/// close enough, else the nearest face.
pub fn pick(ents: &Entities, proj: &Proj, x: f64, y: f64, grab: f64) -> Option<(Ref, V3)> {
    let (o, d) = proj.ray(x, y);
    let mut best_face: Option<(f64, usize, V3)> = None;
    for f in 0..ents.faces.len() {
        for t in face_triangles(ents, f) {
            if let Some(dist) = ray_triangle(o, d, t[0], t[1], t[2]) {
                if best_face.map(|(b, _, _)| dist < b).unwrap_or(true) {
                    best_face = Some((dist, f, o + d * dist));
                }
            }
        }
    }
    // a vertex or an edge wins when it is within `grab` pixels
    let mut best_near: Option<(f64, Ref, V3)> = None;
    for (i, v) in ents.verts.iter().enumerate() {
        if !v.alive {
            continue;
        }
        let s = proj.project(v.p);
        if !s.infront {
            continue;
        }
        let dd = ((s.x - x).powi(2) + (s.y - y).powi(2)).sqrt();
        if dd <= grab && best_near.as_ref().map(|(b, _, _)| dd < *b).unwrap_or(true) {
            best_near = Some((dd, Ref::new(Kind::Vertex, i), v.p));
        }
    }
    if best_near.is_none() {
        for (i, e) in ents.edges.iter().enumerate() {
            if !e.alive {
                continue;
            }
            let (a, b) = (ents.verts[e.v[0]].p, ents.verts[e.v[1]].p);
            let (sa, sb) = (proj.project(a), proj.project(b));
            if !sa.infront || !sb.infront {
                continue;
            }
            let (dd, t) = seg_dist(x, y, sa.x, sa.y, sb.x, sb.y);
            if dd <= grab && best_near.as_ref().map(|(bb, _, _)| dd < *bb).unwrap_or(true) {
                best_near = Some((dd, Ref::new(Kind::Edge, i), a + (b - a) * t));
            }
        }
    }
    if let Some((_, r, p)) = best_near {
        return Some((r, p));
    }
    best_face.map(|(_, f, p)| (Ref::new(Kind::Face, f), p))
}

fn seg_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> (f64, f64) {
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-9 {
        return (((px - ax).powi(2) + (py - ay).powi(2)).sqrt(), 0.0);
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0.0, 1.0);
    let (qx, qy) = (ax + dx * t, ay + dy * t);
    ((((px - qx).powi(2)) + ((py - qy).powi(2))).sqrt(), t)
}

/// Is the point inside the loop, in the plane's own axes?
pub fn point_in_loop(p: V3, pts: &[V3], plane: &Plane) -> bool {
    if !plane.on(p) {
        return false;
    }
    let flat = flatten(pts, plane);
    let (u, v) = plane.axes();
    let o = pts.first().copied().unwrap_or_default();
    let q = V2::new((p - o).dot(u), (p - o).dot(v));
    let mut inside = false;
    let n = flat.len();
    for i in 0..n {
        let a = flat[i];
        let b = flat[(i + 1) % n];
        if (a.y > q.y) != (b.y > q.y) {
            let x = a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if x > q.x {
                inside = !inside;
            }
        }
    }
    let _ = TOL;
    inside
}
