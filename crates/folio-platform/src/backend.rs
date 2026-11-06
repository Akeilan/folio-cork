//! The desktop backend: one window drawn as a pixel buffer. Built on winit and softbuffer,
//! which cover X11 and Wayland on Linux, Windows and macOS.

use crate::{App, Button, ClickCounter, Cursor, Edge, Event, Host, Image, Key, Mods, WindowSpec};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key as WKey, NamedKey};
use winit::window::{CursorIcon, ResizeDirection, Theme, Window, WindowId};

struct WinHost {
    window: Rc<Window>,
    start: Instant,
    clipboard: Option<arboard::Clipboard>,
    /// What the app asked for: the compositor may take a moment to do it.
    fullscreen: bool,
    /// On Wayland, text goes through the compositor's own clipboard (arboard only reaches the
    /// X11 one, which a session without XWayland doesn't have).
    #[cfg(all(unix, not(target_os = "macos")))]
    wl_clip: Option<smithay_clipboard::Clipboard>,
    want_redraw: bool,
    want_close: bool,
    last_cursor: Option<Cursor>,
}

impl Host for WinHost {
    fn redraw(&mut self) {
        self.want_redraw = true;
    }
    fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }
    fn set_cursor(&mut self, cursor: Cursor) {
        if self.last_cursor == Some(cursor) {
            return;
        }
        self.last_cursor = Some(cursor);
        let icon = match cursor {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::Text => CursorIcon::Text,
            Cursor::Move => CursorIcon::Move,
            Cursor::Grab => CursorIcon::Grab,
            Cursor::Grabbing => CursorIcon::Grabbing,
            Cursor::Crosshair => CursorIcon::Crosshair,
            Cursor::ResizeNS => CursorIcon::NsResize,
            Cursor::ResizeEW => CursorIcon::EwResize,
            Cursor::ResizeNWSE => CursorIcon::NwseResize,
            Cursor::ResizeNESW => CursorIcon::NeswResize,
            Cursor::NotAllowed => CursorIcon::NotAllowed,
        };
        self.window.set_cursor(icon);
    }
    fn drag_window(&mut self) {
        let _ = self.window.drag_window();
    }
    fn drag_resize(&mut self, edge: Edge) {
        let d = match edge {
            Edge::N => ResizeDirection::North,
            Edge::S => ResizeDirection::South,
            Edge::E => ResizeDirection::East,
            Edge::W => ResizeDirection::West,
            Edge::NE => ResizeDirection::NorthEast,
            Edge::NW => ResizeDirection::NorthWest,
            Edge::SE => ResizeDirection::SouthEast,
            Edge::SW => ResizeDirection::SouthWest,
        };
        let _ = self.window.drag_resize_window(d);
    }
    fn minimize(&mut self) {
        self.window.set_minimized(true);
    }
    fn toggle_maximize(&mut self) {
        let m = self.window.is_maximized();
        self.window.set_maximized(!m);
    }
    fn is_maximized(&self) -> bool {
        self.window.is_maximized()
    }
    fn set_fullscreen(&mut self, on: bool) {
        self.fullscreen = on;
        self.window.set_fullscreen(if on { Some(winit::window::Fullscreen::Borderless(None)) } else { None });
    }
    fn is_fullscreen(&self) -> bool {
        // what was asked for: Wayland compositors answer the window itself only once they have
        // reconfigured it, which is after this call
        self.fullscreen || self.window.fullscreen().is_some()
    }
    fn clipboard_images(&self) -> bool {
        // arboard reaches the X11 clipboard. Under Wayland the compositor's own clipboard carries
        // words; a picture goes through the wl-clipboard helpers when they are installed (X4-04).
        #[cfg(all(unix, not(target_os = "macos")))]
        if self.wl_clip.is_some() {
            return wl_clipboard_helpers();
        }
        true
    }
    fn close(&mut self) {
        self.want_close = true;
    }
    fn system_dark(&self) -> Option<bool> {
        if let Ok(v) = std::env::var("FOLIO_THEME") {
            return Some(v == "dark");
        }
        match self.window.theme() {
            Some(Theme::Dark) => Some(true),
            Some(Theme::Light) => Some(false),
            None => linux_dark_preference(),
        }
    }
    fn clipboard_text(&mut self) -> Option<String> {
        #[cfg(all(unix, not(target_os = "macos")))]
        if let Some(w) = &self.wl_clip {
            return w.load().ok();
        }
        self.clip()?.get_text().ok()
    }
    fn set_clipboard_text(&mut self, text: &str) {
        #[cfg(all(unix, not(target_os = "macos")))]
        if let Some(w) = &self.wl_clip {
            w.store(text);
            return;
        }
        if let Some(c) = self.clip() {
            let _ = c.set_text(text.to_string());
        }
    }
    fn clipboard_image(&mut self) -> Option<Image> {
        let img = self.clip()?.get_image().ok()?;
        Some(Image { w: img.width as u32, h: img.height as u32, rgba: img.bytes.into_owned() })
    }
    fn set_clipboard_image(&mut self, img: &Image) {
        if let Some(c) = self.clip() {
            let _ = c.set_image(arboard::ImageData {
                width: img.w as usize,
                height: img.h as usize,
                bytes: std::borrow::Cow::Borrowed(&img.rgba),
            });
        }
    }
    fn clipboard_png(&mut self) -> Option<Vec<u8>> {
        #[cfg(all(unix, not(target_os = "macos")))]
        if self.wl_clip.is_some() {
            return wl_paste_png();
        }
        None
    }
    fn set_clipboard_png(&mut self, _png: &[u8]) -> bool {
        #[cfg(all(unix, not(target_os = "macos")))]
        if self.wl_clip.is_some() {
            return wl_copy_png(_png);
        }
        false
    }
    fn now(&self) -> Duration {
        self.start.elapsed()
    }
}

