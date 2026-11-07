//! The Folio Gable window on the portable base (G3-06): the toolbar, the
//! viewport the model is drawn in, the measurements box and the status bar.

use folio_gable_engine::draw::{render, Style};
use folio_gable_engine::geom;
use folio_gable_engine::math::{v3, Plane, V3, Xf};
use folio_gable_engine::model::{Kind, Model, Ref};
use folio_gable_engine::solids;
use folio_gable_engine::store;
use folio_gable_engine::view::{Camera, Proj};
use folio_kit::chrome::{TitleAction, TitleBar, TITLE_H};
use folio_kit::files::{FileAction, FilePane};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::platform::{Button, Cursor, Event, Host, Key, Mods};
use folio_kit::text::{Align, Text};
use folio_kit::theme::Theme;
use folio_kit::ui::{Ctx, Dialog, DialogResult, Menu, MenuItem, MenuResult, TextField};
use std::path::PathBuf;
use std::time::Duration;

use crate::settings::Settings;
use crate::{icons, uitest::UiTest};

/// Gable's own brown, on the title bar and the highlights.
pub const BROWN: Color = Color::hex(0x8C6A3F);
const TOOLBAR_W: f32 = 44.0;
const STATUS_H: f32 = 26.0;

/// What the pointer does next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Line,
    Rect,
    Circle,
    PushPull,
    Move,
    Erase,
    Paint,
    Tape,
    Orbit,
    Pan,
    Zoom,
}

impl Tool {
    pub fn all() -> [(Tool, &'static str, &'static str, &'static str); 12] {
        [
            (Tool::Select, "select", "select", "Space"),
            (Tool::Line, "line", "line", "L"),
            (Tool::Rect, "rect", "rect", "R"),
            (Tool::Circle, "circle", "circle", "C"),
            (Tool::PushPull, "pushpull", "pushpull", "P"),
            (Tool::Move, "move", "move", "M"),
            (Tool::Erase, "eraser", "erase", "E"),
            (Tool::Paint, "paint", "paint", "B"),
            (Tool::Tape, "tape", "tape", "T"),
            (Tool::Orbit, "orbit", "orbit", "O"),
            (Tool::Pan, "pan", "pan", "H"),
            (Tool::Zoom, "zoom", "zoom", "Z"),
        ]
    }

    pub fn title(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Line => "Line",
            Tool::Rect => "Rectangle",
            Tool::Circle => "Circle",
            Tool::PushPull => "Push/Pull",
            Tool::Move => "Move",
            Tool::Erase => "Eraser",
            Tool::Paint => "Paint",
            Tool::Tape => "Tape Measure",
            Tool::Orbit => "Orbit",
            Tool::Pan => "Pan",
            Tool::Zoom => "Zoom",
        }
    }

    /// What the measurements box is for, with this tool in hand.
    pub fn measure_label(self) -> &'static str {
        match self {
            Tool::Line => "Length",
            Tool::Rect => "Sides",
            Tool::Circle => "Radius",
            Tool::PushPull => "Distance",
            Tool::Move => "Distance",
            Tool::Tape => "Length",
            _ => "",
        }
    }
}

pub struct Res {
    pub text: Text,
    pub icons: Icons,
}

/// What the tool in hand is part way through.
#[derive(Clone, Debug, Default)]
pub struct Pending {
    /// where the drag or the click run started, in the model
    pub from: Option<V3>,
    /// the face being pushed or pulled
    pub face: Option<usize>,
    /// how far it has gone so far
    pub dist: f64,
    /// what is under the pointer now
    pub at: Option<V3>,
}

pub struct GableApp {
    pub model: Model,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub cam: Camera,
    pub style: Style,
    pub tool: Tool,
    pub sel: Vec<Ref>,
    pub pending: Pending,
    /// the last thing the pointer was over, for the status line
    pub hover: Option<(Ref, V3)>,
    pub measure: TextField,
    pub undo: Vec<Model>,
    pub redo: Vec<Model>,
    pub settings: Settings,
    pub res: Option<Box<Res>>,
    pub theme: Theme,
    pub title: TitleBar,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub files: Option<(String, FilePane)>,
    pub flash: Option<(String, Duration)>,
    pub now: Duration,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub mods: Mods,
    pub mouse: (f32, f32),
    pub down_at: Option<(f32, f32)>,
    pub dragging: bool,
    pub view_rect: Rect,
    pub tool_hits: Vec<(Tool, Rect)>,
    pub exit: bool,
    pub exit_code: i32,
    pub test: Option<UiTest>,
    pm: Option<sk::Pixmap>,
}

impl GableApp {
    pub fn new(model: Model, path: Option<PathBuf>) -> GableApp {
        let mut ic = Icons::new();
        icons::install(&mut ic);
        let settings = Settings::load();
        let mut cam = Camera { perspective: settings.perspective, ..Default::default() };
        if let Some((lo, hi)) = model.bounds() {
            cam.look_at_box(lo, hi);
        }
        GableApp {
            model,
            path,
            dirty: false,
            cam,
            style: Style { faces: settings.faces, axes: settings.axes, ..Default::default() },
            tool: Tool::Select,
            sel: Vec::new(),
            pending: Pending::default(),
            hover: None,
            measure: TextField::new(""),
            undo: Vec::new(),
            redo: Vec::new(),
            settings,
            res: Some(Box::new(Res { text: Text::bundled(), icons: ic })),
            theme: Theme::new(false, BROWN, BROWN),
            title: TitleBar::new("gable", vec![("new", "new", "New (Ctrl+N)"), ("open", "open", "Open (Ctrl+O)"), ("save", "save", "Save (Ctrl+S)"), ("undo", "undo", "Undo (Ctrl+Z)"), ("redo", "redo", "Redo (Ctrl+Y)")]),
            menu: None,
            dialog: None,
            files: None,
            flash: None,
            now: Duration::ZERO,
            w: 1280.0,
            h: 820.0,
            s: 1.0,
            mods: Mods::NONE,
            mouse: (0.0, 0.0),
            down_at: None,
            dragging: false,
            view_rect: Rect::default(),
            tool_hits: Vec::new(),
            exit: false,
            exit_code: 0,
            test: None,
            pm: None,
        }
    }

