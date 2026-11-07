//! The Folio Show window on the portable base (P3-14): the ribbon, the slide pane, the slide
//! itself, the notes strip and the status bar. The slide is drawn by the engine.
use folio_kit::chrome::{TitleAction, TitleBar, TITLE_H};
use folio_kit::files::{FileAction, FilePane};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::platform::{Button, Cursor, Event, Host, Key, Mods};
use folio_kit::ribbon::{Ribbon, RibbonAction};
use folio_kit::text::{Align, Text};
use folio_kit::theme::Theme;
use folio_kit::ui::{CommandState, Ctx, Dialog, DialogResult, Menu, MenuItem, MenuResult};
use folio_canvas::{Cx, Glyphs};
use folio_show_engine::model::Deck;
use folio_show_engine::render::{Opts, Painter, ShapeInfo};
use std::path::PathBuf;
use std::time::Duration;

use crate::settings::Settings;
use crate::{icons, ribbon_defs, uitest::UiTest};

/// Show's own red, on the title bar and the highlights.
pub const RED: Color = Color::hex(0xB7472A);
const PANE_W: f32 = 170.0;
const NOTES_H: f32 = 110.0;
const BAR_H: f32 = 24.0;

/// The File pages, down the side of the backstage.
const BACK_PAGES: &[(&str, &str)] = &[
    ("new", "New"),
    ("open", "Open"),
    ("save", "Save"),
    ("saveas", "Save As"),
    ("png", "Export PNG"),
    ("close", "Back"),
];

/// What the ribbon asks about to grey buttons out and fill the boxes.
pub struct State {
    pub can_undo: bool,
    pub can_redo: bool,
    pub has_shape: bool,
    pub can_paste: bool,
    pub zoom: String,
    pub font: String,
    pub size: String,
    pub pane_on: bool,
    pub notes_on: bool,
}

impl CommandState for State {
    fn enabled(&self, id: &str) -> bool {
        match id {
            "undo" => self.can_undo,
            "redo" => self.can_redo,
            "paste" => self.can_paste,
            "cut" | "copy" | "shape-delete" | "bold" | "italic" | "underline" | "font-color" | "bullets"
            | "align-left" | "align-center" | "align-right" => self.has_shape,
            _ => true,
        }
    }
    fn checked(&self, id: &str) -> bool {
        match id {
            "pane" => self.pane_on,
            "notes" => self.notes_on,
            _ => false,
        }
    }
    fn text(&self, id: &str) -> Option<String> {
        match id {
            "zoom-menu" => Some(self.zoom.clone()),
            "font" => Some(self.font.clone()),
            "size" => Some(self.size.clone()),
            _ => None,
        }
    }
}

pub struct Res {
    pub text: Text,
    pub icons: Icons,
}

/// What the window shows instead of the slide: the File pages.
pub enum Pane {
    None,
    Backstage(Back),
}

pub struct Back {
    pub page: String,
    pub files: Option<FilePane>,
}

/// Where the caret is while a shape's text is being typed into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Caret {
    pub shape: u32,
    pub para: usize,
    pub off: usize,
}

/// What was copied, waiting to be pasted.
pub struct Clip {
    pub xml: String,
}

pub struct ShowApp {
    pub deck: Deck,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    /// the slide being looked at, and the shapes picked out on it
    pub slide: usize,
    pub sel: Vec<u32>,
    pub caret: Option<Caret>,
    pub zoom: f64,
    /// 0 = fit the window
    pub fit: bool,
    pub painter: Painter,
    pub glyphs: Glyphs,
    pub res: Option<Box<Res>>,
    pub settings: Settings,
    pub theme: Theme,
    pub ribbon: Ribbon,
    pub title: TitleBar,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub pane: Pane,
    pub clip: Option<Clip>,
    pub flash: Option<(String, Duration)>,
    pub now: Duration,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub mods: Mods,
    pub mouse: (f32, f32),
    /// dragging a shape: the id, where the pointer went down, and the boxes then
    pub drag: Option<(f32, f32, Vec<(u32, f64, f64, f64, f64)>, u8)>,
    pub pane_on: bool,
    pub notes_on: bool,
    pub presenting: bool,
    pub closing: bool,
    /// where things were drawn last time, for the mouse
    pub canvas: Rect,
    pub slide_rect: Rect,
    pub pane_rect: Rect,
    pub notes_rect: Rect,
    pub thumb_hits: Vec<(usize, Rect)>,
    pub shapes: Vec<ShapeInfo>,
    pub exit: bool,
    pub exit_code: i32,
    pub test: Option<UiTest>,
    pm: Option<sk::Pixmap>,
}

impl ShowApp {
    pub fn blank() -> Deck {
        for d in crate::template_dirs() {
            let p = d.join("Blank.pptx");
            if let Ok(b) = std::fs::read(&p) {
                if let Ok(d) = Deck::open(&b) {
                    return d;
                }
            }
        }
        Deck::open(include_bytes!("blank.pptx")).expect("the blank deck")
    }

    pub fn new(deck: Deck, path: Option<PathBuf>) -> ShowApp {
        let mut ic = Icons::new();
        icons::install(&mut ic);
        let res = Res { text: Text::bundled(), icons: ic };
        let mut deck = deck;
        deck.read_slides();
        ShowApp {
            deck,
            path,
            dirty: false,
            slide: 0,
            sel: Vec::new(),
            caret: None,
            zoom: 1.0,
            fit: true,
            painter: Painter::new(),
            glyphs: Glyphs::new(),
            res: Some(Box::new(res)),
            settings: Settings::load(),
            theme: Theme::new(false, RED, RED),
            ribbon: Ribbon::new(ribbon_defs::tabs()),
            title: TitleBar::new("show", vec![("save", "save", "Save (Ctrl+S)"), ("undo", "undo", "Undo (Ctrl+Z)"), ("redo", "redo", "Redo (Ctrl+Y)")]),
            menu: None,
            dialog: None,
            pane: Pane::None,
            clip: None,
            flash: None,
            now: Duration::ZERO,
            w: 1280.0,
            h: 860.0,
            s: 1.0,
            mods: Mods::NONE,
            mouse: (0.0, 0.0),
            drag: None,
            pane_on: true,
            notes_on: false,
            presenting: false,
            closing: false,
            canvas: Rect::default(),
            slide_rect: Rect::default(),
            pane_rect: Rect::default(),
            notes_rect: Rect::default(),
            thumb_hits: Vec::new(),
            shapes: Vec::new(),
            exit: false,
            exit_code: 0,
            test: None,
            pm: None,
        }
    }

