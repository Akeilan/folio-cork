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
/// The navigation pane's width and the ruler's height, in points at 100%.
const NAV_W: f32 = 230.0;
const RULER_H: f32 = 22.0;
/// The space round and between the pages, in points at 100%.
const GAP: f32 = 16.0;

pub const ZOOMS: [f32; 11] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0];
pub const SIZES: [f32; 16] =
    [8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0, 48.0, 72.0, 96.0];
/// The colours of the font-colour and highlight menus.
/// The symbols the Insert menu offers.
pub const SYMBOLS: [char; 24] = ['\u{a9}', '\u{ae}', '\u{2122}', '\u{b0}', '\u{b1}', '\u{d7}', '\u{f7}', '\u{2260}',
                                 '\u{2264}', '\u{2265}', '\u{2248}', '\u{221e}', '\u{3bc}', '\u{3c0}', '\u{3a9}',
                                 '\u{3b1}', '\u{3b2}', '\u{2211}', '\u{221a}', '\u{20ac}', '\u{a3}', '\u{a5}',
                                 '\u{2026}', '\u{2014}'];

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
    nav: bool,
    ruler: bool,
    read_mode: bool,
    printers: Vec<String>,
    /// what the cross-reference dialog is offering
    ref_targets: Vec<String>,
    /// the headings in the navigation pane, with where each one sits
    nav_rows: Vec<(folio_page_editor::eng::xml::NodeId, Rect)>,
    nav_rect: Rect,
    ruler_rect: Rect,
    /// an indent marker being dragged: 0 first line, 1 hanging, 2 left, 3 right
    drag_indent: Option<u8>,
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
            nav: false,
            ruler: false,
            read_mode: false,
            printers: Vec::new(),
            ref_targets: Vec::new(),
            nav_rows: Vec::new(),
            nav_rect: Rect::default(),
            ruler_rect: Rect::default(),
            drag_indent: None,
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
        self.nav = self.settings.nav;
        self.ruler = self.settings.ruler;
        if self.settings.view_mode != 0 {
            self.view_mode = self.settings.view_mode;
            self.ed.set_view_mode(self.view_mode, 600.0);
        }
    }

    fn remember(&mut self) {
        self.settings.zoom = self.zoom;
        self.settings.marks = self.marks;
        self.settings.nav = self.nav;
        self.settings.ruler = self.ruler;
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
        let meta = self.ed.doc_properties();
        match self.ed.export_pdf_opts(&p.to_string_lossy(), Vec::new(), true, meta) {
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
        let ribbon_h = if self.read_mode { 0.0 } else { self.ribbon.height(s) };
        let top = TITLE_H * s + ribbon_h;
        let nav_w = if self.nav && !self.read_mode { NAV_W * s } else { 0.0 };
        let ruler_h = if self.ruler && !self.read_mode { RULER_H * s } else { 0.0 };
        self.nav_rect = Rect::new(0.0, top, nav_w, (self.h - top - BAR_H * s).max(1.0));
        self.ruler_rect = Rect::new(nav_w, top, (self.w - nav_w).max(1.0), ruler_h);
        self.canvas = Rect::new(nav_w, top + ruler_h,
                                (self.w - nav_w).max(1.0),
                                (self.h - top - ruler_h - BAR_H * s).max(1.0));

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
            if self.read_mode {
                self.paint_bar(pm, r);
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
                let tip = "Reading. Esc goes back to the ribbon.";
                cx.label(tip, Rect::new(0.0, TITLE_H * s + 4.0 * s, self.w, 20.0 * s), 12.0, Color::hex(0xDDDDDD),
                         false, Align::Center);
                return;
            }
            if self.ruler {
                self.paint_ruler(pm, r);
            }
            if self.nav {
                self.paint_nav(pm, r);
            }
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

    /// The navigation pane: the document's headings, to jump by.
    fn paint_nav(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let area = self.nav_rect;
        gfx::fill_rect(pm, area, theme.panel);
        gfx::vline(pm, area.right() - 0.5, area.y, area.bottom(), theme.border);
        self.nav_rows.clear();
        let heads: Vec<(folio_page_editor::eng::xml::NodeId, String, i32)> = self
            .ed
            .outline_paras()
            .into_iter()
            .filter(|(_, text, level, ..)| *level <= 9 && !text.trim().is_empty())
            .map(|(n, text, level, ..)| (n, text, level))
            .collect();
        let here = self.ed.caret.para;
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
        cx.label("Headings", Rect::new(area.x + 12.0 * s, area.y + 6.0 * s, area.w - 16.0 * s, 22.0 * s), 12.0,
                 theme.muted, true, Align::Left);
        let mut y = area.y + 30.0 * s;
        if heads.is_empty() {
            cx.label("No headings yet", Rect::new(area.x + 12.0 * s, y, area.w - 16.0 * s, 22.0 * s), 12.0,
                     theme.muted, false, Align::Left);
            return;
        }
        for (node, text, level) in heads {
            if y > area.bottom() - 18.0 * s {
                break;
            }
            let row = Rect::new(area.x, y, area.w, 22.0 * s);
            if node == here {
                gfx::fill_rect(cx.pm, row, theme.checked);
            }
            let indent = 12.0 * s + (level.max(1) - 1) as f32 * 12.0 * s;
            let line: String = text.chars().take(60).collect();
            cx.label(&line, Rect::new(row.x + indent, row.y, row.w - indent - 8.0 * s, row.h), 12.5, theme.text,
                     level <= 1, Align::Left);
            self.nav_rows.push((node, row));
            y += 22.0 * s;
        }
    }

    /// The ruler over the page: the margins, and the paragraph's own indents.
    fn paint_ruler(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let area = self.ruler_rect;
        gfx::fill_rect(pm, area, theme.panel);
        gfx::hline(pm, area.x, area.right(), area.bottom() - 0.5, theme.border);
        let Some(page) = self.page_rects.first().map(|(_, r)| *r) else { return };
        let z = self.zoom * self.s;
        let pp = self.ed.current_para_props();
        let (left_m, right_edge) = self.ed.page_margins(0);
        let band = Rect::new(page.x, area.y + 6.0 * s, page.w, area.h - 12.0 * s);
        // the paper, then the part the text sits in
        gfx::fill_rect(pm, band, if theme.dark { Color::hex(0x3A3A3A) } else { Color::hex(0xD9D9D9) });
        let text_x = page.x + (left_m as f32) * z;
        let text_w = ((right_edge - left_m) as f32 * z).max(1.0);
        gfx::fill_rect(pm, Rect::new(text_x, band.y, text_w, band.h), if theme.dark { Color::hex(0x555555) } else { Color::WHITE });
        // every inch, a mark
        let inch = 72.0 * z;
        let mut x = text_x;
        while x < page.right() {
            gfx::vline(pm, x.round(), band.y + band.h * 0.3, band.bottom() - band.h * 0.3, theme.muted);
            x += inch;
        }
        let twip = |v: Option<i32>| v.unwrap_or(0) as f32 / 20.0 * z;
        let first = text_x + twip(pp.ind_left) + twip(pp.ind_first);
        let hang = text_x + twip(pp.ind_left);
        let right = text_x + text_w - twip(pp.ind_right);
        for (cx_pos, up) in [(first, true), (hang, false), (right, false)] {
            let t = 5.0 * s;
            let y = if up { band.y } else { band.bottom() - t };
            let tri = Rect::new(cx_pos - t, y, t * 2.0, t);
            gfx::fill_rect(pm, tri, theme.accent);
        }
        let _ = r;
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
                     ("close", "Back")];
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
            let note = "";
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
                if self.nav && self.nav_rect.contains(*x, *y) {
                    if let Some((node, _)) = self.nav_rows.iter().find(|(_, r)| r.contains(*x, *y)).cloned() {
                        let to = folio_page_editor::Pos { para: node, off: 0 };
                        self.ed.move_to(to, false, false);
                        self.show_caret();
                        host.redraw();
                    }
                    return;
                }
                if self.ruler && self.ruler_rect.contains(*x, *y) {
                    self.drag_indent = self.indent_handle_at(*x, *y);
                    return;
                }
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
                if let Some(which) = self.drag_indent {
                    self.drag_indent_to(which, *x);
                    host.redraw();
                    return;
                }
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
            Event::MouseUp { button: Button::Left, .. } => {
                self.dragging = false;
                self.drag_indent = None;
            }
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
            Event::Key { key: Key::Escape, down: true, .. } if self.read_mode => {
                self.read_mode = false;
                host.redraw();
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
                let items = ["new", "open", "save", "saveas", "pdf", "close"];
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
        if d.id == "print" {
            if id == "ok" {
                self.print_now(&d);
            }
            host.redraw();
            return;
        }
        if matches!(d.id.as_str(), "caption" | "xref" | "bookmark" | "citation") {
            if id == "ok" {
                self.apply_dialog(&d);
            }
            host.redraw();
            return;
        }
        if d.id == "para" || d.id == "font" || d.id == "pagesetup" || d.id == "props" {
            if id == "ok" {
                self.apply_dialog(&d);
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

    /// Which indent marker is under a point on the ruler. The first-line
    /// marker sits on top and the hanging one underneath, as Word has them,
    /// so the half of the ruler the click lands in tells them apart when they
    /// are at the same place.
    fn indent_handle_at(&self, x: f32, y: f32) -> Option<u8> {
        let (first, hang, right) = self.indent_marks()?;
        let near = 7.0 * self.s;
        let lower = y > self.ruler_rect.cy();
        let order: [(f32, u8); 3] =
            if lower { [(hang, 1), (right, 3), (first, 0)] } else { [(first, 0), (right, 3), (hang, 1)] };
        let mut best: Option<(f32, u8)> = None;
        for (pos, which) in order {
            let d = (pos - x).abs();
            if d <= near && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, which));
            }
        }
        best.map(|(_, w)| w)
    }

    /// Where the three markers sit on the ruler, in window pixels.
    fn indent_marks(&self) -> Option<(f32, f32, f32)> {
        let page = self.page_rects.first().map(|(_, r)| *r)?;
        let z = self.zoom * self.s;
        let pp = self.ed.current_para_props();
        let (left_m, right_edge) = self.ed.page_margins(0);
        let text_x = page.x + left_m as f32 * z;
        let text_w = ((right_edge - left_m) as f32 * z).max(1.0);
        let twip = |v: Option<i32>| v.unwrap_or(0) as f32 / 20.0 * z;
        Some((text_x + twip(pp.ind_left) + twip(pp.ind_first), text_x + twip(pp.ind_left),
              text_x + text_w - twip(pp.ind_right)))
    }

    /// Dragging a marker sets the paragraph's indent, to the nearest 1/16 inch.
    fn drag_indent_to(&mut self, which: u8, x: f32) {
        let Some(page) = self.page_rects.first().map(|(_, r)| *r) else { return };
        let z = (self.zoom * self.s).max(0.01);
        let (left_m, right_edge) = self.ed.page_margins(0);
        let text_x = page.x + left_m as f32 * z;
        let text_w = ((right_edge - left_m) as f32 * z).max(1.0);
        let step = 72.0 / 16.0; // points
        let snap = |pts: f32| ((pts / step).round() * step).max(0.0);
        let pp = self.ed.current_para_props();
        let left = pp.ind_left.unwrap_or(0) as f32 / 20.0;
        match which {
            0 => {
                let first = snap((x - text_x) / z) - left;
                let v = format!("{}", (first * 20.0).round() as i64);
                self.ed.format_paras_attrs("w:ind", &[("w:firstLine", None), ("w:hanging", None)], "Indent");
                let (name, val) = if first < 0.0 {
                    ("w:hanging", format!("{}", (-first * 20.0).round() as i64))
                } else {
                    ("w:firstLine", v)
                };
                self.ed.format_paras_attrs("w:ind", &[(name, Some(&val))], "Indent");
            }
            1 => {
                let v = format!("{}", (snap((x - text_x) / z) * 20.0).round() as i64);
                self.ed.format_paras_attrs("w:ind", &[("w:left", Some(&v))], "Indent");
            }
            _ => {
                let right = snap((text_x + text_w - x) / z);
                let v = format!("{}", (right * 20.0).round() as i64);
                self.ed.format_paras_attrs("w:ind", &[("w:right", Some(&v))], "Indent");
            }
        }
        self.cache.clear();
    }

    /// Printing: the pages go to the printer as a PDF, which is what the
    /// system's printing takes.
    fn print_now(&mut self, d: &Dialog) {
        let copies = d.value("copies").trim().parse::<u32>().unwrap_or(1).clamp(1, 99);
        let printer = self.printers.get(d.choice_index("printer")).cloned();
        let pages = parse_pages(&d.value("pages"), self.ed.page_count());
        let tmp = folio_kit::platform::temp_file("folio-page-print", "pdf");
        let meta = self.ed.doc_properties();
        if let Err(e) = self.ed.export_pdf_opts(&tmp.to_string_lossy(), pages, false, meta) {
            self.say("Can't print", &e);
            return;
        }
        match folio_kit::platform::print_pdf(&tmp, copies, printer.as_deref()) {
            Ok(()) => self.flash("Sent to the printer"),
            Err(e) => self.say("Can't print", &e),
        }
    }

    /// What a dialog's OK does.
    fn apply_dialog(&mut self, d: &Dialog) {
        let inches = |v: String| (v.trim().parse::<f64>().unwrap_or(0.0) * 1440.0).round() as i64;
        let points = |v: String| (v.trim().parse::<f64>().unwrap_or(0.0) * 20.0).round() as i64;
        match d.id.as_str() {
            "para" => {
                let jc = ["left", "center", "right", "both"][d.choice_index("jc").min(3)];
                self.ed.set_align(jc);
                let (l, r, f) = (inches(d.value("left")), inches(d.value("right")), inches(d.value("first")));
                let (ls, rs) = (l.to_string(), r.to_string());
                self.ed.format_paras_attrs("w:ind", &[("w:left", Some(&ls)), ("w:right", Some(&rs)),
                                                      ("w:firstLine", None), ("w:hanging", None)], "Paragraph");
                if f != 0 {
                    let (name, v) = if f < 0 { ("w:hanging", (-f).to_string()) } else { ("w:firstLine", f.to_string()) };
                    self.ed.format_paras_attrs("w:ind", &[(name, Some(&v))], "Paragraph");
                }
                let before = points(d.value("before")).to_string();
                let after = points(d.value("after")).to_string();
                let line = ((d.value("line").trim().parse::<f64>().unwrap_or(1.0) * 240.0).round() as i64).to_string();
                self.ed.format_paras_attrs(
                    "w:spacing",
                    &[("w:before", Some(&before)), ("w:after", Some(&after)), ("w:line", Some(&line)),
                      ("w:lineRule", Some("auto"))],
                    "Paragraph",
                );
            }
            "font" => {
                let family = d.value("family");
                if !family.trim().is_empty() {
                    self.ed.run_prop("w:rFonts", Some(&[("w:ascii", &family), ("w:hAnsi", &family)]), "Font");
                }
                if let Ok(sz) = d.value("size").trim().parse::<f64>() {
                    let hp = format!("{}", (sz * 2.0).round() as i64);
                    self.ed.run_prop("w:sz", Some(&[("w:val", &hp)]), "Font");
                }
                for (id, name) in [("bold", "w:b"), ("italic", "w:i"), ("underline", "w:u"), ("strike", "w:strike"),
                                   ("caps", "w:caps")] {
                    let on = d.checked(id);
                    let attrs: Vec<(&str, &str)> = if name == "w:u" && on { vec![("w:val", "single")] } else { vec![] };
                    if on {
                        self.ed.run_prop(name, Some(&attrs), "Font");
                    } else {
                        self.ed.run_prop(name, Some(&[("w:val", "0")]), "Font");
                    }
                }
            }
            "pagesetup" => {
                let sec = self.ed.current_section();
                let (pw, ph) = match d.choice_index("paper") {
                    0 => (12240, 15840),
                    1 => (12240, 20160),
                    2 => (11906, 16838),
                    _ => {
                        let (w, h) = self.ed.page_size(self.caret_page());
                        ((w * 20.0).round() as i64, (h * 20.0).round() as i64)
                    }
                };
                let land = d.choice_index("orient") == 1;
                let (w, h) = if land { (pw.max(ph), pw.min(ph)) } else { (pw.min(ph), pw.max(ph)) };
                self.ed.section_attr(sec, "w:pgSz", "w:w", Some(&w.to_string()), "Page Setup");
                self.ed.section_attr(sec, "w:pgSz", "w:h", Some(&h.to_string()), "Page Setup");
                self.ed.section_attr(sec, "w:pgSz", "w:orient",
                                     Some(if land { "landscape" } else { "portrait" }), "Page Setup");
                for (id, attr) in [("left", "w:left"), ("right", "w:right"), ("top", "w:top"), ("bottom", "w:bottom")] {
                    let v = inches(d.value(id)).to_string();
                    self.ed.section_attr(sec, "w:pgMar", attr, Some(&v), "Page Setup");
                }
                self.ed.relayout();
            }
            "props" => {
                self.ed.set_doc_properties([d.value("title"), d.value("author"), d.value("subject"),
                                            d.value("keywords")]);
            }
            "caption" => {
                let label = ["Figure", "Table", "Equation"][d.choice_index("label").min(2)];
                let below = d.choice_index("where") == 0;
                self.ed.insert_caption(label, &d.value("text"), below, "ARABIC", false);
            }
            "xref" => {
                let i = d.choice_index("target");
                let Some(target) = self.ref_targets.get(i).cloned() else { return };
                let page = d.choice_index("kind") == 1;
                let part = if page { "Page" } else { "Text" };
                let marks = self.ed.bookmark_names().len();
                let ok = if i < marks {
                    self.ed.insert_cross_reference("Bookmark", &target, part, true)
                } else {
                    self.ed.insert_cross_reference("Heading", &format!("h{}", i - marks), part, true)
                };
                if !ok {
                    self.flash("That reference couldn't be made");
                }
            }
            "bookmark" => {
                let name = d.value("name");
                if !name.trim().is_empty() && !self.ed.add_bookmark(name.trim()) {
                    self.flash("That name won't do: bookmarks start with a letter");
                }
            }
            "citation" => {
                let (a, t, y) = (d.value("author"), d.value("title"), d.value("year"));
                self.ed.insert_citation(&a, &t, &y);
            }
            _ => {}
        }
        self.cache.clear();
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
            "symbol" => {
                for ch in SYMBOLS {
                    items.push(MenuItem::new(&format!("sym:{ch}"), &ch.to_string()));
                }
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
            "new" | "open" | "saveas" | "pdf" => {
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
            "para-dialog" => {
                let pp = self.ed.current_para_props();
                let jc = match pp.jc.as_deref() {
                    Some("center") => 1,
                    Some("right") | Some("end") => 2,
                    Some("both") => 3,
                    _ => 0,
                };
                let tw = |v: Option<i32>| format!("{:.2}", v.unwrap_or(0) as f64 / 1440.0);
                let pt = |v: Option<i32>| format!("{}", (v.unwrap_or(0) as f64 / 20.0).round() as i64);
                let line = pp.line.map(|l| l as f64 / 240.0).unwrap_or(1.0);
                self.dialog = Some(
                    Dialog::new("para", "Paragraph")
                        .choice("jc", "Alignment", &["Left", "Centre", "Right", "Justified"], jc)
                        .text("left", "Indent left (inches)", &tw(pp.ind_left))
                        .text("right", "Indent right (inches)", &tw(pp.ind_right))
                        .text("first", "First line (inches)", &tw(pp.ind_first))
                        .text("before", "Space before (points)", &pt(pp.before))
                        .text("after", "Space after (points)", &pt(pp.after))
                        .text("line", "Line spacing", &format!("{line:.2}"))
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "font-dialog" => {
                let rp = self.ed.current_run_props();
                let family = rp.font_ascii.clone().unwrap_or_default();
                let size = rp.size.map(|h| h as f64 / 2.0).unwrap_or(11.0);
                self.dialog = Some(
                    Dialog::new("font", "Font")
                        .text("family", "Font", &family)
                        .text("size", "Size (points)", &format!("{size}"))
                        .check("bold", "Bold", rp.bold.unwrap_or(false))
                        .check("italic", "Italic", rp.italic.unwrap_or(false))
                        .check("underline", "Underline", rp.underline.is_some())
                        .check("strike", "Strikethrough", rp.strike.unwrap_or(false))
                        .check("caps", "All caps", rp.caps.unwrap_or(false))
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "page-setup" => {
                let sec = self.ed.current_section();
                let (pw, ph) = self.ed.page_size(self.caret_page());
                let (left, right_edge) = self.ed.page_margins(self.caret_page());
                let paper = if (pw - 612.0).abs() < 2.0 && (ph - 792.0).abs() < 2.0 {
                    0
                } else if (pw - 595.0).abs() < 3.0 {
                    2
                } else if (ph - 1008.0).abs() < 3.0 {
                    1
                } else {
                    3
                };
                let _ = sec;
                self.dialog = Some(
                    Dialog::new("pagesetup", "Page Setup")
                        .choice("paper", "Paper", &["Letter", "Legal", "A4", "As it is"], paper)
                        .choice("orient", "Orientation", &["Portrait", "Landscape"], if pw > ph { 1 } else { 0 })
                        .text("left", "Left margin (inches)", &format!("{:.2}", left / 72.0))
                        .text("right", "Right margin (inches)", &format!("{:.2}", (pw - right_edge) / 72.0))
                        .text("top", "Top margin (inches)", &format!("{:.2}", self.ed.page_vmargins(self.caret_page()).0 / 72.0))
                        .text("bottom", "Bottom margin (inches)", &format!("{:.2}", (self.ed.page_size(self.caret_page()).1 - self.ed.page_vmargins(self.caret_page()).1) / 72.0))
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "properties" => {
                let [title, author, subject, keywords] = self.ed.doc_properties();
                self.dialog = Some(
                    Dialog::new("props", "Document Properties")
                        .text("title", "Title", &title)
                        .text("author", "Author", &author)
                        .text("subject", "Subject", &subject)
                        .text("keywords", "Keywords", &keywords)
                        .note("These go into the document, and into the PDF it exports.")
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "symbol" => {}
            "sym" => {
                if let Some(ch) = arg.and_then(|a| a.chars().next()) {
                    self.ed.type_text(&ch.to_string());
                    self.show_caret();
                }
            }
            "print" => {
                let printers = folio_kit::platform::printers();
                if printers.is_empty() {
                    self.flash("No printer is set up on this computer. Export a PDF instead.");
                    return;
                }
                let names: Vec<&str> = printers.iter().map(|p| p.as_str()).collect();
                self.printers = printers.clone();
                self.dialog = Some(
                    Dialog::new("print", "Print")
                        .note(&format!("{} pages", self.ed.page_count().max(1)))
                        .choice("printer", "Printer", &names, 0)
                        .text("copies", "Copies", "1")
                        .text("pages", "Pages (blank for all)", "")
                        .button("ok", "Print")
                        .button("cancel", "Cancel"),
                );
            }
            "read-mode" => {
                self.read_mode = !self.read_mode;
                self.cache.clear();
            }
            "accessibility" => {
                let issues = self.ed.accessibility_issues();
                if issues.is_empty() {
                    self.flash("Nothing to put right: the document reads well");
                } else {
                    let mut d = Dialog::new("a11y", "Accessibility");
                    for (what, why, _) in issues.iter().take(12) {
                        d = d.note(&format!("{what}: {why}"));
                    }
                    if issues.len() > 12 {
                        d = d.note(&format!("...and {} more", issues.len() - 12));
                    }
                    self.dialog = Some(d.button("ok", "Close"));
                }
            }
            "caption" => {
                self.dialog = Some(
                    Dialog::new("caption", "Caption")
                        .choice("label", "Label", &["Figure", "Table", "Equation"], 0)
                        .text("text", "Caption", "")
                        .choice("where", "Put it", &["Below the selection", "Above the selection"], 0)
                        .button("ok", "OK")
                        .button("cancel", "Cancel"),
                );
            }
            "cross-ref" => {
                let names = self.ed.bookmark_names();
                let heads: Vec<String> = self
                    .ed
                    .outline_paras()
                    .into_iter()
                    .filter(|(_, t, l, ..)| *l <= 9 && !t.trim().is_empty())
                    .map(|(_, t, ..)| t.chars().take(40).collect::<String>())
                    .collect();
                if names.is_empty() && heads.is_empty() {
                    self.flash("Nothing to refer to yet: add a heading or a bookmark first");
                    return;
                }
                let all: Vec<String> = names.iter().cloned().chain(heads.iter().cloned()).collect();
                let opts: Vec<&str> = all.iter().map(|s| s.as_str()).collect();
                self.ref_targets = all.clone();
                self.dialog = Some(
                    Dialog::new("xref", "Cross-reference")
                        .choice("target", "Refer to", &opts, 0)
                        .choice("kind", "Insert", &["The text", "The page number"], 0)
                        .button("ok", "Insert")
                        .button("cancel", "Cancel"),
                );
            }
            "bookmark" => {
                self.dialog = Some(
                    Dialog::new("bookmark", "Bookmark")
                        .text("name", "Name", "")
                        .note("A bookmark is a place you can refer to or jump back to.")
                        .button("ok", "Add")
                        .button("cancel", "Cancel"),
                );
            }
            "bibliography" => {
                if !self.ed.insert_table_field("BIBLIOGRAPHY", None, None, None, None, 1) {
                    self.flash("A bibliography goes in the body of the document");
                }
                self.cache.clear();
            }
            "citation" => {
                self.dialog = Some(
                    Dialog::new("citation", "Citation")
                        .text("author", "Author", "")
                        .text("title", "Title", "")
                        .text("year", "Year", "")
                        .button("ok", "Insert")
                        .button("cancel", "Cancel"),
                );
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
            "nav" => {
                self.nav = !self.nav;
                self.cache.clear();
            }
            "ruler" => {
                self.ruler = !self.ruler;
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
            nav: self.nav,
            ruler: self.ruler,
            read_mode: self.read_mode,
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
    pub nav: bool,
    pub ruler: bool,
    pub read_mode: bool,
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
            "read-mode" => self.read_mode,
            "nav" => self.nav,
            "ruler" => self.ruler,
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

/// "2-5, 8" as page numbers, counting from zero. Empty means every page.
fn parse_pages(spec: &str, count: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (a.trim().parse::<usize>().unwrap_or(1), b.trim().parse::<usize>().unwrap_or(count));
                for p in a.max(1)..=b.min(count) {
                    out.push(p - 1);
                }
            }
            None => {
                if let Ok(n) = part.parse::<usize>() {
                    if n >= 1 && n <= count {
                        out.push(n - 1);
                    }
                }
            }
        }
    }
    out.dedup();
    out
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

    /// The ruler's place, for the tests.
    pub fn ruler_rect_now(&self) -> Rect {
        self.ruler_rect
    }

    /// The headings the pane is showing, for the tests.
    pub fn nav_rows_now(&self) -> Vec<Rect> {
        self.nav_rows.iter().map(|(_, r)| *r).collect()
    }

    /// Whether a marker is being dragged, for the tests.
    pub fn dragging_indent(&self) -> Option<u8> {
        self.drag_indent
    }

    /// Where the indent markers sit, for the tests.
    pub fn indent_marks_now(&self) -> Option<(f32, f32, f32)> {
        self.indent_marks()
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

