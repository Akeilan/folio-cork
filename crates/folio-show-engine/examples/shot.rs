//! Draw a slide to a PNG: cargo run -p folio-show-engine --example shot -- IN.pptx OUT.png [N]
use folio_canvas::{Cx, Glyphs};
use folio_kit::gfx::sk;
use folio_show_engine::model::Deck;
use folio_show_engine::render::{Opts, Painter};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (inp, out) = (a[1].clone(), a[2].clone());
    let n: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let bytes = std::fs::read(&inp).expect("the deck");
    let mut deck = Deck::open(&bytes).expect("open");
    deck.read_slides();
    let part = deck.slides[n].clone();
    let (w, h) = deck.page_size(&part);
    let scale = 2.0;
    let mut pm = sk::Pixmap::new((w * scale) as u32, (h * scale) as u32).unwrap();
    pm.fill(sk::Color::WHITE);
    let mut glyphs = Glyphs::new();
    let mut p = Painter::new();
    {
        let mut cx = Cx::new(&mut pm, &mut glyphs);
        cx.scale(scale, scale);
        let got = p.draw(&mut deck, &part, &mut cx, &Opts::default());
        for g in &got { println!("shape {} at {:.1},{:.1} {:.1}x{:.1}", g.name, g.x, g.y, g.w, g.h); }
    }
    std::fs::write(&out, folio_kit::gfx::encode_png(&pm)).unwrap();
    println!("wrote {out} ({} x {} points, {} slides)", w, h, deck.slides.len());
}
