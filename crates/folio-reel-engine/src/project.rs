//! A Reel project: the media bin and four tracks of clips, saved as JSON (`.reel`).
//!
//! Times are seconds. Every edit goes through `Project`, which keeps each track in
//! order and records an undo step. The file is the same JSON the earlier Reel
//! wrote, so a project made there opens here and the other way about.

use crate::json::{parse, J};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const TRACKS: [&str; 4] = ["titles", "video", "audio", "music"];
pub const IMAGE_SECONDS: f64 = 5.0;
pub const TITLE_SECONDS: f64 = 4.0;
pub const EPS: f64 = 1e-6;

pub fn track_name(t: &str) -> &'static str {
    match t {
        "titles" => "Titles",
        "video" => "Video",
        "audio" => "Audio",
        _ => "Music",
    }
}

/// The nine ways one clip can become the next.
pub const TRANSITIONS: [(&str, &str); 9] = [
    ("crossfade", "Crossfade"),
    ("dip-black", "Fade to Black"),
    ("wipe-right", "Wipe Right"),
    ("wipe-left", "Wipe Left"),
    ("wipe-down", "Wipe Down"),
    ("wipe-up", "Wipe Up"),
    ("box", "Box"),
    ("clock", "Clock"),
    ("iris", "Iris"),
];

pub const PRESETS: [(&str, u32, u32); 3] = [("720p", 1280, 720), ("1080p", 1920, 1080), ("4k", 3840, 2160)];

/// What it is called, the format, and the size.
pub const EXPORT_PRESETS: [(&str, &str, &str); 5] = [
    ("Full HD video (MP4, 1080p)", "mp4", "1080p"),
    ("4K video (MP4)", "mp4", "4k"),
    ("Smaller file (MP4, 720p)", "mp4", "720p"),
    ("For the web (WebM, 1080p)", "webm", "1080p"),
    ("For the web, smaller (WebM, 720p)", "webm", "720p"),
];

pub const MARKER_COLORS: [(&str, &str); 6] = [
    ("yellow", "Yellow"),
    ("red", "Red"),
    ("green", "Green"),
    ("blue", "Blue"),
    ("purple", "Purple"),
    ("white", "White"),
];

pub fn marker_rgb(c: &str) -> (f32, f32, f32) {
    match c {
        "red" => (0.85, 0.20, 0.18),
        "green" => (0.22, 0.70, 0.30),
        "blue" => (0.20, 0.52, 0.90),
        "purple" => (0.60, 0.35, 0.85),
        "white" => (0.95, 0.95, 0.95),
        _ => (0.98, 0.78, 0.10),
    }
}

/// Which edge of a clip is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// What a clip is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Media,
    Title,
    LowerThird,
    Caption,
}

impl Kind {
    pub fn text(self) -> &'static str {
        match self {
            Kind::Media => "media",
            Kind::Title => "title",
            Kind::LowerThird => "lower-third",
            Kind::Caption => "caption",
        }
    }
    pub fn of(s: &str) -> Kind {
        match s {
            "title" => Kind::Title,
            "lower-third" => Kind::LowerThird,
            "caption" => Kind::Caption,
            _ => Kind::Media,
        }
    }
    pub fn is_text(self) -> bool {
        self != Kind::Media
    }
}

/// An edit that can't be made; the message says why, in words to show someone.
#[derive(Clone, Debug)]
pub struct EditError(pub String);

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(m: impl Into<String>) -> Result<T, EditError> {
    Err(EditError(m.into()))
}

/// Colour adjustment on one clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub hue: f64,
}

impl Default for Color {
    fn default() -> Self {
        Color { brightness: 0.0, contrast: 1.0, saturation: 1.0, hue: 0.0 }
    }
}

impl Color {
    pub fn is_default(&self) -> bool {
        *self == Color::default()
    }
}

/// How much is cut off each side, as fractions of the picture.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Crop {
    pub l: f64,
    pub r: f64,
    pub t: f64,
    pub b: f64,
}

impl Crop {
    pub fn any(&self) -> bool {
        self.l > EPS || self.r > EPS || self.t > EPS || self.b > EPS
    }
}

/// A transition into a clip.
#[derive(Clone, Debug, PartialEq)]
pub struct Trans {
    pub kind: String,
    pub duration: f64,
}

