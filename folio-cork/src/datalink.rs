//! Shape data from a spreadsheet (K4-03): a column says which shape a row belongs to, the other
//! columns become that shape's properties, and Refresh reads the file again.

use crate::editor::Editor;
use crate::layout::read_rows;
use crate::model::*;
use std::path::Path;

/// What one pass over a file did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Shapes that were given data.
    pub filled: usize,
    /// Rows of the file that matched no shape.
    pub unmatched: usize,
    /// Columns taken from the file, the key column apart.
    pub columns: usize,
}

/// The header row of a file, for the box that asks which column is the key.
pub fn columns(path: &Path) -> Result<Vec<String>, String> {
    let rows = read_rows(path)?;
    let head = rows.first().ok_or("The file is empty.")?;
    let cols: Vec<String> = head.iter().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect();
    if cols.is_empty() {
        return Err("The first row of the file should name the columns.".into());
    }
    Ok(cols)
}

/// What a shape is matched on: its text, or one of its properties.
fn key_of(s: &Shape, match_on: &str) -> String {
    let raw = if match_on.is_empty() { s.text.lines().next().unwrap_or("").to_string() } else { s.prop(match_on).to_string() };
    raw.trim().to_lowercase()
}

/// Fill shape data from a file, and remember it so Refresh can read it again.
pub fn link(ed: &mut Editor, path: &Path, key: &str, match_on: &str) -> Result<Report, String> {
    let report = fill(ed, path, key, match_on, true)?;
    let link = DataLink { path: path.to_string_lossy().to_string(), key: key.to_string(), match_on: match_on.to_string() };
    ed.doc.links.retain(|l| l.path != link.path);
    ed.doc.links.push(link);
    ed.changed();
    Ok(report)
}

/// Read every linked file again. Returns what was filled and the files that could not be read.
pub fn refresh(ed: &mut Editor) -> (Report, Vec<String>) {
    let links = ed.doc.links.clone();
    let mut total = Report::default();
    let mut trouble = vec![];
    for l in &links {
        match fill(ed, Path::new(&l.path), &l.key, &l.match_on, true) {
            Ok(r) => {
                total.filled += r.filled;
                total.unmatched += r.unmatched;
                total.columns = total.columns.max(r.columns);
            }
            Err(e) => trouble.push(format!("{}: {e}", l.path)),
        }
    }
    if total.filled > 0 {
        ed.changed();
    }
    Ok::<(), ()>(()).ok();
    (total, trouble)
}

/// One pass over a file. `checkpoint` puts an undo step in before anything changes.
pub fn fill(ed: &mut Editor, path: &Path, key: &str, match_on: &str, checkpoint: bool) -> Result<Report, String> {
    let rows = read_rows(path)?;
    let head: Vec<String> = rows.first().ok_or("The file is empty.")?.iter().map(|h| h.trim().to_string()).collect();
    let ki = head
        .iter()
        .position(|h| h.eq_ignore_ascii_case(key.trim()))
        .ok_or_else(|| format!("The file has no column called \u{201c}{key}\u{201d}."))?;
    let cols: Vec<(usize, String)> = head.iter().enumerate().filter(|(i, h)| *i != ki && !h.is_empty()).map(|(i, h)| (i, h.clone())).collect();
    if cols.is_empty() {
        return Err("The file has nothing but the key column in it.".into());
    }
    // the rows, by what the key column says
    let mut by_key: Vec<(String, Vec<String>)> = vec![];
    for r in rows.iter().skip(1) {
        let Some(k) = r.get(ki) else { continue };
        let k = k.trim().to_lowercase();
        if k.is_empty() {
            continue;
        }
        by_key.push((k, r.clone()));
    }
    if by_key.is_empty() {
        return Err("No rows under the header row have anything in the key column.".into());
    }
    if checkpoint {
        ed.end_edit();
        ed.checkpoint("Link to data", "");
    }
    let mut used = vec![false; by_key.len()];
    let mut filled = 0;
    for pi in 0..ed.doc.pages.len() {
        for si in 0..ed.doc.pages[pi].shapes.len() {
            let s = &ed.doc.pages[pi].shapes[si];
            if s.is_connector() || s.is_group() {
                continue;
            }
            let k = key_of(s, match_on);
            if k.is_empty() {
                continue;
            }
            let Some(ri) = by_key.iter().position(|(rk, _)| *rk == k) else { continue };
            used[ri] = true;
            let row = by_key[ri].1.clone();
            let s = &mut ed.doc.pages[pi].shapes[si];
            for (i, name) in &cols {
                let v = row.get(*i).map(|x| x.trim()).unwrap_or("");
                s.set_prop(name, v);
            }
            filled += 1;
        }
    }
    if filled == 0 && checkpoint {
        ed.drop_checkpoint();
    } else {
        ed.changed();
    }
    Ok(Report { filled, unmatched: used.iter().filter(|u| !**u).count(), columns: cols.len() })
}

/// Every property name the shapes already carry, for the box that asks what to match on.
pub fn property_names(ed: &Editor) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for p in &ed.doc.pages {
        for s in &p.shapes {
            for (n, _) in s.props() {
                if !out.iter().any(|x| x.eq_ignore_ascii_case(&n)) {
                    out.push(n);
                }
            }
        }
    }
    out.sort_by_key(|n| n.to_lowercase());
    out
}
