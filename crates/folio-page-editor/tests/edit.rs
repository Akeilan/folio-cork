use folio_page_editor::eng::document::Document;
use folio_page_editor::{Editor, Pos, PropEdit};
use folio_page_editor::eng::fonts::FontCache;

const CORPUS: &str = "/home/claude/corpus/poi/test-data/document";

fn open(name: &str) -> Editor {
    let doc = Document::open_path(&format!("{CORPUS}/{name}")).unwrap();
    Editor::new(doc, FontCache::new())
}

fn text(e: &Editor, i: usize) -> String {
    e.doc.para_text(e.layout.paras[i].node)
}

fn reopen(e: &mut Editor) -> Editor {
    let bytes = e.doc.write().unwrap();
    Editor::new(Document::open(&bytes).unwrap(), FontCache::new())
}

#[test]
fn typing_undo_redo() {
    let mut e = open("sample.docx");
    let p0 = e.layout.paras[0].node;
    let before = text(&e, 0);
    e.move_to(Pos { para: p0, off: 0 }, false, false);
    for c in "Hello ".chars() {
        e.type_text(&c.to_string());
    }
    assert_eq!(text(&e, 0), format!("Hello {before}"));
    assert_eq!(e.caret.off, 6);
    e.undo();
    e.undo();
    assert_eq!(text(&e, 0), before, "undo restores");
    e.redo();
    assert!(text(&e, 0).starts_with("Hello"));
    let mut r = reopen(&mut e);
    assert!(text(&r, 0).starts_with("Hello"));
    r.undo();
}

#[test]
fn enter_and_backspace() {
    let mut e = open("sample.docx");
    let n = e.layout.paras.len();
    let p0 = e.layout.paras[0].node;
    let t0 = text(&e, 0);
    let mid = t0.chars().count() / 2;
    e.move_to(Pos { para: p0, off: mid }, false, false);
    e.new_paragraph();
    assert_eq!(e.layout.paras.len(), n + 1);
    let a = text(&e, 0);
    let b = text(&e, 1);
    assert_eq!(format!("{a}{b}"), t0);
    assert_eq!(e.caret.off, 0);
    e.delete(false, false);
    assert_eq!(e.layout.paras.len(), n);
    assert_eq!(text(&e, 0), t0);
    e.undo();
    assert_eq!(e.layout.paras.len(), n + 1);
    e.undo();
    assert_eq!(e.layout.paras.len(), n);
    assert_eq!(text(&e, 0), t0);
}

#[test]
fn bold_selection_roundtrip() {
    let mut e = open("sample.docx");
    let p0 = e.layout.paras[0].node;
    e.move_to(Pos { para: p0, off: 1 }, false, false);
    e.move_to(Pos { para: p0, off: 4 }, true, false);
    e.toggle("w:b", "Bold");
    assert!(e.toggle_state("w:b"));
    let mut r = reopen(&mut e);
    let p = r.layout.paras[0].node;
    r.move_to(Pos { para: p, off: 1 }, false, false);
    r.move_to(Pos { para: p, off: 4 }, true, false);
    assert!(r.toggle_state("w:b"));
    r.move_to(Pos { para: p, off: 4 }, false, false);
    r.move_to(Pos { para: p, off: 6 }, true, false);
    assert_eq!(text(&r, 0), text(&e, 0));
    e.format_runs(&[PropEdit::set("w:sz", &[("w:val", "40")])], "Size");
    e.undo();
    e.undo();
    assert!(!{
        e.move_to(Pos { para: p0, off: 1 }, false, false);
        e.move_to(Pos { para: p0, off: 4 }, true, false);
        e.toggle_state("w:b")
    });
}

#[test]
fn delete_across_paragraphs() {
    let mut e = open("sample.docx");
    if e.layout.paras.len() < 3 {
        return;
    }
    let n = e.layout.paras.len();
    let (p0, p2) = (e.layout.paras[0].node, e.layout.paras[2].node);
    let t0 = text(&e, 0);
    let t2 = text(&e, 2);
    e.move_to(Pos { para: p0, off: 2 }, false, false);
    e.move_to(Pos { para: p2, off: 3.min(t2.chars().count()) }, true, false);
    e.delete(false, false);
    assert_eq!(e.layout.paras.len(), n - 2);
    let expect: String = t0.chars().take(2).chain(t2.chars().skip(3)).collect();
    assert_eq!(text(&e, 0), expect);
    e.undo();
    assert_eq!(e.layout.paras.len(), n);
    assert_eq!(text(&e, 2), t2);
}

#[test]
fn caret_geometry_and_hit() {
    let e = open("sample.docx");
    let p0 = e.layout.paras[0].node;
    for off in 0..text(&e, 0).chars().count().min(20) {
        let (pg, x, y, h) = e.caret_rect(Pos { para: p0, off }).unwrap();
        assert!(h > 0.0);
        let hit = e.hit(pg, x + 0.1, y + h / 2.0).unwrap();
        assert_eq!(hit.para, p0);
        assert_eq!(hit.off, off, "hit at caret x gives the same position");
    }
}

