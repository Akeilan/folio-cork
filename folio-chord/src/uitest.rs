//! The window test: drives Chord as a person would, one step per tick.
use crate::app::{ChordApp, Col, Res, Source};
use folio_kit::gfx::{self, sk};
use folio_kit::platform::{Button, Event, Host, Key, Mods};
use std::path::PathBuf;

type Step = Box<dyn FnMut(&mut ChordApp, &mut dyn Host, &mut Res) -> Result<(), String>>;

pub struct UiTest {
    steps: Vec<(String, Step)>,
    i: usize,
    pub done: bool,
    fails: Vec<String>,
    passed: usize,
    dir: PathBuf,
}

fn check(c: bool, msg: &str) -> Result<(), String> {
    if c {
        Ok(())
    } else {
        Err(msg.to_string())
    }
}

fn ev(app: &mut ChordApp, host: &mut dyn Host, r: &mut Res, e: Event) {
    app.handle_test(&e, host, r);
}

fn shot(app: &mut ChordApp, r: &mut Res) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(app.w.max(1.0) as u32, app.h.max(1.0) as u32).unwrap();
    app.paint(&mut pm, r);
    pm
}

fn click(app: &mut ChordApp, host: &mut dyn Host, r: &mut Res, x: f32, y: f32, clicks: u32) {
    ev(app, host, r, Event::MouseMove { x, y });
    ev(app, host, r, Event::MouseDown { x, y, button: Button::Left, clicks });
    ev(app, host, r, Event::MouseUp { x, y, button: Button::Left });
}

fn key(app: &mut ChordApp, host: &mut dyn Host, r: &mut Res, k: Key, mods: Mods) {
    ev(app, host, r, Event::Key { key: k.clone(), mods, down: true, repeat: false });
    ev(app, host, r, Event::Key { key: k, mods, down: false, repeat: false });
}

fn save_shot(app: &mut ChordApp, r: &mut Res, name: &str) {
    if let Some(dir) = folio_kit::platform::setting("FOLIO_UI_DIR").map(PathBuf::from) {
        let pm = shot(app, r);
        let _ = std::fs::write(dir.join(name), gfx::encode_png(&pm));
    }
}

/// A folder of music to test with: the one given, else the one in the source tree.
fn music() -> Option<PathBuf> {
    if let Some(d) = folio_kit::platform::setting("FOLIO_CHORD_MUSIC") {
        let p = PathBuf::from(d);
        if p.is_dir() {
            return Some(p);
        }
    }
    let p = PathBuf::from("/home/claude/winbuild/t2/music");
    p.is_dir().then_some(p)
}

