//! Folio Graze: the window (B3-01), tabs (B3-02) and private-by-default browsing (B3-03).
//!
//! The app draws its own chrome (lime title bar with the tabs, then the toolbar with the
//! address bar, then the find bar when open) and the page fills the rest of the window.

use crate::nav;
use folio_kit::chrome::{TitleAction, TitleBar};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::text::{Align, Text};
use folio_kit::theme::{app_colors, Theme};
use folio_kit::ui::{Ctx, FieldResult, Menu, MenuItem, MenuResult, TextField};
use folio_platform::{App, Button, Cursor, Event, Host, Key, Mods, ViewId, WebCommand, WebEvent};
use std::time::Duration;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ACCENT: Color = Color::hex(0x4E8A1E);
const TITLE_H: f32 = 36.0;
const BAR_H: f32 = 42.0;
const FIND_H: f32 = 38.0;
const ZOOMS: &[f64] = &[0.3, 0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

#[derive(Clone, Debug)]
pub struct Tab {
    pub view: ViewId,
    pub title: String,
    pub uri: String,
    pub progress: f64,
    pub loading: bool,
    pub back: bool,
    pub forward: bool,
    pub zoom: f64,
    pub insecure: bool,
    pub found: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Tab(usize),
    TabClose(usize),
    NewTab,
    Back,
    Forward,
    Reload,
    Zoom,
    Menu,
    FindPrev,
    FindNext,
    FindClose,
}

pub struct GrazeApp {
    pub private: bool,
    pub tabs: Vec<Tab>,
    pub active: usize,
    /// Addresses of closed tabs, newest last (Ctrl+Shift+T).
    pub closed: Vec<String>,
    pub address: TextField,
    pub find: Option<TextField>,
    pub menu: Option<Menu>,
    pub theme: Theme,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    title: TitleBar,
    hits: Vec<(Hit, Rect)>,
    hover: Option<Hit>,
    pressed: Option<Hit>,
    drag_tab: Option<(usize, f32)>,
    text: Text,
    icons: Icons,
    mods: Mods,
    chrome_sent: Option<(u32, Vec<(u32, u32, u32, u32)>)>,
    pending_open: Vec<String>,
    started: bool,
    pub exit: bool,
    pub code: i32,
    pub test: Option<crate::uitest::UiTest>,
    pub last_script: Option<String>,
    pub spawned_private: u32,
    pub testing: bool,
    pub frame: Option<sk::Pixmap>,
}

impl GrazeApp {
    pub fn new(private: bool, open: Vec<String>) -> GrazeApp {
        let mut icons = Icons::new();
        crate::icons::load(&mut icons);
        GrazeApp {
            private,
            tabs: vec![],
            active: 0,
            closed: vec![],
            address: TextField::new(""),
            find: None,
            menu: None,
            theme: Theme::new(false, if private { Color::hex(0x5E8C2E) } else { app_colors::GRAZE }, ACCENT),
            w: 1200.0,
            h: 800.0,
            s: 1.0,
            title: TitleBar::new("graze", vec![]),
            hits: vec![],
            hover: None,
            pressed: None,
            drag_tab: None,
            text: Text::new(),
            icons,
            mods: Mods::NONE,
            chrome_sent: None,
            pending_open: open,
            started: false,
            exit: false,
            code: 0,
            test: None,
            last_script: None,
            spawned_private: 0,
            testing: false,
            frame: None,
        }
    }

    pub fn chrome_h(&self) -> f32 {
        (TITLE_H + BAR_H + if self.find.is_some() { FIND_H } else { 0.0 }) * self.s
    }

    pub fn tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    fn tab_of(&mut self, view: ViewId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.view == view)
    }

    // ------------------------------------------------------------------ tabs (B3-02)

    /// Open a tab after the current one (or at the end) and show it if `show`.
    pub fn new_tab(&mut self, uri: Option<&str>, show: bool, host: &mut dyn Host) {
        let Some(view) = host.web_new_view() else { return };
        let t = Tab { view, title: "New tab".into(), uri: uri.unwrap_or("about:blank").into(), progress: 0.0, loading: false, back: false, forward: false, zoom: 1.0, insecure: false, found: None };
        let at = if self.tabs.is_empty() { 0 } else { self.active + 1 };
        self.tabs.insert(at, t);
        if let Some(u) = uri {
            host.web(view, WebCommand::Load(u.to_string()));
        }
        if show || self.tabs.len() == 1 {
            self.select(at, host);
            if uri.is_none() {
                self.focus_address(host);
            }
        } else if at <= self.active && self.tabs.len() > 1 {
            // (inserted after the active one, so the active index stays)
        }
    }

    pub fn select(&mut self, i: usize, host: &mut dyn Host) {
        if i >= self.tabs.len() {
            return;
        }
        self.active = i;
        let t = self.tabs[i].clone();
        host.web_show(t.view);
        self.address.set(&nav::shown(&t.uri));
        self.address.focused = false;
        if let Some(f) = &self.find {
            if !f.text.is_empty() {
                host.web(t.view, WebCommand::Find(f.text.clone()));
            }
        }
        if t.uri != "about:blank" {
            host.web(t.view, WebCommand::Focus);
        }
        host.redraw();
    }

    pub fn close_tab(&mut self, i: usize, host: &mut dyn Host) {
        if i >= self.tabs.len() {
            return;
        }
        let t = self.tabs.remove(i);
        if t.uri != "about:blank" {
            self.closed.push(t.uri.clone());
            if self.closed.len() > 25 {
                self.closed.remove(0);
            }
        }
        host.web_close(t.view);
        if self.tabs.is_empty() {
            // Closing the last tab closes the window, as other browsers do.
            self.exit = true;
            host.close();
            return;
        }
        let next = if i < self.active || self.active >= self.tabs.len() { self.active.saturating_sub(1) } else { self.active };
        let next = next.min(self.tabs.len() - 1);
        self.select(next, host);
    }

    pub fn reopen_closed(&mut self, host: &mut dyn Host) {
        if let Some(u) = self.closed.pop() {
            self.new_tab(Some(&u), true, host);
        }
    }

    pub fn cycle(&mut self, d: i32, host: &mut dyn Host) {
        if self.tabs.is_empty() {
            return;
        }
        let n = self.tabs.len() as i32;
        let i = ((self.active as i32 + d) % n + n) % n;
        self.select(i as usize, host);
    }

    fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let t = self.tabs.remove(from);
        self.tabs.insert(to, t);
        if self.active == from {
            self.active = to;
        } else if from < self.active && to >= self.active {
            self.active -= 1;
        } else if from > self.active && to <= self.active {
            self.active += 1;
        }
    }

    // ------------------------------------------------------------------ navigation (B3-01)

    pub fn go(&mut self, input: &str, host: &mut dyn Host) {
        if input.trim().is_empty() {
            return;
        }
        let uri = nav::resolve(input);
        if self.tabs.is_empty() {
            self.new_tab(Some(&uri), true, host);
            return;
        }
        let i = self.active;
        self.tabs[i].uri = uri.clone();
        self.tabs[i].insecure = false;
        self.address.set(&uri);
        self.address.focused = false;
        let v = self.tabs[i].view;
        host.web(v, WebCommand::Load(uri));
        host.web(v, WebCommand::Focus);
    }

    fn cmd(&mut self, c: WebCommand, host: &mut dyn Host) {
        if let Some(t) = self.tab() {
            host.web(t.view, c);
        }
    }

    pub fn zoom_step(&mut self, d: i32, host: &mut dyn Host) {
        let Some(t) = self.tab() else { return };
        let z = t.zoom;
        let nz = if d == 0 {
            1.0
        } else if d > 0 {
            ZOOMS.iter().copied().find(|&v| v > z + 0.001).unwrap_or(z)
        } else {
            ZOOMS.iter().rev().copied().find(|&v| v < z - 0.001).unwrap_or(z)
        };
        let i = self.active;
        self.tabs[i].zoom = nz;
        self.cmd(WebCommand::Zoom(nz), host);
        host.redraw();
    }

    pub fn focus_address(&mut self, host: &mut dyn Host) {
        self.address.focused = true;
        self.address.select_all();
        if let Some(f) = self.find.as_mut() {
            f.focused = false;
        }
        host.focus_chrome();
        host.redraw();
    }

    pub fn open_find(&mut self, host: &mut dyn Host) {
        let mut f = self.find.take().unwrap_or_else(|| TextField::new(""));
        f.focused = true;
        f.select_all();
        self.address.focused = false;
        self.find = Some(f);
        host.focus_chrome();
        host.redraw();
    }

    pub fn close_find(&mut self, host: &mut dyn Host) {
        if self.find.take().is_some() {
            self.cmd(WebCommand::FindEnd, host);
            let i = self.active;
            if let Some(t) = self.tabs.get_mut(i) {
                t.found = None;
            }
            self.cmd(WebCommand::Focus, host);
            host.redraw();
        }
    }

    pub fn new_private_window(&mut self) {
        self.spawned_private += 1;
        if !self.testing {
            let _ = folio_platform::spawn_self(&["--private"]);
        }
    }

    fn open_menu(&mut self, anchor: Rect) {
        let can_reopen = !self.closed.is_empty();
        let items = vec![
            MenuItem::new("new-tab", "New tab").key("Ctrl+T"),
            MenuItem::new("private", "New private window").key("Ctrl+Shift+N"),
            MenuItem::new("reopen", "Reopen closed tab").key("Ctrl+Shift+T").off(!can_reopen),
            MenuItem::sep(),
            MenuItem::new("zoom-in", "Zoom in").key("Ctrl++"),
            MenuItem::new("zoom-out", "Zoom out").key("Ctrl+-"),
            MenuItem::new("zoom-reset", "Actual size").key("Ctrl+0"),
            MenuItem::sep(),
            MenuItem::new("find", "Find in page").key("Ctrl+F"),
            MenuItem::new("print", "Print").key("Ctrl+P"),
            MenuItem::sep(),
            MenuItem::new("about", &format!("Folio Graze {VERSION}")).off(true),
        ];
        let m = Menu::new("main", Rect::new(anchor.right() - 240.0 * self.s, anchor.y, 240.0 * self.s, anchor.h), items);
        self.menu = Some(m);
    }

    fn command(&mut self, id: &str, host: &mut dyn Host) {
        match id {
            "new-tab" => self.new_tab(None, true, host),
            "private" => self.new_private_window(),
            "reopen" => self.reopen_closed(host),
            "zoom-in" => self.zoom_step(1, host),
            "zoom-out" => self.zoom_step(-1, host),
            "zoom-reset" => self.zoom_step(0, host),
            "find" => self.open_find(host),
            "print" => self.cmd(WebCommand::Print, host),
            _ => {}
        }
        host.redraw();
    }

    /// Keyboard shortcuts; true when the key was one.
    fn shortcut(&mut self, key: &Key, m: &Mods, host: &mut dyn Host) -> bool {
        let c = m.cmd();
        match key {
            Key::Char('t') if c && m.shift => self.reopen_closed(host),
            Key::Char('t') if c => self.new_tab(None, true, host),
            Key::Char('w') if c => {
                let i = self.active;
                self.close_tab(i, host)
            }
            Key::F(4) if c => {
                let i = self.active;
                self.close_tab(i, host)
            }
            Key::Char('n') if c && m.shift => self.new_private_window(),
            Key::Tab if c => self.cycle(if m.shift { -1 } else { 1 }, host),
            Key::PageDown if c => self.cycle(1, host),
            Key::PageUp if c => self.cycle(-1, host),
            Key::Char(d @ '1'..='9') if c => {
                let n = *d as usize - '0' as usize;
                let i = if n == 9 { self.tabs.len().saturating_sub(1) } else { n - 1 };
                self.select(i, host)
            }
            Key::Char('l') if c => self.focus_address(host),
            Key::Char('d') if m.alt => self.focus_address(host),
            Key::F(6) => self.focus_address(host),
            Key::Char('r') if c => self.cmd(WebCommand::Reload, host),
            Key::F(5) => self.cmd(WebCommand::Reload, host),
            Key::Left if m.alt => self.cmd(WebCommand::Back, host),
            Key::Right if m.alt => self.cmd(WebCommand::Forward, host),
            Key::Char('=') | Key::Char('+') if c => self.zoom_step(1, host),
            Key::Char('-') if c => self.zoom_step(-1, host),
            Key::Char('0') if c => self.zoom_step(0, host),
            Key::Char('f') if c => self.open_find(host),
            Key::F(3) => {
                if self.find.is_none() {
                    self.open_find(host)
                } else {
                    self.cmd(if m.shift { WebCommand::FindPrevious } else { WebCommand::FindNext }, host)
                }
            }
            Key::Char('g') if c => self.cmd(if m.shift { WebCommand::FindPrevious } else { WebCommand::FindNext }, host),
            Key::Char('p') if c => self.cmd(WebCommand::Print, host),
            Key::Escape if self.find.is_some() && !self.address.focused => self.close_find(host),
            Key::Escape if self.tab().map(|t| t.loading).unwrap_or(false) && !self.address.focused => self.cmd(WebCommand::Stop, host),
            _ => return false,
        }
        true
    }

    fn web_event(&mut self, ev: &WebEvent, host: &mut dyn Host) {
        let active_view = self.tab().map(|t| t.view);
        match ev {
            WebEvent::Title { view, title } => {
                if let Some(t) = self.tab_of(*view) {
                    t.title = if title.trim().is_empty() { t.uri.clone() } else { title.clone() };
                }
            }
            WebEvent::Uri { view, uri } => {
                if let Some(t) = self.tab_of(*view) {
                    // The "not secure" page stands in for the plain address it explains.
                    if t.insecure && *uri == t.uri.replacen("http://", "https://", 1) {
                        host.redraw();
                        return;
                    }
                    t.insecure = false;
                    t.uri = uri.clone();
                }
                if Some(*view) == active_view && !self.address.focused {
                    self.address.set(&nav::shown(uri));
                }
            }
            WebEvent::Progress { view, value } => {
                if let Some(t) = self.tab_of(*view) {
                    t.progress = *value;
                }
            }
            WebEvent::Loading { view, loading } => {
                if let Some(t) = self.tab_of(*view) {
                    t.loading = *loading;
                }
            }
            WebEvent::History { view, back, forward } => {
                if let Some(t) = self.tab_of(*view) {
                    t.back = *back;
                    t.forward = *forward;
                }
            }
            WebEvent::OpenTab { uri, .. } => {
                // Links opened this way go in the background, after the current tab.
                self.new_tab(Some(uri), false, host);
            }
            WebEvent::Insecure { view, uri } => {
                if let Some(t) = self.tab_of(*view) {
                    t.insecure = true;
                    t.title = "Not secure".into();
                    t.uri = uri.clone();
                }
                if Some(*view) == active_view && !self.address.focused {
                    self.address.set(uri);
                }
            }
            WebEvent::Failed { view, uri, .. } => {
                if let Some(t) = self.tab_of(*view) {
                    if t.title == "New tab" {
                        t.title = uri.clone();
                    }
                }
            }
            WebEvent::Found { view, count } => {
                if let Some(t) = self.tab_of(*view) {
                    t.found = Some(*count);
                }
            }
            WebEvent::Zoom { view, level } => {
                if let Some(t) = self.tab_of(*view) {
                    t.zoom = *level;
                }
            }
            WebEvent::PageFocus { .. } => {
                self.address.focused = false;
                if let Some(f) = self.find.as_mut() {
                    f.focused = false;
                }
                if let Some(t) = self.tab() {
                    let u = nav::shown(&t.uri);
                    self.address.set(&u);
                }
            }
            WebEvent::ScriptResult { value, .. } => self.last_script = Some(value.clone()),
        }
        host.redraw();
    }

    fn hit(&self, x: f32, y: f32) -> Option<Hit> {
        // Close buttons sit on top of their tabs.
        let mut best = None;
        for (h, r) in &self.hits {
            if r.contains(x, y) {
                match h {
                    Hit::TabClose(_) => return Some(*h),
                    _ => best = Some(*h),
                }
            }
        }
        best
    }

    fn mouse(&mut self, ev: &Event, host: &mut dyn Host) {
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    self.menu = None;
                    self.command(&id, host);
                    return;
                }
                MenuResult::Closed => {
                    self.menu = None;
                    host.redraw();
                    return;
                }
                MenuResult::None => {
                    if matches!(ev, Event::MouseMove { .. } | Event::MouseUp { .. }) || m.contains(mouse_xy(ev).0, mouse_xy(ev).1) {
                        host.redraw();
                        return;
                    }
                }
            }
        }
        match ev {
            Event::MouseMove { x, y } => {
                if let Some((from, grab_dx)) = self.drag_tab {
                    // Reorder by dragging: the tab moves over the one under the pointer.
                    if let Some(Hit::Tab(to)) = self.hit(*x - grab_dx * 0.0, (TITLE_H * self.s) / 2.0) {
                        if to != from {
                            self.move_tab(from, to);
                            self.drag_tab = Some((to, grab_dx));
                        }
                    }
                }
                let h = self.hit(*x, *y);
                if h != self.hover {
                    self.hover = h;
                    host.redraw();
                }
                let (_, used) = self.title.event(ev);
                if used {
                    host.redraw();
                }
                let over_field = self.address.rect.contains(*x, *y) || self.find.as_ref().map(|f| f.rect.contains(*x, *y)).unwrap_or(false);
                host.set_cursor(if over_field { Cursor::Text } else { Cursor::Default });
            }
            Event::MouseDown { x, y, button, clicks } => {
                let h = self.hit(*x, *y);
                // Text fields.
                if self.address.rect.contains(*x, *y) && *button == Button::Left {
                    let first = !self.address.focused;
                    self.address.event(ev, host, &mut self.text, self.s);
                    if first {
                        self.address.select_all();
                    }
                    self.address.focused = true;
                    if let Some(f) = self.find.as_mut() {
                        f.focused = false;
                    }
                    host.focus_chrome();
                    host.redraw();
                    return;
                }
                if let Some(f) = self.find.as_mut() {
                    if f.rect.contains(*x, *y) && *button == Button::Left {
                        f.event(ev, host, &mut self.text, self.s);
                        f.focused = true;
                        self.address.focused = false;
                        host.focus_chrome();
                        host.redraw();
                        return;
                    }
                }
                match (h, button) {
                    (Some(Hit::Tab(i)), Button::Middle) | (Some(Hit::TabClose(i)), Button::Middle) => {
                        self.close_tab(i, host);
                        return;
                    }
                    (Some(Hit::Tab(i)), Button::Left) => {
                        self.select(i, host);
                        self.drag_tab = Some((i, 0.0));
                        return;
                    }
                    (Some(h), Button::Left) => {
                        self.pressed = Some(h);
                        host.redraw();
                        return;
                    }
                    (None, Button::Left) if *y < TITLE_H * self.s && *clicks == 2 => {
                        host.toggle_maximize();
                        return;
                    }
                    _ => {}
                }
                match self.title.event(ev) {
                    (Some(TitleAction::Drag), _) => host.drag_window(),
                    (Some(TitleAction::Maximize), _) => host.toggle_maximize(),
                    _ => {}
                }
                host.redraw();
            }
            Event::MouseUp { x, y, .. } => {
                self.drag_tab = None;
                if let Some(p) = self.pressed.take() {
                    if self.hit(*x, *y) == Some(p) {
                        self.click(p, host);
                    }
                    host.redraw();
                    return;
                }
                match self.title.event(ev) {
                    (Some(TitleAction::Minimize), _) => host.minimize(),
                    (Some(TitleAction::Maximize), _) => host.toggle_maximize(),
                    (Some(TitleAction::Close), _) => {
                        self.exit = true;
                        host.close();
                    }
                    _ => {}
                }
                host.redraw();
            }
            Event::MouseLeave => {
                self.hover = None;
                self.title.event(ev);
                host.redraw();
            }
            _ => {}
        }
    }

    fn click(&mut self, h: Hit, host: &mut dyn Host) {
        match h {
            Hit::TabClose(i) => self.close_tab(i, host),
            Hit::NewTab => self.new_tab(None, true, host),
            Hit::Back => self.cmd(WebCommand::Back, host),
            Hit::Forward => self.cmd(WebCommand::Forward, host),
            Hit::Reload => {
                let loading = self.tab().map(|t| t.loading).unwrap_or(false);
                self.cmd(if loading { WebCommand::Stop } else { WebCommand::Reload }, host)
            }
            Hit::Zoom => self.zoom_step(0, host),
            Hit::Menu => {
                let r = self.hits.iter().find(|(x, _)| *x == Hit::Menu).map(|(_, r)| *r).unwrap_or_default();
                self.open_menu(r);
            }
            Hit::FindPrev => self.cmd(WebCommand::FindPrevious, host),
            Hit::FindNext => self.cmd(WebCommand::FindNext, host),
            Hit::FindClose => self.close_find(host),
            Hit::Tab(_) => {}
        }
    }

    fn key(&mut self, ev: &Event, host: &mut dyn Host) {
        let Event::Key { key, mods, down, .. } = ev else { return };
        if !*down {
            if self.address.focused || self.find.as_ref().map(|f| f.focused).unwrap_or(false) {
                host.set_handled();
            }
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    self.menu = None;
                    self.command(&id, host);
                }
                MenuResult::Closed => self.menu = None,
                MenuResult::None => {}
            }
            host.set_handled();
            host.redraw();
            return;
        }
        if self.shortcut(key, mods, host) {
            host.set_handled();
            host.redraw();
            return;
        }
        if self.address.focused {
            host.set_handled();
            match self.address.event(ev, host, &mut self.text, self.s) {
                FieldResult::Enter => {
                    let t = self.address.text.clone();
                    self.go(&t, host);
                }
                FieldResult::Escape => {
                    let u = self.tab().map(|t| nav::shown(&t.uri)).unwrap_or_default();
                    self.address.set(&u);
                    self.address.focused = false;
                    self.cmd(WebCommand::Focus, host);
                }
                _ => {}
            }
            host.redraw();
            return;
        }
        if let Some(f) = self.find.as_mut() {
            if f.focused {
                host.set_handled();
                let r = f.event(ev, host, &mut self.text, self.s);
                let text = f.text.clone();
                match r {
                    FieldResult::Changed => self.cmd(WebCommand::Find(text), host),
                    FieldResult::Enter => self.cmd(if mods.shift { WebCommand::FindPrevious } else { WebCommand::FindNext }, host),
                    FieldResult::Escape => self.close_find(host),
                    _ => {}
                }
                host.redraw();
            }
        }
    }

    fn text_input(&mut self, ev: &Event, host: &mut dyn Host) {
        if self.address.focused {
            self.address.event(ev, host, &mut self.text, self.s);
        } else if let Some(f) = self.find.as_mut() {
            if f.focused {
                if let FieldResult::Changed = f.event(ev, host, &mut self.text, self.s) {
                    let t = f.text.clone();
                    self.cmd(WebCommand::Find(t), host);
                }
            }
        }
        host.redraw();
    }

    // ------------------------------------------------------------------ drawing

    pub fn draw(&mut self, pm: &mut sk::Pixmap) {
        let s = self.s;
        let t = self.theme.clone();
        let w = self.w;
        self.hits.clear();
        {
            let mut cx = Ctx { pm, text: &mut self.text, icons: &mut self.icons, theme: &t, s };
            // Title bar (app colour) with the tabs in it.
            let tr = Rect::new(0.0, 0.0, w, TITLE_H * s);
            self.title.paint(&mut cx, tr, "", false, &|_| true);
            let fg = t.app_text;
            let x0 = 42.0 * s;
            let x1 = w - 3.0 * 46.0 * s - 44.0 * s;
            let n = self.tabs.len().max(1) as f32;
            let tw = ((x1 - x0) / n).clamp(80.0 * s, 230.0 * s);
            let mut x = x0;
            for (i, tab) in self.tabs.iter().enumerate() {
                let r = Rect::new(x, 5.0 * s, tw - 2.0 * s, TITLE_H * s - 5.0 * s);
                if r.right() > x1 + 1.0 {
                    break;
                }
                let active = i == self.active;
                if active {
                    gfx::fill_round(cx.pm, Rect::new(r.x, r.y, r.w, r.h + 6.0 * s), 7.0 * s, t.panel);
                } else if self.hover == Some(Hit::Tab(i)) || self.hover == Some(Hit::TabClose(i)) {
                    gfx::fill_round(cx.pm, r, 7.0 * s, fg.with_alpha(28));
                }
                let col = if active { t.text } else { fg };
                // Loading shows as a small bar under the title; HTTPS-only stops as a warning.
                let icon = if tab.insecure { "warn" } else if tab.loading { "loading" } else { "page" };
                cx.icon_colored(icon, Rect::new(r.x + 8.0 * s, r.y + (r.h - 16.0 * s) / 2.0, 16.0 * s, 16.0 * s), col);
                let lr = Rect::new(r.x + 30.0 * s, r.y, r.w - 58.0 * s, r.h);
                let label = if tab.title.is_empty() { "New tab" } else { tab.title.as_str() };
                let shown = ellipsize(cx.text, label, lr.w, 12.5 * s);
                cx.label(&shown, lr, 12.5, col, false, Align::Left);
                let cr = Rect::new(r.right() - 24.0 * s, r.y + (r.h - 18.0 * s) / 2.0, 18.0 * s, 18.0 * s);
                if self.hover == Some(Hit::TabClose(i)) {
                    gfx::fill_round(cx.pm, cr, 9.0 * s, col.with_alpha(40));
                }
                cx.icon_colored("x", Rect::new(cr.x + 3.0 * s, cr.y + 3.0 * s, 12.0 * s, 12.0 * s), col);
                self.hits.push((Hit::Tab(i), r));
                self.hits.push((Hit::TabClose(i), cr));
                x += tw;
            }
            let nr = Rect::new(x + 2.0 * s, 7.0 * s, 26.0 * s, 24.0 * s);
            if self.hover == Some(Hit::NewTab) {
                gfx::fill_round(cx.pm, nr, 5.0 * s, fg.with_alpha(30));
            }
            cx.icon_colored("plus", Rect::new(nr.x + 5.0 * s, nr.y + 4.0 * s, 16.0 * s, 16.0 * s), fg);
            self.hits.push((Hit::NewTab, nr));
            if self.private {
                let pr = Rect::new(x1 - 70.0 * s, 0.0, 70.0 * s, TITLE_H * s);
                cx.label("Private", pr, 12.0, fg, true, Align::Right);
            }

            // Toolbar.
            let br = Rect::new(0.0, TITLE_H * s, w, BAR_H * s);
            gfx::fill_rect(cx.pm, br, t.panel);
            let tab = self.tabs.get(self.active).cloned();
            let mut bx = 8.0 * s;
            let by = br.y + 5.0 * s;
            let bs = 32.0 * s;
            let loading = tab.as_ref().map(|t| t.loading).unwrap_or(false);
            for (h, icon, on) in [
                (Hit::Back, "back", tab.as_ref().map(|t| t.back).unwrap_or(false)),
                (Hit::Forward, "forward", tab.as_ref().map(|t| t.forward).unwrap_or(false)),
                (Hit::Reload, if loading { "stop" } else { "reload" }, tab.is_some()),
            ] {
                let r = Rect::new(bx, by, bs, bs);
                if on && (self.hover == Some(h) || self.pressed == Some(h)) {
                    gfx::fill_round(cx.pm, r, 5.0 * s, if self.pressed == Some(h) { t.pressed } else { t.hover });
                }
                cx.icon_colored(icon, Rect::new(r.x + 7.0 * s, r.y + 7.0 * s, 18.0 * s, 18.0 * s), if on { t.text } else { t.muted.with_alpha(120) });
                if on {
                    self.hits.push((h, r));
                }
                bx += bs + 2.0 * s;
            }
            let menu_r = Rect::new(w - 8.0 * s - bs, by, bs, bs);
            let zoom = tab.as_ref().map(|t| t.zoom).unwrap_or(1.0);
            let mut ax1 = menu_r.x - 6.0 * s;
            if (zoom - 1.0).abs() > 0.001 {
                let zr = Rect::new(ax1 - 56.0 * s, by + 4.0 * s, 54.0 * s, bs - 8.0 * s);
                gfx::fill_round(cx.pm, zr, 4.0 * s, if self.hover == Some(Hit::Zoom) { t.hover } else { t.window });
                cx.label(&format!("{:.0}%", zoom * 100.0), zr, 12.0, t.text, false, Align::Center);
                self.hits.push((Hit::Zoom, zr));
                ax1 = zr.x - 6.0 * s;
            }
            let ar = Rect::new(bx + 6.0 * s, by + 1.0 * s, ax1 - bx - 6.0 * s, bs - 2.0 * s);
            let secure = tab.as_ref().map(|t| t.uri.starts_with("https://")).unwrap_or(false);
            self.address.paint(&mut cx, Rect::new(ar.x + 24.0 * s, ar.y, ar.w - 24.0 * s, ar.h));
            let lock = if tab.as_ref().map(|t| t.insecure).unwrap_or(false) { "warn" } else if secure { "lock" } else { "search" };
            cx.icon_colored(lock, Rect::new(ar.x + 2.0 * s, ar.y + (ar.h - 16.0 * s) / 2.0, 16.0 * s, 16.0 * s), if secure { ACCENT } else { t.muted });
            if self.address.text.is_empty() && !self.address.focused {
                cx.label("Search or type an address", Rect::new(ar.x + 32.0 * s, ar.y, ar.w, ar.h), 13.0, t.muted, false, Align::Left);
            }
            if self.hover == Some(Hit::Menu) || self.menu.is_some() {
                gfx::fill_round(cx.pm, menu_r, 5.0 * s, t.hover);
            }
            cx.icon_colored("menu", Rect::new(menu_r.x + 7.0 * s, menu_r.y + 7.0 * s, 18.0 * s, 18.0 * s), t.text);
            self.hits.push((Hit::Menu, menu_r));
            // Loading progress along the bottom of the toolbar.
            if let Some(tab) = &tab {
                if tab.loading {
                    let pw = (w * tab.progress.clamp(0.05, 1.0) as f32).round();
                    gfx::fill_rect(cx.pm, Rect::new(0.0, br.bottom() - 2.0 * s, pw, 2.0 * s), ACCENT);
                }
            }
            gfx::fill_rect(cx.pm, Rect::new(0.0, br.bottom() - 1.0, w, 1.0), t.border);

            // Find bar.
            if self.find.is_some() {
                let fr = Rect::new(0.0, br.bottom(), w, FIND_H * s);
                gfx::fill_rect(cx.pm, fr, t.panel);
                gfx::fill_rect(cx.pm, Rect::new(0.0, fr.bottom() - 1.0, w, 1.0), t.border);
                let field = Rect::new(12.0 * s, fr.y + 5.0 * s, 260.0 * s, fr.h - 10.0 * s);
                if let Some(f) = self.find.as_mut() {
                    f.paint(&mut cx, field);
                    if f.text.is_empty() && !f.focused {
                        cx.label("Find in page", Rect::new(field.x + 8.0 * s, field.y, field.w, field.h), 13.0, t.muted, false, Align::Left);
                    }
                }
                let found = tab.as_ref().and_then(|t| t.found);
                let txt = match (found, self.find.as_ref().map(|f| f.text.is_empty()).unwrap_or(true)) {
                    (_, true) => String::new(),
                    (Some(0), false) => "No matches".into(),
                    (Some(1), false) => "1 match".into(),
                    (Some(n), false) => format!("{n} matches"),
                    (None, false) => String::new(),
                };
                let mut fx = field.right() + 8.0 * s;
                for (h, icon) in [(Hit::FindPrev, "up"), (Hit::FindNext, "down")] {
                    let r = Rect::new(fx, fr.y + 5.0 * s, 28.0 * s, 28.0 * s);
                    if self.hover == Some(h) {
                        gfx::fill_round(cx.pm, r, 4.0 * s, t.hover);
                    }
                    cx.icon_colored(icon, Rect::new(r.x + 6.0 * s, r.y + 6.0 * s, 16.0 * s, 16.0 * s), t.text);
                    self.hits.push((h, r));
                    fx += 30.0 * s;
                }
                cx.label(&txt, Rect::new(fx + 8.0 * s, fr.y, 200.0 * s, fr.h), 12.5, if found == Some(0) { t.danger } else { t.muted }, false, Align::Left);
                let cr = Rect::new(w - 40.0 * s, fr.y + 5.0 * s, 28.0 * s, 28.0 * s);
                if self.hover == Some(Hit::FindClose) {
                    gfx::fill_round(cx.pm, cr, 4.0 * s, t.hover);
                }
                cx.icon_colored("x", Rect::new(cr.x + 7.0 * s, cr.y + 7.0 * s, 14.0 * s, 14.0 * s), t.text);
                self.hits.push((Hit::FindClose, cr));
            }
            // New-tab page hint when the tab is empty.
            if let Some(m) = self.menu.as_mut() {
                m.paint(&mut cx, Rect::new(0.0, 0.0, w, self.h));
            }
        }
    }

    fn input_rects(&self) -> Vec<(u32, u32, u32, u32)> {
        let mut v = vec![(0, 0, self.w as u32, self.chrome_h().ceil() as u32)];
        if self.menu.is_some() {
            // The open menu (and everything else, so a click outside it closes it).
            v = vec![(0, 0, self.w as u32, self.h as u32)];
        }
        v
    }
}

