//! The web backend (X3-13, for Folio Graze): one window holding web views, with the app's own
//! drawing (title bar, tabs, toolbar, menus) laid over the top of them. Linux uses WebKitGTK
//! inside a GTK window; Windows (WebView2) and macOS (WKWebView) come through this same API.
//!
//! The app sees the usual pixel buffer and events for the whole window. The top rows it names
//! in `Host::set_chrome` (its chrome) are shown above the page. While it needs more than that (a
//! menu open over the page), the window shows all of its pixels over a still picture of the
//! page, so it paints only its chrome and menus and leaves the rest clear.

use crate::{host_of, is_local_host, App, Button, ClickCounter, Cursor, Edge, Event, Host, Image, Key, Mods, ViewId, WebCommand, WebEvent, WebPolicy, WindowSpec};
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::time::{Duration, Instant};
use webkit2gtk::{CookieManagerExt, FindControllerExt, NavigationPolicyDecisionExt, PolicyDecisionExt, PrintOperationExt, SettingsExt, URIRequestExt, WebContextExt, WebViewExt, WebsiteDataManagerExt};
use webkit2gtk::{CookieAcceptPolicy, CookiePersistentStorage, LoadEvent, NavigationPolicyDecision, PolicyDecisionType, WebContext, WebView, WebsiteDataManager};

/// What the app asked for during an event, carried out once the app is no longer borrowed (web
/// views send signals back synchronously).
enum Op {
    NewView(ViewId),
    Show(ViewId),
    Close(ViewId),
    Web(ViewId, WebCommand),
    Chrome(u32, Vec<(u32, u32, u32, u32)>),
    FocusChrome,
    Title(String),
    Cursor(Cursor),
    DragWindow,
    DragResize(Edge),
    Minimize,
    ToggleMaximize,
    Close_,
}

struct WebHostImpl {
    ops: Vec<Op>,
    next_view: ViewId,
    start: Instant,
    maximized: bool,
    handled: bool,
    redraw: bool,
}

impl Host for WebHostImpl {
    fn redraw(&mut self) {
        self.redraw = true;
    }
    fn set_title(&mut self, title: &str) {
        self.ops.push(Op::Title(title.to_string()));
    }
    fn set_cursor(&mut self, cursor: Cursor) {
        self.ops.push(Op::Cursor(cursor));
    }
    fn drag_window(&mut self) {
        self.ops.push(Op::DragWindow);
    }
    fn drag_resize(&mut self, edge: Edge) {
        self.ops.push(Op::DragResize(edge));
    }
    fn minimize(&mut self) {
        self.ops.push(Op::Minimize);
    }
    fn toggle_maximize(&mut self) {
        self.ops.push(Op::ToggleMaximize);
    }
    fn is_maximized(&self) -> bool {
        self.maximized
    }
    fn close(&mut self) {
        self.ops.push(Op::Close_);
    }
    fn system_dark(&self) -> Option<bool> {
        crate::backend::linux_dark_preference()
    }
    fn clipboard_text(&mut self) -> Option<String> {
        gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).wait_for_text().map(|s| s.to_string())
    }
    fn set_clipboard_text(&mut self, text: &str) {
        gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).set_text(text);
    }
    fn clipboard_image(&mut self) -> Option<Image> {
        let pb = gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).wait_for_image()?;
        let pb = if pb.has_alpha() { pb } else { pb.add_alpha(false, 0, 0, 0).ok()? };
        let (w, h, stride) = (pb.width() as usize, pb.height() as usize, pb.rowstride() as usize);
        let bytes = pb.read_pixel_bytes();
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            rgba.extend_from_slice(&bytes[y * stride..y * stride + w * 4]);
        }
        Some(Image { w: w as u32, h: h as u32, rgba })
    }
    fn set_clipboard_image(&mut self, img: &Image) {
        let pb = gtk::gdk_pixbuf::Pixbuf::from_bytes(&glib::Bytes::from(&img.rgba), gtk::gdk_pixbuf::Colorspace::Rgb, true, 8, img.w as i32, img.h as i32, img.w as i32 * 4);
        gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD).set_image(&pb);
    }
    fn now(&self) -> Duration {
        self.start.elapsed()
    }
    fn web_new_view(&mut self) -> Option<ViewId> {
        self.next_view += 1;
        self.ops.push(Op::NewView(self.next_view));
        Some(self.next_view)
    }
    fn web_show(&mut self, view: ViewId) {
        self.ops.push(Op::Show(view));
    }
    fn web_close(&mut self, view: ViewId) {
        self.ops.push(Op::Close(view));
    }
    fn web(&mut self, view: ViewId, cmd: WebCommand) {
        self.ops.push(Op::Web(view, cmd));
    }
    fn set_chrome(&mut self, height: u32, input: &[(u32, u32, u32, u32)]) {
        self.ops.push(Op::Chrome(height, input.to_vec()));
    }
    fn focus_chrome(&mut self) {
        self.ops.push(Op::FocusChrome);
    }
    fn set_handled(&mut self) {
        self.handled = true;
    }
}

