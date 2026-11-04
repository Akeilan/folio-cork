//! X3-13: the platform contract's services, checked. A backend for a new system is done when
//! these pass there (and tools/conformance.sh passes for its window).

use folio_platform::*;
use std::io::{Read, Write};
use std::time::Duration;

#[test]
fn atomic_write_replaces_whole_and_leaves_nothing_behind() {
    let dir = temp_dir().join(format!("folio-conf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("a.json");
    atomic_write(&p, b"old").unwrap();
    atomic_write(&p, "nouveau ✓".as_bytes()).unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "nouveau ✓");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "temporary file left behind");
    // A failed save leaves the old file alone.
    let bad = dir.join("no-such-folder").join("b.json");
    assert!(atomic_write(&bad, b"x").is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn folders() {
    set_setting("FOLIO_CONFIG_DIR", std::path::Path::new("/tmp/folio-conf-config"));
    assert_eq!(config_dir("folio-x"), std::path::PathBuf::from("/tmp/folio-conf-config"));
    assert!(home_dir().is_absolute());
    assert!(documents_dir().is_absolute());
    let places = places();
    assert!(places.len() >= 2 && places.iter().all(|(n, p)| !n.is_empty() && p.is_absolute()));
    assert!(temp_file("x", "pdf").to_string_lossy().ends_with(".pdf"));
}

#[test]
fn sockets_carry_bytes_both_ways() {
    let l = net::listen_local().unwrap();
    let addr = l.local_addr().unwrap();
    let t = spawn("conf-server", move || {
        let (mut s, _) = l.accept().unwrap();
        let mut b = [0u8; 5];
        s.read_exact(&mut b).unwrap();
        s.write_all(&b.map(|c| c.to_ascii_uppercase())).unwrap();
    });
    let mut c = net::connect(&addr, Duration::from_secs(2)).unwrap();
    c.write_all(b"folio").unwrap();
    let mut b = [0u8; 5];
    c.read_exact(&mut b).unwrap();
    assert_eq!(&b, b"FOLIO");
    t.join().unwrap();
    let ip: std::net::IpAddr = net::local_ip().parse().unwrap();
    assert!(!ip.is_unspecified());
}

#[test]
fn threads_and_sleep() {
    let (tx, rx) = std::sync::mpsc::channel();
    let h = spawn("conf-worker", move || {
        sleep(Duration::from_millis(20));
        tx.send(std::thread::current().name().map(String::from)).unwrap();
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap().as_deref(), Some("conf-worker"));
    h.join().unwrap();
}

#[test]
fn identity_randomness_and_memory() {
    assert!(!user_name().is_empty());
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    random_bytes(&mut a).unwrap();
    random_bytes(&mut b).unwrap();
    assert_ne!(a, b);
    assert_ne!(random_u64(), random_u64());
    if cfg!(target_os = "linux") {
        assert!(memory_used().unwrap() > 1024 * 1024);
        assert!(memory_peak().unwrap() >= memory_used().unwrap());
    }
}

#[test]
fn clicks_are_counted() {
    let mut c = ClickCounter::default();
    let t = std::time::Instant::now();
    assert_eq!(c.press(t, 10.0, 10.0, Button::Left, 1.0), 1);
    assert_eq!(c.press(t + Duration::from_millis(200), 12.0, 11.0, Button::Left, 1.0), 2);
    assert_eq!(c.press(t + Duration::from_millis(400), 12.0, 11.0, Button::Left, 1.0), 3);
    assert_eq!(c.press(t + Duration::from_millis(1000), 12.0, 11.0, Button::Left, 1.0), 1, "too slow");
    assert_eq!(c.press(t + Duration::from_millis(1100), 40.0, 11.0, Button::Left, 1.0), 1, "too far");
}

/// The headless backend keeps the same contract as a window.
struct Probe {
    events: Vec<Event>,
    painted: Option<(u32, u32)>,
    ticks: u32,
}

impl App for Probe {
    fn event(&mut self, ev: &Event, host: &mut dyn Host) {
        if self.events.is_empty() {
            assert!(self.painted.is_none(), "painted before the first Resized");
        }
        self.events.push(ev.clone());
        if *ev == Event::Tick {
            self.ticks += 1;
            if self.ticks == 1 {
                host.set_clipboard_text("x");
                assert_eq!(host.clipboard_text().as_deref(), Some("x"));
            }
        }
    }
    fn paint(&mut self, px: &mut [u8], w: u32, h: u32, _host: &mut dyn Host) {
        assert_eq!(px.len(), (w * h * 4) as usize);
        px.fill(200);
        self.painted = Some((w, h));
    }
    fn next_tick(&self) -> Option<Duration> {
        Some(Duration::from_millis(100))
    }
    fn wants_exit(&self) -> bool {
        self.ticks >= 5
    }
    fn exit_code(&self) -> i32 {
        7
    }
}

#[test]
fn headless_runs_an_app_like_a_window() {
    let mut p = Probe { events: vec![], painted: None, ticks: 0 };
    let mut frame = Vec::new();
    let code = run_headless(&mut p, 320, 200, 1.5, 100, &mut frame);
    assert_eq!(code, Some(7));
    assert_eq!(p.events[0], Event::Resized { w: 320, h: 200, scale: 1.5 });
    assert_eq!(p.painted, Some((320, 200)));
    assert_eq!(frame.len(), 320 * 200 * 4);
    assert!(frame.iter().all(|&b| b == 200));
}

#[cfg(feature = "audio")]
#[test]
fn sound_output_plays_or_says_why_not() {
    let r = audio::audio_out(48_000, 2, |buf| buf.fill(0.0));
    match r {
        Ok(a) => {
            assert_eq!((a.rate, a.channels), (48_000, 2));
            sleep(Duration::from_millis(100));
        }
        Err(e) => {
            assert!(!e.is_empty());
            eprintln!("no sound device here: {e}");
        }
    }
}
