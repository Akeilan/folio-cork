//! Stencils (K3-03): named shape libraries for the Shapes pane.
//!
//! A stencil either uses a built-in geometry (path empty) or a small path language in a unit
//! box, stretched to the shape's size:
//! `F` starts a filled sub-path, `S` a stroke-only one; `M x y`, `L x y`, `C x1 y1 x2 y2 x y`,
//! `Z`, `E cx cy rx ry` (ellipse), `A cx cy rx ry start sweep` (arc, degrees, y down),
//! `D cx cy r` (filled dot).

use crate::geom::{ellipse_segs, Geo, Seg, Sub};

pub struct Stencil {
    pub id: &'static str,
    pub name: &'static str,
    pub cat: &'static str,
    pub path: &'static str,
    pub points: &'static [(f32, f32)],
    pub text: (f32, f32, f32, f32),
    pub w: f32,
    pub h: f32,
    /// Default text placed in new shapes.
    pub label: &'static str,
    /// Draw with no fill by default (line symbols).
    pub line_art: bool,
}

const SIDES: &[(f32, f32)] = &[(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)];
const LR: &[(f32, f32)] = &[(0.0, 0.5), (1.0, 0.5)];
const TB: &[(f32, f32)] = &[(0.5, 0.0), (0.5, 1.0)];
const FULL: (f32, f32, f32, f32) = (0.0, 0.0, 1.0, 1.0);
const BELOW: (f32, f32, f32, f32) = (-0.5, 1.0, 2.0, 0.6);

macro_rules! st {
    ($id:expr, $name:expr, $cat:expr, $path:expr, $pts:expr, $text:expr, $w:expr, $h:expr) => {
        Stencil { id: $id, name: $name, cat: $cat, path: $path, points: $pts, text: $text, w: $w, h: $h, label: "", line_art: false }
    };
    ($id:expr, $name:expr, $cat:expr, $path:expr, $pts:expr, $text:expr, $w:expr, $h:expr, line) => {
        Stencil { id: $id, name: $name, cat: $cat, path: $path, points: $pts, text: $text, w: $w, h: $h, label: "", line_art: true }
    };
    ($id:expr, $name:expr, $cat:expr, $path:expr, $pts:expr, $text:expr, $w:expr, $h:expr, label $l:expr) => {
        Stencil { id: $id, name: $name, cat: $cat, path: $path, points: $pts, text: $text, w: $w, h: $h, label: $l, line_art: false }
    };
}

pub const CATEGORIES: &[&str] = &[
    "Basic shapes",
    "Flowchart",
    "Block diagram",
    "Org chart",
    "Network",
    "UML",
    "Database (ERD)",
    "Floor plan",
    "Electrical",
    "Electronic",
];