struct Shared {
    app: Box<dyn App>,
    host: WebHostImpl,
}

/// Everything GTK-side, shared by the signal handlers.
struct Ui {
    window: gtk::Window,
    area: gtk::DrawingArea,
    /// The whole window, shown instead while a menu covers the page.
    cover: gtk::DrawingArea,
    outer: gtk::Stack,
    cover_img: RefCell<Option<gtk::cairo::ImageSurface>>,
    covering: RefCell<bool>,
    visible_view: RefCell<Option<ViewId>>,
    stack: gtk::Stack,
    im: gtk::IMMulticontext,
    ctx: WebContext,
    settings: webkit2gtk::Settings,
    views: RefCell<HashMap<ViewId, WebView>>,
    policy: WebPolicy,
    /// Hosts the user chose to open without encryption (this window only).
    insecure_ok: RefCell<HashSet<String>>,
    /// Pages HTTPS-only upgraded, so a failure can explain itself.
    upgraded: RefCell<HashSet<String>>,
    shared: RefCell<Shared>,
    queue: RefCell<VecDeque<Event>>,
    pixels: RefCell<Vec<u8>>,
    size: RefCell<(u32, u32, f32)>,
    mods: RefCell<Mods>,
    clicks: RefCell<ClickCounter>,
    last_press: RefCell<Option<(u32, (f64, f64), u32)>>,
    tick_at: RefCell<Option<Instant>>,
    chrome_h: RefCell<u32>,
    exit: RefCell<Option<i32>>,
}

fn key_of(k: &gdk::keys::Key) -> Key {
    use gdk::keys::constants as K;
    match k.clone() {
        K::Return | K::KP_Enter | K::ISO_Enter => Key::Enter,
        K::Escape => Key::Escape,
        K::BackSpace => Key::Backspace,
        K::Delete | K::KP_Delete => Key::Delete,
        K::Tab | K::ISO_Left_Tab => Key::Tab,
        K::space => Key::Space,
        K::Left | K::KP_Left => Key::Left,
        K::Right | K::KP_Right => Key::Right,
        K::Up | K::KP_Up => Key::Up,
        K::Down | K::KP_Down => Key::Down,
        K::Home | K::KP_Home => Key::Home,
        K::End | K::KP_End => Key::End,
        K::Page_Up | K::KP_Page_Up => Key::PageUp,
        K::Page_Down | K::KP_Page_Down => Key::PageDown,
        K::Insert | K::KP_Insert => Key::Insert,
        K::Alt_L | K::Alt_R => Key::Alt,
        K::Control_L | K::Control_R => Key::Ctrl,
        K::Shift_L | K::Shift_R => Key::Shift,
        K::Super_L | K::Super_R => Key::Logo,
        _ => {
            let name = k.name().map(|n| n.to_string()).unwrap_or_default();
            if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
                return Key::F(n);
            }
            match k.to_lower().to_unicode() {
                Some(c) if !c.is_control() => Key::Char(c.to_lowercase().next().unwrap_or(c)),
                _ => Key::Other,
            }
        }
    }
}

fn mods_of(m: gdk::ModifierType) -> Mods {
    Mods { ctrl: m.contains(gdk::ModifierType::CONTROL_MASK), shift: m.contains(gdk::ModifierType::SHIFT_MASK), alt: m.contains(gdk::ModifierType::MOD1_MASK), logo: m.contains(gdk::ModifierType::SUPER_MASK) }
}

fn cursor_name(c: Cursor) -> &'static str {
    match c {
        Cursor::Default => "default",
        Cursor::Pointer => "pointer",
        Cursor::Text => "text",
        Cursor::Move => "move",
        Cursor::Grab => "grab",
        Cursor::Grabbing => "grabbing",
        Cursor::Crosshair => "crosshair",
        Cursor::ResizeNS => "ns-resize",
        Cursor::ResizeEW => "ew-resize",
        Cursor::ResizeNWSE => "nwse-resize",
        Cursor::ResizeNESW => "nesw-resize",
        Cursor::NotAllowed => "not-allowed",
    }
}

fn insecure_page(uri: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
    let plain = uri.replacen("https://", "http://", 1);
    let sep = if plain.contains('#') { "&" } else { "#" };
    format!(
        "<!doctype html><meta charset=utf-8><title>Not secure</title><style>body{{font:15px sans-serif;max-width:560px;margin:12vh auto;color:#333}}h1{{font-size:22px}}a{{color:#3a6b00}}</style>\
<h1>This site doesn't offer a secure connection</h1><p>Folio Graze only opens encrypted pages, and <b>{}</b> didn't answer over HTTPS.</p>\
<p>Someone on the network could read or change what you see and send there.</p><p><a href=\"{}{}folio-allow-insecure\">Continue to the site anyway</a></p>",
        esc(&host_of(uri).unwrap_or_default()),
        esc(&plain),
        sep
    )
}

