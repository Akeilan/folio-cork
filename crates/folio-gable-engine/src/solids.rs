//! Making solids: a box, and the push/pull that turns a face into one.

use crate::geom::face_triangles;
use crate::math::{V3, TOL};
use crate::model::{plane_of, Entities};

/// A box with one corner at `at`, `size` across.
pub fn add_box(ents: &mut Entities, at: V3, size: V3) -> Vec<usize> {
    let (x, y, z) = (size.x, size.y, size.z);
    let p = |dx: f64, dy: f64, dz: f64| at + V3::new(dx, dy, dz);
    let c = [
        p(0.0, 0.0, 0.0),
        p(x, 0.0, 0.0),
        p(x, y, 0.0),
        p(0.0, y, 0.0),
        p(0.0, 0.0, z),
        p(x, 0.0, z),
        p(x, y, z),
        p(0.0, y, z),
    ];
    let v: Vec<usize> = c.iter().map(|q| ents.add_vertex(*q)).collect();
    let loops = [
        [v[0], v[3], v[2], v[1]], // bottom, facing down
        [v[4], v[5], v[6], v[7]], // top
        [v[0], v[1], v[5], v[4]], // front
        [v[1], v[2], v[6], v[5]], // right
        [v[2], v[3], v[7], v[6]], // back
        [v[3], v[0], v[4], v[7]], // left
    ];
    let mut faces = Vec::new();
    for l in loops {
        if let Some(f) = ents.add_face(&l) {
            faces.push(f);
        }
    }
    ents.touch();
    faces
}

/// A flat face, from a loop of points in order.
pub fn add_polygon(ents: &mut Entities, pts: &[V3]) -> Option<usize> {
    if pts.len() < 3 {
        return None;
    }
    let v: Vec<usize> = pts.iter().map(|p| ents.add_vertex(*p)).collect();
    ents.add_face(&v)
}

/// Push or pull a face along its normal by `dist`: the face moves and the walls
/// are built behind it. Returns the faces the new solid is made of.
pub fn push_pull(ents: &mut Entities, f: usize, dist: f64) -> Vec<usize> {
    if dist.abs() < TOL {
        return Vec::new();
    }
    let Some(face) = ents.faces.get(f).cloned() else { return Vec::new() };
    if !face.alive {
        return Vec::new();
    }
    let n = face.plane.n;
    let d = n * dist;
    let old: Vec<usize> = face.outer.clone();
    let pts: Vec<V3> = old.iter().map(|i| ents.verts[*i].p).collect();
    // the moved copy of the loop
    let top: Vec<usize> = pts.iter().map(|p| ents.add_vertex(*p + d)).collect();
    let mut made = Vec::new();
    // the walls
    for i in 0..old.len() {
        let j = (i + 1) % old.len();
        let wall = if dist > 0.0 {
            [old[i], old[j], top[j], top[i]]
        } else {
            [old[j], old[i], top[i], top[j]]
        };
        if let Some(w) = ents.add_face(&wall) {
            made.push(w);
        }
    }
    // the new top, facing the way the old face did
    let top_loop: Vec<usize> = if dist > 0.0 { top.clone() } else { top.iter().rev().copied().collect() };
    if let Some(t) = ents.add_face(&top_loop) {
        made.push(t);
    }
    // the face that was pushed is now inside the solid
    ents.remove_face(f);
    ents.touch();
    made
}

/// How much surface a face has.
pub fn face_area(ents: &Entities, f: usize) -> f64 {
    face_triangles(ents, f)
        .iter()
        .map(|t| (t[1] - t[0]).cross(t[2] - t[0]).len() * 0.5)
        .sum()
}

/// How much room the model's closed shells take up, by the divergence theorem.
pub fn volume(ents: &Entities) -> f64 {
    let mut v = 0.0;
    for f in 0..ents.faces.len() {
        let Some(face) = ents.faces.get(f) else { continue };
        if !face.alive {
            continue;
        }
        let pts = ents.face_points(f);
        let Some(pl) = plane_of(&pts) else { continue };
        for t in face_triangles(ents, f) {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            let sign = if n.dot(pl.n) < 0.0 { -1.0 } else { 1.0 };
            v += sign * t[0].dot((t[1]).cross(t[2])) / 6.0;
        }
    }
    v.abs()
}
