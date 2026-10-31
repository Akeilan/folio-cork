//! Connector routes (K3-02): straight, right-angle (routed around shapes) and curved lines,
//! glued to shapes, with arrowheads, labels and line jumps.

use crate::geom::{self, Seg};
use crate::model::{Arrow, End, Page, Route, Shape};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

#[derive(Clone, Debug, Default)]
pub struct Routed {
    /// Polyline for straight and right-angle routes; for curves [p0, c1, c2, p3].
    pub pts: Vec<(f32, f32)>,
    pub curve: bool,
    /// Directions pointing out of the line at each end (for arrowheads).
    pub dir_a: (f32, f32),
    pub dir_b: (f32, f32),
}

impl Routed {
    pub fn segs(&self) -> Vec<Seg> {
        let mut v = vec![];
        if self.pts.is_empty() {
            return v;
        }
        v.push(Seg::M(self.pts[0].0, self.pts[0].1));
        if self.curve && self.pts.len() == 4 {
            let p = &self.pts;
            v.push(Seg::C(p[1].0, p[1].1, p[2].0, p[2].1, p[3].0, p[3].1));
        } else {
            for p in &self.pts[1..] {
                v.push(Seg::L(p.0, p.1));
            }
        }
        v
    }

    /// The route as a polyline (curves flattened).
    pub fn polyline(&self) -> Vec<(f32, f32)> {
        if self.curve {
            geom::flatten(&self.segs(), 24).into_iter().next().unwrap_or_default()
        } else {
            self.pts.clone()
        }
    }

    /// The point halfway along the route (where the label goes).
    pub fn midpoint(&self) -> (f32, f32) {
        let pl = self.polyline();
        let total: f32 = pl.windows(2).map(|w| dist(w[0], w[1])).sum();
        let mut acc = 0.0;
        for w in pl.windows(2) {
            let d = dist(w[0], w[1]);
            if acc + d >= total / 2.0 && d > 0.0 {
                let t = (total / 2.0 - acc) / d;
                return (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t);
            }
            acc += d;
        }
        pl.first().copied().unwrap_or((0.0, 0.0))
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Outward direction of a connection point: toward the nearest side of the shape's box.
fn side_dir(s: &Shape, p: (f32, f32)) -> (f32, f32) {
    let (x, y, w, h) = geom::bounds(s);
    let d = [(p.1 - y, (0.0, -1.0)), (x + w - p.0, (1.0, 0.0)), (y + h - p.1, (0.0, 1.0)), (p.0 - x, (-1.0, 0.0))];
    d.iter().min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal)).map(|x| x.1).unwrap_or((1.0, 0.0))
}

fn axis_dir(from: (f32, f32), to: (f32, f32)) -> (f32, f32) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    if dx.abs() >= dy.abs() {
        (dx.signum(), 0.0)
    } else {
        (0.0, dy.signum())
    }
}

/// Where an end sits and which way the line leaves it.
fn resolve(page: &Page, e: &End, other: (f32, f32), route: Route) -> ((f32, f32), (f32, f32), Option<usize>) {
    if let Some(s) = e.shape.and_then(|id| page.get(id)) {
        if let Some(i) = e.point {
            if let Some(p) = geom::conn_point(s, i) {
                return (p, side_dir(s, p), Some(i));
            }
        }
        if route == Route::Elbow {
            // Dynamic glue: the side facing the other end, where a line straight out of the
            // centre leaves the outline.
            let c = (s.cx(), s.cy());
            let far = 1.0e5;
            let cands = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)];
            let best = cands
                .iter()
                .map(|&d: &(f32, f32)| {
                    let p = geom::boundary_point(s, (c.0 + d.0 * far, c.1 + d.1 * far));
                    let to = (other.0 - c.0, other.1 - c.1);
                    let facing = d.0 * to.0 + d.1 * to.1;
                    (p, d, dist(p, other) - facing * 0.5)
                })
                .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal));
            if let Some((p, d, _)) = best {
                return (p, d, None);
            }
        }
        let p = geom::boundary_point(s, other);
        let c = (s.cx(), s.cy());
        let d = (p.0 - c.0, p.1 - c.1);
        let l = (d.0 * d.0 + d.1 * d.1).sqrt().max(0.001);
        return (p, (d.0 / l, d.1 / l), None);
    }
    let p = (e.x, e.y);
    (p, axis_dir(other, p), None)
}

fn end_center(page: &Page, e: &End) -> (f32, f32) {
    if let Some(s) = e.shape.and_then(|id| page.get(id)) {
        if let Some(i) = e.point {
            if let Some(p) = geom::conn_point(s, i) {
                return p;
            }
        }
        return (s.cx(), s.cy());
    }
    (e.x, e.y)
}

