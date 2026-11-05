//! The Draw window: title bar, ribbon, the picture, the status bar, File pages and dialogs.

use crate::ribbon_defs;
use folio_draw_engine::doc::{self, Doc, Pt};
use folio_draw_engine::files::{self, Format};
use folio_kit::chrome::{TitleAction, TitleBar, TITLE_H};
use folio_kit::files::{FileAction, FilePane};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::math::Det;
use folio_kit::platform::{Button, Cursor, Event, Host, Image, Key, Mods};
use folio_kit::ribbon::{Ribbon, RibbonAction};
use folio_kit::text::{Align, Text};
use folio_kit::theme::Theme;
use folio_kit::ui::{CommandState, Ctx, Dialog, DialogResult, Menu, MenuItem, MenuResult};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Draw's colour: maroon, as the GTK version.
pub const MAROON: Color = Color::hex(0x800000);
pub const ACCENT: Color = Color::hex(0x2B579A);
const BAR_H: f32 = 26.0;

/// The 20 colours of the Colours menus.
pub const PALETTE: [u32; 20] = [
    0x000000, 0x7F7F7F, 0x880015, 0xED1C24, 0xFF7F27, 0xFFF200, 0x22B14C, 0x00A2E8, 0x3F48CC, 0xA349A4, 0xFFFFFF, 0xC3C3C3, 0xB97A57, 0xFFAEC9, 0xFFC90E, 0xEFE4B0, 0xB5E61D, 0x99D9EA,
    0x7092BE, 0xC8BFE7,
];
pub const SIZES: [f32; 6] = [1.0, 3.0, 5.0, 8.0, 12.0, 20.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pencil,
    Brush,
    Eraser,
    Fill,
    Picker,
    Line,
    Rect,
    Ellipse,
    Select,
}

