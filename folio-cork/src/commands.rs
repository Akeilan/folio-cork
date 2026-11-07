//! Editing commands: clipboard, arranging, formatting, pages and layers, keyboard.

use crate::editor::{bbox_of, Editor, Tool};
use crate::geom;
use crate::model::*;
use crate::render::{self, View};
use folio_kit::platform::{Key, Mods};
use folio_kit::text::{Align, Text, VAlign};
use folio_kit::Color;
use std::collections::HashMap;

pub const ZOOMS: [f32; 13] = [0.1, 0.25, 0.33, 0.5, 0.67, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0, 8.0];

impl Editor {
    // ---------- Clipboard ----------

    /// Copies of the selected shapes (with group members and glued connectors between them).
    pub fn copy_shapes(&self) -> (Vec<Shape>, Vec<(String, ImageData)>) {
        let ids = self.sel_deep();
        let page = self.pg();
        let mut out: Vec<Shape> = page.shapes.iter().filter(|s| ids.contains(&s.id)).cloned().collect();
        // Connectors between copied shapes come along.
        for s in &page.shapes {
            if let Some(c) = &s.conn {
                if !ids.contains(&s.id) {
                    if let (Some(a), Some(b)) = (c.a.shape, c.b.shape) {
                        if ids.contains(&a) && ids.contains(&b) {
                            out.push(s.clone());
                        }
                    }
                }
            }
        }
        // Freeze connectors glued to shapes that aren't copied.
        for s in out.iter_mut() {
            let r = crate::route::route(page, s);
            if let Some(c) = s.conn.as_mut() {
                for (e, p) in [(&mut c.a, r.pts.first().copied()), (&mut c.b, r.pts.last().copied())] {
                    if let Some(id) = e.shape {
                        if !ids.contains(&id) {
                            let p = p.unwrap_or((0.0, 0.0));
                            *e = End::free(p.0, p.1);
                        }
                    }
                }
            }
        }
        let imgs = out.iter().filter(|s| !s.image.is_empty()).filter_map(|s| self.doc.images.get(&s.image).map(|d| (s.image.clone(), d.clone()))).collect();
        (out, imgs)
    }

    pub fn copy(&mut self) {
        let (s, i) = self.copy_shapes();
        if !s.is_empty() {
            self.clip = s;
            self.clip_images = i;
        }
    }

    pub fn cut(&mut self) {
        self.copy();
        self.delete_sel("Cut");
    }

    /// Paste shapes with fresh ids, offset or centred on `at`.
    pub fn paste_shapes(&mut self, shapes: &[Shape], images: &[(String, ImageData)], at: Option<(f32, f32)>, label: &str) -> Vec<Id> {
        if shapes.is_empty() {
            return vec![];
        }
        self.end_edit();
        self.checkpoint(label, "");
        for (k, d) in images {
            self.doc.images.entry(k.clone()).or_insert_with(|| d.clone());
        }
        let mut map: HashMap<Id, Id> = HashMap::new();
        for s in shapes {
            let n = self.doc.new_id();
            map.insert(s.id, n);
        }
        let ids: Vec<Id> = shapes.iter().map(|s| s.id).collect();
        let tmp = Page { shapes: shapes.to_vec(), ..Default::default() };
        let bb = bbox_of(&tmp, &ids).unwrap_or((0.0, 0.0, 0.0, 0.0));
        let (dx, dy) = match at {
            Some((x, y)) => (x - (bb.0 + bb.2 / 2.0), y - (bb.1 + bb.3 / 2.0)),
            None => {
                // Offset copies that land on their originals.
                let clash = shapes.iter().any(|s| self.pg().shapes.iter().any(|o| o.kind == s.kind && (o.x - s.x).abs() < 1.0 && (o.y - s.y).abs() < 1.0));
                if clash {
                    (self.grid_size * 2.0, self.grid_size * 2.0)
                } else {
                    (0.0, 0.0)
                }
            }
        };
        let layer = self.active_layer();
        let mut new_ids = vec![];
        let mut top = vec![];
        for s in shapes {
            let mut c = s.clone();
            c.id = map[&s.id];
            c.x += dx;
            c.y += dy;
            c.group = s.group.and_then(|g| map.get(&g).copied());
            if !self.pg().layers.iter().any(|l| l.id == c.layer) {
                c.layer = layer;
            }
            if let Some(cn) = c.conn.as_mut() {
                for e in [&mut cn.a, &mut cn.b] {
                    match e.shape {
                        Some(id) if map.contains_key(&id) => e.shape = Some(map[&id]),
                        Some(_) => e.shape = None,
                        None => {
                            e.x += dx;
                            e.y += dy;
                        }
                    }
                }
            }
            if c.group.is_none() {
                top.push(c.id);
            }
            new_ids.push(c.id);
            self.pgm().shapes.push(c);
        }
        self.sel = top;
        self.changed();
        new_ids
    }

    pub fn paste(&mut self, at: Option<(f32, f32)>) {
        let (c, i) = (self.clip.clone(), self.clip_images.clone());
        self.paste_shapes(&c, &i, at, "Paste");
    }

    pub fn duplicate(&mut self) {
        let (s, i) = self.copy_shapes();
        let ids = self.paste_shapes(&s, &i, None, "Duplicate");
        if !ids.is_empty() {
            // Duplicates always step down and right.
            let g = self.grid_size * 2.0;
            let tops = self.sel.clone();
            let deep: Vec<Id> = ids.clone();
            let _ = tops;
            for id in deep {
                if let Some(s) = self.pgm().get_mut(id) {
                    if s.conn.is_none() {
                        s.x += g;
                        s.y += g;
                    } else if let Some(c) = s.conn.as_mut() {
                        for e in [&mut c.a, &mut c.b] {
                            if e.shape.is_none() {
                                e.x += g;
                                e.y += g;
                            }
                        }
                    }
                }
            }
            self.changed();
        }
    }

    pub fn delete_sel(&mut self, label: &str) {
        let mut ids = self.sel_deep();
        ids.retain(|id| self.editable(*id));
        if ids.is_empty() {
            return;
        }
        self.end_edit();
        self.checkpoint(label, "");
        let page = self.pgm();
        // Connectors glued to deleted shapes become free at their current ends.
        let routes: Vec<(Id, (f32, f32), (f32, f32))> = page
            .shapes
            .iter()
            .filter(|s| s.is_connector())
            .map(|s| {
                let r = crate::route::route(page, s);
                (s.id, r.pts.first().copied().unwrap_or_default(), r.pts.last().copied().unwrap_or_default())
            })
            .collect();
        for (cid, a, b) in routes {
            if let Some(s) = page.get_mut(cid) {
                if let Some(c) = s.conn.as_mut() {
                    if c.a.shape.map(|x| ids.contains(&x)).unwrap_or(false) {
                        c.a = End::free(a.0, a.1);
                    }
                    if c.b.shape.map(|x| ids.contains(&x)).unwrap_or(false) {
                        c.b = End::free(b.0, b.1);
                    }
                }
            }
        }
        page.shapes.retain(|s| !ids.contains(&s.id));
        page.comments.retain(|c| c.shape.map(|x| !ids.contains(&x)).unwrap_or(true));
        // Ungroup groups left with one child.
        self.sel.clear();
        self.doc.prune_images();
        self.changed();
    }

