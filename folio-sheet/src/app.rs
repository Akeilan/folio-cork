//! The Folio Sheet window on the portable base (S3-19): the ribbon, the grid, the formula bar,
//! the sheet tabs and the status bar. The grid itself is drawn by the engine.
use folio_kit::chrome::{TitleAction, TitleBar, TITLE_H};
use folio_kit::files::{FileAction, FilePane};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::platform::{Button, Cursor, Event, Host, Key, Mods};
use folio_kit::ribbon::{Ribbon, RibbonAction};
use folio_kit::text::{Align, Text, TextStyle};
use folio_kit::theme::Theme;
use folio_kit::ui::{CommandState, Ctx, Dialog, DialogResult, Menu, MenuItem, MenuResult, TextField};
use folio_kit::icons::Icons;
use folio_sheet_engine::book::Book;
use folio_sheet_engine::book_clip::PasteOpts;
use folio_sheet_engine::book_find::FindOpts;
use folio_sheet_engine::book_fmt::{spec, Spec};
use folio_sheet_engine::cellref::Rect as SheetRect;
use folio_sheet_engine::styles::{AlignChange, Change, FontChange};
use folio_sheet_engine::view::ViewParams;
use std::path::PathBuf;
use std::time::Duration;

use crate::settings::Settings;
use crate::{icons, ribbon_defs, uitest::UiTest};

/// Sheet's own green, on the title bar and the highlights.
pub const GREEN: Color = Color::hex(0x217346);
const FORMULA_H: f32 = 30.0;
/// The File pages, down the side of the backstage.
const BACK_PAGES: &[(&str, &str)] = &[
    ("new", "New"),
    ("open", "Open"),
    ("save", "Save"),
    ("saveas", "Save As"),
    ("csv", "Export CSV"),
    ("close", "Back"),
];
const TABS_H: f32 = 26.0;
const BAR_H: f32 = 24.0;

/// What the ribbon asks about to grey buttons out.
pub struct State {
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_paste: bool,
    pub zoom: String,
    pub font: String,
    pub size: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub wrap: bool,
    pub halign: String,
    pub numfmt: String,
    pub grid_on: bool,
    pub headings_on: bool,
    pub formulas_on: bool,
    pub frozen: bool,
}

impl CommandState for State {
    fn enabled(&self, id: &str) -> bool {
        match id {
            "undo" => self.can_undo,
            "redo" => self.can_redo,
            "paste" => self.can_paste,
            _ => true,
        }
    }
    fn checked(&self, id: &str) -> bool {
        match id {
            "bold" => self.bold,
            "italic" => self.italic,
            "underline" => self.underline,
            "wrap" => self.wrap,
            "gridlines" => self.grid_on,
            "headings" => self.headings_on,
            "show-formulas" => self.formulas_on,
            "freeze" => self.frozen,
            "align-left" => self.halign == "left",
            "align-center" => self.halign == "center",
            "align-right" => self.halign == "right",
            _ => false,
        }
    }
    fn text(&self, id: &str) -> Option<String> {
        match id {
            "zoom-menu" => Some(self.zoom.clone()),
            "font" => Some(self.font.clone()),
            "size" => Some(self.size.clone()),
            "numfmt-menu" => Some(self.numfmt.clone()),
            _ => None,
        }
    }
}

/// What the window shows instead of the grid: the File pages.
pub enum Pane {
    None,
    Backstage(Back),
}

pub struct Back {
    pub page: String,
    pub files: Option<FilePane>,
}

/// What was last looked for, so Find Next knows where to go.
#[derive(Clone, Default)]
pub struct Find {
    pub pattern: String,
    pub with: String,
    pub case: bool,
    pub whole: bool,
}

/// A block copied or cut, waiting to be pasted.
pub struct Clip {
    pub sheet: usize,
    pub rect: SheetRect,
    pub cut: bool,
}

pub struct Res {
    pub text: Text,
    pub icons: Icons,
}

/// What the person is typing into, when they are typing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Typing {
    No,
    /// in the cell itself
    Cell,
    /// in the formula bar
    Bar,
}

pub struct SheetApp {
    pub book: Book,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub sheet: usize,
    /// the cell the keyboard is on, and the block selected from it
    pub cur: (u32, u32),
    pub anchor: (u32, u32),
    pub top_row: u32,
    pub left_col: u32,
    pub zoom: f64,
    pub editor: TextField,
    pub typing: Typing,
    pub res: Option<Box<Res>>,
    pub theme: Theme,
    pub ribbon: Ribbon,
    pub title: TitleBar,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub flash: Option<(String, Duration)>,
    pub now: Duration,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub mods: Mods,
    pub mouse: (f32, f32),
    pub drag_select: bool,
    pub clip: Option<Clip>,
    pub find: Find,
    /// true while the Save-before-closing question is up
    pub closing: bool,
    /// View tab: frozen rows and columns, and what is drawn
    pub freeze: (u32, u32),
    pub show_grid: bool,
    pub show_headers: bool,
    pub show_formulas: bool,
    pub pane: Pane,
    pub settings: Settings,
    pub grid: Rect,
    pub tabs: Rect,
    pub tab_hits: Vec<(usize, Rect)>,
    pub exit: bool,
    pub exit_code: i32,
    pub test: Option<UiTest>,
    pm: Option<sk::Pixmap>,
}