pub static STENCILS: &[Stencil] = &[
    // Basic shapes (built-in geometry).
    st!("rect", "Rectangle", "Basic shapes", "", SIDES, FULL, 120.0, 72.0),
    st!("rounded", "Rounded rectangle", "Basic shapes", "", SIDES, FULL, 120.0, 72.0),
    st!("ellipse", "Ellipse", "Basic shapes", "", SIDES, FULL, 120.0, 80.0),
    st!("circle", "Circle", "Basic shapes", "", SIDES, FULL, 80.0, 80.0),
    st!("triangle", "Triangle", "Basic shapes", "", SIDES, FULL, 100.0, 86.0),
    st!("rtriangle", "Right triangle", "Basic shapes", "", SIDES, FULL, 90.0, 90.0),
    st!("diamond", "Diamond", "Basic shapes", "", SIDES, FULL, 100.0, 100.0),
    st!("parallelogram", "Parallelogram", "Basic shapes", "", SIDES, FULL, 130.0, 70.0),
    st!("trapezoid", "Trapezoid", "Basic shapes", "", SIDES, FULL, 120.0, 72.0),
    st!("pentagon", "Pentagon", "Basic shapes", "", SIDES, FULL, 90.0, 86.0),
    st!("hexagon", "Hexagon", "Basic shapes", "", SIDES, FULL, 110.0, 86.0),
    st!("octagon", "Octagon", "Basic shapes", "", SIDES, FULL, 90.0, 90.0),
    st!("star", "Star", "Basic shapes", "", SIDES, FULL, 96.0, 92.0),
    st!("cross", "Cross", "Basic shapes", "", SIDES, FULL, 90.0, 90.0),
    st!("arrow-right", "Right arrow", "Basic shapes", "", SIDES, FULL, 120.0, 60.0),
    st!("arrow-left", "Left arrow", "Basic shapes", "", SIDES, FULL, 120.0, 60.0),
    st!("arrow-up", "Up arrow", "Basic shapes", "", SIDES, FULL, 60.0, 120.0),
    st!("arrow-down", "Down arrow", "Basic shapes", "", SIDES, FULL, 60.0, 120.0),
    st!("arrow-double", "Double arrow", "Basic shapes", "", SIDES, FULL, 140.0, 60.0),
    st!("chevron", "Chevron", "Basic shapes", "", SIDES, FULL, 110.0, 60.0),
    st!("cylinder", "Cylinder", "Basic shapes", "", SIDES, FULL, 80.0, 100.0),
    st!("cloud", "Cloud", "Basic shapes", "", SIDES, FULL, 140.0, 90.0),
    st!("callout", "Callout", "Basic shapes", "", SIDES, FULL, 140.0, 90.0),
    st!("text", "Text", "Basic shapes", "", SIDES, FULL, 140.0, 40.0),
    // Flowchart.
    st!("flow.process", "Process", "Flowchart", "", SIDES, FULL, 120.0, 72.0),
    st!("flow.decision", "Decision", "Flowchart", "", SIDES, FULL, 120.0, 90.0),
    st!("flow.terminator", "Start/End", "Flowchart", "", SIDES, FULL, 120.0, 56.0),
    st!("flow.data", "Data", "Flowchart", "", SIDES, FULL, 130.0, 66.0),
    st!("flow.predefined", "Subprocess", "Flowchart", "", SIDES, FULL, 130.0, 72.0),
    st!("flow.document", "Document", "Flowchart", "", SIDES, FULL, 120.0, 80.0),
    st!("flow.manual-input", "Manual input", "Flowchart", "", SIDES, FULL, 120.0, 72.0),
    st!("flow.manual-op", "Manual operation", "Flowchart", "", SIDES, FULL, 120.0, 66.0),
    st!("flow.preparation", "Preparation", "Flowchart", "", SIDES, FULL, 130.0, 66.0),
    st!("flow.delay", "Delay", "Flowchart", "", SIDES, FULL, 110.0, 66.0),
    st!("flow.offpage", "Off-page reference", "Flowchart", "", SIDES, FULL, 70.0, 80.0),
    st!("flow.connector", "On-page reference", "Flowchart", "", SIDES, FULL, 50.0, 50.0),
    st!("flow.merge", "Merge", "Flowchart", "", SIDES, FULL, 80.0, 66.0),
    st!("flow.extract", "Extract", "Flowchart", "", SIDES, FULL, 80.0, 66.0),
    st!("flow.stored-data", "Stored data", "Flowchart", "", SIDES, FULL, 120.0, 66.0),
    st!("flow.internal-storage", "Internal storage", "Flowchart", "", SIDES, FULL, 100.0, 80.0),
    st!("flow.card", "Card", "Flowchart", "", SIDES, FULL, 110.0, 70.0),
    st!("flow.display", "Display", "Flowchart", "", SIDES, FULL, 120.0, 66.0),
    st!("flow.loop-limit", "Loop limit", "Flowchart", "", SIDES, FULL, 120.0, 66.0),
    st!("flow.or", "Or", "Flowchart", "", SIDES, FULL, 50.0, 50.0),
    st!("flow.summing", "Summing junction", "Flowchart", "", SIDES, FULL, 50.0, 50.0),
    st!("flow.database", "Database", "Flowchart", "", SIDES, FULL, 80.0, 96.0),
    // Block diagram.
    st!("block.box", "Block", "Block diagram", "F M0 0 L1 0 L1 1 L0 1 Z", SIDES, FULL, 120.0, 72.0),
    st!("block.box3d", "3-D box", "Block diagram", "F M0 .2 L.8 .2 L.8 1 L0 1 Z F M0 .2 L.2 0 L1 0 L.8 .2 Z F M.8 .2 L1 0 L1 .8 L.8 1 Z", SIDES, (0.0, 0.2, 0.8, 0.8), 120.0, 90.0),
    st!("block.circle", "Circle block", "Block diagram", "F E .5 .5 .5 .5", SIDES, (0.15, 0.15, 0.7, 0.7), 80.0, 80.0),
    st!("block.split", "Splitter", "Block diagram", "F M0 .35 L.5 .35 L.5 0 L1 .5 L.5 1 L.5 .65 L0 .65 Z", SIDES, (0.0, 0.3, 0.6, 0.4), 120.0, 60.0),
    st!("block.brace", "Brace", "Block diagram", "S M1 0 C.6 0 .5 .1 .5 .3 L.5 .4 C.5 .48 .4 .5 0 .5 C.4 .5 .5 .52 .5 .6 L.5 .7 C.5 .9 .6 1 1 1", &[(0.0, 0.5), (1.0, 0.0), (1.0, 1.0)], FULL, 30.0, 120.0, line),
    // Org chart.
    st!("org.person", "Person", "Org chart", "F M.06 0 L.94 0 C.98 0 1 .04 1 .1 L1 .9 C1 .96 .98 1 .94 1 L.06 1 C.02 1 0 .96 0 .9 L0 .1 C0 .04 .02 0 .06 0 Z F M0 .1 C0 .04 .02 0 .06 0 L.94 0 C.98 0 1 .04 1 .1 L1 .16 L0 .16 Z", SIDES, (0.0, 0.16, 1.0, 0.84), 150.0, 70.0, label "Name\nTitle"),
    st!("org.manager", "Manager", "Org chart", "F M0 0 L1 0 L1 1 L0 1 Z S M0 .2 L1 .2", SIDES, (0.0, 0.2, 1.0, 0.8), 160.0, 76.0, label "Name\nManager"),
    st!("org.assistant", "Assistant", "Org chart", "F M.1 0 L1 0 L.9 1 L0 1 Z", SIDES, (0.1, 0.0, 0.8, 1.0), 150.0, 64.0, label "Name\nAssistant"),
    st!("org.vacancy", "Vacancy", "Org chart", "F M0 0 L1 0 L1 1 L0 1 Z S M.05 .12 L.95 .12", SIDES, FULL, 150.0, 64.0, label "Vacant"),
    st!("org.team", "Team frame", "Org chart", "F M0 .12 L1 .12 L1 1 L0 1 Z F M0 0 L.4 0 L.45 .12 L0 .12 Z", SIDES, (0.0, 0.0, 0.4, 0.12), 300.0, 200.0, label "Team"),
    // Network.
    st!("net.computer", "Computer", "Network", "F M.1 0 L.9 0 L.9 .65 L.1 .65 Z F M.15 .05 L.85 .05 L.85 .6 L.15 .6 Z F M.42 .65 L.58 .65 L.6 .8 L.4 .8 Z F M.25 .8 L.75 .8 L.8 .9 L.2 .9 Z", &[(0.5, 0.0), (0.9, 0.35), (0.5, 0.9), (0.1, 0.35)], BELOW, 70.0, 64.0),
    st!("net.laptop", "Laptop", "Network", "F M.15 .1 L.85 .1 L.85 .7 L.15 .7 Z F M.2 .15 L.8 .15 L.8 .65 L.2 .65 Z F M.05 .75 L.95 .75 L1 .85 L0 .85 Z", &[(0.5, 0.1), (1.0, 0.8), (0.5, 0.85), (0.0, 0.8)], BELOW, 72.0, 56.0),
    st!("net.server", "Server", "Network", "F M.2 0 L.8 0 L.8 1 L.2 1 Z S M.28 .15 L.72 .15 M.28 .3 L.72 .3 M.28 .45 L.72 .45 F E .65 .8 .04 .03", &[(0.5, 0.0), (0.8, 0.5), (0.5, 1.0), (0.2, 0.5)], BELOW, 56.0, 90.0),
    st!("net.router", "Router", "Network", "F E .5 .5 .5 .32 S M.2 .5 L.4 .5 M.6 .5 L.8 .5 M.5 .3 L.5 .42 M.5 .58 L.5 .7 F M.4 .45 L.45 .5 L.4 .55 Z F M.6 .45 L.55 .5 L.6 .55 Z", SIDES, BELOW, 80.0, 64.0),
    st!("net.switch", "Switch", "Network", "F M0 .25 L1 .25 L1 .75 L0 .75 Z S M.2 .42 L.8 .42 M.2 .58 L.8 .58 F M.8 .37 L.88 .42 L.8 .47 Z F M.2 .53 L.12 .58 L.2 .63 Z", SIDES, BELOW, 100.0, 50.0),
    st!("net.firewall", "Firewall", "Network", "F M0 0 L1 0 L1 1 L0 1 Z S M0 .25 L1 .25 M0 .5 L1 .5 M0 .75 L1 .75 M.33 0 L.33 .25 M.66 0 L.66 .25 M.17 .25 L.17 .5 M.5 .25 L.5 .5 M.83 .25 L.83 .5 M.33 .5 L.33 .75 M.66 .5 L.66 .75 M.17 .75 L.17 1 M.5 .75 L.5 1 M.83 .75 L.83 1", SIDES, BELOW, 80.0, 70.0),
    st!("net.cloud", "Internet", "Network", "", SIDES, FULL, 140.0, 90.0, label "Internet"),
    st!("net.printer", "Printer", "Network", "F M.25 0 L.75 0 L.75 .3 L.25 .3 Z F M0 .3 L1 .3 L1 .75 L0 .75 Z F M.2 .6 L.8 .6 L.8 1 L.2 1 Z", SIDES, BELOW, 70.0, 64.0),
    st!("net.phone", "Phone", "Network", "F M.3 0 L.7 0 C.75 0 .78 .03 .78 .08 L.78 .92 C.78 .97 .75 1 .7 1 L.3 1 C.25 1 .22 .97 .22 .92 L.22 .08 C.22 .03 .25 0 .3 0 Z F M.27 .1 L.73 .1 L.73 .85 L.27 .85 Z", SIDES, BELOW, 40.0, 70.0),
    st!("net.wifi", "Wireless access point", "Network", "F M.2 .6 L.8 .6 L.8 .9 L.2 .9 Z S M.5 .6 L.5 .45 A .5 .45 .15 .12 200 140 A .5 .45 .28 .22 200 140 A .5 .45 .41 .32 200 140", &[(0.5, 0.9), (0.2, 0.75), (0.8, 0.75)], BELOW, 70.0, 64.0),
    st!("net.database", "Database server", "Network", "", SIDES, BELOW, 70.0, 86.0),
    // UML.
    st!("uml.class", "Class", "UML", "F M0 0 L1 0 L1 1 L0 1 Z S M0 .25 L1 .25 M0 .6 L1 .6", SIDES, (0.0, 0.0, 1.0, 0.25), 160.0, 110.0, label "ClassName"),
    st!("uml.actor", "Actor", "UML", "F E .5 .12 .12 .12 S M.5 .24 L.5 .62 M.15 .38 L.85 .38 M.5 .62 L.2 .95 M.5 .62 L.8 .95", &[(0.5, 0.0), (0.85, 0.38), (0.5, 0.95), (0.15, 0.38)], BELOW, 44.0, 84.0, label "Actor"),
    st!("uml.usecase", "Use case", "UML", "F E .5 .5 .5 .5", SIDES, (0.12, 0.12, 0.76, 0.76), 140.0, 70.0, label "Use case"),
    st!("uml.package", "Package", "UML", "F M0 .15 L1 .15 L1 1 L0 1 Z F M0 0 L.4 0 L.4 .15 L0 .15 Z", SIDES, (0.0, 0.15, 1.0, 0.85), 150.0, 100.0, label "Package"),
    st!("uml.component", "Component", "UML", "F M.1 0 L1 0 L1 1 L.1 1 Z F M0 .2 L.2 .2 L.2 .35 L0 .35 Z F M0 .55 L.2 .55 L.2 .7 L0 .7 Z", SIDES, (0.2, 0.0, 0.8, 1.0), 140.0, 80.0, label "Component"),
    st!("uml.note", "Note", "UML", "F M0 0 L.8 0 L1 .25 L1 1 L0 1 Z S M.8 0 L.8 .25 L1 .25", SIDES, FULL, 140.0, 80.0, label "Note"),
    st!("uml.interface", "Interface", "UML", "F E .5 .5 .5 .5", SIDES, BELOW, 30.0, 30.0),
    st!("uml.state", "State", "UML", "F M.15 0 L.85 0 C.95 0 1 .1 1 .25 L1 .75 C1 .9 .95 1 .85 1 L.15 1 C.05 1 0 .9 0 .75 L0 .25 C0 .1 .05 0 .15 0 Z", SIDES, FULL, 130.0, 70.0, label "State"),
    st!("uml.initial", "Initial state", "UML", "D .5 .5 .5", SIDES, BELOW, 30.0, 30.0),
    st!("uml.final", "Final state", "UML", "F E .5 .5 .5 .5 D .5 .5 .32", SIDES, BELOW, 32.0, 32.0),
    st!("uml.lifeline", "Lifeline", "UML", "F M0 0 L1 0 L1 .15 L0 .15 Z S M.5 .15 L.5 1", &[(0.5, 0.0), (0.5, 0.3), (0.5, 0.5), (0.5, 0.7), (0.5, 1.0)], (0.0, 0.0, 1.0, 0.15), 120.0, 320.0, label "object"),
    // Database (ERD).
    st!("erd.entity", "Entity", "Database (ERD)", "F M0 0 L1 0 L1 1 L0 1 Z F M0 0 L1 0 L1 .22 L0 .22 Z", SIDES, (0.0, 0.0, 1.0, 0.22), 160.0, 120.0, label "Table"),
    st!("erd.weak", "Weak entity", "Database (ERD)", "F M0 0 L1 0 L1 1 L0 1 Z S M.05 .08 L.95 .08 L.95 .92 L.05 .92 Z", SIDES, FULL, 140.0, 70.0, label "Entity"),
    st!("erd.attribute", "Attribute", "Database (ERD)", "F E .5 .5 .5 .5", SIDES, (0.12, 0.12, 0.76, 0.76), 110.0, 56.0, label "attribute"),
    st!("erd.key", "Key attribute", "Database (ERD)", "F E .5 .5 .5 .5", SIDES, (0.12, 0.12, 0.76, 0.76), 110.0, 56.0, label "id"),
    st!("erd.relationship", "Relationship", "Database (ERD)", "F M.5 0 L1 .5 L.5 1 L0 .5 Z", SIDES, (0.2, 0.2, 0.6, 0.6), 120.0, 80.0, label "has"),
    // Floor plan.
    st!("floor.wall", "Wall", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z", LR, FULL, 200.0, 10.0),
    st!("floor.room", "Room", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z", SIDES, FULL, 300.0, 220.0, label "Room"),
    st!("floor.door", "Door", "Floor plan", "S M0 1 L0 0 A 0 1 1 1 270 90 S M0 1 L1 1", &[(0.0, 1.0), (1.0, 1.0)], BELOW, 72.0, 72.0, line),
    st!("floor.window", "Window", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z S M0 .5 L1 .5", LR, BELOW, 96.0, 10.0),
    st!("floor.stairs", "Stairs", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z S M.125 0 L.125 1 M.25 0 L.25 1 M.375 0 L.375 1 M.5 0 L.5 1 M.625 0 L.625 1 M.75 0 L.75 1 M.875 0 L.875 1", SIDES, BELOW, 160.0, 72.0),
    st!("floor.bed", "Bed", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z F M.08 .04 L.45 .04 L.45 .2 L.08 .2 Z F M.55 .04 L.92 .04 L.92 .2 L.55 .2 Z S M0 .28 L1 .28", SIDES, BELOW, 150.0, 190.0),
    st!("floor.table", "Table", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z", SIDES, BELOW, 120.0, 80.0),
    st!("floor.round-table", "Round table", "Floor plan", "F E .5 .5 .5 .5", SIDES, BELOW, 90.0, 90.0),
    st!("floor.chair", "Chair", "Floor plan", "F M.1 .2 L.9 .2 L.9 1 L.1 1 Z F M.1 0 L.9 0 L.9 .2 L.1 .2 Z", SIDES, BELOW, 44.0, 44.0),
    st!("floor.sofa", "Sofa", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z S M0 .3 L1 .3 M.12 .3 L.12 1 M.88 .3 L.88 1 M.5 .3 L.5 1", SIDES, BELOW, 190.0, 80.0),
    st!("floor.toilet", "Toilet", "Floor plan", "F M.15 0 L.85 0 L.85 .3 L.15 .3 Z F E .5 .62 .35 .36", SIDES, BELOW, 44.0, 66.0),
    st!("floor.sink", "Sink", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z F E .5 .55 .35 .32 D .5 .2 .05", SIDES, BELOW, 60.0, 46.0),
    st!("floor.bath", "Bathtub", "Floor plan", "F M0 0 L1 0 L1 1 L0 1 Z F M.06 .12 L.94 .12 C.97 .12 .97 .88 .94 .88 L.06 .88 C.03 .88 .03 .12 .06 .12 Z", SIDES, BELOW, 160.0, 76.0),
    st!("floor.desk", "Desk", "Floor plan", "F M0 0 L1 0 L1 .6 L0 .6 Z F M.35 .7 L.65 .7 L.65 1 L.35 1 Z", SIDES, BELOW, 140.0, 90.0),
    // Electrical (building wiring).
    st!("elec.outlet", "Outlet", "Electrical", "F E .5 .5 .35 .35 S M.15 .5 L0 .5 M.85 .5 L1 .5 M.35 .38 L.35 .62 M.65 .38 L.65 .62", LR, BELOW, 48.0, 32.0),
    st!("elec.switch", "Wall switch", "Electrical", "F E .2 .5 .12 .2 S M.3 .43 L.9 .05", &[(0.08, 0.5), (0.32, 0.5)], BELOW, 50.0, 40.0, line),
    st!("elec.light", "Light fixture", "Electrical", "F E .5 .5 .5 .5 S M.15 .15 L.85 .85 M.85 .15 L.15 .85", SIDES, BELOW, 40.0, 40.0),
    st!("elec.junction", "Junction box", "Electrical", "F M0 0 L1 0 L1 1 L0 1 Z S M0 0 L1 1 M1 0 L0 1", SIDES, BELOW, 36.0, 36.0),
    st!("elec.breaker", "Circuit breaker", "Electrical", "S M0 .7 L.25 .7 M.75 .7 L1 .7 A .5 .7 .25 .4 180 180 F E .25 .7 .04 .07 F E .75 .7 .04 .07", LR, BELOW, 70.0, 36.0, line),
    st!("elec.meter", "Meter", "Electrical", "F E .5 .5 .5 .5", SIDES, (0.15, 0.15, 0.7, 0.7), 44.0, 44.0, label "kWh"),
    st!("elec.panel", "Panel board", "Electrical", "F M0 0 L1 0 L1 1 L0 1 Z F M0 0 L1 1 L0 1 Z", SIDES, BELOW, 70.0, 40.0),
    st!("elec.motor", "Motor", "Electrical", "F E .5 .5 .5 .5", SIDES, (0.15, 0.15, 0.7, 0.7), 50.0, 50.0, label "M"),
    st!("elec.generator", "Generator", "Electrical", "F E .5 .5 .5 .5", SIDES, (0.15, 0.15, 0.7, 0.7), 50.0, 50.0, label "G"),
    st!("elec.transformer", "Transformer", "Electrical", "F E .33 .5 .33 .33 F E .67 .5 .33 .33", &[(0.0, 0.5), (1.0, 0.5)], BELOW, 80.0, 50.0),
    st!("elec.earth", "Earth", "Electrical", "S M.5 0 L.5 .4 M.1 .4 L.9 .4 M.25 .6 L.75 .6 M.4 .8 L.6 .8", &[(0.5, 0.0)], BELOW, 40.0, 40.0, line),
    // Electronic (schematic symbols).
    st!("ee.resistor", "Resistor", "Electronic", "S M0 .5 L.2 .5 L.25 .2 L.35 .8 L.45 .2 L.55 .8 L.65 .2 L.75 .8 L.8 .5 L1 .5", LR, BELOW, 96.0, 28.0, line),
    st!("ee.resistor-iec", "Resistor (IEC)", "Electronic", "S M0 .5 L.2 .5 F M.2 .2 L.8 .2 L.8 .8 L.2 .8 Z S M.8 .5 L1 .5", LR, BELOW, 96.0, 28.0),
    st!("ee.pot", "Potentiometer", "Electronic", "S M0 .7 L.2 .7 L.25 .5 L.35 .9 L.45 .5 L.55 .9 L.65 .5 L.75 .9 L.8 .7 L1 .7 M.5 0 L.5 .45 F M.44 .32 L.5 .45 L.56 .32 Z", &[(0.0, 0.7), (1.0, 0.7), (0.5, 0.0)], BELOW, 96.0, 40.0, line),
    st!("ee.capacitor", "Capacitor", "Electronic", "S M0 .5 L.44 .5 M.44 .1 L.44 .9 M.56 .1 L.56 .9 M.56 .5 L1 .5", LR, BELOW, 72.0, 36.0, line),
    st!("ee.polcap", "Polarized capacitor", "Electronic", "S M0 .5 L.44 .5 M.44 .1 L.44 .9 M.62 .1 C.54 .3 .54 .7 .62 .9 M.57 .5 L1 .5 M.3 .15 L.3 .35 M.2 .25 L.4 .25", LR, BELOW, 72.0, 36.0, line),
    st!("ee.inductor", "Inductor", "Electronic", "S M0 .7 L.2 .7 A .275 .7 .075 .5 180 180 A .425 .7 .075 .5 180 180 A .575 .7 .075 .5 180 180 A .725 .7 .075 .5 180 180 M.8 .7 L1 .7", &[(0.0, 0.7), (1.0, 0.7)], BELOW, 96.0, 28.0, line),
    st!("ee.diode", "Diode", "Electronic", "S M0 .5 L.35 .5 F M.35 .15 L.65 .5 L.35 .85 Z S M.65 .15 L.65 .85 M.65 .5 L1 .5", LR, BELOW, 72.0, 36.0),
    st!("ee.zener", "Zener diode", "Electronic", "S M0 .5 L.35 .5 F M.35 .15 L.65 .5 L.35 .85 Z S M.58 .05 L.65 .15 L.65 .85 L.72 .95 M.65 .5 L1 .5", LR, BELOW, 72.0, 36.0),
    st!("ee.led", "LED", "Electronic", "S M0 .6 L.35 .6 F M.35 .3 L.6 .6 L.35 .9 Z S M.6 .3 L.6 .9 M.6 .6 L1 .6 M.5 .2 L.68 0 M.62 .25 L.8 .05 F M.68 0 L.62 .03 L.66 .08 Z F M.8 .05 L.74 .08 L.78 .13 Z", &[(0.0, 0.6), (1.0, 0.6)], BELOW, 72.0, 44.0),
    st!("ee.npn", "NPN transistor", "Electronic", "S M0 .5 L.38 .5 M.38 .2 L.38 .8 M.38 .38 L.75 .1 L.75 0 M.38 .62 L.75 .9 L.75 1 F M.75 .9 L.6 .88 L.68 .78 Z", &[(0.0, 0.5), (0.75, 0.0), (0.75, 1.0)], (0.8, 0.3, 0.8, 0.4), 60.0, 72.0, line),
    st!("ee.pnp", "PNP transistor", "Electronic", "S M0 .5 L.38 .5 M.38 .2 L.38 .8 M.38 .38 L.75 .1 L.75 0 M.38 .62 L.75 .9 L.75 1 F M.38 .62 L.53 .64 L.46 .74 Z", &[(0.0, 0.5), (0.75, 0.0), (0.75, 1.0)], (0.8, 0.3, 0.8, 0.4), 60.0, 72.0, line),
    st!("ee.nmos", "N-channel MOSFET", "Electronic", "S M0 .6 L.3 .6 M.3 .25 L.3 .75 M.4 .2 L.4 .34 M.4 .43 L.4 .57 M.4 .66 L.4 .8 M.4 .27 L.75 .27 L.75 0 M.4 .73 L.75 .73 L.75 1 M.4 .5 L.75 .5 L.75 .73 F M.4 .5 L.52 .45 L.52 .55 Z", &[(0.0, 0.6), (0.75, 0.0), (0.75, 1.0)], (0.8, 0.3, 0.8, 0.4), 60.0, 72.0, line),
    st!("ee.opamp", "Op-amp", "Electronic", "F M.15 0 L.85 .5 L.15 1 Z S M0 .3 L.15 .3 M0 .7 L.15 .7 M.85 .5 L1 .5 M.2 .3 L.3 .3 M.2 .7 L.3 .7 M.25 .65 L.25 .75", &[(0.0, 0.3), (0.0, 0.7), (1.0, 0.5)], (0.3, 0.3, 0.4, 0.4), 96.0, 80.0),
    st!("ee.ground", "Ground", "Electronic", "S M.5 0 L.5 .4 M.1 .4 L.9 .4 M.25 .6 L.75 .6 M.4 .8 L.6 .8", &[(0.5, 0.0)], BELOW, 36.0, 36.0, line),
    st!("ee.vcc", "Supply", "Electronic", "S M.5 1 L.5 .35 M.15 .35 L.85 .35", &[(0.5, 1.0)], (-0.5, -0.6, 2.0, 0.6), 36.0, 30.0, line),
    st!("ee.battery", "Battery", "Electronic", "S M0 .5 L.42 .5 M.42 .1 L.42 .9 M.5 .3 L.5 .7 M.58 .1 L.58 .9 M.66 .3 L.66 .7 M.66 .5 L1 .5", LR, BELOW, 80.0, 40.0, line),
    st!("ee.vsource", "Voltage source", "Electronic", "S M0 .5 L.25 .5 M.75 .5 L1 .5 F E .5 .5 .25 .45 S M.34 .5 L.42 .5 M.38 .42 L.38 .58 M.58 .5 L.66 .5", LR, BELOW, 96.0, 52.0),
    st!("ee.isource", "Current source", "Electronic", "S M0 .5 L.25 .5 M.75 .5 L1 .5 F E .5 .5 .25 .45 S M.35 .5 L.62 .5 F M.66 .5 L.58 .42 L.58 .58 Z", LR, BELOW, 96.0, 52.0),
    st!("ee.acsource", "AC source", "Electronic", "S M0 .5 L.25 .5 M.75 .5 L1 .5 F E .5 .5 .25 .45 S M.36 .5 C.4 .25 .46 .25 .5 .5 C.54 .75 .6 .75 .64 .5", LR, BELOW, 96.0, 52.0),
    st!("ee.switch", "Switch", "Electronic", "S M0 .7 L.28 .7 M.32 .7 L.72 .2 M.72 .7 L1 .7 F E .3 .7 .025 .08 F E .72 .7 .025 .08", &[(0.0, 0.7), (1.0, 0.7)], BELOW, 80.0, 32.0, line),
    st!("ee.pushbutton", "Push button", "Electronic", "S M0 .8 L.28 .8 M.72 .8 L1 .8 M.25 .55 L.75 .55 M.5 .55 L.5 .1 M.38 .1 L.62 .1 F E .3 .8 .025 .07 F E .7 .8 .025 .07", &[(0.0, 0.8), (1.0, 0.8)], BELOW, 80.0, 40.0, line),
    st!("ee.fuse", "Fuse", "Electronic", "S M0 .5 L1 .5 F M.25 .25 L.75 .25 L.75 .75 L.25 .75 Z S M0 .5 L1 .5", LR, BELOW, 80.0, 28.0),
    st!("ee.lamp", "Lamp", "Electronic", "S M0 .5 L.3 .5 M.7 .5 L1 .5 F E .5 .5 .2 .45 S M.36 .18 L.64 .82 M.64 .18 L.36 .82", LR, BELOW, 80.0, 44.0),
    st!("ee.crystal", "Crystal", "Electronic", "S M0 .5 L.3 .5 M.3 .2 L.3 .8 M.7 .2 L.7 .8 M.7 .5 L1 .5 F M.38 .1 L.62 .1 L.62 .9 L.38 .9 Z", LR, BELOW, 72.0, 40.0),
    st!("ee.speaker", "Speaker", "Electronic", "F M0 .3 L.35 .3 L.35 .7 L0 .7 Z F M.35 .3 L.75 0 L.75 1 L.35 .7 Z", &[(0.0, 0.4), (0.0, 0.6)], BELOW, 50.0, 56.0),
    st!("ee.mic", "Microphone", "Electronic", "F E .4 .5 .3 .4 S M.72 .1 L.72 .9 M.72 .5 L1 .5", &[(1.0, 0.5)], BELOW, 50.0, 40.0),
    st!("ee.antenna", "Antenna", "Electronic", "S M.5 1 L.5 .3 M.5 .3 L.1 0 L.9 0 Z", &[(0.5, 1.0)], BELOW, 36.0, 50.0, line),
    st!("ee.relay", "Relay coil", "Electronic", "S M0 .5 L.25 .5 M.75 .5 L1 .5 F M.25 .15 L.75 .15 L.75 .85 L.25 .85 Z S M.25 .85 L.75 .15", LR, BELOW, 80.0, 40.0),
    st!("ee.and", "AND gate", "Electronic", "F M.2 0 L.55 0 C.8 0 .85 .25 .85 .5 C.85 .75 .8 1 .55 1 L.2 1 Z S M0 .25 L.2 .25 M0 .75 L.2 .75 M.85 .5 L1 .5", &[(0.0, 0.25), (0.0, 0.75), (1.0, 0.5)], BELOW, 80.0, 56.0),
    st!("ee.nand", "NAND gate", "Electronic", "F M.15 0 L.5 0 C.72 0 .78 .25 .78 .5 C.78 .75 .72 1 .5 1 L.15 1 Z F E .83 .5 .05 .07 S M0 .25 L.15 .25 M0 .75 L.15 .75 M.88 .5 L1 .5", &[(0.0, 0.25), (0.0, 0.75), (1.0, 0.5)], BELOW, 80.0, 56.0),
    st!("ee.or", "OR gate", "Electronic", "F M.15 0 C.5 0 .75 .15 .88 .5 C.75 .85 .5 1 .15 1 C.28 .7 .28 .3 .15 0 Z S M0 .25 L.24 .25 M0 .75 L.24 .75 M.88 .5 L1 .5", &[(0.0, 0.25), (0.0, 0.75), (1.0, 0.5)], BELOW, 80.0, 56.0),
    st!("ee.nor", "NOR gate", "Electronic", "F M.12 0 C.45 0 .67 .15 .78 .5 C.67 .85 .45 1 .12 1 C.25 .7 .25 .3 .12 0 Z F E .83 .5 .05 .07 S M0 .25 L.21 .25 M0 .75 L.21 .75 M.88 .5 L1 .5", &[(0.0, 0.25), (0.0, 0.75), (1.0, 0.5)], BELOW, 80.0, 56.0),
    st!("ee.xor", "XOR gate", "Electronic", "F M.2 0 C.53 0 .76 .15 .88 .5 C.76 .85 .53 1 .2 1 C.33 .7 .33 .3 .2 0 Z S M.12 0 C.25 .3 .25 .7 .12 1 M0 .25 L.22 .25 M0 .75 L.22 .75 M.88 .5 L1 .5", &[(0.0, 0.25), (0.0, 0.75), (1.0, 0.5)], BELOW, 80.0, 56.0),
    st!("ee.not", "NOT gate", "Electronic", "F M.15 0 L.75 .5 L.15 1 Z F E .8 .5 .05 .08 S M0 .5 L.15 .5 M.85 .5 L1 .5", LR, BELOW, 72.0, 48.0),
    st!("ee.terminal", "Terminal", "Electronic", "F E .5 .5 .5 .5", &[(0.5, 0.5)], BELOW, 12.0, 12.0),
    st!("ee.junction", "Junction dot", "Electronic", "D .5 .5 .5", &[(0.5, 0.5)], BELOW, 8.0, 8.0),
    st!("ee.ic", "IC (8 pins)", "Electronic", "F M.15 0 L.85 0 L.85 1 L.15 1 Z S M0 .125 L.15 .125 M0 .375 L.15 .375 M0 .625 L.15 .625 M0 .875 L.15 .875 M.85 .125 L1 .125 M.85 .375 L1 .375 M.85 .625 L1 .625 M.85 .875 L1 .875 F E .5 0 .08 .05", &[(0.0, 0.125), (0.0, 0.375), (0.0, 0.625), (0.0, 0.875), (1.0, 0.125), (1.0, 0.375), (1.0, 0.625), (1.0, 0.875)], (0.15, 0.3, 0.7, 0.4), 110.0, 130.0, label "IC"),
];

pub fn find(id: &str) -> Option<&'static Stencil> {
    STENCILS.iter().find(|s| s.id == id)
}

/// Does the stencil have a closed, filled outline that should decide clicks?
pub fn closed(id: &str) -> bool {
    find(id).map(|s| !s.line_art).unwrap_or(true)
}

pub fn parse(path: &str, w: f32, h: f32) -> Vec<Sub> {
    let toks: Vec<String> = path
        .replace(',', " ")
        .split_whitespace()
        .flat_map(|t| {
            // Allow "M0" and "L.5" forms.
            let first = t.chars().next().unwrap();
            if first.is_ascii_alphabetic() && t.len() > 1 {
                vec![first.to_string(), t[1..].to_string()]
            } else {
                vec![t.to_string()]
            }
        })
        .collect();
    let mut subs: Vec<Sub> = vec![];
    let mut i = 0;
    let num = |i: &mut usize| -> f32 {
        let v = toks.get(*i).and_then(|t| t.parse::<f32>().ok()).unwrap_or(0.0);
        *i += 1;
        v
    };
    let mut cur = Sub { segs: vec![], fill: true, solid: false };
    let flush = |subs: &mut Vec<Sub>, cur: &mut Sub, fill: bool| {
        if !cur.segs.is_empty() {
            subs.push(std::mem::replace(cur, Sub { segs: vec![], fill, solid: false }));
        } else {
            cur.fill = fill;
        }
    };
    while i < toks.len() {
        let t = toks[i].clone();
        i += 1;
        match t.as_str() {
            "F" => flush(&mut subs, &mut cur, true),
            "S" => flush(&mut subs, &mut cur, false),
            "M" => {
                let (x, y) = (num(&mut i), num(&mut i));
                cur.segs.push(Seg::M(x * w, y * h));
            }
            "L" => {
                let (x, y) = (num(&mut i), num(&mut i));
                cur.segs.push(Seg::L(x * w, y * h));
            }
            "C" => {
                let v: Vec<f32> = (0..6).map(|_| num(&mut i)).collect();
                cur.segs.push(Seg::C(v[0] * w, v[1] * h, v[2] * w, v[3] * h, v[4] * w, v[5] * h));
            }
            "Z" => cur.segs.push(Seg::Z),
            "E" => {
                let v: Vec<f32> = (0..4).map(|_| num(&mut i)).collect();
                cur.segs.extend(ellipse_segs(v[0] * w, v[1] * h, v[2] * w, v[3] * h));
            }
            "D" => {
                let v: Vec<f32> = (0..3).map(|_| num(&mut i)).collect();
                let fill = cur.fill;
                flush(&mut subs, &mut cur, fill);
                let r = v[2] * w.min(h);
                subs.push(Sub { segs: ellipse_segs(v[0] * w, v[1] * h, r, r), fill: true, solid: true });
            }
            "A" => {
                let v: Vec<f32> = (0..6).map(|_| num(&mut i)).collect();
                let (cx, cy, rx, ry) = (v[0] * w, v[1] * h, v[2] * w, v[3] * h);
                let (a0, sweep) = (v[4].to_radians(), v[5].to_radians());
                let n = ((sweep.abs() / std::f32::consts::FRAC_PI_2).ceil() as usize).max(1);
                let da = sweep / n as f32;
                let start = (cx + rx * a0.cos(), cy + ry * a0.sin());
                match cur.segs.last() {
                    None | Some(Seg::Z) => cur.segs.push(Seg::M(start.0, start.1)),
                    _ => cur.segs.push(Seg::L(start.0, start.1)),
                }
                let k = 4.0 / 3.0 * (da / 4.0).tan();
                for j in 0..n {
                    let a = a0 + da * j as f32;
                    let b = a + da;
                    let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
                    cur.segs.push(Seg::C(
                        cx + rx * (ca - k * sa),
                        cy + ry * (sa + k * ca),
                        cx + rx * (cb + k * sb),
                        cy + ry * (sb - k * cb),
                        cx + rx * cb,
                        cy + ry * sb,
                    ));
                }
            }
            _ => {}
        }
    }
    if !cur.segs.is_empty() {
        subs.push(cur);
    }
    subs
}

pub fn geometry(st: &Stencil, w: f32, h: f32) -> Geo {
    let subs = if st.path.is_empty() {
        let base = match st.id {
            "net.cloud" => "cloud",
            "net.database" => "cylinder",
            _ => "rect",
        };
        let s = crate::model::Shape { kind: base.into(), w, h, ..Default::default() };
        return Geo { text: (st.text.0 * w, st.text.1 * h, st.text.2 * w, st.text.3 * h), ..crate::geom::geometry(&s) };
    } else {
        let mut subs = parse(st.path, w, h);
        if st.line_art {
            for s in subs.iter_mut().filter(|s| s.fill) {
                s.solid = true;
            }
        }
        subs
    };
    Geo {
        subs,
        points: st.points.iter().map(|&(x, y)| (x * w, y * h)).collect(),
        text: (st.text.0 * w, st.text.1 * h, st.text.2 * w, st.text.3 * h),
    }
}

/// Stencils matching a search, by name or category.
pub fn search(q: &str) -> Vec<&'static Stencil> {
    let q = q.trim().to_lowercase();
    STENCILS.iter().filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q) || s.cat.to_lowercase().contains(&q)).collect()
}
