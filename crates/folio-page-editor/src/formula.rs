//! Formula fields (Table Layout > Formula): = SUM(ABOVE), = A1*B2/2,
//! AVERAGE, COUNT, MAX, MIN, PRODUCT, ABS, INT, ROUND, MOD, cell references,
//! ABOVE / BELOW / LEFT / RIGHT and bookmarks holding numbers.

use folio_math::Det;
use crate::eng;
use super::Editor;
use eng::xml::NodeId;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn tokens(s: &str) -> Option<Vec<Tok>> {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && cs.get(i + 1).map_or(false, |d| d.is_ascii_digit())) {
            let mut t = String::new();
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                t.push(cs[i]);
                i += 1;
            }
            out.push(Tok::Num(t.parse().ok()?));
        } else if c.is_alphabetic() || c == '_' {
            let mut t = String::new();
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == ':') {
                t.push(cs[i]);
                i += 1;
            }
            out.push(Tok::Ident(t.to_ascii_uppercase()));
        } else {
            out.push(match c {
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                ',' | ';' => Tok::Comma,
                '+' | '-' | '*' | '/' | '^' | '%' => Tok::Op(c),
                _ => return None,
            });
            i += 1;
        }
    }
    Some(out)
}

/// "B3" -> (row 2, col 1)
fn cell_ref(s: &str) -> Option<(usize, usize)> {
    let letters: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = &s[letters.len()..];
    if letters.is_empty() || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut col = 0usize;
    for c in letters.chars() {
        col = col * 26 + (c as usize - 'A' as usize + 1);
    }
    let row: usize = digits.parse().ok()?;
    Some((row.checked_sub(1)?, col.checked_sub(1)?))
}

fn number_in(text: &str) -> Option<f64> {
    let t: String = text.trim().chars().filter(|c| !matches!(c, ',' | '$' | '€' | '£' | '%' | ' ')).collect();
    if t.is_empty() {
        return None;
    }
    let t = if t.starts_with('(') && t.ends_with(')') { format!("-{}", &t[1..t.len() - 1]) } else { t };
    t.parse().ok()
}

struct Ctx {
    /// the table's cells' text, rows of cells
    grid: Vec<Vec<String>>,
    /// where the field is: (row, col)
    here: Option<(usize, usize)>,
    marks: std::collections::HashMap<String, f64>,
}

impl Ctx {
    fn cell(&self, r: usize, c: usize) -> Option<f64> {
        self.grid.get(r).and_then(|row| row.get(c)).and_then(|t| number_in(t))
    }

    fn direction(&self, d: &str) -> Vec<f64> {
        let Some((r, c)) = self.here else { return Vec::new() };
        let mut out = Vec::new();
        let mut started = false;
        let mut step = |rr: usize, cc: usize, out: &mut Vec<f64>| -> bool {
            match self.cell(rr, cc) {
                Some(v) => {
                    out.push(v);
                    started = true;
                    true
                }
                // blank cells before the first number are passed over; text ends it
                None => !started && self.grid.get(rr).and_then(|x| x.get(cc)).map_or(true, |t| t.trim().is_empty()),
            }
        };
        match d {
            "ABOVE" => {
                for rr in (0..r).rev() {
                    if !step(rr, c, &mut out) {
                        break;
                    }
                }
            }
            "BELOW" => {
                for rr in r + 1..self.grid.len() {
                    if !step(rr, c, &mut out) {
                        break;
                    }
                }
            }
            "LEFT" => {
                for cc in (0..c).rev() {
                    if !step(r, cc, &mut out) {
                        break;
                    }
                }
            }
            "RIGHT" => {
                let n = self.grid.get(r).map_or(0, |x| x.len());
                for cc in c + 1..n {
                    if !step(r, cc, &mut out) {
                        break;
                    }
                }
            }
            _ => {}
        }
        out
    }

    fn range(&self, a: (usize, usize), b: (usize, usize)) -> Vec<f64> {
        let mut out = Vec::new();
        for r in a.0.min(b.0)..=a.0.max(b.0) {
            for c in a.1.min(b.1)..=a.1.max(b.1) {
                if let Some(v) = self.cell(r, c) {
                    out.push(v);
                }
            }
        }
        out
    }
}

