//! The window test: drives Show as a person would, one step per tick.
use crate::app::{Res, ShowApp};
use folio_kit::gfx::{self, sk};
use folio_kit::platform::{Button, Event, Host, Key, Mods};
use std::path::PathBuf;

type Step = Box<dyn FnMut(&mut ShowApp, &mut dyn Host, &mut Res) -> Result<(), String>>;

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

fn ev(app: &mut ShowApp, host: &mut dyn Host, r: &mut Res, e: Event) {
    app.handle_test(&e, host, r);
}

fn shot(app: &mut ShowApp, r: &mut Res) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(app.w.max(1.0) as u32, app.h.max(1.0) as u32).unwrap();
    app.paint(&mut pm, r);
    pm
}

fn key(app: &mut ShowApp, host: &mut dyn Host, r: &mut Res, k: Key, mods: Mods) {
    ev(app, host, r, Event::Key { key: k.clone(), mods, down: true, repeat: false });
    ev(app, host, r, Event::Key { key: k, mods, down: false, repeat: false });
}

fn type_text(app: &mut ShowApp, host: &mut dyn Host, r: &mut Res, text: &str) {
    for c in text.chars() {
        ev(app, host, r, Event::Key { key: Key::Char(c), mods: Mods::NONE, down: true, repeat: false });
        ev(app, host, r, Event::Text(c.to_string()));
        ev(app, host, r, Event::Key { key: Key::Char(c), mods: Mods::NONE, down: false, repeat: false });
    }
}

fn click(app: &mut ShowApp, host: &mut dyn Host, r: &mut Res, x: f32, y: f32, clicks: u32) {
    ev(app, host, r, Event::MouseMove { x, y });
    ev(app, host, r, Event::MouseDown { x, y, button: Button::Left, clicks });
    ev(app, host, r, Event::MouseUp { x, y, button: Button::Left });
}

fn ribbon_click(app: &mut ShowApp, host: &mut dyn Host, r: &mut Res, tab: &str, id: &str) -> Result<(), String> {
    app.ribbon.select_tab(tab);
    shot(app, r);
    let rr = app.ribbon.rect_of(id).ok_or(format!("ribbon button {id} not shown"))?;
    click(app, host, r, rr.cx(), rr.cy(), 1);
    Ok(())
}

