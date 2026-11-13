//! Unit tests: model, geometry, routing, files and export.

use crate::editor::Editor;
use crate::model::*;
use folio_kit::text::Text;

fn ed() -> Editor {
    Editor::new(Document::default())
}

#[test]
fn shapes_connect_and_route_around() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    let b = e.insert_kind_at("rect", 500.0, 100.0);
    let _block = e.insert_kind_at("rect", 300.0, 100.0);
    let c = e.add_connector(End::glued(a, None), End::glued(b, None));
    let s = e.pg().get(c).unwrap().clone();
    let r = crate::route::route(e.pg(), &s);
    assert!(r.pts.len() >= 2);
    // The right-angle route must not cross the blocking shape in the middle.
    let blk = e.pg().shapes.iter().find(|x| x.id == _block).unwrap().clone();
    for w in r.pts.windows(2) {
        let mid = ((w[0].0 + w[1].0) / 2.0, (w[0].1 + w[1].1) / 2.0);
        assert!(!crate::geom::hit_shape(&blk, mid, 0.0), "route passes through a shape: {:?}", r.pts);
    }
}

#[test]
fn vsdx_round_trip_keeps_everything() {
    let mut e = ed();
    let a = e.insert_kind_at("flow.decision", 100.0, 100.0);
    let b = e.insert_kind_at("ee.resistor", 400.0, 120.0);
    e.add_connector(End::glued(a, Some(1)), End::glued(b, Some(0)));
    let st = e.make_shape("sticky", 300.0, 400.0);
    e.add_shape(st, "s");
    e.pgm().get_mut(a).unwrap().text = "Ready?".into();
    let bytes = crate::vsdx::write(&e.doc).unwrap();
    let back = crate::vsdx::read(&bytes).unwrap();
    assert_eq!(back.pages[0].shapes.len(), e.doc.pages[0].shapes.len());
    assert_eq!(back.pages[0].get(a).unwrap().text, "Ready?");
}

#[test]
fn vsdx_pages_readable_without_cork_copy() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    let b = e.insert_kind_at("ellipse", 400.0, 300.0);
    e.pgm().get_mut(a).unwrap().text = "Box".into();
    e.add_connector(End::glued(a, None), End::glued(b, None));
    let bytes = crate::vsdx::write(&e.doc).unwrap();
    // Remove Cork's own copy: what's left is plain Visio XML.
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut out);
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let name = f.name().map(|n| n.to_string()).unwrap_or_default();
            if name == "folio/cork.json" {
                continue;
            }
            let mut v = vec![];
            std::io::Read::read_to_end(&mut f, &mut v).unwrap();
            w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut w, &v).unwrap();
        }
        w.finish().unwrap();
    }
    let back = crate::vsdx::read(&out.into_inner()).unwrap();
    let p = &back.pages[0];
    let boxes: Vec<&Shape> = p.shapes.iter().filter(|s| !s.is_connector()).collect();
    assert_eq!(boxes.len(), 2);
    let r = boxes.iter().find(|s| s.text == "Box").expect("text kept");
    let o = e.pg().get(a).unwrap();
    assert!((r.w - o.w).abs() < 0.5 && (r.cx() - o.cx()).abs() < 0.5 && (r.cy() - o.cy()).abs() < 0.5, "position {} {} {}", r.w, r.cx(), r.cy());
    let c = p.shapes.iter().find(|s| s.is_connector()).expect("connector kept");
    let cn = c.conn.as_ref().unwrap();
    assert!(cn.a.shape.is_some() && cn.b.shape.is_some(), "glue kept");
}

#[test]
fn exports_are_valid() {
    let mut e = ed();
    crate::templates::apply("flowchart", &mut e);
    let mut t = Text::new();
    let svg = crate::export::svg_string(&e.doc, e.pg(), &mut t);
    assert!(folio_kit::icons::render_svg(&svg, 300, 200).is_some());
    let pdf = crate::export::pdf_bytes(&e.doc, &[0], &mut t, false).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}

#[test]
fn org_chart_from_csv() {
    let mut e = ed();
    let mut t = Text::new();
    let rows = crate::layout::parse_csv("Name,Title,Reports To\nAda,Director,\nBo,Engineer,Ada\nCy,\"Designer, UX\",Ada\nDee,Intern,Bo\n", ',');
    assert_eq!(rows[3][1], "Designer, UX");
    let n = crate::layout::org_from_rows(&mut e, &rows, &mut t).unwrap();
    assert_eq!(n, 4);
    let pg = e.pg();
    let y = |name: &str| pg.shapes.iter().find(|s| s.get("name") == name).unwrap().y;
    assert!(y("Bo") > y("Ada") && y("Dee") > y("Bo") && (y("Bo") - y("Cy")).abs() < 0.5);
}

#[test]
fn undo_merges_typing() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 0.0, 0.0);
    e.begin_edit(a);
    e.type_text("a");
    e.type_text("b");
    e.end_edit();
    assert_eq!(e.pg().get(a).unwrap().text, "ab");
    e.undo();
    assert_eq!(e.pg().get(a).unwrap().text, "");
    e.undo();
    assert!(e.pg().get(a).is_none());
}

#[test]
fn every_stencil_has_geometry() {
    for st in crate::stencils::STENCILS {
        let mut e = ed();
        let id = e.insert_kind_at(st.id, 0.0, 0.0);
        let s = e.pg().get(id).unwrap().clone();
        let g = crate::geom::geometry(&s);
        assert!(!g.subs.is_empty() || st.id == "text", "{} has no outline", st.id);
        assert!(!g.points.is_empty(), "{} has no connection points", st.id);
    }
}