pub fn route(page: &Page, s: &Shape) -> Routed {
    let Some(c) = &s.conn else { return Routed::default() };
    let ca = end_center(page, &c.a);
    let cb = end_center(page, &c.b);
    let (pa, da, _) = resolve(page, &c.a, cb, c.route);
    let (pb, db, _) = resolve(page, &c.b, if c.route == Route::Elbow { pa } else { ca }, c.route);
    let (pa, da) = if c.a.shape.is_some() && c.a.point.is_none() && c.route != Route::Elbow {
        let r = resolve(page, &c.a, pb, c.route);
        (r.0, r.1)
    } else {
        (pa, da)
    };
    match c.route {
        Route::Straight => {
            let d = norm((pa.0 - pb.0, pa.1 - pb.1));
            Routed { pts: vec![pa, pb], curve: false, dir_a: d, dir_b: (-d.0, -d.1) }
        }
        Route::Curved => {
            let l = dist(pa, pb) * 0.4;
            let c1 = (pa.0 + da.0 * l, pa.1 + da.1 * l);
            let c2 = (pb.0 + db.0 * l, pb.1 + db.1 * l);
            Routed { pts: vec![pa, c1, c2, pb], curve: true, dir_a: norm((pa.0 - c1.0, pa.1 - c1.1)), dir_b: norm((pb.0 - c2.0, pb.1 - c2.1)) }
        }
        Route::Elbow => {
            let skip = [c.a.shape, c.b.shape];
            let pts = elbow(page, pa, da, pb, db, &skip, s.id);
            let n = pts.len();
            let dir_a = if n >= 2 { norm((pts[0].0 - pts[1].0, pts[0].1 - pts[1].1)) } else { (-1.0, 0.0) };
            let dir_b = if n >= 2 { norm((pts[n - 1].0 - pts[n - 2].0, pts[n - 1].1 - pts[n - 2].1)) } else { (1.0, 0.0) };
            Routed { pts, curve: false, dir_a, dir_b }
        }
    }
}

fn norm(d: (f32, f32)) -> (f32, f32) {
    let l = (d.0 * d.0 + d.1 * d.1).sqrt();
    if l < 1e-6 {
        (1.0, 0.0)
    } else {
        (d.0 / l, d.1 / l)
    }
}

#[derive(Copy, Clone, PartialEq)]
struct St {
    cost: f32,
    node: usize,
    dir: u8,
}

