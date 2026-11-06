//! Draw a page of a document to a PNG, for looking at.
fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().expect("usage: page_png <file.docx> [page] [out.png]");
    let page: usize = a.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let out = a.next().unwrap_or_else(|| "page.png".into());
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let mut fonts = folio_page_engine::fonts::FontCache::new();
            let mut doc = folio_page_engine::document::Document::open_path(&path).expect("open");
            let layout = folio_page_engine::paginate::layout_document(&mut doc, &mut fonts);
            let mut r = folio_page_render::Renderer::new();
            let pm = folio_page_render::render_page_image(
                &mut r, &layout, page, &mut fonts, 2.0, &folio_page_render::Overlay::default(),
            )
            .expect("page");
            pm.save_png(&out).expect("write");
            println!("{} pages, wrote {out}", layout.pages.len());
        })
        .unwrap()
        .join()
        .unwrap();
}
