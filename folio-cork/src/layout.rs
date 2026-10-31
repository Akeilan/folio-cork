//! Automatic layout (K3-06): flowcharts in layers, trees and org charts, mind maps; and org
//! charts built from a CSV file or spreadsheet.

use crate::editor::Editor;
use crate::model::*;
use folio_kit::text::Text;
use folio_kit::Color;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

struct Graph {
    nodes: Vec<Id>,
    edges: Vec<(Id, Id, Id)>,
}

/// Shapes and the connectors between them: the selection, or the whole page.
fn graph(ed: &Editor) -> Graph {
    let page = ed.pg();
    let pick: HashSet<Id> = if ed.sel.len() > 1 { ed.sel_deep().into_iter().collect() } else { page.shapes.iter().map(|s| s.id).collect() };
    let mut nodes = vec![];
    let mut edges = vec![];
    for s in &page.shapes {
        if !pick.contains(&s.id) || s.group.is_some() {
            continue;
        }
        if let Some(c) = &s.conn {
            if let (Some(a), Some(b)) = (c.a.shape, c.b.shape) {
                edges.push((page.top_group(a), page.top_group(b), s.id));
            }
        } else if !matches!(s.kind.as_str(), "frame" | "ink" | "text") || page.shapes.iter().any(|c| c.conn.as_ref().map(|c| c.a.shape == Some(s.id) || c.b.shape == Some(s.id)).unwrap_or(false)) {
            nodes.push(s.id);
        }
    }
    let set: HashSet<Id> = nodes.iter().copied().collect();
    edges.retain(|(a, b, _)| set.contains(a) && set.contains(b) && a != b);
    // Only shapes that take part in the diagram move.
    let linked: HashSet<Id> = edges.iter().flat_map(|e| [e.0, e.1]).collect();
    nodes.retain(|n| linked.contains(n));
    Graph { nodes, edges }
}

pub fn relayout(ed: &mut Editor, how: &str, _text: &mut Text) {
    let g = graph(ed);
    if g.nodes.len() < 2 {
        ed.status = "Connect some shapes first.".into();
        return;
    }
    ed.end_edit();
    ed.checkpoint("Re-layout", "");
    let origin = {
        let page = ed.pg();
        let mut x = f32::MAX;
        let mut y = f32::MAX;
        for n in &g.nodes {
            if let Some(s) = page.get(*n) {
                x = x.min(s.x);
                y = y.min(s.y);
            }
        }
        (x, y)
    };
    match how {
        "down" | "right" => layered(ed, &g, how == "right", origin),
        "tree" => tree(ed, &g, origin, false),
        "mind" => mind(ed, &g),
        _ => {}
    }
    let route = if how == "mind" { Route::Curved } else { Route::Elbow };
    for (_, _, cid) in &g.edges {
        if let Some(s) = ed.pgm().get_mut(*cid) {
            if let Some(c) = s.conn.as_mut() {
                c.route = route;
                // Trees join bottom to top; other layouts pick the facing sides.
                c.a.point = if how == "tree" { Some(2) } else { None };
                c.b.point = if how == "tree" { Some(0) } else { None };
            }
        }
    }
    ed.changed();
}

fn size_of(ed: &Editor, id: Id) -> (f32, f32) {
    ed.pg().get(id).map(|s| (s.w, s.h)).unwrap_or((100.0, 60.0))
}

fn move_to(ed: &mut Editor, id: Id, x: f32, y: f32) {
    let (ox, oy) = ed.pg().get(id).map(|s| (s.x, s.y)).unwrap_or((x, y));
    ed.shift(id, x - ox, y - oy);
}

