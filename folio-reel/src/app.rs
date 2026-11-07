//! The Folio Reel window on the portable base (R3-11): the ribbon, the media
//! bin, the preview, the four-track timeline and the properties pane.
//!
//! Reel is the one app that cannot read its own files: H.264 and AAC are not
//! ours to write. So it hands each file to ffmpeg when the computer has it, and
//! says so plainly when it does not.
use folio_kit::chrome::{TitleAction, TitleBar, TITLE_H};
use folio_kit::files::{FileAction, FilePane};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::platform::media;
use folio_kit::platform::{Button, Cursor, Event, Host, Key, Mods};
use folio_kit::ribbon::{Ribbon, RibbonAction};
use folio_kit::text::{Align, Text};
use folio_kit::theme::Theme;
use folio_kit::ui::{CommandState, Ctx, Dialog, DialogResult, Menu, MenuItem, MenuResult};
use folio_reel_engine::edits::{default_text, Source};
use folio_reel_engine::project::{self as pr, Clip, Edge, Kind, Media, Project, EXPORT_PRESETS, TRACKS, TRANSITIONS};
use folio_reel_engine::render;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::preview::{Frames, Sound};
use crate::settings::Settings;
use crate::{icons, ribbon_defs, uitest::UiTest};

/// Reel's own navy, on the title bar and the highlights.
pub const NAVY: Color = Color::hex(0x1F3864);
const BAR_H: f32 = 24.0;
const RULER_H: f32 = 22.0;
const TRACK_H: f32 = 56.0;
const BIN_W: f32 = 210.0;
/// The strip down the left of the timeline that names each track.
const LABEL_W: f32 = 56.0;
const PROPS_W: f32 = 230.0;
/// The File pages, down the side of the backstage.
const BACK_PAGES: &[(&str, &str)] = &[("new", "New"), ("open", "Open"), ("save", "Save"), ("saveas", "Save As"), ("export", "Export"), ("close", "Back")];

/// What the ribbon asks about to grey buttons out.
pub struct State {
    pub can_undo: bool,
    pub can_redo: bool,
    pub can_paste: bool,
    pub has_clip: bool,
    pub playing: bool,
    pub snapping: bool,
    pub waveforms: bool,
    pub show_bin: bool,
    pub show_props: bool,
    pub dark: bool,
    pub muted: bool,
    pub music_bed: bool,
    pub chroma: bool,
    pub volume: String,
    pub fade_in: String,
    pub fade_out: String,
    pub speed: String,
    pub transition: String,
    pub brightness: String,
    pub contrast: String,
    pub saturation: String,
    pub size: String,
    pub fps: String,
}

impl CommandState for State {
    fn enabled(&self, id: &str) -> bool {
        match id {
            "undo" => self.can_undo,
            "redo" => self.can_redo,
            "paste" => self.can_paste,
            "cut" | "copy" | "split" | "delete" | "ripple-delete" | "trim-start" | "trim-end" | "group" | "mute" | "music-bed"
            | "chroma" | "panzoom" | "volume" | "fade-in" | "fade-out" | "speed" | "transition" | "brightness" | "contrast"
            | "saturation" | "to-video" | "to-audio" => self.has_clip,
            _ => true,
        }
    }

    fn checked(&self, id: &str) -> bool {
        match id {
            "snapping" => self.snapping,
            "waveforms" => self.waveforms,
            "show-bin" => self.show_bin,
            "show-props" => self.show_props,
            "dark" => self.dark,
            "mute" => self.muted,
            "music-bed" => self.music_bed,
            "chroma" => self.chroma,
            _ => false,
        }
    }

    fn text(&self, id: &str) -> Option<String> {
        match id {
            "volume" => Some(self.volume.clone()),
            "fade-in" => Some(self.fade_in.clone()),
            "fade-out" => Some(self.fade_out.clone()),
            "speed" => Some(self.speed.clone()),
            "transition" => Some(self.transition.clone()),
            "brightness" => Some(self.brightness.clone()),
            "contrast" => Some(self.contrast.clone()),
            "saturation" => Some(self.saturation.clone()),
            "project-size" => Some(self.size.clone()),
            "project-fps" => Some(self.fps.clone()),
            "play" => Some(if self.playing { "Pause".into() } else { "Play".into() }),
            _ => None,
        }
    }
}

/// The backstage, when it is up.
pub struct Back {
    pub page: String,
    pub files: Option<FilePane>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    None,
    Backstage,
}

/// What is being dragged on the timeline.
#[derive(Clone, Copy, PartialEq)]
pub enum Drag {
    None,
    Playhead,
    Clip { id: u64, grab: f64, track: usize },
    Trim { id: u64, edge: Edge },
}

pub struct Res {
    pub text: Text,
    pub icons: Icons,
}

/// How an export is getting on.
pub struct Job {
    pub done: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub total: f64,
    pub out: PathBuf,
    pub result: std::sync::mpsc::Receiver<Result<(), String>>,
}

pub struct ReelApp {
    pub p: Project,
    pub playhead: f64,
    pub sel: Vec<u64>,
    /// seconds across one pixel of the timeline
    pub scale: f64,
    pub scroll: f64,
    pub playing: bool,
    pub frames: Frames,
    pub sound: Sound,
    pub bin_sel: Option<usize>,
    pub waves: Vec<(String, Vec<f32>)>,
    /// titles already drawn for the preview, keyed by clip and the words on it
    pub drawn: Vec<(u64, String, std::sync::Arc<sk::Pixmap>)>,
    pub job: Option<Job>,
    pub res: Option<Box<Res>>,
    pub theme: Theme,
    pub ribbon: Ribbon,
    pub title: TitleBar,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub flash: Option<(String, Duration)>,
    pub now: Duration,
    pub last_tick: Duration,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub mods: Mods,
    pub mouse: (f32, f32),
    pub drag: Drag,
    pub clipboard: Vec<(usize, Clip, f64, Option<Media>)>,
    pub pane: Pane,
    pub back: Option<Back>,
    pub settings: Settings,
    pub preview: Rect,
    pub bin: Rect,
    pub props: Rect,
    pub timeline: Rect,
    pub bin_hits: Vec<(usize, Rect)>,
    pub closing: bool,
    pub exit: bool,
    pub exit_code: i32,
    pub test: Option<UiTest>,
    /// true once the sound of the timeline needs rendering again
    pub sound_stale: bool,
    pm: Option<sk::Pixmap>,
}

impl ReelApp {
    pub fn new(p: Project) -> ReelApp {
        let mut ic = Icons::new();
        icons::install(&mut ic);
        let settings = Settings::load();
        let mut a = ReelApp {
            p,
            playhead: 0.0,
            sel: Vec::new(),
            scale: 0.02,
            scroll: 0.0,
            playing: false,
            frames: Frames::new(),
            sound: Sound::new(),
            bin_sel: None,
            waves: Vec::new(),
            drawn: Vec::new(),
            job: None,
            res: Some(Box::new(Res { text: Text::bundled(), icons: ic })),
            theme: Theme::new(settings.dark, NAVY, NAVY),
            ribbon: Ribbon::new(ribbon_defs::tabs()),
            title: TitleBar::new("reel", vec![("save", "save", "Save (Ctrl+S)"), ("undo", "undo", "Undo (Ctrl+Z)"), ("redo", "redo", "Redo (Ctrl+Y)")]),
            menu: None,
            dialog: None,
            flash: None,
            now: Duration::ZERO,
            last_tick: Duration::ZERO,
            w: 1280.0,
            h: 860.0,
            s: 1.0,
            mods: Mods::NONE,
            mouse: (0.0, 0.0),
            drag: Drag::None,
            clipboard: Vec::new(),
            pane: Pane::None,
            back: None,
            settings,
            preview: Rect::default(),
            bin: Rect::default(),
            props: Rect::default(),
            timeline: Rect::default(),
            bin_hits: Vec::new(),
            closing: false,
            exit: false,
            exit_code: 0,
            test: None,
            sound_stale: true,
            pm: None,
        };
        a.p.snapping = a.settings.snapping;
        a
    }

    pub fn say(&mut self, title: &str, text: &str) {
        self.dialog = Some(Dialog::new("say", title).note(text).button("ok", "OK"));
    }

    pub fn flash(&mut self, msg: &str) {
        self.flash = Some((msg.to_string(), self.now + Duration::from_secs(5)));
    }

    pub fn doc_name(&self) -> String {
        self.p.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Film1.reel".into())
    }

    pub fn window_title(&self) -> String {
        format!("{}{} - Folio Reel", if self.p.dirty { "*" } else { "" }, self.doc_name())
    }

    pub fn one(&self) -> Option<&Clip> {
        self.sel.first().and_then(|id| self.p.clip(*id))
    }

