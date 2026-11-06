//! The picture effects that work on pixels: recolouring, soft edges, glows,
//! shadows and a perspective turn. The values themselves are read by the
//! engine; this is what becomes of the pixels.

use crate::eng::effects::Fx;
use folio_kit::gfx::sk;
use folio_math::Det;

/// Change a picture's pixels (premultiplied) as its colour settings say.
pub fn recolor(s: &mut sk::Pixmap, fx: &Fx) {
    let b = fx.brightness;
    let c = fx.contrast;
    // contrast as a slope about the middle grey
    let slope = if c >= 0.0 { 1.0 / (1.0 - c).max(0.01) } else { 1.0 + c };
    for px in s.data_mut().chunks_exact_mut(4) {
        let a = px[3] as f64 / 255.0;
        if a <= 0.0 {
            continue;
        }
        let mut ch = [px[0] as f64 / 255.0 / a, px[1] as f64 / 255.0 / a, px[2] as f64 / 255.0 / a];
        for v in ch.iter_mut() {
            *v = ((*v - 0.5) * slope + 0.5 + b).clamp(0.0, 1.0);
        }
        let lum = 0.299 * ch[0] + 0.587 * ch[1] + 0.114 * ch[2];
        if fx.grayscale {
            ch = [lum, lum, lum];
        }
        if let Some(t) = fx.bilevel {
            let v = if lum >= t { 1.0 } else { 0.0 };
            ch = [v, v, v];
        }
        if let Some((d, l)) = fx.duotone {
            let mix = |k: u32| ((d >> k) & 255) as f64 / 255.0 * (1.0 - lum) + ((l >> k) & 255) as f64 / 255.0 * lum;
            ch = [mix(16), mix(8), mix(0)];
        }
        let na = a * fx.opacity;
        px[0] = (ch[0] * na * 255.0).round() as u8;
        px[1] = (ch[1] * na * 255.0).round() as u8;
        px[2] = (ch[2] * na * 255.0).round() as u8;
        px[3] = (na * 255.0).round() as u8;
    }
}

/// The alpha of a picture, in a grey picture `pad` pixels bigger on each side.
pub fn alpha_mask(s: &sk::Pixmap, pad: i32) -> Option<sk::Mask> {
    let (w, h) = (s.width() as i32, s.height() as i32);
    let mut m = sk::Mask::new((w + 2 * pad).max(1) as u32, (h + 2 * pad).max(1) as u32)?;
    let mw = m.width() as usize;
    let src = s.data().to_vec();
    let dst = m.data_mut();
    for y in 0..h as usize {
        for x in 0..w as usize {
            dst[(y + pad as usize) * mw + x + pad as usize] = src[y * w as usize * 4 + x * 4 + 3];
        }
    }
    Some(m)
}