    pub fn flash(&mut self, msg: &str) {
        self.flash = Some((msg.to_string(), self.now + Duration::from_secs(5)));
    }

    pub fn say(&mut self, title: &str, text: &str) {
        self.dialog = Some(Dialog::new("say", title).note(text).button("ok", "OK"));
    }

    pub fn doc_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled.gable".into())
    }

    pub fn window_title(&self) -> String {
        format!("{}{} - Folio Gable", if self.dirty { "*" } else { "" }, self.doc_name())
    }

    pub fn proj(&self) -> Proj {
        self.cam.matrix((self.view_rect.w / self.s) as f64, (self.view_rect.h / self.s) as f64)
    }

    /// A window point in the viewport's own coordinates (model units per pixel).
    pub fn to_view(&self, x: f32, y: f32) -> (f64, f64) {
        (((x - self.view_rect.x) / self.s) as f64, ((y - self.view_rect.y) / self.s) as f64)
    }

    /// Remember the model so the change can be undone.
    pub fn mark(&mut self) {
        self.undo.push(self.model.clone());
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// How a length reads, in the units the drawing uses.
    pub fn length_text(&self, mm: f64) -> String {
        match self.settings.units.as_str() {
            "cm" => format!("{:.2} cm", mm / 10.0),
            "m" => format!("{:.3} m", mm / 1000.0),
            "in" => format!("{:.2}\"", mm / 25.4),
            "ft" => format!("{:.3}'", mm / 304.8),
            _ => format!("{:.1} mm", mm),
        }
    }

    /// A length typed into the measurements box, in millimetres.
    pub fn parse_length(&self, text: &str) -> Option<f64> {
        let t = text.trim().to_lowercase();
        if t.is_empty() {
            return None;
        }
        let (num, unit): (String, String) = t.chars().partition(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+');
        let v: f64 = num.trim().parse().ok()?;
        let u = unit.trim();
        Some(match u {
            "cm" => v * 10.0,
            "m" => v * 1000.0,
            "in" | "\"" => v * 25.4,
            "ft" | "'" => v * 304.8,
            "mm" | "" => {
                if u.is_empty() {
                    // no unit given: the drawing's own
                    match self.settings.units.as_str() {
                        "cm" => v * 10.0,
                        "m" => v * 1000.0,
                        "in" => v * 25.4,
                        "ft" => v * 304.8,
                        _ => v,
                    }
                } else {
                    v
                }
            }
            _ => v,
        })
    }
}

impl GableApp {
    // ---------------------------------------------------------------- painting

    pub fn paint(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        self.w = pm.width() as f32;
        self.h = pm.height() as f32;
        let t = self.theme.clone();
        let s = self.s;
        pm.fill(t.window.to_sk());
        let top = TITLE_H * s;
        self.view_rect = Rect::new(TOOLBAR_W * s, top, self.w - TOOLBAR_W * s, self.h - top - STATUS_H * s);
        self.paint_view(pm);
        self.paint_toolbar(pm, r);
        self.paint_status(pm, r);
        {
            let title = self.window_title();
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            let can_undo = !self.undo.is_empty();
            let can_redo = !self.redo.is_empty();
            self.title.paint(&mut cx, Rect::new(0.0, 0.0, self.w, TITLE_H * s), &title, false, &|id| match id {
                "undo" => can_undo,
                "redo" => can_redo,
                _ => true,
            });
        }
        if let Some((_, f)) = self.files.as_mut() {
            let box_r = Rect::new(self.w * 0.1, self.h * 0.1, self.w * 0.8, self.h * 0.8);
            gfx::fill_rect(pm, box_r, t.panel);
            gfx::stroke_round(pm, box_r, 0.0, t.border, 1.0);
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            f.paint(&mut cx, box_r.inset(12.0 * s));
        }
        if let Some(m) = self.menu.as_mut() {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            m.paint(&mut cx, Rect::new(0.0, 0.0, self.w, self.h));
        }
        if let Some(d) = self.dialog.as_mut() {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            d.paint(&mut cx, Rect::new(0.0, 0.0, self.w, self.h));
        }
    }

    /// The model itself, drawn by the engine into the viewport.
    fn paint_view(&mut self, pm: &mut sk::Pixmap) {
        let (vw, vh) = ((self.view_rect.w / self.s) as u32, (self.view_rect.h / self.s) as u32);
        if vw == 0 || vh == 0 {
            return;
        }
        let proj = self.proj();
        let frame = render(&self.model, &proj, &self.style, vw, vh, &self.sel);
        let Some(img) = frame.to_pixmap() else { return };
        // the preview of what the tool is doing, drawn over the model
        let mut img = img;
        self.paint_preview(&mut img, &proj);
        let paint = sk::PixmapPaint { opacity: 1.0, quality: sk::FilterQuality::Nearest, ..Default::default() };
        pm.draw_pixmap(
            0,
            0,
            img.as_ref(),
            &paint,
            sk::Transform::from_row(self.s, 0.0, 0.0, self.s, self.view_rect.x, self.view_rect.y),
            None,
        );
    }

    /// The rubber band: the line, the rectangle, the circle, the push.
    fn paint_preview(&mut self, img: &mut sk::Pixmap, proj: &Proj) {
        let Some(from) = self.pending.from else { return };
        let Some(at) = self.pending.at else { return };
        let put = |img: &mut sk::Pixmap, a: V3, b: V3| {
            let (sa, sb) = (proj.project(a), proj.project(b));
            if !sa.infront || !sb.infront {
                return;
            }
            draw_line(img, sa.x, sa.y, sb.x, sb.y, 0x1D7FD8);
        };
        match self.tool {
            Tool::Line | Tool::Tape | Tool::Move => put(img, from, at),
            Tool::Rect => {
                for (a, b) in rect_edges(from, at) {
                    put(img, a, b);
                }
            }
            Tool::Circle => {
                let pts = circle_points(from, at, 24);
                for i in 0..pts.len() {
                    put(img, pts[i], pts[(i + 1) % pts.len()]);
                }
            }
            Tool::PushPull => {
                if let Some(f) = self.pending.face {
                    let n = self.model.ents.faces.get(f).map(|f| f.plane.n).unwrap_or_default();
                    let pts = self.model.ents.face_points(f);
                    for i in 0..pts.len() {
                        let a = pts[i] + n * self.pending.dist;
                        let b = pts[(i + 1) % pts.len()] + n * self.pending.dist;
                        put(img, a, b);
                        put(img, pts[i], a);
                    }
                }
            }
            _ => {}
        }
    }

    /// The tools down the side.
    fn paint_toolbar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        let bar = Rect::new(0.0, TITLE_H * s, TOOLBAR_W * s, self.h - TITLE_H * s - STATUS_H * s);
        gfx::fill_rect(pm, bar, t.panel);
        gfx::fill_rect(pm, Rect::new(bar.right() - 1.0, bar.y, 1.0, bar.h), t.border);
        self.tool_hits.clear();
        let size = 32.0 * s;
        let mut y = bar.y + 6.0 * s;
        for (tool, icon, _, _) in Tool::all() {
            let rr = Rect::new(bar.x + (bar.w - size) / 2.0, y, size, size);
            let on = self.tool == tool;
            if on {
                gfx::fill_round(pm, rr, 5.0 * s, t.checked);
            } else if rr.contains(self.mouse.0, self.mouse.1) {
                gfx::fill_round(pm, rr, 5.0 * s, t.hover);
            }
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.icon_colored(icon, rr.inset(7.0 * s), if on { BROWN } else { t.text });
            }
            self.tool_hits.push((tool, rr));
            y += size + 4.0 * s;
        }
    }

    /// The strip along the bottom: what the tool wants, and what is measured.
    fn paint_status(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        let rect = Rect::new(0.0, self.h - STATUS_H * s, self.w, STATUS_H * s);
        gfx::fill_rect(pm, rect, t.panel);
        gfx::fill_rect(pm, Rect::new(0.0, rect.y, self.w, 1.0), t.border);
        let left = match self.flash.as_ref() {
            Some((m, until)) if *until > self.now => m.clone(),
            _ => self.hint(),
        };
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            cx.label(&left, Rect::new(10.0 * s, rect.y, self.w * 0.6, rect.h), 12.0, t.muted, false, Align::Left);
        }
        // the measurements box, as the original has it at the right
        let label = self.tool.measure_label();
        if !label.is_empty() {
            let bw = 150.0 * s;
            let bx = self.w - bw - 10.0 * s;
            let br = Rect::new(bx, rect.y + 3.0 * s, bw, rect.h - 6.0 * s);
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.label(label, Rect::new(bx - 70.0 * s, rect.y, 64.0 * s, rect.h), 12.0, t.muted, false, Align::Right);
            }
            self.measure.rect = br;
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            self.measure.paint(&mut cx, br);
        }
    }

    /// What the status line says when nothing else is happening.
    fn hint(&self) -> String {
        if let Some((r, p)) = &self.hover {
            let what = match r.kind {
                Kind::Face => "Face",
                Kind::Edge => "Edge",
                Kind::Vertex => "Endpoint",
                Kind::Instance => "Group",
                _ => "",
            };
            if !what.is_empty() {
                return format!("{what} at {}, {}, {}", self.length_text(p.x), self.length_text(p.y), self.length_text(p.z));
            }
        }
        match self.tool {
            Tool::Select => "Select: click a face or an edge. Shift adds to what is picked out.".into(),
            Tool::Line => "Line: click where it starts, then where it ends.".into(),
            Tool::Rect => "Rectangle: drag from one corner to the other.".into(),
            Tool::Circle => "Circle: click the middle, then the edge.".into(),
            Tool::PushPull => "Push/Pull: drag a face to give it depth.".into(),
            Tool::Move => "Move: drag what is picked out.".into(),
            Tool::Erase => "Eraser: click an edge to take it out.".into(),
            Tool::Paint => "Paint: click a face to give it the colour.".into(),
            Tool::Tape => "Tape Measure: drag from one point to another.".into(),
            Tool::Orbit => "Orbit: drag to turn the model.".into(),
            Tool::Pan => "Pan: drag to slide the view.".into(),
            Tool::Zoom => "Zoom: drag up and down, or roll the wheel.".into(),
        }
    }

    pub fn paint_into(&mut self, pixels: &mut [u8], w: u32, h: u32) {
        let mut pm = match self.pm.take() {
            Some(p) if p.width() == w && p.height() == h => p,
            _ => sk::Pixmap::new(w.max(1), h.max(1)).unwrap(),
        };
        let mut r = self.res.take().expect("res");
        self.paint(&mut pm, &mut r);
        self.res = Some(r);
        pixels.copy_from_slice(pm.data());
        self.pm = Some(pm);
    }
}

