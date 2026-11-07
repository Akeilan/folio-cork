//! The window test: drives Sheet as a person would, one step per tick, in a
//! real window or in memory (`--headless`).

use crate::app::{Res, SheetApp};
use folio_kit::gfx::{self, sk};
use folio_kit::platform::{Button, Event, Host, Key, Mods};
use folio_kit::ui::CommandState;
use std::path::PathBuf;

type Step = Box<dyn FnMut(&mut SheetApp, &mut dyn Host, &mut Res) -> Result<(), String>>;

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

fn ev(app: &mut SheetApp, host: &mut dyn Host, r: &mut Res, e: Event) {
    app.handle_inner(&e, host, r);
}

fn shot(app: &mut SheetApp, r: &mut Res) -> sk::Pixmap {
    let mut pm = sk::Pixmap::new(app.w.max(1.0) as u32, app.h.max(1.0) as u32).unwrap();
    app.paint(&mut pm, r);
    pm
}

fn key(app: &mut SheetApp, host: &mut dyn Host, r: &mut Res, k: Key, mods: Mods) {
    ev(app, host, r, Event::Key { key: k.clone(), mods, down: true, repeat: false });
    ev(app, host, r, Event::Key { key: k, mods, down: false, repeat: false });
}

fn ctrl() -> Mods {
    Mods { ctrl: true, ..Mods::NONE }
}

#[allow(dead_code)]
fn typed(app: &mut SheetApp, host: &mut dyn Host, r: &mut Res, text: &str) {
    ev(app, host, r, Event::Text(text.to_string()));
}

fn ribbon_click(app: &mut SheetApp, host: &mut dyn Host, r: &mut Res, tab: &str, id: &str) -> Result<(), String> {
    app.ribbon.select_tab(tab);
    shot(app, r);
    let rr = app.ribbon.rect_of(id).ok_or(format!("ribbon button {id} not shown"))?;
    ev(app, host, r, Event::MouseMove { x: rr.cx(), y: rr.cy() });
    ev(app, host, r, Event::MouseDown { x: rr.cx(), y: rr.cy(), button: Button::Left, clicks: 1 });
    ev(app, host, r, Event::MouseUp { x: rr.cx(), y: rr.cy(), button: Button::Left });
    Ok(())
}

/// Type a run of characters into the grid, as a person does.
fn type_text(app: &mut SheetApp, host: &mut dyn Host, r: &mut Res, text: &str) {
    for c in text.chars() {
        // A keypress on a real window: the key, then the character it stands for.
        ev(app, host, r, Event::Key { key: Key::Char(c), mods: Mods::NONE, down: true, repeat: false });
        ev(app, host, r, Event::Text(c.to_string()));
        ev(app, host, r, Event::Key { key: Key::Char(c), mods: Mods::NONE, down: false, repeat: false });
    }
}

fn click(app: &mut SheetApp, host: &mut dyn Host, r: &mut Res, x: f32, y: f32, clicks: u32) {
    ev(app, host, r, Event::MouseMove { x, y });
    ev(app, host, r, Event::MouseDown { x, y, button: Button::Left, clicks });
    ev(app, host, r, Event::MouseUp { x, y, button: Button::Left });
}

/// Paint the window so the hit areas are current.
fn shot_into(app: &mut SheetApp, r: &mut Res) -> sk::Pixmap {
    shot(app, r)
}

