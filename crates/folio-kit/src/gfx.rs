//! Drawing helpers over tiny-skia, the toolkit's software renderer.

pub use resvg::tiny_skia as sk;
use serde::{Deserialize, Serialize};
use sk::{FillRule, Paint, PathBuilder, Pixmap, Rect as SkRect, Stroke, Transform};

/// An sRGB colour with alpha. Serialised as "#rrggbb" or "#rrggbbaa".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r, g, b, a }
    }
    pub const fn hex(v: u32) -> Color {
        Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    pub fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }
    pub fn to_sk(self) -> sk::Color {
        sk::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        let v = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Color::rgb(v(0)?, v(2)?, v(4)?)),
            8 => Some(Color::rgba(v(0)?, v(2)?, v(4)?, v(6)?)),
            3 => {
                let d = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok().map(|x| x * 17);
                Some(Color::rgb(d(0)?, d(1)?, d(2)?))
            }
            _ => None,
        }
    }
    /// Mix toward `o` by `t` (0..1).
    pub fn mix(self, o: Color, t: f32) -> Color {
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;
        Color::rgba(l(self.r, o.r), l(self.g, o.g), l(self.b, o.b), l(self.a, o.a))
    }
    /// Relative luminance, 0..1.
    pub fn luma(self) -> f32 {
        (0.2126 * self.r as f32 + 0.7152 * self.g as f32 + 0.0722 * self.b as f32) / 255.0
    }
    /// Black or white, whichever reads better on this colour.
    pub fn contrast_text(self) -> Color {
        if self.luma() > 0.55 { Color::hex(0x202020) } else { Color::WHITE }
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("bad colour {s}")))
    }
}

/// An axis-aligned rectangle in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
    pub fn to_sk(&self) -> Option<SkRect> {
        SkRect::from_xywh(self.x, self.y, self.w.max(0.01), self.h.max(0.01))
    }
    pub fn round(&self) -> Rect {
        Rect::new(self.x.round(), self.y.round(), self.w.round(), self.h.round())
    }
}

pub fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c.to_sk());
    p.anti_alias = true;
    p
}

pub fn fill_rect(pm: &mut Pixmap, r: Rect, c: Color) {
    if c.a == 0 {
        return;
    }
    if c.a == 255 {
        // Opaque: write the pixels directly (same edges as the renderer: rounded to nearest).
        let rd = |v: f32| (v + 0.5).floor() as i64;
        let (w, h) = (pm.width() as i64, pm.height() as i64);
        let (x0, y0) = (rd(r.x).clamp(0, w), rd(r.y).clamp(0, h));
        let (x1, y1) = (rd(r.x + r.w).clamp(0, w), rd(r.y + r.h).clamp(0, h));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let px = [c.r, c.g, c.b, 255];
        let d = pm.data_mut();
        for y in y0..y1 {
            let row = &mut d[((y * w + x0) * 4) as usize..((y * w + x1) * 4) as usize];
            for p in row.chunks_exact_mut(4) {
                p.copy_from_slice(&px);
            }
        }
        return;
    }
    if let Some(sr) = r.to_sk() {
        let mut p = paint(c);
        p.anti_alias = false;
        pm.fill_rect(sr, &p, Transform::identity(), None);
    }
}

pub fn rounded_path(r: Rect, rad: f32) -> Option<sk::Path> {
    let rad = rad.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    let mut pb = PathBuilder::new();
    if rad <= 0.01 {
        pb.push_rect(r.to_sk()?);
        return pb.finish();
    }
    let k = 0.5523 * rad;
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    pb.move_to(x0 + rad, y0);
    pb.line_to(x1 - rad, y0);
    pb.cubic_to(x1 - rad + k, y0, x1, y0 + rad - k, x1, y0 + rad);
    pb.line_to(x1, y1 - rad);
    pb.cubic_to(x1, y1 - rad + k, x1 - rad + k, y1, x1 - rad, y1);
    pb.line_to(x0 + rad, y1);
    pb.cubic_to(x0 + rad - k, y1, x0, y1 - rad + k, x0, y1 - rad);
    pb.line_to(x0, y0 + rad);
    pb.cubic_to(x0, y0 + rad - k, x0 + rad - k, y0, x0 + rad, y0);
    pb.close();
    pb.finish()
}

