//! The library: the songs Chord knows about, the folders it watches, and the
//! playlists over them. Ratings are 0-100 in fives of twenty, as iTunes kept them.

use crate::db::{self, Record};
use crate::smart::Rules;
use crate::tags::{self, Tags};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// One song in the library.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub id: u64,
    pub path: PathBuf,
    pub tags: Tags,
    pub size: u64,
    pub mtime: f64,
    pub date_added: f64,
    pub play_count: u32,
    pub last_played: f64,
    pub skip_count: u32,
    pub rating: u32,
    /// unticked songs are skipped when the whole library plays
    pub checked: bool,
    pub missing: bool,
}

impl Track {
    pub fn artist(&self) -> &str {
        if self.tags.artist.is_empty() {
            &self.tags.album_artist
        } else {
            &self.tags.artist
        }
    }

    pub fn display_album_artist(&self) -> &str {
        if self.tags.album_artist.is_empty() {
            self.artist()
        } else {
            &self.tags.album_artist
        }
    }

    /// "4:07", as the song list shows it.
    pub fn length(&self) -> String {
        let s = self.tags.duration.max(0.0).round() as u64;
        format!("{}:{:02}", s / 60, s % 60)
    }

    pub fn stars(&self) -> u32 {
        (self.rating / 20).min(5)
    }
}

/// What a playlist is: a list someone made, a rule-driven one, or a folder of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Smart,
    Folder,
}

#[derive(Clone, Debug)]
pub struct Playlist {
    pub id: u64,
    pub name: String,
    pub kind: Kind,
    pub parent: u64,
    pub rules: Rules,
    pub tracks: Vec<u64>,
}

#[derive(Default)]
pub struct Library {
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
    pub folders: Vec<PathBuf>,
    by_path: HashMap<PathBuf, usize>,
    next_id: u64,
    pub dirty: bool,
    pub path: PathBuf,
}

impl Library {
    /// Where the library lives for this computer's user.
    pub fn default_path() -> PathBuf {
        folio_kit::platform::config_dir("folio-chord").join("library.txt")
    }

    pub fn open(path: &Path) -> Library {
        let mut lib = Library { path: path.to_path_buf(), next_id: 1, ..Default::default() };
        for r in db::read(path) {
            match r.kind.as_str() {
                "folder" => lib.folders.push(PathBuf::from(r.get("path"))),
                "track" => {
                    let t = track_from(&r);
                    lib.next_id = lib.next_id.max(t.id + 1);
                    lib.tracks.push(t);
                }
                "playlist" => {
                    let p = Playlist {
                        id: r.num("id"),
                        name: r.get("name").to_string(),
                        kind: match r.get("kind") {
                            "smart" => Kind::Smart,
                            "folder" => Kind::Folder,
                            _ => Kind::Plain,
                        },
                        parent: r.num("parent"),
                        rules: Rules::parse(r.get("rules")),
                        tracks: r.get("tracks").split(',').filter_map(|s| s.parse().ok()).collect(),
                    };
                    lib.next_id = lib.next_id.max(p.id + 1);
                    lib.playlists.push(p);
                }
                _ => {}
            }
        }
        lib.reindex();
        lib
    }