#[test]
fn working_together_over_loopback() {
    let mut host_ed = ed();
    let a = host_ed.insert_kind_at("rect", 100.0, 100.0);
    host_ed.pgm().get_mut(a).unwrap().text = "Hello".into();
    let mut host = crate::collab::Collab::host(&host_ed, "Host").unwrap();
    let info = host.info();
    let port = info.lines().find(|l| l.contains("Address:")).and_then(|l| l.rsplit(':').next()).unwrap().trim().to_string();
    let code = info.lines().find(|l| l.contains("Code:")).and_then(|l| l.split("Code:").nth(1)).unwrap().trim().to_string();
    let mut guest_ed = ed();
    let mut guest = crate::collab::Collab::join(&format!("127.0.0.1:{port}"), &code, "Guest").unwrap();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        host.poll(&mut host_ed);
        guest.poll(&mut guest_ed);
        if guest_ed.pg().shapes.iter().any(|s| s.text == "Hello") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(guest_ed.pg().shapes.iter().any(|s| s.text == "Hello"), "guest didn't receive the board");
    // The guest edits; the host sees it.
    let id = guest_ed.pg().shapes[0].id;
    guest_ed.checkpoint("x", "");
    guest_ed.pgm().get_mut(id).unwrap().text = "From guest".into();
    guest_ed.changed();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        guest.poll(&mut guest_ed);
        host.poll(&mut host_ed);
        if host_ed.pg().shapes.iter().any(|s| s.text == "From guest") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(host_ed.pg().shapes.iter().any(|s| s.text == "From guest"), "host didn't receive the edit");
    // A wrong code gets nothing.
    let mut bad_ed = ed();
    let mut bad = crate::collab::Collab::join(&format!("127.0.0.1:{port}"), "AAAA-BBBB-CCCC", "Bad").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    host.poll(&mut host_ed);
    bad.poll(&mut bad_ed);
    assert!(!bad_ed.pg().shapes.iter().any(|s| s.text == "From guest"));
}

