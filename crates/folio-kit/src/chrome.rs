//! Window chrome: the title bar in the app colour, window buttons, resize edges, status bar.

use crate::gfx::{self, Color, Rect};
use crate::text::Align;
use crate::ui::{Ctx, FieldResult, TextField};
use folio_platform::{Button, Edge, Event};

#[derive(Clone, Debug, PartialEq)]
pub enum TitleAction {
    Command(String),
    Minimize,
    Maximize,
    Close,
    Drag,
    /// The search box's words changed.
    SearchChanged,
    /// Enter pressed in the search box.
    SearchEnter,
    /// Escape pressed in the search box (it has been emptied and let go).
    SearchCancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TitleHit {
    Quick(usize),
    QuickMenu,
    Search,
    Min,
    Max,
    Close,
}

pub struct TitleBar {
    /// Quick-access buttons: (command id, icon, tooltip).
    pub quick: Vec<(String, String, String)>,
    pub app_icon: String,
    /// A box for searching the app's commands, at the right of the bar.
    pub search: Option<TextField>,
    /// A chevron after the quick-access buttons, for choosing which of them show.
    pub quick_menu: bool,
    pub rect: Rect,
    hits: Vec<(TitleHit, Rect)>,
    hover: Option<TitleHit>,
    pressed: Option<TitleHit>,
}

pub const TITLE_H: f32 = 34.0;

impl TitleBar {
    pub fn new(app_icon: &str, quick: Vec<(&str, &str, &str)>) -> TitleBar {
        TitleBar {
            quick: quick.into_iter().map(|(a, b, c)| (a.into(), b.into(), c.into())).collect(),
            app_icon: app_icon.into(),
            search: None,
            quick_menu: false,
            rect: Rect::default(),
            hits: vec![],
            hover: None,
            pressed: None,
        }
    }

    /// Add a search box; the app reads `search` and acts on SearchChanged and SearchEnter.
    pub fn with_search(mut self) -> TitleBar {
        self.search = Some(TextField::new(""));
        self
    }

    /// Add the chevron that asks the app for its quick-access menu (the command `quick.menu`).
    pub fn with_quick_menu(mut self) -> TitleBar {
        self.quick_menu = true;
        self
    }

    /// Where the search box is, for a list of matches drawn under it.
    pub fn search_rect(&self) -> Rect {
        self.search.as_ref().map(|f| f.rect).unwrap_or_default()
    }

    pub fn paint(&mut self, cx: &mut Ctx, r: Rect, title: &str, maximized: bool, enabled: &dyn Fn(&str) -> bool) {
        self.rect = r;
        self.hits.clear();
        let s = cx.s;
        let t = cx.theme.clone();
        gfx::fill_rect(cx.pm, r, t.app);
        let fg = t.app_text;
        let ai = self.app_icon.clone();
        cx.icon_colored(&ai, Rect::new(r.x + 10.0 * s, r.y + (r.h - 20.0 * s) / 2.0, 20.0 * s, 20.0 * s), fg);
        let mut x = r.x + 40.0 * s;
        for (i, (id, icon, _)) in self.quick.clone().iter().enumerate() {
            let br = Rect::new(x, r.y + 4.0 * s, 28.0 * s, r.h - 8.0 * s);
            let on = enabled(id);
            if self.hover == Some(TitleHit::Quick(i)) && on {
                gfx::fill_round(cx.pm, br, 4.0 * s, fg.with_alpha(30));
            }
            cx.icon_colored(icon, Rect::new(br.x + 6.0 * s, br.y + (br.h - 16.0 * s) / 2.0, 16.0 * s, 16.0 * s), if on { fg } else { fg.with_alpha(90) });
            self.hits.push((TitleHit::Quick(i), br));
            x += 30.0 * s;
        }
        if self.quick_menu {
            let br = Rect::new(x, r.y + 4.0 * s, 18.0 * s, r.h - 8.0 * s);
            if self.hover == Some(TitleHit::QuickMenu) {
                gfx::fill_round(cx.pm, br, 4.0 * s, fg.with_alpha(30));
            }
            cx.icon_colored("chevron-down", Rect::new(br.x + 3.0 * s, br.cy() - 6.0 * s, 12.0 * s, 12.0 * s), fg);
            self.hits.push((TitleHit::QuickMenu, br));
            x += 20.0 * s;
        }
        let bw = 46.0 * s;
        let close = Rect::new(r.right() - bw, r.y, bw, r.h);
        let max = Rect::new(close.x - bw, r.y, bw, r.h);
        let min = Rect::new(max.x - bw, r.y, bw, r.h);
        for (h, br, icon) in [
            (TitleHit::Min, min, "win-min"),
            (TitleHit::Max, max, if maximized { "win-restore" } else { "win-max" }),
            (TitleHit::Close, close, "win-close"),
        ] {
            let hov = self.hover == Some(h);
            if hov {
                let c = if h == TitleHit::Close { Color::hex(0xC42B1C) } else { fg.with_alpha(28) };
                gfx::fill_rect(cx.pm, br, c);
            }
            let ic = if hov && h == TitleHit::Close { Color::WHITE } else { fg };
            cx.icon_colored(icon, Rect::new(br.cx() - 6.0 * s, br.cy() - 6.0 * s, 12.0 * s, 12.0 * s), ic);
            self.hits.push((h, br));
        }
        let mut right = min.x;
        if let Some(mut f) = self.search.take() {
            let w = 220.0 * s;
            let sr = Rect::new((min.x - w - 10.0 * s).max(x + 20.0 * s), r.y + 5.0 * s, w, r.h - 10.0 * s);
            gfx::fill_round(cx.pm, sr, 4.0 * s, fg.with_alpha(if f.focused { 54 } else { 30 }));
            f.paint(cx, Rect::new(sr.x + 8.0 * s, sr.y, sr.w - 30.0 * s, sr.h));
            f.rect = sr;
            if f.text.is_empty() && !f.focused {
                cx.label("Search (Alt+Q)", Rect::new(sr.x + 8.0 * s, sr.y, sr.w - 30.0 * s, sr.h), 12.0, fg.with_alpha(150), false, Align::Left);
            }
            cx.icon_colored("search", Rect::new(sr.right() - 22.0 * s, sr.cy() - 7.0 * s, 14.0 * s, 14.0 * s), fg.with_alpha(190));
            self.hits.push((TitleHit::Search, sr));
            right = sr.x;
            self.search = Some(f);
        }
        let tr = Rect::new(x + 10.0 * s, r.y, (right - x - 20.0 * s).max(0.0), r.h);
        let tw = cx.measure(title, 13.0, false);
        let centre = Rect::new(r.x + (r.w - tw) / 2.0, r.y, tw, r.h);
        let tr = if centre.x > tr.x && centre.right() < tr.right() { centre } else { tr };
        cx.label(title, tr, 13.0, fg, false, Align::Left);
    }

