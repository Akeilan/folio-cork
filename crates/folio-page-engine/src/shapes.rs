//! Drawn shapes: preset geometry, fill and outline (DrawingML and VML), and
//! reading them from DrawingML and VML (no drawing).

use folio_math::Det;
use crate::xml::{NodeId, XmlDoc};
use std::collections::HashMap;

const EMU: f64 = 12700.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// preset geometry ("rect", "ellipse", "roundRect", "line"...)
    pub geom: String,
    /// fill colour and opacity
    pub fill: Option<(u32, f64)>,
    /// outline: width (points), colour, dash ("solid", "dash", "dot"...)
    pub line: Option<(f64, u32, String)>,
    /// adjust values (a:avLst gd "adj", "adj1"...) in 1/100000
    pub adj: Vec<f64>,
    /// arrowheads at the start and the end of a line
    pub ends: (bool, bool),
    /// a:custGeom paths (geom is "custom")
    pub custom: Vec<CustPath>,
}

/// One path of a custom geometry, in its own w x h coordinate space.
#[derive(Clone, Debug, PartialEq)]
pub struct CustPath {
    pub w: f64,
    pub h: f64,
    pub fill: bool,
    pub stroke: bool,
    pub cmds: Vec<Cmd>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Move(f64, f64),
    Line(f64, f64),
    Cubic(f64, f64, f64, f64, f64, f64),
    Quad(f64, f64, f64, f64),
    /// wR, hR, start angle, swing angle (degrees)
    Arc(f64, f64, f64, f64),
    Close,
}

