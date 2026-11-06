//! Field codes edited in place, as in Word: Alt+F9 shows every field's code as text in the
//! document between its braces, Shift+F9 one field's, Ctrl+F9 makes empty braces to type a
//! code in, F9 works the code out and shows the result.
use crate::eng;
use super::fields::{order, para_of};
use super::{Editor, Pos};
use eng::document::Item;
use eng::xml::NodeId;

impl Editor {
    /// Fields with a w:fldSimple (code in an attribute) become begin / code / separate /
    /// result / end, so their code can be edited.
    fn simple_to_complex(&mut self, fs: NodeId) -> Option<NodeId> {
        let dom = &mut self.doc.dom;
        let instr = dom.attr(fs, "w:instr").unwrap_or("").to_string();
        let parent = dom.parent(fs)?;
        let idx = dom.index_in_parent(fs)?;
        let kids: Vec<NodeId> = dom.children(fs).to_vec();
        let like_rpr = kids.iter().copied().find(|&k| dom.is(k, "w:r")).and_then(|r| dom.child(r, "w:rPr"));
        let rpr = like_rpr.map(|r| dom.clone_node(r));
        let mut runs = Vec::new();
        let mut begin = None;
        for kind in ["begin", "instr", "separate"] {
            let r = dom.new_element("w:r", &[]);
            if let Some(rp) = rpr {
                let c = dom.clone_node(rp);
                dom.append(r, c);
            }
            let c = if kind == "instr" {
                let e = dom.new_element("w:instrText", &[("xml:space", "preserve")]);
                let t = dom.new_text(&format!(" {} ", instr.trim()));
                dom.append(e, t);
                e
            } else {
                dom.new_element("w:fldChar", &[("w:fldCharType", kind)])
            };
            if kind == "begin" {
                begin = Some(c);
            }
            dom.append(r, c);
            runs.push(r);
        }
        for k in kids {
            dom.detach(k);
            runs.push(k);
        }
        let r = dom.new_element("w:r", &[]);
        if let Some(rp) = rpr {
            let c = dom.clone_node(rp);
            dom.append(r, c);
        }
        let c = dom.new_element("w:fldChar", &[("w:fldCharType", "end")]);
        dom.append(r, c);
        runs.push(r);
        dom.detach(fs);
        for (i, r) in runs.into_iter().enumerate() {
            dom.insert(parent, idx + i, r);
        }
        begin
    }

    fn all_roots(&self) -> Vec<NodeId> {
        let mut v = vec![self.doc.body];
        for p in &self.doc.parts {
            v.push(p.root);
        }
        v
    }

    fn simple_fields(&self) -> Vec<NodeId> {
        let dom = &self.doc.dom;
        let mut out = Vec::new();
        for root in self.all_roots() {
            let mut nodes = Vec::new();
            order(dom, root, &mut nodes);
            out.extend(nodes.into_iter().filter(|&n| dom.is(n, "w:fldSimple")));
        }
        out
    }

    /// Show field codes instead of results everywhere, or back (Alt+F9).
    pub fn toggle_all_field_codes(&mut self) {
        self.begin();
        if !self.doc.show_codes {
            for fs in self.simple_fields() {
                self.simple_to_complex(fs);
            }
        }
        self.doc.show_codes = !self.doc.show_codes;
        self.doc.code_fields.clear();
        self.cache.clear();
        self.commit("Toggle Field Codes", false);
        self.relayout();
        let p = self.caret.para;
        self.caret = Pos { para: p, off: self.caret.off.min(self.para_len(p)) };
        self.anchor = self.caret;
    }

    /// Fields in a paragraph: (begin node, start offset, end offset), innermost last.
    fn para_field_spans(&self, p: NodeId) -> Vec<(NodeId, usize, usize)> {
        let pieces = self.pieces(p);
        let dom = &self.doc.dom;
        let mut open: Vec<(NodeId, usize)> = Vec::new();
        let mut out = Vec::new();
        for (pc, s, _e) in &pieces {
            let Some(node) = pc.node else { continue };
            if dom.is(node, "w:fldChar") {
                match dom.attr(node, "w:fldCharType") {
                    Some("begin") => open.push((node, *s)),
                    Some("end") => {
                        if let Some((b, a)) = open.pop() {
                            out.push((b, a, *s));
                        }
                    }
                    _ => {}
                }
            } else if dom.is(node, "w:fldSimple") {
                out.push((node, *s, *s));
            }
        }
        // fields running on past this paragraph
        let end = self.para_len(p);
        for (b, a) in open {
            out.push((b, a, end));
        }
        out
    }

    /// The innermost field the selection is in: its begin node.
    fn field_here(&self) -> Option<(NodeId, usize, usize)> {
        let (a, b) = self.selection();
        let spans = self.para_field_spans(a.para);
        spans.into_iter()
            .filter(|(_n, s, e)| *s <= a.off && b.off <= *e.max(s) || (a.para != b.para && *s <= a.off))
            .min_by_key(|(_n, s, e)| e - s)
    }

