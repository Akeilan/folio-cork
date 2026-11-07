//! The Folio Chord window on the portable base (C3-39): the player bar, the
//! sources down the side, the column browser and the song list.

use folio_canvas::Glyphs;
use folio_chord_engine::library::{Kind, Library, Track};
use folio_chord_engine::player::Player;
use folio_chord_engine::smart;
use folio_kit::chrome::{TitleAction, TitleBar, TITLE_H};
use folio_kit::files::{FileAction, FilePane};
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::platform::{Button, Cursor, Event, Host, Key, Mods};
use folio_kit::text::{Align, Text};
use folio_kit::theme::Theme;
use folio_kit::ui::{Ctx, Dialog, DialogResult, Menu, MenuItem, MenuResult, TextField};
use std::path::PathBuf;
use std::time::Duration;

use crate::settings::Settings;
use crate::{icons, uitest::UiTest};

/// Chord's own fuchsia, on the title bar and the highlights.
pub const FUCHSIA: Color = Color::hex(0xC7007F);
const BAR_H: f32 = 64.0;
const SIDE_W: f32 = 190.0;
const BROWSE_H: f32 = 180.0;
const STATUS_H: f32 = 24.0;
const ROW_H: f32 = 22.0;
const HEAD_H: f32 = 22.0;

/// Which column the song list is sorted by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Col {
    Name,
    Time,
    Artist,
    Album,
    Genre,
    Rating,
    Plays,
}

impl Col {
    pub fn title(self) -> &'static str {
        match self {
            Col::Name => "Name",
            Col::Time => "Time",
            Col::Artist => "Artist",
            Col::Album => "Album",
            Col::Genre => "Genre",
            Col::Rating => "Rating",
            Col::Plays => "Plays",
        }
    }

    pub fn all() -> [Col; 7] {
        [Col::Name, Col::Time, Col::Artist, Col::Album, Col::Genre, Col::Rating, Col::Plays]
    }

    /// How wide the column is drawn, as a share of the list.
    fn share(self) -> f32 {
        match self {
            Col::Name => 0.30,
            Col::Time => 0.07,
            Col::Artist => 0.20,
            Col::Album => 0.20,
            Col::Genre => 0.11,
            Col::Rating => 0.07,
            Col::Plays => 0.05,
        }
    }
}

/// What the middle of the window is showing.
#[derive(Clone, PartialEq, Debug)]
pub enum Source {
    Music,
    Playlist(u64),
}

/// What the person is picking out in the column browser.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Browse {
    pub genre: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

pub struct Res {
    pub text: Text,
    pub icons: Icons,
}

pub struct ChordApp {
    pub lib: Library,
    pub player: Player,
    audio: Option<folio_kit::platform::audio::AudioOut>,
    pub settings: Settings,
    pub source: Source,
    pub browse: Browse,
    pub search: TextField,
    /// the songs the list is showing, in order
    pub rows: Vec<u64>,
    pub sel: Vec<u64>,
    pub sort: (Col, bool),
    pub scroll: f32,
    /// what is playing, and what is queued after it
    pub now: Option<u64>,
    pub queue: Vec<u64>,
    pub queue_at: usize,
    pub res: Option<Box<Res>>,
    pub glyphs: Glyphs,
    pub theme: Theme,
    pub title: TitleBar,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub files: Option<FilePane>,
    pub flash: Option<(String, Duration)>,
    pub clock: Duration,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub mods: Mods,
    pub mouse: (f32, f32),
    pub dragging_seek: bool,
    pub dragging_volume: bool,
    // where things were drawn last time
    pub bar_rect: Rect,
    pub side_rect: Rect,
    pub browse_rect: Rect,
    pub list_rect: Rect,
    pub seek_rect: Rect,
    pub vol_rect: Rect,
    pub transport: Vec<(String, Rect)>,
    pub side_hits: Vec<(Source, Rect)>,
    pub row_hits: Vec<(u64, Rect)>,
    pub col_hits: Vec<(Col, Rect)>,
    pub browse_hits: Vec<(u8, String, Rect)>,
    pub exit: bool,
    pub exit_code: i32,
    pub test: Option<UiTest>,
    pm: Option<sk::Pixmap>,
}

impl ChordApp {
    pub fn new(lib: Library) -> ChordApp {
        let mut ic = Icons::new();
        icons::install(&mut ic);
        let settings = Settings::load();
        let player = Player::new(48000, 2);
        player.set_volume(settings.volume);
        player.set_eq(settings.eq, settings.eq_on);
        player.set_crossfade(settings.crossfade);
        let mut app = ChordApp {
            lib,
            player,
            audio: None,
            settings,
            source: Source::Music,
            browse: Browse::default(),
            search: TextField::new(""),
            rows: Vec::new(),
            sel: Vec::new(),
            sort: (Col::Artist, true),
            scroll: 0.0,
            now: None,
            queue: Vec::new(),
            queue_at: 0,
            res: Some(Box::new(Res { text: Text::bundled(), icons: ic })),
            glyphs: Glyphs::new(),
            theme: Theme::new(false, FUCHSIA, FUCHSIA),
            title: TitleBar::new("chord", vec![]),
            menu: None,
            dialog: None,
            files: None,
            flash: None,
            clock: Duration::ZERO,
            w: 1280.0,
            h: 800.0,
            s: 1.0,
            mods: Mods::NONE,
            mouse: (0.0, 0.0),
            dragging_seek: false,
            dragging_volume: false,
            bar_rect: Rect::default(),
            side_rect: Rect::default(),
            browse_rect: Rect::default(),
            list_rect: Rect::default(),
            seek_rect: Rect::default(),
            vol_rect: Rect::default(),
            transport: Vec::new(),
            side_hits: Vec::new(),
            row_hits: Vec::new(),
            col_hits: Vec::new(),
            browse_hits: Vec::new(),
            exit: false,
            exit_code: 0,
            test: None,
            pm: None,
        };
        app.refresh();
        app
    }

