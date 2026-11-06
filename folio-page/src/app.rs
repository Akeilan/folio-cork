//! The Page window: title bar, ribbon, the pages, the status bar, the File
//! pages and the dialogs.

use crate::ribbon_defs;
use folio_canvas::{Cx, Glyphs};
use folio_kit::chrome::{TitleAction, TitleBar, TITLE_H};
use folio_kit::files::{FileAction, FilePane};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::platform::{Button, Cursor, Event, Host, Key, Mods};
use folio_kit::ribbon::{Ribbon, RibbonAction};
use folio_kit::text::{Align, Text};
use folio_kit::theme::Theme;
use folio_kit::ui::{CommandState, Ctx, Dialog, DialogResult, Menu, MenuItem, MenuResult};
use folio_page_editor::eng::document::Document;
use folio_page_editor::eng::fonts::FontCache;
use folio_page_editor::Editor;
use folio_page_render::{Overlay, Renderer};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Page's colour: the blue the GTK version used.
pub const BLUE: Color = Color::hex(0x2B579A);
const BAR_H: f32 = 26.0;
/// The space round and between the pages, in points at 100%.
const GAP: f32 = 16.0;

pub const ZOOMS: [f32; 11] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0];
pub const SIZES: [f32; 16] =
    [8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0, 48.0, 72.0, 96.0];
/// The colours of the font-colour and highlight menus.
pub const PALETTE: [u32; 20] = [
    0x000000, 0x404040, 0x7F7F7F, 0xBFBFBF, 0xFFFFFF, 0xC00000, 0xFF0000, 0xFFC000, 0xFFFF00, 0x92D050, 0x00B050,
    0x00B0F0, 0x0070C0, 0x002060, 0x7030A0, 0xE36C0A, 0x8064A2, 0x4BACC6, 0x9BBB59, 0x1F497D,
];

pub struct Res {
    pub text: Text,
    pub icons: Icons,
}

/// What the window is showing besides the pages.
pub enum Pane {
    None,
    /// The File pages: open, save, print, properties.
    Backstage(Back),
}

pub struct Back {
    pub page: String,
    pub files: Option<FilePane>,
}

pub struct PageApp {
    pub ed: Editor,
    pub path: Option<PathBuf>,
    pub zoom: f32,
    /// How far down the document the view is, in points.
    pub scroll: f32,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub theme: Theme,
    pub title: TitleBar,
    pub ribbon: Ribbon,
    pub pane: Pane,
    pub dialog: Option<Dialog>,
    pub menu: Option<Menu>,
    pub flash: Option<(String, Duration)>,
    pub now: Duration,
    pub exit: bool,
    pub exit_code: i32,
    /// The pages as pictures, by page and the document version they were drawn from.
    cache: Vec<(usize, u64, f32, sk::Pixmap)>,
    renderer: Renderer,
    glyphs: Glyphs,
    pm: Option<sk::Pixmap>,
    res: Option<Res>,
    /// Where each page sits in the view, in window pixels.
    page_rects: Vec<(usize, Rect)>,
    canvas: Rect,
    dragging: bool,
    marks: bool,
    painter: bool,
    view_mode: u8,
    pub test: Option<crate::uitest::UiTest>,
    pub settings: crate::settings::Settings,
    /// a close that is waiting on "save first?"
    closing: bool,
}

impl PageApp {
    pub fn new(doc: Document, path: Option<PathBuf>) -> PageApp {
        let mut res = Res { text: Text::new(), icons: Icons::new() };
        crate::icons::install(&mut res.icons);
        let ed = Editor::new(doc, FontCache::new());
        PageApp {
            ed,
            path,
            zoom: 1.0,
            scroll: 0.0,
            w: 1280.0,
            h: 800.0,
            s: 1.0,
            theme: Theme::new(false, BLUE, BLUE),
            title: TitleBar::new(
                "page",
                vec![
                    ("save", "save", "Save (Ctrl+S)"),
                    ("undo", "undo", "Undo (Ctrl+Z)"),
                    ("redo", "redo", "Redo (Ctrl+Y)"),
                ],
            ),
            ribbon: Ribbon::new(ribbon_defs::tabs()),
            pane: Pane::None,
            dialog: None,
            menu: None,
            flash: None,
            now: Duration::ZERO,
            exit: false,
            exit_code: 0,
            cache: Vec::new(),
            renderer: Renderer::new(),
            glyphs: Glyphs::new(),
            pm: None,
            res: Some(res),
            page_rects: Vec::new(),
            canvas: Rect::default(),
            dragging: false,
            marks: false,
            painter: false,
            view_mode: 0,
            test: None,
            settings: crate::settings::Settings::load(),
            closing: false,
        }
    }

    /// A document to start from: the blank template if there is one.
    pub fn blank() -> Document {
        for d in crate::template_dirs() {
            for name in ["Normal-Letter.docx", "Normal-A4.docx"] {
                if let Ok(doc) = Document::open_path(&d.join(name).to_string_lossy()) {
                    return doc;
                }
            }
        }
        Document::open(include_bytes!("blank.docx")).expect("the blank document")
    }

    /// Start with what was remembered last time.
    pub fn apply_settings(&mut self) {
        self.zoom = self.settings.zoom;
        self.marks = self.settings.marks;
        if self.settings.view_mode != 0 {
            self.view_mode = self.settings.view_mode;
            self.ed.set_view_mode(self.view_mode, 600.0);
        }
    }

    fn remember(&mut self) {
        self.settings.zoom = self.zoom;
        self.settings.marks = self.marks;
        self.settings.view_mode = self.view_mode;
        if let Some(p) = self.path.clone() {
            self.settings.remember(&p);
        }
        self.settings.save();
    }

    pub fn open_path(&mut self, p: &Path) {
        match Document::open_path(&p.to_string_lossy()) {
            Ok(doc) => {
                self.ed = Editor::new(doc, FontCache::new());
                self.path = Some(p.to_path_buf());
                self.scroll = 0.0;
                self.cache.clear();
                self.flash(&format!("Opened {}", name_of(p)));
                self.remember();
            }
            Err(e) => self.say("Can't open the file", &e.0),
        }
    }

