//! The window test: drives Page as a person would, one step per tick, in a
//! real window or in memory (`--headless`).

use crate::app::{PageApp, Res};
use folio_kit::gfx::{self, sk};
use folio_kit::platform::{Button, Event, Host, Key, Mods};
use std::path::PathBuf;

type Step = Box<dyn FnMut(&mut PageApp, &mut dyn Host, &mut Res) -> Result<(), String>>;

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

fn ev(app: &mut PageApp, host: &mut dyn Host, e: Event) {
    app.handle(&e, host);
}

fn shot(app: &mut PageApp, r: &mut Res) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(app.w.max(1.0) as u32, app.h.max(1.0) as u32).unwrap();
    app.paint(&mut pm, r);
    pm
}

fn key(app: &mut PageApp, host: &mut dyn Host, k: Key, mods: Mods) {
    ev(app, host, Event::Key { key: k.clone(), mods, down: true, repeat: false });
    ev(app, host, Event::Key { key: k, mods, down: false, repeat: false });
}

fn ctrl() -> Mods {
    Mods { ctrl: true, ..Mods::NONE }
}

fn typed(app: &mut PageApp, host: &mut dyn Host, text: &str) {
    ev(app, host, Event::Text(text.to_string()));
}

fn ribbon_click(app: &mut PageApp, host: &mut dyn Host, r: &mut Res, tab: &str, id: &str) -> Result<(), String> {
    app.ribbon.select_tab(tab);
    shot(app, r);
    let rr = app.ribbon.rect_of(id).ok_or(format!("ribbon button {id} not shown"))?;
    ev(app, host, Event::MouseMove { x: rr.cx(), y: rr.cy() });
    ev(app, host, Event::MouseDown { x: rr.cx(), y: rr.cy(), button: Button::Left, clicks: 1 });
    ev(app, host, Event::MouseUp { x: rr.cx(), y: rr.cy(), button: Button::Left });
    Ok(())
}

/// The ink on the page area: how much of the document is showing.
fn page_ink(app: &mut PageApp, r: &mut Res) -> usize {
    let pm = shot(app, r);
    let c = app.canvas_rect();
    let mut n = 0;
    for y in c.y as u32..(c.bottom() as u32).min(pm.height()) {
        for x in c.x as u32..(c.right() as u32).min(pm.width()) {
            if let Some(p) = pm.pixel(x, y) {
                let p = p.demultiply();
                if p.red() < 120 && p.green() < 120 && p.blue() < 120 {
                    n += 1;
                }
            }
        }
    }
    n
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
        let d_save = dir.clone();
        let d_pdf = dir.clone();

        step!("the window shows a page", |app: &mut PageApp, _h: &mut dyn Host, r: &mut Res| {
            let pm = shot(app, r);
            check(app.canvas_rect().h > 200.0, "no room for the pages")?;
            check(!app.page_rects_now().is_empty(), "no page on screen")?;
            let rr = app.page_rects_now()[0].1;
            let p = pm.pixel(rr.cx() as u32, (rr.y + 4.0) as u32).ok_or("no pixel")?.demultiply();
            check(p.red() > 230 && p.green() > 230 && p.blue() > 230, "the page isn't white")
        });
        step!("typing puts words on the page", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let before = page_ink(app, r);
            typed(app, h, "Hello from Folio Page");
            let after = page_ink(app, r);
            check(after > before + 50, &format!("typing drew nothing ({before} -> {after})"))?;
            check(app.ed.word_count() >= 4, &format!("{} words counted", app.ed.word_count()))
        });
        step!("Enter starts a new paragraph", |app: &mut PageApp, h: &mut dyn Host, _r: &mut Res| {
            let before = app.ed.page_count();
            key(app, h, Key::Enter, Mods::NONE);
            typed(app, h, "A second paragraph");
            check(app.ed.page_count() >= before, "pages went missing")?;
            check(app.ed.text_of_document().contains("A second paragraph"), "the text isn't in the document")
        });
        step!("Ctrl+Z takes it back, Ctrl+Y brings it again", |app: &mut PageApp, h: &mut dyn Host, _r: &mut Res| {
            key(app, h, Key::Char('z'), ctrl());
            check(!app.ed.text_of_document().contains("A second paragraph"), "undo left the text")?;
            key(app, h, Key::Char('y'), ctrl());
            check(app.ed.text_of_document().contains("A second paragraph"), "redo lost the text")
        });
        step!("bold from the ribbon marks the selection", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            key(app, h, Key::Char('a'), ctrl());
            ribbon_click(app, h, r, "home", "bold")?;
            check(app.ed.toggle_state("w:b"), "the text didn't go bold")?;
            ribbon_click(app, h, r, "home", "bold")?;
            check(!app.ed.toggle_state("w:b"), "bold didn't come off")
        });
        step!("centring the paragraph moves the words along", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "align-center")?;
            check(app.ed.current_align() == "center", &format!("alignment is {}", app.ed.current_align()))?;
            ribbon_click(app, h, r, "home", "align-left")?;
            check(app.ed.current_align() == "left", "it didn't go back to the left")
        });
        step!("clicking in the text puts the caret there", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let rr = app.page_rects_now()[0].1;
            let (x, y) = (rr.x + rr.w * 0.3, rr.y + rr.h * 0.12);
            ev(app, h, Event::MouseMove { x, y });
            ev(app, h, Event::MouseDown { x, y, button: Button::Left, clicks: 1 });
            ev(app, h, Event::MouseUp { x, y, button: Button::Left });
            check(!app.ed.has_selection(), "a click shouldn't select anything")?;
            Ok(())
        });
        step!("zoom changes how big the page is drawn", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            shot(app, r);
            let before = app.page_rects_now()[0].1.w;
            ribbon_click(app, h, r, "view", "zoom-in")?;
            shot(app, r);
            let after = app.page_rects_now()[0].1.w;
            check(after > before, &format!("page stayed {before} wide"))?;
            ribbon_click(app, h, r, "view", "zoom-100")?;
            Ok(())
        });
        step!("the marks button shows the paragraph marks", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let before = page_ink(app, r);
            ribbon_click(app, h, r, "view", "marks")?;
            let after = page_ink(app, r);
            check(after > before, "nothing extra was drawn")?;
            ribbon_click(app, h, r, "view", "marks")?;
            Ok(())
        });
        step!("the document saves and opens again", move |app: &mut PageApp, _h: &mut dyn Host, _r: &mut Res| {
            let p = d_save.join("typed.docx");
            app.save(&p);
            check(p.is_file(), "no file was written")?;
            let want = app.ed.text_of_document();
            app.open_path(&p);
            check(app.ed.text_of_document().trim() == want.trim(), "the text changed on the way out and back")
        });
        step!("Export PDF writes a PDF of the pages", move |app: &mut PageApp, _h: &mut dyn Host, _r: &mut Res| {
            let p = d_pdf.join("typed.pdf");
            app.export_pdf(&p);
            check(p.is_file(), "no PDF was written")?;
            let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
            check(bytes.starts_with(b"%PDF-"), "that isn't a PDF")?;
            check(bytes.len() > 1000, "the PDF is suspiciously small")
        });
        UiTest { steps, i: 0, done: false, fails: vec![], passed: 0, dir }
    }

    pub fn step(&mut self, app: &mut PageApp, host: &mut dyn Host, r: &mut Res) {
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