    pub fn say(&mut self, title: &str, text: &str) {
        self.dialog = Some(Dialog::new("say", title).note(text).button("ok", "OK"));
    }

    pub fn flash(&mut self, msg: &str) {
        self.flash = Some((msg.to_string(), self.now + Duration::from_secs(4)));
    }

    pub fn doc_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Presentation.pptx".into())
    }

    pub fn window_title(&self) -> String {
        format!("{}{} - Folio Show", if self.dirty { "*" } else { "" }, self.doc_name())
    }

    pub fn part(&self) -> String {
        self.deck.slides.get(self.slide).cloned().unwrap_or_default()
    }

    pub fn state(&mut self) -> State {
        State {
            can_undo: self.deck.undo_label().is_some(),
            can_redo: self.deck.redo_label().is_some(),
            has_shape: !self.sel.is_empty(),
            can_paste: self.clip.is_some(),
            zoom: if self.fit { "Fit".into() } else { format!("{}%", (self.zoom * 100.0).round() as i32) },
            font: self.shape_font(),
            size: self.shape_size(),
            pane_on: self.pane_on,
            notes_on: self.notes_on,
        }
    }

    /// The font and size the picked-out shape's text starts with, for the ribbon's boxes.
    fn run_here(&mut self) -> Option<folio_show_engine::scene::RunP> {
        let id = *self.sel.first()?;
        let part = self.deck.slides.get(self.slide)?.clone();
        let dom = self.deck.pkg.xml(&part).ok()?.clone();
        let dom = &dom;
        let tree = folio_show_engine::model::path(dom, dom.root, &["p:cSld", "p:spTree"])?;
        let mut all = Vec::new();
        folio_show_engine::scene::collect_shapes(dom, tree, &mut all);
        let n = all.into_iter().find(|n| folio_show_engine::scene::shape_id(dom, *n).0 == id)?;
        let tb = dom.child(n, "p:txBody")?;
        let paras = dom.elements(tb).filter(|c| dom.name(*c) == "a:p").collect::<Vec<_>>();
        let first = paras.first().copied()?;
        let run = dom.elements(first).find(|c| dom.name(*c) == "a:r")?;
        let rpr = dom.child(run, "a:rPr")?;
        let scheme = folio_show_engine::color::Scheme::office();
        Some(folio_show_engine::scene::read_rpr(dom, rpr, &scheme))
    }

    fn shape_font(&mut self) -> String {
        self.run_here().and_then(|p| p.latin).unwrap_or_default()
    }

    fn shape_size(&mut self) -> String {
        match self.run_here().and_then(|p| p.sz) {
            Some(v) => format!("{}", v.round() as i64),
            None => String::new(),
        }
    }
}

impl ShowApp {
    // ---------------------------------------------------------------- painting

