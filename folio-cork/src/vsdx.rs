//! Visio drawings (.vsdx): write every page as Visio shapes, and read drawings made in Visio.
//!
//! Cork also stores its own complete copy of the document inside the package (folio/cork.json)
//! so stickies, ink, votes and comments come back exactly. When the Visio pages have been
//! changed by another program since, Cork reads the Visio shapes instead.

use folio_kit::math::Det;
use crate::geom::{self, Seg};
use crate::model::*;
use crate::route;
use folio_kit::text::{Align, VAlign};
use folio_kit::Color;
use std::collections::HashMap;
use std::io::{Read, Write};

const NS: &str = "http://schemas.microsoft.com/office/visio/2012/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL_CORK: &str = "https://folio.studio/relationships/cork";

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\r' => {}
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

fn f(v: f32) -> String {
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s
    }
}

fn cell(n: &str, v: &str) -> String {
    format!("<Cell N='{n}' V='{}'/>", esc(v))
}

fn cellu(n: &str, v: f32, u: &str) -> String {
    format!("<Cell N='{n}' V='{}' U='{u}'/>", f(v))
}

fn color_v(c: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

fn hash(s: &[u8]) -> String {
    let mut h: u64 = 1469598103934665603;
    for b in s {
        h = (h ^ *b as u64).wrapping_mul(1099511628211);
    }
    format!("{h:016x}")
}

struct PageOut {
    xml: String,
    rels: Vec<(String, String, String)>,
}

struct Ctx<'a> {
    doc: &'a Document,
    page: &'a Page,
    ph: f32,
    rels: Vec<(String, String, String)>,
    media: &'a mut Vec<(String, Vec<u8>)>,
    media_ids: &'a mut HashMap<String, String>,
}

fn style_cells(s: &Shape, line_art: bool) -> String {
    let mut o = String::new();
    let st = &s.style;
    if st.width <= 0.0 || st.line.a == 0 {
        o += &cell("LinePattern", "0");
    } else {
        o += &cell("LineColor", &color_v(st.line));
        o += &cellu("LineWeight", st.width / 96.0, "PT");
        o += &cell(
            "LinePattern",
            match st.dash {
                Dash::Solid => "1",
                Dash::Dash => "2",
                Dash::Dot => "3",
                Dash::DashDot => "4",
            },
        );
        if st.line.a < 255 {
            o += &cell("LineColorTrans", &f(1.0 - st.line.a as f32 / 255.0));
        }
    }
    let fill = if line_art { st.line } else { st.fill };
    if fill.a == 0 {
        o += &cell("FillPattern", "0");
    } else {
        o += &cell("FillForegnd", &color_v(fill));
        o += &cell("FillPattern", "1");
        if fill.a < 255 {
            o += &cell("FillForegndTrans", &f(1.0 - fill.a as f32 / 255.0));
        }
    }
    if st.shadow {
        o += &cell("ShdwPattern", "1");
    }
    o
}

fn text_cells(s: &Shape) -> String {
    let t = &s.tstyle;
    let mut o = String::new();
    o += &cell(
        "VerticalAlign",
        match t.valign {
            VAlign::Top => "0",
            VAlign::Middle => "1",
            VAlign::Bottom => "2",
        },
    );
    let mut ch = String::from("<Section N='Character'><Row IX='0'>");
    if !t.family.is_empty() {
        ch += &cell("Font", &t.family);
    }
    ch += &cell("Color", &color_v(t.color));
    ch += &cellu("Size", t.size / 96.0, "PT");
    let style = (t.bold as u8) | (t.italic as u8) << 1 | (t.underline as u8) << 2;
    ch += &cell("Style", &style.to_string());
    ch += "</Row></Section>";
    let para = format!(
        "<Section N='Paragraph'><Row IX='0'>{}</Row></Section>",
        cell(
            "HorzAlign",
            match t.align {
                Align::Left => "0",
                Align::Center => "1",
                Align::Right => "2",
            }
        )
    );
    o + &ch + &para
}

fn geometry_sections(s: &Shape) -> String {
    let g = geom::geometry(s);
    let (w, h) = (s.w.max(0.01), s.h.max(0.01));
    let mut out = String::new();
    for (i, sub) in g.subs.iter().enumerate() {
        let mut rows = String::new();
        let mut n = 1;
        let mut start = (0.0, 0.0);
        let mut row = |t: &str, cells: &[(&str, f32)]| {
            let mut r = format!("<Row T='{t}' IX='{n}'>");
            for (k, v) in cells {
                r += &cell(k, &f(*v));
            }
            r += "</Row>";
            n += 1;
            r
        };
        for seg in &sub.segs {
            match *seg {
                Seg::M(x, y) => {
                    start = (x, y);
                    rows += &row("RelMoveTo", &[("X", x / w), ("Y", 1.0 - y / h)]);
                }
                Seg::L(x, y) => rows += &row("RelLineTo", &[("X", x / w), ("Y", 1.0 - y / h)]),
                Seg::C(a, b, c, d, x, y) => rows += &row("RelCubBezTo", &[("X", x / w), ("Y", 1.0 - y / h), ("A", a / w), ("B", 1.0 - b / h), ("C", c / w), ("D", 1.0 - d / h)]),
                Seg::Z => rows += &row("RelLineTo", &[("X", start.0 / w), ("Y", 1.0 - start.1 / h)]),
            }
        }
        out += &format!("<Section N='Geometry' IX='{i}'>{}{}{rows}</Section>", cell("NoFill", if sub.fill { "0" } else { "1" }), cell("NoLine", if sub.solid { "1" } else { "0" }));
    }
    out
}

/// A shape's link, as Visio's own Hyperlink row (K4-04): a page of the drawing goes in the
/// SubAddress, a file or an address in the Address.
fn hyperlink_section(s: &Shape) -> String {
    let Some((kind, target)) = s.link() else { return String::new() };
    let (addr, sub) = match kind {
        LinkKind::Page => (String::new(), target.to_string()),
        _ => (target.to_string(), String::new()),
    };
    format!(
        "<Section N='Hyperlink'><Row N='Link'>{}{}{}</Row></Section>",
        format!("<Cell N='Address' V='{}' U='STR'/>", esc(&addr)),
        format!("<Cell N='SubAddress' V='{}' U='STR'/>", esc(&sub)),
        cell("Default", "1")
    )
}

fn connection_section(s: &Shape) -> String {
    let g = geom::geometry(s);
    if g.points.is_empty() {
        return String::new();
    }
    let mut o = String::from("<Section N='Connection'>");
    for (i, (x, y)) in g.points.iter().enumerate() {
        o += &format!("<Row IX='{i}'>{}{}</Row>", cellu("X", x / 96.0, "IN"), cellu("Y", (s.h - y) / 96.0, "IN"));
    }
    o + "</Section>"
}

/// Cork's own fields, which are not the shape data a person puts on a shape (K4-02).
pub(crate) const OWN_KEYS: [&str; 8] = ["frame", "from", "hl", "lane", "lineart", "name", "tbox", "title"];

/// Visio wants a plain identifier for a row's name; the name as typed rides in the Label.
fn row_name(name: &str) -> String {
    let mut out: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        out.insert(0, 'P');
    }
    out
}