impl SheetApp {
    pub fn blank() -> Book {
        for d in crate::template_dirs() {
            let p = d.join("Blank.xlsx");
            if p.is_file() {
                if let Ok(b) = Book::open(&p.to_string_lossy()) {
                    return b;
                }
            }
        }
        Book::from_bytes(include_bytes!("blank.xlsx")).expect("the blank workbook")
    }

    pub fn new(book: Book, path: Option<PathBuf>) -> SheetApp {
        let mut ic = Icons::new();
        icons::install(&mut ic);
        let res = Res { text: Text::bundled(), icons: ic };
        SheetApp {
            book,
            path,
            dirty: false,
            sheet: 0,
            cur: (0, 0),
            anchor: (0, 0),
            top_row: 0,
            left_col: 0,
            zoom: 1.0,
            editor: TextField::new(""),
            typing: Typing::No,
            res: Some(Box::new(res)),
            theme: Theme::new(false, GREEN, GREEN),
            ribbon: Ribbon::new(ribbon_defs::tabs()),
            title: TitleBar::new("sheet", vec![("save", "save", "Save (Ctrl+S)"), ("undo", "undo", "Undo (Ctrl+Z)"), ("redo", "redo", "Redo (Ctrl+Y)")]),
            menu: None,
            dialog: None,
            flash: None,
            now: Duration::ZERO,
            w: 1280.0,
            h: 860.0,
            s: 1.0,
            mods: Mods::NONE,
            mouse: (0.0, 0.0),
            drag_select: false,
            clip: None,
            find: Find::default(),
            closing: false,
            freeze: (0, 0),
            show_grid: true,
            show_headers: true,
            show_formulas: false,
            pane: Pane::None,
            settings: Settings::load(),
            grid: Rect::default(),
            tabs: Rect::default(),
            tab_hits: vec![],
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
        self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Book1.xlsx".into())
    }

    pub fn window_title(&self) -> String {
        format!("{}{} - Folio Sheet", if self.dirty { "*" } else { "" }, self.doc_name())
    }

    /// What the engine needs to draw the grid at its current place.
    pub fn view(&self) -> ViewParams {
        let mut p = ViewParams {
            width: (self.grid.w / self.s) as f64,
            height: (self.grid.h / self.s) as f64,
            zoom: self.zoom,
            top_row: self.top_row,
            left_col: self.left_col,
            ..Default::default()
        };
        p.show_headers = self.show_headers;
        p.show_grid = self.show_grid;
        p.show_formulas = self.show_formulas;
        p.freeze_rows = self.freeze.0;
        p.freeze_cols = self.freeze.1;
        p.dark = self.theme.dark;
        p.active = self.cur;
        p.sel = vec![self.sel_rect()];
        if let Some(c) = self.clip.as_ref() {
            if c.sheet == self.sheet {
                p.copy = Some(c.rect);
            }
        }
        if self.typing == Typing::Cell {
            p.editing = Some(self.cur);
        }
        p
    }

    pub fn sel_rect(&self) -> SheetRect {
        SheetRect {
            r1: self.cur.0.min(self.anchor.0),
            c1: self.cur.1.min(self.anchor.1),
            r2: self.cur.0.max(self.anchor.0),
            c2: self.cur.1.max(self.anchor.1),
        }
    }

    // ---------------- the grid
    /// The cell under a window point, if the point is on the grid.
    pub fn cell_at(&self, x: f32, y: f32) -> Option<(u32, u32)> {
        if !self.grid.contains(x, y) {
            return None;
        }
        let p = self.view();
        let (gx, gy) = (((x - self.grid.x) / self.s) as f64, ((y - self.grid.y) / self.s) as f64);
        let g = self.book.geometry(self.sheet, &p);
        if gx < g.hw || gy < g.hh {
            return None;
        }
        let r = g.rows.iter().find(|(_, y0, h)| gy >= *y0 && gy < *y0 + *h).map(|(r, _, _)| *r);
        let c = g.cols.iter().find(|(_, x0, w)| gx >= *x0 && gx < *x0 + *w).map(|(c, _, _)| *c);
        match (r, c) {
            (Some(r), Some(c)) => Some((r, c)),
            _ => None,
        }
    }

    /// The middle of a cell in window coordinates, for the window test and for menus.
    pub fn cell_point(&self, row: u32, col: u32) -> (f32, f32) {
        let p = self.view();
        let g = self.book.geometry(self.sheet, &p);
        let y = g.rows.iter().find(|(r, _, _)| *r == row).map(|(_, y, h)| y + h / 2.0).unwrap_or(g.hh + 8.0);
        let x = g.cols.iter().find(|(c, _, _)| *c == col).map(|(_, x, w)| x + w / 2.0).unwrap_or(g.hw + 8.0);
        (self.grid.x + x as f32 * self.s, self.grid.y + y as f32 * self.s)
    }

    pub fn go_to(&mut self, r: u32, c: u32, extend: bool) {
        self.end_edit(true);
        self.cur = (r, c);
        if !extend {
            self.anchor = (r, c);
        }
        self.scroll_to_cursor();
    }

    /// Keep the cursor on screen.
    pub fn scroll_to_cursor(&mut self) {
        let p = self.view();
        let g = self.book.geometry(self.sheet, &p);
        let (r, c) = self.cur;
        if r < self.top_row {
            self.top_row = r;
        } else if !g.rows.iter().any(|(rr, y, h)| *rr == r && *y + *h <= p.height) {
            let mut top = self.top_row;
            while top < r {
                top += 1;
                let mut q = p.clone();
                q.top_row = top;
                let gg = self.book.geometry(self.sheet, &q);
                if gg.rows.iter().any(|(rr, y, h)| *rr == r && *y + *h <= q.height) {
                    break;
                }
            }
            self.top_row = top;
        }
        if c < self.left_col {
            self.left_col = c;
        } else if !g.cols.iter().any(|(cc, x, w)| *cc == c && *x + *w <= p.width) {
            let mut left = self.left_col;
            while left < c {
                left += 1;
                let mut q = p.clone();
                q.left_col = left;
                let gg = self.book.geometry(self.sheet, &q);
                if gg.cols.iter().any(|(cc, x, w)| *cc == c && *x + *w <= q.width) {
                    break;
                }
            }
            self.left_col = left;
        }
    }

    // ---------------- typing
    pub fn begin_edit(&mut self, from_cell: bool, start: Option<&str>) {
        let text = match start {
            Some(s) => s.to_string(),
            None => self.book.input(self.sheet, self.cur.0, self.cur.1),
        };
        self.editor = TextField::new(&text);
        self.editor.focused = true;
        self.editor.caret = text.chars().count();
        self.editor.anchor = self.editor.caret;
        self.typing = if from_cell { Typing::Cell } else { Typing::Bar };
    }

    /// Finish typing. `keep` puts what was typed in the cell.
    pub fn end_edit(&mut self, keep: bool) {
        if self.typing == Typing::No {
            return;
        }
        let text = self.editor.text.clone();
        self.typing = Typing::No;
        self.editor.focused = false;
        if !keep {
            return;
        }
        let (r, c) = self.cur;
        if self.book.input(self.sheet, r, c) == text {
            return;
        }
        match self.book.set_input(self.sheet, r, c, &text) {
            Ok(()) => {
                self.book.recalc(false);
                self.dirty = true;
            }
            Err(e) => {
                let e = e.clone();
                self.say("That can't go in the cell", &e);
            }
        }
    }

    pub fn clear_cells(&mut self) {
        let rc = self.sel_rect();
        if self.book.clear_contents(self.sheet, &[rc], "Clear").is_ok() {
            self.book.recalc(false);
            self.dirty = true;
        }
    }

    // ---------------- files
    pub fn save(&mut self) -> bool {
        let Some(p) = self.path.clone() else {
            self.flash("This workbook has no file yet. Save As is on the File tab.");
            return false;
        };
        match self.book.save(&p.to_string_lossy()) {
            Ok(()) => {
                self.dirty = false;
                self.flash(&format!("Saved {}.", self.doc_name()));
                true
            }
            Err(e) => {
                let e = e.clone();
                self.say("Can't save", &e);
                false
            }
        }
    }

    pub fn open_path(&mut self, p: &std::path::Path) {
        match Book::open(&p.to_string_lossy()) {
            Ok(b) => {
                self.book = b;
                self.path = Some(p.to_path_buf());
                self.sheet = 0;
                self.cur = (0, 0);
                self.anchor = (0, 0);
                self.top_row = 0;
                self.left_col = 0;
                self.dirty = false;
            }
            Err(e) => {
                let e = e.clone();
                self.say("Can't open the file", &e);
            }
        }
    }
}

// ---------------------------------------------------------------- drawing
impl SheetApp {
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
        let t = self.theme.clone();
        let s = self.s;
        pm.fill(t.window.to_sk());
        let ribbon_h = self.ribbon.height(s);
        let top = TITLE_H * s + ribbon_h + FORMULA_H * s;
        self.grid = Rect::new(0.0, top, self.w, (self.h - top - TABS_H * s - BAR_H * s).max(1.0));
        self.tabs = Rect::new(0.0, self.grid.bottom(), self.w, TABS_H * s);
        // the grid, drawn by the engine on the portable canvas
        self.paint_grid(pm);
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
        self.paint_formula_bar(pm, r);
        self.paint_tabs(pm, r);
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

