//! Playing songs: a decoding thread fills a ring, and the sound card's callback
//! drains it. Gapless, crossfade, Sound Check (ReplayGain), the ten-band
//! equalizer and the volume all happen on the way through.

use crate::tags;
use folio_math::Det;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// How the ten bands of the equaliser sit, in hertz, as iTunes had them.
pub const BANDS: [f64; 10] = [32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

/// One band of a graphic equaliser: a peaking filter at a fixed frequency.
#[derive(Clone, Copy, Debug, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    /// A peaking filter of `gain` decibels at `f0`, with the usual Q of 1.
    fn peaking(f0: f64, gain_db: f64, rate: f64) -> Biquad {
        let a = (10f64).dpow(gain_db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * f0 / rate.max(1.0);
        let (sn, cs) = (w0.dsin(), w0.dcos());
        let alpha = sn / 2.0;
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cs;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cs;
        let a2 = 1.0 - alpha / a;
        Biquad { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, ..Default::default() }
    }

    fn run(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// How the song is playing: what the window shows under the transport.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub track: Option<u64>,
    pub path: PathBuf,
    pub playing: bool,
    /// seconds into the song
    pub position: f64,
    pub duration: f64,
    /// true the moment a song runs out, so the app can count the play
    pub finished: bool,
    /// set when a song could not be opened at all (a format with no decoder here)
    pub error: Option<String>,
}

/// What the app asks the player to do.
enum Cmd {
    Play(u64, PathBuf, f64),
    Pause,
    Resume,
    Stop,
    Seek(f64),
    Volume(f64),
    Gain(f64),
    Eq([f64; 10], bool),
    Crossfade(f64),
}

/// The handle the app holds.
pub struct Player {
    tx: std::sync::mpsc::Sender<Cmd>,
    status: Arc<Mutex<Status>>,
    /// samples ready to be played, interleaved at the output's rate
    ring: Arc<Mutex<VecDeque<f32>>>,
    pub rate: u32,
    pub channels: u16,
    stop: Arc<AtomicBool>,
    pos_frames: Arc<AtomicU64>,
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// The samples of one song, read as they are needed.
struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    rate: u32,
    channels: usize,
    /// what is left of the last decoded packet, interleaved
    left: VecDeque<f32>,
    done: bool,
}

impl Source {
    fn open(path: &Path) -> Option<Source> {
        let file = std::fs::File::open(path).ok()?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_string();
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if !ext.is_empty() {
            hint.with_extension(&ext);
        }
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
            .ok()?;
        let format = probed.format;
        let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL)?;
        let track_id = track.id;
        let rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);
        let decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).ok()?;
        Some(Source { format, decoder, track_id, rate, channels, left: VecDeque::new(), done: false })
    }

    fn seek(&mut self, secs: f64) {
        let _ = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time { time: Time::from(secs.max(0.0)), track_id: Some(self.track_id) },
        );
        self.decoder.reset();
        self.left.clear();
        self.done = false;
    }

    /// Fill `left` with the next packet. False when the song has run out.
    fn more(&mut self) -> bool {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => {
                    self.done = true;
                    return false;
                }
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buf) => {
                    push_buffer(&buf, &mut self.left);
                    return true;
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(_) => {
                    self.done = true;
                    return false;
                }
            }
        }
    }

    /// Take up to `n` interleaved samples.
    fn take(&mut self, n: usize, out: &mut Vec<f32>) -> usize {
        let mut got = 0;
        while got < n {
            if self.left.is_empty() && !self.more() {
                break;
            }
            while got < n {
                match self.left.pop_front() {
                    Some(s) => {
                        out.push(s);
                        got += 1;
                    }
                    None => break,
                }
            }
        }
        got
    }
}