/// Layers by longest path from the sources, ordered to reduce crossings.
fn layered(ed: &mut Editor, g: &Graph, horizontal: bool, origin: (f32, f32)) {
    let mut out: HashMap<Id, Vec<Id>> = HashMap::new();
    let mut indeg: HashMap<Id, usize> = g.nodes.iter().map(|n| (*n, 0)).collect();
    // Drop edges that close cycles (found by DFS order).
    let mut order: HashMap<Id, usize> = HashMap::new();
    let mut visited = HashSet::new();
    let adj: HashMap<Id, Vec<Id>> = g.nodes.iter().map(|n| (*n, g.edges.iter().filter(|e| e.0 == *n).map(|e| e.1).collect())).collect();
    let mut onstack = HashSet::new();
    let mut back: HashSet<(Id, Id)> = HashSet::new();
    fn dfs(n: Id, adj: &HashMap<Id, Vec<Id>>, visited: &mut HashSet<Id>, onstack: &mut HashSet<Id>, back: &mut HashSet<(Id, Id)>, order: &mut HashMap<Id, usize>) {
        visited.insert(n);
        onstack.insert(n);
        let k = order.len();
        order.insert(n, k);
        for &m in adj.get(&n).map(|v| v.as_slice()).unwrap_or(&[]) {
            if onstack.contains(&m) {
                back.insert((n, m));
            } else if !visited.contains(&m) {
                dfs(m, adj, visited, onstack, back, order);
            }
        }
        onstack.remove(&n);
    }
    let roots: Vec<Id> = g.nodes.iter().copied().filter(|n| !g.edges.iter().any(|e| e.1 == *n)).collect();
    for r in roots.iter().chain(g.nodes.iter()) {
        if !visited.contains(r) {
            dfs(*r, &adj, &mut visited, &mut onstack, &mut back, &mut order);
        }
    }
    for (a, b, _) in &g.edges {
        if back.contains(&(*a, *b)) {
            continue;
        }
        out.entry(*a).or_default().push(*b);
        *indeg.get_mut(b).unwrap() += 1;
    }
    let mut layer: HashMap<Id, usize> = HashMap::new();
    let mut q: VecDeque<Id> = g.nodes.iter().copied().filter(|n| indeg[n] == 0).collect();
    let mut deg = indeg.clone();
    while let Some(n) = q.pop_front() {
        let l = *layer.entry(n).or_insert(0);
        for &m in out.get(&n).map(|v| v.as_slice()).unwrap_or(&[]) {
            let e = layer.entry(m).or_insert(0);
            *e = (*e).max(l + 1);
            let d = deg.get_mut(&m).unwrap();
            *d -= 1;
            if *d == 0 {
                q.push_back(m);
            }
        }
    }
    let nl = layer.values().copied().max().unwrap_or(0) + 1;
    let mut layers: Vec<Vec<Id>> = vec![vec![]; nl];
    let mut nodes = g.nodes.clone();
    nodes.sort_by_key(|n| order.get(n).copied().unwrap_or(0));
    for n in nodes {
        layers[*layer.get(&n).unwrap_or(&0)].push(n);
    }
    // Barycentre sweeps.
    for _ in 0..4 {
        for li in 1..nl {
            let prev: HashMap<Id, f32> = layers[li - 1].iter().enumerate().map(|(i, n)| (*n, i as f32)).collect();
            let mut keyed: Vec<(f32, Id)> = layers[li]
                .iter()
                .map(|n| {
                    let ps: Vec<f32> = g.edges.iter().filter(|e| e.1 == *n).filter_map(|e| prev.get(&e.0).copied()).collect();
                    (if ps.is_empty() { f32::MAX } else { ps.iter().sum::<f32>() / ps.len() as f32 }, *n)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            layers[li] = keyed.into_iter().map(|x| x.1).collect();
        }
    }
    let (gap_main, gap_cross) = (70.0, 50.0);
    let widest: f32 = layers
        .iter()
        .map(|l| l.iter().map(|n| { let (w, h) = size_of(ed, *n); if horizontal { h } else { w } }).sum::<f32>() + gap_cross * (l.len().saturating_sub(1)) as f32)
        .fold(0.0, f32::max);
    let mut main = 0.0;
    for l in &layers {
        let thick = l.iter().map(|n| { let (w, h) = size_of(ed, *n); if horizontal { w } else { h } }).fold(0.0, f32::max);
        let total: f32 = l.iter().map(|n| { let (w, h) = size_of(ed, *n); if horizontal { h } else { w } }).sum::<f32>() + gap_cross * (l.len().saturating_sub(1)) as f32;
        let mut cross = (widest - total) / 2.0;
        for &n in l {
            let (w, h) = size_of(ed, n);
            if horizontal {
                move_to(ed, n, origin.0 + main + (thick - w) / 2.0, origin.1 + cross);
                cross += h + gap_cross;
            } else {
                move_to(ed, n, origin.0 + cross, origin.1 + main + (thick - h) / 2.0);
                cross += w + gap_cross;
            }
        }
        main += thick + gap_main;
    }
}

/// Tidy tree: children under their parent, parents centred over children.
fn tree(ed: &mut Editor, g: &Graph, origin: (f32, f32), _unused: bool) {
    let mut kids: HashMap<Id, Vec<Id>> = HashMap::new();
    let mut has_parent = HashSet::new();
    let mut seen_edge = HashSet::new();
    for (a, b, _) in &g.edges {
        if has_parent.contains(b) || !seen_edge.insert((*a, *b)) {
            continue;
        }
        kids.entry(*a).or_default().push(*b);
        has_parent.insert(*b);
    }
    // Children in left-to-right order of where they are now.
    for v in kids.values_mut() {
        v.sort_by(|a, b| {
            let xa = ed.pg().get(*a).map(|s| s.x).unwrap_or(0.0);
            let xb = ed.pg().get(*b).map(|s| s.x).unwrap_or(0.0);
            xa.partial_cmp(&xb).unwrap()
        });
    }
    let roots: Vec<Id> = g.nodes.iter().copied().filter(|n| !has_parent.contains(n)).collect();
    let (hgap, vgap) = (36.0, 70.0);
    fn width(ed: &Editor, n: Id, kids: &HashMap<Id, Vec<Id>>, hgap: f32, depth: usize, memo: &mut HashMap<Id, f32>) -> f32 {
        if let Some(w) = memo.get(&n) {
            return *w;
        }
        let own = ed.pg().get(n).map(|s| s.w).unwrap_or(100.0);
        let cw: f32 = if depth > 60 { 0.0 } else {
            let k = kids.get(&n).cloned().unwrap_or_default();
            let sum: f32 = k.iter().map(|c| width(ed, *c, kids, hgap, depth + 1, memo)).sum();
            sum + hgap * (k.len().saturating_sub(1)) as f32
        };
        let w = own.max(cw);
        memo.insert(n, w);
        w
    }
    let mut memo = HashMap::new();
    let mut row_h: HashMap<usize, f32> = HashMap::new();
    fn depths(n: Id, d: usize, kids: &HashMap<Id, Vec<Id>>, ed: &Editor, row_h: &mut HashMap<usize, f32>, seen: &mut HashSet<Id>) {
        if !seen.insert(n) {
            return;
        }
        let h = ed.pg().get(n).map(|s| s.h).unwrap_or(60.0);
        let e = row_h.entry(d).or_insert(0.0);
        *e = e.max(h);
        for c in kids.get(&n).cloned().unwrap_or_default() {
            depths(c, d + 1, kids, ed, row_h, seen);
        }
    }
    let mut seen = HashSet::new();
    for r in &roots {
        depths(*r, 0, &kids, ed, &mut row_h, &mut seen);
    }
    let mut ys = vec![origin.1];
    for d in 0..row_h.len() {
        let last = ys[d];
        ys.push(last + row_h.get(&d).copied().unwrap_or(60.0) + vgap);
    }
    #[allow(clippy::too_many_arguments)]
    fn place(ed: &mut Editor, n: Id, x: f32, d: usize, kids: &HashMap<Id, Vec<Id>>, memo: &mut HashMap<Id, f32>, hgap: f32, ys: &[f32], seen: &mut HashSet<Id>) {
        if !seen.insert(n) {
            return;
        }
        let w = width(ed, n, kids, hgap, 0, memo);
        let own = ed.pg().get(n).map(|s| s.w).unwrap_or(100.0);
        move_to(ed, n, x + (w - own) / 2.0, ys[d.min(ys.len() - 1)]);
        let k = kids.get(&n).cloned().unwrap_or_default();
        let total: f32 = k.iter().map(|c| width(ed, *c, kids, hgap, 0, memo)).sum::<f32>() + hgap * (k.len().saturating_sub(1)) as f32;
        let mut cx = x + (w - total) / 2.0;
        for c in k {
            let cw = width(ed, c, kids, hgap, 0, memo);
            place(ed, c, cx, d + 1, kids, memo, hgap, ys, seen);
            cx += cw + hgap;
        }
    }
    let mut x = origin.0;
    let mut seen = HashSet::new();
    for r in roots {
        let w = width(ed, r, &kids, hgap, 0, &mut memo);
        place(ed, r, x, 0, &kids, &mut memo, hgap, &ys, &mut seen);
        x += w + hgap * 2.0;
    }
}

/// Mind map: the most connected shape in the middle, branches alternating right and left.
fn mind(ed: &mut Editor, g: &Graph) {
    let mut nb: HashMap<Id, Vec<Id>> = HashMap::new();
    for (a, b, _) in &g.edges {
        nb.entry(*a).or_default().push(*b);
        nb.entry(*b).or_default().push(*a);
    }
    let root = ed.sel.iter().copied().find(|s| g.nodes.contains(s)).filter(|_| ed.sel.len() == 1).unwrap_or_else(|| *g.nodes.iter().max_by_key(|n| nb.get(n).map(|v| v.len()).unwrap_or(0)).unwrap());
    let (rcx, rcy) = ed.pg().get(root).map(|s| (s.cx(), s.cy())).unwrap();
    // Breadth-first tree from the root.
    let mut kids: HashMap<Id, Vec<Id>> = HashMap::new();
    let mut seen = HashSet::from([root]);
    let mut q = VecDeque::from([root]);
    while let Some(n) = q.pop_front() {
        for &m in nb.get(&n).map(|v| v.as_slice()).unwrap_or(&[]) {
            if seen.insert(m) {
                kids.entry(n).or_default().push(m);
                q.push_back(m);
            }
        }
    }
    fn height(ed: &Editor, n: Id, kids: &HashMap<Id, Vec<Id>>) -> f32 {
        let own = ed.pg().get(n).map(|s| s.h).unwrap_or(50.0);
        let k = kids.get(&n).cloned().unwrap_or_default();
        let sum: f32 = k.iter().map(|c| height(ed, *c, kids)).sum::<f32>() + 18.0 * (k.len().saturating_sub(1)) as f32;
        own.max(sum)
    }
    fn place(ed: &mut Editor, n: Id, x_edge: f32, cy: f32, dir: f32, kids: &HashMap<Id, Vec<Id>>) {
        let (w, h) = ed.pg().get(n).map(|s| (s.w, s.h)).unwrap_or((100.0, 50.0));
        let x = if dir > 0.0 { x_edge } else { x_edge - w };
        move_to(ed, n, x, cy - h / 2.0);
        let k = kids.get(&n).cloned().unwrap_or_default();
        let total: f32 = k.iter().map(|c| height(ed, *c, kids)).sum::<f32>() + 18.0 * (k.len().saturating_sub(1)) as f32;
        let mut y = cy - total / 2.0;
        let next_edge = if dir > 0.0 { x + w + 80.0 } else { x - 80.0 };
        for c in k {
            let ch = height(ed, c, kids);
            place(ed, c, next_edge, y + ch / 2.0, dir, kids);
            y += ch + 18.0;
        }
    }
    let first = kids.get(&root).cloned().unwrap_or_default();
    let (right, left): (Vec<(usize, Id)>, Vec<(usize, Id)>) = first.into_iter().enumerate().partition(|(i, _)| i % 2 == 0);
    let (rw, _) = ed.pg().get(root).map(|s| (s.w, s.h)).unwrap();
    for (side, list) in [(1.0f32, right), (-1.0f32, left)] {
        let total: f32 = list.iter().map(|(_, c)| height(ed, *c, &kids)).sum::<f32>() + 30.0 * (list.len().saturating_sub(1)) as f32;
        let mut y = rcy - total / 2.0;
        let edge = rcx + side * (rw / 2.0 + 100.0);
        for (_, c) in list {
            let ch = height(ed, c, &kids);
            place(ed, c, edge, y + ch / 2.0, side, &kids);
            y += ch + 30.0;
        }
    }
}

/// Read rows from a CSV file or the first sheet of a spreadsheet.
pub fn read_rows(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext == "xlsx" || ext == "xls" || ext == "ods" {
        use calamine::{open_workbook_auto, Reader};
        let mut wb = open_workbook_auto(path).map_err(|e| format!("Can't read the spreadsheet: {e}"))?;
        let name = wb.sheet_names().first().cloned().ok_or("The spreadsheet has no sheets.")?;
        let range = wb.worksheet_range(&name).map_err(|e| e.to_string())?;
        return Ok(range.rows().map(|r| r.iter().map(|c| c.to_string()).collect()).collect());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes).trim_start_matches('\u{feff}').to_string();
    let delim = if text.lines().next().map(|l| l.matches(';').count() > l.matches(',').count()).unwrap_or(false) { ';' } else { ',' };
    Ok(parse_csv(&text, delim))
}

pub fn parse_csv(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows = vec![];
    let mut row = vec![];
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == delim {
            row.push(std::mem::take(&mut cell).trim().to_string());
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell).trim().to_string());
            if row.iter().any(|x| !x.is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            cell.push(c);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell.trim().to_string());
        if row.iter().any(|x| !x.is_empty()) {
            rows.push(row);
        }
    }
    rows
}