    fn paint_grid(&mut self, pm: &mut sk::Pixmap) {
        let (gw, gh) = ((self.grid.w.max(1.0)) as u32, (self.grid.h.max(1.0)) as u32);
        let mut sub = sk::Pixmap::new(gw, gh).unwrap();
        sub.fill(sk::Color::WHITE);
        let p = self.view();
        let painter = folio_sheet_engine::ffi::context_on(&mut sub);
        unsafe {
            let cr = painter.cr();
            if self.s != 1.0 {
                folio_sheet_engine::ffi::cairo_scale(cr, self.s as f64, self.s as f64);
            }
            self.book.render(self.sheet, cr, &p);
        }
        painter.finish(&mut sub);
        pm.draw_pixmap(self.grid.x as i32, self.grid.y as i32, sub.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
    }

    fn paint_backstage(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let theme = self.theme.clone();
        let s = self.s;
        let area = Rect::new(0.0, TITLE_H * s, self.w, self.h - TITLE_H * s);
        gfx::fill_rect(pm, area, theme.panel);
        let side = 190.0 * s;
        gfx::fill_rect(pm, Rect::new(area.x, area.y, side, area.h), GREEN);
        let Pane::Backstage(b) = &mut self.pane else { return };
        let page = b.page.clone();
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            let mut y = area.y + 16.0 * s;
            for (id, label) in BACK_PAGES {
                let rr = Rect::new(area.x, y, side, 34.0 * s);
                if *id == page {
                    gfx::fill_rect(cx.pm, rr, Color::hex(0x17532F));
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
                cx.label("Recent workbooks", Rect::new(body.x, body.y, body.w, 24.0 * s), 13.0, theme.muted, true, Align::Left);
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

    /// Where the file chooser starts, and what it calls a new file.
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
            .unwrap_or_else(|| "Book1".into());
        FilePane::new(&dir, &[ext], save, &format!("{name}.{ext}"))
    }

    /// Go to the next cell holding what was last looked for.
    pub fn find_next(&mut self) {
        if self.find.pattern.is_empty() {
            self.command_find_dialog();
            return;
        }
        let o = FindOpts {
            pattern: self.find.pattern.clone(),
            case: self.find.case,
            whole: self.find.whole,
            formulas: false,
            by_cols: false,
        };
        let hits = self.book.find_all(&[self.sheet], &o);
        if hits.is_empty() {
            self.flash(&format!("{} isn't on this sheet.", self.find.pattern));
            return;
        }
        let (cr, cc) = self.cur;
        let next = hits
            .iter()
            .find(|(_, r, c)| (*r, *c) > (cr, cc))
            .or_else(|| hits.first())
            .copied();
        if let Some((_, r, c)) = next {
            self.go_to(r, c, false);
        }
    }

    fn command_find_dialog(&mut self) {
        self.dialog = Some(
            Dialog::new("find", "Find")
                .text("what", "Find what", &self.find.pattern)
                .check("case", "Match case", self.find.case)
                .check("whole", "Match the whole cell", self.find.whole)
                .button("go", "Find Next")
                .button("cancel", "Cancel"),
        );
    }

    /// Closing: a changed workbook asks to be saved first.
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
                    self.say("Save this workbook first", "This workbook has changes that have not been saved. Save it, or close the window without saving, before starting a new one.");
                    return;
                }
                self.book = SheetApp::blank();
                self.path = None;
                self.sheet = 0;
                self.cur = (0, 0);
                self.anchor = (0, 0);
                self.top_row = 0;
                self.left_col = 0;
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
            "open" => ("open", self.file_pane(false, "xlsx")),
            "csv" => ("csv", self.file_pane(true, "csv")),
            _ => ("saveas", self.file_pane(true, "xlsx")),
        };
        if let Pane::Backstage(b) = &mut self.pane {
            b.page = page.into();
            b.files = Some(f);
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
                "csv" => self.export_csv(&p),
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

    /// Write the sheet in view as comma-separated values.
    pub fn export_csv(&mut self, p: &std::path::Path) {
        let Some((mr, mc)) = self.book.sheets[self.sheet].used_extent() else {
            self.flash("This sheet is empty.");
            return;
        };
        let mut out = String::new();
        for row in 0..=mr {
            for col in 0..=mc {
                if col > 0 {
                    out.push(',');
                }
                let cell = self.book.display(self.sheet, row, col);
                if cell.contains([',', '"', '\n']) {
                    out.push('"');
                    out.push_str(&cell.replace('"', "\"\""));
                    out.push('"');
                } else {
                    out.push_str(&cell);
                }
            }
            out.push('\n');
        }
        match std::fs::write(p, out) {
            Ok(()) => {
                let n = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.flash(&format!("Wrote {n}."));
            }
            Err(e) => {
                let e = e.to_string();
                self.say("Can't write the file", &e);
            }
        }
    }

    fn paint_formula_bar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        let y = TITLE_H * s + self.ribbon.height(s);
        let rect = Rect::new(0.0, y, self.w, FORMULA_H * s);
        gfx::fill_rect(pm, rect, t.panel);
        gfx::hline(pm, 0.0, self.w, rect.bottom() - 1.0, t.border);
        let name = folio_sheet_engine::cellref::a1(self.cur.0, self.cur.1);
        let boxw = 110.0 * s;
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        cx.label(&name, Rect::new(8.0 * s, rect.y, boxw, rect.h), 13.0, t.text, false, Align::Left);
        gfx::vline(cx.pm, boxw + 12.0 * s, rect.y + 4.0 * s, rect.bottom() - 4.0 * s, t.border);
        let fx = boxw + 22.0 * s;
        let text = if self.typing != Typing::No { self.editor.text.clone() } else { self.book.input(self.sheet, self.cur.0, self.cur.1) };
        cx.label(&text, Rect::new(fx, rect.y, self.w - fx - 8.0 * s, rect.h), 13.0, t.text, false, Align::Left);
        if self.typing == Typing::Bar {
            let w = cx.measure(&text, 13.0, false);
            gfx::fill_rect(cx.pm, Rect::new(fx + w + 1.0, rect.y + 6.0 * s, 1.5 * s, rect.h - 12.0 * s), t.text);
        }
    }

    fn paint_tabs(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        gfx::fill_rect(pm, self.tabs, t.tabs);
        gfx::hline(pm, 0.0, self.w, self.tabs.y, t.border);
        self.tab_hits.clear();
        let mut x = 8.0 * s;
        let names: Vec<String> = self.book.sheets.iter().map(|sh| sh.name.clone()).collect();
        for (i, name) in names.iter().enumerate() {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            let w = cx.measure(name, 12.5, i == self.sheet) + 22.0 * s;
            let rect = Rect::new(x, self.tabs.y + 2.0 * s, w, self.tabs.h - 2.0 * s);
            if i == self.sheet {
                gfx::fill_rect(cx.pm, rect, t.window);
                gfx::fill_rect(cx.pm, Rect::new(rect.x, rect.bottom() - 2.0 * s, rect.w, 2.0 * s), t.accent);
            }
            cx.label(name, rect, 12.5, t.text, i == self.sheet, Align::Center);
            self.tab_hits.push((i, rect));
            x += w + 2.0 * s;
        }
    }

    fn paint_bar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        let rect = Rect::new(0.0, self.h - BAR_H * s, self.w, BAR_H * s);
        gfx::fill_rect(pm, rect, t.panel);
        gfx::hline(pm, 0.0, self.w, rect.y, t.border);
        let said = match &self.flash {
            Some((m, until)) if *until > self.now => m.clone(),
            _ => {
                let rc = self.sel_rect();
                if rc.r1 == rc.r2 && rc.c1 == rc.c2 {
                    "Ready".to_string()
                } else {
                    let agg = self.book.aggregates(self.sheet, &[rc]);
                    format!("Average: {}   Count: {}   Sum: {}", agg.0, agg.2, agg.1)
                }
            }
        };
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        cx.label(&said, Rect::new(10.0 * s, rect.y, self.w * 0.7, rect.h), 12.0, t.muted, false, Align::Left);
        cx.label(&format!("{}%", (self.zoom * 100.0).round() as i32), Rect::new(self.w - 70.0 * s, rect.y, 60.0 * s, rect.h), 12.0, t.muted, false, Align::Right);
    }
}

// ---------------------------------------------------------------- events
impl SheetApp {
    pub fn state(&self) -> State {
        let sp = self.spec_here();
        State {
            can_undo: !self.book.undo_stack.is_empty(),
            can_redo: !self.book.redo_stack.is_empty(),
            can_paste: self.clip.is_some(),
            zoom: format!("{}%", (self.zoom * 100.0).round() as i32),
            font: sp.font_name.clone(),
            size: if sp.size.fract() == 0.0 { format!("{}", sp.size as i64) } else { format!("{:.1}", sp.size) },
            bold: sp.bold,
            italic: sp.italic,
            underline: sp.underline.is_some(),
            wrap: sp.wrap,
            halign: sp.halign.clone(),
            numfmt: if sp.numfmt == "General" { "General".into() } else { sp.numfmt.clone() },
            grid_on: self.show_grid,
            headings_on: self.show_headers,
            formulas_on: self.show_formulas,
            frozen: self.freeze != (0, 0),
        }
    }

