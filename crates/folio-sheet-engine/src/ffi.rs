//! The drawing calls the engine makes, on Folio's portable base (S3-19).
//!
//! The engine was written against cairo and pango. Rather than rewrite ten thousand lines of
//! drawing code, this module keeps those names and signatures and does the work with
//! `folio-canvas` (the software renderer) and `folio-kit`'s text. Nothing here talks to the
//! operating system, so the engine is portable.
#![allow(non_camel_case_types, non_snake_case, dead_code)]

use folio_canvas::{Cx, Glyphs, Matrix, Source};
use folio_kit::gfx::sk;
use folio_kit::text::{Align, Text, TextStyle};
use folio_kit::Color;
use std::cell::RefCell;
use core::ffi::{c_char, c_double, c_int, c_uint, c_void};

// ---------------------------------------------------------------- the handles the engine passes
pub type cairo_t = c_void;
pub type cairo_surface_t = c_void;
pub type cairo_pattern_t = c_void;
pub type PangoLayout = c_void;
pub type PangoContext = c_void;
pub type PangoFontMap = c_void;
pub type PangoFontDescription = c_void;
pub type PangoAttrList = c_void;
pub type PangoAttribute = c_void;
pub type PangoFontMetrics = c_void;
pub type PangoLanguage = c_void;
pub type PangoFont = c_void;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct PangoRectangle {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
}

pub const PANGO_SCALE: f64 = 1024.0;
pub const CAIRO_FORMAT_ARGB32: c_int = 0;
pub const PANGO_WRAP_WORD_CHAR: c_int = 2;
pub const PANGO_ALIGN_LEFT: c_int = 0;
pub const PANGO_ALIGN_CENTER: c_int = 1;
pub const PANGO_ALIGN_RIGHT: c_int = 2;
pub const PANGO_STYLE_NORMAL: c_int = 0;
pub const PANGO_STYLE_ITALIC: c_int = 2;
pub const PANGO_UNDERLINE_SINGLE: c_int = 1;
pub const PANGO_UNDERLINE_DOUBLE: c_int = 2;
pub const CAIRO_ANTIALIAS_NONE: c_int = 1;
pub const CAIRO_FILL_RULE_EVEN_ODD: c_int = 1;
pub const CAIRO_LINE_CAP_BUTT: c_int = 0;
pub const CAIRO_LINE_CAP_ROUND: c_int = 1;
pub const CAIRO_LINE_CAP_SQUARE: c_int = 2;
pub const CAIRO_LINE_JOIN_MITER: c_int = 0;
pub const CAIRO_LINE_JOIN_ROUND: c_int = 1;
pub const CAIRO_LINE_JOIN_BEVEL: c_int = 2;
pub const CAIRO_OPERATOR_OVER: c_int = 2;

// ---------------------------------------------------------------- the text engine
thread_local! {
    static TEXT: RefCell<Text> = RefCell::new(Text::bundled());
}

/// Lay a piece of text out with the shared text engine.
fn with_text<T>(f: impl FnOnce(&mut Text) -> T) -> T {
    TEXT.with(|t| f(&mut t.borrow_mut()))
}

// ---------------------------------------------------------------- surfaces
struct Surf {
    pm: sk::Pixmap,
    /// Set when the pixels belong to someone else; the data is copied back on flush.
    borrowed: Option<(*mut u8, usize)>,
}

fn surf<'a>(s: *mut cairo_surface_t) -> &'a mut Surf {
    unsafe { &mut *(s as *mut Surf) }
}

pub unsafe fn cairo_image_surface_create(_format: c_int, w: c_int, h: c_int) -> *mut cairo_surface_t {
    let pm = sk::Pixmap::new(w.max(1) as u32, h.max(1) as u32).unwrap_or_else(|| sk::Pixmap::new(1, 1).unwrap());
    Box::into_raw(Box::new(Surf { pm, borrowed: None })) as *mut cairo_surface_t
}

