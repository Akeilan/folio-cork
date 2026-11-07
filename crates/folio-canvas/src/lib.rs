//! A drawing surface for Folio's document engines.
//!
//! Pages, equations, shapes and charts are described by the engines as paths,
//! fills and glyphs; this draws them, on any system, with the same pixels
//! everywhere (X3-12). Paths are kept in surface coordinates, so a transform
//! set after a point was added leaves that point where it was put.
//!
//! The one thing it does not do is lay text out: an engine that has already
//! measured its text asks for the glyphs it measured, by their number in the
//! font, so what is drawn is what was measured.

use folio_kit::gfx::sk;
use folio_math::Det;
use std::collections::HashMap;
use std::sync::Arc;

pub mod glyphs;
pub use glyphs::Glyphs;

/// How a fill or a stroke is coloured.
#[derive(Clone)]
pub enum Source {
    Solid(u32, f64),
    /// start, end, and colour stops from 0 to 1
    Linear((f64, f64), (f64, f64), Vec<(f32, u32, f64)>),
    /// centre, radius, and colour stops from the middle out
    Radial((f64, f64), f64, Vec<(f32, u32, f64)>),
    /// a picture, with the surface-space box it fills
    Image(Arc<sk::Pixmap>, (f64, f64, f64, f64)),
}

impl Source {
    fn shader(&self, alpha: f32) -> Option<sk::Shader<'_>> {
        match self {
            Source::Solid(..) => None,
            Source::Linear(a, b, stops) => sk::LinearGradient::new(
                pt(*a),
                pt(*b),
                stops.iter().map(|(o, c, al)| sk::GradientStop::new(*o, color(*c, *al * alpha as f64))).collect(),
                sk::SpreadMode::Pad,
                sk::Transform::identity(),
            ),
            Source::Radial(c, r, stops) => sk::RadialGradient::new(
                pt(*c),
                0.0,
                pt(*c),
                (*r as f32).max(0.01),
                stops.iter().map(|(o, c, al)| sk::GradientStop::new(*o, color(*c, *al * alpha as f64))).collect(),
                sk::SpreadMode::Pad,
                sk::Transform::identity(),
            ),
            Source::Image(pm, (x, y, w, h)) => {
                let sx = if pm.width() > 0 { *w as f32 / pm.width() as f32 } else { 1.0 };
                let sy = if pm.height() > 0 { *h as f32 / pm.height() as f32 } else { 1.0 };
                Some(sk::Pattern::new(
                    (**pm).as_ref(),
                    sk::SpreadMode::Pad,
                    sk::FilterQuality::Bilinear,
                    alpha,
                    sk::Transform::from_row(sx, 0.0, 0.0, sy, *x as f32, *y as f32),
                ))
            }
        }
    }
}

fn pt(p: (f64, f64)) -> sk::Point {
    sk::Point::from_xy(p.0 as f32, p.1 as f32)
}