/// A piece of media in the bin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Media {
    pub uri: String,
    pub name: String,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_video: bool,
    pub has_audio: bool,
    pub is_image: bool,
    /// Where the file sat relative to the project, for a folder copied elsewhere.
    pub relative: String,
}

impl Media {
    pub fn path(&self) -> Option<PathBuf> {
        uri_to_path(&self.uri)
    }
}

/// One clip on one track.
#[derive(Clone, Debug)]
pub struct Clip {
    pub id: u64,
    pub kind: Kind,
    pub media: String,
    pub name: String,
    pub start: f64,
    pub inpoint: f64,
    pub duration: f64,
    pub speed: f64,
    pub volume: f64,
    pub fade_in: f64,
    pub fade_out: f64,
    pub color: Color,
    pub crop: Crop,
    pub zoom: f64,
    pub panzoom: String,
    pub transition: Option<Trans>,
    pub music_bed: bool,
    pub muted: bool,
    pub keyframes: Vec<(f64, f64)>,
    pub group: u64,
    pub chroma: bool,
    pub chroma_color: String,
    pub chroma_tolerance: f64,
    pub chroma_softness: f64,
    pub chroma_edges: i64,
    pub text: String,
    pub font: String,
    pub text_color: String,
    pub bg_color: String,
    pub halign: String,
    pub valign: String,
    /// Anything in the file this build doesn't know about, kept so a save loses nothing.
    pub extra: BTreeMap<String, J>,
}

impl Default for Clip {
    fn default() -> Self {
        Clip {
            id: 0,
            kind: Kind::Media,
            media: String::new(),
            name: String::new(),
            start: 0.0,
            inpoint: 0.0,
            duration: 0.0,
            speed: 1.0,
            volume: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            color: Color::default(),
            crop: Crop::default(),
            zoom: 1.0,
            panzoom: String::new(),
            transition: None,
            music_bed: false,
            muted: false,
            keyframes: Vec::new(),
            group: 0,
            chroma: false,
            chroma_color: "green".into(),
            chroma_tolerance: 20.0,
            chroma_softness: 2.0,
            chroma_edges: 100,
            text: String::new(),
            font: String::new(),
            text_color: String::new(),
            bg_color: String::new(),
            halign: String::new(),
            valign: String::new(),
            extra: BTreeMap::new(),
        }
    }
}

impl Clip {
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }
    /// Does this clip show a picture?
    pub fn visual(&self) -> bool {
        self.kind.is_text() || !self.media.is_empty()
    }
}

/// A marker on the timeline.
#[derive(Clone, Debug, Default)]
pub struct Marker {
    pub id: u64,
    pub t: f64,
    pub name: String,
    pub color: String,
}

/// The project as it sits in memory.
#[derive(Clone, Debug)]
pub struct Data {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub media: Vec<Media>,
    /// titles, video, audio, music — in that order, matching `TRACKS`.
    pub tracks: [Vec<Clip>; 4],
    pub markers: Vec<Marker>,
    pub next_id: u64,
}

impl Default for Data {
    fn default() -> Self {
        Data {
            width: 1920,
            height: 1080,
            fps: 30.0,
            media: Vec::new(),
            tracks: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            markers: Vec::new(),
            next_id: 1,
        }
    }
}

pub fn track_index(name: &str) -> Option<usize> {
    TRACKS.iter().position(|t| *t == name)
}

/// The project, its undo steps, and the file it came from.
#[derive(Debug)]
pub struct Project {
    pub data: Data,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub snapping: bool,
    undo: Vec<(String, Data)>,
    redo: Vec<(String, Data)>,
    merge: Option<(String, u64)>,
    clock: u64,
}

impl Default for Project {
    fn default() -> Self {
        Project::new()
    }
}

impl Project {
    pub fn new() -> Project {
        Project {
            data: Data::default(),
            path: None,
            dirty: false,
            snapping: true,
            undo: Vec::new(),
            redo: Vec::new(),
            merge: None,
            clock: 0,
        }
    }

    pub fn of(data: Data, path: Option<PathBuf>) -> Project {
        Project { data, path, ..Project::new() }
    }