    pub fn select_all(&mut self) {
        let page = self.pg();
        self.sel = page.shapes.iter().filter(|s| s.group.is_none() && page.visible(s) && !page.layer_locked(s)).map(|s| s.id).collect();
    }

    // ---------- Arrange ----------

    pub fn group(&mut self) {
        let ids: Vec<Id> = self.sel.iter().copied().filter(|id| self.editable(*id)).collect();
        if ids.len() < 2 {
            return;
        }
        self.checkpoint("Group", "");
        let gid = self.doc.new_id();
        let layer = self.pg().get(ids[0]).map(|s| s.layer).unwrap_or(0);
        let (x, y, w, h) = bbox_of(self.pg(), &ids).unwrap_or_default();
        // The group sits where its topmost member was; members keep their order.
        let top_idx = ids.iter().filter_map(|id| self.pg().index(*id)).max().unwrap_or(0);
        let g = Shape { id: gid, kind: "group".into(), x, y, w, h, layer, ..Default::default() };
        let page = self.pgm();
        for id in &ids {
            if let Some(s) = page.get_mut(*id) {
                s.group = Some(gid);
            }
        }
        page.shapes.insert((top_idx + 1).min(page.shapes.len()), g);
        self.sel = vec![gid];
        self.changed();
    }

    pub fn ungroup(&mut self) {
        let groups: Vec<Id> = self.sel.iter().copied().filter(|id| self.pg().get(*id).map(|s| s.is_group()).unwrap_or(false)).collect();
        if groups.is_empty() {
            return;
        }
        self.checkpoint("Ungroup", "");
        let mut new_sel = vec![];
        for g in groups {
            let parent = self.pg().get(g).and_then(|s| s.group);
            let kids = self.pg().children(g);
            for k in &kids {
                if let Some(s) = self.pgm().get_mut(*k) {
                    s.group = parent;
                }
            }
            new_sel.extend(kids);
            self.pgm().shapes.retain(|s| s.id != g);
        }
        self.sel = new_sel;
        self.changed();
    }

    /// Move the selection in the drawing order: front, back, forward, backward.
    pub fn order(&mut self, how: &str) {
        let ids = self.sel_deep();
        if ids.is_empty() {
            return;
        }
        self.checkpoint("Change order", "");
        let page = self.pgm();
        let order_before: Vec<Id> = page.shapes.iter().map(|s| s.id).collect();
        let (mut moving, mut rest): (Vec<Shape>, Vec<Shape>) = page.shapes.drain(..).partition(|s| ids.contains(&s.id));
        match how {
            "front" => rest.extend(moving),
            "back" => {
                moving.extend(rest);
                rest = moving;
            }
            "forward" | "backward" => {
                let mut all = rest;
                all.extend(moving);
                // Put shapes back in their original order, then step each selected one.
                all.sort_by_key(|s| order_before.iter().position(|x| *x == s.id).unwrap_or(usize::MAX));
                let n = all.len();
                if how == "forward" {
                    for i in (0..n.saturating_sub(1)).rev() {
                        if ids.contains(&all[i].id) && !ids.contains(&all[i + 1].id) {
                            all.swap(i, i + 1);
                        }
                    }
                } else {
                    for i in 1..n {
                        if ids.contains(&all[i].id) && !ids.contains(&all[i - 1].id) {
                            all.swap(i, i - 1);
                        }
                    }
                }
                rest = all;
            }
            _ => rest.extend(moving),
        }
        // Frames and lanes stay behind everything.
        let (frames, others): (Vec<Shape>, Vec<Shape>) =
            rest.into_iter().partition(|s| s.kind == "frame" || s.kind == "lane");
        page.shapes = frames;
        page.shapes.extend(others);
        self.changed();
    }

    pub fn align(&mut self, how: &str) {
        let ids: Vec<Id> = self.sel.iter().copied().filter(|id| self.editable(*id) && !self.pg().get(*id).map(|s| s.is_connector()).unwrap_or(true)).collect();
        if ids.len() < 2 {
            return;
        }
        let Some((x, y, w, h)) = bbox_of(self.pg(), &ids) else { return };
        self.checkpoint("Align", "");
        for id in &ids {
            let Some(b) = bbox_of(self.pg(), &[*id]) else { continue };
            let (dx, dy) = match how {
                "left" => (x - b.0, 0.0),
                "center" => (x + w / 2.0 - (b.0 + b.2 / 2.0), 0.0),
                "right" => (x + w - (b.0 + b.2), 0.0),
                "top" => (0.0, y - b.1),
                "middle" => (0.0, y + h / 2.0 - (b.1 + b.3 / 2.0)),
                "bottom" => (0.0, y + h - (b.1 + b.3)),
                _ => (0.0, 0.0),
            };
            self.shift(*id, dx, dy);
        }
        self.changed();
    }

