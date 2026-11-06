//! The Cork window: title bar, ribbon, Shapes pane, board, side panes, page tabs, File view.

use folio_kit::math::Det;
use crate::editor::{handle_points, Drag, Editor, Tool};
use crate::model::*;
use crate::render::{self, Images, View};
use crate::{export, files, icons, layout, mood, ribbon_defs, settings::Settings, stencils, templates};
use folio_kit::chrome::{self, TitleAction, TitleBar, TITLE_H};
use folio_kit::files::{FileAction, FilePane};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::platform::{Button, Cursor, Event, Host, Key, Mods};
use folio_kit::ribbon::{Ribbon, RibbonAction};
use folio_kit::text::{Align, Text, TextStyle};
use folio_kit::theme::{app_colors, Theme};
use folio_kit::ui::{standard_palette, CommandState, Ctx, Dialog, DialogResult, FieldResult, Menu, MenuItem, MenuResult, TextField};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ACCENT: Color = Color::hex(0xB8860B);

/// Things painting and events both need, taken out of the app while in use.
pub struct Res {
    pub text: Text,
    pub icons: Icons,
    pub images: Images,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Layers,
    Comments,
    Data,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackPage {
    New,
    Open,
    SaveAs,
    Export,
    Print,
    Options,
    About,
}

pub struct Backstage {
    pub page: BackPage,
    pub files: Option<FilePane>,
    pub export_fmt: usize,
    pub export_all: bool,
    pub export_scale: usize,
    pub print_fit: bool,
    pub print_all: bool,
    pub copies: u32,
    pub printer: usize,
    pub printers: Vec<String>,
    pub msg: String,
    hits: Vec<(String, Rect)>,
}

/// What to do once the user has dealt with unsaved changes.
#[derive(Clone, Debug)]
pub enum After {
    New(String),
    Open(PathBuf),
    Close,
}

pub struct CorkApp {
    pub ed: Editor,
    pub res: Option<Box<Res>>,
    pub theme: Theme,
    pub settings: Settings,
    pub ribbon: Ribbon,
    pub title: TitleBar,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub backstage: Option<Backstage>,
    pub mouse: (f32, f32),
    pub mods: Mods,
    pub space: bool,
    pub now: Duration,
    pub canvas: Rect,
    pub left: Rect,
    pub right: Rect,
    pub bar: Rect,
    cache: Option<CanvasCache>,
    pm: Option<sk::Pixmap>,
    tiles: Vec<(String, Rect)>,
    cats: Vec<(String, Rect)>,
    thumbs: HashMap<(String, u32, bool), sk::Pixmap>,
    mini: Option<((u64, usize, [u32; 5], bool), sk::Pixmap)>,
    pub search: TextField,
    pub collapsed: HashSet<String>,
    left_scroll: f32,
    pub side: Side,
    pub(crate) side_hits: Vec<(String, Rect)>,
    pub reply: TextField,
    bar_hits: Vec<(String, Rect)>,
    pub drag_tile: Option<String>,
    pub after: Option<After>,
    pub pending: Option<After>,
    pub exit: bool,
    pub code: i32,
    pub test: Option<crate::uitest::UiTest>,
    pub collab: Option<crate::collab::Collab>,
    pub flash: Option<(String, Duration)>,
    last_click_tab: Option<(usize, Duration)>,
    pub crop: Option<Id>,
}

impl CorkApp {
    pub fn new(doc: Document, path: Option<PathBuf>) -> CorkApp {
        let settings = Settings::load();
        let mut ic = Icons::new();
        icons::load(&mut ic);
        let res = Res { text: Text::new(), icons: ic, images: Images::default() };
        let mut ed = Editor::new(doc);
        ed.path = path;
        ed.grid = settings.grid;
        ed.snap = settings.snap;
        ed.snap_shapes = settings.guides;
        if !settings.author.is_empty() {
            ed.author = settings.author.clone();
        }
        let dark = settings.theme == "dark";
        let mut ribbon = Ribbon::new(ribbon_defs::tabs());
        ribbon.collapsed = settings.ribbon_collapsed;
        CorkApp {
            ed,
            res: Some(Box::new(res)),
            theme: Theme::new(dark, app_colors::CORK, ACCENT),
            settings,
            ribbon,
            title: TitleBar::new("cork", vec![("save", "save", "Save (Ctrl+S)"), ("undo", "undo", "Undo (Ctrl+Z)"), ("redo", "redo", "Redo (Ctrl+Y)")]),
            w: 1280.0,
            h: 800.0,
            s: 1.0,
            menu: None,
            dialog: None,
            backstage: None,
            mouse: (0.0, 0.0),
            mods: Mods::NONE,
            space: false,
            now: Duration::ZERO,
            canvas: Rect::default(),
            left: Rect::default(),
            right: Rect::default(),
            bar: Rect::default(),
            cache: None,
            pm: None,
            tiles: vec![],
            cats: vec![],
            thumbs: HashMap::new(),
            mini: None,
            search: TextField::new(""),
            collapsed: ["Org chart", "Network", "UML", "Database (ERD)", "Floor plan", "Electrical", "Electronic", "Block diagram"].iter().map(|s| s.to_string()).collect(),
            left_scroll: 0.0,
            side: Side::Layers,
            side_hits: vec![],
            reply: TextField::new(""),
            bar_hits: vec![],
            drag_tile: None,
            after: None,
            pending: None,
            exit: false,
            code: 0,
            test: None,
            collab: None,
            flash: None,
            last_click_tab: None,
            crop: None,
        }
    }

    pub fn doc_name(&self) -> String {
        self.ed.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Drawing1".into())
    }

    pub fn window_title(&self) -> String {
        format!("{}{} - Folio Cork", if self.ed.dirty { "*" } else { "" }, self.doc_name())
    }

    fn apply_theme(&mut self, host: &dyn Host) {
        let dark = match self.settings.theme.as_str() {
            "dark" => true,
            "light" => false,
            _ => host.system_dark().unwrap_or(false),
        };
        if dark != self.theme.dark {
            self.theme = Theme::new(dark, app_colors::CORK, ACCENT);
            self.cache = None;
            self.thumbs.clear();
        }
    }

    pub fn layout(&mut self) {
        let s = self.s;
        let top = TITLE_H * s + if self.backstage.is_some() { 0.0 } else { self.ribbon.height(s) };
        let bar_h = 30.0 * s;
        self.bar = Rect::new(0.0, self.h - bar_h, self.w, bar_h);
        let lw = if self.settings.shapes_pane { 236.0 * s } else { 0.0 };
        let rw = if self.settings.side_pane { 270.0 * s } else { 0.0 };
        self.left = Rect::new(0.0, top, lw, self.bar.y - top);
        self.right = Rect::new(self.w - rw, top, rw, self.bar.y - top);
        let ruler = if self.settings.rulers { 20.0 * s } else { 0.0 };
        self.canvas = Rect::new(lw + ruler, top + ruler, (self.w - lw - rw - ruler).max(10.0), (self.bar.y - top - ruler).max(10.0));
    }

    /// Mouse position on the page.
    pub fn page_pt(&self, x: f32, y: f32) -> (f32, f32) {
        self.ed.view().to_page(x - self.canvas.x, y - self.canvas.y)
    }

    pub fn fit(&mut self) {
        let (w, h) = (self.canvas.w, self.canvas.h);
        self.ed.fit_page(w, h);
    }

    // ---------------- Events ----------------

    pub fn handle(&mut self, ev: &Event, host: &mut dyn Host) {
        let mut res = self.res.take().expect("res");
        self.handle_inner(ev, host, &mut res);
        self.res = Some(res);
        if self.after.is_some() && self.dialog.is_none() {
            self.run_after(host);
        }
        host.set_title(&self.window_title());
        host.redraw();
    }

    /// Events from the UI test, which already holds the resources.
    pub fn handle_test(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        self.handle_inner(ev, host, r);
        if self.after.is_some() && self.dialog.is_none() {
            self.run_after(host);
        }
    }

