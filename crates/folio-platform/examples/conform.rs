//! Window conformance app (X3-13), driven by tools/conformance.sh. It logs every event it gets,
//! paints a known picture, and checks the system clipboard, then exits on Escape.

use folio_platform::*;
use std::time::Duration;

struct Conform {
    ticks: u32,
    exit: bool,
    code: i32,
    started: bool,
    copy_at: Option<u32>,
}

impl App for Conform {
    fn event(&mut self, ev: &Event, host: &mut dyn Host) {
        println!("EV {ev:?}");
        match ev {
            Event::Resized { .. } if !self.started => {
                self.started = true;
                host.set_title("folio-conform-ready");
            }
            Event::Tick => {
                self.ticks += 1;
                // The clipboard is checked a little after a key press: a Wayland compositor only
                // takes a selection from the window with the keyboard, after real input.
                if self.copy_at == Some(self.ticks) {
                    host.set_clipboard_text("folio conform ✓");
                }
                if self.copy_at.map(|t| t + 4) == Some(self.ticks) {
                    let ok = host.clipboard_text().as_deref() == Some("folio conform ✓");
                    println!("CLIP text {}", if ok { "ok" } else { "fail" });
                    let img = Image { w: 2, h: 1, rgba: vec![255, 0, 0, 255, 0, 0, 255, 255] };
                    host.set_clipboard_image(&img);
                }
                if self.copy_at.map(|t| t + 8) == Some(self.ticks) {
                    let img = Image { w: 2, h: 1, rgba: vec![255, 0, 0, 255, 0, 0, 255, 255] };
                    let back = host.clipboard_image();
                    println!("CLIP image {}", if back.as_ref() == Some(&img) { "ok" } else { "fail" });
                    println!("CLOCK {:?}", host.now());
                }
                if self.ticks > 600 {
                    println!("TIMEOUT");
                    self.code = 1;
                    self.exit = true;
                }
            }
            Event::Key { key: Key::Char('a'), down: true, .. } if self.copy_at.is_none() => self.copy_at = Some(self.ticks + 1),
            Event::Key { key: Key::Escape, down: true, .. } => self.exit = true,
            Event::CloseRequested => self.exit = true,
            _ => {}
        }
        host.redraw();
    }
    fn paint(&mut self, px: &mut [u8], w: u32, h: u32, _host: &mut dyn Host) {
        // Blue everywhere, a red square at the top left, green at the bottom right.
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let c = if x < 100 && y < 100 {
                    [255, 0, 0, 255]
                } else if x >= w - 100 && y >= h - 100 {
                    [0, 255, 0, 255]
                } else {
                    [0x33, 0x66, 0xCC, 255]
                };
                px[i..i + 4].copy_from_slice(&c);
            }
        }
        println!("PAINT {w}x{h}");
    }
    fn next_tick(&self) -> Option<Duration> {
        Some(Duration::from_millis(50))
    }
    fn wants_exit(&self) -> bool {
        self.exit
    }
    fn exit_code(&self) -> i32 {
        self.code
    }
}

fn main() {
    let code = run(Conform { ticks: 0, exit: false, code: 0, started: false, copy_at: None }, WindowSpec { title: "Folio conformance".into(), app_id: "folio-conform".into(), width: 900, height: 600, icon: None });
    std::process::exit(code);
}
