//! Folio platform layer (X3-13): the only code in Folio that talks to the operating system.
//!
//! Apps and the toolkit see a window as a pixel buffer plus a stream of [`Event`]s, and reach
//! the system only through [`Host`]. A new platform means a new implementation of this crate's
//! backend; everything above it stays the same.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod backend;
#[cfg(feature = "web")]
mod web;
#[cfg(feature = "audio")]
pub mod audio;
pub mod services;
mod netaddr;
pub mod sys;

pub use backend::run;
#[cfg(feature = "web")]
pub use web::run_web;
pub use netaddr::{host_of, is_local_host};
pub use services::*;

/// Physical-pixel window events, already turned into Folio's own vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Window size in physical pixels and the display scale (1.0 = 96 dpi).
    Resized { w: u32, h: u32, scale: f32 },
    MouseMove { x: f32, y: f32 },
    MouseDown { x: f32, y: f32, button: Button, clicks: u32 },
    MouseUp { x: f32, y: f32, button: Button },
    /// Wheel in lines (positive y = away from the user).
    Wheel { dx: f32, dy: f32 },
    /// Mouse left the window.
    MouseLeave,
    Key { key: Key, mods: Mods, down: bool, repeat: bool },
    /// Text typed by the user (already composed by the system's input method).
    Text(String),
    Modifiers(Mods),
    FileDropped(PathBuf),
    Focus(bool),
    ThemeChanged { dark: bool },
    CloseRequested,
    /// A timer tick requested through [`App::next_tick`].
    Tick,
    /// News from a web view (apps run with [`run_web`] only).
    Web(WebEvent),
}

/// A web view inside the window, made with [`Host::web_new_view`].
pub type ViewId = u32;

/// What a web view reports (X3-13, the web engine part of the contract).
#[derive(Clone, Debug, PartialEq)]
pub enum WebEvent {
    Title { view: ViewId, title: String },
    Uri { view: ViewId, uri: String },
    /// Loading progress, 0.0 to 1.0.
    Progress { view: ViewId, value: f64 },
    Loading { view: ViewId, loading: bool },
    History { view: ViewId, back: bool, forward: bool },
    /// The page asked for a link in a new tab (middle click, Ctrl+click, target=_blank, window.open).
    OpenTab { view: ViewId, uri: String },
    /// A page couldn't be loaded.
    Failed { view: ViewId, uri: String, message: String },
    /// HTTPS-only stopped a page that only works without encryption; the view shows why.
    Insecure { view: ViewId, uri: String },
    /// Find in page: how many matches (0 = none).
    Found { view: ViewId, count: u32 },
    Zoom { view: ViewId, level: f64 },
    /// The page took the keyboard (the user clicked into it).
    PageFocus { view: ViewId },
    /// Result of [`WebCommand::Script`], as text (tests).
    ScriptResult { view: ViewId, value: String },
}

/// What an app can ask a web view to do.
#[derive(Clone, Debug, PartialEq)]
pub enum WebCommand {
    Load(String),
    Back,
    Forward,
    Reload,
    Stop,
    Zoom(f64),
    Find(String),
    FindNext,
    FindPrevious,
    FindEnd,
    Print,
    /// Give the page the keyboard.
    Focus,
    /// Run a script in the page and report its result as text (tests only).
    Script(String),
    /// Save a picture of the visible page as PNG (tests); reported as ScriptResult "snapshot".
    Snapshot(PathBuf),
}

/// Privacy settings a web window starts with (B3-03).
#[derive(Clone, Debug)]
pub struct WebPolicy {
    /// Keep nothing on disk: cookies, cache and history end with the window.
    pub private: bool,
    /// Only encrypted pages; plain http:// is upgraded, and a site without encryption shows a
    /// page that says so (this computer's own addresses are exempt).
    pub https_only: bool,
    pub block_third_party_cookies: bool,
    /// The engine's tracking prevention (it removes cross-site tracking storage).
    pub tracking_protection: bool,
    /// Where cookies and site data are kept when not private.
    pub data_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Left,
    Middle,
    Right,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { ctrl: false, shift: false, alt: false, logo: false };
    /// The "command" modifier: Ctrl on Linux and Windows, Cmd on macOS.
    pub fn cmd(&self) -> bool {
        if cfg!(target_os = "macos") { self.logo } else { self.ctrl }
    }
}

/// Keys, independent of any OS's key codes. `Char` holds the logical character in lower case.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Space,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),
    Alt,
    Ctrl,
    Shift,
    Logo,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Default,
    Pointer,
    Text,
    Move,
    Grab,
    Grabbing,
    Crosshair,
    ResizeNS,
    ResizeEW,
    ResizeNWSE,
    ResizeNESW,
    NotAllowed,
}

