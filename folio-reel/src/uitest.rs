//! The window test: drives Reel as a person would, one step per paint.
use crate::app::{Drag, ReelApp, Res};
use folio_kit::gfx::{self, sk};
use folio_kit::platform::media;
use folio_kit::platform::{Button, Event, Host, Key, Mods};
use folio_reel_engine::project::{self as pr, Kind, Media};
use std::path::PathBuf;

type Step = Box<dyn FnMut(&mut ReelApp, &mut dyn Host, &mut Res) -> Result<(), String>>;

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

fn ev(app: &mut ReelApp, host: &mut dyn Host, r: &mut Res, e: Event) {
    app.handle_test(&e, host, r);
}

fn shot(app: &mut ReelApp, r: &mut Res) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(app.w.max(1.0) as u32, app.h.max(1.0) as u32).unwrap();
    app.paint(&mut pm, r);
    pm
}

fn click(app: &mut ReelApp, host: &mut dyn Host, r: &mut Res, x: f32, y: f32) {
    ev(app, host, r, Event::MouseMove { x, y });
    ev(app, host, r, Event::MouseDown { x, y, button: Button::Left, clicks: 1 });
    ev(app, host, r, Event::MouseUp { x, y, button: Button::Left });
}

fn dclick(app: &mut ReelApp, host: &mut dyn Host, r: &mut Res, x: f32, y: f32) {
    ev(app, host, r, Event::MouseMove { x, y });
    ev(app, host, r, Event::MouseDown { x, y, button: Button::Left, clicks: 1 });
    ev(app, host, r, Event::MouseUp { x, y, button: Button::Left });
    ev(app, host, r, Event::MouseDown { x, y, button: Button::Left, clicks: 2 });
    ev(app, host, r, Event::MouseUp { x, y, button: Button::Left });
}

fn drag(app: &mut ReelApp, host: &mut dyn Host, r: &mut Res, from: (f32, f32), to: (f32, f32)) {
    ev(app, host, r, Event::MouseMove { x: from.0, y: from.1 });
    ev(app, host, r, Event::MouseDown { x: from.0, y: from.1, button: Button::Left, clicks: 1 });
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        ev(app, host, r, Event::MouseMove { x: from.0 + (to.0 - from.0) * t, y: from.1 + (to.1 - from.1) * t });
    }
    ev(app, host, r, Event::MouseUp { x: to.0, y: to.1, button: Button::Left });
}

fn key(app: &mut ReelApp, host: &mut dyn Host, r: &mut Res, k: Key, mods: Mods) {
    ev(app, host, r, Event::Key { key: k.clone(), mods, down: true, repeat: false });
    ev(app, host, r, Event::Key { key: k, mods, down: false, repeat: false });
}

fn save_shot(app: &mut ReelApp, r: &mut Res, name: &str) {
    if let Some(dir) = folio_kit::platform::setting("FOLIO_UI_DIR").map(PathBuf::from) {
        let pm = shot(app, r);
        let _ = std::fs::write(dir.join(name), gfx::encode_png(&pm));
    }
}

fn dir_of() -> PathBuf {
    PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()))
}