/// The blank document the app starts from.
fn blank() -> Editor {
    let doc = Document::open(include_bytes!("blank.docx")).unwrap();
    Editor::new(doc, FontCache::new())
}

fn labels(e: &Editor) -> Vec<String> {
    e.layout.paras.iter().map(|p| p.label.as_ref().map(|l| l.text.clone()).unwrap_or_default()).collect()
}

#[test]
fn lists_bullets_numbering() {
    let mut e = blank();
    e.type_text("one\ntwo\nthree");
    e.select_all();
    e.toggle_list("number");
    assert_eq!(labels(&e), vec!["1.", "2.", "3."]);
    // level change on the second item
    let p1 = e.layout.paras[1].node;
    e.move_to(Pos { para: p1, off: 0 }, false, false);
    assert!(e.at_list_item_start());
    assert!(e.change_list_level(1));
    assert_eq!(labels(&e), vec!["1.", "a.", "2."]);
    // Enter at the end of the last item, then Enter again ends the list
    let p2 = e.layout.paras[2].node;
    e.move_to(Pos { para: p2, off: 5 }, false, false);
    e.type_text("\n");
    assert_eq!(labels(&e), vec!["1.", "a.", "2.", "3."]);
    e.type_text("\n");
    assert_eq!(labels(&e), vec!["1.", "a.", "2.", ""]);
    // saved and opened again
    let r = reopen(&mut e);
    assert_eq!(labels(&r), vec!["1.", "a.", "2.", ""]);
    // undo back to plain paragraphs
    for _ in 0..4 {
        e.undo();
    }
    assert_eq!(labels(&e), vec!["", "", ""]);
    e.redo();
    assert_eq!(labels(&e), vec!["1.", "2.", "3."]);
    // bullets replace numbering? toggling bullets on numbered items makes bullets
    e.select_all();
    e.toggle_list("bullet");
    assert!(labels(&e).iter().all(|l| l == "\u{2022}"), "{:?}", labels(&e));
    e.select_all();
    e.toggle_list("bullet");
    assert_eq!(labels(&e), vec!["", "", ""]);
}

fn table_shape(e: &Editor) -> Vec<usize> {
    let dom = &e.doc.dom;
    let tbl = dom.elements(e.doc.body).find(|&n| dom.is(n, "w:tbl"));
    match tbl {
        Some(t) => dom.children_named(t, "w:tr").map(|r| dom.children_named(r, "w:tc").count()).collect(),
        None => vec![],
    }
}

#[test]
fn tables_insert_edit_merge() {
    let mut e = blank();
    e.type_text("Before");
    e.type_text("\n");
    e.insert_table(2, 3);
    assert_eq!(table_shape(&e), vec![3, 3]);
    assert!(e.cell_of(e.caret.para).is_some(), "caret in the first cell");
    e.type_text("A1");
    assert!(e.cell_step(false));
    e.type_text("B1");
    // Tab from the last cell adds a row
    for _ in 0..5 {
        e.cell_step(false);
    }
    assert_eq!(table_shape(&e), vec![3, 3, 3]);
    e.insert_rows(false);
    assert_eq!(table_shape(&e), vec![3, 3, 3, 3]);
    e.delete_rows();
    assert_eq!(table_shape(&e), vec![3, 3, 3]);
    e.insert_cols(true);
    assert_eq!(table_shape(&e), vec![4, 4, 4]);
    e.delete_cols();
    assert_eq!(table_shape(&e), vec![3, 3, 3]);
    // merge the first two cells of row 1
    let dom = &e.doc.dom;
    let tbl = dom.elements(e.doc.body).find(|&n| dom.is(n, "w:tbl")).unwrap();
    let r0 = dom.children_named(tbl, "w:tr").next().unwrap();
    let cells: Vec<_> = dom.children_named(r0, "w:tc").collect();
    let p0 = dom.child(cells[0], "w:p").unwrap();
    let p1 = dom.child(cells[1], "w:p").unwrap();
    e.move_to(Pos { para: p0, off: 0 }, false, false);
    e.move_to(Pos { para: p1, off: 2 }, true, false);
    assert!(e.merge_cells());
    assert_eq!(table_shape(&e), vec![2, 3, 3]);
    let r = reopen(&mut e);
    assert_eq!(table_shape(&r), vec![2, 3, 3]);
    assert!(r.layout.pages[0].cells.len() >= 8);
    // all of it undoes
    for _ in 0..20 {
        e.undo();
    }
    assert_eq!(table_shape(&e), Vec::<usize>::new());
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, 128, 255]);
        }
    }
    let pm = folio_kit::gfx::pixmap_from_rgba(w, h, &rgba).unwrap();
    folio_kit::gfx::encode_png(&pm)
}

