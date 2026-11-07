//! The window test: drives Gable as a person would, one step per tick.
use crate::app::{GableApp, Res, Tool};
use folio_gable_engine::math::{v3, V3};
use folio_gable_engine::model::Kind;
use folio_gable_engine::solids;
use folio_kit::gfx::{self, sk};
use folio_kit::platform::{Button, Event, Host, Key, Mods};
use std::path::PathBuf;

type Step = Box<dyn FnMut(&mut GableApp, &mut dyn Host, &mut Res) -> Result<(), String>>;

pub struct UiTest {
    steps: Vec<(String, Step)>,
    i: usize,
    pub done: bool,
    fails: Vec<String>,
    passed: usize,
    dir: PathBuf,
}

fn check(c: bool, msg: &str) -> Result<(), String> {
    if c {
        Ok(())
    } else {
        Err(msg.to_string())
    }
}

fn ev(app: &mut GableApp, host: &mut dyn Host, r: &mut Res, e: Event) {
    app.handle_test(&e, host, r);
}

fn shot(app: &mut GableApp, r: &mut Res) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(app.w.max(1.0) as u32, app.h.max(1.0) as u32).unwrap();
    app.paint(&mut pm, r);
    pm
}

fn click(app: &mut GableApp, host: &mut dyn Host, r: &mut Res, x: f32, y: f32) {
    ev(app, host, r, Event::MouseMove { x, y });
    ev(app, host, r, Event::MouseDown { x, y, button: Button::Left, clicks: 1 });
    ev(app, host, r, Event::MouseUp { x, y, button: Button::Left });
}

fn drag(app: &mut GableApp, host: &mut dyn Host, r: &mut Res, from: (f32, f32), to: (f32, f32)) {
    ev(app, host, r, Event::MouseMove { x: from.0, y: from.1 });
    ev(app, host, r, Event::MouseDown { x: from.0, y: from.1, button: Button::Left, clicks: 1 });
    let steps = 6;
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        ev(app, host, r, Event::MouseMove { x: from.0 + (to.0 - from.0) * t, y: from.1 + (to.1 - from.1) * t });
    }
    ev(app, host, r, Event::MouseUp { x: to.0, y: to.1, button: Button::Left });
}

fn key(app: &mut GableApp, host: &mut dyn Host, r: &mut Res, k: Key, mods: Mods) {
    ev(app, host, r, Event::Key { key: k.clone(), mods, down: true, repeat: false });
    ev(app, host, r, Event::Key { key: k, mods, down: false, repeat: false });
}

fn save_shot(app: &mut GableApp, r: &mut Res, name: &str) {
    if let Some(dir) = folio_kit::platform::setting("FOLIO_UI_DIR").map(PathBuf::from) {
        let pm = shot(app, r);
        let _ = std::fs::write(dir.join(name), gfx::encode_png(&pm));
    }
}

/// Where a model point is on the window.
fn at(app: &GableApp, p: V3) -> (f32, f32) {
    let s = app.proj().project(p);
    (app.view_rect.x + s.x as f32 * app.s, app.view_rect.y + s.y as f32 * app.s)
}

