//! Board templates (K3-04) and the starting points in File > New.

use crate::editor::Editor;
use crate::model::*;
use folio_kit::Color;

pub const LIST: &[(&str, &str, &str)] = &[
    ("blank", "Blank board", "An empty page"),
    ("flowchart", "Flowchart", "Start, steps, a decision and an end"),
    ("brainstorm", "Brainstorm", "A topic, sticky notes and a parking lot"),
    ("mindmap", "Mind map", "A central idea with branches"),
    ("kanban", "Kanban", "To do, Doing, Done"),
    ("retro", "Retrospective", "Went well, To improve, Actions"),
    ("timeline", "Timeline", "Milestones along a line"),
    ("storymap", "User story map", "Activities, tasks and releases"),
    ("mood", "Mood board", "Pictures, colours and type"),
    ("swimlane", "Cross-functional flowchart", "Lanes for each team, with the flow across them"),
    ("orgchart", "Org chart", "A small team to start from"),
    ("schematic", "Schematic", "A sample circuit to start from"),
];

pub fn icon(k: &str) -> &'static str {
    match k {
        "blank" => "new",
        "flowchart" => "flow-down",
        "brainstorm" => "sticky",
        "mindmap" => "mind",
        "kanban" => "tidy-grid",
        "retro" => "comments",
        "timeline" => "flow-right",
        "storymap" => "tidy-masonry",
        "mood" => "palette",
        "swimlane" => "lane",
        "orgchart" => "org",
        "schematic" => "stencil",
        _ => "template",
    }
}

const YELLOW: u32 = 0xFFF176;
const ORANGE: u32 = 0xFFCC80;
const PINK: u32 = 0xF8BBD0;
const BLUE: u32 = 0xB3E5FC;
const GREEN: u32 = 0xC5E1A5;
const PURPLE: u32 = 0xE1BEE7;

struct B<'a> {
    ed: &'a mut Editor,
    ox: f32,
    oy: f32,
}

impl B<'_> {
    fn shape(&mut self, kind: &str, x: f32, y: f32, w: f32, h: f32, text: &str) -> Id {
        let mut s = self.ed.make_shape(kind, 0.0, 0.0);
        s.x = self.ox + x;
        s.y = self.oy + y;
        s.w = w;
        s.h = h;
        s.text = text.into();
        let id = s.id;
        if kind == "frame" {
            self.ed.pgm().shapes.insert(0, s);
        } else {
            self.ed.pgm().shapes.push(s);
        }
        id
    }
    fn sticky(&mut self, x: f32, y: f32, color: u32, text: &str) -> Id {
        let id = self.shape("sticky", x, y, 150.0, 150.0, text);
        if let Some(s) = self.ed.pgm().get_mut(id) {
            s.style.fill = Color::hex(color);
            s.tstyle.size = 18.0;
        }
        id
    }
    fn frame(&mut self, x: f32, y: f32, w: f32, h: f32, title: &str, fill: u32) -> Id {
        let id = self.shape("frame", x, y, w, h, title);
        if let Some(s) = self.ed.pgm().get_mut(id) {
            s.style.fill = Color::hex(fill).with_alpha(160);
        }
        id
    }
    fn lane(&mut self, x: f32, y: f32, w: f32, h: f32, title: &str) -> Id {
        let id = self.shape("lane", x, y, w, h, title);
        if let Some(s) = self.ed.pgm().get_mut(id) {
            s.data.insert("lane".into(), "h".into());
        }
        // a lane sits behind what it holds
        let pos = self.ed.pgm().shapes.iter().position(|s| s.id == id);
        if let Some(i) = pos {
            let s = self.ed.pgm().shapes.remove(i);
            self.ed.pgm().shapes.insert(0, s);
        }
        id
    }
    fn connect(&mut self, a: Id, b: Id, route: Route, label: &str) -> Id {
        let id = self.ed.doc.new_id();
        let mut s = Shape { id, kind: "connector".into(), layer: self.ed.active_layer(), text: label.into(), ..Default::default() };
        s.style.line = Color::hex(0x404040);
        s.style.width = 1.5;
        s.tstyle.size = 12.0;
        s.conn = Some(Conn { a: End::glued(a, None), b: End::glued(b, None), route, arrow_a: Arrow::None, arrow_b: Arrow::Triangle, jumps: false });
        self.ed.pgm().shapes.push(s);
        id
    }
    fn conn_pts(&mut self, a: (Id, usize), b: (Id, usize)) -> Id {
        let id = self.ed.doc.new_id();
        let mut s = Shape { id, kind: "connector".into(), layer: self.ed.active_layer(), ..Default::default() };
        s.style.line = Color::hex(0x1E1E1E);
        s.style.width = 1.5;
        s.conn = Some(Conn { a: End::glued(a.0, Some(a.1)), b: End::glued(b.0, Some(b.1)), route: Route::Elbow, arrow_a: Arrow::None, arrow_b: Arrow::None, jumps: true });
        self.ed.pgm().shapes.push(s);
        id
    }
}