    pub fn state(&self) -> State {
        let c = self.one();
        let num = |v: f64| format!("{:.2}", v);
        State {
            can_undo: self.p.can_undo(),
            can_redo: self.p.can_redo(),
            can_paste: !self.clipboard.is_empty(),
            has_clip: c.is_some(),
            playing: self.playing,
            snapping: self.settings.snapping,
            waveforms: self.settings.waveforms,
            show_bin: self.settings.show_bin,
            show_props: self.settings.show_props,
            dark: self.settings.dark,
            muted: c.map(|c| c.muted).unwrap_or(false),
            music_bed: c.map(|c| c.music_bed).unwrap_or(false),
            chroma: c.map(|c| c.chroma).unwrap_or(false),
            volume: c.map(|c| num(c.volume)).unwrap_or_default(),
            fade_in: c.map(|c| num(c.fade_in)).unwrap_or_default(),
            fade_out: c.map(|c| num(c.fade_out)).unwrap_or_default(),
            speed: c.map(|c| format!("{:.2}x", c.speed)).unwrap_or_default(),
            transition: c
                .map(|c| match &c.transition {
                    None => "None".to_string(),
                    Some(t) => TRANSITIONS.iter().find(|(k, _)| *k == t.kind).map(|(_, n)| n.to_string()).unwrap_or_else(|| t.kind.clone()),
                })
                .unwrap_or_default(),
            brightness: c.map(|c| num(c.color.brightness)).unwrap_or_default(),
            contrast: c.map(|c| num(c.color.contrast)).unwrap_or_default(),
            saturation: c.map(|c| num(c.color.saturation)).unwrap_or_default(),
            size: format!("{} x {}", self.p.data.width, self.p.data.height),
            fps: format!("{:.0}", self.p.data.fps),
        }
    }

    // ---------------------------------------------------------------- media
    /// Ask the computer what a file is, and put it in the bin.
    pub fn bring_in(&mut self, path: &Path) {
        if !media::have() {
            let m = media::missing_message();
            self.say("No ffmpeg on this computer", &m);
            return;
        }
        let info = match media::probe(path) {
            Ok(i) => i,
            Err(e) => {
                self.say("Can't read that file", &e);
                return;
            }
        };
        if !info.has_video && !info.has_audio {
            self.say("Can't read that file", &format!("There's no picture and no sound in {}.", path.display()));
            return;
        }
        let m = Media {
            uri: pr::path_to_uri(path),
            name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            duration: info.duration,
            width: info.width,
            height: info.height,
            fps: info.fps,
            has_video: info.has_video,
            has_audio: info.has_audio,
            is_image: info.is_image(),
            relative: String::new(),
        };
        let name = m.name.clone();
        self.p.add_media(m);
        self.bin_sel = Some(self.p.data.media.len() - 1);
        self.flash(&format!("{} is in the bin.", name));
    }

    /// Put the bin's chosen file on a track.
    pub fn place(&mut self, track: Option<usize>) {
        let Some(i) = self.bin_sel else {
            self.flash("Pick something in the media bin first.");
            return;
        };
        let Some(m) = self.p.data.media.get(i).cloned() else { return };
        let ti = track.unwrap_or_else(|| self.p.media_track(&m));
        match self.p.add_clip(ti, Source::Media(&m), None, false) {
            Ok(id) => {
                self.sel = vec![id];
                self.touched();
            }
            Err(e) => self.say("Can't put that there", &e.0),
        }
    }

    /// After anything that changes the film.
    pub fn touched(&mut self) {
        self.sound_stale = true;
        self.fit_if_empty();
    }

    fn fit_if_empty(&mut self) {
        let d = self.p.duration();
        if d > 0.0 && self.timeline.w > 10.0 && self.scale * ((self.timeline.w - LABEL_W * self.s) as f64) < d * 0.5 {
            self.zoom_fit();
        }
    }

    pub fn zoom_fit(&mut self) {
        let d = self.p.duration().max(1.0);
        let w = (self.timeline.w - (LABEL_W + 60.0) * self.s).max(100.0) as f64;
        self.scale = (d * 1.05 / w).max(0.0005);
        self.scroll = 0.0;
    }

    // ---------------------------------------------------------------- the timeline's shape
    pub fn track_rect(&self, ti: usize) -> Rect {
        let s = self.s;
        let top = self.timeline.y + RULER_H * s;
        Rect::new(self.timeline.x, top + ti as f32 * TRACK_H * s, self.timeline.w, TRACK_H * s)
    }

    /// Where the clips begin: past the strip that names the tracks.
    pub fn lane_x(&self) -> f32 {
        self.timeline.x + LABEL_W * self.s
    }

    pub fn time_to_x(&self, t: f64) -> f32 {
        self.lane_x() + ((t - self.scroll) / self.scale) as f32
    }

    pub fn x_to_time(&self, x: f32) -> f64 {
        self.scroll + (x - self.lane_x()) as f64 * self.scale
    }

    pub fn clip_rect(&self, ti: usize, c: &Clip) -> Rect {
        let r = self.track_rect(ti);
        let x0 = self.time_to_x(c.start);
        let x1 = self.time_to_x(c.end());
        Rect::new(x0, r.y + 3.0 * self.s, (x1 - x0).max(2.0), r.h - 6.0 * self.s)
    }

    pub fn clip_at(&self, x: f32, y: f32) -> Option<(usize, u64)> {
        if x < self.lane_x() {
            return None;
        }
        for ti in 0..TRACKS.len() {
            let r = self.track_rect(ti);
            if y < r.y || y >= r.bottom() {
                continue;
            }
            for c in self.p.track(ti) {
                let cr = self.clip_rect(ti, c);
                if x >= cr.x && x < cr.right() {
                    return Some((ti, c.id));
                }
            }
        }
        None
    }

    pub fn track_at(&self, y: f32) -> Option<usize> {
        (0..TRACKS.len()).find(|ti| {
            let r = self.track_rect(*ti);
            y >= r.y && y < r.bottom()
        })
    }

    /// How far a drag may be from an edge and still count as a trim.
    fn grip(&self) -> f32 {
        6.0 * self.s
    }

    // ---------------------------------------------------------------- the preview
    fn want_frame(&mut self) {
        let t = self.playhead;
        // the topmost clip that has a picture of its own; titles go over it
        let seen = render::visible_at(&self.p, t);
        let found = seen
            .iter()
            .rev()
            .find(|(_, id)| self.p.clip(*id).map(|c| !c.kind.is_text()).unwrap_or(false))
            .copied();
        let Some((_, id)) = found else {
            self.frames.forget();
            return;
        };
        let Some(c) = self.p.clip(id) else { return };
        let Some(m) = self.p.media_by_uri(&c.media) else { return };
        let Some(path) = m.path() else { return };
        let at = render::media_time(c, t);
        let width = (self.preview.w.max(160.0) as u32).min(960) & !1;
        self.frames.ask(&path, at, width.max(32));
    }

    /// Render the timeline's sound so the preview can play it.
    fn want_sound(&mut self) {
        if !self.sound_stale || !media::have() {
            return;
        }
        self.sound_stale = false;
        let raw = media::cache_dir().join("preview.raw");
        match render::audio_command(&self.p, &raw) {
            Ok(cmd) => self.sound.render(cmd.args, raw),
            Err(_) => {}
        }
    }

    pub fn toggle_play(&mut self) {
        if self.playing {
            self.playing = false;
            self.sound.pause();
            return;
        }
        if self.p.duration() <= 0.0 {
            self.flash("There's nothing on the timeline yet.");
            return;
        }
        self.want_sound();
        self.playing = true;
        self.last_tick = self.now;
        self.sound.play(self.playhead);
    }

    pub fn seek(&mut self, t: f64) {
        self.playhead = t.clamp(0.0, self.p.duration().max(0.0));
        self.sound.seek(self.playhead);
        self.want_frame();
    }

