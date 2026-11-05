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
pub const STITCH_TYPES: [(&str, &str); 6] = [
    ("auto", "Automatic (by width)"),
    ("fill", "Fill (tatami)"),
    ("satin", "Satin"),
    ("run", "Running stitch"),
    ("applique", "Appliqué (placement, tack-down, satin cover)"),
    ("knockdown", "Knockdown (light net under the design)"),
];
pub const BLENDS: [&str; 8] = ["Normal", "Multiply", "Screen", "Overlay", "Darken", "Lighten", "Difference", "Add"];
pub const TEXT_SIZES: [u32; 8] = [12, 18, 24, 36, 48, 72, 96, 144];

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
    FreeSelect,
    Wand,
    Text,
    Magnifier,
    Polygon,
    Curve,
    EmbSelect,
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
            Tool::FreeSelect => "freeselect",
            Tool::Wand => "wand",
            Tool::Text => "text",
            Tool::Magnifier => "zoom",
            Tool::Polygon => "polygon",
            Tool::Curve => "curve",
            Tool::EmbSelect => "emb",
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
            "freeselect" => Tool::FreeSelect,
            "wand" => Tool::Wand,
            "text" => Tool::Text,
            "zoom" => Tool::Magnifier,
            "polygon" => Tool::Polygon,
            "curve" => Tool::Curve,
            "emb" => Tool::EmbSelect,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
enum Drag {
    Stroke,
    Shape { tool: Tool, a: Pt, color: [u8; 4] },
    Select { a: Pt, b: Pt },
    Lasso(Vec<Pt>),
    Move { start: Pt, orig: (i32, i32) },
    Pan { start: (f32, f32), scroll: (f32, f32) },
    PendMove { start: Pt, orig: Pending },
    PendResize { handle: usize, orig: Pending },
    EmbMove { start: (f32, f32), last: (f32, f32) },
}

/// A shape just drawn that can still be moved and resized before it is kept.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pending {
    pub tool: Tool,
    pub a: Pt,
    pub b: Pt,
    pub color: [u8; 4],
}