// ---------------------------------------------------------------- pictures on Wayland (X4-04)
/// Wayland hands whole files between programs, and the compositor's own clipboard here carries
/// only words. wl-copy and wl-paste are the standard helpers for the rest; when they are
/// installed, a picture travels as a PNG the way every other Wayland program expects.
#[cfg(all(unix, not(target_os = "macos")))]
fn wl_clipboard_helpers() -> bool {
    use std::sync::OnceLock;
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| which("wl-copy").is_some() && which("wl-paste").is_some())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn which(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file())
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn wl_copy_png(png: &[u8]) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    if !wl_clipboard_helpers() || png.is_empty() {
        return false;
    }
    // wl-copy stays alive to serve the clipboard, as the protocol asks: don't wait for it.
    let child = Command::new("wl-copy")
        .arg("--type")
        .arg("image/png")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(_) => return false,
    };
    let wrote = child.stdin.take().map(|mut s| s.write_all(png).is_ok()).unwrap_or(false);
    // wl-copy puts a server of its own in the background and the command returns once it has the
    // clipboard. Its server still has to answer the first question, so wait until the picture is
    // really being offered: a paste right afterwards would otherwise find nothing.
    let _ = child.wait();
    for _ in 0..40 {
        if wl_offers_picture() {
            return wrote;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    wrote
}

/// Whether the clipboard is offering a picture at all.
#[cfg(all(unix, not(target_os = "macos")))]
fn wl_offers_picture() -> bool {
    wl_types().map(|t| t.contains("image/")).unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn wl_types() -> Option<String> {
    let out = std::process::Command::new("wl-paste").arg("--list-types").output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_lowercase())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn wl_paste_png() -> Option<Vec<u8>> {
    use std::process::Command;
    if !wl_clipboard_helpers() {
        return None;
    }
    let types = wl_types()?;
    let want = ["image/png", "image/bmp", "image/jpeg"].into_iter().find(|t| types.contains(t))?;
    let out = Command::new("wl-paste").arg("--no-newline").arg("--type").arg(want).output().ok()?;
    if out.stdout.is_empty() {
        None
    } else {
        Some(out.stdout)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn wayland_clipboard(window: &Window) -> Option<smithay_clipboard::Clipboard> {
    use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
    match window.display_handle().ok()?.as_raw() {
        // SAFETY: the display outlives the window, and the clipboard lives in the window's host.
        RawDisplayHandle::Wayland(h) => Some(unsafe { smithay_clipboard::Clipboard::new(h.display.as_ptr()) }),
        _ => None,
    }
}

impl WinHost {
    fn clip(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }
}

/// GNOME and its relatives (Zorin, Ubuntu) keep the dark preference in gsettings.
fn linux_dark_preference() -> Option<bool> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    if s.contains("dark") {
        return Some(true);
    }
    let out = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_lowercase().contains("dark"))
}

struct Runner<A: App> {
    app: A,
    spec: WindowSpec,
    host: Option<WinHost>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    pixels: Vec<u8>,
    size: (u32, u32),
    scale: f32,
    mouse: (f32, f32),
    mods: Mods,
    clicks: ClickCounter,
    next_tick: Option<Instant>,
    shown: bool,
    failed: bool,
}

impl<A: App> Runner<A> {
    fn send(&mut self, ev: Event) {
        if let Some(h) = self.host.as_mut() {
            self.app.event(&ev, h);
        }
    }

    fn paint(&mut self) {
        let (w, h) = self.size;
        if w == 0 || h == 0 {
            return;
        }
        let (Some(host), Some(surface)) = (self.host.as_mut(), self.surface.as_mut()) else { return };
        let n = (w * h * 4) as usize;
        if self.pixels.len() != n {
            self.pixels = vec![0; n];
        }
        self.app.paint(&mut self.pixels, w, h, host);
        if surface.resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap()).is_err() {
            return;
        }
        let Ok(mut buf) = surface.buffer_mut() else { return };
        for (dst, px) in buf.iter_mut().zip(self.pixels.chunks_exact(4)) {
            *dst = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
        }
        let _ = buf.present();
    }

    fn schedule(&mut self, el: &ActiveEventLoop) {
        if self.app.wants_exit() {
            el.exit();
            return;
        }
        let want_redraw = self.host.as_ref().map(|h| h.want_redraw).unwrap_or(false);
        if want_redraw {
            if let Some(h) = self.host.as_mut() {
                h.want_redraw = false;
                h.window.request_redraw();
            }
        }
        if self.host.as_ref().map(|h| h.want_close).unwrap_or(false) {
            el.exit();
            return;
        }
        self.next_tick = self.app.next_tick().map(|d| Instant::now() + d);
        match self.next_tick {
            Some(t) => el.set_control_flow(ControlFlow::WaitUntil(t)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }
}

fn map_key(k: &WKey) -> Key {
    match k {
        WKey::Character(s) => s.chars().next().map(|c| Key::Char(c.to_lowercase().next().unwrap_or(c))).unwrap_or(Key::Other),
        WKey::Named(n) => match n {
            NamedKey::Enter => Key::Enter,
            NamedKey::Escape => Key::Escape,
            NamedKey::Backspace => Key::Backspace,
            NamedKey::Delete => Key::Delete,
            NamedKey::Tab => Key::Tab,
            NamedKey::Space => Key::Space,
            NamedKey::ArrowLeft => Key::Left,
            NamedKey::ArrowRight => Key::Right,
            NamedKey::ArrowUp => Key::Up,
            NamedKey::ArrowDown => Key::Down,
            NamedKey::Home => Key::Home,
            NamedKey::End => Key::End,
            NamedKey::PageUp => Key::PageUp,
            NamedKey::PageDown => Key::PageDown,
            NamedKey::Insert => Key::Insert,
            NamedKey::Alt | NamedKey::AltGraph => Key::Alt,
            NamedKey::Control => Key::Ctrl,
            NamedKey::Shift => Key::Shift,
            NamedKey::Super | NamedKey::Meta => Key::Logo,
            NamedKey::F1 => Key::F(1),
            NamedKey::F2 => Key::F(2),
            NamedKey::F3 => Key::F(3),
            NamedKey::F4 => Key::F(4),
            NamedKey::F5 => Key::F(5),
            NamedKey::F6 => Key::F(6),
            NamedKey::F7 => Key::F(7),
            NamedKey::F8 => Key::F(8),
            NamedKey::F9 => Key::F(9),
            NamedKey::F10 => Key::F(10),
            NamedKey::F11 => Key::F(11),
            NamedKey::F12 => Key::F(12),
            _ => Key::Other,
        },
        _ => Key::Other,
    }
}

impl<A: App> ApplicationHandler for Runner<A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.host.is_some() {
            return;
        }
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title(self.spec.title.clone())
            .with_inner_size(LogicalSize::new(self.spec.width as f64, self.spec.height as f64))
            .with_min_inner_size(LogicalSize::new(640.0, 420.0))
            .with_decorations(false);
        if let Some(icon) = &self.spec.icon {
            attrs = attrs.with_window_icon(winit::window::Icon::from_rgba(icon.rgba.clone(), icon.w, icon.h).ok());
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            use winit::platform::wayland::WindowAttributesExtWayland;
            use winit::platform::x11::WindowAttributesExtX11;
            attrs = WindowAttributesExtX11::with_name(attrs, self.spec.app_id.clone(), self.spec.app_id.clone());
            attrs = WindowAttributesExtWayland::with_name(attrs, self.spec.app_id.clone(), "");
        }
        let window = match el.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                eprintln!("Can't open a window: {e}");
                self.failed = true;
                el.exit();
                return;
            }
        };
        let context = softbuffer::Context::new(window.clone()).expect("display");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("surface");
        let size = window.inner_size();
        self.size = (size.width, size.height);
        self.scale = window.scale_factor() as f32;
        self.surface = Some(surface);
        self.host = Some(WinHost {
            window: window.clone(),
            start: Instant::now(),
            clipboard: None,
            fullscreen: false,
            #[cfg(all(unix, not(target_os = "macos")))]
            wl_clip: wayland_clipboard(&window),
            want_redraw: true,
            want_close: false,
            last_cursor: None,
        });
        let (w, h, scale) = (self.size.0, self.size.1, self.scale);
        self.send(Event::Resized { w, h, scale });
        window.request_redraw();
        self.schedule(el);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => {
                self.paint();
                if !self.shown {
                    // A newly started app takes the keyboard once it's on screen (a bare X
                    // server has no window manager to give it).
                    self.shown = true;
                    if let Some(h) = &self.host {
                        h.window.focus_window();
                    }
                }
            }
            WindowEvent::Resized(s) => {
                self.size = (s.width, s.height);
                let scale = self.scale;
                self.send(Event::Resized { w: s.width, h: s.height, scale });
                if let Some(h) = self.host.as_mut() {
                    h.want_redraw = true;
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale = scale_factor as f32;
                let (w, h) = self.size;
                self.send(Event::Resized { w, h, scale: self.scale });
            }
            WindowEvent::CloseRequested => self.send(Event::CloseRequested),
            WindowEvent::Focused(f) => self.send(Event::Focus(f)),
            WindowEvent::ThemeChanged(t) => self.send(Event::ThemeChanged { dark: t == Theme::Dark }),
            WindowEvent::DroppedFile(p) => self.send(Event::FileDropped(p)),
            WindowEvent::CursorMoved { position: PhysicalPosition { x, y }, .. } => {
                self.mouse = (x as f32, y as f32);
                self.send(Event::MouseMove { x: x as f32, y: y as f32 });
            }
            WindowEvent::CursorLeft { .. } => self.send(Event::MouseLeave),
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    MouseButton::Middle => Button::Middle,
                    _ => Button::Other,
                };
                let (x, y) = self.mouse;
                if state == ElementState::Pressed {
                    let clicks = self.clicks.press(Instant::now(), x, y, b, self.scale);
                    self.send(Event::MouseDown { x, y, button: b, clicks });
                } else {
                    self.send(Event::MouseUp { x, y, button: b });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32 / 40.0, p.y as f32 / 40.0),
                };
                self.send(Event::Wheel { dx, dy });
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                self.mods = Mods { ctrl: s.control_key(), shift: s.shift_key(), alt: s.alt_key(), logo: s.super_key() };
                self.send(Event::Modifiers(self.mods));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = map_key(&event.logical_key);
                let down = event.state == ElementState::Pressed;
                let mods = self.mods;
                self.send(Event::Key { key, mods, down, repeat: event.repeat });
                if down && !mods.ctrl && !mods.logo {
                    if let Some(t) = event.text {
                        let t: String = t.chars().filter(|c| !c.is_control()).collect();
                        if !t.is_empty() {
                            self.send(Event::Text(t));
                        }
                    }
                }
            }
            WindowEvent::Ime(winit::event::Ime::Commit(t)) => self.send(Event::Text(t)),
            _ => {}
        }
        self.schedule(el);
    }

    fn new_events(&mut self, el: &ActiveEventLoop, cause: winit::event::StartCause) {
        if let winit::event::StartCause::ResumeTimeReached { .. } = cause {
            self.send(Event::Tick);
            self.schedule(el);
        }
    }
}

/// Open the app's window and run it until it closes. Returns the app's exit code.
pub fn run<A: App + 'static>(app: A, spec: WindowSpec) -> i32 {
    let el = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("Can't open a window: {e}");
            return 1;
        }
    };
    let mut r = Runner {
        app,
        spec,
        host: None,
        surface: None,
        pixels: Vec::new(),
        size: (0, 0),
        scale: 1.0,
        mouse: (0.0, 0.0),
        mods: Mods::NONE,
        clicks: ClickCounter::default(),
        next_tick: None,
        shown: false,
        failed: false,
    };
    if let Err(e) = el.run_app(&mut r) {
        eprintln!("{e}");
        return 1;
    }
    if r.failed { 1 } else { r.app.exit_code() }
}
