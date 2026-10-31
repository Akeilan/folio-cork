//! The Folio ribbon: tabs of grouped commands, KeyTips (Alt) and ScreenTips (hover).

use crate::gfx::{self, Color, Rect};
use crate::text::Align;
use crate::ui::{paint_keytip, paint_tip, CommandState, Ctx};
use folio_platform::{Button, Event, Key};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Large,
    Small,
    Icon,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Button,
    /// Opens a menu the app supplies.
    Menu,
    /// Button part runs the command; arrow part opens the menu (`<id>.menu`).
    Split,
    /// Shows live text from [`CommandState::text`] and opens a menu; the width is in points.
    Combo(u16),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub size: Size,
    pub kind: Kind,
    pub desc: String,
    pub shortcut: String,
    pub keytip: String,
}

impl Item {
    fn new(id: &str, label: &str, icon: &str, size: Size) -> Item {
        Item { id: id.into(), label: label.into(), icon: icon.into(), size, kind: Kind::Button, desc: String::new(), shortcut: String::new(), keytip: String::new() }
    }
    pub fn large(id: &str, label: &str, icon: &str) -> Item {
        Item::new(id, label, icon, Size::Large)
    }
    pub fn small(id: &str, label: &str, icon: &str) -> Item {
        Item::new(id, label, icon, Size::Small)
    }
    pub fn icon(id: &str, label: &str, icon: &str) -> Item {
        Item::new(id, label, icon, Size::Icon)
    }
    pub fn combo(id: &str, label: &str, width: u16) -> Item {
        let mut i = Item::new(id, label, "", Size::Small);
        i.kind = Kind::Combo(width);
        i
    }
    pub fn desc(mut self, d: &str) -> Item {
        self.desc = d.into();
        self
    }
    pub fn key(mut self, k: &str) -> Item {
        self.shortcut = k.into();
        self
    }
    pub fn tip(mut self, k: &str) -> Item {
        self.keytip = k.into();
        self
    }
    pub fn menu(mut self) -> Item {
        self.kind = Kind::Menu;
        self
    }
    pub fn split(mut self) -> Item {
        self.kind = Kind::Split;
        self
    }
}

#[derive(Clone, Debug)]
pub struct Group {
    pub label: String,
    pub items: Vec<Item>,
    /// When set, items flow left to right in rows; a new row starts at each index listed.
    pub breaks: Vec<usize>,
}

impl Group {
    pub fn new(label: &str, items: Vec<Item>) -> Group {
        Group { label: label.into(), items, breaks: vec![] }
    }
    /// A group laid out in rows, like a font group: each inner list is one row.
    pub fn rows(label: &str, rows: Vec<Vec<Item>>) -> Group {
        let mut items = vec![];
        let mut breaks = vec![];
        for r in rows {
            breaks.push(items.len());
            items.extend(r);
        }
        Group { label: label.into(), items, breaks }
    }
}

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: String,
    pub label: String,
    pub keytip: String,
    pub groups: Vec<Group>,
}