fn color(c: u32, a: f64) -> sk::Color {
    sk::Color::from_rgba8(((c >> 16) & 255) as u8, ((c >> 8) & 255) as u8, (c & 255) as u8, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// A 2x3 transform, as cairo and PostScript write it.
#[derive(Clone, Copy, PartialEq)]
pub struct Matrix {
    pub xx: f64,
    pub yx: f64,
    pub xy: f64,
    pub yy: f64,
    pub x0: f64,
    pub y0: f64,
}

impl Matrix {
    pub fn identity() -> Matrix {
        Matrix { xx: 1.0, yx: 0.0, xy: 0.0, yy: 1.0, x0: 0.0, y0: 0.0 }
    }

    pub fn new(xx: f64, yx: f64, xy: f64, yy: f64, x0: f64, y0: f64) -> Matrix {
        Matrix { xx, yx, xy, yy, x0, y0 }
    }

    /// `self` then `m`.
    pub fn then(&self, m: &Matrix) -> Matrix {
        Matrix {
            xx: self.xx * m.xx + self.yx * m.xy,
            yx: self.xx * m.yx + self.yx * m.yy,
            xy: self.xy * m.xx + self.yy * m.xy,
            yy: self.xy * m.yx + self.yy * m.yy,
            x0: self.x0 * m.xx + self.y0 * m.xy + m.x0,
            y0: self.x0 * m.yx + self.y0 * m.yy + m.y0,
        }
    }

    pub fn xx(&self) -> f64 {
        self.xx
    }

    pub fn yx(&self) -> f64 {
        self.yx
    }

    pub fn xy(&self) -> f64 {
        self.xy
    }

    pub fn yy(&self) -> f64 {
        self.yy
    }

    pub fn x0(&self) -> f64 {
        self.x0
    }

    pub fn y0(&self) -> f64 {
        self.y0
    }

    /// `a` then `b`.
    pub fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
        a.then(b)
    }

    pub fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        self.apply(x, y)
    }

    pub fn transform_distance(&self, x: f64, y: f64) -> (f64, f64) {
        self.apply_dist(x, y)
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.xx * x + self.xy * y + self.x0, self.yx * x + self.yy * y + self.y0)
    }

    /// The same, for a distance: no translation.
    pub fn apply_dist(&self, x: f64, y: f64) -> (f64, f64) {
        (self.xx * x + self.xy * y, self.yx * x + self.yy * y)
    }

    /// How much the transform magnifies, as one number (the geometric mean of
    /// the two axes), for line widths and for picking a picture's size.
    pub fn scale(&self) -> f64 {
        let a = (self.xx * self.xx + self.yx * self.yx).dpow(0.5);
        let b = (self.xy * self.xy + self.yy * self.yy).dpow(0.5);
        ((a * b).max(0.0)).dpow(0.5)
    }

    pub fn invert(&self) -> Option<Matrix> {
        let det = self.xx * self.yy - self.yx * self.xy;
        if det.abs() < 1e-12 {
            return None;
        }
        let d = 1.0 / det;
        Some(Matrix {
            xx: self.yy * d,
            yx: -self.yx * d,
            xy: -self.xy * d,
            yy: self.xx * d,
            x0: (self.xy * self.y0 - self.yy * self.x0) * d,
            y0: (self.yx * self.x0 - self.xx * self.y0) * d,
        })
    }
}

#[derive(Clone)]
struct State {
    xf: Matrix,
    src: Source,
    line_width: f64,
    dash: Vec<f64>,
    dash_offset: f64,
    cap: sk::LineCap,
    join: sk::LineJoin,
    even_odd: bool,
    clip: Option<Arc<sk::Mask>>,
}

impl Default for State {
    fn default() -> State {
        State {
            xf: Matrix::identity(),
            src: Source::Solid(0, 1.0),
            line_width: 1.0,
            dash: Vec::new(),
            dash_offset: 0.0,
            cap: sk::LineCap::Butt,
            join: sk::LineJoin::Miter,
            even_odd: false,
            clip: None,
        }
    }
}

/// One path, as it is being built: points are in surface coordinates.
#[derive(Clone, Default)]
pub struct Path {
    pub cmds: Vec<Cmd>,
}

#[derive(Clone, Copy)]
pub enum Cmd {
    Move(f64, f64),
    Line(f64, f64),
    Curve(f64, f64, f64, f64, f64, f64),
    Close,
}

impl Path {
    fn build(&self) -> Option<sk::Path> {
        let mut b = sk::PathBuilder::new();
        let mut open = false;
        for c in &self.cmds {
            match *c {
                Cmd::Move(x, y) => {
                    b.move_to(x as f32, y as f32);
                    open = true;
                }
                Cmd::Line(x, y) => {
                    if !open {
                        b.move_to(x as f32, y as f32);
                        open = true;
                    } else {
                        b.line_to(x as f32, y as f32);
                    }
                }
                Cmd::Curve(x1, y1, x2, y2, x3, y3) => {
                    if !open {
                        b.move_to(x1 as f32, y1 as f32);
                        open = true;
                    }
                    b.cubic_to(x1 as f32, y1 as f32, x2 as f32, y2 as f32, x3 as f32, y3 as f32);
                }
                Cmd::Close => b.close(),
            }
        }
        b.finish()
    }

