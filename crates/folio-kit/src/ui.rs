//! Shared drawing context and small controls: text fields, menus, dialogs, tips.

use crate::gfx::{self, sk, Color, Rect};
use crate::icons::Icons;
use crate::text::{Align, Text, TextStyle};
use crate::theme::Theme;
use folio_platform::{Button, Event, Host, Key, Mods};

/// Everything a control needs to draw itself.
pub struct Ctx<'a> {
    pub pm: &'a mut sk::Pixmap,
    pub text: &'a mut Text,
    pub icons: &'a mut Icons,
    pub theme: &'a Theme,
    /// Interface scale (1.0 at 96 dpi).
    pub s: f32,
}

impl<'a> Ctx<'a> {
    pub fn style(&self, size: f32) -> TextStyle {
        TextStyle { size: size * self.s, color: self.theme.text, ..Default::default() }
    }
    /// Draw one line of interface text, vertically centred in `r`, clipped by ellipsis.
    pub fn label(&mut self, s: &str, r: Rect, size: f32, color: Color, bold: bool, align: Align) {
        let mut st = self.style(size);
        st.color = color;
        st.bold = bold;
        let mut txt = s.to_string();
        let (mut w, h) = self.text.measure(&txt, &st);
        while w > r.w && txt.chars().count() > 1 {
            txt.pop();
            while txt.ends_with(' ') {
                txt.pop();
            }
            let t2 = format!("{txt}…");
            w = self.text.measure(&t2, &st).0;
            if w <= r.w {
                txt = t2;
                break;
            }
        }
        let x = match align {
            Align::Left => r.x,
            Align::Center => r.x + (r.w - w) / 2.0,
            Align::Right => r.right() - w,
        };
        let y = r.y + (r.h - h) / 2.0;
        self.text.draw(self.pm, &txt, &st, x, y, None);
    }
    pub fn measure(&mut self, s: &str, size: f32, bold: bool) -> f32 {
        let mut st = self.style(size);
        st.bold = bold;
        self.text.measure(s, &st).0
    }
    pub fn icon(&mut self, name: &str, r: Rect) {
        let c = self.theme.text;
        let a = self.theme.accent;
        self.icons.draw(self.pm, name, r, c, a);
    }
    pub fn icon_colored(&mut self, name: &str, r: Rect, c: Color) {
        let a = self.theme.accent;
        self.icons.draw(self.pm, name, r, c, a);
    }
}

/// Which commands are available and checked; apps implement this for the ribbon and menus.
pub trait CommandState {
    fn enabled(&self, _id: &str) -> bool {
        true
    }
    fn checked(&self, _id: &str) -> bool {
        false
    }
    /// Live text for combo boxes (font name, zoom level...).
    fn text(&self, _id: &str) -> Option<String> {
        None
    }
    /// A colour shown under an icon (fill colour, line colour...).
    fn swatch(&self, _id: &str) -> Option<Color> {
        None
    }
}

/// A single-line text box.
#[derive(Clone, Debug, Default)]
pub struct TextField {
    pub text: String,
    pub caret: usize,
    pub anchor: usize,
    pub focused: bool,
    pub rect: Rect,
    scroll: f32,
}

pub enum FieldResult {
    None,
    Changed,
    Enter,
    Escape,
}

