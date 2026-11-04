//! `folio-cork --bench` (X3-12): Cork measured against the Raspberry Pi 4B budgets, with no
//! window, at 1080p. On the Pi itself this is the pass/fail check; elsewhere it shows headroom.

use crate::app::CorkApp;
use crate::model::{Document, End};
use folio_kit::platform::{self, App, Event, Headless, Mods};
use std::time::{Duration, Instant};

/// The budgets every Folio app is held to on a Pi 4B: usable, not optimal (Product Owner,
/// 2026-11-04). Faster machines are expected to do much better.
pub const FRAME_BUDGET: Duration = Duration::from_millis(50); // 20 fps on the Pi
pub const OPEN_BUDGET: Duration = Duration::from_secs(2);
pub const MEMORY_BUDGET: u64 = 250 * 1024 * 1024;
/// How much slower a Pi 4B is than the machine the bench runs on, unless told
/// (FOLIO_PI_FACTOR=1 on the Pi itself). A Pi 4B runs this software renderer about 3× slower
/// than a current x86-64 desktop core.
fn pi_factor() -> f64 {
    platform::setting("FOLIO_PI_FACTOR").and_then(|s| s.parse().ok()).unwrap_or(3.0)
}

const W: u32 = 1920;
const H: u32 = 1080;

pub(crate) fn busy_board(a: &mut CorkApp) {
    // 400 shapes in a grid, joined by 380 connectors: a large real-world diagram.
    let ed = &mut a.ed;
    let mut ids = Vec::new();
    for i in 0..400 {
        let (cx, cy) = (80.0 + (i % 20) as f32 * 140.0, 80.0 + (i / 20) as f32 * 110.0);
        let kind = ["rect", "rounded", "ellipse", "diamond"][i % 4];
        let id = ed.insert_kind_at(kind, cx, cy);
        ed.begin_edit(id);
        ed.type_text(&format!("Step {i}"));
        ed.end_edit();
        ids.push(id);
    }
    for i in 0..380 {
        let b = if i % 20 == 19 { i + 20 } else { i + 1 };
        if b < ids.len() {
            ed.add_connector(End::glued(ids[i], None), End::glued(ids[b], None));
        }
    }
    ed.sel.clear();
}

fn stats(mut v: Vec<Duration>) -> (Duration, Duration) {
    if v.is_empty() {
        return (Duration::ZERO, Duration::ZERO);
    }
    v.sort();
    let avg = v.iter().sum::<Duration>() / v.len().max(1) as u32;
    let p95 = v[(v.len() * 95 / 100).min(v.len() - 1)];
    (avg, p95)
}

fn line(name: &str, got: Duration, budget: Duration) -> bool {
    let pi = got.as_secs_f64() * pi_factor();
    let ok = pi <= budget.as_secs_f64();
    println!("{} {name}: {:.1} ms here, about {:.0} ms on a Pi 4B (budget {:.0} ms)", if ok { "ok  " } else { "OVER" }, got.as_secs_f64() * 1000.0, pi * 1000.0, budget.as_secs_f64() * 1000.0);
    ok
}

pub fn run() -> i32 {
    let t0 = Instant::now();
    let mut host = Headless::default();
    let mut px = vec![0u8; (W * H * 4) as usize];
    let mut a = CorkApp::new(Document::default(), None);
    App::event(&mut a, &Event::Resized { w: W, h: H, scale: 1.0 }, &mut host);
    App::paint(&mut a, &mut px, W, H, &mut host);
    let open = t0.elapsed();
    let mut ok = line("open to first frame", open, OPEN_BUDGET);

    busy_board(&mut a);
    App::paint(&mut a, &mut px, W, H, &mut host);
    // The pointer over the middle of the drawing, where the wheel scrolls it.
    App::event(&mut a, &Event::MouseMove { x: W as f32 / 2.0, y: H as f32 / 2.0 }, &mut host);
    let mut scroll = Vec::new();
    let frames = platform::setting("FOLIO_BENCH_FRAMES").and_then(|s| s.parse().ok()).unwrap_or(120);
    for i in 0..frames {
        let dy = if (i / 30) % 2 == 0 { -3.0 } else { 3.0 };
        App::event(&mut a, &Event::Wheel { dx: 0.0, dy }, &mut host);
        let t = Instant::now();
        App::paint(&mut a, &mut px, W, H, &mut host);
        scroll.push(t.elapsed());
    }
    let (avg, p95) = stats(scroll);
    ok &= line("scrolling a 400-shape board, average frame", avg, FRAME_BUDGET);
    ok &= line("scrolling, 95th percentile frame", p95, FRAME_BUDGET);

    let id = a.ed.insert_kind_at("rect", 300.0, 300.0);
    a.ed.sel.clear();
    a.ed.begin_edit(id);
    let mut typing = Vec::new();
    let (mut ev_t, mut paint_t) = (Duration::ZERO, Duration::ZERO);
    for c in "The quick brown fox jumps over the lazy dog. ".repeat(3).chars() {
        let t = Instant::now();
        App::event(&mut a, &Event::Text(c.to_string()), &mut host);
        let t1 = Instant::now();
        App::paint(&mut a, &mut px, W, H, &mut host);
        ev_t += t1 - t;
        paint_t += t1.elapsed();
        typing.push(t.elapsed());
    }
    if platform::setting("FOLIO_BENCH_DETAIL").is_some() {
        println!("     typing: handling {:.1} ms, drawing {:.1} ms a key", ev_t.as_secs_f64() * 1000.0 / typing.len() as f64, paint_t.as_secs_f64() * 1000.0 / typing.len() as f64);
    }
    App::event(&mut a, &Event::Modifiers(Mods::NONE), &mut host);
    let (avg, p95) = stats(typing);
    ok &= line("typing into a shape, average keystroke to frame", avg, FRAME_BUDGET);
    ok &= line("typing, 95th percentile", p95, FRAME_BUDGET);

    match platform::memory_peak() {
        Some(m) => {
            let good = m <= MEMORY_BUDGET;
            println!("{} peak memory: {} MB (budget {} MB)", if good { "ok  " } else { "OVER" }, m / 1024 / 1024, MEMORY_BUDGET / 1024 / 1024);
            ok &= good;
        }
        None => println!("--   peak memory: not reported by this system"),
    }
    println!("{}", if ok { "within the Pi 4B budgets" } else { "over budget" });
    if ok { 0 } else { 1 }
}