#[test]
fn pictures_insert_wrap_move_delete() {
    let mut e = blank();
    e.type_text("Some text before the picture and more text after it, enough to wrap around the picture when it floats.");
    let p0 = e.layout.paras[0].node;
    e.move_to(Pos { para: p0, off: 5 }, false, false);
    assert!(e.insert_picture(&png(200, 100), "pic.png"));
    let info = e.selected_object().expect("selected after insert");
    assert!(info.inline);
    assert_eq!(info.kind, "picture");
    assert!((info.w - 150.0).abs() < 0.5 && (info.h - 75.0).abs() < 0.5, "96 dpi: {}x{}", info.w, info.h);
    assert_eq!(e.doc.para_text(p0).chars().nth(5), Some('\u{fffc}'));
    // found where it is drawn
    let hit = e.object_at(info.page, info.x + 10.0, info.y + 10.0);
    assert_eq!(hit, Some(info.node));
    // resize
    assert!(e.object_resize(300.0, 150.0));
    let i2 = e.selected_object().unwrap();
    assert!((i2.w - 300.0).abs() < 0.5);
    // square wrap: floats, the text flows around it
    let lines_inline = e.layout.paras[0].lines.len();
    assert!(e.object_set_wrap("square"));
    let i3 = e.selected_object().unwrap();
    assert!(!i3.inline);
    assert_eq!(i3.wrap, "square");
    assert!((i3.x - i2.x).abs() < 1.0 && (i3.y - i2.y).abs() < 1.0, "stays put: {:?} {:?}", (i2.x, i2.y), (i3.x, i3.y));
    assert!(e.layout.pages[0].floats.len() == 1);
    let _ = lines_inline;
    // move it
    assert!(e.object_move(i3.x + 50.0, i3.y + 20.0));
    let i4 = e.selected_object().unwrap();
    assert!((i4.x - i3.x - 50.0).abs() < 0.5 && (i4.y - i3.y - 20.0).abs() < 0.5, "{:?}", (i4.x, i4.y));
    // behind the text, then back in line
    assert!(e.object_set_wrap("behind"));
    assert_eq!(e.selected_object().unwrap().wrap, "behind");
    assert!(e.object_set_wrap("inline"));
    assert!(e.selected_object().unwrap().inline);
    // survives a save
    let mut r = reopen(&mut e);
    assert_eq!(r.doc.images.len(), 1);
    let n = r.layout.pages[0].lines.iter().any(|l| {
        let p = &r.layout.paras[l.para];
        p.units.iter().any(|u| u.kind == folio_page_editor::eng::layout::UnitKind::Object)
    });
    assert!(n);
    let _ = &mut r;
    // delete, undo
    assert!(e.object_delete());
    assert!(!e.doc.para_text(p0).contains('\u{fffc}'));
    e.undo();
    assert!(e.doc.para_text(p0).contains('\u{fffc}'));
}

fn hf_text(e: &Editor, pg: usize) -> String {
    let page = &e.layout.pages[pg];
    let mut s = String::new();
    for l in &page.hf_lines {
        let p = &e.layout.paras[l.para];
        let line = &p.lines[l.line];
        for u in &p.units[line.start..line.end] {
            if let folio_page_editor::eng::layout::UnitKind::Char(c) = u.kind {
                s.push(c);
            }
        }
        s.push('|');
    }
    s
}

#[test]
fn headers_footers_page_numbers() {
    let mut e = open("Bug51170.docx");
    assert!(e.layout.pages.len() >= 3);
    // a title page: no header on the first page
    assert!(hf_text(&e, 0).is_empty());
    let a = hf_text(&e, 1);
    let b = hf_text(&e, 2);
    assert!(!a.is_empty(), "page 2 has a header or footer");
    assert_ne!(a, b, "page numbers differ: {a} / {b}");
    // edit the first page's header
    e.edit_header_footer(Some(1), false);
    let before = hf_text(&e, 2);
    e.type_text("Draft ");
    assert!(hf_text(&e, 2).contains("Draft"), "every page shows it: {}", hf_text(&e, 2));
    let mut r = reopen(&mut e);
    assert!(hf_text(&r, 1).contains("Draft"), "saved in the header part");
    let _ = &mut r;
    e.undo();
    assert_eq!(hf_text(&e, 2), before);
    e.edit_header_footer(None, false);
    assert_eq!(e.story, folio_page_editor::Story::Body);
}