impl TextField {
    pub fn new(text: &str) -> TextField {
        TextField { text: text.to_string(), caret: text.len(), anchor: 0, focused: false, rect: Rect::default(), scroll: 0.0 }
    }
    pub fn set(&mut self, text: &str) {
        self.text = text.to_string();
        self.caret = self.text.len();
        self.anchor = 0;
    }
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
    }
    fn sel(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }
    fn delete_sel(&mut self) -> bool {
        let (a, b) = self.sel();
        if a == b {
            return false;
        }
        self.text.replace_range(a..b, "");
        self.caret = a;
        self.anchor = a;
        true
    }
    fn prev(&self, i: usize) -> usize {
        self.text[..i].char_indices().last().map(|(j, _)| j).unwrap_or(0)
    }
    fn next(&self, i: usize) -> usize {
        self.text[i..].char_indices().nth(1).map(|(j, _)| i + j).unwrap_or(self.text.len())
    }

    pub fn paint(&mut self, cx: &mut Ctx, r: Rect) {
        self.rect = r;
        let t = cx.theme;
        gfx::fill_round(cx.pm, r, 3.0 * cx.s, t.field);
        gfx::stroke_round(cx.pm, r, 3.0 * cx.s, if self.focused { t.accent } else { t.border }, if self.focused { 2.0 } else { 1.0 });
        let st = cx.style(13.0);
        let pad = 6.0 * cx.s;
        let lay = cx.text.layout(&self.text, &st, None);
        let (cxp, _, _) = lay.caret(self.caret);
        let vis = r.w - 2.0 * pad;
        if cxp - self.scroll > vis {
            self.scroll = cxp - vis;
        }
        if cxp < self.scroll {
            self.scroll = cxp;
        }
        let ty = r.y + (r.h - lay.height) / 2.0;
        // Clip by drawing into a sub-pixmap.
        let (w, h) = (vis.max(1.0) as u32, r.h.max(1.0) as u32);
        if let Some(mut sub) = sk::Pixmap::new(w, h) {
            if self.focused {
                let (a, b) = self.sel();
                if a != b {
                    let (xa, _, _) = lay.caret(a);
                    let (xb, _, _) = lay.caret(b);
                    gfx::fill_rect(&mut sub, Rect::new(xa - self.scroll, ty - r.y, xb - xa, lay.height), t.selection);
                }
            }
            cx.text.draw(&mut sub, &self.text, &st, -self.scroll, ty - r.y, None);
            if self.focused {
                gfx::fill_rect(&mut sub, Rect::new((cxp - self.scroll).round(), ty - r.y, (1.0 * cx.s).max(1.0), lay.height), t.text);
            }
            cx.pm.draw_pixmap((r.x + pad) as i32, r.y as i32, sub.as_ref(), &sk::PixmapPaint::default(), sk::Transform::identity(), None);
        }
    }

    /// Handle an event; returns what happened. Mouse focus is decided by the owner.
    pub fn event(&mut self, ev: &Event, host: &mut dyn Host, text: &mut Text, s: f32) -> FieldResult {
        match ev {
            Event::MouseDown { x, y, button: Button::Left, clicks } if self.rect.contains(*x, *y) => {
                self.focused = true;
                let st = TextStyle { size: 13.0 * s, ..Default::default() };
                let lay = text.layout(&self.text, &st, None);
                let i = lay.hit(x - self.rect.x - 6.0 * s + self.scroll, 0.0);
                self.caret = i;
                self.anchor = i;
                if *clicks >= 2 {
                    self.select_all();
                }
                FieldResult::None
            }
            Event::Text(t) if self.focused => {
                self.delete_sel();
                self.text.insert_str(self.caret, t);
                self.caret += t.len();
                self.anchor = self.caret;
                FieldResult::Changed
            }
            Event::Key { key, mods, down: true, .. } if self.focused => self.key(key, mods, host),
            _ => FieldResult::None,
        }
    }

    fn key(&mut self, key: &Key, mods: &Mods, host: &mut dyn Host) -> FieldResult {
        let shift = mods.shift;
        let mv = |f: &mut TextField, to: usize| {
            f.caret = to;
            if !shift {
                f.anchor = to;
            }
        };
        match key {
            Key::Enter => return FieldResult::Enter,
            Key::Escape => return FieldResult::Escape,
            Key::Left => {
                let (a, b) = self.sel();
                let to = if a != b && !shift { a } else { self.prev(self.caret) };
                mv(self, to)
            }
            Key::Right => {
                let (a, b) = self.sel();
                let to = if a != b && !shift { b } else { self.next(self.caret) };
                mv(self, to)
            }
            Key::Home => mv(self, 0),
            Key::End => mv(self, self.text.len()),
            Key::Backspace => {
                if !self.delete_sel() && self.caret > 0 {
                    let p = self.prev(self.caret);
                    self.text.replace_range(p..self.caret, "");
                    self.caret = p;
                    self.anchor = p;
                }
                return FieldResult::Changed;
            }
            Key::Delete => {
                if !self.delete_sel() && self.caret < self.text.len() {
                    let n = self.next(self.caret);
                    self.text.replace_range(self.caret..n, "");
                }
                return FieldResult::Changed;
            }
            Key::Char('a') if mods.cmd() => self.select_all(),
            Key::Char('c') if mods.cmd() => {
                let (a, b) = self.sel();
                if a != b {
                    host.set_clipboard_text(&self.text[a..b]);
                }
            }
            Key::Char('x') if mods.cmd() => {
                let (a, b) = self.sel();
                if a != b {
                    host.set_clipboard_text(&self.text[a..b].to_string());
                    self.delete_sel();
                    return FieldResult::Changed;
                }
            }
            Key::Char('v') if mods.cmd() => {
                if let Some(t) = host.clipboard_text() {
                    let t: String = t.chars().filter(|c| !c.is_control()).collect();
                    self.delete_sel();
                    self.text.insert_str(self.caret, &t);
                    self.caret += t.len();
                    self.anchor = self.caret;
                    return FieldResult::Changed;
                }
            }
            _ => {}
        }
        FieldResult::None
    }
}

/// One entry in a menu.
#[derive(Clone, Debug, Default)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub shortcut: String,
    pub icon: Option<String>,
    pub checked: bool,
    pub disabled: bool,
    pub swatch: Option<Color>,
    pub separator: bool,
    pub submenu: Vec<MenuItem>,
    /// Draw the label in this font family (font pickers).
    pub font: Option<String>,
}

