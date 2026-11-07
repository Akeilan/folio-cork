//! The drawing's own file: one line a thing, fields separated by tabs, so a
//! model can be read by eye and merged by hand if it ever needs to be.
//!
//! `.gable` holds the model; the app also writes `.obj` and `.stl` for other
//! programs to read.

use crate::math::{Plane, V3, Xf};
use crate::model::{Curve, Definition, Edge, Entities, Face, Instance, Material, Model, TexMap, Vertex};
use std::path::Path;

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

fn unesc(s: &str) -> String {
    let mut out = String::new();
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

fn nums(s: &str) -> Vec<f64> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn ids(s: &str) -> Vec<usize> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn v3_text(p: V3) -> String {
    format!("{:.6},{:.6},{:.6}", p.x, p.y, p.z)
}

fn v3_of(s: &str) -> V3 {
    let n = nums(s);
    V3::new(n.first().copied().unwrap_or(0.0), n.get(1).copied().unwrap_or(0.0), n.get(2).copied().unwrap_or(0.0))
}

fn ents_text(out: &mut String, tag: &str, ents: &Entities) {
    for (i, v) in ents.verts.iter().enumerate() {
        if v.alive {
            out.push_str(&format!("{tag}v\t{i}\t{}\n", v3_text(v.p)));
        }
    }
    for (i, e) in ents.edges.iter().enumerate() {
        if e.alive {
            out.push_str(&format!(
                "{tag}e\t{i}\t{},{}\t{}{}{}\n",
                e.v[0],
                e.v[1],
                if e.soft { "s" } else { "" },
                if e.smooth { "m" } else { "" },
                if e.hidden { "h" } else { "" }
            ));
        }
    }
    for (i, f) in ents.faces.iter().enumerate() {
        if !f.alive {
            continue;
        }
        let outer = f.outer.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
        let holes = f
            .holes
            .iter()
            .map(|h| h.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","))
            .collect::<Vec<_>>()
            .join(";");
        out.push_str(&format!("{tag}f\t{i}\t{outer}\t{holes}\t{}\t{}\n", f.mat_front, f.mat_back));
    }
    for (i, c) in ents.curves.iter().enumerate() {
        if c.alive {
            out.push_str(&format!(
                "{tag}c\t{i}\t{}\t{}\t{}\t{:.6}\t{:.6}\t{:.6}\t{}\t{}{}{}\n",
                v3_text(c.center),
                v3_text(c.normal),
                v3_text(c.xaxis),
                c.radius,
                c.start_ang,
                c.end_ang,
                c.segments,
                if c.is_arc { "a" } else { "" },
                if c.closed { "c" } else { "" },
                if c.polygon { "p" } else { "" }
            ));
        }
    }
    for (i, n) in ents.insts.iter().enumerate() {
        if !n.alive {
            continue;
        }
        let m = n.xf.m;
        let xf = (0..3).map(|r| (0..4).map(|c| format!("{:.6}", m[r][c])).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join(";");
        out.push_str(&format!("{tag}i\t{i}\t{}\t{xf}\t{}\t{}\n", n.def, esc(&n.name), n.material));
    }
}

/// Write the model.
pub fn save(model: &Model, path: &Path) -> Result<(), String> {
    let mut out = String::from("folio-gable model 1\n");
    out.push_str(&format!("units\t{}\n", model.units));
    for (i, m) in model.materials.iter().enumerate() {
        out.push_str(&format!("mat\t{i}\t{}\t{:06X}\t{:.3}\n", esc(&m.name), m.color & 0xFFFFFF, m.alpha));
    }
    ents_text(&mut out, "", &model.ents);
    for (i, d) in model.defs.iter().enumerate() {
        out.push_str(&format!("def\t{i}\t{}\t{}\n", esc(&d.name), if d.is_component { 1 } else { 0 }));
        ents_text(&mut out, &format!("{i}:"), &d.ents);
    }
    folio_kit::platform::atomic_write(path, out.as_bytes())
}

/// Read a model back.
pub fn open(path: &Path) -> Result<Model, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut m = Model::new();
    let mut defs: Vec<Definition> = Vec::new();
    for line in text.lines().skip(1) {
        let mut p = line.split('\t');
        let Some(kind) = p.next() else { continue };
        let rest: Vec<&str> = p.collect();
        // a line inside a definition starts "<n>:"
        let (into, kind) = match kind.split_once(':') {
            Some((n, k)) => (n.parse::<usize>().ok(), k),
            None => (None, kind),
        };
        let ents: &mut Entities = match into {
            Some(i) => {
                while defs.len() <= i {
                    defs.push(Definition::default());
                }
                &mut defs[i].ents
            }
            None => &mut m.ents,
        };
        match kind {
            "units" => m.units = rest.first().copied().unwrap_or("mm").to_string(),
            "mat" => {
                let i: usize = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                while m.materials.len() <= i {
                    m.materials.push(Material::default());
                }
                m.materials[i] = Material {
                    name: unesc(rest.get(1).copied().unwrap_or("Material")),
                    color: u32::from_str_radix(rest.get(2).copied().unwrap_or("B4B4B4"), 16).unwrap_or(0xB4B4B4),
                    alpha: rest.get(3).and_then(|v| v.parse().ok()).unwrap_or(1.0),
                    ..Default::default()
                };
            }
            "def" => {
                let i: usize = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                while defs.len() <= i {
                    defs.push(Definition::default());
                }
                defs[i].name = unesc(rest.get(1).copied().unwrap_or(""));
                defs[i].is_component = rest.get(2).copied().unwrap_or("0") == "1";
            }
            "v" => {
                let i: usize = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                while ents.verts.len() <= i {
                    ents.verts.push(Vertex::default());
                }
                ents.verts[i] = Vertex { p: v3_of(rest.get(1).copied().unwrap_or("")), edges: Vec::new(), alive: true };
            }
            "e" => {
                let i: usize = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                let vs = ids(rest.get(1).copied().unwrap_or(""));
                let flags = rest.get(2).copied().unwrap_or("");
                while ents.edges.len() <= i {
                    ents.edges.push(Edge::default());
                }
                ents.edges[i] = Edge {
                    v: [vs.first().copied().unwrap_or(0), vs.get(1).copied().unwrap_or(0)],
                    faces: Vec::new(),
                    soft: flags.contains('s'),
                    smooth: flags.contains('m'),
                    hidden: flags.contains('h'),
                    material: -1,
                    curve: -1,
                    alive: true,
                };
            }
            "f" => {
                let i: usize = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                let outer = ids(rest.get(1).copied().unwrap_or(""));
                let holes: Vec<Vec<usize>> = rest
                    .get(2)
                    .copied()
                    .unwrap_or("")
                    .split(';')
                    .filter(|s| !s.trim().is_empty())
                    .map(ids)
                    .collect();
                while ents.faces.len() <= i {
                    ents.faces.push(Face::default());
                }
                ents.faces[i] = Face {
                    outer,
                    holes,
                    plane: Plane::default(),
                    mat_front: rest.get(3).and_then(|v| v.parse().ok()).unwrap_or(-1),
                    mat_back: rest.get(4).and_then(|v| v.parse().ok()).unwrap_or(-1),
                    tex_front: TexMap::default(),
                    tex_back: TexMap::default(),
                    hidden: false,
                    alive: true,
                };
            }
            "c" => {
                let i: usize = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                while ents.curves.len() <= i {
                    ents.curves.push(Curve::default());
                }
                let flags = rest.get(8).copied().unwrap_or("");
                ents.curves[i] = Curve {
                    center: v3_of(rest.get(1).copied().unwrap_or("")),
                    normal: v3_of(rest.get(2).copied().unwrap_or("")),
                    xaxis: v3_of(rest.get(3).copied().unwrap_or("")),
                    radius: rest.get(4).and_then(|v| v.parse().ok()).unwrap_or(0.0),
                    start_ang: rest.get(5).and_then(|v| v.parse().ok()).unwrap_or(0.0),
                    end_ang: rest.get(6).and_then(|v| v.parse().ok()).unwrap_or(0.0),
                    segments: rest.get(7).and_then(|v| v.parse().ok()).unwrap_or(0),
                    is_arc: flags.contains('a'),
                    closed: flags.contains('c'),
                    polygon: flags.contains('p'),
                    alive: true,
                };
            }
            "i" => {
                let i: usize = rest.first().and_then(|v| v.parse().ok()).unwrap_or(0);
                while ents.insts.len() <= i {
                    ents.insts.push(Instance::default());
                }
                let mut xf = Xf::identity();
                for (r, row) in rest.get(2).copied().unwrap_or("").split(';').enumerate().take(3) {
                    for (c, v) in row.split(',').enumerate().take(4) {
                        xf.m[r][c] = v.parse().unwrap_or(xf.m[r][c]);
                    }
                }
                ents.insts[i] = Instance {
                    def: rest.get(1).and_then(|v| v.parse().ok()).unwrap_or(-1),
                    xf,
                    name: unesc(rest.get(3).copied().unwrap_or("")),
                    material: rest.get(4).and_then(|v| v.parse().ok()).unwrap_or(-1),
                    hidden: false,
                    locked: false,
                    alive: true,
                };
            }
            _ => {}
        }
    }
    m.defs = defs;
    rebuild(&mut m.ents);
    for d in &mut m.defs {
        rebuild(&mut d.ents);
    }
    Ok(m)
}