#[test]
fn page_numbers_in_new_footer() {
    let mut e = blank();
    assert!(e.layout.pages[0].footer.is_none());
    assert!(e.page_number_in(0, true, "center", true, false));
    assert!(hf_text(&e, 0).contains("Page 1 of 1"), "{}", hf_text(&e, 0));
    for _ in 0..60 {
        e.type_text("Another paragraph of text.\n");
    }
    assert!(e.layout.pages.len() >= 2);
    assert!(hf_text(&e, 1).contains(&format!("Page 2 of {}", e.layout.pages.len())), "{}", hf_text(&e, 1));
    // a field typed in the body
    e.insert_field("PAGE", "1");
    let r = reopen(&mut e);
    assert!(hf_text(&r, 1).contains("Page 2 of"));
    let ftr = r.doc.parts.iter().find(|p| p.kind == "footer").unwrap();
    assert!(r.doc.dom.serialize_node(ftr.root).contains("NUMPAGES"));
}

#[test]
fn footnotes_insert_and_layout() {
    let mut e = blank();
    e.type_text("A sentence that needs a source.");
    e.move_to(e.doc_end(), false, false);
    assert!(e.insert_note(false));
    assert_eq!(e.story, folio_page_editor::Story::Notes(0));
    e.type_text("The source.");
    let pg = &e.layout.pages[0];
    assert_eq!(pg.notes.len(), 1);
    assert_eq!(pg.rules.len(), 1, "separator");
    let note_text: String = pg.note_lines.iter().map(|l| e.doc.para_text(e.layout.paras[l.para].node)).collect();
    assert!(note_text.contains("The source."), "{note_text}");
    // the mark in the body
    e.edit_notes(None);
    let body = e.doc.para_text(e.layout.paras[0].node);
    assert!(body.ends_with('1'), "{body}");
    // a second one comes numbered 2, and both survive saving
    e.move_to(Pos { para: e.layout.paras[0].node, off: 1 }, false, false);
    e.insert_note(false);
    e.type_text("Earlier note.");
    e.edit_notes(None);
    let r = reopen(&mut e);
    assert_eq!(r.layout.pages[0].notes.len(), 2);
    let body = r.doc.para_text(r.layout.paras[0].node);
    assert!(body.starts_with("A1"), "renumbered in order: {body}");
}