/// Wrap pixels someone else owns. The engine hands us BGRA, as cairo's ARGB32 is on a
/// little-endian machine; tiny-skia wants RGBA, so the two are swapped in and out.
pub unsafe fn cairo_image_surface_create_for_data(data: *mut u8, _format: c_int, w: c_int, h: c_int, stride: c_int) -> *mut cairo_surface_t {
    let (w, h) = (w.max(1) as u32, h.max(1) as u32);
    let mut pm = sk::Pixmap::new(w, h).unwrap_or_else(|| sk::Pixmap::new(1, 1).unwrap());
    let stride = stride.max(0) as usize;
    if !data.is_null() && stride >= w as usize * 4 {
        let src = std::slice::from_raw_parts(data, stride * h as usize);
        let dst = pm.data_mut();
        for y in 0..h as usize {
            for x in 0..w as usize {
                let s = y * stride + x * 4;
                let d = (y * w as usize + x) * 4;
                let (b, g, r, a) = (src[s], src[s + 1], src[s + 2], src[s + 3]);
                dst[d] = r;
                dst[d + 1] = g;
                dst[d + 2] = b;
                dst[d + 3] = a;
            }
        }
    }
    Box::into_raw(Box::new(Surf { pm, borrowed: Some((data, stride)) })) as *mut cairo_surface_t
}

pub unsafe fn cairo_image_surface_get_data(s: *mut cairo_surface_t) -> *mut u8 {
    surf(s).pm.data_mut().as_mut_ptr()
}

pub unsafe fn cairo_image_surface_get_stride(s: *mut cairo_surface_t) -> c_int {
    (surf(s).pm.width() * 4) as c_int
}

pub unsafe fn cairo_image_surface_get_width(s: *mut cairo_surface_t) -> c_int {
    surf(s).pm.width() as c_int
}

pub unsafe fn cairo_image_surface_get_height(s: *mut cairo_surface_t) -> c_int {
    surf(s).pm.height() as c_int
}

pub unsafe fn cairo_surface_flush(s: *mut cairo_surface_t) {
    let sf = surf(s);
    if let Some((data, stride)) = sf.borrowed {
        let (w, h) = (sf.pm.width() as usize, sf.pm.height() as usize);
        if data.is_null() || stride < w * 4 {
            return;
        }
        let src = sf.pm.data();
        let dst = std::slice::from_raw_parts_mut(data, stride * h);
        for y in 0..h {
            for x in 0..w {
                let d = y * stride + x * 4;
                let s0 = (y * w + x) * 4;
                let (r, g, b, a) = (src[s0], src[s0 + 1], src[s0 + 2], src[s0 + 3]);
                dst[d] = b;
                dst[d + 1] = g;
                dst[d + 2] = r;
                dst[d + 3] = a;
            }
        }
    }
}

pub unsafe fn cairo_surface_destroy(s: *mut cairo_surface_t) {
    if !s.is_null() {
        drop(Box::from_raw(s as *mut Surf));
    }
}

pub unsafe fn cairo_surface_write_to_png(s: *mut cairo_surface_t, path: *const c_char) -> c_int {
    let p = std::ffi::CStr::from_ptr(path).to_string_lossy().to_string();
    let png = folio_kit::gfx::encode_png(&surf(s).pm);
    if std::fs::write(p, png).is_ok() {
        0
    } else {
        1
    }
}

// ---------------------------------------------------------------- patterns
struct Pat {
    a: (f64, f64),
    b: (f64, f64),
    stops: Vec<(f32, u32, f64)>,
}

pub unsafe fn cairo_pattern_create_linear(x0: c_double, y0: c_double, x1: c_double, y1: c_double) -> *mut cairo_pattern_t {
    Box::into_raw(Box::new(Pat { a: (x0, y0), b: (x1, y1), stops: vec![] })) as *mut cairo_pattern_t
}

pub unsafe fn cairo_pattern_add_color_stop_rgb(p: *mut cairo_pattern_t, off: c_double, r: c_double, g: c_double, b: c_double) {
    let pat = &mut *(p as *mut Pat);
    pat.stops.push((off as f32, rgb_u32(r, g, b), 1.0));
}