impl Tool {
    pub fn id(self) -> &'static str {
        match self {
            Tool::Pencil => "pencil",
            Tool::Brush => "brush",
            Tool::Eraser => "eraser",
            Tool::Fill => "fill",
            Tool::Picker => "picker",
            Tool::Line => "line",
            Tool::Rect => "rect",
            Tool::Ellipse => "ellipse",
            Tool::Select => "select",
        }
    }
    pub fn from_id(s: &str) -> Option<Tool> {
        Some(match s {
            "pencil" => Tool::Pencil,
            "brush" => Tool::Brush,
            "eraser" => Tool::Eraser,
            "fill" => Tool::Fill,
            "picker" => Tool::Picker,
            "line" => Tool::Line,
            "rect" => Tool::Rect,
            "ellipse" => Tool::Ellipse,
            "select" => Tool::Select,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
enum Drag {
    Stroke,
    Shape { tool: Tool, a: Pt, color: [u8; 4] },
    Select { a: Pt, b: Pt },
    Move { start: Pt, orig: (i32, i32) },
    Pan { start: (f32, f32), scroll: (f32, f32) },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackPage {
    Open,
    SaveAs,
}

pub struct Back {
    pub page: BackPage,
    pub files: FilePane,
    hits: Vec<(String, Rect)>,
}

pub enum After {
    New,
    Open(PathBuf),
    Close,
}

pub struct Res {
    pub text: Text,
    pub icons: Icons,
}

pub struct DrawApp {
    pub doc: Doc,
    pub path: Option<PathBuf>,
    saved_changes: u64,
    pub theme: Theme,
    pub s: f32,
    pub w: f32,
    pub h: f32,
    pub title: TitleBar,
    pub ribbon: Ribbon,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub back: Option<Back>,
    pub tool: Tool,
    pub size: f32,
    pub c1: [u8; 4],
    pub c2: [u8; 4],
    pub shape_fill: bool,
    pub zoom: f32,
    pub scroll: (f32, f32),
    pub canvas: Rect,
    pub bar: Rect,
    view: Option<sk::Pixmap>,
    drag: Option<Drag>,
    pub mods: Mods,
    pub mouse: (f32, f32),
    now: Duration,
    pub exit: bool,
    pub code: i32,
    pub flash: Option<(String, Duration)>,
    pending: Option<After>,
    pub after: Option<After>,
    pub res: Option<Res>,
    pm: Option<sk::Pixmap>,
    fitted: bool,
    pub test: Option<crate::uitest::UiTest>,
}

impl DrawApp {
    pub fn new(doc: Doc, path: Option<PathBuf>) -> DrawApp {
        let mut icons = Icons::new();
        crate::icons::load(&mut icons);
        let mut doc = doc;
        doc.mark_all();
        let saved = doc.changes;
        DrawApp {
            doc,
            path,
            saved_changes: saved,
            theme: Theme::new(false, MAROON, ACCENT),
            s: 1.0,
            w: 1280.0,
            h: 800.0,
            title: TitleBar::new("app", vec![("save", "save", "Save (Ctrl+S)"), ("undo", "undo", "Undo (Ctrl+Z)"), ("redo", "redo", "Redo (Ctrl+Y)")]),
            ribbon: Ribbon::new(ribbon_defs::tabs()),
            menu: None,
            dialog: None,
            back: None,
            tool: Tool::Pencil,
            size: 3.0,
            c1: [0, 0, 0, 255],
            c2: [255, 255, 255, 255],
            shape_fill: false,
            zoom: 1.0,
            scroll: (0.0, 0.0),
            canvas: Rect::default(),
            bar: Rect::default(),
            view: None,
            drag: None,
            mods: Mods::NONE,
            mouse: (0.0, 0.0),
            now: Duration::ZERO,
            exit: false,
            code: 0,
            flash: None,
            pending: None,
            after: None,
            res: Some(Res { text: Text::bundled(), icons }),
            pm: None,
            fitted: false,
            test: None,
        }
    }

    pub fn dirty(&self) -> bool {
        self.doc.changes != self.saved_changes
    }

    pub fn doc_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into())
    }

    pub fn window_title(&self) -> String {
        format!("{}{} - Folio Draw", if self.dirty() { "*" } else { "" }, self.doc_name())
    }

    pub fn layout(&mut self) {
        let s = self.s;
        let top = TITLE_H * s + if self.back.is_some() { 0.0 } else { self.ribbon.height(s) };
        self.bar = Rect::new(0.0, self.h - BAR_H * s, self.w, BAR_H * s);
        self.canvas = Rect::new(0.0, top, self.w, (self.bar.y - top).max(10.0));
    }

    // ---------------- view ----------------

    pub fn to_doc(&self, x: f32, y: f32) -> Pt {
        ((x - self.canvas.x) / self.zoom + self.scroll.0, (y - self.canvas.y) / self.zoom + self.scroll.1)
    }

    pub fn to_screen(&self, p: Pt) -> (f32, f32) {
        ((p.0 - self.scroll.0) * self.zoom + self.canvas.x, (p.1 - self.scroll.1) * self.zoom + self.canvas.y)
    }

    pub fn fit(&mut self) {
        let (w, h) = (self.doc.w as f32, self.doc.h as f32);
        let z = ((self.canvas.w - 40.0 * self.s) / w).min((self.canvas.h - 40.0 * self.s) / h).min(1.0).max(0.01);
        self.zoom = z;
        self.center();
    }

    fn center(&mut self) {
        let (w, h) = (self.doc.w as f32, self.doc.h as f32);
        self.scroll = (w / 2.0 - self.canvas.w / (2.0 * self.zoom), h / 2.0 - self.canvas.h / (2.0 * self.zoom));
    }

    pub fn set_zoom(&mut self, z: f32, anchor: Option<(f32, f32)>) {
        let z = z.clamp(0.02, 32.0);
        let (ax, ay) = anchor.unwrap_or((self.canvas.cx(), self.canvas.cy()));
        let p = self.to_doc(ax, ay);
        self.zoom = z;
        self.scroll = (p.0 - (ax - self.canvas.x) / z, p.1 - (ay - self.canvas.y) / z);
    }

    fn zoom_step(&mut self, dir: i32, anchor: Option<(f32, f32)>) {
        const STEPS: [f32; 15] = [0.05, 0.1, 0.125, 0.25, 0.333, 0.5, 0.667, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 16.0];
        let z = self.zoom;
        let next = if dir > 0 { STEPS.iter().copied().find(|&s| s > z * 1.001).unwrap_or(32.0) } else { STEPS.iter().rev().copied().find(|&s| s < z * 0.999).unwrap_or(0.02) };
        self.set_zoom(next, anchor);
    }

    /// Bring the cached picture up to date with the document (premultiplied BGRA to RGBA).
    fn sync_view(&mut self) {
        let (w, h) = (self.doc.w as u32, self.doc.h as u32);
        if self.view.as_ref().map(|v| v.width() != w || v.height() != h).unwrap_or(true) {
            self.view = sk::Pixmap::new(w.max(1), h.max(1));
            self.doc.mark_all();
        }
        let Some(v) = self.view.as_mut() else { return };
        while let Some((x0, y0, x1, y1)) = self.doc.comp_update() {
            let data = v.data_mut();
            for y in y0..y1 {
                let o = (y * self.doc.w + x0) * 4;
                let n = (x1 - x0) * 4;
                let src = &self.doc.comp[o..o + n];
                let dst = &mut data[o..o + n];
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = s[3];
                }
            }
        }
    }

    // ---------------- events ----------------

    pub fn handle(&mut self, ev: &Event, host: &mut dyn Host) {
        let mut res = self.res.take().expect("res");
        self.handle_inner(ev, host, &mut res);
        self.res = Some(res);
        self.finish_event(host);
    }

    /// Events from the UI test, which already holds the resources.
    pub fn handle_test(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        self.handle_inner(ev, host, r);
        self.finish_event(host);
    }

    fn finish_event(&mut self, host: &mut dyn Host) {
        if self.after.is_some() && self.dialog.is_none() {
            self.run_after(host);
        }
        host.set_title(&self.window_title());
        host.redraw();
    }

    fn handle_inner(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        match ev {
            Event::Resized { w, h, scale } => {
                self.w = *w as f32;
                self.h = *h as f32;
                self.s = scale.max(0.5);
                self.layout();
                if !self.fitted {
                    self.fit();
                    self.fitted = true;
                }
                if let Some(d) = host.system_dark() {
                    self.theme = Theme::new(d, MAROON, ACCENT);
                }
                return;
            }
            Event::ThemeChanged { dark } => {
                self.theme = Theme::new(*dark, MAROON, ACCENT);
                return;
            }
            Event::Tick => {
                if let Some(mut t) = self.test.take() {
                    t.step(self, host, r);
                    if t.done {
                        self.code = t.code();
                        self.exit = true;
                    } else {
                        self.test = Some(t);
                    }
                }
                return;
            }
            Event::Modifiers(m) => self.mods = *m,
            Event::CloseRequested => {
                self.request(After::Close);
                return;
            }
            Event::FileDropped(p) => {
                self.request(After::Open(p.clone()));
                return;
            }
            Event::MouseMove { x, y } => self.mouse = (*x, *y),
            Event::Key { mods, .. } => self.mods = *mods,
            _ => {}
        }
        if let Some(d) = self.dialog.as_mut() {
            if let DialogResult::Button(b) = d.event(ev, host, &mut r.text, self.s) {
                let d = self.dialog.take().unwrap();
                self.dialog_done(d, &b, host);
            }
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    let mid = m.id.clone();
                    self.menu = None;
                    self.menu_pick(&mid, &id);
                    return;
                }
                MenuResult::Closed => {
                    self.menu = None;
                    if !matches!(ev, Event::MouseDown { .. }) {
                        return;
                    }
                }
                MenuResult::None => return,
            }
        }
        let (act, used) = self.title.event(ev);
        if let Some(a) = act {
            match a {
                TitleAction::Command(c) => self.command(&c, host),
                TitleAction::Minimize => host.minimize(),
                TitleAction::Maximize => host.toggle_maximize(),
                TitleAction::Close => self.request(After::Close),
                TitleAction::Drag => host.drag_window(),
            }
            return;
        }
        if used {
            return;
        }
        if self.back.is_some() {
            self.back_event(ev, host, r);
            return;
        }
        let st = self.state();
        let (act, used) = self.ribbon.event(ev, &st, self.now);
        if let Some(a) = act {
            match a {
                RibbonAction::Command(c) => self.command(&c, host),
                RibbonAction::Menu(id, rect) => self.open_menu(&id, rect),
                RibbonAction::Backstage => self.open_back(BackPage::Open),
                RibbonAction::GroupMenu(items, rect) => {
                    let items = items.into_iter().map(|(id, label, icon)| MenuItem::new(&id, &label).icon(&icon)).collect();
                    self.menu = Some(Menu::new("group", rect, items));
                }
            }
            return;
        }
        if used {
            return;
        }
        match ev {
            Event::Key { key, mods, down: true, .. } => self.key(key, *mods, host),
            Event::MouseDown { x, y, button, .. } if self.canvas.contains(*x, *y) => self.press(*x, *y, *button),
            Event::MouseMove { x, y } => {
                self.motion(*x, *y);
                host.set_cursor(self.cursor_at(*x, *y));
            }
            Event::MouseUp { x, y, .. } => self.release(*x, *y),
            Event::Wheel { dx, dy } => {
                if self.mods.ctrl {
                    self.zoom_step(if *dy > 0.0 { 1 } else { -1 }, Some(self.mouse));
                } else if self.mods.shift {
                    self.scroll.0 -= dy * 40.0 / self.zoom;
                } else {
                    self.scroll.0 -= dx * 40.0 / self.zoom;
                    self.scroll.1 -= dy * 40.0 / self.zoom;
                }
            }
            _ => {}
        }
    }

    fn cursor_at(&self, x: f32, y: f32) -> Cursor {
        if !self.canvas.contains(x, y) {
            return Cursor::Default;
        }
        match self.tool {
            Tool::Select if self.in_selection(self.to_doc(x, y)) => Cursor::Move,
            _ => Cursor::Crosshair,
        }
    }

    fn key(&mut self, key: &Key, m: Mods, host: &mut dyn Host) {
        let c = |ch: char| matches!(key, Key::Char(k) if k.eq_ignore_ascii_case(&ch));
        if m.ctrl {
            let cmd = if c('z') {
                "undo"
            } else if c('y') {
                "redo"
            } else if c('c') {
                "copy"
            } else if c('x') && m.shift {
                "crop"
            } else if c('x') {
                "cut"
            } else if c('v') {
                "paste"
            } else if c('a') {
                "select-all"
            } else if c('s') && m.shift {
                "save-as"
            } else if c('s') {
                "save"
            } else if c('o') {
                "open"
            } else if c('n') {
                "new"
            } else if c('w') {
                "resize"
            } else if c('i') && m.shift {
                "invert"
            } else if c('+') || c('=') {
                "zoom.in"
            } else if c('-') {
                "zoom.out"
            } else if c('0') {
                "zoom.fit"
            } else if c('1') {
                "zoom.100"
            } else {
                return;
            };
            self.command(cmd, host);
            return;
        }
        match key {
            Key::Delete => self.command("delete", host),
            Key::Escape => {
                if self.drag.is_some() {
                    self.cancel_drag();
                } else {
                    self.doc.commit_float();
                    self.doc.sel_none();
                }
            }
            Key::Char(ch) => {
                let t = match ch.to_ascii_lowercase() {
                    'p' => Some(Tool::Pencil),
                    'b' => Some(Tool::Brush),
                    'e' => Some(Tool::Eraser),
                    'f' => Some(Tool::Fill),
                    'i' => Some(Tool::Picker),
                    'l' => Some(Tool::Line),
                    'r' => Some(Tool::Rect),
                    'o' => Some(Tool::Ellipse),
                    's' => Some(Tool::Select),
                    _ => None,
                };
                if let Some(t) = t {
                    self.set_tool(t);
                }
            }
            _ => {}
        }
    }

    pub fn set_tool(&mut self, t: Tool) {
        if t != Tool::Select {
            self.doc.commit_float();
        }
        self.tool = t;
    }

    fn in_selection(&self, p: Pt) -> bool {
        if let Some(f) = &self.doc.float {
            let (x0, y0) = (f.x as f32, f.y as f32);
            return p.0 >= x0 && p.1 >= y0 && p.0 < x0 + f.w as f32 && p.1 < y0 + f.h as f32;
        }
        match self.doc.sel_bounds() {
            Some((x, y, w, h)) => p.0 >= x as f32 && p.1 >= y as f32 && p.0 < (x + w) as f32 && p.1 < (y + h) as f32,
            None => false,
        }
    }

    fn press(&mut self, x: f32, y: f32, button: Button) {
        let p = self.to_doc(x, y);
        if button == Button::Middle {
            self.drag = Some(Drag::Pan { start: (x, y), scroll: self.scroll });
            return;
        }
        let right = button == Button::Right;
        let (fg, bg) = if right { (self.c2, self.c1) } else { (self.c1, self.c2) };
        match self.tool {
            Tool::Pencil | Tool::Brush | Tool::Eraser => {
                let (tool, size, hard) = match self.tool {
                    Tool::Pencil => (doc::TOOL_PENCIL, self.size.min(4.0).max(1.0), 1.0),
                    Tool::Brush => (doc::TOOL_BRUSH, self.size * 1.5, 0.6),
                    _ => (doc::TOOL_ERASER, self.size * 2.0, 1.0),
                };
                let col = if self.tool == Tool::Eraser { bg } else { fg };
                self.doc.stroke_begin(tool, size, hard, col);
                let (px, py) = if self.tool == Tool::Pencil { (p.0.floor() + 0.5, p.1.floor() + 0.5) } else { p };
                self.doc.stroke_to(px, py);
                self.drag = Some(Drag::Stroke);
            }
            Tool::Fill => {
                if self.doc.flood_fill(p.0 as i64, p.1 as i64, fg, 0) {
                    self.doc.label_last("Fill");
                }
            }
            Tool::Picker => {
                let c = self.doc.pick(p.0 as i64, p.1 as i64, true);
                if right {
                    self.c2 = c;
                } else {
                    self.c1 = c;
                }
            }
            Tool::Line | Tool::Rect | Tool::Ellipse => {
                self.doc.commit_float();
                self.doc.op_begin();
                self.drag = Some(Drag::Shape { tool: self.tool, a: p, color: fg });
            }
            Tool::Select => {
                if self.in_selection(p) {
                    if self.doc.float.is_none() {
                        self.doc.float_lift(true);
                    }
                    if let Some(f) = &self.doc.float {
                        self.drag = Some(Drag::Move { start: p, orig: (f.x, f.y) });
                    }
                } else {
                    self.doc.commit_float();
                    self.doc.sel_none();
                    self.drag = Some(Drag::Select { a: p, b: p });
                }
            }
        }
    }

    fn motion(&mut self, x: f32, y: f32) {
        let p = self.to_doc(x, y);
        let shift = self.mods.shift;
        match self.drag.clone() {
            Some(Drag::Stroke) => {
                let (px, py) = if self.tool == Tool::Pencil { (p.0.floor() + 0.5, p.1.floor() + 0.5) } else { p };
                self.doc.stroke_to(px, py);
            }
            Some(Drag::Shape { tool, a, color, .. }) => {
                let b = if shift { square(a, p, tool == Tool::Line) } else { p };
                self.doc.op_restore();
                self.draw_shape(tool, a, b, color);
            }
            Some(Drag::Select { a, .. }) => self.drag = Some(Drag::Select { a, b: p }),
            Some(Drag::Move { start, orig }) => {
                if let Some(f) = &self.doc.float {
                    let (w, h) = (f.w, f.h);
                    let nx = orig.0 + (p.0 - start.0).round() as i32;
                    let ny = orig.1 + (p.1 - start.1).round() as i32;
                    self.doc.float_set_rect(nx, ny, w, h);
                }
            }
            Some(Drag::Pan { start, scroll }) => {
                self.scroll = (scroll.0 - (x - start.0) / self.zoom, scroll.1 - (y - start.1) / self.zoom);
            }
            None => {}
        }
    }

    fn release(&mut self, x: f32, y: f32) {
        let Some(d) = self.drag.take() else { return };
        match d {
            Drag::Stroke => {
                self.doc.stroke_end();
                self.doc.label_last(match self.tool {
                    Tool::Pencil => "Pencil",
                    Tool::Eraser => "Eraser",
                    _ => "Brush",
                });
            }
            Drag::Shape { tool, a, color, .. } => {
                let p = self.to_doc(x, y);
                let b = if self.mods.shift { square(a, p, tool == Tool::Line) } else { p };
                self.doc.op_restore();
                self.draw_shape(tool, a, b, color);
                self.doc.op_end();
                self.doc.label_last(match tool {
                    Tool::Line => "Line",
                    Tool::Rect => "Rectangle",
                    _ => "Oval",
                });
            }
            Drag::Select { a, .. } => {
                let b = self.to_doc(x, y);
                let (x0, y0) = (a.0.min(b.0).max(0.0).floor(), a.1.min(b.1).max(0.0).floor());
                let (x1, y1) = (a.0.max(b.0).min(self.doc.w as f32).ceil(), a.1.max(b.1).min(self.doc.h as f32).ceil());
                if x1 - x0 >= 1.0 && y1 - y0 >= 1.0 {
                    self.doc.sel_rect(x0 as i64, y0 as i64, (x1 - x0) as i64, (y1 - y0) as i64);
                }
            }
            Drag::Move { .. } => self.doc.label_last("Move"),
            Drag::Pan { .. } => {}
        }
    }

    fn cancel_drag(&mut self) {
        match self.drag.take() {
            Some(Drag::Shape { .. }) => self.doc.op_cancel(),
            Some(Drag::Stroke) => {
                self.doc.stroke_end();
                self.doc.undo();
            }
            _ => {}
        }
    }

    fn draw_shape(&mut self, tool: Tool, a: Pt, b: Pt, color: [u8; 4]) {
        let w = self.size;
        let (pts, closed) = match tool {
            Tool::Line => (vec![a, b], false),
            Tool::Rect => (vec![a, (b.0, a.1), b, (a.0, b.1)], true),
            _ => {
                let (cx, cy, rx, ry) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, (b.0 - a.0).abs() / 2.0, (b.1 - a.1).abs() / 2.0);
                let n = ((rx.max(ry) * 0.8) as usize).clamp(24, 360);
                let pts = (0..n)
                    .map(|i| {
                        let t = i as f32 / n as f32 * std::f32::consts::TAU;
                        (cx + rx * t.dcos(), cy + ry * t.dsin())
                    })
                    .collect();
                (pts, true)
            }
        };
        let fill = self.shape_fill && closed;
        let other = if color == self.c1 { self.c2 } else { self.c1 };
        self.doc.shape(&pts, closed, w, color, other, if fill { 3 } else { 1 }, true);
    }

    // ---------------- commands ----------------

    pub fn command(&mut self, id: &str, host: &mut dyn Host) {
        if let Some(t) = id.strip_prefix("tool.").and_then(Tool::from_id) {
            self.set_tool(t);
            return;
        }
        match id {
            "undo" => {
                self.doc.commit_float();
                self.doc.undo();
            }
            "redo" => {
                self.doc.redo();
            }
            "copy" | "cut" => {
                self.doc.commit_float();
                let had_sel = self.doc.sel.is_some();
                if self.doc.clip_extract() {
                    let img = Image { w: self.doc.clip_w as u32, h: self.doc.clip_h as u32, rgba: self.doc.clip.clone() };
                    host.set_clipboard_image(&img);
                    if id == "cut" && had_sel && self.doc.sel_delete() {
                        self.doc.label_last("Cut");
                    }
                }
            }
            "paste" => {
                if let Some(img) = host.clipboard_image() {
                    self.paste_rgba(img.w as usize, img.h as usize, img.rgba);
                } else if let Some(t) = host.clipboard_text() {
                    let p = PathBuf::from(t.trim());
                    if let Ok(b) = std::fs::read(&p) {
                        if let Ok((w, h, px)) = files::decode(&b) {
                            self.paste_rgba(w, h, px);
                        }
                    }
                }
            }
            "delete" => {
                if self.doc.float.is_some() {
                    self.doc.float_discard();
                } else if self.doc.sel_delete() {
                    self.doc.label_last("Delete");
                }
            }
            "select-all" => {
                self.doc.commit_float();
                self.set_tool(Tool::Select);
                self.doc.sel_all();
            }
            "crop" => {
                self.doc.commit_float();
                if self.doc.crop_to_selection() {
                    self.doc.label_last("Crop");
                    self.fit();
                } else {
                    self.flash("Select part of the picture first.");
                }
            }
            "resize" => {
                self.dialog = Some(
                    Dialog::new("resize", "Resize")
                        .text("w", "Width (pixels)", &self.doc.w.to_string())
                        .text("h", "Height (pixels)", &self.doc.h.to_string())
                        .check("keep", "Keep the proportions", true)
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "rotate.right" | "rotate.left" | "rotate.180" | "flip.h" | "flip.v" => {
                // the engine's transform kinds: 0 flip h, 1 flip v, 2 right, 3 180, 4 left
                let k = match id {
                    "flip.h" => 0,
                    "flip.v" => 1,
                    "rotate.right" => 2,
                    "rotate.180" => 3,
                    _ => 4,
                };
                if self.doc.float.is_some() {
                    self.doc.float_transform(k);
                } else {
                    self.doc.image_transform(k);
                    self.doc.label_last("Rotate");
                }
            }
            "invert" => {
                self.doc.invert_colors();
                self.doc.label_last("Invert colours");
            }
            "shape.fill" => self.shape_fill = !self.shape_fill,
            "layer.add" => {
                self.doc.commit_float();
                let n = self.doc.layers.len() + 1;
                self.doc.layer_add(&format!("Layer {n}"));
            }
            "zoom.in" => self.zoom_step(1, None),
            "zoom.out" => self.zoom_step(-1, None),
            "zoom.100" => self.set_zoom(1.0, None),
            "zoom.fit" => self.fit(),
            "new" => self.request(After::New),
            "open" => self.open_back(BackPage::Open),
            "save" => {
                self.save();
            }
            "save-as" => self.open_back(BackPage::SaveAs),
            "close" => self.request(After::Close),
            _ => {}
        }
    }

    pub fn paste_rgba(&mut self, w: usize, h: usize, px: Vec<u8>) {
        if w == 0 || h == 0 || px.len() < w * h * 4 {
            return;
        }
        self.doc.commit_float();
        if w > self.doc.w || h > self.doc.h {
            // the picture grows to hold what is pasted, as Paint does
            self.doc.canvas_size(w.max(self.doc.w), h.max(self.doc.h), 0.0, 0.0);
        }
        let (x, y) = (self.scroll.0.max(0.0) as i32, self.scroll.1.max(0.0) as i32);
        let (x, y) = (x.min((self.doc.w - w) as i32), y.min((self.doc.h - h) as i32));
        self.doc.float_from_rgba(w, h, px, x, y);
        self.tool = Tool::Select;
    }

    fn open_menu(&mut self, id: &str, rect: Rect) {
        let menu = match id {
            "rotate" => Menu::new(
                "rotate",
                rect,
                vec![
                    MenuItem::new("rotate.right", "Rotate right 90°"),
                    MenuItem::new("rotate.left", "Rotate left 90°"),
                    MenuItem::new("rotate.180", "Rotate 180°"),
                    MenuItem::sep(),
                    MenuItem::new("flip.v", "Flip vertical"),
                    MenuItem::new("flip.h", "Flip horizontal"),
                ],
            ),
            "size" => Menu::new("size", rect, SIZES.iter().map(|s| MenuItem::new(&format!("size:{s}"), &format!("{s} px")).check((self.size - s).abs() < 0.01)).collect()),
            "color1" | "color2" => Menu::new(id, rect, vec![]).with_palette(PALETTE.iter().map(|&c| Color::hex(c)).collect(), 10),
            _ => return,
        };
        self.menu = Some(menu);
    }

    fn menu_pick(&mut self, menu: &str, id: &str) {
        if let Some(c) = id.strip_prefix("color:").and_then(Color::parse) {
            let v = [c.r, c.g, c.b, 255];
            if menu == "color2" {
                self.c2 = v;
            } else {
                self.c1 = v;
            }
            return;
        }
        if let Some(s) = id.strip_prefix("size:").and_then(|s| s.parse::<f32>().ok()) {
            self.size = s;
            return;
        }
        let mut h = folio_kit::platform::Headless::default();
        self.command(id, &mut h);
    }

    fn dialog_done(&mut self, d: Dialog, button: &str, host: &mut dyn Host) {
        match (d.id.as_str(), button) {
            ("resize", "ok") => {
                let w: usize = d.value("w").trim().parse().unwrap_or(0);
                let mut h: usize = d.value("h").trim().parse().unwrap_or(0);
                if d.checked("keep") && w > 0 && w != self.doc.w {
                    h = ((w as f32 * self.doc.h as f32 / self.doc.w as f32).round() as usize).max(1);
                }
                if (1..=32768).contains(&w) && (1..=32768).contains(&h) {
                    self.doc.commit_float();
                    self.doc.image_resize(w, h, true);
                    self.doc.label_last("Resize");
                    self.fit();
                } else {
                    self.flash("Width and height go from 1 to 32768 pixels.");
                }
            }
            ("unsaved", "save") => {
                if self.save() {
                    self.after = self.pending.take();
                }
            }
            ("unsaved", "discard") => self.after = self.pending.take(),
            ("unsaved", _) => self.pending = None,
            _ => {}
        }
        let _ = host;
    }

    pub fn flash(&mut self, msg: &str) {
        self.flash = Some((msg.to_string(), self.now + Duration::from_secs(3)));
    }

    // ---------------- files ----------------

    pub fn save(&mut self) -> bool {
        let Some(p) = self.path.clone() else {
            self.open_back(BackPage::SaveAs);
            return false;
        };
        self.save_to(&p)
    }

    pub fn save_to(&mut self, p: &Path) -> bool {
        let fmt = Format::from_name(&p.to_string_lossy());
        if !fmt.keeps_layers() && self.doc.layers.len() > 1 {
            // a flat format: the file gets the picture as you see it; the document keeps its layers
        }
        let bytes = files::save(&mut self.doc, fmt, [255, 255, 255]);
        match folio_kit::platform::atomic_write(p, &bytes) {
            Ok(()) => {
                self.path = Some(p.to_path_buf());
                self.saved_changes = self.doc.changes;
                self.flash(&format!("Saved {}", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                true
            }
            Err(e) => {
                self.dialog = Some(Dialog::new("info", "Can't save").note(&e).button("ok", "Close"));
                false
            }
        }
    }

    pub fn load(path: &Path) -> Result<Doc, String> {
        let b = std::fs::read(path).map_err(|e| format!("Can't open {}: {e}", path.display()))?;
        files::load(&b)
    }

    pub fn request(&mut self, after: After) {
        if self.dirty() {
            self.dialog = Some(
                Dialog::new("unsaved", "Folio Draw")
                    .note(&format!("Save changes to \"{}\"?", self.doc_name()))
                    .button("save", "Save")
                    .button("discard", "Don't Save")
                    .button("cancel", "Cancel"),
            );
            self.pending = Some(after);
        } else {
            self.after = Some(after);
        }
    }

    fn run_after(&mut self, host: &mut dyn Host) {
        let Some(a) = self.after.take() else { return };
        match a {
            After::Close => {
                host.close();
                self.exit = true;
            }
            After::New => self.replace(Doc::new(1152, 648, [255, 255, 255, 255]), None),
            After::Open(p) => match Self::load(&p) {
                Ok(d) => {
                    self.replace(d, Some(p));
                    self.back = None;
                    self.layout();
                    self.fit();
                }
                Err(e) => self.dialog = Some(Dialog::new("info", "Can't open the file").note(&e).button("ok", "Close")),
            },
        }
    }

    fn replace(&mut self, d: Doc, path: Option<PathBuf>) {
        self.doc = d;
        self.doc.mark_all();
        self.saved_changes = self.doc.changes;
        self.path = path;
        self.view = None;
        self.drag = None;
        self.fit();
    }

    pub fn open_back(&mut self, page: BackPage) {
        let dir = self.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf()).unwrap_or_else(folio_kit::platform::home_dir);
        let files = match page {
            BackPage::Open => FilePane::new(&dir, &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff", "ico", "ora", "psd"], false, ""),
            BackPage::SaveAs => {
                let stem = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into());
                FilePane::new(&dir, &["png", "jpg", "bmp", "gif", "webp", "tiff", "ico", "ora", "psd"], true, &format!("{stem}.png"))
            }
        };
        self.back = Some(Back { page, files, hits: vec![] });
        self.layout();
    }

    fn back_event(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        let Some(mut b) = self.back.take() else { return };
        if let Event::Key { key: Key::Escape, down: true, .. } = ev {
            self.layout();
            return;
        }
        if let Event::MouseDown { x, y, .. } = ev {
            if let Some((id, _)) = b.hits.iter().find(|(_, r)| r.contains(*x, *y)).cloned() {
                match id.as_str() {
                    "back" => {
                        self.layout();
                        return;
                    }
                    "new" => {
                        self.layout();
                        self.request(After::New);
                        return;
                    }
                    "save" => {
                        self.back = Some(b);
                        if self.path.is_some() {
                            self.save();
                            self.back = None;
                            self.layout();
                        } else {
                            self.open_back(BackPage::SaveAs);
                        }
                        return;
                    }
                    "open" => {
                        self.open_back(BackPage::Open);
                        return;
                    }
                    "saveas" => {
                        self.open_back(BackPage::SaveAs);
                        return;
                    }
                    "close" => {
                        self.layout();
                        self.request(After::Close);
                        return;
                    }
                    _ => {}
                }
            }
        }
        if let FileAction::Chosen(p) = b.files.event(ev, host, &mut r.text, self.s) {
            match b.page {
                BackPage::Open => {
                    self.layout();
                    self.request(After::Open(p));
                    return;
                }
                BackPage::SaveAs => {
                    if self.save_to(&p) {
                        self.layout();
                        if self.pending.is_some() {
                            self.after = self.pending.take();
                        }
                        return;
                    }
                }
            }
        }
        self.back = Some(b);
    }

    // ---------------- state for the ribbon ----------------

    pub fn state(&self) -> St {
        St {
            tool: self.tool,
            can_undo: self.doc.can_undo(),
            can_redo: self.doc.can_redo(),
            has_sel: self.doc.sel.is_some() || self.doc.float.is_some(),
            c1: self.c1,
            c2: self.c2,
            shape_fill: self.shape_fill,
        }
    }

    // ---------------- painting ----------------

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

    pub fn paint(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        self.w = pm.width() as f32;
        self.h = pm.height() as f32;
        self.layout();
        let theme = self.theme.clone();
        let s = self.s;
        pm.fill(theme.window.to_sk());
        let st = self.state();
        let title = self.window_title();
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            self.title.paint(&mut cx, Rect::new(0.0, 0.0, self.w, TITLE_H * s), &title, false, &|id| match id {
                "undo" => st.can_undo,
                "redo" => st.can_redo,
                _ => true,
            });
        }
        if self.back.is_some() {
            self.paint_back(pm, r);
        } else {
            self.paint_canvas(pm);
            self.paint_bar(pm, r);
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            let rr = Rect::new(0.0, TITLE_H * s, self.w, self.ribbon.height(s));
            self.ribbon.paint(&mut cx, rr, &st, self.now);
        }
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        let win = Rect::new(0.0, 0.0, self.w, self.h);
        if let Some((msg, until)) = self.flash.clone() {
            if self.now < until {
                let w = cx.measure(&msg, 13.0, false) + 32.0 * s;
                let fr = Rect::new((self.w - w) / 2.0, self.bar.y - 48.0 * s, w, 34.0 * s).round();
                gfx::fill_round(cx.pm, fr, 6.0 * s, if theme.dark { Color::hex(0xEDEDED) } else { Color::hex(0x2B2B2B) });
                cx.label(&msg, fr, 13.0, if theme.dark { Color::hex(0x1E1E1E) } else { Color::WHITE }, false, Align::Center);
            } else {
                self.flash = None;
            }
        }
        if let Some(m) = self.menu.as_mut() {
            m.paint(&mut cx, win);
        }
        if let Some(d) = self.dialog.as_mut() {
            d.paint(&mut cx, win);
        }
        gfx::stroke_round(cx.pm, win, 0.0, theme.border, 1.0);
    }

    fn paint_canvas(&mut self, pm: &mut sk::Pixmap) {
        self.sync_view();
        let c = self.canvas;
        let (cw, ch) = (c.w.max(1.0) as u32, c.h.max(1.0) as u32);
        let Some(mut cv) = sk::Pixmap::new(cw, ch) else { return };
        cv.fill(self.theme.canvas.to_sk());
        let (ox, oy) = ((0.0 - self.scroll.0) * self.zoom, (0.0 - self.scroll.1) * self.zoom);
        let (iw, ih) = (self.doc.w as f32 * self.zoom, self.doc.h as f32 * self.zoom);
        let img = Rect::new(ox, oy, iw, ih);
        // see-through pixels show a checkerboard
        let cell = 8.0;
        gfx::fill_rect(&mut cv, img, Color::WHITE);
        let (gx0, gy0) = (img.x.max(0.0), img.y.max(0.0));
        let (gx1, gy1) = (img.right().min(cw as f32), img.bottom().min(ch as f32));
        let mut y = gy0 - ((gy0 - img.y) % (2.0 * cell));
        while y < gy1 {
            let mut x = gx0 - ((gx0 - img.x) % (2.0 * cell));
            while x < gx1 {
                for (dx, dy) in [(cell, 0.0), (0.0, cell)] {
                    let r = Rect::new((x + dx).max(gx0), (y + dy).max(gy0), cell, cell);
                    let r = Rect::new(r.x, r.y, (x + dx + cell).min(gx1) - r.x, (y + dy + cell).min(gy1) - r.y);
                    if r.w > 0.0 && r.h > 0.0 {
                        gfx::fill_rect(&mut cv, r, Color::hex(0xD9D9D9));
                    }
                }
                x += 2.0 * cell;
            }
            y += 2.0 * cell;
        }
        if let Some(v) = &self.view {
            let q = if self.zoom >= 1.0 { sk::FilterQuality::Nearest } else { sk::FilterQuality::Bilinear };
            let paint = sk::PixmapPaint { quality: q, ..Default::default() };
            cv.draw_pixmap(0, 0, v.as_ref(), &paint, sk::Transform::from_row(self.zoom, 0.0, 0.0, self.zoom, ox, oy), None);
        }
        gfx::stroke_round(&mut cv, Rect::new(img.x - 1.0, img.y - 1.0, img.w + 2.0, img.h + 2.0), 0.0, self.theme.border, 1.0);
        // the selection and a rectangle being dragged out
        let ants = |cv: &mut sk::Pixmap, r: Rect| {
            gfx::stroke_round(cv, r, 0.0, Color::WHITE, 1.0);
            let mut pb = sk::PathBuilder::new();
            pb.push_rect(sk::Rect::from_xywh(r.x, r.y, r.w.max(1.0), r.h.max(1.0)).unwrap());
            if let Some(path) = pb.finish() {
                let stroke = sk::Stroke { width: 1.0, dash: sk::StrokeDash::new(vec![4.0, 4.0], 0.0), ..Default::default() };
                cv.stroke_path(&path, &gfx::paint(Color::BLACK), &stroke, sk::Transform::identity(), None);
            }
        };
        let to = |p: Pt| ((p.0 - self.scroll.0) * self.zoom, (p.1 - self.scroll.1) * self.zoom);
        if let Some(Drag::Select { a, b }) = &self.drag {
            let (p, q) = (to(*a), to(*b));
            ants(&mut cv, Rect::new(p.0.min(q.0), p.1.min(q.1), (p.0 - q.0).abs(), (p.1 - q.1).abs()));
        } else if let Some(f) = &self.doc.float {
            let p = to((f.x as f32, f.y as f32));
            ants(&mut cv, Rect::new(p.0, p.1, f.w as f32 * self.zoom, f.h as f32 * self.zoom));
        } else if let Some((x, y, w, h)) = self.doc.sel_bounds() {
            let p = to((x as f32, y as f32));
            ants(&mut cv, Rect::new(p.0, p.1, w as f32 * self.zoom, h as f32 * self.zoom));
        }
        gfx::blit_copy(pm, &cv, c.x as i32, c.y as i32);
    }

    fn paint_bar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let b = self.bar;
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        gfx::fill_rect(cx.pm, b, theme.ribbon);
        gfx::hline(cx.pm, 0.0, self.w, b.y, theme.border);
        let p = self.to_doc(self.mouse.0, self.mouse.1);
        let inside = self.canvas.contains(self.mouse.0, self.mouse.1) && p.0 >= 0.0 && p.1 >= 0.0 && p.0 < self.doc.w as f32 && p.1 < self.doc.h as f32;
        let pos = if inside { format!("{}, {} px", p.0 as i64, p.1 as i64) } else { String::new() };
        let size = format!("{} × {} px", self.doc.w, self.doc.h);
        let sel = self.doc.float.as_ref().map(|f| (f.w, f.h)).or_else(|| self.doc.sel_bounds().map(|(_, _, w, h)| (w, h)));
        let selt = sel.map(|(w, h)| format!("Selection {w} × {h} px")).unwrap_or_default();
        let layers = if self.doc.layers.len() > 1 { format!("{} layers", self.doc.layers.len()) } else { String::new() };
        let zoom = format!("{}%", (self.zoom * 100.0).round());
        let mut x = 12.0 * s;
        for t in [pos, size, selt, layers] {
            let w = 170.0 * s;
            cx.label(&t, Rect::new(x, b.y, w, b.h), 12.0, theme.text, false, Align::Left);
            x += w;
        }
        cx.label(&zoom, Rect::new(self.w - 90.0 * s, b.y, 78.0 * s, b.h), 12.0, theme.text, false, Align::Right);
    }

    fn paint_back(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let top = TITLE_H * s;
        let left = Rect::new(0.0, top, 200.0 * s, self.h - top);
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        gfx::fill_rect(cx.pm, left, MAROON);
        let Some(b) = self.back.as_mut() else { return };
        b.hits.clear();
        let mut y = top + 12.0 * s;
        for (id, label) in [("back", "←  Back"), ("new", "New"), ("open", "Open"), ("save", "Save"), ("saveas", "Save As"), ("close", "Close")] {
            let rr = Rect::new(0.0, y, left.w, 40.0 * s);
            let on = (id == "open" && b.page == BackPage::Open) || (id == "saveas" && b.page == BackPage::SaveAs);
            if on {
                gfx::fill_rect(cx.pm, rr, Color::hex(0x5C0000));
            }
            cx.label(label, Rect::new(24.0 * s, y, left.w - 24.0 * s, rr.h), 14.0, Color::WHITE, on, Align::Left);
            b.hits.push((id.to_string(), rr));
            y += rr.h + if id == "back" { 12.0 * s } else { 0.0 };
        }
        let main = Rect::new(left.w, top, self.w - left.w, self.h - top);
        gfx::fill_rect(cx.pm, main, theme.window);
        let heading = if b.page == BackPage::Open { "Open" } else { "Save As" };
        cx.label(heading, Rect::new(main.x + 32.0 * s, top + 16.0 * s, 400.0 * s, 40.0 * s), 26.0, theme.text, false, Align::Left);
        b.files.paint(&mut cx, Rect::new(main.x + 32.0 * s, top + 70.0 * s, main.w - 64.0 * s, main.h - 90.0 * s));
    }
}

