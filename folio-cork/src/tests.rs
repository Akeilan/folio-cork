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