pub unsafe fn cairo_pattern_destroy(p: *mut cairo_pattern_t) {
    if !p.is_null() {
        drop(Box::from_raw(p as *mut Pat));
    }
}

fn rgb_u32(r: c_double, g: c_double, b: c_double) -> u32 {
    let c = |v: c_double| ((v.clamp(0.0, 1.0) * 255.0).round() as u32) & 255;
    (c(r) << 16) | (c(g) << 8) | c(b)
}

// ---------------------------------------------------------------- the drawing context
struct Ctx {
    /// The pixels being drawn on, when this context owns them.
    own: Option<Box<Surf>>,
    /// The surface the context draws on, owned or borrowed.
    target: *mut Surf,
    glyphs: Box<Glyphs>,
    cx: Option<Cx<'static>>,
}

impl Ctx {
    fn cx(&mut self) -> &mut Cx<'static> {
        self.cx.as_mut().expect("context")
    }
}

fn ctx<'a>(c: *mut cairo_t) -> &'a mut Ctx {
    unsafe { &mut *(c as *mut Ctx) }
}

fn cx<'a>(c: *mut cairo_t) -> &'a mut Cx<'static> {
    ctx(c).cx()
}

/// Build a context on a surface. The surface must outlive it, as it must in cairo.
pub unsafe fn cairo_create(target: *mut cairo_surface_t) -> *mut cairo_t {
    let mut c = Box::new(Ctx { own: None, target: target as *mut Surf, glyphs: Box::new(Glyphs::new()), cx: None });
    let pm: &'static mut sk::Pixmap = std::mem::transmute(&mut (*(c.target)).pm);
    let gl: &'static mut Glyphs = std::mem::transmute(&mut *c.glyphs);
    c.cx = Some(Cx::new(pm, gl));
    Box::into_raw(c) as *mut cairo_t
}

/// A context on a pixmap the caller owns (how the app hands the engine its window).
pub fn context_on(pm: &mut sk::Pixmap) -> Painter {
    let w = pm.width();
    let h = pm.height();
    let mut surf = Box::new(Surf { pm: sk::Pixmap::new(w.max(1), h.max(1)).unwrap(), borrowed: None });
    surf.pm.data_mut().copy_from_slice(pm.data());
    let target: *mut Surf = &mut *surf;
    let mut c = Box::new(Ctx { own: Some(surf), target, glyphs: Box::new(Glyphs::new()), cx: None });
    unsafe {
        let p: &'static mut sk::Pixmap = std::mem::transmute(&mut (*(c.target)).pm);
        let gl: &'static mut Glyphs = std::mem::transmute(&mut *c.glyphs);
        c.cx = Some(Cx::new(p, gl));
    }
    Painter { cr: Box::into_raw(c) as *mut cairo_t }
}

/// A context handed to the engine, which copies what was drawn back when it is done.
pub struct Painter {
    cr: *mut cairo_t,
}

impl Painter {
    pub fn cr(&self) -> *mut cairo_t {
        self.cr
    }

    /// Put what was drawn onto the caller's pixmap.
    pub fn finish(self, pm: &mut sk::Pixmap) {
        let c = ctx(self.cr);
        let drawn = unsafe { &(*(c.target)).pm };
        if drawn.width() == pm.width() && drawn.height() == pm.height() {
            pm.data_mut().copy_from_slice(drawn.data());
        }
        unsafe { cairo_destroy(self.cr) };
        std::mem::forget(self);
    }
}

impl Drop for Painter {
    fn drop(&mut self) {
        unsafe { cairo_destroy(self.cr) };
    }
}

pub unsafe fn cairo_destroy(cr: *mut cairo_t) {
    if !cr.is_null() {
        let mut c = Box::from_raw(cr as *mut Ctx);
        c.cx = None; // the borrows go first
        drop(c);
    }
}