#[test]
fn field_instructions_and_formats() {
    use folio_page_editor::{format_date, parse_instr};
    let i = parse_instr(r#" DATE \@ "dddd, MMMM d, yyyy" \* MERGEFORMAT "#);
    assert_eq!(i.kind, "DATE");
    assert_eq!(i.switches[0], ('@', Some("dddd, MMMM d, yyyy".into())));
    let d = chrono::NaiveDate::from_ymd_opt(2026, 10, 9).unwrap().and_hms_opt(15, 4, 5).unwrap();
    assert_eq!(format_date(&d, "dddd, MMMM d, yyyy"), "Friday, October 9, 2026");
    assert_eq!(format_date(&d, "M/d/yy h:mm am/pm"), "10/9/26 3:04 pm");
    assert_eq!(format_date(&d, "dd.MM.yyyy HH:mm"), "09.10.2026 15:04");
}

#[test]
fn fields_update_and_toc() {
    let mut e = blank();
    let style = |e: &mut Editor, id: &str| e.format_paras(&[PropEdit::set("w:pStyle", &[("w:val", id)])], "Style");
    e.type_text("Introduction");
    style(&mut e, "Heading1");
    e.type_text("\nSome text.\nDetails");
    style(&mut e, "Heading2");
    e.type_text("\nMore text.");
    for _ in 0..50 {
        e.type_text("\nFiller paragraph to make pages.");
    }
    e.type_text("\nConclusion");
    style(&mut e, "Heading1");
    // a table of contents at the top
    let first = e.layout.paras[0].node;
    e.move_to(Pos { para: first, off: 0 }, false, false);
    e.insert_toc(Some("Contents"));
    let texts: Vec<String> = (0..6).map(|i| text(&e, i)).collect();
    assert_eq!(texts[0], "Contents");
    assert!(texts[1].starts_with("Introduction\t1"), "{texts:?}");
    assert!(texts[2].starts_with("Details\t1"), "{texts:?}");
    let last_page = e.layout.pages.len().to_string();
    assert!(texts[3].starts_with("Conclusion\t") && texts[3].ends_with(&last_page), "{texts:?} {last_page}");
    // a SEQ and a PAGE field
    e.move_to(e.doc_end(), false, false);
    e.type_text("\nFigure ");
    e.insert_field_now("SEQ Figure \\* ARABIC");
    e.type_text(" and Figure ");
    e.insert_field_now("SEQ Figure \\* ROMAN");
    let p = e.caret.para;
    assert!(e.doc.para_text(p).contains("Figure 1 and Figure II"), "{}", e.doc.para_text(p));
    // headings change: Update Table follows
    e.move_to(Pos { para: e.layout.paras[1].node, off: 0 }, false, false);
    let n = e.update_fields(true);
    assert!(n >= 1);
    let r = reopen(&mut e);
    assert!(text(&r, 1).starts_with("Introduction"));
    assert!(e.remove_toc());
    assert_eq!(text(&e, 0), "Introduction");
}

#[test]
fn field_codes_toggle() {
    let mut e = blank();
    e.type_text("Page ");
    e.insert_field_now("PAGE \\* MERGEFORMAT");
    let p = e.caret.para;
    assert_eq!(e.doc.para_text(p), "Page 1");
    e.toggle_field_codes();
    let shown: String = e.layout.paras[0].units.iter().filter_map(|u| match u.kind {
        folio_page_editor::eng::layout::UnitKind::Char(c) => Some(c),
        _ => None,
    }).collect();
    assert!(shown.contains("{ PAGE"), "{shown}");
    e.toggle_field_codes();
    assert_eq!(e.doc.para_text(p), "Page 1");
}

#[test]
fn toc_links_go_to_headings() {
    let mut e = blank();
    e.type_text("Alpha");
    e.format_paras(&[PropEdit::set("w:pStyle", &[("w:val", "Heading1")])], "Style");
    for _ in 0..60 {
        e.type_text("\nText.");
    }
    e.type_text("\nBeta");
    e.format_paras(&[PropEdit::set("w:pStyle", &[("w:val", "Heading1")])], "Style");
    let first = e.layout.paras[0].node;
    e.move_to(Pos { para: first, off: 0 }, false, false);
    e.insert_toc(None);
    // the second entry's text, where it is drawn
    let pg = &e.layout.pages[0];
    let l = &pg.lines[1];
    let link = e.link_at(0, l.x + 5.0, l.y + 3.0).expect("a link");
    assert!(link.starts_with("#_Toc"), "{link}");
    assert!(e.goto_bookmark(&link[1..]));
    assert_eq!(e.doc.para_text(e.caret.para), "Beta");
}

#[test]
fn links_bookmarks_cross_references() {
    let mut e = blank();
    e.type_text("Methods");
    e.format_paras(&[PropEdit::set("w:pStyle", &[("w:val", "Heading1")])], "Style");
    e.type_text("\nSee ");
    e.insert_hyperlink("https://example.org/a?b=1&c=2", Some("the site"));
    e.type_text(".");
    let p = e.caret.para;
    assert_eq!(e.doc.para_text(p), "See the site.");
    e.move_to(Pos { para: p, off: 6 }, false, false);
    assert_eq!(e.link_at_caret(), "https://example.org/a?b=1&c=2");
    // a figure caption and references to it and to the heading
    e.move_to(e.doc_end(), false, false);
    e.type_text("\n");
    e.insert_caption("Figure", "A chart", true, "ARABIC", false);
    e.move_to(e.doc_end(), false, false);
    e.type_text("\nAs in ");
    let refs = e.references();
    let fig = refs.iter().find(|r| r.0 == "Figure").unwrap().2.clone();
    assert!(e.insert_cross_reference("Figure", &fig, "label-number", true));
    e.type_text(" under ");
    assert!(e.insert_cross_reference("Heading", "h0", "text", true));
    e.type_text(" on page ");
    assert!(e.insert_cross_reference("Heading", "h0", "page", false));
    let t = e.doc.para_text(e.caret.para);
    assert_eq!(t, "As in Figure 1 under Methods on page 1", "{t}");
    // bookmarks
    e.select_all();
    let first = e.layout.paras[0].node;
    e.move_to(Pos { para: first, off: 0 }, false, false);
    e.move_to(Pos { para: first, off: 4 }, true, false);
    assert!(e.add_bookmark("Start"));
    assert_eq!(e.bookmark_names(), vec!["Start".to_string()]);
    e.move_to(e.doc_end(), false, false);
    assert!(e.goto_bookmark("Start"));
    assert_eq!(e.caret, Pos { para: first, off: 0 });
    assert!(e.remove_link() == false);
    let r = reopen(&mut e);
    assert_eq!(r.bookmark_names(), vec!["Start".to_string()]);
}

fn xml_of(e: &Editor, i: usize) -> String {
    e.doc.dom.serialize_node(e.layout.paras[i].node)
}

#[test]
fn track_changes_record_accept_reject() {
    use folio_page_editor::eng::document::Markup;
    let mut e = blank();
    e.type_text("The quick fox");
    e.set_track_changes(true);
    e.author = "Ann".into();
    let p = e.layout.paras[0].node;
    // insert "brown " before "fox"
    e.move_to(Pos { para: p, off: 10 }, false, false);
    for c in "brown ".chars() {
        e.type_text(&c.to_string());
    }
    assert_eq!(text(&e, 0), "The quick brown fox");
    let x = xml_of(&e, 0);
    assert_eq!(x.matches("<w:ins ").count(), 1, "one insertion: {x}");
    assert!(x.contains("w:author=\"Ann\""));
    // delete "quick " (Simple markup: it disappears from the text)
    e.move_to(Pos { para: p, off: 4 }, false, false);
    e.move_to(Pos { para: p, off: 10 }, true, false);
    e.delete(false, false);
    assert_eq!(text(&e, 0), "The brown fox");
    assert!(xml_of(&e, 0).contains(">quick </w:delText>"));
    // deleting own insertion removes it for real
    e.move_to(Pos { para: p, off: 4 }, false, false);
    e.move_to(Pos { para: p, off: 6 }, true, false);
    e.delete(true, false);
    assert_eq!(text(&e, 0), "The own fox");
    assert!(!xml_of(&e, 0).contains("<w:delText>br"));
    // All Markup shows the deletion
    e.set_markup(Markup::All);
    assert_eq!(text(&e, 0), "The quick own fox");
    e.set_markup(Markup::Original);
    assert_eq!(text(&e, 0), "The quick fox");
    e.set_markup(Markup::Simple);
    // bold with tracking keeps the old formatting
    e.move_to(Pos { para: p, off: 0 }, false, false);
    e.move_to(Pos { para: p, off: 3 }, true, false);
    e.format_runs(&[PropEdit::set("w:b", &[])], "Bold");
    assert!(xml_of(&e, 0).contains("<w:rPrChange "));
    // a new paragraph mark is tracked
    e.move_to(Pos { para: p, off: 11 }, false, false);
    e.type_text("\nNext");
    let n = e.layout.paras.len();
    assert!(xml_of(&e, 0).contains("<w:rPr><w:ins "), "{}", xml_of(&e, 0));
    let all = e.all_changes();
    assert!(all.len() >= 4, "{all:?}");
    // navigation
    e.move_to(Pos { para: p, off: 0 }, false, false);
    assert!(e.goto_change(true));
    assert!(e.has_selection());
    // reject everything: the original
    let mut r = reopen(&mut e);
    assert!(r.track, "trackRevisions saved");
    r.accept_reject_all(false, None);
    assert_eq!(text(&r, 0), "The quick fox");
    assert!(!xml_of(&r, 0).contains("w:b/"));
    assert_eq!(r.layout.paras.len(), n - 1);
    // accept everything: the final text
    e.accept_reject_all(true, None);
    assert_eq!(text(&e, 0), "The own fox");
    assert_eq!(text(&e, 1), "Next");
    let x = xml_of(&e, 0);
    assert!(!x.contains("w:ins") && !x.contains("w:del") && !x.contains("Change"), "{x}");
    e.undo();
    assert_eq!(e.all_changes().len(), all.len());
    // accept the change at the caret only (Enter put the head in a new
    // paragraph)
    let p = e.layout.paras[0].node;
    e.move_to(Pos { para: p, off: 4 }, false, false);
    let k = e.accept_reject(false);
    assert!(k >= 1);
    assert_eq!(text(&e, 0), "The quick own fox");
}

#[test]
fn track_changes_join_paragraphs() {
    let mut e = blank();
    e.type_text("One\nTwo");
    e.set_track_changes(true);
    let p1 = e.layout.paras[1].node;
    e.move_to(Pos { para: p1, off: 0 }, false, false);
    e.delete(false, false);
    // the mark is marked deleted; the paragraphs stay apart in the XML
    assert_eq!(e.layout.paras.len(), 2);
    assert!(xml_of(&e, 0).contains("<w:del "));
    // the final text shows them joined on one line
    let lines = &e.layout.pages[0].lines;
    assert_eq!(lines.len(), 2);
    assert!((lines[0].y - lines[1].y).abs() < 0.5, "{} {}", lines[0].y, lines[1].y);
    assert!(lines[1].x + e.layout.paras[1].lines[0].xs[0] > lines[0].x + 10.0);
    e.accept_reject_all(true, None);
    assert_eq!(e.layout.paras.len(), 1);
    assert_eq!(text(&e, 0), "OneTwo");
}

#[test]
fn comments_insert_reply_resolve_delete() {
    let mut e = blank();
    e.type_text("Alpha beta gamma");
    let p = e.layout.paras[0].node;
    e.move_to(Pos { para: p, off: 6 }, false, false);
    e.move_to(Pos { para: p, off: 10 }, true, false);
    let id = e.insert_comment("Check this\nsecond line", "Bob Smith").unwrap();
    assert_eq!(text(&e, 0), "Alpha beta gamma", "markers take no room");
    let cs = e.comments();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].text, "Check this\nsecond line");
    assert_eq!(cs[0].initials, "BS");
    let (s, en) = cs[0].range.unwrap();
    assert_eq!((s.off, en.off), (6, 10));
    assert!(!e.body_range_rects(s, en).is_empty());
    // caret in the word: the comment there
    e.move_to(Pos { para: p, off: 8 }, false, false);
    assert_eq!(e.comment_at_caret(), Some(id.clone()));
    // a comment with no selection takes the word
    e.move_to(Pos { para: p, off: 13 }, false, false);
    let id2 = e.insert_comment("On gamma", "").unwrap();
    let r2 = e.comments().into_iter().find(|c| c.id == id2).unwrap().range.unwrap();
    assert_eq!((r2.0.off, r2.1.off), (11, 16));
    let rep = e.reply_comment(&id, "Done", "Ann").unwrap();
    assert!(e.resolve_comment(&id, None));
    let mut r = reopen(&mut e);
    let cs = r.comments();
    assert_eq!(cs.len(), 3);
    let c0 = cs.iter().find(|c| c.id == id).unwrap();
    assert!(c0.done);
    let reply = cs.iter().find(|c| c.id == rep).unwrap();
    assert_eq!(reply.parent.as_deref(), Some(id.as_str()));
    assert_eq!(reply.range.map(|x| (x.0.off, x.1.off)), Some((6, 10)));
    assert!(r.edit_comment(&id2, "Changed"));
    assert_eq!(r.comments().iter().find(|c| c.id == id2).unwrap().text, "Changed");
    // navigation
    r.move_to(Pos { para: r.layout.paras[0].node, off: 0 }, false, false);
    assert!(r.goto_comment(true));
    assert_eq!(r.selection().0.off, 6);
    assert!(r.goto_comment(true));
    assert_eq!(r.selection().0.off, 11);
    assert!(!r.goto_comment(true));
    // deleting a comment takes its replies
    assert_eq!(r.delete_comments(&[id.clone()]), 2);
    assert_eq!(r.comments().len(), 1);
    let x = xml_of(&r, 0);
    assert!(!x.contains(&format!("w:id=\"{id}\"")), "{x}");
    r.undo();
    assert_eq!(r.comments().len(), 3);
    assert_eq!(r.delete_all_comments(None), 3);
    assert!(!xml_of(&r, 0).contains("comment"));
}

