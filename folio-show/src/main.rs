//! Folio Show on the portable base (S3-19): the spreadsheet, with its own engine beside it.

mod app;
mod icons;
mod settings;
mod ribbon_defs;
mod uitest;

use folio_kit::platform::{run, Image, WindowSpec};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!("Usage: folio-show [FILE]\n       folio-show --png IN.pptx OUT.png\n       folio-show --ui-test [DIR] [--headless]\n       folio-show --version");
}

/// Where the blank presentations live: beside the program, or where it was installed.
pub fn template_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = folio_kit::platform::setting("FOLIO_SHOW_TEMPLATES") {
        v.push(PathBuf::from(d));
    }
    for d in folio_kit::platform::data_dirs("folio-show") {
        v.push(d.join("templates"));
    }
    v.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/templates")));
    v.retain(|d| d.is_dir());
    v
}

/// Draw the first sheet of a presentation into a picture, without a window.
fn to_png(input: &str, out: &str) -> i32 {
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let deck = match folio_show_engine::model::Deck::open(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let mut a = app::ShowApp::new(deck, None);
    a.export_png(std::path::Path::new(out));
    println!("  --png wrote {out}");
    0
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
                println!("Folio Show {VERSION}");
                return;
            }
            "--help" | "-h" => {
                usage();
                return;
            }
            "--png" => {
                let (Some(a), Some(b)) = (args.get(i + 1), args.get(i + 2)) else {
                    usage();
                    std::process::exit(2);
                };
                std::process::exit(to_png(a, b));
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
                ui_test = Some(next.map(PathBuf::from).unwrap_or_else(|| folio_kit::platform::temp_dir().join("folio-show-ui")));
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
    let (deck, path, err) = match &file {
        Some(p) => match std::fs::read(p).map_err(|e| e.to_string()).and_then(|b| folio_show_engine::model::Deck::open(&b)) {
            Ok(d) => (d, Some(p.clone()), None),
            Err(e) => (app::ShowApp::blank(), None, Some(e)),
        },
        None => (app::ShowApp::blank(), None, None),
    };
    let mut a = app::ShowApp::new(deck, path);
    if let Some(e) = err {
        a.say("Can't open the file", &e);
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
        let code = folio_kit::platform::run_headless(&mut a, 1280, 860, 1.0, 20_000, &mut frame).unwrap_or(3);
        std::process::exit(code);
    }
    let icon = folio_kit::icons::render_svg(icons::app_icon_svg(), 64, 64)
        .map(|pm| Image { w: 64, h: 64, rgba: folio_kit::gfx::rgba_from_pixmap(&pm) });
    let code = run(a, WindowSpec { title: "Folio Show".into(), app_id: "folio-show".into(), width: 1280, height: 860, icon });
    std::process::exit(code);
}