    /// Open the sound output. Without one, everything else still works.
    pub fn start_audio(&mut self) {
        if self.audio.is_some() {
            return;
        }
        let fill = self.player.filler();
        match folio_kit::platform::audio::audio_out(self.player.rate, self.player.channels, fill) {
            Ok(a) => self.audio = Some(a),
            Err(e) => self.flash(&e),
        }
    }

    pub fn flash(&mut self, msg: &str) {
        self.flash = Some((msg.to_string(), self.clock + Duration::from_secs(5)));
    }

    pub fn say(&mut self, title: &str, text: &str) {
        self.dialog = Some(Dialog::new("say", title).note(text).button("ok", "OK"));
    }

    pub fn window_title(&self) -> String {
        match self.now.and_then(|i| self.lib.track(i)) {
            Some(t) => format!("{} — {} - Folio Chord", t.tags.title, t.artist()),
            None => "Folio Chord".into(),
        }
    }

    // ---------------------------------------------------------------- the list

    /// Work out which songs the list shows, in the order the columns ask for.
    pub fn refresh(&mut self) {
        let base: Vec<u64> = match &self.source {
            Source::Music => self.lib.tracks.iter().map(|t| t.id).collect(),
            Source::Playlist(id) => self.lib.playlist_tracks(*id),
        };
        let found = self.lib.search(&self.search.text);
        let b = self.browse.clone();
        let mut rows: Vec<u64> = base
            .into_iter()
            .filter(|i| found.contains(i))
            .filter(|i| {
                let Some(t) = self.lib.track(*i) else { return false };
                b.genre.as_ref().map(|g| &t.tags.genre == g).unwrap_or(true)
                    && b.artist.as_ref().map(|a| t.display_album_artist() == a).unwrap_or(true)
                    && b.album.as_ref().map(|a| &t.tags.album == a).unwrap_or(true)
            })
            .collect();
        let (col, up) = self.sort;
        let key = |t: &Track| -> (String, String, u32, String) {
            (
                t.display_album_artist().to_lowercase(),
                t.tags.album.to_lowercase(),
                t.tags.disc_no * 1000 + t.tags.track_no,
                t.tags.title.to_lowercase(),
            )
        };
        rows.sort_by(|a, b2| {
            let (Some(x), Some(y)) = (self.lib.track(*a), self.lib.track(*b2)) else { return std::cmp::Ordering::Equal };
            let o = match col {
                Col::Name => x.tags.title.to_lowercase().cmp(&y.tags.title.to_lowercase()),
                Col::Time => x.tags.duration.total_cmp(&y.tags.duration),
                Col::Artist => key(x).cmp(&key(y)),
                Col::Album => (x.tags.album.to_lowercase(), x.tags.disc_no, x.tags.track_no)
                    .cmp(&(y.tags.album.to_lowercase(), y.tags.disc_no, y.tags.track_no)),
                Col::Genre => x.tags.genre.to_lowercase().cmp(&y.tags.genre.to_lowercase()),
                Col::Rating => x.rating.cmp(&y.rating),
                Col::Plays => x.play_count.cmp(&y.play_count),
            };
            if up {
                o
            } else {
                o.reverse()
            }
        });
        self.rows = rows;
        self.sel.retain(|i| self.rows.contains(i));
    }

    pub fn total_time(&self) -> String {
        let secs: f64 = self.rows.iter().filter_map(|i| self.lib.track(*i)).map(|t| t.tags.duration).sum();
        let s = secs.round() as u64;
        if s >= 3600 {
            format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
        } else {
            format!("{}:{:02}", s / 60, s % 60)
        }
    }

    // ---------------------------------------------------------------- playing

    pub fn play_row(&mut self, id: u64) {
        let Some(t) = self.lib.track(id).cloned() else { return };
        if !t.path.is_file() {
            self.flash(&format!("{} is not where it was.", t.tags.title));
            return;
        }
        self.queue = self.rows.clone();
        if self.settings.shuffle {
            shuffle(&mut self.queue);
            self.queue.retain(|i| *i != id);
            self.queue.insert(0, id);
        }
        self.queue_at = self.queue.iter().position(|i| *i == id).unwrap_or(0);
        self.start(id);
    }

    fn start(&mut self, id: u64) {
        let Some(t) = self.lib.track(id).cloned() else { return };
        let db = if self.settings.sound_check { t.tags.rg_track.or(t.tags.rg_album).unwrap_or(0.0) } else { 0.0 };
        self.player.set_gain_db(db.clamp(-15.0, 15.0));
        self.player.play(id, &t.path, 0.0);
        self.now = Some(id);
        self.lib.played(id);
        self.start_audio();
    }

    pub fn play_pause(&mut self) {
        let st = self.player.status();
        if st.playing {
            self.player.pause();
            return;
        }
        if self.now.is_some() {
            self.player.resume();
            self.start_audio();
        } else if let Some(first) = self.sel.first().copied().or_else(|| self.rows.first().copied()) {
            self.play_row(first);
        }
    }

    pub fn next(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        if self.settings.repeat == 2 {
            if let Some(id) = self.now {
                self.start(id);
                return;
            }
        }
        if self.queue_at + 1 < self.queue.len() {
            self.queue_at += 1;
        } else if self.settings.repeat == 1 {
            self.queue_at = 0;
        } else {
            self.player.stop();
            self.now = None;
            return;
        }
        let id = self.queue[self.queue_at];
        self.start(id);
    }

    pub fn previous(&mut self) {
        let st = self.player.status();
        if st.position > 3.0 {
            self.player.seek(0.0);
            return;
        }
        if self.queue_at > 0 {
            self.queue_at -= 1;
            let id = self.queue[self.queue_at];
            self.start(id);
        } else {
            self.player.seek(0.0);
        }
    }

    pub fn set_volume(&mut self, v: f64) {
        self.settings.volume = v.clamp(0.0, 1.0);
        self.player.set_volume(self.settings.volume);
        self.settings.save();
    }
}

