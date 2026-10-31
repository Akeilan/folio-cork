//! Cork's document: pages of shapes, connectors, layers, comments and pictures.
//! Coordinates are pixels at 96 per inch; y points down.

use folio_kit::text::{Align, VAlign};
use folio_kit::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type Id = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Dash {
    #[default]
    Solid,
    Dash,
    Dot,
    DashDot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Arrow {
    #[default]
    None,
    Triangle,
    Open,
    Circle,
    Diamond,
    Bar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Route {
    Straight,
    #[default]
    Elbow,
    Curved,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub fill: Color,
    pub line: Color,
    pub width: f32,
    pub dash: Dash,
    pub shadow: bool,
    pub opacity: f32,
}

impl Default for Style {
    fn default() -> Self {
        Style { fill: Color::hex(0xDCE8F7), line: Color::hex(0x2F5597), width: 1.5, dash: Dash::Solid, shadow: false, opacity: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TStyle {
    pub family: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Color,
    pub align: Align,
    pub valign: VAlign,
}

impl Default for TStyle {
    fn default() -> Self {
        TStyle {
            family: String::new(),
            size: 14.0,
            bold: false,
            italic: false,
            underline: false,
            color: Color::hex(0x1E1E1E),
            align: Align::Center,
            valign: VAlign::Middle,
        }
    }
}

/// One end of a connector: glued to a shape's connection point, glued to a shape's outline
/// (the connector picks the nearest side), or free at a page position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct End {
    pub shape: Option<Id>,
    pub point: Option<usize>,
    pub x: f32,
    pub y: f32,
}

impl End {
    pub fn free(x: f32, y: f32) -> End {
        End { shape: None, point: None, x, y }
    }
    pub fn glued(shape: Id, point: Option<usize>) -> End {
        End { shape: Some(shape), point, x: 0.0, y: 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Conn {
    pub a: End,
    pub b: End,
    pub route: Route,
    pub arrow_a: Arrow,
    pub arrow_b: Arrow,
    /// Draw small hops where this line crosses an earlier one.
    pub jumps: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shape {
    pub id: Id,
    /// Geometry name: a built-in ("rect", "ellipse", "sticky"...) or a stencil ("flow.decision").
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Rotation in degrees, clockwise, about the centre.
    pub rot: f32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub style: Style,
    pub text: String,
    pub tstyle: TStyle,
    pub layer: Id,
    pub locked: bool,
    /// The group this shape belongs to.
    pub group: Option<Id>,
    pub conn: Option<Conn>,
    /// Ink points relative to (x, y).
    pub points: Vec<(f32, f32)>,
    /// Picture key in [`Document::images`].
    pub image: String,
    /// Crop of the picture as fractions (left, top, right, bottom).
    pub crop: (f32, f32, f32, f32),
    pub votes: u32,
    /// Custom outline (kind "custom", e.g. from Visio files) in the stencil path language.
    pub path: String,
    /// Connection points of a custom outline, as fractions of the size.
    pub cpts: Vec<(f32, f32)>,
    /// Extra properties (org-chart fields, frame style, swatch names...).
    pub data: BTreeMap<String, String>,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            id: 0,
            kind: "rect".into(),
            x: 0.0,
            y: 0.0,
            w: 120.0,
            h: 72.0,
            rot: 0.0,
            flip_h: false,
            flip_v: false,
            style: Style::default(),
            text: String::new(),
            tstyle: TStyle::default(),
            layer: 0,
            locked: false,
            group: None,
            conn: None,
            points: vec![],
            image: String::new(),
            crop: (0.0, 0.0, 1.0, 1.0),
            votes: 0,
            path: String::new(),
            cpts: vec![],
            data: BTreeMap::new(),
        }
    }
}

impl Shape {
    pub fn is_connector(&self) -> bool {
        self.conn.is_some()
    }
    pub fn is_group(&self) -> bool {
        self.kind == "group"
    }
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }
    pub fn get(&self, k: &str) -> &str {
        self.data.get(k).map(|s| s.as_str()).unwrap_or("")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub print: bool,
}

impl Default for Layer {
    fn default() -> Self {
        Layer { id: 0, name: "Layer 1".into(), visible: true, locked: false, print: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Reply {
    pub author: String,
    pub text: String,
    pub time: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Comment {
    pub id: Id,
    /// Pin position on the page; follows the shape when attached.
    pub x: f32,
    pub y: f32,
    pub shape: Option<Id>,
    pub author: String,
    pub text: String,
    pub time: u64,
    pub replies: Vec<Reply>,
    pub resolved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    /// Width and height in millimetres, portrait.
    pub w_mm: f32,
    pub h_mm: f32,
    pub landscape: bool,
    pub name: &'static str,
}

impl Paper {
    pub const A4: Paper = Paper { w_mm: 210.0, h_mm: 297.0, landscape: true, name: "A4" };
    pub const A3: Paper = Paper { w_mm: 297.0, h_mm: 420.0, landscape: true, name: "A3" };
    pub const LETTER: Paper = Paper { w_mm: 215.9, h_mm: 279.4, landscape: true, name: "Letter" };
    pub const LEGAL: Paper = Paper { w_mm: 215.9, h_mm: 355.6, landscape: true, name: "Legal" };
    pub const TABLOID: Paper = Paper { w_mm: 279.4, h_mm: 431.8, landscape: true, name: "Tabloid" };
    pub const ALL: [Paper; 5] = [Paper::A4, Paper::A3, Paper::LETTER, Paper::LEGAL, Paper::TABLOID];

    /// Size in page pixels (96 per inch), orientation applied.
    pub fn px(&self) -> (f32, f32) {
        let w = self.w_mm / 25.4 * 96.0;
        let h = self.h_mm / 25.4 * 96.0;
        if self.landscape { (h, w) } else { (w, h) }
    }
    pub fn by_name(n: &str) -> Paper {
        Paper::ALL.iter().copied().find(|p| p.name == n).unwrap_or(Paper::A4)
    }
}

impl Default for Paper {
    fn default() -> Self {
        Paper::A4
    }
}

mod paper_serde {
    use super::Paper;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    #[derive(Serialize, Deserialize)]
    struct P {
        name: String,
        landscape: bool,
    }
    pub fn serialize<S: Serializer>(p: &Paper, s: S) -> Result<S::Ok, S::Error> {
        P { name: p.name.to_string(), landscape: p.landscape }.serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Paper, D::Error> {
        let p = P::deserialize(d)?;
        let mut paper = Paper::by_name(&p.name);
        paper.landscape = p.landscape;
        Ok(paper)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Page {
    pub id: Id,
    pub name: String,
    #[serde(with = "paper_serde")]
    pub paper: Paper,
    pub background: Color,
    pub shapes: Vec<Shape>,
    pub layers: Vec<Layer>,
    pub comments: Vec<Comment>,
    /// Mood-board pages get swatch and tidy tools in front.
    pub mood: bool,
}

impl Default for Page {
    fn default() -> Self {
        Page {
            id: 0,
            name: "Page-1".into(),
            paper: Paper::A4,
            background: Color::WHITE,
            shapes: vec![],
            layers: vec![Layer::default()],
            comments: vec![],
            mood: false,
        }
    }
}

impl Page {
    pub fn get(&self, id: Id) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }
    pub fn get_mut(&mut self, id: Id) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|s| s.id == id)
    }
    pub fn index(&self, id: Id) -> Option<usize> {
        self.shapes.iter().position(|s| s.id == id)
    }
    pub fn layer(&self, id: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id).or(self.layers.first())
    }
    pub fn visible(&self, s: &Shape) -> bool {
        self.layer(s.layer).map(|l| l.visible).unwrap_or(true)
    }
    pub fn layer_locked(&self, s: &Shape) -> bool {
        self.layer(s.layer).map(|l| l.locked).unwrap_or(false)
    }
    /// Children of a group, in drawing order.
    pub fn children(&self, gid: Id) -> Vec<Id> {
        self.shapes.iter().filter(|s| s.group == Some(gid)).map(|s| s.id).collect()
    }
    /// All shapes inside a group, recursively.
    pub fn descendants(&self, gid: Id) -> Vec<Id> {
        let mut out = vec![];
        for c in self.children(gid) {
            out.push(c);
            if self.get(c).map(|s| s.is_group()).unwrap_or(false) {
                out.extend(self.descendants(c));
            }
        }
        out
    }
    /// The outermost group containing a shape (or the shape itself).
    pub fn top_group(&self, id: Id) -> Id {
        let mut cur = id;
        while let Some(g) = self.get(cur).and_then(|s| s.group) {
            cur = g;
        }
        cur
    }
}

/// A picture stored in the document (original bytes, PNG or JPEG).
#[derive(Clone, Debug, PartialEq)]
pub struct ImageData {
    pub bytes: Arc<Vec<u8>>,
    pub mime: String,
}

impl Serialize for ImageData {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("ImageData", 2)?;
        st.serialize_field("mime", &self.mime)?;
        st.serialize_field("data", &b64_encode(&self.bytes))?;
        st.end()
    }
}

impl<'de> Deserialize<'de> for ImageData {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct R {
            mime: String,
            data: String,
        }
        let r = R::deserialize(d)?;
        Ok(ImageData { bytes: Arc::new(b64_decode(&r.data)), mime: r.mime })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Document {
    pub pages: Vec<Page>,
    pub images: BTreeMap<String, ImageData>,
    pub next_id: Id,
}

impl Default for Document {
    fn default() -> Self {
        let mut d = Document { pages: vec![], images: BTreeMap::new(), next_id: 1 };
        let mut p = Page::default();
        p.id = d.new_id();
        p.layers[0].id = d.new_id();
        d.pages.push(p);
        d
    }
}

impl Document {
    pub fn new_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    /// Make sure every id is below next_id (after loading files).
    pub fn fix_ids(&mut self) {
        let mut m = self.next_id;
        for p in &self.pages {
            m = m.max(p.id + 1);
            for s in &p.shapes {
                m = m.max(s.id + 1);
            }
            for l in &p.layers {
                m = m.max(l.id + 1);
            }
            for c in &p.comments {
                m = m.max(c.id + 1);
            }
        }
        self.next_id = m;
        for p in self.pages.iter_mut() {
            if p.layers.is_empty() {
                p.layers.push(Layer { id: m, ..Default::default() });
                m += 1;
            }
        }
        self.next_id = m;
    }
    /// Store picture bytes; returns the key.
    pub fn add_image(&mut self, bytes: Vec<u8>) -> String {
        let mime = if bytes.starts_with(&[0xFF, 0xD8]) { "image/jpeg" } else { "image/png" };
        let mut h: u64 = 1469598103934665603;
        for b in &bytes {
            h = (h ^ *b as u64).wrapping_mul(1099511628211);
        }
        let key = format!("img{h:016x}");
        self.images.entry(key.clone()).or_insert(ImageData { bytes: Arc::new(bytes), mime: mime.into() });
        key
    }
    /// Drop pictures no shape uses.
    pub fn prune_images(&mut self) {
        let used: std::collections::HashSet<String> = self.pages.iter().flat_map(|p| p.shapes.iter().map(|s| s.image.clone())).collect();
        self.images.retain(|k, _| used.contains(k));
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn b64_decode(s: &str) -> Vec<u8> {
    let mut v = [255u8; 256];
    for (i, &c) in B64.iter().enumerate() {
        v[c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0;
    for b in s.bytes() {
        let d = v[b as usize];
        if d == 255 {
            continue;
        }
        acc = acc << 6 | d as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}