fn save_shot(app: &mut ShowApp, r: &mut Res, name: &str) {
    if let Some(dir) = folio_kit::platform::setting("FOLIO_UI_DIR").map(PathBuf::from) {
        let pm = shot(app, r);
        let _ = std::fs::write(dir.join(name), gfx::encode_png(&pm));
    }
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
        step!("the window opens on a blank presentation", |app: &mut ShowApp, _h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            check(!app.deck.slides.is_empty(), "the deck has a slide")?;
            check(app.slide_rect.w > 100.0, "the slide is drawn with room on it")?;
            check(!app.shapes.is_empty(), "the slide's placeholders are there")
        });
        step!("a new slide, and the pane shows both", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            let before = app.deck.slides.len();
            ribbon_click(app, h, r, "home", "slide-new")?;
            check(app.deck.slides.len() == before + 1, &format!("now {} slides", app.deck.slides.len()))?;
            check(app.slide == before, "and it is the one being looked at")?;
            shot(app, r);
            check(app.thumb_hits.len() == app.deck.slides.len(), "every slide has a thumbnail")
        });
        step!("clicking a thumbnail goes to that slide", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let t = app.thumb_hits[0].1;
            click(app, h, r, t.cx(), t.cy(), 1);
            check(app.slide == 0, &format!("back on slide 1, not {}", app.slide + 1))
        });
        step!("a text box, typed into", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "insert", "textbox")?;
            let id = *app.sel.first().ok_or("the text box is picked out")?;
            app.start_typing(id);
            type_text(app, h, r, " and more");
            let text = app.deck.paragraphs(app.slide, id).join("\n");
            check(text.contains("and more"), &format!("the box holds {text:?}"))?;
            key(app, h, r, Key::Enter, Mods::NONE);
            type_text(app, h, r, "a second line");
            let paras = app.deck.paragraphs(app.slide, id);
            check(paras.len() == 2, &format!("two paragraphs, not {}", paras.len()))?;
            save_shot(app, r, "ui-textbox.png");
            Ok(())
        });
        step!("Backspace takes a letter out", |app: &mut ShowApp, _h: &mut dyn Host, _r: &mut Res| {
            let id = *app.sel.first().ok_or("a shape is picked out")?;
            let before = app.deck.paragraphs(app.slide, id).join("\n");
            app.backspace();
            let after = app.deck.paragraphs(app.slide, id).join("\n");
            check(after.chars().count() + 1 == before.chars().count(), &format!("{before:?} -> {after:?}"))
        });
        step!("a shape from the Shapes menu", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            app.caret = None;
            let before = app.shapes.len();
            app.command("shape:ellipse", h, r);
            shot(app, r);
            check(app.shapes.len() == before + 1, "the shape went on the slide")?;
            check(!app.sel.is_empty(), "and it is picked out")
        });
        step!("dragging moves what is picked out", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let id = *app.sel.first().ok_or("a shape is picked out")?;
            let (x0, y0, _, _) = app.box_of(id).ok_or("the shape has a box")?;
            let k = app.slide_rect.w / app.page_w() as f32;
            let p = (app.slide_rect.x + (x0 as f32 + 10.0) * k, app.slide_rect.y + (y0 as f32 + 10.0) * k);
            ev(app, h, r, Event::MouseMove { x: p.0, y: p.1 });
            ev(app, h, r, Event::MouseDown { x: p.0, y: p.1, button: Button::Left, clicks: 1 });
            ev(app, h, r, Event::MouseMove { x: p.0 + 40.0, y: p.1 + 20.0 });
            ev(app, h, r, Event::MouseUp { x: p.0 + 40.0, y: p.1 + 20.0, button: Button::Left });
            shot(app, r);
            let (x1, y1, _, _) = app.box_of(id).ok_or("the shape is still there")?;
            check(x1 > x0 + 10.0 && y1 > y0 + 5.0, &format!("it moved from {x0:.0},{y0:.0} to {x1:.0},{y1:.0}"))
        });
        step!("bold goes on the text", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            let id = app.shapes.iter().find(|s| s.has_text).map(|s| s.id).ok_or("a shape with text")?;
            app.sel = vec![id];
            app.caret = None;
            ribbon_click(app, h, r, "home", "bold")?;
            let p = app.deck.run_props_at(app.slide, id, 0, 0).ok_or("the run's properties")?;
            check(p.b == Some(true), &format!("bold is {:?}", p.b))
        });
        step!("presenting, and Escape comes back", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            app.command("present", h, r);
            check(app.presenting, "the show started")?;
            shot(app, r);
            save_shot(app, r, "ui-present.png");
            key(app, h, r, Key::Right, Mods::NONE);
            key(app, h, r, Key::Escape, Mods::NONE);
            check(!app.presenting, "Escape ends it")
        });
        step!("the File pages open and go back", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            app.open_backstage("open");
            shot(app, r);
            check(matches!(app.pane, crate::app::Pane::Backstage(_)), "the File pages are showing")?;
            save_shot(app, r, "ui-file.png");
            key(app, h, r, Key::Escape, Mods::NONE);
            check(matches!(app.pane, crate::app::Pane::None), "Escape goes back to the slide")
        });
        step!("a changed presentation asks before closing", |app: &mut ShowApp, _h: &mut dyn Host, _r: &mut Res| {
            let was = app.test.take();
            app.dirty = true;
            app.ask_to_close();
            let asked = app.dialog.is_some() && !app.exit;
            app.dialog = None;
            app.closing = false;
            app.test = was;
            check(asked, "the window asks to save first")
        });
        step!("saving and opening it again", |app: &mut ShowApp, h: &mut dyn Host, r: &mut Res| {
            let _ = (h, r);
            let dir = PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()));
            let path = dir.join("ui-test.pptx");
            app.path = Some(path.clone());
            check(app.save(), "the presentation was written")?;
            let slides = app.deck.slides.len();
            app.open_path(&path);
            check(app.deck.slides.len() == slides, &format!("it opens again with {slides} slides"))?;
            let png = dir.join("ui-slide.png");
            app.export_png(&png);
            check(png.is_file() && std::fs::metadata(&png).map(|m| m.len() > 1000).unwrap_or(false), "and draws a slide to a PNG")
        });
        UiTest { steps, i: 0, done: false, fails: vec![], passed: 0, dir }
    }

    pub fn step(&mut self, app: &mut ShowApp, host: &mut dyn Host, r: &mut Res) {
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
