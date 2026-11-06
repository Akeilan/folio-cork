//! The surface draws what it is told to, in the place it was told to.

use folio_canvas::{Cx, Glyphs, Source};
use folio_kit::gfx::sk;

fn blank(w: u32, h: u32) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(w, h).unwrap();
    pm.fill(sk::Color::WHITE);
    pm
}

fn px(pm: &sk::Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
    let p = pm.pixel(x, y).unwrap().demultiply();
    (p.red(), p.green(), p.blue())
}

fn ink(pm: &sk::Pixmap) -> usize {
    pm.pixels().iter().filter(|p| p.demultiply().red() < 200).count()
}

#[test]
fn a_filled_rectangle_lands_where_it_was_put() {
    let mut pm = blank(40, 40);
    let mut g = Glyphs::new();
    {
        let mut cx = Cx::new(&mut pm, &mut g);
        cx.set_rgb(0x0000FF);
        cx.rectangle(10.0, 10.0, 20.0, 20.0);
        cx.fill();
    }
    assert_eq!(px(&pm, 20, 20), (0, 0, 255), "inside");
    assert_eq!(px(&pm, 5, 5), (255, 255, 255), "outside");
    assert_eq!(px(&pm, 35, 20), (255, 255, 255), "past the right edge");
}

#[test]
fn the_transform_moves_and_magnifies() {
    let mut pm = blank(40, 40);
    let mut g = Glyphs::new();
    {
        let mut cx = Cx::new(&mut pm, &mut g);
        cx.translate(10.0, 10.0);
        cx.scale(2.0, 2.0);
        cx.set_rgb(0x000000);
        cx.rectangle(0.0, 0.0, 5.0, 5.0); // 10x10 at (10,10)
        cx.fill();
    }
    assert_eq!(px(&pm, 15, 15), (0, 0, 0));
    assert_eq!(px(&pm, 25, 15), (255, 255, 255));
}

#[test]
fn a_point_keeps_the_place_it_was_added_at() {
    // cairo's rule: a transform set after the point does not move it
    let mut pm = blank(40, 40);
    let mut g = Glyphs::new();
    {
        let mut cx = Cx::new(&mut pm, &mut g);
        cx.rectangle(5.0, 5.0, 10.0, 10.0);
        cx.translate(20.0, 20.0);
        cx.set_rgb(0x000000);
        cx.fill();
    }
    assert_eq!(px(&pm, 10, 10), (0, 0, 0));
    assert_eq!(px(&pm, 30, 30), (255, 255, 255));
}

#[test]
fn a_clip_narrows_what_is_painted() {
    let mut pm = blank(40, 40);
    let mut g = Glyphs::new();
    {
        let mut cx = Cx::new(&mut pm, &mut g);
        cx.rectangle(0.0, 0.0, 20.0, 40.0);
        cx.clip();
        cx.set_rgb(0x000000);
        cx.paint();
    }
    assert_eq!(px(&pm, 10, 20), (0, 0, 0));
    assert_eq!(px(&pm, 30, 20), (255, 255, 255));
}

#[test]
fn a_clip_is_put_back_by_restore() {
    let mut pm = blank(40, 40);
    let mut g = Glyphs::new();
    {
        let mut cx = Cx::new(&mut pm, &mut g);
        cx.save();
        cx.rectangle(0.0, 0.0, 10.0, 10.0);
        cx.clip();
        cx.restore();
        cx.set_rgb(0x000000);
        cx.rectangle(20.0, 20.0, 10.0, 10.0);
        cx.fill();
    }
    assert_eq!(px(&pm, 25, 25), (0, 0, 0));
}

#[test]
fn a_circle_covers_about_what_a_circle_should() {
    let mut pm = blank(100, 100);
    let mut g = Glyphs::new();
    {
        let mut cx = Cx::new(&mut pm, &mut g);
        cx.set_rgb(0x000000);
        cx.arc(50.0, 50.0, 30.0, 0.0, std::f64::consts::TAU);
        cx.close_path();
        cx.fill();
    }
    let area = ink(&pm) as f64;
    let want = std::f64::consts::PI * 30.0 * 30.0;
    assert!((area - want).abs() < want * 0.03, "filled {area}, a circle is {want}");
}

#[test]
fn a_dashed_line_has_gaps_in_it() {
    let mut solid = blank(60, 10);
    let mut dashed = blank(60, 10);
    let mut g = Glyphs::new();
    for (pm, dash) in [(&mut solid, false), (&mut dashed, true)] {
        let mut cx = Cx::new(pm, &mut g);
        cx.set_rgb(0x000000);
        cx.set_line_width(2.0);
        if dash {
            cx.set_dash(&[4.0, 4.0], 0.0);
        }
        cx.move_to(2.0, 5.0);
        cx.line_to(58.0, 5.0);
        cx.stroke();
    }
    assert!(ink(&dashed) > 0 && ink(&dashed) < ink(&solid) * 3 / 4, "dashes should leave gaps");
}

#[test]
fn a_gradient_runs_from_one_colour_to_the_other() {
    let mut pm = blank(60, 10);
    let mut g = Glyphs::new();
    {
        let mut cx = Cx::new(&mut pm, &mut g);
        cx.set_source(Source::Linear((0.0, 0.0), (60.0, 0.0), vec![(0.0, 0x000000, 1.0), (1.0, 0xFFFFFF, 1.0)]));
        cx.rectangle(0.0, 0.0, 60.0, 10.0);
        cx.fill();
    }
    let (l, m, r) = (px(&pm, 2, 5).0, px(&pm, 30, 5).0, px(&pm, 57, 5).0);
    assert!(l < m && m < r, "{l} {m} {r}");
}

#[test]
fn the_same_drawing_gives_the_same_pixels_every_time() {
    let run = || {
        let mut pm = blank(80, 80);
        let mut g = Glyphs::new();
        {
            let mut cx = Cx::new(&mut pm, &mut g);
            cx.set_rgb(0x336699);
            cx.arc(40.0, 40.0, 25.0, 0.3, 4.0);
            cx.close_path();
            cx.fill();
            cx.rotate(0.4);
            cx.set_rgba(0xCC3300, 0.6);
            cx.rectangle(10.0, 10.0, 30.0, 12.0);
            cx.fill();
        }
        pm.data().to_vec()
    };
    assert_eq!(run(), run());
}

#[test]
fn glyphs_are_drawn_from_the_font_file() {
    let Some(path) = ["/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
                      "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf"]
        .iter()
        .find(|p| std::path::Path::new(p).is_file())
    else {
        return; // no fonts on this machine
    };
    let data = std::sync::Arc::new(std::fs::read(path).unwrap());
    let face = ttf_parser::Face::parse(&data, 0).unwrap();
    let gid = face.glyph_index('B').unwrap().0;
    let font = folio_canvas::glyphs::FontRef {
        key: 1,
        data: data.clone(),
        index: 0,
        upem: face.units_per_em() as f64,
        synth_bold: false,
        synth_italic: false,
    };
    let mut pm = blank(60, 60);
    let mut g = Glyphs::new();
    {
        let mut cx = Cx::new(&mut pm, &mut g);
        cx.set_rgb(0x000000);
        cx.show_glyphs(&font, 40.0, &[(gid, 10.0, 45.0)]);
    }
    assert!(ink(&pm) > 100, "a letter should leave ink: {}", ink(&pm));
    // and the ink sits above the baseline
    let below = (46..60).any(|y| (0..60).any(|x| px(&pm, x, y).0 < 200));
    assert!(!below, "nothing of a B hangs below the baseline");
}