    pub fn save(&mut self, p: &Path) {
        let p = with_ext(p, "docx");
        match self.ed.save(&p.to_string_lossy()) {
            Ok(()) => {
                self.path = Some(p.clone());
                self.flash(&format!("Saved {}", name_of(&p)));
                self.remember();
            }
            Err(e) => self.say("Can't save the file", &e),
        }
    }

    pub fn export_pdf(&mut self, p: &Path) {
        let p = with_ext(p, "pdf");
        match self.ed.export_pdf(&p.to_string_lossy()) {
            Ok(()) => self.flash(&format!("Wrote {}", name_of(&p))),
            Err(e) => self.say("Can't write the PDF", &e),
        }
    }

    pub fn window_title(&self) -> String {
        let name = self.path.as_deref().map(name_of).unwrap_or_else(|| "Document".into());
        let star = if self.ed.modified { "*" } else { "" };
        format!("{name}{star} — Folio Page")
    }

    pub fn flash(&mut self, msg: &str) {
        self.flash = Some((msg.to_string(), self.now + Duration::from_millis(2200)));
    }

    pub fn say(&mut self, title: &str, note: &str) {
        self.dialog = Some(Dialog::new("info", title).note(note).button("ok", "Close"));
    }

    // ------------------------------------------------------------ the view

    /// The page pictures and where they go, for the scroll position now.
    fn layout_pages(&mut self) {
        self.page_rects.clear();
        let gap = GAP * self.s;
        let z = self.zoom * self.s;
        let mut y = self.canvas.y - self.scroll * z + gap;
        for i in 0..self.ed.page_count() {
            let (pw, ph) = self.ed.page_size(i);
            let (w, h) = ((pw as f32 * z).round(), (ph as f32 * z).round());
            let x = self.canvas.x + ((self.canvas.w - w) / 2.0).max(gap);
            let r = Rect::new(x.round(), y.round(), w, h);
            if r.bottom() > self.canvas.y && r.y < self.canvas.bottom() {
                self.page_rects.push((i, r));
            }
            y += h + gap;
        }
    }

    /// How far the view can scroll, in points.
    fn scroll_max(&self) -> f32 {
        let gap = GAP;
        let mut total = gap;
        for i in 0..self.ed.page_count() {
            total += self.ed.page_size(i).1 as f32 + gap;
        }
        (total - self.canvas.h / (self.zoom * self.s).max(0.01)).max(0.0)
    }

    fn scroll_by(&mut self, points: f32) {
        self.scroll = (self.scroll + points).clamp(0.0, self.scroll_max());
    }

    /// The page and the point on it under a window point.
    fn point_on_page(&self, x: f32, y: f32) -> Option<(usize, f64, f64)> {
        let z = self.zoom * self.s;
        // the page it is on, or the nearest one above
        let mut best: Option<(usize, f64, f64)> = None;
        for (i, r) in &self.page_rects {
            if y >= r.y && y <= r.bottom() {
                return Some((*i, ((x - r.x) / z) as f64, ((y - r.y) / z) as f64));
            }
            if y > r.bottom() {
                best = Some((*i, ((x - r.x) / z) as f64, ((r.h) / z) as f64));
            }
        }
        best.or_else(|| self.page_rects.first().map(|(i, r)| (*i, ((x - r.x) / z) as f64, 0.0)))
    }

    /// Bring the caret into view.
    fn show_caret(&mut self) {
        let Some((pg, _x, y, h)) = self.ed.caret_rect(self.ed.caret) else { return };
        let gap = GAP;
        let mut top = gap;
        for i in 0..pg {
            top += self.ed.page_size(i).1 as f32 + gap;
        }
        let caret_top = top + y as f32;
        let caret_bottom = caret_top + h as f32;
        let view_h = self.canvas.h / (self.zoom * self.s).max(0.01);
        if caret_top < self.scroll + 8.0 {
            self.scroll = (caret_top - 24.0).max(0.0);
        } else if caret_bottom > self.scroll + view_h - 8.0 {
            self.scroll = (caret_bottom - view_h + 24.0).max(0.0);
        }
        self.scroll = self.scroll.clamp(0.0, self.scroll_max());
    }

    fn page_picture(&mut self, i: usize) -> Option<&sk::Pixmap> {
        let z = self.zoom * self.s;
        let ver = self.ed.version_hint() as u64;
        if let Some(at) = self.cache.iter().position(|(p, v, cz, _)| *p == i && *v == ver && (*cz - z).abs() < 1e-3) {
            return Some(&self.cache[at].3);
        }
        let (pw, ph) = self.ed.page_size(i);
        let (w, h) = (((pw * z as f64).ceil() as u32).max(1), ((ph * z as f64).ceil() as u32).max(1));
        if w > 8000 || h > 8000 {
            return None;
        }
        let mut pm = sk::Pixmap::new(w, h)?;
        pm.fill(sk::Color::WHITE);
        {
            let mut cx = Cx::new(&mut pm, &mut self.glyphs);
            cx.scale(z as f64, z as f64);
            let mut ov = Overlay { gridlines: false, marks: self.marks, ..Default::default() };
            for (pg, x, y, sw, sh) in self.ed.selection_rects() {
                if pg == i {
                    ov.selection.push((x, y, sw, sh));
                }
            }
            if !self.ed.has_selection() {
                if let Some((pg, x, y, ch)) = self.ed.caret_rect(self.ed.caret) {
                    if pg == i {
                        ov.caret = Some((x, y, ch));
                    }
                }
            }
            if let Some(page) = self.ed.layout.pages.get(i).cloned() {
                self.renderer.draw_page(&mut cx, &self.ed.layout, &page, &mut self.ed.fonts, &ov);
            }
        }
        if self.cache.len() > 8 {
            self.cache.remove(0);
        }
        self.cache.push((i, ver, z, pm));
        self.cache.last().map(|(_, _, _, p)| p)
    }