/// Put back what the file leaves out: which edges a vertex has, which faces an
/// edge has, and the plane of every face.
fn rebuild(ents: &mut Entities) {
    for v in &mut ents.verts {
        v.edges.clear();
    }
    for (i, e) in ents.edges.iter().enumerate() {
        if !e.alive {
            continue;
        }
        for v in e.v {
            if let Some(vert) = ents.verts.get_mut(v) {
                vert.edges.push(i);
            }
        }
    }
    for e in &mut ents.edges {
        e.faces.clear();
    }
    let faces: Vec<(usize, Vec<usize>)> = ents
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.alive)
        .map(|(i, f)| (i, f.outer.clone()))
        .collect();
    for (fi, outer) in faces {
        let pts: Vec<V3> = outer.iter().filter_map(|i| ents.verts.get(*i).map(|v| v.p)).collect();
        if let Some(pl) = crate::model::plane_of(&pts) {
            ents.faces[fi].plane = pl;
        }
        for k in 0..outer.len() {
            let (a, b) = (outer[k], outer[(k + 1) % outer.len()]);
            if let Some(e) = ents.find_edge(a, b) {
                ents.edges[e].faces.push(fi);
            }
        }
    }
    ents.touch();
}

/// Write the model as a Wavefront .obj, for other programs.
pub fn export_obj(model: &Model, path: &Path) -> Result<(), String> {
    let mut out = String::from("# written by Folio Gable\n");
    let faces = model.world_faces();
    let mut n = 1;
    for (pts, _, _) in &faces {
        for p in pts {
            out.push_str(&format!("v {:.4} {:.4} {:.4}\n", p.x, p.y, p.z));
        }
        out.push('f');
        for k in 0..pts.len() {
            out.push_str(&format!(" {}", n + k));
        }
        out.push('\n');
        n += pts.len();
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

/// Write the model as an STL, as 3D printers take it.
pub fn export_stl(model: &Model, path: &Path) -> Result<(), String> {
    let mut out = String::from("solid gable\n");
    for (pts, plane, _) in model.world_faces() {
        if pts.len() < 3 {
            continue;
        }
        let flat = crate::geom::flatten(&pts, &plane);
        for t in crate::geom::triangulate(&flat) {
            out.push_str(&format!("facet normal {:.5} {:.5} {:.5}\nouter loop\n", plane.n.x, plane.n.y, plane.n.z));
            for i in t {
                let p = pts[i];
                out.push_str(&format!("vertex {:.4} {:.4} {:.4}\n", p.x, p.y, p.z));
            }
            out.push_str("endloop\nendfacet\n");
        }
    }
    out.push_str("endsolid gable\n");
    std::fs::write(path, out).map_err(|e| e.to_string())
}