    /// Shift+F9: show the code of the field at the cursor, or its result again.
    pub fn toggle_field_code_here(&mut self) -> bool {
        let Some((mut begin, start, _end)) = self.field_here() else { return false };
        self.begin();
        if self.doc.dom.is(begin, "w:fldSimple") {
            match self.simple_to_complex(begin) {
                Some(b) => begin = b,
                None => return false,
            }
        }
        if !self.doc.code_fields.remove(&begin) {
            self.doc.code_fields.insert(begin);
        }
        self.cache.clear();
        self.commit("Toggle Field Codes", false);
        self.relayout();
        let p = self.caret.para;
        self.caret = Pos { para: p, off: start.min(self.para_len(p)) };
        self.anchor = self.caret;
        true
    }

    fn shows_code(&self, begin: NodeId) -> bool {
        self.doc.show_codes != self.doc.code_fields.contains(&begin)
    }

    /// Ctrl+F9: empty field braces at the cursor (the selection goes inside), the cursor
    /// between them to type the code.
    pub fn insert_field_braces(&mut self) -> bool {
        let sel = if self.has_selection() { self.selected_text() } else { String::new() };
        let sel = sel.split_whitespace().collect::<Vec<_>>().join(" ");
        self.begin();
        self.delete_selection();
        let p = self.caret.para;
        let off = self.caret.off;
        let (parent, mut idx, like) = self.split_runs_at(p, off);
        let code = if sel.is_empty() { "  ".to_string() } else { format!(" {sel} ") };
        let mut begin_node = None;
        for kind in ["begin", "instr", "end"] {
            let r = self.new_run(p, like, &[]);
            let dom = &mut self.doc.dom;
            let c = if kind == "instr" {
                let e = dom.new_element("w:instrText", &[("xml:space", "preserve")]);
                let t = dom.new_text(&code);
                dom.append(e, t);
                e
            } else {
                dom.new_element("w:fldChar", &[("w:fldCharType", kind)])
            };
            if kind == "begin" {
                begin_node = Some(c);
            }
            dom.append(r, c);
            dom.insert(parent, idx, r);
            idx += 1;
        }
        let b = begin_node.unwrap();
        if !self.doc.show_codes {
            self.doc.code_fields.insert(b);
        }
        self.cache.clear();
        self.commit("Insert Field", false);
        self.relayout();
        // inside the braces: after the first space and the selected text
        let n = if sel.is_empty() { 1 } else { 1 + sel.chars().count() };
        self.caret = Pos { para: p, off: (off + n).min(self.para_len(p)) };
        self.anchor = self.caret;
        true
    }

    /// F9 on fields showing their code: the results show again (after the update).
    pub fn show_results_here(&mut self) -> usize {
        let (a, b) = self.selection();
        let paras = self.paras_between(a.para, b.para);
        let mut n = 0;
        for p in paras {
            for (begin, s, e) in self.para_field_spans(p) {
                let inside = if a.para == b.para { s <= b.off && a.off <= e } else { true };
                if inside && self.shows_code(begin) && self.doc.code_fields.contains(&begin) {
                    self.doc.code_fields.remove(&begin);
                    n += 1;
                }
            }
        }
        if n > 0 {
            self.cache.clear();
            self.relayout();
            let p = self.caret.para;
            self.caret = Pos { para: p, off: self.caret.off.min(self.para_len(p)) };
            self.anchor = self.caret;
        }
        n
    }

    /// Text typed into a field's code shown in place is part of the code (w:instrText),
    /// not of the result.
    pub(super) fn code_text_to_instr(&mut self, p: NodeId) {
        if !self.doc.show_codes && self.doc.code_fields.is_empty() {
            return;
        }
        let dom = &self.doc.dom;
        let mut nodes = Vec::new();
        order(dom, p, &mut nodes);
        let mut in_code: Vec<bool> = Vec::new();
        let mut fix = Vec::new();
        for n in nodes {
            if dom.is(n, "w:fldChar") {
                match dom.attr(n, "w:fldCharType") {
                    Some("begin") => in_code.push(true),
                    Some("separate") => {
                        if let Some(l) = in_code.last_mut() {
                            *l = false;
                        }
                    }
                    Some("end") => {
                        in_code.pop();
                    }
                    _ => {}
                }
            } else if dom.is(n, "w:t") && in_code.last().copied().unwrap_or(false) {
                fix.push(n);
            }
        }
        for n in fix {
            self.doc.dom.rename(n, "w:instrText");
            self.doc.dom.set_attr(n, "xml:space", "preserve");
        }
        let _ = para_of;
        let _ = Item::Tab;
    }
}