    pub fn paint(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        self.w = pm.width() as f32;
        self.h = pm.height() as f32;
        let t = self.theme.clone();
        let s = self.s;
        pm.fill(t.window.to_sk());
        if self.presenting {
            self.paint_show(pm);
            return;
        }
        let ribbon_h = self.ribbon.height(s);
        let top = TITLE_H * s + ribbon_h;
        let body_h = (self.h - top - BAR_H * s).max(1.0);
        self.pane_rect = if self.pane_on { Rect::new(0.0, top, PANE_W * s, body_h) } else { Rect::new(0.0, top, 0.0, body_h) };
        let notes_h = if self.notes_on { NOTES_H * s } else { 0.0 };
        self.canvas = Rect::new(self.pane_rect.right(), top, self.w - self.pane_rect.w, body_h - notes_h);
        self.notes_rect = Rect::new(self.canvas.x, self.canvas.bottom(), self.canvas.w, notes_h);
        self.paint_canvas(pm, r);
        if self.pane_on {
            self.paint_pane(pm, r);
        }
        if self.notes_on {
            self.paint_notes(pm, r);
        }
        let title = self.window_title();
        {
            let st = self.state();
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            let tr = Rect::new(0.0, 0.0, self.w, TITLE_H * s);
            self.title.paint(&mut cx, tr, &title, false, &|id| match id {
                "undo" => st.can_undo,
                "redo" => st.can_redo,
                _ => true,
            });
        }
        let st = self.state();
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            self.ribbon.paint(&mut cx, Rect::new(0.0, TITLE_H * s, self.w, ribbon_h), &st, self.now);
        }
        self.paint_bar(pm, r);
        if matches!(self.pane, Pane::Backstage(_)) {
            self.paint_backstage(pm, r);
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

    /// The slide, as large as it fits, with the picked-out shapes ringed.
    fn paint_canvas(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        gfx::fill_rect(pm, self.canvas, t.panel);
        let part = self.part();
        if part.is_empty() {
            return;
        }
        let (pw, ph) = self.deck.page_size(&part);
        let margin = 16.0 * self.s;
        let k = if self.fit {
            ((self.canvas.w - margin * 2.0) as f64 / pw).min((self.canvas.h - margin * 2.0) as f64 / ph).max(0.05)
        } else {
            self.zoom * self.s as f64
        };
        let (dw, dh) = (pw * k, ph * k);
        let x = self.canvas.x as f64 + ((self.canvas.w as f64 - dw) / 2.0).max(margin as f64);
        let y = self.canvas.y as f64 + ((self.canvas.h as f64 - dh) / 2.0).max(margin as f64);
        self.slide_rect = Rect::new(x as f32, y as f32, dw as f32, dh as f32);
        gfx::fill_rect(pm, self.slide_rect, Color::WHITE);
        let opts = Opts { edit: true, ..Default::default() };
        let shapes = {
            let mut cx = Cx::new(pm, &mut self.glyphs);
            cx.save();
            cx.rectangle(x, y, dw, dh);
            cx.clip();
            cx.translate(x, y);
            cx.scale(k, k);
            let out = self.painter.draw(&mut self.deck, &part, &mut cx, &opts);
            cx.restore();
            out
        };
        self.shapes = shapes;
        // the frame round the slide, then the handles on what is picked out
        gfx::stroke_round(pm, self.slide_rect, 0.0, t.border, 1.0);
        let sel: Vec<ShapeInfo> = self.shapes.iter().filter(|sh| self.sel.contains(&sh.id)).cloned().collect();
        for sh in &sel {
            let rr = Rect::new((x + sh.x * k) as f32, (y + sh.y * k) as f32, (sh.w * k) as f32, (sh.h * k) as f32);
            gfx::stroke_round(pm, rr, 0.0, RED, 1.5);
            for (hx, hy) in [(rr.x, rr.y), (rr.right(), rr.y), (rr.x, rr.bottom()), (rr.right(), rr.bottom()),
                             (rr.cx(), rr.y), (rr.cx(), rr.bottom()), (rr.x, rr.cy()), (rr.right(), rr.cy())] {
                let hs = 4.0 * self.s;
                gfx::fill_rect(pm, Rect::new(hx - hs, hy - hs, hs * 2.0, hs * 2.0), RED);
            }
        }
        if let Some(c) = self.caret {
            self.paint_caret(pm, c, x, y, k);
        }
        let _ = r;
    }

    /// The line where the next letter goes.
    fn paint_caret(&mut self, pm: &mut sk::Pixmap, c: Caret, x: f64, y: f64, k: f64) {
        let Some(sh) = self.shapes.iter().find(|s| s.id == c.shape).cloned() else { return };
        let Some((cx0, top, height)) = self.caret_at(&sh, c) else { return };
        let r = Rect::new((x + (sh.x + cx0) * k) as f32, (y + (sh.y + top) * k) as f32, (1.5 * self.s).max(1.0), (height * k) as f32);
        gfx::fill_rect(pm, r, Color::hex(0x202020));
    }

    /// Where the caret sits in the shape's own box: x, top and height.
    pub fn caret_at(&mut self, sh: &ShapeInfo, c: Caret) -> Option<(f64, f64, f64)> {
        let part = self.part();
        let lay = self.layout_of(sh)?;
        for l in &lay.lines {
            if l.para != c.para {
                continue;
            }
            if c.off >= l.start && c.off <= l.end {
                let i = c.off - l.start;
                let x = *l.xs.get(i).or_else(|| l.xs.last())?;
                return Some((lay.x + x, lay.y + l.top, l.height));
            }
        }
        let _ = part;
        lay.lines.last().map(|l| (lay.x + l.xs.last().copied().unwrap_or(0.0), lay.y + l.top, l.height))
    }

    /// Lay the shape's text out as the engine draws it.
    pub fn layout_of(&mut self, sh: &ShapeInfo) -> Option<folio_show_engine::text::Layout> {
        let part = self.part();
        let ctx = folio_show_engine::scene::context(&mut self.deck, &part);
        let node = self.deck.find_shape(&part, sh.id)?;
        let dom = self.deck.pkg.xml(&part).ok()?.clone();
        let tb = dom.child(node, "p:txBody")?;
        let env = folio_show_engine::text::Env { slide_num: self.slide + 1, date: String::new(), font_scale: 1.0, spc_reduction: 0.0 };
        Some(self.painter.layout_text(&mut self.deck, &ctx, &part, &dom, node, tb, sh.w, sh.h, &env))
    }

    /// The slides down the side, each drawn small.
    fn paint_pane(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        gfx::fill_rect(pm, self.pane_rect, t.panel);
        gfx::fill_rect(pm, Rect::new(self.pane_rect.right() - 1.0, self.pane_rect.y, 1.0, self.pane_rect.h), t.border);
        self.thumb_hits.clear();
        let pad = 8.0 * s;
        let num_w = 20.0 * s;
        let tw = self.pane_rect.w - pad * 2.0 - num_w;
        let slides = self.deck.slides.clone();
        let mut y = self.pane_rect.y + pad;
        for (i, part) in slides.iter().enumerate() {
            let (pw, ph) = self.deck.page_size(part);
            let th = tw as f64 * ph / pw.max(1.0);
            let rr = Rect::new(self.pane_rect.x + pad + num_w, y, tw, th as f32);
            if rr.y > self.pane_rect.bottom() {
                break;
            }
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.label(&format!("{}", i + 1), Rect::new(self.pane_rect.x + 2.0, rr.y, num_w, 16.0 * s), 11.0, t.muted, false, Align::Right);
            }
            gfx::fill_rect(pm, rr, Color::WHITE);
            let k = tw as f64 / pw.max(1.0);
            {
                let mut cx = Cx::new(pm, &mut self.glyphs);
                cx.save();
                cx.rectangle(rr.x as f64, rr.y as f64, rr.w as f64, th);
                cx.clip();
                cx.translate(rr.x as f64, rr.y as f64);
                cx.scale(k, k);
                self.painter.draw(&mut self.deck, part, &mut cx, &Opts::default());
                cx.restore();
            }
            gfx::stroke_round(pm, rr, 0.0, if i == self.slide { RED } else { t.border }, if i == self.slide { 2.0 } else { 1.0 });
            self.thumb_hits.push((i, rr));
            y = rr.bottom() + pad;
        }
    }

    /// The notes under the slide.
    fn paint_notes(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        gfx::fill_rect(pm, self.notes_rect, t.panel);
        gfx::fill_rect(pm, Rect::new(self.notes_rect.x, self.notes_rect.y, self.notes_rect.w, 1.0), t.border);
        let notes = self.notes_text();
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        let inner = Rect::new(self.notes_rect.x + 12.0 * s, self.notes_rect.y + 8.0 * s, self.notes_rect.w - 24.0 * s, self.notes_rect.h - 16.0 * s);
        cx.label("Notes", Rect::new(inner.x, inner.y, inner.w, 16.0 * s), 11.0, t.muted, true, Align::Left);
        let text = if notes.trim().is_empty() { "Click Notes on the View tab to write them here.".to_string() } else { notes };
        cx.label(&text, Rect::new(inner.x, inner.y + 18.0 * s, inner.w, inner.h - 18.0 * s), 12.0, t.text, false, Align::Left);
    }

