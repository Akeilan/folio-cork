//! A file browser for Open, Save As and Export, drawn by the toolkit so it looks and works the
//! same on every system.

use crate::gfx::{self, Rect};
use crate::text::Align;
use crate::ui::{Ctx, FieldResult, TextField};
use folio_platform::{Button, Event, Host, Key};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

pub enum FileAction {
    None,
    Chosen(PathBuf),
}

pub struct FilePane {
    pub dir: PathBuf,
    /// Extensions shown (lower case, no dot). Empty shows everything.
    pub exts: Vec<String>,
    pub save: bool,
    pub name: TextField,
    pub button: String,
    entries: Vec<Entry>,
    sel: Option<usize>,
    scroll: f32,
    rows: Vec<(usize, Rect)>,
    places: Vec<(String, PathBuf, Rect)>,
    up: Rect,
    go: Rect,
    list: Rect,
    pub error: String,
}

impl FilePane {
    pub fn new(dir: &Path, exts: &[&str], save: bool, name: &str) -> FilePane {
        let mut f = FilePane {
            dir: dir.to_path_buf(),
            exts: exts.iter().map(|s| s.to_string()).collect(),
            save,
            name: TextField::new(name),
            button: if save { "Save".into() } else { "Open".into() },
            entries: vec![],
            sel: None,
            scroll: 0.0,
            rows: vec![],
            places: vec![],
            up: Rect::default(),
            go: Rect::default(),
            list: Rect::default(),
            error: String::new(),
        };
        if save {
            f.name.focused = true;
            let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().len()).unwrap_or(name.len());
            f.name.anchor = 0;
            f.name.caret = stem;
        }
        f.refresh();
        f
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        self.sel = None;
        self.scroll = 0.0;
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let md = e.metadata().ok();
                let dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                if !dir && !self.exts.is_empty() {
                    let ext = Path::new(&name).extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                    if !self.exts.contains(&ext) {
                        continue;
                    }
                }
                self.entries.push(Entry {
                    name,
                    path: e.path(),
                    dir,
                    size: md.as_ref().map(|m| m.len()).unwrap_or(0),
                    modified: md.and_then(|m| m.modified().ok()),
                });
            }
        }
        self.entries.sort_by(|a, b| b.dir.cmp(&a.dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn enter(&mut self, dir: &Path) {
        self.dir = dir.to_path_buf();
        self.refresh();
    }

    fn places_list() -> Vec<(String, PathBuf)> {
        let home = folio_platform::home_dir();
        let mut v = vec![("Home".to_string(), home.clone()), ("Documents".to_string(), folio_platform::documents_dir())];
        for (n, d) in [("Desktop", "Desktop"), ("Pictures", "Pictures"), ("Downloads", "Downloads")] {
            let p = home.join(d);
            if p.is_dir() {
                v.push((n.into(), p));
            }
        }
        if cfg!(unix) {
            v.push(("Computer".into(), PathBuf::from("/")));
        }
        v
    }

    pub fn paint(&mut self, cx: &mut Ctx, r: Rect) {
        let s = cx.s;
        let t = cx.theme.clone();
        let pw = 150.0 * s;
        self.places.clear();
        let mut y = r.y;
        for (n, p) in FilePane::places_list() {
            let pr = Rect::new(r.x, y, pw - 10.0 * s, 30.0 * s);
            if p == self.dir {
                gfx::fill_round(cx.pm, pr, 4.0 * s, t.checked);
            }
            cx.icon("folder", Rect::new(pr.x + 8.0 * s, pr.y + 7.0 * s, 16.0 * s, 16.0 * s));
            cx.label(&n, Rect::new(pr.x + 32.0 * s, pr.y, pr.w - 34.0 * s, pr.h), 13.0, t.text, false, Align::Left);
            self.places.push((n, p, pr));
            y += 32.0 * s;
        }
        let x0 = r.x + pw;
        let w = r.w - pw;
        // Path bar with an Up button.
        self.up = Rect::new(x0, r.y, 30.0 * s, 30.0 * s);
        gfx::fill_round(cx.pm, self.up, 4.0 * s, t.field);
        gfx::stroke_round(cx.pm, self.up, 4.0 * s, t.border, 1.0);
        cx.icon("arrow-up", self.up.inset(7.0 * s));
        let pr = Rect::new(x0 + 36.0 * s, r.y, w - 36.0 * s, 30.0 * s);
        gfx::fill_round(cx.pm, pr, 4.0 * s, t.field);
        gfx::stroke_round(cx.pm, pr, 4.0 * s, t.border, 1.0);
        cx.label(&self.dir.to_string_lossy(), pr.inset(8.0 * s), 13.0, t.text, false, Align::Left);
        let bottom_h = if self.save { 46.0 * s } else { 46.0 * s };
        self.list = Rect::new(x0, r.y + 38.0 * s, w, r.h - 38.0 * s - bottom_h);
        let l = self.list;
        gfx::fill_round(cx.pm, l, 4.0 * s, t.field);
        gfx::stroke_round(cx.pm, l, 4.0 * s, t.border, 1.0);
        let rh = 28.0 * s;
        let head = Rect::new(l.x, l.y, l.w, 26.0 * s);
        cx.label("Name", Rect::new(head.x + 34.0 * s, head.y, 200.0 * s, head.h), 12.0, t.muted, false, Align::Left);
        cx.label("Modified", Rect::new(l.right() - 230.0 * s, head.y, 140.0 * s, head.h), 12.0, t.muted, false, Align::Left);
        cx.label("Size", Rect::new(l.right() - 90.0 * s, head.y, 80.0 * s, head.h), 12.0, t.muted, false, Align::Right);
        gfx::hline(cx.pm, l.x, l.right(), head.bottom(), t.border);
        self.rows.clear();
        let max_scroll = (self.entries.len() as f32 * rh - (l.h - 30.0 * s)).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max_scroll);
        let mut y = head.bottom() + 2.0 * s - self.scroll;
        let entries = self.entries.clone();
        for (i, e) in entries.iter().enumerate() {
            let rr = Rect::new(l.x + 2.0 * s, y, l.w - 4.0 * s, rh);
            y += rh;
            if rr.y < head.bottom() || rr.bottom() > l.bottom() {
                continue;
            }
            if self.sel == Some(i) {
                gfx::fill_round(cx.pm, rr, 3.0 * s, t.selection);
            }
            cx.icon(if e.dir { "folder" } else { "file" }, Rect::new(rr.x + 8.0 * s, rr.y + 6.0 * s, 16.0 * s, 16.0 * s));
            cx.label(&e.name, Rect::new(rr.x + 32.0 * s, rr.y, rr.w - 270.0 * s, rr.h), 13.0, t.text, false, Align::Left);
            if let Some(m) = e.modified {
                cx.label(&format_time(m), Rect::new(l.right() - 230.0 * s, rr.y, 140.0 * s, rr.h), 12.0, t.muted, false, Align::Left);
            }
            if !e.dir {
                cx.label(&format_size(e.size), Rect::new(l.right() - 90.0 * s, rr.y, 80.0 * s, rr.h), 12.0, t.muted, false, Align::Right);
            }
            self.rows.push((i, rr));
        }
        if self.entries.is_empty() {
            cx.label("No matching files here", Rect::new(l.x, l.y + 40.0 * s, l.w, 24.0 * s), 13.0, t.muted, false, Align::Center);
        }
        let by = l.bottom() + 10.0 * s;
        let bw = (cx.measure(&self.button, 13.0, true) + 36.0 * s).max(90.0 * s);
        self.go = Rect::new(r.right() - bw, by, bw, 32.0 * s);
        if self.save {
            cx.label("File name", Rect::new(x0, by, 80.0 * s, 32.0 * s), 13.0, t.text, false, Align::Left);
            let nr = Rect::new(x0 + 84.0 * s, by, self.go.x - x0 - 96.0 * s, 32.0 * s);
            self.name.paint(cx, nr);
        } else if !self.error.is_empty() {
            let e = self.error.clone();
            cx.label(&e, Rect::new(x0, by, self.go.x - x0 - 10.0 * s, 32.0 * s), 13.0, t.danger, false, Align::Left);
        }
        if self.save && !self.error.is_empty() {
            let e = self.error.clone();
            cx.label(&e, Rect::new(x0, by + 34.0 * s, w, 20.0 * s), 12.0, t.danger, false, Align::Left);
        }
        gfx::fill_round(cx.pm, self.go, 4.0 * s, t.accent);
        let b = self.button.clone();
        cx.label(&b, self.go, 13.0, t.accent.contrast_text(), true, Align::Center);
    }

    fn choose_save(&mut self) -> FileAction {
        let name = self.name.text.trim().to_string();
        if name.is_empty() {
            self.error = "Type a file name.".into();
            return FileAction::None;
        }
        let mut p = if Path::new(&name).is_absolute() { PathBuf::from(&name) } else { self.dir.join(&name) };
        if p.is_dir() {
            self.enter(&p.clone());
            return FileAction::None;
        }
        if let Some(e) = self.exts.first() {
            let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !self.exts.contains(&ext) {
                p.set_extension(e);
            }
        }
        FileAction::Chosen(p)
    }

    pub fn event(&mut self, ev: &Event, host: &mut dyn Host, text: &mut crate::text::Text, s: f32) -> FileAction {
        if self.save {
            match self.name.event(ev, host, text, s) {
                FieldResult::Enter => return self.choose_save(),
                FieldResult::Changed => {
                    self.error.clear();
                    return FileAction::None;
                }
                _ => {}
            }
        }
        match ev {
            Event::Wheel { dy, .. } => {
                self.scroll -= dy * 3.0 * 28.0 * s;
                FileAction::None
            }
            Event::MouseDown { x, y, button: Button::Left, clicks } => {
                let (x, y) = (*x, *y);
                if let Some((_, p, _)) = self.places.iter().find(|(_, _, r)| r.contains(x, y)).cloned() {
                    self.enter(&p);
                    return FileAction::None;
                }
                if self.up.contains(x, y) {
                    if let Some(p) = self.dir.parent().map(|p| p.to_path_buf()) {
                        self.enter(&p);
                    }
                    return FileAction::None;
                }
                if self.go.contains(x, y) {
                    if self.save {
                        return self.choose_save();
                    }
                    if let Some(i) = self.sel {
                        let e = self.entries[i].clone();
                        if e.dir {
                            self.enter(&e.path);
                            return FileAction::None;
                        }
                        return FileAction::Chosen(e.path);
                    }
                    self.error = "Choose a file.".into();
                    return FileAction::None;
                }
                if let Some((i, _)) = self.rows.iter().find(|(_, r)| r.contains(x, y)).cloned() {
                    self.sel = Some(i);
                    let e = self.entries[i].clone();
                    if self.save && !e.dir {
                        self.name.set(&e.name);
                    }
                    if *clicks >= 2 {
                        if e.dir {
                            self.enter(&e.path);
                        } else {
                            return if self.save { self.choose_save() } else { FileAction::Chosen(e.path) };
                        }
                    }
                }
                FileAction::None
            }
            Event::Key { key: Key::Backspace, down: true, mods, .. } if mods.alt => {
                if let Some(p) = self.dir.parent().map(|p| p.to_path_buf()) {
                    self.enter(&p);
                }
                FileAction::None
            }
            _ => FileAction::None,
        }
    }
}

pub fn format_size(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.0} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / 1024.0 / 1024.0)
    }
}

/// Local-agnostic "YYYY-MM-DD HH:MM" (UTC) without a date library.
pub fn format_time(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", rem / 3600, rem % 3600 / 60)
}

fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