impl Tab {
    pub fn new(id: &str, label: &str, keytip: &str, groups: Vec<Group>) -> Tab {
        Tab { id: id.into(), label: label.into(), keytip: keytip.into(), groups }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    File,
    Tab(usize),
    Item(usize, usize),
    Arrow(usize, usize),
    Group(usize),
    Collapse,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RibbonAction {
    Command(String),
    /// Open the app's menu for this command under the given rectangle.
    Menu(String, Rect),
    Backstage,
    /// A collapsed group was clicked: show these commands as a menu.
    GroupMenu(Vec<(String, String, String)>, Rect),
}

#[derive(Clone, Debug, PartialEq)]
enum Tips {
    Off,
    Tabs,
    Items,
}

pub struct Ribbon {
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub collapsed: bool,
    pub file_label: String,
    pub rect: Rect,
    hits: Vec<(Hit, Rect)>,
    hover: Option<Hit>,
    pressed: Option<Hit>,
    hover_at: Duration,
    now: Duration,
    tips: Tips,
    typed: String,
    alt_clean: bool,
    item_tips: Vec<(String, usize, usize)>,
    collapsed_groups: Vec<bool>,
}

pub const TAB_H: f32 = 30.0;
pub const BODY_H: f32 = 98.0;

impl Ribbon {
    pub fn new(tabs: Vec<Tab>) -> Ribbon {
        Ribbon {
            tabs,
            active: 0,
            collapsed: false,
            file_label: "File".into(),
            rect: Rect::default(),
            hits: vec![],
            hover: None,
            pressed: None,
            hover_at: Duration::ZERO,
            now: Duration::ZERO,
            tips: Tips::Off,
            typed: String::new(),
            alt_clean: false,
            item_tips: vec![],
            collapsed_groups: vec![],
        }
    }

    pub fn height(&self, s: f32) -> f32 {
        (TAB_H + if self.collapsed { 0.0 } else { BODY_H }) * s
    }

    pub fn keytips_on(&self) -> bool {
        self.tips != Tips::Off
    }

    pub fn select_tab(&mut self, id: &str) {
        if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
            self.active = i;
        }
    }

    pub fn active_id(&self) -> &str {
        &self.tabs[self.active].id
    }

    /// The rectangle of a command's control, from the last paint (for anchoring menus).
    pub fn rect_of(&self, id: &str) -> Option<Rect> {
        let tab = &self.tabs[self.active];
        for (h, r) in &self.hits {
            if let Hit::Item(g, i) = h {
                if tab.groups[*g].items[*i].id == id {
                    return Some(*r);
                }
            }
        }
        None
    }

    /// When the app should send a tick so a ScreenTip can appear.
    pub fn next_tick(&self) -> Option<Duration> {
        match self.hover {
            Some(Hit::Item(..)) | Some(Hit::Arrow(..)) if self.now.saturating_sub(self.hover_at) < Duration::from_millis(650) => Some(Duration::from_millis(700)),
            _ => None,
        }
    }

    fn assign_item_tips(&mut self) {
        self.item_tips.clear();
        let tab = &self.tabs[self.active];
        let mut used: Vec<String> = Vec::new();
        for (g, grp) in tab.groups.iter().enumerate() {
            for (i, it) in grp.items.iter().enumerate() {
                let mut tip = it.keytip.to_uppercase();
                if tip.is_empty() || used.contains(&tip) {
                    tip.clear();
                    let letters: Vec<char> = it.label.chars().filter(|c| c.is_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
                    for &c in &letters {
                        let t = c.to_string();
                        if !used.iter().any(|u| *u == t || u.starts_with(&t)) {
                            tip = t;
                            break;
                        }
                    }
                    if tip.is_empty() {
                        'outer: for &a in &letters {
                            for &b in letters.iter().chain(['1', '2', '3', '4', '5', '6', '7', '8', '9'].iter()) {
                                let t = format!("{a}{b}");
                                if !used.iter().any(|u| *u == t || t.starts_with(u.as_str())) {
                                    tip = t;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
                used.push(tip.clone());
                self.item_tips.push((tip, g, i));
            }
        }
    }

    fn item_width(&self, cx: &mut Ctx, it: &Item) -> f32 {
        let s = cx.s;
        match (it.size, it.kind) {
            (_, Kind::Combo(w)) => w as f32 * s,
            (Size::Large, _) => {
                let (a, b) = split_label(&it.label);
                let w = cx.measure(&a, 12.0, false).max(cx.measure(&b, 12.0, false) + if it.kind != Kind::Button { 12.0 * s } else { 0.0 });
                (w + 12.0 * s).max(46.0 * s)
            }
            (Size::Small, k) => 26.0 * s + cx.measure(&it.label, 12.0, false) + if k != Kind::Button { 14.0 * s } else { 0.0 } + 8.0 * s,
            (Size::Icon, k) => 26.0 * s + if k != Kind::Button { 10.0 * s } else { 0.0 },
        }
    }

    /// Row layout: (item index, x offset, row) and the group's width.
    fn row_layout(&self, cx: &mut Ctx, g: &Group) -> (Vec<(usize, f32, usize)>, f32) {
        let mut out = vec![];
        let mut w: f32 = 0.0;
        let mut row = 0;
        let mut x = 0.0;
        for (i, it) in g.items.iter().enumerate() {
            if i > 0 && g.breaks.contains(&i) {
                row += 1;
                x = 0.0;
            }
            out.push((i, x, row));
            x += self.item_width(cx, it) + 2.0 * cx.s;
            w = w.max(x);
        }
        (out, w)
    }

    /// Columns of a group: each is a list of item indices with widths.
    fn columns(&self, cx: &mut Ctx, g: &Group) -> Vec<(Vec<usize>, f32)> {
        let mut cols: Vec<(Vec<usize>, f32)> = Vec::new();
        let mut cur: Vec<usize> = Vec::new();
        let mut cw: f32 = 0.0;
        let mut icon_row: Vec<usize> = Vec::new();
        let flush_icons = |cur: &mut Vec<usize>, cw: &mut f32, icon_row: &mut Vec<usize>, s: f32| {
            if !icon_row.is_empty() {
                *cw = cw.max(icon_row.len() as f32 * 26.0 * s);
                cur.extend(icon_row.drain(..));
            }
        };
        for (i, it) in g.items.iter().enumerate() {
            let w = self.item_width(cx, it);
            if it.size == Size::Large {
                flush_icons(&mut cur, &mut cw, &mut icon_row, cx.s);
                if !cur.is_empty() {
                    cols.push((std::mem::take(&mut cur), cw));
                    cw = 0.0;
                }
                cols.push((vec![i], w));
            } else {
                cur.push(i);
                cw = cw.max(w);
                if cur.len() == 3 {
                    cols.push((std::mem::take(&mut cur), cw));
                    cw = 0.0;
                }
            }
        }
        flush_icons(&mut cur, &mut cw, &mut icon_row, cx.s);
        if !cur.is_empty() {
            cols.push((cur, cw));
        }
        cols
    }

    pub fn paint(&mut self, cx: &mut Ctx, r: Rect, st: &dyn CommandState, now: Duration) {
        self.now = now;
        self.rect = r;
        self.hits.clear();
        let s = cx.s;
        let t = cx.theme.clone();
        let tab_h = TAB_H * s;
        gfx::fill_rect(cx.pm, Rect::new(r.x, r.y, r.w, tab_h), t.tabs);
        // File tab.
        let mut x = r.x + 6.0 * s;
        let fw = cx.measure(&self.file_label, 13.0, false) + 26.0 * s;
        let fr = Rect::new(x, r.y + 3.0 * s, fw, tab_h - 6.0 * s);
        let fhov = self.hover == Some(Hit::File);
        gfx::fill_round(cx.pm, fr, 4.0 * s, if fhov { t.accent.mix(Color::BLACK, 0.12) } else { t.accent });
        let fl = self.file_label.clone();
        cx.label(&fl, fr, 13.0, t.accent.contrast_text(), false, Align::Center);
        self.hits.push((Hit::File, fr));
        x += fw + 6.0 * s;
        let mut tab_rects = vec![];
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = cx.measure(&tab.label, 13.0, i == self.active) + 22.0 * s;
            let tr = Rect::new(x, r.y, w, tab_h);
            if self.hover == Some(Hit::Tab(i)) && i != self.active {
                gfx::fill_round(cx.pm, tr.inset(3.0 * s), 4.0 * s, t.hover);
            }
            cx.label(&tab.label, tr, 13.0, t.text, i == self.active, Align::Center);
            if i == self.active && !self.collapsed {
                gfx::fill_round(cx.pm, Rect::new(tr.x + 8.0 * s, tr.bottom() - 3.0 * s, tr.w - 16.0 * s, 3.0 * s), 1.5 * s, t.accent);
            }
            tab_rects.push(tr);
            x += w;
        }
        for (i, tr) in tab_rects.iter().enumerate() {
            self.hits.push((Hit::Tab(i), *tr));
        }
        // Collapse/expand toggle at the right of the tab strip.
        let cr = Rect::new(r.right() - 30.0 * s, r.y + 4.0 * s, 24.0 * s, tab_h - 8.0 * s);
        if self.hover == Some(Hit::Collapse) {
            gfx::fill_round(cx.pm, cr, 4.0 * s, t.hover);
        }
        cx.icon(if self.collapsed { "chevron-down" } else { "chevron-up" }, Rect::new(cr.x + 6.0 * s, cr.y + (cr.h - 12.0 * s) / 2.0, 12.0 * s, 12.0 * s));
        self.hits.push((Hit::Collapse, cr));

        if self.tips == Tips::Tabs {
            paint_keytip(cx, fr.cx(), fr.bottom() - 6.0 * s, "F", false);
            for (i, tr) in tab_rects.iter().enumerate() {
                let k = self.tabs[i].keytip.clone();
                paint_keytip(cx, tr.cx(), tr.bottom() - 6.0 * s, &k, false);
            }
        }
        if self.collapsed {
            gfx::hline(cx.pm, r.x, r.right(), r.y + tab_h - 1.0, t.border);
            return;
        }
        let body = Rect::new(r.x, r.y + tab_h, r.w, BODY_H * s);
        gfx::fill_rect(cx.pm, body, t.ribbon);
        gfx::hline(cx.pm, r.x, r.right(), body.bottom() - 1.0, t.border);
        let tab = self.tabs[self.active].clone();
        // Decide which groups collapse to fit.
        let mut widths = Vec::new();
        let mut cols_all = Vec::new();
        for g in &tab.groups {
            let cols = self.columns(cx, g);
            let gw: f32 = if g.breaks.is_empty() { cols.iter().map(|c| c.1).sum::<f32>() + (cols.len().saturating_sub(1)) as f32 * 4.0 * s } else { self.row_layout(cx, g).1 };
            let lw = cx.measure(&g.label, 11.5, false);
            widths.push(gw.max(lw) + 16.0 * s);
            cols_all.push(cols);
        }
        let avail = r.w - 40.0 * s;
        self.collapsed_groups = vec![false; tab.groups.len()];
        let mut total: f32 = widths.iter().sum();
        let collapsed_w = 64.0 * s;
        let mut gi = tab.groups.len();
        while total > avail && gi > 0 {
            gi -= 1;
            self.collapsed_groups[gi] = true;
            total += collapsed_w - widths[gi];
        }
        let mut gx = r.x + 6.0 * s;
        let gy = body.y + 4.0 * s;
        let gh = body.h - 8.0 * s;
        let label_h = 18.0 * s;
        let show_tip = |hov: Option<Hit>, at: Duration| hov.is_some() && now.saturating_sub(at) >= Duration::from_millis(650);
        let mut tip: Option<(Rect, String, String, String)> = None;
        for (g, grp) in tab.groups.iter().enumerate() {
            if self.collapsed_groups[g] {
                let br = Rect::new(gx, gy, collapsed_w - 8.0 * s, gh);
                if self.hover == Some(Hit::Group(g)) {
                    gfx::fill_round(cx.pm, br, 4.0 * s, t.hover);
                }
                let icon = grp.items.first().map(|i| i.icon.clone()).unwrap_or_default();
                cx.icon(&icon, Rect::new(br.cx() - 16.0 * s, br.y + 10.0 * s, 32.0 * s, 32.0 * s));
                cx.label(&grp.label, Rect::new(br.x, br.y + 48.0 * s, br.w, 18.0 * s), 11.5, t.text, false, Align::Center);
                cx.icon("chevron-down", Rect::new(br.cx() - 5.0 * s, br.y + 66.0 * s, 10.0 * s, 10.0 * s));
                self.hits.push((Hit::Group(g), br));
                gx += collapsed_w;
                gfx::vline(cx.pm, gx - 4.0 * s, gy + 4.0 * s, gy + gh - 4.0 * s, t.border);
                continue;
            }
            let gw = widths[g] - 16.0 * s;
            // Row-mode groups become one single-item "column" per item at its own place.
            let placed: Vec<(Vec<usize>, f32, f32, f32)> = if grp.breaks.is_empty() {
                let mut v = vec![];
                let mut x = gx + 4.0 * s;
                for (items, cw) in &cols_all[g] {
                    v.push((items.clone(), *cw, x, gy));
                    x += cw + 4.0 * s;
                }
                v
            } else {
                let (lay, _) = self.row_layout(cx, grp);
                lay.into_iter().map(|(i, x, r)| (vec![i], self.item_width(cx, &grp.items[i]), gx + 4.0 * s + x, gy + r as f32 * 25.0 * s)).collect()
            };
            for (items, cw, cx0, y0) in placed.iter().map(|(a, b, c, d)| (a, b, *c, *d)) {
                let mut iy = y0;
                for &i in items {
                    let it = &grp.items[i];
                    let enabled = st.enabled(&it.id);
                    let checked = st.checked(&it.id);
                    let fg = if enabled { t.text } else { t.muted };
                    let hov = self.hover == Some(Hit::Item(g, i));
                    let ahov = self.hover == Some(Hit::Arrow(g, i));
                    let press = self.pressed == Some(Hit::Item(g, i)) || self.pressed == Some(Hit::Arrow(g, i));
                    match (it.size, it.kind) {
                        (_, Kind::Combo(w)) => {
                            let ir = Rect::new(cx0, iy + 2.0 * s, w as f32 * s, 22.0 * s);
                            gfx::fill_round(cx.pm, ir, 3.0 * s, t.field);
                            gfx::stroke_round(cx.pm, ir, 3.0 * s, if hov { t.accent } else { t.border }, 1.0);
                            let txt = st.text(&it.id).unwrap_or_default();
                            cx.label(&txt, Rect::new(ir.x + 6.0 * s, ir.y, ir.w - 24.0 * s, ir.h), 12.0, fg, false, Align::Left);
                            cx.icon_colored("chevron-down", Rect::new(ir.right() - 16.0 * s, ir.y + 6.0 * s, 10.0 * s, 10.0 * s), fg);
                            self.hits.push((Hit::Item(g, i), ir));
                            iy += 25.0 * s;
                        }
                        (Size::Large, _) => {
                            let ir = Rect::new(cx0, gy, *cw, gh - label_h);
                            let split = it.kind == Kind::Split;
                            let top = if split { Rect::new(ir.x, ir.y, ir.w, 42.0 * s) } else { ir };
                            if checked {
                                gfx::fill_round(cx.pm, ir, 4.0 * s, t.checked);
                            }
                            if (hov || press) && enabled {
                                gfx::fill_round(cx.pm, top, 4.0 * s, if press { t.pressed } else { t.hover });
                            }
                            if split && (ahov || hov) && enabled {
                                let ar = Rect::new(ir.x, ir.y + 42.0 * s, ir.w, ir.h - 42.0 * s);
                                gfx::fill_round(cx.pm, ar, 4.0 * s, if ahov { t.hover } else { t.hover.with_alpha(120) });
                            }
                            cx.icon_colored(&it.icon, Rect::new(ir.cx() - 16.0 * s, ir.y + 6.0 * s, 32.0 * s, 32.0 * s), fg);
                            if let Some(c) = st.swatch(&it.id) {
                                gfx::fill_rect(cx.pm, Rect::new(ir.cx() - 12.0 * s, ir.y + 36.0 * s, 24.0 * s, 4.0 * s), c);
                            }
                            let (a, b) = split_label(&it.label);
                            cx.label(&a, Rect::new(ir.x, ir.y + 42.0 * s, ir.w, 15.0 * s), 12.0, fg, false, Align::Center);
                            let arrow = it.kind != Kind::Button;
                            let bw = cx.measure(&b, 12.0, false);
                            let total = bw + if arrow { 12.0 * s } else { 0.0 };
                            let bx = ir.cx() - total / 2.0;
                            cx.label(&b, Rect::new(bx, ir.y + 57.0 * s, bw + 2.0, 15.0 * s), 12.0, fg, false, Align::Left);
                            if arrow {
                                cx.icon_colored("chevron-down", Rect::new(bx + bw + 3.0 * s, ir.y + 61.0 * s, 8.0 * s, 8.0 * s), fg);
                            }
                            if split {
                                self.hits.push((Hit::Item(g, i), top));
                                self.hits.push((Hit::Arrow(g, i), Rect::new(ir.x, ir.y + 42.0 * s, ir.w, ir.h - 42.0 * s)));
                            } else {
                                self.hits.push((Hit::Item(g, i), ir));
                            }
                            if (hov || ahov) && show_tip(self.hover, self.hover_at) {
                                tip = Some((ir, it.label.clone(), it.desc.clone(), it.shortcut.clone()));
                            }
                        }
                        (size, kind) => {
                            let w = if size == Size::Icon { 26.0 * s + if kind != Kind::Button { 10.0 * s } else { 0.0 } } else { *cw };
                            let ir = Rect::new(cx0, iy + 1.0 * s, w, 23.0 * s);
                            let split = kind == Kind::Split;
                            let main = if split { Rect::new(ir.x, ir.y, ir.w - 14.0 * s, ir.h) } else { ir };
                            if checked {
                                gfx::fill_round(cx.pm, ir, 3.0 * s, t.checked);
                            }
                            if (hov || press) && enabled {
                                gfx::fill_round(cx.pm, main, 3.0 * s, if press { t.pressed } else { t.hover });
                            }
                            if split && (ahov || hov) && enabled {
                                gfx::fill_round(cx.pm, Rect::new(ir.right() - 14.0 * s, ir.y, 14.0 * s, ir.h), 3.0 * s, if ahov { t.hover } else { t.hover.with_alpha(120) });
                            }
                            cx.icon_colored(&it.icon, Rect::new(ir.x + 5.0 * s, ir.y + 3.5 * s, 16.0 * s, 16.0 * s), fg);
                            if let Some(c) = st.swatch(&it.id) {
                                gfx::fill_rect(cx.pm, Rect::new(ir.x + 5.0 * s, ir.y + 18.0 * s, 16.0 * s, 3.0 * s), c);
                            }
                            if size == Size::Small {
                                cx.label(&it.label, Rect::new(ir.x + 26.0 * s, ir.y, ir.w - 26.0 * s, ir.h), 12.0, fg, false, Align::Left);
                            }
                            if kind != Kind::Button {
                                cx.icon_colored("chevron-down", Rect::new(ir.right() - 11.0 * s, ir.y + 8.0 * s, 8.0 * s, 8.0 * s), fg);
                            }
                            if split {
                                self.hits.push((Hit::Item(g, i), main));
                                self.hits.push((Hit::Arrow(g, i), Rect::new(ir.right() - 14.0 * s, ir.y, 14.0 * s, ir.h)));
                            } else {
                                self.hits.push((Hit::Item(g, i), ir));
                            }
                            if (hov || ahov) && show_tip(self.hover, self.hover_at) {
                                tip = Some((ir, it.label.clone(), it.desc.clone(), it.shortcut.clone()));
                            }
                            iy += 25.0 * s;
                        }
                    }
                }
            }
            cx.label(&grp.label, Rect::new(gx, gy + gh - label_h, gw + 8.0 * s, label_h), 11.5, t.muted, false, Align::Center);
            gx += gw + 16.0 * s;
            gfx::vline(cx.pm, gx - 6.0 * s, gy + 4.0 * s, gy + gh - 4.0 * s, t.border);
        }
        if self.tips == Tips::Items {
            if self.item_tips.is_empty() {
                self.assign_item_tips();
            }
            for (tip, g, i) in self.item_tips.clone() {
                if self.collapsed_groups.get(g).copied().unwrap_or(false) {
                    continue;
                }
                if let Some((_, r)) = self.hits.iter().find(|(h, _)| *h == Hit::Item(g, i)) {
                    let dim = !self.typed.is_empty() && !tip.starts_with(&self.typed);
                    let y = if r.h > 40.0 * s { r.bottom() - 12.0 * s } else { r.bottom() - 8.0 * s };
                    paint_keytip(cx, r.x + 12.0 * s, y, &tip, dim);
                }
            }
        }
        if let Some((ar, a, b, c)) = tip {
            if self.tips == Tips::Off {
                let win = Rect::new(0.0, 0.0, cx.pm.width() as f32, cx.pm.height() as f32);
                paint_tip(cx, win, ar, &a, &b, &c);
            }
        }
    }

    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        self.hits.iter().rev().find(|(_, r)| r.contains(x, y)).map(|(h, _)| *h)
    }

    fn activate(&mut self, h: Hit, st: &dyn CommandState) -> Option<RibbonAction> {
        match h {
            Hit::File => Some(RibbonAction::Backstage),
            Hit::Tab(i) => {
                self.active = i;
                if self.collapsed {
                    self.collapsed = false;
                }
                None
            }
            Hit::Collapse => {
                self.collapsed = !self.collapsed;
                None
            }
            Hit::Group(g) => {
                let grp = &self.tabs[self.active].groups[g];
                let items = grp.items.iter().filter(|i| st.enabled(&i.id)).map(|i| (i.id.clone(), i.label.clone(), i.icon.clone())).collect();
                let r = self.hits.iter().find(|(x, _)| *x == h).map(|x| x.1).unwrap_or_default();
                Some(RibbonAction::GroupMenu(items, r))
            }
            Hit::Item(g, i) | Hit::Arrow(g, i) => {
                let it = &self.tabs[self.active].groups[g].items[i];
                if !st.enabled(&it.id) {
                    return None;
                }
                let r = self.hits.iter().find(|(x, _)| *x == h).map(|x| x.1).unwrap_or_default();
                let full = self
                    .hits
                    .iter()
                    .filter(|(x, _)| matches!(x, Hit::Item(a, b) | Hit::Arrow(a, b) if *a == g && *b == i))
                    .fold(r, |acc, (_, r2)| union(acc, *r2));
                match (it.kind, matches!(h, Hit::Arrow(..))) {
                    (Kind::Menu, _) | (Kind::Combo(_), _) => Some(RibbonAction::Menu(it.id.clone(), full)),
                    (Kind::Split, true) => Some(RibbonAction::Menu(format!("{}.menu", it.id), full)),
                    _ => Some(RibbonAction::Command(it.id.clone())),
                }
            }
        }
    }

    /// Feed an event. Returns an action when a control is used. `consumed` tells whether the
    /// ribbon took the event (KeyTips swallow keys).
    pub fn event(&mut self, ev: &Event, st: &dyn CommandState, now: Duration) -> (Option<RibbonAction>, bool) {
        self.now = now;
        match ev {
            Event::MouseMove { x, y } => {
                let h = if self.rect.contains(*x, *y) { self.hit(*x, *y) } else { None };
                if h != self.hover {
                    self.hover = h;
                    self.hover_at = now;
                }
                (None, h.is_some())
            }
            Event::MouseLeave => {
                self.hover = None;
                (None, false)
            }
            Event::Wheel { dy, .. } => {
                if let Some(Hit::Tab(_)) = self.hover {
                    let n = self.tabs.len();
                    self.active = if *dy > 0.0 { (self.active + n - 1) % n } else { (self.active + 1) % n };
                    return (None, true);
                }
                (None, false)
            }
            Event::MouseDown { x, y, button: Button::Left, clicks } => {
                if self.tips != Tips::Off {
                    self.tips = Tips::Off;
                }
                if !self.rect.contains(*x, *y) {
                    return (None, false);
                }
                let h = self.hit(*x, *y);
                if let (Some(Hit::Tab(_)), 2) = (h, *clicks) {
                    self.collapsed = !self.collapsed;
                    return (None, true);
                }
                self.pressed = h;
                self.hover_at = now + Duration::from_secs(3600);
                match h {
                    Some(h @ (Hit::Item(..) | Hit::Arrow(..))) => {
                        let it_kind = match h {
                            Hit::Item(g, i) | Hit::Arrow(g, i) => self.tabs[self.active].groups[g].items[i].kind,
                            _ => Kind::Button,
                        };
                        // Menus open on press; plain buttons run on release.
                        if matches!(it_kind, Kind::Menu | Kind::Combo(_)) || matches!(h, Hit::Arrow(..)) {
                            self.pressed = None;
                            return (self.activate(h, st), true);
                        }
                        (None, true)
                    }
                    Some(h) => {
                        self.pressed = None;
                        (self.activate(h, st), true)
                    }
                    None => (None, true),
                }
            }
            Event::MouseUp { x, y, button: Button::Left } => {
                let p = self.pressed.take();
                if let Some(p) = p {
                    if self.hit(*x, *y) == Some(p) {
                        return (self.activate(p, st), true);
                    }
                    return (None, true);
                }
                (None, false)
            }
            Event::Key { key: Key::Alt, down, mods, .. } => {
                if *down {
                    self.alt_clean = !mods.ctrl && !mods.shift;
                } else if self.alt_clean {
                    self.alt_clean = false;
                    self.tips = if self.tips == Tips::Off { Tips::Tabs } else { Tips::Off };
                    self.typed.clear();
                    return (None, true);
                }
                (None, false)
            }
            Event::Key { key: Key::F(10), down: true, .. } => {
                self.tips = if self.tips == Tips::Off { Tips::Tabs } else { Tips::Off };
                self.typed.clear();
                (None, true)
            }
            Event::Key { key: Key::F(1), down: true, mods, .. } if mods.ctrl => {
                self.collapsed = !self.collapsed;
                (None, true)
            }
            Event::Key { down: true, .. } if self.alt_clean => {
                self.alt_clean = false;
                (None, false)
            }
            Event::Key { key, down: true, .. } if self.tips != Tips::Off => {
                match key {
                    Key::Escape => {
                        self.tips = if self.tips == Tips::Items { Tips::Tabs } else { Tips::Off };
                        self.typed.clear();
                    }
                    Key::Char(c) => {
                        let c = c.to_ascii_uppercase();
                        if self.tips == Tips::Tabs {
                            if c == 'F' {
                                self.tips = Tips::Off;
                                return (Some(RibbonAction::Backstage), true);
                            }
                            if let Some(i) = self.tabs.iter().position(|t| t.keytip.eq_ignore_ascii_case(&c.to_string())) {
                                self.active = i;
                                self.collapsed = false;
                                self.tips = Tips::Items;
                                self.typed.clear();
                                self.assign_item_tips();
                            }
                        } else {
                            self.typed.push(c);
                            let typed = self.typed.clone();
                            if let Some((_, g, i)) = self.item_tips.iter().find(|(t, _, _)| *t == typed).cloned() {
                                self.tips = Tips::Off;
                                self.typed.clear();
                                return (self.activate(Hit::Item(g, i), st), true);
                            }
                            if !self.item_tips.iter().any(|(t, _, _)| t.starts_with(&typed)) {
                                self.typed.clear();
                            }
                        }
                    }
                    _ => {}
                }
                (None, true)
            }
            Event::Text(_) if self.tips != Tips::Off => (None, true),
            _ => (None, false),
        }
    }

    /// Turn KeyTips off (e.g. when a menu opens).
    pub fn cancel_tips(&mut self) {
        self.tips = Tips::Off;
        self.typed.clear();
    }

    /// Show the tab KeyTips (tests and the Alt key).
    pub fn show_tips(&mut self) {
        self.tips = Tips::Tabs;
    }

    pub fn hovering(&self) -> bool {
        self.hover.is_some()
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Split a large button's label over two lines at the space nearest the middle.
pub fn split_label(s: &str) -> (String, String) {
    let words: Vec<&str> = s.split(' ').collect();
    if words.len() < 2 {
        return (s.to_string(), String::new());
    }
    let mut best = 1;
    let mut bd = usize::MAX;
    for i in 1..words.len() {
        let a = words[..i].join(" ").len();
        let b = words[i..].join(" ").len();
        let d = a.abs_diff(b);
        if d < bd {
            bd = d;
            best = i;
        }
    }
    (words[..best].join(" "), words[best..].join(" "))
}
