//! X3-14: the toolkit drawn into memory and compared pixel for pixel with stored images, with
//! no display server. Bundled fonts only, so the result is the same on every machine.
//! `FOLIO_UPDATE_GOLDEN=1 cargo test` rewrites the stored images after a deliberate change.

use folio_kit::chrome::TitleBar;
use folio_kit::gfx::{self, sk, Color, Rect};
use folio_kit::icons::Icons;
use folio_kit::ribbon::{Group, Item, Ribbon, Tab};
use folio_kit::text::Text;
use folio_kit::theme::{app_colors, Theme};
use folio_kit::ui::{CommandState, Ctx, Dialog, Menu, MenuItem};
use std::path::PathBuf;
use std::time::Duration;

struct St;
impl CommandState for St {
    fn checked(&self, id: &str) -> bool {
        id == "bold"
    }
    fn enabled(&self, id: &str) -> bool {
        id != "paste"
    }
}

const SQUARE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect x="2" y="2" width="12" height="12" fill="none" stroke="currentColor"/><circle cx="8" cy="8" r="3" fill="var(--accent)"/></svg>"#;

fn frame(dark: bool) -> sk::Pixmap {
    let mut text = Text::bundled();
    let mut icons = Icons::new();
    for n in ["app", "save", "undo", "paste", "bold", "shape"] {
        icons.add(n, SQUARE);
    }
    let theme = Theme::new(dark, app_colors::CORK, Color::rgb(0xB8, 0x86, 0x0B));
    let (w, h) = (900u32, 420u32);
    let mut pm = sk::Pixmap::new(w, h).unwrap();
    pm.fill(theme.window.to_sk());
    let home = Tab::new(
        "home",
        "Home",
        "H",
        vec![
            Group::new("Clipboard", vec![Item::large("paste", "Paste", "paste").split(), Item::small("cut", "Cut", "shape"), Item::small("copy", "Copy", "shape")]),
            Group::new("Font", vec![Item::combo("font", "Font", 120), Item::icon("bold", "Bold", "bold")]),
            Group::new("Shapes", vec![Item::large("shape", "Shapes", "shape").menu()]),
        ],
    );
    let mut ribbon = Ribbon::new(vec![home, Tab::new("insert", "Insert", "N", vec![])]);
    let mut title = TitleBar::new("app", vec![("save", "Save", "save"), ("undo", "Undo", "undo")]);
    let mut cx = Ctx { pm: &mut pm, text: &mut text, icons: &mut icons, theme: &theme, s: 1.0 };
    title.paint(&mut cx, Rect::new(0.0, 0.0, w as f32, 32.0), "Drawing1 - Folio Cork", false, &|_| true);
    let rh = ribbon.height(1.0);
    ribbon.paint(&mut cx, Rect::new(0.0, 32.0, w as f32, rh), &St, Duration::ZERO);
    let mut menu = Menu::new(
        "m",
        Rect::new(20.0, 32.0 + rh, 10.0, 10.0),
        vec![MenuItem::new("a", "Bring to Front").key("Ctrl+Shift+]"), MenuItem::sep(), MenuItem::new("b", "Group").check(true), MenuItem::new("c", "Ungroup").off(true)],
    );
    menu.paint(&mut cx, Rect::new(0.0, 0.0, w as f32, h as f32));
    let mut dlg = Dialog::new("d", "Page Setup").text("name", "Name", "Page-1").check("grid", "Show the grid", true).button("ok", "OK").button("cancel", "Cancel");
    dlg.paint(&mut cx, Rect::new(300.0, 32.0 + rh, 600.0, h as f32 - 32.0 - rh));
    pm
}

fn compare(name: &str, pm: &sk::Pixmap) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let path = dir.join(format!("{name}.png"));
    let got = gfx::encode_png(pm);
    if std::env::var_os("FOLIO_UPDATE_GOLDEN").is_some() || !path.exists() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, &got).unwrap();
        return;
    }
    let want = gfx::decode_image(&std::fs::read(&path).unwrap()).expect("golden image");
    assert_eq!((want.width(), want.height()), (pm.width(), pm.height()), "{name}: size");
    let diff = want.data().chunks(4).zip(pm.data().chunks(4)).filter(|(a, b)| a != b).count();
    if diff != 0 {
        let out = std::env::temp_dir().join(format!("{name}-got.png"));
        std::fs::write(&out, &got).unwrap();
        panic!("{name}: {diff} pixels differ from {} (got {})", path.display(), out.display());
    }
}

#[test]
fn chrome_light_matches_golden() {
    compare("chrome-light", &frame(false));
}

#[test]
fn chrome_dark_matches_golden() {
    compare("chrome-dark", &frame(true));
}

#[test]
fn drawing_twice_gives_identical_pixels() {
    assert!(frame(false).data() == frame(false).data());
}