    /// The path as closed polygons: curves cut into straight pieces no further
    /// than `tol` from the curve. For fitting text round a shape.
    pub fn flatten(&self, tol: f64) -> Vec<Vec<(f64, f64)>> {
        let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut cur: Vec<(f64, f64)> = Vec::new();
        let mut at = (0.0, 0.0);
        for c in &self.cmds {
            match *c {
                Cmd::Move(x, y) => {
                    if cur.len() > 2 {
                        out.push(std::mem::take(&mut cur));
                    } else {
                        cur.clear();
                    }
                    cur.push((x, y));
                    at = (x, y);
                }
                Cmd::Line(x, y) => {
                    cur.push((x, y));
                    at = (x, y);
                }
                Cmd::Curve(x1, y1, x2, y2, x3, y3) => {
                    let d = ((x3 - at.0).abs() + (y3 - at.1).abs() + (x1 - at.0).abs() + (y1 - at.1).abs()).max(1.0);
                    let n = ((d / tol.max(0.01)).dpow(0.5).ceil() as usize).clamp(3, 48);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let u = 1.0 - t;
                        let x = u * u * u * at.0 + 3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t * x3;
                        let y = u * u * u * at.1 + 3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t * y3;
                        cur.push((x, y));
                    }
                    at = (x3, y3);
                }
                Cmd::Close => {
                    if cur.len() > 2 {
                        let first = cur[0];
                        cur.push(first);
                        out.push(std::mem::take(&mut cur));
                        at = first;
                    } else {
                        cur.clear();
                    }
                }
            }
        }
        if cur.len() > 2 {
            out.push(cur);
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    /// The last point, as cairo's current point.
    pub fn current(&self) -> Option<(f64, f64)> {
        let mut start = None;
        let mut cur = None;
        for c in &self.cmds {
            match *c {
                Cmd::Move(x, y) => {
                    start = Some((x, y));
                    cur = Some((x, y));
                }
                Cmd::Line(x, y) => cur = Some((x, y)),
                Cmd::Curve(.., x, y) => cur = Some((x, y)),
                Cmd::Close => cur = start,
            }
        }
        cur
    }
}

/// Somewhere a drawing can be written down as it is made: an exporter that
/// wants the paths and the glyphs themselves, not the pixels.
pub trait Sink {
    fn save(&mut self) {}
    fn restore(&mut self) {}
    fn clip(&mut self, _path: &Path, _even_odd: bool) {}
    fn fill(&mut self, path: &Path, src: &Source, even_odd: bool);
    #[allow(clippy::too_many_arguments)]
    fn stroke(&mut self, path: &Path, src: &Source, width: f64, dash: &[f64], offset: f64, cap: sk::LineCap,
              join: sk::LineJoin);
    /// Glyphs in surface coordinates: the font, the size and the places, with
    /// the transform that was in force (so the exporter can turn text too).
    fn glyphs(&mut self, font: &glyphs::FontRef, size: f64, xscale: f64, run: &[(u16, f64, f64)], xf: &Matrix,
              src: &Source);
    fn image(&mut self, pm: &sk::Pixmap, x: f64, y: f64, w: f64, h: f64, opacity: f64);
}

/// A surface to draw on.
pub struct Cx<'a> {
    pm: &'a mut sk::Pixmap,
    sink: Option<&'a mut dyn Sink>,
    st: State,
    stack: Vec<State>,
    path: Path,
    /// where a `move_to` would go if the next command needs one
    start: Option<(f64, f64)>,
    glyphs: &'a mut Glyphs,
    /// pictures already scaled, by (their key, the size wanted)
    scaled: HashMap<(usize, u32, u32), Arc<sk::Pixmap>>,
}

