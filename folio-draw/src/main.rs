//! Folio Draw on the portable base (D3-06). Until it matches the GTK version's tests, that one
//! keeps shipping; this one is built and tested alongside it.

mod app;
mod emb;
mod icons;
mod letters;
mod ribbon_defs;
mod settings;
mod stabilizer;
mod uitest;

use folio_kit::platform::{run, Image, WindowSpec};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!("Usage: folio-draw [FILE]\n       folio-draw --ui-test [DIR] [--headless]\n       folio-draw --convert IN OUT\n       folio-draw --version");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file = None;
    let mut ui_test: Option<PathBuf> = None;
    let mut headless = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--version" | "-v" => {
                println!("Folio Draw {VERSION}");
                return;
            }
            "--help" | "-h" => {
                usage();
                return;
            }
            "--convert" => {
                let (Some(a), Some(b)) = (args.get(i + 1), args.get(i + 2)) else {
                    usage();
                    std::process::exit(2);
                };
                let r = app::DrawApp::load(std::path::Path::new(a)).and_then(|mut d| {
                    let f = folio_draw_engine::files::Format::from_name(b);
                    let bytes = folio_draw_engine::files::save(&mut d, f, [255, 255, 255]);
                    folio_kit::platform::atomic_write(std::path::Path::new(b), &bytes)
                });
                if let Err(e) = r {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
                return;
            }
            "--ui-test" => {
                let next = args.get(i + 1).filter(|a| !a.starts_with("--"));
                ui_test = Some(next.map(PathBuf::from).unwrap_or_else(|| folio_kit::platform::temp_dir().join("folio-draw-ui")));
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
        Some(p) => match app::DrawApp::load(p) {
            Ok(d) => (d, Some(p.clone()), None),
            Err(e) => (folio_draw_engine::doc::Doc::new(1152, 648, [255, 255, 255, 255]), None, Some(e)),
        },
        None => (folio_draw_engine::doc::Doc::new(1152, 648, [255, 255, 255, 255]), None, None),
    };
    let mut a = app::DrawApp::new(doc, path);
    if let Some(e) = err {
        a.dialog = Some(folio_kit::ui::Dialog::new("info", "Can't open the file").note(&e).button("ok", "Close"));
    }
    if let Some(d) = ui_test {
        folio_kit::platform::set_setting("FOLIO_UI_DIR", &d);
        // tests never touch the user's settings
        if folio_kit::platform::setting("FOLIO_CONFIG_DIR").is_none() {
            folio_kit::platform::set_setting("FOLIO_CONFIG_DIR", &d.join("config"));
        }
        a.settings = settings::Settings::default();
        a.test = Some(uitest::UiTest::new(d));
    }
    if headless {
        let mut frame = Vec::new();
        let code = folio_kit::platform::run_headless(&mut a, 1280, 800, 1.0, 20_000, &mut frame).unwrap_or(3);
        std::process::exit(code);
    }
    let icon = folio_kit::icons::render_svg(icons::app_icon_svg(), 64, 64).map(|pm| Image { w: 64, h: 64, rgba: folio_kit::gfx::rgba_from_pixmap(&pm) });
    let code = run(a, WindowSpec { title: "Folio Draw".into(), app_id: "folio-draw".into(), width: 1280, height: 800, icon });
    std::process::exit(code);
}