/// A line straight onto the picture, for the rubber band.
fn draw_line(img: &mut sk::Pixmap, x0: f64, y0: f64, x1: f64, y1: f64, color: u32) {
    let steps = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as i32).max(1);
    let (w, h) = (img.width() as i32, img.height() as i32);
    let px = img.pixels_mut();
    let (r, g, b) = (((color >> 16) & 255) as u8, ((color >> 8) & 255) as u8, (color & 255) as u8);
    let c = sk::PremultipliedColorU8::from_rgba(r, g, b, 255).unwrap();
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = (x0 + (x1 - x0) * t).round() as i32;
        let y = (y0 + (y1 - y0) * t).round() as i32;
        for (dx, dy) in [(0, 0), (1, 0), (0, 1)] {
            let (x, y) = (x + dx, y + dy);
            if x >= 0 && y >= 0 && x < w && y < h {
                px[(y * w + x) as usize] = c;
            }
        }
    }
}

/// The four corners of a rectangle between two points, in the plane they share.
pub fn rect_corners(a: V3, b: V3) -> [V3; 4] {
    let d = b - a;
    if d.z.abs() < 1e-6 {
        [a, v3(b.x, a.y, a.z), b, v3(a.x, b.y, a.z)]
    } else if d.x.abs() < 1e-6 {
        [a, v3(a.x, b.y, a.z), b, v3(a.x, a.y, b.z)]
    } else {
        [a, v3(b.x, a.y, a.z), b, v3(a.x, a.y, b.z)]
    }
}

