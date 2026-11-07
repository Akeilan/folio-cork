fn main() {
    let mut b = folio_sheet_engine::book::Book::open(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/Blank.xlsx")).unwrap();
    b.set_input(0, 0, 0, "Quarter").unwrap();
    b.set_input(0, 0, 1, "Sales").unwrap();
    b.set_input(0, 1, 0, "Q1").unwrap();
    b.set_input(0, 1, 1, "1200").unwrap();
    b.set_input(0, 2, 0, "Q2").unwrap();
    b.set_input(0, 2, 1, "1890.5").unwrap();
    b.set_input(0, 3, 0, "Total").unwrap();
    b.set_input(0, 3, 1, "=SUM(B2:B3)").unwrap();
    b.recalc(true);
    let mut p = folio_sheet_engine::view::ViewParams::default();
    p.width = 700.0;
    p.height = 320.0;
    folio_sheet_engine::render::render_png(&b, 0, &p, "/tmp/sheet-shot.png").unwrap();
}