impl Ui {
    /// Hand an event to the app, now if it's free, else after the current one.
    fn send(self: &Rc<Self>, ev: Event) -> bool {
        self.queue.borrow_mut().push_back(ev);
        self.pump()
    }

    /// Deliver queued events; returns whether the last key event was handled by the app.
    fn pump(self: &Rc<Self>) -> bool {
        let mut handled = false;
        loop {
            let Some(ev) = self.queue.borrow_mut().pop_front() else { break };
            let ops = {
                let Ok(mut sh) = self.shared.try_borrow_mut() else {
                    // Called from inside the app (a signal during one of its requests): later.
                    self.queue.borrow_mut().push_front(ev);
                    let me = self.clone();
                    glib::idle_add_local_once(move || {
                        me.pump();
                    });
                    return false;
                };
                let sh = &mut *sh;
                sh.host.handled = false;
                sh.host.maximized = self.window.is_maximized();
                sh.app.event(&ev, &mut sh.host);
                handled = sh.host.handled;
                if sh.app.wants_exit() {
                    *self.exit.borrow_mut() = Some(sh.app.exit_code());
                }
                std::mem::take(&mut sh.host.ops)
            };
            self.run_ops(ops);
            self.after_app();
        }
        handled
    }

    fn after_app(self: &Rc<Self>) {
        if let Some(code) = *self.exit.borrow() {
            let _ = code;
            gtk::main_quit();
            return;
        }
        let redraw = std::mem::take(&mut self.shared.borrow_mut().host.redraw);
        if redraw {
            self.area.queue_draw();
            self.cover.queue_draw();
        }
        self.schedule_tick();
    }

    fn schedule_tick(self: &Rc<Self>) {
        let Some(d) = self.shared.borrow().app.next_tick() else { return };
        let at = Instant::now() + d;
        if matches!(*self.tick_at.borrow(), Some(t) if t <= at) {
            return;
        }
        *self.tick_at.borrow_mut() = Some(at);
        let me = self.clone();
        glib::timeout_add_local_once(d, move || {
            *me.tick_at.borrow_mut() = None;
            me.send(Event::Tick);
        });
    }

    fn view(&self, id: ViewId) -> Option<WebView> {
        self.views.borrow().get(&id).cloned()
    }

    fn run_ops(self: &Rc<Self>, ops: Vec<Op>) {
        for op in ops {
            match op {
                Op::NewView(id) => self.new_view(id),
                Op::Show(id) => {
                    if let Some(v) = self.view(id) {
                        self.stack.set_visible_child(&v);
                        *self.visible_view.borrow_mut() = Some(id);
                    }
                }
                Op::Close(id) => {
                    if let Some(v) = self.views.borrow_mut().remove(&id) {
                        v.stop_loading();
                        self.stack.remove(&v);
                    }
                }
                Op::Web(id, cmd) => self.command(id, cmd),
                Op::Chrome(h, rects) => self.set_chrome(h, &rects),
                Op::FocusChrome => self.area.grab_focus(),
                Op::Title(t) => self.window.set_title(&t),
                Op::Cursor(c) => {
                    if let Some(d) = gdk::Display::default() {
                        let cur = gdk::Cursor::from_name(&d, cursor_name(c));
                        for a in [&self.area, &self.cover] {
                            if let Some(w) = a.window() {
                                w.set_cursor(cur.as_ref());
                            }
                        }
                    }
                }
                Op::DragWindow => {
                    if let Some((b, (x, y), t)) = *self.last_press.borrow() {
                        self.window.begin_move_drag(b as i32, x as i32, y as i32, t);
                    }
                }
                Op::DragResize(e) => {
                    if let Some((b, (x, y), t)) = *self.last_press.borrow() {
                        let edge = match e {
                            Edge::N => gdk::WindowEdge::North,
                            Edge::S => gdk::WindowEdge::South,
                            Edge::E => gdk::WindowEdge::East,
                            Edge::W => gdk::WindowEdge::West,
                            Edge::NE => gdk::WindowEdge::NorthEast,
                            Edge::NW => gdk::WindowEdge::NorthWest,
                            Edge::SE => gdk::WindowEdge::SouthEast,
                            Edge::SW => gdk::WindowEdge::SouthWest,
                        };
                        self.window.begin_resize_drag(edge, b as i32, x as i32, y as i32, t);
                    }
                }
                Op::Minimize => self.window.iconify(),
                Op::ToggleMaximize => {
                    if self.window.is_maximized() {
                        self.window.unmaximize()
                    } else {
                        self.window.maximize()
                    }
                }
                Op::Close_ => {
                    *self.exit.borrow_mut() = Some(self.shared.borrow().app.exit_code());
                }
            }
        }
    }