fn data_section(s: &Shape) -> String {
    let mut rows: Vec<String> = s
        .data
        .iter()
        // Cork's own fields ride along (so a lane is still a lane after another program has
        // touched the file), except the few that would only be noise in Visio.
        .filter(|(k, _)| !matches!(k.as_str(), "frame" | "from" | "hl" | "lineart" | "tbox" | "link" | "linkto") && !k.starts_with(PROP) && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .map(|(k, v)| format!("<Row N='{}'>{}{}</Row>", esc(k), cell("Label", k), format!("<Cell N='Value' V='{}' U='STR'/>", esc(v))))
        .collect();
    // the shape data a person put on the shape, as Visio writes it
    for (name, value) in s.props() {
        rows.push(format!(
            "<Row N='{}'>{}{}{}</Row>",
            esc(&row_name(&name)),
            cell("Label", &name),
            format!("<Cell N='Value' V='{}' U='STR'/>", esc(&value)),
            cell("Type", "0")
        ));
    }
    if rows.is_empty() {
        String::new()
    } else {
        format!("<Section N='Property'>{}</Section>", rows.join(""))
    }
}

/// Write one 2-D shape. `parent` is the group's page box (x, y, w, h) when inside a group.
fn shape_xml(cx: &mut Ctx, s: &Shape, parent: Option<(f32, f32, f32, f32)>) -> String {
    let (pin_x, pin_y) = match parent {
        Some((gx, gy, _, gh)) => ((s.cx() - gx) / 96.0, (gy + gh - s.cy()) / 96.0),
        None => (s.cx() / 96.0, cx.ph - s.cy() / 96.0),
    };
    let line_art = crate::stencils::find(&s.kind).map(|x| x.line_art).unwrap_or(false) || s.get("lineart") == "1";
    let mut cells = String::new();
    cells += &cellu("PinX", pin_x, "IN");
    cells += &cellu("PinY", pin_y, "IN");
    cells += &cellu("Width", s.w / 96.0, "IN");
    cells += &cellu("Height", s.h / 96.0, "IN");
    cells += &cellu("LocPinX", s.w / 192.0, "IN");
    cells += &cellu("LocPinY", s.h / 192.0, "IN");
    if s.rot.abs() > 0.001 {
        cells += &cellu("Angle", -s.rot.to_radians(), "DEG");
    }
    if s.flip_h {
        cells += &cell("FlipX", "1");
    }
    if s.flip_v {
        cells += &cell("FlipY", "1");
    }
    let id = s.id;
    let name = crate::stencils::find(&s.kind).map(|x| x.name.to_string()).unwrap_or_else(|| s.kind.clone());
    if s.is_group() {
        let b = (s.x, s.y, s.w, s.h);
        let kids: Vec<Shape> = cx.page.shapes.iter().filter(|c| c.group == Some(s.id)).cloned().collect();
        let mut inner = String::new();
        for k in kids {
            inner += &shape_xml(cx, &k, Some(b));
        }
        return format!("<Shape ID='{id}' NameU='Group.{id}' Name='Group' Type='Group'>{cells}<Shapes>{inner}</Shapes></Shape>");
    }
    if s.kind == "image" {
        if let Some(img) = cx.doc.images.get(&s.image) {
            let (l, t, r, b) = s.crop;
            let full_w = s.w / (r - l).max(0.01) / 96.0;
            let full_h = s.h / (b - t).max(0.01) / 96.0;
            cells += &cellu("ImgWidth", full_w, "IN");
            cells += &cellu("ImgHeight", full_h, "IN");
            cells += &cellu("ImgOffsetX", -l * full_w, "IN");
            cells += &cellu("ImgOffsetY", -(1.0 - b) * full_h, "IN");
            let jpeg = img.mime.contains("jpeg");
            let rid = match cx.media_ids.get(&s.image) {
                Some(name) => {
                    let rid = format!("rId{}", cx.rels.len() + 1);
                    cx.rels.push((rid.clone(), format!("../media/{name}"), format!("{NS_R}/image")));
                    rid
                }
                None => {
                    let name = format!("image{}.{}", cx.media.len() + 1, if jpeg { "jpeg" } else { "png" });
                    cx.media.push((name.clone(), img.bytes.to_vec()));
                    cx.media_ids.insert(s.image.clone(), name.clone());
                    let rid = format!("rId{}", cx.rels.len() + 1);
                    cx.rels.push((rid.clone(), format!("../media/{name}"), format!("{NS_R}/image")));
                    rid
                }
            };
            let line = style_cells(s, false);
            return format!(
                "<Shape ID='{id}' NameU='Picture.{id}' Name='Picture' Type='Foreign'>{cells}{line}<ForeignData ForeignType='Bitmap' CompressionType='{}'><Rel r:id='{rid}'/></ForeignData></Shape>",
                if jpeg { "JPEG" } else { "PNG" }
            );
        }
    }
    if s.kind == "ink" {
        let mut segs = vec![];
        for (i, p) in s.points.iter().enumerate() {
            segs.push(if i == 0 { Seg::M(p.0, p.1) } else { Seg::L(p.0, p.1) });
        }
        let tmp = Shape { kind: "custom".into(), path: segs_to_path(&segs, s.w.max(1.0), s.h.max(1.0), false), ..s.clone() };
        let mut st = s.clone();
        st.style.fill = Color::TRANSPARENT;
        return format!("<Shape ID='{id}' NameU='Ink.{id}' Name='Ink' Type='Shape'>{cells}{}{}</Shape>", style_cells(&st, false), geometry_sections(&tmp));
    }
    let g = geom::geometry(s);
    let (tx, ty, tw, th) = g.text;
    if (tx, ty, tw, th) != (0.0, 0.0, s.w, s.h) && !s.text.is_empty() {
        cells += &cellu("TxtPinX", (tx + tw / 2.0) / 96.0, "IN");
        cells += &cellu("TxtPinY", (s.h - ty - th / 2.0) / 96.0, "IN");
        cells += &cellu("TxtWidth", tw / 96.0, "IN");
        cells += &cellu("TxtHeight", th / 96.0, "IN");
        cells += &cellu("TxtLocPinX", tw / 192.0, "IN");
        cells += &cellu("TxtLocPinY", th / 192.0, "IN");
    }
    let mut body = String::new();
    body += &style_cells(s, line_art);
    body += &text_cells(s);
    body += &connection_section(s);
    body += &data_section(s);
    body += &hyperlink_section(s);
    body += &geometry_sections(s);
    let text = if s.kind == "swatch" && s.text.is_empty() { s.style.fill.to_hex().to_uppercase() } else { s.text.clone() };
    if !text.is_empty() {
        body += &format!("<Text>{}</Text>", esc(&text));
    }
    format!("<Shape ID='{id}' NameU='{}.{id}' Name='{}' Type='Shape'>{cells}{body}</Shape>", esc(&name), esc(&name))
}

fn segs_to_path(segs: &[Seg], w: f32, h: f32, fill: bool) -> String {
    let mut o = String::from(if fill { "F" } else { "S" });
    for s in segs {
        match *s {
            Seg::M(x, y) => o += &format!(" M{} {}", f(x / w), f(y / h)),
            Seg::L(x, y) => o += &format!(" L{} {}", f(x / w), f(y / h)),
            Seg::C(a, b, c, d, x, y) => o += &format!(" C{} {} {} {} {} {}", f(a / w), f(b / h), f(c / w), f(d / h), f(x / w), f(y / h)),
            Seg::Z => o += " Z",
        }
    }
    o
}

fn arrow_v(a: Arrow) -> &'static str {
    match a {
        Arrow::None => "0",
        Arrow::Open => "1",
        Arrow::Triangle => "4",
        Arrow::Circle => "10",
        Arrow::Diamond => "11",
        Arrow::Bar => "2",
    }
}

fn arrow_from(v: &str) -> Arrow {
    match v.trim() {
        "0" | "" => Arrow::None,
        "1" | "2" | "3" | "6" | "7" | "8" | "9" => Arrow::Open,
        "10" | "19" | "20" | "21" => Arrow::Circle,
        "11" | "22" | "23" => Arrow::Diamond,
        "24" | "25" => Arrow::Bar,
        _ => Arrow::Triangle,
    }
}