fn save_shot(app: &mut SheetApp, r: &mut Res, name: &str) {
    let dir = folio_kit::platform::setting("FOLIO_UI_DIR").map(PathBuf::from);
    if let Some(dir) = dir {
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
        step!("the window opens on a blank workbook", |app: &mut SheetApp, _h: &mut dyn Host, r: &mut Res| {
            shot_into(app, r);
            check(app.book.sheets.len() >= 1, "a workbook with a sheet")?;
            check(app.cur == (0, 0), "the cursor starts at A1")?;
            check(app.grid.w > 100.0 && app.grid.h > 100.0, "the grid has room on the window")
        });
        step!("typing into a cell, and the formula bar shows it", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            type_text(app, h, r, "Quarter");
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.book.display(0, 0, 0) == "Quarter", &format!("A1 holds {:?}", app.book.display(0, 0, 0)))?;
            check(app.cur == (1, 0), "Enter moves down")?;
            type_text(app, h, r, "1200");
            key(app, h, r, Key::Enter, Mods::NONE);
            type_text(app, h, r, "1890.5");
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.book.display(0, 1, 0) == "1200" && app.book.display(0, 2, 0) == "1890.5",
                  "numbers go in as numbers")
        });
        step!("a formula adds the cells up", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            type_text(app, h, r, "=SUM(A2:A3)");
            key(app, h, r, Key::Enter, Mods::NONE);
            let got = app.book.display(0, 3, 0);
            check(got == "3090.5", &format!("A4 is {got:?}"))?;
            save_shot(app, r, "ui-grid.png");
            Ok(())
        });
        step!("the arrow keys and the mouse move the cursor", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.go_to(0, 0, false);
            key(app, h, r, Key::Down, Mods::NONE);
            key(app, h, r, Key::Right, Mods::NONE);
            check(app.cur == (1, 1), &format!("the cursor is at {:?}", app.cur))?;
            shot_into(app, r);
            let p = app.cell_point(2, 2);
            click(app, h, r, p.0, p.1, 1);
            check(app.cur == (2, 2), &format!("clicking C3 lands on {:?}", app.cur))?;
            key(app, h, r, Key::Down, Mods { shift: true, ..Mods::NONE });
            let rc = app.sel_rect();
            check(rc.r1 == 2 && rc.r2 == 3 && rc.c1 == 2, &format!("Shift+Down selects two cells: {:?}", (rc.r1, rc.r2)))
        });
        step!("Delete clears what is selected", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.go_to(0, 0, false);
            key(app, h, r, Key::Delete, Mods::NONE);
            check(app.book.display(0, 0, 0).is_empty(), "A1 is empty again")?;
            app.command("undo", h, r);
            check(app.book.display(0, 0, 0) == "Quarter", "undo puts it back")
        });
        step!("another sheet, and the tabs show it", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            let before = app.book.sheets.len();
            app.command("sheet-new", h, r);
            check(app.book.sheets.len() == before + 1, "a sheet was added")?;
            shot_into(app, r);
            check(app.tab_hits.len() == app.book.sheets.len(), "every sheet has a tab")?;
            let t = app.tab_hits[0].1;
            click(app, h, r, t.cx(), t.cy(), 1);
            check(app.sheet == 0, "clicking a tab goes to that sheet")
        });
        step!("bold, italic and alignment from the ribbon", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.go_to(0, 0, false);
            ribbon_click(app, h, r, "home", "bold")?;
            check(app.spec_here().bold, "A1 is bold")?;
            ribbon_click(app, h, r, "home", "italic")?;
            check(app.spec_here().italic, "and italic")?;
            ribbon_click(app, h, r, "home", "align-center")?;
            check(app.spec_here().halign == "center", &format!("centred, not {:?}", app.spec_here().halign))?;
            check(app.state().checked("bold"), "the ribbon's Bold shows as on")?;
            ribbon_click(app, h, r, "home", "bold")?;
            check(!app.spec_here().bold, "and it turns off again")
        });
        step!("copy and paste a block of cells", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.go_to(0, 0, false);
            app.go_to(3, 0, true);
            app.command("copy", h, r);
            app.go_to(0, 2, false);
            app.command("paste", h, r);
            check(app.book.display(0, 0, 2) == "Quarter", &format!("C1 holds {:?}", app.book.display(0, 0, 2)))?;
            check(app.book.display(0, 3, 2) == "3090.5", "and the formula came with it, adding up its own column")
        });
        step!("AutoSum totals the column", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.go_to(0, 4, false);
            type_text(app, h, r, "5");
            key(app, h, r, Key::Enter, Mods::NONE);
            type_text(app, h, r, "7");
            key(app, h, r, Key::Enter, Mods::NONE);
            ribbon_click(app, h, r, "home", "autosum")?;
            check(app.editor.text == "=SUM(E1:E2)", &format!("AutoSum offers {:?}", app.editor.text))?;
            key(app, h, r, Key::Enter, Mods::NONE);
            check(app.book.display(0, 2, 4) == "12", &format!("E3 is {:?}", app.book.display(0, 2, 4)))
        });
        step!("a number format from the Number box", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.go_to(1, 0, false);
            app.command("numfmt:#,##0.00", h, r);
            check(app.book.display(0, 1, 0) == "1,200.00", &format!("A2 shows {:?}", app.book.display(0, 1, 0)))?;
            app.command("dec-less", h, r);
            check(app.book.display(0, 1, 0) == "1,200.0", &format!("one place fewer shows {:?}", app.book.display(0, 1, 0)))
        });
        step!("inserting and deleting a row", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.go_to(1, 0, false);
            let was = app.book.display(0, 1, 0);
            ribbon_click(app, h, r, "home", "row-above")?;
            check(app.book.display(0, 1, 0).is_empty(), "the new row is empty")?;
            check(app.book.display(0, 2, 0) == was, "and what was there moved down")?;
            ribbon_click(app, h, r, "home", "row-delete")?;
            check(app.book.display(0, 1, 0) == was, "deleting the row puts it back")
        });
        step!("Find goes to the cell", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.dialog = None;
            app.go_to(5, 5, false);
            key(app, h, r, Key::Char('f'), ctrl());
            shot_into(app, r);
            check(app.dialog.is_some(), "Ctrl+F opens Find")?;
            app.dialog = None;
            app.find.pattern = "Quarter".into();
            app.find_next();
            check(app.cur == (0, 0), &format!("Find lands on A1, not {:?}", app.cur))
        });
        step!("the View tab freezes panes and hides the gridlines", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.go_to(1, 1, false);
            ribbon_click(app, h, r, "view", "freeze")?;
            check(app.freeze == (1, 1), &format!("frozen at {:?}", app.freeze))?;
            check(app.view().freeze_rows == 1, "and the engine draws it frozen")?;
            ribbon_click(app, h, r, "view", "gridlines")?;
            check(!app.show_grid, "the gridlines are off")?;
            save_shot(app, r, "ui-view.png");
            ribbon_click(app, h, r, "view", "gridlines")?;
            ribbon_click(app, h, r, "view", "freeze")?;
            check(app.show_grid && app.freeze == (0, 0), "and both go back")
        });
        step!("the File pages open and go back", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            app.open_backstage("open");
            shot_into(app, r);
            check(matches!(app.pane, crate::app::Pane::Backstage(_)), "the File pages are showing")?;
            save_shot(app, r, "ui-file.png");
            key(app, h, r, Key::Escape, Mods::NONE);
            check(matches!(app.pane, crate::app::Pane::None), "Escape goes back to the sheet")
        });
        step!("exporting the sheet as CSV", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            let _ = h;
            let dir = PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()));
            let path = dir.join("ui-test.csv");
            app.export_csv(&path);
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            check(text.lines().next().map(|l| l.starts_with("Quarter,")).unwrap_or(false),
                  &format!("the first line is {:?}", text.lines().next()))
        });
        step!("a changed workbook asks before closing", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            let _ = (h, r);
            let was = app.test.take();
            app.dirty = true;
            app.ask_to_close();
            let asked = app.dialog.is_some() && !app.exit;
            app.dialog = None;
            app.closing = false;
            app.test = was;
            check(asked, "the window asks to save first")
        });
        step!("saving and opening it again", |app: &mut SheetApp, h: &mut dyn Host, r: &mut Res| {
            let dir = PathBuf::from(folio_kit::platform::setting("FOLIO_UI_DIR").unwrap_or_else(|| "/tmp".into()));
            let path = dir.join("ui-test.xlsx");
            app.path = Some(path.clone());
            app.command("save", h, r);
            check(path.is_file() && std::fs::metadata(&path).map(|m| m.len() > 1000).unwrap_or(false),
                  "the workbook was written")?;
            app.open_path(&path);
            check(app.book.display(0, 3, 0) == "3090.5", "and opens again with the formula's answer")
        });
        UiTest { steps, i: 0, done: false, fails: vec![], passed: 0, dir }
    }

    pub fn step(&mut self, app: &mut SheetApp, host: &mut dyn Host, r: &mut Res) {
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