/// A shuffle that does not keep the same order twice.
fn shuffle(v: &mut [u64]) {
    let mut seed = folio_kit::platform::random_u64() | 1;
    for i in (1..v.len()).rev() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (seed >> 33) as usize % (i + 1);
        v.swap(i, j);
    }
}

impl ChordApp {
    // ---------------------------------------------------------------- painting

    pub fn paint(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        self.w = pm.width() as f32;
        self.h = pm.height() as f32;
        let t = self.theme.clone();
        let s = self.s;
        pm.fill(t.window.to_sk());
        let top = TITLE_H * s;
        self.bar_rect = Rect::new(0.0, top, self.w, BAR_H * s);
        let body_y = self.bar_rect.bottom();
        let body_h = (self.h - body_y - STATUS_H * s).max(1.0);
        self.side_rect = Rect::new(0.0, body_y, SIDE_W * s, body_h);
        let right_x = self.side_rect.right();
        let browse_h = if matches!(self.source, Source::Music) { BROWSE_H * s } else { 0.0 };
        self.browse_rect = Rect::new(right_x, body_y, self.w - right_x, browse_h);
        self.list_rect = Rect::new(right_x, body_y + browse_h, self.w - right_x, body_h - browse_h);
        self.paint_bar(pm, r);
        self.paint_side(pm, r);
        if browse_h > 0.0 {
            self.paint_browser(pm, r);
        }
        self.paint_list(pm, r);
        self.paint_status(pm, r);
        {
            let title = self.window_title();
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            self.title.paint(&mut cx, Rect::new(0.0, 0.0, self.w, TITLE_H * s), &title, false, &|_| true);
        }
        if let Some(f) = self.files.as_mut() {
            let box_r = Rect::new(self.w * 0.1, self.h * 0.1, self.w * 0.8, self.h * 0.8);
            gfx::fill_rect(pm, box_r, t.panel);
            gfx::stroke_round(pm, box_r, 0.0, t.border, 1.0);
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            f.paint(&mut cx, box_r.inset(12.0 * s));
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

    /// The player bar: the transport, what is playing, the search box.
    fn paint_bar(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        gfx::fill_rect(pm, self.bar_rect, t.panel);
        gfx::fill_rect(pm, Rect::new(0.0, self.bar_rect.bottom() - 1.0, self.w, 1.0), t.border);
        let st = self.player.status();
        self.transport.clear();
        let bs = 26.0 * s;
        let mut x = 14.0 * s;
        let y = self.bar_rect.y + (self.bar_rect.h - bs) / 2.0;
        for (id, icon) in [("prev", "prev"), ("playpause", if st.playing { "pause" } else { "play" }), ("next", "next")] {
            let rr = Rect::new(x, y, bs, bs);
            let hot = rr.contains(self.mouse.0, self.mouse.1);
            if hot {
                gfx::fill_round(pm, rr, 4.0 * s, t.hover);
            }
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.icon_colored(icon, rr.inset(4.0 * s), t.text);
            }
            self.transport.push((id.to_string(), rr));
            x += bs + 6.0 * s;
        }
        // what is playing, with the time either side of the line
        let disp = Rect::new(x + 14.0 * s, self.bar_rect.y + 8.0 * s, (self.w * 0.42).max(220.0 * s), self.bar_rect.h - 16.0 * s);
        gfx::fill_round(pm, disp, 5.0 * s, t.field);
        gfx::stroke_round(pm, disp, 5.0 * s, t.border, 1.0);
        let (line1, line2) = match self.now.and_then(|i| self.lib.track(i)) {
            Some(tr) => (tr.tags.title.clone(), format!("{} — {}", tr.artist(), tr.tags.album)),
            None => ("Folio Chord".to_string(), String::new()),
        };
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            cx.label(&line1, Rect::new(disp.x + 10.0 * s, disp.y + 3.0 * s, disp.w - 20.0 * s, 15.0 * s), 12.0, t.text, true, Align::Center);
            cx.label(&line2, Rect::new(disp.x + 10.0 * s, disp.y + 16.0 * s, disp.w - 20.0 * s, 14.0 * s), 11.0, t.muted, false, Align::Center);
        }
        // the seek line
        let line = Rect::new(disp.x + 42.0 * s, disp.bottom() - 11.0 * s, disp.w - 84.0 * s, 4.0 * s);
        self.seek_rect = line;
        gfx::fill_round(pm, line, 2.0 * s, t.border);
        let dur = self.now.and_then(|i| self.lib.track(i)).map(|t| t.tags.duration).unwrap_or(0.0);
        let frac = if dur > 0.0 { (st.position / dur).clamp(0.0, 1.0) } else { 0.0 };
        gfx::fill_round(pm, Rect::new(line.x, line.y, line.w * frac as f32, line.h), 2.0 * s, FUCHSIA);
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            let left = Rect::new(disp.x + 6.0 * s, line.y - 5.0 * s, 34.0 * s, 14.0 * s);
            let right = Rect::new(line.right() + 4.0 * s, line.y - 5.0 * s, 36.0 * s, 14.0 * s);
            cx.label(&mmss(st.position), left, 10.0, t.muted, false, Align::Left);
            cx.label(&format!("-{}", mmss((dur - st.position).max(0.0))), right, 10.0, t.muted, false, Align::Right);
        }
        // shuffle, repeat, the volume and the search box
        let mut rx = self.w - 14.0 * s;
        let sw = 190.0 * s;
        rx -= sw;
        let sbox = Rect::new(rx, self.bar_rect.y + (self.bar_rect.h - 24.0 * s) / 2.0, sw, 24.0 * s);
        gfx::fill_round(pm, sbox, 12.0 * s, t.field);
        gfx::stroke_round(pm, sbox, 12.0 * s, t.border, 1.0);
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            cx.icon_colored("search", Rect::new(sbox.x + 5.0 * s, sbox.y + 5.0 * s, 14.0 * s, 14.0 * s), t.muted);
        }
        self.search.rect = Rect::new(sbox.x + 24.0 * s, sbox.y, sbox.w - 30.0 * s, sbox.h);
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            self.search.paint(&mut cx, self.search.rect);
        }
        rx -= 100.0 * s;
        let vol = Rect::new(rx, self.bar_rect.y + (self.bar_rect.h - 4.0 * s) / 2.0, 80.0 * s, 4.0 * s);
        self.vol_rect = vol;
        gfx::fill_round(pm, vol, 2.0 * s, t.border);
        gfx::fill_round(pm, Rect::new(vol.x, vol.y, vol.w * self.settings.volume as f32, vol.h), 2.0 * s, t.text);
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            cx.icon_colored(
                if self.settings.volume <= 0.001 { "mute" } else { "volume" },
                Rect::new(vol.x - 22.0 * s, vol.y - 8.0 * s, 16.0 * s, 16.0 * s),
                t.muted,
            );
        }
        for (id, icon, on) in [
            ("shuffle", "shuffle", self.settings.shuffle),
            ("repeat", "repeat", self.settings.repeat > 0),
        ] {
            rx -= 32.0 * s;
            let rr = Rect::new(rx, self.bar_rect.y + (self.bar_rect.h - 24.0 * s) / 2.0, 24.0 * s, 24.0 * s);
            if on {
                gfx::fill_round(pm, rr, 4.0 * s, t.checked);
            }
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.icon_colored(icon, rr.inset(4.0 * s), if on { FUCHSIA } else { t.muted });
            }
            self.transport.push((id.to_string(), rr));
        }
    }

    /// The sources: the library, then the playlists.
    fn paint_side(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        gfx::fill_rect(pm, self.side_rect, t.panel);
        gfx::fill_rect(pm, Rect::new(self.side_rect.right() - 1.0, self.side_rect.y, 1.0, self.side_rect.h), t.border);
        self.side_hits.clear();
        let mut y = self.side_rect.y + 8.0 * s;
        let row = 24.0 * s;
        let mut draw = |pm: &mut sk::Pixmap, r: &mut Res, label: &str, icon: &str, src: Option<Source>, y: f32, cur: bool| {
            let rr = Rect::new(self.side_rect.x, y, self.side_rect.w, row);
            if cur {
                gfx::fill_rect(pm, rr, t.checked);
            }
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            if !icon.is_empty() {
                cx.icon_colored(icon, Rect::new(rr.x + 12.0 * s, rr.y + 5.0 * s, 14.0 * s, 14.0 * s), if cur { FUCHSIA } else { t.muted });
            }
            cx.label(
                label,
                Rect::new(rr.x + 32.0 * s, rr.y, rr.w - 40.0 * s, rr.h),
                12.0,
                if cur { t.text } else { t.text },
                cur,
                Align::Left,
            );
            if let Some(src) = src {
                self.side_hits.push((src, rr));
            }
        };
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            cx.label("Library", Rect::new(self.side_rect.x + 12.0 * s, y, self.side_rect.w, 16.0 * s), 10.0, t.muted, true, Align::Left);
        }
        y += 18.0 * s;
        draw(pm, r, "Music", "music", Some(Source::Music), y, self.source == Source::Music);
        y += row + 10.0 * s;
        {
            let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
            cx.label("Playlists", Rect::new(self.side_rect.x + 12.0 * s, y, self.side_rect.w, 16.0 * s), 10.0, t.muted, true, Align::Left);
        }
        y += 18.0 * s;
        let lists: Vec<(u64, String, Kind)> = self.lib.playlists.iter().map(|p| (p.id, p.name.clone(), p.kind)).collect();
        for (id, name, kind) in lists {
            if y > self.side_rect.bottom() - row {
                break;
            }
            let cur = self.source == Source::Playlist(id);
            draw(pm, r, &name, if kind == Kind::Smart { "smart" } else { "playlist" }, Some(Source::Playlist(id)), y, cur);
            y += row;
        }
    }

    /// Genres, artists and albums, side by side.
    fn paint_browser(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        gfx::fill_rect(pm, self.browse_rect, t.window);
        gfx::fill_rect(pm, Rect::new(self.browse_rect.x, self.browse_rect.bottom() - 1.0, self.browse_rect.w, 1.0), t.border);
        self.browse_hits.clear();
        let ids: Vec<u64> = match &self.source {
            Source::Music => self.lib.tracks.iter().map(|t| t.id).collect(),
            Source::Playlist(id) => self.lib.playlist_tracks(*id),
        };
        let (genres, artists, albums) = self.lib.browse(&ids);
        let cw = self.browse_rect.w / 3.0;
        let cols: [(u8, &str, Vec<String>, Option<String>); 3] = [
            (0, "Genres", genres, self.browse.genre.clone()),
            (1, "Artists", artists, self.browse.artist.clone()),
            (2, "Albums", albums, self.browse.album.clone()),
        ];
        for (which, title, items, picked) in cols {
            let x = self.browse_rect.x + cw * which as f32;
            let head = Rect::new(x, self.browse_rect.y, cw, HEAD_H * s);
            gfx::fill_rect(pm, head, t.panel);
            gfx::fill_rect(pm, Rect::new(x + cw - 1.0, self.browse_rect.y, 1.0, self.browse_rect.h), t.border);
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.label(title, Rect::new(head.x + 8.0 * s, head.y, head.w - 10.0 * s, head.h), 11.0, t.muted, true, Align::Left);
            }
            let mut y = head.bottom();
            let all = Rect::new(x, y, cw, ROW_H * s);
            let is_all = picked.is_none();
            if is_all {
                gfx::fill_rect(pm, all, t.checked);
            }
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                let label = format!("All ({})", items.len());
                cx.label(&label, Rect::new(all.x + 8.0 * s, all.y, all.w - 10.0 * s, all.h), 12.0, t.text, is_all, Align::Left);
            }
            self.browse_hits.push((which, String::new(), all));
            y = all.bottom();
            for it in items {
                if y > self.browse_rect.bottom() - ROW_H * s {
                    break;
                }
                let rr = Rect::new(x, y, cw, ROW_H * s);
                let cur = picked.as_deref() == Some(it.as_str());
                if cur {
                    gfx::fill_rect(pm, rr, t.checked);
                }
                {
                    let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                    cx.label(&it, Rect::new(rr.x + 8.0 * s, rr.y, rr.w - 10.0 * s, rr.h), 12.0, t.text, cur, Align::Left);
                }
                self.browse_hits.push((which, it, rr));
                y = rr.bottom();
            }
        }
    }

    /// The songs, with their columns.
    fn paint_list(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        gfx::fill_rect(pm, self.list_rect, t.window);
        self.row_hits.clear();
        self.col_hits.clear();
        let head = Rect::new(self.list_rect.x, self.list_rect.y, self.list_rect.w, HEAD_H * s);
        gfx::fill_rect(pm, head, t.panel);
        gfx::fill_rect(pm, Rect::new(head.x, head.bottom() - 1.0, head.w, 1.0), t.border);
        let mut x = head.x;
        for c in Col::all() {
            let w = head.w * c.share();
            let rr = Rect::new(x, head.y, w, head.h);
            {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                let mark = if self.sort.0 == c { if self.sort.1 { " ^" } else { " v" } } else { "" };
                cx.label(
                    &format!("{}{mark}", c.title()),
                    Rect::new(rr.x + 8.0 * s, rr.y, rr.w - 10.0 * s, rr.h),
                    11.0,
                    if self.sort.0 == c { t.text } else { t.muted },
                    self.sort.0 == c,
                    if matches!(c, Col::Time | Col::Plays) { Align::Right } else { Align::Left },
                );
            }
            self.col_hits.push((c, rr));
            x += w;
        }
        let rows = self.rows.clone();
        let mut y = head.bottom() - self.scroll;
        for id in rows {
            if y > self.list_rect.bottom() {
                break;
            }
            let rr = Rect::new(self.list_rect.x, y, self.list_rect.w, ROW_H * s);
            if rr.bottom() < head.bottom() {
                y = rr.bottom();
                continue;
            }
            let Some(tr) = self.lib.track(id).cloned() else { continue };
            let picked = self.sel.contains(&id);
            let playing = self.now == Some(id);
            if picked {
                gfx::fill_rect(pm, rr, t.checked);
            } else if playing {
                gfx::fill_rect(pm, rr, t.hover);
            }
            let mut cx0 = rr.x;
            for c in Col::all() {
                let w = rr.w * c.share();
                let cell = Rect::new(cx0 + 8.0 * s, rr.y, w - 12.0 * s, rr.h);
                let (text, align) = match c {
                    Col::Name => (tr.tags.title.clone(), Align::Left),
                    Col::Time => (tr.length(), Align::Right),
                    Col::Artist => (tr.artist().to_string(), Align::Left),
                    Col::Album => (tr.tags.album.clone(), Align::Left),
                    Col::Genre => (tr.tags.genre.clone(), Align::Left),
                    Col::Rating => (String::new(), Align::Left),
                    Col::Plays => (if tr.play_count > 0 { tr.play_count.to_string() } else { String::new() }, Align::Right),
                };
                if c == Col::Rating {
                    let mut sx = cell.x;
                    for k in 0..5 {
                        let star = Rect::new(sx, cell.y + 4.0 * s, 12.0 * s, 12.0 * s);
                        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                        cx.icon_colored(
                            if tr.stars() > k { "star" } else { "star-empty" },
                            star,
                            if tr.stars() > k { FUCHSIA } else { t.border },
                        );
                        sx += 13.0 * s;
                    }
                } else {
                    let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                    let colour = if tr.missing { t.muted } else { t.text };
                    cx.label(&text, cell, 12.0, colour, playing, align);
                }
                cx0 += w;
            }
            if playing {
                let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
                cx.icon_colored("play", Rect::new(rr.x + 1.0 * s, rr.y + 5.0 * s, 10.0 * s, 10.0 * s), FUCHSIA);
            }
            self.row_hits.push((id, rr));
            y = rr.bottom();
        }
    }

    fn paint_status(&mut self, pm: &mut sk::Pixmap, r: &mut Res) {
        let t = self.theme.clone();
        let s = self.s;
        let rect = Rect::new(0.0, self.h - STATUS_H * s, self.w, STATUS_H * s);
        gfx::fill_rect(pm, rect, t.panel);
        gfx::fill_rect(pm, Rect::new(0.0, rect.y, self.w, 1.0), t.border);
        let text = match self.flash.as_ref() {
            Some((m, until)) if *until > self.clock => m.clone(),
            _ => {
                let n = self.rows.len();
                format!("{n} song{}, {}", if n == 1 { "" } else { "s" }, self.total_time())
            }
        };
        let mut cx = Ctx { pm, text: &mut r.text, icons: &mut r.icons, theme: &t, s };
        cx.label(&text, Rect::new(10.0 * s, rect.y, self.w - 20.0 * s, rect.h), 12.0, t.muted, false, Align::Left);
    }

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
}