impl Pending {
    pub fn bbox(&self) -> (f32, f32, f32, f32) {
        (self.a.0.min(self.b.0), self.a.1.min(self.b.1), self.a.0.max(self.b.0), self.a.1.max(self.b.1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackPage {
    Open,
    SaveAs,
    PasteFrom,
    ExportStitches,
    ImportStitches,
    PlacePicture,
}

pub struct Back {
    pub page: BackPage,
    pub files: FilePane,
    hits: Vec<(String, Rect)>,
}

impl Back {
    pub fn hit_rect(&self, id: &str) -> Option<Rect> {
        self.hits.iter().find(|(i, _)| i == id).map(|(_, r)| *r)
    }
}

pub enum After {
    New,
    NewEmb(usize),
    Open(PathBuf),
    Close,
}

pub struct Res {
    pub text: Text,
    pub icons: Icons,
}

/// How many times a saved animation plays: 0 is forever, -1 once, n for n+1 times.
pub const GIF_LOOPS: &[(i32, &str)] =
    &[(0, "Forever"), (-1, "Once"), (1, "2 times"), (2, "3 times"), (4, "5 times"), (9, "10 times")];

/// The icon a quick-access button uses.
fn quick_icon(id: &str) -> String {
    match id {
        "undo" => "undo",
        "redo" => "redo",
        "crop" => "crop",
        "resize" => "resize",
        "remove-bg" => "nobg",
        "pane.layers" => "layers",
        _ => "save",
    }
    .into()
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
    pub brush: u8,
    text_at: Option<Pt>,
    /// The text layer being typed again, when the Text tool clicked one.
    text_edit: Option<usize>,
    /// A polygon or curve being made over several clicks: its points and colour.
    pub multi: Option<(Tool, Vec<Pt>, [u8; 4], u8)>,
    pub pending: Option<Pending>,
    /// Frames playing: when the next one is due.
    pub playing: Option<Duration>,
    /// The stitch view: every planned stitch drawn over the picture.
    pub stitch_view: bool,
    pub density_view: bool,
    density_cache: Option<(u64, usize, usize, Vec<u8>)>,
    /// The stitch-out simulator: how many commands have run, and when the next step is due.
    pub stitch_out: Option<usize>,
    pub full_screen: bool,
    stitch_due: Option<Duration>,
    stitch_cache: Option<(u64, Vec<([u8; 3], Vec<(f32, f32)>)>)>,
    pub export_ext: String,
    /// The selected embroidery object: (layer, object id).
    pub emb_sel: Option<(usize, i64)>,
    pub text_size: usize,
    pub text_font: usize,
    pub text_underline: bool,
    pub text_opaque: bool,
    pub zoom: f32,
    pub scroll: (f32, f32),
    pub canvas: Rect,
    pub bar: Rect,
    pub side: Rect,
    pub show_layers: bool,
    pub gridlines: bool,
    pub transparent_sel: bool,
    pub wand_tol: f32,
    pub stab: [[u8; 3]; 3],
    /// Font names for lettering, filled on the first paint (it needs the text engine).
    pub fonts: Vec<String>,
    pub letter_font: String,
    pub page: crate::page::Setup,
    printers: Vec<String>,
    stab_run: Option<crate::stabilizer::Stabilizer>,
    pub wand_contiguous: bool,
    pub rulers: bool,
    side_hits: Vec<(String, Rect)>,
    view: Option<sk::Pixmap>,
    drag: Option<Drag>,
    pub mods: Mods,
    pub mouse: (f32, f32),
    pub now: Duration,
    pub exit: bool,
    pub code: i32,
    pub flash: Option<(String, Duration)>,
    then: Option<After>,
    pub after: Option<After>,
    pub res: Option<Res>,
    pm: Option<sk::Pixmap>,
    fitted: bool,
    pub test: Option<crate::uitest::UiTest>,
    pub settings: crate::settings::Settings,
}

impl DrawApp {
    pub fn new(doc: Doc, path: Option<PathBuf>) -> DrawApp {
        let mut icons = Icons::new();
        crate::icons::load(&mut icons);
        let mut doc = doc;
        doc.mark_all();
        let saved = doc.changes;
        let settings = crate::settings::Settings::load();
        DrawApp {
            doc,
            path,
            saved_changes: saved,
            theme: Theme::new(false, MAROON, ACCENT),
            s: 1.0,
            w: 1280.0,
            h: 800.0,
            title: TitleBar::new("app", vec![("save", "save", "Save (Ctrl+S)"), ("undo", "undo", "Undo (Ctrl+Z)"), ("redo", "redo", "Redo (Ctrl+Y)")])
                .with_search()
                .with_quick_menu(),
            ribbon: Ribbon::new(ribbon_defs::tabs()),
            menu: None,
            dialog: None,
            back: None,
            tool: Tool::Pencil,
            size: 3.0,
            c1: [0, 0, 0, 255],
            c2: [255, 255, 255, 255],
            shape_fill: false,
            brush: doc::TOOL_BRUSH,
            text_at: None,
            text_edit: None,
            multi: None,
            pending: None,
            playing: None,
            stitch_view: false,
            density_view: false,
            density_cache: None,
            stitch_out: None,
            full_screen: false,
            stitch_due: None,
            stitch_cache: None,
            export_ext: "pes".into(),
            emb_sel: None,
            text_size: 2,
            text_font: 0,
            text_underline: false,
            text_opaque: false,
            zoom: 1.0,
            scroll: (0.0, 0.0),
            canvas: Rect::default(),
            bar: Rect::default(),
            side: Rect::default(),
            show_layers: false,
            gridlines: false,
            transparent_sel: false,
            wand_tol: 32.0,
            stab: [[0; 3]; 3],
            fonts: vec![],
            letter_font: "Carlito Bold".into(),
            page: crate::page::Setup::default(),
            printers: vec![],
            stab_run: None,
            wand_contiguous: true,
            rulers: false,
            side_hits: vec![],
            view: None,
            drag: None,
            mods: Mods::NONE,
            mouse: (0.0, 0.0),
            now: Duration::ZERO,
            exit: false,
            code: 0,
            flash: None,
            then: None,
            after: None,
            res: Some(Res { text: Text::bundled(), icons }),
            pm: None,
            fitted: false,
            test: None,
            settings,
        }
        .with_settings()
    }

    /// Which of the three stroke tools the stabilizer settings are for.
    pub fn stab_slot(&self) -> usize {
        match self.tool {
            Tool::Brush => 1,
            Tool::Eraser => 2,
            _ => 0,
        }
    }

    fn with_settings(mut self) -> DrawApp {
        let st = self.settings.clone();
        self.size = st.size;
        self.c1 = st.c1;
        self.c2 = st.c2;
        self.rulers = st.rulers;
        self.gridlines = st.grid;
        self.show_layers = st.layers;
        self.wand_tol = st.wand_tol;
        self.wand_contiguous = st.wand_contiguous;
        self.doc.sample_all = st.sample_all;
        self.stab = st.stab;
        self.page = st.page.clone();
        if !st.quick.is_empty() {
            let have = self.title.quick.clone();
            let known = self.quick_choices();
            self.title.quick = st
                .quick
                .iter()
                .filter_map(|id| {
                    have.iter()
                        .find(|q| q.0 == *id)
                        .cloned()
                        .or_else(|| known.iter().find(|(k, _)| k == id).map(|(k, label)| ((*k).to_string(), quick_icon(k), label.to_string())))
                })
                .collect();
            if self.title.quick.is_empty() {
                self.title.quick = have;
            }
        }
        if let Some(p) = self.path.clone() {
            self.settings.add_recent(&p);
        }
        self
    }

    /// Keep the current choices for next time.
    pub fn remember(&mut self) {
        self.settings.size = self.size;
        self.settings.c1 = self.c1;
        self.settings.c2 = self.c2;
        self.settings.rulers = self.rulers;
        self.settings.grid = self.gridlines;
        self.settings.layers = self.show_layers;
        self.settings.wand_tol = self.wand_tol;
        self.settings.wand_contiguous = self.wand_contiguous;
        self.settings.sample_all = self.doc.sample_all;
        self.settings.stab = self.stab;
        self.settings.page = self.page.clone();
        self.settings.quick = self.title.quick.iter().map(|q| q.0.clone()).collect();
        self.settings.save();
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
        let sw = if self.show_layers && self.back.is_none() { 230.0 * s } else { 0.0 };
        self.side = Rect::new(self.w - sw, top, sw, self.bar.y - top);
        let ru = if self.rulers { 18.0 * s } else { 0.0 };
        self.canvas = Rect::new(ru, top + ru, (self.w - sw - ru).max(10.0), (self.bar.y - top - ru).max(10.0));
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
                if let (Some(at), Some(due)) = (self.stitch_out, self.stitch_due) {
                    if self.now >= due {
                        // 400 stitches a minute, ten times over: about 67 commands a tick at 40 ms
                        let total = crate::emb::make_plan(&self.doc).map(|p| crate::emb::command_count(&p)).unwrap_or(0);
                        let next = at + 67;
                        if next >= total {
                            self.stitch_out = None;
                            self.stitch_due = None;
                            self.flash(&format!("Stitched out: {total} commands, about {:.0} seconds on the machine.", total as f32 / 400.0 * 60.0));
                        } else {
                            self.stitch_out = Some(next);
                            self.stitch_due = Some(self.now + Duration::from_millis(40));
                        }
                        self.doc.mark_all();
                    }
                }
                if let Some(due) = self.playing {
                    if self.now >= due {
                        let n = self.doc.layers.len().max(1);
                        let next = (self.doc.active + 1) % n;
                        self.doc.set_active(next);
                        self.doc.mark_all();
                        self.playing = Some(self.now + self.frame_delay());
                    }
                }
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
                self.dialog_done(d, &b, host, r);
            }
            return;
        }
        // the search box has the keyboard while it is focused; the list of matches keeps the arrows
        let arrows = matches!(ev, Event::Key { key: Key::Up | Key::Down, .. });
        if !arrows {
        if let Some(a) = self.title.search_event(ev, host, &mut r.text, self.s) {
                match a {
                    TitleAction::SearchChanged => self.open_search_menu(),
                    TitleAction::SearchEnter => {
                        let first = self.search_matches().first().map(|(id, _)| id.clone());
                        self.title.search.as_mut().map(|f| {
                            f.focused = false;
                            f.set("")
                        });
                        self.menu = None;
                        if let Some(id) = first {
                            self.command(&id, host);
                        }
                    }
                    TitleAction::SearchCancel => self.menu = None,
                    _ => {}
                }
                return;
            }
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    let mid = m.id.clone();
                    self.menu = None;
                    if mid == "search" {
                        if let Some(f) = self.title.search.as_mut() {
                            f.focused = false;
                            f.set("");
                        }
                        self.command(&id, host);
                    } else {
                        self.menu_pick(&mid, &id);
                    }
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
                TitleAction::SearchChanged | TitleAction::SearchEnter | TitleAction::SearchCancel => {}
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
            Event::MouseDown { x, y, button, clicks } if self.side.contains(*x, *y) => {
                self.side_click(*x, *y, *button);
                if *clicks >= 2 && self.side_hits.iter().any(|(id, r)| id.starts_with("row:") && r.contains(*x, *y)) {
                    self.command("layer.props", host);
                }
            }
            Event::MouseDown { x, y, button, clicks } if self.canvas.contains(*x, *y) => self.press(*x, *y, *button, *clicks),
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
            Tool::Select | Tool::FreeSelect | Tool::Wand if self.in_selection(self.to_doc(x, y)) => Cursor::Move,
            Tool::Text => Cursor::Text,
            Tool::Magnifier => Cursor::Pointer,
            _ => Cursor::Crosshair,
        }
    }

    fn key(&mut self, key: &Key, m: Mods, host: &mut dyn Host) {
        let c = |ch: char| matches!(key, Key::Char(k) if k.eq_ignore_ascii_case(&ch));
        if m.alt && c('q') {
            self.title.focus_search();
            return;
        }
        if matches!(key, Key::F(11)) {
            self.command("full-screen", host);
            return;
        }
        if matches!(key, Key::Escape) && self.full_screen {
            self.command("full-screen", host);
            return;
        }
        if m.ctrl && c('p') {
            self.command("print", host);
            return;
        }
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
            } else if c('n') && m.shift {
                "layer.add"
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
            } else if c('r') {
                "view.rulers"
            } else if c('g') {
                "view.grid"
            } else {
                return;
            };
            self.command(cmd, host);
            return;
        }
        match key {
            Key::Delete => self.command("delete", host),
            Key::F(7) => self.command("pane.layers", host),
            Key::Left | Key::Right | Key::Up | Key::Down => {
                let step = if m.shift { 10 } else { 1 };
                let (dx, dy) = match key {
                    Key::Left => (-step, 0),
                    Key::Right => (step, 0),
                    Key::Up => (0, -step),
                    _ => (0, step),
                };
                if self.doc.float.is_none() && self.doc.sel.is_some() {
                    self.doc.float_lift(true);
                }
                if let Some(f) = &self.doc.float {
                    let (x, y, w, h) = (f.x + dx, f.y + dy, f.w, f.h);
                    self.doc.float_set_rect(x, y, w, h);
                } else {
                    self.scroll.0 += dx as f32 * 20.0 / self.zoom;
                    self.scroll.1 += dy as f32 * 20.0 / self.zoom;
                }
            }
            Key::Escape if self.pending.is_some() && self.drag.is_none() => {
                self.pending = None;
                self.doc.op_cancel();
            }
            Key::Enter if self.pending.is_some() => self.commit_pending(),
            Key::Escape if self.multi.is_some() => {
                self.multi = None;
                self.doc.op_cancel();
            }
            Key::Escape => {
                if self.drag.is_some() {
                    self.cancel_drag();
                } else {
                    self.doc.commit_float();
                    self.doc.sel_none();
                }
            }
            Key::Char('.') if self.doc.solo => self.command("frame.next", host),
            Key::Char(',') if self.doc.solo => self.command("frame.prev", host),
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
                    't' => Some(Tool::Text),
                    'm' => Some(Tool::Magnifier),
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
        self.commit_pending();
        if self.multi.take().is_some() {
            self.doc.op_cancel();
        }
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

    fn press(&mut self, x: f32, y: f32, button: Button, clicks: u32) {
        let p = self.to_doc(x, y);
        if self.playing.take().is_some() {
            return;
        }
        if self.pending.is_some() && button == Button::Left && self.pending_press(p, x, y) {
            return;
        }
        if button != Button::Middle {
            self.commit_pending();
        }
        if self.multi.is_some() || matches!(self.tool, Tool::Polygon | Tool::Curve) {
            if button != Button::Middle {
                self.multi_press(p, x, y, button, clicks);
                return;
            }
        }
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
                    Tool::Brush => (self.brush, self.size * if self.brush == doc::TOOL_AIRBRUSH { 4.0 } else { 1.5 }, if self.brush == doc::TOOL_BRUSH { 0.6 } else { 1.0 }),
                    _ => (doc::TOOL_ERASER, self.size * 2.0, 1.0),
                };
                let col = if self.tool == Tool::Eraser { bg } else { fg };
                self.doc.stroke_begin(tool, size, hard, col);
                let (px, py) = if self.tool == Tool::Pencil { (p.0.floor() + 0.5, p.1.floor() + 0.5) } else { p };
                self.doc.stroke_to(px, py);
                let mut st = crate::stabilizer::Stabilizer::new(self.stab[self.stab_slot()], self.zoom);
                st.start(p.0, p.1);
                self.stab_run = if st.active() { Some(st) } else { None };
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
            Tool::Magnifier => {
                self.zoom_step(if right { -1 } else { 1 }, Some((x, y)));
            }
            Tool::Polygon | Tool::Curve => {}
            Tool::EmbSelect => {
                if let Some(st) = crate::emb::settings(&self.doc) {
                    let mm = crate::emb::px_to_mm(&st, p.0, p.1);
                    self.emb_sel = crate::emb::object_at(&self.doc, mm.0, mm.1);
                    if self.emb_sel.is_some() {
                        self.drag = Some(Drag::EmbMove { start: mm, last: mm });
                    }
                }
            }
            Tool::Text => {
                self.doc.commit_float();
                let (mut txt, mut bold, mut italic, mut at) = (String::new(), false, false, p);
                let (mut under, mut opaque) = (self.text_underline, self.text_opaque);
                self.text_edit = None;
                if let Some((i, spec)) = self.text_layer_at(p) {
                    // a text layer: type it again
                    txt = spec["text"].as_str().unwrap_or("").to_string();
                    bold = spec["bold"].as_bool().unwrap_or(false);
                    italic = spec["italic"].as_bool().unwrap_or(false);
                    under = spec["underline"].as_bool().unwrap_or(false);
                    opaque = spec["text_opaque"].as_bool().unwrap_or(false);
                    at = (spec["x"].as_f64().unwrap_or(0.0) as f32, spec["y"].as_f64().unwrap_or(0.0) as f32);
                    let size = spec["font_size"].as_f64().unwrap_or(24.0) as u32;
                    self.text_size = TEXT_SIZES.iter().position(|&s| s >= size).unwrap_or(TEXT_SIZES.len() - 1);
                    let fam = spec["font_family"].as_str().unwrap_or("");
                    if let Some(k) = self.text_fonts().iter().position(|f| f == fam) {
                        self.text_font = k;
                    }
                    self.text_edit = Some(i);
                    self.doc.set_active(i);
                }
                self.text_at = Some(at);
                let fonts = self.text_fonts();
                let names: Vec<&str> = fonts.iter().map(|f| if f.is_empty() { "The app's font" } else { f.as_str() }).collect();
                self.dialog = Some(
                    Dialog::new("text", "Text")
                        .text("text", "Text", &txt)
                        .choice("font", "Font", &names, self.text_font.min(names.len().saturating_sub(1)))
                        .choice("size", "Size", &TEXT_SIZES.iter().map(|s| s.to_string()).collect::<Vec<_>>().iter().map(|s| s.as_str()).collect::<Vec<_>>(), self.text_size)
                        .check("bold", "Bold", bold)
                        .check("italic", "Italic", italic)
                        .check("underline", "Underline", under)
                        .check("opaque", "Fill the box behind with colour 2", opaque)
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            Tool::Wand => {
                self.doc.commit_float();
                self.doc.sel_mode = self.sel_mode();
                let cont = self.wand_contiguous != self.mods.ctrl;
                self.doc.sel_magic(p.0 as i64, p.1 as i64, self.wand_tol.round().clamp(0.0, 255.0) as u8, cont);
            }
            Tool::FreeSelect => {
                self.doc.commit_float();
                self.drag = Some(Drag::Lasso(vec![p]));
            }
            Tool::Select => {
                if self.in_selection(p) && !self.mods.shift && !self.mods.alt {
                    if self.doc.float.is_none() {
                        // Ctrl leaves the original where it was
                        self.doc.float_lift(!self.mods.ctrl);
                        if self.transparent_sel {
                            self.doc.float_color_key(self.c2, 0);
                        }
                    }
                    if let Some(f) = &self.doc.float {
                        self.drag = Some(Drag::Move { start: p, orig: (f.x, f.y) });
                    }
                } else {
                    self.doc.commit_float();
                    if !self.mods.shift && !self.mods.alt {
                        self.doc.sel_none();
                    }
                    self.drag = Some(Drag::Select { a: p, b: p });
                }
            }
        }
    }

    fn motion(&mut self, x: f32, y: f32) {
        let p = self.to_doc(x, y);
        if self.multi.is_some() && !matches!(self.drag, Some(Drag::Pan { .. })) {
            self.multi_move(p);
            return;
        }
        let shift = self.mods.shift;
        match self.drag.clone() {
            Some(Drag::Stroke) => {
                let mut pts = vec![p];
                if let Some(st) = self.stab_run.as_mut() {
                    pts = st.feed(p.0, p.1).into_iter().collect();
                }
                for q in pts {
                    let (px, py) = if self.tool == Tool::Pencil { (q.0.floor() + 0.5, q.1.floor() + 0.5) } else { q };
                    self.doc.stroke_to(px, py);
                }
            }
            Some(Drag::Shape { tool, a, color, .. }) => {
                let b = if shift { square(a, p, tool == Tool::Line) } else { p };
                self.doc.op_restore();
                self.draw_shape(tool, a, b, color);
            }
            Some(Drag::Select { a, .. }) => self.drag = Some(Drag::Select { a, b: p }),
            Some(Drag::Lasso(mut pts)) => {
                if pts.last().map(|q| (q.0 - p.0).abs() + (q.1 - p.1).abs() > 0.5).unwrap_or(true) {
                    pts.push(p);
                }
                self.drag = Some(Drag::Lasso(pts));
            }
            Some(Drag::Move { start, orig }) => {
                if let Some(f) = &self.doc.float {
                    let (w, h) = (f.w, f.h);
                    let nx = orig.0 + (p.0 - start.0).round() as i32;
                    let ny = orig.1 + (p.1 - start.1).round() as i32;
                    self.doc.float_set_rect(nx, ny, w, h);
                }
            }
            Some(Drag::PendMove { start, orig }) => {
                let (dx, dy) = (p.0 - start.0, p.1 - start.1);
                self.pending = Some(Pending { a: (orig.a.0 + dx, orig.a.1 + dy), b: (orig.b.0 + dx, orig.b.1 + dy), ..orig });
                self.redraw_pending();
            }
            Some(Drag::PendResize { handle, orig }) => {
                let np = if orig.tool == Tool::Line {
                    if handle == 0 {
                        Pending { a: p, ..orig }
                    } else {
                        Pending { b: p, ..orig }
                    }
                } else {
                    let (mut x0, mut y0, mut x1, mut y1) = orig.bbox();
                    if matches!(handle, 0 | 6 | 7) {
                        x0 = p.0;
                    }
                    if matches!(handle, 2 | 3 | 4) {
                        x1 = p.0;
                    }
                    if matches!(handle, 0 | 1 | 2) {
                        y0 = p.1;
                    }
                    if matches!(handle, 4 | 5 | 6) {
                        y1 = p.1;
                    }
                    Pending { a: (x0, y0), b: (x1, y1), ..orig }
                };
                self.pending = Some(np);
                self.redraw_pending();
            }
            Some(Drag::EmbMove { start, .. }) => {
                if let Some(st) = crate::emb::settings(&self.doc) {
                    let mm = crate::emb::px_to_mm(&st, p.0, p.1);
                    self.drag = Some(Drag::EmbMove { start, last: mm });
                }
            }
            Some(Drag::Pan { start, scroll }) => {
                self.scroll = (scroll.0 - (x - start.0) / self.zoom, scroll.1 - (y - start.1) / self.zoom);
            }
            None => {}
        }
    }

    fn release(&mut self, x: f32, y: f32) {
        if let Some((Tool::Curve, pts, color, stage)) = self.multi.clone() {
            // a curve: the line, then each bend, is set when the button comes up
            let stage = stage + 1;
            if stage >= 3 {
                self.doc.op_restore();
                self.draw_multi(Tool::Curve, &pts, color);
                self.doc.op_end();
                self.doc.label_last("Curve");
                self.multi = None;
            } else {
                self.multi = Some((Tool::Curve, pts, color, stage));
            }
            let _ = (x, y);
            return;
        }
        let Some(d) = self.drag.take() else { return };
        match d {
            Drag::Stroke => {
                if let Some(mut st) = self.stab_run.take() {
                    for q in st.finish() {
                        let (px, py) = if self.tool == Tool::Pencil { (q.0.floor() + 0.5, q.1.floor() + 0.5) } else { q };
                        self.doc.stroke_to(px, py);
                    }
                }
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
                // kept open: it can still be moved and resized
                self.pending = Some(Pending { tool, a, b, color });
            }
            Drag::PendMove { .. } | Drag::PendResize { .. } => {}
            Drag::EmbMove { start, last } => {
                let (dx, dy) = (last.0 - start.0, last.1 - start.1);
                if dx.abs() + dy.abs() > 0.05 {
                    self.edit_emb_object("Move shapes", |o| crate::emb::move_object(o, dx, dy));
                }
            }
            Drag::Select { a, .. } => {
                let b = self.to_doc(x, y);
                let (x0, y0) = (a.0.min(b.0).max(0.0).floor(), a.1.min(b.1).max(0.0).floor());
                let (x1, y1) = (a.0.max(b.0).min(self.doc.w as f32).ceil(), a.1.max(b.1).min(self.doc.h as f32).ceil());
                if x1 - x0 >= 1.0 && y1 - y0 >= 1.0 {
                    self.doc.sel_mode = self.sel_mode();
                    self.doc.sel_rect(x0 as i64, y0 as i64, (x1 - x0) as i64, (y1 - y0) as i64);
                }
            }
            Drag::Lasso(pts) => {
                if pts.len() >= 3 {
                    self.doc.sel_mode = self.sel_mode();
                    self.doc.sel_poly(&pts);
                }
            }
            Drag::Move { .. } => self.doc.label_last("Move"),
            Drag::Pan { .. } => {}
        }
    }

    fn cancel_drag(&mut self) {
        match self.drag.take() {
            Some(Drag::Shape { .. }) => self.doc.op_cancel(),
            Some(Drag::PendMove { orig, .. }) | Some(Drag::PendResize { orig, .. }) => {
                self.pending = Some(orig);
                self.redraw_pending();
            }
            Some(Drag::Stroke) => {
                self.doc.stroke_end();
                self.doc.undo();
            }
            _ => {}
        }
    }

    fn multi_press(&mut self, p: Pt, x: f32, y: f32, button: Button, clicks: u32) {
        let fg = if button == Button::Right { self.c2 } else { self.c1 };
        match self.multi.take() {
            None => {
                self.doc.commit_float();
                self.doc.op_begin();
                let pts = if self.tool == Tool::Curve { vec![p, p, p, p] } else { vec![p, p] };
                self.multi = Some((self.tool, pts, fg, 0));
            }
            Some((Tool::Polygon, mut pts, color, st)) => {
                let first = self.to_screen(pts[0]);
                let near = (first.0 - x).abs() < 6.0 && (first.1 - y).abs() < 6.0;
                if clicks >= 2 || (near && pts.len() > 3) {
                    // finished: closed and drawn for good
                    pts.pop();
                    if near && pts.len() > 3 {
                        pts.pop();
                    }
                    self.doc.op_restore();
                    if pts.len() >= 3 {
                        self.draw_closed(&pts, color);
                        self.doc.op_end();
                        self.doc.label_last("Polygon");
                    } else {
                        self.doc.op_cancel();
                    }
                } else {
                    *pts.last_mut().unwrap() = p;
                    pts.push(p);
                    self.multi = Some((Tool::Polygon, pts, color, st));
                }
            }
            Some(m) => self.multi = Some(m),
        }
    }

    fn multi_move(&mut self, p: Pt) {
        let Some((tool, mut pts, color, stage)) = self.multi.take() else { return };
        match tool {
            Tool::Curve => {
                match stage {
                    0 => {
                        // the line: end and both bends follow
                        pts[3] = p;
                        pts[1] = pts[0];
                        pts[2] = p;
                    }
                    1 => {
                        pts[1] = p;
                        pts[2] = p;
                    }
                    _ => pts[2] = p,
                }
            }
            _ => *pts.last_mut().unwrap() = p,
        }
        self.doc.op_restore();
        self.draw_multi(tool, &pts, color);
        self.multi = Some((tool, pts, color, stage));
    }

    fn draw_multi(&mut self, tool: Tool, pts: &[Pt], color: [u8; 4]) {
        if tool == Tool::Curve {
            let (a, b, c, d) = (pts[0], pts[1], pts[2], pts[3]);
            let path: Vec<Pt> = (0..=64)
                .map(|i| {
                    let t = i as f32 / 64.0;
                    let u = 1.0 - t;
                    let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    (k0 * a.0 + k1 * b.0 + k2 * c.0 + k3 * d.0, k0 * a.1 + k1 * b.1 + k2 * c.1 + k3 * d.1)
                })
                .collect();
            self.doc.shape(&path, false, self.size, color, color, 1, true);
        } else {
            self.doc.shape(pts, false, self.size, color, color, 1, true);
        }
    }

    fn draw_closed(&mut self, pts: &[Pt], color: [u8; 4]) {
        let other = if color == self.c1 { self.c2 } else { self.c1 };
        self.doc.shape(pts, true, self.size, color, other, if self.shape_fill { 3 } else { 1 }, true);
    }

    fn frame_delay(&self) -> Duration {
        let d = self.doc.layers.get(self.doc.active).map(|l| l.delay_ms).unwrap_or(0);
        Duration::from_millis(if d == 0 { 100 } else { d.max(20) } as u64)
    }

    /// Change the selected embroidery object and store its layer again (one undo step).
    /// The fonts the Text tool offers: the app's own first, then what is installed.
    pub fn text_fonts(&self) -> Vec<String> {
        let mut out = vec![String::new()];
        for f in &self.fonts {
            if !f.ends_with(" Bold") && !out.contains(f) {
                out.push(f.clone());
            }
        }
        out
    }

    /// Every command the ribbon offers, as (id, label), for the search box.
    pub fn all_commands(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = vec![];
        for t in &self.ribbon.tabs {
            for g in &t.groups {
                for it in &g.items {
                    if it.id.is_empty() || out.iter().any(|(id, _)| *id == it.id) {
                        continue;
                    }
                    out.push((it.id.clone(), it.label.replace('\n', " ")));
                }
            }
        }
        for (id, label) in [
            ("new", "New picture"),
            ("open", "Open"),
            ("save", "Save"),
            ("save-as", "Save as"),
            ("undo", "Undo"),
            ("redo", "Redo"),
            ("select-all", "Select all"),
            ("select-invert", "Invert selection"),
            ("select-none", "Select none"),
            ("canvas-size", "Canvas size"),
            ("rotate.angle", "Rotate by angle"),
            ("skew", "Skew"),
            ("stabilizer", "Stabilizer"),
            ("wand-settings", "Magic wand settings"),
            ("close", "Close picture"),
        ] {
            if !out.iter().any(|(i, _)| i == id) {
                out.push((id.into(), label.into()));
            }
        }
        out
    }

    /// The commands whose names hold the words typed in the search box, best first.
    pub fn search_matches(&self) -> Vec<(String, String)> {
        let q = self.title.search_text().trim().to_lowercase();
        if q.is_empty() {
            return vec![];
        }
        let mut starts = vec![];
        let mut holds = vec![];
        for (id, label) in self.all_commands() {
            let l = label.to_lowercase();
            if l.starts_with(&q) {
                starts.push((id, label));
            } else if l.contains(&q) {
                holds.push((id, label));
            }
        }
        starts.extend(holds);
        starts.truncate(12);
        starts
    }

    fn open_search_menu(&mut self) {
        let hits = self.search_matches();
        if hits.is_empty() {
            self.menu = None;
            return;
        }
        let r = self.title.search_rect();
        let items = hits.into_iter().map(|(id, label)| MenuItem::new(&id, &label)).collect();
        self.menu = Some(Menu::new("search", Rect::new(r.x, r.bottom(), r.w, 0.0), items));
    }

    /// The buttons the quick-access bar can show, and whether each is on it now.
    pub fn quick_choices(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("save", "Save"),
            ("undo", "Undo"),
            ("redo", "Redo"),
            ("new", "New picture"),
            ("open", "Open"),
            ("crop", "Crop"),
            ("resize", "Resize"),
            ("remove-bg", "Remove background"),
            ("pane.layers", "Layers pane"),
        ]
    }

    fn edit_emb_object(&mut self, label: &str, f: impl FnOnce(&mut serde_json::Value)) {
        let Some((li, id)) = self.emb_sel else { return };
        let Some(mut spec) = crate::emb::layer_spec(&self.doc, li) else { return };
        if let Some(objs) = spec["objects"].as_array_mut() {
            if let Some(o) = objs.iter_mut().find(|o| o["id"].as_i64() == Some(id)) {
                f(o);
            }
        }
        spec["edited"] = serde_json::json!(true);
        crate::emb::store_layer(&mut self.doc, li, &spec);
        self.doc.label_last(label);
    }

    fn delete_emb_object(&mut self) {
        let Some((li, id)) = self.emb_sel.take() else { return };
        let Some(mut spec) = crate::emb::layer_spec(&self.doc, li) else { return };
        if let Some(objs) = spec["objects"].as_array_mut() {
            objs.retain(|o| o["id"].as_i64() != Some(id));
        }
        spec["edited"] = serde_json::json!(true);
        crate::emb::store_layer(&mut self.doc, li, &spec);
        self.doc.label_last("Delete shapes");
    }

    /// Keep the pending shape for good (one undo step).
    pub fn commit_pending(&mut self) {
        if let Some(p) = self.pending.take() {
            self.doc.op_end();
            self.doc.label_last(match p.tool {
                Tool::Line => "Line",
                Tool::Rect => "Rectangle",
                _ => "Oval",
            });
        }
    }

    fn redraw_pending(&mut self) {
        if let Some(p) = self.pending {
            self.doc.op_restore();
            self.draw_shape(p.tool, p.a, p.b, p.color);
        }
    }

    /// The pending shape's handles in picture coordinates: both ends of a line, else the box's
    /// corners and sides (0 top left, then clockwise).
    pub fn pending_handles(&self) -> Vec<Pt> {
        let Some(p) = self.pending else { return vec![] };
        if p.tool == Tool::Line {
            return vec![p.a, p.b];
        }
        let (x0, y0, x1, y1) = p.bbox();
        let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        vec![(x0, y0), (mx, y0), (x1, y0), (x1, my), (x1, y1), (mx, y1), (x0, y1), (x0, my)]
    }

    fn pending_press(&mut self, p: Pt, x: f32, y: f32) -> bool {
        let Some(pend) = self.pending else { return false };
        let slop = 6.0;
        for (i, h) in self.pending_handles().iter().enumerate() {
            let s = self.to_screen(*h);
            if (s.0 - x).abs() <= slop && (s.1 - y).abs() <= slop {
                self.drag = Some(Drag::PendResize { handle: i, orig: pend });
                return true;
            }
        }
        let (x0, y0, x1, y1) = pend.bbox();
        let m = slop / self.zoom;
        if p.0 >= x0 - m && p.0 <= x1 + m && p.1 >= y0 - m && p.1 <= y1 + m {
            self.drag = Some(Drag::PendMove { start: p, orig: pend });
            return true;
        }
        false
    }

    /// How a new selection combines with the one there: Shift adds, Alt takes away, both intersect.
    fn sel_mode(&self) -> u8 {
        match (self.mods.shift, self.mods.alt) {
            (true, true) => 3,
            (true, false) => 1,
            (false, true) => 2,
            _ => 0,
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
        match id {
            // these change the pending shape rather than keep it
            "shape.fill" => {
                self.shape_fill = !self.shape_fill;
                self.redraw_pending();
                return;
            }
            _ => self.commit_pending(),
        }
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
            "delete" if self.tool == Tool::EmbSelect && self.emb_sel.is_some() => self.delete_emb_object(),
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
            "adj.grey" | "adj.sepia" => {
                use folio_draw_engine::adjust;
                self.doc.adjust(if id == "adj.grey" { adjust::GREY } else { adjust::SEPIA }, 0.0, 0.0, 0.0);
                self.doc.label_last(if id == "adj.grey" { "Greyscale" } else { "Sepia" });
            }
            "adj.bc" => self.dialog = Some(Dialog::new("adj.bc", "Brightness and contrast").text("a", "Brightness (-100 to 100)", "0").text("b", "Contrast (-100 to 100)", "0").button("ok", "OK").button("cancel", "Cancel")),
            "adj.hue" => {
                self.dialog = Some(
                    Dialog::new("adj.hue", "Hue and saturation")
                        .text("a", "Hue (-180 to 180 degrees)", "0")
                        .text("b", "Saturation (-100 to 100)", "0")
                        .text("c", "Lightness (-100 to 100)", "0")
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                )
            }
            "adj.blur" => self.dialog = Some(Dialog::new("adj.blur", "Blur").text("a", "Radius (pixels, 1 to 50)", "2").button("ok", "OK").button("cancel", "Cancel")),
            "adj.sharpen" => self.dialog = Some(Dialog::new("adj.sharpen", "Sharpen").text("a", "Amount (1 to 500)", "100").button("ok", "OK").button("cancel", "Cancel")),
            "rotate.angle" => self.dialog = Some(Dialog::new("rotate.angle", "Rotate").text("deg", "Angle (degrees, clockwise)", "15").check("fill", "Fill the corners with colour 2", true).button("ok", "OK").button("cancel", "Cancel")),
            "skew" => self.dialog = Some(Dialog::new("skew", "Skew").text("x", "Horizontal (degrees, -89 to 89)", "0").text("y", "Vertical (degrees, -89 to 89)", "0").button("ok", "OK").button("cancel", "Cancel")),
            "canvas-size" => {
                self.dialog = Some(
                    Dialog::new("canvas-size", "Canvas size")
                        .text("w", "Width (pixels)", &self.doc.w.to_string())
                        .text("h", "Height (pixels)", &self.doc.h.to_string())
                        .choice("anchor", "Picture sits", &["Top left", "Top", "Top right", "Left", "Centre", "Right", "Bottom left", "Bottom", "Bottom right"], 4)
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                )
            }
            "select-transparent" => self.transparent_sel = !self.transparent_sel,
            "select-sample-all" => self.doc.sample_all = !self.doc.sample_all,
            "stabilizer" => {
                use crate::stabilizer::NAMES;
                let v = self.stab[self.stab_slot()];
                let who = match self.stab_slot() {
                    1 => "the brushes",
                    2 => "the eraser",
                    _ => "the pencil",
                };
                self.dialog = Some(
                    Dialog::new("stabilizer", &format!("Stabilizer for {who}"))
                        .note("Each is 0 to 100. They steady a hand-drawn line before it reaches the picture.")
                        .text("a", &format!("{} (trails behind the pointer)", NAMES[0]), &v[0].to_string())
                        .text("b", &format!("{} (averages the last few points)", NAMES[1]), &v[1].to_string())
                        .text("c", &format!("{} (ignores small shakes)", NAMES[2]), &v[2].to_string())
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                )
            }
            "wand-settings" => {
                self.dialog = Some(
                    Dialog::new("wand-settings", "Magic wand")
                        .text("tol", "Tolerance (0 to 255)", &format!("{}", self.wand_tol.round() as i32))
                        .check("cont", "One patch only (Ctrl while clicking does the opposite)", self.wand_contiguous)
                        .check("all", "Sample all layers", self.doc.sample_all)
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                )
            }
            "remove-bg" => {
                self.doc.commit_float();
                if self.doc.remove_background() {
                    self.doc.label_last("Remove background");
                } else {
                    self.flash("No background could be told apart from the subject.");
                }
            }
            "select-invert" => {
                self.doc.commit_float();
                self.doc.sel_invert();
            }
            "select-none" => {
                self.doc.commit_float();
                self.doc.sel_none();
            }
            "layer.add" => {
                self.doc.commit_float();
                let n = self.doc.layers.len() + 1;
                self.doc.layer_add(&format!("Layer {n}"));
                self.show_layers = true;
            }
            "layer.dup" => {
                self.doc.layer_duplicate();
            }
            "layer.delete" => {
                if self.doc.layers.len() > 1 {
                    self.doc.layer_delete();
                }
            }
            "layer.up" => {
                let a = self.doc.active;
                if a + 1 < self.doc.layers.len() {
                    self.doc.layer_move(a, a + 1);
                }
            }
            "layer.down" => {
                let a = self.doc.active;
                if a > 0 {
                    self.doc.layer_move(a, a - 1);
                }
            }
            "layer.merge" => {
                self.doc.layer_merge_down();
            }
            "layer.props" => {
                let a = self.doc.active;
                if let Some(l) = self.doc.layers.get(a) {
                    self.dialog = Some(
                        Dialog::new("layer.props", "Layer properties")
                            .text("name", "Name", &l.name)
                            .text("opacity", "Opacity (0 to 100 %)", &format!("{}", (l.opacity * 100.0).round()))
                            .choice("blend", "Blend mode", &BLENDS, l.blend as usize)
                            .check("visible", "Visible", l.visible)
                            .button("ok", "OK")
                            .button("cancel", "Cancel"),
                    );
                }
            }
            "paste-layer" => {
                if let Some(img) = host.clipboard_image() {
                    self.doc.commit_float();
                    let n = self.doc.layers.len() + 1;
                    self.doc.layer_add(&format!("Layer {n}"));
                    self.paste_rgba(img.w as usize, img.h as usize, img.rgba);
                    self.doc.commit_float();
                    let m = self.doc.undo_mark().saturating_sub(2);
                    self.doc.group_since(m, "Paste as new layer");
                    self.show_layers = true;
                }
            }
            "layer.flatten" => {
                self.doc.flatten();
            }
            "pane.layers" => self.show_layers = !self.show_layers,
            "view.grid" => self.gridlines = !self.gridlines,
            "quick.menu" => {
                let on: Vec<String> = self.title.quick.iter().map(|q| q.0.clone()).collect();
                let items = self
                    .quick_choices()
                    .into_iter()
                    .map(|(id, label)| MenuItem::new(&format!("quick:{id}"), label).check(on.iter().any(|q| q == id)))
                    .collect();
                let r = self.title.rect;
                self.menu = Some(Menu::new("quick", Rect::new(r.x + 40.0 * self.s, r.bottom(), 0.0, 0.0), items));
            }
            "search" => self.title.focus_search(),
            "page-setup" => {
                let papers: Vec<&str> = crate::page::PAPERS.iter().map(|p| p.0).collect();
                let pg = self.page.clone();
                let m = pg.margins;
                self.dialog = Some(
                    Dialog::new("page-setup", "Page setup")
                        .note(&crate::page::describe(&pg, self.doc.w, self.doc.h))
                        .choice("paper", "Paper", &papers, pg.paper)
                        .choice("orientation", "Orientation", &["Portrait", "Landscape"], pg.landscape as usize)
                        .text("margins", "Margins (mm: top, right, bottom, left)", &format!("{}, {}, {}, {}", m[0], m[1], m[2], m[3]))
                        .check("ch", "Centre across the page", pg.centre_h)
                        .check("cv", "Centre down the page", pg.centre_v)
                        .choice("mode", "Size", &["Fit to the page", "A percentage of normal size"], (!pg.fit) as usize)
                        .text("percent", "Percentage", &format!("{}", pg.percent.round() as i32))
                        .text("sheets", "Fit to this many sheets (across by down)", &format!("{} x {}", pg.fit_w, pg.fit_h))
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "print" => {
                let printers = folio_kit::platform::printers();
                if printers.is_empty() {
                    self.flash("No printer is set up on this computer.");
                    return;
                }
                let names: Vec<&str> = printers.iter().map(|p| p.as_str()).collect();
                self.printers = printers.clone();
                self.dialog = Some(
                    Dialog::new("print", "Print")
                        .note(&crate::page::describe(&self.page, self.doc.w, self.doc.h))
                        .choice("printer", "Printer", &names, 0)
                        .text("copies", "Copies", "1")
                        .button("ok", "Print")
                        .button("setup", "Page setup...")
                        .button("cancel", "Cancel"),
                );
            }
            "full-screen" => {
                self.full_screen = !self.full_screen;
                host.set_fullscreen(self.full_screen);
                self.ribbon.collapsed = self.full_screen;
            }
            "emb.stitchout" => {
                if self.stitch_out.take().is_some() {
                    self.stitch_due = None;
                    self.doc.mark_all();
                    return;
                }
                match crate::emb::make_plan(&self.doc) {
                    Some(p) if crate::emb::command_count(&p) > 1 => {
                        self.stitch_view = true;
                        self.stitch_out = Some(0);
                        self.stitch_due = Some(self.now);
                    }
                    Some(_) => self.flash("There are no stitches to sew yet."),
                    None => self.flash("This picture isn't an embroidery design."),
                }
            }
            "emb.density" => {
                if crate::emb::settings(&self.doc).is_none() {
                    self.flash("This picture isn't an embroidery design.");
                    return;
                }
                self.density_view = !self.density_view;
                if self.density_view {
                    match crate::emb::make_plan(&self.doc) {
                        Some(p) => {
                            let (covered, three, four) = crate::emb::density_stats(&self.doc, &p);
                            let pct = |n: usize| if covered == 0 { 0.0 } else { n as f32 * 100.0 / covered as f32 };
                            self.flash(&format!(
                                "Density: {:.0}% of the stitched area has three layers of thread, {:.0}% four or more.",
                                pct(three),
                                pct(four)
                            ));
                        }
                        None => self.flash("Nothing to measure yet."),
                    }
                }
            }
            "emb.view" => {
                self.stitch_view = !self.stitch_view;
            }
            "emb.info" => {
                match crate::emb::make_plan(&self.doc) {
                    Some(p) => {
                        let (x0, y0, x1, y1) = p.bounds();
                        let size = if p.stitch_count() > 0 { format!("{:.1} × {:.1} mm", x1 - x0, y1 - y0) } else { "empty".into() };
                        let threads: Vec<String> = p.blocks.iter().map(|b| if b.thread.name.is_empty() { format!("#{:02X}{:02X}{:02X}", b.thread.rgb[0], b.thread.rgb[1], b.thread.rgb[2]) } else { b.thread.name.clone() }).collect();
                        let note = format!(
                            "{} stitches, {} colour changes, {} trims.\nDesign size: {size}.\nThreads in order: {}.",
                            p.stitch_count(),
                            p.blocks.len().saturating_sub(1),
                            p.count(folio_draw_engine::emb::plan::TRIM),
                            if threads.is_empty() { "none".to_string() } else { threads.join(", ") }
                        );
                        self.dialog = Some(Dialog::new("info", "Stitch information").note(&note).button("ok", "Close"));
                    }
                    None => self.flash("This picture isn't an embroidery design."),
                }
            }
            "emb.import" => self.open_back(BackPage::ImportStitches),
            "emb.stitch" => {
                let Some((li, id)) = self.emb_sel else {
                    self.flash("Select a shape first.");
                    return;
                };
                let o = crate::emb::layer_spec(&self.doc, li).and_then(|sp| sp["objects"].as_array().and_then(|a| a.iter().find(|o| o["id"].as_i64() == Some(id)).cloned())).unwrap_or_default();
                let st = &o["stitch"];
                let t = st["type"].as_str().unwrap_or("auto");
                let ti = STITCH_TYPES.iter().position(|(k, _)| *k == t).unwrap_or(0);
                let ang = st["angle"].as_f64().map(|a| format!("{a}")).unwrap_or_default();
                self.dialog = Some(
                    Dialog::new("emb.stitch", "Stitch properties")
                        .choice("type", "Stitch type", &STITCH_TYPES.iter().map(|(_, l)| *l).collect::<Vec<_>>(), ti)
                        .text("angle", "Fill angle (degrees; empty = automatic)", &ang)
                        .text("spacing", "Row spacing (mm)", &format!("{}", st["spacing"].as_f64().unwrap_or(0.35)))
                        .text("length", "Stitch length (mm)", &format!("{}", st["length"].as_f64().unwrap_or(3.5)))
                        .text("pull", "Pull compensation (mm)", &format!("{}", st["pull"].as_f64().unwrap_or(0.2)))
                        .check("underlay", "Underlay", st["underlay"].as_bool().unwrap_or(true))
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "emb.hoop" => {
                let Some(stt) = crate::emb::settings(&self.doc) else {
                    self.flash("This picture isn't an embroidery design.");
                    return;
                };
                let cur = crate::emb::HOOPS.iter().position(|h| stt["hoop"].as_str() == Some(h.0)).unwrap_or(0);
                self.dialog = Some(
                    Dialog::new("emb.hoop", "Hoop and machine")
                        .choice("hoop", "Hoop", &crate::emb::HOOPS.iter().map(|h| h.0).collect::<Vec<_>>(), cur)
                        .text("machine", "Machine", stt["machine"].as_str().unwrap_or(""))
                        .text("trim", "Trim jumps longer than (mm)", &format!("{}", stt["trim"].as_f64().unwrap_or(2.0)))
                        .check("ties", "Tie in and tie off", stt["ties"].as_bool().unwrap_or(true))
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "emb.letter" => {
                if crate::emb::settings(&self.doc).is_none() {
                    self.flash("Start an embroidery design first (New embroidery).");
                    return;
                }
                let fonts: Vec<&str> = self.fonts.iter().map(|s| s.as_str()).collect();
                let at = self.fonts.iter().position(|f| *f == self.letter_font).unwrap_or(0);
                self.dialog = Some(
                    Dialog::new("emb.letter", "Lettering")
                        .text("text", "Words", "")
                        .choice("font", "Font", &fonts, at)
                        .text("cap", "Letter height (mm, a capital)", "10")
                        .text("spacing", "Extra space between letters (mm)", "0")
                        .choice("arc", "Set on", &["A straight line", "An arc, letters outwards", "An arc, letters inwards"], 0)
                        .text("radius", "Arc radius (mm)", "40")
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "emb.envelope" => {
                if self.emb_sel.is_none() {
                    self.flash("Select shapes or lettering first.");
                    return;
                }
                let kinds: Vec<&str> = crate::letters::ENVELOPES.iter().map(|e| e.1).collect();
                self.dialog = Some(
                    Dialog::new("emb.envelope", "Envelope")
                        .note("Bends the selected shapes. None puts lettering back as it was typed.")
                        .choice("kind", "Shape", &kinds, 0)
                        .text("amount", "Amount (per cent)", "30")
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "emb.place" => {
                if crate::emb::settings(&self.doc).is_none() {
                    self.flash("Start an embroidery design first (New embroidery).");
                } else {
                    self.open_back(BackPage::PlacePicture);
                }
            }
            "emb.trace" => match crate::emb::convert_layer(&mut self.doc) {
                Ok((n, c)) => self.flash(&format!("{n} shapes in {c} colours, one layer per colour.")),
                Err(e) => self.flash(&e),
            },
            id if id.starts_with("emb.new:") => {
                let hoop = id[8..].parse::<usize>().unwrap_or(0);
                self.request(After::NewEmb(hoop));
            }
            id if id.starts_with("emb.export:") => {
                if crate::emb::settings(&self.doc).is_none() {
                    self.flash("This picture isn't an embroidery design.");
                } else {
                    self.export_ext = id[11..].to_string();
                    self.open_back(BackPage::ExportStitches);
                }
            }
            "frames" => {
                self.doc.solo = !self.doc.solo;
                self.playing = None;
                self.doc.mark_all();
            }
            "frame.next" | "frame.prev" => {
                let n = self.doc.layers.len();
                if n > 1 {
                    self.doc.solo = true;
                    let a = self.doc.active;
                    let to = if id == "frame.next" { (a + 1) % n } else { (a + n - 1) % n };
                    self.doc.set_active(to);
                    self.doc.mark_all();
                }
            }
            "frame.play" => {
                if self.playing.is_some() {
                    self.playing = None;
                } else if self.doc.layers.len() > 1 {
                    self.doc.solo = true;
                    self.doc.mark_all();
                    self.playing = Some(self.now + self.frame_delay());
                }
            }
            "view.rulers" => {
                self.rulers = !self.rulers;
                self.layout();
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
            "paste-file" => self.open_back(BackPage::PasteFrom),
            "close" => self.request(After::Close),
            _ => {}
        }
    }

    /// Typed text drawn onto the active layer in colour 1, its top left at `at`.
    pub fn put_text(&mut self, t: &str, at: Pt, size: f32, bold: bool, italic: bool, r: &mut Res) {
        let family = self.text_fonts().get(self.text_font).cloned().unwrap_or_default();
        let st = folio_kit::text::TextStyle {
            family: family.clone(),
            size,
            bold,
            italic,
            underline: self.text_underline,
            color: Color::rgba(self.c1[0], self.c1[1], self.c1[2], self.c1[3]),
            align: Align::Left,
            line: 1.2,
        };
        let lay = r.text.layout(t, &st, None);
        let (w, h) = ((lay.width.ceil() as u32 + 4).max(1), (lay.height.ceil() as u32 + 4).max(1));
        let Some(mut pm) = sk::Pixmap::new(w, h) else { return };
        if self.text_opaque {
            pm.fill(Color::rgba(self.c2[0], self.c2[1], self.c2[2], self.c2[3]).to_sk());
        }
        r.text.draw(&mut pm, t, &st, 2.0, 2.0, None);
        // the engine takes premultiplied BGRA
        let mut data = pm.take();
        for p in data.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        let (x, y) = (at.0 as i64, at.1 as i64);
        // what the GTK version keeps too, so either can type it again
        let spec = serde_json::json!({
            "text": t, "x": x, "y": y, "w": w, "h": h,
            "color1": self.c1, "color2": self.c2,
            "font_family": family, "font_size": size, "bold": bold, "italic": italic,
            "underline": self.text_underline, "strike": false, "text_opaque": self.text_opaque,
        })
        .to_string();
        match self.text_edit.take() {
            Some(i) if i < self.doc.layers.len() && !self.doc.layers[i].text.is_empty() => {
                self.doc.text_layer_set(i, &spec, Some((x, y, w as usize, h as usize, &data, w as usize * 4)));
            }
            _ => {
                self.doc.text_layer_add("Text", &spec, x, y, w as usize, h as usize, &data, w as usize * 4);
            }
        }
        self.doc.label_last("Text");
    }

    /// The topmost visible text layer whose box holds the point, with its description.
    pub fn text_layer_at(&self, p: Pt) -> Option<(usize, serde_json::Value)> {
        for (i, l) in self.doc.layers.iter().enumerate().rev() {
            if !l.visible || l.text.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&l.text) else { continue };
            let n = |k: &str| v[k].as_f64().unwrap_or(0.0) as f32;
            if p.0 >= n("x") && p.1 >= n("y") && p.0 < n("x") + n("w") && p.1 < n("y") + n("h") {
                return Some((i, v));
            }
        }
        None
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
                    MenuItem::sep(),
                    MenuItem::new("rotate.angle", "Rotate by angle..."),
                    MenuItem::new("skew", "Skew..."),
                    MenuItem::new("canvas-size", "Canvas size..."),
                ],
            ),
            "tool.select" => Menu::new(
                "select",
                rect,
                vec![
                    MenuItem::new("tool.select", "Rectangular selection").check(self.tool == Tool::Select),
                    MenuItem::new("tool.freeselect", "Free-form selection").check(self.tool == Tool::FreeSelect),
                    MenuItem::new("tool.wand", "Magic wand").check(self.tool == Tool::Wand),
                    MenuItem::sep(),
                    MenuItem::new("select-all", "Select all").key("Ctrl+A"),
                    MenuItem::new("select-invert", "Invert selection"),
                    MenuItem::new("select-none", "Select none"),
                    MenuItem::new("select-transparent", "Transparent selection").check(self.transparent_sel),
                    MenuItem::new("select-sample-all", "Sample all layers").check(self.doc.sample_all),
                    MenuItem::new("wand-settings", "Magic wand..."),
                    MenuItem::new("delete", "Delete").key("Delete"),
                ],
            ),
            "tool.brush" => Menu::new(
                "brush",
                rect,
                [(doc::TOOL_BRUSH, "Brush"), (doc::TOOL_CALLIGRAPHY, "Calligraphy brush"), (doc::TOOL_CALLIGRAPHY2, "Calligraphy pen"), (doc::TOOL_AIRBRUSH, "Airbrush"), (doc::TOOL_MARKER, "Marker")]
                    .iter()
                    .map(|(k, n)| MenuItem::new(&format!("brush:{k}"), n).check(self.brush == *k))
                    .chain([MenuItem::sep(), MenuItem::new("stabilizer", "Stabilizer...")])
                    .collect(),
            ),
            "adjust" => Menu::new(
                "adjust",
                rect,
                vec![
                    MenuItem::new("adj.bc", "Brightness and contrast..."),
                    MenuItem::new("adj.hue", "Hue and saturation..."),
                    MenuItem::new("adj.grey", "Greyscale"),
                    MenuItem::new("adj.sepia", "Sepia"),
                    MenuItem::sep(),
                    MenuItem::new("adj.blur", "Blur..."),
                    MenuItem::new("adj.sharpen", "Sharpen..."),
                    MenuItem::sep(),
                    MenuItem::new("remove-bg", "Remove background"),
                ],
            ),
            "paste" => Menu::new("paste", rect, vec![MenuItem::new("paste", "Paste").key("Ctrl+V"), MenuItem::new("paste-layer", "Paste as new layer"), MenuItem::new("paste-file", "Paste from file...")]),
            "emb.new" => Menu::new("emb.new", rect, crate::emb::HOOPS.iter().enumerate().map(|(i, h)| MenuItem::new(&format!("emb.new:{i}"), h.0)).collect()),
            "emb.recolour" => Menu::new("emb.recolour", rect, vec![]).with_palette(PALETTE.iter().map(|&c| Color::hex(c)).collect(), 10),
            "emb.export" => Menu::new("emb.export", rect, crate::emb::FORMATS.iter().map(|(e, l)| MenuItem::new(&format!("emb.export:{e}"), l)).collect()),
            "frame.loop" => Menu::new(
                "frame.loop",
                rect,
                GIF_LOOPS.iter().map(|(v, n)| MenuItem::new(&format!("frame.loop:{v}"), n).check(self.doc.gif_loop == *v)).collect(),
            ),
            "size" => Menu::new("size", rect, SIZES.iter().map(|s| MenuItem::new(&format!("size:{s}"), &format!("{s} px")).check((self.size - s).abs() < 0.01)).collect()),
            "color1" | "color2" => Menu::new(id, rect, vec![MenuItem::new(&format!("edit-color:{id}"), "More colours...")]).with_palette(PALETTE.iter().map(|&c| Color::hex(c)).collect(), 10),
            _ => return,
        };
        self.menu = Some(menu);
    }

    /// Treat the picture as saved, so a test can open another one without the prompt.
    pub fn saved_for_test(&mut self) {
        self.saved_changes = self.doc.changes;
    }

    pub fn menu_pick_for_test(&mut self, menu: &str, id: &str) {
        self.menu_pick(menu, id);
    }

    fn menu_pick(&mut self, menu: &str, id: &str) {
        if let Some(c) = id.strip_prefix("color:").and_then(Color::parse) {
            let v = [c.r, c.g, c.b, 255];
            if menu == "emb.recolour" {
                let rgb = [c.r, c.g, c.b];
                self.edit_emb_object("Thread colour", |o| {
                    if o["kind"] == "stitches" {
                        if let Some(bs) = o["blocks"].as_array_mut() {
                            for b in bs {
                                b["thread"] = serde_json::json!({"rgb": rgb});
                            }
                        }
                    } else {
                        o["thread"] = serde_json::json!({"rgb": rgb});
                    }
                });
                return;
            }
            if menu == "color2" {
                self.c2 = v;
            } else {
                self.c1 = v;
                if let Some(p) = self.pending.as_mut() {
                    p.color = v;
                }
            }
            self.redraw_pending();
            return;
        }
        if let Some(which) = id.strip_prefix("edit-color:") {
            let c = if which == "color2" { self.c2 } else { self.c1 };
            let hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
            self.dialog = Some(
                Dialog::new(&format!("color:{which}"), "Edit colour")
                    .text("hex", "Colour (#RRGGBB)", &hex)
                    .text("alpha", "Opacity (0 to 255)", &c[3].to_string())
                    .button("ok", "OK")
                    .button("cancel", "Cancel"),
            );
            return;
        }
        if let Some(which) = id.strip_prefix("quick:") {
            let icons: Vec<(&str, &str, &str)> = vec![
                ("save", "save", "Save (Ctrl+S)"),
                ("undo", "undo", "Undo (Ctrl+Z)"),
                ("redo", "redo", "Redo (Ctrl+Y)"),
                ("new", "save", "New picture (Ctrl+N)"),
                ("open", "save", "Open (Ctrl+O)"),
                ("crop", "crop", "Crop"),
                ("resize", "resize", "Resize"),
                ("remove-bg", "nobg", "Remove background"),
                ("pane.layers", "layers", "Layers pane (F7)"),
            ];
            if let Some(at) = self.title.quick.iter().position(|q| q.0 == which) {
                if self.title.quick.len() > 1 {
                    self.title.quick.remove(at);
                }
            } else if let Some((id, icon, tip)) = icons.iter().find(|(i, _, _)| *i == which) {
                // keep them in the order they are offered in
                let order: Vec<&str> = self.quick_choices().iter().map(|(i, _)| *i).collect();
                let at = order.iter().position(|o| *o == *id).unwrap_or(usize::MAX);
                let pos = self
                    .title
                    .quick
                    .iter()
                    .position(|q| order.iter().position(|o| *o == q.0).unwrap_or(usize::MAX) > at)
                    .unwrap_or(self.title.quick.len());
                self.title.quick.insert(pos, ((*id).into(), (*icon).into(), (*tip).into()));
            }
            self.settings.quick = self.title.quick.iter().map(|q| q.0.clone()).collect();
            self.settings.save();
            return;
        }
        if let Some(v) = id.strip_prefix("frame.loop:").and_then(|s| s.parse::<i32>().ok()) {
            if self.doc.gif_loop != v {
                self.doc.gif_loop = v;
                self.saved_changes = u64::MAX;      // a setting of the file: it can be saved again
            }
            return;
        }
        if let Some(k) = id.strip_prefix("brush:").and_then(|s| s.parse::<u8>().ok()) {
            self.brush = k;
            self.set_tool(Tool::Brush);
            return;
        }
        if let Some(s) = id.strip_prefix("size:").and_then(|s| s.parse::<f32>().ok()) {
            self.size = s;
            self.redraw_pending();
            return;
        }
        let mut h = folio_kit::platform::Headless::default();
        self.command(id, &mut h);
    }

    fn dialog_done(&mut self, d: Dialog, button: &str, host: &mut dyn Host, r: &mut Res) {
        match (d.id.as_str(), button) {
            ("adj.bc" | "adj.hue" | "adj.blur" | "adj.sharpen", "ok") => {
                use folio_draw_engine::adjust;
                let num = |id: &str, lo: f32, hi: f32| d.value(id).trim().replace(',', ".").parse::<f32>().unwrap_or(0.0).clamp(lo, hi);
                let (kind, a, b, c, label) = match d.id.as_str() {
                    "adj.bc" => (adjust::BRIGHT_CONTRAST, num("a", -100.0, 100.0), num("b", -100.0, 100.0), 0.0, "Brightness and contrast"),
                    "adj.hue" => (adjust::HUE_SAT, num("a", -180.0, 180.0), num("b", -100.0, 100.0), num("c", -100.0, 100.0), "Hue and saturation"),
                    "adj.blur" => (adjust::BLUR, num("a", 1.0, 50.0), 0.0, 0.0, "Blur"),
                    _ => (adjust::SHARPEN, num("a", 1.0, 500.0), 0.0, 0.0, "Sharpen"),
                };
                self.doc.adjust(kind, a, b, c);
                self.doc.label_last(label);
            }
            ("stabilizer", "ok") => {
                let n = |id: &str| d.value(id).trim().parse::<f32>().unwrap_or(0.0).clamp(0.0, 100.0).round() as u8;
                let slot = self.stab_slot();
                self.stab[slot] = [n("a"), n("b"), n("c")];
            }
            ("wand-settings", "ok") => {
                self.wand_tol = d.value("tol").trim().replace(',', ".").parse::<f32>().unwrap_or(self.wand_tol).clamp(0.0, 255.0);
                self.wand_contiguous = d.checked("cont");
                self.doc.sample_all = d.checked("all");
            }
            ("rotate.angle", "ok") => {
                let deg = d.value("deg").trim().replace(',', ".").parse::<f32>().unwrap_or(0.0);
                if deg.abs() > 0.001 {
                    let bg = if d.checked("fill") { Some(self.c2) } else { None };
                    if self.doc.float.is_some() {
                        self.doc.float_rotate(deg, true);
                    } else {
                        self.doc.image_rotate(deg, true, bg);
                        self.doc.label_last("Rotate");
                        self.fit();
                    }
                }
            }
            ("skew", "ok") => {
                let n = |id: &str| d.value(id).trim().replace(',', ".").parse::<f32>().unwrap_or(0.0).clamp(-89.0, 89.0);
                let (ax, ay) = (n("x"), n("y"));
                if ax != 0.0 || ay != 0.0 {
                    self.doc.commit_float();
                    self.doc.image_skew(ax, ay);
                    self.doc.label_last("Skew");
                    self.fit();
                }
            }
            ("canvas-size", "ok") => {
                let w: usize = d.value("w").trim().parse().unwrap_or(0);
                let h: usize = d.value("h").trim().parse().unwrap_or(0);
                let a = d.choice_index("anchor");
                let (ax, ay) = ((a % 3) as f32 / 2.0, (a / 3) as f32 / 2.0);
                if (1..=32768).contains(&w) && (1..=32768).contains(&h) {
                    self.doc.commit_float();
                    self.doc.canvas_size(w, h, ax, ay);
                    self.doc.label_last("Canvas size");
                    self.fit();
                } else {
                    self.flash("Width and height go from 1 to 32768 pixels.");
                }
            }
            (id, "ok") if id.starts_with("color:") => {
                if let Some(c) = Color::parse(d.value("hex").trim()) {
                    let a = d.value("alpha").trim().parse::<u8>().unwrap_or(255);
                    let v = [c.r, c.g, c.b, a];
                    if id == "color:color2" {
                        self.c2 = v;
                    } else {
                        self.c1 = v;
                    }
                } else {
                    self.flash("Type a colour as #RRGGBB.");
                }
            }
            ("emb.stitch", "ok") => {
                let num = |id: &str, lo: f64, hi: f64, def: f64| d.value(id).trim().replace(',', ".").parse::<f64>().map(|v| v.clamp(lo, hi)).unwrap_or(def);
                let ty = STITCH_TYPES[d.choice_index("type").min(STITCH_TYPES.len() - 1)].0;
                let angle = d.value("angle").trim().replace(',', ".").parse::<f64>().ok().map(|a| a.rem_euclid(360.0));
                let (sp, ln, pl, ul) = (num("spacing", 0.2, 2.0, 0.35), num("length", 1.0, 7.0, 3.5), num("pull", 0.0, 1.0, 0.2), d.checked("underlay"));
                self.edit_emb_object("Stitch properties", |o| {
                    let mut st = if o["stitch"].is_object() { o["stitch"].clone() } else { serde_json::json!({}) };
                    st["type"] = serde_json::json!(ty);
                    st["angle"] = angle.map(|a| serde_json::json!(a)).unwrap_or(serde_json::Value::Null);
                    st["spacing"] = serde_json::json!(sp);
                    st["length"] = serde_json::json!(ln);
                    st["pull"] = serde_json::json!(pl);
                    st["underlay"] = serde_json::json!(ul);
                    o["stitch"] = st;
                });
            }
            ("page-setup", "ok") => {
                let num = |id: &str, dflt: f32| d.value(id).trim().replace(',', ".").parse::<f32>().unwrap_or(dflt);
                let mut m = self.page.margins;
                let parts: Vec<f32> = d.value("margins").split(&[',', ';'][..]).filter_map(|v| v.trim().replace(',', ".").parse::<f32>().ok()).collect();
                for (i, v) in parts.iter().take(4).enumerate() {
                    m[i] = v.clamp(0.0, 100.0);
                }
                let sheets: Vec<u32> = d.value("sheets").split(|c: char| !c.is_ascii_digit()).filter_map(|v| v.parse::<u32>().ok()).collect();
                self.page = crate::page::Setup {
                    paper: d.choice_index("paper").min(crate::page::PAPERS.len() - 1),
                    landscape: d.choice_index("orientation") == 1,
                    margins: m,
                    centre_h: d.checked("ch"),
                    centre_v: d.checked("cv"),
                    fit: d.choice_index("mode") == 0,
                    percent: num("percent", 100.0).clamp(1.0, 2000.0),
                    fit_w: sheets.first().copied().unwrap_or(1).clamp(1, 20),
                    fit_h: sheets.get(1).copied().unwrap_or(1).clamp(1, 20),
                };
                self.settings.page = self.page.clone();
                self.settings.save();
                self.flash(&crate::page::describe(&self.page, self.doc.w, self.doc.h));
            }
            ("print", "setup") => self.command("page-setup", host),
            ("print", "ok") => {
                let copies = d.value("copies").trim().parse::<u32>().unwrap_or(1).clamp(1, 99);
                let printer = self.printers.get(d.choice_index("printer")).cloned();
                self.doc.commit_float();
                let px = self.doc.flatten_rgba();
                let (w, h) = (self.doc.w, self.doc.h);
                match crate::page::pdf_bytes(&px, w, h, &self.page) {
                    Ok(bytes) => {
                        let tmp = folio_kit::platform::temp_file("folio-draw-print", "pdf");
                        let res = std::fs::write(&tmp, &bytes)
                            .map_err(|e| e.to_string())
                            .and_then(|_| folio_kit::platform::print_pdf(&tmp, copies, printer.as_deref()));
                        match res {
                            Ok(()) => self.flash("Sent to the printer."),
                            Err(e) => self.flash(&format!("Printing failed: {e}")),
                        }
                    }
                    Err(e) => self.flash(&e),
                }
            }
            ("emb.letter", "ok") => {
                let words = d.value("text");
                if !words.trim().is_empty() && crate::emb::settings(&self.doc).is_some() {
                    let num = |id: &str, dflt: f32| d.value(id).trim().replace(',', ".").parse::<f32>().unwrap_or(dflt);
                    let cap = num("cap", 10.0).clamp(1.0, 200.0);
                    let font = self.fonts.get(d.choice_index("font")).cloned().unwrap_or_else(|| self.letter_font.clone());
                    self.letter_font = font.clone();
                    let arc = ["none", "top", "bottom"][d.choice_index("arc").min(2)];
                    let c = self.c1;
                    let mut o = serde_json::json!({
                        "kind": "lettering", "text": words, "font": font, "cap": cap,
                        "spacing": num("spacing", 0.0).clamp(-cap, cap * 4.0), "arc": arc,
                        "radius": num("radius", 40.0).max(1.0), "x": 0.0, "y": 0.0,
                        "thread": {"rgb": [c[0], c[1], c[2]]},
                        "stitch": {"type": crate::letters::default_type(cap)},
                    });
                    crate::letters::regen(&mut r.text, &mut o);
                    if o["glyphs"].as_array().map(|g| g.is_empty()).unwrap_or(true) {
                        self.flash("Those letters aren't in that font.");
                    } else {
                        self.doc.commit_float();
                        let layer = crate::emb::lettering_layer(&self.doc);
                        let li = crate::emb::add_object(&mut self.doc, layer, "Lettering", o);
                        self.doc.label_last("Lettering");
                        self.doc.active = li;
                    }
                }
            }
            ("emb.envelope", "ok") => {
                let kind = crate::letters::ENVELOPES[d.choice_index("kind").min(crate::letters::ENVELOPES.len() - 1)].0;
                let amount = d.value("amount").trim().replace(',', ".").parse::<f32>().unwrap_or(30.0).clamp(0.0, 100.0);
                let env = serde_json::json!({"kind": kind, "amount": amount});
                let text = &mut r.text;
                self.edit_emb_object("Envelope", |o| {
                    o["envelope"] = env.clone();
                    if o["kind"] == "lettering" {
                        crate::letters::regen(text, o);
                    } else if let Some(rings) = o["rings"].as_array().cloned() {
                        if o.get("rings_plain").is_none() {
                            o["rings_plain"] = serde_json::json!(rings);
                        }
                        let plain = o["rings_plain"].clone();
                        o["rings"] = crate::letters::warp_json(&plain, &env);
                    }
                });
            }
            ("emb.hoop", "ok") => {
                if let Some(mut stt) = crate::emb::settings(&self.doc) {
                    let (name, w, h) = crate::emb::HOOPS[d.choice_index("hoop").min(crate::emb::HOOPS.len() - 1)];
                    let old = crate::emb::canvas_px(&stt);
                    stt["hoop"] = serde_json::json!(name);
                    stt["hoop_mm"] = serde_json::json!([w, h]);
                    stt["machine"] = serde_json::json!(d.value("machine").trim());
                    stt["trim"] = serde_json::json!(d.value("trim").trim().replace(',', ".").parse::<f64>().unwrap_or(2.0).clamp(0.5, 20.0));
                    stt["ties"] = serde_json::json!(d.checked("ties"));
                    let new = crate::emb::canvas_px(&stt);
                    self.doc.commit_float();
                    let mark = self.doc.undo_mark();
                    // shapes are kept in mm from the hoop's centre: they come through a new hoop as they are
                    let specs: Vec<(usize, serde_json::Value)> = (0..self.doc.layers.len()).filter_map(|i| crate::emb::layer_spec(&self.doc, i).map(|sp| (i, sp))).collect();
                    if new != old {
                        // the hoop's centre stays the design's centre
                        self.doc.canvas_size(new.0, new.1, 0.5, 0.5);
                    }
                    self.doc.set_emb(&stt.to_string());
                    for (i, sp) in &specs {
                        crate::emb::store_layer(&mut self.doc, *i, sp);
                    }
                    self.doc.group_since(mark, "Hoop");
                    self.fit();
                }
            }
            ("layer.props", "ok") => {
                let a = self.doc.active;
                let op = d.value("opacity").trim().replace(',', ".").parse::<f32>().unwrap_or(100.0).clamp(0.0, 100.0) / 100.0;
                let delay = self.doc.layers.get(a).map(|l| l.delay_ms).unwrap_or(0);
                let name = d.value("name");
                let name = if name.trim().is_empty() { None } else { Some(name) };
                self.doc.layer_set_props(a, name, op, d.checked("visible"), d.choice_index("blend") as u8, delay, true);
                self.doc.label_last("Layer properties");
            }
            ("text", "ok") => {
                let t = d.value("text");
                self.text_size = d.choice_index("size");
                self.text_font = d.choice_index("font");
                self.text_underline = d.checked("underline");
                self.text_opaque = d.checked("opaque");
                if let (Some(at), false) = (self.text_at.take(), t.trim().is_empty()) {
                    self.put_text(&t, at, TEXT_SIZES[self.text_size.min(TEXT_SIZES.len() - 1)] as f32, d.checked("bold"), d.checked("italic"), r);
                }
            }
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
                    self.after = self.then.take();
                }
            }
            ("unsaved", "discard") => self.after = self.then.take(),
            ("unsaved", _) => self.then = None,
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
                self.settings.add_recent(p);
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

    /// The planned stitches written as a machine file (the format its name says).
    pub fn export_stitches(&mut self, p: &Path) -> Result<(), String> {
        self.commit_pending();
        let plan = crate::emb::make_plan(&self.doc).ok_or("This picture isn't an embroidery design.")?;
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "pes".into());
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let date = civil_date(secs);
        let bytes = crate::emb::write(&plan, &ext, &name, &date)?;
        folio_kit::platform::atomic_write(p, &bytes)?;
        self.flash(&format!("{} stitches saved to {}", plan.stitch_count(), p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
        Ok(())
    }

    /// A machine file's stitches as a stitch object on a new shape layer.
    pub fn import_stitches(&mut self, p: &Path) -> Result<(), String> {
        let data = std::fs::read(p).map_err(|e| e.to_string())?;
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let plan = folio_draw_engine::emb::formats::read_named(&data, &ext)?;
        if crate::emb::settings(&self.doc).is_none() {
            self.replace(crate::emb::new_doc(0), None);
        }
        let blocks: Vec<serde_json::Value> = plan
            .blocks
            .iter()
            .map(|b| {
                serde_json::json!({
                    "thread": {"rgb": b.thread.rgb, "name": b.thread.name},
                    "stop": b.stop,
                    "pts": b.cmds.iter().map(|c| serde_json::json!([c.0, c.1, c.2])).collect::<Vec<_>>(),
                })
            })
            .collect();
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Design".into());
        let spec = serde_json::json!({"objects": [{"id": 1, "kind": "stitches", "name": name, "blocks": blocks}], "next_id": 2});
        crate::emb::add_shape_layer(&mut self.doc, &name, &spec);
        self.doc.label_last("Import stitches");
        self.stitch_view = true;
        Ok(())
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
            self.then = Some(after);
        } else {
            self.after = Some(after);
        }
    }

    fn run_after(&mut self, host: &mut dyn Host) {
        let Some(a) = self.after.take() else { return };
        match a {
            After::Close => {
                self.remember();
                host.close();
                self.exit = true;
            }
            After::New => self.replace(Doc::new(1152, 648, [255, 255, 255, 255]), None),
            After::NewEmb(hoop) => {
                self.replace(crate::emb::new_doc(hoop), None);
                self.ribbon.select_tab("embroidery");
            }
            After::Open(p) => match Self::load(&p) {
                Ok(d) => {
                    self.settings.add_recent(&p);
                    self.settings.save();
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
            BackPage::Open | BackPage::PasteFrom | BackPage::PlacePicture => FilePane::new(&dir, &["png", "jpg", "jpeg", "bmp", "gif", "webp", "tif", "tiff", "ico", "ora", "psd"], false, ""),
            BackPage::ImportStitches => FilePane::new(&dir, &["pes", "dst", "jef", "exp", "vp3", "xxx", "pec"], false, ""),
            BackPage::ExportStitches => {
                let stem = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Design".into());
                let e = self.export_ext.clone();
                FilePane::new(&dir, &[e.as_str()], true, &format!("{stem}.{e}"))
            }
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
                    id if id.starts_with("recent:") => {
                        if let Some(p) = id[7..].parse::<usize>().ok().and_then(|i| self.settings.recent.get(i).cloned()) {
                            self.layout();
                            self.request(After::Open(PathBuf::from(p)));
                            return;
                        }
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
                BackPage::ExportStitches => match self.export_stitches(&p) {
                    Ok(()) => {
                        self.layout();
                        return;
                    }
                    Err(e) => b.files.error = e,
                },
                BackPage::PlacePicture => match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| files::decode(&b)) {
                    Ok((w, h, px)) => {
                        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Picture".into());
                        crate::emb::place_picture(&mut self.doc, w, h, px, &name);
                        self.doc.label_last("Place picture");
                        self.flash("Picture placed. Convert to shapes turns it into stitches.");
                        self.layout();
                        return;
                    }
                    Err(e) => b.files.error = e,
                },
                BackPage::ImportStitches => match self.import_stitches(&p) {
                    Ok(()) => {
                        self.layout();
                        return;
                    }
                    Err(e) => b.files.error = e,
                },
                BackPage::PasteFrom => {
                    match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| files::decode(&b)) {
                        Ok((w, h, px)) => {
                            self.layout();
                            self.paste_rgba(w, h, px);
                            return;
                        }
                        Err(e) => b.files.error = e,
                    }
                }
                BackPage::SaveAs => {
                    if self.save_to(&p) {
                        self.layout();
                        if self.then.is_some() {
                            self.after = self.then.take();
                        }
                        return;
                    }
                }
            }
        }
        self.back = Some(b);
    }

    pub fn side_hits_rect(&self, id: &str) -> Option<Rect> {
        self.side_hits.iter().find(|(i, _)| i == id).map(|(_, r)| *r)
    }

    fn side_click(&mut self, x: f32, y: f32, _button: Button) {
        let Some((id, _)) = self.side_hits.iter().find(|(_, r)| r.contains(x, y)).cloned() else { return };
        if let Some(i) = id.strip_prefix("eye:").and_then(|i| i.parse::<usize>().ok()) {
            if let Some(l) = self.doc.layers.get(i) {
                let (name, op, vis, blend, delay) = (None, l.opacity, !l.visible, l.blend, l.delay_ms);
                self.doc.layer_set_props(i, name, op, vis, blend, delay, true);
                self.doc.label_last(if vis { "Show layer" } else { "Hide layer" });
            }
        } else if let Some(i) = id.strip_prefix("row:").and_then(|i| i.parse::<usize>().ok()) {
            self.doc.set_active(i);
        } else {
            let mut h = folio_kit::platform::Headless::default();
            self.command(&id, &mut h);
        }
    }

    fn paint_side(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let side = self.side;
        if side.w <= 0.0 {
            return;
        }
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        gfx::fill_rect(cx.pm, side, theme.panel);
        gfx::vline(cx.pm, side.x, side.y, side.bottom(), theme.border);
        cx.label("Layers", Rect::new(side.x + 12.0 * s, side.y + 4.0 * s, side.w, 28.0 * s), 14.0, theme.text, true, Align::Left);
        self.side_hits.clear();
        let row_h = 48.0 * s;
        let n = self.doc.layers.len();
        let mut y = side.y + 34.0 * s;
        let (tw, th) = (56usize, 40usize);
        for vi in 0..n {
            let i = n - 1 - vi;
            if y + row_h > side.bottom() - 40.0 * s {
                break;
            }
            let row = Rect::new(side.x + 1.0, y, side.w - 2.0, row_h);
            if i == self.doc.active {
                gfx::fill_rect(cx.pm, row, theme.checked);
            }
            let eye = Rect::new(side.x + 8.0 * s, y + 14.0 * s, 20.0 * s, 20.0 * s);
            let vis = self.doc.layers[i].visible;
            gfx::stroke_round(cx.pm, eye, 3.0 * s, theme.text, 1.2);
            if vis {
                gfx::fill_round(cx.pm, eye.inset(5.0 * s), 2.0 * s, theme.text);
            }
            let tr = Rect::new(side.x + 36.0 * s, y + 4.0 * s, tw as f32 * s, th as f32 * s);
            gfx::fill_rect(cx.pm, tr, Color::WHITE);
            let mut t = self.doc.layer_thumb(i, tw, th);
            for p in t.chunks_exact_mut(4) {
                p.swap(0, 2);
            }
            if let Some(tp) = sk::Pixmap::from_vec(t, sk::IntSize::from_wh(tw as u32, th as u32).unwrap()) {
                let paint = sk::PixmapPaint { quality: sk::FilterQuality::Bilinear, ..Default::default() };
                cx.pm.draw_pixmap(0, 0, tp.as_ref(), &paint, sk::Transform::from_row(s, 0.0, 0.0, s, tr.x, tr.y), None);
            }
            gfx::stroke_round(cx.pm, tr, 0.0, theme.border, 1.0);
            let name = self.doc.layers[i].name.clone();
            cx.label(&name, Rect::new(tr.right() + 8.0 * s, y, side.right() - tr.right() - 12.0 * s, row_h), 13.0, theme.text, false, Align::Left);
            self.side_hits.push((format!("eye:{i}"), eye.inset(-4.0 * s)));
            self.side_hits.push((format!("row:{i}"), row));
            y += row_h;
        }
        let by = side.bottom() - 34.0 * s;
        let mut bx = side.x + 6.0 * s;
        for (id, icon) in [("layer.add", "layer-add"), ("layer.dup", "copy"), ("layer.delete", "trash"), ("layer.up", "frame-up"), ("layer.down", "frame-down"), ("layer.merge", "merge")] {
            let br = Rect::new(bx, by, 34.0 * s, 28.0 * s);
            gfx::fill_round(cx.pm, br, 3.0 * s, theme.window);
            gfx::stroke_round(cx.pm, br, 3.0 * s, theme.border, 1.0);
            cx.icon(icon, Rect::new(br.cx() - 9.0 * s, br.cy() - 9.0 * s, 18.0 * s, 18.0 * s));
            self.side_hits.push((id.to_string(), br));
            bx += 36.0 * s;
        }
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
            layers: self.show_layers,
            grid: self.gridlines,
            rulers: self.rulers,
            frames: self.doc.solo,
            playing: self.playing.is_some(),
            emb: !self.doc.emb.is_empty(),
            stitch_view: self.stitch_view,
            density_view: self.density_view,
            stitch_out: self.stitch_out.is_some(),
            full_screen: self.full_screen,
            emb_sel: self.emb_sel.is_some(),
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
        if self.fonts.is_empty() {
            self.fonts = crate::letters::fonts(&mut r.text);
        }
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
            self.paint_rulers(pm, r);
            self.paint_side(pm, r);
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
        if self.gridlines && self.zoom >= 2.0 {
            // a line round every pixel once they are big enough to see
            let c = Color::rgba(128, 128, 128, 90);
            let mut x = (self.scroll.0.max(0.0)).floor();
            while x <= self.doc.w as f32 && (x - self.scroll.0) * self.zoom < cw as f32 {
                let sx = ((x - self.scroll.0) * self.zoom).round() + 0.5;
                gfx::vline(&mut cv, sx, img.y.max(0.0), img.bottom().min(ch as f32), c);
                x += 1.0;
            }
            let mut y = (self.scroll.1.max(0.0)).floor();
            while y <= self.doc.h as f32 && (y - self.scroll.1) * self.zoom < ch as f32 {
                let sy = ((y - self.scroll.1) * self.zoom).round() + 0.5;
                gfx::hline(&mut cv, img.x.max(0.0), img.right().min(cw as f32), sy, c);
                y += 1.0;
            }
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
        if self.pending.is_some() {
            let hs: Vec<(f32, f32)> = self.pending_handles().iter().map(|h| to(*h)).collect();
            if let Some(p) = self.pending {
                if p.tool != Tool::Line {
                    let (x0, y0, x1, y1) = p.bbox();
                    let (q0, q1) = (to((x0, y0)), to((x1, y1)));
                    ants(&mut cv, Rect::new(q0.0, q0.1, q1.0 - q0.0, q1.1 - q0.1));
                }
            }
            for h in hs {
                let r = Rect::new(h.0 - 4.0, h.1 - 4.0, 8.0, 8.0);
                gfx::fill_rect(&mut cv, r, Color::WHITE);
                gfx::stroke_round(&mut cv, r, 0.0, ACCENT, 1.0);
            }
        }
        if let Some(Drag::Lasso(pts)) = &self.drag {
            let mut pb = sk::PathBuilder::new();
            for (i, p) in pts.iter().enumerate() {
                let q = to(*p);
                if i == 0 {
                    pb.move_to(q.0, q.1);
                } else {
                    pb.line_to(q.0, q.1);
                }
            }
            if let Some(path) = pb.finish() {
                let stroke = sk::Stroke { width: 1.0, dash: sk::StrokeDash::new(vec![4.0, 4.0], 0.0), ..Default::default() };
                cv.stroke_path(&path, &gfx::paint(Color::BLACK), &stroke, sk::Transform::identity(), None);
            }
        } else if let Some(Drag::Select { a, b }) = &self.drag {
            let (p, q) = (to(*a), to(*b));
            ants(&mut cv, Rect::new(p.0.min(q.0), p.1.min(q.1), (p.0 - q.0).abs(), (p.1 - q.1).abs()));
        } else if let Some(f) = &self.doc.float {
            let p = to((f.x as f32, f.y as f32));
            ants(&mut cv, Rect::new(p.0, p.1, f.w as f32 * self.zoom, f.h as f32 * self.zoom));
        } else if let Some((x, y, w, h)) = self.doc.sel_bounds() {
            let p = to((x as f32, y as f32));
            ants(&mut cv, Rect::new(p.0, p.1, w as f32 * self.zoom, h as f32 * self.zoom));
        }
        if let (Some((li, id)), Some(st)) = (self.emb_sel, crate::emb::settings(&self.doc)) {
            if let Some(o) = crate::emb::layer_spec(&self.doc, li).and_then(|sp| sp["objects"].as_array().and_then(|a| a.iter().find(|o| o["id"].as_i64() == Some(id)).cloned())) {
                let (mut x0, mut y0, mut x1, mut y1) = crate::emb::object_bounds(&o);
                if let Some(Drag::EmbMove { start, last }) = &self.drag {
                    let (dx, dy) = (last.0 - start.0, last.1 - start.1);
                    x0 += dx;
                    x1 += dx;
                    y0 += dy;
                    y1 += dy;
                }
                let a = crate::emb::mm_to_px(&st, x0, y0);
                let b = crate::emb::mm_to_px(&st, x1, y1);
                let (p, q) = (to(a), to(b));
                ants(&mut cv, Rect::new(p.0, p.1, q.0 - p.0, q.1 - p.1));
            }
        }
        if self.density_view {
            let ch = self.doc.changes;
            if self.density_cache.as_ref().map(|c| c.0 != ch).unwrap_or(true) {
                let map = crate::emb::make_plan(&self.doc).and_then(|p| crate::emb::density_rgba(&self.doc, &p));
                self.density_cache = map.map(|(w, h, px)| (ch, w, h, px));
            }
            if let Some((_, dw, dh, px)) = self.density_cache.clone() {
                if let Some(mut src) = sk::Pixmap::new(dw as u32, dh as u32) {
                    for (d, q) in src.pixels_mut().iter_mut().zip(px.chunks_exact(4)) {
                        *d = sk::PremultipliedColorU8::from_rgba(
                            (q[0] as u32 * q[3] as u32 / 255) as u8,
                            (q[1] as u32 * q[3] as u32 / 255) as u8,
                            (q[2] as u32 * q[3] as u32 / 255) as u8,
                            q[3],
                        )
                        .unwrap_or_else(|| sk::PremultipliedColorU8::from_rgba(0, 0, 0, 0).unwrap());
                    }
                    let t = sk::Transform::from_translate(-self.scroll.0 * self.zoom, -self.scroll.1 * self.zoom).pre_scale(self.zoom, self.zoom);
                    let pat = sk::Pattern::new(src.as_ref(), sk::SpreadMode::Pad, sk::FilterQuality::Nearest, 1.0, sk::Transform::identity());
                    let mut paint = sk::Paint::default();
                    paint.shader = pat;
                    cv.fill_rect(sk::Rect::from_xywh(0.0, 0.0, cv.width() as f32, cv.height() as f32).unwrap(), &paint, t, None);
                }
            }
        }
        if self.stitch_view {
            let ch = self.doc.changes;
            let mut needle = None;
            if let Some(upto) = self.stitch_out {
                let (lines, at) = crate::emb::make_plan(&self.doc).map(|p| crate::emb::stitch_lines_upto(&self.doc, &p, upto)).unwrap_or_default();
                self.stitch_cache = Some((u64::MAX, lines));
                needle = at;
            } else if self.stitch_cache.as_ref().map(|c| c.0 != ch).unwrap_or(true) {
                let lines = crate::emb::make_plan(&self.doc).map(|p| crate::emb::stitch_lines(&self.doc, &p)).unwrap_or_default();
                self.stitch_cache = Some((ch, lines));
            }
            if let Some(q) = needle {
                let t = ((q.0 - self.scroll.0) * self.zoom, (q.1 - self.scroll.1) * self.zoom);
                let rr = (6.0 * self.zoom).max(4.0);
                gfx::stroke_round(&mut cv, Rect::new(t.0 - rr, t.1 - rr, rr * 2.0, rr * 2.0), rr, Color::rgb(204, 0, 0), 2.0);
            }
            if let Some((_, lines)) = &self.stitch_cache {
                let w = (0.35 * crate::emb::PX_PER_MM * self.zoom).max(1.0);
                for (rgb, run) in lines {
                    let mut pb = sk::PathBuilder::new();
                    for (i, q) in run.iter().enumerate() {
                        let t = ((q.0 - self.scroll.0) * self.zoom, (q.1 - self.scroll.1) * self.zoom);
                        if i == 0 {
                            pb.move_to(t.0, t.1);
                        } else {
                            pb.line_to(t.0, t.1);
                        }
                    }
                    if let Some(path) = pb.finish() {
                        let stroke = sk::Stroke { width: w, line_cap: sk::LineCap::Round, line_join: sk::LineJoin::Round, ..Default::default() };
                        cv.stroke_path(&path, &gfx::paint(Color::rgb(rgb[0], rgb[1], rgb[2])), &stroke, sk::Transform::identity(), None);
                    }
                }
            }
        }
        gfx::blit_copy(pm, &cv, c.x as i32, c.y as i32);
    }

    fn paint_rulers(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        if !self.rulers {
            return;
        }
        let theme = self.theme.clone();
        let s = self.s;
        let c = self.canvas;
        let ru = 18.0 * s;
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        let top = Rect::new(c.x - ru, c.y - ru, c.w + ru, ru);
        let left = Rect::new(c.x - ru, c.y, ru, c.h);
        gfx::fill_rect(cx.pm, top, theme.panel);
        gfx::fill_rect(cx.pm, left, theme.panel);
        gfx::hline(cx.pm, top.x, top.right(), top.bottom() - 0.5, theme.border);
        gfx::vline(cx.pm, left.right() - 0.5, left.y, left.bottom(), theme.border);
        // a labelled tick about every 80 screen pixels, at a round number of picture pixels
        let raw = 80.0 / self.zoom;
        let mag = 10f32.dpow(raw.max(1.0).dln() / std::f32::consts::LN_10).floor().max(1.0);
        let step = [1.0, 2.0, 5.0, 10.0].iter().map(|k| k * mag).find(|&v| v >= raw).unwrap_or(10.0 * mag);
        let mut v = (self.scroll.0 / step).floor() * step;
        while (v - self.scroll.0) * self.zoom < c.w {
            let x = c.x + (v - self.scroll.0) * self.zoom;
            if x >= c.x {
                gfx::vline(cx.pm, x.round() + 0.5, top.y + ru * 0.4, top.bottom(), theme.muted);
                cx.label(&format!("{}", v as i64), Rect::new(x + 2.0 * s, top.y, 60.0 * s, ru * 0.7), 9.0, theme.muted, false, Align::Left);
            }
            v += step;
        }
        let mut v = (self.scroll.1 / step).floor() * step;
        while (v - self.scroll.1) * self.zoom < c.h {
            let y = c.y + (v - self.scroll.1) * self.zoom;
            if y >= c.y {
                gfx::hline(cx.pm, left.x + ru * 0.4, left.right(), y.round() + 0.5, theme.muted);
                cx.label(&format!("{}", v as i64), Rect::new(left.x + 1.0, y + 1.0 * s, ru, ru * 0.7), 8.0, theme.muted, false, Align::Left);
            }
            v += step;
        }
        // where the mouse is
        if c.contains(self.mouse.0, self.mouse.1) {
            gfx::vline(cx.pm, self.mouse.0.round() + 0.5, top.y, top.bottom(), theme.accent);
            gfx::hline(cx.pm, left.x, left.right(), self.mouse.1.round() + 0.5, theme.accent);
        }
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
        let heading = match b.page {
            BackPage::ExportStitches => "Export stitches",
            BackPage::ImportStitches => "Import stitches",
            BackPage::PlacePicture => "Place picture",
            BackPage::Open => "Open",
            BackPage::PasteFrom => "Paste from",
            BackPage::SaveAs => "Save As",
        };
        cx.label(heading, Rect::new(main.x + 32.0 * s, top + 16.0 * s, 400.0 * s, 40.0 * s), 26.0, theme.text, false, Align::Left);
        let mut fx = main.x + 32.0 * s;
        if b.page == BackPage::Open && !self.settings.recent.is_empty() {
            let rw = 320.0 * s;
            cx.label("Recent", Rect::new(fx, top + 70.0 * s, rw, 24.0 * s), 14.0, theme.text, true, Align::Left);
            let mut y = top + 100.0 * s;
            for (i, path) in self.settings.recent.iter().enumerate() {
                if y > self.h - 60.0 * s {
                    break;
                }
                let p = Path::new(path);
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let dir = p.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                let rr = Rect::new(fx, y, rw - 16.0 * s, 44.0 * s);
                cx.label(&name, Rect::new(rr.x + 6.0 * s, y + 2.0 * s, rr.w, 22.0 * s), 13.0, theme.text, false, Align::Left);
                cx.label(&dir, Rect::new(rr.x + 6.0 * s, y + 21.0 * s, rr.w, 18.0 * s), 11.0, theme.muted, false, Align::Left);
                b.hits.push((format!("recent:{i}"), rr));
                y += 46.0 * s;
            }
            fx += rw;
        }
        b.files.paint(&mut cx, Rect::new(fx, top + 70.0 * s, main.right() - fx - 32.0 * s, main.h - 90.0 * s));
    }
}

/// YYYYMMDDhhmmss from seconds since 1970 (UTC), for the files that record when they were made.
fn civil_date(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (z, era);
    z = days + 719468;
    era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}{m:02}{d:02}{:02}{:02}{:02}", rem / 3600, rem / 60 % 60, rem % 60)
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
    pub layers: bool,
    pub grid: bool,
    pub rulers: bool,
    pub frames: bool,
    pub playing: bool,
    pub emb: bool,
    pub stitch_view: bool,
    pub density_view: bool,
    pub stitch_out: bool,
    pub full_screen: bool,
    pub emb_sel: bool,
}

impl CommandState for St {
    fn enabled(&self, id: &str) -> bool {
        match id {
            "undo" => self.can_undo,
            "redo" => self.can_redo,
            "crop" => self.has_sel,
            "emb.view" | "emb.density" | "emb.stitchout" | "emb.info" | "emb.export" | "emb.place" | "emb.trace" | "tool.emb" => self.emb,
            "emb.recolour" | "emb.stitch" => self.emb_sel,
            "emb.hoop" => self.emb,
            _ => true,
        }
    }
    fn checked(&self, id: &str) -> bool {
        match id {
            "shape.fill" => self.shape_fill,
            "pane.layers" => self.layers,
            "view.grid" => self.grid,
            "view.rulers" => self.rulers,
            "frames" => self.frames,
            "emb.view" => self.stitch_view,
            "emb.density" => self.density_view,
            "emb.stitchout" => self.stitch_out,
            "full-screen" => self.full_screen,
            "frame.play" => self.playing,
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
        if let Some(due) = self.stitch_due {
            return Some(due.saturating_sub(self.now).max(Duration::from_millis(10)));
        }
        if let Some(due) = self.playing {
            return Some(due.saturating_sub(self.now).max(Duration::from_millis(10)));
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