/// Which window edge or corner a resize drag starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

/// An RGBA image, 8 bits per channel, not premultiplied.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// What an app can ask of the window and the system.
pub trait Host {
    fn redraw(&mut self);
    fn set_title(&mut self, title: &str);
    fn set_cursor(&mut self, cursor: Cursor);
    fn drag_window(&mut self);
    fn drag_resize(&mut self, edge: Edge);
    fn minimize(&mut self);
    fn toggle_maximize(&mut self);
    fn is_maximized(&self) -> bool;
    fn close(&mut self);
    /// The system's light/dark preference, when it says.
    fn system_dark(&self) -> Option<bool>;
    fn clipboard_text(&mut self) -> Option<String>;
    fn set_clipboard_text(&mut self, text: &str);
    fn clipboard_image(&mut self) -> Option<Image>;
    fn set_clipboard_image(&mut self, img: &Image);
    /// Time since the app started (tests may run on a virtual clock).
    fn now(&self) -> Duration;

    // ---- web views (apps run with run_web); other hosts have none
    /// A new, hidden web view.
    fn web_new_view(&mut self) -> Option<ViewId> {
        None
    }
    /// Show this view below the app's own drawing (the others are hidden).
    fn web_show(&mut self, _view: ViewId) {}
    fn web_close(&mut self, _view: ViewId) {}
    fn web(&mut self, _view: ViewId, _cmd: WebCommand) {}
    /// The app draws the top `height` pixels of the window (its chrome) and takes the mouse in
    /// `input` (x, y, w, h, in pixels: the chrome and any open menu); the page fills the rest.
    fn set_chrome(&mut self, _height: u32, _input: &[(u32, u32, u32, u32)]) {}
    /// Take the keyboard for the app's own drawing (the address bar, a find box).
    fn focus_chrome(&mut self) {}
    /// The key event being handled was used by the app: don't pass it on to the page.
    fn set_handled(&mut self) {}
}

/// An app run by the platform layer.
pub trait App {
    fn event(&mut self, ev: &Event, host: &mut dyn Host);
    /// Draw the whole window into `pixels` (premultiplied RGBA, `w * h * 4` bytes).
    fn paint(&mut self, pixels: &mut [u8], w: u32, h: u32, host: &mut dyn Host);
    /// When the app next wants an [`Event::Tick`], if at all.
    fn next_tick(&self) -> Option<Duration> {
        None
    }
    fn wants_exit(&self) -> bool {
        false
    }
    fn exit_code(&self) -> i32 {
        0
    }
}

pub struct WindowSpec {
    pub title: String,
    pub app_id: String,
    pub width: u32,
    pub height: u32,
    pub icon: Option<Image>,
}

/// A host with no window, for tests and command-line work: an in-memory clipboard and a
/// virtual clock that tests advance.
#[derive(Default)]
pub struct Headless {
    pub title: String,
    pub cursor: Option<Cursor>,
    pub text: Option<String>,
    pub image: Option<Image>,
    pub clock: Duration,
    pub maximized: bool,
    pub closed: bool,
    pub redraws: u32,
    pub dark: Option<bool>,
    /// Web views and what was asked of them, for tests: "new 1", "show 1", "1 Load(..)".
    pub web_log: Vec<String>,
    pub web_views: u32,
    pub chrome: (u32, Vec<(u32, u32, u32, u32)>),
    pub handled: bool,
    pub chrome_focus: bool,
}

impl Host for Headless {
    fn redraw(&mut self) {
        self.redraws += 1;
    }
    fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }
    fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = Some(cursor);
    }
    fn drag_window(&mut self) {}
    fn drag_resize(&mut self, _edge: Edge) {}
    fn minimize(&mut self) {}
    fn toggle_maximize(&mut self) {
        self.maximized = !self.maximized;
    }
    fn is_maximized(&self) -> bool {
        self.maximized
    }
    fn close(&mut self) {
        self.closed = true;
    }
    fn system_dark(&self) -> Option<bool> {
        self.dark
    }
    fn clipboard_text(&mut self) -> Option<String> {
        self.text.clone()
    }
    fn set_clipboard_text(&mut self, text: &str) {
        self.text = Some(text.to_string());
        self.image = None;
    }
    fn clipboard_image(&mut self) -> Option<Image> {
        self.image.clone()
    }
    fn set_clipboard_image(&mut self, img: &Image) {
        self.image = Some(img.clone());
        self.text = None;
    }
    fn now(&self) -> Duration {
        self.clock
    }
    fn web_new_view(&mut self) -> Option<ViewId> {
        self.web_views += 1;
        self.web_log.push(format!("new {}", self.web_views));
        Some(self.web_views)
    }
    fn web_show(&mut self, view: ViewId) {
        self.web_log.push(format!("show {view}"));
    }
    fn web_close(&mut self, view: ViewId) {
        self.web_log.push(format!("close {view}"));
    }
    fn web(&mut self, view: ViewId, cmd: WebCommand) {
        self.web_log.push(format!("{view} {cmd:?}"));
    }
    fn set_chrome(&mut self, height: u32, input: &[(u32, u32, u32, u32)]) {
        self.chrome = (height, input.to_vec());
    }
    fn focus_chrome(&mut self) {
        self.chrome_focus = true;
    }
    fn set_handled(&mut self) {
        self.handled = true;
    }
}