/// Add a template's content to the current page, right of anything already there.
pub fn apply(key: &str, ed: &mut Editor) {
    if key == "blank" {
        return;
    }
    ed.end_edit();
    ed.checkpoint("Insert template", "");
    let ox = crate::render::content_bounds(ed.pg()).map(|b| b.0 + b.2 + 120.0).unwrap_or(48.0);
    let oy = 48.0;
    let mut b = B { ed, ox, oy };
    match key {
        "flowchart" => {
            let s = b.shape("flow.terminator", 180.0, 0.0, 140.0, 56.0, "Start");
            let p1 = b.shape("flow.process", 180.0, 110.0, 140.0, 72.0, "Do something");
            let d = b.shape("flow.decision", 170.0, 240.0, 160.0, 100.0, "Did it work?");
            let p2 = b.shape("flow.process", 440.0, 254.0, 140.0, 72.0, "Fix it");
            let e = b.shape("flow.terminator", 180.0, 400.0, 140.0, 56.0, "End");
            b.connect(s, p1, Route::Elbow, "");
            b.connect(p1, d, Route::Elbow, "");
            b.connect(d, e, Route::Elbow, "Yes");
            b.connect(d, p2, Route::Elbow, "No");
            b.connect(p2, p1, Route::Elbow, "");
        }
        "brainstorm" => {
            let topic = b.shape("rounded", 300.0, 0.0, 260.0, 80.0, "What's the question?");
            if let Some(s) = b.ed.pgm().get_mut(topic) {
                s.style.fill = Color::hex(0x2F5597);
                s.style.line = Color::hex(0x1F3864);
                s.tstyle.color = Color::WHITE;
                s.tstyle.size = 20.0;
                s.tstyle.bold = true;
            }
            b.frame(0.0, 140.0, 860.0, 400.0, "Ideas", 0xFFFFFF);
            let colors = [YELLOW, ORANGE, PINK, BLUE, GREEN];
            for i in 0..10 {
                b.sticky(20.0 + (i % 5) as f32 * 168.0, 170.0 + (i / 5) as f32 * 180.0, colors[i % 5], "");
            }
            b.frame(900.0, 140.0, 360.0, 400.0, "Parking lot", 0xF5F5F5);
        }
        "mindmap" => {
            let root = b.shape("ellipse", 300.0, 200.0, 200.0, 90.0, "Main idea");
            if let Some(s) = b.ed.pgm().get_mut(root) {
                s.style.fill = Color::hex(0xFFC000);
                s.style.line = Color::hex(0xBF9000);
                s.tstyle.size = 18.0;
                s.tstyle.bold = true;
            }
            let pos = [(0.0, 40.0), (0.0, 360.0), (620.0, 40.0), (620.0, 360.0)];
            let cols = [0xDCE8F7, 0xE2F0D9, 0xFBE5D6, 0xE8D5F5];
            for (i, (x, y)) in pos.iter().enumerate() {
                let n = b.shape("mind", *x, *y, 160.0, 56.0, &format!("Topic {}", i + 1));
                if let Some(s) = b.ed.pgm().get_mut(n) {
                    s.style.fill = Color::hex(cols[i]);
                }
                let c = b.connect(root, n, Route::Curved, "");
                if let Some(s) = b.ed.pgm().get_mut(c) {
                    if let Some(cn) = s.conn.as_mut() {
                        cn.arrow_b = Arrow::None;
                    }
                    s.style.width = 2.5;
                }
            }
        }
        "swimlane" => {
            for (i, t) in ["Customer", "Sales", "Warehouse"].iter().enumerate() {
                b.lane(0.0, i as f32 * 180.0, 900.0, 180.0, t);
            }
            let start = b.shape("flow.terminator", 70.0, 60.0, 130.0, 54.0, "Order placed");
            let check = b.shape("flow.process", 260.0, 240.0, 150.0, 72.0, "Check the order");
            let dec = b.shape("flow.decision", 460.0, 230.0, 170.0, 92.0, "In stock?");
            let order = b.shape("flow.process", 300.0, 420.0, 150.0, 72.0, "Order more");
            let pick = b.shape("flow.process", 540.0, 420.0, 150.0, 72.0, "Pick and pack");
            let done = b.shape("flow.terminator", 700.0, 66.0, 130.0, 54.0, "Parcel arrives");
            b.connect(start, check, Route::Elbow, "");
            b.connect(check, dec, Route::Elbow, "");
            b.connect(dec, pick, Route::Elbow, "Yes");
            b.connect(dec, order, Route::Elbow, "No");
            b.connect(order, pick, Route::Elbow, "");
            b.connect(pick, done, Route::Elbow, "");
        }
        "kanban" => {
            for (i, (t, c)) in [("To do", 0xFFFDE7), ("Doing", 0xE3F2FD), ("Done", 0xE8F5E9)].iter().enumerate() {
                let x = i as f32 * 300.0;
                b.frame(x, 0.0, 280.0, 640.0, t, *c);
                b.sticky(x + 20.0, 24.0, [YELLOW, BLUE, GREEN][i], "");
                b.sticky(x + 20.0, 194.0, [YELLOW, BLUE, GREEN][i], "");
            }
        }
        "retro" => {
            for (i, (t, c, st)) in [("Went well", 0xE8F5E9, GREEN), ("To improve", 0xFCE4EC, PINK), ("Actions", 0xE3F2FD, BLUE)].iter().enumerate() {
                let x = i as f32 * 400.0;
                b.frame(x, 0.0, 380.0, 520.0, t, *c);
                b.sticky(x + 20.0, 24.0, *st, "");
                b.sticky(x + 200.0, 24.0, *st, "");
            }
        }
        "timeline" => {
            let id = b.ed.doc.new_id();
            let mut line = Shape { id, kind: "connector".into(), layer: b.ed.active_layer(), ..Default::default() };
            line.style.line = Color::hex(0x404040);
            line.style.width = 3.0;
            line.conn = Some(Conn { a: End::free(b.ox, b.oy + 120.0), b: End::free(b.ox + 1100.0, b.oy + 120.0), route: Route::Straight, arrow_a: Arrow::None, arrow_b: Arrow::Triangle, jumps: false });
            b.ed.pgm().shapes.push(line);
            for i in 0..5 {
                let x = 80.0 + i as f32 * 230.0;
                let dot = b.shape("circle", x - 12.0, 108.0, 24.0, 24.0, "");
                if let Some(s) = b.ed.pgm().get_mut(dot) {
                    s.style.fill = Color::hex([0x4472C4, 0xED7D31, 0x70AD47, 0xFFC000, 0x7030A0][i]);
                    s.style.line = Color::WHITE;
                    s.style.width = 2.0;
                }
                let up = i % 2 == 0;
                let t = b.shape("text", x - 80.0, if up { 20.0 } else { 150.0 }, 160.0, 70.0, &format!("Milestone {}\nDate", i + 1));
                if let Some(s) = b.ed.pgm().get_mut(t) {
                    s.tstyle.align = folio_kit::text::Align::Center;
                    s.tstyle.valign = if up { folio_kit::text::VAlign::Bottom } else { folio_kit::text::VAlign::Top };
                }
            }
        }
        "storymap" => {
            for i in 0..4 {
                let x = i as f32 * 190.0;
                b.sticky(x, 0.0, ORANGE, &format!("Activity {}", i + 1));
                for j in 0..2 {
                    b.sticky(x, 190.0 + j as f32 * 230.0, YELLOW, "Task");
                }
            }
            b.frame(-20.0, 170.0, 790.0, 200.0, "Release 1", 0xF1F8E9);
            b.frame(-20.0, 400.0, 790.0, 200.0, "Release 2", 0xE3F2FD);
        }
        "mood" => {
            b.ed.pgm().mood = true;
            if b.ed.pg().background == Color::WHITE {
                b.ed.pgm().background = Color::hex(0xFAF8F3);
            }
            b.frame(0.0, 0.0, 760.0, 520.0, "Pictures", 0xFFFFFF);
            let hint = b.shape("text", 160.0, 230.0, 440.0, 60.0, "Drop or paste pictures here, or use Board > Add Pictures");
            if let Some(s) = b.ed.pgm().get_mut(hint) {
                s.tstyle.color = Color::hex(0x9E9E9E);
                s.tstyle.align = folio_kit::text::Align::Center;
            }
            b.frame(800.0, 0.0, 420.0, 250.0, "Colours", 0xFFFFFF);
            let pal = [0x2E3A59, 0xC9A27E, 0xE8DCCB, 0x8FA998, 0xD96C4F];
            for (i, c) in pal.iter().enumerate() {
                let id = b.shape("swatch", 820.0 + i as f32 * 78.0, 40.0, 70.0, 90.0, "");
                if let Some(s) = b.ed.pgm().get_mut(id) {
                    s.style.fill = Color::hex(*c);
                }
            }
            b.frame(800.0, 280.0, 420.0, 240.0, "Type", 0xFFFFFF);
            let t1 = b.shape("font", 820.0, 310.0, 380.0, 60.0, "Headline");
            let t2 = b.shape("font", 820.0, 390.0, 380.0, 40.0, "Body text sample");
            if let Some(s) = b.ed.pgm().get_mut(t1) {
                s.tstyle.size = 34.0;
                s.tstyle.bold = true;
            }
            if let Some(s) = b.ed.pgm().get_mut(t2) {
                s.tstyle.size = 18.0;
            }
        }
        "orgchart" => {
            let ceo = b.shape("org.manager", 335.0, 0.0, 170.0, 76.0, "Name\nDirector");
            let mut kids = vec![];
            for i in 0..3 {
                let k = b.shape("org.person", 60.0 + i as f32 * 280.0, 160.0, 160.0, 70.0, &format!("Name\nRole {}", i + 1));
                kids.push(k);
                b.connect(ceo, k, Route::Elbow, "");
            }
            for id in std::iter::once(ceo).chain(kids) {
                if let Some(s) = b.ed.pgm().get_mut(id) {
                    if let Some(c) = s.conn.as_mut() {
                        c.arrow_b = Arrow::None;
                    }
                }
            }
            for s in b.ed.pgm().shapes.iter_mut() {
                if let Some(c) = s.conn.as_mut() {
                    c.arrow_b = Arrow::None;
                    c.a.point = Some(2);
                    c.b.point = Some(0);
                }
            }
        }
        "schematic" => {
            let v = b.shape("ee.vsource", 0.0, 120.0, 96.0, 52.0, "9 V");
            if let Some(s) = b.ed.pgm().get_mut(v) {
                s.rot = 90.0;
            }
            let r = b.shape("ee.resistor", 120.0, 20.0, 96.0, 28.0, "R1 470 Ω");
            let led = b.shape("ee.led", 260.0, 120.0, 72.0, 44.0, "D1");
            if let Some(s) = b.ed.pgm().get_mut(led) {
                s.rot = 90.0;
            }
            let g = b.shape("ee.ground", 30.0, 260.0, 36.0, 36.0, "");
            b.conn_pts((v, 0), (r, 0));
            b.conn_pts((r, 1), (led, 0));
            b.conn_pts((led, 1), (g, 0));
            b.conn_pts((v, 1), (g, 0));
        }
        _ => {}
    }
    ed.changed();
}

#[allow(dead_code)]
pub fn sticky_colors() -> [u32; 6] {
    [YELLOW, ORANGE, PINK, BLUE, GREEN, PURPLE]
}