fn mmss(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl ChordApp {
    // ---------------------------------------------------------------- events

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
        if let Event::Modifiers(m) = ev {
            self.mods = *m;
        }
        if let Event::Key { mods, .. } = ev {
            self.mods = *mods;
        }
        if let Event::MouseMove { x, y } = ev {
            self.mouse = (*x, *y);
        }
        if let Some(d) = self.dialog.as_mut() {
            if let DialogResult::Button(b) = d.event(ev, host, &mut r.text, self.s) {
                let d = self.dialog.take().unwrap();
                self.dialog_done(d, &b);
            }
            return;
        }
        if let Some(f) = self.files.as_mut() {
            match f.event(ev, host, &mut r.text, self.s) {
                FileAction::Chosen(p) => {
                    self.files = None;
                    self.add_folder(&p);
                }
                _ => {}
            }
            if let Event::Key { key: Key::Escape, down: true, .. } = ev {
                self.files = None;
            }
            return;
        }
        if let Some(m) = self.menu.as_mut() {
            match m.event(ev) {
                MenuResult::Picked(id) => {
                    self.menu = None;
                    self.command(&id, host);
                    return;
                }
                MenuResult::Closed => self.menu = None,
                MenuResult::None => return,
            }
        }
        let (ta, took) = self.title.event(ev);
        if let Some(a) = ta {
            match a {
                TitleAction::Close => self.exit = true,
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
        // the search box takes what is typed while it has the caret
        if self.search.focused {
            let before = self.search.text.clone();
            self.search.event(ev, host, &mut r.text, self.s);
            if self.search.text != before {
                self.refresh();
                self.scroll = 0.0;
                return;
            }
            if let Event::Key { key: Key::Escape, down: true, .. } = ev {
                self.search.focused = false;
                self.search.text.clear();
                self.refresh();
                return;
            }
            if matches!(ev, Event::Text(_)) || matches!(ev, Event::Key { down: true, .. }) {
                return;
            }
        }
        if let Event::Key { key, mods, down: true, .. } = ev {
            if self.keys(key, mods, host) {
                return;
            }
        }
        match ev {
            Event::CloseRequested => self.exit = true,
            Event::MouseDown { x, y, button, clicks } => {
                let (x, y) = (*x, *y);
                if *button == Button::Right {
                    if let Some((id, _)) = self.row_hits.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                        if !self.sel.contains(&id) {
                            self.sel = vec![id];
                        }
                        self.open_song_menu(x, y);
                        return;
                    }
                    self.open_library_menu(x, y);
                    return;
                }
                if self.search.rect.contains(x, y) {
                    self.search.focused = true;
                    self.search.event(ev, host, &mut r.text, self.s);
                    return;
                }
                self.search.focused = false;
                if let Some((id, _)) = self.transport.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                    self.command(&id, host);
                    return;
                }
                if self.seek_rect.contains(x, y) || (self.seek_rect.inset(-6.0).contains(x, y) && self.now.is_some()) {
                    self.dragging_seek = true;
                    self.seek_to(x);
                    return;
                }
                if self.vol_rect.inset(-6.0).contains(x, y) {
                    self.dragging_volume = true;
                    let v = ((x - self.vol_rect.x) / self.vol_rect.w.max(1.0)) as f64;
                    self.set_volume(v);
                    return;
                }
                if let Some((src, _)) = self.side_hits.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                    self.source = src;
                    self.browse = Browse::default();
                    self.scroll = 0.0;
                    self.refresh();
                    return;
                }
                if let Some((which, name, _)) = self.browse_hits.iter().find(|(_, _, rr)| rr.contains(x, y)).cloned() {
                    let v = if name.is_empty() { None } else { Some(name) };
                    match which {
                        0 => {
                            self.browse.genre = v;
                            self.browse.artist = None;
                            self.browse.album = None;
                        }
                        1 => {
                            self.browse.artist = v;
                            self.browse.album = None;
                        }
                        _ => self.browse.album = v,
                    }
                    self.scroll = 0.0;
                    self.refresh();
                    return;
                }
                if let Some((c, _)) = self.col_hits.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                    if self.sort.0 == c {
                        self.sort.1 = !self.sort.1;
                    } else {
                        self.sort = (c, true);
                    }
                    self.refresh();
                    return;
                }
                if let Some((id, rr)) = self.row_hits.iter().find(|(_, rr)| rr.contains(x, y)).cloned() {
                    // the stars are set by clicking them
                    let rating_x = self.list_rect.x
                        + self.list_rect.w
                            * (Col::Name.share() + Col::Time.share() + Col::Artist.share() + Col::Album.share() + Col::Genre.share());
                    if x >= rating_x && x < rating_x + 70.0 * self.s {
                        let star = (((x - rating_x - 8.0 * self.s) / (13.0 * self.s)).floor() as i32 + 1).clamp(0, 5) as u32;
                        let was = self.lib.track(id).map(|t| t.stars()).unwrap_or(0);
                        self.lib.set_rating(id, if was == star { 0 } else { star });
                        self.save_library();
                        return;
                    }
                    let _ = rr;
                    if *clicks >= 2 {
                        self.play_row(id);
                        return;
                    }
                    if self.mods.ctrl {
                        if self.sel.contains(&id) {
                            self.sel.retain(|i| *i != id);
                        } else {
                            self.sel.push(id);
                        }
                    } else if self.mods.shift && !self.sel.is_empty() {
                        let last = *self.sel.last().unwrap();
                        let (a, b) = (
                            self.rows.iter().position(|i| *i == last).unwrap_or(0),
                            self.rows.iter().position(|i| *i == id).unwrap_or(0),
                        );
                        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                        self.sel = self.rows[lo..=hi].to_vec();
                    } else {
                        self.sel = vec![id];
                    }
                }
            }
            Event::MouseMove { x, .. } => {
                if self.dragging_seek {
                    self.seek_to(*x);
                }
                if self.dragging_volume {
                    let v = ((*x - self.vol_rect.x) / self.vol_rect.w.max(1.0)) as f64;
                    self.set_volume(v);
                }
                let over = self.row_hits.iter().any(|(_, rr)| rr.contains(self.mouse.0, self.mouse.1));
                host.set_cursor(if over { Cursor::Default } else { Cursor::Default });
            }
            Event::MouseUp { .. } => {
                self.dragging_seek = false;
                self.dragging_volume = false;
            }
            Event::Wheel { dy, .. } => {
                let rows = self.rows.len() as f32 * ROW_H * self.s;
                let view = (self.list_rect.h - HEAD_H * self.s).max(1.0);
                self.scroll = (self.scroll - dy * 3.0 * ROW_H * self.s).clamp(0.0, (rows - view).max(0.0));
            }
            Event::Tick => self.after_song(),
            _ => {}
        }
    }

    fn seek_to(&mut self, x: f32) {
        let Some(id) = self.now else { return };
        let dur = self.lib.track(id).map(|t| t.tags.duration).unwrap_or(0.0);
        if dur <= 0.0 {
            return;
        }
        let frac = ((x - self.seek_rect.x) / self.seek_rect.w.max(1.0)).clamp(0.0, 1.0) as f64;
        self.player.seek(frac * dur);
    }

    fn keys(&mut self, key: &Key, mods: &Mods, host: &mut dyn Host) -> bool {
        if mods.cmd() {
            match key {
                Key::Char('f') => {
                    self.search.focused = true;
                    return true;
                }
                Key::Char('l') => {
                    self.command("rescan", host);
                    return true;
                }
                Key::Char('n') => {
                    self.command("playlist-new", host);
                    return true;
                }
                Key::Char('a') => {
                    self.sel = self.rows.clone();
                    return true;
                }
                _ => {}
            }
        }
        match key {
            Key::Space => self.play_pause(),
            Key::Right => {
                if mods.cmd() {
                    self.next()
                } else {
                    let st = self.player.status();
                    self.player.seek(st.position + 5.0);
                }
            }
            Key::Left => {
                if mods.cmd() {
                    self.previous()
                } else {
                    let st = self.player.status();
                    self.player.seek((st.position - 5.0).max(0.0));
                }
            }
            Key::Down => self.move_pick(1),
            Key::Up => self.move_pick(-1),
            Key::Enter => {
                if let Some(id) = self.sel.first().copied() {
                    self.play_row(id);
                }
            }
            Key::Escape => {
                self.sel.clear();
            }
            _ => return false,
        }
        true
    }

    fn move_pick(&mut self, by: i32) {
        if self.rows.is_empty() {
            return;
        }
        let cur = self.sel.first().and_then(|i| self.rows.iter().position(|r| r == i)).map(|p| p as i32).unwrap_or(-1);
        let next = (cur + by).clamp(0, self.rows.len() as i32 - 1) as usize;
        self.sel = vec![self.rows[next]];
        // keep it in view
        let y = next as f32 * ROW_H * self.s;
        let view = (self.list_rect.h - HEAD_H * self.s).max(1.0);
        if y < self.scroll {
            self.scroll = y;
        } else if y + ROW_H * self.s > self.scroll + view {
            self.scroll = y + ROW_H * self.s - view;
        }
    }
}