/// Recognises double and triple clicks from a stream of presses.
pub struct ClickCounter {
    last: Option<(Instant, f32, f32, Button)>,
    count: u32,
}

impl Default for ClickCounter {
    fn default() -> Self {
        ClickCounter { last: None, count: 0 }
    }
}

impl ClickCounter {
    pub fn press(&mut self, at: Instant, x: f32, y: f32, b: Button, scale: f32) -> u32 {
        let near = 5.0 * scale;
        match self.last {
            Some((t, lx, ly, lb))
                if lb == b && at.duration_since(t) < Duration::from_millis(450) && (lx - x).abs() < near && (ly - y).abs() < near =>
            {
                self.count += 1
            }
            _ => self.count = 1,
        }
        self.last = Some((at, x, y, b));
        self.count
    }
}

/// Where an app keeps its settings: `~/.config/<app>` on Linux, `%APPDATA%\<app>` on Windows,
/// `~/Library/Application Support/<app>` on macOS. `FOLIO_CONFIG_DIR` overrides it (tests).
pub fn config_dir(app: &str) -> PathBuf {
    if let Some(d) = std::env::var_os("FOLIO_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home_dir)
    } else if cfg!(target_os = "macos") {
        home_dir().join("Library/Application Support")
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home_dir().join(".config"))
    };
    base.join(app)
}

pub fn home_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\"))
    } else {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
    }
}

/// The user's documents folder, falling back to the home folder.
pub fn documents_dir() -> PathBuf {
    let h = home_dir();
    for name in ["Documents", "Documenten", "Dokumente", "Documentos"] {
        let p = h.join(name);
        if p.is_dir() {
            return p;
        }
    }
    h
}

/// The folders the Open and Save panes list on the left: home, documents, the usual user
/// folders that exist, and the top of the file system.
pub fn places() -> Vec<(String, PathBuf)> {
    let home = home_dir();
    let mut v = vec![("Home".to_string(), home.clone()), ("Documents".to_string(), documents_dir())];
    for d in ["Desktop", "Pictures", "Downloads"] {
        let p = home.join(d);
        if p.is_dir() {
            v.push((d.into(), p));
        }
    }
    if cfg!(unix) {
        v.push(("Computer".into(), PathBuf::from("/")));
    } else if cfg!(windows) {
        for l in b'C'..=b'Z' {
            let p = PathBuf::from(format!("{}:\\", l as char));
            if p.is_dir() {
                v.push((format!("Drive {}:", l as char), p));
            }
        }
    }
    v
}

/// Print a PDF on the default printer. `copies` ≥ 1.
pub fn print_pdf(path: &Path, copies: u32, printer: Option<&str>) -> Result<(), String> {
    sys::print_pdf(path, copies, printer)
}

/// The system's printers, default first.
pub fn printers() -> Vec<String> {
    sys::printers()
}

/// Open a file or folder in the system's default app.
pub fn open_externally(path: &Path) -> Result<(), String> {
    sys::open_externally(path)
}

/// Run an app with no window (X3-14): the same events a window gives, painted into memory, on
/// a virtual clock. Returns the exit code, or `None` when the app went idle before exiting.
/// The last frame is left in `frame` (premultiplied RGBA).
pub fn run_headless(app: &mut dyn App, w: u32, h: u32, scale: f32, max_ticks: u32, frame: &mut Vec<u8>) -> Option<i32> {
    let mut host = Headless::default();
    frame.resize((w * h * 4) as usize, 0);
    app.event(&Event::Resized { w, h, scale }, &mut host);
    app.event(&Event::Focus(true), &mut host);
    for _ in 0..max_ticks {
        app.paint(frame, w, h, &mut host);
        if app.wants_exit() || host.closed {
            return Some(app.exit_code());
        }
        let d = app.next_tick()?;
        host.clock += d;
        app.event(&Event::Tick, &mut host);
    }
    app.paint(frame, w, h, &mut host);
    if app.wants_exit() { Some(app.exit_code()) } else { None }
}