    pub fn save(&mut self) -> Result<(), String> {
        let mut out = Vec::new();
        for f in &self.folders {
            let mut r = Record::new("folder");
            r.put("path", f.to_string_lossy());
            out.push(r);
        }
        for t in &self.tracks {
            out.push(track_record(t));
        }
        for p in &self.playlists {
            let mut r = Record::new("playlist");
            r.put("id", p.id)
                .put("name", &p.name)
                .put("kind", match p.kind {
                    Kind::Plain => "plain",
                    Kind::Smart => "smart",
                    Kind::Folder => "folder",
                })
                .put("parent", if p.parent == 0 { String::new() } else { p.parent.to_string() })
                .put("rules", p.rules.text())
                .put("tracks", p.tracks.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","));
            out.push(r);
        }
        let path = self.path.clone();
        db::write(&path, &out)?;
        self.dirty = false;
        Ok(())
    }

    fn reindex(&mut self) {
        self.by_path = self.tracks.iter().enumerate().map(|(i, t)| (t.path.clone(), i)).collect();
    }

    pub fn track(&self, id: u64) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: u64) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn add_folder(&mut self, p: &Path) {
        if !self.folders.iter().any(|f| f == p) {
            self.folders.push(p.to_path_buf());
            self.dirty = true;
        }
    }

    /// Walk a folder and take in every music file, keeping what is already known.
    /// Returns how many songs were added and how many were refreshed.
    pub fn scan(&mut self, root: &Path) -> (usize, usize) {
        let mut added = 0;
        let mut changed = 0;
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if !tags::is_music(&p) {
                    continue;
                }
                let meta = e.metadata().ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let mtime = meta
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                match self.by_path.get(&p).copied() {
                    Some(i) => {
                        let same = self.tracks[i].size == size && (self.tracks[i].mtime - mtime).abs() < 1.0;
                        self.tracks[i].missing = false;
                        if !same {
                            if let Some(t) = tags::read(&p) {
                                self.tracks[i].tags = t;
                                self.tracks[i].size = size;
                                self.tracks[i].mtime = mtime;
                                changed += 1;
                            }
                        }
                    }
                    None => {
                        let Some(t) = tags::read(&p) else { continue };
                        let id = self.next_id;
                        self.next_id += 1;
                        self.tracks.push(Track {
                            id,
                            path: p.clone(),
                            tags: t,
                            size,
                            mtime,
                            date_added: now(),
                            checked: true,
                            ..Default::default()
                        });
                        self.by_path.insert(p, self.tracks.len() - 1);
                        added += 1;
                    }
                }
            }
        }
        if added > 0 || changed > 0 {
            self.dirty = true;
        }
        (added, changed)
    }

    /// Scan every watched folder, and mark what is no longer on disk.
    pub fn rescan(&mut self) -> (usize, usize) {
        let (mut a, mut c) = (0, 0);
        for f in self.folders.clone() {
            let (x, y) = self.scan(&f);
            a += x;
            c += y;
        }
        for t in &mut self.tracks {
            t.missing = !t.path.is_file();
        }
        (a, c)
    }

    pub fn remove(&mut self, ids: &[u64]) {
        self.tracks.retain(|t| !ids.contains(&t.id));
        for p in &mut self.playlists {
            p.tracks.retain(|i| !ids.contains(i));
        }
        self.reindex();
        self.dirty = true;
    }

    // ------------------------------------------------------------ playlists

    pub fn new_playlist(&mut self, name: &str, kind: Kind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.playlists.push(Playlist { id, name: name.to_string(), kind, parent: 0, rules: Rules::default(), tracks: Vec::new() });
        self.dirty = true;
        id
    }

    pub fn playlist(&self, id: u64) -> Option<&Playlist> {
        self.playlists.iter().find(|p| p.id == id)
    }

    pub fn delete_playlist(&mut self, id: u64) {
        self.playlists.retain(|p| p.id != id);
        self.dirty = true;
    }

    pub fn add_to_playlist(&mut self, id: u64, ids: &[u64]) {
        if let Some(p) = self.playlists.iter_mut().find(|p| p.id == id) {
            for i in ids {
                if !p.tracks.contains(i) {
                    p.tracks.push(*i);
                }
            }
            self.dirty = true;
        }
    }

    /// What a playlist holds: its own list, or what its rules pick out.
    pub fn playlist_tracks(&self, id: u64) -> Vec<u64> {
        let Some(p) = self.playlist(id) else { return Vec::new() };
        match p.kind {
            Kind::Smart => p.rules.pick(&self.tracks),
            _ => p.tracks.clone(),
        }
    }

    // ------------------------------------------------------------ browsing

    /// The genres, artists and albums of a set of songs, each sorted and unique.
    pub fn browse(&self, ids: &[u64]) -> (Vec<String>, Vec<String>, Vec<String>) {
        let mut g: Vec<String> = Vec::new();
        let mut a: Vec<String> = Vec::new();
        let mut al: Vec<String> = Vec::new();
        for t in self.tracks.iter().filter(|t| ids.contains(&t.id)) {
            for (v, list) in [(&t.tags.genre, &mut g), (&t.display_album_artist().to_string(), &mut a), (&t.tags.album, &mut al)] {
                if !v.is_empty() && !list.contains(v) {
                    list.push(v.clone());
                }
            }
        }
        g.sort();
        a.sort();
        al.sort();
        (g, a, al)
    }

    /// Songs whose words match, as the search box does it.
    pub fn search(&self, q: &str) -> Vec<u64> {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return self.tracks.iter().map(|t| t.id).collect();
        }
        self.tracks
            .iter()
            .filter(|t| {
                let hay = format!("{} {} {} {}", t.tags.title, t.artist(), t.tags.album, t.tags.genre).to_lowercase();
                q.split_whitespace().all(|w| hay.contains(w))
            })
            .map(|t| t.id)
            .collect()
    }

    pub fn played(&mut self, id: u64) {
        if let Some(t) = self.track_mut(id) {
            t.play_count += 1;
            t.last_played = now();
        }
        self.dirty = true;
    }

    pub fn skipped(&mut self, id: u64) {
        if let Some(t) = self.track_mut(id) {
            t.skip_count += 1;
        }
        self.dirty = true;
    }

    pub fn set_rating(&mut self, id: u64, stars: u32) {
        if let Some(t) = self.track_mut(id) {
            t.rating = stars.min(5) * 20;
        }
        self.dirty = true;
    }
}

