//! Page setup and printing: how a picture is laid out on paper, and the PDF that is sent to the
//! printer. The maths is the GTK version's, so both print the same sheets.

use folio_draw_engine::deflate::zlib_compress;

/// Paper sizes, in millimetres.
pub const PAPERS: [(&str, f32, f32); 5] =
    [("A4", 210.0, 297.0), ("Letter", 215.9, 279.4), ("Legal", 215.9, 355.6), ("A3", 297.0, 420.0), ("A5", 148.0, 210.0)];

/// A picture's pixel at normal size (96 dpi), in millimetres.
pub const MM_PER_PX: f32 = 25.4 / 96.0;

/// How the picture goes on the paper.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    /// Which of [`PAPERS`].
    pub paper: usize,
    pub landscape: bool,
    /// Top, right, bottom, left, in millimetres.
    pub margins: [f32; 4],
    pub centre_h: bool,
    pub centre_v: bool,
    /// Shrink to fit `fit_w` by `fit_h` sheets; otherwise scale to `percent`.
    pub fit: bool,
    pub percent: f32,
    pub fit_w: u32,
    pub fit_h: u32,
}

impl Default for Setup {
    fn default() -> Setup {
        Setup {
            paper: 0,
            landscape: false,
            margins: [19.1, 19.1, 19.1, 19.1],
            centre_h: true,
            centre_v: false,
            fit: true,
            percent: 100.0,
            fit_w: 1,
            fit_h: 1,
        }
    }
}

impl Setup {
    pub fn paper_name(&self) -> &'static str {
        PAPERS[self.paper.min(PAPERS.len() - 1)].0
    }
}

/// The paper, the long way round if it is landscape.
pub fn paper_size(s: &Setup) -> (f32, f32) {
    let (_, w, h) = PAPERS[s.paper.min(PAPERS.len() - 1)];
    if s.landscape {
        (h, w)
    } else {
        (w, h)
    }
}

/// One sheet: the part of the picture on it (pixels), where it goes (millimetres from the paper's
/// corner) and how many millimetres a picture pixel takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheet {
    pub sx: f32,
    pub sy: f32,
    pub sw: f32,
    pub sh: f32,
    pub dx: f32,
    pub dy: f32,
    pub mmpp: f32,
}

/// The sheets a picture takes, reading across then down.
pub fn layout(s: &Setup, img_w: usize, img_h: usize) -> Vec<Sheet> {
    let (pw, ph) = paper_size(s);
    let [top, right, bottom, left] = s.margins;
    let (aw, ah) = ((pw - left - right).max(10.0), (ph - top - bottom).max(10.0));
    let (iw, ih) = (img_w.max(1) as f32, img_h.max(1) as f32);
    let k = if s.fit {
        let (fw, fh) = (s.fit_w.max(1) as f32, s.fit_h.max(1) as f32);
        // shrunk to fit, never enlarged past normal size (a percentage enlarges)
        (aw * fw / (iw * MM_PER_PX)).min(ah * fh / (ih * MM_PER_PX)).min(1.0)
    } else {
        (s.percent.max(1.0)) / 100.0
    };
    let mmpp = MM_PER_PX * k;
    let (total_w, total_h) = (iw * mmpp, ih * mmpp);
    // in f64, and a hair of slack: at f32 precision a picture that just fits would spill over
    let cols = ((total_w as f64 / aw as f64 - 1e-6).ceil() as usize).max(1);
    let rows = ((total_h as f64 / ah as f64 - 1e-6).ceil() as usize).max(1);
    let (ppw, pph) = (aw / mmpp, ah / mmpp);
    let mut out = vec![];
    for r in 0..rows {
        for c in 0..cols {
            let (sx, sy) = (c as f32 * ppw, r as f32 * pph);
            let (sw, sh) = (ppw.min(iw - sx), pph.min(ih - sy));
            if sw <= 0.0 || sh <= 0.0 {
                continue;
            }
            let dx = if cols == 1 && s.centre_h { left + (aw - sw * mmpp) / 2.0 } else { left };
            let dy = if rows == 1 && s.centre_v { top + (ah - sh * mmpp) / 2.0 } else { top };
            out.push(Sheet { sx, sy, sw, sh, dx, dy, mmpp });
        }
    }
    out
}

