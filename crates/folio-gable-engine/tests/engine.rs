//! The engine builds a box, pushes a face, and draws what it made.
use folio_gable_engine::draw::{render, Style};
use folio_gable_engine::math::{v3, V3};
use folio_gable_engine::model::Model;
use folio_gable_engine::solids;
use folio_gable_engine::view::Camera;

#[test]
fn a_box_has_six_faces_and_the_room_it_should() {
    let mut m = Model::new();
    let faces = solids::add_box(&mut m.ents, V3::default(), v3(1000.0, 2000.0, 500.0));
    assert_eq!(faces.len(), 6, "six faces");
    assert_eq!(m.ents.verts.iter().filter(|v| v.alive).count(), 8, "eight corners");
    assert_eq!(m.ents.edges.iter().filter(|e| e.alive).count(), 12, "twelve edges");
    let v = solids::volume(&m.ents);
    assert!((v - 1000.0 * 2000.0 * 500.0).abs() < 1.0, "the room inside: {v}");
    let (lo, hi) = m.bounds().expect("bounds");
    assert!(lo.near(V3::default()) && hi.near(v3(1000.0, 2000.0, 500.0)), "the box it sits in");
}

#[test]
fn push_pull_turns_a_face_into_a_solid() {
    let mut m = Model::new();
    let f = solids::add_polygon(
        &mut m.ents,
        &[v3(0.0, 0.0, 0.0), v3(1000.0, 0.0, 0.0), v3(1000.0, 1000.0, 0.0), v3(0.0, 1000.0, 0.0)],
    )
    .expect("a square");
    let made = solids::push_pull(&mut m.ents, f, 300.0);
    assert_eq!(made.len(), 5, "four walls and a top");
    let v = solids::volume(&m.ents);
    assert!((v - 1000.0 * 1000.0 * 300.0).abs() < 1.0, "the room inside: {v}");
}

#[test]
fn the_model_is_drawn_with_its_faces_and_edges() {
    let mut m = Model::new();
    solids::add_box(&mut m.ents, V3::default(), v3(1000.0, 1000.0, 1000.0));
    let mut cam = Camera::default();
    let (lo, hi) = m.bounds().unwrap();
    cam.look_at_box(lo, hi);
    let proj = cam.matrix(800.0, 600.0);
    let frame = render(&m, &proj, &Style::default(), 800, 600, &[]);
    // the middle of the picture is the box, not the background
    let mid = frame.color[(300 * 800 + 400) as usize] & 0xFFFFFF;
    assert_ne!(mid, 0xF2F2F2, "the box is in the middle of the view");
    // and something was drawn in the darker edge colour
    let edges = frame.color.iter().filter(|c| (*c & 0xFFFFFF) == 0x2A2A2A).count();
    assert!(edges > 100, "the edges are drawn: {edges} pixels");
}

#[test]
fn a_click_finds_the_face_under_it() {
    let mut m = Model::new();
    solids::add_box(&mut m.ents, V3::default(), v3(1000.0, 1000.0, 1000.0));
    let mut cam = Camera::default();
    let (lo, hi) = m.bounds().unwrap();
    cam.look_at_box(lo, hi);
    let proj = cam.matrix(800.0, 600.0);
    let hit = folio_gable_engine::geom::pick(&m.ents, &proj, 400.0, 300.0, 6.0);
    let (r, p) = hit.expect("something under the middle of the view");
    assert!(r.valid(), "it found a thing");
    assert!(p.x > -1.0 && p.x < 1001.0, "and a point on the box: {p:?}");
}

#[test]
fn a_model_comes_back_as_it_was_saved() {
    let mut m = Model::new();
    solids::add_box(&mut m.ents, V3::default(), v3(1000.0, 2000.0, 500.0));
    let red = m.add_material("Red", 0xCC2020);
    m.ents.faces[0].mat_front = red as i32;
    let tmp = folio_kit::platform::temp_dir().join(format!("gable-test-{}.gable", std::process::id()));
    folio_gable_engine::store::save(&m, &tmp).expect("save");
    let back = folio_gable_engine::store::open(&tmp).expect("open");
    assert_eq!(back.ents.faces.iter().filter(|f| f.alive).count(), 6, "six faces came back");
    assert_eq!(back.ents.verts.iter().filter(|v| v.alive).count(), 8, "eight corners");
    assert_eq!(back.materials.len(), 1, "the material came back");
    assert_eq!(back.ents.faces[0].mat_front, red as i32, "painted as it was");
    let v = solids::volume(&back.ents);
    assert!((v - 1000.0 * 2000.0 * 500.0).abs() < 1.0, "and holds its shape: {v}");
    // the edges and planes were worked out again on the way in
    assert!(back.ents.verts[0].edges.len() >= 3, "a corner knows its edges");
    assert!(back.ents.faces[0].plane.n.len() > 0.9, "a face knows its plane");
    let obj = tmp.with_extension("obj");
    folio_gable_engine::store::export_obj(&back, &obj).expect("obj");
    let text = std::fs::read_to_string(&obj).unwrap();
    assert!(text.lines().filter(|l| l.starts_with("f ")).count() == 6, "the .obj has the six faces");
    let stl = tmp.with_extension("stl");
    folio_gable_engine::store::export_stl(&back, &stl).expect("stl");
    let text = std::fs::read_to_string(&stl).unwrap();
    assert!(text.matches("facet normal").count() >= 12, "the .stl has the triangles");
    let _ = std::fs::remove_file(&tmp);
    let _ = std::fs::remove_file(&obj);
    let _ = std::fs::remove_file(&stl);
}
