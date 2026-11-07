//! Spelling: the words on the pages in view, checked against the system's
//! dictionaries and the person's own list.

use folio_page_editor::Editor;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Spell {
    dict: Option<spellbook::Dictionary>,
    /// what has been decided about a word already
    seen: HashMap<String, bool>,
    /// words the person added or told it to ignore
    mine: Vec<String>,
    pub on: bool,
}

impl Default for Spell {
    fn default() -> Self {
        Spell { dict: None, seen: HashMap::new(), mine: Vec::new(), on: true }
    }
}

/// Where a dictionary might live. The first pair that is there is used.
fn dict_paths() -> Vec<(PathBuf, PathBuf)> {
    let mut v = Vec::new();
    let mut langs = vec![];
    if let Some(l) = folio_kit::platform::setting("FOLIO_PAGE_DICT") {
        langs.push(l);
    }
    for l in ["en_US", "en_GB", "en-US", "en"] {
        langs.push(l.to_string());
    }
    for dir in ["/usr/share/hunspell", "/usr/share/myspell", "/usr/share/myspell/dicts", "/usr/local/share/hunspell"] {
        for l in &langs {
            let aff = PathBuf::from(dir).join(format!("{l}.aff"));
            let dic = PathBuf::from(dir).join(format!("{l}.dic"));
            if aff.is_file() && dic.is_file() {
                v.push((aff, dic));
            }
        }
    }
    v
}

fn user_words_file() -> PathBuf {
    folio_kit::platform::config_dir("folio-page").join("dictionary.txt")
}

impl Spell {
    /// Load the dictionary, once, when it is first needed.
    pub fn ready(&mut self) -> bool {
        if self.dict.is_some() {
            return true;
        }
        self.mine = std::fs::read_to_string(user_words_file())
            .map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
            .unwrap_or_default();
        for (aff, dic) in dict_paths() {
            let (Ok(a), Ok(d)) = (std::fs::read_to_string(&aff), std::fs::read_to_string(&dic)) else { continue };
            if let Ok(dict) = spellbook::Dictionary::new(&a, &d) {
                self.dict = Some(dict);
                return true;
            }
        }
        false
    }

    pub fn known(&mut self, word: &str) -> bool {
        if word.chars().any(|c| c.is_ascii_digit()) || word.chars().count() < 2 {
            return true;
        }
        if let Some(v) = self.seen.get(word) {
            return *v;
        }
        let ok = self.mine.iter().any(|w| w == word)
            || self.dict.as_ref().map(|d| d.check(word)).unwrap_or(true);
        self.seen.insert(word.to_string(), ok);
        ok
    }

    /// Other spellings for a word, best first.
    pub fn suggestions(&self, word: &str) -> Vec<String> {
        let Some(d) = self.dict.as_ref() else { return Vec::new() };
        let mut out = Vec::new();
        d.suggest(word, &mut out);
        out.truncate(6);
        out
    }

    /// Keep a word: it goes in the person's own list and stays known.
    pub fn add_word(&mut self, word: &str) {
        let word = word.trim().to_string();
        if word.is_empty() || self.mine.iter().any(|w| *w == word) {
            return;
        }
        self.mine.push(word.clone());
        self.seen.insert(word.clone(), true);
        let f = user_words_file();
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let text = format!("{}\n", self.mine.join("\n"));
        let _ = folio_kit::platform::atomic_write(&f, text.as_bytes());
    }

    /// Ignore a word for this session only.
    pub fn ignore(&mut self, word: &str) {
        self.seen.insert(word.trim().to_string(), true);
    }

    pub fn forget_results(&mut self) {
        self.seen.clear();
    }
}

/// The words of a piece of text, with where each one starts and ends.
pub fn words(text: &str) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_alphabetic() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len()
            && (chars[i].is_alphabetic()
                || ((chars[i] == '\'' || chars[i] == '\u{2019}')
                    && i + 1 < chars.len()
                    && chars[i + 1].is_alphabetic()))
        {
            i += 1;
        }
        out.push((start, i, chars[start..i].iter().collect()));
    }
    out
}

/// The misspelled words on one page: where each sits, for the wavy line.
pub fn page_squiggles(ed: &mut Editor, spell: &mut Spell, page: usize) -> Vec<(f64, f64, f64)> {
    if !spell.on || !spell.ready() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let paras: Vec<folio_page_editor::eng::xml::NodeId> = ed
        .layout
        .pages
        .get(page)
        .map(|p| p.lines.iter().map(|l| ed.layout.paras[l.para].node).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut done: Vec<folio_page_editor::eng::xml::NodeId> = Vec::new();
    for p in paras {
        if done.contains(&p) {
            continue;
        }
        done.push(p);
        let text = ed.para_text_of(p);
        for (a, b, word) in words(&text) {
            if spell.known(&word) {
                continue;
            }
            let from = folio_page_editor::Pos { para: p, off: a };
            let to = folio_page_editor::Pos { para: p, off: b };
            let (Some((pa, xa, ya, ha)), Some((pb, xb, _, _))) = (ed.caret_rect(from), ed.caret_rect(to)) else {
                continue;
            };
            // a word broken across lines is left alone
            if pa != page || pb != pa || xb <= xa {
                continue;
            }
            out.push((xa, ya + ha, xb - xa));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_come_out_with_their_places() {
        let w = words("Hello, don't go — ok?");
        let got: Vec<&str> = w.iter().map(|(_, _, s)| s.as_str()).collect();
        assert_eq!(got, vec!["Hello", "don't", "go", "ok"]);
        assert_eq!(w[0].0, 0);
        assert_eq!(w[0].1, 5);
    }
}