fn connector_xml(cx: &mut Ctx, s: &Shape) -> String {
    let r = route::route(cx.page, s);
    let pts = r.polyline();
    if pts.len() < 2 {
        return String::new();
    }
    let c = s.conn.as_ref().unwrap();
    let to = |p: (f32, f32)| (p.0 / 96.0, cx.ph - p.1 / 96.0);
    let b = to(pts[0]);
    let e = to(*pts.last().unwrap());
    let (dx, dy) = (e.0 - b.0, e.1 - b.1);
    let len = (dx * dx + dy * dy).sqrt().max(0.0001);
    let ang = dy.datan2(dx);
    let mut cells = String::new();
    cells += &cellu("PinX", (b.0 + e.0) / 2.0, "IN");
    cells += &cellu("PinY", (b.1 + e.1) / 2.0, "IN");
    cells += &cellu("Width", len, "IN");
    cells += &cellu("Height", 0.0, "IN");
    cells += &cellu("LocPinX", len / 2.0, "IN");
    cells += &cellu("LocPinY", 0.0, "IN");
    cells += &cellu("Angle", ang, "DEG");
    cells += &cellu("BeginX", b.0, "IN");
    cells += &cellu("BeginY", b.1, "IN");
    cells += &cellu("EndX", e.0, "IN");
    cells += &cellu("EndY", e.1, "IN");
    cells += &cell("ObjType", "2");
    cells += &cell("BeginArrow", arrow_v(c.arrow_a));
    cells += &cell("EndArrow", arrow_v(c.arrow_b));
    cells += &cell("ShapeRouteStyle", if c.route == Route::Elbow { "1" } else { "2" });
    cells += &cell("ConLineRouteExt", if c.route == Route::Curved { "2" } else { "1" });
    if c.jumps {
        cells += &cell("ConLineJumpStyle", "2");
    }
    let (ca, sa) = ((-ang).dcos(), (-ang).dsin());
    let mut rows = String::new();
    for (i, p) in pts.iter().enumerate() {
        let q = to(*p);
        let (rx, ry) = (q.0 - b.0, q.1 - b.1);
        let (lx, ly) = (rx * ca - ry * sa, rx * sa + ry * ca);
        rows += &format!("<Row T='{}' IX='{}'>{}{}</Row>", if i == 0 { "MoveTo" } else { "LineTo" }, i + 1, cellu("X", lx, "IN"), cellu("Y", ly, "IN"));
    }
    let mut st = s.clone();
    st.style.fill = Color::TRANSPARENT;
    let mut body = style_cells(&st, false);
    body += &text_cells(s);
    body += &format!("<Section N='Geometry' IX='0'>{}{rows}</Section>", cell("NoFill", "1"));
    if !s.text.is_empty() {
        let (mx, my) = r.midpoint();
        let q = to((mx, my));
        let (rx, ry) = (q.0 - b.0, q.1 - b.1);
        let longest = s.text.lines().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
        let tw = (longest * s.tstyle.size * 0.62 / 96.0 + 0.1).max(0.3);
        let th = s.text.lines().count().max(1) as f32 * s.tstyle.size * 1.25 / 96.0 + 0.05;
        body = cellu("TxtPinX", rx * ca - ry * sa, "IN")
            + &cellu("TxtPinY", rx * sa + ry * ca, "IN")
            + &cellu("TxtWidth", tw, "IN")
            + &cellu("TxtHeight", th, "IN")
            + &cellu("TxtLocPinX", tw / 2.0, "IN")
            + &cellu("TxtLocPinY", th / 2.0, "IN")
            + &cellu("TxtAngle", -ang, "DEG")
            + &body;
        body += &format!("<Text>{}</Text>", esc(&s.text));
    }
    format!("<Shape ID='{}' NameU='Dynamic connector.{}' Name='Dynamic connector' Type='Shape'>{cells}{body}</Shape>", s.id, s.id)
}

fn page_xml(doc: &Document, page: &Page, media: &mut Vec<(String, Vec<u8>)>, media_ids: &mut HashMap<String, String>) -> PageOut {
    let (_, ph) = page.paper.px();
    let ph = ph.max(render_extent(page)) / 96.0;
    let mut cx = Ctx { doc, page, ph, rels: vec![], media, media_ids };
    let mut shapes = String::new();
    for s in &page.shapes {
        if s.group.is_some() {
            continue;
        }
        if s.is_connector() {
            shapes += &connector_xml(&mut cx, s);
        } else {
            shapes += &shape_xml(&mut cx, s, None);
        }
    }
    let mut connects = String::new();
    for s in &page.shapes {
        let Some(c) = &s.conn else { continue };
        for (e, from, part) in [(&c.a, "BeginX", 9), (&c.b, "EndX", 12)] {
            if let Some(t) = e.shape {
                let (cell, tp) = match e.point {
                    Some(i) => (format!("Connections.X{}", i + 1), 100 + i),
                    None => ("PinX".to_string(), 3),
                };
                connects += &format!("<Connect FromSheet='{}' FromCell='{from}' FromPart='{part}' ToSheet='{t}' ToCell='{cell}' ToPart='{tp}'/>", s.id);
            }
        }
    }
    let connects = if connects.is_empty() { String::new() } else { format!("<Connects>{connects}</Connects>") };
    let xml = format!("<?xml version='1.0' encoding='utf-8' ?>\n<PageContents xmlns='{NS}' xmlns:r='{NS_R}' xml:space='preserve'><Shapes>{shapes}</Shapes>{connects}</PageContents>");
    PageOut { xml, rels: cx.rels }
}

/// The page must be tall enough that nothing gets a negative y in Visio's bottom-up units.
fn render_extent(page: &Page) -> f32 {
    crate::render::content_bounds(page).map(|b| b.1 + b.3 + 48.0).unwrap_or(0.0)
}