    fn hit(&self, x: f32, y: f32) -> Option<TitleHit> {
        self.hits.iter().find(|(_, r)| r.contains(x, y)).map(|(h, _)| *h)
    }

    /// Put the cursor in the search box (Alt+Q).
    pub fn focus_search(&mut self) {
        if let Some(f) = self.search.as_mut() {
            f.focused = true;
            f.select_all();
        }
    }

    pub fn search_text(&self) -> String {
        self.search.as_ref().map(|f| f.text.clone()).unwrap_or_default()
    }

    /// Events the search box takes while it has the cursor. The caller passes the rest on.
    pub fn search_event(&mut self, ev: &Event, host: &mut dyn folio_platform::Host, text: &mut crate::text::Text, s: f32) -> Option<TitleAction> {
        let f = self.search.as_mut()?;
        if !f.focused {
            return None;
        }
        match f.event(ev, host, text, s) {
            FieldResult::Changed => Some(TitleAction::SearchChanged),
            FieldResult::Enter => Some(TitleAction::SearchEnter),
            FieldResult::Escape => {
                f.focused = false;
                f.set("");
                Some(TitleAction::SearchCancel)
            }
            FieldResult::None => None,
        }
    }

    pub fn event(&mut self, ev: &Event) -> (Option<TitleAction>, bool) {
        match ev {
            Event::MouseMove { x, y } => {
                let h = if self.rect.contains(*x, *y) { self.hit(*x, *y) } else { None };
                self.hover = h;
                (None, self.rect.contains(*x, *y))
            }
            Event::MouseLeave => {
                self.hover = None;
                (None, false)
            }
            Event::MouseDown { x, y, button: Button::Left, clicks } if self.rect.contains(*x, *y) => {
                match self.hit(*x, *y) {
                    Some(TitleHit::Search) => {
                        self.focus_search();
                        (None, true)
                    }
                    Some(h) => {
                        if let Some(f) = self.search.as_mut() {
                            f.focused = false;
                        }
                        self.pressed = Some(h);
                        (None, true)
                    }
                    None if *clicks == 2 => (Some(TitleAction::Maximize), true),
                    None => (Some(TitleAction::Drag), true),
                }
            }
            Event::MouseUp { x, y, button: Button::Left } => {
                let p = self.pressed.take();
                match p {
                    Some(p) if self.hit(*x, *y) == Some(p) => {
                        let a = match p {
                            TitleHit::Quick(i) => TitleAction::Command(self.quick[i].0.clone()),
                            TitleHit::QuickMenu => TitleAction::Command("quick.menu".into()),
                            TitleHit::Search => TitleAction::Drag,
                            TitleHit::Min => TitleAction::Minimize,
                            TitleHit::Max => TitleAction::Maximize,
                            TitleHit::Close => TitleAction::Close,
                        };
                        (Some(a), true)
                    }
                    Some(_) => (None, true),
                    None => (None, false),
                }
            }
            _ => (None, false),
        }
    }
}

/// Which window edge the pointer is over, for resizing a window without system borders.
pub fn resize_edge(x: f32, y: f32, w: f32, h: f32, s: f32, maximized: bool) -> Option<Edge> {
    if maximized {
        return None;
    }
    let b = 5.0 * s;
    let c = 14.0 * s;
    let (l, r, t, bo) = (x < b, x > w - b, y < b, y > h - b);
    let (lc, rc, tc, bc) = (x < c, x > w - c, y < c, y > h - c);
    match () {
        _ if (t && lc) || (l && tc) => Some(Edge::NW),
        _ if (t && rc) || (r && tc) => Some(Edge::NE),
        _ if (bo && lc) || (l && bc) => Some(Edge::SW),
        _ if (bo && rc) || (r && bc) => Some(Edge::SE),
        _ if t => Some(Edge::N),
        _ if bo => Some(Edge::S),
        _ if l => Some(Edge::W),
        _ if r => Some(Edge::E),
        _ => None,
    }
}

pub fn edge_cursor(e: Edge) -> folio_platform::Cursor {
    use folio_platform::Cursor as C;
    match e {
        Edge::N | Edge::S => C::ResizeNS,
        Edge::E | Edge::W => C::ResizeEW,
        Edge::NW | Edge::SE => C::ResizeNWSE,
        Edge::NE | Edge::SW => C::ResizeNESW,
    }
}