pub unsafe fn cairo_reference(cr: *mut cairo_t) -> *mut cairo_t {
    cr
}

pub unsafe fn cairo_status(_cr: *mut cairo_t) -> c_int {
    0
}

pub unsafe fn cairo_save(cr: *mut cairo_t) {
    cx(cr).save();
}

pub unsafe fn cairo_restore(cr: *mut cairo_t) {
    cx(cr).restore();
}

pub unsafe fn cairo_rectangle(cr: *mut cairo_t, x: c_double, y: c_double, w: c_double, h: c_double) {
    cx(cr).rectangle(x, y, w, h);
}

pub unsafe fn cairo_clip(cr: *mut cairo_t) {
    cx(cr).clip();
}

pub unsafe fn cairo_clip_preserve(cr: *mut cairo_t) {
    cx(cr).clip_preserve();
}

pub unsafe fn cairo_reset_clip(cr: *mut cairo_t) {
    cx(cr).reset_clip();
}

pub unsafe fn cairo_fill(cr: *mut cairo_t) {
    cx(cr).fill();
}

pub unsafe fn cairo_fill_preserve(cr: *mut cairo_t) {
    cx(cr).fill_preserve();
}

pub unsafe fn cairo_stroke(cr: *mut cairo_t) {
    cx(cr).stroke();
}

pub unsafe fn cairo_stroke_preserve(cr: *mut cairo_t) {
    cx(cr).stroke_preserve();
}

pub unsafe fn cairo_paint(cr: *mut cairo_t) {
    cx(cr).paint();
}

pub unsafe fn cairo_new_path(cr: *mut cairo_t) {
    cx(cr).new_path();
}

pub unsafe fn cairo_close_path(cr: *mut cairo_t) {
    cx(cr).close_path();
}

pub unsafe fn cairo_move_to(cr: *mut cairo_t, x: c_double, y: c_double) {
    cx(cr).move_to(x, y);
}

pub unsafe fn cairo_line_to(cr: *mut cairo_t, x: c_double, y: c_double) {
    cx(cr).line_to(x, y);
}

pub unsafe fn cairo_rel_line_to(cr: *mut cairo_t, dx: c_double, dy: c_double) {
    let c = cx(cr);
    if let Some((x, y)) = c.current_point() {
        c.line_to(x + dx, y + dy);
    } else {
        c.move_to(dx, dy);
    }
}

pub unsafe fn cairo_curve_to(cr: *mut cairo_t, x1: c_double, y1: c_double, x2: c_double, y2: c_double, x3: c_double, y3: c_double) {
    cx(cr).curve_to(x1, y1, x2, y2, x3, y3);
}

pub unsafe fn cairo_arc(cr: *mut cairo_t, xc: c_double, yc: c_double, r: c_double, a1: c_double, a2: c_double) {
    cx(cr).arc(xc, yc, r, a1, a2);
}

pub unsafe fn cairo_arc_negative(cr: *mut cairo_t, xc: c_double, yc: c_double, r: c_double, a1: c_double, a2: c_double) {
    cx(cr).arc_negative(xc, yc, r, a1, a2);
}

pub unsafe fn cairo_set_source_rgb(cr: *mut cairo_t, r: c_double, g: c_double, b: c_double) {
    cx(cr).set_source_rgb(r, g, b);
}

pub unsafe fn cairo_set_source_rgba(cr: *mut cairo_t, r: c_double, g: c_double, b: c_double, a: c_double) {
    cx(cr).set_source_rgba(r, g, b, a);
}

pub unsafe fn cairo_set_line_width(cr: *mut cairo_t, w: c_double) {
    cx(cr).set_line_width(w);
}

pub unsafe fn cairo_set_dash(cr: *mut cairo_t, dashes: *const c_double, n: c_int, offset: c_double) {
    let d: Vec<f64> = if dashes.is_null() || n <= 0 { vec![] } else { std::slice::from_raw_parts(dashes, n as usize).to_vec() };
    cx(cr).set_dash(&d, offset);
}