impl ChordApp {
    // ---------------------------------------------------------------- commands

    fn open_library_menu(&mut self, x: f32, y: f32) {
        let items = vec![
            MenuItem::new("folder-add", "Add a Music Folder..."),
            MenuItem::new("rescan", "Look for New Songs"),
            MenuItem::new("playlist-new", "New Playlist"),
            MenuItem::new("playlist-smart", "New Smart Playlist"),
            MenuItem::new("starters", "Add the Starter Smart Playlists"),
            MenuItem::new("eq", "Equaliser..."),
            MenuItem::new("soundcheck", if self.settings.sound_check { "Sound Check (on)" } else { "Sound Check (off)" }),
        ];
        self.menu = Some(Menu::new("library", Rect::new(x, y, 1.0, 1.0), items));
    }

    fn open_song_menu(&mut self, x: f32, y: f32) {
        let mut items = vec![
            MenuItem::new("play", "Play"),
            MenuItem::new("info", "Get Info"),
        ];
        let lists: Vec<(u64, String)> = self
            .lib
            .playlists
            .iter()
            .filter(|p| p.kind == Kind::Plain)
            .map(|p| (p.id, p.name.clone()))
            .collect();
        for (id, name) in lists {
            items.push(MenuItem::new(&format!("add:{id}"), &format!("Add to {name}")));
        }
        if let Source::Playlist(_) = self.source {
            items.push(MenuItem::new("remove-from", "Take Out of This Playlist"));
        }
        items.push(MenuItem::new("remove", "Take Out of the Library"));
        self.menu = Some(Menu::new("song", Rect::new(x, y, 1.0, 1.0), items));
    }

