//! The engine draws on the portable base (S3-19): a workbook rendered to a picture has the
//! gridlines, the text and a chart in it, with no cairo or pango anywhere.
use folio_sheet_engine::{book::Book, view::ViewParams};

fn blank() -> Book {
    Book::open(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/Blank.xlsx")).expect("Blank.xlsx")
}

fn render(b: &Book, p: &ViewParams) -> folio_kit::gfx::sk::Pixmap {
    let path = std::env::temp_dir().join(format!("folio-sheet-draw-{}.png", std::process::id()));
    folio_sheet_engine::render::render_png(b, 0, p, path.to_str().unwrap()).expect("render");
    let bytes = std::fs::read(&path).expect("png");
    let _ = std::fs::remove_file(&path);
    folio_kit::gfx::decode_image(&bytes).expect("decode")
}

fn ink(pm: &folio_kit::gfx::sk::Pixmap) -> usize {
    pm.pixels().iter().filter(|p| p.alpha() > 0 && (p.red() < 230 || p.green() < 230 || p.blue() < 230)).count()
}

#[test]
fn a_sheet_draws_its_grid_and_text() {
    let mut b = blank();
    let mut p = ViewParams::default();
    p.width = 600.0;
    p.height = 300.0;
    let empty = ink(&render(&b, &p));
    assert!(empty > 200, "an empty sheet still draws its gridlines and headings: {empty}");

    b.set_input(0, 0, 0, "Hello world").unwrap();
    b.set_input(0, 1, 0, "12345").unwrap();
    let typed = ink(&render(&b, &p));
    assert!(typed > empty + 100, "typing shows up on the picture: {empty} -> {typed}");
}