impl<'a> Cx<'a> {
    pub fn new(pm: &'a mut sk::Pixmap, glyphs: &'a mut Glyphs) -> Cx<'a> {
        Cx { pm, sink: None, st: State::default(), stack: Vec::new(), path: Path::default(), start: None, glyphs,
             scaled: HashMap::new() }
    }

    /// A surface that also writes down what it draws. The pixels are still
    /// drawn, so a one-pixel picture is enough when only the writing matters.
    pub fn recording(pm: &'a mut sk::Pixmap, glyphs: &'a mut Glyphs, sink: &'a mut dyn Sink) -> Cx<'a> {
        let mut cx = Cx::new(pm, glyphs);
        cx.sink = Some(sink);
        cx
    }

    pub fn width(&self) -> u32 {
        self.pm.width()
    }

    pub fn height(&self) -> u32 {
        self.pm.height()
    }

    pub fn save(&mut self) {
        self.stack.push(self.st.clone());
        if let Some(s) = self.sink.as_mut() {
            s.save();
        }
    }

    pub fn restore(&mut self) {
        if let Some(s) = self.stack.pop() {
            self.st = s;
        }
        if let Some(s) = self.sink.as_mut() {
            s.restore();
        }
    }

    // --- the transform ---

    pub fn matrix(&self) -> Matrix {
        self.st.xf
    }

    pub fn set_matrix(&mut self, m: Matrix) {
        self.st.xf = m;
    }

    pub fn translate(&mut self, x: f64, y: f64) {
        self.st.xf = Matrix::new(1.0, 0.0, 0.0, 1.0, x, y).then(&self.st.xf);
    }

    pub fn scale(&mut self, x: f64, y: f64) {
        self.st.xf = Matrix::new(x, 0.0, 0.0, y, 0.0, 0.0).then(&self.st.xf);
    }

    /// Turn, in radians, the way cairo does (positive is clockwise on screen).
    pub fn rotate(&mut self, a: f64) {
        let (c, s) = (a.dcos(), a.dsin());
        self.st.xf = Matrix::new(c, s, -s, c, 0.0, 0.0).then(&self.st.xf);
    }

    pub fn transform(&mut self, m: Matrix) {
        self.st.xf = m.then(&self.st.xf);
    }

    pub fn user_to_surface(&self, x: f64, y: f64) -> (f64, f64) {
        self.st.xf.apply(x, y)
    }

    pub fn surface_to_user(&self, x: f64, y: f64) -> (f64, f64) {
        self.st.xf.invert().map(|m| m.apply(x, y)).unwrap_or((x, y))
    }

    pub fn user_to_surface_dist(&self, x: f64, y: f64) -> (f64, f64) {
        self.st.xf.apply_dist(x, y)
    }

    // --- the path ---

    pub fn new_path(&mut self) {
        self.path = Path::default();
        self.start = None;
    }

    /// Start a new piece of the same path, with no point of its own yet
    /// (cairo's `new_sub_path`): the next `line_to` or `arc` begins it.
    pub fn new_sub_path(&mut self) {
        self.start = None;
    }

    pub fn move_to(&mut self, x: f64, y: f64) {
        let p = self.st.xf.apply(x, y);
        self.path.cmds.push(Cmd::Move(p.0, p.1));
        self.start = Some(p);
    }

    pub fn line_to(&mut self, x: f64, y: f64) {
        let p = self.st.xf.apply(x, y);
        if self.start.is_none() {
            self.path.cmds.push(Cmd::Move(p.0, p.1));
            self.start = Some(p);
        } else {
            self.path.cmds.push(Cmd::Line(p.0, p.1));
        }
    }

    pub fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        let a = self.st.xf.apply(x1, y1);
        let b = self.st.xf.apply(x2, y2);
        let c = self.st.xf.apply(x3, y3);
        if self.start.is_none() {
            self.path.cmds.push(Cmd::Move(a.0, a.1));
            self.start = Some(a);
        }
        self.path.cmds.push(Cmd::Curve(a.0, a.1, b.0, b.1, c.0, c.1));
    }

    pub fn close_path(&mut self) {
        if !self.path.cmds.is_empty() {
            self.path.cmds.push(Cmd::Close);
        }
    }

    pub fn rectangle(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.move_to(x, y);
        self.line_to(x + w, y);
        self.line_to(x + w, y + h);
        self.line_to(x, y + h);
        self.close_path();
        self.start = None;
    }

    /// An arc, clockwise from `a0` to `a1` (cairo's `arc`).
    pub fn arc(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64) {
        self.arc_in(cx, cy, r, a0, a1, false);
    }

    /// An arc the other way round (cairo's `arc_negative`).
    pub fn arc_negative(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64) {
        self.arc_in(cx, cy, r, a0, a1, true);
    }

    fn arc_in(&mut self, cx: f64, cy: f64, r: f64, a0: f64, a1: f64, back: bool) {
        let tau = std::f64::consts::TAU;
        let mut sweep = a1 - a0;
        if back {
            while sweep > 0.0 {
                sweep -= tau;
            }
            if sweep < -tau {
                sweep = -tau;
            }
        } else {
            while sweep < 0.0 {
                sweep += tau;
            }
            if sweep > tau {
                sweep = tau;
            }
        }
        // in quarter turns at most, so the curves stay close to the circle
        let steps = ((sweep.abs() / (tau / 4.0)).ceil() as usize).max(1);
        let step = sweep / steps as f64;
        let k = 4.0 / 3.0 * (step / 4.0).dtan();
        let (sx, sy) = (cx + r * a0.dcos(), cy + r * a0.dsin());
        if self.start.is_none() {
            self.move_to(sx, sy);
        } else {
            self.line_to(sx, sy);
        }
        let mut a = a0;
        for _ in 0..steps {
            let b = a + step;
            let (c0, s0) = (a.dcos(), a.dsin());
            let (c1, s1) = (b.dcos(), b.dsin());
            self.curve_to(
                cx + r * (c0 - k * s0),
                cy + r * (s0 + k * c0),
                cx + r * (c1 + k * s1),
                cy + r * (s1 - k * c1),
                cx + r * c1,
                cy + r * s1,
            );
            a = b;
        }
    }

    pub fn has_current_point(&self) -> bool {
        self.start.is_some()
    }

    pub fn current_point(&self) -> Option<(f64, f64)> {
        self.path.current().map(|p| self.surface_to_user(p.0, p.1))
    }

    /// The path as it stands, in surface coordinates.
    pub fn copy_path(&self) -> Path {
        self.path.clone()
    }

    pub fn append_path(&mut self, p: &Path) {
        self.path.cmds.extend_from_slice(&p.cmds);
        self.start = self.path.current();
    }

    // --- what to paint with ---

    pub fn set_source_rgb(&mut self, r: f64, g: f64, b: f64) {
        let c = ((r.clamp(0.0, 1.0) * 255.0).round() as u32) << 16
            | ((g.clamp(0.0, 1.0) * 255.0).round() as u32) << 8
            | (b.clamp(0.0, 1.0) * 255.0).round() as u32;
        self.st.src = Source::Solid(c, 1.0);
    }

    pub fn set_source_rgba(&mut self, r: f64, g: f64, b: f64, a: f64) {
        let c = ((r.clamp(0.0, 1.0) * 255.0).round() as u32) << 16
            | ((g.clamp(0.0, 1.0) * 255.0).round() as u32) << 8
            | (b.clamp(0.0, 1.0) * 255.0).round() as u32;
        self.st.src = Source::Solid(c, a);
    }

    /// A colour as 0xRRGGBB.
    pub fn set_rgb(&mut self, c: u32) {
        self.st.src = Source::Solid(c, 1.0);
    }

    pub fn set_rgba(&mut self, c: u32, a: f64) {
        self.st.src = Source::Solid(c, a);
    }

    pub fn set_source(&mut self, s: Source) {
        self.st.src = s;
    }

    pub fn source(&self) -> Source {
        self.st.src.clone()
    }

    /// A picture, filling the box given in user coordinates.
    pub fn set_source_pixmap(&mut self, pm: Arc<sk::Pixmap>, x: f64, y: f64, w: f64, h: f64) {
        let a = self.st.xf.apply(x, y);
        let b = self.st.xf.apply(x + w, y + h);
        self.st.src = Source::Image(pm, (a.0.min(b.0), a.1.min(b.1), (b.0 - a.0).abs(), (b.1 - a.1).abs()));
    }

    pub fn set_line_width(&mut self, w: f64) {
        self.st.line_width = w;
    }

    pub fn line_width(&self) -> f64 {
        self.st.line_width
    }

    pub fn set_dash(&mut self, dash: &[f64], offset: f64) {
        self.st.dash = dash.to_vec();
        self.st.dash_offset = offset;
    }

    pub fn set_line_cap(&mut self, cap: sk::LineCap) {
        self.st.cap = cap;
    }

    pub fn set_line_join(&mut self, join: sk::LineJoin) {
        self.st.join = join;
    }

    /// Fill crossing subpaths by the even-odd rule instead of winding.
    pub fn set_even_odd(&mut self, on: bool) {
        self.st.even_odd = on;
    }

    // --- putting it on the surface ---

}