    /// How the cell under the cursor is formatted.
    pub fn spec_here(&self) -> Spec {
        let xf = self.book.sheets[self.sheet].style_at(self.cur.0, self.cur.1);
        spec(&self.book.styles, xf)
    }

    /// Change the format of everything selected.
    fn restyle(&mut self, ch: Change, label: &str) {
        let rc = self.sel_rect();
        match self.book.apply_change(self.sheet, &[rc], &ch, label) {
            Ok(()) => {
                self.book.recalc(false);
                self.dirty = true;
            }
            Err(e) => self.flash(&e),
        }
    }

    fn font_change(&mut self, f: FontChange, label: &str) {
        self.restyle(Change { font: Some(f), ..Default::default() }, label);
    }

    fn align_change(&mut self, a: AlignChange, label: &str) {
        self.restyle(Change { align: Some(a), ..Default::default() }, label);
    }

    /// What is selected, as tab-separated lines, for the system clipboard.
    pub fn selection_text(&self) -> String {
        let rc = self.sel_rect();
        let mut out = String::new();
        for r in rc.r1..=rc.r2 {
            if r > rc.r1 {
                out.push('\n');
            }
            for c in rc.c1..=rc.c2 {
                if c > rc.c1 {
                    out.push('\t');
                }
                out.push_str(&self.book.display(self.sheet, r, c));
            }
        }
        out
    }

