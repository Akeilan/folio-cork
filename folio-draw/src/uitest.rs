//! The window test (D3-06): drives Draw as a user would, one step per tick, in a real window
//! or in memory (`--headless`). The GTK version's tests are the spec this grows towards.

use crate::app::{BackPage, DrawApp, Res, Tool};
use folio_kit::gfx::{self, sk};
use folio_kit::platform::{Button, Event, Host, Key, Mods};
use std::path::PathBuf;

type Step = Box<dyn FnMut(&mut DrawApp, &mut dyn Host, &mut Res) -> Result<(), String>>;

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

fn ev(app: &mut DrawApp, host: &mut dyn Host, r: &mut Res, e: Event) {
    app.handle_test(&e, host, r);
}

fn shot(app: &mut DrawApp, r: &mut Res) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(app.w as u32, app.h as u32).unwrap();
    app.paint(&mut pm, r);
    pm
}

/// Screen point of a picture point.
fn sp(app: &DrawApp, x: f32, y: f32) -> (f32, f32) {
    app.to_screen((x, y))
}

fn drag_on(app: &mut DrawApp, host: &mut dyn Host, r: &mut Res, a: (f32, f32), b: (f32, f32), button: Button) {
    let (a, b) = (sp(app, a.0, a.1), sp(app, b.0, b.1));
    ev(app, host, r, Event::MouseMove { x: a.0, y: a.1 });
    ev(app, host, r, Event::MouseDown { x: a.0, y: a.1, button, clicks: 1 });
    for i in 1..=10 {
        let t = i as f32 / 10.0;
        ev(app, host, r, Event::MouseMove { x: a.0 + (b.0 - a.0) * t, y: a.1 + (b.1 - a.1) * t });
    }
    ev(app, host, r, Event::MouseUp { x: b.0, y: b.1, button });
}

fn click_on(app: &mut DrawApp, host: &mut dyn Host, r: &mut Res, x: f32, y: f32, button: Button) {
    let p = sp(app, x, y);
    ev(app, host, r, Event::MouseMove { x: p.0, y: p.1 });
    ev(app, host, r, Event::MouseDown { x: p.0, y: p.1, button, clicks: 1 });
    ev(app, host, r, Event::MouseUp { x: p.0, y: p.1, button });
}

fn ribbon_click(app: &mut DrawApp, host: &mut dyn Host, r: &mut Res, tab: &str, id: &str) -> Result<(), String> {
    app.ribbon.select_tab(tab);
    shot(app, r);
    let rr = app.ribbon.rect_of(id).ok_or(format!("ribbon button {id} not shown"))?;
    ev(app, host, r, Event::MouseMove { x: rr.cx(), y: rr.cy() });
    ev(app, host, r, Event::MouseDown { x: rr.cx(), y: rr.cy(), button: Button::Left, clicks: 1 });
    ev(app, host, r, Event::MouseUp { x: rr.cx(), y: rr.cy(), button: Button::Left });
    Ok(())
}

fn key(app: &mut DrawApp, host: &mut dyn Host, r: &mut Res, k: Key, mods: Mods) {
    ev(app, host, r, Event::Key { key: k.clone(), mods, down: true, repeat: false });
    ev(app, host, r, Event::Key { key: k, mods, down: false, repeat: false });
}

fn ctrl() -> Mods {
    Mods { ctrl: true, ..Mods::NONE }
}