pub fn write(doc: &Document) -> Result<Vec<u8>, String> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        // The host system is written into every zip entry, and the crate takes it from whatever
        // machine is running: say it outright, so a drawing saved on Linux, Windows or a Pi is
        // the same file byte for byte (X3-12).
        let opt: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .system(zip::System::Dos);
        let mut put = |z: &mut zip::ZipWriter<&mut std::io::Cursor<Vec<u8>>>, name: &str, data: &[u8]| -> Result<(), String> {
            z.start_file(name, opt).map_err(|e| e.to_string())?;
            z.write_all(data).map_err(|e| e.to_string())
        };
        let mut media: Vec<(String, Vec<u8>)> = vec![];
        let mut media_ids: HashMap<String, String> = HashMap::new();
        let mut pages_out = vec![];
        for p in &doc.pages {
            pages_out.push(page_xml(doc, p, &mut media, &mut media_ids));
        }
        let mut ct = String::from("<?xml version='1.0' encoding='utf-8' standalone='yes'?>\n<Types xmlns='http://schemas.openxmlformats.org/package/2006/content-types'>");
        ct += "<Default Extension='rels' ContentType='application/vnd.openxmlformats-package.relationships+xml'/><Default Extension='xml' ContentType='application/xml'/>";
        ct += "<Default Extension='png' ContentType='image/png'/><Default Extension='jpeg' ContentType='image/jpeg'/><Default Extension='json' ContentType='application/json'/>";
        ct += "<Override PartName='/visio/document.xml' ContentType='application/vnd.ms-visio.drawing.main+xml'/>";
        ct += "<Override PartName='/visio/pages/pages.xml' ContentType='application/vnd.ms-visio.pages+xml'/>";
        for i in 0..doc.pages.len() {
            ct += &format!("<Override PartName='/visio/pages/page{}.xml' ContentType='application/vnd.ms-visio.page+xml'/>", i + 1);
        }
        ct += "<Override PartName='/visio/windows.xml' ContentType='application/vnd.ms-visio.windows+xml'/>";
        ct += "<Override PartName='/docProps/core.xml' ContentType='application/vnd.openxmlformats-package.core-properties+xml'/>";
        ct += "<Override PartName='/docProps/app.xml' ContentType='application/vnd.openxmlformats-officedocument.extended-properties+xml'/>";
        ct += "</Types>";
        put(&mut z, "[Content_Types].xml", ct.as_bytes())?;
        put(
            &mut z,
            "_rels/.rels",
            format!(
                "<?xml version='1.0' encoding='utf-8' standalone='yes'?>\n<Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'>\
<Relationship Id='rId1' Type='http://schemas.microsoft.com/visio/2010/relationships/document' Target='visio/document.xml'/>\
<Relationship Id='rId2' Type='http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties' Target='docProps/core.xml'/>\
<Relationship Id='rId3' Type='http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties' Target='docProps/app.xml'/>\
<Relationship Id='rId4' Type='{REL_CORK}' Target='folio/cork.json'/></Relationships>"
            )
            .as_bytes(),
        )?;
        put(&mut z, "docProps/app.xml", b"<?xml version='1.0' encoding='utf-8' standalone='yes'?>\n<Properties xmlns='http://schemas.openxmlformats.org/officeDocument/2006/extended-properties'><Application>Folio Cork</Application></Properties>")?;
        put(&mut z, "docProps/core.xml", b"<?xml version='1.0' encoding='utf-8' standalone='yes'?>\n<cp:coreProperties xmlns:cp='http://schemas.openxmlformats.org/package/2006/metadata/core-properties' xmlns:dc='http://purl.org/dc/elements/1.1/'><dc:creator></dc:creator></cp:coreProperties>")?;
        let docxml = format!(
            "<?xml version='1.0' encoding='utf-8' ?>\n<VisioDocument xmlns='{NS}' xmlns:r='{NS_R}' xml:space='preserve'><DocumentSettings TopPage='0' DefaultTextStyle='0' DefaultLineStyle='0' DefaultFillStyle='0' DefaultGuideStyle='0'/><Colors/><FaceNames/>\
<StyleSheets><StyleSheet ID='0' NameU='No Style' Name='No Style'>{}{}{}{}{}{}{}{}{}</StyleSheet></StyleSheets></VisioDocument>",
            cell("LineWeight", "0.01041666666666667"),
            cell("LineColor", "#000000"),
            cell("LinePattern", "1"),
            cell("FillForegnd", "#FFFFFF"),
            cell("FillPattern", "1"),
            cell("CharFont", "0"),
            cell("VerticalAlign", "1"),
            cell("TxtWidth", "0"),
            cell("TxtHeight", "0")
        );
        put(&mut z, "visio/document.xml", docxml.as_bytes())?;
        put(
            &mut z,
            "visio/_rels/document.xml.rels",
            b"<?xml version='1.0' encoding='utf-8' standalone='yes'?>\n<Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'>\
<Relationship Id='rId1' Type='http://schemas.microsoft.com/visio/2010/relationships/pages' Target='pages/pages.xml'/>\
<Relationship Id='rId2' Type='http://schemas.microsoft.com/visio/2010/relationships/windows' Target='windows.xml'/></Relationships>",
        )?;
        put(&mut z, "visio/windows.xml", format!("<?xml version='1.0' encoding='utf-8' ?>\n<Windows xmlns='{NS}' xmlns:r='{NS_R}' ClientWidth='1280' ClientHeight='800'><Window ID='0' WindowType='Drawing' WindowState='1073741824' ViewScale='-1' Page='0'/></Windows>").as_bytes())?;
        let mut pages = format!("<?xml version='1.0' encoding='utf-8' ?>\n<Pages xmlns='{NS}' xmlns:r='{NS_R}' xml:space='preserve'>");
        let mut prels = String::from("<?xml version='1.0' encoding='utf-8' standalone='yes'?>\n<Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'>");
        let mut hashes = vec![];
        for (i, (p, out)) in doc.pages.iter().zip(&pages_out).enumerate() {
            let (pw, ph) = p.paper.px();
            let ph = ph.max(render_extent(p));
            pages += &format!(
                "<Page ID='{i}' NameU='{}' Name='{}' ViewScale='-1'><PageSheet>{}{}{}{}{}</PageSheet><Rel r:id='rId{}'/></Page>",
                esc(&p.name),
                esc(&p.name),
                cellu("PageWidth", pw / 96.0, "IN"),
                cellu("PageHeight", ph / 96.0, "IN"),
                cell("PageScale", "1"),
                cell("DrawingScale", "1"),
                cell("PrintPageOrientation", if p.paper.landscape { "2" } else { "1" }),
                i + 1
            );
            prels += &format!("<Relationship Id='rId{}' Type='http://schemas.microsoft.com/visio/2010/relationships/page' Target='page{}.xml'/>", i + 1, i + 1);
            put(&mut z, &format!("visio/pages/page{}.xml", i + 1), out.xml.as_bytes())?;
            hashes.push(hash(out.xml.as_bytes()));
            if !out.rels.is_empty() {
                let mut r = String::from("<?xml version='1.0' encoding='utf-8' standalone='yes'?>\n<Relationships xmlns='http://schemas.openxmlformats.org/package/2006/relationships'>");
                for (id, target, ty) in &out.rels {
                    r += &format!("<Relationship Id='{id}' Type='{ty}' Target='{target}'/>");
                }
                r += "</Relationships>";
                put(&mut z, &format!("visio/pages/_rels/page{}.xml.rels", i + 1), r.as_bytes())?;
            }
        }
        pages += "</Pages>";
        prels += "</Relationships>";
        put(&mut z, "visio/pages/pages.xml", pages.as_bytes())?;
        put(&mut z, "visio/pages/_rels/pages.xml.rels", prels.as_bytes())?;
        for (name, data) in &media {
            put(&mut z, &format!("visio/media/{name}"), data)?;
        }
        let cork = serde_json::json!({ "format": 1, "pages": hashes, "document": doc });
        put(&mut z, "folio/cork.json", serde_json::to_string(&cork).map_err(|e| e.to_string())?.as_bytes())?;
        z.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf.into_inner())
}

// ------------------------------------------------------------------ reading

#[derive(Clone, Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    kids: Vec<Node>,
    text: String,
}

impl Node {
    fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }
    fn child(&self, n: &str) -> Option<&Node> {
        self.kids.iter().find(|k| k.name == n)
    }
    fn children<'a>(&'a self, n: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.kids.iter().filter(move |k| k.name == n)
    }
    /// Value of a direct <Cell N='name'>.
    fn cell(&self, n: &str) -> Option<&str> {
        self.kids.iter().find(|k| k.name == "Cell" && k.attr("N") == Some(n)).and_then(|c| c.attr("V"))
    }
    fn num(&self, n: &str) -> Option<f32> {
        self.cell(n).and_then(|v| v.trim().parse::<f32>().ok())
    }
    fn section(&self, n: &str) -> Vec<&Node> {
        self.kids.iter().filter(|k| k.name == "Section" && k.attr("N") == Some(n)).collect()
    }
}

fn local(name: &str) -> String {
    let s = name.to_string();
    match s.rfind(':') {
        Some(i) => s[i + 1..].to_string(),
        None => s,
    }
}

fn parse_xml(data: &[u8]) -> Result<Node, String> {
    use quick_xml::events::Event as X;
    let mut r = quick_xml::Reader::from_reader(data);
    r.config_mut().trim_text(false);
    let mut stack: Vec<Node> = vec![Node { name: "#root".into(), ..Default::default() }];
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf).map_err(|e| format!("Damaged drawing: {e}"))? {
            X::Start(e) => {
                let mut n = Node { name: local(e.name().as_ref()), ..Default::default() };
                for a in e.attributes().flatten() {
                    let v = a.normalized_value(quick_xml::XmlVersion::Implicit1_0).map(|x| x.to_string()).unwrap_or_default();
                    n.attrs.push((local(a.key.as_ref()), v));
                }
                stack.push(n);
            }
            X::Empty(e) => {
                let mut n = Node { name: local(e.name().as_ref()), ..Default::default() };
                for a in e.attributes().flatten() {
                    let v = a.normalized_value(quick_xml::XmlVersion::Implicit1_0).map(|x| x.to_string()).unwrap_or_default();
                    n.attrs.push((local(a.key.as_ref()), v));
                }
                stack.last_mut().unwrap().kids.push(n);
            }
            X::End(_) => {
                let n = stack.pop().unwrap();
                if let Some(p) = stack.last_mut() {
                    p.kids.push(n);
                }
            }
            X::Text(t) => {
                if let Some(n) = stack.last_mut() {
                    let raw: &str = t.as_ref();
                    n.text.push_str(raw);
                }
            }
            X::GeneralRef(e) => {
                if let Some(n) = stack.last_mut() {
                    let name: String = AsRef::<str>::as_ref(&e).to_string();
                    let s = match name.as_str() {
                        "amp" => "&".to_string(),
                        "lt" => "<".to_string(),
                        "gt" => ">".to_string(),
                        "quot" => "\"".to_string(),
                        "apos" => "'".to_string(),
                        x if x.starts_with("#x") => u32::from_str_radix(&x[2..], 16).ok().and_then(char::from_u32).map(|c| c.to_string()).unwrap_or_default(),
                        x if x.starts_with('#') => x[1..].parse::<u32>().ok().and_then(char::from_u32).map(|c| c.to_string()).unwrap_or_default(),
                        _ => String::new(),
                    };
                    n.text.push_str(&s);
                }
            }
            X::CData(t) => {
                if let Some(n) = stack.last_mut() {
                    n.text.push_str(AsRef::<str>::as_ref(&t));
                }
            }
            X::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    while stack.len() > 1 {
        let n = stack.pop().unwrap();
        stack.last_mut().unwrap().kids.push(n);
    }
    Ok(stack.pop().unwrap())
}

struct Pkg {
    files: HashMap<String, Vec<u8>>,
}