pub unsafe fn cairo_set_antialias(_cr: *mut cairo_t, _a: c_int) {}

pub unsafe fn cairo_set_fill_rule(cr: *mut cairo_t, r: c_int) {
    cx(cr).set_even_odd(r == CAIRO_FILL_RULE_EVEN_ODD);
}

pub unsafe fn cairo_set_line_cap(cr: *mut cairo_t, c: c_int) {
    cx(cr).set_line_cap(match c {
        CAIRO_LINE_CAP_ROUND => sk::LineCap::Round,
        CAIRO_LINE_CAP_SQUARE => sk::LineCap::Square,
        _ => sk::LineCap::Butt,
    });
}

pub unsafe fn cairo_set_line_join(cr: *mut cairo_t, j: c_int) {
    cx(cr).set_line_join(match j {
        CAIRO_LINE_JOIN_ROUND => sk::LineJoin::Round,
        CAIRO_LINE_JOIN_BEVEL => sk::LineJoin::Bevel,
        _ => sk::LineJoin::Miter,
    });
}

pub unsafe fn cairo_set_operator(_cr: *mut cairo_t, _op: c_int) {}

pub unsafe fn cairo_translate(cr: *mut cairo_t, x: c_double, y: c_double) {
    cx(cr).translate(x, y);
}

pub unsafe fn cairo_rotate(cr: *mut cairo_t, a: c_double) {
    cx(cr).rotate(a);
}

pub unsafe fn cairo_scale(cr: *mut cairo_t, x: c_double, y: c_double) {
    cx(cr).scale(x, y);
}

pub unsafe fn cairo_set_source_surface(cr: *mut cairo_t, s: *mut cairo_surface_t, x: c_double, y: c_double) {
    let sf = surf(s);
    let (w, h) = (sf.pm.width() as f64, sf.pm.height() as f64);
    let copy = std::sync::Arc::new(sf.pm.clone());
    cx(cr).set_source_pixmap(copy, x, y, w, h);
}

pub unsafe fn cairo_set_source(cr: *mut cairo_t, p: *mut cairo_pattern_t) {
    let pat = &*(p as *mut Pat);
    cx(cr).set_source(Source::Linear(pat.a, pat.b, pat.stops.clone()));
}

// ---------------------------------------------------------------- text
#[derive(Clone, Default)]
struct Desc {
    family: String,
    size: f64,
    bold: bool,
    italic: bool,
}

pub unsafe fn pango_font_description_new() -> *mut PangoFontDescription {
    Box::into_raw(Box::new(Desc { family: String::new(), size: 12.0, bold: false, italic: false })) as *mut PangoFontDescription
}

pub unsafe fn pango_font_description_free(d: *mut PangoFontDescription) {
    if !d.is_null() {
        drop(Box::from_raw(d as *mut Desc));
    }
}

pub unsafe fn pango_font_description_set_family(d: *mut PangoFontDescription, f: *const c_char) {
    (*(d as *mut Desc)).family = std::ffi::CStr::from_ptr(f).to_string_lossy().to_string();
}

pub unsafe fn pango_font_description_set_absolute_size(d: *mut PangoFontDescription, s: c_double) {
    (*(d as *mut Desc)).size = s / PANGO_SCALE;
}

pub unsafe fn pango_font_description_set_weight(d: *mut PangoFontDescription, w: c_int) {
    (*(d as *mut Desc)).bold = w >= 600;
}

pub unsafe fn pango_font_description_set_style(d: *mut PangoFontDescription, s: c_int) {
    (*(d as *mut Desc)).italic = s != PANGO_STYLE_NORMAL;
}

#[derive(Default)]
struct Attrs {
    underline: c_int,
    strike: bool,
    rise: f64,
    scale: f64,
}

pub unsafe fn pango_attr_list_new() -> *mut PangoAttrList {
    Box::into_raw(Box::new(Attrs { underline: 0, strike: false, rise: 0.0, scale: 1.0 })) as *mut PangoAttrList
}