    pub fn command(&mut self, id: &str, _host: &mut dyn Host) {
        match id {
            "playpause" => self.play_pause(),
            "next" => self.next(),
            "prev" => self.previous(),
            "shuffle" => {
                self.settings.shuffle = !self.settings.shuffle;
                self.settings.save();
            }
            "repeat" => {
                self.settings.repeat = (self.settings.repeat + 1) % 3;
                self.settings.save();
                self.flash(match self.settings.repeat {
                    1 => "Repeat: the whole list.",
                    2 => "Repeat: this song.",
                    _ => "Repeat: off.",
                });
            }
            "folder-add" => {
                let dir = folio_kit::platform::home_dir();
                self.files = Some(FilePane::new(&dir, &["*"], false, ""));
            }
            "rescan" => {
                let (added, changed) = self.lib.rescan();
                self.save_library();
                self.refresh();
                self.flash(&format!("{added} new, {changed} changed."));
            }
            "playlist-new" | "playlist-smart" => {
                let smart = id == "playlist-smart";
                self.dialog = Some(
                    Dialog::new(if smart { "newsmart" } else { "newlist" }, if smart { "New Smart Playlist" } else { "New Playlist" })
                        .text("name", "Name", if smart { "Smart Playlist" } else { "Playlist" })
                        .button("go", "Make it")
                        .button("cancel", "Cancel"),
                );
            }
            "starters" => {
                for (name, rules) in smart::starters() {
                    if self.lib.playlists.iter().any(|p| p.name == name) {
                        continue;
                    }
                    let id = self.lib.new_playlist(&name, Kind::Smart);
                    if let Some(p) = self.lib.playlists.iter_mut().find(|p| p.id == id) {
                        p.rules = rules;
                    }
                }
                self.save_library();
                self.flash("The starter Smart Playlists are in the list.");
            }
            "soundcheck" => {
                self.settings.sound_check = !self.settings.sound_check;
                self.settings.save();
                self.flash(if self.settings.sound_check { "Sound Check is on." } else { "Sound Check is off." });
            }
            "eq" => {
                let b = self.settings.eq;
                let mut d = Dialog::new("eq", "Equaliser").check("on", "Use the equaliser", self.settings.eq_on);
                for (i, f) in folio_chord_engine::player::BANDS.iter().enumerate() {
                    let label = if *f >= 1000.0 { format!("{} kHz", f / 1000.0) } else { format!("{f} Hz") };
                    d = d.text(&format!("b{i}"), &label, &format!("{}", b[i]));
                }
                self.dialog = Some(d.button("go", "Keep").button("cancel", "Cancel"));
            }
            "play" => {
                if let Some(id) = self.sel.first().copied() {
                    self.play_row(id);
                }
            }
            "info" => self.show_info(),
            "remove" => {
                let ids = self.sel.clone();
                self.lib.remove(&ids);
                self.sel.clear();
                self.save_library();
                self.refresh();
            }
            "remove-from" => {
                if let Source::Playlist(pid) = self.source {
                    let ids = self.sel.clone();
                    if let Some(p) = self.lib.playlists.iter_mut().find(|p| p.id == pid) {
                        p.tracks.retain(|i| !ids.contains(i));
                    }
                    self.lib.dirty = true;
                    self.save_library();
                    self.refresh();
                }
            }
            a if a.starts_with("add:") => {
                if let Ok(pid) = a["add:".len()..].parse::<u64>() {
                    let ids = self.sel.clone();
                    self.lib.add_to_playlist(pid, &ids);
                    self.save_library();
                    self.flash(&format!("{} added.", ids.len()));
                }
            }
            _ => self.flash(&format!("{id} isn't in this version yet.")),
        }
    }

