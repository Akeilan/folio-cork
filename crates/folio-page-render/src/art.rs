//! Page borders: the lines and the little pictures Word draws round a page.
//! The borders themselves are read by the engine; this draws them.

use crate::eng::art::{is_line, PageBorder, PageBorders};
use folio_canvas::Cx;
use folio_math::Det;

fn rgb(cr: &mut Cx, c: u32) {
    cr.set_source_rgb(((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
}

pub fn draw(cr: &mut Cx, b: &PageBorders, page_w: f64, page_h: f64, text: (f64, f64, f64, f64)) {
    // each side's outer edge
    let w = |i: usize| b.sides[i].as_ref().map_or(0.0, |s| s.width);
    let sp = |i: usize| b.sides[i].as_ref().map_or(0.0, |s| s.space);
    let (top, left, bottom, right) = if b.from_page {
        (sp(0), sp(1), page_h - sp(2), page_w - sp(3))
    } else {
        (text.1 - sp(0) - w(0), text.0 - sp(1) - w(1), text.3 + sp(2) + w(2), text.2 + sp(3) + w(3))
    };
    cr.save();
    for (i, side) in b.sides.iter().enumerate() {
        let Some(s) = side else { continue };
        if is_line(&s.val) {
            let bd = crate::eng::props::Border { style: s.val.clone(), size: (s.width * 8.0).round() as u32, space: 0, color: s.color };
            let lw = crate::eng::props::border_width(&Some(bd.clone()));
            let h = lw / 2.0;
            let (x0, y0, x1, y1) = match i {
                0 => (left, top + h, right, top + h),
                1 => (left + h, top, left + h, bottom),
                2 => (left, bottom - h, right, bottom - h),
                _ => (right - h, top, right - h, bottom),
            };
            crate::page::stroke_border(cr, &bd, lw, x0, y0, x1, y1);
            if s.shadow && (i == 2 || i == 3) {
                // a shadow on the bottom and right
                cr.set_source_rgb(0.4, 0.4, 0.4);
                let d = (lw * 2.0).max(3.0);
                if i == 2 {
                    cr.rectangle(left + d, bottom, right - left, d);
                } else {
                    cr.rectangle(right, top + d, d, bottom - top);
                }
                cr.fill();
            }
        } else {
            art_side(cr, s, i, left, top, right, bottom);
        }
    }
    cr.restore();
}

fn art_side(cr: &mut Cx, s: &PageBorder, side: usize, left: f64, top: f64, right: f64, bottom: f64) {
    let size = s.width;
    let (len, horiz) = match side {
        0 | 2 => (right - left, true),
        _ => (bottom - top - 2.0 * size, false),
    };
    if len <= size {
        return;
    }
    let n = (len / size).round().max(1.0) as usize;
    let step = len / n as f64;
    for k in 0..n {
        let (x, y) = match side {
            0 => (left + k as f64 * step, top),
            2 => (left + k as f64 * step, bottom - size),
            1 => (left, top + size + k as f64 * step),
            _ => (right - size, top + size + k as f64 * step),
        };
        let (cw, ch) = if horiz { (step, size) } else { (size, step) };
        motif(cr, &s.val, s.color, x, y, cw, ch, k, side);
    }
}

fn motif(cr: &mut Cx, name: &str, color: Option<u32>, x: f64, y: f64, w: f64, h: f64, k: usize, side: usize) {
    let (kind, mut pal) = kind_of(name);
    if let Some(c) = color {
        if pal.len() == 1 || name.to_lowercase().contains("black") || name.starts_with("basic") {
            pal = vec![c];
        }
    }
    let c = |i: usize| pal[i % pal.len()];
    let s = w.min(h);
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let r = s * 0.42;
    cr.save();
    match kind {
        "heart" => {
            rgb(cr, c(k));
            cr.move_to(cx, cy + r * 0.9);
            cr.curve_to(cx - r * 1.4, cy - r * 0.2, cx - r * 0.6, cy - r * 1.2, cx, cy - r * 0.45);
            cr.curve_to(cx + r * 0.6, cy - r * 1.2, cx + r * 1.4, cy - r * 0.2, cx, cy + r * 0.9);
            cr.fill();
        }
        "star" => {
            rgb(cr, c(k));
            for i in 0..10 {
                let a = std::f64::consts::PI * (i as f64 / 5.0) - std::f64::consts::FRAC_PI_2;
                let rr = if i % 2 == 0 { r } else { r * 0.42 };
                let (px, py) = (cx + rr * a.dcos(), cy + rr * a.dsin());
                if i == 0 { cr.move_to(px, py) } else { cr.line_to(px, py) }
            }
            cr.close_path();
            cr.fill();
        }
        "apple" => {
            rgb(cr, c(0));
            cr.arc(cx - r * 0.3, cy + r * 0.1, r * 0.62, 0.0, std::f64::consts::TAU);
            cr.arc(cx + r * 0.3, cy + r * 0.1, r * 0.62, 0.0, std::f64::consts::TAU);
            cr.fill();
            rgb(cr, c(1));
            cr.save();
            cr.translate(cx + r * 0.25, cy - r * 0.7);
            cr.rotate(-0.6);
            cr.scale(r * 0.35, r * 0.16);
            cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
            cr.restore();
            cr.fill();
            rgb(cr, 0x5D4037);
            cr.set_line_width(s * 0.05);
            cr.move_to(cx, cy - r * 0.4);
            cr.line_to(cx + r * 0.05, cy - r * 0.85);
            cr.stroke();
        }
        "tree" => {
            rgb(cr, c(0));
            cr.move_to(cx, cy - r);
            cr.line_to(cx + r * 0.75, cy + r * 0.55);
            cr.line_to(cx - r * 0.75, cy + r * 0.55);
            cr.close_path();
            cr.fill();
            rgb(cr, c(1));
            cr.rectangle(cx - r * 0.12, cy + r * 0.55, r * 0.24, r * 0.4);
            cr.fill();
        }
        "balloon" => {
            rgb(cr, c(k));
            cr.save();
            cr.translate(cx, cy - r * 0.2);
            cr.scale(r * 0.62, r * 0.78);
            cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
            cr.restore();
            cr.fill();
            rgb(cr, 0x616161);
            cr.set_line_width(s * 0.03);
            cr.move_to(cx, cy + r * 0.58);
            cr.curve_to(cx - r * 0.2, cy + r * 0.75, cx + r * 0.2, cy + r * 0.85, cx, cy + r);
            cr.stroke();
        }
        "flower" => {
            rgb(cr, c(k));
            for i in 0..5 {
                let a = std::f64::consts::TAU * i as f64 / 5.0;
                cr.arc(cx + r * 0.5 * a.dcos(), cy + r * 0.5 * a.dsin(), r * 0.42, 0.0, std::f64::consts::TAU);
                cr.fill();
            }
            rgb(cr, 0xFFD54F);
            cr.arc(cx, cy, r * 0.3, 0.0, std::f64::consts::TAU);
            cr.fill();
        }
        "leaf" => {
            rgb(cr, c(k));
            cr.save();
            cr.translate(cx, cy);
            cr.rotate(if k % 2 == 0 { 0.7 } else { -0.7 });
            cr.move_to(0.0, -r);
            cr.curve_to(r * 0.8, -r * 0.4, r * 0.6, r * 0.6, 0.0, r);
            cr.curve_to(-r * 0.6, r * 0.6, -r * 0.8, -r * 0.4, 0.0, -r);
            cr.fill();
            cr.restore();
        }
        "snowflake" => {
            rgb(cr, c(0));
            cr.set_line_width(s * 0.07);
            for i in 0..3 {
                let a = std::f64::consts::PI * i as f64 / 3.0;
                cr.move_to(cx - r * a.dcos(), cy - r * a.dsin());
                cr.line_to(cx + r * a.dcos(), cy + r * a.dsin());
            }
            cr.stroke();
        }
        "sun" => {
            rgb(cr, c(0));
            cr.arc(cx, cy, r * 0.5, 0.0, std::f64::consts::TAU);
            cr.fill();
            cr.set_line_width(s * 0.06);
            for i in 0..8 {
                let a = std::f64::consts::TAU * i as f64 / 8.0;
                cr.move_to(cx + r * 0.65 * a.dcos(), cy + r * 0.65 * a.dsin());
                cr.line_to(cx + r * a.dcos(), cy + r * a.dsin());
            }
            cr.stroke();
        }
        "cake" => {
            rgb(cr, c(0));
            cr.rectangle(cx - r * 0.8, cy, r * 1.6, r * 0.8);
            cr.fill();
            rgb(cr, c(1));
            cr.rectangle(cx - r * 0.08, cy - r * 0.6, r * 0.16, r * 0.6);
            cr.fill();
            rgb(cr, 0xFF9800);
            cr.arc(cx, cy - r * 0.75, r * 0.13, 0.0, std::f64::consts::TAU);
            cr.fill();
        }
        "pumpkin" => {
            rgb(cr, c(0));
            for dx in [-0.4, 0.0, 0.4] {
                cr.save();
                cr.translate(cx + r * dx, cy + r * 0.1);
                cr.scale(r * 0.45, r * 0.7);
                cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
                cr.restore();
                cr.fill();
            }
            rgb(cr, c(1));
            cr.rectangle(cx - r * 0.08, cy - r * 0.9, r * 0.16, r * 0.35);
            cr.fill();
        }
        "gift" => {
            rgb(cr, c(0));
            cr.rectangle(cx - r * 0.75, cy - r * 0.5, r * 1.5, r * 1.3);
            cr.fill();
            rgb(cr, c(1));
            cr.rectangle(cx - r * 0.12, cy - r * 0.5, r * 0.24, r * 1.3);
            cr.rectangle(cx - r * 0.75, cy - r * 0.05, r * 1.5, r * 0.22);
            cr.fill();
        }
        "confetti" => {
            for i in 0..4 {
                rgb(cr, c(i + k));
                let fx = ((k * 7 + i * 13) % 10) as f64 / 10.0;
                let fy = ((k * 3 + i * 17) % 10) as f64 / 10.0;
                cr.save();
                cr.translate(x + w * (0.15 + 0.7 * fx), y + h * (0.15 + 0.7 * fy));
                cr.rotate((k + i) as f64);
                cr.rectangle(-s * 0.08, -s * 0.03, s * 0.16, s * 0.06);
                cr.restore();
                cr.fill();
            }
        }
        "diamond" => {
            rgb(cr, c(k));
            cr.move_to(cx, cy - r);
            cr.line_to(cx + r * 0.7, cy);
            cr.line_to(cx, cy + r);
            cr.line_to(cx - r * 0.7, cy);
            cr.close_path();
            cr.fill();
        }
        "triangle" => {
            rgb(cr, c(k));
            let up = (side == 0 || side == 2) == (k % 2 == 0);
            if up {
                cr.move_to(cx, cy - r);
                cr.line_to(cx + r, cy + r);
                cr.line_to(cx - r, cy + r);
            } else {
                cr.move_to(cx - r, cy - r);
                cr.line_to(cx + r, cy - r);
                cr.line_to(cx, cy + r);
            }
            cr.close_path();
            cr.fill();
        }
        "zigzag" => {
            rgb(cr, c(0));
            cr.set_line_width(s * 0.12);
            if w >= h {
                cr.move_to(x, cy + r * 0.5);
                cr.line_to(cx, cy - r * 0.5);
                cr.line_to(x + w, cy + r * 0.5);
            } else {
                cr.move_to(cx + r * 0.5, y);
                cr.line_to(cx - r * 0.5, cy);
                cr.line_to(cx + r * 0.5, y + h);
            }
            cr.stroke();
        }
        "wave" => {
            rgb(cr, c(0));
            cr.set_line_width(s * 0.1);
            if w >= h {
                cr.move_to(x, cy);
                cr.curve_to(x + w * 0.25, cy - r, x + w * 0.75, cy + r, x + w, cy);
            } else {
                cr.move_to(cx, y);
                cr.curve_to(cx - r, y + h * 0.25, cx + r, y + h * 0.75, cx, y + h);
            }
            cr.stroke();
        }
        "checker" => {
            let q = s / 2.0;
            rgb(cr, c(0));
            cr.rectangle(x, y, q, q);
            cr.rectangle(x + q, y + q, q, q);
            cr.fill();
        }
        "braid" => {
            rgb(cr, c(0));
            cr.set_line_width(s * 0.14);
            cr.move_to(x, y);
            cr.line_to(x + w, y + h);
            cr.stroke();
            rgb(cr, c(1));
            cr.move_to(x + w, y);
            cr.line_to(x, y + h);
            cr.stroke();
        }
        "square" => {
            rgb(cr, c(k));
            cr.rectangle(cx - r * 0.7, cy - r * 0.7, r * 1.4, r * 1.4);
            cr.fill();
        }
        "lines" => {
            rgb(cr, c(0));
            if w >= h {
                cr.rectangle(x, cy - s * 0.15, w, s * 0.3);
            } else {
                cr.rectangle(cx - s * 0.15, y, s * 0.3, h);
            }
            cr.fill();
        }
        _ => {
            rgb(cr, c(0));
            cr.arc(cx, cy, r * 0.45, 0.0, std::f64::consts::TAU);
            cr.fill();
        }
    }
    cr.restore();
}

fn kind_of(name: &str) -> (&'static str, Vec<u32>) {
    let l = name.to_lowercase();
    let black = l.contains("black") || l.ends_with("bw");
    let pal = |c: &[u32]| if black { vec![0x000000] } else { c.to_vec() };
    let k: (&'static str, Vec<u32>) = if l.contains("heart") {
        ("heart", pal(&[0xD62839, 0xF07C8B]))
    } else if l.contains("star") {
        ("star", pal(&[0xF2B705, 0xE38B06, 0x3C6EB4]))
    } else if l.contains("apple") {
        ("apple", pal(&[0xC8202F, 0x3F8F2B]))
    } else if l.contains("tree") || l.contains("pine") {
        ("tree", pal(&[0x2E7D32, 0x795548]))
    } else if l.contains("balloon") {
        ("balloon", pal(&[0xE53935, 0x1E88E5, 0xFDD835]))
    } else if l.contains("flower") || l.contains("dais") || l.contains("rose") || l.contains("tulip")
        || l.contains("poinsettia") || l.contains("sunflower") || l.contains("pansy") {
        ("flower", pal(&[0xE91E63, 0xFFC107, 0x8E24AA]))
    } else if l.contains("leaf") || l.contains("maple") || l.contains("vine") || l.contains("ivy") || l.contains("holly") {
        ("leaf", pal(&[0x388E3C, 0xD84315]))
    } else if l.contains("snow") || l.contains("ice") {
        ("snowflake", pal(&[0x4FA3D9]))
    } else if l.contains("sun") {
        ("sun", pal(&[0xF9A825]))
    } else if l.contains("cake") || l.contains("candle") {
        ("cake", pal(&[0xF48FB1, 0xFFF59D]))
    } else if l.contains("pumpkin") {
        ("pumpkin", pal(&[0xEF6C00, 0x33691E]))
    } else if l.contains("gift") || l.contains("package") || l.contains("present") {
        ("gift", pal(&[0x1565C0, 0xC62828]))
    } else if l.contains("confetti") || l.contains("firework") || l.contains("party") || l.contains("champagne") {
        ("confetti", pal(&[0xE53935, 0x43A047, 0x1E88E5, 0xFDD835]))
    } else if l.contains("diamond") || l.contains("harlequin") {
        ("diamond", pal(&[0x5C6BC0, 0x26A69A]))
    } else if l.contains("triangle") || l.contains("sawtooth") || l.contains("pyramid") {
        ("triangle", pal(&[0x6D4C41, 0x9E9E9E]))
    } else if l.contains("zigzag") || l.contains("lightning") {
        ("zigzag", pal(&[0x37474F]))
    } else if l.contains("wave") || l.contains("swirl") || l.contains("ripple") {
        ("wave", pal(&[0x0277BD]))
    } else if l.contains("check") || l.contains("checkered") {
        ("checker", pal(&[0x000000]))
    } else if l.contains("weav") || l.contains("braid") || l.contains("celtic") || l.contains("tribal")
        || l.contains("twist") || l.contains("chain") || l.contains("rope") || l.contains("knot") {
        ("braid", pal(&[0x5D4037, 0xA1887F]))
    } else if l.contains("square") || l.contains("block") || l.contains("deco") || l.contains("mosaic") {
        ("square", pal(&[0x455A64, 0x90A4AE]))
    } else if l.contains("dot") || l.contains("circle") || l.contains("ring") {
        ("dot", pal(&[0x000000]))
    } else if l.contains("dash") || l.contains("line") || l.contains("outline") || l.contains("thin") || l.contains("wide") {
        ("lines", pal(&[0x000000]))
    } else {
        ("dot", pal(&[0x546E7A]))
    };
    k
}