impl Pkg {
    fn open(bytes: &[u8]) -> Result<Pkg, String> {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "This isn't a Visio drawing (.vsdx), or it's damaged.".to_string())?;
        let mut files = HashMap::new();
        for i in 0..z.len() {
            let mut f = z.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() || f.size() > 200_000_000 {
                continue;
            }
            let mut v = Vec::new();
            f.read_to_end(&mut v).map_err(|e| e.to_string())?;
            let name = f.name().map(|n| n.trim_start_matches('/').to_string()).unwrap_or_default();
            files.insert(name, v);
        }
        Ok(Pkg { files })
    }
    fn xml(&self, path: &str) -> Option<Node> {
        self.files.get(path).and_then(|b| parse_xml(b).ok())
    }
    /// Relationships of a part: id -> (absolute target, type).
    fn rels(&self, part: &str) -> HashMap<String, (String, String)> {
        let (dir, file) = match part.rfind('/') {
            Some(i) => (&part[..i], &part[i + 1..]),
            None => ("", part),
        };
        let rp = if dir.is_empty() { format!("_rels/{file}.rels") } else { format!("{dir}/_rels/{file}.rels") };
        let mut out = HashMap::new();
        if let Some(root) = self.xml(&rp) {
            if let Some(rs) = root.child("Relationships") {
                for r in rs.children("Relationship") {
                    let id = r.attr("Id").unwrap_or("").to_string();
                    let target = r.attr("Target").unwrap_or("");
                    let ty = r.attr("Type").unwrap_or("").to_string();
                    out.insert(id, (resolve(dir, target), ty));
                }
            }
        }
        out
    }
}

fn resolve(dir: &str, target: &str) -> String {
    if let Some(t) = target.strip_prefix('/') {
        return t.to_string();
    }
    let mut parts: Vec<&str> = if dir.is_empty() { vec![] } else { dir.split('/').collect() };
    for seg in target.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// A 2-D affine transform in Visio inches (y up).
#[derive(Clone, Copy, Debug)]
struct Xf {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

impl Xf {
    fn id() -> Xf {
        Xf { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 }
    }
    fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }
    fn then(&self, o: &Xf) -> Xf {
        // self first, then o.
        Xf {
            a: o.a * self.a + o.c * self.b,
            b: o.b * self.a + o.d * self.b,
            c: o.a * self.c + o.c * self.d,
            d: o.b * self.c + o.d * self.d,
            e: o.a * self.e + o.c * self.f + o.e,
            f: o.b * self.e + o.d * self.f + o.f,
        }
    }
    fn angle(&self) -> f32 {
        self.b.datan2(self.a)
    }
}

struct Reader<'a> {
    pkg: &'a Pkg,
    masters: HashMap<String, Node>,
    doc: Document,
}

/// A shape node and the master shape it inherits from.
struct Sh<'a> {
    n: &'a Node,
    m: Option<&'a Node>,
}

impl Sh<'_> {
    fn cell(&self, k: &str) -> Option<String> {
        self.n.cell(k).or_else(|| self.m.and_then(|m| m.cell(k))).map(|s| s.to_string())
    }
    fn num(&self, k: &str) -> Option<f32> {
        self.n.num(k).or_else(|| self.m.and_then(|m| m.num(k)))
    }
    fn sections(&self, k: &str) -> Vec<&Node> {
        let mine = self.n.section(k);
        if !mine.is_empty() && mine.iter().any(|s| !s.kids.is_empty()) {
            return mine;
        }
        self.m.map(|m| m.section(k)).unwrap_or_default()
    }
    fn text(&self) -> Option<String> {
        self.n.child("Text").or_else(|| self.m.and_then(|m| m.child("Text"))).map(collect_text)
    }
}

fn collect_text(n: &Node) -> String {
    let mut s = n.text.clone();
    for k in &n.kids {
        s.push_str(&collect_text(k));
    }
    s.replace("\r\n", "\n").replace('\r', "\n").replace('\u{2028}', "\n").trim_end_matches('\n').to_string()
}

fn parse_color(v: &str) -> Option<Color> {
    let v = v.trim();
    if v.starts_with('#') {
        return Color::parse(v);
    }
    // Old index colours.
    let idx: usize = v.parse().ok()?;
    let table = [0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x800000, 0x008000, 0x000080, 0x808000, 0x800080, 0x008080, 0xC0C0C0, 0xE6E6E6, 0xCDCDCD, 0xB3B3B3, 0x9A9A9A, 0x808080, 0x666666, 0x4D4D4D, 0x333333, 0x1A1A1A];
    table.get(idx).map(|c| Color::hex(*c))
}