    pub fn distribute(&mut self, horizontal: bool) {
        let mut items: Vec<(Id, (f32, f32, f32, f32))> = self.sel.iter().filter_map(|id| bbox_of(self.pg(), &[*id]).map(|b| (*id, b))).collect();
        if items.len() < 3 {
            return;
        }
        self.checkpoint("Distribute", "");
        if horizontal {
            items.sort_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap());
        } else {
            items.sort_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap());
        }
        let (first, last) = (items[0].1, items[items.len() - 1].1);
        let total: f32 = items.iter().map(|i| if horizontal { i.1 .2 } else { i.1 .3 }).sum();
        let span = if horizontal { last.0 + last.2 - first.0 } else { last.1 + last.3 - first.1 };
        let gap = (span - total) / (items.len() - 1) as f32;
        let mut pos = if horizontal { first.0 } else { first.1 };
        for (id, b) in items {
            let (dx, dy) = if horizontal { (pos - b.0, 0.0) } else { (0.0, pos - b.1) };
            self.shift(id, dx, dy);
            pos += (if horizontal { b.2 } else { b.3 }) + gap;
        }
        self.changed();
    }

    /// Move a shape (and its group members, free connector ends) by a delta.
    pub fn shift(&mut self, id: Id, dx: f32, dy: f32) {
        let mut ids = vec![id];
        if self.pg().get(id).map(|s| s.is_group()).unwrap_or(false) {
            ids.extend(self.pg().descendants(id));
        }
        for i in ids {
            if let Some(s) = self.pgm().get_mut(i) {
                s.x += dx;
                s.y += dy;
                if let Some(c) = s.conn.as_mut() {
                    for e in [&mut c.a, &mut c.b] {
                        if e.shape.is_none() {
                            e.x += dx;
                            e.y += dy;
                        }
                    }
                }
            }
        }
    }

    pub fn nudge(&mut self, dx: f32, dy: f32) {
        let ids: Vec<Id> = self.sel.iter().copied().filter(|id| self.editable(*id)).collect();
        if ids.is_empty() {
            return;
        }
        self.checkpoint("Nudge", "nudge");
        for id in ids {
            self.shift(id, dx, dy);
        }
        self.changed();
    }

    pub fn set_lock(&mut self, lock: bool) {
        if self.sel.is_empty() {
            return;
        }
        self.checkpoint(if lock { "Lock" } else { "Unlock" }, "");
        for id in self.sel_deep() {
            if let Some(s) = self.pgm().get_mut(id) {
                s.locked = lock;
            }
        }
        self.changed();
    }

    pub fn rotate_by(&mut self, deg: f32) {
        self.each_editable("Rotate", |s| {
            if !s.is_connector() && s.kind != "ink" {
                s.rot = (s.rot + deg).rem_euclid(360.0);
            }
        });
    }

    pub fn flip(&mut self, horizontal: bool) {
        self.each_editable("Flip", |s| {
            if horizontal {
                s.flip_h = !s.flip_h;
            } else {
                s.flip_v = !s.flip_v;
            }
        });
    }

    /// Apply a change to every selected, editable shape (group members included).
    pub fn each_editable(&mut self, label: &str, mut f: impl FnMut(&mut Shape)) {
        let ids: Vec<Id> = self.sel_deep().into_iter().filter(|id| self.editable(*id)).collect();
        if ids.is_empty() {
            return;
        }
        let merge = format!("{label}{:?}", ids);
        self.checkpoint(label, &merge);
        for id in ids {
            if let Some(s) = self.pgm().get_mut(id) {
                if s.is_group() {
                    continue;
                }
                f(s);
            }
        }
        self.changed();
    }

    // ---------- Formatting ----------

    pub fn set_fill(&mut self, c: Color) {
        self.each_editable("Fill", |s| {
            if s.kind == "ink" {
                s.style.line = c.with_alpha(s.style.line.a);
            } else {
                s.style.fill = c;
            }
        });
    }
    pub fn set_line(&mut self, c: Color) {
        self.each_editable("Line colour", |s| s.style.line = c);
    }
    pub fn set_weight(&mut self, w: f32) {
        self.each_editable("Line weight", |s| s.style.width = w);
    }
    pub fn set_dash(&mut self, d: Dash) {
        self.each_editable("Dash", |s| s.style.dash = d);
    }
    pub fn toggle_shadow(&mut self) {
        let on = !self.first().map(|s| s.style.shadow).unwrap_or(false);
        self.each_editable("Shadow", |s| s.style.shadow = on);
    }
    pub fn set_text_color(&mut self, c: Color) {
        self.each_editable("Font colour", |s| s.tstyle.color = c);
    }
    pub fn set_font(&mut self, f: &str) {
        let f = f.to_string();
        self.each_editable("Font", |s| s.tstyle.family = f.clone());
    }
    pub fn set_size(&mut self, n: f32) {
        self.each_editable("Font size", |s| s.tstyle.size = n.clamp(4.0, 400.0));
    }
    pub fn grow_font(&mut self, up: bool) {
        self.each_editable("Font size", |s| {
            let sizes = [6.0, 8.0, 9.0, 10.0, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 36.0, 48.0, 72.0, 96.0];
            s.tstyle.size = if up {
                sizes.iter().copied().find(|x| *x > s.tstyle.size + 0.1).unwrap_or(s.tstyle.size * 1.2)
            } else {
                sizes.iter().rev().copied().find(|x| *x < s.tstyle.size - 0.1).unwrap_or((s.tstyle.size / 1.2).max(4.0))
            };
        });
    }
    pub fn toggle_text(&mut self, what: &str) {
        let cur = self.first().map(|s| match what {
            "bold" => s.tstyle.bold,
            "italic" => s.tstyle.italic,
            _ => s.tstyle.underline,
        });
        let on = !cur.unwrap_or(false);
        let w = what.to_string();
        self.each_editable("Text style", |s| match w.as_str() {
            "bold" => s.tstyle.bold = on,
            "italic" => s.tstyle.italic = on,
            _ => s.tstyle.underline = on,
        });
    }
    pub fn set_align(&mut self, a: Align) {
        self.each_editable("Align text", |s| s.tstyle.align = a);
    }
    pub fn set_valign(&mut self, a: VAlign) {
        self.each_editable("Align text", |s| s.tstyle.valign = a);
    }

    #[allow(dead_code)]
    fn remember_style(&mut self) {
        if let Some(s) = self.first() {
            if !s.is_connector() && !matches!(s.kind.as_str(), "sticky" | "text" | "frame" | "lane" | "ink" | "image" | "swatch") {
                self.new_style = Some(s.style.clone());
            }
        }
    }

    /// Quick styles: fill and line pairs.
    pub fn quick_style(&mut self, i: usize) {
        let (fill, line, text) = QUICK[i % QUICK.len()];
        self.each_editable("Quick style", |s| {
            if s.is_connector() {
                s.style.line = Color::hex(line);
                return;
            }
            s.style.fill = Color::hex(fill);
            s.style.line = Color::hex(line);
            s.tstyle.color = Color::hex(text);
        });
    }

    // ---------- Connectors ----------

    pub fn set_route(&mut self, r: Route) {
        self.each_editable("Connector style", |s| {
            if let Some(c) = s.conn.as_mut() {
                c.route = r;
            }
        });
    }
    pub fn set_arrow(&mut self, end_b: bool, a: Arrow) {
        self.each_editable("Arrows", |s| {
            if let Some(c) = s.conn.as_mut() {
                if end_b {
                    c.arrow_b = a;
                } else {
                    c.arrow_a = a;
                }
            }
        });
    }
    pub fn toggle_jumps(&mut self) {
        let on = !self.selected().iter().any(|s| s.conn.as_ref().map(|c| c.jumps).unwrap_or(false));
        self.each_editable("Line jumps", |s| {
            if let Some(c) = s.conn.as_mut() {
                c.jumps = on;
            }
        });
    }

    // ---------- Pages ----------

    pub fn add_page(&mut self, mood: bool) {
        self.end_edit();
        self.checkpoint("Insert page", "");
        let id = self.doc.new_id();
        let lid = self.doc.new_id();
        let n = self.doc.pages.len() + 1;
        let mut p = Page { id, name: format!("Page-{n}"), mood, ..Default::default() };
        p.layers[0].id = lid;
        if let Some(cur) = self.doc.pages.get(self.page) {
            p.paper = cur.paper;
        }
        if mood {
            p.name = "Mood board".into();
            p.background = Color::hex(0xFAF8F3);
        }
        self.doc.pages.insert(self.page + 1, p);
        self.page += 1;
        self.sel.clear();
        self.changed();
    }

    pub fn duplicate_page(&mut self) {
        self.end_edit();
        self.checkpoint("Duplicate page", "");
        let mut p = self.pg().clone();
        p.id = self.doc.new_id();
        p.name = format!("{} (copy)", p.name);
        // Fresh ids for shapes, keeping links inside the page.
        let mut map = HashMap::new();
        for s in &p.shapes {
            let n = self.doc.new_id();
            map.insert(s.id, n);
        }
        for s in p.shapes.iter_mut() {
            s.id = map[&s.id];
            s.group = s.group.and_then(|g| map.get(&g).copied());
            if let Some(c) = s.conn.as_mut() {
                c.a.shape = c.a.shape.and_then(|x| map.get(&x).copied());
                c.b.shape = c.b.shape.and_then(|x| map.get(&x).copied());
            }
        }
        for c in p.comments.iter_mut() {
            c.shape = c.shape.and_then(|x| map.get(&x).copied());
        }
        self.doc.pages.insert(self.page + 1, p);
        self.page += 1;
        self.sel.clear();
        self.changed();
    }

    pub fn delete_page(&mut self) {
        if self.doc.pages.len() < 2 {
            return;
        }
        self.end_edit();
        self.checkpoint("Delete page", "");
        self.doc.pages.remove(self.page);
        self.page = self.page.min(self.doc.pages.len() - 1);
        self.sel.clear();
        self.doc.prune_images();
        self.changed();
    }

    pub fn rename_page(&mut self, name: &str) {
        if name.trim().is_empty() {
            return;
        }
        self.checkpoint("Rename page", "");
        self.pgm().name = name.trim().to_string();
        self.changed();
    }

    pub fn move_page(&mut self, delta: i32) {
        let to = self.page as i32 + delta;
        if to < 0 || to as usize >= self.doc.pages.len() {
            return;
        }
        self.checkpoint("Move page", "");
        self.doc.pages.swap(self.page, to as usize);
        self.page = to as usize;
        self.changed();
    }

    pub fn go_page(&mut self, i: usize) {
        if i < self.doc.pages.len() && i != self.page {
            self.end_edit();
            self.page = i;
            self.sel.clear();
            self.layer = None;
            self.version += 1;
        }
    }

    pub fn set_paper(&mut self, paper: Paper) {
        self.checkpoint("Page size", "");
        let l = self.pg().paper.landscape;
        let p = self.pgm();
        p.paper = paper;
        p.paper.landscape = l;
        self.changed();
    }

    pub fn set_landscape(&mut self, l: bool) {
        self.checkpoint("Orientation", "");
        self.pgm().paper.landscape = l;
        self.changed();
    }

    pub fn set_background(&mut self, c: Color) {
        self.checkpoint("Background", "");
        self.pgm().background = c;
        self.changed();
    }

    // ---------- Layers ----------

    pub fn add_layer(&mut self, name: &str) -> Id {
        self.checkpoint("New layer", "");
        let id = self.doc.new_id();
        let n = self.pg().layers.len() + 1;
        let name = if name.is_empty() { format!("Layer {n}") } else { name.to_string() };
        self.pgm().layers.push(Layer { id, name, ..Default::default() });
        self.layer = Some(id);
        self.changed();
        id
    }

    pub fn layer_toggle(&mut self, id: Id, what: &str) {
        self.checkpoint("Layer", "");
        if let Some(l) = self.pgm().layers.iter_mut().find(|l| l.id == id) {
            match what {
                "visible" => l.visible = !l.visible,
                "locked" => l.locked = !l.locked,
                _ => l.print = !l.print,
            }
        }
        let page = self.pg().clone();
        self.sel.retain(|s| page.get(*s).map(|x| page.visible(x) && !page.layer_locked(x)).unwrap_or(false));
        self.changed();
    }

    pub fn rename_layer(&mut self, id: Id, name: &str) {
        self.checkpoint("Rename layer", "");
        if let Some(l) = self.pgm().layers.iter_mut().find(|l| l.id == id) {
            l.name = name.to_string();
        }
        self.changed();
    }

    pub fn delete_layer(&mut self, id: Id) {
        if self.pg().layers.len() < 2 {
            return;
        }
        self.checkpoint("Delete layer", "");
        let page = self.pgm();
        page.layers.retain(|l| l.id != id);
        let first = page.layers[0].id;
        for s in page.shapes.iter_mut() {
            if s.layer == id {
                s.layer = first;
            }
        }
        if self.layer == Some(id) {
            self.layer = None;
        }
        self.changed();
    }

    pub fn move_to_layer(&mut self, id: Id) {
        self.each_editable("Move to layer", |s| s.layer = id);
    }

    // ---------- View ----------

    pub fn zoom_at(&mut self, z: f32, sx: f32, sy: f32) {
        let mut v = self.view();
        let z = z.clamp(0.05, 16.0);
        let (px, py) = v.to_page(sx, sy);
        v.zoom = z;
        v.ox = sx - px * z;
        v.oy = sy - py * z;
        self.set_view(v);
    }

    pub fn zoom_step(&mut self, up: bool, sx: f32, sy: f32) {
        let z = self.view().zoom;
        let nz = if up { ZOOMS.iter().copied().find(|x| *x > z * 1.01).unwrap_or(z * 1.25) } else { ZOOMS.iter().rev().copied().find(|x| *x < z * 0.99).unwrap_or(z / 1.25) };
        self.zoom_at(nz, sx, sy);
    }

    /// Fit a page-space box into a viewport of the given size.
    pub fn fit_box(&mut self, b: (f32, f32, f32, f32), vw: f32, vh: f32, max: f32) {
        let pad = 40.0;
        let z = ((vw - 2.0 * pad) / b.2.max(1.0)).min((vh - 2.0 * pad) / b.3.max(1.0)).clamp(0.05, max);
        let v = View { zoom: z, ox: vw / 2.0 - (b.0 + b.2 / 2.0) * z, oy: vh / 2.0 - (b.1 + b.3 / 2.0) * z };
        self.set_view(v);
    }

    pub fn fit_page(&mut self, vw: f32, vh: f32) {
        let (pw, ph) = self.pg().paper.px();
        let b = match render::content_bounds(self.pg()) {
            Some(c) => union(c, (0.0, 0.0, pw, ph)),
            None => (0.0, 0.0, pw, ph),
        };
        self.fit_box(b, vw, vh, 1.0);
    }

    pub fn fit_selection(&mut self, vw: f32, vh: f32) {
        if let Some(b) = self.sel_bounds() {
            self.fit_box(b, vw, vh, 4.0);
        }
    }

    // ---------- Keyboard ----------

    /// Keys while editing text. Returns true when handled.
    pub fn edit_key(&mut self, key: &Key, mods: &Mods, text: &mut Text, clip: &mut dyn FnMut(Option<String>) -> Option<String>) -> bool {
        let Some(ed) = self.edit.clone() else { return false };
        let s_text = self.pg().get(ed.id).map(|s| s.text.clone()).unwrap_or_default();
        let prev = |i: usize| s_text[..i].char_indices().last().map(|(j, _)| j).unwrap_or(0);
        let next = |i: usize| s_text[i..].char_indices().nth(1).map(|(j, _)| i + j).unwrap_or(s_text.len());
        let shift = mods.shift;
        let mv = |e: &mut Editor, to: usize| {
            if let Some(ed) = e.edit.as_mut() {
                ed.caret = to;
                if !shift {
                    ed.anchor = to;
                }
            }
            e.version += 1;
        };
        let (a, b) = (ed.caret.min(ed.anchor), ed.caret.max(ed.anchor));
        match key {
            Key::Escape => {
                self.end_edit();
            }
            Key::Enter if mods.cmd() => self.end_edit(),
            Key::Enter => self.type_text("\n"),
            Key::Tab => self.type_text("    "),
            Key::Backspace => {
                if a != b {
                    self.type_text("");
                } else if ed.caret > 0 {
                    let p = prev(ed.caret);
                    self.edit_text(|s, c, an| {
                        s.replace_range(p..*c, "");
                        *c = p;
                        *an = p;
                    });
                }
            }
            Key::Delete => {
                if a != b {
                    self.type_text("");
                } else if ed.caret < s_text.len() {
                    let n = next(ed.caret);
                    self.edit_text(|s, c, _| s.replace_range(*c..n, ""));
                }
            }
            Key::Left => mv(self, if a != b && !shift { a } else { prev(ed.caret) }),
            Key::Right => mv(self, if a != b && !shift { b } else { next(ed.caret) }),
            Key::Up | Key::Down | Key::Home | Key::End => {
                if let Some((_, lay, _)) = self.edit_layout(text) {
                    let line = lay.line_of(ed.caret);
                    let (cx, _, _) = lay.caret(ed.caret);
                    let to = match key {
                        Key::Home => lay.lines.get(line).map(|l| l.start).unwrap_or(0),
                        Key::End => lay.lines.get(line).map(|l| l.end).unwrap_or(s_text.len()),
                        Key::Up if line > 0 => {
                            let l = &lay.lines[line - 1];
                            lay.hit(cx, l.top + l.height / 2.0)
                        }
                        Key::Down if line + 1 < lay.lines.len() => {
                            let l = &lay.lines[line + 1];
                            lay.hit(cx, l.top + l.height / 2.0)
                        }
                        Key::Up => 0,
                        _ => s_text.len(),
                    };
                    let to = if mods.cmd() && *key == Key::Home { 0 } else if mods.cmd() && *key == Key::End { s_text.len() } else { to };
                    mv(self, to);
                }
            }
            Key::Char('a') if mods.cmd() => {
                if let Some(e) = self.edit.as_mut() {
                    e.anchor = 0;
                    e.caret = s_text.len();
                }
                self.version += 1;
            }
            Key::Char('c') | Key::Char('x') if mods.cmd() => {
                if a != b {
                    clip(Some(s_text[a..b].to_string()));
                    if *key == Key::Char('x') {
                        self.type_text("");
                    }
                }
            }
            Key::Char('v') if mods.cmd() => {
                if let Some(t) = clip(None) {
                    let t = t.replace("\r\n", "\n");
                    self.type_text(&t);
                }
            }
            Key::Char('b') if mods.cmd() => self.toggle_text("bold"),
            Key::Char('i') if mods.cmd() => self.toggle_text("italic"),
            Key::Char('u') if mods.cmd() => self.toggle_text("underline"),
            Key::Char('z') if mods.cmd() => self.undo(),
            Key::Char(_) | Key::Space | Key::Shift | Key::Ctrl | Key::Alt | Key::Logo | Key::Other => return !mods.cmd(),
            _ => return false,
        }
        true
    }

    /// Editing keys outside text editing. Returns true when handled.
    pub fn key(&mut self, key: &Key, mods: &Mods, vw: f32, vh: f32) -> bool {
        let step = if mods.shift { self.grid_size.max(10.0) } else { 1.0 };
        match key {
            Key::Delete | Key::Backspace => self.delete_sel("Delete"),
            Key::Escape => {
                if self.tool != Tool::Pointer {
                    self.tool = Tool::Pointer;
                    self.tool_lock = false;
                } else if self.format_brush.is_some() {
                    self.format_brush = None;
                } else {
                    self.sel.clear();
                }
                self.drag = crate::editor::Drag::None;
            }
            Key::Left => self.nudge(-step, 0.0),
            Key::Right => self.nudge(step, 0.0),
            Key::Up => self.nudge(0.0, -step),
            Key::Down => self.nudge(0.0, step),
            Key::F(2) | Key::Enter => {
                if let Some(id) = self.sel.first().copied() {
                    if self.sel.len() == 1 && self.editable(id) {
                        self.begin_edit(id);
                    }
                }
            }
            Key::Char(c) if mods.cmd() => match (c, mods.shift) {
                ('z', false) => self.undo(),
                ('z', true) | ('y', _) => self.redo(),
                ('a', _) => self.select_all(),
                ('d', _) => self.duplicate(),
                ('g', false) => self.group(),
                ('g', true) => self.ungroup(),
                ('b', _) => self.toggle_text("bold"),
                ('i', _) => self.toggle_text("italic"),
                ('u', _) => self.toggle_text("underline"),
                (']', false) => self.order("forward"),
                ('[', false) => self.order("backward"),
                (']', true) => self.order("front"),
                ('[', true) => self.order("back"),
                ('=', _) | ('+', _) => self.zoom_step(true, vw / 2.0, vh / 2.0),
                ('-', _) => self.zoom_step(false, vw / 2.0, vh / 2.0),
                ('0', _) => self.fit_page(vw, vh),
                ('1', _) => self.zoom_at(1.0, vw / 2.0, vh / 2.0),
                ('l', _) => {
                    let lock = !self.first().map(|s| s.locked).unwrap_or(false);
                    self.set_lock(lock)
                }
                _ => return false,
            },
            Key::Char(c) if !mods.alt && !mods.ctrl && !mods.logo && self.sel.is_empty() => {
                // Single-key tools, like whiteboard apps.
                let t = match c {
                    'v' => Tool::Pointer,
                    'h' => Tool::Hand,
                    'r' => Tool::Shape("rect".into()),
                    'o' => Tool::Shape("ellipse".into()),
                    'l' | 'c' => Tool::Connector,
                    't' => Tool::Text,
                    'n' | 's' => Tool::Sticky,
                    'p' => Tool::Pen,
                    'f' => Tool::Frame,
                    'e' => Tool::Eraser,
                    _ => return false,
                };
                self.tool = t;
            }
            _ => return false,
        }
        true
    }

    /// Make a text shape's height fit its text.
    pub fn fit_text_shape(&mut self, text: &mut Text, id: Id) {
        let Some(s) = self.pg().get(id).cloned() else { return };
        if s.kind != "text" {
            return;
        }
        let st = render::text_style(&s, s.tstyle.size);
        let lay = text.layout(&s.text, &st, Some((s.w - 4.0).max(10.0)));
        let h = (lay.height + 4.0).max(st.size * 1.4);
        if let Some(m) = self.pgm().get_mut(id) {
            m.h = h;
        }
    }

    pub fn bounds_page(&self) -> Option<(f32, f32, f32, f32)> {
        render::content_bounds(self.pg())
    }

    pub fn shape_at_center(&self, vw: f32, vh: f32) -> (f32, f32) {
        self.view().to_page(vw / 2.0, vh / 2.0)
    }

    pub fn conn_points_of(&self, id: Id) -> Vec<(f32, f32)> {
        self.pg().get(id).map(geom::conn_points).unwrap_or_default()
    }
}