pub fn fill_round(pm: &mut Pixmap, r: Rect, rad: f32, c: Color) {
    if c.a == 0 {
        return;
    }
    if let Some(p) = rounded_path(r, rad) {
        pm.fill_path(&p, &paint(c), FillRule::Winding, Transform::identity(), None);
    }
}

pub fn stroke_round(pm: &mut Pixmap, r: Rect, rad: f32, c: Color, width: f32) {
    if c.a == 0 {
        return;
    }
    let r = Rect::new(r.x + width / 2.0, r.y + width / 2.0, r.w - width, r.h - width);
    if let Some(p) = rounded_path(r, rad) {
        let st = Stroke { width, ..Stroke::default() };
        pm.stroke_path(&p, &paint(c), &st, Transform::identity(), None);
    }
}

pub fn line(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, c: Color, width: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1, y1);
    if let Some(p) = pb.finish() {
        let st = Stroke { width, ..Stroke::default() };
        pm.stroke_path(&p, &paint(c), &st, Transform::identity(), None);
    }
}

/// A 1-pixel crisp horizontal or vertical line.
pub fn hline(pm: &mut Pixmap, x0: f32, x1: f32, y: f32, c: Color) {
    fill_rect(pm, Rect::new(x0.round(), y.round(), (x1 - x0).round(), 1.0), c);
}

pub fn vline(pm: &mut Pixmap, x: f32, y0: f32, y1: f32, c: Color) {
    fill_rect(pm, Rect::new(x.round(), y0.round(), 1.0, (y1 - y0).round()), c);
}

/// A soft drop shadow under a rounded rectangle (popups, cards).
pub fn shadow(pm: &mut Pixmap, r: Rect, rad: f32, dark: bool) {
    let a = if dark { 70 } else { 28 };
    for i in 1..=6 {
        let d = i as f32 * 1.5;
        fill_round(pm, Rect::new(r.x - d / 2.0, r.y - d / 2.0 + 2.0, r.w + d, r.h + d), rad + d, Color::rgba(0, 0, 0, a / i as u8));
    }
}

/// Blend one pixel (straight-alpha colour) into a premultiplied pixmap.
#[inline]
pub fn blend_px(data: &mut [u8], w: u32, h: u32, x: i32, y: i32, c: Color, cover: u8) {
    if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
        return;
    }
    let a = (c.a as u32 * cover as u32 + 127) / 255;
    if a == 0 {
        return;
    }
    let i = ((y as u32 * w + x as u32) * 4) as usize;
    let inv = 255 - a;
    let px = &mut data[i..i + 4];
    px[0] = ((c.r as u32 * a + px[0] as u32 * inv + 127) / 255) as u8;
    px[1] = ((c.g as u32 * a + px[1] as u32 * inv + 127) / 255) as u8;
    px[2] = ((c.b as u32 * a + px[2] as u32 * inv + 127) / 255) as u8;
    px[3] = ((255 * a + px[3] as u32 * inv + 127) / 255) as u8;
}

/// Draw an RGBA image (straight alpha) scaled into `dst` with an optional clip.
pub fn draw_image(pm: &mut Pixmap, img: &Pixmap, dst: Rect, opacity: f32, quality: sk::FilterQuality) {
    let sx = dst.w / img.width() as f32;
    let sy = dst.h / img.height() as f32;
    let pat = sk::Pattern::new(
        img.as_ref(),
        sk::SpreadMode::Pad,
        quality,
        opacity,
        Transform::from_row(sx, 0.0, 0.0, sy, dst.x, dst.y),
    );
    let mut p = Paint::default();
    p.shader = pat;
    p.anti_alias = true;
    if let Some(r) = dst.to_sk() {
        pm.fill_rect(r, &p, Transform::identity(), None);
    }
}

