//! What a window asks the editor: what the ribbon should show, and the few
//! commands that are one step for the person but several here.

use crate::eng;
use crate::{Editor, PropEdit};

/// A style's name as it is shown: "heading 1" is Word's spelling in the file
/// for the style it calls "Heading 1".
fn shown_name(name: &str) -> String {
    if !name.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false) {
        return name.to_string();
    }
    let mut out = String::with_capacity(name.len());
    let mut start = true;
    for c in name.chars() {
        if start && c.is_ascii_lowercase() {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        start = c == ' ';
    }
    out
}

impl Editor {
    /// Every font family the machine has, for the font box.
    pub fn families(&self) -> Vec<String> {
        self.fonts.families()
    }

    /// The paragraph styles a document offers, the common ones first and
    /// named the way a person reads them: Word keeps "heading 1" in the file
    /// and shows "Heading 1".
    pub fn style_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .doc
            .styles
            .styles
            .values()
            .filter(|s| s.kind == eng::styles::StyleKind::Paragraph)
            .map(|s| shown_name(&s.name))
            .collect();
        v.sort_by_key(|n| {
            let order = ["Normal", "No Spacing", "Heading 1", "Heading 2", "Heading 3", "Title", "Subtitle", "Quote"];
            (order.iter().position(|o| o == n).unwrap_or(order.len()), n.to_lowercase())
        });
        v.dedup();
        v
    }

    /// The words in the document, as the status bar counts them.
    pub fn word_count(&self) -> usize {
        self.stats(false).words
    }

    /// The font family at the caret, as the ribbon shows it.
    pub fn current_font(&self) -> Option<String> {
        let r = self.current_run_props();
        r.font_ascii.clone().or_else(|| r.font_hansi.clone())
    }

    /// Its size in points.
    pub fn current_size(&self) -> Option<f64> {
        self.current_run_props().size.map(|hp| hp as f64 / 2.0)
    }

    /// How the paragraph at the caret is lined up: left, center, right, both.
    pub fn current_align(&self) -> String {
        self.current_para_props().jc.clone().unwrap_or_else(|| "left".into())
    }

    /// Bullets or numbering on the selected paragraphs, off again if they
    /// already have them.
    pub fn list_toggle(&mut self, numbered: bool) -> bool {
        let (a, b) = self.selection();
        let paras = self.paras_between(a.para, b.para);
        let on = paras.iter().any(|p| self.list_kind(*p).is_some());
        if on {
            self.remove_list()
        } else if numbered {
            self.set_list_format("decimal", "%1.", None)
        } else {
            self.set_list_format("bullet", "\u{b7}", Some("Symbol"))
        }
    }

    /// Indent the selected paragraphs one step in or out (half an inch).
    pub fn indent_step(&mut self, dir: i32) {
        let step = 720; // twips
        let cur = self.current_para_props().ind_left.unwrap_or(0);
        let next = (cur + dir * step).max(0);
        let val = next.to_string();
        self.format_paras_attrs("w:ind", &[("w:left", Some(val.as_str()))], "Indent");
    }

    /// Line the selected paragraphs up: left, center, right or both.
    pub fn set_align(&mut self, jc: &str) {
        self.format_paras_attrs("w:jc", &[("w:val", Some(jc))], "Alignment");
    }

    /// Give the selected paragraphs a style, by the name the ribbon shows.
    pub fn apply_style_named(&mut self, name: &str) -> bool {
        let Some(id) = self
            .doc
            .styles
            .styles
            .values()
            .find(|s| s.name.eq_ignore_ascii_case(name) && s.kind == eng::styles::StyleKind::Paragraph)
            .map(|s| s.id.clone())
        else {
            return false;
        };
        let (a, b) = self.selection();
        let paras = self.paras_between_pub(a.para, b.para);
        if paras.is_empty() {
            return false;
        }
        self.begin_group("Style");
        for p in paras {
            self.set_para_style(p, Some(&id));
        }
        self.end_group();
        true
    }

    /// A page's left and right margins, in points.
    pub fn page_margins(&self, page: usize) -> (f64, f64) {
        self.layout.pages.get(page).map(|p| (p.left, p.width - p.right)).map(|(l, r)| (l, r)).unwrap_or((72.0, 540.0))
    }

    /// A page's top and bottom margins, in points.
    pub fn page_vmargins(&self, page: usize) -> (f64, f64) {
        self.layout.pages.get(page).map(|p| (p.top, p.height - p.bottom)).map(|(t, b)| (t, b)).unwrap_or((72.0, 720.0))
    }

    /// All the body text, paragraph by paragraph: what a reader would see.
    pub fn text_of_document(&mut self) -> String {
        let paras: Vec<_> = self.body_blocks().iter().filter(|(k, _, _)| *k == "p").map(|(_, n, _)| *n).collect();
        paras.into_iter().map(|p| self.para_text_of(p)).collect::<Vec<_>>().join("\n")
    }

    /// The document's own properties: title, author, subject, keywords. Read
    /// from the parsed part, so a change made here is seen straight away.
    pub fn doc_properties(&mut self) -> [String; 4] {
        const NAMES: [&str; 4] = ["dc:title", "dc:creator", "dc:subject", "cp:keywords"];
        let mut out = [String::new(), String::new(), String::new(), String::new()];
        let Ok(dom) = self.doc.pkg.xml("docProps/core.xml") else { return out };
        for (i, name) in NAMES.iter().enumerate() {
            let short = name.split(':').last().unwrap_or(name).to_string();
            if let Some(n) = dom.elements(dom.root).find(|&c| dom.name(c) == *name || dom.name(c).ends_with(&format!(":{short}"))) {
                out[i] = dom.text(n);
            }
        }
        out
    }

    /// Write them back into the document.
    pub fn set_doc_properties(&mut self, values: [String; 4]) {
        const NAMES: [&str; 4] = ["dc:title", "dc:creator", "dc:subject", "cp:keywords"];
        if self.doc.pkg.raw("docProps/core.xml").is_none() {
            let head = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<cp:coreProperties                         xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\"                         xmlns:dc=\"http://purl.org/dc/elements/1.1/\"                         xmlns:dcterms=\"http://purl.org/dc/terms/\"                         xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\"                         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"></cp:coreProperties>";
            self.doc.pkg.put_raw("docProps/core.xml", head.as_bytes().to_vec());
        }
        let Ok(dom) = self.doc.pkg.xml_mut("docProps/core.xml") else { return };
        let root = dom.root;
        for (i, name) in NAMES.iter().enumerate() {
            let short = name.split(':').last().unwrap_or(name).to_string();
            let found = dom
                .elements(root)
                .find(|&c| dom.name(c) == *name || dom.name(c).ends_with(&format!(":{short}")));
            let node = match found {
                Some(n) => n,
                None => {
                    let n = dom.new_element(name, &[]);
                    dom.append(root, n);
                    n
                }
            };
            // an element's words live in a text node of its own
            let kids: Vec<_> = dom.children(node).to_vec();
            for k in kids {
                dom.detach(k);
            }
            if !values[i].is_empty() {
                let t = dom.new_text(&values[i]);
                dom.append(node, t);
            }
        }
        self.modified = true;
    }

    /// Set or clear one run property across the selection.
    pub fn run_prop(&mut self, name: &str, attrs: Option<&[(&str, &str)]>, label: &str) {
        match attrs {
            Some(a) => self.format_runs(&[PropEdit::set(name, a)], label),
            None => self.format_runs(&[PropEdit::remove(name)], label),
        }
    }
}
