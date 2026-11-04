//! Folio Cork: diagrams, whiteboards and mood boards.

mod app;
mod bench;
mod collab;
mod commands;
mod editor;
mod export;
mod files;
mod geom;
mod icons;
mod layout;
mod model;
mod mood;
mod render;
mod ribbon_defs;
mod route;
mod settings;
mod stencils;
mod templates;
mod uitest;
mod vsdx;

#[cfg(test)]
mod tests;

use folio_kit::platform::{run, Image, WindowSpec};
use std::path::PathBuf;

fn usage() {
    println!(
        "Folio Cork {}\n\nUsage:\n  folio-cork [FILE.vsdx]          open a drawing\n  folio-cork --export IN OUT      export every page of IN to OUT (.png, .svg or .pdf)\n  folio-cork --ui-test [DIR]      run the window test, saving screenshots in DIR\n  folio-cork --ui-test DIR --headless   the same test with no window or display\n  folio-cork --bench              measure against the Raspberry Pi 4B budgets (X3-12)\n  folio-cork --version",
        app::VERSION
    );
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
                println!("folio-cork {}", app::VERSION);
                return;
            }
            "--help" | "-h" => {
                usage();
                return;
            }
            "--write-icon" => {
                // Packaging: the app icon as a PNG of the given size.
                let (Some(out), Some(size)) = (args.get(i + 1), args.get(i + 2).and_then(|s| s.parse::<u32>().ok())) else { std::process::exit(2) };
                let pm = folio_kit::icons::render_svg(icons::app_icon_svg(), size, size).expect("icon");
                std::fs::write(out, folio_kit::gfx::encode_png(&pm)).expect("write icon");
                return;
            }
            "--write-icon-svg" => {
                std::fs::write(&args[i + 1], icons::app_icon_svg()).expect("write icon");
                return;
            }
            "--export" => {
                let (Some(inp), Some(out)) = (args.get(i + 1), args.get(i + 2)) else {
                    usage();
                    std::process::exit(2);
                };
                std::process::exit(match export_cli(inp, out) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{e}");
                        1
                    }
                });
            }
            "--ui-test" => {
                let dir = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| folio_kit::platform::temp_dir().join("folio-cork-ui").to_string_lossy().to_string());
                if args.get(i + 1).map(|a| !a.starts_with("--")).unwrap_or(false) {
                    i += 1;
                }
                ui_test = Some(PathBuf::from(dir));
            }
            "--headless" => headless = true,
            "--bench" => std::process::exit(bench::run()),
            a if !a.starts_with("--") => file = Some(PathBuf::from(a)),
            _ => {}
        }
        i += 1;
    }
    if let Some(d) = &ui_test {
        folio_kit::platform::set_setting("FOLIO_UI_DIR", d);
        // Tests never touch the user's settings.
        if folio_kit::platform::setting("FOLIO_CONFIG_DIR").is_none() {
            folio_kit::platform::set_setting("FOLIO_CONFIG_DIR", &d.join("config"));
        }
    }
    let (doc, path, err) = match &file {
        Some(p) => match files::load(p) {
            Ok(d) => (d, if p.extension().map(|e| e == "vsdx").unwrap_or(false) { Some(p.clone()) } else { None }, None),
            Err(e) => (model::Document::default(), None, Some(e)),
        },
        None => (model::Document::default(), None, None),
    };
    let mut a = app::CorkApp::new(doc, path.clone());
    if let Some(p) = &path {
        a.settings.add_recent(p);
    }
    if let Some(e) = err {
        a.dialog = Some(folio_kit::ui::Dialog::new("info", "Can't open the file").note(&e).button("ok", "Close"));
    }
    if let Some(d) = ui_test {
        a.settings = settings::Settings::default();
        a.test = Some(uitest::UiTest::new(d));
    }
    if headless {
        // No window at all (X3-14): the same app, painted into memory on a virtual clock.
        let mut frame = Vec::new();
        let code = folio_kit::platform::run_headless(&mut a, 1280, 800, 1.0, 20_000, &mut frame).unwrap_or(3);
        std::process::exit(code);
    }
    let icon = folio_kit::icons::render_svg(icons::app_icon_svg(), 64, 64).map(|pm| Image { w: 64, h: 64, rgba: folio_kit::gfx::rgba_from_pixmap(&pm) });
    let code = run(a, WindowSpec { title: "Folio Cork".into(), app_id: "folio-cork".into(), width: 1280, height: 800, icon });
    std::process::exit(code);
}

fn export_cli(inp: &str, out: &str) -> Result<(), String> {
    let doc = files::load(std::path::Path::new(inp))?;
    let mut text = folio_kit::text::Text::new();
    let mut images = render::Images::default();
    let pages: Vec<usize> = (0..doc.pages.len()).collect();
    let out = std::path::Path::new(out);
    match out.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
        Some("png") => export::png(&doc, &pages, out, &mut text, &mut images, 2.0),
        Some("svg") => export::svg(&doc, &pages, out, &mut text),
        Some("pdf") => export::pdf(&doc, &pages, out, &mut text, &mut images, true, false),
        Some("vsdx") => files::save(&doc, out),
        _ => Err("Export to .png, .svg, .pdf or .vsdx.".into()),
    }
}