/// Convert straight-alpha RGBA bytes into a premultiplied pixmap.
pub fn pixmap_from_rgba(w: u32, h: u32, rgba: &[u8]) -> Option<Pixmap> {
    let mut pm = Pixmap::new(w.max(1), h.max(1))?;
    for (d, s) in pm.data_mut().chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        let a = s[3] as u32;
        d[0] = ((s[0] as u32 * a + 127) / 255) as u8;
        d[1] = ((s[1] as u32 * a + 127) / 255) as u8;
        d[2] = ((s[2] as u32 * a + 127) / 255) as u8;
        d[3] = s[3];
    }
    Some(pm)
}

/// Premultiplied pixmap back to straight-alpha RGBA bytes.
pub fn rgba_from_pixmap(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.data().len());
    for p in pm.data().chunks_exact(4) {
        let a = p[3] as u32;
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            out.push(((p[0] as u32 * 255 + a / 2) / a).min(255) as u8);
            out.push(((p[1] as u32 * 255 + a / 2) / a).min(255) as u8);
            out.push(((p[2] as u32 * 255 + a / 2) / a).min(255) as u8);
            out.push(p[3]);
        }
    }
    out
}

pub fn encode_png(pm: &Pixmap) -> Vec<u8> {
    pm.encode_png().unwrap_or_default()
}

/// Decode PNG, JPEG, GIF, WebP or BMP bytes into a premultiplied pixmap.
pub fn decode_image(bytes: &[u8]) -> Option<Pixmap> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    pixmap_from_rgba(img.width(), img.height(), img.as_raw())
}

/// Copy `src` onto `dst` at (x, y), clipped: for opaque layers, much faster than a general
/// draw (X3-12: a full-screen blit per frame has to be cheap on a Pi).
pub fn blit_copy(dst: &mut Pixmap, src: &Pixmap, x: i32, y: i32) {
    blit(dst, src, x, y, false)
}

/// Draw premultiplied `src` over `dst` at (x, y), clipped; clear pixels cost almost nothing.
pub fn blit_over(dst: &mut Pixmap, src: &Pixmap, x: i32, y: i32) {
    blit(dst, src, x, y, true)
}

fn blit(dst: &mut Pixmap, src: &Pixmap, x: i32, y: i32, over: bool) {
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + sw).min(dw), (y + sh).min(dh));
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let n = ((x1 - x0) * 4) as usize;
    let sd = src.data();
    let dd = dst.data_mut();
    for row in y0..y1 {
        let si = (((row - y) * sw + (x0 - x)) * 4) as usize;
        let di = ((row * dw + x0) * 4) as usize;
        let (s, d) = (&sd[si..si + n], &mut dd[di..di + n]);
        if !over {
            d.copy_from_slice(s);
            continue;
        }
        for (sp, dp) in s.chunks_exact(4).zip(d.chunks_exact_mut(4)) {
            let a = sp[3] as u32;
            if a == 0 {
                continue;
            }
            if a == 255 {
                dp.copy_from_slice(sp);
                continue;
            }
            let inv = 255 - a;
            for k in 0..4 {
                dp[k] = (sp[k] as u32 + (dp[k] as u32 * inv + 127) / 255).min(255) as u8;
            }
        }
    }
}

/// Merge pixel boxes (x0, y0, x1, y1) that overlap or touch, clipped to w×h; one box for the
/// whole area when they cover most of it anyway.
pub fn merge_boxes(mut v: Vec<(i32, i32, i32, i32)>, w: i32, h: i32) -> Vec<(i32, i32, i32, i32)> {
    v = v.into_iter().map(|b| (b.0.clamp(0, w), b.1.clamp(0, h), b.2.clamp(0, w), b.3.clamp(0, h))).filter(|b| b.2 > b.0 && b.3 > b.1).collect();
    loop {
        let mut merged = false;
        'outer: for i in 0..v.len() {
            for j in i + 1..v.len() {
                let (a, b) = (v[i], v[j]);
                if a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3 {
                    v[i] = (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3));
                    v.remove(j);
                    merged = true;
                    break 'outer;
                }
            }
        }
        if !merged {
            break;
        }
    }
    let area: i64 = v.iter().map(|b| (b.2 - b.0) as i64 * (b.3 - b.1) as i64).sum();
    if area * 10 > w as i64 * h as i64 * 6 {
        return vec![(0, 0, w, h)];
    }
    v
}