fn push_buffer(buf: &AudioBufferRef, out: &mut VecDeque<f32>) {
    match buf {
        AudioBufferRef::F32(b) => {
            let ch = b.spec().channels.count();
            for f in 0..b.frames() {
                for c in 0..ch {
                    out.push_back(b.chan(c)[f]);
                }
            }
        }
        AudioBufferRef::S32(b) => {
            let ch = b.spec().channels.count();
            for f in 0..b.frames() {
                for c in 0..ch {
                    out.push_back(b.chan(c)[f] as f32 / i32::MAX as f32);
                }
            }
        }
        AudioBufferRef::S16(b) => {
            let ch = b.spec().channels.count();
            for f in 0..b.frames() {
                for c in 0..ch {
                    out.push_back(b.chan(c)[f] as f32 / 32768.0);
                }
            }
        }
        AudioBufferRef::U8(b) => {
            let ch = b.spec().channels.count();
            for f in 0..b.frames() {
                for c in 0..ch {
                    out.push_back((b.chan(c)[f] as f32 - 128.0) / 128.0);
                }
            }
        }
        other => {
            // the rest go through a copy into f32
            let mut tmp = other.make_equivalent::<f32>();
            other.convert(&mut tmp);
            let ch = tmp.spec().channels.count();
            for f in 0..tmp.frames() {
                for c in 0..ch {
                    out.push_back(tmp.chan(c)[f]);
                }
            }
        }
    }
}

/// How full the ring is kept, in frames: a quarter of a second at 48 kHz.
const RING_FRAMES: usize = 12_000;

