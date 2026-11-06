//! Draw's part of the suite's copy-and-paste chain: paste the picture the program before put on
//! the clipboard, put one back for the program after, and say what happened. Driven by
//! `--clip-relay DIR`, where DIR is the folder the programs leave their notes in.

use crate::app::DrawApp;
use folio_kit::platform::Host;
use std::path::PathBuf;
use std::time::Duration;

pub struct ClipRelay {
    dir: PathBuf,
    stage: u8,
    started: Duration,
    pub done: bool,
    failed: usize,
}

fn note(dir: &std::path::Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name)).ok()
}

impl ClipRelay {
    pub fn new(dir: PathBuf) -> ClipRelay {
        ClipRelay { dir, stage: 0, started: Duration::ZERO, done: false, failed: 0 }
    }

    fn check(&mut self, what: &str, ok: bool, detail: &str) {
        if ok {
            println!("PASS {what}");
        } else {
            println!("FAIL {what}  {detail}");
            self.failed += 1;
        }
    }

    pub fn step(&mut self, app: &mut DrawApp, host: &mut dyn Host, now: Duration) {
        if self.started.is_zero() {
            self.started = now;
        }
        // nobody waits forever: two minutes is longer than the chain takes
        if now.saturating_sub(self.started) > Duration::from_secs(120) && !self.done {
            self.check("the chain finished", false, "it timed out");
            self.finish();
            return;
        }
        match self.stage {
            0 => {
                let Some(ready) = note(&self.dir, "gable.ready") else { return };
                let want: Vec<usize> = ready.split_whitespace().filter_map(|v| v.parse().ok()).collect();
                app.command("paste", host);
                let got = app.doc.float.as_ref().map(|f| (f.w, f.h));
                let ok = want.len() == 2 && got == Some((want[0], want[1]));
                self.check(
                    "Draw pastes the view Gable copied, at its size",
                    ok,
                    &format!("{got:?} wanted {want:?}"),
                );
                app.doc.commit_float();
                app.doc.sel_all();
                app.command("copy", host);
                let back = host.clipboard_image().map(|i| (i.w as usize, i.h as usize));
                self.check("and copies the picture for the next program", back.is_some(), &format!("{back:?}"));
                let _ = std::fs::write(self.dir.join("draw.ready"), format!("{} {}\n", app.doc.w, app.doc.h));
                let _ = std::fs::write(self.dir.join("gable.done"), "");
                self.stage = 1;
            }
            1 => {
                if note(&self.dir, "draw.done").is_some() {
                    self.finish();
                }
            }
            _ => {}
        }
    }

    fn finish(&mut self) {
        println!("{} failed", self.failed);
        self.done = true;
    }

    pub fn code(&self) -> i32 {
        if self.failed == 0 {
            0
        } else {
            1
        }
    }
}