/// Fill, line and text colours for the Quick Styles gallery.
pub const QUICK: [(u32, u32, u32); 12] = [
    (0xDCE8F7, 0x2F5597, 0x1E1E1E),
    (0x4472C4, 0x2F5597, 0xFFFFFF),
    (0xFBE5D6, 0xC55A11, 0x1E1E1E),
    (0xED7D31, 0xAE5A21, 0xFFFFFF),
    (0xE2F0D9, 0x548235, 0x1E1E1E),
    (0x70AD47, 0x507E32, 0xFFFFFF),
    (0xFFF2CC, 0xBF9000, 0x1E1E1E),
    (0xFFC000, 0xBF9000, 0x1E1E1E),
    (0xEDEDED, 0x7F7F7F, 0x1E1E1E),
    (0x595959, 0x262626, 0xFFFFFF),
    (0xFFFFFF, 0x1E1E1E, 0x1E1E1E),
    (0xE8D5F5, 0x7030A0, 0x1E1E1E),
];

fn union(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    (x0, y0, x1 - x0, y1 - y0)
}

// ---------------------------------------------------------------- swimlanes (K4-01)
impl Editor {
    /// The lanes of the page, in the order they run (left to right, or top to bottom).
    pub fn lanes(&self) -> Vec<Shape> {
        let mut v: Vec<Shape> = self.pg().shapes.iter().filter(|s| s.is_lane()).cloned().collect();
        let vertical = v.first().map(|s| s.lane_vertical()).unwrap_or(false);
        if vertical {
            v.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            v.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
        }
        v
    }