    // ------------------------------------------------------------ painting

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
        let theme = self.theme.clone();
        let s = self.s;
        pm.fill(theme.window.to_sk());
        let ribbon_h = self.ribbon.height(s);
        self.canvas = Rect::new(0.0, TITLE_H * s + ribbon_h, self.w, (self.h - TITLE_H * s - ribbon_h - BAR_H * s).max(1.0));

        let title = self.window_title();
        {
            let st = self.state();
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            let tr = Rect::new(0.0, 0.0, self.w, TITLE_H * s);
            self.title.paint(&mut cx, tr, &title, false, &|id| match id {
                "undo" => st.can_undo,
                "redo" => st.can_redo,
                _ => true,
            });
        }
        if let Pane::Backstage(_) = &self.pane {
            self.paint_backstage(pm, r);
        } else {
            self.paint_pages(pm, r);
            {
                let st = self.state();
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
                let rr = Rect::new(0.0, TITLE_H * s, self.w, ribbon_h);
                self.ribbon.paint(&mut cx, rr, &st, self.now);
            }
            self.paint_bar(pm, r);
        }
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        let win = Rect::new(0.0, 0.0, self.w, self.h);
        if let Some((msg, until)) = self.flash.clone() {
            if self.now < until {
                let fw = cx.measure(&msg, 13.0, false) + 32.0 * s;
                let fr = Rect::new((self.w - fw) / 2.0, self.canvas.bottom() - 48.0 * s, fw, 34.0 * s).round();
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
        gfx::stroke_round(cx.pm, win, 0.0, theme.border, 1.0);
    }

    fn paint_pages(&mut self, pm: &mut sk::Pixmap, _r: &mut Res) {
        let theme = self.theme.clone();
        gfx::fill_rect(pm, self.canvas, if theme.dark { Color::hex(0x2A2A2A) } else { Color::hex(0x6E6E6E) });
        self.layout_pages();
        let rects = self.page_rects.clone();
        for (i, rect) in rects {
            gfx::shadow(pm, rect, 0.0, theme.dark);
            if let Some(page) = self.page_picture(i) {
                let page = page.clone();
                gfx::draw_image(pm, &page, rect, 1.0, sk::FilterQuality::Bilinear);
            } else {
                gfx::fill_rect(pm, rect, Color::WHITE);
            }
        }
    }

    fn paint_bar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let br = Rect::new(0.0, self.h - BAR_H * s, self.w, BAR_H * s);
        gfx::fill_rect(pm, br, theme.app);
        let page = self.ed.caret_rect(self.ed.caret).map(|(p, _, _, _)| p + 1).unwrap_or(1);
        let words = self.ed.word_count();
        let left = format!("Page {page} of {}    {words} words", self.ed.page_count().max(1));
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        cx.label(&left, Rect::new(br.x + 12.0 * s, br.y, br.w / 2.0, br.h), 12.0, theme.app_text, false, Align::Left);
        let zoom = format!("{}%", (self.zoom * 100.0).round() as i32);
        cx.label(&zoom, Rect::new(br.right() - 90.0 * s, br.y, 78.0 * s, br.h), 12.0, theme.app_text, false, Align::Right);
    }

