//! In-window UI test (`folio-cork --ui-test`): drives the real window with clicks and keys on
//! X11 and Wayland alike, checks the results, saves screenshots, and exits 0 when all pass.

use crate::app::{CorkApp, Res};
use crate::editor::Tool;
use folio_kit::gfx::{self, sk};
use folio_kit::platform::{Button, Event, Host, Key, Mods};
use std::path::PathBuf;

type Step = Box<dyn FnMut(&mut CorkApp, &mut dyn Host, &mut Res) -> Result<(), String>>;

/// Where a step saves a screenshot of its own (set when the test starts).
static SHOTS: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

fn save_shot(app: &mut CorkApp, r: &mut Res, name: &str) {
    let dir = SHOTS.lock().ok().and_then(|d| d.clone());
    if let Some(dir) = dir {
        let pm = shot_into(app, r);
        let _ = std::fs::write(dir.join(name), gfx::encode_png(&pm));
    }
}

pub struct UiTest {
    steps: Vec<(String, Step)>,
    i: usize,
    pub done: bool,
    pub fails: Vec<String>,
    pub passed: usize,
    dir: PathBuf,
}

fn ev(app: &mut CorkApp, host: &mut dyn Host, r: &mut Res, e: Event) {
    app.handle_test(&e, host, r);
}

fn click(app: &mut CorkApp, host: &mut dyn Host, r: &mut Res, x: f32, y: f32, clicks: u32) {
    ev(app, host, r, Event::MouseMove { x, y });
    ev(app, host, r, Event::MouseDown { x, y, button: Button::Left, clicks });
    ev(app, host, r, Event::MouseUp { x, y, button: Button::Left });
}

fn drag(app: &mut CorkApp, host: &mut dyn Host, r: &mut Res, a: (f32, f32), b: (f32, f32)) {
    ev(app, host, r, Event::MouseMove { x: a.0, y: a.1 });
    ev(app, host, r, Event::MouseDown { x: a.0, y: a.1, button: Button::Left, clicks: 1 });
    for i in 1..=8 {
        let t = i as f32 / 8.0;
        ev(app, host, r, Event::MouseMove { x: a.0 + (b.0 - a.0) * t, y: a.1 + (b.1 - a.1) * t });
    }
    ev(app, host, r, Event::MouseUp { x: b.0, y: b.1, button: Button::Left });
}

fn key(app: &mut CorkApp, host: &mut dyn Host, r: &mut Res, k: Key, mods: Mods) {
    ev(app, host, r, Event::Modifiers(mods));
    ev(app, host, r, Event::Key { key: k.clone(), mods, down: true, repeat: false });
    ev(app, host, r, Event::Key { key: k, mods, down: false, repeat: false });
    ev(app, host, r, Event::Modifiers(Mods::NONE));
}

fn ctrl() -> Mods {
    Mods { ctrl: true, ..Mods::NONE }
}

fn typ(app: &mut CorkApp, host: &mut dyn Host, r: &mut Res, t: &str) {
    ev(app, host, r, Event::Text(t.to_string()));
}

/// Screen point of a page point.
fn sp(app: &CorkApp, p: (f32, f32)) -> (f32, f32) {
    let v = app.ed.view();
    let (x, y) = v.to_screen(p.0, p.1);
    (x + app.canvas.x, y + app.canvas.y)
}

fn ribbon_click(app: &mut CorkApp, host: &mut dyn Host, r: &mut Res, tab: &str, id: &str) -> Result<(), String> {
    app.ribbon.select_tab(tab);
    shot_into(app, r);
    let rr = app.ribbon.rect_of(id).ok_or(format!("ribbon button {id} not shown"))?;
    click(app, host, r, rr.cx(), rr.cy(), 1);
    Ok(())
}

/// Paint the window so hit areas are current.
fn shot_into(app: &mut CorkApp, r: &mut Res) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(app.w as u32, app.h as u32).unwrap();
    app.paint(&mut pm, r);
    pm
}