    /// Add a lane after the selected one, or after the last (K4-01). The first one starts a set.
    pub fn add_lane(&mut self) -> Id {
        let lanes = self.lanes();
        let after = self
            .selected()
            .iter()
            .find(|s| s.is_lane())
            .map(|s| s.id)
            .or_else(|| lanes.last().map(|s| s.id));
        let base = after.and_then(|id| self.pg().get(id)).cloned();
        let mut s = self.make_shape("lane", 0.0, 0.0);
        if let Some(b) = &base {
            s.w = b.w;
            s.h = b.h;
            s.style = b.style.clone();
            s.tstyle = b.tstyle.clone();
            s.data.insert("lane".into(), if b.lane_vertical() { "v".into() } else { "h".into() });
            if b.lane_vertical() {
                s.x = b.x + b.w;
                s.y = b.y;
            } else {
                s.x = b.x;
                s.y = b.y + b.h;
            }
        } else {
            s.x = 120.0;
            s.y = 120.0;
        }
        s.text = format!("Lane {}", lanes.len() + 1);
        let id = s.id;
        let vertical = s.lane_vertical();
        let (nx, ny) = (s.x, s.y);
        self.add_shape(s, "Add Lane");
        // the lanes after it move along, so the set stays in one piece
        if let Some(b) = base {
            let ids: Vec<Id> = self
                .pg()
                .shapes
                .iter()
                .filter(|o| o.is_lane() && o.id != id && if vertical { o.x >= nx } else { o.y >= ny })
                .map(|o| o.id)
                .collect();
            for oid in ids {
                let (dx, dy) = if vertical { (b.w, 0.0) } else { (0.0, b.h) };
                self.shift(oid, dx, dy);
                for cid in self.inside_lane(oid) {
                    self.shift(cid, dx, dy);
                }
            }
        }
        self.sel = vec![id];
        self.changed();
        id
    }