    fn paint_backstage(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let area = Rect::new(0.0, TITLE_H * s, self.w, self.h - TITLE_H * s);
        gfx::fill_rect(pm, area, theme.panel);
        let side = 190.0 * s;
        gfx::fill_rect(pm, Rect::new(area.x, area.y, side, area.h), BLUE);
        let Pane::Backstage(b) = &mut self.pane else { return };
        let page = b.page.clone();
        let items = [("new", "New"), ("open", "Open"), ("save", "Save"), ("saveas", "Save As"), ("pdf", "Export PDF"),
                     ("print", "Print"), ("close", "Back")];
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            let mut y = area.y + 16.0 * s;
            for (id, label) in items {
                let rr = Rect::new(area.x, y, side, 34.0 * s);
                if id == page {
                    gfx::fill_rect(cx.pm, rr, Color::hex(0x1F3F6E));
                }
                cx.label(label, Rect::new(rr.x + 20.0 * s, rr.y, rr.w - 24.0 * s, rr.h), 14.0, Color::WHITE, false, Align::Left);
                y += 34.0 * s;
            }
        }
        let body = Rect::new(area.x + side + 24.0 * s, area.y + 20.0 * s, area.w - side - 48.0 * s, area.h - 44.0 * s);
        if page == "open" && !self.settings.recent.is_empty() {
            let recent: Vec<String> = self.settings.recent.clone();
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            cx.label("Recent documents", Rect::new(body.x, body.y, body.w, 24.0 * s), 13.0, theme.muted, true, Align::Left);
            let mut y = body.y + 28.0 * s;
            for p in recent.iter().take(8) {
                let name = std::path::Path::new(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone());
                cx.label(&name, Rect::new(body.x, y, body.w * 0.4, 22.0 * s), 13.0, theme.text, false, Align::Left);
                y += 22.0 * s;
            }
            let rest = Rect::new(body.x + body.w * 0.42, body.y, body.w * 0.58, body.h);
            let Pane::Backstage(b) = &mut self.pane else { return };
            if let Some(f) = b.files.as_mut() {
                f.paint(&mut cx, rest);
            }
            return;
        }
        let Pane::Backstage(b) = &mut self.pane else { return };
        if let Some(f) = b.files.as_mut() {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            f.paint(&mut cx, body);
        } else {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            let note = match page.as_str() {
                "print" => "Printing writes a PDF: choose Export PDF, then send that to your printer.",
                _ => "",
            };
            cx.label(note, body, 14.0, theme.text, false, Align::Left);
        }
    }

    // ------------------------------------------------------------- events

    pub fn handle(&mut self, ev: &Event, host: &mut dyn Host) {
        if let Event::Resized { w, h, scale } = ev {
            self.w = *w as f32;
            self.h = *h as f32;
            self.s = *scale;
            self.cache.clear();
        }
        if let Event::ThemeChanged { dark } = ev {
            self.theme = Theme::new(*dark, BLUE, BLUE);
            self.cache.clear();
        }
        if let Some(d) = self.dialog.as_mut() {
            let res = {
                let mut r = self.res.take().expect("res");
                let out = d.event(ev, host, &mut r.text, self.s);
                self.res = Some(r);
                out
            };
            match res {
                DialogResult::None => return,
                DialogResult::Button(id) => {
                    let d = self.dialog.take();
                    self.dialog_button(&id, d, host);
                    return;
                }
            }
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::None => return,
                MenuResult::Closed => {
                    self.menu = None;
                    return;
                }
                MenuResult::Picked(id) => {
                    self.menu = None;
                    self.command(&id, host);
                    return;
                }
            }
        }
        {
            let (act, used) = self.title.event(ev);
            if let Some(a) = act {
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
            if used {
                return;
            }
        }
        if let Pane::Backstage(_) = &self.pane {
            self.backstage_event(ev, host);
            return;
        }
        {
            let st = self.state();
            let (act, used) = self.ribbon.event(ev, &st, self.now);
            if let Some(a) = act {
                match a {
                    RibbonAction::Command(id) => self.command(&id, host),
                    RibbonAction::Menu(id, r) => self.open_menu(&id, r),
                    RibbonAction::Backstage => {
                        self.pane = Pane::Backstage(Back { page: "open".into(), files: Some(self.file_pane(false)) })
                    }
                    RibbonAction::GroupMenu(items, r) => {
                        let list = items.iter().map(|(id, label, icon)| MenuItem::new(id, label).icon(icon)).collect();
                        self.menu = Some(Menu::new("group", r, list));
                    }
                }
                return;
            }
            if used {
                return;
            }
        }
        match ev {
            Event::MouseDown { x, y, button: Button::Left, clicks } => {
                if self.canvas.contains(*x, *y) {
                    self.layout_pages();
                    if let Some((pg, px, py)) = self.point_on_page(*x, *y) {
                        if let Some(p) = self.ed.hit(pg, px, py) {
                            match clicks {
                                2 => {
                                    let (a, b) = self.ed.word_at(p);
                                    self.ed.select_ranges(&[(a, b)]);
                                }
                                _ => {
                                    self.ed.move_to(p, false, false);
                                    self.dragging = true;
                                }
                            }
                            host.redraw();
                        }
                    }
                }
            }
            Event::MouseMove { x, y } => {
                if self.dragging {
                    self.layout_pages();
                    if let Some((pg, px, py)) = self.point_on_page(*x, *y) {
                        if let Some(p) = self.ed.hit(pg, px, py) {
                            self.ed.move_to(p, true, false);
                            host.redraw();
                        }
                    }
                }
                host.set_cursor(if self.canvas.contains(*x, *y) { Cursor::Text } else { Cursor::Default });
            }
            Event::MouseUp { button: Button::Left, .. } => self.dragging = false,
            Event::Wheel { dy, .. } => {
                self.scroll_by(-dy * 48.0 / self.zoom.max(0.1));
                host.redraw();
            }
            Event::Text(t) => {
                if !t.is_empty() && !t.chars().any(|c| c.is_control()) {
                    self.ed.type_text(t);
                    self.show_caret();
                    host.redraw();
                }
            }
            Event::Key { key, mods, down: true, .. } => self.key(key.clone(), *mods, host),
            Event::CloseRequested => self.ask_to_close(),
            Event::FileDropped(p) => {
                let p = p.clone();
                self.open_path(&p);
                host.redraw();
            }
            _ => {}
        }
    }

    fn key(&mut self, key: Key, mods: Mods, host: &mut dyn Host) {
        let shift = mods.shift;
        if mods.ctrl {
            let id = match key {
                Key::Char('s') => Some("save"),
                Key::Char('o') => Some("open"),
                Key::Char('n') => Some("new"),
                Key::Char('p') => Some("pdf"),
                Key::Char('z') => Some("undo"),
                Key::Char('y') => Some("redo"),
                Key::Char('b') => Some("bold"),
                Key::Char('i') => Some("italic"),
                Key::Char('u') => Some("underline"),
                Key::Char('a') => Some("select-all"),
                Key::Char('f') => Some("find"),
                Key::Char('c') => Some("copy"),
                Key::Char('x') => Some("cut"),
                Key::Char('v') => Some("paste"),
                Key::Char('=') | Key::Char('+') => Some("zoom-in"),
                Key::Char('-') => Some("zoom-out"),
                Key::Char('0') => Some("zoom-100"),
                Key::Enter => Some("break-page"),
                _ => None,
            };
            if let Some(id) = id {
                self.command(id, host);
                return;
            }
        }
        let caret = self.ed.caret;
        match key {
            Key::Left => {
                let p = if mods.ctrl { self.ed.prev_word(caret) } else { self.ed.prev_pos(caret) };
                self.ed.move_to(p, shift, false);
            }
            Key::Right => {
                let p = if mods.ctrl { self.ed.next_word(caret) } else { self.ed.next_pos(caret) };
                self.ed.move_to(p, shift, false);
            }
            Key::Up => {
                let p = self.ed.line_move(caret, false);
                self.ed.move_to(p, shift, false);
            }
            Key::Down => {
                let p = self.ed.line_move(caret, true);
                self.ed.move_to(p, shift, false);
            }
            Key::Home => {
                let p = if mods.ctrl { self.ed.doc_start() } else { self.ed.line_start(caret) };
                self.ed.move_to(p, shift, false);
            }
            Key::End => {
                let p = if mods.ctrl { self.ed.doc_end() } else { self.ed.line_end(caret) };
                self.ed.move_to(p, shift, false);
            }
            Key::PageUp | Key::PageDown => {
                let lines = (self.canvas.h / (self.zoom * self.s * 14.0)).max(1.0) as i32;
                let mut p = caret;
                let down = key == Key::PageDown;
                for _ in 0..lines {
                    p = self.ed.line_move(p, down);
                }
                self.ed.move_to(p, shift, false);
            }
            Key::Backspace => self.ed.delete(false, mods.ctrl),
            Key::Delete => self.ed.delete(true, mods.ctrl),
            Key::Enter => self.ed.new_paragraph(),
            Key::Tab => self.ed.type_text("\t"),
            _ => return,
        }
        self.show_caret();
        host.redraw();
    }

    fn backstage_event(&mut self, ev: &Event, host: &mut dyn Host) {
        let s = self.s;
        if let Event::MouseDown { x, y, button: Button::Left, .. } = ev {
            let area_y = TITLE_H * s;
            let side = 190.0 * s;
            if *x < side {
                let i = ((*y - area_y - 16.0 * s) / (34.0 * s)).floor();
                let items = ["new", "open", "save", "saveas", "pdf", "print", "close"];
                if i >= 0.0 && (i as usize) < items.len() {
                    let id = items[i as usize];
                    self.backstage_page(id, host);
                    return;
                }
            }
        }
        let mut chosen = None;
        if let Pane::Backstage(b) = &mut self.pane {
            if let Some(f) = b.files.as_mut() {
                let mut r = self.res.take().expect("res");
                let act = f.event(ev, host, &mut r.text, s);
                self.res = Some(r);
                if let FileAction::Chosen(p) = act {
                    chosen = Some((b.page.clone(), p));
                }
            }
        }
        if let Some((page, p)) = chosen {
            match page.as_str() {
                "picture" => match std::fs::read(&p) {
                    Ok(bytes) => {
                        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        if !self.ed.insert_picture(&bytes, &name) {
                            self.say("Can't use that picture", "Folio Page knows PNG, JPEG, GIF, BMP and WebP pictures.");
                        }
                        self.cache.clear();
                    }
                    Err(e) => self.say("Can't read the picture", &e.to_string()),
                },
                "open" => self.open_path(&p),
                "save" | "saveas" => self.save(&p),
                "pdf" => self.export_pdf(&p),
                _ => {}
            }
            self.pane = Pane::None;
            host.redraw();
        }
        if let Event::Key { key: Key::Escape, down: true, .. } = ev {
            self.pane = Pane::None;
            host.redraw();
        }
    }

    fn backstage_page(&mut self, id: &str, host: &mut dyn Host) {
        match id {
            "close" => self.pane = Pane::None,
            "new" => {
                self.ed = Editor::new(PageApp::blank(), FontCache::new());
                self.path = None;
                self.cache.clear();
                self.scroll = 0.0;
                self.pane = Pane::None;
            }
            "save" if self.path.is_some() => {
                let p = self.path.clone().unwrap();
                self.save(&p);
                self.pane = Pane::None;
            }
            "print" => {
                if let Pane::Backstage(b) = &mut self.pane {
                    b.page = "print".into();
                    b.files = None;
                }
            }
            "pdf" => {
                let f = self.file_pane_named(true, "pdf");
                if let Pane::Backstage(b) = &mut self.pane {
                    b.page = "pdf".into();
                    b.files = Some(f);
                }
            }
            "open" => {
                let f = self.file_pane(false);
                if let Pane::Backstage(b) = &mut self.pane {
                    b.page = "open".into();
                    b.files = Some(f);
                }
            }
            _ => {
                let f = self.file_pane(true);
                if let Pane::Backstage(b) = &mut self.pane {
                    b.page = "saveas".into();
                    b.files = Some(f);
                }
            }
        }
        host.redraw();
    }

    fn file_pane(&self, save: bool) -> FilePane {
        self.file_pane_named(save, "docx")
    }

    fn file_pane_named(&self, save: bool, ext: &str) -> FilePane {
        let dir = self
            .path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| folio_kit::platform::home_dir());
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "Document".into());
        FilePane::new(&dir, &[ext], save, &format!("{name}.{ext}"))
    }

    /// A dialog button pressed, for the window tests.
    pub fn press_dialog_button(&mut self, id: &str, host: &mut dyn Host) {
        let d = self.dialog.take();
        self.dialog_button(id, d, host);
    }

    /// A button in a dialog: Find and Replace does its work and stays open.
    fn dialog_button(&mut self, id: &str, d: Option<Dialog>, host: &mut dyn Host) {
        let Some(d) = d else { return };
        if d.id == "close" {
            match id {
                "save" => {
                    match self.path.clone() {
                        Some(p) => {
                            self.save(&p);
                            self.remember();
                            self.exit = true;
                        }
                        None => {
                            self.closing = false;
                            self.command("saveas", host);
                        }
                    };
                }
                "discard" => {
                    self.remember();
                    self.exit = true;
                }
                _ => self.closing = false,
            }
            host.redraw();
            return;
        }
        if d.id != "find" || id == "close" {
            host.redraw();
            return;
        }
        let what = d.value("what");
        let with = d.value("with");
        let opts = folio_page_editor::search::FindOpts {
            match_case: d.checked("case"),
            whole_word: d.checked("word"),
            ..Default::default()
        };
        if !what.is_empty() {
            match id {
                "next" => match self.ed.find(&what, &opts, true, true) {
                    Ok(Some(_)) => self.show_caret(),
                    Ok(None) => self.flash(&format!("{what} isn't in the document")),
                    Err(e) => self.flash(&e),
                },
                "replace" => {
                    if self.ed.replace_one(&what, &with, &opts).unwrap_or(false) {
                        self.show_caret();
                    }
                }
                "all" => match self.ed.replace_all(&what, &with, &opts) {
                    Ok(n) => self.flash(&format!("{n} replaced")),
                    Err(e) => self.flash(&e),
                },
                _ => {}
            }
            self.cache.clear();
        }
        self.dialog = Some(d);
        host.redraw();
    }

    /// Closing: a changed document asks to be saved first.
    fn ask_to_close(&mut self) {
        if !self.ed.modified || self.test.is_some() {
            self.remember();
            self.exit = true;
            return;
        }
        self.closing = true;
        let name = self.path.as_deref().map(name_of).unwrap_or_else(|| "this document".into());
        self.dialog = Some(
            Dialog::new("close", "Save before closing?")
                .note(&format!("{name} has changes that have not been saved."))
                .button("save", "Save")
                .button("discard", "Close without saving")
                .button("cancel", "Keep working"),
        );
    }

    /// The page the caret is on.
    fn caret_page(&self) -> usize {
        self.ed.caret_rect(self.ed.caret).map(|(p, _, _, _)| p).unwrap_or(0)
    }

    fn open_menu(&mut self, id: &str, r: Rect) {
        let mut items: Vec<MenuItem> = Vec::new();
        match id {
            "font-color" | "highlight" => {
                for c in PALETTE {
                    items.push(MenuItem::new(&format!("{id}:{c:06X}"), &color_name(c)).swatch(Color::hex(c)));
                }
                items.push(MenuItem::new(&format!("{id}:none"), "No colour"));
            }
            "zoom" => {
                for z in ZOOMS {
                    items.push(MenuItem::new(&format!("zoom:{}", (z * 100.0) as i32), &format!("{}%", (z * 100.0) as i32)));
                }
            }
            "size" => {
                for v in SIZES {
                    items.push(MenuItem::new(&format!("size:{v}"), &format!("{v}")));
                }
            }
            "font" => {
                for f in self.ed.families().into_iter().take(80) {
                    items.push(MenuItem::new(&format!("font:{f}"), &f));
                }
            }
            "margins" => {
                for (name, twips) in [("Normal (1\")", 1440), ("Narrow (0.5\")", 720), ("Moderate (0.75\")", 1080),
                                      ("Wide (1.5\")", 2160)] {
                    items.push(MenuItem::new(&format!("margin:{twips}"), name));
                }
            }
            "orientation" => {
                items.push(MenuItem::new("orient:portrait", "Portrait"));
                items.push(MenuItem::new("orient:landscape", "Landscape"));
            }
            "paper" => {
                for (name, w, h) in [("Letter", 12240, 15840), ("Legal", 12240, 20160), ("A4", 11906, 16838),
                                     ("A5", 8391, 11906)] {
                    items.push(MenuItem::new(&format!("paper-size:{w}x{h}"), name));
                }
            }
            "case" => {
                for (id, label) in [("upper", "UPPERCASE"), ("lower", "lowercase"), ("sentence", "Sentence case"),
                                    ("title", "Capitalise Each Word"), ("toggle", "tOGGLE cASE")] {
                    items.push(MenuItem::new(&format!("case-set:{id}"), label));
                }
            }
            "line-spacing" => {
                for v in ["1.0", "1.15", "1.5", "2.0"] {
                    items.push(MenuItem::new(&format!("spacing:{v}"), v));
                }
            }
            "borders" => {
                items.push(MenuItem::new("border:all", "All borders"));
                items.push(MenuItem::new("border:outside", "Outside borders"));
                items.push(MenuItem::new("border:none", "No borders"));
            }
            "styles" => {
                for s in self.ed.style_names().into_iter().take(40) {
                    items.push(MenuItem::new(&format!("style:{s}"), &s));
                }
            }
            _ => return,
        }
        self.menu = Some(Menu::new(id, r, items));
    }

    // ----------------------------------------------------------- commands

    pub fn command(&mut self, id: &str, host: &mut dyn Host) {
        let (head, arg) = match id.split_once(':') {
            Some((a, b)) => (a, Some(b.to_string())),
            None => (id, None),
        };
        match head {
            "new" | "open" | "saveas" | "pdf" | "print" => {
                self.pane = Pane::Backstage(Back { page: head.into(), files: None });
                self.backstage_page(head, host);
                return;
            }
            "save" => match self.path.clone() {
                Some(p) => self.save(&p),
                None => {
                    self.pane = Pane::Backstage(Back { page: "saveas".into(), files: None });
                    self.backstage_page("saveas", host);
                    return;
                }
            },
            "undo" => {
                self.ed.undo();
                self.show_caret();
            }
            "redo" => {
                self.ed.redo();
                self.show_caret();
            }
            "select-all" => self.ed.select_all(),
            "copy" | "cut" => {
                let text = self.ed.selected_text();
                if !text.is_empty() {
                    host.set_clipboard_text(&text);
                }
                if head == "cut" {
                    self.ed.delete(true, false);
                }
            }
            "paste" => {
                if let Some(t) = host.clipboard_text() {
                    self.ed.type_text(&t);
                    self.show_caret();
                }
            }
            "bold" | "italic" | "underline" | "strike" | "sub" | "super" => {
                self.ed.toggle(&format!("w:{}", match head { "bold" => "b", "italic" => "i", "underline" => "u", "strike" => "strike", "sub" => "vertAlign", _ => "vertAlign" }), head);
            }
            "align-left" | "align-center" | "align-right" | "align-justify" => {
                let jc = match head {
                    "align-center" => "center",
                    "align-right" => "right",
                    "align-justify" => "both",
                    _ => "left",
                };
                self.ed.set_align(jc);
            }
            "bullets" | "numbering" => {
                self.ed.list_toggle(head == "numbering");
            }
            "indent" | "outdent" => self.ed.indent_step(if head == "indent" { 1 } else { -1 }),
            "font" => {
                if let Some(f) = arg {
                    self.ed.run_prop("w:rFonts", Some(&[("w:ascii", &f), ("w:hAnsi", &f)]), "Font");
                }
            }
            "size" => {
                if let Some(v) = arg.and_then(|a| a.parse::<f64>().ok()) {
                    let hp = format!("{}", (v * 2.0).round() as i64);
                    self.ed.run_prop("w:sz", Some(&[("w:val", &hp)]), "Font size");
                }
            }
            "font-color" => {
                let v = arg.unwrap_or_default();
                if v == "none" {
                    self.ed.run_prop("w:color", None, "Font colour");
                } else {
                    self.ed.run_prop("w:color", Some(&[("w:val", &v)]), "Font colour");
                }
            }
            "highlight" => {
                let v = arg.unwrap_or_default();
                if v == "none" {
                    self.ed.run_prop("w:highlight", None, "Highlight");
                } else {
                    self.ed.run_prop("w:highlight", Some(&[("w:val", "yellow")]), "Highlight");
                }
            }
            "style" => {
                if let Some(s) = arg {
                    self.ed.apply_style_named(&s);
                }
            }
            "break-page" => {
                self.ed.insert_break("page");
                self.show_caret();
            }
            "break-column" => {
                self.ed.insert_break("column");
                self.show_caret();
            }
            "table" => {
                self.ed.insert_table(3, 3);
                self.show_caret();
            }
            "picture" => {
                self.pane = Pane::Backstage(Back { page: "picture".into(), files: Some(self.file_pane_named(false, "png")) });
            }
            "header" => self.ed.edit_header_footer(Some(self.caret_page()), false),
            "footer" => self.ed.edit_header_footer(Some(self.caret_page()), true),
            "body" => self.ed.edit_notes(None),
            "page-number" => {
                self.ed.insert_field("PAGE", "");
                self.show_caret();
            }
            "margins" | "orientation" | "paper" => {}
            "margin" => {
                if let Some(v) = arg.and_then(|a| a.parse::<i32>().ok()) {
                    let t = v.to_string();
                    let sec = self.ed.current_section();
                    self.ed.section_attr(sec, "w:pgMar", "w:top", Some(&t), "Margins");
                    self.ed.section_attr(sec, "w:pgMar", "w:bottom", Some(&t), "Margins");
                    self.ed.section_attr(sec, "w:pgMar", "w:left", Some(&t), "Margins");
                    self.ed.section_attr(sec, "w:pgMar", "w:right", Some(&t), "Margins");
                    self.ed.relayout();
                    self.cache.clear();
                }
            }
            "orient" => {
                let land = arg.as_deref() == Some("landscape");
                let sec = self.ed.current_section();
                let (w, h) = self.ed.page_size(self.caret_page());
                let (long, short) = (w.max(h), w.min(h));
                let (pw, ph) = if land { (long, short) } else { (short, long) };
                let tw = format!("{}", (pw * 20.0).round() as i64);
                let th = format!("{}", (ph * 20.0).round() as i64);
                self.ed.section_attr(sec, "w:pgSz", "w:w", Some(&tw), "Orientation");
                self.ed.section_attr(sec, "w:pgSz", "w:h", Some(&th), "Orientation");
                self.ed.section_attr(sec, "w:pgSz", "w:orient", Some(if land { "landscape" } else { "portrait" }), "Orientation");
                self.ed.relayout();
                self.cache.clear();
            }
            "paper-size" => {
                if let Some((w, h)) = arg.as_deref().and_then(|a| a.split_once('x')) {
                    let sec = self.ed.current_section();
                    self.ed.section_attr(sec, "w:pgSz", "w:w", Some(w), "Paper size");
                    self.ed.section_attr(sec, "w:pgSz", "w:h", Some(h), "Paper size");
                    self.ed.relayout();
                    self.cache.clear();
                }
            }
            "find" => {
                self.dialog = Some(
                    Dialog::new("find", "Find and Replace")
                        .text("what", "Find what", "")
                        .text("with", "Replace with", "")
                        .check("case", "Match case", false)
                        .check("word", "Whole words only", false)
                        .button("next", "Find Next")
                        .button("replace", "Replace")
                        .button("all", "Replace All")
                        .button("close", "Close"),
                );
            }
            "toc" => {
                let ok = self.ed.insert_table_field("TOC \\o \"1-3\" \\h \\z \\u", None, Some("default"), None, None, 1);
                if !ok {
                    self.flash("A contents list goes in the body of the document, not in a table");
                }
                self.cache.clear();
            }
            "update-fields" => {
                let n = self.ed.update_fields(true);
                self.flash(&format!("{n} fields brought up to date"));
                self.cache.clear();
            }
            "footnote" | "endnote" => {
                if self.ed.insert_note(head == "endnote") {
                    self.show_caret();
                } else {
                    self.flash("A note goes in the body of the document");
                }
            }
            "comment" => {
                let who = folio_kit::platform::user_name();
                if self.ed.insert_comment("", &who).is_none() {
                    self.flash("Select the words the comment is about");
                }
            }
            "comment-next" => {
                if !self.ed.goto_comment(true) {
                    self.flash("No comments");
                } else {
                    self.show_caret();
                }
            }
            "comment-delete" => {
                let here: Vec<String> = self.ed.comment_at_caret().into_iter().collect();
                let n = if here.is_empty() {
                    self.flash("The caret isn't in a comment");
                    0
                } else {
                    self.ed.delete_comments(&here)
                };
                let _ = n;
                self.cache.clear();
            }
            "track" => {
                let on = !self.ed.tracking();
                self.ed.set_track_changes(on);
                self.flash(if on { "Changes are being recorded" } else { "Changes are no longer recorded" });
            }
            "accept" | "reject" => {
                let n = self.ed.accept_reject(head == "accept");
                if n == 0 {
                    self.flash("No change there");
                }
                self.cache.clear();
            }
            "accept-all" | "reject-all" => {
                let n = self.ed.accept_reject_all(head == "accept-all", None);
                self.flash(&format!("{n} changes {}", if head == "accept-all" { "accepted" } else { "rejected" }));
                self.cache.clear();
            }
            "row-below" | "row-above" => {
                if !self.ed.insert_rows(head == "row-below") {
                    self.flash("The caret isn't in a table");
                }
                self.cache.clear();
            }
            "col-right" | "col-left" => {
                if !self.ed.insert_cols(head == "col-right") {
                    self.flash("The caret isn't in a table");
                }
                self.cache.clear();
            }
            "row-delete" => {
                self.ed.delete_rows();
                self.cache.clear();
            }
            "col-delete" => {
                self.ed.delete_cols();
                self.cache.clear();
            }
            "table-delete" => {
                self.ed.delete_table();
                self.cache.clear();
            }
            "merge" => {
                if !self.ed.merge_cells() {
                    self.flash("Select the cells to join first");
                }
                self.cache.clear();
            }
            "borders" => {}
            "border" => {
                let which = arg.unwrap_or_default();
                let single = vec![("w:val".to_string(), "single".to_string()), ("w:sz".to_string(), "4".to_string())];
                let sides: Vec<(String, Option<Vec<(String, String)>>)> = match which.as_str() {
                    "all" => ["top", "left", "bottom", "right", "insideH", "insideV"]
                        .iter()
                        .map(|s| (format!("w:{s}"), Some(single.clone())))
                        .collect(),
                    "none" => ["top", "left", "bottom", "right", "insideH", "insideV"]
                        .iter()
                        .map(|s| (format!("w:{s}"), None))
                        .collect(),
                    "outside" => ["top", "left", "bottom", "right"]
                        .iter()
                        .map(|s| (format!("w:{s}"), Some(single.clone())))
                        .collect(),
                    _ => Vec::new(),
                };
                if !sides.is_empty() {
                    self.ed.set_borders("table", &sides);
                    self.cache.clear();
                }
            }
            "case" => {}
            "case-set" => {
                if let Some(m) = arg {
                    self.ed.change_case(&m);
                    self.cache.clear();
                }
            }
            "clear-format" => {
                self.ed.clear_format();
                self.cache.clear();
            }
            "painter" => {
                if self.painter {
                    self.ed.paste_format();
                    self.painter = false;
                } else {
                    self.ed.copy_format();
                    self.painter = true;
                    self.flash("Now select the text to give that look to");
                }
                self.cache.clear();
            }
            "line-spacing" => {}
            "spacing" => {
                if let Some(v) = arg.and_then(|a| a.parse::<f64>().ok()) {
                    let n = format!("{}", (v * 240.0).round() as i64);
                    self.ed.format_paras_attrs("w:spacing", &[("w:line", Some(&n)), ("w:lineRule", Some("auto"))],
                                               "Line spacing");
                    self.cache.clear();
                }
            }
            "view-print" | "view-draft" | "view-web" => {
                let mode = match head {
                    "view-draft" => 1,
                    "view-web" => 2,
                    _ => 0,
                };
                let width = (self.canvas.w / (self.zoom * self.s).max(0.1)) as f64 - 64.0;
                self.ed.set_view_mode(mode, width.max(200.0));
                self.view_mode = mode;
                self.cache.clear();
            }
            "marks" => {
                self.marks = !self.marks;
                self.cache.clear();
            }
            "zoom" => {
                if let Some(v) = arg.and_then(|a| a.parse::<f32>().ok()) {
                    self.set_zoom(v / 100.0);
                }
            }
            "zoom-in" => {
                let z = ZOOMS.iter().copied().find(|z| *z > self.zoom + 0.001).unwrap_or(4.0);
                self.set_zoom(z);
            }
            "zoom-out" => {
                let z = ZOOMS.iter().rev().copied().find(|z| *z < self.zoom - 0.001).unwrap_or(0.25);
                self.set_zoom(z);
            }
            "zoom-100" => self.set_zoom(1.0),
            "close" => self.ask_to_close(),
            _ => {}
        }
        self.cache.retain(|(_, v, _, _)| *v == self.ed.version_hint() as u64);
        host.redraw();
    }

    fn set_zoom(&mut self, z: f32) {
        self.zoom = z.clamp(0.1, 4.0);
        self.cache.clear();
    }

    pub fn state(&self) -> State {
        State {
            can_undo: self.ed.can_undo(),
            can_redo: self.ed.can_redo(),
            bold: self.ed.toggle_state("w:b"),
            italic: self.ed.toggle_state("w:i"),
            underline: self.ed.toggle_state("w:u"),
            marks: self.marks,
            track: self.ed.tracking(),
            painter: self.painter,
            view_mode: self.view_mode,
            zoom: format!("{}%", (self.zoom * 100.0).round() as i32),
            font: self.ed.current_font().unwrap_or_default(),
            size: self.ed.current_size().map(|v| format!("{v}")).unwrap_or_default(),
            align: self.ed.current_align(),
        }
    }
}