    // ---------------------------------------------------------------- undo
    /// Before an edit: remember how things were. `merge` joins a run of changes
    /// under one key — a slider being dragged — into one undo step.
    pub fn checkpoint(&mut self, label: &str, merge: Option<&str>) {
        self.clock += 1;
        let now = self.clock;
        let last = self.merge.take();
        if let Some(key) = merge {
            if let Some((k, when)) = &last {
                if k == key && now - when < 40 && !self.undo.is_empty() {
                    self.merge = Some((key.to_string(), *when));
                    self.redo.clear();
                    return;
                }
            }
            self.merge = Some((key.to_string(), now));
        }
        self.undo.push((label.to_string(), self.data.clone()));
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(l, _)| l.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(l, _)| l.as_str())
    }

    pub fn undo(&mut self) -> Option<String> {
        self.merge = None;
        let (label, data) = self.undo.pop()?;
        self.redo.push((label.clone(), std::mem::replace(&mut self.data, data)));
        self.dirty = true;
        Some(label)
    }

    pub fn redo(&mut self) -> Option<String> {
        self.merge = None;
        let (label, data) = self.redo.pop()?;
        self.undo.push((label.clone(), std::mem::replace(&mut self.data, data)));
        self.dirty = true;
        Some(label)
    }

    fn changed(&mut self) {
        self.dirty = true;
    }

    // ---------------------------------------------------------------- the picture
    pub fn set_size(&mut self, width: u32, height: u32, fps: Option<f64>) {
        self.checkpoint("Project Settings", None);
        self.data.width = width.max(16);
        self.data.height = height.max(16);
        if let Some(f) = fps {
            self.data.fps = f.clamp(1.0, 240.0);
        }
        self.changed();
    }

    // ---------------------------------------------------------------- the bin
    pub fn media_by_uri(&self, uri: &str) -> Option<&Media> {
        self.data.media.iter().find(|m| m.uri == uri)
    }

    pub fn add_media(&mut self, m: Media) -> &Media {
        if let Some(i) = self.data.media.iter().position(|x| x.uri == m.uri) {
            return &self.data.media[i];
        }
        self.checkpoint("Add Media", None);
        self.data.media.push(m);
        self.changed();
        self.data.media.last().unwrap()
    }

    /// Take a file out of the bin, and every clip that used it off the tracks.
    pub fn remove_media(&mut self, uri: &str) {
        if self.media_by_uri(uri).is_none() {
            return;
        }
        self.checkpoint("Remove Media", None);
        self.data.media.retain(|m| m.uri != uri);
        for t in self.data.tracks.iter_mut() {
            t.retain(|c| c.kind.is_text() || c.media != uri);
        }
        self.changed();
    }

    pub fn missing_media(&self) -> Vec<&Media> {
        self.data
            .media
            .iter()
            .filter(|m| m.path().map(|p| !p.exists()).unwrap_or(false))
            .collect()
    }

    /// Where a piece of media goes by default.
    pub fn media_track(&self, m: &Media) -> usize {
        if m.has_video || m.is_image {
            1
        } else {
            2
        }
    }

    // ---------------------------------------------------------------- finding things
    pub fn track(&self, i: usize) -> &[Clip] {
        &self.data.tracks[i]
    }

    pub fn find(&self, id: u64) -> Option<(usize, &Clip)> {
        for (i, t) in self.data.tracks.iter().enumerate() {
            if let Some(c) = t.iter().find(|c| c.id == id) {
                return Some((i, c));
            }
        }
        None
    }

    pub fn find_mut(&mut self, id: u64) -> Option<(usize, &mut Clip)> {
        for (i, t) in self.data.tracks.iter_mut().enumerate() {
            if let Some(c) = t.iter_mut().find(|c| c.id == id) {
                return Some((i, c));
            }
        }
        None
    }

    pub fn clip(&self, id: u64) -> Option<&Clip> {
        self.find(id).map(|(_, c)| c)
    }

    pub fn all_clips(&self) -> Vec<(usize, &Clip)> {
        let mut v = Vec::new();
        for (i, t) in self.data.tracks.iter().enumerate() {
            for c in t {
                v.push((i, c));
            }
        }
        v
    }

    pub fn duration(&self) -> f64 {
        self.all_clips().iter().map(|(_, c)| c.end()).fold(0.0, f64::max)
    }

    pub fn clips_at(&self, t: f64) -> Vec<(usize, &Clip)> {
        self.all_clips()
            .into_iter()
            .filter(|(_, c)| c.start - EPS <= t && t < c.end() - EPS)
            .collect()
    }

    /// The clip before and the clip after, on the same track.
    pub fn neighbours(&self, id: u64) -> (Option<u64>, Option<u64>) {
        let Some((ti, c)) = self.find(id) else { return (None, None) };
        let (start, mut before, mut after) = (c.start, None, None);
        for x in &self.data.tracks[ti] {
            if x.id == id {
                continue;
            }
            if x.start <= start + EPS && before.map(|(s, _): (f64, u64)| x.start >= s).unwrap_or(true) {
                before = Some((x.start, x.id));
            }
            if x.start >= start - EPS && after.map(|(s, _): (f64, u64)| x.start <= s).unwrap_or(true) {
                after = Some((x.start, x.id));
            }
        }
        (before.map(|(_, i)| i), after.map(|(_, i)| i))
    }

    fn new_id(&mut self) -> u64 {
        let n = self.data.next_id;
        self.data.next_id = n + 1;
        n
    }

    // ---------------------------------------------------------------- snapping
    pub fn snap_points(&self, exclude: &[u64]) -> Vec<f64> {
        let mut v = vec![0.0];
        for (_, c) in self.all_clips() {
            if exclude.contains(&c.id) {
                continue;
            }
            v.push(c.start);
            v.push(c.end());
        }
        for m in &self.data.markers {
            v.push(m.t);
        }
        v
    }

    /// The nearest edge within `tolerance`, or `t` itself.
    pub fn snap(&self, t: f64, tolerance: f64, exclude: &[u64], extra: &[f64]) -> f64 {
        if tolerance <= 0.0 {
            return t;
        }
        let mut best = t;
        let mut gap = tolerance;
        for p in self.snap_points(exclude).into_iter().chain(extra.iter().copied()) {
            let d = (p - t).abs();
            if d <= gap {
                gap = d;
                best = p;
            }
        }
        best
    }

    // ---------------------------------------------------------------- groups
    pub fn group_of(&self, id: u64) -> u64 {
        self.clip(id).map(|c| c.group).unwrap_or(0)
    }

    pub fn group_members(&self, gid: u64) -> Vec<u64> {
        if gid == 0 {
            return Vec::new();
        }
        self.all_clips().iter().filter(|(_, c)| c.group == gid).map(|(_, c)| c.id).collect()
    }

    /// Every clip in the same group as each of these.
    pub fn expand_groups(&self, ids: &[u64]) -> Vec<u64> {
        let mut out: Vec<u64> = Vec::new();
        for id in ids {
            let g = self.group_of(*id);
            if g == 0 {
                if !out.contains(id) {
                    out.push(*id);
                }
            } else {
                for m in self.group_members(g) {
                    if !out.contains(&m) {
                        out.push(m);
                    }
                }
            }
        }
        out
    }

    pub fn group(&mut self, ids: &[u64]) -> Result<u64, EditError> {
        let ids = self.expand_groups(ids);
        if ids.len() < 2 {
            return err("Pick two clips or more to group.");
        }
        self.checkpoint("Group", None);
        let g = self.new_id();
        for id in ids {
            if let Some((_, c)) = self.find_mut(id) {
                c.group = g;
            }
        }
        self.changed();
        Ok(g)
    }

    pub fn ungroup(&mut self, ids: &[u64]) {
        let ids = self.expand_groups(ids);
        if ids.iter().all(|id| self.group_of(*id) == 0) {
            return;
        }
        self.checkpoint("Ungroup", None);
        for id in ids {
            if let Some((_, c)) = self.find_mut(id) {
                c.group = 0;
            }
        }
        self.changed();
    }
}