impl MenuItem {
    pub fn new(id: &str, label: &str) -> MenuItem {
        MenuItem { id: id.into(), label: label.into(), ..Default::default() }
    }
    pub fn sep() -> MenuItem {
        MenuItem { separator: true, ..Default::default() }
    }
    pub fn key(mut self, k: &str) -> Self {
        self.shortcut = k.into();
        self
    }
    pub fn icon(mut self, i: &str) -> Self {
        self.icon = Some(i.into());
        self
    }
    pub fn check(mut self, c: bool) -> Self {
        self.checked = c;
        self
    }
    pub fn off(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn swatch(mut self, c: Color) -> Self {
        self.swatch = Some(c);
        self
    }
    pub fn sub(mut self, items: Vec<MenuItem>) -> Self {
        self.submenu = items;
        self
    }
}

/// A popup menu, or a grid of colours.
pub struct Menu {
    pub id: String,
    pub items: Vec<MenuItem>,
    /// When set, the menu shows a colour grid of this many columns before the items.
    pub palette: Vec<Color>,
    pub palette_cols: usize,
    pub anchor: Rect,
    pub rect: Rect,
    hover: Option<usize>,
    hover_color: Option<usize>,
    child: Option<Box<Menu>>,
    rows: Vec<Rect>,
    cells: Vec<Rect>,
    scroll: f32,
    max_h: f32,
}

pub enum MenuResult {
    None,
    Picked(String),
    Closed,
}

impl Menu {
    pub fn new(id: &str, anchor: Rect, items: Vec<MenuItem>) -> Menu {
        Menu {
            id: id.into(),
            items,
            palette: vec![],
            palette_cols: 10,
            anchor,
            rect: Rect::default(),
            hover: None,
            hover_color: None,
            child: None,
            rows: vec![],
            cells: vec![],
            scroll: 0.0,
            max_h: 0.0,
        }
    }

    pub fn with_palette(mut self, colors: Vec<Color>, cols: usize) -> Menu {
        self.palette = colors;
        self.palette_cols = cols;
        self
    }

    fn layout(&mut self, cx: &mut Ctx, win: Rect) {
        let s = cx.s;
        let row_h = 28.0 * s;
        let mut w: f32 = 160.0 * s;
        for it in &self.items {
            if it.separator {
                continue;
            }
            let lw = cx.measure(&it.label, 13.0, false) + cx.measure(&it.shortcut, 12.0, false) + 90.0 * s;
            w = w.max(lw);
        }
        let cell = 22.0 * s;
        let pal_rows = if self.palette.is_empty() { 0 } else { self.palette.len().div_ceil(self.palette_cols) };
        if !self.palette.is_empty() {
            w = w.max(self.palette_cols as f32 * (cell + 4.0 * s) + 16.0 * s);
        }
        let pal_h = if pal_rows > 0 { pal_rows as f32 * (cell + 4.0 * s) + 12.0 * s } else { 0.0 };
        let items_h: f32 = self.items.iter().map(|i| if i.separator { 9.0 * s } else { row_h }).sum();
        let h = pal_h + items_h + 8.0 * s;
        self.max_h = (win.h - 16.0 * s).min(h);
        let mut x = self.anchor.x;
        let mut y = self.anchor.bottom();
        if x + w > win.right() - 4.0 {
            x = (win.right() - w - 4.0).max(win.x);
        }
        if y + self.max_h > win.bottom() - 4.0 {
            let up = self.anchor.y - self.max_h;
            y = if up >= win.y { up } else { (win.bottom() - self.max_h - 4.0).max(win.y) };
        }
        self.rect = Rect::new(x, y, w, self.max_h);
        self.cells.clear();
        let mut cy = y + 6.0 * s - self.scroll;
        if pal_rows > 0 {
            for (i, _) in self.palette.iter().enumerate() {
                let c = i % self.palette_cols;
                let r = i / self.palette_cols;
                self.cells.push(Rect::new(x + 8.0 * s + c as f32 * (cell + 4.0 * s), cy + r as f32 * (cell + 4.0 * s), cell, cell));
            }
            cy += pal_h;
        }
        self.rows.clear();
        for it in &self.items {
            let h = if it.separator { 9.0 * s } else { row_h };
            self.rows.push(Rect::new(x + 4.0 * s, cy, w - 8.0 * s, h));
            cy += h;
        }
    }

