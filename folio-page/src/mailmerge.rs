//! Mail merge: a list of people in a file, and a document that says the same
//! thing to each of them.

use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Merge {
    pub path: Option<PathBuf>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// which row the document is showing, when it is showing one
    pub preview: Option<usize>,
}

impl Merge {
    /// Read a list from a comma- or tab-separated file. The first line names
    /// the columns.
    pub fn open(path: &Path) -> Result<Merge, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let sep = if text.lines().next().map(|l| l.contains('\t')).unwrap_or(false) { '\t' } else { ',' };
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let head = lines.next().ok_or("the file has nothing in it")?;
        let headers: Vec<String> = split(head, sep);
        if headers.is_empty() {
            return Err("no columns in the first line".into());
        }
        let rows: Vec<Vec<String>> = lines.map(|l| split(l, sep)).collect();
        if rows.is_empty() {
            return Err("the file names its columns but has no rows".into());
        }
        Ok(Merge { path: Some(path.to_path_buf()), headers, rows, preview: None })
    }

    pub fn is_open(&self) -> bool {
        !self.rows.is_empty()
    }

    /// One row's value for a column, by the column's name.
    pub fn value(&self, row: usize, column: &str) -> Option<String> {
        let i = self.headers.iter().position(|h| h.eq_ignore_ascii_case(column))?;
        self.rows.get(row).and_then(|r| r.get(i)).cloned()
    }

    /// The name a merged document takes: the stem, then the row's number.
    pub fn out_name(&self, stem: &str, row: usize) -> String {
        format!("{stem}-{}.docx", row + 1)
    }
}

/// One line of a separated file, with "quoted, fields" kept whole.
fn split(line: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    for f in out.iter_mut() {
        *f = f.trim().to_string();
    }
    while out.last().map(|f| f.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out
}

/// The column a MERGEFIELD field code names.
pub fn field_column(code: &str) -> Option<String> {
    let code = code.trim();
    let rest = code.strip_prefix("MERGEFIELD").or_else(|| code.strip_prefix("mergefield"))?;
    let name: String = rest.trim().split_whitespace().next().unwrap_or("").trim_matches('"').to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_fields_stay_whole() {
        assert_eq!(split("a,\"b,c\",d", ','), vec!["a", "b,c", "d"]);
        assert_eq!(split("one\ttwo", '\t'), vec!["one", "two"]);
    }

    #[test]
    fn a_merge_field_names_its_column() {
        assert_eq!(field_column("MERGEFIELD Name \\* MERGEFORMAT").as_deref(), Some("Name"));
        assert_eq!(field_column("PAGE"), None);
    }
}