/// "2 pages at 80% of normal size", for the Page setup box.
pub fn describe(s: &Setup, img_w: usize, img_h: usize) -> String {
    let sheets = layout(s, img_w, img_h);
    let pct = sheets.first().map(|p| p.mmpp / MM_PER_PX * 100.0).unwrap_or(100.0);
    format!("{} page{} at {}% of normal size", sheets.len(), if sheets.len() == 1 { "" } else { "s" }, pct.round() as i32)
}

/// One sheet's pixels, cut out of the picture and laid on white (paper has no transparency).
fn crop_on_white(px: &[u8], w: usize, h: usize, p: &Sheet) -> (usize, usize, Vec<u8>) {
    let x0 = (p.sx.round() as usize).min(w);
    let y0 = (p.sy.round() as usize).min(h);
    let cw = (p.sw.round() as usize).min(w - x0).max(1);
    let ch = (p.sh.round() as usize).min(h - y0).max(1);
    let mut out = vec![255u8; cw * ch * 3];
    for y in 0..ch {
        for x in 0..cw {
            let s = ((y0 + y) * w + x0 + x) * 4;
            let d = (y * cw + x) * 3;
            if s + 3 >= px.len() {
                continue;
            }
            let a = px[s + 3] as u32;
            for k in 0..3 {
                out[d + k] = ((px[s + k] as u32 * a + 255 * (255 - a)) / 255) as u8;
            }
        }
    }
    (cw, ch, out)
}