fn rect_edges(a: V3, b: V3) -> Vec<(V3, V3)> {
    let c = rect_corners(a, b);
    (0..4).map(|i| (c[i], c[(i + 1) % 4])).collect()
}

/// A circle about `center` through `edge`, in the plane the two share.
pub fn circle_points(center: V3, edge: V3, n: usize) -> Vec<V3> {
    let r = (edge - center).len();
    if r < 1e-6 {
        return Vec::new();
    }
    let normal = if (edge - center).z.abs() < 1e-6 { v3(0.0, 0.0, 1.0) } else { (edge - center).cross(v3(0.0, 0.0, 1.0)).unit() };
    let u = (edge - center).unit();
    let v = normal.cross(u).unit();
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            center + u * (r * folio_math::Det::dcos(a)) + v * (r * folio_math::Det::dsin(a))
        })
        .collect()
}

impl GableApp {
    // ---------------------------------------------------------------- events

    pub fn handle(&mut self, ev: &Event, host: &mut dyn Host) {
        let mut r = self.res.take().expect("res");
        self.handle_inner(ev, host, &mut r);
        self.res = Some(r);
    }

    /// Events from the window test, which already holds the resources.
    pub fn handle_test(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        self.handle_inner(ev, host, r);
    }

    pub fn handle_inner(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        if let Event::Modifiers(m) = ev {
            self.mods = *m;
        }
        if let Event::Key { mods, .. } = ev {
            self.mods = *mods;
        }
        if let Event::MouseMove { x, y } = ev {
            self.mouse = (*x, *y);
        }
        if let Some(d) = self.dialog.as_mut() {
            if let DialogResult::Button(b) = d.event(ev, host, &mut r.text, self.s) {
                let d = self.dialog.take().unwrap();
                self.dialog_done(d, &b);
            }
            return;
        }
        if let Some((what, f)) = self.files.as_mut() {
            let what = what.clone();
            if let FileAction::Chosen(p) = f.event(ev, host, &mut r.text, self.s) {
                self.files = None;
                self.file_chosen(&what, &p);
            }
            if let Event::Key { key: Key::Escape, down: true, .. } = ev {
                self.files = None;
            }
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    self.menu = None;
                    self.command(&id, host);
                    return;
                }
                MenuResult::Closed => self.menu = None,
                MenuResult::None => return,
            }
        }
        let (ta, took) = self.title.event(ev);
        if let Some(a) = ta {
            match a {
                TitleAction::Command(id) => self.command(&id, host),
                TitleAction::Close => self.exit = true,
                TitleAction::Minimize => host.minimize(),
                TitleAction::Maximize => host.toggle_maximize(),
                TitleAction::Drag => host.drag_window(),
                _ => {}
            }
            return;
        }
        if took {
            return;
        }
        // the measurements box takes a typed length while a tool is part way through
        if self.measure.focused {
            let before = self.measure.text.clone();
            self.measure.event(ev, host, &mut r.text, self.s);
            if let Event::Key { key: Key::Enter, down: true, .. } = ev {
                let text = self.measure.text.clone();
                self.apply_measurement(&text);
                self.measure.text.clear();
                self.measure.focused = false;
                return;
            }
            if self.measure.text != before {
                return;
            }
        }
        if let Event::Key { key, mods, down: true, .. } = ev {
            if self.keys(key, mods, host) {
                return;
            }
        }
        match ev {
            Event::CloseRequested => self.exit = true,
            Event::MouseDown { x, y, button, .. } => {
                if let Some((tool, _)) = self.tool_hits.iter().find(|(_, rr)| rr.contains(*x, *y)).cloned() {
                    self.set_tool(tool);
                    return;
                }
                if self.measure.rect.contains(*x, *y) {
                    self.measure.focused = true;
                    self.measure.event(ev, host, &mut r.text, self.s);
                    return;
                }
                self.measure.focused = false;
                if !self.view_rect.contains(*x, *y) {
                    return;
                }
                if *button == Button::Right {
                    self.open_menu(*x, *y);
                    return;
                }
                if *button == Button::Middle {
                    self.down_at = Some((*x, *y));
                    self.dragging = true;
                    return;
                }
                self.down_at = Some((*x, *y));
                self.dragging = false;
                self.pointer_down(*x, *y);
            }
            Event::MouseMove { x, y } => {
                let on_view = self.view_rect.contains(*x, *y);
                if self.down_at.is_some() {
                    let (dx, dy) = {
                        let (ox, oy) = self.down_at.unwrap();
                        (*x - ox, *y - oy)
                    };
                    if dx.abs() + dy.abs() > 3.0 {
                        self.dragging = true;
                    }
                    self.pointer_drag(*x, *y, dx, dy);
                } else if on_view {
                    self.hover_at(*x, *y);
                    if self.pending.from.is_some() {
                        self.pending.at = self.point_at(*x, *y);
                        if self.tool == Tool::PushPull {
                            self.update_pushpull(*x, *y);
                        }
                    }
                }
                host.set_cursor(match self.tool {
                    Tool::Orbit | Tool::Pan => Cursor::Grab,
                    Tool::Select => Cursor::Default,
                    _ => Cursor::Crosshair,
                });
            }
            Event::MouseUp { x, y, .. } => {
                self.pointer_up(*x, *y);
                self.down_at = None;
                self.dragging = false;
            }
            Event::Wheel { dy, .. } => {
                self.cam.zoom((*dy as f64) * 0.12);
            }
            _ => {}
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.pending = Pending::default();
        self.measure.text.clear();
    }

    /// What is under the pointer, in the model.
    fn point_at(&self, x: f32, y: f32) -> Option<V3> {
        let (vx, vy) = self.to_view(x, y);
        let proj = self.proj();
        if let Some((_, p)) = geom::pick(&self.model.ents, &proj, vx, vy, 8.0) {
            return Some(p);
        }
        // nothing there: the ground
        proj.on_plane(vx, vy, &Plane::through(V3::default(), v3(0.0, 0.0, 1.0)))
    }

    fn hover_at(&mut self, x: f32, y: f32) {
        let (vx, vy) = self.to_view(x, y);
        let proj = self.proj();
        self.hover = geom::pick(&self.model.ents, &proj, vx, vy, 8.0);
    }

    fn pointer_down(&mut self, x: f32, y: f32) {
        let (vx, vy) = self.to_view(x, y);
        let proj = self.proj();
        let hit = geom::pick(&self.model.ents, &proj, vx, vy, 8.0);
        match self.tool {
            Tool::Select => {
                match hit {
                    Some((r, _)) => {
                        if self.mods.shift {
                            if self.sel.contains(&r) {
                                self.sel.retain(|s| *s != r);
                            } else {
                                self.sel.push(r);
                            }
                        } else {
                            self.sel = vec![r];
                        }
                    }
                    None => self.sel.clear(),
                }
            }
            Tool::Erase => {
                if let Some((r, _)) = hit {
                    self.mark();
                    match r.kind {
                        Kind::Edge => self.model.ents.remove_edge(r.index()),
                        Kind::Face => self.model.ents.remove_face(r.index()),
                        _ => {}
                    }
                    self.sel.clear();
                }
            }
            Tool::Paint => {
                if let Some((r, _)) = hit {
                    if r.kind == Kind::Face {
                        self.mark();
                        let m = if self.model.materials.is_empty() {
                            self.model.add_material("Colour", 0xC8A06A) as i32
                        } else {
                            (self.model.materials.len() - 1) as i32
                        };
                        if let Some(f) = self.model.ents.faces.get_mut(r.index()) {
                            f.mat_front = m;
                        }
                    }
                }
            }
            Tool::PushPull => {
                if let Some((r, _)) = hit {
                    if r.kind == Kind::Face {
                        self.pending = Pending { from: Some(V3::default()), face: Some(r.index()), dist: 0.0, at: None };
                        self.sel = vec![r];
                    }
                }
            }
            Tool::Line | Tool::Rect | Tool::Circle | Tool::Tape | Tool::Move => {
                let p = self.point_at(x, y);
                match self.pending.from {
                    // the second click finishes it
                    Some(_) if self.tool != Tool::Move => self.finish_at(p),
                    _ => self.pending = Pending { from: p, face: None, dist: 0.0, at: p },
                }
            }
            _ => {}
        }
    }

    fn pointer_drag(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        let orbiting = self.tool == Tool::Orbit || (self.mods.shift && self.tool == Tool::Select) || self.mods.alt;
        match self.tool {
            _ if self.mods.ctrl || self.tool == Tool::Pan => {
                let u = self.proj().units_per_pixel(self.cam.target);
                self.cam.pan(-(dx as f64) * u, (dy as f64) * u);
                self.down_at = Some((x, y));
            }
            Tool::Orbit => {
                self.cam.orbit(dx as f64 * 0.01, dy as f64 * 0.01);
                self.down_at = Some((x, y));
            }
            Tool::Zoom => {
                self.cam.zoom(-(dy as f64) * 0.004);
                self.down_at = Some((x, y));
            }
            Tool::PushPull => self.update_pushpull(x, y),
            _ if orbiting => {
                self.cam.orbit(dx as f64 * 0.01, dy as f64 * 0.01);
                self.down_at = Some((x, y));
            }
            _ => {
                self.pending.at = self.point_at(x, y);
            }
        }
    }

    fn pointer_up(&mut self, x: f32, y: f32) {
        if !self.dragging {
            return;
        }
        match self.tool {
            Tool::PushPull => {
                let d = self.pending.dist;
                if let Some(f) = self.pending.face {
                    if d.abs() > 0.5 {
                        self.mark();
                        solids::push_pull(&mut self.model.ents, f, d);
                        self.sel.clear();
                    }
                }
                self.pending = Pending::default();
            }
            Tool::Line | Tool::Rect | Tool::Circle | Tool::Move | Tool::Tape => {
                let p = self.point_at(x, y);
                self.finish_at(p);
            }
            _ => {}
        }
    }

    /// How far the face has been pushed, from how far the pointer went.
    fn update_pushpull(&mut self, x: f32, y: f32) {
        let Some(f) = self.pending.face else { return };
        let Some(face) = self.model.ents.faces.get(f) else { return };
        let n = face.plane.n;
        let origin = self.model.ents.face_points(f).first().copied().unwrap_or_default();
        let (vx, vy) = self.to_view(x, y);
        let proj = self.proj();
        let (o, d) = proj.ray(vx, vy);
        // the distance along the normal where the ray comes closest to the face's axis
        if let Some((t, _)) = folio_gable_engine::math::line_line(origin, n, o, d) {
            self.pending.dist = t;
            self.pending.at = Some(origin + n * t);
        }
    }

    /// The tool's second point: make what it draws.
    fn finish_at(&mut self, p: Option<V3>) {
        let (Some(a), Some(b)) = (self.pending.from, p) else {
            self.pending = Pending::default();
            return;
        };
        match self.tool {
            Tool::Line => {
                if !a.near(b) {
                    self.mark();
                    let (va, vb) = (self.model.ents.add_vertex(a), self.model.ents.add_vertex(b));
                    self.model.ents.add_edge(va, vb);
                    self.model.ents.touch();
                    self.close_loop(vb);
                    // the line keeps going from where it ended
                    self.pending = Pending { from: Some(b), face: None, dist: 0.0, at: Some(b) };
                    return;
                }
            }
            Tool::Rect => {
                if (b - a).len() > 1.0 {
                    self.mark();
                    let c = rect_corners(a, b);
                    solids::add_polygon(&mut self.model.ents, &c);
                }
            }
            Tool::Circle => {
                let pts = circle_points(a, b, 24);
                if pts.len() > 2 {
                    self.mark();
                    solids::add_polygon(&mut self.model.ents, &pts);
                }
            }
            Tool::Move => {
                let d = b - a;
                if d.len() > 0.01 && !self.sel.is_empty() {
                    self.mark();
                    self.move_selection(d);
                }
            }
            Tool::Tape => {
                let d = (b - a).len();
                self.flash(&format!("{}", self.length_text(d)));
            }
            _ => {}
        }
        self.pending = Pending::default();
    }

    /// When a new edge closes a loop of edges, the loop becomes a face.
    fn close_loop(&mut self, from: usize) {
        let ents = &mut self.model.ents;
        // walk the edges from `from` looking for a way back, up to eight steps
        let mut path = vec![from];
        let mut seen = vec![from];
        let mut found: Option<Vec<usize>> = None;
        fn walk(ents: &folio_gable_engine::model::Entities, at: usize, start: usize, path: &mut Vec<usize>, seen: &mut Vec<usize>, out: &mut Option<Vec<usize>>) {
            if out.is_some() || path.len() > 12 {
                return;
            }
            for e in ents.verts[at].edges.clone() {
                let edge = &ents.edges[e];
                if !edge.alive {
                    continue;
                }
                let next = edge.other(at);
                if next == start && path.len() >= 3 {
                    *out = Some(path.clone());
                    return;
                }
                if seen.contains(&next) {
                    continue;
                }
                seen.push(next);
                path.push(next);
                walk(ents, next, start, path, seen, out);
                if out.is_some() {
                    return;
                }
                path.pop();
            }
        }
        walk(ents, from, from, &mut path, &mut seen, &mut found);
        if let Some(loop_) = found {
            let pts: Vec<V3> = loop_.iter().map(|i| ents.verts[*i].p).collect();
            if folio_gable_engine::model::plane_of(&pts).is_some() {
                ents.add_face(&loop_);
            }
        }
    }

    fn move_selection(&mut self, d: V3) {
        let mut verts: Vec<usize> = Vec::new();
        for r in self.sel.clone() {
            match r.kind {
                Kind::Vertex => verts.push(r.index()),
                Kind::Edge => {
                    if let Some(e) = self.model.ents.edges.get(r.index()) {
                        verts.extend_from_slice(&e.v);
                    }
                }
                Kind::Face => {
                    if let Some(f) = self.model.ents.faces.get(r.index()) {
                        verts.extend(f.outer.iter().copied());
                        for h in &f.holes {
                            verts.extend(h.iter().copied());
                        }
                    }
                }
                Kind::Instance => {
                    if let Some(i) = self.model.ents.insts.get_mut(r.index()) {
                        i.xf = i.xf.then(&Xf::translate(d));
                    }
                }
                _ => {}
            }
        }
        verts.sort_unstable();
        verts.dedup();
        for v in verts {
            if let Some(vert) = self.model.ents.verts.get_mut(v) {
                vert.p += d;
            }
        }
        // the planes move with their points
        let faces: Vec<usize> = (0..self.model.ents.faces.len()).collect();
        for f in faces {
            let pts = self.model.ents.face_points(f);
            if let Some(pl) = folio_gable_engine::model::plane_of(&pts) {
                self.model.ents.faces[f].plane = pl;
            }
        }
        self.model.ents.touch();
    }

    /// A length typed into the measurements box.
    fn apply_measurement(&mut self, text: &str) {
        let Some(v) = self.parse_length(text) else { return };
        match self.tool {
            Tool::PushPull => {
                if let Some(f) = self.pending.face {
                    self.mark();
                    solids::push_pull(&mut self.model.ents, f, v);
                    self.pending = Pending::default();
                    self.sel.clear();
                }
            }
            Tool::Line => {
                if let (Some(a), Some(b)) = (self.pending.from, self.pending.at) {
                    let d = (b - a).unit();
                    if !d.is_zero() {
                        self.mark();
                        let (va, vb) = (self.model.ents.add_vertex(a), self.model.ents.add_vertex(a + d * v));
                        self.model.ents.add_edge(va, vb);
                        self.model.ents.touch();
                        self.close_loop(vb);
                        self.pending = Pending::default();
                    }
                }
            }
            Tool::Circle => {
                if let Some(a) = self.pending.from {
                    self.mark();
                    let pts = circle_points(a, a + v3(v, 0.0, 0.0), 24);
                    solids::add_polygon(&mut self.model.ents, &pts);
                    self.pending = Pending::default();
                }
            }
            _ => self.flash("Type a length while a tool is part way through."),
        }
    }

    fn keys(&mut self, key: &Key, mods: &Mods, host: &mut dyn Host) -> bool {
        if mods.cmd() {
            let cmd = match key {
                Key::Char('s') => Some("save"),
                Key::Char('o') => Some("open"),
                Key::Char('n') => Some("new"),
                Key::Char('z') => Some("undo"),
                Key::Char('y') => Some("redo"),
                Key::Char('a') => Some("select-all"),
                _ => None,
            };
            if let Some(c) = cmd {
                self.command(c, host);
                return true;
            }
        }
        match key {
            Key::Char(' ') => self.set_tool(Tool::Select),
            Key::Char('l') => self.set_tool(Tool::Line),
            Key::Char('r') => self.set_tool(Tool::Rect),
            Key::Char('c') => self.set_tool(Tool::Circle),
            Key::Char('p') => self.set_tool(Tool::PushPull),
            Key::Char('m') => self.set_tool(Tool::Move),
            Key::Char('e') => self.set_tool(Tool::Erase),
            Key::Char('b') => self.set_tool(Tool::Paint),
            Key::Char('t') => self.set_tool(Tool::Tape),
            Key::Char('o') => self.set_tool(Tool::Orbit),
            Key::Char('h') => self.set_tool(Tool::Pan),
            Key::Char('z') => self.set_tool(Tool::Zoom),
            Key::Delete | Key::Backspace => self.command("delete", host),
            Key::Escape => {
                self.pending = Pending::default();
                self.sel.clear();
            }
            Key::Char(c) if c.is_ascii_digit() || *c == '.' => {
                // typing a number goes to the measurements box
                self.measure.focused = true;
                self.measure.text.push(*c);
                self.measure.caret = self.measure.text.chars().count();
            }
            _ => return false,
        }
        true
    }
}

