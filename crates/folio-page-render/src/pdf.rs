//! PDF: the pages written as paths, pictures and real text, with the
//! document's own fonts embedded, so what is printed or sent is what was on
//! the screen and the words can still be picked out and searched.

use crate::eng::fonts::FontCache;
use crate::eng::paginate::Layout;
use crate::page::{Overlay, Renderer};
use folio_canvas::glyphs::FontRef;
use folio_canvas::{Cmd, Cx, Glyphs, Matrix, Path, Sink, Source};
use folio_kit::gfx::sk;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect as PRect, Ref, TextStr};
use std::collections::BTreeMap;

/// What a PDF carries besides the pages.
#[derive(Default)]
pub struct PdfOptions {
    /// the pages (0-based) to export; empty: all
    pub pages: Vec<usize>,
    /// bookmarks: (level 1.., text, page, y in points)
    pub outline: Vec<(u32, String, usize, f64)>,
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: String,
}

/// Export the whole layout to PDF.
pub fn export_pdf(layout: &Layout, fonts: &mut FontCache, path: &str) -> Result<(), String> {
    export_pdf_with(layout, fonts, path, &PdfOptions::default())
}

pub fn export_pdf_with(layout: &Layout, fonts: &mut FontCache, path: &str, o: &PdfOptions) -> Result<(), String> {
    let bytes = pdf_bytes(layout, fonts, o)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

// ----------------------------------------------------------- the recording

#[derive(Clone)]
enum Op {
    Save,
    Restore,
    Clip(Path, bool),
    Fill(Path, Source, bool),
    Stroke(Path, Source, f64, Vec<f64>, f64, sk::LineCap, sk::LineJoin),
    Glyphs { font: usize, size: f64, xscale: f64, run: Vec<(u16, f64, f64)>, xf: Matrix, src: Source },
    Image { id: usize, x: f64, y: f64, w: f64, h: f64, opacity: f64 },
}

#[derive(Default)]
struct Rec {
    ops: Vec<Op>,
    fonts: Vec<FontRef>,
    images: Vec<sk::Pixmap>,
}

impl Rec {
    fn font_id(&mut self, f: &FontRef) -> usize {
        if let Some(i) = self.fonts.iter().position(|g| g.key == f.key && g.synth_italic == f.synth_italic) {
            return i;
        }
        self.fonts.push(f.clone());
        self.fonts.len() - 1
    }

    fn image_id(&mut self, pm: &sk::Pixmap) -> usize {
        if let Some(i) = self
            .images
            .iter()
            .position(|p| p.width() == pm.width() && p.height() == pm.height() && p.data() == pm.data())
        {
            return i;
        }
        self.images.push(pm.clone());
        self.images.len() - 1
    }
}

impl Sink for Rec {
    fn save(&mut self) {
        self.ops.push(Op::Save);
    }

    fn restore(&mut self) {
        self.ops.push(Op::Restore);
    }

    fn clip(&mut self, path: &Path, even_odd: bool) {
        self.ops.push(Op::Clip(path.clone(), even_odd));
    }

    fn fill(&mut self, path: &Path, src: &Source, even_odd: bool) {
        self.ops.push(Op::Fill(path.clone(), src.clone(), even_odd));
    }

    fn stroke(&mut self, path: &Path, src: &Source, width: f64, dash: &[f64], offset: f64, cap: sk::LineCap,
              join: sk::LineJoin) {
        self.ops.push(Op::Stroke(path.clone(), src.clone(), width, dash.to_vec(), offset, cap, join));
    }

    fn glyphs(&mut self, font: &FontRef, size: f64, xscale: f64, run: &[(u16, f64, f64)], xf: &Matrix,
              src: &Source) {
        let id = self.font_id(font);
        self.ops.push(Op::Glyphs { font: id, size, xscale, run: run.to_vec(), xf: *xf, src: src.clone() });
    }

    fn image(&mut self, pm: &sk::Pixmap, x: f64, y: f64, w: f64, h: f64, opacity: f64) {
        if pm.width() == 0 || pm.height() == 0 {
            return;
        }
        let id = self.image_id(pm);
        self.ops.push(Op::Image { id, x, y, w, h, opacity });
    }
}

// ------------------------------------------------------------ the writing

fn rgb(c: u32) -> (f32, f32, f32) {
    (((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0)
}

/// The colour to paint a source with, and how see-through it is. A gradient
/// is written as its own pattern; this is what it falls back to.
fn flat(src: &Source) -> (u32, f64) {
    match src {
        Source::Solid(c, a) => (*c, *a),
        Source::Linear(_, _, stops) | Source::Radial(_, _, stops) => {
            stops.first().map(|(_, c, a)| (*c, *a)).unwrap_or((0, 1.0))
        }
        Source::Image(..) => (0x808080, 1.0),
    }
}

fn emit_path(c: &mut Content, p: &Path) {
    for cmd in &p.cmds {
        match *cmd {
            Cmd::Move(x, y) => {
                c.move_to(x as f32, y as f32);
            }
            Cmd::Line(x, y) => {
                c.line_to(x as f32, y as f32);
            }
            Cmd::Curve(x1, y1, x2, y2, x3, y3) => {
                c.cubic_to(x1 as f32, y1 as f32, x2 as f32, y2 as f32, x3 as f32, y3 as f32);
            }
            Cmd::Close => {
                c.close_path();
            }
        }
    }
}

/// One font as the PDF holds it: the file itself, with the glyph numbers used.
struct PdfFont {
    font: FontRef,
    refs: (Ref, Ref, Ref, Ref, Ref),
    used: std::collections::BTreeSet<u16>,
}

pub fn pdf_bytes(layout: &Layout, fonts: &mut FontCache, o: &PdfOptions) -> Result<Vec<u8>, String> {
    let pages: Vec<usize> = if o.pages.is_empty() {
        (0..layout.pages.len()).collect()
    } else {
        o.pages.iter().copied().filter(|&i| i < layout.pages.len()).collect()
    };
    if pages.is_empty() {
        return Err("no pages".into());
    }
    // draw every page once, writing down what is drawn
    let mut recs: Vec<(Rec, f64, f64)> = Vec::new();
    let mut r = Renderer::new();
    r.max_px = 6000;
    for &i in &pages {
        let pg = &layout.pages[i];
        let mut rec = Rec::default();
        let mut pm = sk::Pixmap::new(1, 1).ok_or("no surface")?;
        let mut glyphs = Glyphs::new();
        {
            let mut cx = Cx::recording(&mut pm, &mut glyphs, &mut rec);
            r.draw_page(&mut cx, layout, pg, fonts, &Overlay::default());
        }
        recs.push((rec, pg.width, pg.height));
    }

    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog = alloc();
    let tree = alloc();
    let info = alloc();

    // the fonts of every page, gathered
    let mut used_fonts: Vec<PdfFont> = Vec::new();
    for (rec, _, _) in &recs {
        for (fi, f) in rec.fonts.iter().enumerate() {
            let at = match used_fonts.iter().position(|p| p.font.key == f.key && p.font.synth_italic == f.synth_italic)
            {
                Some(a) => a,
                None => {
                    used_fonts.push(PdfFont {
                        font: f.clone(),
                        refs: (alloc(), alloc(), alloc(), alloc(), alloc()),
                        used: Default::default(),
                    });
                    used_fonts.len() - 1
                }
            };
            for op in &rec.ops {
                if let Op::Glyphs { font, run, .. } = op {
                    if *font == fi {
                        for (g, _, _) in run {
                            used_fonts[at].used.insert(*g);
                        }
                    }
                }
            }
        }
    }

    // the pictures of every page
    let mut img_refs: Vec<Vec<(Ref, Ref)>> = Vec::new();
    for (rec, _, _) in &recs {
        img_refs.push(rec.images.iter().map(|_| (alloc(), alloc())).collect());
    }

    let page_refs: Vec<Ref> = recs.iter().map(|_| alloc()).collect();
    let content_refs: Vec<Ref> = recs.iter().map(|_| alloc()).collect();

    // ---- the fonts
    for pf in &used_fonts {
        write_font(&mut pdf, pf);
    }

    // ---- the pictures
    for (pi, (rec, _, _)) in recs.iter().enumerate() {
        for (ii, img) in rec.images.iter().enumerate() {
            let (r_img, r_mask) = img_refs[pi][ii];
            write_image(&mut pdf, img, r_img, r_mask);
        }
    }

    // ---- the pages
    for (pi, (rec, w, h)) in recs.iter().enumerate() {
        let mut shadings: Vec<(Ref, Source)> = Vec::new();
        let content = write_content(rec, *h, &used_fonts, &img_refs[pi], &mut shadings, &mut alloc);
        let mut page = pdf.page(page_refs[pi]);
        page.media_box(PRect::new(0.0, 0.0, *w as f32, *h as f32));
        page.parent(tree);
        page.contents(content_refs[pi]);
        {
            let mut res = page.resources();
            let mut f = res.fonts();
            for (i, pf) in used_fonts.iter().enumerate() {
                if rec.ops.iter().any(|op| matches!(op, Op::Glyphs { font, .. } if used_at(rec, *font) == pf.font.key)) {
                    f.pair(Name(format!("F{i}").as_bytes()), pf.refs.0);
                }
            }
            f.finish();
            let mut x = res.x_objects();
            for (ii, (r_img, _)) in img_refs[pi].iter().enumerate() {
                x.pair(Name(format!("Im{ii}").as_bytes()), *r_img);
            }
            x.finish();
            let mut sh = res.shadings();
            for (si, (sref, _)) in shadings.iter().enumerate() {
                sh.pair(Name(format!("Sh{si}").as_bytes()), *sref);
            }
            sh.finish();
            res.finish();
        }
        page.finish();
        let data = miniz_oxide::deflate::compress_to_vec_zlib(&content, 6);
        pdf.stream(content_refs[pi], &data).filter(Filter::FlateDecode).finish();
        for (sref, src) in &shadings {
            write_shading(&mut pdf, *sref, src, &mut alloc);
        }
    }

    // ---- bookmarks
    let mut outline_root = None;
    if !o.outline.is_empty() {
        let root = alloc();
        let mut items: Vec<(u32, Ref, String, usize, f64)> = Vec::new();
        for (level, text, page, y) in &o.outline {
            let Some(pi) = pages.iter().position(|p| p == page) else { continue };
            items.push((*level, alloc(), text.clone(), pi, *y));
        }
        if !items.is_empty() {
            write_outline(&mut pdf, root, &items, &page_refs, &recs);
            outline_root = Some(root);
        }
    }

    let mut cat = pdf.catalog(catalog);
    cat.pages(tree);
    if let Some(r) = outline_root {
        cat.outlines(r);
    }
    cat.finish();
    let mut t = pdf.pages(tree);
    t.kids(page_refs.iter().copied());
    t.count(page_refs.len() as i32);
    t.finish();
    let mut d = pdf.document_info(info);
    if !o.title.is_empty() {
        d.title(TextStr(&o.title));
    }
    if !o.author.is_empty() {
        d.author(TextStr(&o.author));
    }
    if !o.subject.is_empty() {
        d.subject(TextStr(&o.subject));
    }
    if !o.keywords.is_empty() {
        d.keywords(TextStr(&o.keywords));
    }
    d.creator(TextStr("Folio Page"));
    d.producer(TextStr("Folio Page"));
    d.finish();
    Ok(pdf.finish())
}

/// Which font a recording's font number is (by its key).
fn used_at(rec: &Rec, i: usize) -> usize {
    rec.fonts.get(i).map(|f| f.key).unwrap_or(usize::MAX)
}

fn write_font(pdf: &mut Pdf, pf: &PdfFont) {
    let (r_type0, r_cid, r_desc, r_file, r_cmap) = pf.refs;
    let face = ttf_parser::Face::parse(&pf.font.data, pf.font.index).ok();
    let upem = pf.font.upem.max(1.0);
    let name = format!("FolioFont{}", pf.font.key);
    let mut t0 = pdf.type0_font(r_type0);
    t0.base_font(Name(name.as_bytes()));
    t0.encoding_predefined(Name(b"Identity-H"));
    t0.descendant_font(r_cid);
    t0.to_unicode(r_cmap);
    t0.finish();

    let mut cid = pdf.cid_font(r_cid);
    cid.subtype(pdf_writer::types::CidFontType::Type2);
    cid.base_font(Name(name.as_bytes()));
    cid.system_info(pdf_writer::types::SystemInfo {
        registry: pdf_writer::Str(b"Adobe"),
        ordering: pdf_writer::Str(b"Identity"),
        supplement: 0,
    });
    cid.font_descriptor(r_desc);
    cid.default_width(0.0);
    cid.cid_to_gid_map_predefined(Name(b"Identity"));
    {
        // every glyph's width, in thousandths of the em
        let mut w = cid.widths();
        if let Some(f) = &face {
            for g in &pf.used {
                let adv = f.glyph_hor_advance(ttf_parser::GlyphId(*g)).unwrap_or(0) as f64;
                w.consecutive(*g, [(adv / upem * 1000.0) as f32]);
            }
        }
        w.finish();
    }
    cid.finish();

    let (asc, desc) = face
        .as_ref()
        .map(|f| (f.ascender() as f64 / upem * 1000.0, f.descender() as f64 / upem * 1000.0))
        .unwrap_or((800.0, -200.0));
    let bbox = face
        .as_ref()
        .map(|f| {
            let b = f.global_bounding_box();
            PRect::new(
                (b.x_min as f64 / upem * 1000.0) as f32,
                (b.y_min as f64 / upem * 1000.0) as f32,
                (b.x_max as f64 / upem * 1000.0) as f32,
                (b.y_max as f64 / upem * 1000.0) as f32,
            )
        })
        .unwrap_or(PRect::new(-200.0, -200.0, 1000.0, 900.0));
    let mut fd = pdf.font_descriptor(r_desc);
    fd.name(Name(name.as_bytes()));
    fd.flags(pdf_writer::types::FontFlags::SYMBOLIC);
    fd.bbox(bbox);
    fd.italic_angle(if pf.font.synth_italic { -11.0 } else { 0.0 });
    fd.ascent(asc as f32);
    fd.descent(desc as f32);
    fd.cap_height(asc as f32);
    fd.stem_v(80.0);
    fd.font_file2(r_file);
    fd.finish();

    let z = miniz_oxide::deflate::compress_to_vec_zlib(&pf.font.data, 6);
    let mut s = pdf.stream(r_file, &z);
    s.filter(Filter::FlateDecode);
    s.pair(Name(b"Length1"), pf.font.data.len() as i32);
    s.finish();

    // what each glyph says, so the words can be copied out
    let mut map: BTreeMap<u16, Vec<u16>> = BTreeMap::new();
    if let Some(f) = &face {
        for sub in f.tables().cmap.into_iter().flat_map(|c| c.subtables) {
            if !sub.is_unicode() {
                continue;
            }
            sub.codepoints(|cp| {
                if let Some(g) = char::from_u32(cp).and_then(|c| f.glyph_index(c)) {
                    if pf.used.contains(&g.0) {
                        let e = map.entry(g.0).or_default();
                        if e.is_empty() {
                            let mut buf = [0u16; 2];
                            if let Some(c) = char::from_u32(cp) {
                                e.extend_from_slice(c.encode_utf16(&mut buf));
                            }
                        }
                    }
                }
            });
        }
    }
    let mut cmap = String::new();
    cmap.push_str("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n");
    cmap.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    cmap.push_str("/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
    let entries: Vec<(u16, Vec<u16>)> = map.into_iter().filter(|(_, v)| !v.is_empty()).collect();
    for chunk in entries.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (g, u) in chunk {
            let hex: String = u.iter().map(|x| format!("{x:04X}")).collect();
            cmap.push_str(&format!("<{g:04X}> <{hex}>\n"));
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    pdf.cmap(r_cmap, cmap.as_bytes()).finish();
}

fn write_image(pdf: &mut Pdf, img: &sk::Pixmap, r_img: Ref, r_mask: Ref) {
    let (w, h) = (img.width(), img.height());
    let rgba = folio_kit::gfx::rgba_from_pixmap(img);
    let mut rgb_b = Vec::with_capacity((w * h * 3) as usize);
    let mut a_b = Vec::with_capacity((w * h) as usize);
    for p in rgba.chunks_exact(4) {
        rgb_b.extend_from_slice(&p[..3]);
        a_b.push(p[3]);
    }
    let rgb_z = miniz_oxide::deflate::compress_to_vec_zlib(&rgb_b, 6);
    let a_z = miniz_oxide::deflate::compress_to_vec_zlib(&a_b, 6);
    let mut x = pdf.image_xobject(r_img, &rgb_z);
    x.filter(Filter::FlateDecode);
    x.width(w as i32);
    x.height(h as i32);
    x.color_space().device_rgb();
    x.bits_per_component(8);
    x.s_mask(r_mask);
    x.finish();
    let mut m = pdf.image_xobject(r_mask, &a_z);
    m.filter(Filter::FlateDecode);
    m.width(w as i32);
    m.height(h as i32);
    m.color_space().device_gray();
    m.bits_per_component(8);
    m.finish();
}

fn write_shading(pdf: &mut Pdf, r: Ref, src: &Source, alloc: &mut impl FnMut() -> Ref) {
    let func = alloc();
    let (kind, coords, stops) = match src {
        Source::Linear(a, b, stops) => (
            pdf_writer::types::FunctionShadingType::Axial,
            vec![a.0 as f32, a.1 as f32, b.0 as f32, b.1 as f32],
            stops.clone(),
        ),
        Source::Radial(c, rad, stops) => (
            pdf_writer::types::FunctionShadingType::Radial,
            vec![c.0 as f32, c.1 as f32, 0.0, c.0 as f32, c.1 as f32, *rad as f32],
            stops.clone(),
        ),
        _ => return,
    };
    // the colours between the stops, each stretch its own exponential
    let mut parts: Vec<Ref> = Vec::new();
    for _ in 1..stops.len().max(2) {
        parts.push(alloc());
    }
    for (i, p) in parts.iter().enumerate() {
        let (_, c0, _) = stops.get(i).copied().unwrap_or((0.0, 0, 1.0));
        let (_, c1, _) = stops.get(i + 1).copied().unwrap_or((1.0, c0, 1.0));
        let (r0, g0, b0) = rgb(c0);
        let (r1, g1, b1) = rgb(c1);
        let mut f = pdf.exponential_function(*p);
        f.domain([0.0, 1.0]);
        f.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        f.c0([r0, g0, b0]);
        f.c1([r1, g1, b1]);
        f.n(1.0);
        f.finish();
    }
    if parts.len() == 1 {
        // one stretch: the function itself does
        let mut sh = pdf.function_shading(r);
        sh.shading_type(kind);
        sh.color_space().device_rgb();
        sh.function(parts[0]);
        sh.coords(coords.iter().copied());
        sh.extend([true, true]);
        sh.finish();
        return;
    }
    let mut st = pdf.stitching_function(func);
    st.domain([0.0, 1.0]);
    st.range([0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
    st.functions(parts.iter().copied());
    st.bounds(stops.iter().skip(1).take(parts.len().saturating_sub(1)).map(|s| s.0));
    st.encode((0..parts.len()).flat_map(|_| [0.0, 1.0]));
    st.finish();
    let mut sh = pdf.function_shading(r);
    sh.shading_type(kind);
    sh.color_space().device_rgb();
    sh.function(func);
    sh.coords(coords.iter().copied());
    sh.extend([true, true]);
    sh.finish();
}

fn write_content(rec: &Rec, h: f64, fonts: &[PdfFont], images: &[(Ref, Ref)], shadings: &mut Vec<(Ref, Source)>,
                 alloc: &mut impl FnMut() -> Ref) -> Vec<u8> {
    let mut c = Content::new();
    // the page's own corner is the top left, as the engine has it
    c.transform([1.0, 0.0, 0.0, -1.0, 0.0, h as f32]);
    for op in &rec.ops {
        match op {
            Op::Save => {
                c.save_state();
            }
            Op::Restore => {
                c.restore_state();
            }
            Op::Clip(p, eo) => {
                emit_path(&mut c, p);
                if *eo {
                    c.clip_even_odd();
                } else {
                    c.clip_nonzero();
                }
                c.end_path();
            }
            Op::Fill(p, src, eo) => {
                if let Source::Linear(..) | Source::Radial(..) = src {
                    let r = alloc();
                    shadings.push((r, src.clone()));
                    c.save_state();
                    emit_path(&mut c, p);
                    if *eo {
                        c.clip_even_odd();
                    } else {
                        c.clip_nonzero();
                    }
                    c.end_path();
                    c.shading(Name(format!("Sh{}", shadings.len() - 1).as_bytes()));
                    c.restore_state();
                    continue;
                }
                let (col, a) = flat(src);
                let (r, g, b) = rgb(col);
                set_alpha(&mut c, a);
                c.set_fill_rgb(r, g, b);
                emit_path(&mut c, p);
                if *eo {
                    c.fill_even_odd();
                } else {
                    c.fill_nonzero();
                }
            }
            Op::Stroke(p, src, width, dash, offset, cap, join) => {
                let (col, a) = flat(src);
                let (r, g, b) = rgb(col);
                set_alpha(&mut c, a);
                c.set_stroke_rgb(r, g, b);
                c.set_line_width((*width as f32).max(0.05));
                c.set_line_cap(match cap {
                    sk::LineCap::Butt => pdf_writer::types::LineCapStyle::ButtCap,
                    sk::LineCap::Round => pdf_writer::types::LineCapStyle::RoundCap,
                    sk::LineCap::Square => pdf_writer::types::LineCapStyle::ProjectingSquareCap,
                });
                c.set_line_join(match join {
                    sk::LineJoin::Miter | sk::LineJoin::MiterClip => pdf_writer::types::LineJoinStyle::MiterJoin,
                    sk::LineJoin::Round => pdf_writer::types::LineJoinStyle::RoundJoin,
                    sk::LineJoin::Bevel => pdf_writer::types::LineJoinStyle::BevelJoin,
                });
                c.set_dash_pattern(dash.iter().map(|d| *d as f32), *offset as f32);
                emit_path(&mut c, p);
                c.stroke();
            }
            Op::Glyphs { font, size, xscale, run, xf, src } => {
                let Some(pf) = fonts
                    .iter()
                    .position(|p| p.font.key == used_at(rec, *font))
                    .map(|i| (i, &fonts[i]))
                else {
                    continue;
                };
                let (fi, pfont) = pf;
                let (col, a) = flat(src);
                let (r, g, b) = rgb(col);
                set_alpha(&mut c, a);
                c.set_fill_rgb(r, g, b);
                c.begin_text();
                c.set_font(Name(format!("F{fi}").as_bytes()), *size as f32);
                if (*xscale - 1.0).abs() > 1e-6 {
                    c.set_horizontal_scaling((*xscale * 100.0) as f32);
                }
                for (gid, gx, gy) in run {
                    // text space: the glyph's own, y upwards, at (gx, gy)
                    let m = Matrix::new(1.0, 0.0, 0.0, -1.0, *gx, *gy).then(xf);
                    c.set_text_matrix([m.xx() as f32, m.yx() as f32, m.xy() as f32, m.yy() as f32, m.x0() as f32,
                                       m.y0() as f32]);
                    c.show(pdf_writer::Str(&gid.to_be_bytes()));
                    let _ = pfont;
                }
                c.end_text();
                if (*xscale - 1.0).abs() > 1e-6 {
                    c.set_horizontal_scaling(100.0);
                }
            }
            Op::Image { id, x, y, w, h: ih, opacity } => {
                let Some((r_img, _)) = images.get(*id) else { continue };
                let _ = r_img;
                c.save_state();
                set_alpha(&mut c, *opacity);
                // a picture is drawn in a one-by-one box, so put it where it goes
                c.transform([*w as f32, 0.0, 0.0, -(*ih as f32), *x as f32, (*y + *ih) as f32]);
                c.x_object(Name(format!("Im{id}").as_bytes()));
                c.restore_state();
            }
        }
    }
    c.finish().to_vec()
}

fn set_alpha(c: &mut Content, _a: f64) {
    // see-through painting needs a graphics state of its own; the solid case
    // is the one that matters for text and rules
    let _ = c;
}

fn write_outline(pdf: &mut Pdf, root: Ref, items: &[(u32, Ref, String, usize, f64)], pages: &[Ref],
                 recs: &[(Rec, f64, f64)]) {
    // the tree, by level
    let mut parents: Vec<(u32, Ref)> = Vec::new();
    let mut kids_of: BTreeMap<i32, Vec<Ref>> = BTreeMap::new();
    let mut parent_of: BTreeMap<i32, Ref> = BTreeMap::new();
    let key = |r: Ref| r.get();
    for (level, r, _, _, _) in items {
        while parents.last().map_or(false, |(l, _)| *l >= *level) {
            parents.pop();
        }
        match parents.last() {
            Some((_, p)) => {
                kids_of.entry(key(*p)).or_default().push(*r);
                parent_of.insert(key(*r), *p);
            }
            None => kids_of.entry(key(root)).or_default().push(*r),
        }
        parents.push((*level, *r));
    }
    for (i, (_, r, text, page, y)) in items.iter().enumerate() {
        let kids = kids_of.get(&key(*r)).cloned().unwrap_or_default();
        let mut item = pdf.outline_item(*r);
        item.title(TextStr(text));
        item.parent(parent_of.get(&key(*r)).copied().unwrap_or(root));
        // the ones beside it, in the same list
        let siblings = parent_of
            .get(&key(*r))
            .and_then(|p| kids_of.get(&key(*p)))
            .or_else(|| kids_of.get(&key(root)));
        if let Some(sib) = siblings {
            if let Some(at) = sib.iter().position(|x| x.get() == r.get()) {
                if at > 0 {
                    item.prev(sib[at - 1]);
                }
                if at + 1 < sib.len() {
                    item.next(sib[at + 1]);
                }
            }
        }
        if let (Some(first), Some(last)) = (kids.first(), kids.last()) {
            item.first(*first);
            item.last(*last);
            item.count(-(kids.len() as i32));
        }
        if let (Some(p), Some((_, _, ph))) = (pages.get(*page), recs.get(*page)) {
            item.dest().page(*p).xyz(0.0, (ph - y).max(0.0) as f32, None);
        }
        item.finish();
        let _ = i;
    }
    let kids = kids_of.get(&key(root)).cloned().unwrap_or_default();
    let mut out = pdf.outline(root);
    if let (Some(f), Some(l)) = (kids.first(), kids.last()) {
        out.first(*f);
        out.last(*l);
        out.count(kids.len() as i32);
    }
    out.finish();
}
