//! The editing engine: tools, selection, dragging with snapping, connector glue, text editing
//! and undo. Works in page coordinates; the window layer converts mouse positions.

use crate::geom;
use crate::model::*;
use crate::render::{self, View};
use crate::route;
use folio_kit::text::{Layout, Text};
use folio_kit::Color;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Tool {
    Pointer,
    Hand,
    Shape(String),
    Connector,
    Text,
    Sticky,
    Pen,
    Highlighter,
    Eraser,
    Frame,
    Comment,
    Vote,
    Eyedropper,
}

#[derive(Clone, Debug)]
pub enum Drag {
    None,
    Move { start: (f32, f32), orig: Vec<(Id, f32, f32, Vec<(f32, f32)>)>, moved: bool },
    Resize { handle: u8, start: (f32, f32), orig: HashMap<Id, Shape>, bbox: (f32, f32, f32, f32), keep: bool },
    Rotate { ids: Vec<Id>, center: (f32, f32), start: f32, orig: HashMap<Id, Shape> },
    Marquee { x0: f32, y0: f32, x1: f32, y1: f32, add: bool },
    Pan { sx: f32, sy: f32, ox: f32, oy: f32 },
    NewShape { kind: String, x0: f32, y0: f32, x1: f32, y1: f32 },
    Connect { from: End, to: (f32, f32), target: Option<(Id, Option<usize>)> },
    ConnEnd { id: Id, b: bool, orig: Shape },
    Ink { id: Id },
    Erase,
}

/// Alignment guides shown while dragging: (vertical?, position, from, to).
#[derive(Clone, Debug)]
pub struct Guide {
    pub vertical: bool,
    pub at: f32,
    pub from: f32,
    pub to: f32,
}

#[derive(Clone, Debug)]
pub struct TextEdit {
    pub id: Id,
    pub caret: usize,
    pub anchor: usize,
}

struct Snapshot {
    doc: Document,
    page: usize,
    sel: Vec<Id>,
    label: String,
}

pub struct Editor {
    pub doc: Document,
    pub page: usize,
    pub sel: Vec<Id>,
    pub views: HashMap<Id, View>,
    pub tool: Tool,
    /// Keep the tool after one use (double-clicked tool button).
    pub tool_lock: bool,
    pub drag: Drag,
    pub edit: Option<TextEdit>,
    pub grid: bool,
    pub snap: bool,
    pub snap_shapes: bool,
    pub grid_size: f32,
    pub guides: Vec<Guide>,
    pub hover: Option<Id>,
    pub hover_point: Option<(Id, usize)>,
    pub layer: Option<Id>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub author: String,
    pub status: String,
    /// Style for new shapes (from Format Painter or the last style picked).
    pub new_style: Option<Style>,
    pub new_tstyle: Option<TStyle>,
    pub pen_color: Color,
    pub pen_width: f32,
    pub sticky_color: Color,
    pub clip: Vec<Shape>,
    pub clip_images: Vec<(String, ImageData)>,
    pub format_brush: Option<(Style, TStyle)>,
    /// Which comment is open in the side pane.
    pub open_comment: Option<Id>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    merge_key: String,
    /// Bumped on every change so caches know to redraw.
    pub version: u64,
}

pub const HANDLE: f32 = 7.0;