// -------------------------------------------------------------------- the file
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    Some(PathBuf::from(unescape(rest)))
}

pub fn path_to_uri(path: &Path) -> String {
    let p = if path.is_absolute() {
        path.to_path_buf()
    } else {
        folio_kit::platform::cwd().unwrap_or_default().join(path)
    };
    format!("file://{}", escape(&p.to_string_lossy()))
}

fn escape(s: &str) -> String {
    let safe = |b: u8| b.is_ascii_alphanumeric() || b"/-_.~".contains(&b);
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        if safe(*b) {
            out.push(*b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", b));
        }
    }
    out
}

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Seconds as a timecode. With `frames_fps`, h:mm:ss:ff; without, m:ss.t.
pub fn fmt_time(secs: f64, frames_fps: Option<f64>) -> String {
    let secs = secs.max(0.0);
    if let Some(fps) = frames_fps {
        let fps = fps.max(1.0).round() as i64;
        let total = (secs * fps as f64).round() as i64;
        let f = total % fps;
        let s = total / fps;
        return format!("{}:{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60, f);
    }
    let s = secs as i64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}.{}", s / 60, s % 60, ((secs - s as f64) * 10.0) as i64)
    }
}

const KNOWN: [&str; 29] = [
    "id", "kind", "media", "name", "start", "inpoint", "duration", "speed", "volume", "fade_in", "fade_out", "color",
    "crop", "zoom", "panzoom", "transition", "music_bed", "muted", "keyframes", "group", "chroma", "chroma_color",
    "chroma_tolerance", "chroma_softness", "chroma_edges", "text", "font", "text_color", "bg_color",
];