fn mouse_xy(ev: &Event) -> (f32, f32) {
    match ev {
        Event::MouseMove { x, y } | Event::MouseDown { x, y, .. } | Event::MouseUp { x, y, .. } => (*x, *y),
        _ => (-1.0, -1.0),
    }
}

fn ellipsize(text: &mut Text, s: &str, w: f32, size: f32) -> String {
    let st = folio_kit::text::TextStyle { size, ..Default::default() };
    if text.layout(s, &st, None).width <= w {
        return s.to_string();
    }
    let mut out: String = s.chars().collect();
    while !out.is_empty() {
        out.pop();
        let c = format!("{}…", out.trim_end());
        if text.layout(&c, &st, None).width <= w {
            return c;
        }
    }
    String::new()
}

impl App for GrazeApp {
    fn event(&mut self, ev: &Event, host: &mut dyn Host) {
        match ev {
            Event::Resized { w, h, scale } => {
                self.w = *w as f32;
                self.h = *h as f32;
                self.s = scale.max(0.5);
                if !self.started {
                    self.started = true;
                    let dark = host.system_dark().unwrap_or(false);
                    self.theme = Theme::new(dark, self.theme.app, ACCENT);
                    let open = std::mem::take(&mut self.pending_open);
                    if open.is_empty() {
                        self.new_tab(None, true, host);
                    }
                    for (i, u) in open.iter().enumerate() {
                        let uri = nav::resolve(u);
                        self.new_tab(Some(&uri), i == 0, host);
                    }
                }
            }
            Event::ThemeChanged { dark } => self.theme = Theme::new(*dark, self.theme.app, ACCENT),
            Event::Modifiers(m) => self.mods = *m,
            Event::Key { .. } => self.key(ev, host),
            Event::Text(_) => self.text_input(ev, host),
            Event::Web(w) => self.web_event(w, host),
            Event::Tick => {
                if let Some(mut t) = self.test.take() {
                    t.step(self, host);
                    if t.done {
                        self.code = t.code();
                        self.exit = true;
                        host.close();
                    } else {
                        self.test = Some(t);
                    }
                }
            }
            Event::CloseRequested => {
                self.exit = true;
                host.close();
            }
            Event::FileDropped(p) => {
                let u = format!("file://{}", p.display());
                self.new_tab(Some(&u), true, host);
            }
            _ => self.mouse(ev, host),
        }
        let title = self.tab().map(|t| t.title.clone()).filter(|t| !t.is_empty() && t != "New tab");
        host.set_title(&match title {
            Some(t) => format!("{t} - Folio Graze"),
            None => "Folio Graze".into(),
        });
        let rects = self.input_rects();
        let ch = self.chrome_h().ceil() as u32;
        if self.chrome_sent.as_ref() != Some(&(ch, rects.clone())) {
            host.set_chrome(ch, &rects);
            self.chrome_sent = Some((ch, rects));
        }
        host.redraw();
    }

    fn paint(&mut self, pixels: &mut [u8], w: u32, h: u32, _host: &mut dyn Host) {
        self.w = w as f32;
        self.h = h as f32;
        let mut pm = match self.frame.take() {
            Some(p) if p.width() == w && p.height() == h => p,
            _ => sk::Pixmap::new(w.max(1), h.max(1)).unwrap(),
        };
        pm.fill(sk::Color::TRANSPARENT);
        self.draw(&mut pm);
        pixels.copy_from_slice(pm.data());
        self.frame = Some(pm);
    }

    fn next_tick(&self) -> Option<Duration> {
        if self.test.is_some() {
            return Some(Duration::from_millis(100));
        }
        None
    }

    fn wants_exit(&self) -> bool {
        self.exit
    }

    fn exit_code(&self) -> i32 {
        self.code
    }
}