fn paint_of(src: &Source, alpha: f32) -> sk::Paint<'_> {
    let mut p = sk::Paint::default();
    p.anti_alias = true;
    match src.shader(alpha) {
        Some(sh) => p.shader = sh,
        None => {
            if let Source::Solid(c, a) = src {
                p.set_color(color(*c, *a * alpha as f64));
            }
        }
    }
    p
}

impl<'a> Cx<'a> {

    pub fn fill(&mut self) {
        self.fill_preserve();
        self.new_path();
    }

    pub fn fill_preserve(&mut self) {
        if let Some(s) = self.sink.as_mut() {
            s.fill(&self.path, &self.st.src, self.st.even_odd);
        }
        let Some(path) = self.path.build() else { return };
        let rule = if self.st.even_odd { sk::FillRule::EvenOdd } else { sk::FillRule::Winding };
        let src = self.st.src.clone();
        let paint = paint_of(&src, 1.0);
        self.pm.fill_path(&path, &paint, rule, sk::Transform::identity(), self.st.clip.as_deref());
    }

    pub fn stroke(&mut self) {
        self.stroke_preserve();
        self.new_path();
    }

    pub fn stroke_preserve(&mut self) {
        if let Some(s) = self.sink.as_mut() {
            let k = self.st.xf.scale();
            s.stroke(&self.path, &self.st.src, self.st.line_width * k,
                     &self.st.dash.iter().map(|d| d * k).collect::<Vec<_>>(), self.st.dash_offset * k,
                     self.st.cap, self.st.join);
        }
        let Some(path) = self.path.build() else { return };
        let mut st = sk::Stroke::default();
        let k = self.st.xf.scale();
        st.width = ((self.st.line_width * k) as f32).max(0.05);
        st.line_cap = self.st.cap;
        st.line_join = self.st.join;
        if !self.st.dash.is_empty() {
            let v: Vec<f32> = self.st.dash.iter().map(|d| ((d * k) as f32).max(0.01)).collect();
            let v = if v.len() % 2 == 1 { [v.clone(), v].concat() } else { v };
            st.dash = sk::StrokeDash::new(v, (self.st.dash_offset * k) as f32);
        }
        let src = self.st.src.clone();
        let paint = paint_of(&src, 1.0);
        self.pm.stroke_path(&path, &paint, &st, sk::Transform::identity(), self.st.clip.as_deref());
    }