    // ---------------------------------------------------------------- files
    pub fn save_to(&mut self, path: Option<PathBuf>) -> bool {
        let p = path.or_else(|| self.p.path.clone());
        let Some(p) = p else {
            self.open_backstage("saveas");
            return false;
        };
        match pr::save(&mut self.p, Some(&p)) {
            Ok(()) => {
                self.settings.remember(&p);
                self.flash(&format!("Saved {}.", p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
                true
            }
            Err(e) => {
                self.say("Can't save the film", &e.0);
                false
            }
        }
    }

    pub fn open_path(&mut self, p: &Path) {
        match pr::open(p) {
            Ok(mut np) => {
                np.snapping = self.settings.snapping;
                let missing: Vec<String> = np.missing_media().iter().map(|m| m.name.clone()).collect();
                self.p = np;
                self.sel.clear();
                self.playhead = 0.0;
                self.bin_sel = if self.p.data.media.is_empty() { None } else { Some(0) };
                self.frames.forget();
                self.waves.clear();
                self.settings.remember(p);
                self.touched();
                self.zoom_fit();
                if !missing.is_empty() {
                    self.say("Some files are missing", &format!("These aren't where the film expects them:\n{}", missing.join("\n")));
                }
            }
            Err(e) => self.say("Can't open the film", &e.0),
        }
    }

    pub fn export_to(&mut self, out: PathBuf, preset: &str, text: &mut Text) {
        if !media::have() {
            let m = media::missing_message();
            self.say("No ffmpeg on this computer", &m);
            return;
        }
        let Some((_, fmt, size)) = EXPORT_PRESETS.iter().find(|(n, _, _)| *n == preset).copied() else {
            self.say("Can't export", "That isn't one of the sizes Reel offers.");
            return;
        };
        let target = render::Target::of(&self.p, fmt, size);
        // the titles are ours to draw, so draw them before ffmpeg is asked for anything
        let titles = folio_reel_engine::titles::write_all(&self.p, target.width, target.height, &media::cache_dir(), text);
        let cmd = match render::export_command_with_titles(&self.p, &target, &out, &titles) {
            Ok(c) => c,
            Err(e) => {
                self.say("Can't export", &e.0);
                return;
            }
        };
        let done = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (tx, rx) = std::sync::mpsc::channel();
        let d = done.clone();
        let args = cmd.args.clone();
        std::thread::spawn(move || {
            let r = media::export(&args, cmd.duration, |secs| {
                d.store((secs * 1000.0) as u64, std::sync::atomic::Ordering::Relaxed);
            });
            let _ = tx.send(r);
        });
        self.flash(&format!("Writing {}...", out.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
        self.job = Some(Job { done, total: cmd.duration, out, result: rx });
    }

    fn poll_job(&mut self) {
        let Some(job) = &self.job else { return };
        match job.result.try_recv() {
            Ok(Ok(())) => {
                let name = job.out.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let where_ = job.out.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                self.job = None;
                self.say("The film is written", &format!("{} is in {}.", name, where_));
            }
            Ok(Err(e)) => {
                self.job = None;
                self.say("The export didn't finish", &e);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(_) => self.job = None,
        }
    }

    pub fn ask_to_close(&mut self) {
        if !self.p.dirty {
            self.exit = true;
            return;
        }
        self.closing = true;
        self.dialog = Some(
            Dialog::new("close", "Save the film?")
                .note(&format!("{} has changes that aren't saved.", self.doc_name()))
                .button("save", "Save")
                .button("discard", "Don't Save")
                .button("cancel", "Cancel"),
        );
    }

    pub fn open_backstage(&mut self, page: &str) {
        self.pane = Pane::Backstage;
        self.back = Some(Back { page: page.to_string(), files: None });
        self.backstage_page(page);
    }

    fn file_pane(&self, save: bool, exts: &[&str], name: &str) -> FilePane {
        let dir = self
            .p
            .path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(folio_kit::platform::home_dir);
        FilePane::new(&dir, exts, save, name)
    }

    fn backstage_page(&mut self, id: &str) {
        let stem = self
            .p
            .path
            .as_ref()
            .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "Film1".into());
        let pane = match id {
            "open" => Some(self.file_pane(false, &["reel"], "")),
            "save" | "saveas" => Some(self.file_pane(true, &["reel"], &format!("{stem}.reel"))),
            "export" => Some(self.file_pane(true, &["mp4", "webm"], &format!("{stem}.mp4"))),
            _ => None,
        };
        if let Some(b) = self.back.as_mut() {
            b.page = id.to_string();
            b.files = pane;
        }
        if id == "new" {
            self.pane = Pane::None;
            self.back = None;
            let keep = self.settings.snapping;
            self.p = Project::new();
            self.p.snapping = keep;
            self.sel.clear();
            self.playhead = 0.0;
            self.bin_sel = None;
            self.frames.forget();
            self.touched();
        }
        if id == "close" {
            self.pane = Pane::None;
            self.back = None;
        }
    }
}

// -------------------------------------------------------------------- drawing
impl ReelApp {
    pub fn paint_into(&mut self, pixels: &mut [u8], w: u32, h: u32) {
        let mut pm = match self.pm.take() {
            Some(p) if p.width() == w && p.height() == h => p,
            _ => sk::Pixmap::new(w.max(1), h.max(1)).unwrap(),
        };
        let mut r = self.res.take().expect("res");
        self.paint(&mut pm, &mut r);
        self.res = Some(r);
        pixels.copy_from_slice(pm.data());
        self.pm = Some(pm);
    }

    pub fn paint(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        self.w = pm.width() as f32;
        self.h = pm.height() as f32;
        let t = self.theme.clone();
        let s = self.s;
        pm.fill(t.window.to_sk());
        let ribbon_h = self.ribbon.height(s);
        let top = TITLE_H * s + ribbon_h;
        let tl_h = (RULER_H + TRACK_H * TRACKS.len() as f32) * s + 10.0 * s;
        let body_h = (self.h - top - tl_h - BAR_H * s).max(60.0);
        let bin_w = if self.settings.show_bin { BIN_W * s } else { 0.0 };
        let props_w = if self.settings.show_props { PROPS_W * s } else { 0.0 };
        self.bin = Rect::new(0.0, top, bin_w, body_h);
        self.props = Rect::new(self.w - props_w, top, props_w, body_h);
        self.preview = Rect::new(bin_w, top, (self.w - bin_w - props_w).max(40.0), body_h);
        self.timeline = Rect::new(0.0, top + body_h, self.w, tl_h);

        self.paint_preview(pm, r);
        if self.settings.show_bin {
            self.paint_bin(pm, r);
        }
        if self.settings.show_props {
            self.paint_props(pm, r);
        }
        self.paint_timeline(pm, r);
        self.paint_bar(pm, r);

        let title = self.window_title();
        {
            let st = self.state();
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            self.title.paint(&mut cx, Rect::new(0.0, 0.0, self.w, TITLE_H * s), &title, false, &|id| match id {
                "undo" => st.can_undo,
                "redo" => st.can_redo,
                _ => true,
            });
        }
        let st = self.state();
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            self.ribbon.paint(&mut cx, Rect::new(0.0, TITLE_H * s, self.w, ribbon_h), &st, self.now);
        }
        if self.pane == Pane::Backstage {
            self.paint_backstage(pm, r);
        }
        if let Some(m) = self.menu.as_mut() {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            m.paint(&mut cx, Rect::new(0.0, 0.0, self.w, self.h));
        }
        if let Some(d) = self.dialog.as_mut() {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            d.paint(&mut cx, Rect::new(0.0, 0.0, self.w, self.h));
        }
    }

    fn paint_preview(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let (t, s, area) = (self.theme.clone(), self.s, self.preview);
        gfx::fill_rect(pm, area, Color::hex(0x101216));
        // the picture, letterboxed in the middle
        let (pw, ph) = (self.p.data.width.max(1) as f32, self.p.data.height.max(1) as f32);
        let pad = 12.0 * s;
        let avail = Rect::new(area.x + pad, area.y + pad, (area.w - pad * 2.0).max(10.0), (area.h - pad * 2.0).max(10.0));
        let k = (avail.w / pw).min(avail.h / ph);
        let (dw, dh) = (pw * k, ph * k);
        let screen = Rect::new(avail.x + (avail.w - dw) / 2.0, avail.y + (avail.h - dh) / 2.0, dw, dh);
        gfx::fill_rect(pm, screen, Color::BLACK);
        if let Some(shot) = self.frames.shot.as_ref() {
            let pic = shot.pm.clone();
            let sx = screen.w / pic.width().max(1) as f32;
            let sy = screen.h / pic.height().max(1) as f32;
            pm.draw_pixmap(
                0,
                0,
                pic.as_ref().as_ref(),
                &sk::PixmapPaint::default(),
                sk::Transform::from_translate(screen.x, screen.y).pre_scale(sx, sy),
                None,
            );
        }
        // titles over the top: the very picture the export will hold, scaled down
        let showing: Vec<u64> = render::visible_at(&self.p, self.playhead).into_iter().map(|(_, id)| id).collect();
        let titles: Vec<u64> = showing.iter().copied().filter(|id| self.p.clip(*id).map(|c| c.kind.is_text()).unwrap_or(false)).collect();
        let any_title = !titles.is_empty();
        for id in titles {
            let pic = self.title_picture(id, &mut r.text);
            if let Some(pic) = pic {
                let sx = screen.w / pic.width().max(1) as f32;
                let sy = screen.h / pic.height().max(1) as f32;
                pm.draw_pixmap(
                    0,
                    0,
                    pic.as_ref().as_ref(),
                    &sk::PixmapPaint::default(),
                    sk::Transform::from_translate(screen.x, screen.y).pre_scale(sx, sy),
                    None,
                );
            }
        }
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        let _ = any_title;
        let empty = self.p.duration() <= 0.0;
        if empty {
            let msg = if media::have() {
                "Add media, then drop it on a track."
            } else {
                "This computer has no ffmpeg, so Reel can't read video here."
            };
            cx.label(msg, Rect::new(screen.x, screen.y + screen.h / 2.0 - 10.0 * s, screen.w, 20.0 * s), 13.0, Color::hex(0x9AA3AF), false, Align::Center);
        } else if self.frames.busy && !any_title {
            // the decoder is behind; say so rather than leave the picture looking wrong
            let behind = self
                .frames
                .shot
                .as_ref()
                .map(|sh| {
                    let want = render::visible_at(&self.p, self.playhead)
                        .last()
                        .and_then(|(_, id)| self.p.clip(*id))
                        .map(|c| render::media_time(c, self.playhead))
                        .unwrap_or(sh.at);
                    (want - sh.at).abs() > 0.3
                })
                .unwrap_or(true);
            if behind {
                cx.label(
                    "Catching up...",
                    Rect::new(screen.x, screen.bottom() - 24.0 * s, screen.w - 8.0 * s, 16.0 * s),
                    11.5,
                    Color::hex(0x9AA3AF),
                    false,
                    Align::Right,
                );
            }
        }
        // time, bottom right of the preview
        let d = self.p.duration();
        let time = format!("{} / {}", pr::fmt_time(self.playhead, Some(self.p.data.fps)), pr::fmt_time(d, Some(self.p.data.fps)));
        cx.label(&time, Rect::new(area.x, area.bottom() - 20.0 * s, area.w - 10.0 * s, 16.0 * s), 12.0, Color::hex(0xB8C0CC), false, Align::Right);
        if let Some(job) = &self.job {
            let got = job.done.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0;
            let frac = (got / job.total.max(0.001)).clamp(0.0, 1.0);
            let bar = Rect::new(area.x + 20.0 * s, area.bottom() - 46.0 * s, area.w - 40.0 * s, 8.0 * s);
            gfx::fill_round(cx.pm, bar, 4.0 * s, Color::hex(0x2A3240));
            gfx::fill_round(cx.pm, Rect::new(bar.x, bar.y, bar.w * frac as f32, bar.h), 4.0 * s, NAVY.mix(Color::WHITE, 0.35));
            cx.label(
                &format!("Writing the film — {:.0}%", frac * 100.0),
                Rect::new(bar.x, bar.y - 18.0 * s, bar.w, 16.0 * s),
                12.0,
                Color::hex(0xD8DEE8),
                false,
                Align::Left,
            );
        }
    }

    /// The picture of one title, drawn once and kept until its words change.
    fn title_picture(&mut self, id: u64, text: &mut Text) -> Option<std::sync::Arc<sk::Pixmap>> {
        let c = self.p.clip(id)?;
        if c.text.trim().is_empty() {
            return None;
        }
        let key = format!("{}|{}|{}|{}", c.text, c.font, c.halign, c.valign);
        if let Some((_, _, pm)) = self.drawn.iter().find(|(i, k, _)| *i == id && *k == key) {
            return Some(pm.clone());
        }
        // small enough to draw quickly, big enough to look right on the preview
        let w = 640u32;
        let h = ((w as f64 * self.p.data.height as f64 / self.p.data.width.max(1) as f64) as u32).max(2);
        let pm = std::sync::Arc::new(folio_reel_engine::titles::draw(c, w, h, text));
        self.drawn.retain(|(i, _, _)| *i != id);
        self.drawn.push((id, key, pm.clone()));
        if self.drawn.len() > 24 {
            self.drawn.remove(0);
        }
        Some(pm)
    }

    fn paint_bin(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let (t, s, area) = (self.theme.clone(), self.s, self.bin);
        gfx::fill_rect(pm, area, t.panel);
        gfx::fill_rect(pm, Rect::new(area.right() - 1.0, area.y, 1.0, area.h), t.border);
        let sel = self.bin_sel;
        let items: Vec<(String, String)> = self
            .p
            .data
            .media
            .iter()
            .map(|m| {
                let kind = if m.is_image {
                    "picture".to_string()
                } else if m.has_video {
                    format!("{} video", pr::fmt_time(m.duration, None))
                } else {
                    format!("{} sound", pr::fmt_time(m.duration, None))
                };
                (m.name.clone(), kind)
            })
            .collect();
        self.bin_hits.clear();
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        cx.label("Media Bin", Rect::new(area.x + 10.0 * s, area.y + 6.0 * s, area.w - 20.0 * s, 20.0 * s), 12.0, t.muted, true, Align::Left);
        let mut y = area.y + 30.0 * s;
        for (i, (name, kind)) in items.iter().enumerate() {
            let row = Rect::new(area.x + 4.0 * s, y, area.w - 8.0 * s, 38.0 * s);
            if Some(i) == sel {
                gfx::fill_round(cx.pm, row, 4.0 * s, t.selection);
            }
            cx.label(name, Rect::new(row.x + 8.0 * s, row.y + 3.0 * s, row.w - 12.0 * s, 18.0 * s), 12.5, t.text, false, Align::Left);
            cx.label(kind, Rect::new(row.x + 8.0 * s, row.y + 19.0 * s, row.w - 12.0 * s, 14.0 * s), 11.0, t.muted, false, Align::Left);
            self.bin_hits.push((i, row));
            y += 40.0 * s;
            if y > area.bottom() - 20.0 * s {
                break;
            }
        }
        if items.is_empty() {
            cx.label("Nothing here yet.", Rect::new(area.x + 10.0 * s, y, area.w - 20.0 * s, 18.0 * s), 12.0, t.muted, false, Align::Left);
        }
    }

    fn paint_props(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let (t, s, area) = (self.theme.clone(), self.s, self.props);
        gfx::fill_rect(pm, area, t.panel);
        gfx::fill_rect(pm, Rect::new(area.x, area.y, 1.0, area.h), t.border);
        let rows: Vec<(String, String)> = match self.one() {
            None => vec![
                ("Picture".into(), format!("{} x {}", self.p.data.width, self.p.data.height)),
                ("Frames a second".into(), format!("{:.0}", self.p.data.fps)),
                ("Length".into(), pr::fmt_time(self.p.duration(), None)),
                ("Clips".into(), format!("{}", self.p.all_clips().len())),
                ("Markers".into(), format!("{}", self.p.markers().len())),
            ],
            Some(c) => {
                let mut v = vec![
                    ("Name".into(), c.name.clone()),
                    ("Starts".into(), pr::fmt_time(c.start, Some(self.p.data.fps))),
                    ("Length".into(), pr::fmt_time(c.duration, Some(self.p.data.fps))),
                ];
                if c.kind == Kind::Media {
                    v.push(("In point".into(), pr::fmt_time(c.inpoint, Some(self.p.data.fps))));
                    v.push(("Speed".into(), format!("{:.2}x", c.speed)));
                    v.push(("Volume".into(), format!("{:.2}", c.volume)));
                } else {
                    v.push(("Words".into(), c.text.lines().next().unwrap_or("").to_string()));
                }
                if c.fade_in > 0.0 {
                    v.push(("Fade in".into(), format!("{:.2}s", c.fade_in)));
                }
                if c.fade_out > 0.0 {
                    v.push(("Fade out".into(), format!("{:.2}s", c.fade_out)));
                }
                if let Some(tr) = &c.transition {
                    let name = TRANSITIONS.iter().find(|(k, _)| *k == tr.kind).map(|(_, n)| *n).unwrap_or("Transition");
                    v.push(("Transition".into(), format!("{} {:.1}s", name, tr.duration)));
                }
                if c.music_bed {
                    v.push(("Music bed".into(), "dips under the voice".into()));
                }
                if c.chroma {
                    v.push(("Green screen".into(), c.chroma_color.clone()));
                }
                if !c.panzoom.is_empty() {
                    v.push(("Pan and zoom".into(), c.panzoom.clone()));
                }
                if c.group != 0 {
                    v.push(("Grouped".into(), format!("with {} more", pr::count(self.p.group_members(c.group).len().saturating_sub(1), "clip", "clips"))));
                }
                v
            }
        };
        let head = if self.one().is_some() { "Clip" } else { "Film" };
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        cx.label(head, Rect::new(area.x + 12.0 * s, area.y + 6.0 * s, area.w - 24.0 * s, 20.0 * s), 12.0, t.muted, true, Align::Left);
        let mut y = area.y + 30.0 * s;
        for (k, v) in rows {
            cx.label(&k, Rect::new(area.x + 12.0 * s, y, area.w * 0.48, 18.0 * s), 11.5, t.muted, false, Align::Left);
            cx.label(&v, Rect::new(area.x + area.w * 0.46, y, area.w * 0.5 - 12.0 * s, 18.0 * s), 11.5, t.text, false, Align::Right);
            y += 20.0 * s;
        }
    }

    fn paint_timeline(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let (t, s, area) = (self.theme.clone(), self.s, self.timeline);
        gfx::fill_rect(pm, area, t.panel.mix(Color::BLACK, 0.03));
        // the ruler
        let ruler = Rect::new(area.x, area.y, area.w, RULER_H * s);
        gfx::fill_rect(pm, ruler, t.panel);
        let _ = &ruler;
        let span = (area.w - LABEL_W * s) as f64 * self.scale;
        let step = [0.1f64, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0]
            .into_iter()
            .find(|v| v / self.scale > 70.0)
            .unwrap_or(600.0);
        let lane = self.lane_x();
        let first = (self.scroll / step).floor() * step;
        let marks: Vec<(f32, String)> = {
            let mut v = Vec::new();
            let mut tt = first;
            while tt <= self.scroll + span + step {
                let x = self.time_to_x(tt);
                if x >= lane && x <= area.right() {
                    v.push((x, pr::fmt_time(tt, None)));
                }
                tt += step;
            }
            v
        };
        // the tracks
        for ti in 0..TRACKS.len() {
            let tr = self.track_rect(ti);
            let shade = if ti % 2 == 0 { t.panel.mix(Color::BLACK, 0.06) } else { t.panel.mix(Color::BLACK, 0.02) };
            gfx::fill_rect(pm, tr, shade);
            gfx::fill_rect(pm, Rect::new(tr.x, tr.bottom() - 1.0, tr.w, 1.0), t.border);
        }
        // gridlines
        for (x, _) in &marks {
            gfx::fill_rect(pm, Rect::new(*x, area.y + RULER_H * s, 1.0, area.h - RULER_H * s), t.border.with_alpha(110));
        }
        // the clips
        let sel = self.sel.clone();
        let waveforms = self.settings.waveforms;
        let mut shapes: Vec<(Rect, Color, String, bool, Option<Vec<f32>>, bool, f64, f64)> = Vec::new();
        for ti in 0..TRACKS.len() {
            for c in self.p.track(ti) {
                let mut cr = self.clip_rect(ti, c);
                if cr.right() < lane || cr.x > area.right() {
                    continue;
                }
                if cr.x < lane {
                    cr = Rect::new(lane, cr.y, (cr.right() - lane).max(1.0), cr.h);
                }
                let base = match ti {
                    0 => Color::hex(0x7A5AA8),
                    1 => NAVY.mix(Color::WHITE, 0.25),
                    2 => Color::hex(0x2E7D6B),
                    _ => Color::hex(0x8A6A2E),
                };
                let wave = if waveforms && ti >= 2 {
                    self.waves.iter().find(|(k, _)| *k == c.media).map(|(_, v)| v.clone())
                } else {
                    None
                };
                shapes.push((cr, base, c.name.clone(), sel.contains(&c.id), wave, c.transition.is_some(), c.fade_in, c.fade_out));
            }
        }
        let markers: Vec<(f32, Color, String)> = self
            .p
            .markers()
            .iter()
            .map(|m| {
                let (rr, gg, bb) = pr::marker_rgb(&m.color);
                let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
                (self.time_to_x(m.t), Color::rgb(byte(rr), byte(gg), byte(bb)), m.name.clone())
            })
            .collect();
        let play_x = self.time_to_x(self.playhead);
        let names: Vec<&str> = TRACKS.iter().map(|n| pr::track_name(n)).collect();
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        for (x, label) in &marks {
            cx.label(label, Rect::new(*x + 3.0 * s, ruler.y + 2.0 * s, 60.0 * s, 16.0 * s), 10.5, t.muted, false, Align::Left);
            gfx::fill_rect(cx.pm, Rect::new(*x, ruler.bottom() - 5.0 * s, 1.0, 5.0 * s), t.muted);
        }
        for (cr, base, name, chosen, wave, has_tr, fi, fo) in shapes {
            gfx::fill_round(cx.pm, cr, 3.0 * s, base);
            if let Some(w) = wave {
                let n = w.len().max(1);
                for (i, v) in w.iter().enumerate() {
                    let x = cr.x + cr.w * i as f32 / n as f32;
                    let hh = (cr.h - 14.0 * s) * v.clamp(0.0, 1.0);
                    gfx::fill_rect(cx.pm, Rect::new(x, cr.y + (cr.h - hh) / 2.0 + 4.0 * s, (cr.w / n as f32).max(1.0), hh), base.mix(Color::WHITE, 0.45));
                }
            }
            if fi > 0.0 {
                let px = (fi / self.scale) as f32;
                gfx::fill_round(cx.pm, Rect::new(cr.x, cr.y, px.min(cr.w), 3.0 * s), 1.5 * s, Color::WHITE.with_alpha(150));
            }
            if fo > 0.0 {
                let px = (fo / self.scale) as f32;
                gfx::fill_round(cx.pm, Rect::new(cr.right() - px.min(cr.w), cr.y, px.min(cr.w), 3.0 * s), 1.5 * s, Color::WHITE.with_alpha(150));
            }
            if has_tr {
                gfx::fill_rect(cx.pm, Rect::new(cr.x, cr.y, 3.0 * s, cr.h), Color::hex(0xFFD166));
            }
            if chosen {
                gfx::stroke_round(cx.pm, cr, 3.0 * s, Color::WHITE, 2.0 * s);
            }
            if cr.w > 30.0 * s {
                cx.label(&name, Rect::new(cr.x + 6.0 * s, cr.y + cr.h / 2.0 - 8.0 * s, cr.w - 10.0 * s, 16.0 * s), 11.0, Color::WHITE, false, Align::Left);
            }
        }
        // the track names sit on top of the clips, in their own strip
        for (ti, name) in names.iter().enumerate() {
            let tr = Rect::new(area.x, area.y + RULER_H * s + ti as f32 * TRACK_H * s, LABEL_W * s, TRACK_H * s);
            gfx::fill_rect(cx.pm, tr, t.panel);
            cx.label(name, Rect::new(tr.x + 6.0 * s, tr.y + tr.h / 2.0 - 8.0 * s, tr.w - 8.0 * s, 16.0 * s), 10.5, t.muted, true, Align::Left);
        }
        for (x, c, name) in markers {
            if x < lane || x > area.right() {
                continue;
            }
            gfx::fill_rect(cx.pm, Rect::new(x, area.y, 1.0, area.h), c.with_alpha(200));
            gfx::fill_round(cx.pm, Rect::new(x, area.y + 2.0 * s, 9.0 * s, 9.0 * s), 2.0 * s, c);
            if !name.is_empty() {
                cx.label(&name, Rect::new(x + 11.0 * s, area.y + 2.0 * s, 90.0 * s, 14.0 * s), 10.0, c, false, Align::Left);
            }
        }
        if play_x >= lane && play_x <= area.right() {
            gfx::fill_rect(cx.pm, Rect::new(play_x - 1.0, area.y, 2.0, area.h), Color::hex(0xE23D3D));
            gfx::fill_round(cx.pm, Rect::new(play_x - 5.0 * s, area.y, 10.0 * s, 10.0 * s), 2.0 * s, Color::hex(0xE23D3D));
        }
    }

    fn paint_bar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let (t, s) = (self.theme.clone(), self.s);
        let bar = Rect::new(0.0, self.h - BAR_H * s, self.w, BAR_H * s);
        gfx::fill_rect(pm, bar, NAVY);
        let left = match &self.flash {
            Some((m, until)) if *until > self.now => m.clone(),
            _ => match self.one() {
                Some(c) => format!("{} — {} long", c.name, pr::fmt_time(c.duration, None)),
                None => {
                    if media::have() {
                        format!("{}, {} long", pr::count(self.p.all_clips().len(), "clip", "clips"), pr::fmt_time(self.p.duration(), None))
                    } else {
                        "No ffmpeg on this computer: Reel can't read or write video here.".to_string()
                    }
                }
            },
        };
        let right = format!(
            "{}{} · {}",
            if self.playing { "Playing · " } else { "" },
            pr::fmt_time(self.playhead, Some(self.p.data.fps)),
            if self.settings.snapping { "snapping on" } else { "snapping off" }
        );
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        cx.label(&left, Rect::new(10.0 * s, bar.y, self.w * 0.7, bar.h), 11.5, Color::WHITE, false, Align::Left);
        cx.label(&right, Rect::new(self.w * 0.6, bar.y, self.w * 0.4 - 10.0 * s, bar.h), 11.5, Color::WHITE.with_alpha(220), false, Align::Right);
    }

    fn paint_backstage(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let (theme, s) = (self.theme.clone(), self.s);
        let area = Rect::new(0.0, TITLE_H * s, self.w, self.h - TITLE_H * s);
        gfx::fill_rect(pm, area, theme.panel);
        let side = 190.0 * s;
        gfx::fill_rect(pm, Rect::new(area.x, area.y, side, area.h), NAVY);
        let page = self.back.as_ref().map(|b| b.page.clone()).unwrap_or_default();
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            let mut y = area.y + 16.0 * s;
            for (id, label) in BACK_PAGES {
                let rr = Rect::new(area.x, y, side, 34.0 * s);
                if *id == page {
                    gfx::fill_rect(cx.pm, rr, Color::hex(0x16284A));
                }
                cx.label(label, Rect::new(rr.x + 20.0 * s, rr.y, rr.w - 24.0 * s, rr.h), 14.0, Color::WHITE, false, Align::Left);
                y += 34.0 * s;
            }
        }
        let body = Rect::new(area.x + side + 24.0 * s, area.y + 20.0 * s, area.w - side - 48.0 * s, area.h - 44.0 * s);
        if page == "export" {
            let preset = self.settings.preset.clone();
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            cx.label(&format!("Size: {preset}"), Rect::new(body.x, body.y, body.w, 20.0 * s), 12.5, theme.muted, true, Align::Left);
        }
        if page == "open" && !self.settings.recent.is_empty() {
            let recent: Vec<String> = self.settings.recent.clone();
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
                cx.label("Recent films", Rect::new(body.x, body.y, body.w, 24.0 * s), 13.0, theme.muted, true, Align::Left);
                let mut y = body.y + 28.0 * s;
                for p in recent.iter().take(8) {
                    let name = Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.clone());
                    cx.label(&name, Rect::new(body.x, y, body.w * 0.4, 22.0 * s), 13.0, theme.text, false, Align::Left);
                    y += 22.0 * s;
                }
            }
            let rest = Rect::new(body.x + body.w * 0.42, body.y, body.w * 0.58, body.h);
            if let Some(f) = self.back.as_mut().and_then(|b| b.files.as_mut()) {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
                f.paint(&mut cx, rest);
            }
            return;
        }
        let top = if page == "export" { body.y + 28.0 * s } else { body.y };
        if let Some(f) = self.back.as_mut().and_then(|b| b.files.as_mut()) {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &theme, s };
            f.paint(&mut cx, Rect::new(body.x, top, body.w, body.h - (top - body.y)));
        }
    }
}