struct Parser<'a> {
    t: Vec<Tok>,
    i: usize,
    ctx: &'a Ctx,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        while let Some(Tok::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.i += 1;
            let r = self.term()?;
            v = if op == '+' { v + r } else { v - r };
        }
        Some(v)
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.power()?;
        while let Some(Tok::Op(op @ ('*' | '/'))) = self.peek().cloned() {
            self.i += 1;
            let r = self.power()?;
            v = if op == '*' { v * r } else if r == 0.0 { return None } else { v / r };
        }
        Some(v)
    }

    fn power(&mut self) -> Option<f64> {
        let v = self.unary()?;
        if let Some(Tok::Op('^')) = self.peek() {
            self.i += 1;
            let r = self.power()?;
            return Some(v.dpow(r));
        }
        if let Some(Tok::Op('%')) = self.peek() {
            self.i += 1;
            return Some(v / 100.0);
        }
        Some(v)
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some(Tok::Op('-')) => {
                self.i += 1;
                Some(-self.unary()?)
            }
            Some(Tok::Op('+')) => {
                self.i += 1;
                self.unary()
            }
            _ => self.primary(),
        }
    }

    /// A function's arguments: each a list of values.
    fn args(&mut self) -> Option<Vec<f64>> {
        let mut vals = Vec::new();
        if let Some(Tok::RParen) = self.peek() {
            self.i += 1;
            return Some(vals);
        }
        loop {
            match self.peek().cloned() {
                Some(Tok::Ident(id)) if matches!(id.as_str(), "ABOVE" | "BELOW" | "LEFT" | "RIGHT") => {
                    self.i += 1;
                    vals.extend(self.ctx.direction(&id));
                }
                Some(Tok::Ident(id)) if id.contains(':') => {
                    self.i += 1;
                    let (a, b) = id.split_once(':')?;
                    vals.extend(self.ctx.range(cell_ref(a)?, cell_ref(b)?));
                }
                _ => vals.push(self.expr()?),
            }
            match self.next()? {
                Tok::Comma => continue,
                Tok::RParen => break,
                _ => return None,
            }
        }
        Some(vals)
    }

    fn primary(&mut self) -> Option<f64> {
        match self.next()? {
            Tok::Num(n) => Some(n),
            Tok::LParen => {
                let v = self.expr()?;
                match self.next()? {
                    Tok::RParen => Some(v),
                    _ => None,
                }
            }
            Tok::Ident(id) => {
                if let Some(Tok::LParen) = self.peek() {
                    self.i += 1;
                    let a = self.args()?;
                    let n = a.len() as f64;
                    return Some(match id.as_str() {
                        "SUM" => a.iter().sum(),
                        "AVERAGE" => if a.is_empty() { 0.0 } else { a.iter().sum::<f64>() / n },
                        "COUNT" => n,
                        "MAX" => if a.is_empty() { 0.0 } else { a.iter().cloned().fold(f64::MIN, f64::max) },
                        "MIN" => if a.is_empty() { 0.0 } else { a.iter().cloned().fold(f64::MAX, f64::min) },
                        "PRODUCT" => if a.is_empty() { 0.0 } else { a.iter().product() },
                        "ABS" => a.first()?.abs(),
                        "INT" => a.first()?.trunc(),
                        "SIGN" => a.first()?.signum(),
                        "ROUND" => {
                            let p = 10f64.dpowi(*a.get(1).unwrap_or(&0.0) as i32);
                            (a.first()? * p).round() / p
                        }
                        "MOD" => a.first()? % a.get(1)?,
                        "AND" => if a.iter().all(|x| *x != 0.0) { 1.0 } else { 0.0 },
                        "OR" => if a.iter().any(|x| *x != 0.0) { 1.0 } else { 0.0 },
                        "NOT" => if *a.first()? == 0.0 { 1.0 } else { 0.0 },
                        "TRUE" => 1.0,
                        "FALSE" => 0.0,
                        _ => return None,
                    });
                }
                if let Some(rc) = cell_ref(&id) {
                    return Some(self.ctx.cell(rc.0, rc.1).unwrap_or(0.0));
                }
                self.ctx.marks.get(&id).copied()
            }
            _ => None,
        }
    }
}

impl Editor {
    /// The value of a formula field: the expression after "=" (switches
    /// taken off); None when it can't be worked out.
    pub(super) fn formula_value(&self, instr: &str, at: Option<NodeId>) -> Option<String> {
        let body = instr.trim_start().strip_prefix('=')?;
        let expr = body.split('\\').next().unwrap_or("").trim();
        let dom = &self.doc.dom;
        // the table the field is in
        let mut grid = Vec::new();
        let mut here = None;
        if let Some(at) = at {
            let mut n = at;
            let mut tc = None;
            while let Some(p) = dom.parent(n) {
                if dom.is(p, "w:tc") && tc.is_none() {
                    tc = Some(p);
                }
                if dom.is(p, "w:tbl") {
                    for (ri, tr) in dom.children_named(p, "w:tr").enumerate() {
                        let mut row = Vec::new();
                        for (ci, c) in dom.children_named(tr, "w:tc").enumerate() {
                            if Some(c) == tc {
                                here = Some((ri, ci));
                            }
                            let text: Vec<String> = dom.children_named(c, "w:p").map(|q| self.doc.para_text(q)).collect();
                            row.push(text.join(" "));
                        }
                        grid.push(row);
                    }
                    break;
                }
                n = p;
            }
        }
        // the formula's own cell counts as blank
        if let Some((r, c)) = here {
            if let Some(x) = grid.get_mut(r).and_then(|row| row.get_mut(c)) {
                x.clear();
            }
        }
        let mut marks = std::collections::HashMap::new();
        for (name, (s, e)) in self.bookmarks() {
            if let Some(v) = number_in(&self.bookmark_text(s, e)) {
                marks.insert(name.to_ascii_uppercase(), v);
            }
        }
        let ctx = Ctx { grid, here, marks };
        let t = tokens(expr)?;
        let mut p = Parser { t, i: 0, ctx: &ctx };
        let v = p.expr()?;
        if p.i != p.t.len() {
            return None;
        }
        let s = if (v - v.round()).abs() < 1e-9 { format!("{}", v.round() as i64) } else {
            let s = format!("{:.10}", v);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        };
        Some(s)
    }
}