fn track_record(t: &Track) -> Record {
    let mut r = Record::new("track");
    let g = &t.tags;
    r.put("id", t.id)
        .put("path", t.path.to_string_lossy())
        .put("title", &g.title)
        .put("artist", &g.artist)
        .put("albumartist", &g.album_artist)
        .put("album", &g.album)
        .put("genre", &g.genre)
        .put("composer", &g.composer)
        .put("grouping", &g.grouping)
        .put("comments", &g.comments)
        .put("year", if g.year == 0 { String::new() } else { g.year.to_string() })
        .put("track", if g.track_no == 0 { String::new() } else { g.track_no.to_string() })
        .put("tracks", if g.track_count == 0 { String::new() } else { g.track_count.to_string() })
        .put("disc", if g.disc_no == 0 { String::new() } else { g.disc_no.to_string() })
        .put("discs", if g.disc_count == 0 { String::new() } else { g.disc_count.to_string() })
        .put("bpm", if g.bpm == 0 { String::new() } else { g.bpm.to_string() })
        .put("compilation", if g.compilation { "1" } else { "" })
        .put("rgtrack", g.rg_track.map(|v| v.to_string()).unwrap_or_default())
        .put("rgalbum", g.rg_album.map(|v| v.to_string()).unwrap_or_default())
        .put("duration", format!("{:.3}", g.duration))
        .put("bitrate", g.bitrate)
        .put("rate", g.sample_rate)
        .put("channels", g.channels)
        .put("kind", &g.kind)
        .put("art", if g.has_art { "1" } else { "" })
        .put("size", t.size)
        .put("mtime", format!("{:.0}", t.mtime))
        .put("added", format!("{:.0}", t.date_added))
        .put("plays", if t.play_count == 0 { String::new() } else { t.play_count.to_string() })
        .put("lastplayed", if t.last_played == 0.0 { String::new() } else { format!("{:.0}", t.last_played) })
        .put("skips", if t.skip_count == 0 { String::new() } else { t.skip_count.to_string() })
        .put("rating", if t.rating == 0 { String::new() } else { t.rating.to_string() })
        .put("checked", if t.checked { "1" } else { "0" })
        .put("missing", if t.missing { "1" } else { "" });
    r
}

fn track_from(r: &Record) -> Track {
    Track {
        id: r.num("id"),
        path: PathBuf::from(r.get("path")),
        tags: Tags {
            title: r.get("title").into(),
            artist: r.get("artist").into(),
            album_artist: r.get("albumartist").into(),
            album: r.get("album").into(),
            genre: r.get("genre").into(),
            composer: r.get("composer").into(),
            grouping: r.get("grouping").into(),
            comments: r.get("comments").into(),
            year: r.num("year"),
            track_no: r.num("track"),
            track_count: r.num("tracks"),
            disc_no: r.num("disc"),
            disc_count: r.num("discs"),
            bpm: r.num("bpm"),
            compilation: r.flag("compilation"),
            rg_track: r.get("rgtrack").parse().ok(),
            rg_album: r.get("rgalbum").parse().ok(),
            duration: r.num("duration"),
            bitrate: r.num("bitrate"),
            sample_rate: r.num("rate"),
            channels: r.num("channels"),
            kind: r.get("kind").into(),
            has_art: r.flag("art"),
        },
        size: r.num("size"),
        mtime: r.num("mtime"),
        date_added: r.num("added"),
        play_count: r.num("plays"),
        last_played: r.num("lastplayed"),
        skip_count: r.num("skips"),
        rating: r.num("rating"),
        checked: r.get("checked") != "0",
        missing: r.flag("missing"),
    }
}