    /// The shapes lying in a lane's body (not the lane itself).
    pub fn inside_lane(&self, id: Id) -> Vec<Id> {
        let lane = match self.pg().get(id) {
            Some(s) if s.is_lane() => s.clone(),
            _ => return vec![],
        };
        let (lx, ly, lw, lh) = lane.lane_body();
        self.pg()
            .shapes
            .iter()
            .filter(|s| s.id != id && !s.is_lane() && !s.is_connector())
            .filter(|s| {
                let (x, y, w, h) = geom::bounds(s);
                x >= lx && y >= ly && x + w <= lx + lw && y + h <= ly + lh
            })
            .map(|s| s.id)
            .collect()
    }

    /// Delete the selected lanes, leaving what was in them (K4-01).
    pub fn delete_lane(&mut self) -> usize {
        let ids: Vec<Id> = self.selected().iter().filter(|s| s.is_lane()).map(|s| s.id).collect();
        if ids.is_empty() {
            return 0;
        }
        self.checkpoint("Delete Lane", "");
        let gone: Vec<Shape> = ids.iter().filter_map(|id| self.pg().get(*id)).cloned().collect();
        self.pgm().shapes.retain(|s| !ids.contains(&s.id));
        // the lanes after each one close the gap
        for g in &gone {
            let vertical = g.lane_vertical();
            let moving: Vec<Id> = self
                .pg()
                .shapes
                .iter()
                .filter(|o| o.is_lane() && if vertical { o.x > g.x } else { o.y > g.y })
                .map(|o| o.id)
                .collect();
            for oid in moving {
                let (dx, dy) = if vertical { (-g.w, 0.0) } else { (0.0, -g.h) };
                for cid in self.inside_lane(oid) {
                    self.shift(cid, dx, dy);
                }
                self.shift(oid, dx, dy);
            }
        }
        self.sel.clear();
        self.changed();
        gone.len()
    }