    fn set_chrome(self: &Rc<Self>, h: u32, rects: &[(u32, u32, u32, u32)]) {
        let scale = self.size.borrow().2.max(1.0);
        if *self.chrome_h.borrow() != h {
            *self.chrome_h.borrow_mut() = h;
            // Not during a size allocation (GTK ignores resizes asked for then): just after.
            let (area, lh) = (self.area.clone(), (h as f32 / scale).round() as i32);
            glib::idle_add_local_once(move || {
                area.set_size_request(-1, lh);
                area.queue_resize();
            });
        }
        let cover = rects.iter().any(|r| r.1 + r.3 > h + 1);
        if cover == *self.covering.borrow() {
            return;
        }
        *self.covering.borrow_mut() = cover;
        if !cover {
            self.outer.set_visible_child_name("normal");
            *self.cover_img.borrow_mut() = None;
            return;
        }
        // A still picture of the page goes under the menu; until it arrives, the page colour.
        self.outer.set_visible_child_name("cover");
        self.cover.grab_focus();
        let view = self.visible_view.borrow().and_then(|id| self.view(id));
        if let Some(v) = view {
            let me = self.clone();
            v.snapshot(webkit2gtk::SnapshotRegion::Visible, webkit2gtk::SnapshotOptions::NONE, None::<&gtk::gio::Cancellable>, move |r| {
                if let Ok(surf) = r {
                    if let Ok(img) = gtk::cairo::ImageSurface::try_from(surf) {
                        if *me.covering.borrow() {
                            *me.cover_img.borrow_mut() = Some(img);
                            me.cover.queue_draw();
                        }
                    }
                }
            });
        }
    }

    fn command(self: &Rc<Self>, id: ViewId, cmd: WebCommand) {
        let Some(v) = self.view(id) else { return };
        let opts = (webkit2gtk::FindOptions::CASE_INSENSITIVE | webkit2gtk::FindOptions::WRAP_AROUND).bits();
        match cmd {
            WebCommand::Load(uri) => v.load_uri(&uri),
            WebCommand::Back => v.go_back(),
            WebCommand::Forward => v.go_forward(),
            WebCommand::Reload => v.reload(),
            WebCommand::Stop => v.stop_loading(),
            WebCommand::Zoom(z) => v.set_zoom_level(z),
            WebCommand::Find(t) => {
                if let Some(fc) = v.find_controller() {
                    if t.is_empty() {
                        fc.search_finish();
                        self.send(Event::Web(WebEvent::Found { view: id, count: 0 }));
                    } else {
                        fc.count_matches(&t, opts, 9999);
                        fc.search(&t, opts, 9999);
                    }
                }
            }
            WebCommand::FindNext => {
                if let Some(fc) = v.find_controller() {
                    fc.search_next();
                }
            }
            WebCommand::FindPrevious => {
                if let Some(fc) = v.find_controller() {
                    fc.search_previous();
                }
            }
            WebCommand::FindEnd => {
                if let Some(fc) = v.find_controller() {
                    fc.search_finish();
                }
            }
            WebCommand::Print => {
                let op = webkit2gtk::PrintOperation::new(&v);
                op.run_dialog(Some(&self.window));
            }
            WebCommand::Focus => v.grab_focus(),
            WebCommand::Snapshot(path) => {
                let me = self.clone();
                v.snapshot(webkit2gtk::SnapshotRegion::Visible, webkit2gtk::SnapshotOptions::NONE, None::<&gtk::gio::Cancellable>, move |r| {
                    let value = match r.map_err(|e| e.to_string()).and_then(|surf| {
                        let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                        surf.write_to_png(&mut f).map_err(|e| e.to_string())
                    }) {
                        Ok(()) => "snapshot".to_string(),
                        Err(e) => format!("snapshot failed: {e}"),
                    };
                    me.send(Event::Web(WebEvent::ScriptResult { view: id, value }));
                });
            }
            WebCommand::Script(js) => {
                let me = self.clone();
                v.evaluate_javascript(&js, None, None, None::<&gtk::gio::Cancellable>, move |r| {
                    let value = match r {
                        Ok(val) => javascriptcore::ValueExt::to_str(&val).to_string(),
                        Err(e) => format!("error: {e}"),
                    };
                    me.send(Event::Web(WebEvent::ScriptResult { view: id, value }));
                });
            }
        }
    }

