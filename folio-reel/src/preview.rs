//! The preview: the picture under the playhead, and the sound of the timeline.
//!
//! Decoding is slow, so it happens on its own thread and the window draws
//! whatever has come back. The sound is rendered once, as plain samples, and
//! played from wherever the playhead is.

use folio_kit::gfx::sk;
use folio_kit::platform::media;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// A picture that has come back from the decoder, and where in the film it is.
pub struct Shot {
    pub at: f64,
    pub pm: Arc<sk::Pixmap>,
}


enum Ask {
    Frame { path: PathBuf, at: f64, width: u32, token: u64 },
    Stop,
}

/// Pictures, decoded one at a time, newest ask first.
pub struct Frames {
    tx: Option<Sender<Ask>>,
    rx: Receiver<(u64, f64, Option<(u32, u32, Vec<u8>)>)>,
    token: u64,
    want: u64,
    /// the last picture that came back, and the moment it belongs to
    pub shot: Option<Shot>,
    pub busy: bool,
    /// what the decoder said, if it couldn't
    pub trouble: Option<String>,
    cache: Vec<(String, u64, Arc<sk::Pixmap>)>,
    pending_key: Option<(String, u64)>,
}

impl Default for Frames {
    fn default() -> Self {
        Frames::new()
    }
}

impl Frames {
    pub fn new() -> Frames {
        let (tx, ask) = channel::<Ask>();
        let (back, rx) = channel();
        std::thread::spawn(move || {
            while let Ok(a) = ask.recv() {
                match a {
                    Ask::Stop => break,
                    Ask::Frame { path, at, width, token } => {
                        let got = media::frame(&path, at, width).ok().map(|f| (f.w, f.h, f.rgba));
                        if back.send((token, at, got)).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Frames { tx: Some(tx), rx, token: 0, want: 0, shot: None, busy: false, trouble: None, cache: Vec::new(), pending_key: None }
    }

    /// Ask for the picture of `path` at `at`. Rounded to a fifth of a second so
    /// scrubbing reuses what is already in hand.
    pub fn ask(&mut self, path: &Path, at: f64, width: u32) {
        let key = path.to_string_lossy().to_string();
        let stamp = (at * 5.0).round() as u64;
        if let Some((_, _, pm)) = self.cache.iter().find(|(k, s, _)| *k == key && *s == stamp) {
            self.shot = Some(Shot { at, pm: pm.clone() });
            self.busy = false;
            return;
        }
        self.token += 1;
        self.want = self.token;
        self.busy = true;
        if let Some(tx) = &self.tx {
            let _ = tx.send(Ask::Frame { path: path.to_path_buf(), at, width, token: self.token });
        }
        self.pending_key = Some((key, stamp));
    }

    /// Take whatever has come back. True if the picture changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((token, at, got)) = self.rx.try_recv() {
            if token != self.want {
                continue;
            }
            self.busy = false;
            match got {
                Some((w, h, rgba)) => {
                    if let Some(pm) = sk::Pixmap::from_vec(rgba, sk::IntSize::from_wh(w.max(1), h.max(1)).unwrap()) {
                        let pm = Arc::new(pm);
                        if let Some((key, stamp)) = self.pending_key.take() {
                            self.cache.push((key, stamp, pm.clone()));
                            if self.cache.len() > 48 {
                                self.cache.remove(0);
                            }
                        }
                        self.shot = Some(Shot { at, pm });
                        self.trouble = None;
                        changed = true;
                    }
                }
                None => {
                    self.pending_key = None;
                    self.shot = None;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn forget(&mut self) {
        self.cache.clear();
        self.shot = None;
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Ask::Stop);
        }
    }
}

/// The timeline's sound, rendered once and played from any point.
pub struct Sound {
    samples: Arc<Mutex<Vec<i16>>>,
    at: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    /// what went wrong, if it did
    trouble: Arc<Mutex<Option<String>>>,
    stream: Option<Box<dyn std::any::Any + Send>>,
}

const RATE: u32 = 48_000;

impl Default for Sound {
    fn default() -> Self {
        Sound::new()
    }
}

impl Sound {
    pub fn new() -> Sound {
        Sound {
            samples: Arc::new(Mutex::new(Vec::new())),
            at: Arc::new(AtomicU64::new(0)),
            playing: Arc::new(AtomicBool::new(false)),
            ready: Arc::new(AtomicBool::new(false)),
            trouble: Arc::new(Mutex::new(None)),
            stream: None,
        }
    }

    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    pub fn trouble(&self) -> Option<String> {
        self.trouble.lock().ok().and_then(|t| t.clone())
    }

    /// Render the timeline's sound in the background. `args` comes from the
    /// engine; `raw` is where the samples go.
    pub fn render(&mut self, args: Vec<String>, raw: PathBuf) {
        self.ready.store(false, Ordering::Relaxed);
        if let Ok(mut s) = self.samples.lock() {
            s.clear();
        }
        let (samples, ready, trouble) = (self.samples.clone(), self.ready.clone(), self.trouble.clone());
        std::thread::spawn(move || {
            let outcome = media::export(&args, 0.0, |_| {}).and_then(|()| std::fs::read(&raw).map_err(|e| e.to_string()));
            match outcome {
                Ok(bytes) => {
                    let v: Vec<i16> = bytes.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
                    if let Ok(mut s) = samples.lock() {
                        *s = v;
                    }
                    if let Ok(mut t) = trouble.lock() {
                        *t = None;
                    }
                    ready.store(true, Ordering::Relaxed);
                }
                Err(e) => {
                    if let Ok(mut t) = trouble.lock() {
                        *t = Some(e);
                    }
                }
            }
            let _ = std::fs::remove_file(&raw);
        });
    }

    /// Open the sound card, once. Nothing to say if there isn't one.
    fn open(&mut self) {
        if self.stream.is_some() {
            return;
        }
        let (samples, at, playing) = (self.samples.clone(), self.at.clone(), self.playing.clone());
        let channels = 2usize;
        let fill = move |out: &mut [f32]| {
            let on = playing.load(Ordering::Relaxed);
            let frames = out.len() / channels.max(1);
            if !on {
                out.fill(0.0);
                return;
            }
            let pos = at.load(Ordering::Relaxed) as usize;
            let s = match samples.lock() {
                Ok(s) => s,
                Err(_) => {
                    out.fill(0.0);
                    return;
                }
            };
            for i in 0..frames {
                let j = (pos + i) * 2;
                let (l, r) = if j + 1 < s.len() {
                    (s[j] as f32 / 32768.0, s[j + 1] as f32 / 32768.0)
                } else {
                    (0.0, 0.0)
                };
                for c in 0..channels {
                    out[i * channels + c] = if c == 0 { l } else { r };
                }
            }
            at.store((pos + frames) as u64, Ordering::Relaxed);
        };
        match folio_kit::platform::audio::audio_out(RATE, 2, fill) {
            Ok(s) => self.stream = Some(Box::new(s)),
            Err(e) => {
                if let Ok(mut t) = self.trouble.lock() {
                    *t = Some(e);
                }
            }
        }
    }

    pub fn play(&mut self, from: f64) {
        self.open();
        self.seek(from);
        self.playing.store(true, Ordering::Relaxed);
    }

    pub fn pause(&mut self) {
        self.playing.store(false, Ordering::Relaxed);
    }

    pub fn seek(&mut self, to: f64) {
        self.at.store((to.max(0.0) * RATE as f64) as u64, Ordering::Relaxed);
    }

    /// Where the sound has got to, in seconds.
    pub fn position(&self) -> f64 {
        self.at.load(Ordering::Relaxed) as f64 / RATE as f64
    }
}