    /// Put tab-separated text in at the cursor, as a paste from another program.
    fn paste_text(&mut self, text: &str) {
        let (r0, c0) = self.cur;
        self.book.begin("Paste", self.sheet);
        for (dr, line) in text.replace("\r\n", "\n").trim_end_matches('\n').split('\n').enumerate() {
            for (dc, cell) in line.split('\t').enumerate() {
                let (r, c) = (r0 + dr as u32, c0 + dc as u32);
                if r > 1_048_575 || c > 16_383 {
                    continue;
                }
                let _ = self.book.set_input(self.sheet, r, c, cell);
            }
        }
        self.book.commit();
        self.book.recalc(false);
        self.dirty = true;
    }

    /// Paste what was copied or cut in this window.
    fn paste_block(&mut self) {
        let Some(cl) = self.clip.take() else { return };
        let (r, c) = self.cur;
        let dst = SheetRect { r1: r, c1: c, r2: r, c2: c };
        match self.book.paste_cells(cl.sheet, cl.rect, self.sheet, dst, PasteOpts::default(), cl.cut) {
            Ok(_) => {
                self.book.recalc(false);
                self.dirty = true;
            }
            Err(e) => self.flash(&e),
        }
        if !cl.cut {
            self.clip = Some(cl);
        }
    }

