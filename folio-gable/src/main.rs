//! Folio Gable on the portable base (G3-06): the 3D modeller, with its own engine beside it.

mod app;
mod icons;
mod settings;
mod uitest;

use folio_gable_engine::model::Model;
use folio_kit::platform::{run, Image, WindowSpec};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!("Usage: folio-gable [FILE]\n       folio-gable --png IN.gable OUT.png\n       folio-gable --export OUT.obj|OUT.stl IN.gable\n       folio-gable --ui-test [DIR] [--headless]\n       folio-gable --version");
}

/// Draw a drawing to a picture, without opening a window.
fn to_png(input: &str, out: &str) -> i32 {
    let model = match folio_gable_engine::store::open(std::path::Path::new(input)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let mut cam = folio_gable_engine::view::Camera::default();
    if let Some((lo, hi)) = model.bounds() {
        cam.look_at_box(lo, hi);
    }
    let (w, h) = (1200u32, 800u32);
    let proj = cam.matrix(w as f64, h as f64);
    let frame = folio_gable_engine::draw::render(&model, &proj, &folio_gable_engine::draw::Style::default(), w, h, &[]);
    let Some(pm) = frame.to_pixmap() else { return 1 };
    match std::fs::write(out, folio_kit::gfx::encode_png(&pm)) {
        Ok(()) => {
            println!("  --png wrote {out}");
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
                println!("Folio Gable {VERSION}");
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
            "--export" => {
                let (Some(out), Some(inp)) = (args.get(i + 1), args.get(i + 2)) else {
                    usage();
                    std::process::exit(2);
                };
                let m = match folio_gable_engine::store::open(std::path::Path::new(inp)) {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                };
                let p = PathBuf::from(out);
                let r = if p.extension().map(|e| e.eq_ignore_ascii_case("stl")).unwrap_or(false) {
                    folio_gable_engine::store::export_stl(&m, &p)
                } else {
                    folio_gable_engine::store::export_obj(&m, &p)
                };
                match r {
                    Ok(()) => println!("  --export wrote {out}"),
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1);
                    }
                }
                return;
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
                ui_test = Some(next.map(PathBuf::from).unwrap_or_else(|| folio_kit::platform::temp_dir().join("folio-gable-ui")));
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
    let (model, path, err) = match &file {
        Some(p) => match folio_gable_engine::store::open(p) {
            Ok(m) => (m, Some(p.clone()), None),
            Err(e) => (Model::new(), None, Some(e)),
        },
        None => (Model::new(), None, None),
    };
    let mut a = app::GableApp::new(model, path);
    if let Some(e) = err {
        a.say("Can't open the drawing", &e);
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
        let code = folio_kit::platform::run_headless(&mut a, 1280, 820, 1.0, 20_000, &mut frame).unwrap_or(3);
        std::process::exit(code);
    }
    let icon = folio_kit::icons::render_svg(icons::app_icon_svg(), 64, 64)
        .map(|pm| Image { w: 64, h: 64, rgba: folio_kit::gfx::rgba_from_pixmap(&pm) });
    let code = run(a, WindowSpec { title: "Folio Gable".into(), app_id: "folio-gable".into(), width: 1280, height: 820, icon });
    std::process::exit(code);
}
