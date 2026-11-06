//! Write a document to PDF, for looking at.
fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("usage: page_pdf <file.docx> [out.pdf]");
    let out = a.next().unwrap_or_else(|| "out.pdf".into());
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let mut fonts = folio_page_engine::fonts::FontCache::new();
            let mut doc = folio_page_engine::document::Document::open_path(&path).expect("open");
            let layout = folio_page_engine::paginate::layout_document(&mut doc, &mut fonts);
            folio_page_render::export_pdf(&layout, &mut fonts, &out).expect("pdf");
            println!("{} pages, wrote {out}", layout.pages.len());
        })
        .unwrap()
        .join()
        .unwrap();
}