pub unsafe fn pango_attr_list_insert(l: *mut PangoAttrList, a: *mut PangoAttribute) {
    let at = Box::from_raw(a as *mut Attrs);
    let list = &mut *(l as *mut Attrs);
    if at.underline != 0 {
        list.underline = at.underline;
    }
    if at.strike {
        list.strike = true;
    }
    if at.rise != 0.0 {
        list.rise = at.rise;
    }
    if at.scale != 1.0 {
        list.scale = at.scale;
    }
}

pub unsafe fn pango_attr_list_unref(l: *mut PangoAttrList) {
    if !l.is_null() {
        drop(Box::from_raw(l as *mut Attrs));
    }
}

pub unsafe fn pango_attr_underline_new(u: c_int) -> *mut PangoAttribute {
    Box::into_raw(Box::new(Attrs { underline: u, ..Default::default() })) as *mut PangoAttribute
}

pub unsafe fn pango_attr_strikethrough_new(s: c_int) -> *mut PangoAttribute {
    Box::into_raw(Box::new(Attrs { strike: s != 0, scale: 1.0, ..Default::default() })) as *mut PangoAttribute
}

pub unsafe fn pango_attr_rise_new(r: c_int) -> *mut PangoAttribute {
    Box::into_raw(Box::new(Attrs { rise: r as f64 / PANGO_SCALE, scale: 1.0, ..Default::default() })) as *mut PangoAttribute
}

pub unsafe fn pango_attr_scale_new(s: c_double) -> *mut PangoAttribute {
    Box::into_raw(Box::new(Attrs { scale: s, ..Default::default() })) as *mut PangoAttribute
}

struct Lay {
    text: String,
    desc: Desc,
    width: c_int,
    align: c_int,
    wrap: c_int,
    attrs: Attrs,
}

impl Lay {
    fn style(&self) -> TextStyle {
        TextStyle {
            family: self.desc.family.clone(),
            size: (self.desc.size * self.attrs.scale.max(0.01)) as f32,
            bold: self.desc.bold,
            italic: self.desc.italic,
            underline: false,
            color: Color::BLACK,
            align: match self.align {
                PANGO_ALIGN_CENTER => Align::Center,
                PANGO_ALIGN_RIGHT => Align::Right,
                _ => Align::Left,
            },
            line: 1.0,
        }
    }

    fn wrap_width(&self) -> Option<f32> {
        if self.width > 0 {
            Some(self.width as f32 / PANGO_SCALE as f32)
        } else {
            None
        }
    }

    fn measure(&self) -> (f64, f64, f64, usize) {
        let st = self.style();
        with_text(|t| {
            let l = t.layout(&self.text, &st, self.wrap_width());
            let base = l.lines.first().map(|x| x.baseline as f64).unwrap_or(st.size as f64);
            (l.width as f64, l.height as f64, base, l.lines.len().max(1))
        })
    }
}

fn lay<'a>(l: *mut PangoLayout) -> &'a mut Lay {
    unsafe { &mut *(l as *mut Lay) }
}

pub unsafe fn pango_layout_new(_ctx: *mut PangoContext) -> *mut PangoLayout {
    Box::into_raw(Box::new(Lay {
        text: String::new(),
        desc: Desc { family: String::new(), size: 12.0, bold: false, italic: false },
        width: -1,
        align: PANGO_ALIGN_LEFT,
        wrap: PANGO_WRAP_WORD_CHAR,
        attrs: Attrs { underline: 0, strike: false, rise: 0.0, scale: 1.0 },
    })) as *mut PangoLayout
}

pub unsafe fn pango_cairo_create_layout(_cr: *mut cairo_t) -> *mut PangoLayout {
    pango_layout_new(std::ptr::null_mut())
}

pub unsafe fn pango_layout_set_text(l: *mut PangoLayout, text: *const c_char, len: c_int) {
    let bytes = std::slice::from_raw_parts(text as *const u8, len.max(0) as usize);
    lay(l).text = String::from_utf8_lossy(bytes).to_string();
}