    fn show_info(&mut self) {
        let Some(id) = self.sel.first().copied() else { return };
        let Some(t) = self.lib.track(id).cloned() else { return };
        let note = format!(
            "{}\n{} — {}\n\n{} · {} kbps · {} Hz · {}\n{}\nPlayed {} time{}",
            t.tags.title,
            t.artist(),
            t.tags.album,
            t.tags.kind,
            t.tags.bitrate,
            t.tags.sample_rate,
            t.length(),
            t.path.to_string_lossy(),
            t.play_count,
            if t.play_count == 1 { "" } else { "s" }
        );
        self.dialog = Some(
            Dialog::new("info", "Get Info")
                .note(&note)
                .text("title", "Name", &t.tags.title)
                .text("artist", "Artist", &t.tags.artist)
                .text("album", "Album", &t.tags.album)
                .text("genre", "Genre", &t.tags.genre)
                .button("go", "Keep")
                .button("cancel", "Cancel"),
        );
    }

    fn dialog_done(&mut self, d: Dialog, button: &str) {
        if button != "go" {
            return;
        }
        match d.id.as_str() {
            "newlist" | "newsmart" => {
                let name = d.value("name");
                if name.trim().is_empty() {
                    return;
                }
                let kind = if d.id == "newsmart" { Kind::Smart } else { Kind::Plain };
                let id = self.lib.new_playlist(name.trim(), kind);
                if kind == Kind::Smart {
                    if let Some(p) = self.lib.playlists.iter_mut().find(|p| p.id == id) {
                        p.rules = smart::Rules::default();
                    }
                }
                if kind == Kind::Plain {
                    let ids = self.sel.clone();
                    self.lib.add_to_playlist(id, &ids);
                }
                self.save_library();
                self.source = Source::Playlist(id);
                self.refresh();
            }
            "info" => {
                let Some(id) = self.sel.first().copied() else { return };
                if let Some(t) = self.lib.track_mut(id) {
                    t.tags.title = d.value("title");
                    t.tags.artist = d.value("artist");
                    t.tags.album = d.value("album");
                    t.tags.genre = d.value("genre");
                }
                self.lib.dirty = true;
                self.save_library();
                self.refresh();
                self.flash("Kept in the library. The file itself is not written yet.");
            }
            "eq" => {
                let mut b = [0.0f64; 10];
                for (i, v) in b.iter_mut().enumerate() {
                    *v = d.value(&format!("b{i}")).parse::<f64>().unwrap_or(0.0).clamp(-12.0, 12.0);
                }
                self.settings.eq = b;
                self.settings.eq_on = d.checked("on");
                self.settings.save();
                self.player.set_eq(b, self.settings.eq_on);
            }
            _ => {}
        }
    }

