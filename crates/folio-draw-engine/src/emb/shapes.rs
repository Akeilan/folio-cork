//! Shape operations on embroidery outlines: merging shapes that touch, breaking a shape into its
//! separate parts, and cutting one along a line.
//!
//! Each works on a raster mask at 20 steps per millimetre and traces the result back to outlines,
//! so a hole stays a hole and the result is always a well-formed set of rings.

use super::geom::{self, Ring};
use super::raster::Mask;
use super::trace::trace_mask;

/// Steps per millimetre the operations work at.
const S: f32 = 20.0;
/// Parts smaller than this (mm²) are specks, not shapes.
const MIN_AREA: f32 = 0.05;

fn mask_for(every: &[Ring], pad: f32) -> Option<Mask> {
    if every.is_empty() {
        return None;
    }
    let b = geom::bbox(every);
    Some(Mask::new(b.0 - pad, b.1 - pad, b.2 + pad, b.3 + pad, S))
}

fn traced(m: &Mask) -> Vec<Vec<Ring>> {
    trace_mask(m, 0.05).into_iter().filter(|r| geom::rings_area(r) > MIN_AREA).collect()
}

/// Several shapes made one: those that touch merge, those that stay apart come back as separate
/// shapes. Empty when there is nothing to work on.
pub fn union(shapes: &[Vec<Ring>]) -> Vec<Vec<Ring>> {
    let every: Vec<Ring> = shapes.iter().flatten().cloned().collect();
    let Some(mut m) = mask_for(&every, 1.0) else { return vec![] };
    for s in shapes {
        let mut one = m.like();
        one.fill(s, true);
        m = m.or(&one);
    }
    traced(&m)
}

/// A shape's separate parts. One part back means it was all in one piece.
pub fn parts(rings: &[Ring]) -> Vec<Vec<Ring>> {
    let Some(mut m) = mask_for(rings, 1.0) else { return vec![] };
    m.fill(rings, true);
    traced(&m)
}

/// A shape cut along a line: the parts either side. Fewer than two parts means the line didn't
/// cross it from one side to the other.
pub fn split(rings: &[Ring], line: &[(f32, f32)]) -> Vec<Vec<Ring>> {
    let Some(mut m) = mask_for(rings, 1.0) else { return vec![] };
    m.fill(rings, true);
    let mut cut = m.like();
    for w in line.windows(2) {
        let (a, c) = (w[0], w[1]);
        let n = ((geom::dist(a, c) * 2.0 * S).ceil() as usize).max(1);
        for k in 0..=n {
            let p = geom::lerp(a, c, k as f32 / n as f32);
            let ci = ((p.0 - cut.x0) * cut.s) as i64;
            let cj = ((p.1 - cut.y0) * cut.s) as i64;
            // a line one step wide would leak: three steps makes a clean cut
            for dj in -1..=1i64 {
                for di in -1..=1i64 {
                    let (i, j) = (ci + di, cj + dj);
                    if i >= 0 && j >= 0 && (i as usize) < cut.w && (j as usize) < cut.h {
                        cut.v[j as usize * cut.w + i as usize] = true;
                    }
                }
            }
        }
    }
    traced(&m.minus(&cut))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f32, y: f32, w: f32) -> Ring {
        vec![(x, y), (x + w, y), (x + w, y + w), (x, y + w)]
    }

    #[test]
    fn two_squares_that_touch_become_one() {
        let a = vec![square(0.0, 0.0, 10.0)];
        let b = vec![square(8.0, 0.0, 10.0)];
        let out = union(&[a, b]);
        assert_eq!(out.len(), 1, "{} shapes", out.len());
        let bb = geom::bbox(&out[0]);
        assert!(bb.2 - bb.0 > 17.0, "the merged shape is {:.1} mm wide", bb.2 - bb.0);
    }

    #[test]
    fn two_squares_apart_stay_two_shapes() {
        let a = vec![square(0.0, 0.0, 10.0)];
        let b = vec![square(30.0, 0.0, 10.0)];
        assert_eq!(union(&[a, b]).len(), 2);
    }

    #[test]
    fn a_shape_in_two_pieces_comes_apart() {
        // one object whose rings are two separate squares
        let rings = vec![square(0.0, 0.0, 10.0), square(30.0, 0.0, 10.0)];
        let out = parts(&rings);
        assert_eq!(out.len(), 2, "{} parts", out.len());
        assert!(parts(&[square(0.0, 0.0, 10.0)]).len() == 1, "one piece stays one");
    }

    #[test]
    fn a_line_across_a_square_cuts_it_in_two() {
        let rings = vec![square(0.0, 0.0, 20.0)];
        let out = split(&rings, &[(10.0, -5.0), (10.0, 25.0)]);
        assert_eq!(out.len(), 2, "{} parts", out.len());
        let (l, r) = (geom::bbox(&out[0]), geom::bbox(&out[1]));
        assert!(l.2 <= 10.5 && r.0 >= 9.5 || r.2 <= 10.5 && l.0 >= 9.5, "the halves overlap: {l:?} {r:?}");
    }

    #[test]
    fn a_line_that_stops_short_cuts_nothing() {
        let rings = vec![square(0.0, 0.0, 20.0)];
        assert_eq!(split(&rings, &[(10.0, -5.0), (10.0, 8.0)]).len(), 1);
    }

    #[test]
    fn a_hole_survives_the_operations() {
        // a ring: a square with a square hole
        let mut outer = square(0.0, 0.0, 30.0);
        let mut hole = square(10.0, 10.0, 10.0);
        hole.reverse();
        let rings = vec![std::mem::take(&mut outer), hole];
        let out = parts(&rings);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 2, "the hole went: {} rings", out[0].len());
    }
}