/// Build an org chart from rows with Name, Title and Reports To columns. Returns people added.
pub fn org_from_file(ed: &mut Editor, path: &Path, text: &mut Text) -> Result<usize, String> {
    let rows = read_rows(path)?;
    org_from_rows(ed, &rows, text)
}

pub fn org_from_rows(ed: &mut Editor, rows: &[Vec<String>], text: &mut Text) -> Result<usize, String> {
    let head: Vec<String> = rows.first().ok_or("The file is empty.")?.iter().map(|h| h.to_lowercase()).collect();
    let find = |keys: &[&str]| head.iter().position(|h| keys.iter().any(|k| h.contains(k)));
    let ni = find(&["name", "naam", "employee", "person"]).ok_or("No Name column found. The first row should have Name, Title and Reports To.")?;
    let ti = find(&["title", "role", "position", "job", "functie"]);
    let mi = find(&["report", "manager", "supervisor", "parent", "boss"]).ok_or("No Reports To column found.")?;
    let people: Vec<(String, String, String)> = rows[1..]
        .iter()
        .filter(|r| r.get(ni).map(|x| !x.trim().is_empty()).unwrap_or(false))
        .map(|r| (r[ni].trim().to_string(), ti.and_then(|i| r.get(i)).cloned().unwrap_or_default(), r.get(mi).cloned().unwrap_or_default().trim().to_string()))
        .collect();
    if people.is_empty() {
        return Err("No people found under the header row.".into());
    }
    ed.end_edit();
    ed.checkpoint("Org chart", "");
    let ox = crate::render::content_bounds(ed.pg()).map(|b| b.0 + b.2 + 120.0).unwrap_or(48.0);
    let mut ids: HashMap<String, Id> = HashMap::new();
    let has_reports: HashSet<String> = people.iter().map(|p| p.2.to_lowercase()).collect();
    for (name, title, _) in &people {
        let kind = if has_reports.contains(&name.to_lowercase()) { "org.manager" } else { "org.person" };
        let mut s = ed.make_shape(kind, 0.0, 0.0);
        s.x = ox;
        s.y = 48.0;
        s.text = if title.is_empty() { name.clone() } else { format!("{name}\n{title}") };
        s.data.insert("name".into(), name.clone());
        s.data.insert("title".into(), title.clone());
        ids.insert(name.to_lowercase(), s.id);
        ed.pgm().shapes.push(s);
    }
    let mut new = vec![];
    for (name, _, mgr) in &people {
        if let (Some(&a), Some(&b)) = (ids.get(&mgr.to_lowercase()), ids.get(&name.to_lowercase())) {
            if a == b {
                continue;
            }
            let id = ed.doc.new_id();
            let mut c = Shape { id, kind: "connector".into(), layer: ed.active_layer(), ..Default::default() };
            c.style.line = Color::hex(0x595959);
            c.style.width = 1.5;
            c.conn = Some(Conn { a: End::glued(a, None), b: End::glued(b, None), route: Route::Elbow, arrow_a: Arrow::None, arrow_b: Arrow::None, jumps: false });
            ed.pgm().shapes.push(c);
            new.push(id);
        }
    }
    let all: Vec<Id> = ids.values().copied().chain(new).collect();
    ed.sel = all;
    let g = graph(ed);
    tree(ed, &g, (ox, 48.0), false);
    for s in ed.pgm().shapes.iter_mut() {
        if let Some(c) = s.conn.as_mut() {
            if ids.values().any(|v| Some(*v) == c.a.shape) {
                c.a.point = Some(2);
                c.b.point = Some(0);
            }
        }
    }
    let _ = text;
    ed.sel = ids.values().copied().collect();
    ed.changed();
    Ok(people.len())
}
