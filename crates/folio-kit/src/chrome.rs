//! Window chrome: the title bar in the app colour, window buttons, resize edges, status bar.

use crate::gfx::{self, Color, Rect};
use crate::text::Align;
use crate::ui::Ctx;
use folio_platform::{Button, Edge, Event};

#[derive(Clone, Debug, PartialEq)]
pub enum TitleAction {
    Command(String),
    Minimize,
    Maximize,
    Close,
    Drag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TitleHit {
    Quick(usize),
    Min,
    Max,
    Close,
}

pub struct TitleBar {
    /// Quick-access buttons: (command id, icon, tooltip).
    pub quick: Vec<(String, String, String)>,
    pub app_icon: String,
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
            rect: Rect::default(),
            hits: vec![],
            hover: None,
            pressed: None,
        }
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
        let tr = Rect::new(x + 10.0 * s, r.y, min.x - x - 20.0 * s, r.h);
        let tw = cx.measure(title, 13.0, false);
        let centre = Rect::new(r.x + (r.w - tw) / 2.0, r.y, tw, r.h);
        let tr = if centre.x > tr.x && centre.right() < tr.right() { centre } else { tr };
        cx.label(title, tr, 13.0, fg, false, Align::Left);
    }

    fn hit(&self, x: f32, y: f32) -> Option<TitleHit> {
        self.hits.iter().find(|(_, r)| r.contains(x, y)).map(|(h, _)| *h)
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
                    Some(h) => {
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