/// Blur a grey picture: three box passes each way, close enough to a gaussian
/// of the radius given.
pub fn blur(m: &mut sk::Mask, radius: f64) {
    let r = (radius / 2.0).round() as i32;
    if r <= 0 {
        return;
    }
    let (w, h) = (m.width() as usize, m.height() as usize);
    let data = m.data_mut();
    let mut buf: Vec<f32> = vec![0.0; w.max(h)];
    let mut line: Vec<f32> = vec![0.0; w.max(h)];
    for _ in 0..3 {
        for y in 0..h {
            let row = &mut data[y * w..y * w + w];
            for (i, v) in row.iter().enumerate() {
                line[i] = *v as f32;
            }
            box_pass(&line[..w], &mut buf[..w], r);
            for (d, v) in row.iter_mut().zip(&buf[..w]) {
                *d = v.round().clamp(0.0, 255.0) as u8;
            }
        }
        for x in 0..w {
            for y in 0..h {
                line[y] = data[y * w + x] as f32;
            }
            box_pass(&line[..h], &mut buf[..h], r);
            for y in 0..h {
                data[y * w + x] = buf[y].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn box_pass(src: &[f32], dst: &mut [f32], r: i32) {
    let n = src.len() as i32;
    let win = (2 * r + 1) as f32;
    let mut sum: f32 = 0.0;
    for i in -r..=r {
        if i >= 0 && i < n {
            sum += src[i as usize];
        }
    }
    for i in 0..n {
        dst[i as usize] = sum / win;
        let out = i - r;
        let inn = i + r + 1;
        if out >= 0 {
            sum -= src[out as usize];
        }
        if inn < n {
            sum += src[inn as usize];
        }
    }
}

/// Strengthen every value: a glow is a blurred outline, made solid.
pub fn boost(m: &mut sk::Mask, f: f64) {
    for v in m.data_mut().iter_mut() {
        *v = ((*v as f64) * f).min(255.0) as u8;
    }
}

/// Turn a grey picture upside down in its values: the inside of an outline.
pub fn invert(m: &mut sk::Mask) {
    for v in m.data_mut().iter_mut() {
        *v = 255 - *v;
    }
}

/// Multiply a picture's alpha by a mask of the same size (soft edges).
pub fn multiply_alpha(s: &mut sk::Pixmap, m: &sk::Mask) {
    let (w, h) = (s.width() as usize, s.height() as usize);
    let mw = m.width() as usize;
    let md = m.data().to_vec();
    let sd = s.data_mut();
    for y in 0..h.min(m.height() as usize) {
        for x in 0..w.min(mw) {
            let f = md[y * mw + x] as u32;
            let p = &mut sd[y * w * 4 + x * 4..y * w * 4 + x * 4 + 4];
            for c in p.iter_mut() {
                *c = ((*c as u32 * f + 127) / 255) as u8;
            }
        }
    }
}

/// A picture drawn foreshortened by a perspective turn. `k` is pixels per
/// point. Gives the new picture and where its top-left corner sits relative
/// to the picture's centre, in points.
pub fn perspective_warp(
    src: &sk::Pixmap,
    w: f64,
    h: f64,
    k: f64,
    cam: (f64, f64, f64),
    fov: f64,
) -> Option<(sk::Pixmap, f64, f64, f64, f64)> {
    use crate::eng::effects::{homography, project};
    let dist = (w.max(h) / 2.0) / (fov.to_radians() / 2.0).dtan().max(0.05) * 1.6;
    let (la, lo, rv) = cam;
    let corners = [(-w / 2.0, -h / 2.0), (w / 2.0, -h / 2.0), (w / 2.0, h / 2.0), (-w / 2.0, h / 2.0)];
    let q: Vec<(f64, f64)> = corners.iter().map(|&(x, y)| project(x, y, la, lo, rv, dist)).collect();
    let x0 = q.iter().map(|p| p.0).fold(f64::MAX, f64::min);
    let x1 = q.iter().map(|p| p.0).fold(f64::MIN, f64::max);
    let y0 = q.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    let y1 = q.iter().map(|p| p.1).fold(f64::MIN, f64::max);
    let (ow, oh) = (((x1 - x0) * k).ceil().max(1.0) as i32, ((y1 - y0) * k).ceil().max(1.0) as i32);
    if ow > 9000 || oh > 9000 {
        return None;
    }
    let (sw, sh) = (src.width() as i32, src.height() as i32);
    let src_px = [(0.0, 0.0), (sw as f64, 0.0), (sw as f64, sh as f64), (0.0, sh as f64)];
    let hm = homography([q[0], q[1], q[2], q[3]], src_px)?;
    let sdata = src.data().to_vec();
    let mut out = sk::Pixmap::new(ow as u32, oh as u32)?;
    let stride_s = sw as usize * 4;
    let stride_o = ow as usize * 4;
    {
        let od = out.data_mut();
        let fetch = |x: i32, y: i32| -> [f64; 4] {
            if x < 0 || y < 0 || x >= sw || y >= sh {
                return [0.0; 4];
            }
            let i = y as usize * stride_s + x as usize * 4;
            [sdata[i] as f64, sdata[i + 1] as f64, sdata[i + 2] as f64, sdata[i + 3] as f64]
        };
        for v in 0..oh {
            for u in 0..ow {
                let px = x0 + (u as f64 + 0.5) / k;
                let py = y0 + (v as f64 + 0.5) / k;
                let d = hm[6] * px + hm[7] * py + hm[8];
                if d.abs() < 1e-12 {
                    continue;
                }
                let sx = (hm[0] * px + hm[1] * py + hm[2]) / d - 0.5;
                let sy = (hm[3] * px + hm[4] * py + hm[5]) / d - 0.5;
                if sx < -1.0 || sy < -1.0 || sx > sw as f64 || sy > sh as f64 {
                    continue;
                }
                let (fx, fy) = (sx.floor(), sy.floor());
                let (ax, ay) = (sx - fx, sy - fy);
                let (ix, iy) = (fx as i32, fy as i32);
                let p00 = fetch(ix, iy);
                let p10 = fetch(ix + 1, iy);
                let p01 = fetch(ix, iy + 1);
                let p11 = fetch(ix + 1, iy + 1);
                let o = v as usize * stride_o + u as usize * 4;
                for c in 0..4 {
                    let val = p00[c] * (1.0 - ax) * (1.0 - ay)
                        + p10[c] * ax * (1.0 - ay)
                        + p01[c] * (1.0 - ax) * ay
                        + p11[c] * ax * ay;
                    od[o + c] = val.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    Some((out, x0, y0, x1 - x0, y1 - y0))
}

/// A mask holding one colour's shape, to paint a shadow or a glow through.
pub fn shift(m: &sk::Mask, dx: i32, dy: i32, w: u32, h: u32) -> Option<sk::Mask> {
    let mut out = sk::Mask::new(w, h)?;
    let (mw, mh) = (m.width() as i32, m.height() as i32);
    let src = m.data().to_vec();
    let dst = out.data_mut();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let (sx, sy) = (x - dx, y - dy);
            if sx >= 0 && sy >= 0 && sx < mw && sy < mh {
                dst[y as usize * w as usize + x as usize] = src[sy as usize * mw as usize + sx as usize];
            }
        }
    }
    Some(out)
}

/// A mask painted in one colour, ready to draw: shadows and glows are
/// nothing but their own shape in a colour.
pub fn colored(m: &sk::Mask, color: u32, alpha: f64) -> Option<sk::Pixmap> {
    let mut pm = sk::Pixmap::new(m.width(), m.height())?;
    let (r, g, b) = (((color >> 16) & 255) as u32, ((color >> 8) & 255) as u32, (color & 255) as u32);
    let src = m.data();
    let dst = pm.data_mut();
    for (i, v) in src.iter().enumerate() {
        let a = (*v as f64 * alpha.clamp(0.0, 1.0)).round() as u32;
        if a == 0 {
            continue;
        }
        let o = i * 4;
        // premultiplied, as the surface keeps its pixels
        dst[o] = ((r * a + 127) / 255) as u8;
        dst[o + 1] = ((g * a + 127) / 255) as u8;
        dst[o + 2] = ((b * a + 127) / 255) as u8;
        dst[o + 3] = a as u8;
    }
    Some(pm)
}

/// Fade a picture out from its bottom edge upwards, as a reflection does:
/// `start` at the bottom, `end` once `end_pos` of the height has been
/// covered, and nothing above that.
pub fn fade_down(s: &mut sk::Pixmap, start: f64, end: f64, end_pos: f64) {
    let (w, h) = (s.width() as usize, s.height() as usize);
    if h == 0 {
        return;
    }
    let span = (end_pos.max(1e-6) * h as f64).max(1.0);
    let data = s.data_mut();
    for y in 0..h {
        let up = (h - 1 - y) as f64; // rows above the bottom edge
        let t = (up / span).clamp(0.0, 1.0);
        let a = (start + (end - start) * t).clamp(0.0, 1.0);
        let f = (a * 255.0).round() as u32;
        for x in 0..w {
            let o = y * w * 4 + x * 4;
            for c in 0..4 {
                data[o + c] = ((data[o + c] as u32 * f + 127) / 255) as u8;
            }
        }
    }
}