pub struct State {
    pub can_undo: bool,
    pub can_redo: bool,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub marks: bool,
    pub track: bool,
    pub painter: bool,
    pub view_mode: u8,
    pub zoom: String,
    pub font: String,
    pub size: String,
    pub align: String,
}

impl CommandState for State {
    fn enabled(&self, id: &str) -> bool {
        match id {
            "undo" => self.can_undo,
            "redo" => self.can_redo,
            _ => true,
        }
    }

    fn checked(&self, id: &str) -> bool {
        match id {
            "bold" => self.bold,
            "italic" => self.italic,
            "underline" => self.underline,
            "marks" => self.marks,
            "track" => self.track,
            "painter" => self.painter,
            "view-print" => self.view_mode == 0,
            "view-draft" => self.view_mode == 1,
            "view-web" => self.view_mode == 2,
            "align-left" => self.align == "left",
            "align-center" => self.align == "center",
            "align-right" => self.align == "right",
            "align-justify" => self.align == "both",
            _ => false,
        }
    }

    fn text(&self, id: &str) -> Option<String> {
        match id {
            "zoom" => Some(self.zoom.clone()),
            "font" => Some(self.font.clone()),
            "size" => Some(self.size.clone()),
            _ => None,
        }
    }
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string_lossy().to_string())
}