// -------------------------------------------------------------------- what happens
impl ReelApp {
    pub fn handle(&mut self, ev: &Event, host: &mut dyn Host) {
        let mut r = self.res.take().expect("res");
        self.handle_inner(ev, host, &mut r);
        self.res = Some(r);
    }

    /// Events from the window test, which already holds the resources.
    pub fn handle_test(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        self.handle_inner(ev, host, r);
    }

    pub fn handle_inner(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        match ev {
            Event::Resized { w, h, scale } => {
                self.w = *w as f32;
                self.h = *h as f32;
                self.s = *scale;
                return;
            }
            Event::Modifiers(m) => self.mods = *m,
            Event::Key { mods, .. } => self.mods = *mods,
            Event::MouseMove { x, y } => self.mouse = (*x, *y),
            Event::ThemeChanged { dark } => {
                if *dark != self.settings.dark {
                    self.settings.dark = *dark;
                    self.theme = Theme::new(*dark, NAVY, NAVY);
                }
            }
            _ => {}
        }
        if self.pane == Pane::Backstage && self.dialog.is_none() {
            self.backstage_event(ev, host, r);
            return;
        }
        if let Some(d) = self.dialog.as_mut() {
            if let DialogResult::Button(b) = d.event(ev, host, &mut r.text, self.s) {
                let d = self.dialog.take().unwrap();
                self.dialog_done(d, &b);
            }
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    self.menu = None;
                    self.command(&id, host, r);
                    return;
                }
                MenuResult::Closed => self.menu = None,
                MenuResult::None => return,
            }
        }
        let (ta, took) = self.title.event(ev);
        if let Some(a) = ta {
            match a {
                TitleAction::Command(id) => self.command(&id, host, r),
                TitleAction::Close => self.ask_to_close(),
                TitleAction::Minimize => host.minimize(),
                TitleAction::Maximize => host.toggle_maximize(),
                TitleAction::Drag => host.drag_window(),
                _ => {}
            }
            return;
        }
        if took {
            return;
        }
        if let Event::Key { key, mods, down: true, .. } = ev {
            if self.keys(key, mods, host, r) {
                return;
            }
        }
        let st = self.state();
        let (ra, took) = self.ribbon.event(ev, &st, self.now);
        if let Some(a) = ra {
            match a {
                RibbonAction::Command(id) => self.command(&id, host, r),
                RibbonAction::Menu(id, rect) => self.open_menu(&id, rect),
                RibbonAction::Backstage => self.open_backstage("open"),
                _ => {}
            }
            return;
        }
        if took {
            return;
        }
        match ev {
            Event::MouseDown { x, y, button: Button::Left, clicks } => self.pointer_down(*x, *y, *clicks, host),
            Event::MouseMove { x, y } => self.pointer_move(*x, *y, host),
            Event::MouseUp { .. } => self.pointer_up(),
            Event::Wheel { dx, dy } => {
                let (x, y) = self.mouse;
                if self.timeline.contains(x, y) {
                    if self.mods.ctrl {
                        let before = self.x_to_time(x);
                        self.scale = (self.scale * if *dy > 0.0 { 0.85 } else { 1.18 }).clamp(0.0005, 2.0);
                        let after = self.x_to_time(x);
                        self.scroll = (self.scroll + before - after).max(0.0);
                    } else {
                        let step = self.scale * 60.0;
                        let by = if *dx != 0.0 { *dx as f64 } else { -*dy as f64 };
                        self.scroll = (self.scroll + by * step).max(0.0);
                    }
                }
            }
            Event::CloseRequested => self.ask_to_close(),
            Event::FileDropped(p) => {
                let p = p.clone();
                if p.extension().map(|e| e.eq_ignore_ascii_case("reel")).unwrap_or(false) {
                    self.open_path(&p);
                } else {
                    self.bring_in(&p);
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) {
        self.frames.poll();
        self.poll_job();
        if self.playing {
            let d = self.p.duration();
            let by = (self.now.saturating_sub(self.last_tick)).as_secs_f64();
            self.last_tick = self.now;
            // the sound keeps the time when it is playing, so the picture follows it
            let next = if self.sound.ready() { self.sound.position() } else { self.playhead + by };
            self.playhead = next.min(d);
            if self.playhead >= d - 1e-6 {
                self.playhead = d;
                self.playing = false;
                self.sound.pause();
            }
            self.want_frame();
        }
        if let Some(t) = self.sound.trouble() {
            self.sound.pause();
            self.flash(&t);
        }
    }

    fn keys(&mut self, key: &Key, mods: &Mods, host: &mut dyn Host, r: &mut Res) -> bool {
        let id = match (mods.ctrl, mods.shift, key) {
            (true, false, Key::Char('s')) => "save",
            (true, true, Key::Char('s')) => "saveas",
            (true, false, Key::Char('o')) => "open-file",
            (true, false, Key::Char('n')) => "new",
            (true, false, Key::Char('z')) => "undo",
            (true, false, Key::Char('y')) => "redo",
            (true, true, Key::Char('z')) => "redo",
            (true, false, Key::Char('x')) => "cut",
            (true, false, Key::Char('c')) => "copy",
            (true, false, Key::Char('v')) => "paste",
            (true, false, Key::Char('i')) => "add-media",
            (true, false, Key::Char('e')) => "export",
            (true, false, Key::Char('g')) => "group",
            (true, true, Key::Char('g')) => "ungroup",
            (true, false, Key::Char('a')) => "select-all",
            (false, false, Key::Char('s')) => "split",
            (false, false, Key::Char('m')) => "marker",
            (false, false, Key::Char(' ')) => "play",
            (false, false, Key::Home) => "home",
            (false, false, Key::End) => "end",
            (false, false, Key::Delete) => "delete",
            (false, true, Key::Delete) => "ripple-delete",
            (false, false, Key::Left) => "nudge-left",
            (false, false, Key::Right) => "nudge-right",
            (false, false, Key::Up) => "prev-clip",
            (false, false, Key::Down) => "next-clip",
            (false, false, Key::Escape) => "deselect",
            _ => return false,
        };
        self.command(id, host, r);
        true
    }

    fn pointer_down(&mut self, x: f32, y: f32, clicks: u32, _host: &mut dyn Host) {
        if self.bin.contains(x, y) {
            if let Some((i, _)) = self.bin_hits.iter().find(|(_, r)| r.contains(x, y)).copied() {
                self.bin_sel = Some(i);
                self.load_wave(i);
                if clicks >= 2 {
                    self.place(None);
                }
            }
            return;
        }
        if self.timeline.contains(x, y) {
            if y < self.timeline.y + RULER_H * self.s {
                self.drag = Drag::Playhead;
                let t = self.x_to_time(x).max(0.0);
                self.seek(t);
                return;
            }
            match self.clip_at(x, y) {
                Some((ti, id)) => {
                    let c = self.p.clip(id).unwrap();
                    let cr = self.clip_rect(ti, c);
                    let grab = self.x_to_time(x) - c.start;
                    if self.mods.ctrl {
                        if self.sel.contains(&id) {
                            self.sel.retain(|s| *s != id);
                        } else {
                            self.sel.push(id);
                        }
                    } else {
                        self.sel = self.p.expand_groups(&[id]);
                    }
                    self.drag = if (x - cr.x).abs() <= self.grip() {
                        Drag::Trim { id, edge: Edge::Start }
                    } else if (x - cr.right()).abs() <= self.grip() {
                        Drag::Trim { id, edge: Edge::End }
                    } else {
                        Drag::Clip { id, grab, track: ti }
                    };
                }
                None => {
                    self.sel.clear();
                    let t = self.x_to_time(x).max(0.0);
                    self.seek(t);
                    self.drag = Drag::Playhead;
                }
            }
            return;
        }
        if self.preview.contains(x, y) {
            self.sel.clear();
        }
    }

    fn pointer_move(&mut self, x: f32, y: f32, host: &mut dyn Host) {
        match self.drag {
            Drag::None => {
                let over_grip = self
                    .clip_at(x, y)
                    .and_then(|(ti, id)| self.p.clip(id).map(|c| self.clip_rect(ti, c)))
                    .map(|cr| (x - cr.x).abs() <= self.grip() || (x - cr.right()).abs() <= self.grip())
                    .unwrap_or(false);
                host.set_cursor(if over_grip { Cursor::ResizeEW } else { Cursor::Default });
            }
            Drag::Playhead => {
                let t = self.x_to_time(x).max(0.0);
                self.seek(t);
            }
            Drag::Clip { id, grab, track } => {
                let to = self.track_at(y).unwrap_or(track);
                let start = (self.x_to_time(x) - grab).max(0.0);
                let tol = if self.settings.snapping { self.scale * 8.0 } else { 0.0 };
                let g = self.p.group_of(id);
                if g != 0 {
                    let was = self.p.clip(id).map(|c| c.start).unwrap_or(0.0);
                    let _ = self.p.move_group(g, start - was, tol, Some(self.playhead));
                } else if let Err(e) = self.p.move_clip(id, start, Some(to), tol, Some(self.playhead)) {
                    self.flash(&e.0);
                    self.drag = Drag::None;
                }
            }
            Drag::Trim { id, edge } => {
                let t = self.x_to_time(x);
                let tol = if self.settings.snapping { self.scale * 8.0 } else { 0.0 };
                let _ = self.p.trim(id, edge, t, tol, Some(self.playhead));
            }
        }
    }

    fn pointer_up(&mut self) {
        if !matches!(self.drag, Drag::None | Drag::Playhead) {
            self.touched();
        }
        self.drag = Drag::None;
    }

    /// Fetch a waveform for a sound file in the bin, so the timeline can draw it.
    fn load_wave(&mut self, i: usize) {
        if !self.settings.waveforms {
            return;
        }
        let Some(m) = self.p.data.media.get(i) else { return };
        if !m.has_audio || self.waves.iter().any(|(k, _)| *k == m.uri) {
            return;
        }
        let (uri, path) = (m.uri.clone(), m.path());
        let Some(path) = path else { return };
        if let Ok(w) = media::waveform(&path, 240) {
            self.waves.push((uri, w));
        }
    }

    fn open_menu(&mut self, id: &str, rect: Rect) {
        let items = match id {
            "transition" => {
                let mut v = vec![MenuItem::new("tr-none", "None")];
                for (k, name) in TRANSITIONS {
                    v.push(MenuItem::new(&format!("tr-{k}"), name));
                }
                v
            }
            "panzoom" => vec![
                MenuItem::new("pz-none", "Still"),
                MenuItem::new("pz-in", "Zoom In"),
                MenuItem::new("pz-out", "Zoom Out"),
                MenuItem::new("pz-left", "Pan Left"),
                MenuItem::new("pz-right", "Pan Right"),
            ],
            "export-preset" => EXPORT_PRESETS
                .iter()
                .enumerate()
                .map(|(i, (name, _, _))| MenuItem::new(&format!("preset-{i}"), name).check(*name == self.settings.preset))
                .collect(),
            "project-size" => pr::PRESETS.iter().map(|(n, w, h)| MenuItem::new(&format!("size-{n}"), &format!("{} — {} x {}", n, w, h))).collect(),
            "project-fps" => [24.0, 25.0, 30.0, 50.0, 60.0]
                .iter()
                .map(|f| MenuItem::new(&format!("fps-{:.0}", f), &format!("{:.0} a second", f)))
                .collect(),
            "speed" => [0.25, 0.5, 1.0, 1.5, 2.0, 4.0].iter().map(|v| MenuItem::new(&format!("speed-{v}"), &format!("{v}x"))).collect(),
            "volume" => [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0].iter().map(|v| MenuItem::new(&format!("vol-{v}"), &format!("{:.0}%", v * 100.0))).collect(),
            "fade-in" | "fade-out" => [0.0, 0.25, 0.5, 1.0, 2.0]
                .iter()
                .map(|v| MenuItem::new(&format!("{id}-{v}"), &format!("{v}s")))
                .collect(),
            "brightness" => [-0.4, -0.2, 0.0, 0.2, 0.4].iter().map(|v| MenuItem::new(&format!("bri-{v}"), &format!("{v}"))).collect(),
            "contrast" => [0.6, 0.8, 1.0, 1.3, 1.6].iter().map(|v| MenuItem::new(&format!("con-{v}"), &format!("{v}"))).collect(),
            "saturation" => [0.0, 0.5, 1.0, 1.4, 2.0].iter().map(|v| MenuItem::new(&format!("sat-{v}"), &format!("{v}"))).collect(),
            _ => return,
        };
        self.menu = Some(Menu::new(id, rect, items));
    }

    /// What the window test uses to answer a question Reel has asked.
    pub fn dialog_done_for_test(&mut self, d: Dialog, button: &str) {
        self.dialog_done(d, button);
    }

    fn dialog_done(&mut self, d: Dialog, button: &str) {
        match d.id.as_str() {
            "close" => match button {
                "save" => {
                    if self.save_to(None) {
                        self.exit = true;
                    }
                    self.closing = false;
                }
                "discard" => self.exit = true,
                _ => self.closing = false,
            },
            "subtitles" => {
                if button == "ok" {
                    let text = d.value("text");
                    let entries = crate::srt::parse(&text);
                    if entries.is_empty() {
                        self.say("Nothing to import", "Those don't look like subtitles. One block per caption: a time range, then the words.");
                    } else {
                        let n = self.p.add_subtitles(&entries).len();
                        self.flash(&format!("{} on the Titles track.", pr::count(n, "caption is", "captions are")));
                        self.touched();
                    }
                }
            }
            "title-text" => {
                if button == "ok" {
                    let kind = match d.value("kind").as_str() {
                        "Lower third" => Kind::LowerThird,
                        "Caption" => Kind::Caption,
                        _ => Kind::Title,
                    };
                    let words = d.value("text");
                    let at = Some(self.playhead);
                    match self.p.add_clip(0, Source::Text(kind, words), at, false) {
                        Ok(id) => {
                            self.sel = vec![id];
                            self.touched();
                        }
                        Err(e) => self.say("Can't put that there", &e.0),
                    }
                }
            }
            "marker-name" => {
                if button == "ok" {
                    let name = d.value("name");
                    let color = d.value("color").to_lowercase();
                    self.p.add_marker(self.playhead, &name, &color);
                    self.touched();
                }
            }
            _ => {}
        }
    }

    fn backstage_event(&mut self, ev: &Event, host: &mut dyn Host, r: &mut Res) {
        let s = self.s;
        let area = Rect::new(0.0, TITLE_H * s, self.w, self.h - TITLE_H * s);
        if let Event::MouseDown { x, y, button: Button::Left, .. } = ev {
            let side = 190.0 * s;
            if *x < side {
                let mut yy = area.y + 16.0 * s;
                for (id, _) in BACK_PAGES {
                    if *y >= yy && *y < yy + 34.0 * s {
                        let id = id.to_string();
                        self.backstage_page(&id);
                        return;
                    }
                    yy += 34.0 * s;
                }
                return;
            }
        }
        if let Event::Key { key: Key::Escape, down: true, .. } = ev {
            self.pane = Pane::None;
            self.back = None;
            return;
        }
        let page = self.back.as_ref().map(|b| b.page.clone()).unwrap_or_default();
        let act = self.back.as_mut().and_then(|b| b.files.as_mut()).map(|f| f.event(ev, host, &mut r.text, s));
        if let Some(FileAction::Chosen(p)) = act {
            self.pane = Pane::None;
            self.back = None;
            match page.as_str() {
                "open" => self.open_path(&p),
                "save" | "saveas" => {
                    self.save_to(Some(p));
                }
                "export" => {
                    let preset = self.settings.preset.clone();
                    let preset = if p.extension().map(|e| e.eq_ignore_ascii_case("webm")).unwrap_or(false) {
                        EXPORT_PRESETS.iter().find(|(_, f, _)| *f == "webm").map(|(n, _, _)| n.to_string()).unwrap_or(preset)
                    } else {
                        preset
                    };
                    self.export_to(p, &preset, &mut r.text);
                }
                _ => {}
            }
        }
    }
}

// -------------------------------------------------------------------- the commands
impl ReelApp {
    pub fn command(&mut self, id: &str, host: &mut dyn Host, r: &mut Res) {
        let one = self.sel.first().copied();
        match id {
            "new" => self.backstage_page("new"),
            "open-file" => self.open_backstage("open"),
            "save" => {
                self.save_to(None);
            }
            "saveas" => self.open_backstage("saveas"),
            "undo" => {
                if let Some(l) = self.p.undo() {
                    self.sel.retain(|id| self.p.clip(*id).is_some());
                    self.touched();
                    self.flash(&format!("Undid {}.", l.to_lowercase()));
                }
            }
            "redo" => {
                if let Some(l) = self.p.redo() {
                    self.sel.retain(|id| self.p.clip(*id).is_some());
                    self.touched();
                    self.flash(&format!("Redid {}.", l.to_lowercase()));
                }
            }
            "add-media" => {
                self.pane = Pane::Backstage;
                self.back = Some(Back { page: "media".into(), files: Some(self.file_pane(false, &["mp4", "mov", "mkv", "webm", "avi", "mp3", "wav", "flac", "m4a", "ogg", "png", "jpg", "jpeg"], "")) });
            }
            "to-video" => self.place(Some(1)),
            "to-audio" => self.place(Some(2)),
            "copy" | "cut" => {
                if self.sel.is_empty() {
                    return;
                }
                self.clipboard = self.p.copy_out(&self.sel);
                if id == "cut" {
                    let sel = self.sel.clone();
                    self.p.delete(&sel, false);
                    self.sel.clear();
                    self.touched();
                }
                self.flash(&format!("{} on the clipboard.", pr::count(self.clipboard.len(), "clip", "clips")));
            }
            "paste" => {
                if self.clipboard.is_empty() {
                    return;
                }
                let items = self.clipboard.clone();
                let made = self.p.paste(&items, self.playhead);
                self.sel = made;
                self.touched();
            }
            "split" => match self.p.split_at(self.playhead, None) {
                Ok(made) => {
                    self.sel = made;
                    self.touched();
                }
                Err(e) => self.flash(&e.0),
            },
            "delete" | "ripple-delete" => {
                if self.sel.is_empty() {
                    self.flash("Pick a clip first.");
                    return;
                }
                let sel = self.p.expand_groups(&self.sel);
                self.p.delete(&sel, id == "ripple-delete");
                self.sel.clear();
                self.touched();
            }
            "trim-start" | "trim-end" => {
                let Some(id0) = one else { return };
                let edge = if id == "trim-start" { Edge::Start } else { Edge::End };
                if let Err(e) = self.p.trim(id0, edge, self.playhead, 0.0, None) {
                    self.flash(&e.0);
                } else {
                    self.touched();
                }
            }
            "group" => match self.p.group(&self.sel) {
                Ok(_) => self.flash("Grouped."),
                Err(e) => self.flash(&e.0),
            },
            "ungroup" => {
                let sel = self.sel.clone();
                self.p.ungroup(&sel);
            }
            "select-all" => self.sel = self.p.all_clips().iter().map(|(_, c)| c.id).collect(),
            "deselect" => self.sel.clear(),
            "title" | "lower-third" | "caption" => {
                let kind = match id {
                    "lower-third" => "Lower third",
                    "caption" => "Caption",
                    _ => "Title",
                };
                let k = match id {
                    "lower-third" => Kind::LowerThird,
                    "caption" => Kind::Caption,
                    _ => Kind::Title,
                };
                self.dialog = Some(
                    Dialog::new("title-text", &format!("New {}", kind.to_lowercase()))
                        .choice("kind", "What it is", &["Title", "Lower third", "Caption"], match id {
                            "lower-third" => 1,
                            "caption" => 2,
                            _ => 0,
                        })
                        .text("text", "The words", default_text(k))
                        .button("ok", "Add")
                        .button("cancel", "Cancel"),
                );
            }
            "subtitles" => {
                self.dialog = Some(
                    Dialog::new("subtitles", "Import subtitles")
                        .note("Paste a subtitle file here. Each block is a time range, then the words.")
                        .text("text", "Subtitles", "")
                        .button("ok", "Import")
                        .button("cancel", "Cancel"),
                );
            }
            "play" => self.toggle_play(),
            "home" => self.seek(0.0),
            "end" => self.seek(self.p.duration()),
            "nudge-left" | "nudge-right" => {
                let frame = 1.0 / self.p.data.fps.max(1.0);
                let by = if id == "nudge-left" { -frame } else { frame };
                if self.sel.is_empty() {
                    self.seek(self.playhead + by);
                } else {
                    let Some(id0) = one else { return };
                    let was = self.p.clip(id0).map(|c| c.start).unwrap_or(0.0);
                    let g = self.p.group_of(id0);
                    let res = if g != 0 { self.p.move_group(g, by, 0.0, None) } else { self.p.move_clip(id0, was + by, None, 0.0, None) };
                    if let Err(e) = res {
                        self.flash(&e.0);
                    } else {
                        self.touched();
                    }
                }
            }
            "prev-clip" | "next-clip" => {
                let mut starts: Vec<f64> = self.p.all_clips().iter().map(|(_, c)| c.start).collect();
                starts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let t = self.playhead;
                let next = if id == "next-clip" {
                    starts.into_iter().find(|s| *s > t + 1e-6)
                } else {
                    starts.into_iter().rev().find(|s| *s < t - 1e-6)
                };
                if let Some(s) = next {
                    self.seek(s);
                }
            }
            "marker" => {
                self.dialog = Some(
                    Dialog::new("marker-name", "Put a marker here")
                        .text("name", "What it is", "")
                        .choice("color", "Colour", &["Yellow", "Red", "Green", "Blue", "Purple", "White"], 0)
                        .button("ok", "Add")
                        .button("cancel", "Cancel"),
                );
            }
            "next-marker" => {
                if let Some(m) = self.p.next_marker(self.playhead).map(|m| m.t) {
                    self.seek(m);
                }
            }
            "prev-marker" => {
                if let Some(m) = self.p.prev_marker(self.playhead).map(|m| m.t) {
                    self.seek(m);
                }
            }
            "mute" => {
                let Some(id0) = one else { return };
                let was = self.p.clip(id0).map(|c| c.muted).unwrap_or(false);
                self.p.checkpoint("Mute", None);
                if let Some((_, c)) = self.p.find_mut(id0) {
                    c.muted = !was;
                }
                self.touched();
            }
            "music-bed" => {
                let Some(id0) = one else { return };
                let was = self.p.clip(id0).map(|c| c.music_bed).unwrap_or(false);
                self.p.checkpoint("Music Bed", None);
                if let Some((_, c)) = self.p.find_mut(id0) {
                    c.music_bed = !was;
                }
                self.touched();
            }
            "chroma" => {
                let Some(id0) = one else { return };
                let was = self.p.clip(id0).map(|c| c.chroma).unwrap_or(false);
                self.p.checkpoint("Green Screen", None);
                if let Some((_, c)) = self.p.find_mut(id0) {
                    c.chroma = !was;
                }
                self.touched();
            }
            "export" => self.open_backstage("export"),
            "poster" => self.save_poster(),
            "zoom-in" => self.scale = (self.scale * 0.7).max(0.0005),
            "zoom-out" => self.scale = (self.scale / 0.7).min(2.0),
            "zoom-fit" => self.zoom_fit(),
            "snapping" => {
                self.settings.snapping = !self.settings.snapping;
                self.p.snapping = self.settings.snapping;
                self.settings.save();
            }
            "waveforms" => {
                self.settings.waveforms = !self.settings.waveforms;
                self.settings.save();
                if self.settings.waveforms {
                    for i in 0..self.p.data.media.len() {
                        self.load_wave(i);
                    }
                } else {
                    self.waves.clear();
                }
            }
            "show-bin" => {
                self.settings.show_bin = !self.settings.show_bin;
                self.settings.save();
            }
            "show-props" => {
                self.settings.show_props = !self.settings.show_props;
                self.settings.save();
            }
            "dark" => {
                self.settings.dark = !self.settings.dark;
                self.theme = Theme::new(self.settings.dark, NAVY, NAVY);
                self.settings.save();
            }
            "transition" | "panzoom" | "export-preset" | "project-size" | "project-fps" | "speed" | "volume" | "fade-in" | "fade-out"
            | "brightness" | "contrast" | "saturation" => {
                let anchor = self.ribbon.rect_of(id).unwrap_or(Rect::new(self.w / 2.0, 100.0, 1.0, 1.0));
                self.open_menu(id, anchor);
            }
            _ => {
                if let Some(rest) = id.strip_prefix("tr-") {
                    let Some(id0) = one else { return };
                    let kind = if rest == "none" { None } else { Some(rest) };
                    match self.p.set_transition(id0, kind, 1.0) {
                        Ok(()) => self.touched(),
                        Err(e) => self.flash(&e.0),
                    }
                } else if let Some(rest) = id.strip_prefix("pz-") {
                    let Some(id0) = one else { return };
                    self.p.checkpoint("Pan and Zoom", None);
                    if let Some((_, c)) = self.p.find_mut(id0) {
                        c.panzoom = if rest == "none" { String::new() } else { rest.to_string() };
                    }
                    self.touched();
                } else if let Some(rest) = id.strip_prefix("preset-") {
                    if let Some(i) = rest.parse::<usize>().ok().filter(|i| *i < EXPORT_PRESETS.len()) {
                        self.settings.preset = EXPORT_PRESETS[i].0.to_string();
                        self.settings.save();
                        self.flash(&format!("Exports will be {}.", EXPORT_PRESETS[i].0));
                    }
                } else if let Some(rest) = id.strip_prefix("size-") {
                    if let Some((_, w, h)) = pr::PRESETS.iter().find(|(n, _, _)| *n == rest) {
                        self.p.set_size(*w, *h, None);
                        self.touched();
                    }
                } else if let Some(rest) = id.strip_prefix("fps-") {
                    if let Ok(f) = rest.parse::<f64>() {
                        let (w, h) = (self.p.data.width, self.p.data.height);
                        self.p.set_size(w, h, Some(f));
                        self.touched();
                    }
                } else if let Some(rest) = id.strip_prefix("speed-") {
                    let Some(id0) = one else { return };
                    if let Ok(v) = rest.parse::<f64>() {
                        match self.p.set_speed(id0, v) {
                            Ok(()) => self.touched(),
                            Err(e) => self.flash(&e.0),
                        }
                    }
                } else if let Some(rest) = id.strip_prefix("vol-") {
                    self.set_number(one, "Volume", rest, |c, v| c.volume = v);
                } else if let Some(rest) = id.strip_prefix("fade-in-") {
                    self.set_number(one, "Fade In", rest, |c, v| c.fade_in = v.min(c.duration));
                } else if let Some(rest) = id.strip_prefix("fade-out-") {
                    self.set_number(one, "Fade Out", rest, |c, v| c.fade_out = v.min(c.duration));
                } else if let Some(rest) = id.strip_prefix("bri-") {
                    self.set_number(one, "Brightness", rest, |c, v| c.color.brightness = v);
                } else if let Some(rest) = id.strip_prefix("con-") {
                    self.set_number(one, "Contrast", rest, |c, v| c.color.contrast = v);
                } else if let Some(rest) = id.strip_prefix("sat-") {
                    self.set_number(one, "Saturation", rest, |c, v| c.color.saturation = v);
                } else {
                    let _ = (host, r);
                }
            }
        }
    }

    fn set_number(&mut self, id: Option<u64>, label: &str, text: &str, f: impl Fn(&mut Clip, f64)) {
        let Some(id) = id else { return };
        let Ok(v) = text.parse::<f64>() else { return };
        self.p.checkpoint(label, None);
        if let Some((_, c)) = self.p.find_mut(id) {
            f(c, v);
        }
        self.touched();
    }

    /// Save the picture under the playhead as a PNG.
    pub fn save_poster(&mut self) {
        let Some(shot) = self.frames.shot.as_ref() else {
            self.flash("There's no picture under the playhead.");
            return;
        };
        let dir = self.p.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(folio_kit::platform::home_dir);
        let name = format!("{}-{:.0}s.png", self.doc_name().trim_end_matches(".reel"), self.playhead);
        let out = dir.join(&name);
        let bytes = folio_kit::gfx::encode_png(shot.pm.as_ref());
        match std::fs::write(&out, bytes) {
            Ok(()) => self.flash(&format!("Saved {}.", name)),
            Err(e) => self.say("Can't save the frame", &e.to_string()),
        }
    }
}

impl folio_kit::platform::App for ReelApp {
    fn event(&mut self, ev: &Event, host: &mut dyn Host) {
        self.now = host.now();
        if matches!(ev, Event::Tick) {
            self.tick();
        }
        self.handle(ev, host);
        host.set_title(&self.window_title());
        if self.exit {
            host.close();
        }
    }

    fn paint(&mut self, pixels: &mut [u8], w: u32, h: u32, host: &mut dyn Host) {
        self.now = host.now();
        if let Some(mut t) = self.test.take() {
            let mut r = self.res.take().expect("res");
            self.w = w as f32;
            self.h = h as f32;
            t.step(self, host, &mut r);
            self.res = Some(r);
            if t.done {
                self.exit_code = t.code();
                self.exit = true;
                host.close();
            } else {
                host.redraw();
            }
            self.test = Some(t);
        }
        self.paint_into(pixels, w, h);
    }

    fn next_tick(&self) -> Option<Duration> {
        if self.test.is_some() {
            return Some(Duration::from_millis(30));
        }
        if self.playing || self.frames.busy || self.job.is_some() {
            return Some(Duration::from_millis(16));
        }
        if self.flash.is_some() {
            return Some(Duration::from_millis(200));
        }
        self.ribbon.next_tick()
    }

    fn wants_exit(&self) -> bool {
        self.exit
    }

    fn exit_code(&self) -> i32 {
        self.exit_code
    }
}