    /// AutoSum: total the run of numbers above the cursor, or to its left.
    fn autosum(&mut self) {
        let (r, c) = self.cur;
        let mut up = 0u32;
        while up < r {
            let v = self.book.value(self.sheet, r - 1 - up, c);
            if !matches!(v, folio_sheet_engine::value::Value::Num(_)) {
                break;
            }
            up += 1;
        }
        let mut left = 0u32;
        while left < c {
            let v = self.book.value(self.sheet, r, c - 1 - left);
            if !matches!(v, folio_sheet_engine::value::Value::Num(_)) {
                break;
            }
            left += 1;
        }
        let f = if up > 0 {
            let rc = SheetRect { r1: r - up, c1: c, r2: r - 1, c2: c };
            format!("=SUM({})", rc.a1())
        } else if left > 0 {
            let rc = SheetRect { r1: r, c1: c - left, r2: r, c2: c - 1 };
            format!("=SUM({})", rc.a1())
        } else {
            self.flash("Nothing above or to the left to add up.");
            return;
        };
        self.begin_edit(true, Some(&f));
    }

    pub fn handle(&mut self, ev: &Event, host: &mut dyn Host) {
        let mut r = self.res.take().expect("res");
        self.handle_inner(ev, host, &mut r);
        self.res = Some(r);
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
            if !t.is_empty() {
                if self.typing == Typing::No {
                    self.begin_edit(true, Some(""));
                }
                self.editor.event(&Event::Text(t), host, &mut r.text, self.s);
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
            Event::MouseDown { x, y, button: Button::Left, clicks } => {
                if let Some((i, _)) = self.tab_hits.iter().find(|(_, rr)| rr.contains(*x, *y)).cloned() {
                    self.end_edit(true);
                    self.sheet = i;
                    self.cur = (0, 0);
                    self.anchor = (0, 0);
                    return;
                }
                if let Some((row, col)) = self.cell_at(*x, *y) {
                    if *clicks >= 2 {
                        self.go_to(row, col, false);
                        self.begin_edit(true, None);
                    } else {
                        self.end_edit(true);
                        self.go_to(row, col, self.mods.shift);
                        self.drag_select = true;
                    }
                }
            }
            Event::MouseMove { x, y } => {
                if self.drag_select {
                    if let Some((row, col)) = self.cell_at(*x, *y) {
                        self.cur = (row, col);
                    }
                }
                host.set_cursor(if self.grid.contains(*x, *y) { Cursor::Crosshair } else { Cursor::Default });
            }
            Event::CloseRequested => self.ask_to_close(),
            Event::MouseUp { .. } => self.drag_select = false,
            Event::Wheel { dy, dx } => {
                let rows = (-*dy).clamp(-5.0, 5.0) as i64 * 3;
                self.top_row = (self.top_row as i64 + rows).clamp(0, 1_048_575) as u32;
                let cols = (*dx).clamp(-5.0, 5.0) as i64 * 2;
                self.left_col = (self.left_col as i64 + cols).clamp(0, 16_383) as u32;
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
                Key::Char('f') => Some("find"),
                Key::Char('h') => Some("replace"),
                _ => None,
            };
            if let Some(c) = cmd {
                self.command(c, host, r);
                return true;
            }
        }
        if self.typing != Typing::No {
            match key {
                Key::Enter => {
                    self.end_edit(true);
                    let r = (self.cur.0 + 1).min(1_048_575);
                    self.go_to(r, self.cur.1, false);
                    return true;
                }
                Key::Escape => {
                    self.end_edit(false);
                    return true;
                }
                Key::Tab => {
                    self.end_edit(true);
                    let c = (self.cur.1 + 1).min(16_383);
                    self.go_to(self.cur.0, c, false);
                    return true;
                }
                _ => {
                    let ev = Event::Key { key: key.clone(), mods: *mods, down: true, repeat: false };
                    self.editor.event(&ev, host, &mut r.text, self.s);
                    return true;
                }
            }
        }
        let (row, col) = self.cur;
        let shift = mods.shift;
        match key {
            Key::Up => self.go_to(row.saturating_sub(1), col, shift),
            Key::Down => self.go_to((row + 1).min(1_048_575), col, shift),
            Key::Left => self.go_to(row, col.saturating_sub(1), shift),
            Key::Right | Key::Tab => self.go_to(row, (col + 1).min(16_383), shift),
            Key::Home => self.go_to(row, 0, shift),
            Key::PageDown => self.go_to((row + 20).min(1_048_575), col, shift),
            Key::PageUp => self.go_to(row.saturating_sub(20), col, shift),
            Key::Enter => self.go_to((row + 1).min(1_048_575), col, false),
            Key::Delete | Key::Backspace => self.clear_cells(),
            Key::F(2) => self.begin_edit(true, None),
            Key::F(3) => self.find_next(),
            // The platform sends the character along as `Text`, which fills it in.
            Key::Char(_) if !mods.cmd() && !mods.alt => self.begin_edit(true, Some("")),
            _ => return false,
        }
        true
    }