#[test]
fn working_together_through_a_go_between() {
    // K3-09: neither end can reach the other, so both reach a go-between instead. Here it runs
    // in this process on a loopback port; in life it is `folio-cork --relay` on a computer both
    // can see.
    let listener = folio_kit::platform::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let relay_port = listener.local_addr().unwrap().port();
    folio_kit::platform::spawn("test-relay", move || {
        let _ = crate::relay::serve_on(listener, &|_| {});
    });
    let relay = format!("127.0.0.1:{relay_port}");

    let mut host_ed = ed();
    let a = host_ed.insert_kind_at("rect", 100.0, 100.0);
    host_ed.pgm().get_mut(a).unwrap().text = "Through the middle".into();
    let mut host = crate::collab::Collab::host_via(&host_ed, "Host", &relay).unwrap();
    let info = host.info();
    let code = info.lines().find(|l| l.contains("Code:")).and_then(|l| l.split("Code:").nth(1)).unwrap().trim().to_string();
    assert!(info.contains(&relay), "the address people are given is the go-between's: {info}");

    let mut guest_ed = ed();
    let mut guest = crate::collab::Collab::join_via(&relay, &code, "Guest").unwrap();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(10) {
        host.poll(&mut host_ed);
        guest.poll(&mut guest_ed);
        if guest_ed.pg().shapes.iter().any(|s| s.text == "Through the middle") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(guest_ed.pg().shapes.iter().any(|s| s.text == "Through the middle"),
            "the board did not reach the guest through the go-between");

    // and back the other way
    let id = guest_ed.pg().shapes[0].id;
    guest_ed.checkpoint("x", "");
    guest_ed.pgm().get_mut(id).unwrap().text = "Answered".into();
    guest_ed.changed();
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(10) {
        guest.poll(&mut guest_ed);
        host.poll(&mut host_ed);
        if host_ed.pg().shapes.iter().any(|s| s.text == "Answered") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(host_ed.pg().shapes.iter().any(|s| s.text == "Answered"),
            "the edit did not come back through the go-between");

    // the wrong code is a different room, and the go-between says so rather than hanging
    let bad = crate::collab::Collab::join_via(&relay, "AAAA-BBBB-CCCC", "Bad");
    assert!(bad.is_err(), "a different code must not reach the board");
    let why = bad.err().unwrap();
    assert!(why.contains("nobody sharing a board with that code"), "{why}");

    // one box for both kinds of address: the go-between answers as one, a direct host does not
    let mut direct_ed = ed();
    direct_ed.insert_kind_at("rect", 0.0, 0.0);
    let direct = crate::collab::Collab::host(&direct_ed, "Direct").unwrap();
    let dinfo = direct.info();
    let dport = dinfo.lines().find(|l| l.contains("Address:")).and_then(|l| l.rsplit(':').next()).unwrap().trim().to_string();
    let dcode = dinfo.lines().find(|l| l.contains("Code:")).and_then(|l| l.split("Code:").nth(1)).unwrap().trim().to_string();
    assert!(crate::collab::Collab::join_any(&format!("127.0.0.1:{dport}"), &dcode, "G2").is_ok(),
            "the same box reaches a host sharing directly");
    assert!(crate::collab::Collab::join_any(&relay, &code, "G3").is_ok(),
            "and reaches a board through a go-between");
}

#[test]
fn pdf_tiles_large_pages() {
    let mut e = ed();
    e.insert_kind_at("rect", 0.0, 0.0);
    e.insert_kind_at("rect", 3000.0, 2000.0);
    let mut t = Text::new();
    let tiled = crate::export::pdf_bytes(&e.doc, &[0], &mut t, false).unwrap();
    let fit = crate::export::pdf_bytes(&e.doc, &[0], &mut t, true).unwrap();
    let count = |b: &[u8]| String::from_utf8_lossy(b).matches("/Type /Page\n").count() + String::from_utf8_lossy(b).matches("/Type /Page/").count() + String::from_utf8_lossy(b).matches("/Type /Page ").count();
    assert!(count(&tiled) > count(&fit), "tiled {} fit {}", count(&tiled), count(&fit));
}

/// Writes a sample drawing for checking in other programs: SAMPLE_OUT=/path.vsdx cargo test write_sample -- --ignored
#[test]
#[ignore]
fn write_sample() {
    let mut e = ed();
    crate::templates::apply("flowchart", &mut e);
    crate::templates::apply("schematic", &mut e);
    crate::templates::apply("orgchart", &mut e);
    let mut pm = folio_kit::sk::Pixmap::new(160, 100).unwrap();
    pm.fill(folio_kit::sk::Color::from_rgba8(40, 120, 200, 255));
    let png = folio_kit::gfx::encode_png(&pm);
    let mut images = crate::render::Images::default();
    let id = crate::mood::insert_picture(&mut e, png, (200.0, 700.0), &mut images).unwrap();
    e.pgm().get_mut(id).unwrap().crop = (0.25, 0.0, 1.0, 1.0);
    for (i, k) in ["ee.npn", "ee.opamp", "ee.and", "net.server", "uml.actor", "floor.door", "erd.entity", "cloud", "star"].iter().enumerate() {
        e.insert_kind_at(k, 500.0 + i as f32 * 120.0, 760.0);
    }
    let out = std::env::var("SAMPLE_OUT").unwrap_or("/tmp/sample.vsdx".into());
    crate::files::save(&e.doc, std::path::Path::new(&out)).unwrap();
    let mut t = Text::new();
    crate::export::png(&e.doc, &[0], &std::path::Path::new(&out).with_extension("png"), &mut t, &mut images, 1.0).unwrap();
}

fn fnv(b: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for x in b {
        h = (h ^ *x as u64).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// X3-12: every template renders and exports to the same bytes on every machine and CPU. The
/// fingerprints are stored in tests/golden.txt; `FOLIO_UPDATE_GOLDEN=1` rewrites them after a
/// deliberate change to drawing or export.
#[test]
fn templates_render_identically_everywhere() {
    let dir = folio_kit::platform::temp_dir().join(format!("folio-cork-golden-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut got = String::new();
    for (key, _, _) in crate::templates::LIST {
        let mut e = ed();
        crate::templates::apply(key, &mut e);
        let mut t = Text::bundled();
        let mut images = crate::render::Images::default();
        let pages: Vec<usize> = (0..e.doc.pages.len()).collect();
        let png = dir.join(format!("{key}.png"));
        let svg = dir.join(format!("{key}.svg"));
        let pdf = dir.join(format!("{key}.pdf"));
        let vsdx = dir.join(format!("{key}.vsdx"));
        crate::export::png(&e.doc, &[0], &png, &mut t, &mut images, 1.0).unwrap();
        crate::export::svg(&e.doc, &[0], &svg, &mut t).unwrap();
        crate::export::pdf(&e.doc, &pages, &pdf, &mut t, &mut images, true, false).unwrap();
        crate::files::save(&e.doc, &vsdx).unwrap();
        let px = folio_kit::gfx::decode_image(&std::fs::read(&png).unwrap()).unwrap();
        for (kind, bytes) in [("pixels", px.data().to_vec()), ("svg", std::fs::read(&svg).unwrap()), ("pdf", std::fs::read(&pdf).unwrap()), ("vsdx", std::fs::read(&vsdx).unwrap())] {
            got.push_str(&format!("{key} {kind} {}\n", fnv(&bytes)));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden.txt");
    if std::env::var_os("FOLIO_UPDATE_GOLDEN").is_some() || !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap();
    let diff: Vec<String> = want.lines().zip(got.lines()).filter(|(a, b)| a != b).map(|(a, b)| format!("want {a} / got {b}")).collect();
    assert!(diff.is_empty() && want.lines().count() == got.lines().count(), "output changed:\n{}", diff.join("\n"));
}

/// X3-14: the whole window test, with no window and no display server.
#[test]
fn ui_test_runs_headless() {
    let dir = folio_kit::platform::temp_dir().join(format!("folio-cork-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    folio_kit::platform::set_setting("FOLIO_UI_DIR", &dir);
    folio_kit::platform::set_setting("FOLIO_CONFIG_DIR", &dir.join("config"));
    folio_kit::platform::set_setting("FOLIO_FONTS_ONLY", std::path::Path::new("1"));
    let mut a = crate::app::CorkApp::new(Document::default(), None);
    a.settings = crate::settings::Settings::default();
    a.test = Some(crate::uitest::UiTest::new(dir.clone()));
    let mut frame = Vec::new();
    let code = folio_kit::platform::run_headless(&mut a, 1280, 800, 1.0, 20_000, &mut frame);
    assert_eq!(code, Some(0), "window test failed (screenshots in {})", dir.display());
    assert!(dir.join("ui-final.png").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Redrawing only what changed (scrolling, typing, moving a shape) must give exactly the
/// pixels a full redraw gives.
#[test]
fn partial_redraw_matches_full_redraw() {
    use folio_kit::platform::{App, Event, Headless};
    let (w, h) = (1280u32, 800u32);
    let mut host = Headless::default();
    let mut a = crate::app::CorkApp::new(Document::default(), None);
    a.settings = crate::settings::Settings::default();
    let mut px = vec![0u8; (w * h * 4) as usize];
    App::event(&mut a, &Event::Resized { w, h, scale: 1.0 }, &mut host);
    crate::bench::busy_board(&mut a);
    App::paint(&mut a, &mut px, w, h, &mut host);
    App::event(&mut a, &Event::MouseMove { x: 640.0, y: 400.0 }, &mut host);
    for dy in [-3.0, -2.0, 1.0, -4.0] {
        App::event(&mut a, &Event::Wheel { dx: 0.0, dy }, &mut host);
        App::paint(&mut a, &mut px, w, h, &mut host);
    }
    App::event(&mut a, &Event::Wheel { dx: -2.0, dy: 0.0 }, &mut host);
    App::paint(&mut a, &mut px, w, h, &mut host);
    let id = a.ed.pg().shapes.iter().find(|s| !s.is_connector() && s.x > 300.0 && s.y > 200.0).map(|s| s.id).unwrap();
    a.ed.sel = vec![id];
    for c in "Moved".chars() {
        App::event(&mut a, &Event::Text(c.to_string()), &mut host);
        App::paint(&mut a, &mut px, w, h, &mut host);
    }
    a.ed.end_edit();
    App::paint(&mut a, &mut px, w, h, &mut host);
    // Move the shape: its old and new places, and the lines glued to it, change.
    if let Some(s) = a.ed.pgm().get_mut(id) {
        s.x += 37.0;
        s.y += 11.0;
    }
    a.ed.changed();
    App::paint(&mut a, &mut px, w, h, &mut host);
    let partial = px.clone();
    a.forget_layer();
    App::paint(&mut a, &mut px, w, h, &mut host);
    let diff = partial.chunks(4).zip(px.chunks(4)).filter(|(a, b)| a != b).count();
    if diff > 0 {
        let _ = std::fs::write("/tmp/partial.rgba", &partial);
    }
    assert_eq!(diff, 0, "partial redraw differs from a full redraw in {diff} pixels");
}

#[test]
fn diagram_picture_carries_the_drawing() {
    // K3-08: the picture Page, Sheet and Show hold opens in Cork again, and saves back as one
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    let b = e.insert_kind_at("ellipse", 400.0, 300.0);
    e.add_connector(End::glued(a, None), End::glued(b, None));
    let png = crate::files::board_png(&e.doc).unwrap();
    let pm = folio_kit::gfx::decode_image(&png).expect("still an ordinary picture");
    assert!(pm.width() > 100 && pm.height() > 100);
    let back = crate::vsdx::read(&crate::files::board_in_png(&png).unwrap()).unwrap();
    assert_eq!(back.pages[0].shapes.len(), e.doc.pages[0].shapes.len());
    // a damaged chunk is ignored, a plain picture has none
    let mut bad = png.clone();
    let n = bad.len();
    bad[n - 20] ^= 0xFF;
    assert!(crate::files::board_in_png(&bad).is_none());
    assert!(crate::files::board_in_png(&folio_kit::gfx::encode_png(&pm)).is_none());
    // save to the picture's own path and open it again
    let p = std::env::temp_dir().join(format!("cork-diagram-{}.png", std::process::id()));
    crate::files::save(&e.doc, &p).unwrap();
    assert!(crate::files::keeps_path(&p));
    let again = crate::files::load(&p).unwrap();
    assert_eq!(again.pages[0].shapes.len(), 3);
    let _ = std::fs::remove_file(&p);
}

// ---------------------------------------------------------------- swimlanes (K4-01)
#[test]
fn lanes_hold_their_shapes_and_carry_them() {
    let mut e = ed();
    let first = e.add_lane();
    assert!(e.pg().get(first).unwrap().is_lane());
    let second = e.add_lane();
    let third = e.add_lane();
    let lanes = e.lanes();
    assert_eq!(lanes.len(), 3);
    // the lanes stack up, one under the next, in order
    assert!(lanes[0].y < lanes[1].y && lanes[1].y < lanes[2].y, "{:?}", lanes.iter().map(|l| l.y).collect::<Vec<_>>());
    assert_eq!(lanes.iter().map(|l| l.id).collect::<Vec<_>>(), vec![first, second, third]);
    // a lane sits behind what it holds
    assert!(e.pg().shapes[0].is_lane());

    // a shape dropped in the second lane belongs to it
    let (bx, by, bw, bh) = e.pg().get(second).unwrap().lane_body();
    let box_id = e.insert_kind_at("rect", bx + bw / 2.0, by + bh / 2.0);
    assert_eq!(e.inside_lane(second), vec![box_id]);
    assert!(e.inside_lane(first).is_empty());

    // moving the lane carries it
    let (x0, y0) = { let s = e.pg().get(box_id).unwrap(); (s.x, s.y) };
    e.sel = vec![second];
    e.shift(second, 0.0, 60.0);
    for id in e.inside_lane(second) {
        e.shift(id, 0.0, 60.0);
    }
    let s = e.pg().get(box_id).unwrap();
    assert_eq!((s.x, s.y), (x0, y0 + 60.0));
}

#[test]
fn a_new_lane_pushes_the_ones_after_it_down() {
    let mut e = ed();
    let a = e.add_lane();
    let b = e.add_lane();
    let before = e.pg().get(b).unwrap().y;
    e.sel = vec![a];
    let mid = e.add_lane();
    let (ya, ym, yb) = (e.pg().get(a).unwrap().y, e.pg().get(mid).unwrap().y, e.pg().get(b).unwrap().y);
    assert!(ya < ym && ym < yb, "{ya} {ym} {yb}");
    assert!(yb > before, "the lane after it moved down");
    assert_eq!(e.lanes().len(), 3);
}

#[test]
fn deleting_a_lane_keeps_its_shapes_and_closes_the_gap() {
    let mut e = ed();
    let a = e.add_lane();
    let b = e.add_lane();
    let c = e.add_lane();
    let (bx, by, bw, bh) = e.pg().get(b).unwrap().lane_body();
    let kept = e.insert_kind_at("rect", bx + bw / 2.0, by + bh / 2.0);
    let y_c = e.pg().get(c).unwrap().y;
    let h_b = e.pg().get(b).unwrap().h;
    e.sel = vec![b];
    assert_eq!(e.delete_lane(), 1);
    assert!(e.pg().get(b).is_none());
    assert!(e.pg().get(kept).is_some(), "what was in the lane stays");
    assert_eq!(e.lanes().iter().map(|l| l.id).collect::<Vec<_>>(), vec![a, c]);
    assert_eq!(e.pg().get(c).unwrap().y, y_c - h_b, "the lane below closed the gap");
    let _ = (bh, bw);
}

#[test]
fn lanes_turn_and_keep_what_they_hold() {
    let mut e = ed();
    let a = e.add_lane();
    let b = e.add_lane();
    let (bx, by, bw, bh) = e.pg().get(b).unwrap().lane_body();
    let box_id = e.insert_kind_at("rect", bx + bw / 2.0, by + bh / 2.0);
    assert!(!e.pg().get(a).unwrap().lane_vertical());
    assert_eq!(e.turn_lanes(), 2);
    assert!(e.pg().get(a).unwrap().lane_vertical() && e.pg().get(b).unwrap().lane_vertical());
    // side by side now, and the shape is still in its own lane
    let (la, lb) = (e.pg().get(a).unwrap().clone(), e.pg().get(b).unwrap().clone());
    assert!(la.x < lb.x && (la.y - lb.y).abs() < 0.01, "{:?} {:?}", (la.x, la.y), (lb.x, lb.y));
    assert_eq!(e.inside_lane(b), vec![box_id]);
    let _ = bh;
    // and back again
    e.turn_lanes();
    assert!(!e.pg().get(a).unwrap().lane_vertical());
    assert_eq!(e.inside_lane(b), vec![box_id]);
}

#[test]
fn the_cross_functional_template_has_lanes_with_a_flow_in_them() {
    let mut e = ed();
    crate::templates::apply("swimlane", &mut e);
    let lanes = e.lanes();
    assert_eq!(lanes.len(), 3);
    assert_eq!(lanes.iter().map(|l| l.text.clone()).collect::<Vec<_>>(), vec!["Customer", "Sales", "Warehouse"]);
    let held: usize = lanes.iter().map(|l| e.inside_lane(l.id).len()).sum();
    assert!(held >= 5, "the flow shapes sit in the lanes: {held}");
    assert!(e.pg().shapes.iter().any(|s| s.is_connector()));
}

#[test]
#[ignore]
fn write_swimlane_picture() {
    // cargo test -p folio-cork write_swimlane_picture -- --ignored --nocapture
    let mut e = ed();
    crate::templates::apply("swimlane", &mut e);
    let mut t = Text::bundled();
    let mut images = crate::render::Images::default();
    let out = std::path::Path::new(&std::env::var("LANE_OUT").unwrap_or("/tmp/swimlane.png".into())).to_path_buf();
    crate::export::png(&e.doc, &[0], &out, &mut t, &mut images, 1.5).unwrap();
    println!("wrote {}", out.display());
}

// ---------------------------------------------------------------- shape data (K4-02)
#[test]
fn shape_data_is_put_on_the_selection_and_read_back() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    let b = e.insert_kind_at("rect", 300.0, 100.0);
    e.sel = vec![a, b];
    assert_eq!(e.set_prop("Owner", "Ron"), 2);
    e.sel = vec![a];
    assert_eq!(e.set_prop("Cost", "12.50"), 1);
    assert_eq!(e.pg().get(a).unwrap().prop("Cost"), "12.50");
    assert_eq!(e.pg().get(b).unwrap().prop("Cost"), "");
    // both selected: the shared value shows, the one they differ on comes back empty
    e.sel = vec![a, b];
    assert_eq!(e.props_of_selection(), vec![("Cost".to_string(), String::new()), ("Owner".to_string(), "Ron".to_string())]);
    // renaming keeps each shape's own value
    e.sel = vec![a];
    assert_eq!(e.rename_prop("Cost", "Price"), 1);
    assert_eq!(e.pg().get(a).unwrap().prop("Price"), "12.50");
    assert_eq!(e.pg().get(a).unwrap().prop("Cost"), "");
    // and a property comes off again
    e.sel = vec![a, b];
    assert_eq!(e.clear_prop("Owner"), 2);
    assert!(e.props_of_selection().iter().all(|(n, _)| n != "Owner"));
    // shape data is not mistaken for Cork's own fields
    assert!(e.pg().get(a).unwrap().data.keys().all(|k| k.starts_with("prop.")));
}

#[test]
fn shape_data_travels_in_the_vsdx_as_visio_writes_it() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    e.sel = vec![a];
    e.set_prop("Part number", "A-120");
    e.set_prop("Owner", "Ron");
    let bytes = crate::vsdx::write(&e.doc).unwrap();

    // the Visio side: a Property section with the names as typed
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).unwrap();
    let page = {
        use std::io::Read;
        let mut f = zip.by_name("visio/pages/page1.xml").unwrap();
        let mut sx = String::new();
        f.read_to_string(&mut sx).unwrap();
        sx
    };
    assert!(page.contains("<Section N='Property'>"), "no Property section:\n{page}");
    assert!(page.contains("N='Part_number'") && page.contains("V='Part number'"), "row name or label missing:\n{page}");
    assert!(page.contains("V='A-120'") && page.contains("V='Ron'"), "values missing");

    // read it back the way another program's file would be read (Cork's own copy left out)
    let stripped = {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut out);
            for i in 0..zip.len() {
                use std::io::{Read, Write};
                let mut f = zip.by_index(i).unwrap();
                let name = f.name().unwrap_or_default().to_string();
                if name == "folio/cork.json" {
                    continue;
                }
                let mut buf = vec![];
                f.read_to_end(&mut buf).unwrap();
                w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
                w.write_all(&buf).unwrap();
            }
            w.finish().unwrap();
        }
        out.into_inner()
    };
    let back = crate::vsdx::read(&stripped).unwrap();
    let s = back.pages[0].shapes.iter().find(|s| !s.is_connector()).expect("no shape read back");
    assert_eq!(s.prop("Part number"), "A-120", "props read back: {:?}", s.props());
    assert_eq!(s.prop("Owner"), "Ron");
    assert_eq!(s.props().len(), 2);
}

#[test]
fn shape_data_comes_back_from_cork_s_own_copy() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    e.sel = vec![a];
    e.set_prop("Cost", "12.50");
    let bytes = crate::vsdx::write(&e.doc).unwrap();
    let back = crate::vsdx::read(&bytes).unwrap();
    let s = back.pages[0].shapes.iter().find(|s| s.id == a).unwrap();
    assert_eq!(s.prop("Cost"), "12.50");
}

// ---------------------------------------------------------------- hyperlinks (K4-04)
#[test]
fn a_shape_points_at_a_page_a_file_or_an_address() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    let b = e.insert_kind_at("rect", 300.0, 100.0);
    e.sel = vec![a, b];
    assert_eq!(e.set_link(LinkKind::Address, "https://example.org/plan"), 2);
    assert_eq!(e.pg().get(a).unwrap().link(), Some((LinkKind::Address, "https://example.org/plan")));
    // one shape at a time, and the link is read back from the selection
    e.sel = vec![b];
    e.set_link(LinkKind::File, "~/Documents/notes.pdf");
    assert_eq!(e.link_of_selection(), Some((LinkKind::File, "~/Documents/notes.pdf".to_string())));
    assert_eq!(e.clear_link(), 1);
    assert_eq!(e.link_of_selection(), None);
    // a link to another page takes you there
    e.add_page(false);
    e.rename_page("Details");
    e.page = 0;
    e.sel = vec![a];
    e.set_link(LinkKind::Page, "Details");
    assert!(e.goto_page_named("details"), "page names are matched whatever the case");
    assert_eq!(e.pg().name, "Details");
    assert!(!e.goto_page_named("Nowhere"));
    // what a typed target plainly is
    let pages = e.page_names();
    assert_eq!(link_kind_of("Details", &pages), LinkKind::Page);
    assert_eq!(link_kind_of("www.example.org", &pages), LinkKind::Address);
    assert_eq!(link_kind_of("mailto:ron@example.org", &pages), LinkKind::Address);
    assert_eq!(link_kind_of("/home/ron/plan.vsdx", &pages), LinkKind::File);
}

#[test]
fn a_link_travels_in_the_vsdx_as_visio_writes_it() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    e.add_page(false);
    e.rename_page("Details");
    e.page = 0;
    e.sel = vec![a];
    e.set_link(LinkKind::Address, "https://example.org/plan");
    let b = e.insert_kind_at("rect", 300.0, 100.0);
    e.sel = vec![b];
    e.set_link(LinkKind::Page, "Details");
    let bytes = crate::vsdx::write(&e.doc).unwrap();

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).unwrap();
    let page = {
        use std::io::Read;
        let mut f = zip.by_name("visio/pages/page1.xml").unwrap();
        let mut sx = String::new();
        f.read_to_string(&mut sx).unwrap();
        sx
    };
    assert!(page.contains("<Section N='Hyperlink'>"), "no Hyperlink section:\n{page}");
    assert!(page.contains("N='Address' V='https://example.org/plan'"), "address missing:\n{page}");
    assert!(page.contains("N='SubAddress' V='Details'"), "page link missing:\n{page}");
    // and the link is not left lying about in the shape data rows
    assert!(!page.contains("V='linkto'"), "link fields leaked into the shape data:\n{page}");

    // read it back the way another program's file would be read
    let stripped = {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut out);
            for i in 0..zip.len() {
                use std::io::{Read, Write};
                let mut f = zip.by_index(i).unwrap();
                let name = f.name().unwrap_or_default().to_string();
                if name == "folio/cork.json" {
                    continue;
                }
                let mut buf = vec![];
                f.read_to_end(&mut buf).unwrap();
                w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
                w.write_all(&buf).unwrap();
            }
            w.finish().unwrap();
        }
        out.into_inner()
    };
    let back = crate::vsdx::read(&stripped).unwrap();
    let links: Vec<(LinkKind, String)> = back.pages[0].shapes.iter().filter_map(|s| s.link().map(|(k, t)| (k, t.to_string()))).collect();
    assert!(links.contains(&(LinkKind::Address, "https://example.org/plan".to_string())), "links read back: {links:?}");
    assert!(links.contains(&(LinkKind::Page, "Details".to_string())), "links read back: {links:?}");
}

// ---------------------------------------------------------------- find and replace (K4-05)
#[test]
fn find_walks_every_page_and_replace_changes_what_it_finds() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    e.pgm().get_mut(a).unwrap().text = "Draft plan".into();
    let b = e.insert_kind_at("rect", 300.0, 100.0);
    e.pgm().get_mut(b).unwrap().text = "draft again".into();
    e.add_page(false);
    let c = e.insert_kind_at("rect", 100.0, 100.0);
    e.pgm().get_mut(c).unwrap().text = "Final DRAFT".into();
    let d = e.insert_kind_at("rect", 300.0, 100.0);
    e.pgm().get_mut(d).unwrap().text = "Redrafted".into();
    e.page = 0;
    e.sel.clear();

    // case-blind, every page
    assert_eq!(e.find_all("draft", false, false).len(), 4);
    assert_eq!(e.find_all("draft", true, false).len(), 2, "with Match case only the two lower-case ones");
    assert_eq!(e.find_all("draft", false, true).len(), 3, "whole words leaves Redrafted out");
    assert_eq!(e.find_all("", false, false).len(), 0);

    // Find Next walks them in order and wraps round
    let first = e.find_next("draft", false, false, false).unwrap();
    assert_eq!((first.page, first.shape), (0, a));
    assert_eq!(e.sel, vec![a]);
    let second = e.find_next("draft", false, false, false).unwrap();
    assert_eq!((second.page, second.shape), (0, b));
    let third = e.find_next("draft", false, false, false).unwrap();
    assert_eq!(third.page, 1, "it carries on to the next page");
    assert_eq!(e.page, 1, "and the window follows");
    e.find_next("draft", false, false, false);
    let wrapped = e.find_next("draft", false, false, false).unwrap();
    assert_eq!((wrapped.page, wrapped.shape), (0, a), "and round again");
    // backwards
    let back = e.find_next("draft", false, false, true).unwrap();
    assert_eq!(back.page, 1);
    assert_eq!(e.find_next("nothing of the sort", false, false, false), None);

    // Replace All across the drawing, keeping each word's own case alone
    e.page = 0;
    assert_eq!(e.replace_all("draft", "final", false, false, false), 4);
    assert_eq!(e.pg().get(a).unwrap().text, "final plan");
    assert_eq!(e.doc.pages[1].get(c).unwrap().text, "Final final");
    assert_eq!(e.doc.pages[1].get(d).unwrap().text, "Refinaled");
    assert!(e.can_undo());
    e.undo();
    assert_eq!(e.pg().get(a).unwrap().text, "Draft plan", "one undo puts the lot back");

    // only the selection, and a locked shape is left alone
    e.sel = vec![a];
    assert_eq!(e.replace_all("Draft", "Final", true, false, true), 1);
    assert_eq!(e.pg().get(a).unwrap().text, "Final plan");
    assert_eq!(e.pg().get(b).unwrap().text, "draft again");
    e.pgm().get_mut(b).unwrap().locked = true;
    e.sel = vec![b];
    assert_eq!(e.replace_all("draft", "nope", false, false, true), 0);
    assert_eq!(e.pg().get(b).unwrap().text, "draft again");
    // a replacement that changed nothing leaves no undo step of its own: one undo goes back
    // past it, to before the replacement that did change something
    e.undo();
    assert_eq!(e.pg().get(a).unwrap().text, "Draft plan");
}

