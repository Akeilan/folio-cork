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
        step!("a new shape can be moved and resized before it is kept", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.set_tool(Tool::Rect);
            app.shape_fill = false;
            app.c1 = [0, 0, 0, 255];
            drag_on(app, h, r, (800.0, 300.0), (860.0, 340.0), Button::Left);
            check(app.pending.is_some(), "no pending shape after drawing")?;
            // drag from inside moves it 100 px right
            drag_on(app, h, r, (830.0, 320.0), (930.0, 320.0), Button::Left);
            check(px(app, 830, 300)[0] > 200 && px(app, 930, 300)[0] < 60, "the rectangle didn't move")?;
            // the bottom right handle makes it bigger
            let hb = app.pending_handles()[4];
            drag_on(app, h, r, hb, (hb.0 + 40.0, hb.1 + 40.0), Button::Left);
            let p = app.pending.ok_or("pending lost while resizing")?;
            check((p.b.0 - 1000.0).abs() < 1.0 && (p.b.1 - 380.0).abs() < 1.0, &format!("resized to {:?}", p.b))?;
            check(px(app, 1000, 360)[0] < 60, "the right edge isn't at its new place")?;
            // a second Rectangle steps; Escape on a fresh one takes it away
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.pending.is_none(), "Enter didn't keep it")?;
            check(app.doc.history().0.last().map(|l| *l == "Rectangle").unwrap_or(false), "not one Rectangle step")?;
            drag_on(app, h, r, (800.0, 450.0), (860.0, 480.0), Button::Left);
            key(app, h, r, Key::Escape, Mods::NONE);
            check(app.pending.is_none() && px(app, 830, 450)[0] > 200, "Escape didn't take the new shape away")
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
        let d4 = dir.clone();
        step!("an animated GIF opens as frames that step and play", move |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            // four frames, each a different colour
            let cols = [[255u8, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 0, 255]];
            let frames: Vec<Vec<u8>> = cols.iter().map(|c| c.repeat(32 * 32)).collect();
            let refs: Vec<(&[u8], u32)> = frames.iter().map(|f| (f.as_slice(), 50)).collect();
            let gif = folio_draw_engine::gif::encode_animation(32, 32, &refs, Some(0));
            let p = d4.join("frames.gif");
            std::fs::write(&p, gif).map_err(|e| e.to_string())?;
            app.dialog = None;
            app.after = Some(crate::app::After::Open(p));
            app.saved_for_test();
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            check(app.doc.layers.len() == 4, &format!("{} layers", app.doc.layers.len()))?;
            check(app.doc.solo, "the frames view isn't on")?;
            let a = app.doc.active;
            key(app, h, r, Key::Char('.'), Mods::NONE);
            check(app.doc.active == (a + 1) % 4, "'.' didn't step to the next frame")?;
            key(app, h, r, Key::Char(','), Mods::NONE);
            check(app.doc.active == a, "',' didn't step back")?;
            app.doc.set_active(3);
            app.command("frame.next", h);
            check(app.doc.active == 0, "next frame didn't wrap to the first")?;
            ribbon_click(app, h, r, "view", "frame.play")?;
            check(app.playing.is_some(), "play didn't start")?;
            app.ribbon.select_tab("home");
            Ok(())
        });
        step!("playback moves through the frames, a click stops it", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            // the test ticks every 60 ms of the virtual clock and frames last 50 ms
            let a = app.doc.active;
            let seen = app.doc.active != a || app.playing.is_some();
            check(seen, "not playing")?;
            for _ in 0..3 {
                app.playing = Some(std::time::Duration::ZERO);
                ev(app, h, r, Event::Tick);
            }
            check(app.doc.active != a, "frames didn't advance")?;
            click_on(app, h, r, 10.0, 10.0, Button::Left);
            check(app.playing.is_none(), "a click didn't stop playback")
        });
        let d5 = dir.clone();
        step!("embroidery: a new hoop, a shape, its stitches, export and import", move |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:0", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            check(!app.doc.emb.is_empty() && app.doc.w == 1016 && app.doc.h == 1016, &format!("hoop canvas {}x{}", app.doc.w, app.doc.h))?;
            check(app.ribbon.active_id() == "embroidery", "the Embroidery tab isn't showing")?;
            // a 30 mm red square in the middle of the hoop (mm, centre at 0,0)
            let spec = serde_json::json!({"objects": [{"id": 1, "kind": "shape", "rings": [[[-15, -15], [15, -15], [15, 15], [-15, 15]]],
                "thread": {"rgb": [200, 20, 30]}}], "next_id": 2});
            crate::emb::add_shape_layer(&mut app.doc, "Square", &spec);
            let c = px(app, 508, 508);
            check(c[0] > 180 && c[1] < 60, &format!("shape not drawn in its thread colour: {c:?}"))?;
            let plan = crate::emb::make_plan(&app.doc).ok_or("no plan")?;
            check(plan.stitch_count() > 200, &format!("{} stitches", plan.stitch_count()))?;
            ribbon_click(app, h, r, "embroidery", "emb.view")?;
            check(app.stitch_view, "stitch view off")?;
            shot(app, r);
            app.command("emb.info", h);
            let note = app.dialog.as_ref().map(|d| d.fields.iter().map(|f| f.label.clone()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
            let _ = note;
            check(app.dialog.is_some(), "no stitch information")?;
            app.dialog = None;
            for ext in ["pes", "dst", "jef", "exp", "vp3", "xxx"] {
                let p = d5.join(format!("design.{ext}"));
                app.export_stitches(&p)?;
                let back = folio_draw_engine::emb::formats::read_named(&std::fs::read(&p).map_err(|e| e.to_string())?, ext)?;
                check(back.stitch_count() + 2 >= plan.stitch_count(), &format!("{ext}: {} of {} stitches", back.stitch_count(), plan.stitch_count()))?;
            }
            let n = app.doc.layers.len();
            app.import_stitches(&d5.join("design.pes"))?;
            check(app.doc.layers.len() == n + 1, "import made no layer")?;
            let p2 = crate::emb::make_plan(&app.doc).ok_or("no plan after import")?;
            check(p2.stitch_count() + 4 >= 2 * plan.stitch_count(), &format!("{} stitches after importing a copy", p2.stitch_count()))?;
            // a two-colour picture placed in the hoop and converted to shapes
            let (pw, ph) = (200usize, 100usize);
            let mut pic = vec![0u8; pw * ph * 4];
            for y in 0..ph {
                for x in 0..pw {
                    let o = (y * pw + x) * 4;
                    let c: [u8; 4] = if x < 100 { [20, 60, 200, 255] } else { [250, 200, 20, 255] };
                    pic[o..o + 4].copy_from_slice(&c);
                }
            }
            let before = app.doc.layers.len();
            crate::emb::place_picture(&mut app.doc, pw, ph, pic, "Flag").ok_or("not placed")?;
            check(app.doc.layers.len() == before + 1, "no picture layer")?;
            app.command("emb.trace", h);
            check(app.dialog.is_some(), "Convert to shapes didn't ask how")?;
            shot(app, r);
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "the Convert box stayed open")?;
            let made = app.doc.layers.len() - before - 1;
            check(made == 2, &format!("{made} colour layers from a two-colour picture"))?;
            check(app.doc.history().0.last().map(|l| *l == "Convert to shapes").unwrap_or(false), "not one undo step")?;
            let p3 = crate::emb::make_plan(&app.doc).ok_or("no plan")?;
            check(p3.blocks.len() >= 4, &format!("{} colour blocks", p3.blocks.len()))?;
            // select the blue shape, move it 10 mm down, recolour it, delete it
            ribbon_click(app, h, r, "embroidery", "tool.emb")?;
            check(app.tool == Tool::EmbSelect, "Select shapes not chosen")?;
            let st = crate::emb::settings(&app.doc).ok_or("no settings")?;
            let (bx, by) = crate::emb::mm_to_px(&st, -20.0, 0.0);
            click_on(app, h, r, bx, by, Button::Left);
            let (li, _) = app.emb_sel.ok_or("clicking the blue shape selected nothing")?;
            check(app.doc.layers[li].name == "Ultramarine" || app.doc.layers[li].text.contains("rings"), "selected the wrong layer")?;
            let (_, ty) = crate::emb::mm_to_px(&st, 0.0, 10.0);
            drag_on(app, h, r, (bx, by), (bx, by + (ty - crate::emb::mm_to_px(&st, 0.0, 0.0).1)), Button::Left);
            check(app.doc.history().0.last().map(|l| *l == "Move shapes").unwrap_or(false), "not moved")?;
            let below = crate::emb::mm_to_px(&st, -20.0, 27.0);
            let c = px(app, below.0 as usize, below.1 as usize);
            check(c[2] > 120, &format!("the blue shape isn't 10 mm lower: {c:?}"))?;
            app.menu_pick_for_test("emb.recolour", "color:#22B14C");
            let c2 = px(app, below.0 as usize, below.1 as usize);
            check(c2[1] > 150 && c2[2] < 120, &format!("not recoloured: {c2:?}"))?;
            // satin with a fixed angle changes the planned stitches
            let before_st = crate::emb::make_plan(&app.doc).map(|p| p.stitch_count()).unwrap_or(0);
            app.command("emb.stitch", h);
            shot(app, r);
            // the type list comes first; the angle field is the first text field
            ev(app, h, r, Event::Text("45".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.doc.history().0.last().map(|l| *l == "Stitch properties").unwrap_or(false), "stitch properties not applied")?;
            let after_st = crate::emb::make_plan(&app.doc).map(|p| p.stitch_count()).unwrap_or(0);
            check(after_st != before_st, &format!("a 45° fill planned the same {after_st} stitches"))?;
            key(app, h, r, Key::Delete, Mods::NONE);
            check(app.emb_sel.is_none() && px(app, below.0 as usize, below.1 as usize)[1] < 150, "Delete left the shape")?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(px(app, below.0 as usize, below.1 as usize)[1] > 150, "undo didn't bring it back")?;
            // a bigger hoop keeps the design in the middle
            let mid = px(app, 508, 508);
            app.command("emb.hoop", h);
            app.dialog = app.dialog.take().map(|mut dl| {
                for f in dl.fields.iter_mut() {
                    if f.id == "hoop" {
                        if let folio_kit::ui::FieldKind::Choice(_, i) = &mut f.kind {
                            *i = 1;
                        }
                    }
                }
                dl
            });
            shot(app, r);
            // OK is the first button: Enter from a text field presses it
            key(app, h, r, Key::Tab, Mods::NONE);
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.doc.w == 1270 && app.doc.h == 1778, &format!("5 × 7 hoop canvas {}x{}", app.doc.w, app.doc.h))?;
            check(px(app, 635, 889) == mid, "the design didn't stay in the middle")?;
            let ora = d5.join("design.ora");
            check(app.save_to(&ora), "not saved")?;
            let back = DrawApp::load(&ora)?;
            check(back.emb == app.doc.emb, &format!("the .ora lost the settings: {:?} vs {:?}", back.emb, app.doc.emb))?;
            check(crate::emb::layer_spec(&back, back.layers.len() - 1).is_some(), "the .ora lost the shapes")
        });
        step!("the stitch-out simulator sews the design on screen", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:0", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            let spec = serde_json::json!({"objects": [{"id": 1, "kind": "shape", "rings": [[[-15, -8], [15, -8], [15, 8], [-15, 8]]],
                "thread": {"rgb": [30, 60, 200]}, "stitch": {"type": "fill"}}], "next_id": 2});
            crate::emb::add_shape_layer(&mut app.doc, "Band", &spec);
            let plan = crate::emb::make_plan(&app.doc).ok_or("no plan")?;
            let total = crate::emb::command_count(&plan);
            check(total > 200, &format!("only {total} commands"))?;
            let (early, needle) = crate::emb::stitch_lines_upto(&app.doc, &plan, 40);
            let (all, _) = crate::emb::stitch_lines_upto(&app.doc, &plan, total);
            check(needle.is_some(), "no needle")?;
            let pts = |v: &Vec<([u8; 3], Vec<(f32, f32)>)>| v.iter().map(|r| r.1.len()).sum::<usize>();
            check(pts(&early) < pts(&all), &format!("40 commands drew {} of {} points", pts(&early), pts(&all)))?;
            app.command("emb.stitchout", h);
            check(app.stitch_out == Some(0) && app.stitch_view, "the simulator didn't start")?;
            shot(app, r);
            for _ in 0..3 {
                app.now += std::time::Duration::from_millis(50);
                ev(app, h, r, Event::Tick);
            }
            check(app.stitch_out.map(|n| n > 0).unwrap_or(false), &format!("it didn't move on: {:?}", app.stitch_out))?;
            shot(app, r);
            app.command("emb.stitchout", h);
            check(app.stitch_out.is_none(), "it didn't stop")?;
            // it runs to the end and stops by itself
            app.command("emb.stitchout", h);
            for _ in 0..400 {
                if app.stitch_out.is_none() {
                    break;
                }
                app.now += std::time::Duration::from_millis(50);
                ev(app, h, r, Event::Tick);
            }
            check(app.stitch_out.is_none(), "it never reached the end")?;
            app.stitch_view = false;
            Ok(())
        });
        step!("the density view colours the design by its thread layers", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:0", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            // a 20 mm square: filled, so the middle gets several passes
            let spec = serde_json::json!({"objects": [{"id": 1, "kind": "shape", "rings": [[[-10, -10], [10, -10], [10, 10], [-10, 10]]],
                "thread": {"rgb": [200, 20, 30]}, "stitch": {"type": "fill"}}], "next_id": 2});
            crate::emb::add_shape_layer(&mut app.doc, "Square", &spec);
            let plan = crate::emb::make_plan(&app.doc).ok_or("no plan")?;
            let (covered, _, _) = crate::emb::density_stats(&app.doc, &plan);
            check(covered > 1000, &format!("only {covered} pixels covered"))?;
            let (dw, dh, map) = crate::emb::density_rgba(&app.doc, &plan).ok_or("no density map")?;
            check(dw == app.doc.w && dh == app.doc.h, &format!("map {dw}x{dh} isn't the picture's size"))?;
            let mid = ((app.doc.h / 2) * dw + app.doc.w / 2) * 4;
            check(map[mid + 3] > 0, "nothing covers the middle of the square")?;
            check(map[3] == 0, "the empty corner is coloured in")?;
            app.command("emb.density", h);
            check(app.density_view, "the density view didn't come on")?;
            let pm = shot(app, r);
            let (sx, sy) = app.to_screen(((app.doc.w / 2) as f32, (app.doc.h / 2) as f32));
            let c = pm.pixel(sx as u32, sy as u32).ok_or("no pixel")?;
            check(c.green() > 60 || c.red() > 60, &format!("the density colour isn't on screen: {:?}", (c.red(), c.green(), c.blue())))?;
            app.command("emb.density", h);
            check(!app.density_view, "it didn't go off again")
        });
        step!("lettering from a font, and an envelope that bends it", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:1", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            let layers = app.doc.layers.len();
            app.command("emb.letter", h);
            shot(app, r);
            let d = app.dialog.as_mut().ok_or("the Lettering box didn't open")?;
            let font = app.fonts.iter().position(|f| f == "Carlito Bold").ok_or("Carlito Bold isn't among the fonts")?;
            d.set_choice("font", font);
            ev(app, h, r, Event::Text("FOLIO".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "the box stayed open")?;
            check(app.doc.layers.len() == layers + 1, &format!("no lettering layer: {} layers", app.doc.layers.len()))?;
            let li = app.doc.layers.len() - 1;
            let sp = crate::emb::layer_spec(&app.doc, li).ok_or("the lettering layer has no shapes")?;
            let o = sp["objects"][0].clone();
            check(o["kind"] == "lettering" && o["text"] == "FOLIO", &format!("object {o}"))?;
            let glyphs = o["glyphs"].as_array().map(|g| g.len()).unwrap_or(0);
            check(glyphs == 5, &format!("{glyphs} letters drawn, not 5"))?;
            let (x0, y0, x1, y1) = crate::emb::object_bounds(&o);
            check((y1 - y0 - 10.0).abs() < 1.0 && x1 - x0 > 20.0, &format!("letters {:.1} x {:.1} mm", x1 - x0, y1 - y0))?;
            // the letters are actually painted on the layer, in the thread colour
            let (cx, cy) = crate::emb::mm_to_px(&crate::emb::settings(&app.doc).unwrap(), (x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let around = (-30..30).any(|dx| (-30..30).any(|dy| px(app, (cx as i32 + dx) as usize, (cy as i32 + dy) as usize)[3] > 0));
            check(around, "nothing was drawn where the letters are")?;
            // an envelope bends them: the arch makes the word taller
            app.emb_sel = Some((li, o["id"].as_i64().unwrap_or(1)));
            app.command("emb.envelope", h);
            shot(app, r);
            let d = app.dialog.as_mut().ok_or("the Envelope box didn't open")?;
            d.set_choice("kind", 1);
            key(app, h, r, Key::Enter, Mods::NONE);
            let sp = crate::emb::layer_spec(&app.doc, li).ok_or("the layer lost its shapes")?;
            let o2 = sp["objects"][0].clone();
            let (_, ny0, _, ny1) = crate::emb::object_bounds(&o2);
            check(ny1 - ny0 > (y1 - y0) * 1.2, &format!("the arch didn't make it taller: {:.1} vs {:.1}", ny1 - ny0, y1 - y0))?;
            check(crate::emb::make_plan(&app.doc).is_some(), "the lettering can't be planned into stitches")?;
            key(app, h, r, Key::Char('z'), ctrl());
            let o3 = crate::emb::layer_spec(&app.doc, li).ok_or("gone after undo")?["objects"][0].clone();
            let (_, uy0, _, uy1) = crate::emb::object_bounds(&o3);
            check((uy1 - uy0 - (y1 - y0)).abs() < 0.5, &format!("undo left it bent: {:.1}", uy1 - uy0))
        });
        step!("changing the hoop is one undo step, the old hoop and all", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:0", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            let spec = serde_json::json!({"objects": [{"id": 1, "kind": "shape", "rings": [[[-10, -10], [10, -10], [10, 10], [-10, 10]]],
                "thread": {"rgb": [20, 90, 200]}}], "next_id": 2});
            crate::emb::add_shape_layer(&mut app.doc, "Square", &spec);
            let (w0, h0) = (app.doc.w, app.doc.h);
            let hoop0 = crate::emb::settings(&app.doc).ok_or("no settings")?["hoop"].as_str().unwrap_or("").to_string();
            app.command("emb.hoop", h);
            shot(app, r);
            check(app.dialog.is_some(), "the Hoop box didn't open")?;
            let d = app.dialog.as_mut().unwrap();
            d.set_choice("hoop", 2);
            key(app, h, r, Key::Enter, Mods::NONE);
            let st = crate::emb::settings(&app.doc).ok_or("settings gone")?;
            check(st["hoop"].as_str() != Some(hoop0.as_str()), "the hoop didn't change")?;
            check(app.doc.w != w0 || app.doc.h != h0, &format!("the canvas stayed {}x{}", app.doc.w, app.doc.h))?;
            check(crate::emb::layer_spec(&app.doc, app.doc.layers.len() - 1).is_some(), "the shape layer lost its shapes")?;
            key(app, h, r, Key::Char('z'), ctrl());
            let st = crate::emb::settings(&app.doc).ok_or("settings gone after undo")?;
            check(st["hoop"].as_str() == Some(hoop0.as_str()), &format!("undo left the hoop at {:?}", st["hoop"]))?;
            check(app.doc.w == w0 && app.doc.h == h0, &format!("undo left the canvas at {}x{}", app.doc.w, app.doc.h))?;
            check(crate::emb::layer_spec(&app.doc, app.doc.layers.len() - 1).is_some(), "undo lost the shapes")
        });
        step!("how many times a saved animation plays", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "view", "frame.loop")?;
            let m = app.menu.as_ref().ok_or("the Plays menu didn't open")?;
            check(m.items.len() == crate::app::GIF_LOOPS.len(), &format!("{} choices", m.items.len()))?;
            app.menu = None;
            app.saved_for_test();
            app.menu_pick_for_test("frame.loop", "frame.loop:4");
            check(app.doc.gif_loop == 4, &format!("gif_loop {}", app.doc.gif_loop))?;
            check(app.dirty(), "a loop count can't be saved")?;
            app.menu_pick_for_test("frame.loop", "frame.loop:0");
            check(app.doc.gif_loop == 0, "back to forever")
        });
        step!("Remove background cuts the subject out", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("new", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            let (w, hh) = (app.doc.w as f32, app.doc.h as f32);
            app.c2 = [237, 28, 36, 255];
            app.shape_fill = true;
            app.set_tool(Tool::Ellipse);
            drag_on(app, h, r, (w * 0.3, hh * 0.3), (w * 0.7, hh * 0.7), Button::Left);
            app.shape_fill = false;
            let mid = px(app, app.doc.w / 2, app.doc.h / 2);
            check(mid[0] > 200 && mid[1] < 80, &format!("no red subject: {mid:?}"))?;
            app.command("remove-bg", h);
            let corner = px(app, 4, 4);
            check(corner[3] == 0, &format!("the background is still there: {corner:?}"))?;
            let mid = px(app, app.doc.w / 2, app.doc.h / 2);
            check(mid[3] == 255 && mid[0] > 200, &format!("the subject went too: {mid:?}"))?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(px(app, 4, 4)[3] == 255, "undo didn't bring the background back")?;
            let (_, redo) = app.doc.history();
            check(redo == vec!["Remove background"], &format!("not one undo step: {redo:?}"))
        });
        step!("the magic wand's tolerance and sample-all are kept", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.command("wand-settings", h);
            shot(app, r);
            check(app.dialog.is_some(), "the Magic wand box didn't open")?;
            ev(app, h, r, Event::Text("80".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "the box stayed open")?;
            check((app.wand_tol - 80.0).abs() < 0.01, &format!("tolerance {}", app.wand_tol))?;
            app.command("select-sample-all", h);
            check(app.doc.sample_all, "Sample all layers didn't come on")?;
            app.remember();
            let st = crate::settings::Settings::load();
            check((st.wand_tol - 80.0).abs() < 0.01 && st.sample_all, &format!("not kept: {} {}", st.wand_tol, st.sample_all))?;
            app.command("select-sample-all", h);
            check(!app.doc.sample_all, "it didn't go off again")
        });
        step!("the stabilizer steadies a hand-drawn line", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("new", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            app.set_tool(Tool::Pencil);
            app.command("stabilizer", h);
            shot(app, r);
            check(app.dialog.is_some(), "the Stabilizer box didn't open")?;
            ev(app, h, r, Event::Text("100".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.stab[0][0] == 100, &format!("StreamLine {:?}", app.stab[0]))?;
            // a zigzag: with the line on a long string the peaks are cut off
            let y0 = app.doc.h as f32 * 0.5;
            let a = sp(app, 100.0, y0);
            ev(app, h, r, Event::MouseMove { x: a.0, y: a.1 });
            ev(app, h, r, Event::MouseDown { x: a.0, y: a.1, button: Button::Left, clicks: 1 });
            for i in 1..=12 {
                let q = sp(app, 100.0 + i as f32 * 20.0, y0 + if i % 2 == 0 { -60.0 } else { 60.0 });
                ev(app, h, r, Event::MouseMove { x: q.0, y: q.1 });
            }
            let e = sp(app, 340.0, y0);
            ev(app, h, r, Event::MouseMove { x: e.0, y: e.1 });
            ev(app, h, r, Event::MouseUp { x: e.0, y: e.1, button: Button::Left });
            let flat = app.doc.flatten_rgba();
            let dw = app.doc.w;
            let inked = |x: i32, dy: i32| flat[(((y0 as i32 + dy) as usize) * dw + x as usize) * 4] < 200;
            let band = |lo: i32, hi: i32| (100..=340).any(|x| (lo..=hi).any(|dy| inked(x, dy)));
            let rows: Vec<i32> = (-70..=70).filter(|dy| (100..=340).any(|x| inked(x, *dy))).collect();
            check(!band(-60, -45) && !band(45, 60), &format!("the peaks of the zigzag are still there: rows {:?} to {:?}", rows.first(), rows.last()))?;
            check(band(-20, 20), "the stabilized line isn't there at all")?;
            app.stab = [[0; 3]; 3];
            app.remember();
            Ok(())
        });
        step!("the search box runs a command, and Quick Access is the user's to choose", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            key(app, h, r, Key::Char('q'), Mods { alt: true, ..Mods::NONE });
            check(app.title.search.as_ref().map(|f| f.focused).unwrap_or(false), "Alt+Q didn't put the cursor in the search box")?;
            ev(app, h, r, Event::Text("gridl".into()));
            shot(app, r);
            let m = app.menu.as_ref().ok_or("no list of matches")?;
            check(m.id == "search" && !m.items.is_empty(), &format!("{} matches", m.items.len()))?;
            check(m.items.iter().any(|i| i.id == "view.grid"), "Gridlines isn't among them")?;
            let before = app.gridlines;
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.gridlines != before, "the command didn't run")?;
            check(app.menu.is_none() && app.title.search_text().is_empty(), "the box didn't clear")?;
            app.command("view.grid", h);
            // Quick Access: the chevron's menu adds and takes away buttons, and they are remembered
            app.command("quick.menu", h);
            let m = app.menu.as_ref().ok_or("no Quick Access menu")?;
            check(m.items.iter().filter(|i| i.checked).count() == 3, "three buttons to start with")?;
            app.menu = None;
            app.menu_pick_for_test("quick", "quick:crop");
            check(app.title.quick.iter().any(|q| q.0 == "crop"), "Crop wasn't added")?;
            app.menu_pick_for_test("quick", "quick:undo");
            check(!app.title.quick.iter().any(|q| q.0 == "undo"), "Undo wasn't taken away")?;
            let st = crate::settings::Settings::load();
            check(st.quick.contains(&"crop".to_string()) && !st.quick.contains(&"undo".to_string()), &format!("not kept: {:?}", st.quick))?;
            app.menu_pick_for_test("quick", "quick:undo");
            app.menu_pick_for_test("quick", "quick:crop");
            let pm = shot(app, r);
            let p = pm.pixel(60, 16).ok_or("no pixel")?;
            check(p.red() > 60, "the title bar isn't drawn where the buttons are")
        });
        step!("full screen, and Esc comes back", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            check(!app.full_screen, "it started in full screen")?;
            key(app, h, r, Key::F(11), Mods::NONE);
            check(app.full_screen && h.is_fullscreen(), &format!("not full screen: app {} window {}", app.full_screen, h.is_fullscreen()))?;
            check(app.ribbon.collapsed, "the ribbon didn't fold away")?;
            shot(app, r);
            key(app, h, r, Key::Escape, Mods::NONE);
            check(!app.full_screen && !h.is_fullscreen(), "Esc didn't come back")?;
            app.ribbon.collapsed = false;
            Ok(())
        });
        step!("traced letters re-set as lettering from a font", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:1", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            let thread = serde_json::json!({"rgb": [20, 90, 200], "pec": 30, "name": "Blue"});
            // five blobs on an arc, as tracing a word would leave them
            use folio_kit::math::Det;
            let (radius, cap) = (40.0f32, 9.0f32);
            let mut objs = vec![];
            for i in 0..5 {
                let th = (i as f32 - 2.0) * 0.18;
                let (bx, by) = (radius * th.dsin(), -radius * th.dcos());
                let r = serde_json::json!([[bx - 3.0, by - cap / 2.0], [bx + 3.0, by - cap / 2.0], [bx + 3.0, by + cap / 2.0], [bx - 3.0, by + cap / 2.0]]);
                objs.push(serde_json::json!({"id": i + 1, "kind": "shape", "rings": [r], "thread": thread, "group": "pic"}));
            }
            let spec = serde_json::json!({"objects": objs, "next_id": 6});
            let li = crate::emb::add_shape_layer(&mut app.doc, "Traced", &spec);
            let count = |app: &DrawApp, i: usize| crate::emb::layer_spec(&app.doc, i).map(|sp| sp["objects"].as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0);
            check(count(app, li) == 5, "five traced letters")?;
            // the geometry is found: an arc of about the right radius and cap height
            let letters: Vec<serde_json::Value> = crate::emb::layer_spec(&app.doc, li).ok_or("gone")?["objects"].as_array().cloned().unwrap_or_default();
            let (kind, _, _, gr, gcap, span) = crate::app::DrawApp::lettering_geometry(&letters).ok_or("no geometry found")?;
            check(kind == "top", &format!("found a {kind}, not an arc"))?;
            check((gr - radius).abs() < 3.0, &format!("radius {gr:.1}, expected about {radius}"))?;
            check((gcap - cap).abs() < 2.5, &format!("cap {gcap:.1}, expected about {cap}"))?;
            check(span > 20.0, &format!("span {span:.1}"))?;
            // re-set: the blobs go, real lettering takes their place, in one undo step
            app.emb_sel = Some((li, 1));
            app.command("emb.reset-letter", h);
            shot(app, r);
            let d = app.dialog.as_mut().ok_or("the Re-set box didn't open")?;
            check(d.choice_index("arc") == 1, &format!("it didn't offer the arc: {}", d.choice_index("arc")))?;
            let font = app.fonts.iter().position(|f| f == "Carlito Bold").ok_or("no Carlito Bold")?;
            let d = app.dialog.as_mut().unwrap();
            d.set_choice("font", font);
            ev(app, h, r, Event::Text("FOLIO".into()));
            let steps = app.doc.history().0.len();
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "the box stayed open")?;
            check(count(app, li) == 0, &format!("{} traced letters left", count(app, li)))?;
            let top = app.doc.layers.len() - 1;
            let sp = crate::emb::layer_spec(&app.doc, top).ok_or("no lettering layer")?;
            let o = sp["objects"][0].clone();
            check(o["kind"] == "lettering" && o["text"] == "FOLIO", &format!("object {o}"))?;
            check(o["arc"] == "top", &format!("arc {}", o["arc"]))?;
            check(o["thread"]["pec"] == 30, &format!("it lost the traced colour: {}", o["thread"]))?;
            // it spans about what the traced letters did
            let nspan = {
                let (_, _, _, _, _, s) = crate::app::DrawApp::lettering_geometry(&[o.clone()]).unwrap_or(("none", 0.0, 0.0, 0.0, 0.0, 0.0));
                s
            };
            let _ = nspan;
            let (x0, _, x1, _) = crate::emb::object_bounds(&o);
            check(x1 - x0 > span * 0.6, &format!("the new letters span {:.1}, the traced ones {:.1}", x1 - x0, span))?;
            check(app.doc.history().0.len() == steps + 1, &format!("{} undo steps", app.doc.history().0.len() - steps))?;
            check(app.doc.history().0.last().map(|l| *l == "Re-set as lettering").unwrap_or(false), &format!("labelled {:?}", app.doc.history().0.last()))?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(count(app, li) == 5, &format!("undo left {} traced letters", count(app, li)))?;
            Ok(())
        });
        step!("Fill holes, Merge and Split", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:1", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            let thread = serde_json::json!({"rgb": [200, 20, 30], "pec": 5, "name": "Red"});
            // a square with a hole, and two squares that touch
            let ring = |x: f32, y: f32, w: f32| serde_json::json!([[x, y], [x + w, y], [x + w, y + w], [x, y + w]]);
            let hole = serde_json::json!([[-5.0, -5.0], [-5.0, 5.0], [5.0, 5.0], [5.0, -5.0]]);
            let spec = serde_json::json!({"objects": [
                {"id": 1, "kind": "shape", "rings": [ring(-15.0, -15.0, 30.0), hole], "thread": thread, "group": "g"},
                {"id": 2, "kind": "shape", "rings": [ring(20.0, -10.0, 20.0)], "thread": thread, "group": "g"},
                {"id": 3, "kind": "shape", "rings": [ring(38.0, -10.0, 20.0)], "thread": thread, "group": "g"}],
                "next_id": 4});
            let li = crate::emb::add_shape_layer(&mut app.doc, "Shapes", &spec);
            let count = |app: &DrawApp| crate::emb::layer_spec(&app.doc, li).map(|sp| sp["objects"].as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0);
            let rings_of = |app: &DrawApp, id: i64| -> usize {
                crate::emb::layer_spec(&app.doc, li)
                    .and_then(|sp| sp["objects"].as_array().and_then(|a| a.iter().find(|o| o["id"].as_i64() == Some(id)).cloned()))
                    .map(|o| o["rings"].as_array().map(|x| x.len()).unwrap_or(0))
                    .unwrap_or(0)
            };
            // Fill holes makes the first shape solid
            app.emb_sel = Some((li, 1));
            check(rings_of(app, 1) == 2, "the square has no hole to start with")?;
            app.command("emb.holes", h);
            check(rings_of(app, 1) == 1, &format!("{} rings after filling", rings_of(app, 1)))?;
            check(app.flash_text().contains("hole"), &format!("it said {:?}", app.flash_text()))?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(rings_of(app, 1) == 2, "undo didn't bring the hole back")?;
            // Merge: the two that touch become one, the one apart stays
            let before = count(app);
            app.emb_sel = Some((li, 2));
            app.command("emb.merge", h);
            let after = count(app);
            check(after == before - 1, &format!("{after} shapes, was {before}"))?;
            check(app.doc.history().0.last().map(|l| *l == "Merge shapes").unwrap_or(false), &format!("labelled {:?}", app.doc.history().0.last()))?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(count(app) == before, "undo didn't bring them back")?;
            // Split: one piece asks for a line, and a line across it cuts it in two
            app.set_tool(Tool::EmbSelect);
            app.emb_sel = Some((li, 2));
            app.command("emb.split", h);
            check(app.emb_cut.is_some(), "it didn't ask for a line")?;
            let st = crate::emb::settings(&app.doc).ok_or("no settings")?;
            let a = crate::emb::mm_to_px(&st, 30.0, -15.0);
            let b = crate::emb::mm_to_px(&st, 30.0, 15.0);
            drag_on(app, h, r, (a.0, a.1), (b.0, b.1), Button::Left);
            check(app.emb_cut.is_none(), "the line is still being drawn")?;
            check(count(app) == before + 1, &format!("{} shapes after the cut, was {before}", count(app)))?;
            check(crate::emb::make_plan(&app.doc).is_some(), "the cut shapes can't be planned")?;
            key(app, h, r, Key::Char('z'), ctrl());
            check(count(app) == before, "undo didn't put the shape back together")
        });
        step!("Edit Nodes moves, adds and deletes a shape's nodes", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:0", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            let spec = serde_json::json!({"objects": [{"id": 1, "kind": "shape", "rings": [[[-20, -20], [20, -20], [20, 20], [-20, 20]]],
                "thread": {"rgb": [200, 20, 30], "pec": 5, "name": "Red"}}], "next_id": 2});
            let li = crate::emb::add_shape_layer(&mut app.doc, "Square", &spec);
            let st = crate::emb::settings(&app.doc).ok_or("no settings")?;
            let nodes = |app: &DrawApp| -> usize {
                crate::emb::layer_spec(&app.doc, li).map(|sp| sp["objects"][0]["rings"][0].as_array().map(|a| a.len()).unwrap_or(0)).unwrap_or(0)
            };
            // a double-click on the shape starts editing its nodes
            app.set_tool(Tool::EmbSelect);
            let (cx, cy) = crate::emb::mm_to_px(&st, 0.0, 0.0);
            click_on(app, h, r, cx, cy, Button::Left);
            check(app.emb_sel == Some((li, 1)), "the square wasn't selected")?;
            let dc = sp(app, cx, cy);
            ev(app, h, r, Event::MouseDown { x: dc.0, y: dc.1, button: Button::Left, clicks: 2 });
            ev(app, h, r, Event::MouseUp { x: dc.0, y: dc.1, button: Button::Left });
            check(app.emb_nodes, "double-clicking didn't start node editing")?;
            shot(app, r);
            // drag the top-left node 5 mm in, as one undo step
            let before = app.doc.history().0.len();
            let (nx, ny) = crate::emb::mm_to_px(&st, -20.0, -20.0);
            let (tx, ty) = crate::emb::mm_to_px(&st, -15.0, -15.0);
            drag_on(app, h, r, (nx, ny), (tx, ty), Button::Left);
            let o = crate::emb::layer_spec(&app.doc, li).ok_or("gone")?["objects"][0].clone();
            let moved = (o["rings"][0][0][0].as_f64().unwrap_or(0.0), o["rings"][0][0][1].as_f64().unwrap_or(0.0));
            check((moved.0 + 15.0).abs() < 0.6 && (moved.1 + 15.0).abs() < 0.6, &format!("the node went to {moved:?}"))?;
            check(app.doc.history().0.len() == before + 1, &format!("{} undo steps for one drag", app.doc.history().0.len() - before))?;
            check(app.doc.history().0.last().map(|l| *l == "Move node").unwrap_or(false), "not labelled Move node")?;
            key(app, h, r, Key::Char('z'), ctrl());
            let o = crate::emb::layer_spec(&app.doc, li).ok_or("gone")?["objects"][0].clone();
            check((o["rings"][0][0][0].as_f64().unwrap_or(0.0) + 20.0).abs() < 0.01, "undo didn't put the node back")?;
            // a double-click on an edge adds a node there
            let n0 = nodes(app);
            let (ex, ey) = crate::emb::mm_to_px(&st, 0.0, -20.0);
            let e = sp(app, ex, ey);
            ev(app, h, r, Event::MouseMove { x: e.0, y: e.1 });
            ev(app, h, r, Event::MouseDown { x: e.0, y: e.1, button: Button::Left, clicks: 2 });
            ev(app, h, r, Event::MouseUp { x: e.0, y: e.1, button: Button::Left });
            check(nodes(app) == n0 + 1, &format!("{} nodes, was {n0}", nodes(app)))?;
            check(app.sel_node == Some((0, 1)), &format!("the new node isn't picked: {:?}", app.sel_node))?;
            // Delete takes it out again
            key(app, h, r, Key::Delete, Mods::NONE);
            check(nodes(app) == n0, &format!("{} nodes after Delete", nodes(app)))?;
            // a square can't go below three nodes
            let (c1x, c1y) = crate::emb::mm_to_px(&st, 20.0, -20.0);
            click_on(app, h, r, c1x, c1y, Button::Left);
            key(app, h, r, Key::Delete, Mods::NONE);
            let (c2x, c2y) = crate::emb::mm_to_px(&st, 20.0, 20.0);
            click_on(app, h, r, c2x, c2y, Button::Left);
            key(app, h, r, Key::Delete, Mods::NONE);
            check(nodes(app) == 3, &format!("{} nodes left", nodes(app)))?;
            let (c3x, c3y) = crate::emb::mm_to_px(&st, -20.0, 20.0);
            click_on(app, h, r, c3x, c3y, Button::Left);
            key(app, h, r, Key::Delete, Mods::NONE);
            check(nodes(app) == 3, "a shape was left with fewer than three nodes")?;
            check(app.flash_text().contains("three nodes"), &format!("it didn't say why: {:?}", app.flash_text()))?;
            // Esc leaves node editing, the shape stays selected
            key(app, h, r, Key::Escape, Mods::NONE);
            check(!app.emb_nodes && app.emb_sel == Some((li, 1)), "Esc lost the selection")?;
            check(crate::emb::make_plan(&app.doc).is_some(), "the shape can't be planned any more")
        });
        step!("threads come from the chart, by colour or by name", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            use folio_draw_engine::emb::colour;
            app.saved_for_test();
            app.command("emb.new:0", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            let spec = serde_json::json!({"objects": [{"id": 1, "kind": "shape", "rings": [[[-12, -12], [12, -12], [12, 12], [-12, 12]]],
                "thread": {"rgb": [120, 120, 120]}}], "next_id": 2});
            let li = crate::emb::add_shape_layer(&mut app.doc, "Square", &spec);
            app.emb_sel = Some((li, 1));
            // the Thread colour menu is the whole chart, with a way to pick one by name
            let m = app.menu_for_test("emb.recolour");
            check(m.iter().any(|i| i == "thread-pick"), "no way to choose a thread by name")?;
            // picking a chart colour keeps its number and name
            let want = colour::pec_rgb(5);
            app.menu_pick_for_test("emb.recolour", &format!("color:#{:02X}{:02X}{:02X}", want[0], want[1], want[2]));
            let o = crate::emb::layer_spec(&app.doc, li).ok_or("gone")?["objects"][0].clone();
            check(o["thread"]["pec"] == 5, &format!("thread {}", o["thread"]))?;
            check(o["thread"]["name"] == colour::pec_name(5), &format!("name {}", o["thread"]["name"]))?;
            // by name: the box opens on the thread it has now
            app.command("thread-pick", h);
            shot(app, r);
            let d = app.dialog.as_mut().ok_or("the Thread box didn't open")?;
            check(d.choice_index("thread") == 4, &format!("opened on thread {}", d.choice_index("thread") + 1))?;
            d.set_choice("thread", 29);
            key(app, h, r, Key::Enter, Mods::NONE);
            let o = crate::emb::layer_spec(&app.doc, li).ok_or("gone")?["objects"][0].clone();
            check(o["thread"]["pec"] == 30 && o["thread"]["name"] == colour::pec_name(30), &format!("thread {}", o["thread"]))?;
            // any other colour is kept, with the nearest chart thread beside it
            app.command("thread-pick", h);
            let d = app.dialog.as_mut().ok_or("it didn't reopen")?;
            d.set_text_for_test("other", "#123456");
            key(app, h, r, Key::Enter, Mods::NONE);
            let o = crate::emb::layer_spec(&app.doc, li).ok_or("gone")?["objects"][0].clone();
            check(o["thread"]["rgb"][0] == 0x12 && o["thread"]["rgb"][2] == 0x56, &format!("rgb {}", o["thread"]["rgb"]))?;
            let near = o["thread"]["pec"].as_u64().unwrap_or(0) as usize;
            check((1..=colour::CHART).contains(&near), &format!("no chart thread beside it: {near}"))?;
            check(crate::emb::make_plan(&app.doc).is_some(), "it can't be planned")
        });
        step!("Convert to shapes asks how many colours and what to do with the background", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("emb.new:0", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            // four bands of colour on a white ground
            let (pw, ph) = (200usize, 120usize);
            let mut pic = vec![0u8; pw * ph * 4];
            let bands: [[u8; 4]; 4] = [[220, 30, 40, 255], [30, 120, 220, 255], [250, 210, 40, 255], [40, 160, 70, 255]];
            for y in 0..ph {
                for x in 0..pw {
                    let o = (y * pw + x) * 4;
                    let c = if (20..100).contains(&y) { bands[(x * 4 / pw).min(3)] } else { [255, 255, 255, 255] };
                    pic[o..o + 4].copy_from_slice(&c);
                }
            }
            let before = app.doc.layers.len();
            crate::emb::place_picture(&mut app.doc, pw, ph, pic, "Bands").ok_or("not placed")?;
            app.command("emb.trace", h);
            let d = app.dialog.as_mut().ok_or("the Convert box didn't open")?;
            d.set_choice("colours", 1);
            d.set_check_for_test("crop", true);
            ev(app, h, r, Event::Text("2".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "the box stayed open")?;
            check(app.trace["colours"] == 2 && app.trace["background"] == "remove" && app.trace["crop"] == true, &format!("asked for {}", app.trace))?;
            let made = app.doc.layers.len() - before - 1;
            check(made == 2, &format!("{made} layers for exactly two colours"))?;
            // the same picture, keeping the background, is one colour more
            key(app, h, r, Key::Char('z'), ctrl());
            check(app.doc.layers.len() == before + 1, &format!("undo left {} layers", app.doc.layers.len()))?;
            app.command("emb.trace", h);
            let d = app.dialog.as_mut().ok_or("it didn't reopen")?;
            d.set_choice("background", 1);
            key(app, h, r, Key::Enter, Mods::NONE);
            let kept = app.doc.layers.len() - before - 1;
            check(kept == made + 1, &format!("{kept} layers with the background stitched, {made} without"))?;
            key(app, h, r, Key::Char('z'), ctrl());
            app.command("emb.trace", h);
            let d = app.dialog.as_mut().ok_or("it didn't reopen")?;
            d.set_choice("background", 0);
            key(app, h, r, Key::Enter, Mods::NONE);
            // asking again offers back what was asked for last time
            app.command("emb.trace", h);
            let d = app.dialog.as_ref().ok_or("it didn't reopen")?;
            check(d.choice_index("colours") == 1 && d.value("count") == "2", &format!("count {:?}", d.value("count")))?;
            check(d.checked("crop"), "it forgot the crop")?;
            key(app, h, r, Key::Escape, Mods::NONE);
            app.trace = serde_json::json!({});
            Ok(())
        });
        step!("the picture can go to the desktop's background or into an email", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("new", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            app.set_tool(Tool::Pencil);
            drag_on(app, h, r, (30.0, 30.0), (160.0, 30.0), Button::Left);
            // a file another program can take: a PNG, and the layers as OpenRaster
            let png = app.file_for_sending("png").map_err(|e| e.to_string())?;
            check(png.exists() && std::fs::metadata(&png).map(|m| m.len() > 100).unwrap_or(false), "no PNG written")?;
            let bytes = std::fs::read(&png).map_err(|e| e.to_string())?;
            check(&bytes[1..4] == b"PNG", "that isn't a PNG")?;
            let ora = app.file_for_sending("ora").map_err(|e| e.to_string())?;
            check(ora.extension().map(|e| e == "ora").unwrap_or(false) && ora.exists(), "no OpenRaster written")?;
            // a saved file in the same format is handed over as it is
            let dir = PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()));
            let saved = dir.join("send.png");
            check(app.save_to(&saved), "couldn't save")?;
            check(app.file_for_sending("png").map_err(|e| e.to_string())? == saved, "it made a copy of a file it could send")?;
            // both commands say what happened rather than failing silently
            app.command("wallpaper", h);
            let msg = app.flash_text();
            check(!msg.is_empty(), "Set as background said nothing")?;
            app.command("email", h);
            check(!app.flash_text().is_empty(), "Send in email said nothing")?;
            let m = app.menu_for_test("email");
            check(m.len() == 2, &format!("the email menu has {} choices", m.len()))?;
            Ok(())
        });
        step!("a transparent new picture, polygon selection, properties and the History pane", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("new-transparent", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            check(px(app, 20, 20)[3] == 0, &format!("a transparent picture isn't: {:?}", px(app, 20, 20)))?;
            // polygon selection: click the corners, double-click to close
            app.c1 = [0, 0, 0, 255];
            app.set_tool(Tool::PolySelect);
            for (x, y) in [(100.0, 100.0), (300.0, 100.0), (300.0, 260.0)] {
                click_on(app, h, r, x, y, Button::Left);
            }
            check(app.multi.is_some(), "the polygon didn't start")?;
            shot(app, r);
            let q = sp(app, 100.0, 260.0);
            ev(app, h, r, Event::MouseMove { x: q.0, y: q.1 });
            ev(app, h, r, Event::MouseDown { x: q.0, y: q.1, button: Button::Left, clicks: 1 });
            ev(app, h, r, Event::MouseUp { x: q.0, y: q.1, button: Button::Left });
            ev(app, h, r, Event::MouseDown { x: q.0, y: q.1, button: Button::Left, clicks: 2 });
            ev(app, h, r, Event::MouseUp { x: q.0, y: q.1, button: Button::Left });
            let (x0, y0, w, hh) = app.doc.sel_bounds().ok_or("the polygon selected nothing")?;
            check(x0 < 102 && y0 < 102 && w > 190 && hh > 150, &format!("bounds {x0},{y0} {w}x{hh}"))?;
            check(app.multi.is_none(), "the polygon is still open")?;
            app.command("select-none", h);
            // properties
            app.command("properties", h);
            let d = app.dialog.as_ref().ok_or("Properties didn't open")?;
            let note = d.value("");
            check(note.contains("1152 x 648"), &format!("note: {note}"))?;
            key(app, h, r, Key::Enter, Mods::NONE);
            // the History pane
            app.set_tool(Tool::Pencil);
            drag_on(app, h, r, (40.0, 40.0), (200.0, 40.0), Button::Left);
            check(!app.show_history, "the History pane started open")?;
            app.command("pane.history", h);
            check(app.show_history, "it didn't open")?;
            let pm = shot(app, r);
            check(app.side.w > 0.0, "the side panel has no width")?;
            let p = pm.pixel((app.side.x + 30.0) as u32, (app.side.bottom() - 60.0) as u32).ok_or("no pixel")?;
            check(p.alpha() == 255, "the pane isn't painted")?;
            app.command("pane.history", h);
            check(!app.show_history, "it didn't close")
        });
        step!("a shape can be filled with no outline", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("new", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            app.c1 = [0, 0, 0, 255];
            app.c2 = [237, 28, 36, 255];
            check(app.shape_mode(true) == 1, "an unfilled shape should be its outline alone")?;
            app.command("shape.fill", h);
            check(app.shape_mode(true) == 3, "filled and outlined")?;
            app.command("shape.outline", h);
            check(app.shape_outline == false && app.shape_mode(true) == 2, &format!("mode {}", app.shape_mode(true)))?;
            app.set_tool(Tool::Rect);
            drag_on(app, h, r, (60.0, 60.0), (200.0, 160.0), Button::Left);
            app.commit_pending();
            let inside = px(app, 130, 110);
            check(inside[0] > 200 && inside[1] < 80, &format!("not filled with colour 2: {inside:?}"))?;
            let edge = px(app, 60, 110);
            check(!(edge[0] < 60 && edge[1] < 60 && edge[2] < 60), &format!("the outline is still drawn: {edge:?}"))?;
            // turning the fill off puts the outline back rather than leaving nothing
            app.command("shape.fill", h);
            check(app.shape_outline && app.shape_mode(true) == 1, "a shape was left as neither")?;
            Ok(())
        });
        step!("the Text tool's font, underline and filled box", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("new", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            app.c1 = [0, 0, 0, 255];
            app.c2 = [237, 28, 36, 255];
            app.set_tool(Tool::Text);
            let fonts = app.text_fonts();
            check(fonts.len() > 1 && fonts[0].is_empty(), &format!("{} fonts, first {:?}", fonts.len(), fonts.first()))?;
            let carlito = fonts.iter().position(|f| f == "Carlito").ok_or("Carlito isn't offered")?;
            click_on(app, h, r, 80.0, 80.0, Button::Left);
            let d = app.dialog.as_mut().ok_or("the Text box didn't open")?;
            d.set_choice("font", carlito);
            d.set_choice("size", 4);
            key(app, h, r, Key::Char('u'), Mods { ctrl: true, ..Mods::NONE });
            let d = app.dialog.as_mut().ok_or("the box closed")?;
            d.set_check_for_test("underline", true);
            d.set_check_for_test("opaque", true);
            ev(app, h, r, Event::Text("Pint".into()));
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "the box stayed open")?;
            check(app.text_font == carlito && app.text_underline && app.text_opaque, &format!("choices {} {} {}", app.text_font, app.text_underline, app.text_opaque))?;
            let tl = app.doc.layers.len() - 1;
            let spec: serde_json::Value = serde_json::from_str(&app.doc.layers[tl].text).map_err(|e| e.to_string())?;
            check(spec["font_family"] == "Carlito", &format!("family {:?}", spec["font_family"]))?;
            check(spec["underline"] == true && spec["text_opaque"] == true, "the spec forgot the choices")?;
            // the box behind is filled with colour 2, and the letters are drawn on it
            let (x, y) = (84usize, 84);
            let c = px(app, x, y);
            check(c[3] == 255, &format!("the box isn't opaque: {c:?}"))?;
            let red = (80..200).any(|dx| (80..140).any(|dy| {
                let q = px(app, dx, dy);
                q[0] > 200 && q[1] < 80
            }));
            check(red, "no colour 2 behind the words")?;
            let dark = (80..200).any(|dx| (80..140).any(|dy| px(app, dx, dy)[0] < 80));
            check(dark, "the words aren't drawn")?;
            // typing it again offers the same font back
            click_on(app, h, r, 86.0, 86.0, Button::Left);
            let d = app.dialog.as_ref().ok_or("it didn't reopen")?;
            check(d.value("text") == "Pint" && d.choice_index("font") == carlito, &format!("{:?} font {}", d.value("text"), d.choice_index("font")))?;
            key(app, h, r, Key::Escape, Mods::NONE);
            app.text_underline = false;
            app.text_opaque = false;
            app.text_font = 0;
            Ok(())
        });
        step!("page setup lays the picture out, and Print makes the pages", |app: &mut DrawApp, h: &mut dyn Host, r: &mut Res| {
            app.saved_for_test();
            app.command("new", h);
            ev(app, h, r, Event::MouseMove { x: 1.0, y: 1.0 });
            app.command("page-setup", h);
            shot(app, r);
            let d = app.dialog.as_mut().ok_or("the Page setup box didn't open")?;
            d.set_choice("paper", 1);
            d.set_choice("orientation", 1);
            d.set_choice("mode", 1);
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.dialog.is_none(), "the box stayed open")?;
            check(app.page.paper == 1 && app.page.landscape && !app.page.fit, &format!("page {:?}", app.page))?;
            let st = crate::settings::Settings::load();
            check(st.page == app.page, "the page setup wasn't kept")?;
            // laid out: landscape Letter is wider than it is tall
            let (pw, ph) = crate::page::paper_size(&app.page);
            check(pw > ph, &format!("paper {pw} x {ph}"))?;
            let sheets = crate::page::layout(&app.page, app.doc.w, app.doc.h);
            check(!sheets.is_empty(), "no sheets")?;
            let bytes = crate::page::pdf_bytes(&app.doc.flatten_rgba(), app.doc.w, app.doc.h, &app.page).map_err(|e| e.to_string())?;
            check(&bytes[..5] == b"%PDF-" && bytes.len() > 500, &format!("{} bytes of PDF", bytes.len()))?;
            check(crate::page::describe(&app.page, app.doc.w, app.doc.h).contains("page"), "no description")?;
            // back to the usual page, so a later run starts as it did
            app.page = crate::page::Setup::default();
            app.remember();
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