fn clip_from(j: &J) -> Clip {
    let mut c = Clip {
        id: j.get("id").i(0) as u64,
        kind: Kind::of(j.get("kind").s()),
        media: j.get("media").s().to_string(),
        name: j.get("name").s().to_string(),
        start: j.get("start").f(0.0),
        inpoint: j.get("inpoint").f(0.0),
        duration: j.get("duration").f(0.0),
        speed: j.get("speed").f(1.0),
        volume: j.get("volume").f(1.0),
        fade_in: j.get("fade_in").f(0.0),
        fade_out: j.get("fade_out").f(0.0),
        color: Color {
            brightness: j.get("color").get("brightness").f(0.0),
            contrast: j.get("color").get("contrast").f(1.0),
            saturation: j.get("color").get("saturation").f(1.0),
            hue: j.get("color").get("hue").f(0.0),
        },
        crop: Crop {
            l: j.get("crop").get("l").f(0.0),
            r: j.get("crop").get("r").f(0.0),
            t: j.get("crop").get("t").f(0.0),
            b: j.get("crop").get("b").f(0.0),
        },
        zoom: j.get("zoom").f(1.0),
        panzoom: j.get("panzoom").s().to_string(),
        music_bed: j.get("music_bed").b(false),
        muted: j.get("muted").b(false),
        group: j.get("group").i(0) as u64,
        chroma: j.get("chroma").b(false),
        chroma_color: {
            let s = j.get("chroma_color").s();
            if s.is_empty() { "green".to_string() } else { s.to_string() }
        },
        chroma_tolerance: j.get("chroma_tolerance").f(20.0),
        chroma_softness: j.get("chroma_softness").f(2.0),
        chroma_edges: j.get("chroma_edges").i(100),
        text: j.get("text").s().to_string(),
        font: j.get("font").s().to_string(),
        text_color: j.get("text_color").s().to_string(),
        bg_color: j.get("bg_color").s().to_string(),
        halign: j.get("halign").s().to_string(),
        valign: j.get("valign").s().to_string(),
        ..Clip::default()
    };
    let t = j.get("transition");
    if !t.is_null() && !t.get("type").s().is_empty() {
        c.transition = Some(Trans { kind: t.get("type").s().to_string(), duration: t.get("duration").f(1.0) });
    }
    for k in j.get("keyframes").arr() {
        c.keyframes.push((k.at(0).f(0.0), k.at(1).f(1.0)));
    }
    if let J::Obj(map) = j {
        for (k, v) in map {
            if !KNOWN.contains(&k.as_str()) && k != "halign" && k != "valign" {
                c.extra.insert(k.clone(), v.clone());
            }
        }
    }
    c
}

