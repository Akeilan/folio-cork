//! Double-precision vectors, planes and transforms, as the modeller works in
//! them: millimetres, right-handed, Z up.

use folio_math::Det;

/// Points closer than this are the same point.
pub const TOL: f64 = 1e-3;
pub const ANG_TOL: f64 = 1e-7;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V2 {
    pub x: f64,
    pub y: f64,
}

impl V2 {
    pub fn new(x: f64, y: f64) -> V2 {
        V2 { x, y }
    }
    pub fn dot(self, o: V2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: V2) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y).dsqrt()
    }
}

impl std::ops::Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, s: f64) -> V2 {
        V2::new(self.x * s, self.y * s)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct V3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const fn v3(x: f64, y: f64, z: f64) -> V3 {
    V3 { x, y, z }
}

impl V3 {
    pub fn new(x: f64, y: f64, z: f64) -> V3 {
        V3 { x, y, z }
    }

    pub fn dot(self, o: V3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: V3) -> V3 {
        V3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }

    pub fn len(self) -> f64 {
        self.len2().dsqrt()
    }

    pub fn len2(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn unit(self) -> V3 {
        let l = self.len();
        if l > 1e-300 {
            self * (1.0 / l)
        } else {
            V3::default()
        }
    }

    pub fn is_zero(self) -> bool {
        self.len2() < 1e-24
    }

    pub fn near(self, o: V3) -> bool {
        (self - o).len2() < TOL * TOL
    }

    pub fn at(self, i: usize) -> f64 {
        match i {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }

    /// Any unit vector at right angles to this one.
    pub fn any_perp(self) -> V3 {
        let a = if self.x.abs() < 0.9 { v3(1.0, 0.0, 0.0) } else { v3(0.0, 1.0, 0.0) };
        self.cross(a).unit()
    }
}

impl std::ops::Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        V3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl std::ops::Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        V3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl std::ops::Neg for V3 {
    type Output = V3;
    fn neg(self) -> V3 {
        V3::new(-self.x, -self.y, -self.z)
    }
}

impl std::ops::Mul<f64> for V3 {
    type Output = V3;
    fn mul(self, s: f64) -> V3 {
        V3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl std::ops::AddAssign for V3 {
    fn add_assign(&mut self, o: V3) {
        *self = *self + o;
    }
}

/// A flat surface: the points where `n · p = d`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Plane {
    pub n: V3,
    pub d: f64,
}

impl Plane {
    pub fn through(p: V3, n: V3) -> Plane {
        let n = n.unit();
        Plane { n, d: n.dot(p) }
    }

    /// The plane of three points, or none when they are in a line.
    pub fn of(a: V3, b: V3, c: V3) -> Option<Plane> {
        let n = (b - a).cross(c - a);
        if n.len2() < 1e-18 {
            return None;
        }
        Some(Plane::through(a, n))
    }

    pub fn distance(&self, p: V3) -> f64 {
        self.n.dot(p) - self.d
    }

    pub fn on(&self, p: V3) -> bool {
        self.distance(p).abs() <= TOL
    }

    /// Where a line crosses the plane, as a distance along it.
    pub fn hit(&self, from: V3, dir: V3) -> Option<f64> {
        let k = self.n.dot(dir);
        if k.abs() < 1e-12 {
            return None;
        }
        Some((self.d - self.n.dot(from)) / k)
    }

    /// Two axes in the plane, for working in 2D on it.
    pub fn axes(&self) -> (V3, V3) {
        let u = self.n.any_perp();
        (u, self.n.cross(u).unit())
    }
}

/// A 4x4 transform, kept as three rows (the last is always 0,0,0,1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub m: [[f64; 4]; 3],
}

impl Default for Xf {
    fn default() -> Self {
        Xf::identity()
    }
}

impl Xf {
    pub fn identity() -> Xf {
        Xf { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] }
    }

    pub fn translate(t: V3) -> Xf {
        let mut x = Xf::identity();
        x.m[0][3] = t.x;
        x.m[1][3] = t.y;
        x.m[2][3] = t.z;
        x
    }

    pub fn scale(s: V3) -> Xf {
        let mut x = Xf::identity();
        x.m[0][0] = s.x;
        x.m[1][1] = s.y;
        x.m[2][2] = s.z;
        x
    }

    /// Turn by `ang` radians about an axis through the origin.
    pub fn rotate(axis: V3, ang: f64) -> Xf {
        let a = axis.unit();
        let (s, c) = (ang.dsin(), ang.dcos());
        let t = 1.0 - c;
        Xf {
            m: [
                [t * a.x * a.x + c, t * a.x * a.y - s * a.z, t * a.x * a.z + s * a.y, 0.0],
                [t * a.x * a.y + s * a.z, t * a.y * a.y + c, t * a.y * a.z - s * a.x, 0.0],
                [t * a.x * a.z - s * a.y, t * a.y * a.z + s * a.x, t * a.z * a.z + c, 0.0],
            ],
        }
    }

    /// Turn about an axis through a point.
    pub fn rotate_about(point: V3, axis: V3, ang: f64) -> Xf {
        Xf::translate(point).then(&Xf::rotate(axis, ang)).then(&Xf::translate(-point))
    }

    /// `self` applied first, then `o`.
    pub fn then(&self, o: &Xf) -> Xf {
        let mut r = Xf::identity();
        for i in 0..3 {
            for j in 0..4 {
                let mut v = 0.0;
                for k in 0..3 {
                    v += o.m[i][k] * self.m[k][j];
                }
                if j == 3 {
                    v += o.m[i][3];
                }
                r.m[i][j] = v;
            }
        }
        r
    }

    pub fn point(&self, p: V3) -> V3 {
        V3::new(
            self.m[0][0] * p.x + self.m[0][1] * p.y + self.m[0][2] * p.z + self.m[0][3],
            self.m[1][0] * p.x + self.m[1][1] * p.y + self.m[1][2] * p.z + self.m[1][3],
            self.m[2][0] * p.x + self.m[2][1] * p.y + self.m[2][2] * p.z + self.m[2][3],
        )
    }

    /// A direction: the shift is left out.
    pub fn dir(&self, p: V3) -> V3 {
        V3::new(
            self.m[0][0] * p.x + self.m[0][1] * p.y + self.m[0][2] * p.z,
            self.m[1][0] * p.x + self.m[1][1] * p.y + self.m[1][2] * p.z,
            self.m[2][0] * p.x + self.m[2][1] * p.y + self.m[2][2] * p.z,
        )
    }

    pub fn invert(&self) -> Option<Xf> {
        let m = &self.m;
        let a = [[m[0][0], m[0][1], m[0][2]], [m[1][0], m[1][1], m[1][2]], [m[2][0], m[2][1], m[2][2]]];
        let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
            + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
        if det.abs() < 1e-18 {
            return None;
        }
        let id = 1.0 / det;
        let inv = [
            [(a[1][1] * a[2][2] - a[1][2] * a[2][1]) * id, (a[0][2] * a[2][1] - a[0][1] * a[2][2]) * id, (a[0][1] * a[1][2] - a[0][2] * a[1][1]) * id],
            [(a[1][2] * a[2][0] - a[1][0] * a[2][2]) * id, (a[0][0] * a[2][2] - a[0][2] * a[2][0]) * id, (a[0][2] * a[1][0] - a[0][0] * a[1][2]) * id],
            [(a[1][0] * a[2][1] - a[1][1] * a[2][0]) * id, (a[0][1] * a[2][0] - a[0][0] * a[2][1]) * id, (a[0][0] * a[1][1] - a[0][1] * a[1][0]) * id],
        ];
        let t = V3::new(m[0][3], m[1][3], m[2][3]);
        let mut out = Xf { m: [[inv[0][0], inv[0][1], inv[0][2], 0.0], [inv[1][0], inv[1][1], inv[1][2], 0.0], [inv[2][0], inv[2][1], inv[2][2], 0.0]] };
        let ti = out.dir(t);
        out.m[0][3] = -ti.x;
        out.m[1][3] = -ti.y;
        out.m[2][3] = -ti.z;
        Some(out)
    }
}

/// The shortest distance from a point to a segment, and where on it that is.
pub fn point_on_segment(p: V3, a: V3, b: V3) -> (f64, f64) {
    let ab = b - a;
    let l2 = ab.len2();
    if l2 < 1e-18 {
        return ((p - a).len(), 0.0);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    ((p - (a + ab * t)).len(), t)
}

/// Where two lines come closest: the distance along each.
pub fn line_line(p1: V3, d1: V3, p2: V3, d2: V3) -> Option<(f64, f64)> {
    let n = d1.cross(d2);
    if n.len2() < 1e-18 {
        return None;
    }
    let r = p2 - p1;
    let t1 = r.cross(d2).dot(n) / n.len2();
    let t2 = r.cross(d1).dot(n) / n.len2();
    Some((t1, t2))
}