pub unsafe fn set_text(l: *mut PangoLayout, s: &str) {
    lay(l).text = s.to_string();
}

pub unsafe fn pango_layout_set_font_description(l: *mut PangoLayout, d: *const PangoFontDescription) {
    if !d.is_null() {
        lay(l).desc = (*(d as *const Desc)).clone();
    }
}

pub unsafe fn pango_layout_set_width(l: *mut PangoLayout, w: c_int) {
    lay(l).width = w;
}

pub unsafe fn pango_layout_set_wrap(l: *mut PangoLayout, w: c_int) {
    lay(l).wrap = w;
}

pub unsafe fn pango_layout_set_alignment(l: *mut PangoLayout, a: c_int) {
    lay(l).align = a;
}

pub unsafe fn pango_layout_set_attributes(l: *mut PangoLayout, a: *mut PangoAttrList) {
    let list = if a.is_null() { Attrs { underline: 0, strike: false, rise: 0.0, scale: 1.0 } } else { Attrs { ..*(&*(a as *mut Attrs)) } };
    lay(l).attrs = list;
}

pub unsafe fn pango_layout_get_pixel_size(l: *mut PangoLayout, w: *mut c_int, h: *mut c_int) {
    let (lw, lh, _, _) = lay(l).measure();
    if !w.is_null() {
        *w = lw.ceil() as c_int;
    }
    if !h.is_null() {
        *h = lh.ceil() as c_int;
    }
}

pub unsafe fn pango_layout_get_extents(l: *mut PangoLayout, ink: *mut PangoRectangle, logical: *mut PangoRectangle) {
    let (lw, lh, _, _) = lay(l).measure();
    let r = PangoRectangle { x: 0, y: 0, width: (lw * PANGO_SCALE) as c_int, height: (lh * PANGO_SCALE) as c_int };
    if !ink.is_null() {
        *ink = r;
    }
    if !logical.is_null() {
        *logical = r;
    }
}

pub unsafe fn pango_layout_get_line_count(l: *mut PangoLayout) -> c_int {
    lay(l).measure().3 as c_int
}

pub unsafe fn pango_layout_get_baseline(l: *mut PangoLayout) -> c_int {
    (lay(l).measure().2 * PANGO_SCALE) as c_int
}

pub unsafe fn pango_layout_get_context(_l: *mut PangoLayout) -> *mut PangoContext {
    std::ptr::null_mut()
}

pub unsafe fn pango_layout_context_changed(_l: *mut PangoLayout) {}

pub unsafe fn pango_cairo_update_layout(_cr: *mut cairo_t, _l: *mut PangoLayout) {}

pub unsafe fn pango_cairo_show_layout(cr: *mut cairo_t, l: *mut PangoLayout) {
    draw_layout(cr, l, false);
}

pub unsafe fn pango_cairo_layout_path(cr: *mut cairo_t, l: *mut PangoLayout) {
    draw_layout(cr, l, true);
}

/// Draw the layout at the current point, or add it to the path.
unsafe fn draw_layout(cr: *mut cairo_t, l: *mut PangoLayout, as_path: bool) {
    let la = lay(l);
    if la.text.is_empty() {
        return;
    }
    let st = la.style();
    let w = la.wrap_width();
    let rise = la.attrs.rise;
    let (paths, lines, width, height) = with_text(|t| {
        let lay_out = t.layout(&la.text, &st, w);
        (t.outlines(&la.text, &st, w), lay_out.lines.clone(), lay_out.width, lay_out.height)
    });
    let c = cx(cr);
    let (ox, oy) = c.current_point().unwrap_or((0.0, 0.0));
    if as_path {
        c.new_path();
    }
    for p in &paths {
        if as_path {
            c.append_sk_path(p, ox, oy - rise);
        } else {
            c.fill_sk_path(p, ox, oy - rise);
        }
    }
    // underline and strikethrough, which pango would draw for us
    if la.attrs.underline != 0 || la.attrs.strike {
        let thick = (st.size as f64 / 14.0).max(0.7);
        for li in &lines {
            let (x0, x1) = (ox + li.x as f64, ox + (li.x + li.width) as f64);
            if la.attrs.underline != 0 {
                let y = oy + (li.top + li.baseline) as f64 + thick * 2.0 - rise;
                c.rectangle(x0, y, x1 - x0, thick);
                if la.attrs.underline == PANGO_UNDERLINE_DOUBLE {
                    c.rectangle(x0, y + thick * 2.0, x1 - x0, thick);
                }
            }
            if la.attrs.strike {
                let y = oy + (li.top + li.baseline) as f64 - st.size as f64 * 0.3 - rise;
                c.rectangle(x0, y, x1 - x0, thick);
            }
        }
        if as_path {
            // the marks are part of the path the caller is building
        } else {
            c.fill();
        }
    }
    let _ = (width, height);
}