    fn handle_inner(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        match ev {
            Event::Resized { w, h, scale } => {
                let first = self.w == 1280.0 && self.canvas.w == 0.0;
                self.w = *w as f32;
                self.h = *h as f32;
                self.s = scale.max(0.5);
                self.layout();
                self.apply_theme(host);
                if first || !self.ed.views.contains_key(&self.ed.pg().id) {
                    self.fit();
                }
                return;
            }
            Event::ThemeChanged { .. } => {
                self.apply_theme(host);
                return;
            }
            Event::Tick => {
                if let Some(mut t) = self.test.take() {
                    t.step(self, host, r);
                    if !t.done {
                        self.test = Some(t);
                    } else {
                        self.code = t.code();
                        self.exit = true;
                    }
                }
                if let Some(c) = self.collab.as_mut() {
                    c.poll(&mut self.ed);
                }
                return;
            }
            Event::Modifiers(m) => {
                self.mods = *m;
            }
            Event::CloseRequested => {
                self.request(After::Close, host);
                return;
            }
            Event::FileDropped(p) => {
                self.drop_file(p, r);
                return;
            }
            _ => {}
        }
        if let Event::Key { mods, .. } = ev {
            self.mods = *mods;
        }
        if let Event::MouseMove { x, y } = ev {
            self.mouse = (*x, *y);
        }
        if let Some(d) = self.dialog.as_mut() {
            let res = d.event(ev, host, &mut r.text, self.s);
            if let DialogResult::Button(b) = res {
                let d = self.dialog.take().unwrap();
                self.dialog_done(d, &b, host, r);
            }
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    let mid = m.id.clone();
                    self.menu = None;
                    self.menu_pick(&mid, &id, host, r);
                    return;
                }
                MenuResult::Closed => {
                    self.menu = None;
                    if matches!(ev, Event::MouseDown { .. }) {
                        // Fall through so the click lands where it was aimed.
                    } else {
                        return;
                    }
                }
                MenuResult::None => {
                    if !matches!(ev, Event::MouseMove { .. }) || self.menu.as_ref().map(|m| m.contains(self.mouse.0, self.mouse.1)).unwrap_or(false) {
                        return;
                    }
                }
            }
        }
        // Window edges resize the borderless window.
        if let Event::MouseDown { x, y, button: Button::Left, .. } = ev {
            if let Some(e) = chrome::resize_edge(*x, *y, self.w, self.h, self.s, host.is_maximized()) {
                host.drag_resize(e);
                return;
            }
        }
        if let Event::MouseMove { x, y } = ev {
            if let Some(e) = chrome::resize_edge(*x, *y, self.w, self.h, self.s, host.is_maximized()) {
                host.set_cursor(chrome::edge_cursor(e));
            }
        }
        let (ta, took) = self.title.event(ev);
        if let Some(a) = ta {
            match a {
                TitleAction::Command(c) => self.command(&c, host, r),
                TitleAction::Minimize => host.minimize(),
                TitleAction::Maximize => host.toggle_maximize(),
                TitleAction::Close => self.request(After::Close, host),
                TitleAction::Drag => host.drag_window(),
                // Cork has no search box in its title bar (its own is over the stencils)
                TitleAction::SearchChanged | TitleAction::SearchEnter | TitleAction::SearchCancel => {}
            }
            return;
        }
        if took && !matches!(ev, Event::MouseMove { .. }) {
            return;
        }
        if self.backstage.is_some() {
            self.backstage_event(ev, host, r);
            return;
        }
        // App-wide shortcuts.
        if let Event::Key { key, mods, down: true, .. } = ev {
            if mods.cmd() && self.ed.edit.is_none() || mods.cmd() && matches!(key, Key::Char('s') | Key::Char('o') | Key::Char('n') | Key::Char('p')) {
                let cmd = match (key, mods.shift) {
                    (Key::Char('s'), false) => Some("save"),
                    (Key::Char('s'), true) => Some("saveas"),
                    (Key::Char('o'), _) => Some("open"),
                    (Key::Char('n'), _) => Some("new"),
                    (Key::Char('p'), _) => Some("print"),
                    (Key::Char('c'), false) => Some("copy"),
                    (Key::Char('x'), _) => Some("cut"),
                    (Key::Char('v'), false) => Some("paste"),
                    _ => None,
                };
                if let Some(c) = cmd {
                    self.command(c, host, r);
                    return;
                }
            }
            if *key == Key::Space && self.ed.edit.is_none() && !matches!(self.focus(), Some(_)) {
                self.space = true;
                host.set_cursor(Cursor::Grab);
                return;
            }
        }
        if let Event::Key { key: Key::Space, down: false, .. } = ev {
            self.space = false;
        }
        let st = self.state_snapshot();
        let (ra, took) = self.ribbon.event(ev, &st, self.now);
        if let Some(a) = ra {
            self.ribbon_action(a, host, r);
            return;
        }
        if took {
            if let Event::MouseMove { .. } = ev {
                host.set_cursor(Cursor::Default);
                self.ed.hover = None;
            }
            return;
        }
        // Panes and bars.
        if self.pane_event(ev, host, r) {
            return;
        }
        self.canvas_event(ev, host, r);
    }

    /// Text field that has keyboard focus outside the board.
    fn focus(&self) -> Option<&'static str> {
        if self.search.focused {
            Some("search")
        } else if self.reply.focused {
            Some("reply")
        } else {
            None
        }
    }

    fn pane_event(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) -> bool {
        let (mx, my) = self.mouse;
        // Focused text fields take keys first.
        if self.search.focused {
            if let Event::Key { .. } | Event::Text(_) = ev {
                match self.search.event(ev, host, &mut r.text, self.s) {
                    FieldResult::Escape => {
                        self.search.focused = false;
                        self.search.set("");
                    }
                    FieldResult::Enter => self.search.focused = false,
                    _ => self.left_scroll = 0.0,
                }
                return true;
            }
        }
        if self.reply.focused {
            if let Event::Key { .. } | Event::Text(_) = ev {
                match self.reply.event(ev, host, &mut r.text, self.s) {
                    FieldResult::Escape => self.reply.focused = false,
                    FieldResult::Enter => self.post_reply(),
                    _ => {}
                }
                return true;
            }
        }
        match ev {
            Event::MouseDown { x, y, button, clicks } => {
                let (x, y) = (*x, *y);
                self.search.focused = false;
                self.reply.focused = false;
                if self.left.contains(x, y) {
                    self.search.event(ev, host, &mut r.text, self.s);
                    if let Some((c, _)) = self.cats.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                        if !self.collapsed.remove(&c) {
                            self.collapsed.insert(c);
                        }
                        return true;
                    }
                    if let Some((id, _)) = self.tiles.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                        if *button == Button::Right && id.starts_with("my:") {
                            self.menu = Some(Menu::new("myshape", Rect::new(x, y, 1.0, 1.0), vec![MenuItem::new(&format!("delete-my:{}", &id[3..]), "Remove from My Shapes").icon("delete")]));
                            return true;
                        }
                        if *clicks >= 2 {
                            self.drag_tile = None;
                            return true;
                        }
                        self.drag_tile = Some(id);
                    }
                    return true;
                }
                if self.right.contains(x, y) {
                    if self.reply.rect.contains(x, y) {
                        self.reply.event(ev, host, &mut r.text, self.s);
                        return true;
                    }
                    if let Some((id, _)) = self.side_hits.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                        self.side_click(&id, *clicks, host, r);
                    }
                    return true;
                }
                if self.bar.contains(x, y) {
                    if let Some((id, rr)) = self.bar_hits.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                        self.bar_click(&id, rr, *button, host, r);
                    }
                    return true;
                }
                if y < self.canvas.y && x < self.canvas.x && self.settings.rulers && y > self.left.y {
                    return true;
                }
                false
            }
            Event::MouseUp { x, y, .. } => {
                if let Some(id) = self.drag_tile.take() {
                    let (x, y) = (*x, *y);
                    if self.canvas.contains(x, y) {
                        let p = self.page_pt(x, y);
                        self.insert_tile(&id, Some(p), r);
                    } else if self.left.contains(x, y) && self.tiles.iter().any(|(t, rr)| *t == id && rr.contains(x, y)) {
                        self.insert_tile(&id, None, r);
                    }
                    return true;
                }
                false
            }
            Event::MouseMove { x, y } => {
                if self.drag_tile.is_some() {
                    host.set_cursor(if self.canvas.contains(*x, *y) { Cursor::Crosshair } else { Cursor::Default });
                    return true;
                }
                if self.left.contains(*x, *y) || self.right.contains(*x, *y) || self.bar.contains(*x, *y) {
                    host.set_cursor(Cursor::Default);
                    self.ed.hover = None;
                    return true;
                }
                false
            }
            Event::Wheel { dy, .. } if self.left.contains(mx, my) => {
                self.left_scroll = (self.left_scroll - dy * 60.0 * self.s).max(0.0);
                true
            }
            Event::Wheel { .. } if self.right.contains(mx, my) || self.bar.contains(mx, my) => true,
            _ => false,
        }
    }

    fn canvas_event(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        let c = self.canvas;
        match ev {
            Event::MouseDown { x, y, button, clicks } => {
                if !c.contains(*x, *y) {
                    return;
                }
                let p = self.page_pt(*x, *y);
                let sp = (*x - c.x, *y - c.y);
                if *button == Button::Right {
                    if self.ed.hit(p, false).map(|id| !self.ed.sel.contains(&id)).unwrap_or(false) {
                        self.ed.sel = vec![self.ed.hit(p, false).unwrap()];
                    }
                    self.context_menu(*x, *y, p);
                    return;
                }
                // Comment pins.
                if *button == Button::Left {
                    if let Some(cid) = self.hit_pin(*x, *y) {
                        self.ed.open_comment = Some(cid);
                        self.settings.side_pane = true;
                        self.side = Side::Comments;
                        self.reply.focused = true;
                        self.layout();
                        return;
                    }
                }
                // Text caret placement inside the edited shape.
                if let (Some(ed), Button::Left) = (self.ed.edit.clone(), button) {
                    if let Some((s, lay, origin)) = self.ed.edit_layout(&mut r.text) {
                        let inv = if s.is_connector() { sk::Transform::identity() } else { crate::geom::shape_xf(&s).invert().unwrap_or_default() };
                        let (lx, ly) = crate::geom::map_pt(&inv, p.0, p.1);
                        let inside = if s.is_connector() { true } else { crate::geom::hit_shape(&s, p, 2.0) };
                        if inside && self.ed.tool == Tool::Pointer {
                            let i = lay.hit(lx - origin.0, ly - origin.1);
                            if let Some(e) = self.ed.edit.as_mut() {
                                e.caret = i;
                                if !self.mods.shift {
                                    e.anchor = i;
                                }
                                if *clicks == 2 {
                                    let t = s.text.clone();
                                    let (a, b) = word_at(&t, i);
                                    e.anchor = a;
                                    e.caret = b;
                                } else if *clicks >= 3 {
                                    e.anchor = 0;
                                    e.caret = s.text.len();
                                }
                            }
                            self.ed.version += 1;
                            let _ = ed;
                            return;
                        }
                    }
                }
                let was_edit = self.ed.edit.as_ref().map(|e| e.id);
                self.ed.mouse_down(p, sp, self.mods.shift, self.mods.ctrl, *clicks, *button == Button::Middle, self.space);
                if let Some(id) = was_edit {
                    if self.ed.edit.is_none() {
                        self.ed.fit_text_shape(&mut r.text, id);
                    }
                }
                if self.ed.tool == Tool::Comment || self.ed.open_comment.is_some() && matches!(self.ed.tool, Tool::Pointer) && self.ed.pg().comments.iter().any(|c| Some(c.id) == self.ed.open_comment && c.text.is_empty()) {
                    self.settings.side_pane = true;
                    self.side = Side::Comments;
                    self.reply.focused = true;
                    self.layout();
                }
            }
            Event::MouseMove { x, y } => {
                let p = self.page_pt(*x, *y);
                let sp = (*x - c.x, *y - c.y);
                if let Drag::Ink { .. } = self.ed.drag {
                    self.ed.mouse_move(p, sp, self.mods.shift, self.mods.alt);
                    return;
                }
                if !c.contains(*x, *y) && matches!(self.ed.drag, Drag::None) {
                    self.ed.hover = None;
                    return;
                }
                self.ed.mouse_move(p, sp, self.mods.shift, self.mods.alt);
                self.set_cursor(host, p);
                if let Some(cb) = self.collab.as_mut() {
                    cb.cursor(self.ed.pg().id, p);
                }
            }
            Event::MouseUp { x, y, .. } => {
                let p = self.page_pt(*x, *y);
                let was = matches!(self.ed.drag, Drag::NewShape { .. } | Drag::Connect { .. });
                self.ed.mouse_up(p);
                if was && self.ed.edit.is_none() && self.ed.tool == Tool::Pointer {
                    host.set_cursor(Cursor::Default);
                }
            }
            Event::Wheel { dx, dy } => {
                let (mx, my) = self.mouse;
                if !c.contains(mx, my) {
                    return;
                }
                if self.mods.ctrl {
                    let z = self.ed.view().zoom * if *dy > 0.0 { 1.12 } else { 1.0 / 1.12 };
                    self.ed.zoom_at(z, mx - c.x, my - c.y);
                } else {
                    let mut v = self.ed.view();
                    let (ddx, ddy) = if self.mods.shift { (*dy, *dx) } else { (*dx, *dy) };
                    v.ox += ddx * 50.0 * self.s;
                    v.oy += ddy * 50.0 * self.s;
                    self.ed.set_view(v);
                }
            }
            Event::Key { key, mods, down: true, .. } => {
                if self.ed.edit.is_some() {
                    let mut clip = |set: Option<String>| -> Option<String> {
                        match set {
                            Some(t) => {
                                host.set_clipboard_text(&t);
                                None
                            }
                            None => host.clipboard_text(),
                        }
                    };
                    let edited = self.ed.edit.as_ref().map(|e| e.id);
                    if self.ed.edit_key(key, mods, &mut r.text, &mut clip) {
                        if self.ed.edit.is_none() {
                            if let Some(id) = edited {
                                self.ed.fit_text_shape(&mut r.text, id);
                            }
                        }
                        return;
                    }
                }
                if *key == Key::Delete || *key == Key::Backspace {
                    if !self.ed.sel.is_empty() {
                        self.ed.delete_sel("Delete");
                    }
                    return;
                }
                let (vw, vh) = (c.w, c.h);
                if mods.cmd() && *key == Key::Char('c') && mods.shift {
                    self.command("painter", host, r);
                    return;
                }
                self.ed.key(key, mods, vw, vh);
            }
            Event::Text(t) => {
                if self.ed.edit.is_none() && self.ed.sel.len() == 1 && !self.mods.cmd() {
                    // Typing on a selected shape replaces its text, as in Visio.
                    let id = self.ed.sel[0];
                    if self.ed.editable(id) && !self.ed.pg().get(id).map(|s| s.is_group() || s.kind == "ink" || s.kind == "image").unwrap_or(true) {
                        self.ed.begin_edit(id);
                    }
                }
                if self.ed.edit.is_some() {
                    self.ed.type_text(t);
                }
            }
            Event::Key { key: Key::Space, down: false, .. } => self.space = false,
            _ => {}
        }
    }

    fn set_cursor(&mut self, host: &mut dyn Host, p: (f32, f32)) {
        let cur = match (&self.ed.tool, &self.ed.drag) {
            (_, Drag::Pan { .. }) => Cursor::Grabbing,
            (_, Drag::Move { .. }) => Cursor::Move,
            (_, Drag::Connect { .. }) | (_, Drag::ConnEnd { .. }) => Cursor::Crosshair,
            (Tool::Hand, _) => Cursor::Grab,
            _ if self.space => Cursor::Grab,
            (Tool::Pointer, _) => {
                if self.ed.edit.as_ref().map(|e| self.ed.pg().get(e.id).map(|s| crate::geom::hit_shape(s, p, 2.0)).unwrap_or(false)).unwrap_or(false) {
                    Cursor::Text
                } else if let Some(h) = self.ed.hit_handle(p) {
                    match h {
                        0 | 4 => Cursor::ResizeNWSE,
                        2 | 6 => Cursor::ResizeNESW,
                        1 | 5 => Cursor::ResizeNS,
                        3 | 7 => Cursor::ResizeEW,
                        _ => Cursor::Pointer,
                    }
                } else if self.ed.hit_conn_end(p).is_some() || self.ed.hover_point.is_some() {
                    Cursor::Crosshair
                } else if self.ed.format_brush.is_some() {
                    Cursor::Pointer
                } else if self.ed.hover.is_some() {
                    if self.ed.hover.map(|h| self.ed.editable(h)).unwrap_or(false) { Cursor::Move } else { Cursor::NotAllowed }
                } else {
                    Cursor::Default
                }
            }
            (Tool::Text, _) => Cursor::Text,
            _ => Cursor::Crosshair,
        };
        host.set_cursor(cur);
    }

    fn hit_pin(&self, x: f32, y: f32) -> Option<Id> {
        let v = self.ed.view();
        for c in self.ed.pg().comments.iter().rev() {
            if c.resolved {
                continue;
            }
            let (px, py) = self.pin_pos(c, &v);
            if (x - px).abs() < 12.0 * self.s && y > py - 26.0 * self.s && y < py + 2.0 {
                return Some(c.id);
            }
        }
        None
    }

    fn pin_pos(&self, c: &Comment, v: &View) -> (f32, f32) {
        let (mut x, mut y) = (c.x, c.y);
        if let Some(s) = c.shape.and_then(|id| self.ed.pg().get(id)) {
            // Pins on shapes stick to the shape's top right corner.
            let (bx, by, bw, _) = crate::geom::bounds(s);
            x = bx + bw;
            y = by;
        }
        let (sx, sy) = v.to_screen(x, y);
        (sx + self.canvas.x, sy + self.canvas.y)
    }

    // ---------------- Commands ----------------

    pub fn state_snapshot(&self) -> St {
        let ed = &self.ed;
        let sel = ed.selected();
        let first = sel.first().map(|s| (*s).clone());
        St {
            has_sel: !sel.is_empty(),
            multi: sel.len() > 1,
            three: sel.len() > 2,
            conn: sel.iter().any(|s| s.is_connector()),
            group: sel.iter().any(|s| s.is_group()),
            images: sel.iter().any(|s| s.kind == "image"),
            first,
            can_undo: ed.can_undo(),
            can_redo: ed.can_redo(),
            clip: !ed.clip.is_empty(),
            tool: ed.tool.clone(),
            painter: ed.format_brush.is_some(),
            settings: self.settings.clone(),
            zoom: ed.view().zoom,
            landscape: ed.pg().paper.landscape,
            dirty: ed.dirty,
            route: sel.iter().find_map(|s| s.conn.as_ref().map(|c| c.route)),
            jumps: sel.iter().any(|s| s.conn.as_ref().map(|c| c.jumps).unwrap_or(false)),
            sharing: self.collab.is_some(),
            side_layers: self.settings.side_pane && self.side == Side::Layers,
            side_comments: self.settings.side_pane && self.side == Side::Comments,
            side_data: self.settings.side_pane && self.side == Side::Data,
        }
    }

    fn ribbon_action(&mut self, a: RibbonAction, host: &mut dyn Host, r: &mut Res) {
        match a {
            RibbonAction::Command(c) => self.command(&c, host, r),
            RibbonAction::Backstage => self.open_backstage(BackPage::New),
            RibbonAction::Menu(id, rect) => self.open_menu(&id, rect, r),
            RibbonAction::GroupMenu(items, rect) => {
                let items = items.into_iter().map(|(id, label, icon)| MenuItem::new(&id, &label).icon(&icon)).collect();
                self.menu = Some(Menu::new("group", rect, items));
            }
        }
    }

    pub fn open_backstage(&mut self, page: BackPage) {
        self.ed.end_edit();
        self.ribbon.cancel_tips();
        let mut b = Backstage {
            page,
            files: None,
            export_fmt: 2,
            export_all: false,
            export_scale: 1,
            print_fit: true,
            print_all: false,
            copies: 1,
            printer: 0,
            printers: vec![],
            msg: String::new(),
            hits: vec![],
        };
        self.backstage_page(&mut b, page);
        self.backstage = Some(b);
        self.layout();
    }

    fn backstage_page(&self, b: &mut Backstage, page: BackPage) {
        b.page = page;
        b.msg.clear();
        let dir = self.ed.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf()).unwrap_or_else(folio_kit::platform::documents_dir);
        b.files = match page {
            BackPage::Open => Some(FilePane::new(&dir, &["vsdx", "vsd", "csv"], false, "")),
            BackPage::SaveAs => Some(FilePane::new(&dir, &["vsdx"], true, &format!("{}.vsdx", stem(&self.doc_name())))),
            BackPage::Export => {
                let ext = ["png", "svg", "pdf"][b.export_fmt];
                let mut f = FilePane::new(&dir, &[ext], true, &format!("{}.{ext}", stem(&self.doc_name())));
                f.button = "Export".into();
                Some(f)
            }
            _ => None,
        };
        if page == BackPage::Print && b.printers.is_empty() {
            b.printers = folio_kit::platform::printers();
        }
    }

    pub fn command(&mut self, id: &str, host: &mut dyn Host, r: &mut Res) {
        let (vw, vh) = (self.canvas.w, self.canvas.h);
        let ed = &mut self.ed;
        if !id.starts_with("tool.") && ed.edit.is_some() && !matches!(id, "bold" | "italic" | "underline" | "grow" | "shrink" | "undo" | "redo" | "save") {
            let eid = ed.edit.as_ref().map(|e| e.id);
            ed.end_edit();
            if let Some(eid) = eid {
                ed.fit_text_shape(&mut r.text, eid);
            }
        }
        match id {
            "new" => self.open_backstage(BackPage::New),
            "open" => self.open_backstage(BackPage::Open),
            "save" => {
                self.save(host, r);
            }
            "saveas" => self.open_backstage(BackPage::SaveAs),
            "export" => self.open_backstage(BackPage::Export),
            "print" => self.open_backstage(BackPage::Print),
            "undo" => self.ed.undo(),
            "redo" => self.ed.redo(),
            "lane.add" => {
                self.ed.add_lane();
                self.flash("Lane added. Type to name it; moving a lane moves what's in it.");
            }
            "lane.delete" => {
                if self.ed.delete_lane() == 0 {
                    self.flash("Pick a lane first.");
                }
            }
            "lane.turn" => {
                if self.ed.turn_lanes() == 0 {
                    self.flash("There are no lanes on this page yet.");
                }
            }
            "copy" => self.copy_out(host, r),
            "cut" => {
                self.copy_out(host, r);
                self.ed.delete_sel("Cut");
            }
            "paste" => self.paste_in(host, r, None),
            "duplicate" => self.ed.duplicate(),
            "delete" => self.ed.delete_sel("Delete"),
            "painter" => {
                if let Some(s) = self.ed.first() {
                    self.ed.format_brush = Some((s.style.clone(), s.tstyle.clone()));
                    self.flash("Click a shape to give it this look.");
                }
            }
            "bold" | "italic" | "underline" => self.ed.toggle_text(id),
            "grow" => self.ed.grow_font(true),
            "shrink" => self.ed.grow_font(false),
            "align-left" => self.ed.set_align(Align::Left),
            "align-center" => self.ed.set_align(Align::Center),
            "align-right" => self.ed.set_align(Align::Right),
            "valign-top" => self.ed.set_valign(folio_kit::text::VAlign::Top),
            "valign-middle" => self.ed.set_valign(folio_kit::text::VAlign::Middle),
            "valign-bottom" => self.ed.set_valign(folio_kit::text::VAlign::Bottom),
            "fill" => {
                let c = self.settings.last_fill;
                self.ed.set_fill(c)
            }
            "line" => {
                let c = self.settings.last_line;
                self.ed.set_line(c)
            }
            "shadow" => self.ed.toggle_shadow(),
            "group" => self.ed.group(),
            "ungroup" => self.ed.ungroup(),
            "lock" => self.ed.set_lock(true),
            "unlock" => self.ed.set_lock(false),
            "dist.h" => self.ed.distribute(true),
            "dist.v" => self.ed.distribute(false),
            "rotate.right" => self.ed.rotate_by(90.0),
            "rotate.left" => self.ed.rotate_by(-90.0),
            "flip.h" => self.ed.flip(true),
            "flip.v" => self.ed.flip(false),
            "route.elbow" => self.ed.set_route(Route::Elbow),
            "route.straight" => self.ed.set_route(Route::Straight),
            "route.curved" => self.ed.set_route(Route::Curved),
            "jumps" => self.ed.toggle_jumps(),
            "picture" | "mood.add" => self.open_backstage_pictures(),
            "page-new" => {
                self.ed.add_page(false);
                self.fit();
            }
            "page-mood" => {
                self.ed.add_page(true);
                templates::apply("mood", &mut self.ed);
                self.fit();
            }
            "page-dup" => self.ed.duplicate_page(),
            "page-del" => {
                if self.ed.doc.pages.len() > 1 {
                    self.dialog = Some(Dialog::new("page-del", "Delete page").note(&format!("Delete \"{}\" and everything on it?", self.ed.pg().name)).button("delete", "Delete").button("cancel", "Cancel"));
                }
            }
            "page-rename" => {
                let n = self.ed.pg().name.clone();
                self.dialog = Some(Dialog::new("page-rename", "Rename page").text("name", "Page name", &n).button("ok", "Rename").button("cancel", "Cancel"));
            }
            "layer-new" => {
                self.ed.add_layer("");
                self.settings.side_pane = true;
                self.side = Side::Layers;
                self.layout();
            }
            "orgchart" => self.open_backstage_org(),
            "vote.results" => {
                let txt = mood::vote_results(&self.ed);
                self.dialog = Some(Dialog::new("info", "Vote results").note(&txt).button("ok", "Close").width(420.0));
            }
            "vote.reset" => {
                self.ed.checkpoint("Clear votes", "");
                for s in self.ed.pgm().shapes.iter_mut() {
                    s.votes = 0;
                }
                self.ed.changed();
            }
            "mood.palette" => {
                let n = mood::extract_palette(&mut self.ed, 5);
                if n == 0 {
                    self.flash("Select one or more pictures first.");
                }
            }
            "mood.font" => {
                let (cx, cy) = self.ed.shape_at_center(vw, vh);
                let id = self.ed.insert_kind_at("font", cx, cy);
                if let Some(s) = self.ed.pgm().get_mut(id) {
                    s.text = "Aa — The quick brown fox".into();
                }
                self.open_menu("mood-font", Rect::new(self.mouse.0, self.mouse.1, 1.0, 1.0), r);
            }
            "mood.swatch" => {
                let (cx, cy) = self.ed.shape_at_center(vw, vh);
                self.ed.insert_kind_at("swatch", cx, cy);
                let anchor = self.ribbon.rect_of("mood.swatch").unwrap_or_default();
                self.open_menu("swatch-color", anchor, r);
            }
            "mood.polaroid" => {
                let on = !self.ed.selected().iter().any(|s| s.get("frame") == "polaroid");
                self.ed.each_editable("Polaroid frame", |s| {
                    if s.kind == "image" {
                        if on {
                            s.data.insert("frame".into(), "polaroid".into());
                        } else {
                            s.data.remove("frame");
                        }
                    }
                });
            }
            "mood.crop" => self.crop_dialog(),
            "mood.section" => mood::section(&mut self.ed),
            "mood.grid" => mood::tidy(&mut self.ed, false),
            "mood.masonry" => mood::tidy(&mut self.ed, true),
            "mood.copy-palette" => {
                let t = mood::palette_text(&self.ed);
                if t.is_empty() {
                    self.flash("No swatches to copy.");
                } else {
                    host.set_clipboard_text(&t);
                    self.flash("Palette copied.");
                }
            }
            "share-host" => self.share_host(),
            "share-join" => {
                self.dialog = Some(Dialog::new("join", "Join a board").text("addr", "Address shown on the host (computer:port)", "").text("code", "Board code", "").button("join", "Join").button("cancel", "Cancel"));
            }
            "share-stop" => {
                self.collab = None;
                self.flash("Stopped sharing.");
            }
            "view.rulers" => {
                self.settings.rulers = !self.settings.rulers;
                self.layout();
            }
            "view.grid" => {
                self.settings.grid = !self.settings.grid;
                self.ed.grid = self.settings.grid;
            }
            "view.snap" => {
                self.settings.snap = !self.settings.snap;
                self.ed.snap = self.settings.snap;
            }
            "view.guides" => {
                self.settings.guides = !self.settings.guides;
                self.ed.snap_shapes = self.settings.guides;
            }
            "view.minimap" => self.settings.minimap = !self.settings.minimap,
            "view.pagebreaks" => self.settings.page_breaks = !self.settings.page_breaks,
            "zoom.fit" => self.fit(),
            "zoom.sel" => self.ed.fit_selection(vw, vh),
            "zoom.100" => self.ed.zoom_at(1.0, vw / 2.0, vh / 2.0),
            "zoom.in" => self.ed.zoom_step(true, vw / 2.0, vh / 2.0),
            "zoom.out" => self.ed.zoom_step(false, vw / 2.0, vh / 2.0),
            "pane.shapes" => {
                self.settings.shapes_pane = !self.settings.shapes_pane;
                self.layout();
            }
            "pane.layers" | "pane.comments" | "pane.data" => {
                let want = match id {
                    "pane.layers" => Side::Layers,
                    "pane.data" => Side::Data,
                    _ => Side::Comments,
                };
                if self.settings.side_pane && self.side == want {
                    self.settings.side_pane = false;
                } else {
                    self.settings.side_pane = true;
                    self.side = want;
                }
                self.layout();
            }
            "save-myshape" => self.save_my_shape(r),
            "comment-here" => {
                let p = self.ed.sel_bounds().map(|b| (b.0 + b.2, b.1)).unwrap_or(self.ed.shape_at_center(vw, vh));
                self.ed.add_comment(p);
                self.settings.side_pane = true;
                self.side = Side::Comments;
                self.reply.focused = true;
                self.layout();
            }
            "edit-text" => {
                if let Some(id) = self.ed.sel.first().copied() {
                    self.ed.begin_edit(id);
                }
            }
            "select-all" => self.ed.select_all(),
            _ => {
                if let Some(t) = id.strip_prefix("tool.") {
                    let tool = match t {
                        "pointer" => Tool::Pointer,
                        "hand" => Tool::Hand,
                        "connector" => Tool::Connector,
                        "text" => Tool::Text,
                        "sticky" => Tool::Sticky,
                        "pen" => Tool::Pen,
                        "highlighter" => Tool::Highlighter,
                        "eraser" => Tool::Eraser,
                        "frame" => Tool::Frame,
                        "comment" => Tool::Comment,
                        "vote" => Tool::Vote,
                        "eyedropper" => Tool::Eyedropper,
                        k => Tool::Shape(k.strip_prefix("shape.").unwrap_or(k).to_string()),
                    };
                    self.ed.end_edit();
                    self.ed.tool_lock = matches!(tool, Tool::Pen | Tool::Highlighter | Tool::Eraser | Tool::Vote | Tool::Connector);
                    self.ed.tool = tool;
                } else if let Some(a) = id.strip_prefix("align.") {
                    self.ed.align(a);
                } else if let Some(o) = id.strip_prefix("order.") {
                    self.ed.order(o);
                } else if let Some(t) = id.strip_prefix("template.") {
                    templates::apply(t, &mut self.ed);
                    self.fit();
                } else if let Some(l) = id.strip_prefix("layout.") {
                    layout::relayout(&mut self.ed, l, &mut r.text);
                } else if let Some(i) = id.strip_prefix("quick.") {
                    self.ed.quick_style(i.parse().unwrap_or(0));
                }
            }
        }
        self.cache_check();
    }

    fn cache_check(&mut self) {}

    pub fn flash(&mut self, msg: &str) {
        self.flash = Some((msg.to_string(), self.now + Duration::from_secs(4)));
    }

    fn open_backstage_pictures(&mut self) {
        let dir = folio_kit::platform::home_dir().join("Pictures");
        let dir = if dir.is_dir() { dir } else { folio_kit::platform::documents_dir() };
        self.open_backstage(BackPage::Open);
        if let Some(b) = self.backstage.as_mut() {
            let mut f = FilePane::new(&dir, &["png", "jpg", "jpeg", "gif", "webp", "bmp"], false, "");
            f.button = "Insert".into();
            b.files = Some(f);
            b.msg = "pictures".into();
        }
    }

    fn open_backstage_org(&mut self) {
        let dir = folio_kit::platform::documents_dir();
        self.open_backstage(BackPage::Open);
        if let Some(b) = self.backstage.as_mut() {
            let mut f = FilePane::new(&dir, &["csv", "xlsx"], false, "");
            f.button = "Build Org Chart".into();
            b.files = Some(f);
            b.msg = "orgchart".into();
        }
    }

    fn crop_dialog(&mut self) {
        let Some(s) = self.ed.selected().into_iter().find(|s| s.kind == "image").cloned() else {
            self.flash("Select a picture to crop.");
            return;
        };
        let pct = |v: f32| format!("{:.0}", v * 100.0);
        self.crop = Some(s.id);
        self.dialog = Some(
            Dialog::new("crop", "Crop picture")
                .note("How much to trim from each side, in percent of the picture.")
                .text("l", "Left", &pct(s.crop.0))
                .text("t", "Top", &pct(s.crop.1))
                .text("r", "Right", &pct(1.0 - s.crop.2))
                .text("b", "Bottom", &pct(1.0 - s.crop.3))
                .button("ok", "Crop")
                .button("reset", "Reset")
                .button("cancel", "Cancel"),
        );
    }

    fn share_host(&mut self) {
        if self.collab.is_some() {
            let info = self.collab.as_ref().map(|c| c.info()).unwrap_or_default();
            self.dialog = Some(Dialog::new("share-info", "Sharing this board").note(&info).button("ok", "Close").button("stop", "Stop Sharing"));
            return;
        }
        match crate::collab::Collab::host(&self.ed, &self.ed.author.clone()) {
            Ok(c) => {
                let info = c.info();
                self.collab = Some(c);
                self.dialog = Some(Dialog::new("share-info", "Sharing this board").note(&info).button("ok", "Close").button("stop", "Stop Sharing").width(460.0));
            }
            Err(e) => self.dialog = Some(Dialog::new("info", "Can't share").note(&e).button("ok", "Close")),
        }
    }

    fn save_my_shape(&mut self, r: &mut Res) {
        let (shapes, imgs) = self.ed.copy_shapes();
        if shapes.is_empty() {
            return;
        }
        let name = shapes.iter().find(|s| !s.text.is_empty()).map(|s| s.text.lines().next().unwrap_or("").to_string()).unwrap_or_else(|| "My shape".into());
        self.settings.add_my_shape(&name, shapes, imgs);
        self.thumbs.retain(|k, _| !k.0.starts_with("my:"));
        self.collapsed.remove("My Shapes");
        let _ = r;
        self.flash("Saved to My Shapes.");
    }

    fn insert_tile(&mut self, id: &str, at: Option<(f32, f32)>, r: &mut Res) {
        let at = at.unwrap_or_else(|| self.ed.shape_at_center(self.canvas.w, self.canvas.h));
        if let Some(n) = id.strip_prefix("my:") {
            if let Some(m) = self.settings.my_shapes().into_iter().find(|m| m.name == n) {
                self.ed.paste_shapes(&m.shapes, &m.images, Some(at), "Insert my shape");
            }
            return;
        }
        let _ = r;
        self.ed.end_edit();
        self.ed.insert_kind_at(id, at.0, at.1);
    }

    fn copy_out(&mut self, host: &mut dyn Host, r: &mut Res) {
        if let Some(ed) = &self.ed.edit {
            let _ = ed;
            return;
        }
        self.ed.copy();
        if self.ed.clip.is_empty() {
            return;
        }
        // Other apps get a picture; Cork keeps the shapes next to it so pasting comes back editable.
        let (shapes, imgs) = (self.ed.clip.clone(), self.ed.clip_images.clone());
        if let Some(img) = export::shapes_image(&self.ed.doc, &shapes, &imgs, &mut r.text, &mut r.images, 2.0) {
            let rgba = gfx::rgba_from_pixmap(&img);
            let im = folio_kit::platform::Image { w: img.width(), h: img.height(), rgba };
            host.set_clipboard_image(&im);
            crate::settings::store_clip(&shapes, &imgs, &im);
            let mut d = Document { pages: vec![], images: self.ed.doc.images.clone(), next_id: self.ed.doc.next_id };
            for (k, v) in &imgs {
                d.images.insert(k.clone(), v.clone());
            }
            let mut page = Page { shapes: shapes.clone(), ..Default::default() };
            let layer = page.layers.first().map(|l| l.id).unwrap_or(0);
            for s in page.shapes.iter_mut() {
                s.layer = layer;
            }
            d.pages.push(page);
            d.prune_images();
            if let Ok(board) = crate::vsdx::write(&d) {
                crate::settings::store_clip_board(&files::with_chunk(&gfx::encode_png(&img), files::BOARD_CHUNK, &board));
            }
        }
    }

    fn paste_in(&mut self, host: &mut dyn Host, r: &mut Res, at: Option<(f32, f32)>) {
        if let Some(img) = host.clipboard_image() {
            if let Some((shapes, imgs)) = crate::settings::match_clip(&img) {
                self.ed.paste_shapes(&shapes, &imgs, at, "Paste");
                return;
            }
            if let Some(pm) = gfx::pixmap_from_rgba(img.w, img.h, &img.rgba) {
                let png = gfx::encode_png(&pm);
                let at = at.unwrap_or_else(|| self.ed.shape_at_center(self.canvas.w, self.canvas.h));
                mood::insert_picture(&mut self.ed, png, at, &mut r.images);
                return;
            }
        }
        if !self.ed.clip.is_empty() {
            self.ed.paste(at);
            return;
        }
        if let Some(t) = host.clipboard_text() {
            if !t.trim().is_empty() {
                let at = at.unwrap_or_else(|| self.ed.shape_at_center(self.canvas.w, self.canvas.h));
                let id = self.ed.insert_kind_at("text", at.0, at.1);
                if let Some(s) = self.ed.pgm().get_mut(id) {
                    s.text = t.trim_end().to_string();
                    s.w = 280.0;
                }
                self.ed.fit_text_shape(&mut r.text, id);
            }
        }
    }

    fn drop_file(&mut self, p: &Path, r: &mut Res) {
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => {
                if let Ok(b) = std::fs::read(p) {
                    let at = self.page_pt(self.mouse.0, self.mouse.1);
                    let at = if self.canvas.contains(self.mouse.0, self.mouse.1) { at } else { self.ed.shape_at_center(self.canvas.w, self.canvas.h) };
                    mood::insert_picture(&mut self.ed, b, at, &mut r.images);
                }
            }
            "vsdx" | "vsd" => {
                self.after = Some(After::Open(p.to_path_buf()));
            }
            _ => self.flash("Cork can't use that kind of file."),
        }
    }

    pub fn save(&mut self, host: &mut dyn Host, _r: &mut Res) -> bool {
        let Some(path) = self.ed.path.clone() else {
            self.open_backstage(BackPage::SaveAs);
            return false;
        };
        let _ = host;
        match files::save(&self.ed.doc, &path) {
            Ok(()) => {
                self.ed.dirty = false;
                self.settings.add_recent(&path);
                self.flash(&format!("Saved {}", path.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()));
                true
            }
            Err(e) => {
                self.dialog = Some(Dialog::new("info", "Can't save").note(&e).button("ok", "Close"));
                false
            }
        }
    }

    /// Run `after` now, or ask about unsaved changes first.
    pub fn request(&mut self, after: After, host: &mut dyn Host) {
        let _ = host;
        if self.ed.dirty {
            self.dialog = Some(
                Dialog::new("unsaved", "Folio Cork")
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
                self.settings.ribbon_collapsed = self.ribbon.collapsed;
                self.settings.save();
                host.close();
                self.exit = true;
            }
            After::New(t) => {
                let mut ed = Editor::new(Document::default());
                std::mem::swap(&mut self.ed, &mut ed);
                self.carry_settings(&ed);
                templates::apply(&t, &mut self.ed);
                self.ed.dirty = false;
                self.ed.version += 1;
                self.backstage = None;
                self.layout();
                self.fit();
            }
            After::Open(p) => match files::load(&p) {
                Ok(doc) => {
                    let mut ed = Editor::new(doc);
                    ed.path = if files::keeps_path(&p) { Some(p.clone()) } else { None };
                    std::mem::swap(&mut self.ed, &mut ed);
                    self.carry_settings(&ed);
                    self.settings.add_recent(&p);
                    self.backstage = None;
                    if let Some(r) = self.res.as_mut() {
                        r.images = Images::default();
                    }
                    self.layout();
                    self.fit();
                }
                Err(e) => self.dialog = Some(Dialog::new("info", "Can't open the file").note(&e).button("ok", "Close")),
            },
        }
    }

    fn carry_settings(&mut self, old: &Editor) {
        self.ed.grid = old.grid;
        self.ed.snap = old.snap;
        self.ed.snap_shapes = old.snap_shapes;
        self.ed.author = old.author.clone();
        self.ed.sticky_color = old.sticky_color;
        self.ed.pen_color = old.pen_color;
        self.ed.pen_width = old.pen_width;
        self.cache = None;
    }

    fn dialog_done(&mut self, d: Dialog, button: &str, host: &mut dyn Host, r: &mut Res) {
        match (d.id.as_str(), button) {
            ("unsaved", "save") => {
                if self.save(host, r) {
                    self.after = self.pending.take();
                } else {
                    // Save As opened; carry on after the user saves.
                }
            }
            ("unsaved", "discard") => self.after = self.pending.take(),
            ("unsaved", _) => self.pending = None,
            ("page-rename", "ok") => self.ed.rename_page(&d.value("name")),
            ("prop-edit", "ok") => {
                let (name, value, was) = (d.value("name"), d.value("value"), d.value("was"));
                if !was.is_empty() && was != name {
                    self.ed.rename_prop(&was, &name);
                }
                if name.trim().is_empty() {
                    self.flash("A property needs a name.");
                } else {
                    self.ed.set_prop(&name, &value);
                }
            }
            ("page-del", "delete") => {
                self.ed.delete_page();
                self.fit();
            }
            ("layer-rename", "ok") => {
                if let Some(id) = d.value("id").parse::<Id>().ok().or(self.ed.layer) {
                    self.ed.rename_layer(id, &d.value("name"));
                }
            }
            ("hex", "ok") => {
                if let Some(c) = Color::parse(&d.value("hex")) {
                    let target = d.value("target");
                    self.apply_color(&target, c);
                } else {
                    self.flash("That isn't a colour code like #3366CC.");
                }
            }
            ("crop", "ok") | ("crop", "reset") => {
                let num = |k: &str| d.value(k).trim().trim_end_matches('%').parse::<f32>().unwrap_or(0.0).clamp(0.0, 95.0) / 100.0;
                let (l, t, rr, b) = if button == "reset" { (0.0, 0.0, 0.0, 0.0) } else { (num("l"), num("t"), num("r"), num("b")) };
                if let Some(id) = self.crop.take() {
                    let ok = l + rr < 0.95 && t + b < 0.95;
                    if ok {
                        self.ed.checkpoint("Crop", "");
                        if let Some(s) = self.ed.pgm().get_mut(id) {
                            let (ow, oh) = ((s.crop.2 - s.crop.0).max(0.01), (s.crop.3 - s.crop.1).max(0.01));
                            let full_w = s.w / ow;
                            let full_h = s.h / oh;
                            let old_x = s.x - s.crop.0 * full_w;
                            let old_y = s.y - s.crop.1 * full_h;
                            s.crop = (l, t, 1.0 - rr, 1.0 - b);
                            s.x = old_x + l * full_w;
                            s.y = old_y + t * full_h;
                            s.w = full_w * (1.0 - rr - l);
                            s.h = full_h * (1.0 - b - t);
                        }
                        self.ed.changed();
                    }
                }
            }
            ("join", "join") => match crate::collab::Collab::join(&d.value("addr"), &d.value("code"), &self.ed.author.clone()) {
                Ok(c) => {
                    self.collab = Some(c);
                    self.flash("Joined. The board appears in a moment.");
                }
                Err(e) => self.dialog = Some(Dialog::new("info", "Can't join").note(&e).button("ok", "Close")),
            },
            ("share-info", "stop") => self.collab = None,
            ("author", "ok") => {
                let n = d.value("name").trim().to_string();
                if !n.is_empty() {
                    self.ed.author = n.clone();
                    self.settings.author = n;
                    self.settings.save();
                }
            }
            _ => {}
        }
        let _ = r;
    }

    fn apply_color(&mut self, target: &str, c: Color) {
        match target {
            "fill" => {
                self.settings.last_fill = c;
                self.ed.set_fill(c);
            }
            "line" => {
                self.settings.last_line = c;
                self.ed.set_line(c);
            }
            "font-color" => self.ed.set_text_color(c),
            "background" => self.ed.set_background(c),
            "pen" => {
                self.ed.pen_color = c;
                self.ed.tool = Tool::Pen;
                self.ed.tool_lock = true;
            }
            "sticky" => {
                self.ed.sticky_color = c;
                let stickies: Vec<Id> = self.ed.selected().iter().filter(|s| s.kind == "sticky").map(|s| s.id).collect();
                if !stickies.is_empty() {
                    self.ed.set_fill(c);
                }
            }
            "swatch-color" => self.ed.set_fill(c),
            _ => {}
        }
    }

    // ---------------- Menus ----------------

    pub fn open_menu(&mut self, id: &str, rect: Rect, r: &mut Res) {
        let st = self.state_snapshot();
        let items: Vec<MenuItem> = match id {
            "fill.menu" | "line.menu" | "font-color" | "background" | "swatch-color" | "sticky-color" | "pen.menu" => {
                let target = match id {
                    "fill.menu" => "fill",
                    "line.menu" => "line",
                    "sticky-color" => "sticky",
                    "pen.menu" => "pen",
                    x => x,
                };
                let mut items = vec![];
                match target {
                    "fill" => items.push(MenuItem::new("none", "No Fill").swatch(Color::TRANSPARENT)),
                    "line" => items.push(MenuItem::new("none", "No Line").swatch(Color::TRANSPARENT)),
                    "pen" => {
                        for w in [1.5, 3.0, 5.0, 8.0] {
                            items.push(MenuItem::new(&format!("penw:{w}"), &format!("{w} pt")).check((self.ed.pen_width - w).abs() < 0.1));
                        }
                        items.push(MenuItem::sep());
                    }
                    _ => {}
                }
                items.push(MenuItem::new("hex", "More Colours…").icon("palette"));
                let palette = if target == "sticky" {
                    vec![Color::hex(0xFFF176), Color::hex(0xFFCC80), Color::hex(0xF8BBD0), Color::hex(0xB3E5FC), Color::hex(0xC5E1A5), Color::hex(0xE1BEE7), Color::hex(0xFFFFFF), Color::hex(0xCFD8DC)]
                } else {
                    standard_palette()
                };
                let cols = if target == "sticky" { 8 } else { 10 };
                self.menu = Some(Menu::new(&format!("color:{target}"), rect, items).with_palette(palette, cols));
                return;
            }
            "tool.pen.menu" => {
                self.open_menu("pen.menu", rect, r);
                return;
            }
            "font" => {
                let cur = st.first.as_ref().map(|s| s.tstyle.family.clone()).unwrap_or_default();
                let mut v = vec![MenuItem::new("font:", "Default (interface font)").check(cur.is_empty())];
                for f in r.text.families().into_iter().take(400) {
                    let mut it = MenuItem::new(&format!("font:{f}"), &f).check(cur == f);
                    it.font = Some(f.clone());
                    v.push(it);
                }
                v
            }
            "mood-font" => {
                let mut v = vec![];
                for f in r.text.families().into_iter().take(400) {
                    let mut it = MenuItem::new(&format!("font:{f}"), &f);
                    it.font = Some(f.clone());
                    v.push(it);
                }
                v
            }
            "size" => [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 60, 72, 96].iter().map(|n| MenuItem::new(&format!("size:{n}"), &n.to_string()).check(st.first.as_ref().map(|s| (s.tstyle.size - *n as f32).abs() < 0.1).unwrap_or(false))).collect(),
            "weight" => [0.0f32, 0.75, 1.0, 1.5, 2.25, 3.0, 4.5, 6.0].iter().map(|w| MenuItem::new(&format!("weight:{w}"), &if *w == 0.0 { "No line".to_string() } else { format!("{w} pt") }).check(st.first.as_ref().map(|s| (s.style.width - w).abs() < 0.01).unwrap_or(false))).collect(),
            "dash" => vec![
                MenuItem::new("dash:solid", "Solid"),
                MenuItem::new("dash:dash", "Dashed"),
                MenuItem::new("dash:dot", "Dotted"),
                MenuItem::new("dash:dashdot", "Dash dot"),
            ],
            "quick" => crate::commands::QUICK.iter().enumerate().map(|(i, (f, l, _))| MenuItem::new(&format!("quick.{i}"), &format!("Style {}", i + 1)).swatch(Color::hex(*f)).key(&Color::hex(*l).to_hex().to_uppercase())).collect(),
            "align-menu" => vec![
                MenuItem::new("align.left", "Align Left").icon("obj-left").off(!st.multi),
                MenuItem::new("align.center", "Align Centre").icon("obj-center").off(!st.multi),
                MenuItem::new("align.right", "Align Right").icon("obj-right").off(!st.multi),
                MenuItem::sep(),
                MenuItem::new("align.top", "Align Top").icon("obj-top").off(!st.multi),
                MenuItem::new("align.middle", "Align Middle").icon("obj-middle").off(!st.multi),
                MenuItem::new("align.bottom", "Align Bottom").icon("obj-bottom").off(!st.multi),
                MenuItem::sep(),
                MenuItem::new("dist.h", "Distribute Horizontally").icon("dist-h").off(!st.three),
                MenuItem::new("dist.v", "Distribute Vertically").icon("dist-v").off(!st.three),
            ],
            "order-menu" => vec![
                MenuItem::new("order.front", "Bring to Front").icon("front").key("Ctrl+Shift+]"),
                MenuItem::new("order.forward", "Bring Forward").icon("forward").key("Ctrl+]"),
                MenuItem::new("order.backward", "Send Backward").icon("backward").key("Ctrl+["),
                MenuItem::new("order.back", "Send to Back").icon("back").key("Ctrl+Shift+["),
            ],
            "shapes-menu" => {
                let mut v = vec![];
                for cat in stencils::CATEGORIES {
                    let sub: Vec<MenuItem> = stencils::STENCILS.iter().filter(|s| s.cat == *cat).map(|s| MenuItem::new(&format!("tool.shape.{}", s.id), s.name)).collect();
                    v.push(MenuItem::new(cat, cat).sub(sub));
                }
                v
            }
            "paper-menu" => Paper::ALL.iter().map(|p| MenuItem::new(&format!("paper:{}", p.name), &format!("{} ({:.0} × {:.0} mm)", p.name, p.w_mm, p.h_mm)).check(self.ed.pg().paper.name == p.name)).collect(),
            "orient-menu" => vec![MenuItem::new("orient:portrait", "Portrait").icon("portrait").check(!st.landscape), MenuItem::new("orient:landscape", "Landscape").icon("landscape").check(st.landscape)],
            "arrow-a" | "arrow-b" => {
                let cur = st.first.as_ref().and_then(|s| s.conn.as_ref()).map(|c| if id == "arrow-a" { c.arrow_a } else { c.arrow_b });
                [("none", "None", Arrow::None), ("triangle", "Filled", Arrow::Triangle), ("open", "Open", Arrow::Open), ("circle", "Dot", Arrow::Circle), ("diamond", "Diamond", Arrow::Diamond), ("bar", "Bar", Arrow::Bar)]
                    .iter()
                    .map(|(k, l, a)| MenuItem::new(&format!("{id}:{k}"), l).check(cur == Some(*a)))
                    .collect()
            }
            "layout-menu" => vec![
                MenuItem::new("layout.down", "Flowchart, top to bottom").icon("flow-down"),
                MenuItem::new("layout.right", "Flowchart, left to right").icon("flow-right"),
                MenuItem::new("layout.tree", "Tree / org chart").icon("tree"),
                MenuItem::new("layout.mind", "Mind map").icon("mind"),
            ],
            "layers-menu" => {
                let page = self.ed.pg();
                let active = self.ed.active_layer();
                let mut v: Vec<MenuItem> = page.layers.iter().map(|l| MenuItem::new(&format!("layer:{}", l.id), &l.name).check(l.id == active)).collect();
                v.push(MenuItem::sep());
                v.push(MenuItem::new("layer-new", "New Layer").icon("plus"));
                v.push(MenuItem::new("pane.layers", "Layers Pane").icon("layers"));
                let to: Vec<MenuItem> = page.layers.iter().map(|l| MenuItem::new(&format!("tolayer:{}", l.id), &l.name)).collect();
                v.push(MenuItem::new("tolayer", "Move Selection to").sub(to).off(!st.has_sel));
                v
            }
            "template-menu" => templates::LIST.iter().map(|(k, n, _)| MenuItem::new(&format!("template.{k}"), n).icon("template")).collect(),
            "theme-menu" => vec![
                MenuItem::new("theme:system", "Same as System").check(self.settings.theme == "system"),
                MenuItem::new("theme:light", "Light").check(self.settings.theme == "light"),
                MenuItem::new("theme:dark", "Dark").check(self.settings.theme == "dark"),
            ],
            "zoom-menu" => {
                let mut v: Vec<MenuItem> = crate::commands::ZOOMS.iter().rev().map(|z| MenuItem::new(&format!("zoom:{z}"), &format!("{:.0}%", z * 100.0)).check((st.zoom - z).abs() < 0.001)).collect();
                v.push(MenuItem::sep());
                v.push(MenuItem::new("zoom.fit", "Fit Page").key("Ctrl+0"));
                v.push(MenuItem::new("zoom.sel", "Fit Selection").off(!st.has_sel));
                v
            }
            "page-menu" => vec![
                MenuItem::new("page-new", "Insert Page").icon("page-add"),
                MenuItem::new("page-mood", "Insert Mood Board Page").icon("palette"),
                MenuItem::new("page-dup", "Duplicate").icon("page-dup"),
                MenuItem::new("page-rename", "Rename").icon("rename"),
                MenuItem::new("page-del", "Delete").icon("delete").off(self.ed.doc.pages.len() < 2),
                MenuItem::sep(),
                MenuItem::new("page-left", "Move Left").off(self.ed.page == 0),
                MenuItem::new("page-right", "Move Right").off(self.ed.page + 1 >= self.ed.doc.pages.len()),
            ],
            _ => return,
        };
        self.ribbon.cancel_tips();
        self.menu = Some(Menu::new(id, rect, items));
    }

    fn context_menu(&mut self, x: f32, y: f32, p: (f32, f32)) {
        let st = self.state_snapshot();
        let mut v = vec![];
        if st.has_sel {
            v.push(MenuItem::new("cut", "Cut").icon("cut").key("Ctrl+X"));
            v.push(MenuItem::new("copy", "Copy").icon("copy").key("Ctrl+C"));
        }
        v.push(MenuItem::new("paste", "Paste").icon("paste").key("Ctrl+V"));
        if st.has_sel {
            v.push(MenuItem::new("duplicate", "Duplicate").icon("duplicate").key("Ctrl+D"));
            v.push(MenuItem::new("delete", "Delete").icon("delete").key("Del"));
            v.push(MenuItem::sep());
            v.push(MenuItem::new("edit-text", "Edit Text").icon("text").key("F2").off(st.multi));
            v.push(MenuItem::new("painter", "Format Painter").icon("painter"));
            if st.multi {
                v.push(MenuItem::new("group", "Group").icon("group").key("Ctrl+G"));
            }
            if st.group {
                v.push(MenuItem::new("ungroup", "Ungroup").icon("ungroup").key("Ctrl+Shift+G"));
            }
            v.push(MenuItem::new("order", "Order").icon("front").sub(vec![
                MenuItem::new("order.front", "Bring to Front"),
                MenuItem::new("order.forward", "Bring Forward"),
                MenuItem::new("order.backward", "Send Backward"),
                MenuItem::new("order.back", "Send to Back"),
            ]));
            let locked = st.first.as_ref().map(|s| s.locked).unwrap_or(false);
            v.push(if locked { MenuItem::new("unlock", "Unlock").icon("unlock") } else { MenuItem::new("lock", "Lock").icon("lock") });
            if st.conn {
                v.push(MenuItem::sep());
                v.push(MenuItem::new("route.elbow", "Right-Angle Connector").icon("route-elbow").check(st.route == Some(Route::Elbow)));
                v.push(MenuItem::new("route.straight", "Straight Connector").icon("route-straight").check(st.route == Some(Route::Straight)));
                v.push(MenuItem::new("route.curved", "Curved Connector").icon("route-curved").check(st.route == Some(Route::Curved)));
            }
            if st.images {
                v.push(MenuItem::sep());
                v.push(MenuItem::new("mood.crop", "Crop…").icon("crop"));
                v.push(MenuItem::new("mood.polaroid", "Polaroid Frame").icon("polaroid"));
                v.push(MenuItem::new("mood.palette", "Extract Palette").icon("palette"));
            }
            v.push(MenuItem::sep());
            v.push(MenuItem::new("comment-here", "Add Comment").icon("comment"));
            v.push(MenuItem::new("save-myshape", "Add to My Shapes").icon("myshapes"));
        } else {
            v.push(MenuItem::new("select-all", "Select All").key("Ctrl+A"));
            v.push(MenuItem::sep());
            v.push(MenuItem::new(&format!("comment-at:{}:{}", p.0, p.1), "Add Comment Here").icon("comment"));
            v.push(MenuItem::new("background", "Background Colour").icon("background"));
        }
        self.menu = Some(Menu::new("context", Rect::new(x, y, 1.0, 1.0), v));
    }

    fn menu_pick(&mut self, menu: &str, id: &str, host: &mut dyn Host, r: &mut Res) {
        if let Some(c) = id.strip_prefix("color:") {
            let target = menu.strip_prefix("color:").unwrap_or("fill").to_string();
            if let Some(c) = Color::parse(c) {
                let c = if c.a == 0 { Color::TRANSPARENT } else { c };
                self.apply_color(&target, c);
            }
            return;
        }
        if menu == "background" {
            // Opened from the context menu: show the palette.
            return;
        }
        if id == "none" {
            match menu {
                "color:fill" => self.ed.set_fill(Color::TRANSPARENT),
                "color:line" => self.ed.set_line(Color::TRANSPARENT),
                _ => {}
            }
            return;
        }
        if id == "hex" {
            let target = menu.strip_prefix("color:").unwrap_or("fill");
            let cur = match target {
                "fill" => self.ed.first().map(|s| s.style.fill),
                "line" => self.ed.first().map(|s| s.style.line),
                _ => None,
            }
            .unwrap_or(Color::hex(0x4472C4));
            self.dialog = Some(Dialog::new("hex", "More colours").text("hex", "Colour code (#RRGGBB)", &cur.to_hex().to_uppercase()).text("target", "Applies to", target).button("ok", "OK").button("cancel", "Cancel"));
            return;
        }
        if let Some(w) = id.strip_prefix("penw:") {
            self.ed.pen_width = w.parse().unwrap_or(3.0);
            self.ed.tool = Tool::Pen;
            self.ed.tool_lock = true;
            return;
        }
        if let Some(f) = id.strip_prefix("font:") {
            self.ed.set_font(f);
            return;
        }
        if let Some(n) = id.strip_prefix("size:") {
            self.ed.set_size(n.parse().unwrap_or(14.0));
            return;
        }
        if let Some(w) = id.strip_prefix("weight:") {
            self.ed.set_weight(w.parse().unwrap_or(1.0));
            return;
        }
        if let Some(d) = id.strip_prefix("dash:") {
            self.ed.set_dash(match d {
                "dash" => Dash::Dash,
                "dot" => Dash::Dot,
                "dashdot" => Dash::DashDot,
                _ => Dash::Solid,
            });
            return;
        }
        if let Some(p) = id.strip_prefix("paper:") {
            self.ed.set_paper(Paper::by_name(p));
            return;
        }
        if let Some(o) = id.strip_prefix("orient:") {
            self.ed.set_landscape(o == "landscape");
            return;
        }
        for (pre, b) in [("arrow-a:", false), ("arrow-b:", true)] {
            if let Some(k) = id.strip_prefix(pre) {
                let a = match k {
                    "triangle" => Arrow::Triangle,
                    "open" => Arrow::Open,
                    "circle" => Arrow::Circle,
                    "diamond" => Arrow::Diamond,
                    "bar" => Arrow::Bar,
                    _ => Arrow::None,
                };
                self.ed.set_arrow(b, a);
                return;
            }
        }
        if let Some(l) = id.strip_prefix("layer:") {
            self.ed.layer = l.parse().ok();
            return;
        }
        if let Some(l) = id.strip_prefix("tolayer:") {
            if let Ok(l) = l.parse() {
                self.ed.move_to_layer(l);
            }
            return;
        }
        if let Some(t) = id.strip_prefix("theme:") {
            self.settings.theme = t.to_string();
            self.apply_theme(host);
            self.settings.save();
            return;
        }
        if let Some(z) = id.strip_prefix("zoom:") {
            let z: f32 = z.parse().unwrap_or(1.0);
            self.ed.zoom_at(z, self.canvas.w / 2.0, self.canvas.h / 2.0);
            return;
        }
        if let Some(rest) = id.strip_prefix("comment-at:") {
            let v: Vec<f32> = rest.split(':').filter_map(|x| x.parse().ok()).collect();
            if v.len() == 2 {
                self.ed.add_comment((v[0], v[1]));
                self.settings.side_pane = true;
                self.side = Side::Comments;
                self.reply.focused = true;
                self.layout();
            }
            return;
        }
        if let Some(n) = id.strip_prefix("delete-my:") {
            self.settings.remove_my_shape(n);
            self.thumbs.retain(|k, _| !k.0.starts_with("my:"));
            return;
        }
        match id {
            "page-left" => self.ed.move_page(-1),
            "page-right" => self.ed.move_page(1),
            "background" => {
                let rect = Rect::new(self.mouse.0, self.mouse.1, 1.0, 1.0);
                self.open_menu("background", rect, r);
            }
            _ => self.command(id, host, r),
        }
    }

    // ---------------- Panes ----------------

    fn side_click(&mut self, id: &str, clicks: u32, host: &mut dyn Host, r: &mut Res) {
        let _ = host;
        let _ = r;
        if let Some(rest) = id.strip_prefix("layer:") {
            let mut it = rest.split(':');
            let lid: Id = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            match it.next() {
                Some("visible") => self.ed.layer_toggle(lid, "visible"),
                Some("locked") => self.ed.layer_toggle(lid, "locked"),
                Some("print") => self.ed.layer_toggle(lid, "print"),
                _ => {
                    self.ed.layer = Some(lid);
                    if clicks >= 2 {
                        let name = self.ed.pg().layers.iter().find(|l| l.id == lid).map(|l| l.name.clone()).unwrap_or_default();
                        self.dialog = Some(Dialog::new("layer-rename", "Rename layer").text("name", "Layer name", &name).text("id", "Layer number", &lid.to_string()).button("ok", "Rename").button("cancel", "Cancel"));
                    }
                }
            }
            return;
        }
        if let Some(rest) = id.strip_prefix("prop:") {
            // Shape Data (K4-02): a row to change, or the bin to take it off
            let (name, del) = match rest.strip_suffix(":delete") {
                Some(n) => (n.to_string(), true),
                None => (rest.to_string(), false),
            };
            if del {
                self.ed.clear_prop(&name);
            } else {
                let value = self.ed.props_of_selection().into_iter().find(|(n, _)| *n == name).map(|(_, v)| v).unwrap_or_default();
                self.dialog = Some(
                    Dialog::new("prop-edit", "Shape data")
                        .text("name", "Name", &name)
                        .text("value", "Value", &value)
                        .text("was", "", &name)
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            return;
        }
        if id == "prop-add" {
            self.dialog = Some(
                Dialog::new("prop-edit", "Shape data")
                    .text("name", "Name", "")
                    .text("value", "Value", "")
                    .text("was", "", "")
                    .button("ok", "OK")
                    .button("cancel", "Cancel"),
            );
            return;
        }
        if let Some(rest) = id.strip_prefix("comment:") {
            let cid: Id = rest.split(':').next().and_then(|x| x.parse().ok()).unwrap_or(0);
            if rest.ends_with(":resolve") {
                self.ed.checkpoint("Resolve comment", "");
                if let Some(c) = self.ed.pgm().comments.iter_mut().find(|c| c.id == cid) {
                    c.resolved = !c.resolved;
                }
                self.ed.changed();
            } else if rest.ends_with(":delete") {
                self.ed.checkpoint("Delete comment", "");
                self.ed.pgm().comments.retain(|c| c.id != cid);
                if self.ed.open_comment == Some(cid) {
                    self.ed.open_comment = None;
                }
                self.ed.changed();
            } else {
                self.ed.open_comment = Some(cid);
                self.reply.focused = true;
                // Bring the pin into view.
                if let Some(c) = self.ed.pg().comments.iter().find(|c| c.id == cid).cloned() {
                    let v = self.ed.view();
                    let (sx, sy) = v.to_screen(c.x, c.y);
                    if sx < 0.0 || sy < 0.0 || sx > self.canvas.w || sy > self.canvas.h {
                        let mut v = v;
                        v.ox += self.canvas.w / 2.0 - sx;
                        v.oy += self.canvas.h / 2.0 - sy;
                        self.ed.set_view(v);
                    }
                    if let Some(sid) = c.shape {
                        self.ed.sel = vec![sid];
                    }
                }
            }
            return;
        }
        match id {
            "tab:layers" => self.side = Side::Layers,
            "tab:comments" => self.side = Side::Comments,
            "tab:data" => self.side = Side::Data,
            "close-side" => {
                self.settings.side_pane = false;
                self.layout();
            }
            "layer-new" => {
                self.ed.add_layer("");
            }
            "layer-del" => {
                let l = self.ed.active_layer();
                self.ed.delete_layer(l);
            }
            "layer-move" => {
                let l = self.ed.active_layer();
                self.ed.move_to_layer(l);
            }
            "comment-new" => {
                let p = self.ed.shape_at_center(self.canvas.w, self.canvas.h);
                self.ed.add_comment(p);
                self.reply.focused = true;
            }
            "reply-send" => self.post_reply(),
            "show-resolved" => self.settings.show_resolved = !self.settings.show_resolved,
            _ => {}
        }
    }

    fn post_reply(&mut self) {
        let t = self.reply.text.trim().to_string();
        if t.is_empty() {
            return;
        }
        let Some(cid) = self.ed.open_comment else { return };
        self.ed.checkpoint("Comment", "");
        let author = self.ed.author.clone();
        if let Some(c) = self.ed.pgm().comments.iter_mut().find(|c| c.id == cid) {
            if c.text.is_empty() {
                c.text = t;
                c.author = author;
                c.time = crate::editor::now_secs();
            } else {
                c.replies.push(Reply { author, text: t, time: crate::editor::now_secs() });
            }
        }
        self.ed.changed();
        self.reply.set("");
    }

    fn bar_click(&mut self, id: &str, rect: Rect, button: Button, host: &mut dyn Host, r: &mut Res) {
        if let Some(i) = id.strip_prefix("page:") {
            let i: usize = i.parse().unwrap_or(0);
            let double = self.last_click_tab.map(|(j, t)| j == i && self.now.saturating_sub(t) < Duration::from_millis(450)).unwrap_or(false);
            self.last_click_tab = Some((i, self.now));
            if i != self.ed.page {
                self.ed.go_page(i);
                if !self.ed.views.contains_key(&self.ed.pg().id) {
                    self.fit();
                }
            }
            if button == Button::Right {
                self.open_menu("page-menu", rect, r);
            } else if double {
                self.command("page-rename", host, r);
            }
            return;
        }
        match id {
            "page-add" => {
                self.ed.add_page(false);
                self.fit();
            }
            "zoom-menu" => self.open_menu("zoom-menu", Rect::new(rect.x, rect.y - 400.0 * self.s, rect.w, 1.0), r),
            "zoom-out" => self.command("zoom.out", host, r),
            "zoom-in" => self.command("zoom.in", host, r),
            "fit" => self.fit(),
            _ => {}
        }
    }

    // ---------------- Backstage ----------------

    fn backstage_event(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        let Some(mut b) = self.backstage.take() else { return };
        let mut close = false;
        if let Event::Key { key: Key::Escape, down: true, .. } = ev {
            close = true;
        }
        if let Event::MouseDown { x, y, button: Button::Left, .. } = ev {
            if let Some((id, _)) = b.hits.iter().find(|(_, rr)| rr.contains(*x, *y)).cloned() {
                match id.as_str() {
                    "back" => close = true,
                    "nav:new" => self.backstage_page(&mut b, BackPage::New),
                    "nav:open" => self.backstage_page(&mut b, BackPage::Open),
                    "nav:save" => {
                        self.backstage = Some(b);
                        if self.ed.path.is_some() {
                            self.save(host, r);
                            self.backstage = None;
                        } else if let Some(mut bb2) = self.backstage.take() {
                            self.backstage_page(&mut bb2, BackPage::SaveAs);
                            self.backstage = Some(bb2);
                        }
                        self.layout();
                        return;
                    }
                    "nav:saveas" => self.backstage_page(&mut b, BackPage::SaveAs),
                    "nav:export" => self.backstage_page(&mut b, BackPage::Export),
                    "nav:print" => self.backstage_page(&mut b, BackPage::Print),
                    "nav:options" => self.backstage_page(&mut b, BackPage::Options),
                    "nav:about" => self.backstage_page(&mut b, BackPage::About),
                    "nav:close" => {
                        self.backstage = None;
                        self.request(After::Close, host);
                        self.layout();
                        return;
                    }
                    x if x.starts_with("tpl:") => {
                        let t = x[4..].to_string();
                        self.backstage = None;
                        self.layout();
                        self.request(After::New(t), host);
                        return;
                    }
                    x if x.starts_with("recent:") => {
                        let p = PathBuf::from(&x[7..]);
                        self.backstage = None;
                        self.layout();
                        self.request(After::Open(p), host);
                        return;
                    }
                    x if x.starts_with("fmt:") => {
                        b.export_fmt = x[4..].parse().unwrap_or(2);
                        let name = b.files.as_ref().map(|f| f.name.text.clone()).unwrap_or_default();
                        self.backstage_page(&mut b, BackPage::Export);
                        if let Some(f) = b.files.as_mut() {
                            let ext = ["png", "svg", "pdf"][b.export_fmt];
                            f.name.set(&format!("{}.{ext}", stem(&name)));
                        }
                    }
                    "export-all" => b.export_all = !b.export_all,
                    "export-scale" => b.export_scale = (b.export_scale + 1) % 3,
                    "print-fit" => b.print_fit = !b.print_fit,
                    "print-all" => b.print_all = !b.print_all,
                    "copies-up" => b.copies = (b.copies + 1).min(99),
                    "copies-down" => b.copies = b.copies.saturating_sub(1).max(1),
                    "printer" => b.printer = if b.printers.is_empty() { 0 } else { (b.printer + 1) % b.printers.len() },
                    "do-print" => {
                        let pages: Vec<usize> = if b.print_all { (0..self.ed.doc.pages.len()).collect() } else { vec![self.ed.page] };
                        let tmp = folio_kit::platform::temp_file("folio-cork-print", "pdf");
                        let res = export::pdf(&self.ed.doc, &pages, &tmp, &mut r.text, &mut r.images, b.print_fit, true)
                            .and_then(|_| folio_kit::platform::print_pdf(&tmp, b.copies, b.printers.get(b.printer).map(|s| s.as_str())));
                        match res {
                            Ok(()) => {
                                self.flash("Sent to the printer.");
                                close = true;
                            }
                            Err(e) => b.msg = e,
                        }
                    }
                    "print-pdf" => {
                        b.export_fmt = 2;
                        self.backstage_page(&mut b, BackPage::Export);
                    }
                    "opt:theme" => {
                        self.settings.theme = match self.settings.theme.as_str() {
                            "system" => "light".into(),
                            "light" => "dark".into(),
                            _ => "system".into(),
                        };
                        self.apply_theme(host);
                        self.settings.save();
                    }
                    "opt:grid" => {
                        let sizes = [6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0];
                        let i = sizes.iter().position(|x| (*x - self.ed.grid_size).abs() < 0.1).unwrap_or(3);
                        self.ed.grid_size = sizes[(i + 1) % sizes.len()];
                        self.settings.grid_size = self.ed.grid_size;
                        self.settings.save();
                    }
                    "opt:author" => {
                        self.backstage = Some(b);
                        let a = self.ed.author.clone();
                        self.dialog = Some(Dialog::new("author", "Your name").text("name", "Name shown on comments", &a).button("ok", "OK").button("cancel", "Cancel"));
                        return;
                    }
                    _ => {}
                }
            }
        }
        if let Some(f) = b.files.as_mut() {
            if let FileAction::Chosen(p) = f.event(ev, host, &mut r.text, self.s) {
                match (b.page, b.msg.as_str()) {
                    (BackPage::Open, "pictures") => {
                        if let Ok(bytes) = std::fs::read(&p) {
                            let at = self.ed.shape_at_center(self.canvas.w, self.canvas.h);
                            mood::insert_picture(&mut self.ed, bytes, at, &mut r.images);
                            close = true;
                        } else {
                            f.error = "Can't read that file.".into();
                        }
                    }
                    (BackPage::Open, "orgchart") => match layout::org_from_file(&mut self.ed, &p, &mut r.text) {
                        Ok(n) => {
                            self.flash(&format!("Org chart with {n} people."));
                            close = true;
                            self.layout();
                            self.fit();
                        }
                        Err(e) => f.error = e,
                    },
                    (BackPage::Open, _) => {
                        self.backstage = None;
                        self.layout();
                        self.request(After::Open(p), host);
                        return;
                    }
                    (BackPage::SaveAs, _) => match files::save(&self.ed.doc, &p) {
                        Ok(()) => {
                            self.ed.path = Some(p.clone());
                            self.ed.dirty = false;
                            self.settings.add_recent(&p);
                            self.flash("Saved.");
                            close = true;
                            if self.pending.is_some() {
                                self.after = self.pending.take();
                            }
                        }
                        Err(e) => f.error = e,
                    },
                    (BackPage::Export, _) => {
                        let pages: Vec<usize> = if b.export_all { (0..self.ed.doc.pages.len()).collect() } else { vec![self.ed.page] };
                        let scale = [1.0, 2.0, 4.0][b.export_scale];
                        let res = match b.export_fmt {
                            0 => export::png(&self.ed.doc, &pages, &p, &mut r.text, &mut r.images, scale),
                            1 => export::svg(&self.ed.doc, &pages, &p, &mut r.text),
                            _ => export::pdf(&self.ed.doc, &pages, &p, &mut r.text, &mut r.images, true, false),
                        };
                        match res {
                            Ok(()) => {
                                self.flash(&format!("Exported {}", p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()));
                                close = true;
                            }
                            Err(e) => f.error = e,
                        }
                    }
                    _ => {}
                }
            }
        }
        if close {
            self.backstage = None;
        } else {
            self.backstage = Some(b);
        }
        self.layout();
    }

    // ---------------- Painting ----------------

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

        let title = self.window_title();
        let maximized = false;
        {
            let st = self.state_snapshot();
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            let tr = Rect::new(0.0, 0.0, self.w, TITLE_H * s);
            self.title.paint(&mut cx, tr, &title, maximized, &|id| match id {
                "undo" => st.can_undo,
                "redo" => st.can_redo,
                _ => true,
            });
        }
        if self.backstage.is_some() {
            self.paint_backstage(pm, r);
        } else {
            self.paint_canvas(pm, r);
            self.paint_left(pm, r);

            self.paint_right(pm, r);
            self.paint_bar(pm, r);
            let st = self.state_snapshot();
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
                gfx::shadow(cx.pm, fr, 6.0 * s, theme.dark);
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
        // Window border for the borderless window.
        gfx::stroke_round(cx.pm, win, 0.0, theme.border, 1.0);
    }

    fn paint_canvas(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let c = self.canvas;
        let theme = self.theme.clone();
        let s = self.s;
        let v = self.ed.view();
        let (cw, ch) = (c.w.max(1.0) as u32, c.h.max(1.0) as u32);
        let hide = self.ed.edit.as_ref().map(|e| e.id);
        self.update_layer(r, v, (cw, ch), theme.dark, hide);

        if let Some(cc) = &self.cache {
            gfx::blit_copy(pm, &cc.layer, c.x as i32, c.y as i32);
        }
        // Overlays in a clipped sub-canvas.
        let mut ov = sk::Pixmap::new(cw, ch).unwrap();
        self.paint_overlays(&mut ov, r, v);

        gfx::blit_over(pm, &ov, c.x as i32, c.y as i32);
        if self.settings.rulers {
            self.paint_rulers(pm, r);
        }
        if self.settings.minimap {
            self.paint_minimap(pm, r);
        }
        let _ = s;
    }

    /// Bring the drawing layer up to date, redrawing as little as possible (X3-12: 60 fps on a
    /// Pi): when the view only moved, the old pixels shift and only the strips that came into
    /// view are drawn; when a few shapes changed, only the boxes around them are redrawn.
    /// Everything else (zoom, page, layers, grid, theme) redraws the whole layer.
    fn update_layer(&mut self, r: &mut Res, v: View, size: (u32, u32), dark: bool, hide: Option<Id>) {
        let (cw, ch) = size;
        if matches!(&self.cache, Some(cc) if cc.ver == self.ed.version && cc.view == v && cc.size == size && cc.dark == dark && cc.hide == hide) {
            return;
        }
        let page = self.ed.pg();
        let vis = |v: View| {
            let (x0, y0) = v.to_page(0.0, 0.0);
            let (x1, y1) = v.to_page(cw as f32, ch as f32);
            (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
        };
        let pkey = self.page_key(page, v.zoom, dark, hide, size);
        let prints = render::shape_prints(page, vis(v));
        let full_only = folio_kit::platform::setting("FOLIO_FULL_REDRAW").is_some();
        let mut dirty: Vec<(i32, i32, i32, i32)> = vec![];
        let mut reuse = None;
        if let Some(mut cc) = self.cache.take() {
            let (dx, dy) = (v.ox - cc.view.ox, v.oy - cc.view.oy);
            let shift_ok = cc.pkey == pkey && cc.size == size && dx.fract() == 0.0 && dy.fract() == 0.0 && dx.abs() < cw as f32 && dy.abs() < ch as f32;
            if shift_ok && !full_only && !render::jumps_changed(page, &cc.prints, &prints) {
                let (dx, dy) = (dx as i32, dy as i32);
                if dx != 0 || dy != 0 {
                    let old = cc.layer.clone();
                    gfx::blit_copy(&mut cc.layer, &old, dx, dy);
                    if dx > 0 { dirty.push((0, 0, dx, ch as i32)); }
                    if dx < 0 { dirty.push((cw as i32 + dx, 0, cw as i32, ch as i32)); }
                    if dy > 0 { dirty.push((0, 0, cw as i32, dy)); }
                    if dy < 0 { dirty.push((0, ch as i32 + dy, cw as i32, ch as i32)); }
                }
                // Shapes that changed, appeared or went: their old and new boxes, on screen now.
                let boxes = render::changed_boxes(&cc.prints, &prints);
                for b in boxes {
                    let (x0, y0) = v.to_screen(b.0, b.1);
                    let (x1, y1) = v.to_screen(b.2, b.3);
                    dirty.push(((x0.floor() as i32) - 2, (y0.floor() as i32) - 2, (x1.ceil() as i32) + 2, (y1.ceil() as i32) + 2));
                }
                reuse = Some(cc.layer);
            } else if cc.layer.width() == cw && cc.layer.height() == ch {
                reuse = Some(cc.layer);
                dirty.push((0, 0, cw as i32, ch as i32));
            } else {
                dirty.push((0, 0, cw as i32, ch as i32));
            }
        } else {
            dirty.push((0, 0, cw as i32, ch as i32));
        }
        let mut layer = reuse.unwrap_or_else(|| sk::Pixmap::new(cw, ch).unwrap());
        let rects = gfx::merge_boxes(dirty, cw as i32, ch as i32);
        for (x0, y0, x1, y1) in rects {
            let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
            if w == 0 || h == 0 {
                continue;
            }
            // The box drawn on its own, with the view moved so the box's corner is at 0, 0.
            let sv = View { zoom: v.zoom, ox: v.ox - x0 as f32, oy: v.oy - y0 as f32 };
            let mut sub = sk::Pixmap::new(w, h).unwrap();
            sub.fill(page.background.to_sk());
            self.paint_grid(&mut sub, page, sv);
            let opts = render::DrawOpts { hide_text: hide, visible: Some(vis_box(sv, w, h)), ..Default::default() };
            render::draw_page(&mut sub, &self.ed.doc, page, &mut r.text, &mut r.images, sv.xf(), &opts);
            gfx::blit_copy(&mut layer, &sub, x0, y0);
        }
        self.cache = Some(CanvasCache { ver: self.ed.version, view: v, size, dark, hide, pkey, prints, layer });
    }

    /// Everything about the page other than its shapes that changes how the layer looks.
    /// Drop the drawn layer so the next frame draws everything (tests compare the two).
    pub(crate) fn forget_layer(&mut self) {
        self.cache = None;
    }

    fn page_key(&self, page: &Page, zoom: f32, dark: bool, hide: Option<Id>, size: (u32, u32)) -> u64 {
        let mut h = render::Fnv::default();
        use std::fmt::Write;
        let tiles = if self.settings.page_breaks { render::content_bounds(page).map(|b| (b.0 + b.2, b.1 + b.3)) } else { None };
        let _ = write!(h, "{} {:?} {:?} {:?} {} {} {} {:?} {:?} {:?} {:?}", page.id, page.paper.px(), page.background, page.layers, self.ed.grid, self.ed.grid_size, self.settings.page_breaks, tiles, zoom.to_bits(), (dark, hide), size);
        h.0
    }

    fn paint_grid(&self, pm: &mut sk::Pixmap, page: &Page, v: View) {
        let (w, h) = (pm.width() as f32, pm.height() as f32);
        let bg = page.background;
        let dark_bg = bg.luma() < 0.4;
        // Print page outline and page breaks.
        let (pw, ph) = page.paper.px();
        if self.settings.page_breaks {
            let tiles_x = render::content_bounds(page).map(|b| ((b.0 + b.2) / pw).ceil().max(1.0) as i32).unwrap_or(1).min(20);
            let tiles_y = render::content_bounds(page).map(|b| ((b.1 + b.3) / ph).ceil().max(1.0) as i32).unwrap_or(1).min(20);
            let col = if dark_bg { Color::rgba(255, 255, 255, 50) } else { Color::rgba(0, 0, 0, 40) };
            for i in 0..tiles_x.max(1) {
                for j in 0..tiles_y.max(1) {
                    let (x0, y0) = v.to_screen(i as f32 * pw, j as f32 * ph);
                    let r = Rect::new(x0, y0, pw * v.zoom, ph * v.zoom);
                    if let Some(p) = gfx::rounded_path(r, 0.0) {
                        let mut st = sk::Stroke { width: 1.0, ..Default::default() };
                        st.dash = sk::StrokeDash::new(vec![6.0, 4.0], 0.0);
                        pm.stroke_path(&p, &gfx::paint(col), &st, sk::Transform::identity(), None);
                    }
                }
            }
        }
        if !self.ed.grid {
            return;
        }
        let mut step = self.ed.grid_size * v.zoom;
        let mut major = 5;
        while step < 9.0 {
            step *= 5.0;
            major = 4;
        }
        let gcol = if dark_bg { Color::rgba(255, 255, 255, 26) } else { Color::rgba(0, 0, 0, 22) };
        let mcol = if dark_bg { Color::rgba(255, 255, 255, 46) } else { Color::rgba(0, 0, 0, 40) };
        let x0 = v.ox.rem_euclid(step);
        let y0 = v.oy.rem_euclid(step);
        let ix0 = ((-v.ox) / step).floor() as i64;
        let iy0 = ((-v.oy) / step).floor() as i64;
        let mut i = 0;
        let mut x = x0;
        while x < w {
            let n = ix0 + 1 + i;
            gfx::vline(pm, x, 0.0, h, if n.rem_euclid(major) == 0 { mcol } else { gcol });
            x += step;
            i += 1;
        }
        let mut j = 0;
        let mut y = y0;
        while y < h {
            let n = iy0 + 1 + j;
            gfx::hline(pm, 0.0, w, y, if n.rem_euclid(major) == 0 { mcol } else { gcol });
            y += step;
            j += 1;
        }
    }

    fn paint_overlays(&mut self, ov: &mut sk::Pixmap, r: &mut Res, v: View) {
        let theme = self.theme.clone();
        let s = self.s;
        let acc = Color::hex(0x2F80ED);
        let page = self.ed.pg().clone();
        let to = |x: f32, y: f32| v.to_screen(x, y);
        let stroke_poly = |pm: &mut sk::Pixmap, pts: &[(f32, f32)], c: Color, w: f32, dash: bool| {
            let mut pb = sk::PathBuilder::new();
            for (i, p) in pts.iter().enumerate() {
                if i == 0 {
                    pb.move_to(p.0, p.1)
                } else {
                    pb.line_to(p.0, p.1)
                }
            }
            if let Some(p) = pb.finish() {
                let mut st = sk::Stroke { width: w, ..Default::default() };
                if dash {
                    st.dash = sk::StrokeDash::new(vec![4.0, 3.0], 0.0);
                }
                pm.stroke_path(&p, &gfx::paint(c), &st, sk::Transform::identity(), None);
            }
        };
        // Hover outline.
        if let Some(hid) = self.ed.hover {
            if !self.ed.sel.contains(&hid) && matches!(self.ed.drag, Drag::None) {
                if let Some(sh) = page.get(hid) {
                    if !sh.is_connector() {
                        for poly in crate::geom::outline(sh) {
                            let pts: Vec<(f32, f32)> = poly.iter().map(|&(x, y)| to(x, y)).collect();
                            stroke_poly(ov, &pts, acc.with_alpha(150), 1.5, false);
                        }
                    }
                }
            }
        }
        // Connection points when connecting or hovering with the connector tool.
        let show_points: Vec<Id> = match (&self.ed.tool, &self.ed.drag) {
            (_, Drag::Connect { target: Some((id, _)), .. }) => vec![*id],
            (_, Drag::ConnEnd { .. }) => self.ed.hover.into_iter().collect(),
            (Tool::Connector, _) => self.ed.hover.into_iter().collect(),
            (Tool::Pointer, Drag::None) => self.ed.hover_point.map(|h| h.0).into_iter().collect(),
            _ => vec![],
        };
        for id in show_points {
            if let Some(sh) = page.get(id) {
                for (i, p) in crate::geom::conn_points(sh).into_iter().enumerate() {
                    let (x, y) = to(p.0, p.1);
                    let hot = self.ed.hover_point == Some((id, i)) || matches!(&self.ed.drag, Drag::Connect { target: Some((tid, Some(ti))), .. } if *tid == id && *ti == i);
                    let d = if hot { 5.0 } else { 3.5 } * s;
                    gfx::line(ov, x - d, y - d, x + d, y + d, if hot { Color::hex(0x00A651) } else { acc }, 1.8);
                    gfx::line(ov, x - d, y + d, x + d, y - d, if hot { Color::hex(0x00A651) } else { acc }, 1.8);
                }
            }
        }
        // Selection.
        if !self.ed.sel.is_empty() && self.ed.edit.is_none() {
            let single_conn = self.ed.sel.len() == 1 && self.ed.first().map(|s| s.is_connector()).unwrap_or(false);
            for id in self.ed.sel.clone() {
                if let Some(sh) = page.get(id) {
                    if sh.is_connector() {
                        let rt = crate::route::route(&page, sh);
                        let pts: Vec<(f32, f32)> = rt.polyline().iter().map(|&(x, y)| to(x, y)).collect();
                        stroke_poly(ov, &pts, acc.with_alpha(110), 5.0, false);
                        if single_conn {
                            let c = sh.conn.clone().unwrap();
                            for (p, glued) in [(rt.pts.first(), c.a.shape.is_some()), (rt.pts.last(), c.b.shape.is_some())] {
                                if let Some(p) = p {
                                    let (x, y) = to(p.0, p.1);
                                    let r = Rect::new(x - 5.0 * s, y - 5.0 * s, 10.0 * s, 10.0 * s);
                                    gfx::fill_round(ov, r, 2.0, if glued { Color::hex(0x00A651) } else { Color::WHITE });
                                    gfx::stroke_round(ov, r, 2.0, if glued { Color::hex(0x00753A) } else { acc }, 1.5);
                                }
                            }
                        }
                    } else if self.ed.sel.len() > 1 || sh.rot.abs() > 0.01 {
                        for poly in crate::geom::outline(sh).into_iter().take(1) {
                            let _ = poly;
                        }
                        let (x, y, w, h) = crate::geom::bounds(sh);
                        let (a, b) = to(x, y);
                        let rr = Rect::new(a, b, w * v.zoom, h * v.zoom);
                        gfx::stroke_round(ov, rr, 0.0, acc.with_alpha(150), 1.0);
                    }
                }
            }
            if !single_conn {
                if let Some((x, y, w, h)) = self.ed.sel_bounds() {
                    let (a, b) = to(x, y);
                    let rr = Rect::new(a, b, w * v.zoom, h * v.zoom);
                    if let Some(p) = gfx::rounded_path(rr, 0.0) {
                        let mut st = sk::Stroke { width: 1.0, ..Default::default() };
                        st.dash = sk::StrokeDash::new(vec![4.0, 3.0], 0.0);
                        ov.stroke_path(&p, &gfx::paint(acc), &st, sk::Transform::identity(), None);
                    }
                    let locked = self.ed.sel.iter().any(|id| !self.ed.editable(*id));
                    let hs = crate::editor::HANDLE * s;
                    for (i, (hx, hy)) in handle_points(x, y, w, h, v.zoom).iter().enumerate() {
                        let (sx, sy) = to(*hx, *hy);
                        if locked {
                            if i == 0 {
                                let lr = Rect::new(sx - 9.0 * s, sy - 20.0 * s, 16.0 * s, 16.0 * s);
                                let mut cx = Ctx { pm: ov, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
                                cx.icon_colored("lock", lr, acc);
                            }
                            continue;
                        }
                        if i == 8 {
                            let (tx, ty) = to(x + w / 2.0, y);
                            gfx::line(ov, tx, ty, sx, sy, acc, 1.0);
                            let rr = Rect::new(sx - hs / 1.3, sy - hs / 1.3, hs * 1.55, hs * 1.55);
                            gfx::fill_round(ov, rr, hs, Color::WHITE);
                            gfx::stroke_round(ov, rr, hs, acc, 1.5);
                            continue;
                        }
                        let rr = Rect::new(sx - hs / 2.0 - 1.0, sy - hs / 2.0 - 1.0, hs + 2.0, hs + 2.0);
                        gfx::fill_round(ov, rr, 2.0, Color::WHITE);
                        gfx::stroke_round(ov, rr, 2.0, acc, 1.5);
                    }
                }
            }
        }
        // Drags in progress.
        match self.ed.drag.clone() {
            Drag::Marquee { x0, y0, x1, y1, .. } => {
                let (a, b) = to(x0.min(x1), y0.min(y1));
                let rr = Rect::new(a, b, (x1 - x0).abs() * v.zoom, (y1 - y0).abs() * v.zoom);
                gfx::fill_rect(ov, rr, acc.with_alpha(28));
                gfx::stroke_round(ov, rr, 0.0, acc, 1.0);
            }
            Drag::NewShape { kind, x0, y0, x1, y1 } => {
                let mut sh = self.ed.make_shape(&kind, 0.0, 0.0);
                sh.x = x0.min(x1);
                sh.y = y0.min(y1);
                sh.w = (x1 - x0).abs().max(2.0);
                sh.h = (y1 - y0).abs().max(2.0);
                sh.style.opacity = 0.6;
                let doc = self.ed.doc.clone();
                render::draw_shape(ov, &doc, &sh, &mut r.text, &mut r.images, v.xf(), true);
            }
            Drag::Connect { from, to: pt, target } => {
                let a = match from.shape.and_then(|id| page.get(id)) {
                    Some(sh) => from.point.and_then(|i| crate::geom::conn_point(sh, i)).unwrap_or((sh.cx(), sh.cy())),
                    None => (from.x, from.y),
                };
                let b = match target.and_then(|(id, pi)| page.get(id).map(|sh| (sh, pi))) {
                    Some((sh, Some(i))) => crate::geom::conn_point(sh, i).unwrap_or(pt),
                    _ => pt,
                };
                stroke_poly(ov, &[to(a.0, a.1), to(b.0, b.1)], acc, 1.5, true);
            }
            _ => {}
        }
        // Smart guides.
        for g in &self.ed.guides {
            let col = Color::hex(0xE0007A);
            if g.vertical {
                let (x, y0) = to(g.at, g.from);
                let (_, y1) = to(g.at, g.to);
                gfx::line(ov, x, y0, x, y1, col, 1.0);
            } else {
                let (x0, y) = to(g.from, g.at);
                let (x1, _) = to(g.to, g.at);
                gfx::line(ov, x0, y, x1, y, col, 1.0);
            }
        }
        // Text being edited: the text, its selection and caret.
        if let Some(ed) = self.ed.edit.clone() {
            if let Some((sh, lay, origin)) = self.ed.edit_layout(&mut r.text) {
                let xf = if sh.is_connector() { v.xf().pre_translate(origin.0, origin.1) } else { crate::geom::shape_xf(&sh).post_concat(v.xf()).pre_translate(origin.0, origin.1) };
                let (a, b) = (ed.caret.min(ed.anchor), ed.caret.max(ed.anchor));
                if a != b {
                    for l in &lay.lines {
                        let (s0, s1) = (a.max(l.start), b.min(l.end));
                        if s0 > s1 || (s0 == s1 && !(a <= l.start && b > l.end)) {
                            continue;
                        }
                        let (xa, _, _) = lay.caret(s0);
                        let (xb, _, _) = lay.caret(s1);
                        let xb = if b > l.end { l.x + l.width + 4.0 } else { xb };
                        if let Some(p) = gfx::rounded_path(Rect::new(xa, l.top, (xb - xa).max(2.0), l.height), 0.0) {
                            ov.fill_path(&p, &gfx::paint(acc.with_alpha(80)), sk::FillRule::Winding, xf, None);
                        }
                    }
                }
                // The text itself (hidden in the cached layer).
                let st = if sh.kind == "frame" {
                    TextStyle { size: 14.0, bold: true, color: sh.tstyle.color, ..Default::default() }
                } else {
                    let (_, tb, size, _) = render::text_origin(&mut r.text, &sh);
                    let _ = tb;
                    render::text_style(&sh, if sh.is_connector() { sh.tstyle.size } else { size })
                };
                let width = if sh.kind == "frame" || sh.is_connector() { None } else { Some(render::text_fit(&mut r.text, &sh).0 .2) };
                for p in r.text.outlines(&sh.text, &st, width) {
                    ov.fill_path(&p, &gfx::paint(st.color), sk::FillRule::Winding, xf, None);
                }
                let (cx, ctop, chh) = lay.caret(ed.caret);
                let mut pb = sk::PathBuilder::new();
                pb.move_to(cx, ctop);
                pb.line_to(cx, ctop + chh);
                if let Some(p) = pb.finish() {
                    let blink = (self.now.as_millis() / 530) % 2 == 0;
                    if blink || self.test.is_some() {
                        let stw = sk::Stroke { width: 1.5 / v.zoom.max(0.1), ..Default::default() };
                        ov.stroke_path(&p, &gfx::paint(theme.text.mix(Color::BLACK, if theme.dark { 0.0 } else { 0.3 })), &stw, xf, None);
                    }
                }
                // Edit box outline.
                if !sh.is_connector() {
                    let g = render::text_fit(&mut r.text, &sh).0;
                    let bx = [crate::geom::Seg::M(g.0, g.1), crate::geom::Seg::L(g.0 + g.2, g.1), crate::geom::Seg::L(g.0 + g.2, g.1 + g.3), crate::geom::Seg::L(g.0, g.1 + g.3), crate::geom::Seg::Z];
                    if let Some(p) = crate::geom::to_path(&bx) {
                        let mut st = sk::Stroke { width: 1.0, ..Default::default() };
                        st.dash = sk::StrokeDash::new(vec![3.0, 3.0], 0.0);
                        ov.stroke_path(&p, &gfx::paint(acc.with_alpha(140)), &st, crate::geom::shape_xf(&sh).post_concat(v.xf()), None);
                    }
                }
            }
        }
        // Comment pins.
        let mut cx = Ctx { pm: ov, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        for c in page.comments.iter() {
            if c.resolved && !self.settings.show_resolved {
                continue;
            }
            let (px, py) = self.pin_pos(c, &v);
            let (px, py) = (px - self.canvas.x, py - self.canvas.y);
            let open = self.ed.open_comment == Some(c.id);
            let pr = Rect::new(px - 2.0 * s, py - 24.0 * s, 24.0 * s, 22.0 * s);
            gfx::shadow(cx.pm, pr, 6.0 * s, theme.dark);
            gfx::fill_round(cx.pm, pr, 6.0 * s, if c.resolved { Color::hex(0x9E9E9E) } else if open { ACCENT } else { Color::hex(0xF5B400) });
            cx.icon_colored("comment", Rect::new(pr.x + 4.0 * s, pr.y + 3.0 * s, 16.0 * s, 16.0 * s), Color::hex(0x2B2B2B));
        }
        // People working with you.
        if let Some(cb) = &self.collab {
            for (name, pid, p, col) in cb.cursors() {
                if pid != page.id {
                    continue;
                }
                let (x, y) = to(p.0, p.1);
                let mut pb = sk::PathBuilder::new();
                pb.move_to(x, y);
                pb.line_to(x + 4.0 * s, y + 15.0 * s);
                pb.line_to(x + 7.0 * s, y + 9.0 * s);
                pb.line_to(x + 13.0 * s, y + 8.0 * s);
                pb.close();
                if let Some(path) = pb.finish() {
                    cx.pm.fill_path(&path, &gfx::paint(col), sk::FillRule::Winding, sk::Transform::identity(), None);
                }
                let w = cx.measure(&name, 11.0, false) + 10.0 * s;
                let lr = Rect::new(x + 12.0 * s, y + 14.0 * s, w, 17.0 * s);
                gfx::fill_round(cx.pm, lr, 3.0 * s, col);
                cx.label(&name, lr, 11.0, col.contrast_text(), false, Align::Center);
            }
        }
        // Format painter and tool hint.
        let hint = match (&self.ed.tool, self.ed.format_brush.is_some()) {
            (_, true) => Some("Format Painter: click a shape. Esc cancels."),
            (Tool::Pen, _) | (Tool::Highlighter, _) => Some("Drawing. Esc or V returns to the pointer."),
            (Tool::Vote, _) => Some("Voting: click shapes to add a dot, Shift+click to remove one. Esc ends voting."),
            (Tool::Eyedropper, _) => Some("Click a picture or shape to make a swatch of its colour."),
            (Tool::Connector, _) => Some("Drag from one shape to another. Esc ends."),
            _ => None,
        };
        if let Some(h) = hint {
            let w = cx.measure(h, 12.5, false) + 24.0 * s;
            let hr = Rect::new((cx.pm.width() as f32 - w) / 2.0, 10.0 * s, w, 28.0 * s);
            gfx::fill_round(cx.pm, hr, 14.0 * s, Color::rgba(30, 30, 30, 210));
            cx.label(h, hr, 12.5, Color::WHITE, false, Align::Center);
        }
    }

    fn paint_rulers(&self, pm: &mut sk::Pixmap, r: &mut Res) {
        let s = self.s;
        let t = &self.theme;
        let c = self.canvas;
        let rs = 20.0 * s;
        let top = Rect::new(c.x - rs, c.y - rs, c.w + rs, rs);
        let left = Rect::new(c.x - rs, c.y, rs, c.h);
        gfx::fill_rect(pm, top, t.panel);
        gfx::fill_rect(pm, left, t.panel);
        gfx::hline(pm, top.x, top.right(), top.bottom() - 1.0, t.border);
        gfx::vline(pm, left.right() - 1.0, top.y, left.bottom(), t.border);
        let v = self.ed.view();
        // Ruler units: centimetres (96 px per inch).
        let unit = 96.0 / 2.54;
        let mut step = unit;
        while step * v.zoom < 40.0 * s {
            step *= if (step / unit).round() as i32 % 5 == 0 { 2.0 } else { 5.0 };
        }
        while step * v.zoom > 160.0 * s && step > unit / 10.0 {
            step /= 2.0;
        }
        let st = TextStyle { size: 9.5 * s, color: t.muted, ..Default::default() };
        let (px0, _) = v.to_page(0.0, 0.0);
        let (px1, _) = v.to_page(c.w, 0.0);
        let mut x = (px0 / step).floor() * step;
        while x <= px1 {
            let (sx, _) = v.to_screen(x, 0.0);
            let sx = sx + c.x;
            gfx::vline(pm, sx, top.bottom() - 8.0 * s, top.bottom(), t.muted);
            for k in 1..5 {
                let (mx, _) = v.to_screen(x + step * k as f32 / 5.0, 0.0);
                gfx::vline(pm, mx + c.x, top.bottom() - 4.0 * s, top.bottom(), t.border);
            }
            let label = format!("{}", ((x / unit) * 10.0).round() / 10.0);
            r.text.draw(pm, &label, &st, sx + 2.0 * s, top.y + 1.0 * s, None);
            x += step;
        }
        let (_, py0) = v.to_page(0.0, 0.0);
        let (_, py1) = v.to_page(0.0, c.h);
        let mut y = (py0 / step).floor() * step;
        while y <= py1 {
            let (_, sy) = v.to_screen(0.0, y);
            let sy = sy + c.y;
            if sy >= c.y {
                gfx::hline(pm, left.right() - 8.0 * s, left.right(), sy, t.muted);
                let label = format!("{}", ((y / unit) * 10.0).round() / 10.0);
                r.text.draw(pm, &label, &st, left.x + 2.0 * s, sy + 1.0 * s, None);
            }
            for k in 1..5 {
                let (_, my) = v.to_screen(0.0, y + step * k as f32 / 5.0);
                if my + c.y >= c.y {
                    gfx::hline(pm, left.right() - 4.0 * s, left.right(), my + c.y, t.border);
                }
            }
            y += step;
        }
        // Mouse position markers.
        let (mx, my) = self.mouse;
        if c.contains(mx, my) {
            gfx::vline(pm, mx, top.y, top.bottom(), ACCENT);
            gfx::hline(pm, left.x, left.right(), my, ACCENT);
        }
    }

    fn paint_minimap(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let s = self.s;
        let c = self.canvas;
        let Some(b) = render::content_bounds(self.ed.pg()) else { return };
        let v = self.ed.view();
        let (vx0, vy0) = v.to_page(0.0, 0.0);
        let (vx1, vy1) = v.to_page(c.w, c.h);
        let x0 = b.0.min(vx0);
        let y0 = b.1.min(vy0);
        let x1 = (b.0 + b.2).max(vx1);
        let y1 = (b.1 + b.3).max(vy1);
        let (mw, mh) = (200.0 * s, 130.0 * s);
        let z = (mw / (x1 - x0).max(1.0)).min(mh / (y1 - y0).max(1.0));
        let mr = Rect::new(c.right() - mw - 14.0 * s, c.bottom() - mh - 14.0 * s, mw, mh).round();
        let ox = (mw - (x1 - x0) * z) / 2.0 - x0 * z;
        let oy = (mh - (y1 - y0) * z) / 2.0 - y0 * z;
        // The drawing in the minimap only changes with the drawing or its scale: keep it.
        let key = (self.ed.version, self.ed.page, [z, ox, oy, mw, mh].map(f32::to_bits), self.theme.dark);
        let mut mini = match &self.mini {
            Some((k, p)) if *k == key => p.clone(),
            _ => {
                let mut p = sk::Pixmap::new(mw as u32, mh as u32).unwrap();
                let page = self.ed.pg();
                p.fill(page.background.to_sk());
                let xf = sk::Transform::from_row(z, 0.0, 0.0, z, ox, oy);
                render::draw_overview(&mut p, page, xf);
                self.mini = Some((key, p.clone()));
                p
            }
        };
        let vr = Rect::new(vx0 * z + ox, vy0 * z + oy, (vx1 - vx0) * z, (vy1 - vy0) * z);
        gfx::fill_rect(&mut mini, vr, ACCENT.with_alpha(30));
        gfx::stroke_round(&mut mini, vr, 0.0, ACCENT, 1.5);
        gfx::shadow(pm, mr, 4.0 * s, self.theme.dark);
        gfx::blit_copy(pm, &mini, mr.x as i32, mr.y as i32);
        gfx::stroke_round(pm, mr, 4.0 * s, self.theme.border, 1.0);
    }

    fn thumb(&mut self, r: &mut Res, kind: &str, px: u32) -> sk::Pixmap {
        let key = (kind.to_string(), px, self.theme.dark);
        if let Some(t) = self.thumbs.get(&key) {
            return t.clone();
        }
        let mut pm = sk::Pixmap::new(px, px).unwrap();
        let (shapes, imgs): (Vec<Shape>, Vec<(String, ImageData)>) = if let Some(n) = kind.strip_prefix("my:") {
            self.settings.my_shapes().into_iter().find(|m| m.name == n).map(|m| (m.shapes, m.images)).unwrap_or_default()
        } else {
            let mut sh = self.ed.make_shape(kind, 0.0, 0.0);
            self.ed.doc.next_id -= 1;
            if kind == "text" {
                sh.text = "Text".into();
            }
            if self.theme.dark && sh.style.fill == Color::WHITE {
                sh.style.fill = Color::hex(0x2B2B2B);
                sh.style.line = Color::hex(0xDDDDDD);
            }
            if kind == "ee.junction" || kind == "ee.terminal" {
                sh.w = 12.0;
                sh.h = 12.0;
            }
            sh.text = if kind == "text" { "Text".into() } else { String::new() };
            (vec![sh], vec![])
        };
        let tmp = Page { shapes: shapes.clone(), ..Default::default() };
        let ids: Vec<Id> = shapes.iter().map(|s| s.id).collect();
        if let Some(b) = crate::editor::bbox_of(&tmp, &ids) {
            let pad = 6.0;
            let z = ((px as f32 - 2.0 * pad) / b.2.max(1.0)).min((px as f32 - 2.0 * pad) / b.3.max(1.0)).min(1.0);
            let xf = sk::Transform::from_row(z, 0.0, 0.0, z, px as f32 / 2.0 - (b.0 + b.2 / 2.0) * z, px as f32 / 2.0 - (b.1 + b.3 / 2.0) * z);
            let mut doc = Document::default();
            for (k, d) in imgs {
                doc.images.insert(k, d);
            }
            let page = Page { shapes, ..Default::default() };
            let mut text_sty = render::DrawOpts::default();
            text_sty.print = false;
            render::draw_page(&mut pm, &doc, &page, &mut r.text, &mut r.images, xf, &text_sty);
        }
        self.thumbs.insert(key, pm.clone());
        pm
    }

    fn paint_left(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        if !self.settings.shapes_pane {
            return;
        }
        let s = self.s;
        let t = self.theme.clone();
        let l = self.left;
        gfx::fill_rect(pm, l, t.panel);
        gfx::vline(pm, l.right() - 1.0, l.y, l.bottom(), t.border);
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            cx.label("Shapes", Rect::new(l.x + 12.0 * s, l.y + 6.0 * s, l.w - 24.0 * s, 24.0 * s), 14.0, t.text, true, Align::Left);
            let sr = Rect::new(l.x + 10.0 * s, l.y + 34.0 * s, l.w - 20.0 * s, 28.0 * s);
            self.search.paint(&mut cx, sr);
            if self.search.text.is_empty() && !self.search.focused {
                cx.label("Search shapes", Rect::new(sr.x + 28.0 * s, sr.y, sr.w, sr.h), 12.5, t.muted, false, Align::Left);
            }
            cx.icon_colored("search", Rect::new(sr.right() - 22.0 * s, sr.y + 7.0 * s, 14.0 * s, 14.0 * s), t.muted);
        }
        let list = Rect::new(l.x, l.y + 70.0 * s, l.w - 1.0, l.h - 70.0 * s);
        let mut sub = sk::Pixmap::new(list.w.max(1.0) as u32, list.h.max(1.0) as u32).unwrap();
        sub.fill(t.panel.to_sk());
        self.tiles.clear();
        self.cats.clear();
        let q = self.search.text.clone();
        let cols = 3;
        let tile = ((list.w - 16.0 * s) / cols as f32).floor();
        let mut y = -self.left_scroll;
        let mut groups: Vec<(String, Vec<(String, String)>)> = vec![];
        let my = self.settings.my_shapes();
        if !my.is_empty() || q.is_empty() {
            groups.push(("My Shapes".into(), my.iter().filter(|m| q.is_empty() || m.name.to_lowercase().contains(&q.to_lowercase())).map(|m| (format!("my:{}", m.name), m.name.clone())).collect()));
        }
        for cat in stencils::CATEGORIES {
            let items: Vec<(String, String)> = stencils::search(&q).into_iter().filter(|st| st.cat == *cat).map(|st| (st.id.to_string(), st.name.to_string())).collect();
            if !items.is_empty() {
                groups.push((cat.to_string(), items));
            }
        }
        let searching = !q.is_empty();
        for (cat, items) in groups {
            let hr = Rect::new(8.0 * s, y, list.w - 16.0 * s, 28.0 * s);
            let open = searching || !self.collapsed.contains(&cat);
            {
                let mut cx = Ctx { pm: &mut sub, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.icon(if open { "chevron-down" } else { "chevron-right" }, Rect::new(hr.x + 4.0 * s, hr.y + 9.0 * s, 10.0 * s, 10.0 * s));
                cx.label(&cat, Rect::new(hr.x + 20.0 * s, hr.y, hr.w - 50.0 * s, hr.h), 12.5, t.text, true, Align::Left);
                cx.label(&items.len().to_string(), hr, 11.5, t.muted, false, Align::Right);
            }
            self.cats.push((cat.clone(), Rect::new(list.x + hr.x, list.y + hr.y, hr.w, hr.h)));
            y += 30.0 * s;
            if !open {
                continue;
            }
            if items.is_empty() {
                let mut st = TextStyle { size: 11.0 * s, color: t.muted, ..Default::default() };
                st.line = 1.3;
                let hint = "Right-click a selection and choose Add to My Shapes.";
                let hh = r.text.layout(hint, &st, Some(list.w - 24.0 * s)).height;
                r.text.draw(&mut sub, hint, &st, 12.0 * s, y, Some(list.w - 24.0 * s));
                y += hh + 10.0 * s;
                continue;
            }
            for (i, (id, name)) in items.iter().enumerate() {
                let (cxi, ryi) = (i % cols, i / cols);
                let tr = Rect::new(8.0 * s + cxi as f32 * tile, y + ryi as f32 * (tile + 18.0 * s), tile, tile + 16.0 * s);
                if tr.bottom() < 0.0 || tr.y > list.h {
                    continue;
                }
                let hov = Rect::new(list.x + tr.x, list.y + tr.y, tr.w, tr.h).contains(self.mouse.0, self.mouse.1);
                if hov {
                    gfx::fill_round(&mut sub, tr, 4.0 * s, t.hover);
                }
                let tp = (tile - 14.0 * s).max(8.0) as u32;
                let th = self.thumb(r, id, tp);
                sub.draw_pixmap((tr.x + (tile - tp as f32) / 2.0) as i32, (tr.y + 2.0 * s) as i32, th.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
                let mut cx = Ctx { pm: &mut sub, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.label(name, Rect::new(tr.x + 2.0 * s, tr.y + tp as f32 + 2.0 * s, tr.w - 4.0 * s, 14.0 * s), 10.5, t.text, false, Align::Center);
                self.tiles.push((id.clone(), Rect::new(list.x + tr.x, list.y + tr.y, tr.w, tr.h)));
            }
            let rows = items.len().div_ceil(cols);
            y += rows as f32 * (tile + 18.0 * s) + 6.0 * s;
        }
        let total = y + self.left_scroll;
        self.left_scroll = self.left_scroll.min((total - list.h).max(0.0));
        pm.draw_pixmap(list.x as i32, list.y as i32, sub.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
        self.tiles.retain(|(_, rr)| rr.y >= list.y - rr.h && rr.y < list.bottom());
        self.cats.retain(|(_, rr)| rr.y >= list.y && rr.y < list.bottom());
        // Drag preview.
        if let Some(id) = self.drag_tile.clone() {
            if !self.left.contains(self.mouse.0, self.mouse.1) {
                let th = self.thumb(r, &id, (48.0 * s) as u32);
                let mut p = sk::PixmapPaint::default();
                p.opacity = 0.7;
                pm.draw_pixmap((self.mouse.0 - 24.0 * s) as i32, (self.mouse.1 - 24.0 * s) as i32, th.as_ref(), &p, sk::Transform::identity(), None);
            }
        }
    }

    fn paint_right(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        if !self.settings.side_pane {
            return;
        }
        let s = self.s;
        let t = self.theme.clone();
        let rr = self.right;
        self.side_hits.clear();
        gfx::fill_rect(pm, rr, t.panel);
        gfx::vline(pm, rr.x, rr.y, rr.bottom(), t.border);
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        let mut x = rr.x + 10.0 * s;
        for (id, label, side) in [("tab:layers", "Layers", Side::Layers), ("tab:comments", "Comments", Side::Comments), ("tab:data", "Data", Side::Data)] {
            let w = cx.measure(label, 13.0, true) + 20.0 * s;
            let tr = Rect::new(x, rr.y + 4.0 * s, w, 28.0 * s);
            cx.label(label, tr, 13.0, if self.side == side { t.text } else { t.muted }, self.side == side, Align::Center);
            if self.side == side {
                gfx::fill_rect(cx.pm, Rect::new(tr.x + 8.0 * s, tr.bottom() - 2.0 * s, tr.w - 16.0 * s, 2.0 * s), ACCENT);
            }
            self.side_hits.push((id.into(), tr));
            x += w;
        }
        let close = Rect::new(rr.right() - 30.0 * s, rr.y + 6.0 * s, 24.0 * s, 24.0 * s);
        cx.icon("close", close.inset(5.0 * s));
        self.side_hits.push(("close-side".into(), close));
        let mut y = rr.y + 40.0 * s;
        match self.side {
            Side::Layers => {
                let page = self.ed.pg().clone();
                let active = self.ed.active_layer();
                for l in page.layers.iter().rev() {
                    let row = Rect::new(rr.x + 8.0 * s, y, rr.w - 16.0 * s, 30.0 * s);
                    if l.id == active {
                        gfx::fill_round(cx.pm, row, 4.0 * s, t.checked);
                    }
                    let n = page.shapes.iter().filter(|x| x.layer == l.id && x.group.is_none()).count();
                    cx.label(&format!("{}  ({n})", l.name), Rect::new(row.x + 8.0 * s, row.y, row.w - 100.0 * s, row.h), 13.0, t.text, l.id == active, Align::Left);
                    let mut bx = row.right() - 26.0 * s;
                    for (what, icon_on, icon_off, on) in [("print", "print", "print", l.print), ("locked", "lock", "unlock", l.locked), ("visible", "eye", "eye-off", l.visible)] {
                        let br = Rect::new(bx, row.y + 4.0 * s, 22.0 * s, 22.0 * s);
                        let col = if on || what == "visible" { t.text } else { t.muted.with_alpha(110) };
                        cx.icon_colored(if on { icon_on } else { icon_off }, br.inset(3.0 * s), if what == "print" && !on { t.muted.with_alpha(90) } else { col });
                        self.side_hits.push((format!("layer:{}:{what}", l.id), br));
                        bx -= 24.0 * s;
                    }
                    self.side_hits.push((format!("layer:{}", l.id), row));
                    y += 32.0 * s;
                }
                y += 8.0 * s;
                for (id, label, icon) in [("layer-new", "New Layer", "plus"), ("layer-move", "Move Selection Here", "layers"), ("layer-del", "Delete Layer", "delete")] {
                    let br = Rect::new(rr.x + 10.0 * s, y, rr.w - 20.0 * s, 28.0 * s);
                    if br.contains(self.mouse.0, self.mouse.1) {
                        gfx::fill_round(cx.pm, br, 4.0 * s, t.hover);
                    }
                    cx.icon(icon, Rect::new(br.x + 6.0 * s, br.y + 6.0 * s, 16.0 * s, 16.0 * s));
                    cx.label(label, Rect::new(br.x + 30.0 * s, br.y, br.w - 30.0 * s, br.h), 12.5, t.text, false, Align::Left);
                    self.side_hits.push((id.into(), br));
                    y += 30.0 * s;
                }
                cx.label("Double-click a layer to rename it. Hidden layers don't show or print; locked layers can't be changed.", Rect::new(rr.x + 12.0 * s, y + 6.0 * s, rr.w - 24.0 * s, 20.0 * s), 11.0, t.muted, false, Align::Left);
            }
            Side::Comments => {
                let br = Rect::new(rr.x + 10.0 * s, y, 130.0 * s, 28.0 * s);
                gfx::fill_round(cx.pm, br, 4.0 * s, ACCENT);
                cx.icon_colored("plus", Rect::new(br.x + 8.0 * s, br.y + 6.0 * s, 16.0 * s, 16.0 * s), ACCENT.contrast_text());
                cx.label("New Comment", Rect::new(br.x + 28.0 * s, br.y, br.w - 30.0 * s, br.h), 12.5, ACCENT.contrast_text(), true, Align::Left);
                self.side_hits.push(("comment-new".into(), br));
                let sr = Rect::new(br.right() + 10.0 * s, y, rr.right() - br.right() - 20.0 * s, 28.0 * s);
                cx.label(if self.settings.show_resolved { "Hide resolved" } else { "Show resolved" }, sr, 12.0, t.muted, false, Align::Right);
                self.side_hits.push(("show-resolved".into(), sr));
                y += 38.0 * s;
                let page = self.ed.pg().clone();
                let wrap = rr.w - 36.0 * s;
                for c in page.comments.iter().rev() {
                    if c.resolved && !self.settings.show_resolved {
                        continue;
                    }
                    let open = self.ed.open_comment == Some(c.id);
                    let mut st = cx.style(12.5);
                    st.color = t.text;
                    let body = if c.text.is_empty() { "(type your comment below)".to_string() } else { c.text.clone() };
                    let bh = cx.text.layout(&body, &st, Some(wrap)).height;
                    let mut rh = 0.0;
                    let mut reps = vec![];
                    for rp in &c.replies {
                        let h = cx.text.layout(&rp.text, &st, Some(wrap - 12.0 * s)).height;
                        reps.push(h);
                        rh += h + 22.0 * s;
                    }
                    let card_h = 50.0 * s + bh + rh + if open { 44.0 * s } else { 0.0 };
                    let card = Rect::new(rr.x + 8.0 * s, y, rr.w - 16.0 * s, card_h);
                    if card.y > rr.bottom() {
                        break;
                    }
                    gfx::fill_round(cx.pm, card, 6.0 * s, if open { t.popup } else { t.window });
                    gfx::stroke_round(cx.pm, card, 6.0 * s, if open { ACCENT } else { t.border }, 1.0);
                    cx.label(&c.author, Rect::new(card.x + 10.0 * s, card.y + 6.0 * s, card.w - 90.0 * s, 18.0 * s), 12.5, t.text, true, Align::Left);
                    cx.label(&folio_kit::files::format_time(std::time::UNIX_EPOCH + Duration::from_secs(c.time)), Rect::new(card.x + 10.0 * s, card.y + 22.0 * s, card.w - 20.0 * s, 16.0 * s), 10.5, t.muted, false, Align::Left);
                    let res = Rect::new(card.right() - 52.0 * s, card.y + 6.0 * s, 22.0 * s, 22.0 * s);
                    cx.icon_colored("check", res.inset(3.0 * s), if c.resolved { Color::hex(0x2E7D32) } else { t.muted });
                    self.side_hits.push((format!("comment:{}:resolve", c.id), res));
                    let del = Rect::new(card.right() - 28.0 * s, card.y + 6.0 * s, 22.0 * s, 22.0 * s);
                    cx.icon_colored("delete", del.inset(3.0 * s), t.muted);
                    self.side_hits.push((format!("comment:{}:delete", c.id), del));
                    let mut st2 = st.clone();
                    st2.color = if c.text.is_empty() { t.muted } else { t.text };
                    cx.text.draw(cx.pm, &body, &st2, card.x + 10.0 * s, card.y + 42.0 * s, Some(wrap));
                    let mut ry = card.y + 46.0 * s + bh;
                    for (i, rp) in c.replies.iter().enumerate() {
                        gfx::vline(cx.pm, card.x + 12.0 * s, ry, ry + reps[i] + 18.0 * s, t.border);
                        cx.label(&rp.author, Rect::new(card.x + 20.0 * s, ry, card.w - 30.0 * s, 16.0 * s), 11.5, t.text, true, Align::Left);
                        cx.text.draw(cx.pm, &rp.text, &st, card.x + 20.0 * s, ry + 17.0 * s, Some(wrap - 12.0 * s));
                        ry += reps[i] + 22.0 * s;
                    }
                    self.side_hits.push((format!("comment:{}", c.id), card));
                    if open {
                        let fr = Rect::new(card.x + 8.0 * s, card.bottom() - 38.0 * s, card.w - 50.0 * s, 30.0 * s);
                        self.reply.paint(&mut cx, fr);
                        if self.reply.text.is_empty() && !self.reply.focused {
                            cx.label(if c.text.is_empty() { "Comment" } else { "Reply" }, fr.inset(8.0 * s), 12.5, t.muted, false, Align::Left);
                        }
                        let sb = Rect::new(fr.right() + 4.0 * s, fr.y, 34.0 * s, 30.0 * s);
                        gfx::fill_round(cx.pm, sb, 4.0 * s, ACCENT);
                        cx.icon_colored("chevron-right", sb.inset(8.0 * s), ACCENT.contrast_text());
                        self.side_hits.push(("reply-send".into(), sb));
                    }
                    y += card_h + 8.0 * s;
                }
                if page.comments.is_empty() {
                    cx.label("No comments on this page.", Rect::new(rr.x + 12.0 * s, y, rr.w - 24.0 * s, 20.0 * s), 12.5, t.muted, false, Align::Left);
                }
            }
            Side::Data => {
                // Shape Data (K4-02): the properties of the selected shapes, to read and change
                let n = self.ed.sel.len();
                let props = self.ed.props_of_selection();
                let head = match n {
                    0 => "Pick a shape to see its data.".to_string(),
                    1 => self.ed.first().map(|x| if x.text.trim().is_empty() { x.kind.clone() } else { x.text.trim().to_string() }).unwrap_or_default(),
                    _ => format!("{n} shapes"),
                };
                cx.label(&head, Rect::new(rr.x + 12.0 * s, y, rr.w - 24.0 * s, 20.0 * s), 12.5, t.muted, false, Align::Left);
                y += 26.0 * s;
                for (name, value) in &props {
                    let row = Rect::new(rr.x + 8.0 * s, y, rr.w - 16.0 * s, 30.0 * s);
                    if row.contains(self.mouse.0, self.mouse.1) {
                        gfx::fill_round(cx.pm, row, 4.0 * s, t.hover);
                    }
                    cx.label(name, Rect::new(row.x + 8.0 * s, row.y, row.w * 0.42, row.h), 12.5, t.text, true, Align::Left);
                    let shown = if value.is_empty() && n > 1 { "(different)".to_string() } else { value.clone() };
                    cx.label(&shown, Rect::new(row.x + row.w * 0.44, row.y, row.w * 0.44, row.h), 12.5, t.text, false, Align::Left);
                    let del = Rect::new(row.right() - 26.0 * s, row.y + 4.0 * s, 22.0 * s, 22.0 * s);
                    cx.icon_colored("delete", del.inset(3.0 * s), t.muted);
                    self.side_hits.push((format!("prop:{name}:delete"), del));
                    self.side_hits.push((format!("prop:{name}"), row));
                    y += 32.0 * s;
                }
                if props.is_empty() && n > 0 {
                    cx.label("No data on this shape yet.", Rect::new(rr.x + 12.0 * s, y, rr.w - 24.0 * s, 20.0 * s), 12.5, t.muted, false, Align::Left);
                    y += 24.0 * s;
                }
                if n > 0 {
                    let br = Rect::new(rr.x + 10.0 * s, y + 6.0 * s, 150.0 * s, 28.0 * s);
                    gfx::fill_round(cx.pm, br, 4.0 * s, ACCENT);
                    cx.icon_colored("plus", Rect::new(br.x + 8.0 * s, br.y + 6.0 * s, 16.0 * s, 16.0 * s), ACCENT.contrast_text());
                    cx.label("Add Property", Rect::new(br.x + 28.0 * s, br.y, br.w - 30.0 * s, br.h), 12.5, ACCENT.contrast_text(), true, Align::Left);
                    self.side_hits.push(("prop-add".into(), br));
                    y += 38.0 * s;
                }
                cx.label("Click a row to change it.", Rect::new(rr.x + 12.0 * s, y + 6.0 * s, rr.w - 24.0 * s, 20.0 * s), 11.0, t.muted, false, Align::Left);
                cx.label("Saved in the drawing, and in Visio.", Rect::new(rr.x + 12.0 * s, y + 24.0 * s, rr.w - 24.0 * s, 20.0 * s), 11.0, t.muted, false, Align::Left);
            }
        }
    }

    fn paint_bar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let s = self.s;
        let t = self.theme.clone();
        let b = self.bar;
        self.bar_hits.clear();
        gfx::fill_rect(pm, b, t.tabs);
        gfx::hline(pm, b.x, b.right(), b.y, t.border);
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        let mut x = b.x + 8.0 * s;
        let names: Vec<String> = self.ed.doc.pages.iter().map(|p| p.name.clone()).collect();
        let zoom_w = 250.0 * s;
        for (i, n) in names.iter().enumerate() {
            let w = (cx.measure(n, 12.5, i == self.ed.page) + 24.0 * s).min(180.0 * s);
            if x + w > b.right() - zoom_w - 40.0 * s {
                break;
            }
            let tr = Rect::new(x, b.y + 1.0, w, b.h - 4.0 * s);
            if i == self.ed.page {
                gfx::fill_round(cx.pm, tr, 4.0 * s, t.ribbon);
                gfx::fill_rect(cx.pm, Rect::new(tr.x + 6.0 * s, tr.bottom() - 2.0 * s, tr.w - 12.0 * s, 2.0 * s), ACCENT);
            } else if tr.contains(self.mouse.0, self.mouse.1) {
                gfx::fill_round(cx.pm, tr, 4.0 * s, t.hover);
            }
            cx.label(n, tr.inset(6.0 * s), 12.5, t.text, i == self.ed.page, Align::Center);
            self.bar_hits.push((format!("page:{i}"), tr));
            x += w + 2.0 * s;
        }
        let ar = Rect::new(x + 4.0 * s, b.y + 4.0 * s, 24.0 * s, b.h - 8.0 * s);
        cx.icon("plus", ar.inset(4.0 * s));
        self.bar_hits.push(("page-add".into(), ar));
        // Status: selection size, or what's on the page.
        let status = match self.ed.selected().as_slice() {
            [one] if !one.is_connector() => format!("W {:.0}  H {:.0}  {:.0}°", one.w, one.h, one.rot),
            [] => format!("{} shapes", self.ed.pg().shapes.iter().filter(|s| s.group.is_none()).count()),
            many => format!("{} selected", many.len()),
        };
        let status = if let Some(c) = &self.collab { format!("{status}   ·   {}", c.status()) } else { status };
        let zx = b.right() - zoom_w;
        cx.label(&status, Rect::new(zx - 300.0 * s, b.y, 290.0 * s, b.h), 12.0, t.muted, false, Align::Right);
        let zo = Rect::new(zx, b.y + 4.0 * s, 22.0 * s, b.h - 8.0 * s);
        cx.icon("zoom-out", zo.inset(3.0 * s));
        self.bar_hits.push(("zoom-out".into(), zo));
        let sl = Rect::new(zo.right() + 6.0 * s, b.cy() - 1.0 * s, 100.0 * s, 2.0 * s);
        gfx::fill_rect(cx.pm, sl, t.border);
        let z = self.ed.view().zoom;
        let pos = ((z.dln() - 0.1f32.dln()) / (8.0f32.dln() - 0.1f32.dln())).clamp(0.0, 1.0);
        gfx::fill_round(cx.pm, Rect::new(sl.x + pos * sl.w - 4.0 * s, b.cy() - 7.0 * s, 8.0 * s, 14.0 * s), 2.0 * s, t.text);
        let zi = Rect::new(sl.right() + 6.0 * s, b.y + 4.0 * s, 22.0 * s, b.h - 8.0 * s);
        cx.icon("zoom-in", zi.inset(3.0 * s));
        self.bar_hits.push(("zoom-in".into(), zi));
        let zp = Rect::new(zi.right() + 4.0 * s, b.y + 2.0 * s, 52.0 * s, b.h - 4.0 * s);
        cx.label(&format!("{:.0}%", z * 100.0), zp, 12.0, t.text, false, Align::Center);
        self.bar_hits.push(("zoom-menu".into(), zp));
        let fr = Rect::new(zp.right() + 2.0 * s, b.y + 4.0 * s, 24.0 * s, b.h - 8.0 * s);
        cx.icon("fit", fr.inset(3.0 * s));
        self.bar_hits.push(("fit".into(), fr));
    }

    fn paint_backstage(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let s = self.s;
        let t = self.theme.clone();
        let Some(mut b) = self.backstage.take() else { return };
        b.hits.clear();
        let top = TITLE_H * s;
        let nav = Rect::new(0.0, top, 210.0 * s, self.h - top);
        gfx::fill_rect(pm, nav, ACCENT.mix(Color::BLACK, if t.dark { 0.45 } else { 0.1 }));
        let content = Rect::new(nav.right(), top, self.w - nav.right(), self.h - top);
        gfx::fill_rect(pm, content, t.window);
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        let fg = Color::WHITE;
        let back = Rect::new(nav.x + 12.0 * s, nav.y + 14.0 * s, 36.0 * s, 36.0 * s);
        gfx::stroke_round(cx.pm, back, 18.0 * s, fg.with_alpha(160), 1.5);
        cx.icon_colored("back", back.inset(9.0 * s), fg);
        b.hits.push(("back".into(), back));
        let mut y = nav.y + 70.0 * s;
        for (id, label, icon, page) in [
            ("nav:new", "New", "new", Some(BackPage::New)),
            ("nav:open", "Open", "open", Some(BackPage::Open)),
            ("nav:save", "Save", "save", None),
            ("nav:saveas", "Save As", "save-as", Some(BackPage::SaveAs)),
            ("nav:export", "Export", "export", Some(BackPage::Export)),
            ("nav:print", "Print", "print", Some(BackPage::Print)),
            ("nav:options", "Options", "options", Some(BackPage::Options)),
            ("nav:about", "About", "info", Some(BackPage::About)),
            ("nav:close", "Close", "close", None),
        ] {
            let rr = Rect::new(nav.x, y, nav.w, 40.0 * s);
            if page == Some(b.page) && b.msg.is_empty() || page == Some(b.page) && page != Some(BackPage::Open) {
                gfx::fill_rect(cx.pm, rr, fg.with_alpha(40));
            } else if rr.contains(self.mouse.0, self.mouse.1) {
                gfx::fill_rect(cx.pm, rr, fg.with_alpha(22));
            }
            cx.icon_colored(icon, Rect::new(rr.x + 22.0 * s, rr.y + 11.0 * s, 18.0 * s, 18.0 * s), fg);
            cx.label(label, Rect::new(rr.x + 54.0 * s, rr.y, rr.w - 60.0 * s, rr.h), 14.0, fg, false, Align::Left);
            b.hits.push((id.into(), rr));
            y += 42.0 * s;
        }
        let pad = 40.0 * s;
        let area = Rect::new(content.x + pad, content.y + 26.0 * s, content.w - 2.0 * pad, content.h - 50.0 * s);
        let heading = match b.page {
            BackPage::New => "New",
            BackPage::Open => match b.msg.as_str() {
                "pictures" => "Insert Pictures",
                "orgchart" => "Org Chart from a File",
                _ => "Open",
            },
            BackPage::SaveAs => "Save As",
            BackPage::Export => "Export",
            BackPage::Print => "Print",
            BackPage::Options => "Options",
            BackPage::About => "About",
        };
        cx.label(heading, Rect::new(area.x, area.y, area.w, 40.0 * s), 26.0, t.text, false, Align::Left);
        let body = Rect::new(area.x, area.y + 56.0 * s, area.w, area.h - 56.0 * s);
        match b.page {
            BackPage::New => {
                let cw = 170.0 * s;
                let ch = 140.0 * s;
                let cols = ((body.w + 16.0 * s) / (cw + 16.0 * s)).floor().max(1.0) as usize;
                for (i, (k, name, desc)) in templates::LIST.iter().enumerate() {
                    let cr = Rect::new(body.x + (i % cols) as f32 * (cw + 16.0 * s), body.y + (i / cols) as f32 * (ch + 16.0 * s), cw, ch);
                    let hov = cr.contains(self.mouse.0, self.mouse.1);
                    gfx::fill_round(cx.pm, cr, 6.0 * s, if hov { t.hover } else { t.ribbon });
                    gfx::stroke_round(cx.pm, cr, 6.0 * s, if hov { ACCENT } else { t.border }, 1.0);
                    cx.icon(templates::icon(k), Rect::new(cr.cx() - 20.0 * s, cr.y + 22.0 * s, 40.0 * s, 40.0 * s));
                    cx.label(name, Rect::new(cr.x + 8.0 * s, cr.y + 74.0 * s, cr.w - 16.0 * s, 20.0 * s), 13.5, t.text, true, Align::Center);
                    let mut st = cx.style(11.0);
                    st.color = t.muted;
                    st.align = Align::Center;
                    cx.text.draw(cx.pm, desc, &st, cr.x + 8.0 * s, cr.y + 96.0 * s, Some(cr.w - 16.0 * s));
                    b.hits.push((format!("tpl:{k}"), cr));
                }
            }
            BackPage::Open => {
                let mut fy = body.y;
                if b.msg.is_empty() {
                    let recent = self.settings.recent.clone();
                    if !recent.is_empty() {
                        cx.label("Recent", Rect::new(body.x, fy, 200.0 * s, 22.0 * s), 13.0, t.muted, true, Align::Left);
                        fy += 24.0 * s;
                        for p in recent.iter().take(5) {
                            let rr = Rect::new(body.x, fy, body.w, 30.0 * s);
                            if rr.contains(self.mouse.0, self.mouse.1) {
                                gfx::fill_round(cx.pm, rr, 4.0 * s, t.hover);
                            }
                            cx.icon("file", Rect::new(rr.x + 6.0 * s, rr.y + 7.0 * s, 16.0 * s, 16.0 * s));
                            let name = Path::new(p).file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                            cx.label(&name, Rect::new(rr.x + 30.0 * s, rr.y, 240.0 * s, rr.h), 13.0, t.text, false, Align::Left);
                            cx.label(p, Rect::new(rr.x + 280.0 * s, rr.y, rr.w - 290.0 * s, rr.h), 12.0, t.muted, false, Align::Left);
                            b.hits.push((format!("recent:{p}"), rr));
                            fy += 30.0 * s;
                        }
                        fy += 14.0 * s;
                    }
                } else if b.msg == "orgchart" {
                    cx.label("A CSV or spreadsheet with columns Name, Title and Reports To (the manager's name).", Rect::new(body.x, fy, body.w, 22.0 * s), 12.5, t.muted, false, Align::Left);
                    fy += 30.0 * s;
                }
                if let Some(f) = b.files.as_mut() {
                    f.paint(&mut cx, Rect::new(body.x, fy, body.w, body.bottom() - fy));
                }
            }
            BackPage::SaveAs => {
                cx.label("Saved as a Visio drawing (.vsdx) with every page.", Rect::new(body.x, body.y, body.w, 22.0 * s), 12.5, t.muted, false, Align::Left);
                if let Some(f) = b.files.as_mut() {
                    f.paint(&mut cx, Rect::new(body.x, body.y + 30.0 * s, body.w, body.h - 30.0 * s));
                }
            }
            BackPage::Export => {
                let mut x = body.x;
                for (i, (label, icon)) in [("PNG picture", "picture"), ("SVG drawing", "shapes"), ("PDF document", "file")].iter().enumerate() {
                    let rr = Rect::new(x, body.y, 150.0 * s, 40.0 * s);
                    gfx::fill_round(cx.pm, rr, 6.0 * s, if b.export_fmt == i { t.checked } else { t.ribbon });
                    gfx::stroke_round(cx.pm, rr, 6.0 * s, if b.export_fmt == i { ACCENT } else { t.border }, 1.0);
                    cx.icon(icon, Rect::new(rr.x + 10.0 * s, rr.y + 11.0 * s, 18.0 * s, 18.0 * s));
                    cx.label(label, Rect::new(rr.x + 36.0 * s, rr.y, rr.w - 40.0 * s, rr.h), 13.0, t.text, b.export_fmt == i, Align::Left);
                    b.hits.push((format!("fmt:{i}"), rr));
                    x += 160.0 * s;
                }
                let oy = body.y + 52.0 * s;
                let chk = |cx: &mut Ctx, rr: Rect, on: bool, label: &str| {
                    let br = Rect::new(rr.x, rr.y + 5.0 * s, 18.0 * s, 18.0 * s);
                    gfx::fill_round(cx.pm, br, 3.0 * s, if on { ACCENT } else { t.field });
                    gfx::stroke_round(cx.pm, br, 3.0 * s, if on { ACCENT } else { t.muted }, 1.0);
                    if on {
                        cx.icon_colored("check", br.inset(2.0 * s), Color::WHITE);
                    }
                    cx.label(label, Rect::new(rr.x + 26.0 * s, rr.y, rr.w - 26.0 * s, rr.h), 13.0, t.text, false, Align::Left);
                };
                let r1 = Rect::new(body.x, oy, 200.0 * s, 28.0 * s);
                chk(&mut cx, r1, b.export_all, "All pages");
                b.hits.push(("export-all".into(), r1));
                if b.export_fmt == 0 {
                    let r2 = Rect::new(body.x + 220.0 * s, oy, 220.0 * s, 28.0 * s);
                    cx.label(&format!("Resolution: {}×  (click to change)", [1, 2, 4][b.export_scale]), r2, 13.0, t.text, false, Align::Left);
                    b.hits.push(("export-scale".into(), r2));
                }
                if let Some(f) = b.files.as_mut() {
                    f.paint(&mut cx, Rect::new(body.x, oy + 40.0 * s, body.w, body.bottom() - oy - 40.0 * s));
                }
            }
            BackPage::Print => {
                let mut y = body.y;
                let printer = b.printers.get(b.printer).cloned().unwrap_or_else(|| "No printer found".into());
                for (id, label, value) in [
                    ("printer", "Printer", printer),
                    ("copies", "Copies", b.copies.to_string()),
                    ("print-all", "Pages", if b.print_all { "All pages".into() } else { "This page".into() }),
                    ("print-fit", "Scale", if b.print_fit { "Fit each page on one sheet".into() } else { "Actual size, tiled over sheets".into() }),
                    ("paper", "Paper", format!("{} {}", self.ed.pg().paper.name, if self.ed.pg().paper.landscape { "landscape" } else { "portrait" })),
                ] {
                    cx.label(label, Rect::new(body.x, y, 120.0 * s, 32.0 * s), 13.0, t.muted, false, Align::Left);
                    let vr = Rect::new(body.x + 130.0 * s, y, 360.0 * s, 32.0 * s);
                    gfx::fill_round(cx.pm, vr, 4.0 * s, t.field);
                    gfx::stroke_round(cx.pm, vr, 4.0 * s, t.border, 1.0);
                    cx.label(&value, vr.inset(8.0 * s), 13.0, t.text, false, Align::Left);
                    if id == "copies" {
                        let up = Rect::new(vr.right() - 60.0 * s, vr.y + 4.0 * s, 24.0 * s, 24.0 * s);
                        let dn = Rect::new(vr.right() - 30.0 * s, vr.y + 4.0 * s, 24.0 * s, 24.0 * s);
                        cx.icon("plus", up.inset(4.0 * s));
                        cx.icon("win-min", dn.inset(4.0 * s));
                        b.hits.push(("copies-up".into(), up));
                        b.hits.push(("copies-down".into(), dn));
                    } else if id != "paper" {
                        b.hits.push((id.into(), vr));
                    }
                    y += 40.0 * s;
                }
                let pr = Rect::new(body.x + 130.0 * s, y + 10.0 * s, 140.0 * s, 38.0 * s);
                gfx::fill_round(cx.pm, pr, 4.0 * s, ACCENT);
                cx.icon_colored("print", Rect::new(pr.x + 12.0 * s, pr.y + 10.0 * s, 18.0 * s, 18.0 * s), Color::WHITE);
                cx.label("Print", Rect::new(pr.x + 36.0 * s, pr.y, pr.w - 40.0 * s, pr.h), 14.0, Color::WHITE, true, Align::Left);
                b.hits.push(("do-print".into(), pr));
                let pp = Rect::new(pr.right() + 12.0 * s, pr.y, 160.0 * s, 38.0 * s);
                gfx::stroke_round(cx.pm, pp, 4.0 * s, t.border, 1.0);
                cx.label("Save as PDF instead", pp, 13.0, t.text, false, Align::Center);
                b.hits.push(("print-pdf".into(), pp));
                if !b.msg.is_empty() {
                    let m = b.msg.clone();
                    cx.label(&m, Rect::new(body.x + 130.0 * s, pr.bottom() + 10.0 * s, body.w, 22.0 * s), 12.5, t.danger, false, Align::Left);
                }
            }
            BackPage::Options => {
                let mut y = body.y;
                for (id, label, value) in [
                    ("opt:theme", "Theme", match self.settings.theme.as_str() { "dark" => "Dark", "light" => "Light", _ => "Same as system" }.to_string()),
                    ("opt:author", "Your name on comments", self.ed.author.clone()),
                    ("opt:grid", "Grid spacing", format!("{:.0} px", self.ed.grid_size)),
                ] {
                    cx.label(label, Rect::new(body.x, y, 200.0 * s, 32.0 * s), 13.0, t.muted, false, Align::Left);
                    let vr = Rect::new(body.x + 210.0 * s, y, 300.0 * s, 32.0 * s);
                    gfx::fill_round(cx.pm, vr, 4.0 * s, t.field);
                    gfx::stroke_round(cx.pm, vr, 4.0 * s, t.border, 1.0);
                    cx.label(&value, vr.inset(8.0 * s), 13.0, t.text, false, Align::Left);
                    b.hits.push((id.into(), vr));
                    y += 42.0 * s;
                }
            }
            BackPage::About => {
                let lines = [
                    format!("Folio Cork {VERSION}"),
                    "Diagrams, whiteboards and mood boards. Part of Folio Studio.".to_string(),
                    "Opens and saves Visio drawings (.vsdx). Exports PNG, SVG and PDF.".to_string(),
                    "Nothing leaves your computer: no accounts, no cloud, no tracking.".to_string(),
                ];
                for (i, l) in lines.iter().enumerate() {
                    cx.label(l, Rect::new(body.x, body.y + i as f32 * 28.0 * s, body.w, 24.0 * s), if i == 0 { 16.0 } else { 13.0 }, t.text, i == 0, Align::Left);
                }
            }
        }
        self.backstage = Some(b);
    }
}

/// Command state for the ribbon.
#[derive(Clone)]
/// The drawing as last drawn, and what it was drawn from.
struct CanvasCache {
    ver: u64,
    view: View,
    size: (u32, u32),
    dark: bool,
    hide: Option<Id>,
    pkey: u64,
    prints: HashMap<Id, (u64, (f32, f32, f32, f32))>,
    layer: sk::Pixmap,
}

/// The page-space box a view shows in a w×h pixel area.
fn vis_box(v: View, w: u32, h: u32) -> (f32, f32, f32, f32) {
    let (x0, y0) = v.to_page(0.0, 0.0);
    let (x1, y1) = v.to_page(w as f32, h as f32);
    (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}

pub struct St {
    pub has_sel: bool,
    pub multi: bool,
    pub three: bool,
    pub conn: bool,
    pub group: bool,
    pub images: bool,
    pub first: Option<Shape>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub clip: bool,
    pub tool: Tool,
    pub painter: bool,
    pub settings: Settings,
    pub zoom: f32,
    pub landscape: bool,
    pub dirty: bool,
    pub route: Option<Route>,
    pub jumps: bool,
    pub sharing: bool,
    pub side_layers: bool,
    pub side_comments: bool,
    pub side_data: bool,
}

impl CommandState for St {
    fn enabled(&self, id: &str) -> bool {
        match id {
            "undo" => self.can_undo,
            "redo" => self.can_redo,
            "cut" | "copy" | "painter" | "delete" | "duplicate" | "lock" | "unlock" | "fill" | "line" | "weight" | "dash" | "shadow" | "quick" | "font" | "size" | "bold" | "italic"
            | "underline" | "grow" | "shrink" | "font-color" | "align-left" | "align-center" | "align-right" | "valign-top" | "valign-middle" | "valign-bottom" | "order-menu" | "rotate.right"
            | "rotate.left" | "flip.h" | "flip.v" | "zoom.sel" | "mood.section" => self.has_sel,
            "group" | "align-menu" | "align.left" | "align.center" | "align.right" | "align.top" | "align.middle" | "align.bottom" => self.multi,
            "dist.h" | "dist.v" => self.three,
            "ungroup" => self.group,
            "route.elbow" | "route.straight" | "route.curved" | "arrow-a" | "arrow-b" | "jumps" => self.conn,
            "order.front" | "order.back" | "order.forward" | "order.backward" => self.has_sel,
            "mood.polaroid" | "mood.crop" | "mood.palette" => self.images,
            _ => true,
        }
    }
    fn checked(&self, id: &str) -> bool {
        let f = self.first.as_ref();
        match id {
            "bold" => f.map(|s| s.tstyle.bold).unwrap_or(false),
            "italic" => f.map(|s| s.tstyle.italic).unwrap_or(false),
            "underline" => f.map(|s| s.tstyle.underline).unwrap_or(false),
            "align-left" => f.map(|s| s.tstyle.align == Align::Left).unwrap_or(false),
            "align-center" => f.map(|s| s.tstyle.align == Align::Center && !s.is_connector()).unwrap_or(false),
            "align-right" => f.map(|s| s.tstyle.align == Align::Right).unwrap_or(false),
            "shadow" => f.map(|s| s.style.shadow).unwrap_or(false),
            "lock" => f.map(|s| s.locked).unwrap_or(false),
            "painter" => self.painter,
            "route.elbow" => self.route == Some(Route::Elbow),
            "route.straight" => self.route == Some(Route::Straight),
            "route.curved" => self.route == Some(Route::Curved),
            "jumps" => self.jumps,
            "view.rulers" => self.settings.rulers,
            "view.grid" => self.settings.grid,
            "view.snap" => self.settings.snap,
            "view.guides" => self.settings.guides,
            "view.minimap" => self.settings.minimap,
            "view.pagebreaks" => self.settings.page_breaks,
            "pane.shapes" => self.settings.shapes_pane,
            "pane.layers" => self.side_layers,
            "pane.comments" => self.side_comments,
            "pane.data" => self.side_data,
            "share-host" => self.sharing,
            "mood.polaroid" => f.map(|s| s.get("frame") == "polaroid").unwrap_or(false),
            t if t.starts_with("tool.") => {
                let want = &t[5..];
                match (&self.tool, want) {
                    (Tool::Pointer, "pointer") | (Tool::Hand, "hand") | (Tool::Connector, "connector") | (Tool::Text, "text") | (Tool::Sticky, "sticky") | (Tool::Pen, "pen") => true,
                    (Tool::Highlighter, "highlighter") | (Tool::Eraser, "eraser") | (Tool::Frame, "frame") | (Tool::Comment, "comment") | (Tool::Vote, "vote") | (Tool::Eyedropper, "eyedropper") => true,
                    (Tool::Shape(k), w) => w.strip_prefix("shape.") == Some(k.as_str()),
                    _ => false,
                }
            }
            _ => false,
        }
    }
    fn text(&self, id: &str) -> Option<String> {
        let f = self.first.as_ref();
        match id {
            "font" => Some(f.map(|s| if s.tstyle.family.is_empty() { "Default".to_string() } else { s.tstyle.family.clone() }).unwrap_or_else(|| "Default".into())),
            "size" => Some(f.map(|s| format!("{}", s.tstyle.size.round())).unwrap_or_else(|| "14".into())),
            _ => None,
        }
    }
    fn swatch(&self, id: &str) -> Option<Color> {
        match id {
            "fill" => Some(self.settings.last_fill),
            "line" => Some(self.settings.last_line),
            "font-color" => self.first.as_ref().map(|s| s.tstyle.color),
            _ => None,
        }
    }
}

fn stem(name: &str) -> String {
    Path::new(name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| name.to_string())
}

fn word_at(t: &str, i: usize) -> (usize, usize) {
    let is_w = |c: char| c.is_alphanumeric() || c == '_';
    let mut a = i.min(t.len());
    while a > 0 {
        let p = t[..a].char_indices().last().map(|(j, _)| j).unwrap_or(0);
        if !t[p..].chars().next().map(is_w).unwrap_or(false) {
            break;
        }
        a = p;
    }
    let mut b = i.min(t.len());
    while b < t.len() {
        let c = t[b..].chars().next().unwrap();
        if !is_w(c) {
            break;
        }
        b += c.len_utf8();
    }
    (a, b)
}

impl folio_kit::platform::App for CorkApp {
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
        if self.collab.is_some() {
            return Some(Duration::from_millis(80));
        }
        if self.ed.edit.is_some() {
            return Some(Duration::from_millis(530));
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