impl<'a> Reader<'a> {
    fn new(pkg: &'a Pkg) -> Reader<'a> {
        Reader { pkg, masters: HashMap::new(), doc: Document { pages: vec![], ..Default::default() } }
    }

    fn load_masters(&mut self, doc_part: &str) {
        let rels = self.pkg.rels(doc_part);
        let Some((mpath, _)) = rels.values().find(|(_, t)| t.ends_with("/masters")) else { return };
        let mrels = self.pkg.rels(mpath);
        let Some(root) = self.pkg.xml(mpath) else { return };
        let Some(ms) = root.child("Masters") else { return };
        for m in ms.children("Master") {
            let id = m.attr("ID").unwrap_or("").to_string();
            let rid = m.child("Rel").and_then(|r| r.attr("id")).unwrap_or("");
            if let Some((path, _)) = mrels.get(rid) {
                if let Some(mx) = self.pkg.xml(path) {
                    if let Some(shape) = mx.child("MasterContents").and_then(|c| c.child("Shapes")).and_then(|s| s.child("Shape")) {
                        let mut shape = shape.clone();
                        shape.attrs.push(("MasterNameU".into(), m.attr("NameU").unwrap_or("").to_string()));
                        self.masters.insert(id, shape);
                    }
                }
            }
        }
    }

    fn page(&mut self, part: &str, name: &str, pw: f32, ph: f32) -> Page {
        let id = self.doc.new_id();
        let lid = self.doc.new_id();
        let mut page = Page { id, name: name.to_string(), ..Default::default() };
        page.layers[0].id = lid;
        page.paper = nearest_paper(pw * 96.0, ph * 96.0);
        let rels = self.pkg.rels(part);
        let Some(root) = self.pkg.xml(part) else { return page };
        let Some(pc) = root.child("PageContents") else { return page };
        let mut idmap: HashMap<String, Id> = HashMap::new();
        if let Some(shapes) = pc.child("Shapes") {
            for s in shapes.children("Shape") {
                self.shape(s, None, Xf::id(), ph, &rels, &mut page, &mut idmap, None);
            }
        }
        // Glue connectors.
        if let Some(cs) = pc.child("Connects") {
            for c in cs.children("Connect") {
                let (Some(from), Some(to)) = (c.attr("FromSheet").and_then(|x| idmap.get(x)), c.attr("ToSheet").and_then(|x| idmap.get(x))) else { continue };
                let (from, to) = (*from, *to);
                let begin = c.attr("FromCell") == Some("BeginX");
                let point = c.attr("ToCell").and_then(|t| t.strip_prefix("Connections.X").or_else(|| t.strip_prefix("Connections.Row_")).and_then(|n| n.parse::<usize>().ok())).map(|n| n.saturating_sub(1));
                if let Some(cs) = page.get_mut(from) {
                    if let Some(cn) = cs.conn.as_mut() {
                        let e = End::glued(to, point);
                        if begin {
                            cn.a = e;
                        } else {
                            cn.b = e;
                        }
                    }
                }
            }
        }
        page
    }

    #[allow(clippy::too_many_arguments)]
    fn shape(&mut self, n: &Node, parent_xf: Option<Xf>, _unused: Xf, ph: f32, rels: &HashMap<String, (String, String)>, page: &mut Page, idmap: &mut HashMap<String, Id>, group: Option<Id>) {
        let m = n.attr("Master").and_then(|id| self.masters.get(id)).cloned();
        let sh = Sh { n, m: m.as_ref() };
        let w = sh.num("Width").unwrap_or(1.0);
        let h = sh.num("Height").unwrap_or(1.0);
        let pinx = sh.num("PinX").unwrap_or(0.0);
        let piny = sh.num("PinY").unwrap_or(0.0);
        let lx = sh.num("LocPinX").unwrap_or(w / 2.0);
        let ly = sh.num("LocPinY").unwrap_or(h / 2.0);
        let ang = sh.num("Angle").unwrap_or(0.0);
        let fx = sh.cell("FlipX").map(|v| v == "1").unwrap_or(false);
        let fy = sh.cell("FlipY").map(|v| v == "1").unwrap_or(false);
        // Local -> parent.
        let (ca, sa) = (ang.dcos(), ang.dsin());
        let flip = Xf { a: if fx { -1.0 } else { 1.0 }, b: 0.0, c: 0.0, d: if fy { -1.0 } else { 1.0 }, e: 0.0, f: 0.0 };
        let local = Xf { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: -lx, f: -ly }.then(&flip).then(&Xf { a: ca, b: sa, c: -sa, d: ca, e: pinx, f: piny });
        let to_page = match parent_xf {
            Some(p) => local.then(&p),
            None => local,
        };
        let ty = n.attr("Type").unwrap_or("Shape");
        let id = self.doc.new_id();
        if let Some(sid) = n.attr("ID") {
            idmap.insert(sid.to_string(), id);
        }
        let px = |p: (f32, f32)| (p.0 * 96.0, (ph - p.1) * 96.0);
        // 1-D shapes: lines and connectors.
        if sh.num("BeginX").is_some() && sh.num("EndX").is_some() && (sh.cell("ObjType").as_deref() == Some("2") || h.abs() < 1e-4 || n.attr("Type") != Some("Group")) && ty != "Group" && h.abs() < 1e-3 {
            let (bx, by, ex, ey) = (sh.num("BeginX").unwrap(), sh.num("BeginY").unwrap_or(0.0), sh.num("EndX").unwrap(), sh.num("EndY").unwrap_or(0.0));
            let (b, e) = match parent_xf {
                Some(p) => (p.apply(bx, by), p.apply(ex, ey)),
                None => ((bx, by), (ex, ey)),
            };
            let (b, e) = (px(b), px(e));
            let dynamic = m.as_ref().and_then(|m| m.attr("MasterNameU")).map(|x| x.to_lowercase().contains("connector")).unwrap_or(false) || n.attr("NameU").map(|x| x.to_lowercase().contains("connector")).unwrap_or(false);
            let curved = sh.cell("ConLineRouteExt").as_deref() == Some("2");
            let mut s = Shape { id, kind: "connector".into(), group, layer: page.layers[0].id, ..Default::default() };
            s.style = self.style(&sh, true);
            s.style.fill = Color::TRANSPARENT;
            s.tstyle = self.tstyle(&sh);
            s.text = sh.text().unwrap_or_default();
            s.conn = Some(Conn {
                a: End::free(b.0, b.1),
                b: End::free(e.0, e.1),
                route: if curved { Route::Curved } else if dynamic { Route::Elbow } else { Route::Straight },
                arrow_a: arrow_from(&sh.cell("BeginArrow").unwrap_or_default()),
                arrow_b: arrow_from(&sh.cell("EndArrow").unwrap_or_default()),
                jumps: sh.cell("ConLineJumpStyle").map(|v| v != "0" && v != "1").unwrap_or(false),
            });
            page.shapes.push(s);
            return;
        }
        // Box in page pixels.
        let centre = px(to_page.apply(lx + (w / 2.0 - lx) * if fx { -1.0 } else { 1.0 } * 0.0 + (w / 2.0 - lx), ly + (h / 2.0 - ly)));
        let _ = centre;
        let c_local = (w / 2.0, h / 2.0);
        let cp = px(to_page.apply(c_local.0, c_local.1));
        let total_ang = to_page.angle();
        let mut s = Shape { id, group, layer: page.layers[0].id, ..Default::default() };
        s.w = (w * 96.0).abs().max(0.5);
        s.h = (h * 96.0).abs().max(0.5);
        s.x = cp.0 - s.w / 2.0;
        s.y = cp.1 - s.h / 2.0;
        s.rot = (-total_ang.to_degrees()).rem_euclid(360.0);
        if s.rot > 359.95 {
            s.rot = 0.0;
        }
        s.flip_h = fx;
        s.flip_v = fy;
        s.style = self.style(&sh, false);
        s.tstyle = self.tstyle(&sh);
        s.text = sh.text().unwrap_or_default();
        // Shape data.
        for sec in sh.sections("Property") {
            for row in sec.children("Row") {
                if let (Some(k), Some(v)) = (row.attr("N"), row.cell("Value")) {
                    if v.is_empty() {
                        continue;
                    }
                    if OWN_KEYS.contains(&k) {
                        s.data.insert(k.to_string(), v.to_string());
                    } else {
                        // anything else is shape data a person put on it, under the name they gave
                        let label = row.cell("Label").filter(|l: &&str| !l.is_empty()).unwrap_or(k);
                        s.set_prop(&label, &v);
                    }
                }
            }
        }
        // Hyperlinks (K4-04).
        for sec in sh.sections("Hyperlink") {
            for row in sec.children("Row") {
                let addr = row.cell("Address").unwrap_or("").trim().to_string();
                let sub = row.cell("SubAddress").unwrap_or("").trim().to_string();
                if !addr.is_empty() {
                    let kind = link_kind_of(&addr, &[]);
                    s.set_link(kind, &addr);
                } else if !sub.is_empty() {
                    s.set_link(LinkKind::Page, &sub);
                }
                if s.link().is_some() {
                    break;
                }
            }
        }
        if ty == "Group" {
            s.kind = "group".into();
            page.shapes.push(s);
            if let Some(kids) = n.child("Shapes") {
                for k in kids.children("Shape") {
                    self.shape(k, Some(to_page), Xf::id(), ph, rels, page, idmap, Some(id));
                }
            }
            // Group text shows as its own text box.
            let gtext = page.get(id).map(|g| g.text.clone()).unwrap_or_default();
            if !gtext.is_empty() {
                let g = page.get(id).unwrap().clone();
                let tid = self.doc.new_id();
                let t = Shape { id: tid, kind: "text".into(), group: Some(id), text: gtext, style: Style { fill: Color::TRANSPARENT, line: Color::TRANSPARENT, width: 0.0, ..Default::default() }, ..g };
                page.shapes.push(t);
                if let Some(g) = page.get_mut(id) {
                    g.text.clear();
                }
            }
            return;
        }
        if ty == "Foreign" {
            let rid = n.child("ForeignData").and_then(|fd| fd.child("Rel")).and_then(|r| r.attr("id")).unwrap_or("");
            if let Some((path, _)) = rels.get(rid) {
                if let Some(bytes) = self.pkg.files.get(path) {
                    let is_img = bytes.starts_with(&[0x89, b'P']) || bytes.starts_with(&[0xFF, 0xD8]) || bytes.starts_with(b"GIF") || bytes.starts_with(b"BM");
                    if is_img {
                        s.kind = "image".into();
                        s.image = self.doc.add_image(bytes.clone());
                        let iw = sh.num("ImgWidth").unwrap_or(w).max(0.0001);
                        let ih = sh.num("ImgHeight").unwrap_or(h).max(0.0001);
                        let ox = sh.num("ImgOffsetX").unwrap_or(0.0);
                        let oy = sh.num("ImgOffsetY").unwrap_or(0.0);
                        let l = (-ox / iw).clamp(0.0, 1.0);
                        let r = ((w - ox) / iw).clamp(0.0, 1.0);
                        let b = 1.0 - (-oy / ih).clamp(0.0, 1.0);
                        let t = 1.0 - ((h - oy) / ih).clamp(0.0, 1.0);
                        s.crop = (l, t.min(b - 0.01), r.max(l + 0.01), b);
                        s.style.fill = Color::TRANSPARENT;
                        if sh.cell("LinePattern").as_deref() != Some("1") {
                            s.style.width = 0.0;
                        }
                        page.shapes.push(s);
                        return;
                    }
                }
            }
        }
        // Geometry.
        let (path, solid_only) = self.path(&sh, w, h);
        if path.is_empty() {
            s.kind = "text".into();
            s.style.fill = Color::TRANSPARENT;
            s.style.line = Color::TRANSPARENT;
            if s.text.is_empty() {
                return;
            }
        } else {
            s.kind = "custom".into();
            s.path = path;
            if solid_only {
                s.data.insert("lineart".into(), "1".into());
            }
        }
        let mut pts = vec![];
        for sec in sh.sections("Connection") {
            for row in sec.children("Row") {
                if let (Some(x), Some(y)) = (row.num("X"), row.num("Y")) {
                    pts.push(((x / w).clamp(-1.0, 2.0), (1.0 - y / h).clamp(-1.0, 2.0)));
                }
            }
        }
        s.cpts = pts;
        // Text box.
        if let (Some(tw), Some(th)) = (sh.num("TxtWidth"), sh.num("TxtHeight")) {
            if tw > 0.0 && th > 0.0 {
                let tpx = sh.num("TxtPinX").unwrap_or(w / 2.0);
                let tpy = sh.num("TxtPinY").unwrap_or(h / 2.0);
                let tlx = sh.num("TxtLocPinX").unwrap_or(tw / 2.0);
                let tly = sh.num("TxtLocPinY").unwrap_or(th / 2.0);
                let (x0, y0) = (tpx - tlx, tpy - tly);
                let tb = [x0 / w, 1.0 - (y0 + th) / h, tw / w, th / h];
                if (tb[0].abs() + tb[1].abs() + (tb[2] - 1.0).abs() + (tb[3] - 1.0).abs()) > 0.01 {
                    s.data.insert("tbox".into(), tb.iter().map(|v| f(*v)).collect::<Vec<_>>().join(","));
                }
            }
        }
        page.shapes.push(s);
    }

    fn style(&self, sh: &Sh, one_d: bool) -> Style {
        let mut st = Style::default();
        let lp = sh.cell("LinePattern").unwrap_or_else(|| "1".into());
        st.line = sh.cell("LineColor").and_then(|v| parse_color(&v)).unwrap_or(Color::BLACK);
        if let Some(t) = sh.num("LineColorTrans") {
            st.line = st.line.with_alpha(((1.0 - t) * 255.0) as u8);
        }
        st.width = sh.num("LineWeight").map(|v| v * 96.0).unwrap_or(1.0).max(0.25);
        st.dash = match lp.as_str() {
            "2" | "9" | "16" => Dash::Dash,
            "3" | "10" | "17" => Dash::Dot,
            "4" | "5" | "11" => Dash::DashDot,
            _ => Dash::Solid,
        };
        if lp == "0" {
            st.line = Color::TRANSPARENT;
            st.width = 0.0;
        }
        let fp = sh.cell("FillPattern").unwrap_or_else(|| "1".into());
        st.fill = sh.cell("FillForegnd").and_then(|v| parse_color(&v)).unwrap_or(Color::WHITE);
        if let Some(t) = sh.num("FillForegndTrans") {
            st.fill = st.fill.with_alpha(((1.0 - t) * 255.0) as u8);
        }
        if fp == "0" || one_d {
            st.fill = Color::TRANSPARENT;
        }
        st.shadow = sh.cell("ShdwPattern").map(|v| v != "0").unwrap_or(false);
        st
    }

    fn tstyle(&self, sh: &Sh) -> TStyle {
        let mut t = TStyle::default();
        let ch = sh.sections("Character");
        if let Some(row) = ch.first().and_then(|s| s.children("Row").next()) {
            if let Some(sz) = row.num("Size") {
                t.size = (sz * 96.0).clamp(4.0, 400.0);
            }
            if let Some(c) = row.cell("Color").and_then(parse_color) {
                t.color = c;
            }
            if let Some(st) = row.num("Style") {
                let st = st as u32;
                t.bold = st & 1 != 0;
                t.italic = st & 2 != 0;
                t.underline = st & 4 != 0;
            }
            if let Some(f) = row.cell("Font") {
                if f.parse::<f32>().is_err() && !f.eq_ignore_ascii_case("themed") {
                    t.family = f.to_string();
                }
            }
        } else {
            t.size = 12.0 / 72.0 * 96.0;
        }
        if let Some(row) = sh.sections("Paragraph").first().and_then(|s| s.children("Row").next()) {
            t.align = match row.cell("HorzAlign") {
                Some("0") => Align::Left,
                Some("2") => Align::Right,
                _ => Align::Center,
            };
        }
        t.valign = match sh.cell("VerticalAlign").as_deref() {
            Some("0") => VAlign::Top,
            Some("2") => VAlign::Bottom,
            _ => VAlign::Middle,
        };
        t
    }

    /// Geometry sections to Cork's path language (fractions of the box, y down).
    fn path(&self, sh: &Sh, w: f32, h: f32) -> (String, bool) {
        let w = if w.abs() < 1e-6 { 1.0 } else { w };
        let h = if h.abs() < 1e-6 { 1.0 } else { h };
        let mut out = String::new();
        let mut any_fill = false;
        for sec in sh.sections("Geometry") {
            if sec.cell("NoShow") == Some("1") {
                continue;
            }
            let nofill = sec.cell("NoFill") == Some("1");
            let mut segs = String::new();
            let mut cur = (0.0f32, 0.0f32);
            let nx = |x: f32| x / w;
            let ny = |y: f32| 1.0 - y / h;
            for row in sec.children("Row") {
                if row.attr("Del") == Some("1") {
                    continue;
                }
                let t = row.attr("T").unwrap_or("");
                let x = row.num("X").unwrap_or(0.0);
                let y = row.num("Y").unwrap_or(0.0);
                let (ax, ay) = match t {
                    "RelMoveTo" | "RelLineTo" | "RelCubBezTo" | "RelQuadBezTo" | "RelEllipticalArcTo" => (x, 1.0 - y),
                    _ => (nx(x), ny(y)),
                };
                match t {
                    "MoveTo" | "RelMoveTo" => segs += &format!(" M{} {}", f(ax), f(ay)),
                    "LineTo" | "RelLineTo" | "PolylineTo" | "NURBSTo" | "SplineKnot" | "SplineStart" => segs += &format!(" L{} {}", f(ax), f(ay)),
                    "RelCubBezTo" => {
                        let (a, b, c, d) = (row.num("A").unwrap_or(0.0), row.num("B").unwrap_or(0.0), row.num("C").unwrap_or(0.0), row.num("D").unwrap_or(0.0));
                        segs += &format!(" C{} {} {} {} {} {}", f(a), f(1.0 - b), f(c), f(1.0 - d), f(ax), f(ay));
                    }
                    "RelQuadBezTo" => {
                        let (a, b) = (row.num("A").unwrap_or(0.0), 1.0 - row.num("B").unwrap_or(0.0));
                        let (c1x, c1y) = (cur.0 + 2.0 / 3.0 * (a - cur.0), cur.1 + 2.0 / 3.0 * (b - cur.1));
                        let (c2x, c2y) = (ax + 2.0 / 3.0 * (a - ax), ay + 2.0 / 3.0 * (b - ay));
                        segs += &format!(" C{} {} {} {} {} {}", f(c1x), f(c1y), f(c2x), f(c2y), f(ax), f(ay));
                    }
                    "ArcTo" => {
                        // Bulge A: distance from the chord's middle to the arc's middle.
                        let a = row.num("A").unwrap_or(0.0);
                        let (mx, my) = ((cur.0 * w + x) / 2.0, ((1.0 - cur.1) * h + y) / 2.0);
                        let (dx, dy) = (x - cur.0 * w, y - (1.0 - cur.1) * h);
                        let l = (dx * dx + dy * dy).sqrt().max(1e-6);
                        let (px, py) = (mx - dy / l * a, my + dx / l * a);
                        segs += &quad_through(cur, (nx(px), ny(py)), (ax, ay));
                    }
                    "EllipticalArcTo" | "RelEllipticalArcTo" => {
                        let (a, b) = (row.num("A").unwrap_or(0.0), row.num("B").unwrap_or(0.0));
                        let mid = if t == "RelEllipticalArcTo" { (a, 1.0 - b) } else { (nx(a), ny(b)) };
                        segs += &quad_through(cur, mid, (ax, ay));
                    }
                    "Ellipse" => {
                        let (a, d) = (row.num("A").unwrap_or(x), row.num("D").unwrap_or(y));
                        let rx = (a - x).abs() / w;
                        let ry = (d - y).abs() / h;
                        segs += &format!(" E{} {} {} {}", f(ax), f(ay), f(rx.max(0.001)), f(ry.max(0.001)));
                    }
                    _ => {}
                }
                if t != "Ellipse" {
                    cur = (ax, ay);
                }
            }
            if segs.trim().is_empty() {
                continue;
            }
            if !nofill {
                any_fill = true;
            }
            out += if nofill { " S" } else { " F" };
            out += &segs;
        }
        (out.trim().to_string(), false && any_fill)
    }
}

/// A curve from `a` through `m` (at its middle) to `b`, as one cubic segment.
fn quad_through(a: (f32, f32), m: (f32, f32), b: (f32, f32)) -> String {
    let q = (2.0 * m.0 - (a.0 + b.0) / 2.0, 2.0 * m.1 - (a.1 + b.1) / 2.0);
    let c1 = (a.0 + 2.0 / 3.0 * (q.0 - a.0), a.1 + 2.0 / 3.0 * (q.1 - a.1));
    let c2 = (b.0 + 2.0 / 3.0 * (q.0 - b.0), b.1 + 2.0 / 3.0 * (q.1 - b.1));
    format!(" C{} {} {} {} {} {}", f(c1.0), f(c1.1), f(c2.0), f(c2.1), f(b.0), f(b.1))
}

fn nearest_paper(w: f32, h: f32) -> Paper {
    let (lw, lh) = (w.max(h), w.min(h));
    let mut best = Paper::A4;
    let mut bd = f32::MAX;
    for p in Paper::ALL {
        let mut q = p;
        q.landscape = true;
        let (pw, ph) = q.px();
        let d = (pw - lw).abs() + (ph - lh).abs();
        if d < bd {
            bd = d;
            best = q;
        }
    }
    best.landscape = w >= h;
    best
}

pub fn read(bytes: &[u8]) -> Result<Document, String> {
    let pkg = Pkg::open(bytes)?;
    let root_rels = pkg.rels("");
    let root_rels = if root_rels.is_empty() { pkg.rels("_") } else { root_rels };
    let mut root = HashMap::new();
    if let Some(r) = pkg.xml("_rels/.rels") {
        if let Some(rs) = r.child("Relationships") {
            for x in rs.children("Relationship") {
                root.insert(x.attr("Type").unwrap_or("").to_string(), resolve("", x.attr("Target").unwrap_or("")));
            }
        }
    }
    let _ = root_rels;
    let doc_part = root.iter().find(|(t, _)| t.ends_with("/document")).map(|x| x.1.clone()).unwrap_or_else(|| "visio/document.xml".into());
    let doc_rels = pkg.rels(&doc_part);
    let pages_part = doc_rels.values().find(|(_, t)| t.ends_with("/pages")).map(|x| x.0.clone()).unwrap_or_else(|| "visio/pages/pages.xml".into());
    let pages_xml = pkg.xml(&pages_part).ok_or("The drawing has no pages.")?;
    let page_rels = pkg.rels(&pages_part);
    // Cork's own copy, if the Visio pages haven't changed since Cork saved them.
    if let Some(cork_path) = root.get(REL_CORK) {
        if let Some(b) = pkg.files.get(cork_path) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(b) {
                let hashes: Vec<String> = v["pages"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                let mut current = vec![];
                if let Some(ps) = pages_xml.child("Pages") {
                    for p in ps.children("Page") {
                        let rid = p.child("Rel").and_then(|r| r.attr("id")).unwrap_or("");
                        if let Some((path, _)) = page_rels.get(rid) {
                            current.push(pkg.files.get(path).map(|b| hash(b)).unwrap_or_default());
                        }
                    }
                }
                if current == hashes {
                    if let Ok(mut d) = serde_json::from_value::<Document>(v["document"].clone()) {
                        d.fix_ids();
                        return Ok(d);
                    }
                }
            }
        }
    }
    let mut rd = Reader::new(&pkg);
    rd.load_masters(&doc_part);
    let ps = pages_xml.child("Pages").ok_or("The drawing has no pages.")?;
    for p in ps.children("Page") {
        if p.attr("Background") == Some("1") {
            continue;
        }
        let name = p.attr("Name").or(p.attr("NameU")).unwrap_or("Page").to_string();
        let sheet = p.child("PageSheet");
        let pw = sheet.and_then(|s| s.num("PageWidth")).unwrap_or(11.0);
        let ph = sheet.and_then(|s| s.num("PageHeight")).unwrap_or(8.5);
        let rid = p.child("Rel").and_then(|r| r.attr("id")).unwrap_or("");
        let Some((path, _)) = page_rels.get(rid).cloned() else { continue };
        let page = rd.page(&path, &name, pw, ph);
        rd.doc.pages.push(page);
    }
    if rd.doc.pages.is_empty() {
        return Err("The drawing has no pages Cork can show.".into());
    }
    let mut d = rd.doc;
    d.fix_ids();
    Ok(d)
}

#[allow(dead_code)]
pub fn is_vsdx(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK")
}

#[allow(dead_code)]
fn unused(_: &Seg) {}

// ---------------------------------------------------------------- Visio stencils (K4-06)
/// Read a Visio stencil (.vssx or .vssm): every master in it becomes a named set of shapes,
/// ready to go in the Shapes pane. Masters are laid out at their own size, top-left at (0, 0).
pub fn read_stencil(bytes: &[u8]) -> Result<Vec<(String, Vec<Shape>)>, String> {
    let pkg = Pkg::open(bytes).map_err(|_| "This isn't a Visio stencil (.vssx), or it's damaged.".to_string())?;
    let mut root = HashMap::new();
    if let Some(r) = pkg.xml("_rels/.rels") {
        if let Some(rs) = r.child("Relationships") {
            for x in rs.children("Relationship") {
                root.insert(x.attr("Type").unwrap_or("").to_string(), resolve("", x.attr("Target").unwrap_or("")));
            }
        }
    }
    let doc_part = root.iter().find(|(t, _)| t.ends_with("/document")).map(|x| x.1.clone()).unwrap_or_else(|| "visio/document.xml".into());
    let rels = pkg.rels(&doc_part);
    let mpath = rels
        .values()
        .find(|(_, t)| t.ends_with("/masters"))
        .map(|x| x.0.clone())
        .or_else(|| pkg.files.keys().find(|k| k.ends_with("masters.xml")).cloned())
        .ok_or("The stencil has no shapes in it.")?;
    let mrels = pkg.rels(&mpath);
    let root_xml = pkg.xml(&mpath).ok_or("The stencil has no shapes in it.")?;
    let ms = root_xml.child("Masters").ok_or("The stencil has no shapes in it.")?;
    let mut out = vec![];
    for m in ms.children("Master") {
        let name = m
            .attr("Name")
            .or(m.attr("NameU"))
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .unwrap_or_else(|| format!("Shape {}", out.len() + 1));
        let rid = m.child("Rel").and_then(|r| r.attr("id")).unwrap_or("");
        let Some((path, _)) = mrels.get(rid) else { continue };
        let Some(mx) = pkg.xml(path) else { continue };
        let Some(shapes) = mx.child("MasterContents").and_then(|c| c.child("Shapes")) else { continue };
        // the master's own box, which the shapes inside it are measured against
        let sheet = m.child("PageSheet");
        let mh = sheet
            .and_then(|s| s.num("PageHeight"))
            .or_else(|| shapes.child("Shape").and_then(|s| s.num("Height")))
            .unwrap_or(1.0);
        let mut rd = Reader::new(&pkg);
        let mut page = Page { id: 1, name: name.clone(), ..Default::default() };
        page.layers[0].id = 2;
        let mut idmap = HashMap::new();
        for s in shapes.children("Shape") {
            rd.shape(s, None, Xf::id(), mh, &mrels, &mut page, &mut idmap, None);
        }
        let mut got = page.shapes;
        if got.is_empty() {
            continue;
        }
        // bring the set to the top-left corner, so it drops where it is put
        let (mut x0, mut y0) = (f32::MAX, f32::MAX);
        for s in &got {
            let (bx, by, _, _) = geom::bounds(s);
            x0 = x0.min(bx);
            y0 = y0.min(by);
            if let Some(c) = &s.conn {
                x0 = x0.min(c.a.x).min(c.b.x);
                y0 = y0.min(c.a.y).min(c.b.y);
            }
        }
        if x0.is_finite() && y0.is_finite() {
            for s in got.iter_mut() {
                s.x -= x0;
                s.y -= y0;
                if let Some(c) = s.conn.as_mut() {
                    c.a.x -= x0;
                    c.a.y -= y0;
                    c.b.x -= x0;
                    c.b.y -= y0;
                }
            }
        }
        // the ids are renumbered when the set is dropped into a drawing; the glue between the
        // shapes of one master is kept as it is
        out.push((name, got));
    }
    if out.is_empty() {
        return Err("The stencil has no shapes Cork can read.".into());
    }
    Ok(out)
}

/// Whether a file name is a Visio stencil (K4-06).
pub fn is_stencil_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.ends_with(".vssx") || n.ends_with(".vssm") || n.ends_with(".vss")
}