// ---------------------------------------------------------------- font metrics
struct Metrics {
    ascent: f64,
    descent: f64,
}

pub unsafe fn pango_cairo_font_map_get_default() -> *mut PangoFontMap {
    std::ptr::null_mut()
}

pub unsafe fn pango_font_map_create_context(_m: *mut PangoFontMap) -> *mut PangoContext {
    std::ptr::null_mut()
}

pub unsafe fn pango_cairo_context_set_resolution(_c: *mut PangoContext, _dpi: c_double) {}

pub unsafe fn pango_context_load_font(_c: *mut PangoContext, _d: *const PangoFontDescription) -> *mut PangoFont {
    1 as *mut PangoFont
}

pub unsafe fn pango_context_get_metrics(_c: *mut PangoContext, d: *const PangoFontDescription, _lang: *mut PangoLanguage) -> *mut PangoFontMetrics {
    let desc = if d.is_null() { Desc { family: String::new(), size: 12.0, bold: false, italic: false } } else { (*(d as *const Desc)).clone() };
    let st = TextStyle {
        family: desc.family.clone(),
        size: desc.size as f32,
        bold: desc.bold,
        italic: desc.italic,
        underline: false,
        color: Color::BLACK,
        align: Align::Left,
        line: 1.0,
    };
    let (ascent, descent) = with_text(|t| {
        let l = t.layout("Ag", &st, None);
        let base = l.lines.first().map(|x| x.baseline as f64).unwrap_or(desc.size * 0.8);
        (base, (l.height as f64 - base).max(desc.size * 0.2))
    });
    Box::into_raw(Box::new(Metrics { ascent, descent })) as *mut PangoFontMetrics
}

pub unsafe fn pango_font_metrics_get_ascent(m: *mut PangoFontMetrics) -> c_int {
    ((*(m as *mut Metrics)).ascent * PANGO_SCALE) as c_int
}

pub unsafe fn pango_font_metrics_get_descent(m: *mut PangoFontMetrics) -> c_int {
    ((*(m as *mut Metrics)).descent * PANGO_SCALE) as c_int
}

pub unsafe fn pango_font_metrics_get_height(m: *mut PangoFontMetrics) -> c_int {
    let me = &*(m as *mut Metrics);
    ((me.ascent + me.descent) * PANGO_SCALE) as c_int
}

pub unsafe fn pango_font_metrics_unref(m: *mut PangoFontMetrics) {
    if !m.is_null() {
        drop(Box::from_raw(m as *mut Metrics));
    }
}

pub unsafe fn g_object_unref(o: *mut c_void) {
    // layouts are the only things the engine unrefs
    if !o.is_null() {
        drop(Box::from_raw(o as *mut Lay));
    }
}

pub fn cstr(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s.replace('\0', " ")).unwrap()
}

pub fn unused(_: c_uint) {}

/// The transform the context is drawing with (the app uses it for hit testing).
pub unsafe fn matrix_of(cr: *mut cairo_t) -> Matrix {
    cx(cr).matrix()
}