/// The paths of an a:custGeom (guides are not evaluated: plain numbers only).
pub fn parse_custom(dom: &XmlDoc, cg: NodeId) -> Vec<CustPath> {
    let mut out = Vec::new();
    let Some(pl) = dom.child(cg, "a:pathLst") else { return out };
    let n = |e: NodeId, k: &str| dom.attr(e, k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    for p in dom.elements(pl) {
        if !dom.is(p, "a:path") {
            continue;
        }
        let mut cp = CustPath { w: n(p, "w"), h: n(p, "h"), fill: dom.attr(p, "fill") != Some("none"),
                                stroke: !matches!(dom.attr(p, "stroke"), Some("0") | Some("false")), cmds: Vec::new() };
        for c in dom.elements(p) {
            let pts: Vec<(f64, f64)> = dom.elements(c).filter(|k| dom.is(*k, "a:pt")).map(|k| (n(k, "x"), n(k, "y"))).collect();
            match dom.name(c) {
                "a:moveTo" if !pts.is_empty() => cp.cmds.push(Cmd::Move(pts[0].0, pts[0].1)),
                "a:lnTo" if !pts.is_empty() => cp.cmds.push(Cmd::Line(pts[0].0, pts[0].1)),
                "a:cubicBezTo" if pts.len() >= 3 => cp.cmds.push(Cmd::Cubic(pts[0].0, pts[0].1, pts[1].0, pts[1].1, pts[2].0, pts[2].1)),
                "a:quadBezTo" if pts.len() >= 2 => cp.cmds.push(Cmd::Quad(pts[0].0, pts[0].1, pts[1].0, pts[1].1)),
                "a:arcTo" => cp.cmds.push(Cmd::Arc(n(c, "wR"), n(c, "hR"), n(c, "stAng") / 60000.0, n(c, "swAng") / 60000.0)),
                "a:close" => cp.cmds.push(Cmd::Close),
                _ => {}
            }
        }
        out.push(cp);
    }
    out
}

/// An a:custGeom element for paths (each in its own coordinate space).
pub fn custom_xml(paths: &[CustPath]) -> String {
    let mut s = String::from("<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l=\"0\" t=\"0\" r=\"r\" b=\"b\"/><a:pathLst>");
    let pt = |x: f64, y: f64| format!("<a:pt x=\"{}\" y=\"{}\"/>", x.round() as i64, y.round() as i64);
    for p in paths {
        s.push_str(&format!("<a:path w=\"{}\" h=\"{}\"{}>", p.w.round() as i64, p.h.round() as i64,
                            if p.fill { "" } else { " fill=\"none\"" }));
        for c in &p.cmds {
            match *c {
                Cmd::Move(x, y) => s.push_str(&format!("<a:moveTo>{}</a:moveTo>", pt(x, y))),
                Cmd::Line(x, y) => s.push_str(&format!("<a:lnTo>{}</a:lnTo>", pt(x, y))),
                Cmd::Cubic(a, b, c2, d, e, f) => s.push_str(&format!("<a:cubicBezTo>{}{}{}</a:cubicBezTo>", pt(a, b), pt(c2, d), pt(e, f))),
                Cmd::Quad(a, b, c2, d) => s.push_str(&format!("<a:quadBezTo>{}{}</a:quadBezTo>", pt(a, b), pt(c2, d))),
                Cmd::Arc(wr, hr, st, sw) => s.push_str(&format!("<a:arcTo wR=\"{}\" hR=\"{}\" stAng=\"{}\" swAng=\"{}\"/>",
                    wr.round() as i64, hr.round() as i64, (st * 60000.0).round() as i64, (sw * 60000.0).round() as i64)),
                Cmd::Close => s.push_str("<a:close/>"),
            }
        }
        s.push_str("</a:path>");
    }
    s.push_str("</a:pathLst></a:custGeom>");
    s
}


fn hsl(c: u32) -> (f64, f64, f64) {
    let r = ((c >> 16) & 255) as f64 / 255.0;
    let g = ((c >> 8) & 255) as f64 / 255.0;
    let b = (c & 255) as f64 / 255.0;
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let l = (mx + mn) / 2.0;
    if mx == mn {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn from_hsl(h: f64, s: f64, l: f64) -> u32 {
    let to = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    if s == 0.0 {
        let v = to(l);
        return (v << 16) | (v << 8) | v;
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (to(f(h + 1.0 / 3.0)) << 16) | (to(f(h)) << 8) | to(f(h - 1.0 / 3.0))
}

fn preset(name: &str) -> Option<u32> {
    Some(match name {
        "black" => 0x000000,
        "white" => 0xFFFFFF,
        "red" => 0xFF0000,
        "green" => 0x008000,
        "blue" => 0x0000FF,
        "yellow" => 0xFFFF00,
        "gray" | "grey" => 0x808080,
        "darkGray" => 0xA9A9A9,
        "lightGray" => 0xD3D3D3,
        "orange" => 0xFFA500,
        "cyan" => 0x00FFFF,
        "magenta" => 0xFF00FF,
        _ => return None,
    })
}

/// The colour of a colour element (a:srgbClr, a:schemeClr, a:prstClr,
/// a:sysClr, a:scrgbClr) with its modifiers, and its opacity.
pub fn color(dom: &XmlDoc, c: NodeId, colors: &HashMap<String, u32>) -> Option<(u32, f64)> {
    let val = dom.attr(c, "val").unwrap_or("");
    let mut rgb = match dom.name(c) {
        "a:srgbClr" => u32::from_str_radix(val, 16).ok()?,
        "a:schemeClr" => {
            let key = match val {
                "tx1" | "dk1" => "dk1",
                "bg1" | "lt1" => "lt1",
                "tx2" | "dk2" => "dk2",
                "bg2" | "lt2" => "lt2",
                "phClr" => "accent1",
                v => v,
            };
            *colors.get(key).or_else(|| match key {
                "dk1" => Some(&0),
                "lt1" => Some(&0xFFFFFF),
                "accent1" => Some(&0x4472C4),
                "accent2" => Some(&0xED7D31),
                "accent3" => Some(&0xA5A5A5),
                "accent4" => Some(&0xFFC000),
                "accent5" => Some(&0x5B9BD5),
                "accent6" => Some(&0x70AD47),
                "dk2" => Some(&0x44546A),
                "lt2" => Some(&0xE7E6E6),
                _ => None,
            })?
        }
        "a:prstClr" => preset(val)?,
        "a:sysClr" => dom.attr(c, "lastClr").and_then(|v| u32::from_str_radix(v, 16).ok())
            .unwrap_or(if val == "window" { 0xFFFFFF } else { 0 }),
        "a:scrgbClr" => {
            let g = |k: &str| (dom.attr(c, k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100000.0)
                .clamp(0.0, 1.0);
            let to = |v: f64| (v.dpow(1.0 / 2.2) * 255.0).round() as u32;
            (to(g("r")) << 16) | (to(g("g")) << 8) | to(g("b"))
        }
        _ => return None,
    };
    let mut alpha = 1.0;
    for m in dom.elements(c) {
        let v = dom.attr(m, "val").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100000.0;
        match dom.name(m) {
            "a:lumMod" => {
                let (h, s, l) = hsl(rgb);
                rgb = from_hsl(h, s, l * v);
            }
            "a:lumOff" => {
                let (h, s, l) = hsl(rgb);
                rgb = from_hsl(h, s, l + v);
            }
            "a:tint" => {
                let f = |x: u32| (x as f64 + (255.0 - x as f64) * (1.0 - v)).round() as u32;
                rgb = (f((rgb >> 16) & 255) << 16) | (f((rgb >> 8) & 255) << 8) | f(rgb & 255);
            }
            "a:shade" => {
                let f = |x: u32| (x as f64 * v).round() as u32;
                rgb = (f((rgb >> 16) & 255) << 16) | (f((rgb >> 8) & 255) << 8) | f(rgb & 255);
            }
            "a:alpha" => alpha = v,
            _ => {}
        }
    }
    Some((rgb & 0xFFFFFF, alpha))
}

/// The colour inside a fill-like element (a:solidFill, a:fillRef...).
fn color_in(dom: &XmlDoc, n: NodeId, colors: &HashMap<String, u32>) -> Option<(u32, f64)> {
    dom.elements(n).find_map(|c| color(dom, c, colors))
}

/// An outline (a:ln): width, colour, dash. None for no line.
pub fn line_of(dom: &XmlDoc, ln: NodeId, colors: &HashMap<String, u32>) -> Option<(f64, u32, String)> {
    if dom.child(ln, "a:noFill").is_some() {
        return None;
    }
    let (c, _) = dom.child(ln, "a:solidFill").and_then(|f| color_in(dom, f, colors))?;
    let w = dom.attr(ln, "w").and_then(|v| v.parse::<f64>().ok()).unwrap_or(9525.0) / EMU;
    let dash = dom.child(ln, "a:prstDash").and_then(|d| dom.attr(d, "val")).unwrap_or("solid").to_string();
    Some((w, c, dash))
}

impl Shape {
    /// A wps:wsp shape.
    /// A picture's spPr (geometry and outline), as a shape.
    pub fn parse_sppr(dom: &XmlDoc, sp: NodeId, colors: &HashMap<String, u32>) -> Shape {
        Self::parse_parts(dom, Some(sp), None, colors)
    }

    pub fn parse(dom: &XmlDoc, wsp: NodeId, colors: &HashMap<String, u32>) -> Shape {
        let sp = dom.child(wsp, "wps:spPr");
        let style = dom.child(wsp, "wps:style");
        Self::parse_parts(dom, sp, style, colors)
    }

    fn parse_parts(dom: &XmlDoc, sp: Option<NodeId>, style: Option<NodeId>, colors: &HashMap<String, u32>) -> Shape {
        let mut sh = Shape { geom: "rect".into(), fill: None, line: None, adj: Vec::new(), ends: (false, false), custom: Vec::new() };
        let mut fill_set = false;
        let mut line_set = false;
        if let Some(sp) = sp {
            if let Some(g) = dom.child(sp, "a:prstGeom") {
                sh.geom = dom.attr(g, "prst").unwrap_or("rect").to_string();
                if let Some(av) = dom.child(g, "a:avLst") {
                    for gd in dom.elements(av) {
                        let f = dom.attr(gd, "fmla").unwrap_or("");
                        if let Some(v) = f.strip_prefix("val ").and_then(|v| v.trim().parse::<f64>().ok()) {
                            sh.adj.push(v);
                        }
                    }
                }
            } else if let Some(cg) = dom.child(sp, "a:custGeom") {
                sh.custom = parse_custom(dom, cg);
                sh.geom = if sh.custom.is_empty() { "rect".into() } else { "custom".into() };
            }
            if dom.child(sp, "a:noFill").is_some() {
                fill_set = true;
            } else if let Some(f) = dom.child(sp, "a:solidFill") {
                sh.fill = color_in(dom, f, colors);
                fill_set = true;
            } else if let Some(f) = dom.child(sp, "a:gradFill") {
                // a gradient: its first stop
                sh.fill = crate::pictures::find(dom, f, "a:gs").and_then(|gs| color_in(dom, gs, colors));
                fill_set = true;
            } else if dom.child(sp, "a:blipFill").is_some() || dom.child(sp, "a:pattFill").is_some() {
                sh.fill = Some((0xD9D9D9, 1.0));
                fill_set = true;
            }
            if let Some(ln) = dom.child(sp, "a:ln") {
                let arrow = |name: &str| dom.child(ln, name).and_then(|e| dom.attr(e, "type")).is_some_and(|t| t != "none");
                sh.ends = (arrow("a:headEnd"), arrow("a:tailEnd"));
                if dom.child(ln, "a:noFill").is_some() {
                    line_set = true;
                } else if dom.child(ln, "a:solidFill").is_some() {
                    sh.line = line_of(dom, ln, colors);
                    line_set = true;
                } else if let (Some(st), Some(w)) = (style, dom.attr(ln, "w")) {
                    // width here, colour from the style
                    if let Some(lr) = dom.child(st, "a:lnRef") {
                        if let Some((c, _)) = color_in(dom, lr, colors) {
                            sh.line = Some((w.parse::<f64>().unwrap_or(9525.0) / EMU, c, "solid".into()));
                            line_set = true;
                        }
                    }
                }
            }
        }
        if let Some(st) = style {
            if !fill_set {
                if let Some(fr) = dom.child(st, "a:fillRef") {
                    if dom.attr(fr, "idx").unwrap_or("0") != "0" {
                        sh.fill = color_in(dom, fr, colors);
                    }
                }
            }
            if !line_set {
                if let Some(lr) = dom.child(st, "a:lnRef") {
                    let idx: f64 = dom.attr(lr, "idx").and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    if idx > 0.0 {
                        if let Some((c, _)) = color_in(dom, lr, colors) {
                            sh.line = Some((0.5 * idx, c, "solid".into()));
                        }
                    }
                }
            }
        }
        sh
    }

    /// A VML shape element.
    pub fn parse_vml(dom: &XmlDoc, el: NodeId) -> Shape {
        let geom = match dom.name(el) {
            "v:oval" => "ellipse",
            "v:roundrect" => "roundRect",
            "v:line" => "line",
            _ => "rect",
        };
        let col = |v: &str| -> Option<u32> {
            let v = v.split_whitespace().next().unwrap_or("");
            if let Some(h) = v.strip_prefix('#') {
                if h.len() == 3 {
                    let x = u32::from_str_radix(h, 16).ok()?;
                    let (r, g, b) = ((x >> 8) & 15, (x >> 4) & 15, x & 15);
                    return Some((r * 17) << 16 | (g * 17) << 8 | b * 17);
                }
                return u32::from_str_radix(h, 16).ok();
            }
            preset(v).or(match v {
                "silver" => Some(0xC0C0C0),
                "navy" => Some(0x000080),
                "maroon" => Some(0x800000),
                "lime" => Some(0x00FF00),
                "aqua" => Some(0x00FFFF),
                "fuchsia" => Some(0xFF00FF),
                "olive" => Some(0x808000),
                "purple" => Some(0x800080),
                "teal" => Some(0x008080),
                _ => None,
            })
        };
        let filled = dom.attr(el, "filled").map(|v| !(v == "f" || v == "false")).unwrap_or(true);
        let stroked = dom.attr(el, "stroked").map(|v| !(v == "f" || v == "false")).unwrap_or(true);
        let fill_el = dom.child(el, "v:fill");
        let fill_on = fill_el.and_then(|f| dom.attr(f, "on")).map(|v| !(v == "f" || v == "false")).unwrap_or(filled);
        let fill = if fill_on && geom != "line" {
            let c = dom.attr(el, "fillcolor").and_then(col).unwrap_or(0xFFFFFF);
            let a = fill_el.and_then(|f| dom.attr(f, "opacity")).and_then(|v| {
                if let Some(x) = v.strip_suffix('f') {
                    x.parse::<f64>().ok().map(|x| x / 65536.0)
                } else {
                    v.parse().ok()
                }
            }).unwrap_or(1.0);
            Some((c, a))
        } else {
            None
        };
        let stroke_el = dom.child(el, "v:stroke");
        let stroke_on = stroke_el.and_then(|f| dom.attr(f, "on")).map(|v| !(v == "f" || v == "false")).unwrap_or(stroked);
        let line = if stroke_on {
            let c = dom.attr(el, "strokecolor").and_then(col).unwrap_or(0);
            let w = dom.attr(el, "strokeweight").and_then(crate::pictures::css_len).unwrap_or(0.75);
            let dash = stroke_el.and_then(|s| dom.attr(s, "dashstyle")).unwrap_or("solid").to_string();
            Some((w, c, dash))
        } else {
            None
        };
        Shape { geom: geom.into(), fill, line, adj: Vec::new(), ends: (false, false), custom: Vec::new() }
    }




}



