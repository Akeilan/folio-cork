//! Folio Graze: a simple, private web browser.

mod app;
mod icons;
mod nav;
mod uitest;

#[cfg(test)]
mod tests;

use folio_kit::platform::{Image, WebPolicy, WindowSpec};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut private = false;
    let mut ui_test: Option<PathBuf> = None;
    let mut open = vec![];
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--version" | "-v" => {
                println!("folio-graze {}", app::VERSION);
                return;
            }
            "--help" | "-h" => {
                println!("Folio Graze {}\n\nUsage:\n  folio-graze [ADDRESS...]        open the addresses in tabs\n  folio-graze --private           a private window: nothing kept on this computer\n  folio-graze --ui-test DIR       run the window test\n  folio-graze --version", app::VERSION);
                return;
            }
            "--private" => private = true,
            "--write-icon" => {
                let (Some(out), Some(size)) = (args.get(i + 1), args.get(i + 2).and_then(|s| s.parse::<u32>().ok())) else { std::process::exit(2) };
                let pm = folio_kit::icons::render_svg(icons::app_icon_svg(), size, size).expect("icon");
                std::fs::write(out, folio_kit::gfx::encode_png(&pm)).expect("write icon");
                return;
            }
            "--write-icon-svg" => {
                std::fs::write(&args[i + 1], icons::app_icon_svg()).expect("write icon");
                return;
            }
            "--ui-test" => {
                let d = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| folio_kit::platform::temp_dir().join("folio-graze-ui").to_string_lossy().to_string());
                if args.get(i + 1).map(|a| !a.starts_with("--")).unwrap_or(false) {
                    i += 1;
                }
                ui_test = Some(PathBuf::from(d));
            }
            a if !a.starts_with("--") => open.push(a.to_string()),
            _ => {}
        }
        i += 1;
    }
    let mut a = app::GrazeApp::new(private, open);
    let mut data_dir = folio_kit::platform::config_dir("folio-graze");
    if let Some(d) = &ui_test {
        let _ = std::fs::create_dir_all(d);
        // Tests keep their cookies and cache to themselves.
        data_dir = d.join("profile");
        let _ = std::fs::remove_dir_all(&data_dir);
        a.test = Some(uitest::UiTest::new(d.clone()));
        a.testing = true;
    }
    let policy = WebPolicy { private, https_only: true, block_third_party_cookies: true, tracking_protection: true, data_dir };
    let icon = folio_kit::icons::render_svg(icons::app_icon_svg(), 64, 64).map(|pm| Image { w: 64, h: 64, rgba: folio_kit::gfx::rgba_from_pixmap(&pm) });
    let title = if private { "Folio Graze (private)" } else { "Folio Graze" };
    let code = folio_platform::run_web(a, WindowSpec { title: title.into(), app_id: "folio-graze".into(), width: 1280, height: 860, icon }, policy);
    std::process::exit(code);
}
