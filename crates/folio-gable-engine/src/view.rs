//! The camera: where the model is looked at from, and how a point in the model
//! lands on the window.

use crate::math::{Plane, Xf, V3};
use folio_math::Det;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// where the eye is, and what it looks at
    pub eye: V3,
    pub target: V3,
    pub up: V3,
    /// the angle across the view, in radians (perspective only)
    pub fov: f64,
    /// how wide the view is in model units, when the camera is parallel
    pub height: f64,
    pub perspective: bool,
    pub near: f64,
    pub far: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            eye: V3::new(4000.0, -6000.0, 3500.0),
            target: V3::new(0.0, 0.0, 500.0),
            up: V3::new(0.0, 0.0, 1.0),
            fov: 35f64.to_radians(),
            height: 6000.0,
            perspective: true,
            near: 1.0,
            far: 1.0e7,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> V3 {
        (self.target - self.eye).unit()
    }

    pub fn right(&self) -> V3 {
        self.forward().cross(self.up).unit()
    }

    pub fn true_up(&self) -> V3 {
        self.right().cross(self.forward()).unit()
    }

    pub fn distance(&self) -> f64 {
        (self.target - self.eye).len()
    }

    /// Turn the camera about the target: `dx` and `dy` in radians.
    pub fn orbit(&mut self, dx: f64, dy: f64) {
        let r = self.eye - self.target;
        let about_z = Xf::rotate(V3::new(0.0, 0.0, 1.0), -dx);
        let r = about_z.dir(r);
        let right = self.right();
        let tilt = Xf::rotate(right, -dy);
        let r2 = tilt.dir(r);
        // keep the camera off the poles
        let up = V3::new(0.0, 0.0, 1.0);
        let ang = r2.unit().dot(up).abs();
        let r = if ang > 0.999 { r } else { r2 };
        self.eye = self.target + r;
    }

    /// Slide the camera and its target across the view.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let d = self.right() * dx + self.true_up() * dy;
        self.eye += d;
        self.target += d;
    }

    /// Move the eye towards or away from the target.
    pub fn zoom(&mut self, by: f64) {
        if self.perspective {
            let d = (self.target - self.eye) * by.clamp(-0.9, 0.9);
            self.eye += d;
            if (self.target - self.eye).len() < 1.0 {
                self.eye = self.target - self.forward();
            }
        } else {
            self.height = (self.height * (1.0 - by)).clamp(1.0, 1.0e8);
        }
    }

    /// Put the whole box in view.
    pub fn look_at_box(&mut self, lo: V3, hi: V3) {
        let c = (lo + hi) * 0.5;
        let size = (hi - lo).len().max(1.0);
        let dir = self.forward();
        self.target = c;
        self.eye = c - dir * (size * 1.8);
        self.height = size * 1.4;
    }

    /// The view and projection, as one transform into clip space (-1..1).
    pub fn matrix(&self, w: f64, h: f64) -> Proj {
        let f = self.forward();
        let r = self.right();
        let u = self.true_up();
        let e = self.eye;
        let view = Xf {
            m: [
                [r.x, r.y, r.z, -r.dot(e)],
                [u.x, u.y, u.z, -u.dot(e)],
                [-f.x, -f.y, -f.z, f.dot(e)],
            ],
        };
        let aspect = (w / h.max(1.0)).max(0.01);
        Proj { view, aspect, cam: *self, w, h }
    }
}

/// A camera worked out for one window size.
#[derive(Clone, Copy, Debug)]
pub struct Proj {
    pub view: Xf,
    pub aspect: f64,
    pub cam: Camera,
    pub w: f64,
    pub h: f64,
}

/// A point of the model, on the window: x and y in pixels, and how far away.
#[derive(Clone, Copy, Debug, Default)]
pub struct Screen {
    pub x: f64,
    pub y: f64,
    /// distance from the eye along the view, in model units
    pub depth: f64,
    /// false when the point is behind the camera
    pub infront: bool,
}

impl Proj {
    /// Where a model point lands on the window.
    pub fn project(&self, p: V3) -> Screen {
        let v = self.view.point(p);
        let depth = -v.z;
        if self.cam.perspective {
            if depth <= self.cam.near {
                return Screen { x: 0.0, y: 0.0, depth, infront: false };
            }
            let t = (self.cam.fov * 0.5).dtan();
            let ndc_x = v.x / (depth * t * self.aspect);
            let ndc_y = v.y / (depth * t);
            Screen {
                x: (ndc_x * 0.5 + 0.5) * self.w,
                y: (0.5 - ndc_y * 0.5) * self.h,
                depth,
                infront: true,
            }
        } else {
            let half_h = self.cam.height * 0.5;
            let half_w = half_h * self.aspect;
            Screen {
                x: (v.x / half_w * 0.5 + 0.5) * self.w,
                y: (0.5 - v.y / half_h * 0.5) * self.h,
                depth,
                infront: depth > 0.0,
            }
        }
    }

    /// The ray through a point on the window: where it starts and which way it goes.
    pub fn ray(&self, x: f64, y: f64) -> (V3, V3) {
        let ndc_x = (x / self.w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (y / self.h) * 2.0;
        let r = self.cam.right();
        let u = self.cam.true_up();
        let f = self.cam.forward();
        if self.cam.perspective {
            let t = (self.cam.fov * 0.5).dtan();
            let dir = (f + r * (ndc_x * t * self.aspect) + u * (ndc_y * t)).unit();
            (self.cam.eye, dir)
        } else {
            let half_h = self.cam.height * 0.5;
            let half_w = half_h * self.aspect;
            let o = self.cam.eye + r * (ndc_x * half_w) + u * (ndc_y * half_h);
            (o, f)
        }
    }

    /// Where a click lands on a plane in the model.
    pub fn on_plane(&self, x: f64, y: f64, plane: &Plane) -> Option<V3> {
        let (o, d) = self.ray(x, y);
        let t = plane.hit(o, d)?;
        (t > 0.0 || !self.cam.perspective).then(|| o + d * t)
    }

    /// How many model units one pixel covers at a point.
    pub fn units_per_pixel(&self, at: V3) -> f64 {
        if self.cam.perspective {
            let depth = (-self.view.point(at).z).max(self.cam.near);
            2.0 * depth * (self.cam.fov * 0.5).dtan() / self.h.max(1.0)
        } else {
            self.cam.height / self.h.max(1.0)
        }
    }
}
