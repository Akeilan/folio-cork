//! Draw a small model to a PNG: cargo run -p folio-gable-engine --example shot -- OUT.png
use folio_gable_engine::draw::{render, Style};
use folio_gable_engine::math::{v3, V3};
use folio_gable_engine::model::Model;
use folio_gable_engine::{solids, view::Camera};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/gable.png".into());
    let mut m = Model::new();
    // a house: a box with a roof pulled out of a gable
    solids::add_box(&mut m.ents, V3::default(), v3(4000.0, 6000.0, 2400.0));
    let gable = solids::add_polygon(
        &mut m.ents,
        &[v3(0.0, 0.0, 2400.0), v3(4000.0, 0.0, 2400.0), v3(2000.0, 0.0, 4200.0)],
    );
    if let Some(g) = gable {
        solids::push_pull(&mut m.ents, g, -6000.0);
    }
    let mut cam = Camera::default();
    let (lo, hi) = m.bounds().unwrap();
    cam.look_at_box(lo, hi);
    let (w, h) = (1000u32, 700u32);
    let proj = cam.matrix(w as f64, h as f64);
    let frame = render(&m, &proj, &Style::default(), w, h, &[]);
    let pm = frame.to_pixmap().unwrap();
    std::fs::write(&out, folio_kit::gfx::encode_png(&pm)).unwrap();
    println!("wrote {out}: {} faces, {} edges", m.ents.faces.iter().filter(|f| f.alive).count(), m.ents.edges.iter().filter(|e| e.alive).count());
}