    pub fn paint(&mut self, cx: &mut Ctx, win: Rect) {
        self.layout(cx, win);
        let t = cx.theme.clone();
        let s = cx.s;
        gfx::shadow(cx.pm, self.rect, 6.0 * s, t.dark);
        gfx::fill_round(cx.pm, self.rect, 6.0 * s, t.popup);
        gfx::stroke_round(cx.pm, self.rect, 6.0 * s, t.border, 1.0);
        let clip = self.rect;
        for (i, c) in self.palette.clone().iter().enumerate() {
            let r = self.cells[i];
            if r.y < clip.y || r.bottom() > clip.bottom() {
                continue;
            }
            if *c == Color::TRANSPARENT {
                gfx::fill_rect(cx.pm, r, Color::WHITE);
                gfx::line(cx.pm, r.x, r.bottom(), r.right(), r.y, Color::hex(0xD32F2F), 1.5 * s);
            } else {
                gfx::fill_rect(cx.pm, r, *c);
            }
            let hov = self.hover_color == Some(i);
            gfx::stroke_round(cx.pm, r, 0.0, if hov { t.accent } else { t.border }, if hov { 2.0 } else { 1.0 });
        }
        let items = self.items.clone();
        for (i, it) in items.iter().enumerate() {
            let r = self.rows[i];
            if r.y < clip.y - 1.0 || r.bottom() > clip.bottom() + 1.0 {
                continue;
            }
            if it.separator {
                gfx::hline(cx.pm, r.x + 8.0 * s, r.right() - 8.0 * s, r.cy(), t.border);
                continue;
            }
            if self.hover == Some(i) && !it.disabled {
                gfx::fill_round(cx.pm, r, 4.0 * s, t.hover);
            }
            let fg = if it.disabled { t.muted } else { t.text };
            let ir = Rect::new(r.x + 8.0 * s, r.y + (r.h - 16.0 * s) / 2.0, 16.0 * s, 16.0 * s);
            if it.checked {
                gfx::fill_round(cx.pm, ir.inset(-3.0 * s), 3.0 * s, t.checked);
                if it.icon.is_none() {
                    cx.icon_colored("check", ir, fg);
                }
            }
            if let Some(ic) = &it.icon {
                cx.icon_colored(ic, ir, fg);
            }
            if let Some(c) = it.swatch {
                let sr = Rect::new(r.x + 8.0 * s, r.y + 6.0 * s, 16.0 * s, r.h - 12.0 * s);
                gfx::fill_rect(cx.pm, sr, c);
                gfx::stroke_round(cx.pm, sr, 0.0, t.border, 1.0);
            }
            let lr = Rect::new(r.x + 34.0 * s, r.y, r.w - 60.0 * s, r.h);
            if let Some(f) = &it.font {
                let mut st = cx.style(13.0);
                st.family = f.clone();
                st.color = fg;
                let h = cx.text.measure(&it.label, &st).1;
                cx.text.draw(cx.pm, &it.label, &st, lr.x, lr.y + (lr.h - h) / 2.0, None);
            } else {
                cx.label(&it.label, lr, 13.0, fg, false, Align::Left);
            }
            if !it.shortcut.is_empty() {
                cx.label(&it.shortcut, Rect::new(r.x, r.y, r.w - 10.0 * s, r.h), 12.0, t.muted, false, Align::Right);
            }
            if !it.submenu.is_empty() {
                cx.icon_colored("chevron-right", Rect::new(r.right() - 20.0 * s, r.y + (r.h - 12.0 * s) / 2.0, 12.0 * s, 12.0 * s), fg);
            }
        }
        if let Some(ch) = self.child.as_mut() {
            ch.paint(cx, win);
        }
    }