    /// Fill everything inside the clip with the current source.
    pub fn paint(&mut self) {
        self.paint_with_alpha(1.0);
    }

    pub fn paint_with_alpha(&mut self, alpha: f64) {
        if self.sink.is_some() {
            // the whole of what the clip allows, in surface coordinates
            let (w, h) = (self.pm.width() as f64, self.pm.height() as f64);
            let whole = Path {
                cmds: vec![Cmd::Move(0.0, 0.0), Cmd::Line(w, 0.0), Cmd::Line(w, h), Cmd::Line(0.0, h), Cmd::Close],
            };
            let src = self.st.src.clone();
            if let Some(s) = self.sink.as_mut() {
                s.fill(&whole, &src, false);
            }
        }
        let (w, h) = (self.pm.width() as f32, self.pm.height() as f32);
        let Some(r) = sk::Rect::from_xywh(0.0, 0.0, w, h) else { return };
        let src = self.st.src.clone();
        let paint = paint_of(&src, alpha as f32);
        self.pm.fill_rect(r, &paint, sk::Transform::identity(), self.st.clip.as_deref());
    }

    /// Narrow what can be drawn to the path (and keep the path).
    pub fn clip_preserve(&mut self) {
        if let Some(s) = self.sink.as_mut() {
            s.clip(&self.path, self.st.even_odd);
        }
        let Some(path) = self.path.build() else { return };
        let rule = if self.st.even_odd { sk::FillRule::EvenOdd } else { sk::FillRule::Winding };
        let mut m = match &self.st.clip {
            Some(c) => (**c).clone(),
            None => {
                let Some(mut m) = sk::Mask::new(self.pm.width(), self.pm.height()) else { return };
                m.fill_path(&path, rule, true, sk::Transform::identity());
                self.st.clip = Some(Arc::new(m));
                return;
            }
        };
        m.intersect_path(&path, rule, true, sk::Transform::identity());
        self.st.clip = Some(Arc::new(m));
    }

    pub fn clip(&mut self) {
        self.clip_preserve();
        self.new_path();
    }

    pub fn reset_clip(&mut self) {
        self.st.clip = None;
    }

    /// Paint the current source through a grey picture used as a mask
    /// (shadows and blurs are made this way).
    pub fn mask_alpha(&mut self, mask: &sk::Mask) {
        let joined = match &self.st.clip {
            Some(c) => {
                let mut m = (**c).clone();
                let a = m.data_mut();
                for (i, v) in mask.data().iter().enumerate() {
                    if i < a.len() {
                        a[i] = ((a[i] as u16 * *v as u16) / 255) as u8;
                    }
                }
                m
            }
            None => mask.clone(),
        };
        let (w, h) = (self.pm.width() as f32, self.pm.height() as f32);
        let Some(r) = sk::Rect::from_xywh(0.0, 0.0, w, h) else { return };
        let src = self.st.src.clone();
        let paint = paint_of(&src, 1.0);
        self.pm.fill_rect(r, &paint, sk::Transform::identity(), Some(&joined));
    }

    /// Draw a picture into the box given, in user coordinates.
    pub fn draw_pixmap(&mut self, img: &sk::Pixmap, x: f64, y: f64, w: f64, h: f64, opacity: f64) {
        if img.width() == 0 || img.height() == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let a = self.st.xf.apply(x, y);
        let b = self.st.xf.apply(x + w, y + h);
        let (x0, y0) = (a.0.min(b.0), a.1.min(b.1));
        let (dw, dh) = ((b.0 - a.0).abs(), (b.1 - a.1).abs());
        if let Some(s) = self.sink.as_mut() {
            s.image(img, x0, y0, dw, dh, opacity);
        }
        let sx = dw as f32 / img.width() as f32;
        let sy = dh as f32 / img.height() as f32;
        let mut paint = sk::PixmapPaint::default();
        paint.opacity = opacity as f32;
        paint.quality = if sx < 1.0 || sy < 1.0 { sk::FilterQuality::Bicubic } else { sk::FilterQuality::Bilinear };
        self.pm.draw_pixmap(
            0,
            0,
            img.as_ref(),
            &paint,
            sk::Transform::from_row(sx, 0.0, 0.0, sy, x0 as f32, y0 as f32),
            self.st.clip.as_deref(),
        );
    }

