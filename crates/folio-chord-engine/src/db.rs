//! The library file: one line per record, fields separated by tabs.
//!
//! Chord keeps its library in its own format rather than a database engine, so
//! nothing under the app comes from the system. The file is small (a few hundred
//! bytes a song) and is written whole, through a temporary file, so a crash
//! half way cannot leave half a library.

use std::collections::BTreeMap;
use std::path::Path;

/// One record: a kind ("track", "playlist"...) and its fields by name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub kind: String,
    pub fields: BTreeMap<String, String>,
}

impl Record {
    pub fn new(kind: &str) -> Record {
        Record { kind: kind.to_string(), fields: BTreeMap::new() }
    }

    pub fn put(&mut self, k: &str, v: impl ToString) -> &mut Record {
        let v = v.to_string();
        if !v.is_empty() {
            self.fields.insert(k.to_string(), v);
        }
        self
    }

    pub fn get(&self, k: &str) -> &str {
        self.fields.get(k).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn num<T: std::str::FromStr + Default>(&self, k: &str) -> T {
        self.get(k).parse().unwrap_or_default()
    }

    pub fn flag(&self, k: &str) -> bool {
        self.get(k) == "1"
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

/// Write the records to a file, through a temporary one beside it.
pub fn write(path: &Path, records: &[Record]) -> Result<(), String> {
    let mut text = String::from("folio-chord library 1\n");
    for r in records {
        text.push_str(&r.kind);
        for (k, v) in &r.fields {
            text.push('\t');
            text.push_str(k);
            text.push('=');
            text.push_str(&escape(v));
        }
        text.push('\n');
    }
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    folio_kit::platform::atomic_write(path, text.as_bytes())
}

/// Read the records back. A file that is not there is an empty library.
pub fn read(path: &Path) -> Vec<Record> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(kind) = parts.next() else { continue };
        let mut r = Record::new(kind);
        for p in parts {
            if let Some((k, v)) = p.split_once('=') {
                r.fields.insert(k.to_string(), unescape(v));
            }
        }
        out.push(r);
    }
    out
}