    fn open_menu(&mut self, id: &str, rect: Rect) {
        let items = match id {
            "zoom-menu" => vec![
                MenuItem::new("zoom:0.5", "50%"),
                MenuItem::new("zoom:0.75", "75%"),
                MenuItem::new("zoom:1", "100%"),
                MenuItem::new("zoom:1.5", "150%"),
                MenuItem::new("zoom:2", "200%"),
            ],
            "font" => ["Calibri", "Arial", "Times New Roman", "Courier New", "Verdana", "Georgia", "DejaVu Sans"]
                .iter()
                .map(|n| MenuItem::new(&format!("font:{n}"), n))
                .collect(),
            "size" => [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48]
                .iter()
                .map(|n| MenuItem::new(&format!("size:{n}"), &format!("{n}")))
                .collect(),
            "numfmt-menu" => vec![
                MenuItem::new("numfmt:General", "General"),
                MenuItem::new("numfmt:0", "Whole number"),
                MenuItem::new("numfmt:0.00", "Number, two places"),
                MenuItem::new("numfmt:#,##0.00", "Thousands, two places"),
                MenuItem::new("numfmt:0%", "Percentage"),
                MenuItem::new("numfmt:0.00%", "Percentage, two places"),
                MenuItem::new("numfmt:yyyy-mm-dd", "Date"),
                MenuItem::new("numfmt:hh:mm", "Time"),
            ],
            _ => return,
        };
        self.menu = Some(Menu::new(id, rect, items));
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
        if button != "go" {
            return;
        }
        match d.id.as_str() {
            "find" | "replace" => {
                self.find.pattern = d.value("what");
                self.find.case = d.checked("case");
                self.find.whole = d.checked("whole");
                if self.find.pattern.is_empty() {
                    return;
                }
                if d.id == "replace" {
                    self.find.with = d.value("with");
                    let o = FindOpts {
                        pattern: self.find.pattern.clone(),
                        case: self.find.case,
                        whole: self.find.whole,
                        formulas: false,
                        by_cols: false,
                    };
                    let with = self.find.with.clone();
                    match self.book.replace_all(&[self.sheet], &o, &with) {
                        Ok(0) => self.flash(&format!("{} isn't on this sheet.", self.find.pattern)),
                        Ok(n) => {
                            self.book.recalc(false);
                            self.dirty = true;
                            self.flash(&format!("Replaced {n} cell{}.", if n == 1 { "" } else { "s" }));
                        }
                        Err(e) => self.flash(&e),
                    }
                } else {
                    self.find_next();
                }
            }
            _ => {}
        }
    }

