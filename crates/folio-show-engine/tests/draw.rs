//! The engine opens a deck and draws a slide on the portable canvas.
use folio_canvas::{Cx, Glyphs};
use folio_kit::gfx::sk;
use folio_show_engine::model::Deck;
use folio_show_engine::render::{Opts, Painter};

fn deck_bytes() -> Vec<u8> {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/sample.pptx");
    std::fs::read(p).expect("the sample deck")
}

#[test]
fn opens_a_deck_and_draws_a_slide() {
    let bytes = deck_bytes();
    let mut deck = Deck::open(&bytes).expect("open");
    deck.read_slides();
    assert!(deck.slides.len() >= 2, "the deck has slides");
    let part = deck.slides[0].clone();
    let (w, h) = deck.page_size(&part);
    assert!(w > 100.0 && h > 100.0, "the page has a size: {w} x {h}");
    let mut pm = sk::Pixmap::new(w as u32, h as u32).unwrap();
    pm.fill(sk::Color::WHITE);
    let mut glyphs = Glyphs::new();
    let mut p = Painter::new();
    let shapes = {
        let mut cx = Cx::new(&mut pm, &mut glyphs);
        p.draw(&mut deck, &part, &mut cx, &Opts::default())
    };
    assert!(!shapes.is_empty(), "the slide has shapes");
    // the title's words left ink on the page
    let ink = pm.pixels().iter().filter(|p| p.red() < 200 || p.green() < 200 || p.blue() < 200).count();
    assert!(ink > 200, "the slide was drawn: {ink} marked pixels");
}

#[test]
fn a_shape_lands_where_the_model_says() {
    let bytes = deck_bytes();
    let mut deck = Deck::open(&bytes).expect("open");
    deck.read_slides();
    let part = deck.slides[2].clone();
    let (w, h) = deck.page_size(&part);
    let mut pm = sk::Pixmap::new(w as u32, h as u32).unwrap();
    pm.fill(sk::Color::WHITE);
    let mut glyphs = Glyphs::new();
    let mut p = Painter::new();
    let shapes = {
        let mut cx = Cx::new(&mut pm, &mut glyphs);
        p.draw(&mut deck, &part, &mut cx, &Opts::default())
    };
    let oval = shapes.iter().find(|s| s.name.starts_with("Oval")).expect("the oval");
    // its middle is orange, and a point outside it is still white
    let at = |x: f64, y: f64| {
        let p = pm.pixel(x as u32, y as u32).unwrap();
        (p.red(), p.green(), p.blue())
    };
    let (r, g, b) = at(oval.x + oval.w / 2.0, oval.y + oval.h / 2.0);
    assert!(r > 200 && g > 100 && g < 190 && b < 60, "the oval is orange: {r},{g},{b}");
    let (r2, g2, b2) = at(oval.x + oval.w + 20.0, oval.y);
    assert!(r2 > 240 && g2 > 240 && b2 > 240, "beside it is white: {r2},{g2},{b2}");
}