/// Short test films, made with whatever is on the computer. Reel can't invent
/// an H.264 file, so when there is no ffmpeg the test proves the message instead.
fn make_media(dir: &std::path::Path) -> bool {
    if !media::have() {
        return false;
    }
    let a = dir.join("a.mp4");
    let c = dir.join("c.mp4");
    let m = dir.join("music.mp3");
    let s = dir.join("still.png");
    if a.is_file() && c.is_file() && m.is_file() && s.is_file() {
        return true;
    }
    let runs: Vec<Vec<String>> = vec![
        vec!["-y".into(), "-f".into(), "lavfi".into(), "-i".into(), "testsrc=size=320x180:rate=25:duration=6".into(),
             "-f".into(), "lavfi".into(), "-i".into(), "sine=frequency=440:duration=6".into(),
             "-c:v".into(), "libx264".into(), "-pix_fmt".into(), "yuv420p".into(), "-c:a".into(), "aac".into(),
             "-shortest".into(), a.to_string_lossy().to_string()],
        vec!["-y".into(), "-f".into(), "lavfi".into(), "-i".into(), "smptebars=size=320x180:rate=25:duration=5".into(),
             "-f".into(), "lavfi".into(), "-i".into(), "sine=frequency=880:duration=5".into(),
             "-c:v".into(), "libx264".into(), "-pix_fmt".into(), "yuv420p".into(), "-c:a".into(), "aac".into(),
             "-shortest".into(), c.to_string_lossy().to_string()],
        vec!["-y".into(), "-f".into(), "lavfi".into(), "-i".into(), "sine=frequency=220:duration=12".into(), m.to_string_lossy().to_string()],
        vec!["-y".into(), "-f".into(), "lavfi".into(), "-i".into(), "color=c=orange:s=400x300:d=1".into(),
             "-frames:v".into(), "1".into(), s.to_string_lossy().to_string()],
    ];
    for args in runs {
        if media::export(&args, 1.0, |_| {}).is_err() {
            return false;
        }
    }
    a.is_file() && c.is_file() && m.is_file() && s.is_file()
}

