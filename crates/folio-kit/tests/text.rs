use folio_kit::gfx::{sk, Color};
use folio_kit::text::{Text, TextStyle};

#[test]
fn ui_text_draws_and_outlines_point_down() {
    let mut t = Text::new();
    assert!(!t.ui_family().is_empty(), "no interface font found");
    let st = TextStyle { size: 20.0, color: Color::BLACK, ..Default::default() };
    let mut pm = sk::Pixmap::new(200, 60).unwrap();
    pm.fill(sk::Color::WHITE);
    t.draw(&mut pm, "Hg", &st, 10.0, 10.0, None);
    let dark = pm.data().chunks(4).filter(|p| p[0] < 100).count();
    assert!(dark > 30, "text not drawn ({dark})");
    let paths = t.outlines("Hg", &st, None);
    assert_eq!(paths.len(), 1);
    let b = paths[0].bounds();
    // Top of the H near the top of the line, descender of g below the baseline.
    assert!(b.top() > 0.0 && b.top() < 10.0, "top {}", b.top());
    assert!(b.bottom() > 18.0 && b.bottom() < 26.0, "bottom {}", b.bottom());
    let lay = t.layout("one two three four", &st, Some(60.0));
    assert!(lay.lines.len() >= 2, "wrapping");
    let (x, _, _) = lay.caret(4);
    assert!(x.abs() < 1.0 || lay.lines[0].end >= 4);
}
