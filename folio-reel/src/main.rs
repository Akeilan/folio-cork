//! Folio Reel on the portable base (R3-11): the video editor.
//!
//! Reel hands each file to ffmpeg when the computer has it, and says so plainly
//! when it does not. Everything else — the project, the tracks, the edits, the
//! command the export runs — is ours.

mod app;
mod icons;
mod preview;
mod ribbon_defs;
mod settings;
mod srt;
mod uitest;

use folio_kit::platform::media;
use folio_kit::platform::{run, Image, WindowSpec};
use folio_reel_engine::project::{self as pr, Project};
use folio_reel_engine::render;
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!(
        "Usage: folio-reel [FILE.reel]\n       \
         folio-reel --export OUT.mp4|OUT.webm IN.reel [--preset NAME]\n       \
         folio-reel --frame OUT.png IN.reel [AT_SECONDS]\n       \
         folio-reel --info IN.reel\n       \
         folio-reel --probe FILE\n       \
         folio-reel --presets\n       \
         folio-reel --ui-test [DIR] [--headless]\n       \
         folio-reel --version"
    );
}

fn open_or_exit(path: &str) -> Project {
    match pr::open(Path::new(path)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e.0);
            std::process::exit(1);
        }
    }
}

/// Write a film out, without opening a window.
fn export(out: &str, input: &str, preset: &str) -> i32 {
    if !media::have() {
        eprintln!("{}", media::missing_message());
        return 1;
    }
    let p = open_or_exit(input);
    let webm = Path::new(out).extension().map(|e| e.eq_ignore_ascii_case("webm")).unwrap_or(false);
    let (_, fmt, size) = match pr::EXPORT_PRESETS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(preset)) {
        Some(x) => *x,
        None => {
            let want = if webm { "webm" } else { "mp4" };
            match pr::EXPORT_PRESETS.iter().find(|(_, f, _)| *f == want) {
                Some(x) => *x,
                None => return 2,
            }
        }
    };
    let target = render::Target::of(&p, fmt, size);
    let mut text = folio_kit::text::Text::bundled();
    let titles = folio_reel_engine::titles::write_all(&p, target.width, target.height, &media::cache_dir(), &mut text);
    let cmd = match render::export_command_with_titles(&p, &target, Path::new(out), &titles) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e.0);
            return 1;
        }
    };
    let mut shown = -1i64;
    let r = media::export(&cmd.args, cmd.duration, |secs| {
        let pct = (secs / cmd.duration.max(0.001) * 100.0) as i64;
        if pct / 10 > shown / 10 {
            shown = pct;
            println!("  {}%", pct.min(100));
        }
    });
    match r {
        Ok(()) => {
            println!("  --export wrote {out}");
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// One frame of a film as a picture.
fn frame(out: &str, input: &str, at: f64) -> i32 {
    if !media::have() {
        eprintln!("{}", media::missing_message());
        return 1;
    }
    let p = open_or_exit(input);
    let seen = render::visible_at(&p, at);
    let found = seen.iter().rev().find(|(_, id)| p.clip(*id).map(|c| !c.kind.is_text()).unwrap_or(false)).copied();
    let Some((_, id)) = found else {
        eprintln!("There's nothing with a picture on the timeline at {at:.2}s.");
        return 1;
    };
    let Some(c) = p.clip(id) else { return 1 };
    let Some(path) = p.media_by_uri(&c.media).and_then(|m| m.path()) else {
        eprintln!("That clip has no picture to save.");
        return 1;
    };
    let f = match media::frame(&path, render::media_time(c, at), p.data.width) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let Some(pm) = folio_kit::gfx::sk::Pixmap::from_vec(f.rgba, folio_kit::gfx::sk::IntSize::from_wh(f.w, f.h).unwrap()) else {
        return 1;
    };
    match std::fs::write(out, folio_kit::gfx::encode_png(&pm)) {
        Ok(()) => {
            println!("  --frame wrote {out} ({}x{})", f.w, f.h);
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

fn info(input: &str) -> i32 {
    let p = open_or_exit(input);
    println!("  {} x {} at {:.0} a second", p.data.width, p.data.height, p.data.fps);
    println!("  {} long", pr::fmt_time(p.duration(), Some(p.data.fps)));
    for (i, name) in pr::TRACKS.iter().enumerate() {
        let clips = p.track(i);
        if clips.is_empty() {
            continue;
        }
        println!("  {}: {} clips", pr::track_name(name), clips.len());
        for c in clips {
            println!("    {:>8}  {:<24} {}", pr::fmt_time(c.start, None), c.name, pr::fmt_time(c.duration, None));
        }
    }
    for m in p.markers() {
        println!("  marker {} at {}", m.name, pr::fmt_time(m.t, None));
    }
    let missing = p.missing_media();
    if !missing.is_empty() {
        println!("  missing: {}", missing.iter().map(|m| m.name.clone()).collect::<Vec<_>>().join(", "));
    }
    0
}

fn probe(path: &str) -> i32 {
    if !media::have() {
        eprintln!("{}", media::missing_message());
        return 1;
    }
    match media::probe(Path::new(path)) {
        Ok(i) => {
            println!(
                "  {} — {:.2}s, {}x{} at {:.2} a second, picture {}, sound {}{}",
                path,
                i.duration,
                i.width,
                i.height,
                i.fps,
                if i.has_video { &i.video_codec } else { "none" },
                if i.has_audio { &i.audio_codec } else { "none" },
                if i.is_image() { " (a still)" } else { "" }
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file: Option<PathBuf> = None;
    let mut ui_test: Option<PathBuf> = None;
    let mut headless = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--version" | "-v" => {
                println!("Folio Reel {VERSION}");
                return;
            }
            "--help" | "-h" => {
                usage();
                return;
            }
            "--presets" => {
                for (name, fmt, size) in pr::EXPORT_PRESETS {
                    println!("  {name}  ({fmt}, {size})");
                }
                return;
            }
            "--export" => {
                let (Some(out), Some(inp)) = (args.get(i + 1), args.get(i + 2)) else {
                    usage();
                    std::process::exit(2);
                };
                let preset = args.iter().position(|a| a == "--preset").and_then(|j| args.get(j + 1)).cloned().unwrap_or_default();
                std::process::exit(export(out, inp, &preset));
            }
            "--frame" => {
                let (Some(out), Some(inp)) = (args.get(i + 1), args.get(i + 2)) else {
                    usage();
                    std::process::exit(2);
                };
                let at = args.get(i + 3).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                std::process::exit(frame(out, inp, at));
            }
            "--info" => {
                let Some(inp) = args.get(i + 1) else {
                    usage();
                    std::process::exit(2);
                };
                std::process::exit(info(inp));
            }
            "--probe" => {
                let Some(p) = args.get(i + 1) else {
                    usage();
                    std::process::exit(2);
                };
                std::process::exit(probe(p));
            }
            "--write-icon" => {
                let (Some(out), Some(size)) = (args.get(i + 1), args.get(i + 2).and_then(|s| s.parse::<u32>().ok())) else {
                    std::process::exit(2)
                };
                let pm = folio_kit::icons::render_svg(icons::app_icon_svg(), size, size).expect("icon");
                std::fs::write(out, folio_kit::gfx::encode_png(&pm)).expect("write icon");
                return;
            }
            "--write-icon-svg" => {
                let Some(out) = args.get(i + 1) else { std::process::exit(2) };
                std::fs::write(out, icons::app_icon_svg()).expect("write icon");
                return;
            }
            "--ui-test" => {
                let next = args.get(i + 1).filter(|a| !a.starts_with("--"));
                ui_test = Some(next.map(PathBuf::from).unwrap_or_else(|| folio_kit::platform::temp_dir().join("folio-reel-ui")));
                if next.is_some() {
                    i += 1;
                }
            }
            "--headless" => headless = true,
            a if !a.starts_with("--") => file = Some(PathBuf::from(a)),
            _ => {}
        }
        i += 1;
    }
    let (project, err) = match &file {
        Some(p) => match pr::open(p) {
            Ok(x) => (x, None),
            Err(e) => (Project::new(), Some(e.0)),
        },
        None => (Project::new(), None),
    };
    let mut a = app::ReelApp::new(project);
    if let Some(e) = err {
        a.say("Can't open the film", &e);
    }
    if let Some(d) = ui_test {
        folio_kit::platform::set_setting("FOLIO_UI_DIR", &d);
        if folio_kit::platform::setting("FOLIO_CONFIG_DIR").is_none() {
            folio_kit::platform::set_setting("FOLIO_CONFIG_DIR", &d.join("config"));
        }
        a.test = Some(uitest::UiTest::new(d));
    }
    if headless {
        let mut frame = Vec::new();
        let code = folio_kit::platform::run_headless(&mut a, 1280, 860, 1.0, 180_000, &mut frame).unwrap_or(3);
        std::process::exit(code);
    }
    let icon = folio_kit::icons::render_svg(icons::app_icon_svg(), 64, 64)
        .map(|pm| Image { w: 64, h: 64, rgba: folio_kit::gfx::rgba_from_pixmap(&pm) });
    let code = run(a, WindowSpec { title: "Folio Reel".into(), app_id: "folio-reel".into(), width: 1280, height: 860, icon });
    std::process::exit(code);
}