/// A PDF of the picture on paper: one page per sheet, the picture placed as laid out.
pub fn pdf_bytes(px: &[u8], w: usize, h: usize, s: &Setup) -> Result<Vec<u8>, String> {
    use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};
    let sheets = layout(s, w, h);
    if sheets.is_empty() {
        return Err("Nothing to print.".into());
    }
    let (pw_mm, ph_mm) = paper_size(s);
    // PDF units are points: 72 to the inch
    let pt = |mm: f32| mm * 72.0 / 25.4;
    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let mut next = 3i32;
    let mut id = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let mut page_refs = vec![];
    for sheet in &sheets {
        let (cw, ch, rgb) = crop_on_white(px, w, h, sheet);
        let img = id();
        let content_ref = id();
        let page_ref = id();
        let z = zlib_compress(&rgb, 6);
        let mut x = pdf.image_xobject(img, &z);
        x.width(cw as i32).height(ch as i32).color_space().device_rgb();
        x.bits_per_component(8).filter(Filter::FlateDecode);
        x.finish();
        let mut c = Content::new();
        let (dw, dh) = (sheet.sw * sheet.mmpp, sheet.sh * sheet.mmpp);
        // PDF's origin is the bottom left; the layout measures from the top
        c.save_state();
        c.transform([pt(dw), 0.0, 0.0, pt(dh), pt(sheet.dx), pt(ph_mm - sheet.dy - dh)]);
        c.x_object(Name(b"Im0"));
        c.restore_state();
        let data = c.finish();
        pdf.stream(content_ref, &data).filter(Filter::FlateDecode);
        let mut page = pdf.page(page_ref);
        page.media_box(Rect::new(0.0, 0.0, pt(pw_mm), pt(ph_mm)));
        page.parent(tree);
        page.contents(content_ref);
        page.resources().x_objects().pair(Name(b"Im0"), img);
        page.finish();
        page_refs.push(page_ref);
    }
    pdf.pages(tree).kids(page_refs.iter().copied()).count(page_refs.len() as i32);
    pdf.catalog(catalog).pages(tree);
    Ok(pdf.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_picture_takes_one_sheet_at_normal_size() {
        let s = Setup::default();
        let sheets = layout(&s, 400, 300);
        assert_eq!(sheets.len(), 1);
        assert!((sheets[0].mmpp - MM_PER_PX).abs() < 1e-6, "not normal size: {}", sheets[0].mmpp);
        // centred across the page by default
        let (pw, _) = paper_size(&s);
        let used = sheets[0].sw * sheets[0].mmpp;
        assert!((sheets[0].dx - (pw - used) / 2.0).abs() < 0.01, "not centred: {}", sheets[0].dx);
    }

    #[test]
    fn a_big_picture_is_shrunk_to_one_sheet() {
        let sheets = layout(&Setup::default(), 4000, 3000);
        assert_eq!(sheets.len(), 1);
        assert!(sheets[0].mmpp < MM_PER_PX, "it wasn't shrunk");
        let (pw, ph) = paper_size(&Setup::default());
        assert!(sheets[0].sw * sheets[0].mmpp <= pw && sheets[0].sh * sheets[0].mmpp <= ph);
    }

    #[test]
    fn a_percentage_spills_over_several_sheets() {
        let s = Setup { fit: false, percent: 400.0, ..Setup::default() };
        let sheets = layout(&s, 1000, 800);
        assert!(sheets.len() > 1, "400% fits on one sheet?");
        assert!((sheets[0].mmpp - MM_PER_PX * 4.0).abs() < 1e-6);
        // the parts cover the picture without overlapping
        let total: f32 = sheets.iter().map(|p| p.sw * p.sh).sum();
        assert!((total - 1000.0 * 800.0).abs() < 1000.0, "the sheets cover {total} of 800000 pixels");
    }

    #[test]
    fn landscape_turns_the_paper() {
        let p = paper_size(&Setup::default());
        let l = paper_size(&Setup { landscape: true, ..Setup::default() });
        assert_eq!((p.1, p.0), l);
    }

    #[test]
    fn fitting_to_two_sheets_across_lets_it_be_bigger() {
        let one = layout(&Setup::default(), 4000, 1000)[0].mmpp;
        let two = layout(&Setup { fit_w: 2, ..Setup::default() }, 4000, 1000)[0].mmpp;
        assert!(two > one, "two sheets across isn't bigger: {two} vs {one}");
    }

    #[test]
    fn the_pdf_has_one_page_per_sheet() {
        let (w, h) = (120usize, 90usize);
        let px = vec![200u8; w * h * 4];
        let s = Setup { fit: false, percent: 800.0, ..Setup::default() };
        let n = layout(&s, w, h).len();
        assert!(n > 1);
        let bytes = pdf_bytes(&px, w, h, &s).expect("no pdf");
        assert_eq!(&bytes[..5], b"%PDF-");
        let text = String::from_utf8_lossy(&bytes);
        assert_eq!(text.matches("/Type /Page\n").count() + text.matches("/Type /Page/").count() + text.matches("/Type /Page ").count(), n, "{n} pages expected");
    }

    #[test]
    fn transparency_prints_as_white() {
        let (w, h) = (4usize, 2);
        let mut px = vec![0u8; w * h * 4];
        px[0] = 255; // a red pixel, fully transparent
        px[3] = 0;
        let (cw, ch, rgb) = crop_on_white(&px, w, h, &Sheet { sx: 0.0, sy: 0.0, sw: 4.0, sh: 2.0, dx: 0.0, dy: 0.0, mmpp: 1.0 });
        assert_eq!((cw, ch), (4, 2));
        assert_eq!(&rgb[..3], &[255, 255, 255]);
    }

    #[test]
    fn the_description_counts_the_sheets() {
        assert_eq!(describe(&Setup::default(), 400, 300), "1 page at 100% of normal size");
        let s = Setup { fit: false, percent: 400.0, ..Setup::default() };
        assert!(describe(&s, 1000, 800).contains("pages at 400%"), "{}", describe(&s, 1000, 800));
    }
}