    /// Feed an event. Returns Picked when an item or colour is chosen, Closed when dismissed.
    pub fn event(&mut self, ev: &Event) -> MenuResult {
        if let Some(ch) = self.child.as_mut() {
            match ev {
                Event::MouseMove { x, y } | Event::MouseDown { x, y, .. } | Event::MouseUp { x, y, .. } if ch.contains(*x, *y) => {
                    return match ch.event(ev) {
                        MenuResult::Closed => MenuResult::None,
                        r => r,
                    };
                }
                Event::Key { .. } => {
                    return match ch.event(ev) {
                        MenuResult::Closed => {
                            self.child = None;
                            MenuResult::None
                        }
                        r => r,
                    }
                }
                _ => {}
            }
        }
        match ev {
            Event::MouseMove { x, y } => {
                self.hover = self.rows.iter().position(|r| r.contains(*x, *y)).filter(|&i| !self.items[i].separator);
                self.hover_color = self.cells.iter().position(|r| r.contains(*x, *y));
                if let Some(i) = self.hover {
                    let it = &self.items[i];
                    if !it.submenu.is_empty() && !it.disabled {
                        let open = self.child.as_ref().map(|c| c.id == it.id).unwrap_or(false);
                        if !open {
                            let r = self.rows[i];
                            let anchor = Rect::new(r.right(), r.y - r.h - 4.0, 1.0, r.h);
                            self.child = Some(Box::new(Menu::new(&it.id, anchor, it.submenu.clone())));
                        }
                    } else {
                        self.child = None;
                    }
                }
                MenuResult::None
            }
            Event::Wheel { dy, .. } => {
                let full: f32 = self.rows.last().map(|r| r.bottom() + self.scroll - self.rect.y).unwrap_or(0.0);
                self.scroll = (self.scroll - dy * 40.0).clamp(0.0, (full - self.max_h + 8.0).max(0.0));
                MenuResult::None
            }
            Event::MouseDown { x, y, .. } => {
                if !self.contains(*x, *y) {
                    return MenuResult::Closed;
                }
                MenuResult::None
            }
            Event::MouseUp { x, y, button: Button::Left } => {
                if let Some(i) = self.cells.iter().position(|r| r.contains(*x, *y)) {
                    return MenuResult::Picked(format!("color:{}", self.palette[i].to_hex()));
                }
                if let Some(i) = self.rows.iter().position(|r| r.contains(*x, *y)) {
                    let it = &self.items[i];
                    if !it.separator && !it.disabled && it.submenu.is_empty() {
                        return MenuResult::Picked(it.id.clone());
                    }
                }
                MenuResult::None
            }
            Event::Key { key, down: true, .. } => match key {
                Key::Escape => MenuResult::Closed,
                Key::Down | Key::Up => {
                    let n = self.items.len();
                    if n == 0 {
                        return MenuResult::None;
                    }
                    let mut i = self.hover.unwrap_or(if *key == Key::Down { n - 1 } else { 0 });
                    for _ in 0..n {
                        i = if *key == Key::Down { (i + 1) % n } else { (i + n - 1) % n };
                        if !self.items[i].separator && !self.items[i].disabled {
                            break;
                        }
                    }
                    self.hover = Some(i);
                    MenuResult::None
                }
                Key::Right => {
                    if let Some(i) = self.hover {
                        let it = &self.items[i];
                        if !it.submenu.is_empty() {
                            let r = self.rows[i];
                            let mut m = Menu::new(&it.id, Rect::new(r.right(), r.y - r.h, 1.0, r.h), it.submenu.clone());
                            m.hover = Some(0);
                            self.child = Some(Box::new(m));
                        }
                    }
                    MenuResult::None
                }
                Key::Left => MenuResult::Closed,
                Key::Enter | Key::Space => match self.hover {
                    Some(i) if !self.items[i].disabled && self.items[i].submenu.is_empty() => MenuResult::Picked(self.items[i].id.clone()),
                    _ => MenuResult::None,
                },
                _ => MenuResult::None,
            },
            _ => MenuResult::None,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.rect.contains(x, y) || self.child.as_ref().map(|c| c.contains(x, y)).unwrap_or(false)
    }
}

/// Standard document colours: a row of greys, a row of brights, then tints.
pub fn standard_palette() -> Vec<Color> {
    let base = [0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47];
    let mut v: Vec<Color> = base.iter().map(|&c| Color::hex(c)).collect();
    for t in [0.8, 0.6, 0.4] {
        for &c in &base {
            v.push(Color::hex(c).mix(Color::WHITE, t as f32));
        }
    }
    for t in [0.25, 0.5] {
        for &c in &base {
            v.push(Color::hex(c).mix(Color::BLACK, t as f32));
        }
    }
    for c in [0xC00000, 0xFF0000, 0xFFC000, 0xFFFF00, 0x92D050, 0x00B050, 0x00B0F0, 0x0070C0, 0x002060, 0x7030A0] {
        v.push(Color::hex(c));
    }
    v
}

/// A field in a dialog.
pub enum FieldKind {
    Text(TextField),
    Check(bool),
    Choice(Vec<String>, usize),
    Note(String),
}

pub struct Field {
    pub id: String,
    pub label: String,
    pub kind: FieldKind,
    rect: Rect,
}

/// A small modal form: title, fields, buttons.
pub struct Dialog {
    pub id: String,
    pub title: String,
    pub fields: Vec<Field>,
    pub buttons: Vec<(String, String)>,
    pub rect: Rect,
    btn_rects: Vec<Rect>,
    pub menu: Option<Menu>,
    pub width: f32,
}

pub enum DialogResult {
    None,
    Button(String),
}

impl Dialog {
    pub fn new(id: &str, title: &str) -> Dialog {
        Dialog { id: id.into(), title: title.into(), fields: vec![], buttons: vec![], rect: Rect::default(), btn_rects: vec![], menu: None, width: 400.0 }
    }
    pub fn text(mut self, id: &str, label: &str, value: &str) -> Dialog {
        let mut f = TextField::new(value);
        if !self.fields.iter().any(|f| matches!(f.kind, FieldKind::Text(_))) {
            f.focused = true;
            f.select_all();
        }
        self.fields.push(Field { id: id.into(), label: label.into(), kind: FieldKind::Text(f), rect: Rect::default() });
        self
    }
    pub fn check(mut self, id: &str, label: &str, v: bool) -> Dialog {
        self.fields.push(Field { id: id.into(), label: label.into(), kind: FieldKind::Check(v), rect: Rect::default() });
        self
    }
    pub fn choice(mut self, id: &str, label: &str, opts: &[&str], sel: usize) -> Dialog {
        self.fields.push(Field {
            id: id.into(),
            label: label.into(),
            kind: FieldKind::Choice(opts.iter().map(|s| s.to_string()).collect(), sel),
            rect: Rect::default(),
        });
        self
    }
    pub fn note(mut self, text: &str) -> Dialog {
        self.fields.push(Field { id: String::new(), label: String::new(), kind: FieldKind::Note(text.into()), rect: Rect::default() });
        self
    }
    pub fn button(mut self, id: &str, label: &str) -> Dialog {
        self.buttons.push((id.into(), label.into()));
        self
    }
    pub fn width(mut self, w: f32) -> Dialog {
        self.width = w;
        self
    }
    pub fn value(&self, id: &str) -> String {
        for f in &self.fields {
            if f.id == id {
                return match &f.kind {
                    FieldKind::Text(t) => t.text.clone(),
                    FieldKind::Check(b) => b.to_string(),
                    FieldKind::Choice(o, i) => o.get(*i).cloned().unwrap_or_default(),
                    FieldKind::Note(n) => n.clone(),
                };
            }
        }
        String::new()
    }
    /// Every note in the dialog, run together: what it is telling the person.
    pub fn note_text(&self) -> String {
        self.fields
            .iter()
            .filter_map(|f| match &f.kind {
                FieldKind::Note(n) => Some(n.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    pub fn checked(&self, id: &str) -> bool {
        self.value(id) == "true"
    }
    /// Where a button sits, once the box has been painted (used by the window tests).
    pub fn button_rect(&self, id: &str) -> Option<Rect> {
        let i = self.buttons.iter().position(|(b, _)| b == id)?;
        self.btn_rects.get(i).copied()
    }

    /// Put words in a text field without typing them (used by the window tests).
    pub fn set_text_for_test(&mut self, id: &str, text: &str) {
        for f in self.fields.iter_mut() {
            if f.id == id {
                if let FieldKind::Text(t) = &mut f.kind {
                    t.set(text);
                }
            }
        }
    }

    /// Tick or clear a check field without clicking it (used by the window tests).
    pub fn set_check_for_test(&mut self, id: &str, on: bool) {
        for f in self.fields.iter_mut() {
            if f.id == id {
                if let FieldKind::Check(v) = &mut f.kind {
                    *v = on;
                }
            }
        }
    }

    /// Pick a choice field's option without clicking it (used by the window tests).
    pub fn set_choice(&mut self, id: &str, sel: usize) {
        for f in self.fields.iter_mut() {
            if f.id == id {
                if let FieldKind::Choice(o, i) = &mut f.kind {
                    *i = sel.min(o.len().saturating_sub(1));
                }
            }
        }
    }

    pub fn choice_index(&self, id: &str) -> usize {
        for f in &self.fields {
            if f.id == id {
                if let FieldKind::Choice(_, i) = f.kind {
                    return i;
                }
            }
        }
        0
    }

    pub fn paint(&mut self, cx: &mut Ctx, win: Rect) {
        let s = cx.s;
        let t = cx.theme.clone();
        // Dim the window behind.
        gfx::fill_rect(cx.pm, win, Color::rgba(0, 0, 0, if t.dark { 110 } else { 70 }));
        let w = self.width * s;
        let pad = 20.0 * s;
        let mut h = 56.0 * s;
        let mut note_h = Vec::new();
        for f in &self.fields {
            h += match &f.kind {
                FieldKind::Note(n) => {
                    let mut st = cx.style(13.0);
                    st.color = t.text;
                    let lh = cx.text.layout(n, &st, Some(w - 2.0 * pad)).height + 10.0 * s;
                    note_h.push(lh);
                    lh
                }
                FieldKind::Check(_) => 30.0 * s,
                _ => 58.0 * s,
            };
        }
        h += 60.0 * s;
        self.rect = Rect::new(win.x + (win.w - w) / 2.0, win.y + (win.h - h) / 2.5, w, h).round();
        let r = self.rect;
        gfx::shadow(cx.pm, r, 8.0 * s, t.dark);
        gfx::fill_round(cx.pm, r, 8.0 * s, t.popup);
        gfx::stroke_round(cx.pm, r, 8.0 * s, t.border, 1.0);
        cx.label(&self.title, Rect::new(r.x + pad, r.y + 14.0 * s, r.w - 2.0 * pad, 26.0 * s), 16.0, t.text, true, Align::Left);
        let mut y = r.y + 52.0 * s;
        let mut ni = 0;
        for f in self.fields.iter_mut() {
            match &mut f.kind {
                FieldKind::Note(n) => {
                    let mut st = cx.style(13.0);
                    st.color = t.text;
                    let n = n.clone();
                    cx.text.draw(cx.pm, &n, &st, r.x + pad, y, Some(r.w - 2.0 * pad));
                    y += note_h[ni];
                    ni += 1;
                }
                FieldKind::Text(tf) => {
                    cx.label(&f.label, Rect::new(r.x + pad, y, r.w - 2.0 * pad, 20.0 * s), 12.5, t.muted, false, Align::Left);
                    let fr = Rect::new(r.x + pad, y + 22.0 * s, r.w - 2.0 * pad, 30.0 * s);
                    tf.paint(cx, fr);
                    f.rect = fr;
                    y += 58.0 * s;
                }
                FieldKind::Check(v) => {
                    let br = Rect::new(r.x + pad, y + 5.0 * s, 18.0 * s, 18.0 * s);
                    gfx::fill_round(cx.pm, br, 3.0 * s, if *v { t.accent } else { t.field });
                    gfx::stroke_round(cx.pm, br, 3.0 * s, if *v { t.accent } else { t.muted }, 1.0);
                    if *v {
                        cx.icon_colored("check", br.inset(2.0 * s), t.accent.contrast_text());
                    }
                    cx.label(&f.label, Rect::new(br.right() + 8.0 * s, y, r.w - 2.0 * pad - 26.0 * s, 28.0 * s), 13.0, t.text, false, Align::Left);
                    f.rect = Rect::new(r.x + pad, y, r.w - 2.0 * pad, 28.0 * s);
                    y += 30.0 * s;
                }
                FieldKind::Choice(opts, i) => {
                    cx.label(&f.label, Rect::new(r.x + pad, y, r.w - 2.0 * pad, 20.0 * s), 12.5, t.muted, false, Align::Left);
                    let fr = Rect::new(r.x + pad, y + 22.0 * s, r.w - 2.0 * pad, 30.0 * s);
                    gfx::fill_round(cx.pm, fr, 3.0 * s, t.field);
                    gfx::stroke_round(cx.pm, fr, 3.0 * s, t.border, 1.0);
                    let o = opts.get(*i).cloned().unwrap_or_default();
                    cx.label(&o, Rect::new(fr.x + 8.0 * s, fr.y, fr.w - 30.0 * s, fr.h), 13.0, t.text, false, Align::Left);
                    cx.icon("chevron-down", Rect::new(fr.right() - 22.0 * s, fr.y + 9.0 * s, 12.0 * s, 12.0 * s));
                    f.rect = fr;
                    y += 58.0 * s;
                }
            }
        }
        self.btn_rects.clear();
        let mut bx = r.right() - pad;
        let by = r.bottom() - 48.0 * s;
        for (n, (_, label)) in self.buttons.iter().enumerate().rev() {
            let bw = (cx.measure(label, 13.0, false) + 32.0 * s).max(84.0 * s);
            bx -= bw;
            let br = Rect::new(bx, by, bw, 32.0 * s);
            let primary = n == 0;
            gfx::fill_round(cx.pm, br, 4.0 * s, if primary { t.accent } else { t.field });
            gfx::stroke_round(cx.pm, br, 4.0 * s, if primary { t.accent } else { t.border }, 1.0);
            cx.label(label, br, 13.0, if primary { t.accent.contrast_text() } else { t.text }, primary, Align::Center);
            self.btn_rects.insert(0, br);
            bx -= 8.0 * s;
        }
        if let Some(m) = self.menu.as_mut() {
            m.paint(cx, win);
        }
    }

    pub fn event(&mut self, ev: &Event, host: &mut dyn Host, text: &mut Text, s: f32) -> DialogResult {
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    let mid = m.id.clone();
                    self.menu = None;
                    for f in self.fields.iter_mut() {
                        if f.id == mid {
                            if let FieldKind::Choice(o, i) = &mut f.kind {
                                if let Some(p) = o.iter().position(|x| *x == id) {
                                    *i = p;
                                }
                            }
                        }
                    }
                }
                MenuResult::Closed => self.menu = None,
                MenuResult::None => {}
            }
            return DialogResult::None;
        }
        if let Event::MouseDown { x, y, button: Button::Left, .. } = ev {
            for (i, r) in self.btn_rects.iter().enumerate() {
                if r.contains(*x, *y) {
                    return DialogResult::Button(self.buttons[i].0.clone());
                }
            }
            let (x, y) = (*x, *y);
            let mut focus_field = None;
            for (n, f) in self.fields.iter_mut().enumerate() {
                if !f.rect.contains(x, y) {
                    continue;
                }
                match &mut f.kind {
                    FieldKind::Check(v) => *v = !*v,
                    FieldKind::Choice(o, i) => {
                        let items = o.iter().map(|x| MenuItem::new(x, x)).collect::<Vec<_>>();
                        let _ = i;
                        self.menu = Some(Menu::new(&f.id, f.rect, items));
                        return DialogResult::None;
                    }
                    FieldKind::Text(_) => focus_field = Some(n),
                    FieldKind::Note(_) => {}
                }
            }
            if let Some(n) = focus_field {
                for (i, f) in self.fields.iter_mut().enumerate() {
                    if let FieldKind::Text(tf) = &mut f.kind {
                        tf.focused = i == n;
                    }
                }
            }
        }
        if let Event::Key { key: Key::Tab, down: true, .. } = ev {
            let texts: Vec<usize> = self.fields.iter().enumerate().filter(|(_, f)| matches!(f.kind, FieldKind::Text(_))).map(|(i, _)| i).collect();
            if !texts.is_empty() {
                let cur = texts.iter().position(|&i| matches!(&self.fields[i].kind, FieldKind::Text(t) if t.focused)).unwrap_or(0);
                let next = texts[(cur + 1) % texts.len()];
                for (i, f) in self.fields.iter_mut().enumerate() {
                    if let FieldKind::Text(tf) = &mut f.kind {
                        tf.focused = i == next;
                        if tf.focused {
                            tf.select_all();
                        }
                    }
                }
            }
            return DialogResult::None;
        }
        for f in self.fields.iter_mut() {
            if let FieldKind::Text(tf) = &mut f.kind {
                if !tf.focused && !matches!(ev, Event::MouseDown { .. }) {
                    continue;
                }
                match tf.event(ev, host, text, s) {
                    FieldResult::Enter => return DialogResult::Button(self.buttons.first().map(|b| b.0.clone()).unwrap_or_default()),
                    FieldResult::Escape => return DialogResult::Button("cancel".into()),
                    _ => {}
                }
            }
        }
        match ev {
            Event::Key { key: Key::Escape, down: true, .. } => DialogResult::Button("cancel".into()),
            Event::Key { key: Key::Enter, down: true, .. } => DialogResult::Button(self.buttons.first().map(|b| b.0.clone()).unwrap_or_default()),
            _ => DialogResult::None,
        }
    }
}

/// A ScreenTip: a bold title, a description and a shortcut, under the hovered control.
pub fn paint_tip(cx: &mut Ctx, win: Rect, anchor: Rect, title: &str, body: &str, shortcut: &str) {
    let s = cx.s;
    let t = cx.theme.clone();
    let w = 260.0 * s;
    let pad = 10.0 * s;
    let head = if shortcut.is_empty() { title.to_string() } else { format!("{title} ({shortcut})") };
    let mut st = cx.style(12.5);
    st.color = t.text;
    let bh = if body.is_empty() { 0.0 } else { cx.text.layout(body, &st, Some(w - 2.0 * pad)).height + 6.0 * s };
    let h = 22.0 * s + bh + 2.0 * pad - 6.0 * s;
    let mut x = anchor.x;
    let mut y = anchor.bottom() + 4.0 * s;
    if x + w > win.right() {
        x = win.right() - w - 4.0;
    }
    if y + h > win.bottom() {
        y = anchor.y - h - 4.0 * s;
    }
    let r = Rect::new(x, y, w, h).round();
    gfx::shadow(cx.pm, r, 4.0 * s, t.dark);
    gfx::fill_round(cx.pm, r, 4.0 * s, t.popup);
    gfx::stroke_round(cx.pm, r, 4.0 * s, t.border, 1.0);
    cx.label(&head, Rect::new(r.x + pad, r.y + pad - 3.0 * s, r.w - 2.0 * pad, 20.0 * s), 12.5, t.text, true, Align::Left);
    if !body.is_empty() {
        cx.text.draw(cx.pm, body, &st, r.x + pad, r.y + pad + 20.0 * s, Some(w - 2.0 * pad));
    }
}

/// A small KeyTip badge.
pub fn paint_keytip(cx: &mut Ctx, x: f32, y: f32, tip: &str, dim: bool) {
    let s = cx.s;
    let t = cx.theme.clone();
    let w = cx.measure(tip, 11.0, true) + 8.0 * s;
    let r = Rect::new(x - w / 2.0, y, w.max(16.0 * s), 17.0 * s).round();
    let bg = if t.dark { Color::hex(0xEDEDED) } else { Color::hex(0x2B2B2B) };
    gfx::fill_round(cx.pm, r, 3.0 * s, if dim { bg.with_alpha(110) } else { bg });
    cx.label(tip, r, 11.0, bg.contrast_text(), true, Align::Center);
}