impl Player {
    /// Start the decoding thread. `rate` and `channels` are what the sound
    /// output wants; the player resamples to them.
    pub fn new(rate: u32, channels: u16) -> Player {
        let (tx, rx) = std::sync::mpsc::channel::<Cmd>();
        let status = Arc::new(Mutex::new(Status::default()));
        let ring = Arc::new(Mutex::new(VecDeque::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let pos_frames = Arc::new(AtomicU64::new(0));
        let st = status.clone();
        let rg = ring.clone();
        let sp = stop.clone();
        let pf = pos_frames.clone();
        folio_kit::platform::spawn("chord-decode", move || {
            run_decoder(rx, st, rg, sp, pf, rate, channels as usize);
        });
        Player { tx, status, ring, rate, channels, stop, pos_frames }
    }

    pub fn play(&self, id: u64, path: &Path, from: f64) {
        let _ = self.tx.send(Cmd::Play(id, path.to_path_buf(), from));
    }

    pub fn pause(&self) {
        let _ = self.tx.send(Cmd::Pause);
    }

    pub fn resume(&self) {
        let _ = self.tx.send(Cmd::Resume);
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }

    pub fn seek(&self, secs: f64) {
        let _ = self.tx.send(Cmd::Seek(secs));
    }

    pub fn set_volume(&self, v: f64) {
        let _ = self.tx.send(Cmd::Volume(v.clamp(0.0, 1.0)));
    }

    /// Sound Check: the song's own loudness, in decibels.
    pub fn set_gain_db(&self, db: f64) {
        let _ = self.tx.send(Cmd::Gain(db));
    }

    pub fn set_eq(&self, bands: [f64; 10], on: bool) {
        let _ = self.tx.send(Cmd::Eq(bands, on));
    }

    pub fn set_crossfade(&self, secs: f64) {
        let _ = self.tx.send(Cmd::Crossfade(secs.clamp(0.0, 12.0)));
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Mark the "a song ended" flag as seen.
    pub fn take_finished(&self) -> bool {
        let mut s = self.status.lock().unwrap();
        let f = s.finished;
        s.finished = false;
        f
    }

    /// The last song that could not be played at all, once.
    pub fn take_error(&self) -> Option<String> {
        let mut s = self.status.lock().unwrap();
        s.error.take()
    }

    /// What the sound output's callback calls: fill `out` with what is ready.
    pub fn fill(&self, out: &mut [f32]) {
        let mut ring = self.ring.lock().unwrap();
        for v in out.iter_mut() {
            *v = ring.pop_front().unwrap_or(0.0);
        }
        self.pos_frames.fetch_add((out.len() / self.channels.max(1) as usize) as u64, Ordering::Relaxed);
    }

    /// A closure for `folio_platform::audio::audio_out`, sharing this player's ring.
    pub fn filler(&self) -> impl FnMut(&mut [f32]) + Send + 'static {
        let ring = self.ring.clone();
        let channels = self.channels.max(1) as usize;
        let pos = self.pos_frames.clone();
        move |out: &mut [f32]| {
            let mut r = ring.lock().unwrap();
            for v in out.iter_mut() {
                *v = r.pop_front().unwrap_or(0.0);
            }
            pos.fetch_add((out.len() / channels) as u64, Ordering::Relaxed);
        }
    }
}

struct Mix {
    src: Option<Source>,
    /// the song fading out under the new one
    fading: Option<(Source, f64, f64)>,
    gain: f64,
    volume: f64,
    eq_on: bool,
    eq: Vec<Biquad>,
    bands: [f64; 10],
    crossfade: f64,
    playing: bool,
    id: u64,
    path: PathBuf,
    /// frames handed to the ring since this song started
    frames: u64,
}

fn run_decoder(
    rx: std::sync::mpsc::Receiver<Cmd>,
    status: Arc<Mutex<Status>>,
    ring: Arc<Mutex<VecDeque<f32>>>,
    stop: Arc<AtomicBool>,
    _pos: Arc<AtomicU64>,
    rate: u32,
    channels: usize,
) {
    let mut m = Mix {
        src: None,
        fading: None,
        gain: 1.0,
        volume: 1.0,
        eq_on: false,
        eq: Vec::new(),
        bands: [0.0; 10],
        crossfade: 0.0,
        playing: false,
        id: 0,
        path: PathBuf::new(),
        frames: 0,
    };
    let mut scratch: Vec<f32> = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        while let Ok(c) = rx.try_recv() {
            if let Some(e) = apply(&mut m, c, rate, channels, &ring) {
                if let Ok(mut s) = status.lock() {
                    s.error = Some(e);
                    s.playing = false;
                    s.finished = true;
                }
            }
        }
        if let Ok(mut st) = status.lock() {
            if st.playing != m.playing {
                st.playing = m.playing;
            }
        }
        let want = {
            let r = ring.lock().unwrap();
            RING_FRAMES.saturating_sub(r.len() / channels.max(1))
        };
        if !m.playing || m.src.is_none() || want < 256 {
            folio_kit::platform::sleep(std::time::Duration::from_millis(8));
            continue;
        }
        scratch.clear();
        let made = render(&mut m, want, rate, channels, &mut scratch);
        if made == 0 {
            // the song ran out
            m.playing = false;
            m.src = None;
            if let Ok(mut s) = status.lock() {
                s.playing = false;
                s.finished = true;
            }
            continue;
        }
        {
            let mut r = ring.lock().unwrap();
            r.extend(scratch.iter().copied());
        }
        m.frames += (made / channels.max(1)) as u64;
        if let Ok(mut s) = status.lock() {
            s.track = Some(m.id);
            s.path = m.path.clone();
            s.playing = m.playing;
            s.position = m.frames as f64 / rate as f64;
        }
    }
}

fn apply(m: &mut Mix, c: Cmd, rate: u32, channels: usize, ring: &Arc<Mutex<VecDeque<f32>>>) -> Option<String> {
    match c {
        Cmd::Play(id, path, from) => {
            let fading = if m.crossfade > 0.0 { m.src.take().map(|s| (s, m.crossfade, 0.0)) } else { None };
            m.fading = fading;
            match Source::open(&path) {
                Some(mut s) => {
                    if from > 0.0 {
                        s.seek(from);
                    }
                    m.src = Some(s);
                    m.playing = true;
                    m.id = id;
                    m.path = path;
                    m.frames = (from * rate as f64) as u64;
                    if m.fading.is_none() {
                        ring.lock().unwrap().clear();
                    }
                }
                None => {
                    m.src = None;
                    m.playing = false;
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    return Some(format!("Chord can't play {name} yet."));
                }
            }
            let _ = channels;
        }
        Cmd::Pause => m.playing = false,
        Cmd::Resume => m.playing = m.src.is_some(),
        Cmd::Stop => {
            m.src = None;
            m.fading = None;
            m.playing = false;
            ring.lock().unwrap().clear();
        }
        Cmd::Seek(secs) => {
            if let Some(s) = m.src.as_mut() {
                s.seek(secs);
                m.frames = (secs * rate as f64) as u64;
                ring.lock().unwrap().clear();
            }
        }
        Cmd::Volume(v) => m.volume = v,
        Cmd::Gain(db) => m.gain = (10f64).dpow(db / 20.0),
        Cmd::Eq(b, on) => {
            m.bands = b;
            m.eq_on = on;
            m.eq = (0..channels)
                .flat_map(|_| BANDS.iter().enumerate().map(|(i, f)| Biquad::peaking(*f, b[i], rate as f64)).collect::<Vec<_>>())
                .collect();
        }
        Cmd::Crossfade(v) => m.crossfade = v,
    }
    None
}

/// Take `frames` frames from the song (and the one fading out), resample them to
/// the output's rate, and put them through the gain and the equaliser.
fn render(m: &mut Mix, frames: usize, rate: u32, channels: usize, out: &mut Vec<f32>) -> usize {
    let Some(src) = m.src.as_mut() else { return 0 };
    let src_rate = src.rate.max(1);
    let src_ch = src.channels.max(1);
    let need = ((frames as f64 * src_rate as f64 / rate as f64).ceil() as usize + 2) * src_ch;
    let mut raw: Vec<f32> = Vec::with_capacity(need);
    let got = src.take(need, &mut raw);
    if got == 0 {
        return 0;
    }
    let in_frames = got / src_ch;
    let out_frames = ((in_frames as f64 * rate as f64 / src_rate as f64).floor() as usize).max(1);
    let start = out.len();
    for f in 0..out_frames {
        let pos = f as f64 * src_rate as f64 / rate as f64;
        let i = pos.floor() as usize;
        let frac = pos - i as f64;
        for c in 0..channels {
            let sc = c.min(src_ch - 1);
            let a = raw.get(i * src_ch + sc).copied().unwrap_or(0.0) as f64;
            let b = raw.get((i + 1) * src_ch + sc).copied().unwrap_or(a as f32) as f64;
            let mut v = a + (b - a) * frac;
            v *= m.gain * m.volume;
            if m.eq_on {
                let base = c * BANDS.len();
                for k in 0..BANDS.len() {
                    if let Some(q) = m.eq.get_mut(base + k) {
                        v = q.run(v);
                    }
                }
            }
            out.push(v.clamp(-1.0, 1.0) as f32);
        }
    }
    // the song fading out underneath
    if let Some((fs, total, done)) = m.fading.as_mut() {
        let mut raw2: Vec<f32> = Vec::new();
        let fch = fs.channels.max(1);
        let frate = fs.rate.max(1);
        let n2 = ((out_frames as f64 * frate as f64 / rate as f64).ceil() as usize + 2) * fch;
        let got2 = fs.take(n2, &mut raw2);
        if got2 == 0 {
            m.fading = None;
        } else {
            let step = 1.0 / (*total * rate as f64);
            for f in 0..out_frames {
                let k = (1.0 - (*done + f as f64 * step)).clamp(0.0, 1.0);
                let pos = f as f64 * frate as f64 / rate as f64;
                let i = pos.floor() as usize;
                for c in 0..channels {
                    let sc = c.min(fch - 1);
                    let v = raw2.get(i * fch + sc).copied().unwrap_or(0.0) as f64 * k * m.volume;
                    let at = start + f * channels + c;
                    if let Some(o) = out.get_mut(at) {
                        *o = (*o as f64 + v).clamp(-1.0, 1.0) as f32;
                    }
                }
            }
            *done += out_frames as f64 * step;
            if *done >= 1.0 {
                m.fading = None;
            }
        }
    }
    out.len() - start
}

/// How long a file plays for, without opening the whole library.
pub fn duration_of(path: &Path) -> f64 {
    tags::read(path).map(|t| t.duration).unwrap_or(0.0)
}