    pub fn add_folder(&mut self, p: &std::path::Path) {
        let dir = if p.is_dir() { p.to_path_buf() } else { p.parent().map(|d| d.to_path_buf()).unwrap_or_default() };
        if dir.as_os_str().is_empty() {
            return;
        }
        self.lib.add_folder(&dir);
        let (added, _) = self.lib.scan(&dir);
        self.save_library();
        self.refresh();
        self.flash(&format!("{added} song{} from {}.", if added == 1 { "" } else { "s" }, dir.to_string_lossy()));
    }

    /// A song that ended, or one that could not be played: move on.
    pub fn after_song(&mut self) {
        if let Some(e) = self.player.take_error() {
            self.flash(&e);
            let _ = self.player.take_finished();
            self.skip_unplayable();
            return;
        }
        if self.player.take_finished() {
            self.next();
        }
    }

    /// Step past a song the decoder could not open, without counting a play.
    fn skip_unplayable(&mut self) {
        if self.queue.is_empty() {
            self.now = None;
            return;
        }
        if self.queue_at + 1 < self.queue.len() {
            self.queue_at += 1;
            let id = self.queue[self.queue_at];
            self.start(id);
        } else {
            self.now = None;
        }
    }

    pub fn save_library(&mut self) {
        if self.lib.dirty {
            if let Err(e) = self.lib.save() {
                self.flash(&e);
            }
        }
    }
}

impl folio_kit::platform::App for ChordApp {
    fn event(&mut self, ev: &Event, host: &mut dyn Host) {
        self.clock = host.now();
        self.handle(ev, host);
        host.set_title(&self.window_title());
        if self.exit {
            self.save_library();
            host.close();
        }
    }

    fn paint(&mut self, pixels: &mut [u8], w: u32, h: u32, host: &mut dyn Host) {
        self.clock = host.now();
        self.after_song();
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
        Some(Duration::from_millis(250))
    }

    fn wants_exit(&self) -> bool {
        self.exit
    }

    fn exit_code(&self) -> i32 {
        self.exit_code
    }
}