    pub fn command(&mut self, id: &str, host: &mut dyn Host, _r: &mut Res) {
        match id {
            "copy" | "cut" => {
                self.end_edit(true);
                let text = self.selection_text();
                host.set_clipboard_text(&text);
                self.clip = Some(Clip { sheet: self.sheet, rect: self.sel_rect(), cut: id == "cut" });
            }
            "paste" => {
                self.end_edit(true);
                if self.clip.is_some() {
                    self.paste_block();
                } else if let Some(t) = host.clipboard_text() {
                    if !t.is_empty() {
                        self.paste_text(&t);
                    }
                } else {
                    self.flash("Nothing to paste.");
                }
            }
            "bold" => {
                let on = !self.spec_here().bold;
                self.font_change(FontChange { bold: Some(on), ..Default::default() }, "Bold");
            }
            "italic" => {
                let on = !self.spec_here().italic;
                self.font_change(FontChange { italic: Some(on), ..Default::default() }, "Italic");
            }
            "underline" => {
                let on = self.spec_here().underline.is_none();
                let u = if on { Some("single".to_string()) } else { None };
                self.font_change(FontChange { underline: Some(u), ..Default::default() }, "Underline");
            }
            "align-left" | "align-center" | "align-right" => {
                let want = &id["align-".len()..];
                let now = self.spec_here().halign;
                let h = if now == want { "general" } else { want };
                self.align_change(AlignChange { horizontal: Some(h.to_string()), ..Default::default() }, "Align");
            }
            "wrap" => {
                let on = !self.spec_here().wrap;
                self.align_change(AlignChange { wrap: Some(on), ..Default::default() }, "Wrap Text");
            }
            "autosum" => self.autosum(),
            "dec-more" | "dec-less" => {
                let rc = self.sel_rect();
                let d = if id == "dec-more" { 1 } else { -1 };
                let (r, c) = self.cur;
                if let Err(e) = self.book.adjust_decimals(self.sheet, &[rc], r, c, d) {
                    self.flash(&e);
                } else {
                    self.dirty = true;
                }
            }
            f if f.starts_with("font:") => {
                let name = f["font:".len()..].to_string();
                self.font_change(FontChange { name: Some(name), ..Default::default() }, "Font");
            }
            z if z.starts_with("size:") => {
                if let Ok(pt) = z["size:".len()..].parse::<f64>() {
                    self.font_change(FontChange { size: Some(pt), ..Default::default() }, "Font Size");
                }
            }
            n if n.starts_with("numfmt:") => {
                let code = n["numfmt:".len()..].to_string();
                let rc = self.sel_rect();
                if let Err(e) = self.book.set_number_format(self.sheet, &[rc], &code) {
                    self.flash(&e);
                } else {
                    self.dirty = true;
                }
            }
            "row-above" | "row-below" => {
                let rc = self.sel_rect();
                let n = rc.r2 - rc.r1 + 1;
                let at = if id == "row-above" { rc.r1 } else { rc.r2 + 1 };
                match self.book.insert_rows(self.sheet, at, n) {
                    Ok(()) => {
                        self.book.recalc(false);
                        self.dirty = true;
                    }
                    Err(e) => self.flash(&e),
                }
            }
            "row-delete" => {
                let rc = self.sel_rect();
                match self.book.delete_rows(self.sheet, rc.r1, rc.r2 - rc.r1 + 1) {
                    Ok(()) => {
                        self.book.recalc(false);
                        self.dirty = true;
                    }
                    Err(e) => self.flash(&e),
                }
            }
            "col-left" | "col-right" => {
                let rc = self.sel_rect();
                let n = rc.c2 - rc.c1 + 1;
                let at = if id == "col-left" { rc.c1 } else { rc.c2 + 1 };
                match self.book.insert_cols(self.sheet, at, n) {
                    Ok(()) => {
                        self.book.recalc(false);
                        self.dirty = true;
                    }
                    Err(e) => self.flash(&e),
                }
            }
            "col-delete" => {
                let rc = self.sel_rect();
                match self.book.delete_cols(self.sheet, rc.c1, rc.c2 - rc.c1 + 1) {
                    Ok(()) => {
                        self.book.recalc(false);
                        self.dirty = true;
                    }
                    Err(e) => self.flash(&e),
                }
            }
            "find" | "replace" => {
                self.end_edit(true);
                let replace = id == "replace";
                let mut d = Dialog::new(if replace { "replace" } else { "find" },
                                        if replace { "Find and Replace" } else { "Find" })
                    .text("what", "Find what", &self.find.pattern);
                if replace {
                    d = d.text("with", "Replace with", &self.find.with);
                }
                d = d
                    .check("case", "Match case", self.find.case)
                    .check("whole", "Match the whole cell", self.find.whole)
                    .button("go", if replace { "Replace All" } else { "Find Next" })
                    .button("cancel", "Cancel");
                self.dialog = Some(d);
            }
            "find-next" => self.find_next(),
            "freeze" => {
                self.freeze = if self.freeze == (0, 0) { self.cur } else { (0, 0) };
                let (r, c) = self.freeze;
                if r == 0 && c == 0 {
                    self.flash("Panes unfrozen.");
                } else {
                    self.flash(&format!("Frozen above row {} and left of column {}.", r + 1, folio_sheet_engine::cellref::col_name(c)));
                }
            }
            "freeze-top" => self.freeze = (1, 0),
            "freeze-first" => self.freeze = (0, 1),
            "unfreeze" => self.freeze = (0, 0),
            "gridlines" => self.show_grid = !self.show_grid,
            "headings" => self.show_headers = !self.show_headers,
            "show-formulas" => self.show_formulas = !self.show_formulas,
            "save" => {
                self.save();
            }
            "undo" => {
                self.end_edit(false);
                if self.book.undo().is_some() {
                    self.book.recalc(false);
                    self.dirty = true;
                } else {
                    self.flash("Nothing to undo.");
                }
            }
            "redo" => {
                self.end_edit(false);
                if self.book.redo().is_some() {
                    self.book.recalc(false);
                    self.dirty = true;
                }
            }
            "delete" => self.clear_cells(),
            "sheet-new" => {
                let n = self.book.sheets.len();
                if self.book.insert_sheet(n, None).is_ok() {
                    self.sheet = n;
                    self.cur = (0, 0);
                    self.anchor = (0, 0);
                    self.dirty = true;
                }
            }
            "open" => self.open_backstage("open"),
            "saveas" => self.open_backstage("saveas"),
            "new" => self.open_backstage("new"),
            "sheet-delete" => {
                if self.book.sheets.len() < 2 {
                    self.flash("A workbook needs one sheet.");
                } else {
                    let i = self.sheet;
                    match self.book.delete_sheet(i) {
                        Ok(()) => {
                            self.sheet = i.min(self.book.sheets.len() - 1);
                            self.cur = (0, 0);
                            self.anchor = (0, 0);
                            self.dirty = true;
                        }
                        Err(e) => self.flash(&e),
                    }
                }
            }
            z if z.starts_with("zoom:") => {
                self.zoom = z[5..].parse::<f64>().unwrap_or(1.0);
            }
            _ => self.flash(&format!("{id} isn't in this version yet.")),
        }
    }
}

impl folio_kit::platform::App for SheetApp {
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
