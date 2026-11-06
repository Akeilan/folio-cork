//! File > Info > Check for Issues: what the Document Inspector and the
//! Compatibility Checker look for, and the Inspector's Remove All.

use crate::eng;
use super::fields::order;
use super::Editor;
use eng::xml::NodeId;
use std::collections::BTreeMap;

fn off(v: Option<&str>) -> bool {
    matches!(v, Some("0" | "false" | "off"))
}

impl Editor {
    /// Every element of the body and of the headers, footers and notes.
    fn all_story_nodes(&self) -> Vec<NodeId> {
        let dom = &self.doc.dom;
        let mut nodes = Vec::new();
        order(dom, self.doc.body, &mut nodes);
        for p in &self.doc.parts {
            order(dom, p.root, &mut nodes);
        }
        nodes
    }

    /// Runs formatted as hidden (directly) that hold text.
    fn hidden_runs(&self) -> Vec<NodeId> {
        let dom = &self.doc.dom;
        self.all_story_nodes().into_iter().filter(|&n| {
            dom.is(n, "w:r")
                && dom.child(n, "w:rPr").and_then(|r| dom.child(r, "w:vanish")).map_or(false, |v| !off(dom.attr(v, "w:val")))
                && dom.elements(n).any(|c| dom.is(c, "w:t") && !dom.text(c).is_empty())
        }).collect()
    }

    /// Drawings (pictures, shapes) set to be invisible: their runs.
    fn invisible_objects(&self) -> Vec<NodeId> {
        let dom = &self.doc.dom;
        self.all_story_nodes().into_iter().filter(|&n| {
            dom.is(n, "w:r") && dom.elements(n).any(|d| {
                dom.is(d, "w:drawing") && dom.elements(d).any(|a| {
                    dom.child(a, "wp:docPr").map_or(false, |p| matches!(dom.attr(p, "hidden"), Some("1" | "true")))
                })
            })
        }).collect()
    }

    /// Counts of what the inspector and the checker report.
    pub fn inspect_counts(&self) -> BTreeMap<String, usize> {
        let dom = &self.doc.dom;
        let mut c: BTreeMap<String, usize> = BTreeMap::new();
        let mut add = |k: &str, n: usize| {
            if n > 0 {
                *c.entry(k.to_string()).or_insert(0) += n;
            }
        };
        let comments = self.comments();
        add("comments", comments.len());
        add("replies", comments.iter().filter(|x| x.parent.is_some()).count());
        add("revisions", self.change_list().len());
        add("hidden", self.hidden_runs().len());
        add("invisible", self.invisible_objects().len());
        let hf = self.doc.sections.iter().filter(|s| !s.hf_rids.is_empty()).count();
        add("hf", hf);
        if self.has_watermark() {
            add("watermark", 1);
        }
        for n in self.all_story_nodes() {
            match dom.name(n) {
                "m:oMath" => add("equations", 1),
                "w:sdt" => add("controls", 1),
                "a:glow" | "a:softEdge" | "a:reflection" => add("effects", 1),
                "w14:ligatures" | "w14:numForm" | "w14:numSpacing" | "w14:stylisticSets" | "w14:cntxtAlts" => add("typography", 1),
                "w14:glow" | "w14:shadow" | "w14:reflection" | "w14:textOutline" | "w14:textFill" | "a:prstTxWarp" => add("wordart", 1),
                "wps:linkedTxbx" => add("textlinks", 1),
                "w:rPrChange" | "w:pPrChange" | "w:sectPrChange" | "w:tblPrChange" => add("tracked_fmt", 1),
                "w:bookmarkStart" => {
                    if dom.attr(n, "w:name").map_or(false, |s| s.chars().count() > 40) {
                        add("bookmarks", 1);
                    }
                }
                _ => {}
            }
        }
        c
    }

    /// Remove All for one of the inspector's categories ("hf", "hidden",
    /// "invisible"); comments, revisions and properties have their own calls.
    pub fn inspect_remove(&mut self, key: &str) -> usize {
        let label = match key {
            "hf" => "Remove Headers and Footers",
            "hidden" => "Remove Hidden Text",
            "invisible" => "Remove Invisible Content",
            _ => return 0,
        };
        let nodes: Vec<NodeId> = match key {
            "hidden" => self.hidden_runs(),
            "invisible" => self.invisible_objects(),
            _ => {
                let dom = &self.doc.dom;
                let mut out = Vec::new();
                for s in &self.doc.sections {
                    if let Some(sp) = s.node {
                        out.extend(dom.elements(sp).filter(|&k| dom.is(k, "w:headerReference") || dom.is(k, "w:footerReference")));
                    }
                }
                out
            }
        };
        if nodes.is_empty() {
            return 0;
        }
        self.begin();
        for &n in &nodes {
            self.doc.dom.detach(n);
        }
        if key == "hf" {
            self.edit_header_footer(None, false);
        }
        let p = self.doc_start();
        self.caret = p;
        self.anchor = p;
        self.commit(label, false);
        self.relayout();
        nodes.len()
    }
}