#[test]
fn whole_words_and_case_are_matched_as_asked() {
    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    e.pgm().get_mut(a).unwrap().text = "cat, cats and a cat_flap".into();
    assert_eq!(e.replace_all("cat", "dog", false, true, false), 1, "only the word on its own");
    assert_eq!(e.pg().get(a).unwrap().text, "dog, cats and a cat_flap");
    e.pgm().get_mut(a).unwrap().text = "Straße und Straße".into();
    assert_eq!(e.replace_all("straße", "Weg", false, false, false), 2, "a case-blind search works on other alphabets");
    assert_eq!(e.pg().get(a).unwrap().text, "Weg und Weg");
    // a letter that grows when it is lowered (Turkish I with a dot) must not shift the match
    e.pgm().get_mut(a).unwrap().text = "\u{130}stanbul to Ankara, \u{130}stanbul again".into();
    assert_eq!(e.replace_all("ankara", "Bursa", false, false, false), 1, "the word after it is found");
    assert_eq!(e.pg().get(a).unwrap().text, "\u{130}stanbul to Bursa, \u{130}stanbul again");
}

// ---------------------------------------------------------------- link to a spreadsheet (K4-03)
#[test]
fn shape_data_comes_from_a_file_and_is_read_again_on_refresh() {
    let dir = std::env::temp_dir().join(format!("cork-datalink-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let csv = dir.join("stock.csv");
    std::fs::write(&csv, "Part,Owner,Cost\nA-120,Ron,12.50\nB-7,Mel,3.00\nC-9,Nobody,1.00\n").unwrap();

    let mut e = ed();
    let a = e.insert_kind_at("rect", 100.0, 100.0);
    e.pgm().get_mut(a).unwrap().text = "A-120".into();
    let b = e.insert_kind_at("rect", 300.0, 100.0);
    e.pgm().get_mut(b).unwrap().text = "b-7\nspare".into();
    let c = e.insert_kind_at("rect", 500.0, 100.0);
    e.pgm().get_mut(c).unwrap().text = "Not in the file".into();

    let rep = crate::datalink::link(&mut e, &csv, "Part", "").unwrap();
    assert_eq!((rep.filled, rep.columns, rep.unmatched), (2, 2, 1), "two shapes filled, one row left over");
    assert_eq!(e.pg().get(a).unwrap().prop("Owner"), "Ron");
    assert_eq!(e.pg().get(a).unwrap().prop("Cost"), "12.50");
    assert_eq!(e.pg().get(b).unwrap().prop("Owner"), "Mel", "the first line of the text is the key, whatever its case");
    assert_eq!(e.pg().get(c).unwrap().props().len(), 0);
    assert_eq!(e.pg().get(a).unwrap().prop("Part"), "", "the key column is not copied onto the shape");

    // the link is remembered, and one undo puts the drawing back
    assert_eq!(e.doc.links.len(), 1);
    assert_eq!(e.doc.links[0].key, "Part");
    e.undo();
    assert_eq!(e.pg().get(a).unwrap().props().len(), 0);
    crate::datalink::link(&mut e, &csv, "Part", "").unwrap();

    // the file changes; Refresh brings the new values in
    std::fs::write(&csv, "Part,Owner,Cost\nA-120,Mel,19.99\nB-7,Mel,3.00\n").unwrap();
    let (rep, trouble) = crate::datalink::refresh(&mut e);
    assert!(trouble.is_empty(), "{trouble:?}");
    assert_eq!(rep.filled, 2);
    assert_eq!(e.pg().get(a).unwrap().prop("Owner"), "Mel");
    assert_eq!(e.pg().get(a).unwrap().prop("Cost"), "19.99");

    // matching on a property instead of the text
    let mut e2 = ed();
    let s = e2.insert_kind_at("rect", 10.0, 10.0);
    e2.sel = vec![s];
    e2.set_prop("Part number", "B-7");
    let rep = crate::datalink::link(&mut e2, &csv, "Part", "Part number").unwrap();
    assert_eq!(rep.filled, 1);
    assert_eq!(e2.pg().get(s).unwrap().prop("Owner"), "Mel");

    // what the box offers, and what it says when the file doesn't fit
    assert_eq!(crate::datalink::columns(&csv).unwrap(), vec!["Part", "Owner", "Cost"]);
    assert!(crate::datalink::property_names(&e2).contains(&"Part number".to_string()));
    assert!(crate::datalink::link(&mut e2, &csv, "Nothing", "").is_err());
    std::fs::write(&csv, "Part\nA-120\n").unwrap();
    assert!(crate::datalink::link(&mut e2, &csv, "Part", "").unwrap_err().contains("nothing but the key column"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_linked_file_is_kept_in_the_saved_drawing() {
    let mut e = ed();
    e.doc.links.push(DataLink { path: "/tmp/stock.csv".into(), key: "Part".into(), match_on: String::new() });
    let bytes = crate::vsdx::write(&e.doc).unwrap();
    let back = crate::vsdx::read(&bytes).unwrap();
    assert_eq!(back.links, e.doc.links);
}

// ---------------------------------------------------------------- Visio stencils (K4-06)
#[test]
fn a_visio_stencil_is_read_into_shapes() {
    let bytes = crate::uitest::sample_stencil();
    let masters = crate::vsdx::read_stencil(&bytes).expect("stencil");
    let names: Vec<&str> = masters.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["Server", "Firewall"]);
    let (_, server) = &masters[0];
    assert_eq!(server.len(), 1);
    let s = &server[0];
    // an inch is 96 pixels, and the shape sits at the top-left corner ready to be dropped
    assert!((s.w - 96.0).abs() < 0.5 && (s.h - 144.0).abs() < 0.5, "size: {} x {}", s.w, s.h);
    assert!(s.x.abs() < 0.5 && s.y.abs() < 0.5, "at {}, {}", s.x, s.y);
    assert_eq!(s.text, "Server");
    assert_eq!(s.style.fill, folio_kit::Color::hex(0x4472C4));
    let (_, fw) = &masters[1];
    assert!((fw[0].w - 192.0).abs() < 0.5);

    // the shapes go into a drawing as they are
    let mut e = ed();
    e.paste_shapes(server, &[], Some((200.0, 150.0)), "Insert stencil shape");
    let added = e.pg().shapes.last().unwrap();
    assert!((added.w - 96.0).abs() < 0.5);
    assert_eq!(added.text, "Server");

    // and something that isn't a stencil says so rather than giving up in a heap
    assert!(crate::vsdx::read_stencil(b"not a zip at all").is_err());
    assert!(crate::vsdx::is_stencil_name("Network.vssx") && !crate::vsdx::is_stencil_name("plan.vsdx"));
}