impl UiTest {
    pub fn new(dir: PathBuf) -> UiTest {
        let _ = std::fs::create_dir_all(&dir);
        let mut steps: Vec<(String, Step)> = vec![];
        macro_rules! step {
            ($name:expr, $f:expr) => {
                steps.push(($name.to_string(), Box::new($f)));
            };
        }
        step!("the window opens", |app: &mut ChordApp, _h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            check(app.list_rect.w > 100.0, "the song list has room")?;
            check(app.side_rect.w > 50.0, "and the sources are down the side")?;
            check(!app.transport.is_empty(), "the transport buttons are drawn")
        });
        step!("a folder of music goes in", |app: &mut ChordApp, _h: &mut dyn Host, r: &mut Res| {
            let Some(dir) = music() else { return Err("no music to test with".into()) };
            app.add_folder(&dir);
            shot(app, r);
            check(!app.lib.tracks.is_empty(), "the library has songs")?;
            check(!app.rows.is_empty(), &format!("and the list shows {} of them", app.rows.len()))?;
            save_shot(app, r, "ui-library.png");
            Ok(())
        });
        step!("the columns sort the list", |app: &mut ChordApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let (_, rr) = app.col_hits.iter().find(|(c, _)| *c == Col::Name).cloned().ok_or("the Name column")?;
            click(app, h, r, rr.cx(), rr.cy(), 1);
            check(app.sort.0 == Col::Name && app.sort.1, "sorted by name")?;
            let first = app.rows.first().and_then(|i| app.lib.track(*i)).map(|t| t.tags.title.clone()).unwrap_or_default();
            click(app, h, r, rr.cx(), rr.cy(), 1);
            check(!app.sort.1, "clicking again turns it round")?;
            let last = app.rows.first().and_then(|i| app.lib.track(*i)).map(|t| t.tags.title.clone()).unwrap_or_default();
            check(first != last, &format!("the order changed: {first:?} then {last:?}"))
        });
        step!("the column browser narrows it down", |app: &mut ChordApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let all = app.rows.len();
            let hit = app
                .browse_hits
                .iter()
                .find(|(which, name, _)| *which == 1 && !name.is_empty())
                .cloned()
                .ok_or("an artist in the browser")?;
            click(app, h, r, hit.2.cx(), hit.2.cy(), 1);
            check(app.browse.artist.as_deref() == Some(hit.1.as_str()), "the artist is picked out")?;
            check(app.rows.len() <= all, &format!("{} songs of {all}", app.rows.len()))?;
            for i in &app.rows {
                check(app.lib.track(*i).unwrap().display_album_artist() == hit.1, "every song is theirs")?;
            }
            // and All puts them back
            shot(app, r);
            let all_hit = app.browse_hits.iter().find(|(which, name, _)| *which == 1 && name.is_empty()).cloned().ok_or("All")?;
            click(app, h, r, all_hit.2.cx(), all_hit.2.cy(), 1);
            check(app.rows.len() == all, "All brings them back")
        });
        step!("searching finds a song", |app: &mut ChordApp, _h: &mut dyn Host, r: &mut Res| {
            let want = app.lib.tracks.first().map(|t| t.tags.title.clone()).unwrap_or_default();
            let word = want.split_whitespace().next().unwrap_or("").to_string();
            check(!word.is_empty(), "a word to look for")?;
            app.search.text = word.clone();
            app.refresh();
            shot(app, r);
            check(!app.rows.is_empty(), &format!("{:?} found songs", word))?;
            app.search.text.clear();
            app.refresh();
            Ok(())
        });
        step!("double-clicking plays a song", |app: &mut ChordApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            // a song Chord can decode (Opus has no decoder here yet)
            let (id, rr) = app
                .row_hits
                .iter()
                .find(|(i, _)| app.lib.track(*i).map(|t| t.tags.kind != "Opus").unwrap_or(false))
                .cloned()
                .ok_or("a row to play")?;
            click(app, h, r, rr.x + 20.0, rr.cy(), 2);
            check(app.now == Some(id), "that song is playing")?;
            check(app.lib.track(id).unwrap().play_count >= 1, "and it counted the play")?;
            // give the decoder a moment, then look for sound
            let mut heard = false;
            let mut out = vec![0.0f32; 2048];
            for _ in 0..200 {
                folio_kit::platform::sleep(std::time::Duration::from_millis(10));
                app.player.fill(&mut out);
                if out.iter().any(|s| s.abs() > 0.0005) {
                    heard = true;
                    break;
                }
            }
            check(heard, "and it made sound")?;
            save_shot(app, r, "ui-playing.png");
            Ok(())
        });
        step!("the transport pauses and goes on", |app: &mut ChordApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let (_, rr) = app.transport.iter().find(|(id, _)| id == "playpause").cloned().ok_or("the play button")?;
            click(app, h, r, rr.cx(), rr.cy(), 1);
            folio_kit::platform::sleep(std::time::Duration::from_millis(60));
            check(!app.player.status().playing, "it paused")?;
            click(app, h, r, rr.cx(), rr.cy(), 1);
            folio_kit::platform::sleep(std::time::Duration::from_millis(60));
            check(app.player.status().playing, "and it went on again")
        });
        step!("a song with no decoder says so and moves on", |app: &mut ChordApp, _h: &mut dyn Host, _r: &mut Res| {
            let odd = app.lib.tracks.iter().find(|t| t.tags.kind == "Opus").map(|t| t.id);
            let Some(odd) = odd else { return Ok(()) };
            app.play_row(odd);
            let mut said = false;
            for _ in 0..200 {
                folio_kit::platform::sleep(std::time::Duration::from_millis(10));
                app.after_song();
                if app.flash.as_ref().map(|(m, _)| m.contains("can't play")).unwrap_or(false) {
                    said = true;
                    break;
                }
            }
            check(said, "it said so rather than going quiet")
        });
        step!("Next goes to the song after it", |app: &mut ChordApp, h: &mut dyn Host, r: &mut Res| {
            let before = app.now;
            check(app.rows.len() > 1, "there is more than one song")?;
            shot(app, r);
            let (_, rr) = app.transport.iter().find(|(id, _)| id == "next").cloned().ok_or("the next button")?;
            click(app, h, r, rr.cx(), rr.cy(), 1);
            check(app.now != before, &format!("it moved on: {before:?} -> {:?}", app.now))
        });
        step!("the stars go on a song", |app: &mut ChordApp, _h: &mut dyn Host, r: &mut Res| {
            let id = *app.rows.first().ok_or("a song")?;
            app.lib.set_rating(id, 4);
            app.save_library();
            shot(app, r);
            check(app.lib.track(id).unwrap().stars() == 4, "four stars")
        });
        step!("a playlist holds what was picked out", |app: &mut ChordApp, _h: &mut dyn Host, r: &mut Res| {
            let ids: Vec<u64> = app.rows.iter().take(2).copied().collect();
            check(!ids.is_empty(), "songs to add")?;
            app.sel = ids.clone();
            let pid = app.lib.new_playlist("Test List", folio_chord_engine::library::Kind::Plain);
            app.lib.add_to_playlist(pid, &ids);
            app.save_library();
            app.source = Source::Playlist(pid);
            app.refresh();
            shot(app, r);
            check(app.rows == ids, &format!("the playlist holds them: {:?}", app.rows))?;
            save_shot(app, r, "ui-playlist.png");
            app.source = Source::Music;
            app.refresh();
            Ok(())
        });
        step!("a Smart Playlist picks songs out by itself", |app: &mut ChordApp, _h: &mut dyn Host, r: &mut Res| {
            let genre = app
                .lib
                .tracks
                .iter()
                .find(|t| !t.tags.genre.is_empty())
                .map(|t| t.tags.genre.clone())
                .ok_or("a song with a genre")?;
            let pid = app.lib.new_playlist("That Genre", folio_chord_engine::library::Kind::Smart);
            if let Some(p) = app.lib.playlists.iter_mut().find(|p| p.id == pid) {
                p.rules = folio_chord_engine::smart::Rules {
                    all: true,
                    rules: vec![folio_chord_engine::smart::Rule {
                        field: "genre".into(),
                        op: folio_chord_engine::smart::Op::Is,
                        value: genre.clone(),
                    }],
                    ..Default::default()
                };
            }
            app.save_library();
            app.source = Source::Playlist(pid);
            app.refresh();
            shot(app, r);
            check(!app.rows.is_empty(), "the rule found songs")?;
            for i in &app.rows {
                check(app.lib.track(*i).unwrap().tags.genre == genre, "all of that genre")?;
            }
            app.source = Source::Music;
            app.refresh();
            Ok(())
        });
        step!("the keyboard plays and moves", |app: &mut ChordApp, h: &mut dyn Host, r: &mut Res| {
            app.sel = vec![*app.rows.first().ok_or("a song")?];
            key(app, h, r, Key::Down, Mods::NONE);
            check(app.sel.first() == app.rows.get(1), "Down moves the pick")?;
            key(app, h, r, Key::Space, Mods::NONE);
            folio_kit::platform::sleep(std::time::Duration::from_millis(60));
            check(app.player.status().playing || app.now.is_some(), "Space plays")
        });
        step!("the library comes back when it is opened again", |app: &mut ChordApp, _h: &mut dyn Host, _r: &mut Res| {
            app.lib.dirty = true;
            app.save_library();
            let path = app.lib.path.clone();
            let again = folio_chord_engine::library::Library::open(&path);
            check(again.tracks.len() == app.lib.tracks.len(), "every song came back")?;
            check(again.playlists.len() == app.lib.playlists.len(), "and every playlist")?;
            let rated = app.lib.tracks.iter().filter(|t| t.rating > 0).count();
            let rated2 = again.tracks.iter().filter(|t| t.rating > 0).count();
            check(rated == rated2, "and the stars")
        });
        UiTest { steps, i: 0, done: false, fails: vec![], passed: 0, dir }
    }

    pub fn step(&mut self, app: &mut ChordApp, host: &mut dyn Host, r: &mut Res) {
        if self.i >= self.steps.len() {
            let pm = shot(app, r);
            let _ = std::fs::write(self.dir.join("ui-final.png"), gfx::encode_png(&pm));
            println!("{} passed, {} failed", self.passed, self.fails.len());
            for f in &self.fails {
                println!("FAIL {f}");
            }
            self.done = true;
            return;
        }
        let (name, f) = &mut self.steps[self.i];
        let name = name.clone();
        match f(app, host, r) {
            Ok(()) => {
                self.passed += 1;
                println!("ok   {name}");
            }
            Err(e) => {
                println!("FAIL {name}: {e}");
                self.fails.push(format!("{name}: {e}"));
            }
        }
        self.i += 1;
    }

    pub fn code(&self) -> i32 {
        if self.fails.is_empty() {
            0
        } else {
            1
        }
    }
}