    /// Glyphs the engine has already measured: `gid` with its place in user
    /// coordinates, drawn from the same font file that was measured.
    pub fn show_glyphs(&mut self, font: &glyphs::FontRef, size: f64, run: &[(u16, f64, f64)]) {
        self.show_glyphs_x(font, size, 1.0, run);
    }

    /// The same, with the glyphs squeezed or stretched sideways about their own
    /// origin (character scaling).
    pub fn show_glyphs_x(&mut self, font: &glyphs::FontRef, size: f64, xscale: f64, run: &[(u16, f64, f64)]) {
        if self.sink.is_some() {
            let (xf, src) = (self.st.xf, self.st.src.clone());
            if let Some(s) = self.sink.as_mut() {
                s.glyphs(font, size, xscale, run, &xf, &src);
            }
        }
        let mut cmds: Vec<Cmd> = Vec::new();
        for (gid, x, y) in run {
            let Some(p) = self.glyphs.outline(font, *gid) else { continue };
            let k = size / font.upem;
            let kx = k * xscale;
            for c in &p.cmds {
                cmds.push(match *c {
                    Cmd::Move(px, py) => {
                        let q = self.st.xf.apply(x + px * kx, y - py * k);
                        Cmd::Move(q.0, q.1)
                    }
                    Cmd::Line(px, py) => {
                        let q = self.st.xf.apply(x + px * kx, y - py * k);
                        Cmd::Line(q.0, q.1)
                    }
                    Cmd::Curve(ax, ay, bx, by, cx2, cy2) => {
                        let a = self.st.xf.apply(x + ax * kx, y - ay * k);
                        let b = self.st.xf.apply(x + bx * kx, y - by * k);
                        let c = self.st.xf.apply(x + cx2 * kx, y - cy2 * k);
                        Cmd::Curve(a.0, a.1, b.0, b.1, c.0, c.1)
                    }
                    Cmd::Close => Cmd::Close,
                });
            }
        }
        if cmds.is_empty() {
            return;
        }
        let keep = std::mem::replace(&mut self.path, Path { cmds });
        let keep_rule = self.st.even_odd;
        self.st.even_odd = false;
        self.fill_preserve();
        self.st.even_odd = keep_rule;
        self.path = keep;
    }

    /// The same glyphs as a path, for an outline or a shadow.
    pub fn glyph_path(&mut self, font: &glyphs::FontRef, size: f64, run: &[(u16, f64, f64)]) {
        self.glyph_path_x(font, size, 1.0, run)
    }

    pub fn glyph_path_x(&mut self, font: &glyphs::FontRef, size: f64, xscale: f64, run: &[(u16, f64, f64)]) {
        for (gid, x, y) in run {
            let Some(p) = self.glyphs.outline(font, *gid) else { continue };
            let k = size / font.upem;
            let kx = k * xscale;
            let mut first = true;
            for c in &p.cmds {
                match *c {
                    Cmd::Move(px, py) => {
                        self.move_to(x + px * kx, y - py * k);
                        first = false;
                    }
                    Cmd::Line(px, py) => {
                        if first {
                            self.move_to(x + px * kx, y - py * k);
                            first = false;
                        } else {
                            self.line_to(x + px * kx, y - py * k)
                        }
                    }
                    Cmd::Curve(ax, ay, bx, by, cx2, cy2) => {
                        self.curve_to(x + ax * kx, y - ay * k, x + bx * kx, y - by * k, x + cx2 * kx, y - cy2 * k)
                    }
                    Cmd::Close => self.close_path(),
                }
            }
        }
    }