impl GableApp {
    // ---------------------------------------------------------------- commands

    fn open_menu(&mut self, x: f32, y: f32) {
        let mut items = vec![
            MenuItem::new("view-iso", "Iso View"),
            MenuItem::new("view-top", "Top View"),
            MenuItem::new("view-front", "Front View"),
            MenuItem::new("zoom-extents", "Zoom Extents"),
            MenuItem::new("perspective", if self.cam.perspective { "Parallel Projection" } else { "Perspective" }),
            MenuItem::new("faces", if self.style.faces { "Wireframe" } else { "Shaded" }),
        ];
        if !self.sel.is_empty() {
            items.insert(0, MenuItem::new("delete", "Erase"));
            items.insert(1, MenuItem::new("paint", "Paint"));
        }
        items.push(MenuItem::new("units", "Units..."));
        self.menu = Some(Menu::new("view", Rect::new(x, y, 1.0, 1.0), items));
    }

    pub fn command(&mut self, id: &str, _host: &mut dyn Host) {
        match id {
            "new" => {
                if self.dirty && self.test.is_none() {
                    self.say("Save this drawing first", "This drawing has changes that have not been saved. Save it, or close the window without saving, before starting a new one.");
                    return;
                }
                self.model = Model::new();
                self.path = None;
                self.sel.clear();
                self.undo.clear();
                self.redo.clear();
                self.dirty = false;
                self.cam = Camera { perspective: self.settings.perspective, ..Default::default() };
            }
            "open" => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(folio_kit::platform::home_dir);
                self.files = Some(("open".into(), FilePane::new(&dir, &["gable"], false, "")));
            }
            "save" => {
                match self.path.clone() {
                    Some(p) => self.save_to(&p),
                    None => self.command("saveas", _host),
                }
            }
            "saveas" => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(folio_kit::platform::home_dir);
                self.files = Some(("saveas".into(), FilePane::new(&dir, &["gable"], true, "Drawing.gable")));
            }
            "export-obj" | "export-stl" => {
                let dir = self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(folio_kit::platform::home_dir);
                let ext = if id == "export-obj" { "obj" } else { "stl" };
                self.files = Some((id.to_string(), FilePane::new(&dir, &[ext], true, &format!("Drawing.{ext}"))));
            }
            "undo" => {
                if let Some(m) = self.undo.pop() {
                    self.redo.push(std::mem::replace(&mut self.model, m));
                    self.sel.clear();
                    self.dirty = true;
                } else {
                    self.flash("Nothing to undo.");
                }
            }
            "redo" => {
                if let Some(m) = self.redo.pop() {
                    self.undo.push(std::mem::replace(&mut self.model, m));
                    self.sel.clear();
                    self.dirty = true;
                }
            }
            "select-all" => self.sel = self.model.ents.all(),
            "delete" => {
                if self.sel.is_empty() {
                    return;
                }
                self.mark();
                let sel = self.sel.clone();
                for r in sel.iter().filter(|r| r.kind == Kind::Face) {
                    self.model.ents.remove_face(r.index());
                }
                for r in sel.iter().filter(|r| r.kind == Kind::Edge) {
                    self.model.ents.remove_edge(r.index());
                }
                self.sel.clear();
            }
            "paint" => {
                self.mark();
                let m = if self.model.materials.is_empty() {
                    self.model.add_material("Colour", 0xC8A06A) as i32
                } else {
                    (self.model.materials.len() - 1) as i32
                };
                let sel = self.sel.clone();
                for r in sel.iter().filter(|r| r.kind == Kind::Face) {
                    if let Some(f) = self.model.ents.faces.get_mut(r.index()) {
                        f.mat_front = m;
                    }
                }
            }
            "view-iso" => {
                let (lo, hi) = self.model.bounds().unwrap_or((V3::default(), v3(1000.0, 1000.0, 1000.0)));
                let c = (lo + hi) * 0.5;
                self.cam.target = c;
                self.cam.eye = c + v3(-1.0, -1.0, 0.8).unit() * ((hi - lo).len().max(1000.0) * 1.8);
                self.cam.up = v3(0.0, 0.0, 1.0);
            }
            "view-top" => {
                let (lo, hi) = self.model.bounds().unwrap_or((V3::default(), v3(1000.0, 1000.0, 1000.0)));
                let c = (lo + hi) * 0.5;
                self.cam.target = c;
                self.cam.eye = c + v3(0.0, -0.001, 1.0).unit() * ((hi - lo).len().max(1000.0) * 1.8);
            }
            "view-front" => {
                let (lo, hi) = self.model.bounds().unwrap_or((V3::default(), v3(1000.0, 1000.0, 1000.0)));
                let c = (lo + hi) * 0.5;
                self.cam.target = c;
                self.cam.eye = c + v3(0.0, -1.0, 0.0) * ((hi - lo).len().max(1000.0) * 1.8);
            }
            "zoom-extents" => {
                if let Some((lo, hi)) = self.model.bounds() {
                    self.cam.look_at_box(lo, hi);
                } else {
                    self.flash("There is nothing drawn yet.");
                }
            }
            "perspective" => {
                self.cam.perspective = !self.cam.perspective;
                self.settings.perspective = self.cam.perspective;
                self.settings.save();
            }
            "faces" => {
                self.style.faces = !self.style.faces;
                self.settings.faces = self.style.faces;
                self.settings.save();
            }
            "axes" => {
                self.style.axes = !self.style.axes;
                self.settings.axes = self.style.axes;
                self.settings.save();
            }
            "units" => {
                self.dialog = Some(
                    Dialog::new("units", "Units")
                        .choice("units", "Lengths in", &["mm", "cm", "m", "in", "ft"], ["mm", "cm", "m", "in", "ft"].iter().position(|u| *u == self.settings.units).unwrap_or(0))
                        .button("go", "Keep")
                        .button("cancel", "Cancel"),
                );
            }
            "box" => {
                self.mark();
                solids::add_box(&mut self.model.ents, V3::default(), v3(1000.0, 1000.0, 1000.0));
                if let Some((lo, hi)) = self.model.bounds() {
                    self.cam.look_at_box(lo, hi);
                }
            }
            _ => self.flash(&format!("{id} isn't in this version yet.")),
        }
    }

    fn file_chosen(&mut self, what: &str, p: &std::path::Path) {
        match what {
            "open" => self.open_path(p),
            "saveas" => {
                let p = with_ext(p, "gable");
                self.path = Some(p.clone());
                self.save_to(&p);
            }
            "export-obj" => match store::export_obj(&self.model, &with_ext(p, "obj")) {
                Ok(()) => self.flash("Wrote the .obj."),
                Err(e) => self.say("Can't write the file", &e),
            },
            "export-stl" => match store::export_stl(&self.model, &with_ext(p, "stl")) {
                Ok(()) => self.flash("Wrote the .stl."),
                Err(e) => self.say("Can't write the file", &e),
            },
            _ => {}
        }
    }

    pub fn open_path(&mut self, p: &std::path::Path) {
        match store::open(p) {
            Ok(m) => {
                self.model = m;
                self.path = Some(p.to_path_buf());
                self.sel.clear();
                self.undo.clear();
                self.redo.clear();
                self.dirty = false;
                if let Some((lo, hi)) = self.model.bounds() {
                    self.cam.look_at_box(lo, hi);
                }
                self.settings.remember(p);
            }
            Err(e) => self.say("Can't open the drawing", &e),
        }
    }

    pub fn save_to(&mut self, p: &std::path::Path) {
        match store::save(&self.model, p) {
            Ok(()) => {
                self.dirty = false;
                self.settings.remember(p);
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.flash(&format!("Saved {name}."));
            }
            Err(e) => self.say("Can't save", &e),
        }
    }

    fn dialog_done(&mut self, d: Dialog, button: &str) {
        if button != "go" {
            return;
        }
        if d.id == "units" {
            let v = d.value("units");
            if !v.is_empty() {
                self.settings.units = v;
                self.settings.save();
            }
        }
    }
}