fn clip_to(c: &Clip) -> J {
    let mut j = J::obj();
    for (k, v) in &c.extra {
        j.set(k, v.clone());
    }
    j.set("id", J::from(c.id as i64));
    j.set("kind", J::from(c.kind.text()));
    j.set("name", J::from(c.name.as_str()));
    j.set("start", J::from(c.start));
    j.set("inpoint", J::from(c.inpoint));
    j.set("duration", J::from(c.duration));
    j.set("speed", J::from(c.speed));
    j.set("volume", J::from(c.volume));
    j.set("fade_in", J::from(c.fade_in));
    j.set("fade_out", J::from(c.fade_out));
    j.set("zoom", J::from(c.zoom));
    j.set("music_bed", J::from(c.music_bed));
    j.set("muted", J::from(c.muted));
    j.set("chroma", J::from(c.chroma));
    j.set("chroma_color", J::from(c.chroma_color.as_str()));
    j.set("chroma_tolerance", J::from(c.chroma_tolerance));
    j.set("chroma_softness", J::from(c.chroma_softness));
    j.set("chroma_edges", J::from(c.chroma_edges));
    if c.kind == Kind::Media {
        j.set("media", J::from(c.media.as_str()));
    } else {
        j.set("text", J::from(c.text.as_str()));
        j.set("font", J::from(c.font.as_str()));
        j.set("text_color", J::from(c.text_color.as_str()));
        j.set("bg_color", J::from(c.bg_color.as_str()));
        j.set("halign", J::from(c.halign.as_str()));
        j.set("valign", J::from(c.valign.as_str()));
    }
    let mut col = J::obj();
    col.set("brightness", J::from(c.color.brightness));
    col.set("contrast", J::from(c.color.contrast));
    col.set("saturation", J::from(c.color.saturation));
    col.set("hue", J::from(c.color.hue));
    j.set("color", col);
    let mut cr = J::obj();
    cr.set("l", J::from(c.crop.l));
    cr.set("r", J::from(c.crop.r));
    cr.set("t", J::from(c.crop.t));
    cr.set("b", J::from(c.crop.b));
    j.set("crop", cr);
    j.set("panzoom", if c.panzoom.is_empty() { J::Null } else { J::from(c.panzoom.as_str()) });
    j.set(
        "transition",
        match &c.transition {
            None => J::Null,
            Some(t) => {
                let mut tj = J::obj();
                tj.set("type", J::from(t.kind.as_str()));
                tj.set("duration", J::from(t.duration));
                tj
            }
        },
    );
    let mut kf = J::Arr(Vec::new());
    for (t, v) in &c.keyframes {
        let mut pair = J::Arr(Vec::new());
        pair.push(J::from(*t));
        pair.push(J::from(*v));
        kf.push(pair);
    }
    j.set("keyframes", kf);
    if c.group != 0 {
        j.set("group", J::from(c.group as i64));
    }
    j
}

fn media_from(j: &J) -> Media {
    Media {
        uri: j.get("uri").s().to_string(),
        name: j.get("name").s().to_string(),
        duration: j.get("duration").f(0.0),
        width: j.get("width").i(0) as u32,
        height: j.get("height").i(0) as u32,
        fps: j.get("fps").f(0.0),
        has_video: j.get("has_video").b(false),
        has_audio: j.get("has_audio").b(false),
        is_image: j.get("is_image").b(false),
        relative: j.get("relative").s().to_string(),
    }
}

fn media_to(m: &Media, base: Option<&Path>) -> J {
    let mut j = J::obj();
    j.set("uri", J::from(m.uri.as_str()));
    j.set("name", J::from(m.name.as_str()));
    j.set("duration", J::from(m.duration));
    j.set("width", J::from(m.width as i64));
    j.set("height", J::from(m.height as i64));
    j.set("fps", J::from(m.fps));
    j.set("has_video", J::from(m.has_video));
    j.set("has_audio", J::from(m.has_audio));
    j.set("is_image", J::from(m.is_image));
    if let (Some(base), Some(p)) = (base, m.path()) {
        if let Some(rel) = relative_to(&p, base) {
            j.set("relative", J::from(rel.as_str()));
        }
    }
    j
}

/// `p` written from `base`, with `..` where it has to climb.
fn relative_to(p: &Path, base: &Path) -> Option<String> {
    let (mut a, mut b) = (p.components().peekable(), base.components().peekable());
    while a.peek().is_some() && a.peek() == b.peek() {
        a.next();
        b.next();
    }
    let up = b.count();
    let rest: Vec<String> = a.map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    if up == 0 && rest.is_empty() {
        return None;
    }
    let mut out: Vec<String> = (0..up).map(|_| "..".to_string()).collect();
    out.extend(rest);
    Some(out.join("/"))
}