    /// Turn the lanes: across the page, or down it (K4-01).
    pub fn turn_lanes(&mut self) -> usize {
        let lanes = self.lanes();
        if lanes.is_empty() {
            return 0;
        }
        self.checkpoint("Turn Lanes", "");
        let vertical = !lanes[0].lane_vertical();
        let (ox, oy) = (lanes[0].x, lanes[0].y);
        // what sits in each lane travels with it, keeping its place in the lane
        let held: Vec<(Id, Vec<(Id, f32, f32)>)> = lanes
            .iter()
            .map(|l| {
                let (bx, by, _, _) = l.lane_body();
                let (_, _, bw, bh) = l.lane_body();
                let inside = self
                    .inside_lane(l.id)
                    .into_iter()
                    .filter_map(|cid| {
                        // where it sits in the lane, as a fraction, so it stays inside when turned
                        self.pg().get(cid).map(|c| (cid, (c.x - bx) / bw.max(1.0), (c.y - by) / bh.max(1.0)))
                    })
                    .collect();
                (l.id, inside)
            })
            .collect();
        let mut at = if vertical { ox } else { oy };
        for (l, inside) in held {
            let (w, h, nx, ny) = {
                let s = match self.pg().get(l) {
                    Some(s) => s,
                    None => continue,
                };
                let (w, h) = (s.h, s.w);       // the lane turns with its contents
                if vertical {
                    (w, h, at, oy)
                } else {
                    (w, h, ox, at)
                }
            };
            {
                let page = self.pgm();
                if let Some(s) = page.shapes.iter_mut().find(|s| s.id == l) {
                    s.w = w;
                    s.h = h;
                    s.x = nx;
                    s.y = ny;
                    s.data.insert("lane".into(), if vertical { "v".into() } else { "h".into() });
                }
            }
            at += if vertical { w } else { h };
            let (bx, by, bw, bh) = match self.pg().get(l) {
                Some(s) => s.lane_body(),
                None => continue,
            };
            for (cid, fx, fy) in inside {
                let page = self.pgm();
                if let Some(c) = page.shapes.iter_mut().find(|s| s.id == cid) {
                    c.x = (bx + fx * bw).min(bx + bw - c.w).max(bx);
                    c.y = (by + fy * bh).min(by + bh - c.h).max(by);
                }
            }
        }
        self.changed();
        lanes.len()
    }
}

// ---------------------------------------------------------------- shape data (K4-02)
impl Editor {
    /// Put a property on every selected shape.
    pub fn set_prop(&mut self, name: &str, value: &str) -> usize {
        let name = name.trim().to_string();
        if name.is_empty() {
            return 0;
        }
        let mut n = 0;
        self.each_editable("Shape Data", |s| {
            s.set_prop(&name, value);
            n += 1;
        });
        n
    }

    /// Take a property off every selected shape.
    pub fn clear_prop(&mut self, name: &str) -> usize {
        let name = name.to_string();
        let mut n = 0;
        self.each_editable("Shape Data", |s| {
            if !s.prop(&name).is_empty() || s.data.contains_key(&format!("{PROP}{name}")) {
                s.clear_prop(&name);
                n += 1;
            }
        });
        n
    }

    /// Rename a property on every selected shape, keeping each one's value.
    pub fn rename_prop(&mut self, from: &str, to: &str) -> usize {
        let (from, to) = (from.to_string(), to.trim().to_string());
        if to.is_empty() || from == to {
            return 0;
        }
        let mut n = 0;
        self.each_editable("Shape Data", |s| {
            if let Some(v) = s.data.remove(&format!("{PROP}{from}")) {
                s.set_prop(&to, &v);
                n += 1;
            }
        });
        n
    }

    /// The properties of the selection: every name on any of them, with the shared value where
    /// they all agree and an empty value where they don't.
    pub fn props_of_selection(&self) -> Vec<(String, String)> {
        let sel: Vec<&Shape> = self.selected();
        let mut out: Vec<(String, String)> = vec![];
        for s in &sel {
            for (k, _) in s.props() {
                if !out.iter().any(|(n, _)| *n == k) {
                    out.push((k, String::new()));
                }
            }
        }
        for (name, value) in out.iter_mut() {
            // the value shows only where every selected shape agrees on it
            let mut vals = sel.iter().map(|s| s.prop(name).to_string());
            let first = vals.next().unwrap_or_default();
            *value = if vals.all(|v| v == first) { first } else { String::new() };
        }
        out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        out
    }
}

// ---------------------------------------------------------------- find and replace (K4-05)
/// One shape whose text holds what was searched for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub page: usize,
    pub shape: Id,
}

/// Where `needle` sits in `hay`, under the given rules, as positions in `hay` itself.
fn finds(hay: &str, needle: &str, case: bool, whole: bool) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return vec![];
    }
    // A case-blind search works on the folded text. Lowercasing can make a character longer
    // (Turkish I with a dot, say), so each byte of the folded text keeps the place it came from
    // and the matches are reported in the original's own positions.
    let (h, back): (String, Vec<usize>) = if case {
        (hay.to_string(), (0..=hay.len()).collect())
    } else {
        let mut f = String::new();
        let mut back = Vec::new();
        for (i, c) in hay.char_indices() {
            for lc in c.to_lowercase() {
                let before = f.len();
                f.push(lc);
                for _ in before..f.len() {
                    back.push(i);
                }
            }
        }
        back.push(hay.len());
        (f, back)
    };
    let n = if case { needle.to_string() } else { needle.to_lowercase() };
    let mut out = vec![];
    let mut at = 0;
    while let Some(i) = h[at..].find(&n) {
        let s = at + i;
        let e = s + n.len();
        at = s + n.len().max(1);
        if whole {
            let before = h[..s].chars().next_back();
            let after = h[e..].chars().next();
            let wordish = |c: char| c.is_alphanumeric() || c == '_';
            if before.map(wordish).unwrap_or(false) || after.map(wordish).unwrap_or(false) {
                continue;
            }
        }
        let (os, oe) = (back.get(s).copied().unwrap_or(hay.len()), back.get(e).copied().unwrap_or(hay.len()));
        if os < oe && hay.is_char_boundary(os) && hay.is_char_boundary(oe) {
            out.push((os, oe));
        }
    }
    out
}