    pub fn notes_text(&mut self) -> String {
        let part = self.part();
        let Some(np) = self.deck.notes_of(&part) else { return String::new() };
        let Ok(dom) = self.deck.pkg.xml(&np) else { return String::new() };
        let dom = dom.clone();
        let Some(tree) = folio_show_engine::model::path(&dom, dom.root, &["p:cSld", "p:spTree"]) else { return String::new() };
        let mut all = Vec::new();
        folio_show_engine::scene::collect_shapes(&dom, tree, &mut all);
        for n in all {
            let ph = folio_show_engine::scene::ph_of(&dom, n);
            if ph.as_ref().map(|p| p.kind == "body").unwrap_or(false) {
                if let Some(tb) = dom.child(n, "p:txBody") {
                    return folio_show_engine::text::plain_text(&dom, tb).join("\n");
                }
            }
        }
        String::new()
    }

    /// The strip along the bottom: which slide, and what just happened.
    fn paint_bar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        let rect = Rect::new(0.0, self.h - BAR_H * s, self.w, BAR_H * s);
        gfx::fill_rect(pm, rect, t.panel);
        gfx::fill_rect(pm, Rect::new(0.0, rect.y, self.w, 1.0), t.border);
        let left = match self.flash.as_ref() {
            Some((m, until)) if *until > self.now => m.clone(),
            _ => format!("Slide {} of {}", self.slide + 1, self.deck.slides.len().max(1)),
        };
        let right = if self.fit { "Fit".to_string() } else { format!("{}%", (self.zoom * 100.0).round() as i32) };
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        cx.label(&left, Rect::new(10.0 * s, rect.y, self.w * 0.6, rect.h), 12.0, t.muted, false, Align::Left);
        cx.label(&right, Rect::new(self.w - 80.0 * s, rect.y, 70.0 * s, rect.h), 12.0, t.muted, false, Align::Right);
    }

    /// Presenting: the slide alone, as large as the window allows.
    fn paint_show(&mut self, pm: &mut sk::Pixmap) {
        pm.fill(sk::Color::BLACK);
        let part = self.part();
        if part.is_empty() {
            return;
        }
        let (pw, ph) = self.deck.page_size(&part);
        let k = ((self.w as f64) / pw).min((self.h as f64) / ph);
        let (dw, dh) = (pw * k, ph * k);
        let x = (self.w as f64 - dw) / 2.0;
        let y = (self.h as f64 - dh) / 2.0;
        self.slide_rect = Rect::new(x as f32, y as f32, dw as f32, dh as f32);
        gfx::fill_rect(pm, self.slide_rect, Color::WHITE);
        let mut cx = Cx::new(pm, &mut self.glyphs);
        cx.save();
        cx.rectangle(x, y, dw, dh);
        cx.clip();
        cx.translate(x, y);
        cx.scale(k, k);
        self.painter.draw(&mut self.deck, &part, &mut cx, &Opts::default());
        cx.restore();
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

    // ---------------------------------------------------------------- the slides

    pub fn go_to_slide(&mut self, i: usize) {
        if self.deck.slides.is_empty() {
            return;
        }
        self.slide = i.min(self.deck.slides.len() - 1);
        self.sel.clear();
        self.caret = None;
    }

    pub fn shape_at(&self, x: f32, y: f32) -> Option<u32> {
        let (sx, sy) = self.to_slide(x, y)?;
        self.shapes
            .iter()
            .rev()
            .find(|s| sx >= s.x && sx <= s.x + s.w && sy >= s.y && sy <= s.y + s.h)
            .map(|s| s.id)
    }

    /// A window point in the slide's own coordinates (points), if it is on the slide.
    pub fn to_slide(&self, x: f32, y: f32) -> Option<(f64, f64)> {
        if !self.slide_rect.contains(x, y) {
            return None;
        }
        let k = self.slide_rect.w as f64 / self.page_w();
        Some(((x - self.slide_rect.x) as f64 / k, (y - self.slide_rect.y) as f64 / k))
    }

    /// How wide the slide is, in points. The engine's own size, as last drawn.
    pub fn page_w(&self) -> f64 {
        self.deck.cx / 12700.0
    }

    /// Where a shape sits, as the slide was last drawn.
    pub fn box_of(&self, id: u32) -> Option<(f64, f64, f64, f64)> {
        self.shapes.iter().find(|s| s.id == id).map(|s| (s.x, s.y, s.w, s.h))
    }

    pub fn new_slide(&mut self) {
        let layout = {
            let part = self.part();
            self.deck.layout_of(&part).unwrap_or_default()
        };
        match self.deck.new_slide(Some(self.slide), &layout) {
            Ok(i) => {
                self.dirty = true;
                self.go_to_slide(i);
            }
            Err(e) => self.flash(&e),
        }
    }

    pub fn delete_slide(&mut self) {
        if self.deck.slides.len() < 2 {
            self.flash("A presentation needs one slide.");
            return;
        }
        let i = self.slide;
        match self.deck.delete_slides(&[i]) {
            Ok(()) => {
                self.dirty = true;
                self.go_to_slide(i.min(self.deck.slides.len().saturating_sub(1)));
            }
            Err(e) => self.flash(&e),
        }
    }

    // ---------------------------------------------------------------- typing

    pub fn start_typing(&mut self, id: u32) {
        let paras = self.deck.paragraphs(self.slide, id);
        let para = paras.len().saturating_sub(1);
        let off = paras.last().map(|p| p.chars().count()).unwrap_or(0);
        self.sel = vec![id];
        self.caret = Some(Caret { shape: id, para, off });
    }

    pub fn type_text(&mut self, text: &str) {
        let Some(c) = self.caret else { return };
        match self.deck.insert_text(self.slide, c.shape, c.para, c.off, text, "Typing") {
            Ok((para, off)) => {
                self.caret = Some(Caret { shape: c.shape, para, off });
                self.dirty = true;
            }
            Err(e) => self.flash(&e),
        }
    }

    pub fn backspace(&mut self) {
        let Some(c) = self.caret else { return };
        let paras = self.deck.paragraphs(self.slide, c.shape);
        let from = if c.off > 0 {
            (c.para, c.off - 1)
        } else if c.para > 0 {
            (c.para - 1, paras.get(c.para - 1).map(|p| p.chars().count()).unwrap_or(0))
        } else {
            return;
        };
        match self.deck.delete_text(self.slide, c.shape, from, (c.para, c.off), "Delete") {
            Ok((para, off)) => {
                self.caret = Some(Caret { shape: c.shape, para, off });
                self.dirty = true;
            }
            Err(e) => self.flash(&e),
        }
    }

    /// Put the caret where the pointer went down inside the shape's text.
    pub fn caret_to(&mut self, id: u32, x: f64, y: f64) {
        let Some(sh) = self.shapes.iter().find(|s| s.id == id).cloned() else { return };
        let (bx, by) = (x - sh.x, y - sh.y);
        let Some(lay) = self.layout_of(&sh) else {
            self.start_typing(id);
            return;
        };
        let mut best: Option<(f64, usize, usize)> = None;
        for l in &lay.lines {
            let top = lay.y + l.top;
            let dy = if by < top { top - by } else if by > top + l.height { by - top - l.height } else { 0.0 };
            for (i, xx) in l.xs.iter().enumerate() {
                let dx = (lay.x + xx - bx).abs();
                let d = dy * 4.0 + dx;
                if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                    best = Some((d, l.para, l.start + i));
                }
            }
        }
        match best {
            Some((_, para, off)) => {
                self.sel = vec![id];
                self.caret = Some(Caret { shape: id, para, off });
            }
            None => self.start_typing(id),
        }
    }

    // ---------------------------------------------------------------- the file

    pub fn save(&mut self) -> bool {
        let Some(p) = self.path.clone() else {
            self.flash("This presentation has no file yet. Save As is on the File tab.");
            return false;
        };
        match self.deck.save(&p.to_string_lossy()) {
            Ok(()) => {
                self.dirty = false;
                self.flash(&format!("Saved {}.", self.doc_name()));
                true
            }
            Err(e) => {
                self.say("Can't save", &e);
                false
            }
        }
    }

    pub fn open_path(&mut self, p: &std::path::Path) {
        match std::fs::read(p).map_err(|e| e.to_string()).and_then(|b| Deck::open(&b)) {
            Ok(mut d) => {
                d.read_slides();
                self.deck = d;
                self.path = Some(p.to_path_buf());
                self.slide = 0;
                self.sel.clear();
                self.caret = None;
                self.dirty = false;
            }
            Err(e) => self.say("Can't open the file", &e),
        }
    }

    /// Draw this slide into a picture.
    pub fn export_png(&mut self, p: &std::path::Path) {
        let part = self.part();
        let (pw, ph) = self.deck.page_size(&part);
        let k = 2.0;
        let Some(mut pm) = sk::Pixmap::new((pw * k) as u32, (ph * k) as u32) else {
            self.flash("That slide is too large to draw.");
            return;
        };
        pm.fill(sk::Color::WHITE);
        {
            let mut cx = Cx::new(&mut pm, &mut self.glyphs);
            cx.scale(k, k);
            self.painter.draw(&mut self.deck, &part, &mut cx, &Opts::default());
        }
        match std::fs::write(p, gfx::encode_png(&pm)) {
            Ok(()) => {
                let n = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.flash(&format!("Wrote {n}."));
            }
            Err(e) => {
                let e = e.to_string();
                self.say("Can't write the picture", &e);
            }
        }
    }

    // ---------------------------------------------------------------- the File pages

    fn file_pane(&self, save: bool, ext: &str) -> FilePane {
        let dir = self
            .path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(folio_kit::platform::home_dir);
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "Presentation".into());
        FilePane::new(&dir, &[ext], save, &format!("{name}.{ext}"))
    }

    pub fn open_backstage(&mut self, page: &str) {
        self.pane = Pane::Backstage(Back { page: "open".into(), files: None });
        self.backstage_page(page);
    }

    fn backstage_page(&mut self, id: &str) {
        match id {
            "close" => {
                self.pane = Pane::None;
                return;
            }
            "new" => {
                if self.dirty && self.test.is_none() {
                    self.pane = Pane::None;
                    self.say("Save this presentation first", "This presentation has changes that have not been saved. Save it, or close the window without saving, before starting a new one.");
                    return;
                }
                self.deck = ShowApp::blank();
                self.deck.read_slides();
                self.path = None;
                self.slide = 0;
                self.sel.clear();
                self.caret = None;
                self.dirty = false;
                self.pane = Pane::None;
                return;
            }
            "save" if self.path.is_some() => {
                self.save();
                self.pane = Pane::None;
                return;
            }
            _ => {}
        }
        let (page, f) = match id {
            "open" => ("open", self.file_pane(false, "pptx")),
            "png" => ("png", self.file_pane(true, "png")),
            _ => ("saveas", self.file_pane(true, "pptx")),
        };
        if let Pane::Backstage(b) = &mut self.pane {
            b.page = page.into();
            b.files = Some(f);
        }
    }

    fn paint_backstage(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let area = Rect::new(0.0, TITLE_H * s, self.w, self.h - TITLE_H * s);
        gfx::fill_rect(pm, area, theme.panel);
        let side = 190.0 * s;
        gfx::fill_rect(pm, Rect::new(area.x, area.y, side, area.h), RED);
        let Pane::Backstage(b) = &mut self.pane else { return };
        let page = b.page.clone();
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            let mut y = area.y + 16.0 * s;
            for (id, label) in BACK_PAGES {
                let rr = Rect::new(area.x, y, side, 34.0 * s);
                if *id == page {
                    gfx::fill_rect(cx.pm, rr, Color::hex(0x8C3620));
                }
                cx.label(label, Rect::new(rr.x + 20.0 * s, rr.y, rr.w - 24.0 * s, rr.h), 14.0, Color::WHITE, false, Align::Left);
                y += 34.0 * s;
            }
        }
        let body = Rect::new(area.x + side + 24.0 * s, area.y + 20.0 * s, area.w - side - 48.0 * s, area.h - 44.0 * s);
        if page == "open" && !self.settings.recent.is_empty() {
            let recent: Vec<String> = self.settings.recent.clone();
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
                cx.label("Recent presentations", Rect::new(body.x, body.y, body.w, 24.0 * s), 13.0, theme.muted, true, Align::Left);
                let mut y = body.y + 28.0 * s;
                for p in recent.iter().take(8) {
                    let name = std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.clone());
                    cx.label(&name, Rect::new(body.x, y, body.w * 0.4, 22.0 * s), 13.0, theme.text, false, Align::Left);
                    y += 22.0 * s;
                }
            }
            let rest = Rect::new(body.x + body.w * 0.42, body.y, body.w * 0.58, body.h);
            let Pane::Backstage(b) = &mut self.pane else { return };
            if let Some(f) = b.files.as_mut() {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
                f.paint(&mut cx, rest);
            }
            return;
        }
        let Pane::Backstage(b) = &mut self.pane else { return };
        if let Some(f) = b.files.as_mut() {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            f.paint(&mut cx, body);
        }
    }

    fn backstage_event(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        let s = self.s;
        if let Event::Key { key: Key::Escape, down: true, .. } = ev {
            self.pane = Pane::None;
            return;
        }
        if let Event::MouseDown { x, y, button: Button::Left, .. } = ev {
            if *x < 190.0 * s {
                let i = ((*y - TITLE_H * s - 16.0 * s) / (34.0 * s)).floor();
                if i >= 0.0 && (i as usize) < BACK_PAGES.len() {
                    let id = BACK_PAGES[i as usize].0;
                    self.backstage_page(id);
                    host.redraw();
                    return;
                }
            }
        }
        let mut chosen = None;
        if let Pane::Backstage(b) = &mut self.pane {
            if let Some(f) = b.files.as_mut() {
                if let FileAction::Chosen(p) = f.event(ev, host, &mut r.text, s) {
                    chosen = Some((b.page.clone(), p));
                }
            }
        }
        if let Some((page, p)) = chosen {
            match page.as_str() {
                "open" => {
                    self.open_path(&p);
                    self.settings.remember(&p);
                }
                "png" => self.export_png(&p),
                _ => {
                    self.path = Some(p.clone());
                    if self.save() {
                        self.settings.remember(&p);
                    }
                }
            }
            self.pane = Pane::None;
            host.redraw();
        }
    }

    /// Closing: a changed presentation asks to be saved first.
    pub fn ask_to_close(&mut self) {
        if !self.dirty || self.test.is_some() {
            self.exit = true;
            return;
        }
        self.closing = true;
        let name = self.doc_name();
        self.dialog = Some(
            Dialog::new("close", "Save before closing?")
                .note(&format!("{name} has changes that have not been saved."))
                .button("save", "Save")
                .button("discard", "Close without saving")
                .button("cancel", "Keep working"),
        );
    }

    fn dialog_done(&mut self, d: Dialog, button: &str) {
        if d.id == "close" {
            self.closing = false;
            match button {
                "save" => {
                    if self.path.is_some() {
                        if self.save() {
                            self.exit = true;
                        }
                    } else {
                        self.open_backstage("saveas");
                    }
                }
                "discard" => self.exit = true,
                _ => {}
            }
            return;
        }
        if d.id == "notes" && button == "go" {
            let text = d.value("text");
            if let Err(e) = self.deck.set_notes(self.slide, &text) {
                self.flash(&e);
            } else {
                self.dirty = true;
            }
        }
    }

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
        if self.presenting {
            self.show_event(ev, host);
            return;
        }
        if matches!(self.pane, Pane::Backstage(_)) && self.dialog.is_none() {
            self.backstage_event(ev, host, r);
            return;
        }
        if let Some(d) = self.dialog.as_mut() {
            if let DialogResult::Button(b) = d.event(ev, host, &mut r.text, self.s) {
                let d = self.dialog.take().unwrap();
                self.dialog_done(d, &b);
            }
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    self.menu = None;
                    self.command(&id, host, r);
                    return;
                }
                MenuResult::Closed => self.menu = None,
                MenuResult::None => return,
            }
        }
        let (ta, took) = self.title.event(ev);
        if let Some(a) = ta {
            match a {
                TitleAction::Command(id) => self.command(&id, host, r),
                TitleAction::Close => self.ask_to_close(),
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
        if let Event::Key { key, mods, down: true, .. } = ev {
            if self.keys(key, mods, host, r) {
                return;
            }
        }
        if let Event::Text(t) = ev {
            let t: String = t.chars().filter(|c| !c.is_control()).collect();
            if !t.is_empty() && self.caret.is_some() {
                self.type_text(&t);
            }
            return;
        }
        let st = self.state();
        let (ra, took) = self.ribbon.event(ev, &st, self.now);
        if let Some(a) = ra {
            match a {
                RibbonAction::Command(id) => self.command(&id, host, r),
                RibbonAction::Menu(id, rect) => self.open_menu(&id, rect),
                RibbonAction::Backstage => self.open_backstage("open"),
                _ => {}
            }
            return;
        }
        if took {
            return;
        }
        match ev {
            Event::CloseRequested => self.ask_to_close(),
            Event::MouseDown { x, y, button: Button::Left, clicks } => {
                if let Some((i, _)) = self.thumb_hits.iter().find(|(_, rr)| rr.contains(*x, *y)).cloned() {
                    self.go_to_slide(i);
                    return;
                }
                if self.slide_rect.contains(*x, *y) {
                    let hit = self.shape_at(*x, *y);
                    match hit {
                        Some(id) => {
                            if *clicks >= 2 {
                                if let Some((sx, sy)) = self.to_slide(*x, *y) {
                                    self.caret_to(id, sx, sy);
                                }
                            } else if self.caret.map(|c| c.shape) == Some(id) {
                                if let Some((sx, sy)) = self.to_slide(*x, *y) {
                                    self.caret_to(id, sx, sy);
                                }
                            } else {
                                self.caret = None;
                                if self.mods.shift && !self.sel.contains(&id) {
                                    self.sel.push(id);
                                } else {
                                    self.sel = vec![id];
                                }
                                let boxes: Vec<(u32, f64, f64, f64, f64)> = self
                                    .sel
                                    .iter()
                                    .filter_map(|i| self.box_of(*i).map(|b| (*i, b.0, b.1, b.2, b.3)))
                                    .collect();
                                self.drag = Some((*x, *y, boxes, 0));
                            }
                        }
                        None => {
                            self.sel.clear();
                            self.caret = None;
                        }
                    }
                }
            }
            Event::MouseMove { x, y } => {
                if let Some((x0, y0, boxes, _)) = self.drag.clone() {
                    let k = self.slide_rect.w as f64 / self.page_w();
                    let (dx, dy) = (((*x - x0) as f64) / k, ((*y - y0) as f64) / k);
                    if dx.abs() > 0.5 || dy.abs() > 0.5 {
                        let moved: Vec<(u32, f64, f64, f64, f64, Option<f64>)> =
                            boxes.iter().map(|(id, bx, by, bw, bh)| (*id, bx + dx, by + dy, *bw, *bh, None)).collect();
                        if self.deck.set_boxes(self.slide, &moved, "Move").is_ok() {
                            self.dirty = true;
                        }
                    }
                }
                let on_shape = self.shape_at(*x, *y).is_some();
                host.set_cursor(if on_shape { Cursor::Move } else { Cursor::Default });
            }
            Event::MouseUp { .. } => self.drag = None,
            Event::Wheel { dy, .. } => {
                if *dy < 0.0 {
                    self.go_to_slide((self.slide + 1).min(self.deck.slides.len().saturating_sub(1)));
                } else if *dy > 0.0 && self.slide > 0 {
                    self.go_to_slide(self.slide - 1);
                }
            }
            _ => {}
        }
    }

    /// While presenting: the keys move through the slides.
    fn show_event(&mut self, ev: &Event, host: &mut dyn Host) {
        match ev {
            Event::Key { key, down: true, .. } => match key {
                Key::Escape => {
                    self.presenting = false;
                    host.redraw();
                }
                Key::Right | Key::Down | Key::PageDown | Key::Space | Key::Enter => {
                    if self.slide + 1 < self.deck.slides.len() {
                        self.slide += 1;
                    } else {
                        self.presenting = false;
                    }
                    host.redraw();
                }
                Key::Left | Key::Up | Key::PageUp => {
                    self.slide = self.slide.saturating_sub(1);
                    host.redraw();
                }
                Key::Home => {
                    self.slide = 0;
                    host.redraw();
                }
                _ => {}
            },
            Event::MouseDown { button: Button::Left, .. } => {
                if self.slide + 1 < self.deck.slides.len() {
                    self.slide += 1;
                } else {
                    self.presenting = false;
                }
                host.redraw();
            }
            _ => {}
        }
    }

    /// Returns true when the key was used.
    fn keys(&mut self, key: &Key, mods: &Mods, host: &mut dyn Host, r: &mut Res) -> bool {
        if mods.cmd() {
            let cmd = match key {
                Key::Char('s') => Some("save"),
                Key::Char('o') => Some("open"),
                Key::Char('n') => Some("new"),
                Key::Char('z') => Some("undo"),
                Key::Char('y') => Some("redo"),
                Key::Char('c') => Some("copy"),
                Key::Char('x') => Some("cut"),
                Key::Char('v') => Some("paste"),
                Key::Char('b') => Some("bold"),
                Key::Char('i') => Some("italic"),
                Key::Char('u') => Some("underline"),
                Key::Char('m') => Some("slide-new"),
                Key::Char('d') => Some("slide-duplicate"),
                _ => None,
            };
            if let Some(c) = cmd {
                self.command(c, host, r);
                return true;
            }
        }
        if self.caret.is_some() {
            match key {
                Key::Escape => {
                    self.caret = None;
                    return true;
                }
                Key::Backspace => {
                    self.backspace();
                    return true;
                }
                Key::Enter => {
                    self.type_text("\n");
                    return true;
                }
                Key::Left | Key::Right | Key::Up | Key::Down => {
                    self.move_caret(key);
                    return true;
                }
                _ => {}
            }
        }
        match key {
            Key::F(5) => self.command("present", host, r),
            Key::F(2) => {
                if let Some(id) = self.sel.first().copied() {
                    self.start_typing(id);
                }
            }
            Key::Escape => {
                self.sel.clear();
                self.caret = None;
            }
            Key::Delete | Key::Backspace if !self.sel.is_empty() => self.command("shape-delete", host, r),
            Key::PageDown | Key::Down => self.go_to_slide((self.slide + 1).min(self.deck.slides.len().saturating_sub(1))),
            Key::PageUp | Key::Up => self.go_to_slide(self.slide.saturating_sub(1)),
            Key::Home => self.go_to_slide(0),
            Key::End => self.go_to_slide(self.deck.slides.len().saturating_sub(1)),
            _ => return false,
        }
        true
    }

    fn move_caret(&mut self, key: &Key) {
        let Some(c) = self.caret else { return };
        let paras = self.deck.paragraphs(self.slide, c.shape);
        let len = |i: usize| paras.get(i).map(|p| p.chars().count()).unwrap_or(0);
        let next = match key {
            Key::Left => {
                if c.off > 0 {
                    (c.para, c.off - 1)
                } else if c.para > 0 {
                    (c.para - 1, len(c.para - 1))
                } else {
                    (c.para, c.off)
                }
            }
            Key::Right => {
                if c.off < len(c.para) {
                    (c.para, c.off + 1)
                } else if c.para + 1 < paras.len() {
                    (c.para + 1, 0)
                } else {
                    (c.para, c.off)
                }
            }
            Key::Up => (c.para.saturating_sub(1), c.off.min(len(c.para.saturating_sub(1)))),
            _ => {
                let p = (c.para + 1).min(paras.len().saturating_sub(1));
                (p, c.off.min(len(p)))
            }
        };
        self.caret = Some(Caret { shape: c.shape, para: next.0, off: next.1 });
    }

    fn open_menu(&mut self, id: &str, rect: Rect) {
        let items = match id {
            "zoom-menu" => vec![
                MenuItem::new("zoom:fit", "Fit the window"),
                MenuItem::new("zoom:0.5", "50%"),
                MenuItem::new("zoom:0.75", "75%"),
                MenuItem::new("zoom:1", "100%"),
                MenuItem::new("zoom:1.5", "150%"),
            ],
            "shape-menu" => ["rect", "roundRect", "ellipse", "triangle", "diamond", "rightArrow", "star5", "flowChartProcess"]
                .iter()
                .map(|p| MenuItem::new(&format!("shape:{p}"), &folio_show_engine::edit::nice_shape_name(p)))
                .collect(),
            "layout-menu" => {
                let part = self.part();
                let master = self.deck.master_of(&part).unwrap_or_default();
                self.deck
                    .layouts_of(&master)
                    .into_iter()
                    .map(|l| {
                        let name = self.deck.layout_name(&l);
                        MenuItem::new(&format!("layout:{l}"), &name)
                    })
                    .collect()
            }
            _ => return,
        };
        self.menu = Some(Menu::new(id, rect, items));
    }

    // ---------------------------------------------------------------- commands

    pub fn command(&mut self, id: &str, host: &mut dyn Host, _r: &mut Res) {
        match id {
            "save" => {
                self.save();
            }
            "open" => self.open_backstage("open"),
            "saveas" => self.open_backstage("saveas"),
            "new" => self.open_backstage("new"),
            "undo" => match self.deck.undo() {
                Some(_) => {
                    self.dirty = true;
                    self.sel.clear();
                    self.caret = None;
                    self.slide = self.slide.min(self.deck.slides.len().saturating_sub(1));
                }
                None => self.flash("Nothing to undo."),
            },
            "redo" => {
                if self.deck.redo().is_some() {
                    self.dirty = true;
                    self.sel.clear();
                    self.caret = None;
                    self.slide = self.slide.min(self.deck.slides.len().saturating_sub(1));
                }
            }
            "slide-new" => self.new_slide(),
            "slide-delete" => self.delete_slide(),
            "slide-duplicate" => match self.deck.duplicate_slides(&[self.slide]) {
                Ok(i) => {
                    self.dirty = true;
                    self.go_to_slide(i);
                }
                Err(e) => self.flash(&e),
            },
            "textbox" => {
                let (w, h) = (300.0, 60.0);
                match self.deck.add_textbox(self.slide, 100.0, 100.0, w, h, "Text") {
                    Ok(id) => {
                        self.dirty = true;
                        self.sel = vec![id];
                        self.flash("Text box added. Double-click it to type.");
                    }
                    Err(e) => self.flash(&e),
                }
            }
            s if s.starts_with("shape:") => {
                let prst = s["shape:".len()..].to_string();
                match self.deck.add_shape(self.slide, &prst, 140.0, 140.0, 220.0, 150.0) {
                    Ok(id) => {
                        self.dirty = true;
                        self.sel = vec![id];
                    }
                    Err(e) => self.flash(&e),
                }
            }
            l if l.starts_with("layout:") => {
                let layout = l["layout:".len()..].to_string();
                match self.deck.set_layout(&[self.slide], &layout) {
                    Ok(()) => self.dirty = true,
                    Err(e) => self.flash(&e),
                }
            }
            "shape-delete" => {
                if self.sel.is_empty() {
                    self.flash("Nothing is picked out.");
                } else {
                    let ids = self.sel.clone();
                    match self.deck.delete_shapes(self.slide, &ids) {
                        Ok(()) => {
                            self.dirty = true;
                            self.sel.clear();
                            self.caret = None;
                        }
                        Err(e) => self.flash(&e),
                    }
                }
            }
            "copy" | "cut" => {
                if self.sel.is_empty() {
                    return;
                }
                let ids = self.sel.clone();
                match self.deck.shapes_xml(self.slide, &ids) {
                    Ok(xml) => {
                        self.clip = Some(Clip { xml });
                        if id == "cut" {
                            if self.deck.delete_shapes(self.slide, &ids).is_ok() {
                                self.dirty = true;
                                self.sel.clear();
                            }
                        }
                    }
                    Err(e) => self.flash(&e),
                }
            }
            "paste" => {
                let Some(c) = self.clip.as_ref().map(|c| c.xml.clone()) else {
                    self.flash("Nothing to paste.");
                    return;
                };
                match self.deck.paste_shapes(self.slide, &c, 12.0, 12.0) {
                    Ok(ids) => {
                        self.dirty = true;
                        self.sel = ids;
                    }
                    Err(e) => self.flash(&e),
                }
            }
            "bold" | "italic" | "underline" => {
                let key = match id {
                    "bold" => "b",
                    "italic" => "i",
                    _ => "u",
                };
                self.format_selection(key);
            }
            "align-left" | "align-center" | "align-right" => {
                let want = match id {
                    "align-left" => "l",
                    "align-center" => "ctr",
                    _ => "r",
                };
                self.format_paras("algn", want);
            }
            "bullets" => self.format_paras("bullet", "toggle"),
            "present" => {
                self.presenting = true;
                self.sel.clear();
                self.caret = None;
                host.redraw();
            }
            "pane" => self.pane_on = !self.pane_on,
            "notes" => {
                self.notes_on = !self.notes_on;
                if self.notes_on {
                    let text = self.notes_text();
                    self.dialog = Some(
                        Dialog::new("notes", "Notes for this slide")
                            .text("text", "Notes", &text)
                            .button("go", "Keep")
                            .button("cancel", "Cancel"),
                    );
                }
            }
            "find" => self.flash("Find is on the way in a later version."),
            "picture" | "table" => self.flash(&format!("{id} isn't in this version yet.")),
            z if z.starts_with("zoom:") => {
                let v = &z["zoom:".len()..];
                if v == "fit" {
                    self.fit = true;
                } else {
                    self.fit = false;
                    self.zoom = v.parse::<f64>().unwrap_or(1.0);
                }
            }
            _ => self.flash(&format!("{id} isn't in this version yet.")),
        }
    }

    /// Bold, italic or underline over the shape's whole text.
    fn format_selection(&mut self, key: &str) {
        let Some(&id) = self.sel.first() else { return };
        let paras = self.deck.paragraphs(self.slide, id);
        if paras.is_empty() {
            return;
        }
        let end = (paras.len() - 1, paras.last().map(|p| p.chars().count()).unwrap_or(0));
        let now = self.deck.run_props_at(self.slide, id, 0, 0);
        let on = match (key, now) {
            ("b", Some(p)) => p.b.unwrap_or(false),
            ("i", Some(p)) => p.i.unwrap_or(false),
            ("u", Some(p)) => p.u.as_deref().map(|u| u != "none").unwrap_or(false),
            _ => false,
        };
        let value = if key == "u" {
            if on { "none".to_string() } else { "sng".to_string() }
        } else if on {
            "0".to_string()
        } else {
            "1".to_string()
        };
        match self.deck.format_runs(self.slide, id, (0, 0), end, key, &value) {
            Ok(()) => self.dirty = true,
            Err(e) => self.flash(&e),
        }
    }

    fn format_paras(&mut self, key: &str, value: &str) {
        let Some(&id) = self.sel.first() else { return };
        let paras = self.deck.paragraphs(self.slide, id);
        if paras.is_empty() {
            return;
        }
        match self.deck.format_paras(self.slide, id, 0, paras.len() - 1, key, value) {
            Ok(()) => self.dirty = true,
            Err(e) => self.flash(&e),
        }
    }
}

impl folio_kit::platform::App for ShowApp {
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