pub fn to_json(p: &Project) -> String {
    let base = p.path.as_ref().and_then(|x| x.parent().map(|d| d.to_path_buf()));
    let d = &p.data;
    let mut j = J::obj();
    j.set("version", J::from(1i64));
    j.set("width", J::from(d.width as i64));
    j.set("height", J::from(d.height as i64));
    j.set("fps", J::from(d.fps));
    let mut media = J::Arr(Vec::new());
    for m in &d.media {
        media.push(media_to(m, base.as_deref()));
    }
    j.set("media", media);
    let mut tracks = J::obj();
    for (i, name) in TRACKS.iter().enumerate() {
        let mut arr = J::Arr(Vec::new());
        for c in &d.tracks[i] {
            arr.push(clip_to(c));
        }
        tracks.set(name, arr);
    }
    j.set("tracks", tracks);
    let mut markers = J::Arr(Vec::new());
    for m in &d.markers {
        let mut mj = J::obj();
        mj.set("id", J::from(m.id as i64));
        mj.set("t", J::from(m.t));
        mj.set("name", J::from(m.name.as_str()));
        mj.set("color", J::from(m.color.as_str()));
        markers.push(mj);
    }
    j.set("markers", markers);
    j.set("next_id", J::from(d.next_id as i64));
    j.dump()
}

pub fn save(p: &mut Project, path: Option<&Path>) -> Result<(), EditError> {
    if let Some(path) = path {
        p.path = Some(path.to_path_buf());
    }
    let Some(path) = p.path.clone() else { return err("no file name") };
    let text = to_json(p);
    folio_kit::platform::atomic_write(&path, text.as_bytes()).map_err(|e| EditError(e.to_string()))?;
    p.dirty = false;
    Ok(())
}

pub fn open(path: &Path) -> Result<Project, EditError> {
    let text = std::fs::read_to_string(path).map_err(|e| EditError(format!("{} can't be read: {}", path.display(), e)))?;
    read(&text, Some(path))
}

pub fn read(text: &str, path: Option<&Path>) -> Result<Project, EditError> {
    let j = parse(text).map_err(|e| EditError(format!("That isn't a Folio Reel project: {}", e)))?;
    if j.get("tracks").is_null() {
        return err("That isn't a Folio Reel project.");
    }
    let mut d = Data {
        width: j.get("width").i(1920) as u32,
        height: j.get("height").i(1080) as u32,
        fps: j.get("fps").f(30.0),
        next_id: j.get("next_id").i(1).max(1) as u64,
        ..Data::default()
    };
    for m in j.get("media").arr() {
        d.media.push(media_from(m));
    }
    // media carried along with the project: a folder copied to another computer
    let base = path.and_then(|p| p.parent().map(|x| x.to_path_buf()));
    let mut moved: Vec<(String, String)> = Vec::new();
    if let Some(base) = &base {
        for m in d.media.iter_mut() {
            let gone = m.path().map(|p| !p.exists()).unwrap_or(false);
            if gone && !m.relative.is_empty() {
                let cand = base.join(&m.relative);
                if cand.exists() {
                    let uri = path_to_uri(&cand);
                    moved.push((m.uri.clone(), uri.clone()));
                    m.uri = uri;
                }
            }
        }
    }
    for (i, name) in TRACKS.iter().enumerate() {
        for cj in j.get("tracks").get(name).arr() {
            let mut c = clip_from(cj);
            if let Some((_, new)) = moved.iter().find(|(old, _)| *old == c.media) {
                c.media = new.clone();
            }
            if c.id >= d.next_id {
                d.next_id = c.id + 1;
            }
            d.tracks[i].push(c);
        }
        d.tracks[i].sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    }
    for mj in j.get("markers").arr() {
        let m = Marker {
            id: mj.get("id").i(0) as u64,
            t: mj.get("t").f(0.0),
            name: mj.get("name").s().to_string(),
            color: {
                let c = mj.get("color").s();
                if c.is_empty() { "yellow".to_string() } else { c.to_string() }
            },
        };
        if m.id >= d.next_id {
            d.next_id = m.id + 1;
        }
        d.markers.push(m);
    }
    Ok(Project::of(d, path.map(|p| p.to_path_buf())))
}
