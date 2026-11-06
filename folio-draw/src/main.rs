//! Folio Draw on the portable base (D3-06). Until it matches the GTK version's tests, that one
//! keeps shipping; this one is built and tested alongside it.

mod app;
mod emb;
mod icons;
mod letters;
mod page;
mod ribbon_defs;
mod settings;
mod stabilizer;
mod uitest;

use folio_kit::platform::{run, Image, WindowSpec};
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage() {
    println!(
        "Usage: folio-draw [FILE]\n       folio-draw --ui-test [DIR] [--headless]\n       folio-draw --convert IN OUT\n{EMB_HELP}       folio-draw --version"
    );
}

const EMB_HELP: &str = "       folio-draw --embroider PICTURE -o DESIGN.pes [--size 4x4] [--colours N]\n\
                        \x20                              [--threads brother|exact] [--crop]\n\
                        \x20        the output format follows the extension: .pes .dst .jef .exp .vp3 .xxx .pec\n\
                        \x20        --size     a hoop (4x4, 5x7, 6x10, 8x8, 8x12) or millimetres (100x100mm)\n\
                        \x20        --colours  exactly this many colours (the default works it out)\n\
                        \x20        --threads  brother (the 63-colour chart, the default) or exact\n\
                        \x20        --crop     trim the picture's border and fill the hoop\n";

/// `--embroider`: a picture (or an embroidery document) turned into a machine file.
fn embroider(args: &[String]) -> i32 {
    let (mut src, mut out, mut size, mut colours, mut threads, mut crop) = (None, None, "4x4".to_string(), 0u64, "brother".to_string(), false);
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let next = || args.get(i + 1).cloned();
        match a {
            "-o" | "--output" => match next() {
                Some(v) => {
                    out = Some(v);
                    i += 1;
                }
                None => return bad("--output needs a file name"),
            },
            "--size" => match next() {
                Some(v) => {
                    size = v;
                    i += 1;
                }
                None => return bad("--size needs a hoop"),
            },
            "--colours" | "--colors" => match next().and_then(|v| v.parse::<u64>().ok()) {
                Some(v) => {
                    colours = v.clamp(1, 30);
                    i += 1;
                }
                None => return bad("--colours needs a number"),
            },
            "--threads" => match next() {
                Some(v) if v == "brother" || v == "exact" => {
                    threads = v;
                    i += 1;
                }
                _ => return bad("--threads is brother or exact"),
            },
            "--crop" => crop = true,
            "-h" | "--help" => {
                println!("{EMB_HELP}");
                return 0;
            }
            v if src.is_none() && !v.starts_with('-') => src = Some(v.to_string()),
            v => return bad(&format!("unexpected argument {v}")),
        }
        i += 1;
    }
    let (Some(src), Some(out)) = (src, out) else { return bad("a picture and -o DESIGN are needed") };
    let Some((hoop, _, _)) = emb::hoop_from(&size) else { return bad(&format!("no hoop fits {size}")) };
    let mut doc = match app::DrawApp::load(std::path::Path::new(&src)) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("folio-draw: can't open {src}: {e}");
            return 1;
        }
    };
    // an embroidery document is planned as it is; a picture is converted first
    let plan = if emb::settings(&doc).is_some() {
        match emb::make_plan(&doc) {
            Some(p) => p,
            None => {
                eprintln!("folio-draw: there are no stitches in {src}");
                return 1;
            }
        }
    } else {
        match emb::design_from_picture(&mut doc, hoop, colours, &threads, crop) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("folio-draw: {e}");
                return 1;
            }
        }
    };
    let ext = out.rsplit('.').next().unwrap_or("pes").to_lowercase();
    let stem: String = std::path::Path::new(&out).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Untitled".into());
    let bytes = match emb::write(&plan, &ext, &stem.chars().take(16).collect::<String>(), "20261106") {
        Ok(b) => b,
        Err(e) => {
            eprintln!("folio-draw: {e}");
            return 1;
        }
    };
    if let Err(e) = folio_kit::platform::atomic_write(std::path::Path::new(&out), &bytes) {
        eprintln!("folio-draw: {e}");
        return 1;
    }
    let (x0, y0, x1, y1) = plan.bounds();
    let threads: Vec<String> = plan.blocks.iter().map(|b| if b.thread.name.is_empty() { format!("#{:02X}{:02X}{:02X}", b.thread.rgb[0], b.thread.rgb[1], b.thread.rgb[2]) } else { b.thread.name.clone() }).collect();
    println!(
        "{out}: {} stitches, {} colours ({}), {:.1} x {:.1} mm",
        plan.stitch_count(),
        plan.blocks.len(),
        threads.join(", "),
        x1 - x0,
        y1 - y0
    );
    0
}

fn bad(msg: &str) -> i32 {
    eprintln!("folio-draw: {msg}\n\n{EMB_HELP}");
    2
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
            "--embroider" => std::process::exit(embroider(&args[i + 1..])),
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
