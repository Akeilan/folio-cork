//! Folio Chord on the portable base (C3-39): the music player, with its own engine beside it.

mod app;
mod icons;
mod settings;
mod uitest;

use folio_chord_engine::library::Library;
use folio_kit::platform::{run, Image, WindowSpec};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!("Usage: folio-chord [FOLDER]\n       folio-chord --scan FOLDER\n       folio-chord --ui-test [DIR] [--headless]\n       folio-chord --version");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut folder: Option<PathBuf> = None;
    let mut ui_test: Option<PathBuf> = None;
    let mut headless = false;
    let mut scan_only: Option<PathBuf> = None;
    let mut lib_path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--version" | "-v" => {
                println!("Folio Chord {VERSION}");
                return;
            }
            "--help" | "-h" => {
                usage();
                return;
            }
            "--library" => {
                lib_path = args.get(i + 1).map(PathBuf::from);
                i += 1;
            }
            "--scan" => {
                scan_only = args.get(i + 1).map(PathBuf::from);
                i += 1;
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
                ui_test = Some(next.map(PathBuf::from).unwrap_or_else(|| folio_kit::platform::temp_dir().join("folio-chord-ui")));
                if next.is_some() {
                    i += 1;
                }
            }
            "--headless" => headless = true,
            a if !a.starts_with("--") => folder = Some(PathBuf::from(a)),
            _ => {}
        }
        i += 1;
    }
    if let Some(d) = &ui_test {
        folio_kit::platform::set_setting("FOLIO_UI_DIR", d);
        if folio_kit::platform::setting("FOLIO_CONFIG_DIR").is_none() {
            folio_kit::platform::set_setting("FOLIO_CONFIG_DIR", &d.join("config"));
        }
    }
    let path = lib_path.unwrap_or_else(Library::default_path);
    let mut lib = Library::open(&path);
    if let Some(d) = scan_only {
        lib.add_folder(&d);
        let (added, changed) = lib.rescan();
        if let Err(e) = lib.save() {
            eprintln!("{e}");
            std::process::exit(1);
        }
        println!("  --scan took in {added} new and {changed} changed songs ({} in the library)", lib.tracks.len());
        return;
    }
    let mut a = app::ChordApp::new(lib);
    if let Some(d) = folder {
        a.add_folder(&d);
    }
    if let Some(d) = ui_test {
        a.test = Some(uitest::UiTest::new(d));
    }
    if headless {
        let mut frame = Vec::new();
        let code = folio_kit::platform::run_headless(&mut a, 1280, 800, 1.0, 20_000, &mut frame).unwrap_or(3);
        std::process::exit(code);
    }
    let icon = folio_kit::icons::render_svg(icons::app_icon_svg(), 64, 64)
        .map(|pm| Image { w: 64, h: 64, rgba: folio_kit::gfx::rgba_from_pixmap(&pm) });
    let code = run(a, WindowSpec { title: "Folio Chord".into(), app_id: "folio-chord".into(), width: 1280, height: 800, icon });
    std::process::exit(code);
}