    fn new_view(self: &Rc<Self>, id: ViewId) {
        let v = WebView::with_context(&self.ctx);
        v.set_settings(&self.settings);
        v.set_hexpand(true);
        v.set_vexpand(true);
        let weak = Rc::downgrade(self);
        let w = weak.clone();
        v.connect_title_notify(move |v| {
            if let Some(me) = w.upgrade() {
                me.send(Event::Web(WebEvent::Title { view: id, title: v.title().map(|t| t.to_string()).unwrap_or_default() }));
            }
        });
        let w = weak.clone();
        v.connect_uri_notify(move |v| {
            if let Some(me) = w.upgrade() {
                let uri = v.uri().map(|t| t.to_string()).unwrap_or_default();
                if !uri.starts_with("data:") {
                    me.send(Event::Web(WebEvent::Uri { view: id, uri }));
                }
            }
        });
        let w = weak.clone();
        v.connect_estimated_load_progress_notify(move |v| {
            if let Some(me) = w.upgrade() {
                me.send(Event::Web(WebEvent::Progress { view: id, value: v.estimated_load_progress() }));
            }
        });
        let w = weak.clone();
        v.connect_is_loading_notify(move |v| {
            if let Some(me) = w.upgrade() {
                me.send(Event::Web(WebEvent::Loading { view: id, loading: v.is_loading() }));
            }
        });
        let w = weak.clone();
        v.connect_zoom_level_notify(move |v| {
            if let Some(me) = w.upgrade() {
                me.send(Event::Web(WebEvent::Zoom { view: id, level: v.zoom_level() }));
            }
        });
        let w = weak.clone();
        v.connect_load_changed(move |v, _ev| {
            if let Some(me) = w.upgrade() {
                me.send(Event::Web(WebEvent::History { view: id, back: v.can_go_back(), forward: v.can_go_forward() }));
            }
        });
        let w = weak.clone();
        v.connect_decide_policy(move |v, decision, ty| {
            let Some(me) = w.upgrade() else { return false };
            if ty != PolicyDecisionType::NavigationAction && ty != PolicyDecisionType::NewWindowAction {
                return false;
            }
            let Some(nd) = decision.downcast_ref::<NavigationPolicyDecision>() else { return false };
            let Some(action) = nd.navigation_action() else { return false };
            let Some(uri) = action.request().and_then(|r| r.uri()).map(|u| u.to_string()) else { return false };
            let m = gdk::ModifierType::from_bits_truncate(action.modifiers());
            let new_tab = ty == PolicyDecisionType::NewWindowAction || action.mouse_button() == 2 || (action.mouse_button() == 1 && m.contains(gdk::ModifierType::CONTROL_MASK));
            if new_tab {
                decision.ignore();
                me.send(Event::Web(WebEvent::OpenTab { view: id, uri }));
                return true;
            }
            // The interstitial's "continue anyway" link.
            if let Some(clean) = uri.strip_suffix("#folio-allow-insecure").or_else(|| uri.strip_suffix("&folio-allow-insecure")) {
                if let Some(h) = host_of(clean) {
                    me.insecure_ok.borrow_mut().insert(h);
                }
                decision.ignore();
                let (v, clean) = (v.clone(), clean.to_string());
                glib::idle_add_local_once(move || v.load_uri(&clean));
                return true;
            }
            if me.policy.https_only && uri.starts_with("http://") {
                let host = host_of(&uri).unwrap_or_default();
                if !is_local_host(&host) && !me.insecure_ok.borrow().contains(&host) {
                    decision.ignore();
                    let up = uri.replacen("http://", "https://", 1);
                    me.upgraded.borrow_mut().insert(up.clone());
                    let v = v.clone();
                    glib::idle_add_local_once(move || v.load_uri(&up));
                    return true;
                }
            }
            false
        });
        let w = weak.clone();
        v.connect_create(move |_v, action| {
            if let Some(me) = w.upgrade() {
                let a = action.clone();
                if let Some(uri) = a.request().and_then(|r| r.uri()) {
                    me.send(Event::Web(WebEvent::OpenTab { view: id, uri: uri.to_string() }));
                }
            }
            None
        });
        let w = weak.clone();
        v.connect_load_failed(move |v, _ev: LoadEvent, uri, err| {
            let Some(me) = w.upgrade() else { return false };
            if err.matches(webkit2gtk::NetworkError::Cancelled) || err.matches(webkit2gtk::PolicyError::FrameLoadInterruptedByPolicyChange) {
                return false;
            }
            if me.upgraded.borrow().contains(uri) {
                v.load_alternate_html(&insecure_page(uri), uri, None);
                me.send(Event::Web(WebEvent::Insecure { view: id, uri: uri.replacen("https://", "http://", 1) }));
                return true;
            }
            me.send(Event::Web(WebEvent::Failed { view: id, uri: uri.to_string(), message: err.to_string() }));
            false
        });
        let w = weak.clone();
        v.connect_load_failed_with_tls_errors(move |v, uri, _cert, _errs| {
            if let Some(me) = w.upgrade() {
                let html = "<!doctype html><meta charset=utf-8><title>Certificate not valid</title><style>body{font:15px sans-serif;max-width:560px;margin:12vh auto;color:#333}</style><h1>This site's certificate isn't valid</h1><p>Folio Graze stopped loading it: the connection may not be private.</p>";
                v.load_alternate_html(html, uri, None);
                me.send(Event::Web(WebEvent::Failed { view: id, uri: uri.to_string(), message: "The site's certificate isn't valid.".into() }));
            }
            true
        });
        let w = weak.clone();
        v.connect_focus_in_event(move |_v, _| {
            if let Some(me) = w.upgrade() {
                me.send(Event::Web(WebEvent::PageFocus { view: id }));
            }
            glib::Propagation::Proceed
        });
        if let Some(fc) = v.find_controller() {
            let w = weak.clone();
            fc.connect_counted_matches(move |_, n| {
                if let Some(me) = w.upgrade() {
                    me.send(Event::Web(WebEvent::Found { view: id, count: n }));
                }
            });
            let w = weak.clone();
            fc.connect_failed_to_find_text(move |_| {
                if let Some(me) = w.upgrade() {
                    me.send(Event::Web(WebEvent::Found { view: id, count: 0 }));
                }
            });
        }
        v.show();
        self.stack.add(&v);
        self.views.borrow_mut().insert(id, v);
    }