#[test]
fn equations_insert_edit_delete() {
    let mut e = blank();
    e.type_text("See ");
    let xml = r#"<m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSup></m:oMath>"#;
    let id = e.insert_math(xml, false).unwrap();
    assert_eq!(e.caret.off, 5);
    let inf = e.math_info(id).unwrap();
    assert!(inf.w > 5.0 && inf.asc > 5.0 && !inf.display, "{inf:?}");
    assert_eq!(e.math_at(inf.page, inf.x + 1.0, inf.base - 1.0), Some(id));
    let frac = r#"<m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f></m:oMath>"#;
    assert!(e.set_math(id, frac));
    assert!(e.math_xml(id).contains("<m:f>"));
    let taller = e.math_info(id).unwrap();
    assert!(taller.asc + taller.desc > inf.asc + inf.desc);
    e.type_text("\n");
    let d = e.insert_math(frac, true).unwrap();
    assert!(e.math_info(d).unwrap().display);
    let mut r = reopen(&mut e);
    assert_eq!(r.all_math().len(), 2);
    let ids = r.all_math();
    assert!(r.delete_math(ids[1]));
    assert_eq!(r.all_math().len(), 1);
    r.undo();
    assert_eq!(r.all_math().len(), 2);
}

#[test]
fn track_changes_table_rows() {
    let mut e = blank();
    e.type_text("Before\n");
    e.insert_table(3, 2);
    e.type_text("A1");
    let visible = |e: &Editor| e.layout.paras.iter().filter(|p| e.doc.dom.parent(p.node).map_or(false, |c| e.doc.dom.is(c, "w:tc"))).count();
    assert_eq!(visible(&e), 6);
    e.set_track_changes(true);
    // delete the first row: marked, hidden in the final text
    assert!(e.delete_rows());
    assert_eq!(table_shape(&e), vec![2, 2, 2], "still in the XML");
    assert_eq!(visible(&e), 4);
    assert!(e.all_changes().iter().any(|c| c.kind == folio_page_editor::ChangeKind::RowDelete));
    // a row inserted while tracking, deleted again: gone for real
    let p = e.layout.paras.iter().find(|p| e.doc.dom.parent(p.node).map_or(false, |c| e.doc.dom.is(c, "w:tc"))).unwrap().node;
    e.move_to(Pos { para: p, off: 0 }, false, false);
    e.insert_rows(true);
    assert_eq!(table_shape(&e).len(), 4);
    e.delete_rows();
    assert_eq!(table_shape(&e).len(), 3);
    let mut r = reopen(&mut e);
    r.accept_reject_all(false, None);
    assert_eq!(table_shape(&r), vec![2, 2, 2]);
    assert_eq!(visible(&r), 6);
    e.accept_reject_all(true, None);
    assert_eq!(table_shape(&e), vec![2, 2]);
    // the whole table deleted with tracking: nothing of it shows
    let p = e.layout.paras.iter().find(|p| e.doc.dom.parent(p.node).map_or(false, |c| e.doc.dom.is(c, "w:tc"))).unwrap().node;
    e.move_to(Pos { para: p, off: 0 }, false, false);
    assert!(e.delete_table());
    assert_eq!(visible(&e), 0);
    e.accept_reject_all(true, None);
    assert!(table_shape(&e).is_empty());
}

