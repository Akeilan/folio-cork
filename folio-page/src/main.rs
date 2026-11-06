//! Folio Page on the portable base (W3-12): the word processor, with the
//! document engine, the renderer and the editor beside it.

mod app;
mod icons;
mod ribbon_defs;
mod uitest;

use folio_kit::platform::{run, Image, WindowSpec};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!(
        "Usage: folio-page [FILE]\n       folio-page --pdf IN.docx OUT.pdf\n       folio-page --ui-test [DIR] [--headless]\n       folio-page --version"
    );
}

/// Where the blank documents live: beside the program, or where it was installed.
pub fn template_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = folio_kit::platform::setting("FOLIO_PAGE_TEMPLATES") {
        v.push(PathBuf::from(d));
    }
    for d in folio_kit::platform::data_dirs("folio-page") {
        v.push(d.join("templates"));
    }
    v.retain(|d| d.is_dir());
    v
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
                println!("Folio Page {VERSION}");
                return;
            }
            "--help" | "-h" => {
                usage();
                return;
            }
            "--pdf" => {
                let (Some(a), Some(b)) = (args.get(i + 1), args.get(i + 2)) else {
                    usage();
                    std::process::exit(2);
                };
                std::process::exit(to_pdf(a, b));
            }
            "--write-icon" => {
                let (Some(out), Some(size)) = (args.get(i + 1), args.get(i + 2).and_then(|s| s.parse::<u32>().ok()))
                else {
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
                ui_test = Some(
                    next.map(PathBuf::from)
                        .unwrap_or_else(|| folio_kit::platform::temp_dir().join("folio-page-ui")),
                );
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
    let (doc, path, err) = match &file {
        Some(p) => match folio_page_engine::document::Document::open_path(&p.to_string_lossy()) {
            Ok(d) => (d, Some(p.clone()), None),
            Err(e) => (app::PageApp::blank(), None, Some(e.0)),
        },
        None => (app::PageApp::blank(), None, None),
    };
    let mut a = app::PageApp::new(doc, path);
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
    let code = run(
        a,
        WindowSpec { title: "Folio Page".into(), app_id: "folio-page".into(), width: 1280, height: 860, icon },
    );
    std::process::exit(code);
}

/// `--pdf`: a document written straight to PDF, for printing from a script.
fn to_pdf(input: &str, out: &str) -> i32 {
    let input = input.to_string();
    let out = out.to_string();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let mut fonts = folio_page_engine::fonts::FontCache::new();
            let mut doc = match folio_page_engine::document::Document::open_path(&input) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{}", e.0);
                    return 1;
                }
            };
            let layout = folio_page_engine::paginate::layout_document(&mut doc, &mut fonts);
            match folio_page_render::export_pdf(&layout, &mut fonts, &out) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            }
        })
        .unwrap()
        .join()
        .unwrap_or(1)
}
