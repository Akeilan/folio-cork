//! What a window asks the editor: what the ribbon should show, and the few
//! commands that are one step for the person but several here.

use crate::eng;
use crate::{Editor, PropEdit};

impl Editor {
    /// Every font family the machine has, for the font box.
    pub fn families(&self) -> Vec<String> {
        self.fonts.families()
    }

    /// The paragraph styles a document offers, the common ones first.
    pub fn style_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .doc
            .styles
            .styles
            .values()
            .filter(|s| s.kind == eng::styles::StyleKind::Paragraph)
            .map(|s| s.name.clone())
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
            .find(|s| s.name == name && s.kind == eng::styles::StyleKind::Paragraph)
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

    /// All the body text, paragraph by paragraph: what a reader would see.
    pub fn text_of_document(&mut self) -> String {
        let paras: Vec<_> = self.body_blocks().iter().filter(|(k, _, _)| *k == "p").map(|(_, n, _)| *n).collect();
        paras.into_iter().map(|p| self.para_text_of(p)).collect::<Vec<_>>().join("\n")
    }

    /// Set or clear one run property across the selection.
    pub fn run_prop(&mut self, name: &str, attrs: Option<&[(&str, &str)]>, label: &str) {
        match attrs {
            Some(a) => self.format_runs(&[PropEdit::set(name, a)], label),
            None => self.format_runs(&[PropEdit::remove(name)], label),
        }
    }
}