impl Eq for St {}
impl Ord for St {
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for St {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

const STUB: f32 = 18.0;
const MARGIN: f32 = 12.0;

/// Right-angle route from `a` leaving along `da` to `b` arriving along `db`, around shapes.
fn elbow(page: &Page, a: (f32, f32), da: (f32, f32), b: (f32, f32), db: (f32, f32), ends: &[Option<u64>; 2], self_id: u64) -> Vec<(f32, f32)> {
    let da = axis_dir((0.0, 0.0), da);
    let db = axis_dir((0.0, 0.0), db);
    let sa = (a.0 + da.0 * STUB, a.1 + da.1 * STUB);
    let sb = (b.0 + db.0 * STUB, b.1 + db.1 * STUB);
    // Obstacles near the route, inflated by the margin.
    let (bx0, by0) = (sa.0.min(sb.0) - 300.0, sa.1.min(sb.1) - 300.0);
    let (bx1, by1) = (sa.0.max(sb.0) + 300.0, sa.1.max(sb.1) + 300.0);
    let mut obs: Vec<(f32, f32, f32, f32)> = vec![];
    for s in &page.shapes {
        if s.id == self_id || s.is_connector() || s.is_group() || !page.visible(s) || matches!(s.kind.as_str(), "frame" | "ink" | "text") {
            continue;
        }
        let (x, y, w, h) = geom::bounds(s);
        let r = (x - MARGIN, y - MARGIN, x + w + MARGIN, y + h + MARGIN);
        if r.2 < bx0 || r.0 > bx1 || r.3 < by0 || r.1 > by1 {
            continue;
        }
        // The shapes the line is glued to only block where the stub doesn't leave them.
        let is_end = ends.contains(&Some(s.id));
        if is_end && ((r.0 < sa.0 && sa.0 < r.2 && r.1 < sa.1 && sa.1 < r.3) || (r.0 < sb.0 && sb.0 < r.2 && r.1 < sb.1 && sb.1 < r.3)) {
            // Stub ends inside the inflated box: shrink the box to the shape itself.
            obs.push((x + 1.0, y + 1.0, x + w - 1.0, y + h - 1.0));
            continue;
        }
        obs.push(r);
        if obs.len() > 160 {
            break;
        }
    }
    let inside = |p: (f32, f32)| obs.iter().any(|o| p.0 > o.0 && p.0 < o.2 && p.1 > o.1 && p.1 < o.3);
    let mut xs = vec![sa.0, sb.0, (sa.0 + sb.0) / 2.0];
    let mut ys = vec![sa.1, sb.1, (sa.1 + sb.1) / 2.0];
    for o in &obs {
        xs.extend([o.0, o.2]);
        ys.extend([o.1, o.3]);
    }
    let dedup = |v: &mut Vec<f32>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        v.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    };
    dedup(&mut xs);
    dedup(&mut ys);
    let nx = xs.len();
    let ny = ys.len();
    if nx * ny > 90_000 {
        return simple_elbow(a, da, sa, sb, db, b);
    }
    let idx = |ix: usize, iy: usize| iy * nx + ix;
    let find = |v: &Vec<f32>, t: f32| v.iter().position(|x| (x - t).abs() < 0.5).unwrap_or(0);
    let start = idx(find(&xs, sa.0), find(&ys, sa.1));
    let goal = idx(find(&xs, sb.0), find(&ys, sb.1));
    let blocked: Vec<bool> = (0..nx * ny).map(|i| inside((xs[i % nx], ys[i / nx]))).collect();
    // Directions: 0 right, 1 down, 2 left, 3 up.
    let dir_of = |d: (f32, f32)| -> u8 {
        if d.0 > 0.5 {
            0
        } else if d.1 > 0.5 {
            1
        } else if d.0 < -0.5 {
            2
        } else {
            3
        }
    };
    let start_dir = dir_of(da);
    let goal_dir = dir_of((-db.0, -db.1));
    let mut best: HashMap<(usize, u8), f32> = HashMap::new();
    let mut prev: HashMap<(usize, u8), (usize, u8)> = HashMap::new();
    let mut heap = BinaryHeap::new();
    heap.push(St { cost: 0.0, node: start, dir: start_dir });
    best.insert((start, start_dir), 0.0);
    let bend = 40.0;
    let mut found: Option<(usize, u8)> = None;
    while let Some(St { cost, node, dir }) = heap.pop() {
        if node == goal {
            let extra = if dir == goal_dir { 0.0 } else { bend };
            let _ = extra;
            found = Some((node, dir));
            break;
        }
        if cost > *best.get(&(node, dir)).unwrap_or(&f32::MAX) {
            continue;
        }
        let (ix, iy) = (node % nx, node / nx);
        for nd in 0u8..4 {
            if (nd + 2) % 4 == dir {
                continue;
            }
            let (jx, jy) = match nd {
                0 if ix + 1 < nx => (ix + 1, iy),
                1 if iy + 1 < ny => (ix, iy + 1),
                2 if ix > 0 => (ix - 1, iy),
                3 if iy > 0 => (ix, iy - 1),
                _ => continue,
            };
            let nn = idx(jx, jy);
            if blocked[nn] && nn != goal {
                continue;
            }
            let mid = ((xs[ix] + xs[jx]) / 2.0, (ys[iy] + ys[jy]) / 2.0);
            if inside(mid) {
                continue;
            }
            let len = (xs[jx] - xs[ix]).abs() + (ys[jy] - ys[iy]).abs();
            let mut c = cost + len + if nd != dir { bend } else { 0.0 };
            if nn == goal && nd != goal_dir {
                c += bend;
            }
            if c < *best.get(&(nn, nd)).unwrap_or(&f32::MAX) {
                best.insert((nn, nd), c);
                prev.insert((nn, nd), (node, dir));
                heap.push(St { cost: c, node: nn, dir: nd });
            }
        }
    }
    let Some(mut cur) = found else { return simple_elbow(a, da, sa, sb, db, b) };
    let mut path = vec![(xs[cur.0 % nx], ys[cur.0 / nx])];
    while let Some(&p) = prev.get(&cur) {
        cur = p;
        path.push((xs[cur.0 % nx], ys[cur.0 / nx]));
    }
    path.reverse();
    let mut pts = vec![a];
    pts.extend(path);
    pts.push(b);
    simplify(pts)
}

fn simple_elbow(a: (f32, f32), da: (f32, f32), sa: (f32, f32), sb: (f32, f32), _db: (f32, f32), b: (f32, f32)) -> Vec<(f32, f32)> {
    let mid = if da.0.abs() > 0.5 { vec![((sa.0 + sb.0) / 2.0, sa.1), ((sa.0 + sb.0) / 2.0, sb.1)] } else { vec![(sa.0, (sa.1 + sb.1) / 2.0), (sb.0, (sa.1 + sb.1) / 2.0)] };
    let mut v = vec![a, sa];
    v.extend(mid);
    v.push(sb);
    v.push(b);
    simplify(v)
}

/// Drop repeated and collinear points.
pub fn simplify(pts: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = vec![];
    for p in pts {
        if let Some(l) = v.last() {
            if (l.0 - p.0).abs() < 0.01 && (l.1 - p.1).abs() < 0.01 {
                continue;
            }
        }
        v.push(p);
    }
    let mut out: Vec<(f32, f32)> = vec![];
    for p in v {
        while out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let cross = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
            let back = (b.0 - a.0) * (p.0 - b.0) + (b.1 - a.1) * (p.1 - b.1) < 0.0;
            if cross.abs() < 0.01 && !back {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}

/// Arrowhead outline at `tip`, pointing along `dir` (out of the line). Returns (segs, filled).
pub fn arrowhead(kind: Arrow, tip: (f32, f32), dir: (f32, f32), width: f32) -> Option<(Vec<Seg>, bool)> {
    let size = 7.0 + width * 2.5;
    let (ux, uy) = (-dir.0, -dir.1); // back along the line
    let (nx, ny) = (-uy, ux);
    let at = |a: f32, b: f32| (tip.0 + ux * a + nx * b, tip.1 + uy * a + ny * b);
    match kind {
        Arrow::None => None,
        Arrow::Triangle => {
            let (p1, p2) = (at(size * 1.4, size * 0.55), at(size * 1.4, -size * 0.55));
            Some((vec![Seg::M(tip.0, tip.1), Seg::L(p1.0, p1.1), Seg::L(p2.0, p2.1), Seg::Z], true))
        }
        Arrow::Open => {
            let (p1, p2) = (at(size * 1.4, size * 0.6), at(size * 1.4, -size * 0.6));
            Some((vec![Seg::M(p1.0, p1.1), Seg::L(tip.0, tip.1), Seg::L(p2.0, p2.1)], false))
        }
        Arrow::Circle => {
            let c = at(size * 0.5, 0.0);
            Some((geom::ellipse_segs(c.0, c.1, size * 0.5, size * 0.5), true))
        }
        Arrow::Diamond => {
            let (p1, p2, p3) = (at(size * 0.8, size * 0.5), at(size * 1.6, 0.0), at(size * 0.8, -size * 0.5));
            Some((vec![Seg::M(tip.0, tip.1), Seg::L(p1.0, p1.1), Seg::L(p2.0, p2.1), Seg::L(p3.0, p3.1), Seg::Z], true))
        }
        Arrow::Bar => {
            let (p1, p2) = (at(size * 0.3, size * 0.7), at(size * 0.3, -size * 0.7));
            Some((vec![Seg::M(p1.0, p1.1), Seg::L(p2.0, p2.1)], false))
        }
    }
}

/// How far the line should stop short of the tip so it doesn't poke through a filled head.
pub fn arrow_inset(kind: Arrow, width: f32) -> f32 {
    let size = 7.0 + width * 2.5;
    match kind {
        Arrow::Triangle => size * 1.2,
        Arrow::Circle => size,
        Arrow::Diamond => size * 1.5,
        _ => 0.0,
    }
}

/// Add hops where this route's horizontal segments cross vertical segments of earlier routes.
pub fn with_jumps(pts: &[(f32, f32)], earlier: &[Vec<(f32, f32)>]) -> Vec<Seg> {
    let r = 6.0;
    let mut out = vec![];
    if pts.is_empty() {
        return out;
    }
    out.push(Seg::M(pts[0].0, pts[0].1));
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let horizontal = (a.1 - b.1).abs() < 0.5 && (a.0 - b.0).abs() > 2.0 * r;
        if !horizontal {
            out.push(Seg::L(b.0, b.1));
            continue;
        }
        let y = a.1;
        let (lo, hi) = (a.0.min(b.0) + r, a.0.max(b.0) - r);
        let mut xs: Vec<f32> = vec![];
        for e in earlier {
            for v in e.windows(2) {
                let (c, d) = (v[0], v[1]);
                if (c.0 - d.0).abs() < 0.5 && c.0 > lo && c.0 < hi && y > c.1.min(d.1) + 1.0 && y < c.1.max(d.1) - 1.0 {
                    xs.push(c.0);
                }
            }
        }
        let fwd = b.0 > a.0;
        xs.sort_by(|p, q| if fwd { p.partial_cmp(q).unwrap() } else { q.partial_cmp(p).unwrap() });
        xs.dedup_by(|p, q| (*p - *q).abs() < 2.0 * r);
        let s = if fwd { 1.0 } else { -1.0 };
        for x in xs {
            let x0 = x - s * r;
            let x1 = x + s * r;
            out.push(Seg::L(x0, y));
            let k = r * 0.5523;
            out.push(Seg::C(x0, y - k, x - s * k, y - r, x, y - r));
            out.push(Seg::C(x + s * k, y - r, x1, y - k, x1, y));
        }
        out.push(Seg::L(b.0, b.1));
    }
    out
}