fn check(c: bool, msg: &str) -> Result<(), String> {
    if c {
        Ok(())
    } else {
        Err(msg.to_string())
    }
}

impl UiTest {
    pub fn new(dir: PathBuf) -> UiTest {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut g) = SHOTS.lock() {
            *g = Some(dir.clone());
        }
        let mut steps: Vec<(String, Step)> = vec![];
        macro_rules! step {
            ($name:expr, $f:expr) => {
                steps.push(($name.to_string(), Box::new($f)));
            };
        }
        step!("window shows the ribbon and board", |app: &mut CorkApp, _h: &mut dyn Host, r: &mut Res| {
            let pm = shot_into(app, r);
            check(app.canvas.w > 200.0 && app.canvas.h > 150.0, "board area too small")?;
            // Title bar is Cork's pale yellow.
            let px = pm.pixel(app.w as u32 / 2, 4).ok_or("no pixel")?;
            check(px.red() > 200 && px.green() > 190 && px.blue() < 200, &format!("title bar colour {:?}", (px.red(), px.green(), px.blue())))
        });
        step!("rectangle from the ribbon", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "insert", "tool.shape.rect")?;
            check(app.ed.tool == Tool::Shape("rect".into()), "rectangle tool not active")?;
            let p = sp(app, (200.0, 200.0));
            click(app, h, r, p.0, p.1, 1);
            check(app.ed.pg().shapes.iter().any(|s| s.kind == "rect"), "no rectangle added")?;
            check(app.ed.tool == Tool::Pointer, "tool didn't return to the pointer")
        });
        step!("typing on a selected shape", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            typ(app, h, r, "Start");
            key(app, h, r, Key::Escape, Mods::NONE);
            let s = app.ed.pg().shapes.iter().find(|s| s.kind == "rect").ok_or("rect gone")?;
            check(s.text == "Start", &format!("text was {:?}", s.text))
        });
        step!("ellipse by dragging", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "insert", "tool.shape.ellipse")?;
            let a = sp(app, (420.0, 320.0));
            let b = sp(app, (540.0, 400.0));
            drag(app, h, r, a, b);
            let e = app.ed.pg().shapes.iter().find(|s| s.kind == "ellipse").ok_or("no ellipse")?;
            check((e.w - 120.0).abs() < 13.0 && (e.h - 80.0).abs() < 13.0, &format!("ellipse size {}x{}", e.w, e.h))
        });
        step!("connector glued between shapes", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "tool.connector")?;
            let (ra, rb) = {
                let pg = app.ed.pg();
                let a = pg.shapes.iter().find(|s| s.kind == "rect").unwrap();
                let b = pg.shapes.iter().find(|s| s.kind == "ellipse").unwrap();
                ((a.cx(), a.cy()), (b.cx(), b.cy()))
            };
            let (a, b) = (sp(app, ra), sp(app, rb));
            drag(app, h, r, a, b);
            key(app, h, r, Key::Escape, Mods::NONE);
            let c = app.ed.pg().shapes.iter().find(|s| s.is_connector()).ok_or("no connector")?;
            let cn = c.conn.as_ref().unwrap();
            check(cn.a.shape.is_some() && cn.b.shape.is_some(), "connector ends not glued")
        });
        step!("moving a shape keeps the connector attached", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            let (c, id) = {
                let s = app.ed.pg().shapes.iter().find(|s| s.kind == "ellipse").unwrap();
                ((s.cx(), s.cy()), s.id)
            };
            let a = sp(app, c);
            let b = sp(app, (c.0 + 60.0, c.1 + 120.0));
            drag(app, h, r, a, b);
            let s = app.ed.pg().get(id).unwrap().clone();
            check((s.cy() - c.1 - 120.0).abs() < 13.0, &format!("ellipse didn't move ({})", s.cy() - c.1))?;
            let con = app.ed.pg().shapes.iter().find(|s| s.is_connector()).unwrap().clone();
            let route = crate::route::route(app.ed.pg(), &con);
            let end = *route.pts.last().unwrap();
            let pts = crate::geom::conn_points(&s);
            let near = pts.iter().any(|p| (p.0 - end.0).abs() < 2.0 && (p.1 - end.1).abs() < 2.0);
            check(near, "connector end didn't follow the shape")
        });
        step!("undo and redo", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            let y = |app: &CorkApp| app.ed.pg().shapes.iter().find(|s| s.kind == "ellipse").map(|s| s.y).unwrap_or(0.0);
            let moved = y(app);
            key(app, h, r, Key::Char('z'), ctrl());
            let back = y(app);
            key(app, h, r, Key::Char('y'), ctrl());
            check((moved - back).abs() > 50.0 && (y(app) - moved).abs() < 0.1, "undo/redo didn't restore the move")
        });
        step!("sticky note and its text", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "insert", "tool.sticky")?;
            let p = sp(app, (720.0, 160.0));
            click(app, h, r, p.0, p.1, 1);
            typ(app, h, r, "Idea one");
            key(app, h, r, Key::Escape, Mods::NONE);
            check(app.ed.pg().shapes.iter().any(|s| s.kind == "sticky" && s.text == "Idea one"), "sticky not written")
        });
        step!("pen stroke", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "insert", "tool.pen")?;
            let a = sp(app, (100.0, 520.0));
            let b = sp(app, (260.0, 560.0));
            drag(app, h, r, a, b);
            key(app, h, r, Key::Escape, Mods::NONE);
            check(app.ed.pg().shapes.iter().any(|s| s.kind == "ink" && s.points.len() > 3), "no ink")
        });
        step!("group and ungroup", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            let ids: Vec<u64> = app.ed.pg().shapes.iter().filter(|s| s.kind == "rect" || s.kind == "ellipse").map(|s| s.id).collect();
            app.ed.sel = ids;
            key(app, h, r, Key::Char('g'), ctrl());
            let g = app.ed.pg().shapes.iter().any(|s| s.is_group());
            key(app, h, r, Key::Char('g'), Mods { ctrl: true, shift: true, ..Mods::NONE });
            check(g && !app.ed.pg().shapes.iter().any(|s| s.is_group()), "grouping failed")
        });
        step!("copy leaves a diagram picture for the other apps", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            let ids: Vec<u64> = app.ed.pg().shapes.iter().filter(|s| s.kind == "rect" || s.kind == "ellipse").map(|s| s.id).collect();
            app.ed.sel = ids.clone();
            let f = folio_kit::platform::config_dir("folio-cork").join("clipboard.png");
            let _ = std::fs::remove_file(&f);
            key(app, h, r, Key::Char('c'), ctrl());
            let png = std::fs::read(&f).map_err(|_| "no clipboard.png after Copy".to_string())?;
            let board = crate::files::board_in_png(&png).ok_or("clipboard.png carries no drawing")?;
            let doc = crate::vsdx::read(&board)?;
            check(doc.pages[0].shapes.len() >= ids.len(), &format!("{} shapes in the copied diagram", doc.pages[0].shapes.len()))
        });
        step!("align from the Arrange tab", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            let ids: Vec<u64> = app.ed.pg().shapes.iter().filter(|s| s.kind == "rect" || s.kind == "sticky").map(|s| s.id).collect();
            app.ed.sel = ids.clone();
            ribbon_click(app, h, r, "arrange", "align.top")?;
            let ys: Vec<f32> = ids.iter().map(|i| app.ed.pg().get(*i).unwrap().y).collect();
            check((ys[0] - ys[1]).abs() < 0.5, &format!("tops not aligned {ys:?}"))
        });
        step!("fill colour from the palette", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            let id = app.ed.pg().shapes.iter().find(|s| s.kind == "rect").unwrap().id;
            app.ed.sel = vec![id];
            app.ribbon.select_tab("home");
            shot_into(app, r);
            let rr = app.ribbon.rect_of("fill").ok_or("fill not shown")?;
            // The split button's arrow is at its right edge.
            click(app, h, r, rr.right() + 7.0 * app.s, rr.cy(), 1);
            check(app.menu.is_some(), "colour menu didn't open")?;
            shot_into(app, r);
            let m = app.menu.as_ref().unwrap().rect;
            // First swatch of the grid (white), second (black): pick the 5th (blue).
            click(app, h, r, m.x + 8.0 * app.s + 4.5 * 26.0 * app.s, m.y + 6.0 * app.s + 11.0 * app.s, 1);
            let c = app.ed.pg().get(id).unwrap().style.fill;
            check(c == folio_kit::Color::hex(0x4472C4), &format!("fill is {}", c.to_hex()))
        });
        step!("comment pinned and written", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "insert", "tool.comment")?;
            let p = sp(app, (600.0, 500.0));
            click(app, h, r, p.0, p.1, 1);
            shot_into(app, r);
            typ(app, h, r, "Check this");
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.ed.pg().comments.iter().any(|c| c.text == "Check this"), "comment not saved")
        });
        step!("layers: new layer, hide it", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "arrange", "layer-new")?;
            let lid = app.ed.active_layer();
            let s = app.ed.make_shape("rect", 900.0, 600.0);
            let sid = app.ed.add_shape(s, "x");
            app.ed.layer_toggle(lid, "visible");
            let hidden = app.ed.hit((900.0, 600.0), false) != Some(sid);
            app.ed.layer_toggle(lid, "visible");
            check(hidden, "hidden layer still clickable")
        });
        step!("template, votes and results", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            app.ed.add_page(false);
            crate::templates::apply("brainstorm", &mut app.ed);
            app.fit();
            ribbon_click(app, h, r, "board", "tool.vote")?;
            let s = app.ed.pg().shapes.iter().find(|s| s.kind == "sticky").unwrap().clone();
            let p = sp(app, (s.cx(), s.cy()));
            click(app, h, r, p.0, p.1, 1);
            click(app, h, r, p.0, p.1, 1);
            key(app, h, r, Key::Escape, Mods::NONE);
            check(app.ed.pg().get(s.id).unwrap().votes == 2, "votes not counted")?;
            check(crate::mood::vote_results(&app.ed).contains("2 votes"), "results wrong")
        });
        step!("swimlanes: add, fill, move and turn", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            app.ed.add_page(false);
            ribbon_click(app, h, r, "insert", "lane.add")?;
            ribbon_click(app, h, r, "insert", "lane.add")?;
            check(app.ed.lanes().len() == 2, &format!("{} lanes", app.ed.lanes().len()))?;
            let lane = app.ed.lanes()[1].clone();
            let (bx, by, bw, bh) = lane.lane_body();
            let s = app.ed.make_shape("rect", bx + bw / 2.0, by + bh / 2.0);
            let sid = app.ed.add_shape(s, "in the lane");
            check(app.ed.inside_lane(lane.id) == vec![sid], "the shape isn't in the lane")?;
            // dragging the lane's band carries the shape with it
            app.fit();
            let before = app.ed.pg().get(sid).unwrap().y;
            let from = sp(app, (lane.x + 8.0, lane.cy()));
            let to = sp(app, (lane.x + 8.0, lane.cy() + 120.0));
            drag(app, h, r, from, to);
            let moved = app.ed.pg().get(sid).unwrap().y - before;
            check(moved > 60.0, &format!("the shape moved {moved} with its lane"))?;
            ribbon_click(app, h, r, "insert", "lane.turn")?;
            check(app.ed.lanes().iter().all(|l| l.lane_vertical()), "the lanes didn't turn")?;
            check(app.ed.inside_lane(app.ed.lanes()[1].id) == vec![sid], "the shape left its lane when turned")?;
            ribbon_click(app, h, r, "insert", "lane.delete")?;
            check(app.ed.lanes().len() == 1 && app.ed.pg().get(sid).is_some(), "deleting the lane took its shape")?;
            let _ = bh;
            Ok(())
        });
        step!("shape data on a shape, in the pane and in the file", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "view", "pane.data")?;
            shot_into(app, r);
            let rect = app.ed.pg().shapes.iter().find(|s| s.kind == "rect").map(|s| (s.cx(), s.cy())).ok_or("no rect on this page")?;
            let p = sp(app, rect);
            click(app, h, r, p.0, p.1, 1);
            shot_into(app, r);
            let add = app.side_hits.iter().find(|(id, _)| id == "prop-add").map(|(_, r2)| *r2).ok_or("no Add Property button")?;
            click(app, h, r, add.cx(), add.cy(), 1);
            let d = app.dialog.as_mut().ok_or("no shape data dialog")?;
            d.set_text_for_test("name", "Part number");
            d.set_text_for_test("value", "A-120");
            key(app, h, r, Key::Enter, Mods::NONE);      // Enter is the dialog's first button, OK
            let s = app.ed.first().ok_or("nothing selected")?;
            check(s.prop("Part number") == "A-120", &format!("shape data is {:?}", s.props()))?;
            save_shot(app, r, "ui-shape-data.png");
            let del = app.side_hits.iter().find(|(id, _)| id == "prop:Part number:delete").map(|(_, r2)| *r2).ok_or("no delete button for the row")?;
            click(app, h, r, del.cx(), del.cy(), 1);
            check(app.ed.first().map(|s| s.props().is_empty()).unwrap_or(false), "the property is still there")
        });
        step!("mood board: picture, palette, tidy", |app: &mut CorkApp, _h: &mut dyn Host, r: &mut Res| {
            app.ed.add_page(true);
            crate::templates::apply("mood", &mut app.ed);
            app.fit();
            let mut pm = sk::Pixmap::new(120, 80).unwrap();
            pm.fill(sk::Color::from_rgba8(200, 40, 40, 255));
            gfx::fill_rect(&mut pm, gfx::Rect::new(60.0, 0.0, 60.0, 80.0), folio_kit::Color::rgb(30, 60, 200));
            let png = gfx::encode_png(&pm);
            let id = crate::mood::insert_picture(&mut app.ed, png, (300.0, 200.0), &mut r.images).ok_or("picture not inserted")?;
            app.ed.sel = vec![id];
            let n = crate::mood::extract_palette(&mut app.ed, 3);
            let sw: Vec<folio_kit::Color> = app.ed.selected().iter().map(|s| s.style.fill).collect();
            check(n == 1 && sw.iter().any(|c| c.r > 150 && c.b < 90) && sw.iter().any(|c| c.b > 150 && c.r < 90), &format!("palette {:?}", sw.iter().map(|c| c.to_hex()).collect::<Vec<_>>()))?;
            app.ed.sel.clear();
            crate::mood::tidy(&mut app.ed, true);
            Ok(())
        });
        step!("re-layout as a tree", |app: &mut CorkApp, _h: &mut dyn Host, r: &mut Res| {
            app.ed.add_page(false);
            crate::templates::apply("orgchart", &mut app.ed);
            // Scramble, then lay out.
            for (i, s) in app.ed.pgm().shapes.iter_mut().filter(|s| !s.is_connector()).enumerate() {
                s.x = 50.0 + i as f32 * 13.0;
                s.y = 400.0 - i as f32 * 40.0;
            }
            crate::layout::relayout(&mut app.ed, "tree", &mut r.text);
            let pg = app.ed.pg();
            let top = pg.shapes.iter().find(|s| s.kind == "org.manager").unwrap();
            let kids: Vec<&crate::model::Shape> = pg.shapes.iter().filter(|s| s.kind == "org.person").collect();
            check(kids.iter().all(|k| k.y > top.y + top.h), "children not below the manager")
        });
        step!("save as .vsdx through File > Save As, then reopen", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            app.ed.go_page(0);
            key(app, h, r, Key::Char('s'), Mods { ctrl: true, shift: true, ..Mods::NONE });
            check(app.backstage.is_some(), "Save As didn't open")?;
            let dir = folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into());
            let path = PathBuf::from(&dir).join("ui-test.vsdx");
            let _ = std::fs::remove_file(&path);
            if let Some(b) = app.backstage.as_mut() {
                if let Some(f) = b.files.as_mut() {
                    f.name.set(path.to_str().unwrap());
                    f.name.focused = true;
                }
            }
            shot_into(app, r);
            key(app, h, r, Key::Enter, Mods::NONE);
            check(path.exists(), "file not written")?;
            check(!app.ed.dirty, "still marked unsaved")?;
            let doc = crate::files::load(&path)?;
            check(doc.pages.len() == app.ed.doc.pages.len(), "page count differs after reopening")?;
            check(doc.pages[0].shapes.len() == app.ed.doc.pages[0].shapes.len(), "shape count differs after reopening")
        });
        step!("export PNG, SVG and PDF", |app: &mut CorkApp, _h: &mut dyn Host, r: &mut Res| {
            let dir = PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()));
            crate::export::png(&app.ed.doc, &[0], &dir.join("ui-test.png"), &mut r.text, &mut r.images, 1.0)?;
            crate::export::svg(&app.ed.doc, &[0], &dir.join("ui-test.svg"), &mut r.text)?;
            crate::export::pdf(&app.ed.doc, &[0, 1, 2], &dir.join("ui-test.pdf"), &mut r.text, &mut r.images, true, false)?;
            let pdf = std::fs::read(dir.join("ui-test.pdf")).map_err(|e| e.to_string())?;
            check(pdf.starts_with(b"%PDF") && pdf.len() > 1000, "PDF not written")?;
            let svg = std::fs::read_to_string(dir.join("ui-test.svg")).map_err(|e| e.to_string())?;
            check(folio_kit::icons::render_svg(&svg, 200, 120).is_some(), "SVG doesn't load")
        });
        step!("dark theme", |app: &mut CorkApp, h: &mut dyn Host, r: &mut Res| {
            app.settings.theme = "dark".into();
            let mut st = app.settings.clone();
            st.theme = "dark".into();
            app.command("zoom.fit", h, r);
            app.theme = folio_kit::theme::Theme::new(true, folio_kit::theme::app_colors::CORK, crate::app::ACCENT);
            let pm = shot_into(app, r);
            let dir = PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()));
            let _ = std::fs::write(dir.join("ui-dark.png"), gfx::encode_png(&pm));
            // An empty part of the ribbon, right of the last group.
            let px = pm.pixel(app.w as u32 - 60, (app.ribbon.rect.y + 60.0) as u32).ok_or("no pixel")?;
            check(px.red() < 80, "ribbon not dark")
        });
        UiTest { steps, i: 0, done: false, fails: vec![], passed: 0, dir }
    }

    pub fn step(&mut self, app: &mut CorkApp, host: &mut dyn Host, r: &mut Res) {
        if self.i >= self.steps.len() {
            let pm = shot_into(app, r);
            let _ = std::fs::write(self.dir.join("ui-final.png"), gfx::encode_png(&pm));
            println!("{} passed, {} failed", self.passed, self.fails.len());
            for f in &self.fails {
                println!("FAIL {f}");
            }
            self.done = true;
            return;
        }
        let (name, f) = &mut self.steps[self.i];
        let name = name.clone();
        match f(app, host, r) {
            Ok(()) => {
                self.passed += 1;
                println!("ok   {name}");
            }
            Err(e) => {
                println!("FAIL {name}: {e}");
                self.fails.push(format!("{name}: {e}"));
            }
        }
        if self.i == 6 {
            let pm = shot_into(app, r);
            let _ = std::fs::write(self.dir.join("ui-shapes.png"), gfx::encode_png(&pm));
        }
        if name.starts_with("swimlanes") {
            let pm = shot_into(app, r);
            let _ = std::fs::write(self.dir.join("ui-swimlanes.png"), gfx::encode_png(&pm));
        }
        self.i += 1;
    }

    pub fn code(&self) -> i32 {
        if self.fails.is_empty() { 0 } else { 1 }
    }
}