    fn paint(self: &Rc<Self>, cr: &gtk::cairo::Context, cover: bool) {
        let (w, h, scale) = *self.size.borrow();
        if w == 0 || h == 0 {
            return;
        }
        let n = (w * h * 4) as usize;
        let mut px = self.pixels.borrow_mut();
        px.resize(n, 0);
        px.fill(0);
        let ops = {
            let Ok(mut sh) = self.shared.try_borrow_mut() else { return };
            let sh = &mut *sh;
            sh.app.paint(&mut px, w, h, &mut sh.host);
            std::mem::take(&mut sh.host.ops)
        };
        // Premultiplied RGBA to cairo's premultiplied BGRA (in native order).
        let mut data = vec![0u8; n];
        for (s, d) in px.chunks_exact(4).zip(data.chunks_exact_mut(4)) {
            let v = u32::from_be_bytes([s[3], s[0], s[1], s[2]]);
            d.copy_from_slice(&v.to_ne_bytes());
        }
        drop(px);
        if cover {
            cr.set_source_rgb(1.0, 1.0, 1.0);
            let _ = cr.paint();
            if let Some(img) = self.cover_img.borrow().as_ref() {
                let top = *self.chrome_h.borrow() as f64 / scale as f64;
                let _ = cr.set_source_surface(img, 0.0, top);
                let _ = cr.paint();
            }
        }
        if let Ok(surf) = gtk::cairo::ImageSurface::create_for_data(data, gtk::cairo::Format::ARgb32, w as i32, h as i32, w as i32 * 4) {
            surf.set_device_scale(scale as f64, scale as f64);
            cr.set_operator(gtk::cairo::Operator::Over);
            let _ = cr.set_source_surface(&surf, 0.0, 0.0);
            let _ = cr.paint();
        }
        self.run_ops(ops);
    }
}