/// `hay` with every match replaced, and how many were replaced.
fn replaced(hay: &str, needle: &str, with: &str, case: bool, whole: bool) -> (String, usize) {
    let spans = finds(hay, needle, case, whole);
    if spans.is_empty() {
        return (hay.to_string(), 0);
    }
    let mut out = String::new();
    let mut at = 0;
    let mut n = 0;
    for (s, e) in &spans {
        if *s < at || *e > hay.len() || !hay.is_char_boundary(*s) || !hay.is_char_boundary(*e) {
            continue;
        }
        out.push_str(&hay[at..*s]);
        out.push_str(with);
        at = *e;
        n += 1;
    }
    out.push_str(&hay[at..]);
    (out, n)
}

impl Editor {
    /// Every shape whose text holds `q`, across all pages, page by page.
    pub fn find_all(&self, q: &str, case: bool, whole: bool) -> Vec<Hit> {
        let mut out = vec![];
        for (pi, p) in self.doc.pages.iter().enumerate() {
            for s in &p.shapes {
                if s.is_group() {
                    continue;
                }
                if !finds(&s.text, q, case, whole).is_empty() {
                    out.push(Hit { page: pi, shape: s.id });
                }
            }
        }
        out
    }

    /// Go to the next shape holding `q` after the one shown now, wrapping round at the end.
    /// Returns what was found, or nothing when the drawing has no match at all.
    pub fn find_next(&mut self, q: &str, case: bool, whole: bool, back: bool) -> Option<Hit> {
        let hits = self.find_all(q, case, whole);
        if hits.is_empty() {
            return None;
        }
        let here = hits.iter().position(|h| h.page == self.page && self.sel.contains(&h.shape));
        let next = match (here, back) {
            (Some(i), false) => (i + 1) % hits.len(),
            (Some(i), true) => (i + hits.len() - 1) % hits.len(),
            // nothing of the kind is selected: start from this page
            (None, false) => hits.iter().position(|h| h.page >= self.page).unwrap_or(0),
            (None, true) => hits.iter().rposition(|h| h.page <= self.page).unwrap_or(hits.len() - 1),
        };
        let hit = hits[next];
        self.end_edit();
        self.page = hit.page;
        self.sel = vec![hit.shape];
        self.version += 1;
        Some(hit)
    }

    /// Replace the text of the shapes found by `q`: the selected ones only, or all of them.
    pub fn replace_all(&mut self, q: &str, with: &str, case: bool, whole: bool, selection_only: bool) -> usize {
        if q.is_empty() {
            return 0;
        }
        let hits = self.find_all(q, case, whole);
        let hits: Vec<Hit> = if selection_only { hits.into_iter().filter(|h| h.page == self.page && self.sel.contains(&h.shape)).collect() } else { hits };
        if hits.is_empty() {
            return 0;
        }
        self.end_edit();
        self.checkpoint("Replace", "");
        let mut n = 0;
        for h in &hits {
            let locked = self.doc.pages[h.page].get(h.shape).map(|s| s.locked).unwrap_or(true);
            let layer_locked = self.doc.pages[h.page].get(h.shape).map(|s| self.doc.pages[h.page].layer_locked(s)).unwrap_or(true);
            if locked || layer_locked {
                continue;
            }
            if let Some(s) = self.doc.pages[h.page].get_mut(h.shape) {
                let (t, c) = replaced(&s.text, q, with, case, whole);
                if c > 0 {
                    s.text = t;
                    n += c;
                }
            }
        }
        if n == 0 {
            self.drop_checkpoint();
        } else {
            self.changed();
        }
        n
    }

    /// Bring a shape into view, moving the page under the window if it is off the edge.
    pub fn reveal(&mut self, id: Id, vw: f32, vh: f32) {
        let Some(s) = self.pg().get(id).cloned() else { return };
        let b = if s.is_connector() {
            let r = crate::route::route(self.pg(), &s);
            let (xs, ys): (Vec<f32>, Vec<f32>) = (r.pts.iter().map(|p| p.0).collect(), r.pts.iter().map(|p| p.1).collect());
            let (x0, x1) = (xs.iter().cloned().fold(f32::MAX, f32::min), xs.iter().cloned().fold(f32::MIN, f32::max));
            let (y0, y1) = (ys.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(f32::MIN, f32::max));
            (x0, y0, x1 - x0, y1 - y0)
        } else {
            crate::geom::bounds(&s)
        };
        if !b.0.is_finite() || !b.1.is_finite() {
            return;
        }
        let mut v = self.view();
        let (sx, sy) = v.to_screen(b.0, b.1);
        let (ex, ey) = v.to_screen(b.0 + b.2, b.1 + b.3);
        let pad = 40.0;
        let inside = sx > pad && sy > pad && ex < vw - pad && ey < vh - pad;
        if inside {
            return;
        }
        v.ox = vw / 2.0 - (b.0 + b.2 / 2.0) * v.zoom;
        v.oy = vh / 2.0 - (b.1 + b.3 / 2.0) * v.zoom;
        self.set_view(v);
        self.version += 1;
    }
}

// ---------------------------------------------------------------- hyperlinks (K4-04)
impl Editor {
    /// Point every selected shape at a page, a file or an address.
    pub fn set_link(&mut self, kind: LinkKind, target: &str) -> usize {
        let target = target.trim().to_string();
        let mut n = 0;
        self.each_editable("Hyperlink", |s| {
            s.set_link(kind, &target);
            n += 1;
        });
        n
    }

    /// Take the link off every selected shape.
    pub fn clear_link(&mut self) -> usize {
        let mut n = 0;
        self.each_editable("Remove hyperlink", |s| {
            if s.link().is_some() {
                s.clear_link();
                n += 1;
            }
        });
        n
    }

    /// The link of the selected shape, if just one is selected and it has one.
    pub fn link_of_selection(&self) -> Option<(LinkKind, String)> {
        let sel = self.selected();
        if sel.len() != 1 {
            return None;
        }
        sel[0].link().map(|(k, t)| (k, t.to_string()))
    }

    pub fn page_names(&self) -> Vec<String> {
        self.doc.pages.iter().map(|p| p.name.clone()).collect()
    }

    /// Go to the page a link points at. Returns whether there is such a page.
    pub fn goto_page_named(&mut self, name: &str) -> bool {
        if let Some(i) = self.doc.pages.iter().position(|p| p.name.eq_ignore_ascii_case(name.trim())) {
            self.end_edit();
            self.page = i;
            self.sel.clear();
            self.version += 1;
            return true;
        }
        false
    }
}