#[test]
fn comments_in_header() {
    let mut e = blank();
    e.type_text("Body text");
    e.ensure_header_footer(0, false);
    e.edit_header_footer(Some(0), false);
    e.type_text("Header words");
    let p = e.caret.para;
    e.move_to(Pos { para: p, off: 0 }, false, false);
    e.move_to(Pos { para: p, off: 6 }, true, false);
    let id = e.insert_comment("In the header", "Ann").unwrap();
    e.edit_header_footer(None, false);
    let c = e.comments().into_iter().find(|c| c.id == id).unwrap();
    let (a, b) = c.range.unwrap();
    assert_eq!((a.off, b.off), (0, 6));
    let rects = e.body_range_rects(a, b);
    assert!(!rects.is_empty() && rects[0].0 == 0, "{rects:?}");
    let r = reopen(&mut e);
    assert!(r.comments().iter().any(|c| c.range.is_some() && c.text == "In the header"));
}

#[test]
fn long_inline_equation_breaks() {
    let mut e = blank();
    e.type_text("Sum: ");
    let mut runs = String::new();
    for i in 0..40 {
        runs.push_str(&format!("<m:r><m:t>a{i}+</m:t></m:r>"));
    }
    runs.push_str("<m:r><m:t>z</m:t></m:r>");
    let xml = format!(r#"<m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">{runs}</m:oMath>"#);
    e.insert_math(&xml, false).unwrap();
    let pl = &e.layout.paras[0];
    assert!(pl.lines.len() >= 2, "{} lines", pl.lines.len());
    // no line runs past the margin
    for (li, l) in pl.lines.iter().enumerate() {
        let end = l.xs.last().unwrap() + l.ws.last().unwrap();
        assert!(end <= pl.width - pl.right + 1.0, "line {li} ends at {end}");
    }
}

#[test]
fn copy_paste_keeps_formatting() {
    let mut e = blank();
    e.type_text("Plain bold end\nSecond line\nThird");
    let p0 = e.layout.paras[0].node;
    e.move_to(Pos { para: p0, off: 6 }, false, false);
    e.move_to(Pos { para: p0, off: 10 }, true, false);
    e.format_runs(&[PropEdit::set("w:b", &[])], "Bold");
    // copy "bold" (inside one paragraph) and paste at the end of line 3
    let frag = e.copy_fragment().unwrap();
    assert!(frag.contains("<w:b/>") && frag.contains("bold"), "{frag}");
    let p2 = e.layout.paras[2].node;
    e.move_to(Pos { para: p2, off: 5 }, false, false);
    assert!(e.paste_fragment(&frag, true));
    assert_eq!(text(&e, 2), "Thirdbold");
    assert_eq!(e.caret.off, 9);
    assert!(xml_of(&e, 2).contains("<w:b/>"));
    // across paragraphs: from "end" to "Sec"
    let p0 = e.layout.paras[0].node;
    let p1 = e.layout.paras[1].node;
    e.move_to(Pos { para: p0, off: 11 }, false, false);
    e.move_to(Pos { para: p1, off: 3 }, true, false);
    let frag = e.copy_fragment().unwrap();
    let p2 = e.layout.paras[2].node;
    e.move_to(Pos { para: p2, off: 0 }, false, false);
    let n = e.layout.paras.len();
    assert!(e.paste_fragment(&frag, true));
    assert_eq!(e.layout.paras.len(), n + 1);
    assert_eq!(text(&e, 2), "end");
    assert_eq!(text(&e, 3), "SecThirdbold");
    e.undo();
    assert_eq!(e.layout.paras.len(), n);
    // an equation copies too
    let xml = r#"<m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:r><m:t>x</m:t></m:r></m:oMath>"#;
    let p0 = e.layout.paras[0].node;
    e.move_to(Pos { para: p0, off: 0 }, false, false);
    e.insert_math(xml, false);
    e.move_to(Pos { para: p0, off: 0 }, false, false);
    e.move_to(Pos { para: p0, off: 1 }, true, false);
    let frag = e.copy_fragment().unwrap();
    assert!(frag.contains("m:oMath"));
    e.move_to(Pos { para: p0, off: 3 }, false, false);
    e.paste_fragment(&frag, false);
    assert_eq!(e.all_math().len(), 2);
    // pasting with Track Changes on is an insertion
    e.set_track_changes(true);
    e.paste_fragment(&frag, true);
    assert!(e.all_changes().iter().any(|c| c.kind == folio_page_editor::ChangeKind::Insert));
}