fn with_ext(p: &Path, ext: &str) -> PathBuf {
    if p.extension().map(|e| e.eq_ignore_ascii_case(ext)).unwrap_or(false) {
        p.to_path_buf()
    } else {
        p.with_extension(ext)
    }
}

fn color_name(c: u32) -> String {
    match c {
        0x000000 => "Black".into(),
        0xFFFFFF => "White".into(),
        0xFF0000 => "Red".into(),
        0x00B050 => "Green".into(),
        0x0070C0 => "Blue".into(),
        0xFFFF00 => "Yellow".into(),
        _ => format!("#{c:06X}"),
    }
}

impl PageApp {
    /// Where the pages are on screen right now.
    pub fn page_rects_now(&self) -> Vec<(usize, Rect)> {
        self.page_rects.clone()
    }

    /// Closing, as the window would ask it, for the tests.
    pub fn ask_to_close_for_test(&mut self) {
        let keep = self.test.take();
        self.ask_to_close();
        self.test = keep;
    }

    /// Whether the window is on its way out, for the tests.
    pub fn wants_exit_for_test(&self) -> bool {
        self.exit
    }

    /// The part of the window the pages are drawn in.
    pub fn canvas_rect(&self) -> Rect {
        self.canvas
    }
}

impl folio_kit::platform::App for PageApp {
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
                // the next step runs on the next frame
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

