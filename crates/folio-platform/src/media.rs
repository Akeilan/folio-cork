//! Video and sound files, through whatever is on the computer.
//!
//! Reel is the one app in the suite that cannot carry its own format readers:
//! H.264 and AAC are not ours to write. So the platform layer hands a file to
//! ffmpeg when the computer has it, and says so plainly when it does not. The
//! rest of the suite never calls in here.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What a media file is made of.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub path: PathBuf,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_video: bool,
    pub has_audio: bool,
    pub sample_rate: u32,
    pub channels: u16,
    /// "h264", "vp9", "png"...
    pub video_codec: String,
    pub audio_codec: String,
}

impl Info {
    pub fn is_image(&self) -> bool {
        self.has_video && !self.has_audio && (self.duration <= 0.0 || matches!(self.video_codec.as_str(), "png" | "mjpeg" | "bmp" | "gif" | "webp"))
    }
}

/// A picture pulled out of a file: straight RGBA, as the canvas wants it.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

fn tool(name: &str) -> Option<String> {
    let ok = Command::new(name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then(|| name.to_string())
}

/// Is there anything on this computer that can read video?
pub fn have() -> bool {
    // the window test uses this to prove what Reel says on a computer without it
    if crate::services::setting("FOLIO_NO_MEDIA").map(|v| v == "1").unwrap_or(false) {
        return false;
    }
    tool("ffmpeg").is_some() && tool("ffprobe").is_some()
}

/// What to tell someone when there is not.
pub fn missing_message() -> String {
    "This computer has no ffmpeg, so Reel can't read or write video here. Install ffmpeg and open the file again.".into()
}

fn ffprobe_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key && !v.trim().is_empty() && v.trim() != "N/A" {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

fn frac(s: &str) -> f64 {
    match s.split_once('/') {
        Some((a, b)) => {
            let (a, b): (f64, f64) = (a.trim().parse().unwrap_or(0.0), b.trim().parse().unwrap_or(1.0));
            if b.abs() < 1e-9 {
                0.0
            } else {
                a / b
            }
        }
        None => s.trim().parse().unwrap_or(0.0),
    }
}

/// Ask what is in a file.
pub fn probe(path: &Path) -> Result<Info, String> {
    if !have() {
        return Err(missing_message());
    }
    let run = |args: &[&str]| -> String {
        Command::new("ffprobe")
            .args(["-v", "error"])
            .args(args)
            .arg(path)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default()
    };
    let fmt = run(&["-show_entries", "format=duration", "-of", "default=noprint_wrappers=1"]);
    let v = run(&["-select_streams", "v:0", "-show_entries", "stream=width,height,avg_frame_rate,codec_name,duration", "-of", "default=noprint_wrappers=1"]);
    let a = run(&["-select_streams", "a:0", "-show_entries", "stream=sample_rate,channels,codec_name,duration", "-of", "default=noprint_wrappers=1"]);
    if fmt.trim().is_empty() && v.trim().is_empty() && a.trim().is_empty() {
        return Err(format!("{} isn't a media file this computer can read.", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    }
    let mut info = Info { path: path.to_path_buf(), ..Default::default() };
    info.duration = ffprobe_value(&fmt, "duration")
        .or_else(|| ffprobe_value(&v, "duration"))
        .or_else(|| ffprobe_value(&a, "duration"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);
    if let Some(w) = ffprobe_value(&v, "width").and_then(|s| s.parse().ok()) {
        info.has_video = true;
        info.width = w;
        info.height = ffprobe_value(&v, "height").and_then(|s| s.parse().ok()).unwrap_or(0);
        info.fps = ffprobe_value(&v, "avg_frame_rate").map(|s| frac(&s)).unwrap_or(0.0);
        info.video_codec = ffprobe_value(&v, "codec_name").unwrap_or_default();
    }
    if let Some(r) = ffprobe_value(&a, "sample_rate").and_then(|s| s.parse().ok()) {
        info.has_audio = true;
        info.sample_rate = r;
        info.channels = ffprobe_value(&a, "channels").and_then(|s| s.parse().ok()).unwrap_or(2);
        info.audio_codec = ffprobe_value(&a, "codec_name").unwrap_or_default();
    }
    Ok(info)
}

/// One picture out of a file, at `at` seconds, no wider than `width`.
pub fn frame(path: &Path, at: f64, width: u32) -> Result<Frame, String> {
    if !have() {
        return Err(missing_message());
    }
    let scale = format!("scale={}:-2", width.max(2));
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{:.3}", at.max(0.0)), "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", &scale, "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .map_err(|e| e.to_string())?;
    if out.stdout.is_empty() {
        return Err("There's no picture there.".into());
    }
    // the height comes from how many bytes came back
    let probed = probe(path)?;
    let w = width.max(2) & !1;
    let h = if probed.width > 0 {
        let h = (w as f64 * probed.height as f64 / probed.width as f64).round() as u32;
        h.max(2) & !1
    } else {
        (out.stdout.len() as u32 / (w * 4)).max(1)
    };
    let want = (w * h * 4) as usize;
    let mut rgba = out.stdout;
    if rgba.len() < want {
        rgba.resize(want, 0);
    }
    rgba.truncate(want);
    Ok(Frame { w, h, rgba })
}

/// The sound of a file as peaks, `points` of them, for drawing a waveform.
pub fn waveform(path: &Path, points: usize) -> Result<Vec<f32>, String> {
    if !have() {
        return Err(missing_message());
    }
    let mut child = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-ac", "1", "-ar", "8000", "-f", "s16le", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_end(&mut raw);
    }
    let _ = child.wait();
    let samples: Vec<i16> = raw.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
    if samples.is_empty() {
        return Ok(Vec::new());
    }
    let n = points.max(1);
    let per = (samples.len() / n).max(1);
    Ok((0..n)
        .map(|i| {
            let from = i * per;
            let to = ((i + 1) * per).min(samples.len());
            let peak = samples[from..to.max(from + 1).min(samples.len())].iter().map(|s| s.unsigned_abs() as f32).fold(0.0, f32::max);
            peak / 32768.0
        })
        .collect())
}

/// Run an export. `args` is everything after the program name; `on_progress`
/// is called with the seconds written so far.
pub fn export(args: &[String], total: f64, mut on_progress: impl FnMut(f64)) -> Result<(), String> {
    if !have() {
        return Err(missing_message());
    }
    let mut child = Command::new("ffmpeg")
        .args(args)
        .args(["-progress", "pipe:1", "-nostats"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(out) = child.stdout.take() {
        use std::io::{BufRead, BufReader};
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(v) = line.strip_prefix("out_time_ms=") {
                if let Ok(us) = v.trim().parse::<i64>() {
                    on_progress((us as f64 / 1.0e6).min(total.max(0.001)));
                }
            }
        }
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("The export didn't finish.");
        Err(last.to_string())
    }
}

/// Where Reel keeps the pictures it has already pulled out.
pub fn cache_dir() -> PathBuf {
    let d = crate::services::temp_dir().join("folio-reel-frames");
    let _ = std::fs::create_dir_all(&d);
    d
}