impl Editor {
    pub fn new(doc: Document) -> Editor {
        Editor {
            doc,
            page: 0,
            sel: vec![],
            views: HashMap::new(),
            tool: Tool::Pointer,
            tool_lock: false,
            drag: Drag::None,
            edit: None,
            grid: true,
            snap: true,
            snap_shapes: true,
            grid_size: 12.0,
            guides: vec![],
            hover: None,
            hover_point: None,
            layer: None,
            path: None,
            dirty: false,
            author: std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "Me".into()),
            status: String::new(),
            new_style: None,
            new_tstyle: None,
            pen_color: Color::hex(0x1E1E1E),
            pen_width: 3.0,
            sticky_color: Color::hex(0xFFF176),
            clip: vec![],
            clip_images: vec![],
            format_brush: None,
            open_comment: None,
            undo: vec![],
            redo: vec![],
            merge_key: String::new(),
            version: 1,
        }
    }

    pub fn pg(&self) -> &Page {
        &self.doc.pages[self.page]
    }
    pub fn pgm(&mut self) -> &mut Page {
        &mut self.doc.pages[self.page]
    }
    pub fn view(&self) -> View {
        self.views.get(&self.pg().id).copied().unwrap_or_default()
    }
    pub fn set_view(&mut self, v: View) {
        let id = self.pg().id;
        self.views.insert(id, v);
    }
    pub fn changed(&mut self) {
        self.dirty = true;
        self.version += 1;
    }

    // ---------- Undo ----------

    /// Record the state before a change. Changes with the same `merge` key in a row share one
    /// undo step (typing, nudging).
    pub fn checkpoint(&mut self, label: &str, merge: &str) {
        if !merge.is_empty() && merge == self.merge_key {
            self.redo.clear();
            return;
        }
        self.merge_key = merge.to_string();
        self.undo.push(Snapshot { doc: self.doc.clone(), page: self.page, sel: self.sel.clone(), label: label.into() });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn break_merge(&mut self) {
        self.merge_key.clear();
    }
    fn drop_checkpoint(&mut self) {
        self.undo.pop();
        self.merge_key.clear();
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }
    pub fn undo(&mut self) {
        self.end_edit();
        if let Some(s) = self.undo.pop() {
            self.redo.push(Snapshot { doc: std::mem::replace(&mut self.doc, s.doc), page: self.page, sel: self.sel.clone(), label: s.label });
            self.page = s.page.min(self.doc.pages.len() - 1);
            self.sel = s.sel;
            self.merge_key.clear();
            self.clean_sel();
            self.changed();
        }
    }
    pub fn redo(&mut self) {
        self.end_edit();
        if let Some(s) = self.redo.pop() {
            self.undo.push(Snapshot { doc: std::mem::replace(&mut self.doc, s.doc), page: self.page, sel: self.sel.clone(), label: s.label });
            self.page = s.page.min(self.doc.pages.len() - 1);
            self.sel = s.sel;
            self.merge_key.clear();
            self.clean_sel();
            self.changed();
        }
    }
    fn clean_sel(&mut self) {
        let page = &self.doc.pages[self.page];
        self.sel.retain(|id| page.get(*id).is_some());
    }

    // ---------- Selection helpers ----------

    pub fn selected(&self) -> Vec<&Shape> {
        self.sel.iter().filter_map(|id| self.pg().get(*id)).collect()
    }
    pub fn first(&self) -> Option<&Shape> {
        self.sel.first().and_then(|id| self.pg().get(*id))
    }
    /// Selected shapes plus everything inside selected groups.
    pub fn sel_deep(&self) -> Vec<Id> {
        let mut v = vec![];
        for &id in &self.sel {
            v.push(id);
            if self.pg().get(id).map(|s| s.is_group()).unwrap_or(false) {
                v.extend(self.pg().descendants(id));
            }
        }
        v
    }
    pub fn sel_bounds(&self) -> Option<(f32, f32, f32, f32)> {
        bbox_of(self.pg(), &self.sel)
    }
    pub fn editable(&self, id: Id) -> bool {
        self.pg().get(id).map(|s| !s.locked && !self.pg().layer_locked(s)).unwrap_or(false)
    }

    /// Topmost shape under a page point. `deep` picks inside groups.
    pub fn hit(&self, p: (f32, f32), deep: bool) -> Option<Id> {
        let page = self.pg();
        let tol = 4.0 / self.view().zoom;
        for s in page.shapes.iter().rev() {
            if s.is_group() || !page.visible(s) || page.layer_locked(s) {
                continue;
            }
            let hit = if s.is_connector() {
                geom::hit_route(&route::route(page, s).polyline(), p, tol + s.style.width)
            } else {
                geom::hit_shape(s, p, tol)
            };
            if hit {
                return Some(if deep { s.id } else { page.top_group(s.id) });
            }
        }
        None
    }

    /// Nearest connection point within reach of a page point.
    pub fn hit_point(&self, p: (f32, f32), exclude: Option<Id>) -> Option<(Id, usize)> {
        let page = self.pg();
        let reach = 9.0 / self.view().zoom;
        let mut best: Option<((Id, usize), f32)> = None;
        for s in page.shapes.iter().rev() {
            if s.is_connector() || s.is_group() || Some(s.id) == exclude || !page.visible(s) || matches!(s.kind.as_str(), "frame" | "ink") {
                continue;
            }
            for (i, q) in geom::conn_points(s).into_iter().enumerate() {
                let d = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt();
                if d < reach && best.map(|b| d < b.1).unwrap_or(true) {
                    best = Some(((s.id, i), d));
                }
            }
        }
        best.map(|b| b.0)
    }

    /// Which selection handle is under the point: 0..7 resize (clockwise from top-left), 8 rotate.
    pub fn hit_handle(&self, p: (f32, f32)) -> Option<u8> {
        if self.sel.is_empty() || self.edit.is_some() {
            return None;
        }
        if self.sel.len() == 1 && self.first().map(|s| s.is_connector()).unwrap_or(false) {
            return None;
        }
        let (x, y, w, h) = self.sel_bounds()?;
        let z = self.view().zoom;
        let r = (HANDLE + 2.0) / z;
        for (i, (hx, hy)) in handle_points(x, y, w, h, z).iter().enumerate() {
            if (p.0 - hx).abs() <= r && (p.1 - hy).abs() <= r {
                return Some(i as u8);
            }
        }
        None
    }

    /// Connector end handle under the point (selected connector only).
    pub fn hit_conn_end(&self, p: (f32, f32)) -> Option<(Id, bool)> {
        if self.sel.len() != 1 {
            return None;
        }
        let s = self.first()?;
        if !s.is_connector() {
            return None;
        }
        let r = route::route(self.pg(), s);
        let reach = 8.0 / self.view().zoom;
        let a = *r.pts.first()?;
        let b = *r.pts.last()?;
        if (a.0 - p.0).abs() < reach && (a.1 - p.1).abs() < reach {
            return Some((s.id, false));
        }
        if (b.0 - p.0).abs() < reach && (b.1 - p.1).abs() < reach {
            return Some((s.id, true));
        }
        None
    }

    // ---------- Creating shapes ----------

    pub fn active_layer(&self) -> Id {
        let page = self.pg();
        self.layer.filter(|l| page.layers.iter().any(|x| x.id == *l)).unwrap_or_else(|| page.layers.first().map(|l| l.id).unwrap_or(0))
    }

    /// A new shape of a kind with defaults, not yet added.
    pub fn make_shape(&mut self, kind: &str, cx: f32, cy: f32) -> Shape {
        let id = self.doc.new_id();
        let mut s = Shape { id, kind: kind.into(), layer: self.active_layer(), ..Default::default() };
        if let Some(st) = crate::stencils::find(kind) {
            s.w = st.w;
            s.h = st.h;
            s.text = st.label.to_string();
            if st.line_art || kind.starts_with("ee.") || kind.starts_with("elec.") {
                s.style.fill = Color::WHITE;
                s.style.line = Color::hex(0x1E1E1E);
                s.style.width = 1.5;
            }
            if st.text.1 >= 1.0 || st.text.1 < 0.0 {
                s.tstyle.size = 11.0;
            }
            if kind.starts_with("floor.") {
                s.style.fill = if kind == "floor.wall" { Color::hex(0x6B6B6B) } else { Color::hex(0xF5F5F5) };
                s.style.line = Color::hex(0x404040);
                s.style.width = 1.0;
            }
            if kind == "org.person" || kind == "org.manager" {
                s.tstyle.size = 12.0;
            }
            if kind == "erd.key" {
                s.tstyle.underline = true;
            }
            if kind == "uml.class" || kind == "erd.entity" {
                s.tstyle.valign = folio_kit::text::VAlign::Top;
                s.tstyle.bold = true;
            }
        }
        match kind {
            "text" => {
                s.w = 160.0;
                s.h = 36.0;
                s.style.fill = Color::TRANSPARENT;
                s.style.line = Color::TRANSPARENT;
                s.style.width = 0.0;
                s.tstyle.align = folio_kit::text::Align::Left;
            }
            "sticky" => {
                s.w = 160.0;
                s.h = 160.0;
                s.style.fill = self.sticky_color;
                s.style.line = Color::TRANSPARENT;
                s.style.width = 0.0;
                s.tstyle.size = 20.0;
            }
            "frame" => {
                s.w = 480.0;
                s.h = 320.0;
                s.style.fill = Color::rgba(245, 245, 245, 200);
                s.style.line = Color::hex(0xB0B0B0);
                s.style.width = 1.0;
                s.text = "Frame".into();
                s.tstyle.color = Color::hex(0x555555);
            }
            "swatch" => {
                s.w = 110.0;
                s.h = 130.0;
                s.style.line = Color::hex(0xDDDDDD);
                s.style.width = 1.0;
            }
            "font" => {
                s.w = 260.0;
                s.h = 70.0;
                s.style.fill = Color::TRANSPARENT;
                s.style.line = Color::TRANSPARENT;
                s.style.width = 0.0;
                s.tstyle.size = 28.0;
                s.tstyle.align = folio_kit::text::Align::Left;
            }
            _ => {
                if let Some(st) = &self.new_style {
                    if crate::stencils::find(kind).map(|x| !x.line_art).unwrap_or(true) && !kind.starts_with("ee.") {
                        s.style = st.clone();
                    }
                }
                if let Some(t) = &self.new_tstyle {
                    s.tstyle = t.clone();
                }
            }
        }
        s.x = (cx - s.w / 2.0).round();
        s.y = (cy - s.h / 2.0).round();
        s
    }

    pub fn add_shape(&mut self, s: Shape, label: &str) -> Id {
        self.checkpoint(label, "");
        let id = s.id;
        if s.kind == "frame" {
            // Frames sit behind what they hold.
            self.pgm().shapes.insert(0, s);
        } else {
            self.pgm().shapes.push(s);
        }
        self.sel = vec![id];
        self.changed();
        id
    }

    pub fn insert_kind_at(&mut self, kind: &str, cx: f32, cy: f32) -> Id {
        let s = self.make_shape(kind, cx, cy);
        let s = self.snap_new(s);
        let name = crate::stencils::find(kind).map(|x| x.name).unwrap_or("shape");
        self.add_shape(s, &format!("Insert {}", name.to_lowercase()))
    }

    fn snap_new(&self, mut s: Shape) -> Shape {
        if self.snap && self.grid {
            let g = self.grid_size;
            s.x = (s.x / g).round() * g;
            s.y = (s.y / g).round() * g;
        }
        s
    }

    // ---------- Mouse ----------

    pub fn mouse_down(&mut self, p: (f32, f32), screen: (f32, f32), shift: bool, ctrl: bool, clicks: u32, middle: bool, space: bool) {
        self.guides.clear();
        if middle || space || self.tool == Tool::Hand {
            let v = self.view();
            self.drag = Drag::Pan { sx: screen.0, sy: screen.1, ox: v.ox, oy: v.oy };
            return;
        }
        // Clicking outside the shape being edited ends editing.
        if let Some(ed) = &self.edit {
            let id = ed.id;
            if let Some(s) = self.pg().get(id) {
                if geom::hit_shape(s, p, 2.0) && self.tool == Tool::Pointer {
                    return; // caret placement is handled by the window layer
                }
            }
            self.end_edit();
        }
        match self.tool.clone() {
            Tool::Pointer => self.pointer_down(p, shift, ctrl, clicks),
            Tool::Shape(kind) => self.drag = Drag::NewShape { kind, x0: p.0, y0: p.1, x1: p.0, y1: p.1 },
            Tool::Frame => self.drag = Drag::NewShape { kind: "frame".into(), x0: p.0, y0: p.1, x1: p.0, y1: p.1 },
            Tool::Connector => self.start_connect(p),
            Tool::Text => {
                let mut s = self.make_shape("text", p.0, p.1);
                s.x = p.0;
                s.y = p.1 - 14.0;
                let id = self.add_shape(s, "Insert text");
                self.begin_edit(id);
                self.tool_done();
            }
            Tool::Sticky => {
                let s = self.make_shape("sticky", p.0, p.1);
                let s = self.snap_new(s);
                let id = self.add_shape(s, "Insert sticky note");
                self.begin_edit(id);
                self.tool_done();
            }
            Tool::Pen | Tool::Highlighter => {
                self.checkpoint("Draw", "");
                let hl = self.tool == Tool::Highlighter;
                let id = self.doc.new_id();
                let mut s = Shape { id, kind: "ink".into(), x: p.0, y: p.1, w: 1.0, h: 1.0, layer: self.active_layer(), ..Default::default() };
                s.points = vec![(0.0, 0.0)];
                s.style.line = if hl { Color::rgba(255, 235, 59, 110) } else { self.pen_color };
                s.style.width = if hl { 16.0 } else { self.pen_width };
                s.style.fill = Color::TRANSPARENT;
                if hl {
                    s.data.insert("hl".into(), "1".into());
                }
                self.pgm().shapes.push(s);
                self.drag = Drag::Ink { id };
            }
            Tool::Eraser => {
                self.checkpoint("Erase", "");
                self.drag = Drag::Erase;
                self.erase_at(p);
            }
            Tool::Comment => {
                self.add_comment(p);
                self.tool_done();
            }
            Tool::Vote => {
                if let Some(id) = self.hit(p, false) {
                    self.checkpoint("Vote", "");
                    if let Some(s) = self.pgm().get_mut(id) {
                        if shift || ctrl {
                            s.votes = s.votes.saturating_sub(1);
                        } else {
                            s.votes += 1;
                        }
                    }
                    self.changed();
                }
            }
            Tool::Eyedropper => {
                self.pick_color(p);
                self.tool_done();
            }
            Tool::Hand => {}
        }
    }

    fn tool_done(&mut self) {
        if !self.tool_lock {
            self.tool = Tool::Pointer;
        }
    }

    fn pointer_down(&mut self, p: (f32, f32), shift: bool, ctrl: bool, clicks: u32) {
        if let Some((id, b)) = self.hit_conn_end(p) {
            if self.editable(id) {
                let orig = self.pg().get(id).cloned().unwrap();
                self.checkpoint("Move connector end", "");
                self.drag = Drag::ConnEnd { id, b, orig };
                return;
            }
        }
        if let Some(h) = self.hit_handle(p) {
            if self.sel.iter().all(|id| self.editable(*id)) {
                let deep = self.sel_deep();
                let orig: HashMap<Id, Shape> = deep.iter().filter_map(|id| self.pg().get(*id).map(|s| (*id, s.clone()))).collect();
                let bbox = self.sel_bounds().unwrap_or_default();
                self.checkpoint(if h == 8 { "Rotate" } else { "Resize" }, "");
                if h == 8 {
                    let c = (bbox.0 + bbox.2 / 2.0, bbox.1 + bbox.3 / 2.0);
                    let start = (p.1 - c.1).atan2(p.0 - c.0).to_degrees();
                    self.drag = Drag::Rotate { ids: self.sel.clone(), center: c, start, orig };
                } else {
                    self.drag = Drag::Resize { handle: h, start: p, orig, bbox, keep: false };
                }
                return;
            }
        }
        // Dragging from a connection point of a hovered shape draws a connector (Visio style).
        if let Some((sid, pi)) = self.hit_point(p, None) {
            if !self.sel.contains(&sid) || clicks == 1 && self.hit(p, false) != Some(sid) {
                self.drag = Drag::Connect { from: End::glued(sid, Some(pi)), to: p, target: None };
                return;
            }
        }
        match self.hit(p, ctrl) {
            Some(id) => {
                if clicks >= 2 {
                    let deep = self.hit(p, true).unwrap_or(id);
                    let target = if self.pg().get(id).map(|s| s.is_group()).unwrap_or(false) { deep } else { id };
                    if self.editable(target) {
                        self.sel = vec![target];
                        self.begin_edit(target);
                    }
                    return;
                }
                if shift {
                    if let Some(i) = self.sel.iter().position(|x| *x == id) {
                        self.sel.remove(i);
                    } else {
                        self.sel.push(id);
                    }
                } else if !self.sel.contains(&id) {
                    self.sel = vec![id];
                }
                if let Some((st, ts)) = self.format_brush.take() {
                    self.apply_format(id, st, ts);
                    return;
                }
                self.start_move(p);
            }
            None => {
                if !shift {
                    self.sel.clear();
                }
                self.drag = Drag::Marquee { x0: p.0, y0: p.1, x1: p.0, y1: p.1, add: shift };
            }
        }
    }

    fn start_move(&mut self, p: (f32, f32)) {
        let mut ids = self.sel_deep();
        ids.retain(|id| self.editable(*id));
        // Frames carry what lies inside them.
        let frames: Vec<Shape> = ids.iter().filter_map(|id| self.pg().get(*id)).filter(|s| s.kind == "frame").cloned().collect();
        for f in frames {
            for s in &self.pg().shapes {
                if s.id == f.id || ids.contains(&s.id) || !self.editable(s.id) {
                    continue;
                }
                let (x, y, w, h) = if s.is_connector() { continue } else { geom::bounds(s) };
                if x >= f.x && y >= f.y && x + w <= f.x + f.w && y + h <= f.y + f.h {
                    ids.push(s.id);
                    if s.is_group() {
                        ids.extend(self.pg().descendants(s.id));
                    }
                }
            }
        }
        // Free connector ends move with selected connectors.
        let orig: Vec<(Id, f32, f32, Vec<(f32, f32)>)> = ids
            .iter()
            .filter_map(|id| self.pg().get(*id))
            .map(|s| {
                let ends = s.conn.as_ref().map(|c| vec![(c.a.x, c.a.y), (c.b.x, c.b.y)]).unwrap_or_default();
                (s.id, s.x, s.y, ends)
            })
            .collect();
        if orig.is_empty() {
            return;
        }
        self.checkpoint("Move", "");
        self.drag = Drag::Move { start: p, orig, moved: false };
    }

    fn start_connect(&mut self, p: (f32, f32)) {
        let from = if let Some((sid, pi)) = self.hit_point(p, None) {
            End::glued(sid, Some(pi))
        } else if let Some(sid) = self.hit(p, true).filter(|id| !self.pg().get(*id).map(|s| s.is_connector()).unwrap_or(true)) {
            End::glued(sid, None)
        } else {
            End::free(p.0, p.1)
        };
        self.drag = Drag::Connect { from, to: p, target: None };
    }

    pub fn mouse_move(&mut self, p: (f32, f32), screen: (f32, f32), shift: bool, alt: bool) {
        match self.drag.clone() {
            Drag::None => {
                self.hover = self.hit(p, false);
                self.hover_point = if matches!(self.tool, Tool::Connector | Tool::Pointer) { self.hit_point(p, None) } else { None };
            }
            Drag::Pan { sx, sy, ox, oy } => {
                let mut v = self.view();
                v.ox = ox + screen.0 - sx;
                v.oy = oy + screen.1 - sy;
                self.set_view(v);
            }
            Drag::Marquee { x0, y0, add, .. } => self.drag = Drag::Marquee { x0, y0, x1: p.0, y1: p.1, add },
            Drag::NewShape { kind, x0, y0, .. } => {
                let (mut x1, mut y1) = p;
                if shift {
                    let d = (x1 - x0).abs().max((y1 - y0).abs());
                    x1 = x0 + d * (x1 - x0).signum();
                    y1 = y0 + d * (y1 - y0).signum();
                }
                if self.snap && self.grid && !alt {
                    let g = self.grid_size;
                    x1 = (x1 / g).round() * g;
                    y1 = (y1 / g).round() * g;
                }
                self.drag = Drag::NewShape { kind, x0, y0, x1, y1 };
            }
            Drag::Move { start, orig, .. } => self.drag_move(start, orig, p, shift, alt),
            Drag::Resize { handle, start, orig, bbox, .. } => self.drag_resize(handle, start, &orig, bbox, p, shift, alt),
            Drag::Rotate { ids, center, start, orig } => {
                let a = (p.1 - center.1).atan2(p.0 - center.0).to_degrees();
                let mut d = a - start;
                if !alt {
                    // Snap to 15 degree steps.
                    let step = if shift { 45.0 } else { 15.0 };
                    let total = orig.get(&ids[0]).map(|s| s.rot).unwrap_or(0.0) + d;
                    d = (total / step).round() * step - orig.get(&ids[0]).map(|s| s.rot).unwrap_or(0.0);
                }
                let rad = d.to_radians();
                for (id, o) in &orig {
                    if let Some(s) = self.pgm().get_mut(*id) {
                        if s.is_connector() {
                            continue;
                        }
                        let (cx, cy) = (o.cx() - center.0, o.cy() - center.1);
                        let (nx, ny) = (cx * rad.cos() - cy * rad.sin(), cx * rad.sin() + cy * rad.cos());
                        s.x = center.0 + nx - o.w / 2.0;
                        s.y = center.1 + ny - o.h / 2.0;
                        s.rot = (o.rot + d).rem_euclid(360.0);
                        if s.kind == "ink" {
                            s.rot = 0.0;
                            s.points = o.points.iter().map(|&(px, py)| {
                                let (ax, ay) = (o.x + px - center.0, o.y + py - center.1);
                                (center.0 + ax * rad.cos() - ay * rad.sin() - s.x, center.1 + ax * rad.sin() + ay * rad.cos() - s.y)
                            }).collect();
                        }
                    }
                }
                let groups: Vec<Id> = ids.clone();
                for g in groups {
                    self.update_group_bounds(g);
                }
                self.changed();
            }
            Drag::Connect { from, .. } => {
                let target = self.hit_point(p, from.shape).map(|(a, b)| (a, Some(b))).or_else(|| {
                    self.hit(p, true).filter(|id| Some(*id) != from.shape && !self.pg().get(*id).map(|s| s.is_connector() || s.is_group()).unwrap_or(true)).map(|id| (id, None))
                });
                self.drag = Drag::Connect { from, to: p, target };
            }
            Drag::ConnEnd { id, b, .. } => {
                let target = self.hit_point(p, None).map(|(a, i)| (a, Some(i))).or_else(|| {
                    self.hit(p, true).filter(|x| *x != id && !self.pg().get(*x).map(|s| s.is_connector() || s.is_group()).unwrap_or(true)).map(|x| (x, None))
                });
                if let Some(s) = self.pgm().get_mut(id) {
                    if let Some(c) = s.conn.as_mut() {
                        let e = if b { &mut c.b } else { &mut c.a };
                        *e = match target {
                            Some((sid, pi)) => End::glued(sid, pi),
                            None => End::free(p.0, p.1),
                        };
                    }
                }
                self.hover_point = target.and_then(|(a, b)| b.map(|b| (a, b)));
                self.changed();
            }
            Drag::Ink { id } => {
                if let Some(s) = self.pgm().get_mut(id) {
                    let q = (p.0 - s.x, p.1 - s.y);
                    if let Some(l) = s.points.last() {
                        if (l.0 - q.0).abs() + (l.1 - q.1).abs() < 1.0 {
                            return;
                        }
                    }
                    s.points.push(q);
                }
                self.version += 1;
            }
            Drag::Erase => self.erase_at(p),
        }
    }

    fn drag_move(&mut self, start: (f32, f32), orig: Vec<(Id, f32, f32, Vec<(f32, f32)>)>, p: (f32, f32), shift: bool, alt: bool) {
        let mut dx = p.0 - start.0;
        let mut dy = p.1 - start.1;
        if shift {
            if dx.abs() > dy.abs() {
                dy = 0.0;
            } else {
                dx = 0.0;
            }
        }
        let z = self.view().zoom;
        if !(dx.abs() * z > 2.0 || dy.abs() * z > 2.0) && !matches!(self.drag, Drag::Move { moved: true, .. }) {
            return;
        }
        // Snap the moving box to other shapes' edges and centres, then to the grid.
        self.guides.clear();
        let ids: Vec<Id> = orig.iter().map(|o| o.0).collect();
        let ob = {
            let page = self.pg();
            let mut b = None;
            for (id, ox, oy, _) in &orig {
                if let Some(s) = page.get(*id) {
                    if s.is_connector() || s.is_group() {
                        continue;
                    }
                    let (x, y, w, h) = geom::bounds(s);
                    let (x, y) = (x - s.x + ox, y - s.y + oy);
                    b = Some(match b {
                        None => (x, y, x + w, y + h),
                        Some((a, bb, c, d)) => (f32::min(a, x), f32::min(bb, y), f32::max(c, x + w), f32::max(d, y + h)),
                    });
                }
            }
            b
        };
        if let (Some((x0, y0, x1, y1)), false) = (ob, alt) {
            let (nx0, ny0, nx1, ny1) = (x0 + dx, y0 + dy, x1 + dx, y1 + dy);
            let mut snapped = (false, false);
            if self.snap_shapes {
                let tol = 6.0 / z;
                let (gx, gy) = self.snap_candidates(&ids);
                let mine_x = [nx0, (nx0 + nx1) / 2.0, nx1];
                let mine_y = [ny0, (ny0 + ny1) / 2.0, ny1];
                let mut best_x: Option<(f32, f32, (f32, f32))> = None;
                for &mx in &mine_x {
                    for &(cx, span) in &gx {
                        let d = cx - mx;
                        if d.abs() < tol && best_x.map(|b| d.abs() < b.0.abs()).unwrap_or(true) {
                            best_x = Some((d, cx, span));
                        }
                    }
                }
                let mut best_y: Option<(f32, f32, (f32, f32))> = None;
                for &my in &mine_y {
                    for &(cy, span) in &gy {
                        let d = cy - my;
                        if d.abs() < tol && best_y.map(|b| d.abs() < b.0.abs()).unwrap_or(true) {
                            best_y = Some((d, cy, span));
                        }
                    }
                }
                if let Some((d, _, _)) = best_x {
                    dx += d;
                    snapped.0 = true;
                }
                if let Some((d, _, _)) = best_y {
                    dy += d;
                    snapped.1 = true;
                }
                if let Some((_, at, span)) = best_x {
                    self.guides.push(Guide { vertical: true, at, from: span.0.min(y0 + dy), to: span.1.max(y1 + dy) });
                }
                if let Some((_, at, span)) = best_y {
                    self.guides.push(Guide { vertical: false, at, from: span.0.min(x0 + dx), to: span.1.max(x1 + dx) });
                }
            }
            if self.snap && self.grid {
                let g = self.grid_size;
                if !snapped.0 {
                    dx = ((x0 + dx) / g).round() * g - x0;
                }
                if !snapped.1 {
                    dy = ((y0 + dy) / g).round() * g - y0;
                }
            }
        }
        for (id, ox, oy, ends) in &orig {
            if let Some(s) = self.pgm().get_mut(*id) {
                s.x = ox + dx;
                s.y = oy + dy;
                if let Some(c) = s.conn.as_mut() {
                    if c.a.shape.is_none() {
                        c.a.x = ends[0].0 + dx;
                        c.a.y = ends[0].1 + dy;
                    }
                    if c.b.shape.is_none() {
                        c.b.x = ends[1].0 + dx;
                        c.b.y = ends[1].1 + dy;
                    }
                }
            }
        }
        self.drag = Drag::Move { start, orig, moved: true };
        self.changed();
    }

    /// Edge and centre lines of shapes not being moved: (position, span along the other axis).
    fn snap_candidates(&self, moving: &[Id]) -> (Vec<(f32, (f32, f32))>, Vec<(f32, (f32, f32))>) {
        let page = self.pg();
        let v = self.view();
        let (mut xs, mut ys) = (vec![], vec![]);
        for s in &page.shapes {
            if moving.contains(&s.id) || s.is_connector() || s.is_group() || !page.visible(s) || s.kind == "ink" {
                continue;
            }
            let (x, y, w, h) = geom::bounds(s);
            // Only shapes near the view count, to keep this quick on big boards.
            let (sx, sy) = v.to_screen(x, y);
            if sx > 6000.0 || sy > 6000.0 || sx + w * v.zoom < -3000.0 || sy + h * v.zoom < -3000.0 {
                continue;
            }
            for cx in [x, x + w / 2.0, x + w] {
                xs.push((cx, (y, y + h)));
            }
            for cy in [y, y + h / 2.0, y + h] {
                ys.push((cy, (x, x + w)));
            }
        }
        (xs, ys)
    }

    #[allow(clippy::too_many_arguments)]
    fn drag_resize(&mut self, handle: u8, _start: (f32, f32), orig: &HashMap<Id, Shape>, bbox: (f32, f32, f32, f32), p: (f32, f32), shift: bool, alt: bool) {
        let (bx, by, bw, bh) = bbox;
        let (mut x0, mut y0, mut x1, mut y1) = (bx, by, bx + bw, by + bh);
        let mut q = p;
        if self.snap && self.grid && !alt {
            let g = self.grid_size;
            q = ((q.0 / g).round() * g, (q.1 / g).round() * g);
        }
        // Handles: 0 TL, 1 T, 2 TR, 3 R, 4 BR, 5 B, 6 BL, 7 L.
        match handle {
            0 => {
                x0 = q.0;
                y0 = q.1
            }
            1 => y0 = q.1,
            2 => {
                x1 = q.0;
                y0 = q.1
            }
            3 => x1 = q.0,
            4 => {
                x1 = q.0;
                y1 = q.1
            }
            5 => y1 = q.1,
            6 => {
                x0 = q.0;
                y1 = q.1
            }
            7 => x0 = q.0,
            _ => {}
        }
        let single_rot = orig.len() == 1 && orig.values().next().map(|s| s.rot.abs() > 0.01).unwrap_or(false);
        let keep_aspect = shift || orig.values().any(|s| s.kind == "image" && orig.len() == 1) && !alt;
        if keep_aspect && matches!(handle, 0 | 2 | 4 | 6) && bw > 0.0 && bh > 0.0 {
            let sx = (x1 - x0) / bw;
            let sy = (y1 - y0) / bh;
            let s = if sx.abs() > sy.abs() { sx } else { sy };
            let (nw, nh) = (bw * s, bh * s);
            match handle {
                0 => {
                    x0 = x1 - nw;
                    y0 = y1 - nh
                }
                2 => {
                    x1 = x0 + nw;
                    y0 = y1 - nh
                }
                4 => {
                    x1 = x0 + nw;
                    y1 = y0 + nh
                }
                6 => {
                    x0 = x1 - nw;
                    y1 = y0 + nh
                }
                _ => {}
            }
        }
        let min = 4.0;
        if x1 - x0 < min {
            if matches!(handle, 0 | 6 | 7) { x0 = x1 - min } else { x1 = x0 + min }
        }
        if y1 - y0 < min {
            if matches!(handle, 0 | 1 | 2) { y0 = y1 - min } else { y1 = y0 + min }
        }
        let sx = (x1 - x0) / bw.max(0.001);
        let sy = (y1 - y0) / bh.max(0.001);
        for (id, o) in orig {
            if let Some(s) = self.pgm().get_mut(*id) {
                if single_rot {
                    // A rotated shape keeps its centre on the dragged side's opposite.
                    let dw = (x1 - x0) - bw;
                    let dh = (y1 - y0) - bh;
                    let ratio_w = o.w / bw.max(0.001);
                    let ratio_h = o.h / bh.max(0.001);
                    s.w = (o.w + dw * ratio_w).max(min);
                    s.h = (o.h + dh * ratio_h).max(min);
                    s.x = o.cx() + (x0 - bx + x1 - bx - bw) / 2.0 - s.w / 2.0;
                    s.y = o.cy() + (y0 - by + y1 - by - bh) / 2.0 - s.h / 2.0;
                    continue;
                }
                if let Some(c) = s.conn.as_mut() {
                    let oc = o.conn.as_ref().unwrap();
                    if c.a.shape.is_none() {
                        c.a.x = x0 + (oc.a.x - bx) * sx;
                        c.a.y = y0 + (oc.a.y - by) * sy;
                    }
                    if c.b.shape.is_none() {
                        c.b.x = x0 + (oc.b.x - bx) * sx;
                        c.b.y = y0 + (oc.b.y - by) * sy;
                    }
                    continue;
                }
                s.x = x0 + (o.x - bx) * sx;
                s.y = y0 + (o.y - by) * sy;
                s.w = (o.w * sx).max(1.0);
                s.h = (o.h * sy).max(1.0);
                if s.kind == "ink" {
                    s.points = o.points.iter().map(|&(px, py)| (px * sx, py * sy)).collect();
                }
                if orig.len() > 1 && (s.kind == "text" || s.kind == "sticky") {
                    s.tstyle.size = (o.tstyle.size * sx.min(sy)).max(4.0);
                }
            }
        }
        let groups: Vec<Id> = self.sel.clone();
        for g in groups {
            self.update_group_bounds(g);
        }
        self.drag = Drag::Resize { handle, start: _start, orig: orig.clone(), bbox, keep: keep_aspect };
        self.changed();
    }

    pub fn mouse_up(&mut self, p: (f32, f32)) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        self.guides.clear();
        match drag {
            Drag::Move { moved, orig, .. } => {
                if !moved {
                    self.drop_checkpoint();
                } else {
                    let ids: Vec<Id> = orig.iter().map(|o| o.0).collect();
                    for id in ids {
                        if self.pg().get(id).map(|s| s.is_group()).unwrap_or(false) {
                            self.update_group_bounds(id);
                        }
                    }
                    self.break_merge();
                }
            }
            Drag::Resize { .. } | Drag::Rotate { .. } | Drag::ConnEnd { .. } | Drag::Erase => self.break_merge(),
            Drag::Marquee { x0, y0, x1, y1, add } => {
                let (ax, ay, bx, by) = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
                if bx - ax > 1.0 || by - ay > 1.0 {
                    let page = self.pg();
                    let mut found: Vec<Id> = vec![];
                    for s in &page.shapes {
                        if s.group.is_some() || !page.visible(s) || page.layer_locked(s) {
                            continue;
                        }
                        let (x, y, w, h) = if s.is_connector() {
                            let pl = route::route(page, s).polyline();
                            match bbox_pts(&pl) {
                                Some(b) => b,
                                None => continue,
                            }
                        } else {
                            geom::bounds(s)
                        };
                        if x >= ax && y >= ay && x + w <= bx && y + h <= by {
                            found.push(s.id);
                        }
                    }
                    if add {
                        for f in found {
                            if !self.sel.contains(&f) {
                                self.sel.push(f);
                            }
                        }
                    } else {
                        self.sel = found;
                    }
                }
            }
            Drag::NewShape { kind, x0, y0, x1, y1 } => {
                let (w, h) = ((x1 - x0).abs(), (y1 - y0).abs());
                let z = self.view().zoom;
                let mut s = self.make_shape(&kind, x0, y0);
                if w * z > 6.0 && h * z > 6.0 {
                    s.x = x0.min(x1);
                    s.y = y0.min(y1);
                    s.w = w;
                    s.h = h;
                } else {
                    s.x = x0 - s.w / 2.0;
                    s.y = y0 - s.h / 2.0;
                    s = self.snap_new(s);
                }
                let name = if kind == "frame" { "frame".to_string() } else { crate::stencils::find(&kind).map(|x| x.name.to_lowercase()).unwrap_or("shape".into()) };
                let frame = kind == "frame";
                let id = self.add_shape(s, &format!("Insert {name}"));
                if frame {
                    self.begin_edit(id);
                }
                self.tool_done();
            }
            Drag::Connect { from, to, target } => {
                let dist = match from.shape.and_then(|id| self.pg().get(id)) {
                    Some(s) => ((s.cx() - to.0).powi(2) + (s.cy() - to.1).powi(2)).sqrt(),
                    None => ((from.x - to.0).powi(2) + (from.y - to.1).powi(2)).sqrt(),
                };
                if dist * self.view().zoom < 8.0 && target.is_none() {
                    self.tool_done();
                    return;
                }
                let b = match target {
                    Some((sid, pi)) => End::glued(sid, pi),
                    None => End::free(to.0, to.1),
                };
                self.add_connector(from, b);
                self.tool_done();
            }
            Drag::Ink { id } => {
                // Normalise: (x, y, w, h) becomes the stroke's box.
                if let Some(s) = self.pgm().get_mut(id) {
                    if s.points.len() < 2 {
                        let p0 = s.points[0];
                        s.points.push((p0.0 + 0.5, p0.1 + 0.5));
                    }
                    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
                    for q in &s.points {
                        x0 = x0.min(q.0);
                        y0 = y0.min(q.1);
                        x1 = x1.max(q.0);
                        y1 = y1.max(q.1);
                    }
                    for q in s.points.iter_mut() {
                        q.0 -= x0;
                        q.1 -= y0;
                    }
                    s.x += x0;
                    s.y += y0;
                    s.w = (x1 - x0).max(1.0);
                    s.h = (y1 - y0).max(1.0);
                }
                self.break_merge();
                self.changed();
            }
            Drag::Pan { .. } | Drag::None => {}
        }
        let _ = p;
    }

    pub fn add_connector(&mut self, a: End, b: End) -> Id {
        let id = self.doc.new_id();
        let mut s = Shape { id, kind: "connector".into(), layer: self.active_layer(), ..Default::default() };
        s.style.line = Color::hex(0x404040);
        s.style.width = 1.5;
        s.style.fill = Color::TRANSPARENT;
        s.tstyle.size = 12.0;
        s.conn = Some(Conn { a, b, route: Route::Elbow, arrow_a: Arrow::None, arrow_b: Arrow::Triangle, jumps: false });
        self.add_shape(s, "Add connector")
    }

    fn erase_at(&mut self, p: (f32, f32)) {
        let tol = 6.0 / self.view().zoom;
        let page = self.pg();
        let hit: Vec<Id> = page.shapes.iter().filter(|s| s.kind == "ink" && geom::hit_shape(s, p, tol)).map(|s| s.id).collect();
        if !hit.is_empty() {
            self.pgm().shapes.retain(|s| !hit.contains(&s.id));
            self.changed();
        }
    }

    fn pick_color(&mut self, p: (f32, f32)) {
        let Some(id) = self.hit(p, true) else { return };
        let Some(s) = self.pg().get(id).cloned() else { return };
        let color = if s.kind == "image" {
            let Some(img) = self.doc.images.get(&s.image).and_then(|d| folio_kit::gfx::decode_image(&d.bytes)) else { return };
            let inv = geom::shape_xf(&s).invert().unwrap_or_default();
            let (lx, ly) = geom::map_pt(&inv, p.0, p.1);
            let (l, t, r, b) = s.crop;
            let u = l + (lx / s.w).clamp(0.0, 0.999) * (r - l);
            let v = t + (ly / s.h).clamp(0.0, 0.999) * (b - t);
            let (px, py) = ((u * img.width() as f32) as u32, (v * img.height() as f32) as u32);
            let i = ((py * img.width() + px) * 4) as usize;
            let d = img.data();
            let a = d[i + 3].max(1) as u32;
            Color::rgb((d[i] as u32 * 255 / a) as u8, (d[i + 1] as u32 * 255 / a) as u8, (d[i + 2] as u32 * 255 / a) as u8)
        } else {
            s.style.fill
        };
        let mut sw = self.make_shape("swatch", 0.0, 0.0);
        let (bx, by, bw, _) = geom::bounds(&s);
        let n = self.pg().shapes.iter().filter(|x| x.kind == "swatch" && x.get("from") == s.id.to_string()).count();
        sw.x = bx + bw + 16.0;
        sw.y = by + n as f32 * (sw.h + 12.0);
        sw.style.fill = color;
        sw.data.insert("from".into(), s.id.to_string());
        self.add_shape(sw, "Pick colour");
    }

    pub fn add_comment(&mut self, p: (f32, f32)) -> Id {
        self.checkpoint("Add comment", "");
        let id = self.doc.new_id();
        let shape = self.hit(p, false);
        let c = Comment { id, x: p.0, y: p.1, shape, author: self.author.clone(), text: String::new(), time: now_secs(), replies: vec![], resolved: false };
        self.pgm().comments.push(c);
        self.open_comment = Some(id);
        self.changed();
        id
    }

    fn apply_format(&mut self, id: Id, st: Style, ts: TStyle) {
        self.checkpoint("Format painter", "");
        if let Some(s) = self.pgm().get_mut(id) {
            s.style = st;
            let keep_align = s.tstyle.align;
            s.tstyle = ts;
            if s.kind == "text" {
                s.tstyle.align = keep_align;
            }
        }
        self.changed();
    }

    // ---------- Groups ----------

    pub fn update_group_bounds(&mut self, gid: Id) {
        let page = self.pg();
        if !page.get(gid).map(|s| s.is_group()).unwrap_or(false) {
            return;
        }
        let kids = page.descendants(gid);
        let Some((x, y, w, h)) = bbox_of(page, &kids) else { return };
        if let Some(g) = self.pgm().get_mut(gid) {
            g.x = x;
            g.y = y;
            g.w = w;
            g.h = h;
        }
        if let Some(parent) = self.pg().get(gid).and_then(|g| g.group) {
            self.update_group_bounds(parent);
        }
    }

    // ---------- Text editing ----------

    pub fn begin_edit(&mut self, id: Id) {
        let Some(s) = self.pg().get(id) else { return };
        if s.kind == "group" || s.kind == "ink" {
            return;
        }
        let n = s.text.len();
        self.checkpoint("Typing", &format!("text{id}"));
        self.edit = Some(TextEdit { id, caret: n, anchor: 0 });
        self.sel = vec![id];
    }

    pub fn end_edit(&mut self) {
        if let Some(ed) = self.edit.take() {
            // Trim empty text shapes away.
            let remove = self.pg().get(ed.id).map(|s| s.kind == "text" && s.text.trim().is_empty()).unwrap_or(false);
            if remove {
                self.pgm().shapes.retain(|s| s.id != ed.id);
                self.sel.retain(|x| *x != ed.id);
            } else {
                self.autosize_text(ed.id);
            }
            self.break_merge();
            self.changed();
        }
    }

    /// Grow a text shape to fit what was typed.
    pub fn autosize_text(&mut self, _id: Id) {}

    pub fn edit_text(&mut self, f: impl FnOnce(&mut String, &mut usize, &mut usize)) {
        let Some(ed) = self.edit.clone() else { return };
        let mut caret = ed.caret;
        let mut anchor = ed.anchor;
        if let Some(s) = self.pgm().get_mut(ed.id) {
            f(&mut s.text, &mut caret, &mut anchor);
        }
        self.edit = Some(TextEdit { id: ed.id, caret, anchor });
        self.changed();
    }

    pub fn type_text(&mut self, t: &str) {
        self.edit_text(|s, c, a| {
            let (x, y) = ((*c).min(*a), (*c).max(*a));
            s.replace_range(x..y, t);
            *c = x + t.len();
            *a = *c;
        });
    }

    /// Text layout of the edited shape and its local origin, for caret placement.
    pub fn edit_layout(&self, text: &mut Text) -> Option<(Shape, Layout, (f32, f32))> {
        let ed = self.edit.as_ref()?;
        let s = self.pg().get(ed.id)?.clone();
        if s.is_connector() {
            let r = route::route(self.pg(), &s);
            let (mx, my) = r.midpoint();
            let st = render::text_style(&s, s.tstyle.size);
            let lay = text.layout(&s.text, &st, None);
            return Some((s, lay.clone(), (mx - lay.width / 2.0, my - lay.height / 2.0)));
        }
        let (origin, tb, size, _) = render::text_origin(text, &s);
        let st = if s.kind == "frame" {
            folio_kit::text::TextStyle { size: 14.0, bold: true, ..Default::default() }
        } else {
            render::text_style(&s, size)
        };
        let lay = text.layout(&s.text, &st, if s.kind == "frame" { None } else { Some(tb.2) });
        let origin = if s.kind == "frame" { (2.0, -22.0) } else { origin };
        Some((s, lay, origin))
    }
}

