//! Real documents drawn: ink where the engine said there would be text, and
//! the same pixels every time.

use folio_page_engine::document::Document;
use folio_page_engine::fonts::FontCache;
use folio_page_engine::paginate::layout_document;
use folio_page_render::{render_page_image, Overlay, Renderer};

const CORPUS: &str = "/home/claude/corpus/poi/test-data/document";

fn docs(n: usize) -> Vec<std::path::PathBuf> {
    let dir = std::env::var("FOLIO_CORPUS").unwrap_or_else(|_| CORPUS.into());
    let Ok(rd) = std::fs::read_dir(dir.split(':').next().unwrap_or(CORPUS)) else { return Vec::new() };
    let mut v: Vec<std::path::PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "docx").unwrap_or(false))
        .collect();
    v.sort();
    v.truncate(n);
    v
}

fn ink(pm: &folio_kit::gfx::sk::Pixmap) -> usize {
    pm.pixels().iter().filter(|p| p.demultiply().red() < 240).count()
}

#[test]
fn documents_draw_their_text() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(body).unwrap().join().unwrap();
}

fn body() {
    let paths = docs(25);
    if paths.is_empty() {
        return; // no corpus on this machine
    }
    let mut fonts = FontCache::new();
    let mut drawn = 0;
    let mut inked = 0;
    for p in &paths {
        let Ok(mut doc) = Document::open_path(&p.to_string_lossy()) else { continue };
        let layout = layout_document(&mut doc, &mut fonts);
        if layout.pages.is_empty() {
            continue;
        }
        let mut r = Renderer::new();
        let Some(pm) = render_page_image(&mut r, &layout, 0, &mut fonts, 1.5, &Overlay::default()) else {
            panic!("{:?}: no page drawn", p.file_name().unwrap())
        };
        assert!(pm.width() > 100 && pm.height() > 100, "{:?}: tiny page", p.file_name().unwrap());
        drawn += 1;
        // a page with text on it must have ink on it
        let has_text = layout.pages[0].lines.iter().any(|pl| {
            layout.paras[pl.para].lines.get(pl.line).map(|l| l.end > l.start).unwrap_or(false)
        });
        if has_text {
            let n = ink(&pm);
            assert!(n > 20, "{:?}: text on the page but {n} pixels of ink", p.file_name().unwrap());
            inked += 1;
        }
    }
    assert!(drawn >= 20, "only {drawn} documents drawn");
    assert!(inked >= 15, "only {inked} documents had text drawn");
    eprintln!("{drawn} pages drawn, {inked} with text");
}

#[test]
fn the_same_page_gives_the_same_pixels() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(same).unwrap().join().unwrap();
}

fn same() {
    let Some(p) = docs(40).into_iter().find(|p| {
        let mut fonts = FontCache::new();
        Document::open_path(&p.to_string_lossy())
            .map(|mut d| !layout_document(&mut d, &mut fonts).pages.is_empty())
            .unwrap_or(false)
    }) else {
        return;
    };
    let once = || {
        let mut fonts = FontCache::new();
        let mut doc = Document::open_path(&p.to_string_lossy()).unwrap();
        let layout = layout_document(&mut doc, &mut fonts);
        let mut r = Renderer::new();
        render_page_image(&mut r, &layout, 0, &mut fonts, 2.0, &Overlay::default()).unwrap().data().to_vec()
    };
    assert_eq!(once(), once(), "{:?} drew differently the second time", p.file_name().unwrap());
}

#[test]
fn the_marks_and_the_caret_are_drawn_over_the_text() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(marks).unwrap().join().unwrap();
}

fn marks() {
    let paths = docs(40);
    let mut fonts = FontCache::new();
    for p in &paths {
        let Ok(mut doc) = Document::open_path(&p.to_string_lossy()) else { continue };
        let layout = layout_document(&mut doc, &mut fonts);
        if layout.pages.is_empty() || layout.pages[0].lines.is_empty() {
            continue;
        }
        let mut r = Renderer::new();
        let plain = render_page_image(&mut r, &layout, 0, &mut fonts, 1.5, &Overlay::default()).unwrap();
        let ov = Overlay {
            marks: true,
            gridlines: true,
            caret: Some((72.0, 72.0, 12.0)),
            selection: vec![(72.0, 90.0, 100.0, 12.0)],
            ..Default::default()
        };
        let with = render_page_image(&mut r, &layout, 0, &mut fonts, 1.5, &ov).unwrap();
        assert!(ink(&with) > ink(&plain), "{:?}: marks added nothing", p.file_name().unwrap());
        return;
    }
}

#[test]
fn a_document_becomes_a_pdf() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(pdf).unwrap().join().unwrap();
}

fn pdf() {
    let paths = docs(40);
    let mut fonts = FontCache::new();
    for p in &paths {
        let Ok(mut doc) = Document::open_path(&p.to_string_lossy()) else { continue };
        let layout = folio_page_engine::paginate::layout_document(&mut doc, &mut fonts);
        if layout.pages.len() < 2 {
            continue;
        }
        let once = || folio_page_render::pdf_bytes(&layout, &mut FontCache::new(), &Default::default()).unwrap();
        let bytes = once();
        assert!(bytes.starts_with(b"%PDF-"), "{:?}: not a PDF", p.file_name().unwrap());
        assert!(bytes.ends_with(b"%%EOF\n") || bytes.ends_with(b"%%EOF"), "no end marker");
        let text = String::from_utf8_lossy(&bytes);
        let pages = text.matches("/Type /Page\n").count() + text.matches("/Type /Page ").count();
        assert!(pages >= layout.pages.len(), "{pages} page objects for {} pages", layout.pages.len());
        assert!(text.contains("/FontFile2"), "no font embedded");
        assert!(text.contains("/ToUnicode"), "nothing to copy the words out by");
        assert_eq!(bytes.len(), once().len(), "the same document gave a different PDF");
        return;
    }
}
