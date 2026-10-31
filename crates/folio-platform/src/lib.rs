//! Folio platform layer (X3-13): the only code in Folio that talks to the operating system.
//!
//! Apps and the toolkit see a window as a pixel buffer plus a stream of [`Event`]s, and reach
//! the system only through [`Host`]. A new platform means a new implementation of this crate's
//! backend; everything above it stays the same.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod backend;
pub mod sys;

pub use backend::run;

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
