//! The model: vertices, edges and faces, the groups and components over them,
//! and the materials they are painted with. Everything is kept in plain lists
//! with an `alive` flag, so an index stays an index while a drawing is edited.

use crate::math::{Plane, V3, Xf};

/// What a thing in the model is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    #[default]
    None,
    Vertex,
    Edge,
    Face,
    Instance,
    Guide,
}

/// One thing in the model, by kind and index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ref {
    pub kind: Kind,
    pub id: i32,
}

impl Ref {
    pub fn new(kind: Kind, id: usize) -> Ref {
        Ref { kind, id: id as i32 }
    }
    pub fn valid(&self) -> bool {
        self.kind != Kind::None && self.id >= 0
    }
    pub fn index(&self) -> usize {
        self.id.max(0) as usize
    }
}

#[derive(Clone, Debug, Default)]
pub struct Vertex {
    pub p: V3,
    pub edges: Vec<usize>,
    pub alive: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Edge {
    pub v: [usize; 2],
    pub faces: Vec<usize>,
    pub soft: bool,
    pub smooth: bool,
    pub hidden: bool,
    pub material: i32,
    /// the arc or circle this edge was drawn as part of
    pub curve: i32,
    pub alive: bool,
}

impl Edge {
    pub fn other(&self, v: usize) -> usize {
        if self.v[0] == v {
            self.v[1]
        } else {
            self.v[0]
        }
    }
}

/// An arc, circle or polygon: the edges drawn by one command, so they pick together.
#[derive(Clone, Debug)]
pub struct Curve {
    pub is_arc: bool,
    pub closed: bool,
    pub polygon: bool,
    pub center: V3,
    pub normal: V3,
    pub xaxis: V3,
    pub radius: f64,
    pub start_ang: f64,
    pub end_ang: f64,
    pub segments: usize,
    pub alive: bool,
}

impl Default for Curve {
    fn default() -> Self {
        Curve {
            is_arc: false,
            closed: false,
            polygon: false,
            center: V3::default(),
            normal: V3::new(0.0, 0.0, 1.0),
            xaxis: V3::new(1.0, 0.0, 0.0),
            radius: 0.0,
            start_ang: 0.0,
            end_ang: 0.0,
            segments: 0,
            alive: false,
        }
    }
}

/// Where a material sits on a face.
#[derive(Clone, Copy, Debug)]
pub struct TexMap {
    pub custom: bool,
    pub u_axis: V3,
    pub v_axis: V3,
    pub u_off: f64,
    pub v_off: f64,
}

impl Default for TexMap {
    fn default() -> Self {
        TexMap { custom: false, u_axis: V3::new(1.0, 0.0, 0.0), v_axis: V3::new(0.0, 1.0, 0.0), u_off: 0.0, v_off: 0.0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Face {
    /// the vertices round the outside, anticlockwise about the plane's normal
    pub outer: Vec<usize>,
    /// the holes in it, clockwise
    pub holes: Vec<Vec<usize>>,
    pub plane: Plane,
    pub mat_front: i32,
    pub mat_back: i32,
    pub tex_front: TexMap,
    pub tex_back: TexMap,
    pub hidden: bool,
    pub alive: bool,
}

/// A group or component placed in the model.
#[derive(Clone, Debug, Default)]
pub struct Instance {
    /// which definition it shows
    pub def: i32,
    pub xf: Xf,
    pub name: String,
    pub material: i32,
    pub hidden: bool,
    pub locked: bool,
    pub alive: bool,
}

/// A guide point or line, from the tape measure.
#[derive(Clone, Debug, Default)]
pub struct Guide {
    pub is_point: bool,
    pub p: V3,
    pub dir: V3,
    pub hidden: bool,
    pub alive: bool,
}

/// A colour, and the picture painted with it.
#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    /// 0xRRGGBB
    pub color: u32,
    pub alpha: f64,
    /// the picture, as the file holds it, and how large it is on the model (mm)
    pub texture: Option<Vec<u8>>,
    pub tex_w: f64,
    pub tex_h: f64,
}

impl Default for Material {
    fn default() -> Self {
        Material { name: "Material".into(), color: 0xB4B4B4, alpha: 1.0, texture: None, tex_w: 1000.0, tex_h: 1000.0 }
    }
}

/// Everything drawn in one place: the model itself, or inside one group.
#[derive(Clone, Debug, Default)]
pub struct Entities {
    pub verts: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub faces: Vec<Face>,
    pub curves: Vec<Curve>,
    pub insts: Vec<Instance>,
    pub guides: Vec<Guide>,
    pub rev: u64,
}

impl Entities {
    pub fn touch(&mut self) {
        self.rev += 1;
    }

    /// Put a vertex in, or find the one already there.
    pub fn add_vertex(&mut self, p: V3) -> usize {
        for (i, v) in self.verts.iter().enumerate() {
            if v.alive && v.p.near(p) {
                return i;
            }
        }
        if let Some(i) = self.verts.iter().position(|v| !v.alive) {
            self.verts[i] = Vertex { p, edges: Vec::new(), alive: true };
            return i;
        }
        self.verts.push(Vertex { p, edges: Vec::new(), alive: true });
        self.verts.len() - 1
    }

    pub fn find_edge(&self, a: usize, b: usize) -> Option<usize> {
        self.verts.get(a)?.edges.iter().copied().find(|e| {
            let ed = &self.edges[*e];
            ed.alive && (ed.v == [a, b] || ed.v == [b, a])
        })
    }

    /// Put an edge between two vertices in, or find the one already there.
    pub fn add_edge(&mut self, a: usize, b: usize) -> Option<usize> {
        if a == b {
            return None;
        }
        if let Some(e) = self.find_edge(a, b) {
            return Some(e);
        }
        let e = Edge { v: [a, b], faces: Vec::new(), material: -1, curve: -1, alive: true, ..Default::default() };
        let i = match self.edges.iter().position(|e| !e.alive) {
            Some(i) => {
                self.edges[i] = e;
                i
            }
            None => {
                self.edges.push(e);
                self.edges.len() - 1
            }
        };
        self.verts[a].edges.push(i);
        self.verts[b].edges.push(i);
        Some(i)
    }

    /// Put a face in, round a loop of vertices. The edges are made as needed.
    pub fn add_face(&mut self, loop_: &[usize]) -> Option<usize> {
        if loop_.len() < 3 {
            return None;
        }
        let pts: Vec<V3> = loop_.iter().map(|i| self.verts[*i].p).collect();
        let plane = plane_of(&pts)?;
        let mut edges = Vec::new();
        for i in 0..loop_.len() {
            let a = loop_[i];
            let b = loop_[(i + 1) % loop_.len()];
            edges.push(self.add_edge(a, b)?);
        }
        let f = Face { outer: loop_.to_vec(), holes: Vec::new(), plane, mat_front: -1, mat_back: -1, alive: true, ..Default::default() };
        let fi = match self.faces.iter().position(|f| !f.alive) {
            Some(i) => {
                self.faces[i] = f;
                i
            }
            None => {
                self.faces.push(f);
                self.faces.len() - 1
            }
        };
        for e in edges {
            self.edges[e].faces.push(fi);
        }
        self.touch();
        Some(fi)
    }

    pub fn remove_face(&mut self, f: usize) {
        if let Some(face) = self.faces.get_mut(f) {
            face.alive = false;
        }
        for e in &mut self.edges {
            e.faces.retain(|i| *i != f);
        }
        self.touch();
    }

    /// Take an edge out, and any face that used it.
    pub fn remove_edge(&mut self, e: usize) {
        let faces: Vec<usize> = self.edges.get(e).map(|ed| ed.faces.clone()).unwrap_or_default();
        for f in faces {
            self.remove_face(f);
        }
        if let Some(ed) = self.edges.get_mut(e) {
            ed.alive = false;
            let vs = ed.v;
            for v in vs {
                if let Some(vert) = self.verts.get_mut(v) {
                    vert.edges.retain(|i| *i != e);
                }
            }
        }
        // a vertex with nothing on it goes too
        let verts: Vec<usize> = (0..self.verts.len()).filter(|i| self.verts[*i].alive && self.verts[*i].edges.is_empty()).collect();
        for v in verts {
            self.verts[v].alive = false;
        }
        self.touch();
    }

    /// The points round a face, in order.
    pub fn face_points(&self, f: usize) -> Vec<V3> {
        self.faces.get(f).map(|f| f.outer.iter().map(|i| self.verts[*i].p).collect()).unwrap_or_default()
    }

    /// Everything alive, as references.
    pub fn all(&self) -> Vec<Ref> {
        let mut out = Vec::new();
        for (i, v) in self.verts.iter().enumerate() {
            if v.alive {
                out.push(Ref::new(Kind::Vertex, i));
            }
        }
        for (i, e) in self.edges.iter().enumerate() {
            if e.alive {
                out.push(Ref::new(Kind::Edge, i));
            }
        }
        for (i, f) in self.faces.iter().enumerate() {
            if f.alive {
                out.push(Ref::new(Kind::Face, i));
            }
        }
        for (i, n) in self.insts.iter().enumerate() {
            if n.alive {
                out.push(Ref::new(Kind::Instance, i));
            }
        }
        out
    }

    /// The box everything sits in.
    pub fn bounds(&self) -> Option<(V3, V3)> {
        let mut lo = V3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut hi = V3::new(f64::MIN, f64::MIN, f64::MIN);
        let mut any = false;
        for v in self.verts.iter().filter(|v| v.alive) {
            any = true;
            lo = V3::new(lo.x.min(v.p.x), lo.y.min(v.p.y), lo.z.min(v.p.z));
            hi = V3::new(hi.x.max(v.p.x), hi.y.max(v.p.y), hi.z.max(v.p.z));
        }
        any.then_some((lo, hi))
    }
}

/// A group or component: a drawing of its own that can be placed many times.
#[derive(Clone, Debug, Default)]
pub struct Definition {
    pub name: String,
    pub is_component: bool,
    pub ents: Entities,
}

/// The whole drawing.
#[derive(Clone, Debug)]
pub struct Model {
    pub ents: Entities,
    pub defs: Vec<Definition>,
    pub materials: Vec<Material>,
    /// millimetres per model unit for display ("mm", "cm", "m", "in", "ft")
    pub units: String,
}

impl Default for Model {
    fn default() -> Self {
        Model { ents: Entities::default(), defs: Vec::new(), materials: Vec::new(), units: "mm".into() }
    }
}

impl Model {
    pub fn new() -> Model {
        Model::default()
    }

    pub fn add_material(&mut self, name: &str, color: u32) -> usize {
        self.materials.push(Material { name: name.to_string(), color, ..Default::default() });
        self.materials.len() - 1
    }

    /// The faces of the model and of everything placed in it, in world space.
    pub fn world_faces(&self) -> Vec<(Vec<V3>, Plane, i32)> {
        let mut out = Vec::new();
        collect(&self.ents, &self.defs, &Xf::identity(), &mut out, 0);
        out
    }

    pub fn bounds(&self) -> Option<(V3, V3)> {
        let faces = self.world_faces();
        let mut lo = V3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut hi = V3::new(f64::MIN, f64::MIN, f64::MIN);
        let mut any = false;
        for (pts, _, _) in &faces {
            for p in pts {
                any = true;
                lo = V3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
                hi = V3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
            }
        }
        if !any {
            return self.ents.bounds();
        }
        Some((lo, hi))
    }
}

fn collect(ents: &Entities, defs: &[Definition], xf: &Xf, out: &mut Vec<(Vec<V3>, Plane, i32)>, depth: usize) {
    if depth > 16 {
        return;
    }
    for (i, f) in ents.faces.iter().enumerate() {
        if !f.alive || f.hidden {
            continue;
        }
        let pts: Vec<V3> = ents.face_points(i).iter().map(|p| xf.point(*p)).collect();
        if let Some(pl) = plane_of(&pts) {
            out.push((pts, pl, f.mat_front));
        }
    }
    for inst in ents.insts.iter().filter(|i| i.alive && !i.hidden) {
        if let Some(d) = defs.get(inst.def.max(0) as usize) {
            let x = inst.xf.then(xf);
            collect(&d.ents, defs, &x, out, depth + 1);
        }
    }
}

/// The plane a loop of points lies in, by Newell's method.
pub fn plane_of(pts: &[V3]) -> Option<Plane> {
    if pts.len() < 3 {
        return None;
    }
    let mut n = V3::default();
    let mut c = V3::default();
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        n += V3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
        c += a;
    }
    if n.len2() < 1e-18 {
        return None;
    }
    Some(Plane::through(c * (1.0 / pts.len() as f64), n))
}