/// A bin entry without touching a file, for the steps that only need the shape.
fn pretend(name: &str, secs: f64, video: bool, audio: bool) -> Media {
    Media {
        uri: format!("file:///tmp/folio-reel-test/{name}"),
        name: name.to_string(),
        duration: secs,
        width: if video { 320 } else { 0 },
        height: if video { 180 } else { 0 },
        fps: if video { 25.0 } else { 0.0 },
        has_video: video,
        has_audio: audio,
        is_image: false,
        relative: String::new(),
    }
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

        step!("the window opens", |app: &mut ReelApp, _h: &mut dyn Host, r: &mut Res| {
            let pm = shot(app, r);
            check(pm.width() > 600 && pm.height() > 400, "the window has a size")?;
            check(app.p.duration() == 0.0, "and nothing on the timeline yet")?;
            save_shot(app, r, "ui-open.png");
            Ok(())
        });

        step!("the media bin takes files in", |app: &mut ReelApp, _h: &mut dyn Host, _r: &mut Res| {
            let dir = dir_of();
            if make_media(&dir) {
                app.bring_in(&dir.join("a.mp4"));
                app.bring_in(&dir.join("c.mp4"));
                app.bring_in(&dir.join("music.mp3"));
                app.bring_in(&dir.join("still.png"));
                check(app.p.data.media.len() == 4, &format!("4 files, not {}", app.p.data.media.len()))?;
                let a = app.p.media_by_uri(&pr::path_to_uri(&dir.join("a.mp4"))).ok_or("a.mp4 is in the bin")?;
                check(a.has_video && a.has_audio, "a.mp4 has picture and sound")?;
                check((a.duration - 6.0).abs() < 0.5, &format!("and is about six seconds, not {:.2}", a.duration))?;
                let still = app.p.media_by_uri(&pr::path_to_uri(&dir.join("still.png"))).ok_or("the picture is in the bin")?;
                check(still.is_image, "and the png came in as a still")
            } else {
                // no ffmpeg: the bin stays empty and Reel says why
                app.bring_in(&dir.join("a.mp4"));
                check(app.p.data.media.is_empty(), "nothing came in")?;
                let said = app.dialog.is_some();
                app.dialog = None;
                check(said, "and Reel said so plainly")?;
                for m in [pretend("a.mp4", 6.0, true, true), pretend("c.mp4", 5.0, true, true), pretend("music.mp3", 12.0, false, true)] {
                    app.p.add_media(m);
                }
                Ok(())
            }
        });

        step!("a clip goes on the video track", |app: &mut ReelApp, h: &mut dyn Host, r: &mut Res| {
            // the bin rows are only known once it has been drawn
            shot(app, r);
            let row = app.bin_hits.first().map(|(_, rr)| (rr.cx(), rr.cy()));
            match row {
                Some((x, y)) => {
                    click(app, h, r, x, y);
                    check(app.bin_sel == Some(0), "one click picks the file")?;
                    check(app.p.track(1).is_empty(), "and does not place it")?;
                    dclick(app, h, r, x, y);
                }
                None => return Err("the bin drew its rows".into()),
            }
            check(app.p.track(1).len() == 1, &format!("one clip on Video, not {}", app.p.track(1).len()))?;
            check(app.p.duration() > 1.0, "and the film has a length")?;
            save_shot(app, r, "ui-one-clip.png");
            Ok(())
        });

        step!("the playhead moves and the preview follows", |app: &mut ReelApp, h: &mut dyn Host, r: &mut Res| {
            let x = app.time_to_x(2.0);
            let y = app.timeline.y + 8.0 * app.s;
            click(app, h, r, x, y);
            check((app.playhead - 2.0).abs() < 0.3, &format!("the playhead is at 2s, not {:.2}", app.playhead))?;
            // let the decoder catch up
            for _ in 0..200 {
                app.tick();
                if app.frames.shot.is_some() {
                    break;
                }
                folio_kit::platform::sleep(std::time::Duration::from_millis(10));
            }
            if media::have() {
                let shot = app.frames.shot.as_ref().ok_or("a picture came back from the decoder")?;
                check(shot.pm.width() > 40, &format!("and it has a size ({}px)", shot.pm.width()))?;
            }
            save_shot(app, r, "ui-preview.png");
            Ok(())
        });

        step!("a second clip queues after the first", |app: &mut ReelApp, _h: &mut dyn Host, _r: &mut Res| {
            app.bin_sel = Some(1);
            app.place(Some(1));
            check(app.p.track(1).len() == 2, &format!("two clips, not {}", app.p.track(1).len()))?;
            let first_end = app.p.track(1)[0].end();
            let second = app.p.track(1)[1].start;
            check((second - first_end).abs() < 0.01, &format!("the second starts where the first ends ({second:.2} vs {first_end:.2})"))
        });

        step!("a crossfade joins the two", |app: &mut ReelApp, _h: &mut dyn Host, _r: &mut Res| {
            let id = app.p.track(1)[1].id;
            app.sel = vec![id];
            let was = app.p.duration();
            app.command("tr-crossfade", _h, _r);
            let c = app.p.clip(id).ok_or("the clip is still there")?;
            check(c.transition.is_some(), "it has a transition")?;
            check(app.p.duration() < was, &format!("and the film is shorter for the overlap ({:.2} was {:.2})", app.p.duration(), was))
        });

        step!("the clip splits at the playhead", |app: &mut ReelApp, h: &mut dyn Host, r: &mut Res| {
            app.seek(1.5);
            let before = app.p.track(1).len();
            key(app, h, r, Key::Char('s'), Mods::NONE);
            let after = app.p.track(1).len();
            check(after == before + 1, &format!("{before} clips became {after}"))?;
            save_shot(app, r, "ui-split.png");
            Ok(())
        });

        step!("undo puts it back and redo splits it again", |app: &mut ReelApp, h: &mut dyn Host, r: &mut Res| {
            let after = app.p.track(1).len();
            key(app, h, r, Key::Char('z'), Mods { ctrl: true, ..Mods::NONE });
            check(app.p.track(1).len() == after - 1, "undo joined it up")?;
            key(app, h, r, Key::Char('y'), Mods { ctrl: true, ..Mods::NONE });
            check(app.p.track(1).len() == after, "and redo split it again")
        });

        step!("a clip drags to another place and trims by its edge", |app: &mut ReelApp, h: &mut dyn Host, r: &mut Res| {
            let id = app.p.track(1).last().map(|c| c.id).ok_or("there's a clip to drag")?;
            let (ti, c) = app.p.find(id).ok_or("it's findable")?;
            let was = (c.start, c.duration);
            let cr = app.clip_rect(ti, c);
            // trim the right edge inwards by a quarter of a second
            let px = (0.25 / app.scale) as f32;
            drag(app, h, r, (cr.right() - 1.0, cr.cy()), (cr.right() - px, cr.cy()));
            let now = app.p.clip(id).ok_or("still there")?.duration;
            check(now < was.1 - 0.05, &format!("the clip is shorter ({now:.2} was {:.2})", was.1))?;
            check(matches!(app.drag, Drag::None), "and the drag finished")
        });

        step!("a title goes on the Titles track", |app: &mut ReelApp, _h: &mut dyn Host, r: &mut Res| {
            app.seek(0.5);
            app.command("title", _h, r);
            let d = app.dialog.as_mut().ok_or("the title question came up")?;
            d.set_text_for_test("text", "Our Holiday");
            let d = app.dialog.take().unwrap();
            app.dialog_done_for_test(d, "ok");
            check(app.p.track(0).len() == 1, &format!("one title, not {}", app.p.track(0).len()))?;
            let c = app.p.track(0)[0].clone();
            check(c.kind == Kind::Title && c.text == "Our Holiday", &format!("with the right words: {}", c.text))?;
            // and the preview shows the very picture the export will hold
            app.seek(c.start + c.duration / 2.0);
            let pm = shot(app, r);
            let lit = pm.pixels().iter().filter(|px| px.red() > 200 && px.green() > 200 && px.blue() > 200).count();
            check(lit > 200, &format!("the words are on the preview ({lit} light pixels)"))?;
            // a title over the film does not blank the film underneath it
            app.seek(c.start + c.duration / 2.0);
            for _ in 0..200 {
                app.tick();
                if app.frames.shot.is_some() {
                    break;
                }
                folio_kit::platform::sleep(std::time::Duration::from_millis(10));
            }
            if media::have() {
                check(app.frames.shot.is_some(), "the picture under the title is still being decoded")?;
            }
            save_shot(app, r, "ui-title.png");
            Ok(())
        });

        step!("the music goes under, and dips for the voice", |app: &mut ReelApp, _h: &mut dyn Host, _r: &mut Res| {
            let i = app.p.data.media.iter().position(|m| m.has_audio && !m.has_video).ok_or("there's a sound file in the bin")?;
            app.bin_sel = Some(i);
            app.place(Some(3));
            check(app.p.track(3).len() == 1, "the music is on the Music track")?;
            let id = app.p.track(3)[0].id;
            app.sel = vec![id];
            app.command("music-bed", _h, _r);
            check(app.p.clip(id).map(|c| c.music_bed).unwrap_or(false), "and it is a music bed")?;
            // and a voice on the Audio track makes it duck
            let v = app.p.data.media.iter().position(|m| m.has_audio && m.has_video).ok_or("a clip with sound")?;
            app.bin_sel = Some(v);
            app.place(Some(2));
            let pts = app.p.duck_points(id, 0.3, 0.4);
            check(pts.iter().any(|(_, g)| *g < 0.5), &format!("the music dips: {pts:?}"))
        });

        step!("a marker goes down and the playhead jumps to it", |app: &mut ReelApp, h: &mut dyn Host, r: &mut Res| {
            app.seek(2.0);
            key(app, h, r, Key::Char('m'), Mods::NONE);
            let d = app.dialog.as_mut().ok_or("the marker question came up")?;
            d.set_text_for_test("name", "Cue");
            let d = app.dialog.take().unwrap();
            app.dialog_done_for_test(d, "ok");
            check(app.p.markers().len() == 1, "there's a marker")?;
            app.seek(0.0);
            app.command("next-marker", h, r);
            check((app.playhead - 2.0).abs() < 0.01, &format!("and the playhead is on it, not at {:.2}", app.playhead))
        });

        step!("the properties pane shows the clip", |app: &mut ReelApp, _h: &mut dyn Host, r: &mut Res| {
            let id = app.p.track(1)[0].id;
            app.sel = vec![id];
            let pm = shot(app, r);
            check(app.settings.show_props, "the pane is open")?;
            check(pm.width() > 0 && app.props.w > 100.0, &format!("and it has room ({:.0}px)", app.props.w))?;
            save_shot(app, r, "ui-props.png");
            Ok(())
        });

        step!("the film saves and opens again the same", |app: &mut ReelApp, _h: &mut dyn Host, _r: &mut Res| {
            let path = dir_of().join("ui-test.reel");
            check(app.save_to(Some(path.clone())), "it saved")?;
            check(path.is_file(), "the file is there")?;
            let counts: Vec<usize> = (0..pr::TRACKS.len()).map(|i| app.p.track(i).len()).collect();
            let (d, markers) = (app.p.duration(), app.p.markers().len());
            app.open_path(&path);
            let again: Vec<usize> = (0..pr::TRACKS.len()).map(|i| app.p.track(i).len()).collect();
            check(again == counts, &format!("{counts:?} came back as {again:?}"))?;
            check((app.p.duration() - d).abs() < 0.01, "the same length")?;
            check(app.p.markers().len() == markers, "and the markers came too")
        });

        step!("it writes a film out, titles and all", |app: &mut ReelApp, _h: &mut dyn Host, r: &mut Res| {
            let out = dir_of().join("ui-test.mp4");
            let _ = std::fs::remove_file(&out);
            if !media::have() {
                app.export_to(out.clone(), "Smaller file (MP4, 720p)", &mut r.text);
                let said = app.dialog.take().is_some();
                return check(said, "with no ffmpeg, Reel says so instead of pretending");
            }
            app.export_to(out.clone(), "Smaller file (MP4, 720p)", &mut r.text);
            check(app.job.is_some(), "the export started")?;
            for _ in 0..3000 {
                app.tick();
                if app.job.is_none() {
                    break;
                }
                folio_kit::platform::sleep(std::time::Duration::from_millis(20));
            }
            let trouble = app.dialog.as_ref().filter(|d| d.id == "say").map(|d| d.title.clone());
            check(out.is_file(), &format!("the file is there{}", trouble.map(|t| format!(" ({t})")).unwrap_or_default()))?;
            app.dialog = None;
            let got = media::probe(&out).map_err(|e| e.to_string())?;
            check(got.has_video, &format!("with a picture ({})", got.video_codec))?;
            check(got.has_audio, &format!("and sound ({})", got.audio_codec))?;
            check((got.width, got.height) == (1280, 720), &format!("at 1280x720, not {}x{}", got.width, got.height))?;
            check(got.duration > 1.0, &format!("and {:.2}s long", got.duration))?;
            // the title is ours to draw, so it has to be in the film, not only the preview
            let at = app.p.track(0).first().map(|c| c.start + c.duration / 2.0).ok_or("there's a title")?;
            let f = media::frame(&out, at, 320).map_err(|e| e.to_string())?;
            let ink = f.rgba.chunks_exact(4).filter(|px| px[0] > 200 && px[1] > 200 && px[2] > 200).count();
            check(ink > 50, &format!("the title is in the picture ({ink} light pixels at {at:.2}s)"))?;
            save_shot(app, r, "ui-exported.png");
            Ok(())
        });

        step!("it says plainly when the computer has no ffmpeg", |app: &mut ReelApp, _h: &mut dyn Host, _r: &mut Res| {
            let msg = media::missing_message();
            check(msg.contains("ffmpeg"), "the message names ffmpeg")?;
            check(msg.contains("Install"), "and says what to do about it")?;
            // with ffmpeg away, nothing is claimed that can't be done
            folio_kit::platform::set_setting("FOLIO_NO_MEDIA", std::path::Path::new("1"));
            let have = media::have();
            folio_kit::platform::set_setting("FOLIO_NO_MEDIA", std::path::Path::new(""));
            check(!have, "and Reel believes a computer that hasn't got it")?;
            check(media::have() || !media::have(), "whichever this computer is")?;
            let _ = app;
            Ok(())
        });

        UiTest { steps, i: 0, done: false, fails: vec![], passed: 0, dir }
    }

    pub fn step(&mut self, app: &mut ReelApp, host: &mut dyn Host, r: &mut Res) {
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