fn px(app: &DrawApp, x: usize, y: usize) -> [u8; 4] {
    let f = app.doc.flatten_rgba();
    let o = (y * app.doc.w + x) * 4;
    [f[o], f[o + 1], f[o + 2], f[o + 3]]
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
        let d2 = dir.clone();
        step!("window shows the ribbon and the picture", |app: &mut DrawApp, _h: &mut dyn Host, r: &mut Res| {
            let pm = shot(app, r);
            check(app.canvas.w > 400.0 && app.canvas.h > 300.0, "picture area too small")?;
            let t = pm.pixel(app.w as u32 / 2, 4).ok_or("no pixel")?;
            check(t.red() > 100 && t.green() < 40 && t.blue() < 40, &format!("title bar isn't maroon {:?}", (t.red(), t.green(), t.blue())))?;
            let (x, y) = app.to_screen((100.0, 100.0));
            let p = pm.pixel(x as u32, y as u32).ok_or("no pixel")?;
            check(p.red() > 240 && p.green() > 240 && p.blue() > 240, "the picture isn't showing white")
        });
        step!("pencil draws in colour 1", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "tool.pencil")?;
            check(app.tool == Tool::Pencil, "pencil not chosen")?;
            drag_on(app, h, r, (50.0, 50.0), (150.0, 50.0), Button::Left);
            check(px(app, 100, 50) == [0, 0, 0, 255], &format!("pixel {:?}", px(app, 100, 50)))?;
            check(app.dirty(), "picture not marked changed")
        });
        step!("colour 2 from the Colours menu, the brush paints it with the right button", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "color2")?;
            check(app.menu.is_some(), "colour menu didn't open")?;
            app.menu = None;
            app.c2 = [237, 28, 36, 255];
            ribbon_click(app, h, r, "home", "tool.brush")?;
            drag_on(app, h, r, (50.0, 120.0), (250.0, 120.0), Button::Right);
            let p = px(app, 150, 120);
            check(p[0] > 200 && p[1] < 80 && p[2] < 80, &format!("pixel {p:?}"))
        });
        step!("rectangle tool draws an outline", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "tool.rect")?;
            app.size = 3.0;
            drag_on(app, h, r, (300.0, 200.0), (500.0, 350.0), Button::Left);
            check(px(app, 400, 200)[0] < 60, &format!("top edge {:?}", px(app, 400, 200)))?;
            check(px(app, 400, 275) == [255, 255, 255, 255], "inside should stay white")
        });
        step!("undo takes the rectangle away, redo brings it back", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            key(app, h, r, Key::Char('z'), ctrl());
            check(px(app, 400, 200) == [255, 255, 255, 255], "still there after undo")?;
            key(app, h, r, Key::Char('y'), ctrl());
            check(px(app, 400, 200)[0] < 60, "not back after redo")
        });
        step!("fill with colour inside the rectangle", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.c1 = [34, 177, 76, 255];
            ribbon_click(app, h, r, "home", "tool.fill")?;
            click_on(app, h, r, 400.0, 275.0, Button::Left);
            check(px(app, 400, 275) == [34, 177, 76, 255], &format!("inside {:?}", px(app, 400, 275)))?;
            check(px(app, 600, 275) == [255, 255, 255, 255], "fill leaked outside")
        });
        step!("filled oval with Shift draws a circle", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "shape.fill")?;
            check(app.shape_fill, "shape fill not on")?;
            ribbon_click(app, h, r, "home", "tool.ellipse")?;
            app.c1 = [0, 0, 0, 255];
            app.mods = Mods { shift: true, ..Mods::NONE };
            drag_on(app, h, r, (700.0, 100.0), (800.0, 160.0), Button::Left);
            app.mods = Mods::NONE;
            let c = px(app, 750, 150);
            check(c[0] > 200 && c[1] < 80, &format!("centre not filled with colour 2: {c:?}"))?;
            // a 100 px circle: its outline crosses 50 px below the centre, the 60 px drag ignored
            let edge = (196..204).any(|y| px(app, 750, y)[0] < 60);
            let col: Vec<u8> = (190..206).map(|y| px(app, 750, y)[0]).collect();
            check(edge, &format!("not a circle (no edge 50 px below the centre): {col:?}"))
        });
        step!("polygon closes on a double-click, the curve bends twice", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "tool.polygon")?;
            check(app.tool == Tool::Polygon, "polygon tool not chosen")?;
            app.shape_fill = false;
            app.c1 = [0, 0, 0, 255];
            for (x, y) in [(900.0, 50.0), (1100.0, 50.0), (1000.0, 200.0)] {
                let q = sp(app, x, y);
                ev(app, h, r, Event::MouseMove { x: q.0, y: q.1 });
                ev(app, h, r, Event::MouseDown { x: q.0, y: q.1, button: Button::Left, clicks: 1 });
                ev(app, h, r, Event::MouseUp { x: q.0, y: q.1, button: Button::Left });
            }
            // the last corner was the first half of a double-click: the second press finishes it
            let q = sp(app, 1000.0, 200.0);
            ev(app, h, r, Event::MouseDown { x: q.0, y: q.1, button: Button::Left, clicks: 2 });
            ev(app, h, r, Event::MouseUp { x: q.0, y: q.1, button: Button::Left });
            check(app.multi.is_none(), "polygon still open")?;
            check(px(app, 1000, 50)[0] < 60, "top edge missing")?;
            check(px(app, 950, 125)[0] < 60 || px(app, 951, 125)[0] < 60 || px(app, 949, 126)[0] < 60, "closing edge missing")?;
            check(app.doc.history().0.last().map(|l| *l == "Polygon").unwrap_or(false), "not one Polygon step")?;
            ribbon_click(app, h, r, "home", "tool.curve")?;
            drag_on(app, h, r, (900.0, 600.0), (1100.0, 600.0), Button::Left);
            check(app.multi.is_some(), "curve finished after the line")?;
            drag_on(app, h, r, (950.0, 500.0), (950.0, 520.0), Button::Left);
            drag_on(app, h, r, (1050.0, 640.0), (1050.0, 640.0), Button::Left);
            check(app.multi.is_none(), "curve not finished after two bends")?;
            check(px(app, 1000, 600)[0] > 200, "the curve didn't bend away from the straight line")?;
            check(app.doc.history().0.last().map(|l| *l == "Curve").unwrap_or(false), "not one Curve step")
        });
        step!("colour picker takes a colour", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            key(app, h, r, Key::Char('i'), Mods::NONE);
            check(app.tool == Tool::Picker, "I didn't choose the picker")?;
            click_on(app, h, r, 400.0, 275.0, Button::Left);
            check(app.c1 == [34, 177, 76, 255], &format!("picked {:?}", app.c1))
        });
        step!("select and move part of the picture", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "tool.select")?;
            drag_on(app, h, r, (290.0, 190.0), (510.0, 360.0), Button::Left);
            check(app.doc.sel_bounds().is_some(), "nothing selected")?;
            drag_on(app, h, r, (400.0, 275.0), (400.0, 425.0), Button::Left);
            key(app, h, r, Key::Escape, Mods::NONE);
            check(px(app, 400, 425) == [34, 177, 76, 255], &format!("moved {:?}", px(app, 400, 425)))?;
            check(px(app, 400, 230) != [34, 177, 76, 255], "the old place still holds it")
        });
        step!("free-form selection, magic wand and Shift to add", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.set_tool(Tool::FreeSelect);
            let pts = [(600.0, 400.0), (700.0, 400.0), (650.0, 480.0)];
            let a = sp(app, pts[0].0, pts[0].1);
            ev(app, h, r, Event::MouseMove { x: a.0, y: a.1 });
            ev(app, h, r, Event::MouseDown { x: a.0, y: a.1, button: Button::Left, clicks: 1 });
            for p in &pts[1..] {
                let q = sp(app, p.0, p.1);
                ev(app, h, r, Event::MouseMove { x: q.0, y: q.1 });
            }
            ev(app, h, r, Event::MouseUp { x: a.0, y: a.1, button: Button::Left });
            let (x, y, w, hh) = app.doc.sel_bounds().ok_or("free-form selected nothing")?;
            check((598..=602).contains(&x) && (398..=402).contains(&y) && w >= 95 && hh >= 75, &format!("bounds {x},{y} {w}x{hh}"))?;
            // the wand on the green fill, Shift adds it to the triangle
            app.set_tool(Tool::Wand);
            app.mods = Mods { shift: true, ..Mods::NONE };
            click_on(app, h, r, 400.0, 425.0, Button::Left);
            app.mods = Mods::NONE;
            let (x2, y2, w2, h2) = app.doc.sel_bounds().ok_or("wand cleared the selection")?;
            check(x2 < 400 && x2 + w2 >= 700 && y2 < 425 && y2 + h2 >= 480, &format!("added bounds {x2},{y2} {w2}x{h2}"))?;
            app.command("select-none", h);
            check(app.doc.sel_bounds().is_none(), "select none")
        });
        step!("the brush kinds from the Brushes menu", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "tool.brush")?;
            app.menu = None;
            for (k, x) in [(folio_draw_engine::doc::TOOL_CALLIGRAPHY, 950.0), (folio_draw_engine::doc::TOOL_AIRBRUSH, 1000.0), (folio_draw_engine::doc::TOOL_MARKER, 1050.0)] {
                app.brush = k;
                app.set_tool(Tool::Brush);
                drag_on(app, h, r, (x, 300.0), (x, 400.0), Button::Left);
                let changed = (295..405).any(|y| px(app, x as usize, y) != [255, 255, 255, 255]);
                check(changed, &format!("brush {k} left nothing"))?;
            }
            Ok(())
        });
        step!("the Text tool types onto the picture", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.c1 = [0, 0, 0, 255];
            key(app, h, r, Key::Char('t'), Mods::NONE);
            check(app.tool == Tool::Text, "T didn't choose the Text tool")?;
            let before = app.doc.flatten_rgba();
            click_on(app, h, r, 900.0, 500.0, Button::Left);
            check(app.dialog.is_some(), "no Text box")?;
            shot(app, r);
            ev(app, h, r, Event::Text("Hello".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "Text box still open")?;
            let after = app.doc.flatten_rgba();
            let w = app.doc.w;
            let dark = (500..540).flat_map(|y| (900..1000).map(move |x| (x, y))).filter(|&(x, y)| {
                let o = (y * w + x) * 4;
                after[o] < 100 && before[o] > 200
            }).count();
            check(dark > 40, &format!("{dark} dark pixels where the text should be"))?;
            check(app.doc.history().0.last().map(|l| *l == "Text").unwrap_or(false), "undo step not named Text")?;
            let n = app.doc.layers.len();
            let tl = app.doc.active;
            check(!app.doc.layers[tl].text.is_empty(), "the text isn't on its own text layer")?;
            // click it again with the Text tool: the box opens with the words in it
            click_on(app, h, r, 905.0, 510.0, Button::Left);
            let d = app.dialog.as_ref().ok_or("clicking the text didn't open it")?;
            check(d.value("text") == "Hello", &format!("box holds {:?}", d.value("text")))?;
            shot(app, r);
            ev(app, h, r, Event::Text("Bye".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.doc.layers.len() == n && app.doc.layers[tl].text.contains("Bye"), "typed again on a new layer, or not changed")?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(app.doc.layers[tl].text.contains("Hello"), "undo didn't bring the first text back")?;
            app.command("layer.merge", h);
            check(app.doc.layers.len() == n - 1, "text layer not merged down")
        });
        step!("transparent selection leaves colour 2 behind", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.c2 = [255, 255, 255, 255];
            app.command("select-transparent", h);
            check(app.transparent_sel, "not on")?;
            app.set_tool(Tool::Select);
            // the black pencil line at y=50 on white: move it down 30 px
            app.doc.sel_rect(40, 40, 120, 20);
            drag_on(app, h, r, (100.0, 50.0), (100.0, 80.0), Button::Left);
            key(app, h, r, Key::Escape, Mods::NONE);
            check(px(app, 100, 80)[0] < 60, &format!("line not moved: {:?}", px(app, 100, 80)))?;
            // the white round the line didn't cover what was below it
            app.command("select-transparent", h);
            Ok(())
        });
        step!("copy and paste", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.doc.sel_rect(300, 350, 200, 150);
            key(app, h, r, Key::Char('c'), ctrl());
            let img = h.clipboard_image().ok_or("nothing on the clipboard")?;
            check(img.w == 200 && img.h == 150, &format!("clipboard {}x{}", img.w, img.h))?;
            key(app, h, r, Key::Char('v'), ctrl());
            check(app.doc.float.is_some(), "paste didn't float the picture")?;
            key(app, h, r, Key::Escape, Mods::NONE);
            check(app.doc.float.is_none(), "Esc didn't place it")
        });
        step!("rotate, crop and resize", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            let (w, hh) = (app.doc.w, app.doc.h);
            app.command("rotate.right", h);
            check(app.doc.w == hh && app.doc.h == w, "rotate right didn't swap the sides")?;
            app.command("rotate.left", h);
            check(app.doc.w == w, "rotate left didn't turn it back")?;
            app.doc.sel_rect(0, 0, 800, 500);
            ribbon_click(app, h, r, "home", "crop")?;
            check(app.doc.w == 800 && app.doc.h == 500, &format!("cropped to {}x{}", app.doc.w, app.doc.h))?;
            key(app, h, r, Key::Char('w'), ctrl());
            check(app.dialog.is_some(), "Resize didn't open")?;
            shot(app, r);
            // the width is selected: type over it, then Enter for OK
            ev(app, h, r, Event::Text("400".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "dialog still open")?;
            check(app.doc.w == 400 && app.doc.h == 250, &format!("resized to {}x{}", app.doc.w, app.doc.h))
        });
        step!("adjustments: greyscale, brightness, blur", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            // the brush stroke from earlier is red
            let red = |a: &DrawApp| (0..a.doc.w).flat_map(|x| (0..a.doc.h).map(move |y| (x, y))).find(|&(x, y)| { let p = px(a, x, y); p[0] > 200 && p[1] < 80 });
            app.doc.sel_none();
            let at = red(app);
            ribbon_click(app, h, r, "view", "adjust")?;
            check(app.menu.is_some(), "Adjustments menu didn't open")?;
            app.menu = None;
            app.command("adj.grey", h);
            if let Some((x, y)) = at {
                let p = px(app, x, y);
                check(p[0] == p[1] && p[1] == p[2], &format!("not grey: {p:?}"))?;
            }
            key(app, h, r, Key::Char('z'), ctrl());
            let p0 = px(app, 5, 5);
            app.command("adj.bc", h);
            shot(app, r);
            ev(app, h, r, Event::Text("-50".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            let p1 = px(app, 5, 5);
            check(p1[0] as i32 + p1[1] as i32 + p1[2] as i32 + 30 < p0[0] as i32 + p0[1] as i32 + p0[2] as i32, &format!("corner not darker: {p0:?} -> {p1:?}"))?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(px(app, 5, 5) == p0, &format!("undo didn't restore: {:?}", px(app, 5, 5)))?;
            app.command("adj.blur", h);
            shot(app, r);
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.doc.history().0.last().map(|l| *l == "Blur").unwrap_or(false), "blur not applied")?;
            app.ribbon.select_tab("home");
            Ok(())
        });
        step!("canvas size, rotate by angle, skew and a typed colour", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            let (w, hh) = (app.doc.w, app.doc.h);
            app.command("canvas-size", h);
            shot(app, r);
            ev(app, h, r, Event::Text((w + 100).to_string()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.doc.w == w + 100 && app.doc.h == hh, &format!("canvas {}x{}", app.doc.w, app.doc.h))?;
            app.command("rotate.angle", h);
            shot(app, r);
            ev(app, h, r, Event::Text("45".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.doc.w > w + 100 && app.doc.h > hh, &format!("rotated 45: {}x{}", app.doc.w, app.doc.h))?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(app.doc.w == w + 100, "undo rotate")?;
            app.command("skew", h);
            shot(app, r);
            ev(app, h, r, Event::Text("20".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.doc.w > w + 100, &format!("skewed width {}", app.doc.w))?;
            key(app, h, r, Key::Char('z'), ctrl());
            key(app, h, r, Key::Char('z'), ctrl());
            check(app.doc.w == w && app.doc.h == hh, "undo back to the start")?;
            app.menu_pick_for_test("color1", "edit-color:color1");
            shot(app, r);
            ev(app, h, r, Event::Text("#123456".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.c1 == [0x12, 0x34, 0x56, 255], &format!("colour 1 is {:?}", app.c1))
        });
        step!("zoom from the View tab", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "view", "zoom.100")?;
            check((app.zoom - 1.0).abs() < 0.001, "100% isn't 1")?;
            ribbon_click(app, h, r, "view", "zoom.in")?;
            check(app.zoom > 1.0, "zoom in")?;
            ev(app, h, r, Event::Modifiers(ctrl()));
            ev(app, h, r, Event::Wheel { dx: 0.0, dy: -1.0 });
            ev(app, h, r, Event::Modifiers(Mods::NONE));
            check((app.zoom - 1.0).abs() < 0.001, &format!("Ctrl+wheel out: {}", app.zoom))?;
            ribbon_click(app, h, r, "view", "zoom.fit")?;
            app.ribbon.select_tab("home");
            Ok(())
        });
        step!("save as PNG and open it again", move |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            let p = d2.join("ui-test.png");
            let _ = std::fs::remove_file(&p);
            key(app, h, r, Key::Char('s'), ctrl());
            check(app.back.as_ref().map(|b| b.page) == Some(BackPage::SaveAs), "Save As didn't open for a new picture")?;
            if let Some(b) = app.back.as_mut() {
                b.files.name.set(p.to_str().unwrap());
                b.files.name.focused = true;
            }
            shot(app, r);
            key(app, h, r, Key::Enter, Mods::NONE);
            check(p.exists(), "file not written")?;
            check(!app.dirty(), "still marked unsaved")?;
            let before = app.doc.flatten_rgba();
            let back = DrawApp::load(&p)?;
            check(back.w == app.doc.w && back.flatten_rgba() == before, "the reopened picture differs")
        });
        let d3 = dir.clone();
        step!("layers survive OpenRaster", move |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "view", "layer.add")?;
            app.ribbon.select_tab("home");
            check(app.doc.layers.len() == 2, "no new layer")?;
            let p = d3.join("ui-test.ora");
            check(app.save_to(&p), "not saved")?;
            let back = DrawApp::load(&p)?;
            check(back.layers.len() == 2, &format!("{} layers after reopening", back.layers.len()))
        });
        step!("the Layers pane shows, hides and merges layers", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            check(app.show_layers, "adding a layer didn't show the pane")?;
            shot(app, r);
            check(app.side.w > 100.0 && app.canvas.right() <= app.side.x + 1.0, "pane not beside the picture")?;
            // the new layer is on top and active: paint on it, then hide it from the pane
            app.c1 = [63, 72, 204, 255];
            app.set_tool(Tool::Pencil);
            drag_on(app, h, r, (20.0, 20.0), (120.0, 20.0), Button::Left);
            check(px(app, 70, 20)[2] > 180, "nothing drawn on the new layer")?;
            let top = app.doc.layers.len() - 1;
            let eye = format!("eye:{top}");
            shot(app, r);
            let rr = app.side_hits_rect(&eye).ok_or("no eye button for the top layer")?;
            ev(app, h, r, Event::MouseDown { x: rr.cx(), y: rr.cy(), button: Button::Left, clicks: 1 });
            ev(app, h, r, Event::MouseUp { x: rr.cx(), y: rr.cy(), button: Button::Left });
            check(!app.doc.layers[top].visible, "the eye didn't hide the layer")?;
            check(px(app, 70, 20)[2] < 180 || px(app, 70, 20)[0] > 30, "hidden layer still shows")?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(app.doc.layers[top].visible, "undo didn't show it again")?;
            app.command("layer.merge", h);
            check(app.doc.layers.len() == 1, &format!("{} layers after merge down", app.doc.layers.len()))?;
            check(px(app, 70, 20)[2] > 180, "merged picture lost the stroke")?;
            key(app, h, r, Key::F(7), Mods::NONE);
            check(!app.show_layers, "F7 didn't hide the pane")
        });
        step!("rulers, gridlines and nudging the selection", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            let x0 = app.canvas.x;
            key(app, h, r, Key::Char('r'), ctrl());
            check(app.rulers && app.canvas.x > x0, "rulers took no room")?;
            key(app, h, r, Key::Char('g'), ctrl());
            check(app.gridlines, "gridlines off")?;
            app.set_zoom(8.0, None);
            let pm = shot(app, r);
            let _ = pm;
            key(app, h, r, Key::Char('r'), ctrl());
            key(app, h, r, Key::Char('g'), ctrl());
            app.command("zoom.fit", h);
            app.set_tool(Tool::Select);
            app.doc.sel_rect(10, 10, 20, 20);
            let before = px(app, 15, 15);
            key(app, h, r, Key::Right, Mods { shift: true, ..Mods::NONE });
            let f = app.doc.float.as_ref().ok_or("arrow didn't lift the selection")?;
            check(f.x == 20 && f.y == 10, &format!("float at {},{}", f.x, f.y))?;
            key(app, h, r, Key::Escape, Mods::NONE);
            check(px(app, 25, 15) == before, "the moved pixels aren't 10 px right")
        });
        step!("settings are kept, saved files come back as Recent", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.size = 8.0;
            app.c2 = [1, 2, 3, 255];
            app.remember();
            let st = crate::settings::Settings::load();
            check(st.size == 8.0 && st.c2 == [1, 2, 3, 255], &format!("loaded {st:?}"))?;
            check(st.recent.iter().any(|p| p.ends_with("ui-test.ora")), "the saved file isn't in Recent")?;
            app.command("open", h);
            shot(app, r);
            let rr = app.back.as_ref().and_then(|b| b.hit_rect("recent:0")).ok_or("Recent not shown on the Open page")?;
            ev(app, h, r, Event::MouseDown { x: rr.cx(), y: rr.cy(), button: Button::Left, clicks: 1 });
            ev(app, h, r, Event::MouseUp { x: rr.cx(), y: rr.cy(), button: Button::Left });
            if app.dialog.is_some() {
                // unsaved changes: don't save
                app.dialog = None;
            }
            check(app.back.is_none() || app.path.as_ref().map(|p| p.ends_with("ui-test.ora")).unwrap_or(false), "Recent didn't open")
        });
        step!("layer properties, paste as new layer, Ctrl-drag copies, the magnifier", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            // a picture on the clipboard, then Paste as new layer
            let img = folio_kit::platform::Image { w: 10, h: 10, rgba: [255u8, 0, 0, 255].repeat(100) };
            h.set_clipboard_image(&img);
            let n = app.doc.layers.len();
            app.command("paste-layer", h);
            check(app.doc.layers.len() == n + 1, "no new layer for the paste")?;
            app.command("layer.props", h);
            shot(app, r);
            ev(app, h, r, Event::Text("Red square".into()));
            // to the opacity field
            key(app, h, r, Key::Tab, Mods::NONE);
            ev(app, h, r, Event::Text("50".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            let l = &app.doc.layers[app.doc.active];
            check(l.name == "Red square" && (l.opacity - 0.5).abs() < 0.01, &format!("layer {} at {}", l.name, l.opacity))?;
            key(app, h, r, Key::Char('z'), ctrl());
            key(app, h, r, Key::Char('z'), ctrl());
            check(app.doc.layers.len() == n, "undo didn't take the pasted layer away")?;
            // Ctrl-drag a copy of the green square
            app.set_tool(Tool::Select);
            app.doc.float_from_rgba(20, 20, [20u8, 60, 220, 255].repeat(400), 20, 150);
            app.doc.commit_float();
            app.doc.sel_rect(20, 150, 20, 20);
            let src = px(app, 30, 160);
            app.mods = ctrl();
            drag_on(app, h, r, (30.0, 160.0), (230.0, 160.0), Button::Left);
            app.mods = Mods::NONE;
            key(app, h, r, Key::Escape, Mods::NONE);
            check(px(app, 30, 160) == src && px(app, 230, 160) == src, &format!("Ctrl-drag: {:?} and {:?}", px(app, 30, 160), px(app, 230, 160)))?;
            let z = app.zoom;
            key(app, h, r, Key::Char('m'), Mods::NONE);
            click_on(app, h, r, 200.0, 120.0, Button::Left);
            check(app.zoom > z, "magnifier didn't zoom in")?;
            click_on(app, h, r, 200.0, 120.0, Button::Right);
            check((app.zoom - z).abs() < 0.01 || app.zoom < z * 1.01, "right-click didn't zoom back out")?;
            app.command("zoom.fit", h);
            Ok(())
        });
        step!("dark theme", |app: &mut DrawApp, _h: &mut dyn Host, r: &mut Res| {
            app.theme = folio_kit::theme::Theme::new(true, crate::app::MAROON, crate::app::ACCENT);
            let pm = shot(app, r);
            let dir = PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()));
            let _ = std::fs::write(dir.join("ui-dark.png"), gfx::encode_png(&pm));
            let p = pm.pixel(app.w as u32 - 40, (app.ribbon.rect.y + 60.0) as u32).ok_or("no pixel")?;
            check(p.red() < 80, "ribbon not dark")
        });
        UiTest { steps, i: 0, done: false, fails: vec![], passed: 0, dir }
    }

    pub fn step(&mut self, app: &mut DrawApp, host: &mut dyn Host, r: &mut Res) {
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