fn square(a: Pt, p: Pt, line: bool) -> Pt {
    let (dx, dy) = (p.0 - a.0, p.1 - a.1);
    if line {
        // the nearest of horizontal, vertical and 45°
        let (ax, ay) = (dx.abs(), dy.abs());
        if ax > 2.0 * ay {
            (p.0, a.1)
        } else if ay > 2.0 * ax {
            (a.0, p.1)
        } else {
            let d = ax.max(ay);
            (a.0 + d * dx.signum(), a.1 + d * dy.signum())
        }
    } else {
        let d = dx.abs().max(dy.abs());
        (a.0 + d * dx.signum(), a.1 + d * dy.signum())
    }
}

pub struct St {
    pub tool: Tool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_sel: bool,
    pub c1: [u8; 4],
    pub c2: [u8; 4],
    pub shape_fill: bool,
}

impl CommandState for St {
    fn enabled(&self, id: &str) -> bool {
        match id {
            "undo" => self.can_undo,
            "redo" => self.can_redo,
            "crop" => self.has_sel,
            _ => true,
        }
    }
    fn checked(&self, id: &str) -> bool {
        match id {
            "shape.fill" => self.shape_fill,
            t => t.strip_prefix("tool.").map(|t| t == self.tool.id()).unwrap_or(false),
        }
    }
    fn swatch(&self, id: &str) -> Option<Color> {
        let c = match id {
            "color1" => self.c1,
            "color2" => self.c2,
            _ => return None,
        };
        Some(Color::rgba(c[0], c[1], c[2], c[3]))
    }
}

impl folio_kit::platform::App for DrawApp {
    fn event(&mut self, ev: &Event, host: &mut dyn Host) {
        self.now = host.now();
        self.handle(ev, host);
        if self.exit {
            host.close();
        }
    }
    fn paint(&mut self, pixels: &mut [u8], w: u32, h: u32, host: &mut dyn Host) {
        self.now = host.now();
        self.paint_into(pixels, w, h);
    }
    fn next_tick(&self) -> Option<Duration> {
        if self.test.is_some() {
            return Some(Duration::from_millis(60));
        }
        if self.flash.is_some() {
            return Some(Duration::from_millis(500));
        }
        self.ribbon.next_tick()
    }
    fn wants_exit(&self) -> bool {
        self.exit
    }
    fn exit_code(&self) -> i32 {
        self.code
    }
}