    /// Fill a ready-made path, offset by (dx, dy) in user coordinates: text
    /// laid out by the toolkit comes this way.
    /// Add a ready-made path (a glyph run, say) to the path being built, in user space.
    pub fn append_sk_path(&mut self, p: &sk::Path, dx: f64, dy: f64) {
        let mut last = (0.0, 0.0);
        for seg in p.segments() {
            match seg {
                sk::PathSegment::MoveTo(a) => {
                    last = (a.x as f64 + dx, a.y as f64 + dy);
                    self.move_to(last.0, last.1);
                }
                sk::PathSegment::LineTo(a) => {
                    last = (a.x as f64 + dx, a.y as f64 + dy);
                    self.line_to(last.0, last.1);
                }
                sk::PathSegment::QuadTo(a, b) => {
                    let (cx, cy) = (a.x as f64 + dx, a.y as f64 + dy);
                    let (px, py) = (b.x as f64 + dx, b.y as f64 + dy);
                    self.curve_to(last.0 + 2.0 / 3.0 * (cx - last.0), last.1 + 2.0 / 3.0 * (cy - last.1),
                                  px + 2.0 / 3.0 * (cx - px), py + 2.0 / 3.0 * (cy - py), px, py);
                    last = (px, py);
                }
                sk::PathSegment::CubicTo(a, b, c) => {
                    let p3 = (c.x as f64 + dx, c.y as f64 + dy);
                    self.curve_to(a.x as f64 + dx, a.y as f64 + dy, b.x as f64 + dx, b.y as f64 + dy, p3.0, p3.1);
                    last = p3;
                }
                sk::PathSegment::Close => self.close_path(),
            }
        }
    }

    pub fn fill_sk_path(&mut self, p: &sk::Path, dx: f64, dy: f64) {
        let m = Matrix::new(1.0, 0.0, 0.0, 1.0, dx, dy).then(&self.st.xf);
        let t = sk::Transform::from_row(m.xx as f32, m.yx as f32, m.xy as f32, m.yy as f32, m.x0 as f32, m.y0 as f32);
        let Some(tp) = p.clone().transform(t) else { return };
        if self.sink.is_some() {
            // the exporter wants the shape itself, not the pixels
            let mut cmds = Vec::new();
            for seg in tp.segments() {
                match seg {
                    sk::PathSegment::MoveTo(p) => cmds.push(Cmd::Move(p.x as f64, p.y as f64)),
                    sk::PathSegment::LineTo(p) => cmds.push(Cmd::Line(p.x as f64, p.y as f64)),
                    sk::PathSegment::QuadTo(a, b) => {
                        let from = match cmds.last() {
                            Some(Cmd::Move(x, y)) | Some(Cmd::Line(x, y)) => (*x, *y),
                            Some(Cmd::Curve(.., x, y)) => (*x, *y),
                            _ => (a.x as f64, a.y as f64),
                        };
                        let (cx, cy) = (a.x as f64, a.y as f64);
                        let (px, py) = (b.x as f64, b.y as f64);
                        cmds.push(Cmd::Curve(from.0 + 2.0 / 3.0 * (cx - from.0), from.1 + 2.0 / 3.0 * (cy - from.1),
                                             px + 2.0 / 3.0 * (cx - px), py + 2.0 / 3.0 * (cy - py), px, py));
                    }
                    sk::PathSegment::CubicTo(a, b, c) => cmds.push(Cmd::Curve(a.x as f64, a.y as f64, b.x as f64,
                                                                             b.y as f64, c.x as f64, c.y as f64)),
                    sk::PathSegment::Close => cmds.push(Cmd::Close),
                }
            }
            let path = Path { cmds };
            let src = self.st.src.clone();
            if let Some(s) = self.sink.as_mut() {
                s.fill(&path, &src, false);
            }
        }
        let src = self.st.src.clone();
        let paint = paint_of(&src, 1.0);
        self.pm.fill_path(&tp, &paint, sk::FillRule::Winding, sk::Transform::identity(), self.st.clip.as_deref());
    }

    /// A picture scaled once and kept, for a fill that repeats it.
    pub fn scaled(&mut self, key: usize, img: &sk::Pixmap, w: u32, h: u32) -> Arc<sk::Pixmap> {
        if let Some(p) = self.scaled.get(&(key, w, h)) {
            return p.clone();
        }
        let mut out = sk::Pixmap::new(w.max(1), h.max(1)).unwrap_or_else(|| sk::Pixmap::new(1, 1).unwrap());
        let sx = w.max(1) as f32 / img.width().max(1) as f32;
        let sy = h.max(1) as f32 / img.height().max(1) as f32;
        let mut paint = sk::PixmapPaint::default();
        paint.quality = sk::FilterQuality::Bicubic;
        out.draw_pixmap(0, 0, img.as_ref(), &paint, sk::Transform::from_scale(sx, sy), None);
        let out = Arc::new(out);
        self.scaled.insert((key, w, h), out.clone());
        out
    }
}
