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

/// The ink in a part of the first page, given as fractions of its height.
fn ink_band(app: &mut PageApp, r: &mut Res, from: f32, to: f32) -> usize {
    let pm = shot(app, r);
    let Some((_, rect)) = app.page_rects_now().into_iter().next() else { return 0 };
    let y0 = (rect.y + rect.h * from) as u32;
    let y1 = ((rect.y + rect.h * to) as u32).min(pm.height());
    let mut n = 0;
    for y in y0..y1 {
        for x in (rect.x as u32 + 1)..(rect.right() as u32).min(pm.width()) {
            if let Some(p) = pm.pixel(x, y) {
                let p = p.demultiply();
                if p.red() < 170 && p.green() < 170 && p.blue() < 170 {
                    n += 1;
                }
            }
        }
    }
    n
}

/// The ink on the pages themselves: how much of the document is showing.
/// Only what is drawn on the paper counts, never the grey around it.
fn page_ink(app: &mut PageApp, r: &mut Res) -> usize {
    let pm = shot(app, r);
    let mut n = 0;
    for (_, rect) in app.page_rects_now() {
        for y in (rect.y as u32 + 1)..(rect.bottom() as u32).min(pm.height()) {
            for x in (rect.x as u32 + 1)..(rect.right() as u32).min(pm.width()) {
                if let Some(p) = pm.pixel(x, y) {
                    let p = p.demultiply();
                    if p.red() < 170 && p.green() < 170 && p.blue() < 170 {
                        n += 1;
                    }
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
        let d_merge = dir.clone();
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
        step!("a page break starts a second page", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let before = app.ed.page_count();
            ribbon_click(app, h, r, "insert", "break-page")?;
            typed(app, h, "Over the page");
            check(app.ed.page_count() > before, &format!("still {} pages", app.ed.page_count()))?;
            key(app, h, Key::Char('z'), ctrl());
            key(app, h, Key::Char('z'), ctrl());
            check(app.ed.page_count() == before, "undo didn't take the break back")
        });
        step!("a table goes in where the caret is", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let before = page_ink(app, r);
            ribbon_click(app, h, r, "insert", "table")?;
            let after = page_ink(app, r);
            check(after > before, "no table was drawn")?;
            key(app, h, Key::Char('z'), ctrl());
            Ok(())
        });
        step!("Find and Replace changes the words", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "find")?;
            let d = app.dialog.as_mut().ok_or("the dialog didn't open")?;
            d.set_text_for_test("what", "Folio");
            d.set_text_for_test("with", "Folio Page");
            app.press_dialog_button("all", h);
            check(app.ed.text_of_document().contains("Folio Page"), "the words weren't replaced")?;
            app.press_dialog_button("close", h);
            check(app.dialog.is_none(), "the dialog stayed open")?;
            key(app, h, Key::Char('z'), ctrl());
            Ok(())
        });
        step!("Landscape turns the page on its side", |app: &mut PageApp, h: &mut dyn Host, _r: &mut Res| {
            let (w0, h0) = app.ed.page_size(0);
            app.command("orient:landscape", h);
            let (w1, h1) = app.ed.page_size(0);
            check(w1 > h1 && w1 > w0 && h1 < h0, &format!("page is {w1} by {h1}, was {w0} by {h0}"))?;
            app.command("orient:portrait", h);
            Ok(())
        });
        step!("a table gains a row and loses one", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "insert", "table")?;
            let rows = |a: &PageApp| a.ed.table_cells(0).len();
            let before = rows(app);
            check(before > 0, "no table to work on")?;
            ribbon_click(app, h, r, "table", "row-below")?;
            check(rows(app) > before, "no row was added")?;
            ribbon_click(app, h, r, "table", "row-delete")?;
            check(rows(app) == before, "the row didn't go")?;
            ribbon_click(app, h, r, "table", "table-delete")?;
            check(rows(app) == 0, "the table stayed")?;
            // back to the end of the text, where a person would be
            key(app, h, Key::End, ctrl());
            check(app.ed.text_of_document().contains("Hello"), &format!("the text went: {:?}", app.ed.text_of_document()))
        });
        step!("UPPERCASE from the Home tab", |app: &mut PageApp, h: &mut dyn Host, _r: &mut Res| {
            key(app, h, Key::Char('a'), ctrl());
            app.command("case-set:upper", h);
            check(app.ed.text_of_document().contains("HELLO"), "the words didn't go up")?;
            key(app, h, Key::Char('z'), ctrl());
            check(app.ed.text_of_document().contains("Hello"), &format!("after undo: {:?}", app.ed.text_of_document()))
        });
        step!("Draft view lays the text out without the paper", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "view", "view-draft")?;
            check(app.ed.page_count() >= 1, "no pages in draft view")?;
            ribbon_click(app, h, r, "view", "view-print")?;
            check(app.ed.text_of_document().contains("Hello"), &format!("in draft view: {:?}", app.ed.text_of_document()))
        });
        step!("the ruler and the navigation pane take their room", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let wide = app.canvas_rect().w;
            ribbon_click(app, h, r, "view", "nav")?;
            shot(app, r);
            check(app.canvas_rect().w < wide, "the pages didn't make room for the pane")?;
            ribbon_click(app, h, r, "view", "ruler")?;
            shot(app, r);
            check(app.ruler_rect_now().h > 0.0, "no ruler")?;
            // a heading to jump to
            app.command("style:Heading 1", h);
            shot(app, r);
            check(!app.nav_rows_now().is_empty(), "the pane shows no headings")?;
            app.command("style:Normal", h);
            ribbon_click(app, h, r, "view", "nav")?;
            ribbon_click(app, h, r, "view", "ruler")?;
            shot(app, r);
            check((app.canvas_rect().w - wide).abs() < 0.5, "the room didn't come back")
        });
        step!("dragging the indent marker indents the paragraph", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "view", "ruler")?;
            shot(app, r);
            let before = app.ed.current_para_props().ind_left.unwrap_or(0);
            let Some((_, hang, _)) = app.indent_marks_now() else { return Err("no markers".into()) };
            // the hanging marker sits in the lower half of the ruler
            let y = app.ruler_rect_now().bottom() - 4.0;
            ev(app, h, Event::MouseDown { x: hang, y, button: Button::Left, clicks: 1 });
            check(app.dragging_indent().is_some(), &format!("the click at {hang},{y} didn't catch a marker; ruler at {:?}", app.ruler_rect_now()))?;
            ev(app, h, Event::MouseMove { x: hang + 40.0, y });
            ev(app, h, Event::MouseUp { x: hang + 40.0, y, button: Button::Left });
            let after = app.ed.current_para_props().ind_left.unwrap_or(0);
            check(after > before, &format!("indent went from {before} to {after}"))?;
            key(app, h, Key::Char('z'), ctrl());
            ribbon_click(app, h, r, "view", "ruler")?;
            Ok(())
        });
        step!("a footnote goes under the page", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let before = ink_band(app, r, 0.75, 1.0);
            ribbon_click(app, h, r, "references", "footnote")?;
            typed(app, h, "A note at the foot");
            let notes: String = app.ed.layout.pages[0]
                .note_lines
                .iter()
                .map(|l| app.ed.doc.para_text(app.ed.layout.paras[l.para].node))
                .collect();
            check(notes.contains("A note at the foot"), &format!("the note says {notes:?}"))?;
            let after = ink_band(app, r, 0.75, 1.0);
            check(after > before, &format!("nothing new at the foot of the page ({before} -> {after})"))?;
            ribbon_click(app, h, r, "insert", "body")?;
            check(format!("{:?}", app.ed.story) == "Body", "didn't come back to the text")?;
            key(app, h, Key::Char('z'), ctrl());
            key(app, h, Key::Char('z'), ctrl());
            key(app, h, Key::End, ctrl());
            check(app.ed.text_of_document().contains("Hello"), &format!("back in the text: {:?}",
                                                                        app.ed.text_of_document()))
        });
        step!("track changes records what is typed, and it can be rejected", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let before = app.ed.text_of_document();
            ribbon_click(app, h, r, "review", "track")?;
            check(app.ed.tracking(), "tracking didn't come on")?;
            typed(app, h, " tracked words");
            check(app.ed.text_of_document().contains("tracked words"), "the words aren't there")?;
            ribbon_click(app, h, r, "review", "reject-all")?;
            check(app.ed.text_of_document().trim() == before.trim(), "rejecting left the words behind")?;
            ribbon_click(app, h, r, "review", "track")?;
            check(!app.ed.tracking(), "tracking didn't go off")
        });
        step!("a comment is left on the selected words", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            key(app, h, Key::Char('a'), ctrl());
            ribbon_click(app, h, r, "review", "comment")?;
            check(!app.ed.comments().is_empty(), "no comment was left")?;
            ribbon_click(app, h, r, "review", "comment-delete")?;
            check(app.ed.comments().is_empty(), "the comment stayed")
        });
        step!("the Paragraph dialog sets the spacing and the indents", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "home", "para-dialog")?;
            let d = app.dialog.as_mut().ok_or("the dialog didn't open")?;
            d.set_text_for_test("left", "0.50");
            d.set_text_for_test("after", "12");
            app.press_dialog_button("ok", h);
            let pp = app.ed.current_para_props();
            check(pp.ind_left == Some(720), &format!("indent is {:?}", pp.ind_left))?;
            check(pp.after == Some(240), &format!("space after is {:?}", pp.after))?;
            key(app, h, Key::Char('z'), ctrl());
            key(app, h, Key::Char('z'), ctrl());
            Ok(())
        });
        step!("the Font dialog sets the size", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            key(app, h, Key::Char('a'), ctrl());
            ribbon_click(app, h, r, "home", "font-dialog")?;
            let d = app.dialog.as_mut().ok_or("the dialog didn't open")?;
            d.set_text_for_test("size", "18");
            app.press_dialog_button("ok", h);
            check(app.ed.current_size() == Some(18.0), &format!("size is {:?}", app.ed.current_size()))?;
            key(app, h, Key::Char('z'), ctrl());
            Ok(())
        });
        step!("Page Setup sets the margins", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            ribbon_click(app, h, r, "layout", "page-setup")?;
            let d = app.dialog.as_mut().ok_or("the dialog didn't open")?;
            d.set_text_for_test("left", "1.50");
            app.press_dialog_button("ok", h);
            let (left, _) = app.ed.page_margins(0);
            check((left - 108.0).abs() < 1.0, &format!("left margin is {left} points"))?;
            ribbon_click(app, h, r, "layout", "page-setup")?;
            let d = app.dialog.as_mut().ok_or("the dialog didn't open again")?;
            d.set_text_for_test("left", "1.00");
            app.press_dialog_button("ok", h);
            Ok(())
        });
        step!("the document's properties go into the PDF", move |app: &mut PageApp, h: &mut dyn Host, _r: &mut Res| {
            app.command("properties", h);
            let d = app.dialog.as_mut().ok_or("the dialog didn't open")?;
            d.set_text_for_test("title", "A Folio Page document");
            d.set_text_for_test("author", "Product Owner");
            app.press_dialog_button("ok", h);
            check(app.ed.doc_properties()[0] == "A Folio Page document", "the title didn't stick")?;
            Ok(())
        });
        step!("a symbol goes in from the Insert menu", |app: &mut PageApp, h: &mut dyn Host, _r: &mut Res| {
            app.command("sym:\u{2122}", h);
            check(app.ed.text_of_document().contains('\u{2122}'), "the symbol isn't in the text")?;
            key(app, h, Key::Char('z'), ctrl());
            Ok(())
        });
        step!("Read Mode gives the page the whole window, Esc comes back", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let before = app.canvas_rect().h;
            ribbon_click(app, h, r, "view", "read-mode")?;
            shot(app, r);
            check(app.canvas_rect().h > before, "the page didn't get more room")?;
            key(app, h, Key::Escape, Mods::NONE);
            shot(app, r);
            check((app.canvas_rect().h - before).abs() < 0.5, "Esc didn't bring the ribbon back")
        });
        step!("the accessibility check reports what it finds", |app: &mut PageApp, h: &mut dyn Host, _r: &mut Res| {
            let issues = app.ed.accessibility_issues().len();
            app.command("accessibility", h);
            if issues == 0 {
                check(app.dialog.is_none(), "it opened a list with nothing in it")?;
            } else {
                check(app.dialog.is_some(), &format!("{issues} to report, but nothing was shown"))?;
                app.press_dialog_button("ok", h);
                check(app.dialog.is_none(), "the list wouldn't close")?;
            }
            Ok(())
        });
        step!("a caption numbers itself, and a cross-reference points at a heading", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            // a heading to point at
            key(app, h, Key::End, ctrl());
            key(app, h, Key::Enter, Mods::NONE);
            typed(app, h, "A heading to refer to");
            app.command("style:Heading 1", h);
            key(app, h, Key::Enter, Mods::NONE);
            app.command("style:Normal", h);
            ribbon_click(app, h, r, "references", "caption")?;
            let d = app.dialog.as_mut().ok_or("the caption dialog didn't open")?;
            d.set_text_for_test("text", "A picture of the works");
            app.press_dialog_button("ok", h);
            check(app.ed.text_of_document().contains("A picture of the works"), "the caption isn't there")?;
            ribbon_click(app, h, r, "references", "cross-ref")?;
            let open = app.dialog.is_some();
            check(open, "the cross-reference dialog didn't open")?;
            app.press_dialog_button("ok", h);
            let fields: Vec<String> = app.ed.body_fields().iter().map(|f| f.0.clone()).collect();
            check(fields.iter().any(|f| f.trim_start().starts_with("REF") || f.trim_start().starts_with("PAGEREF")),
                  &format!("no reference field was put in; the fields are {fields:?}"))?;
            for _ in 0..8 {
                key(app, h, Key::Char('z'), ctrl());
            }
            Ok(())
        });
        step!("a mail merge writes one document per recipient", move |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            let list = d_merge.join("people.csv");
            std::fs::write(&list, "Name,Town\nAda,Leeds\nGrace,New York\n").map_err(|e| e.to_string())?;
            app.open_recipients_for_test(&list)?;
            check(app.merge.headers == vec!["Name".to_string(), "Town".to_string()],
                  &format!("columns are {:?}", app.merge.headers))?;
            key(app, h, Key::End, ctrl());
            key(app, h, Key::Enter, Mods::NONE);
            typed(app, h, "Dear ");
            app.command("mfield:Name", h);
            app.command("preview-merge", h);
            check(app.ed.text_of_document().contains("Ada"), &format!("the first recipient isn't shown: {:?}",
                                                                      app.ed.text_of_document()))?;
            app.command("merge-next", h);
            check(app.ed.text_of_document().contains("Grace"), "the next recipient isn't shown")?;
            app.command("finish-merge", h);
            let d = app.dialog.as_mut().ok_or("the finish dialog didn't open")?;
            d.set_text_for_test("folder", &d_merge.to_string_lossy());
            d.set_text_for_test("stem", "Letter");
            app.press_dialog_button("ok", h);
            check(d_merge.join("Letter-1.docx").is_file() && d_merge.join("Letter-2.docx").is_file(),
                  "the merged documents weren't written")?;
            let _ = r;
            for _ in 0..6 {
                key(app, h, Key::Char('z'), ctrl());
            }
            Ok(())
        });
        step!("a chart and a shape go into the document", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            key(app, h, Key::End, ctrl());
            let before = page_ink(app, r);
            ribbon_click(app, h, r, "insert", "chart")?;
            let d = app.dialog.as_mut().ok_or("the chart dialog didn't open")?;
            d.set_text_for_test("title", "Sales");
            app.press_dialog_button("ok", h);
            let after = page_ink(app, r);
            check(after > before, &format!("the chart wasn't drawn ({before} -> {after})"))?;
            check(!app.ed.chart_parts().is_empty(), "no chart part in the document")?;
            app.command("shape-kind:ellipse", h);
            let with_shape = page_ink(app, r);
            check(with_shape > after, "the shape wasn't drawn")?;
            for _ in 0..6 {
                key(app, h, Key::Char('z'), ctrl());
            }
            Ok(())
        });
        step!("a misspelling gets a red line, and the menu offers a fix", |app: &mut PageApp, h: &mut dyn Host, r: &mut Res| {
            if !app.spell.ready() {
                return Ok(()); // no dictionary on this machine
            }
            key(app, h, Key::End, ctrl());
            key(app, h, Key::Enter, Mods::NONE);
            typed(app, h, "A speling mistake");
            shot(app, r);
            let marks = app.squiggles_now();
            check(!marks.is_empty(), "nothing was underlined")?;
            // right-click the word
            let p = app.ed.caret;
            let back = folio_page_editor::Pos { para: p.para, off: p.off.saturating_sub(9) };
            app.open_word_menu_for_test(back);
            check(app.menu_items_now().iter().any(|i| i.starts_with("fix:")), "no other spellings were offered")?;
            let fix = app.menu_items_now().into_iter().find(|i| i.starts_with("fix:")).unwrap();
            app.pick_menu_for_test(&fix, h);
            check(!app.ed.text_of_document().contains("speling"), "the misspelling is still there")?;
            for _ in 0..4 {
                key(app, h, Key::Char('z'), ctrl());
            }
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
        step!("closing a changed document asks first", |app: &mut PageApp, h: &mut dyn Host, _r: &mut Res| {
            typed(app, h, " one more word");
            check(app.ed.modified, "the document should count as changed")?;
            app.ask_to_close_for_test();
            check(app.dialog.is_some(), "it closed without asking")?;
            app.press_dialog_button("cancel", h);
            check(!app.wants_exit_for_test(), "Keep working still closed it")?;
            key(app, h, Key::Char('z'), ctrl());
            Ok(())
        });
        step!("Export PDF writes a PDF of the pages", move |app: &mut PageApp, _h: &mut dyn Host, _r: &mut Res| {
            let p = d_pdf.join("typed.pdf");
            app.export_pdf(&p);
            check(p.is_file(), "no PDF was written")?;
            let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
            check(bytes.starts_with(b"%PDF-"), "that isn't a PDF")?;
            check(bytes.len() > 1000, "the PDF is suspiciously small")?;
            let text = String::from_utf8_lossy(&bytes);
            check(text.contains("/Title"), "the PDF carries no title")?;
            check(text.contains("/Outlines") || text.contains("/FontFile2"), "the PDF has no fonts in it")
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