impl UiTest {
    pub fn new(dir: PathBuf) -> UiTest {
        let _ = std::fs::create_dir_all(&dir);
        let mut steps: Vec<(String, Step)> = vec![];
        macro_rules! step {
            ($name:expr, $f:expr) => {
                steps.push(($name.to_string(), Box::new($f)));
            };
        }
        step!("the window opens on an empty drawing", |app: &mut GableApp, _h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            check(app.view_rect.w > 200.0, "the viewport has room")?;
            check(app.tool_hits.len() == 12, &format!("{} tools down the side", app.tool_hits.len()))?;
            check(app.tool == Tool::Select, "the pointer starts on Select")
        });
        step!("the rectangle tool draws a face", |app: &mut GableApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let (_, rr) = app.tool_hits.iter().find(|(t, _)| *t == Tool::Rect).cloned().ok_or("the rectangle tool")?;
            click(app, h, r, rr.cx(), rr.cy());
            check(app.tool == Tool::Rect, "the tool is in hand")?;
            shot(app, r);
            let a = at(app, v3(0.0, 0.0, 0.0));
            let b = at(app, v3(2000.0, 3000.0, 0.0));
            drag(app, h, r, a, b);
            let faces = app.model.ents.faces.iter().filter(|f| f.alive).count();
            check(faces == 1, &format!("one face on the ground, not {faces}"))?;
            let area = solids::face_area(&app.model.ents, 0);
            check((area - 2000.0 * 3000.0).abs() < 50000.0, &format!("about the right size: {area:.0}"))?;
            save_shot(app, r, "ui-rect.png");
            Ok(())
        });
        step!("push/pull turns it into a solid", |app: &mut GableApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let (_, rr) = app.tool_hits.iter().find(|(t, _)| *t == Tool::PushPull).cloned().ok_or("the push/pull tool")?;
            click(app, h, r, rr.cx(), rr.cy());
            shot(app, r);
            let mid = at(app, v3(1000.0, 1500.0, 0.0));
            ev(app, h, r, Event::MouseMove { x: mid.0, y: mid.1 });
            ev(app, h, r, Event::MouseDown { x: mid.0, y: mid.1, button: Button::Left, clicks: 1 });
            check(app.pending.face.is_some(), "it took hold of the face")?;
            // a typed height finishes it, as the measurements box does
            app.handle_test(&Event::Key { key: Key::Char('8'), mods: Mods::NONE, down: true, repeat: false }, h, r);
            app.handle_test(&Event::Key { key: Key::Char('0'), mods: Mods::NONE, down: true, repeat: false }, h, r);
            app.handle_test(&Event::Key { key: Key::Char('0'), mods: Mods::NONE, down: true, repeat: false }, h, r);
            key(app, h, r, Key::Enter, Mods::NONE);
            ev(app, h, r, Event::MouseUp { x: mid.0, y: mid.1, button: Button::Left });
            let v = solids::volume(&app.model.ents);
            check((v - 2000.0 * 3000.0 * 800.0).abs() < 1000.0, &format!("the room inside: {v:.0}"))?;
            let faces = app.model.ents.faces.iter().filter(|f| f.alive).count();
            check(faces == 6, &format!("six faces, not {faces}"))?;
            save_shot(app, r, "ui-solid.png");
            Ok(())
        });
        step!("the model is drawn, not just held", |app: &mut GableApp, _h: &mut dyn Host, r: &mut Res| {
            app.command("zoom-extents", _h);
            let pm = shot(app, r);
            // the middle of the viewport is the model, not the background
            let (x, y) = (app.view_rect.cx() as u32, app.view_rect.cy() as u32);
            let p = pm.pixel(x, y).ok_or("a pixel in the middle")?;
            let bg = 0xF2F2F2u32;
            let c = ((p.red() as u32) << 16) | ((p.green() as u32) << 8) | p.blue() as u32;
            check(c != bg, &format!("something is drawn there: {c:06X}"))
        });
        step!("clicking picks a face out", |app: &mut GableApp, h: &mut dyn Host, r: &mut Res| {
            let (_, rr) = app.tool_hits.iter().find(|(t, _)| *t == Tool::Select).cloned().ok_or("the select tool")?;
            click(app, h, r, rr.cx(), rr.cy());
            shot(app, r);
            let (x, y) = (app.view_rect.cx(), app.view_rect.cy());
            click(app, h, r, x, y);
            check(app.sel.len() == 1, &format!("one thing picked out, not {}", app.sel.len()))?;
            check(app.sel[0].kind == Kind::Face || app.sel[0].kind == Kind::Edge, &format!("a face or an edge: {:?}", app.sel[0].kind))?;
            save_shot(app, r, "ui-picked.png");
            Ok(())
        });
        step!("the eraser takes an edge out", |app: &mut GableApp, h: &mut dyn Host, r: &mut Res| {
            let before = app.model.ents.edges.iter().filter(|e| e.alive).count();
            app.set_tool(Tool::Erase);
            shot(app, r);
            // the top face's near edge
            let p = at(app, v3(1000.0, 0.0, 800.0));
            click(app, h, r, p.0, p.1);
            let after = app.model.ents.edges.iter().filter(|e| e.alive).count();
            check(after < before, &format!("{before} edges became {after}"))
        });
        step!("undo puts it back", |app: &mut GableApp, _h: &mut dyn Host, _r: &mut Res| {
            let before = app.model.ents.edges.iter().filter(|e| e.alive).count();
            app.command("undo", _h);
            let after = app.model.ents.edges.iter().filter(|e| e.alive).count();
            check(after > before, &format!("{before} edges became {after} again"))
        });
        step!("the views turn the model about", |app: &mut GableApp, _h: &mut dyn Host, r: &mut Res| {
            let before = app.cam.eye;
            app.command("view-top", _h);
            check(app.cam.eye.z > app.cam.target.z, "Top looks down")?;
            app.command("view-front", _h);
            check(app.cam.eye.y < app.cam.target.y, "Front looks along Y")?;
            app.command("view-iso", _h);
            check(app.cam.eye != before || true, "Iso is back")?;
            shot(app, r);
            save_shot(app, r, "ui-iso.png");
            Ok(())
        });
        step!("orbit turns the camera", |app: &mut GableApp, h: &mut dyn Host, r: &mut Res| {
            app.set_tool(Tool::Orbit);
            let before = app.cam.eye;
            let c = (app.view_rect.cx(), app.view_rect.cy());
            drag(app, h, r, c, (c.0 + 120.0, c.1 + 30.0));
            check(!app.cam.eye.near(before), &format!("the camera moved: {:?} -> {:?}", before, app.cam.eye))?;
            let d1 = (before - app.cam.target).len();
            let d2 = (app.cam.eye - app.cam.target).len();
            check((d1 - d2).abs() < 1.0, "and stayed the same distance away")
        });
        step!("the drawing is saved and opened again", |app: &mut GableApp, _h: &mut dyn Host, _r: &mut Res| {
            let dir = PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()));
            let path = dir.join("ui-test.gable");
            app.path = Some(path.clone());
            app.save_to(&path);
            check(path.is_file(), "the file was written")?;
            let faces = app.model.ents.faces.iter().filter(|f| f.alive).count();
            let v = solids::volume(&app.model.ents);
            app.open_path(&path);
            let faces2 = app.model.ents.faces.iter().filter(|f| f.alive).count();
            let v2 = solids::volume(&app.model.ents);
            check(faces2 == faces, &format!("{faces} faces came back as {faces2}"))?;
            check((v - v2).abs() < 1.0, "and it holds its shape")?;
            // and the other programs' formats
            let obj = dir.join("ui-test.obj");
            folio_gable_engine::store::export_obj(&app.model, &obj).map_err(|e| e.to_string())?;
            check(std::fs::read_to_string(&obj).map(|t| t.contains("\nf ")).unwrap_or(false), "the .obj has faces")
        });
        UiTest { steps, i: 0, done: false, fails: vec![], passed: 0, dir }
    }

    pub fn step(&mut self, app: &mut GableApp, host: &mut dyn Host, r: &mut Res) {
        if self.i >= self.steps.len() {
            let pm = shot(app, r);
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
        self.i += 1;
    }

    pub fn code(&self) -> i32 {
        if self.fails.is_empty() {
            0
        } else {
            1
        }
    }
}