fn with_ext(p: &std::path::Path, ext: &str) -> PathBuf {
    if p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case(ext)).unwrap_or(false) {
        p.to_path_buf()
    } else {
        p.with_extension(ext)
    }
}

impl folio_kit::platform::App for GableApp {
    fn event(&mut self, ev: &Event, host: &mut dyn Host) {
        self.now = host.now();
        self.handle(ev, host);
        host.set_title(&self.window_title());
        if self.exit {
            host.close();
        }
    }

    fn paint(&mut self, pixels: &mut [u8], w: u32, h: u32, host: &mut dyn Host) {
        self.now = host.now();
        if let Some(mut t) = self.test.take() {
            let mut r = self.res.take().expect("res");
            self.w = w as f32;
            self.h = h as f32;
            t.step(self, host, &mut r);
            self.res = Some(r);
            if t.done {
                self.exit_code = t.code();
                self.exit = true;
                host.close();
            } else {
                host.redraw();
            }
            self.test = Some(t);
        }
        self.paint_into(pixels, w, h);
    }

    fn next_tick(&self) -> Option<Duration> {
        if self.test.is_some() {
            return Some(Duration::from_millis(30));
        }
        if self.flash.is_some() {
            return Some(Duration::from_millis(400));
        }
        None
    }

    fn wants_exit(&self) -> bool {
        self.exit
    }

    fn exit_code(&self) -> i32 {
        self.exit_code
    }
}