/// Open a web window and run the app until it closes. Returns the app's exit code.
pub fn run_web<A: App + 'static>(app: A, spec: WindowSpec, policy: WebPolicy) -> i32 {
    glib::set_prgname(Some(spec.app_id.as_str()));
    glib::set_application_name(&spec.title);
    if gtk::init().is_err() {
        eprintln!("Can't open a window: no display.");
        return 1;
    }
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_decorated(false);
    window.set_title(&spec.title);
    window.set_default_size(spec.width as i32, spec.height as i32);
    window.set_size_request(640, 420);
    if let Some(icon) = &spec.icon {
        let pb = gtk::gdk_pixbuf::Pixbuf::from_bytes(&glib::Bytes::from(&icon.rgba), gtk::gdk_pixbuf::Colorspace::Rgb, true, 8, icon.w as i32, icon.h as i32, icon.w as i32 * 4);
        window.set_icon(Some(&pb));
    }
    // The web engine, with the privacy settings.
    let ctx = if policy.private {
        WebContext::new_ephemeral()
    } else {
        let data = policy.data_dir.join("data");
        let cache = policy.data_dir.join("cache");
        let _ = std::fs::create_dir_all(&data);
        let mgr = WebsiteDataManager::builder().base_data_directory(data.to_string_lossy().as_ref()).base_cache_directory(cache.to_string_lossy().as_ref()).build();
        WebContext::with_website_data_manager(&mgr)
    };
    if let Some(mgr) = ctx.website_data_manager() {
        mgr.set_itp_enabled(policy.tracking_protection);
        if let Some(cm) = mgr.cookie_manager() {
            cm.set_accept_policy(if policy.block_third_party_cookies { CookieAcceptPolicy::NoThirdParty } else { CookieAcceptPolicy::Always });
            if !policy.private {
                cm.set_persistent_storage(&policy.data_dir.join("cookies.sqlite").to_string_lossy(), CookiePersistentStorage::Sqlite);
            }
        }
    }
    let settings = webkit2gtk::Settings::new();
    settings.set_enable_dns_prefetching(false);
    settings.set_enable_developer_extras(false);
    settings.set_javascript_can_open_windows_automatically(false);
    settings.set_enable_back_forward_navigation_gestures(true);
    settings.set_enable_write_console_messages_to_stdout(false);
    if crate::services::setting("FOLIO_WEB_SOFTWARE").is_some() {
        settings.set_hardware_acceleration_policy(webkit2gtk::HardwareAccelerationPolicy::Never);
    }

    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    let mk_area = || {
        let a = gtk::DrawingArea::new();
        a.set_can_focus(true);
        a.set_app_paintable(true);
        a.add_events(gdk::EventMask::BUTTON_PRESS_MASK | gdk::EventMask::BUTTON_RELEASE_MASK | gdk::EventMask::POINTER_MOTION_MASK | gdk::EventMask::SCROLL_MASK | gdk::EventMask::SMOOTH_SCROLL_MASK | gdk::EventMask::KEY_PRESS_MASK | gdk::EventMask::KEY_RELEASE_MASK | gdk::EventMask::LEAVE_NOTIFY_MASK | gdk::EventMask::FOCUS_CHANGE_MASK);
        a
    };
    let area = mk_area();
    let cover = mk_area();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.pack_start(&area, false, false, 0);
    column.pack_start(&stack, true, true, 0);
    let outer = gtk::Stack::new();
    outer.add_named(&column, "normal");
    outer.add_named(&cover, "cover");
    window.add(&outer);
    let ui = Rc::new(Ui {
        window: window.clone(),
        area: area.clone(),
        cover: cover.clone(),
        outer: outer.clone(),
        cover_img: RefCell::new(None),
        covering: RefCell::new(false),
        visible_view: RefCell::new(None),
        stack: stack.clone(),
        im: gtk::IMMulticontext::new(),
        ctx,
        settings,
        views: RefCell::new(HashMap::new()),
        policy,
        insecure_ok: RefCell::new(HashSet::new()),
        upgraded: RefCell::new(HashSet::new()),
        shared: RefCell::new(Shared { app: Box::new(app), host: WebHostImpl { ops: vec![], next_view: 0, start: Instant::now(), maximized: false, handled: false, redraw: true } }),
        queue: RefCell::new(VecDeque::new()),
        pixels: RefCell::new(Vec::new()),
        size: RefCell::new((0, 0, 1.0)),
        mods: RefCell::new(Mods::NONE),
        clicks: RefCell::new(ClickCounter::default()),
        last_press: RefCell::new(None),
        tick_at: RefCell::new(None),
        chrome_h: RefCell::new(0),
        exit: RefCell::new(None),
    });

    let u = ui.clone();
    outer.connect_size_allocate(move |a, alloc| {
        let scale = a.scale_factor().max(1) as f32;
        let (w, h) = ((alloc.width() as f32 * scale) as u32, (alloc.height() as f32 * scale) as u32);
        if *u.size.borrow() != (w, h, scale) {
            *u.size.borrow_mut() = (w, h, scale);
            u.send(Event::Resized { w, h, scale });
            a.queue_draw();
        }
    });
    let u = ui.clone();
    area.connect_draw(move |_a, cr| {
        u.paint(cr, false);
        glib::Propagation::Stop
    });
    let u = ui.clone();
    cover.connect_draw(move |_a, cr| {
        u.paint(cr, true);
        glib::Propagation::Stop
    });
    let u = ui.clone();
    area.connect_realize(move |a| {
        if let Some(w) = a.window() {
            u.im.set_client_window(Some(&w));
        }
    });
    let scale_of = |u: &Rc<Ui>| u.size.borrow().2;
    for w in [&area, &cover] {
        let u = ui.clone();
        w.connect_motion_notify_event(move |_a, ev| {
            let s = scale_of(&u);
            let (x, y) = ev.position();
            u.send(Event::MouseMove { x: x as f32 * s, y: y as f32 * s });
            glib::Propagation::Stop
        });
        let u = ui.clone();
        w.connect_leave_notify_event(move |_a, _ev| {
            u.send(Event::MouseLeave);
            glib::Propagation::Proceed
        });
        let u = ui.clone();
        w.connect_button_press_event(move |a, ev| {
            if ev.event_type() != gdk::EventType::ButtonPress {
                return glib::Propagation::Stop;
            }
            a.grab_focus();
            let s = scale_of(&u);
            let (x, y) = ev.position();
            let (x, y) = (x as f32 * s, y as f32 * s);
            *u.last_press.borrow_mut() = Some((ev.button(), ev.root(), ev.time()));
            let button = match ev.button() {
                1 => Button::Left,
                2 => Button::Middle,
                3 => Button::Right,
                _ => Button::Other,
            };
            let clicks = u.clicks.borrow_mut().press(Instant::now(), x, y, button, s);
            u.send(Event::MouseDown { x, y, button, clicks });
            glib::Propagation::Stop
        });
        let u = ui.clone();
        w.connect_button_release_event(move |_a, ev| {
            let s = scale_of(&u);
            let (x, y) = ev.position();
            let button = match ev.button() {
                1 => Button::Left,
                2 => Button::Middle,
                3 => Button::Right,
                _ => Button::Other,
            };
            u.send(Event::MouseUp { x: x as f32 * s, y: y as f32 * s, button });
            glib::Propagation::Stop
        });
        let u = ui.clone();
        w.connect_scroll_event(move |_a, ev| {
            let (dx, dy) = match ev.direction() {
                gdk::ScrollDirection::Up => (0.0, 1.0),
                gdk::ScrollDirection::Down => (0.0, -1.0),
                gdk::ScrollDirection::Left => (1.0, 0.0),
                gdk::ScrollDirection::Right => (-1.0, 0.0),
                _ => {
                    let (dx, dy) = ev.delta();
                    (-dx as f32, -dy as f32)
                }
            };
            u.send(Event::Wheel { dx, dy });
            glib::Propagation::Stop
        });
    }
    let u = ui.clone();
    area.connect_focus_in_event(move |_a, _| {
        u.im.focus_in();
        glib::Propagation::Proceed
    });
    let u = ui.clone();
    area.connect_focus_out_event(move |_a, _| {
        u.im.focus_out();
        glib::Propagation::Proceed
    });
    let u = ui.clone();
    ui.im.connect_commit(move |_, text| {
        let t: String = text.chars().filter(|c| !c.is_control()).collect();
        if !t.is_empty() {
            u.send(Event::Text(t));
        }
    });
    // Keys reach the window first: the app sees every key, and keeps the ones it uses (its
    // shortcuts, and everything while its own drawing has the keyboard); the rest go on to the
    // page.
    let u = ui.clone();
    window.connect_key_press_event(move |_w, ev| {
        let m = mods_of(ev.state());
        let key = key_of(&ev.keyval());
        let mut nm = m;
        match key {
            Key::Ctrl => nm.ctrl = true,
            Key::Shift => nm.shift = true,
            Key::Alt => nm.alt = true,
            Key::Logo => nm.logo = true,
            _ => {}
        }
        if *u.mods.borrow() != nm {
            *u.mods.borrow_mut() = nm;
            u.send(Event::Modifiers(nm));
        }
        let handled = u.send(Event::Key { key, mods: m, down: true, repeat: false });
        if u.area.has_focus() {
            if !m.ctrl && !m.logo {
                u.im.filter_keypress(ev);
            }
            return glib::Propagation::Stop;
        }
        if handled { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    let u = ui.clone();
    window.connect_key_release_event(move |_w, ev| {
        let m = mods_of(ev.state());
        let key = key_of(&ev.keyval());
        let mut nm = m;
        match key {
            Key::Ctrl => nm.ctrl = false,
            Key::Shift => nm.shift = false,
            Key::Alt => nm.alt = false,
            Key::Logo => nm.logo = false,
            _ => {}
        }
        if *u.mods.borrow() != nm {
            *u.mods.borrow_mut() = nm;
            u.send(Event::Modifiers(nm));
        }
        let handled = u.send(Event::Key { key, mods: m, down: false, repeat: false });
        if u.area.has_focus() {
            u.im.filter_keypress(ev);
            return glib::Propagation::Stop;
        }
        if handled { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    let u = ui.clone();
    window.connect_focus_in_event(move |_w, _| {
        u.send(Event::Focus(true));
        glib::Propagation::Proceed
    });
    let u = ui.clone();
    window.connect_focus_out_event(move |_w, _| {
        u.send(Event::Focus(false));
        glib::Propagation::Proceed
    });
    let u = ui.clone();
    window.connect_window_state_event(move |_w, _| {
        u.area.queue_draw();
        glib::Propagation::Proceed
    });
    let u = ui.clone();
    window.connect_delete_event(move |_w, _| {
        u.send(Event::CloseRequested);
        glib::Propagation::Stop
    });
    window.show_all();
    ui.schedule_tick();
    gtk::main();
    let code = *ui.exit.borrow();
    for (_, v) in ui.views.borrow_mut().drain() {
        v.stop_loading();
    }
    code.unwrap_or_else(|| ui.shared.borrow().app.exit_code())
}