pub fn handle_points(x: f32, y: f32, w: f32, h: f32, z: f32) -> [(f32, f32); 9] {
    [
        (x, y),
        (x + w / 2.0, y),
        (x + w, y),
        (x + w, y + h / 2.0),
        (x + w, y + h),
        (x + w / 2.0, y + h),
        (x, y + h),
        (x, y + h / 2.0),
        (x + w / 2.0, y - 24.0 / z),
    ]
}

pub fn bbox_pts(pts: &[(f32, f32)]) -> Option<(f32, f32, f32, f32)> {
    if pts.is_empty() {
        return None;
    }
    let (mut a, mut b, mut c, mut d) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in pts {
        a = a.min(p.0);
        b = b.min(p.1);
        c = c.max(p.0);
        d = d.max(p.1);
    }
    Some((a, b, c - a, d - b))
}

/// Bounding box of shapes (connectors by their route).
pub fn bbox_of(page: &Page, ids: &[Id]) -> Option<(f32, f32, f32, f32)> {
    let mut pts = vec![];
    for id in ids {
        let Some(s) = page.get(*id) else { continue };
        if s.is_connector() {
            pts.extend(route::route(page, s).polyline());
        } else if s.is_group() {
            if let Some((x, y, w, h)) = bbox_of(page, &page.descendants(s.id)) {
                pts.extend([(x, y), (x + w, y + h)]);
            }
        } else {
            let (x, y, w, h) = geom::bounds(s);
            pts.extend([(x, y), (x + w, y + h)]);
        }
    }
    bbox_pts(&pts)
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
