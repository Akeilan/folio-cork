//! Pictures from a scanner or a camera (D4-01), through whatever is on the computer.
//!
//! Scanners talk SANE and cameras talk V4L2, and neither is ours to write. So this module hands
//! the job to the tools the distribution ships — `scanimage` from sane-utils, and ffmpeg for a
//! camera — and says plainly when they are not installed. A picture comes back as PNG bytes,
//! which every Folio app can already read. Nothing here reaches the network.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// A device a picture can come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// What to pass back to `scan` or `snap` (a SANE name, or /dev/videoN).
    pub id: String,
    /// What to show the person.
    pub name: String,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Scanner,
    Camera,
}

fn disabled() -> bool {
    crate::services::setting("FOLIO_NO_CAPTURE").map(|v| v == "1").unwrap_or(false)
}

/// A command to run in place of scanimage and ffmpeg: the program, then any leading arguments,
/// separated by spaces (so `sh /tmp/stub.sh` works without the file being executable). The window
/// test points this at a stub that writes a picture, so the arguments Folio builds are run for
/// real on a machine with no scanner on the bench.
fn stub() -> Option<Vec<String>> {
    if disabled() {
        return None;
    }
    let v = crate::services::setting("FOLIO_CAPTURE_STUB")?;
    let parts: Vec<String> = v.split_whitespace().map(|s| s.to_string()).collect();
    (!parts.is_empty()).then_some(parts)
}

fn tool(name: &str, arg: &str) -> bool {
    if disabled() {
        return false;
    }
    if stub().is_some() {
        return true;
    }
    Command::new(name).arg(arg).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn program(real: &str) -> Command {
    match stub() {
        Some(parts) => {
            let mut c = Command::new(&parts[0]);
            c.args(&parts[1..]);
            c.arg(real);
            c
        }
        None => Command::new(real),
    }
}

/// Is there anything on this computer that can drive a scanner?
pub fn have_scanner_support() -> bool {
    tool("scanimage", "--version")
}

/// Is there anything on this computer that can read a camera?
pub fn have_camera_support() -> bool {
    tool("ffmpeg", "-version")
}

/// What to tell someone when the scanning tool is missing.
pub fn scanner_missing() -> String {
    "This computer has no scanimage, so Folio can't reach a scanner here. Install the sane-utils \
     package and try again."
        .into()
}

/// What to tell someone when the camera tool is missing.
pub fn camera_missing() -> String {
    "This computer has no ffmpeg, so Folio can't read a camera here. Install the ffmpeg package \
     and try again."
        .into()
}

/// What to tell someone when the tools are there but nothing is plugged in.
pub fn nothing_found() -> String {
    "No scanner or camera was found. Plug one in, switch it on, and try again.".into()
}

/// Every scanner SANE can see. An empty list is the usual answer on a machine with none.
pub fn scanners() -> Vec<Device> {
    if !have_scanner_support() {
        return Vec::new();
    }
    let out = match program("scanimage").args(["--formatted-device-list", "%d|%v %m%n"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return Vec::new(),
    };
    out.lines()
        .filter_map(|l| l.split_once('|'))
        .filter(|(id, _)| !id.trim().is_empty())
        .map(|(id, name)| Device {
            id: id.trim().to_string(),
            name: if name.trim().is_empty() { id.trim().to_string() } else { name.trim().to_string() },
            kind: Kind::Scanner,
        })
        .collect()
}

/// Every camera the kernel has a device file for, newest-looking name first.
pub fn cameras() -> Vec<Device> {
    if disabled() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if stub().is_some() {
        out.push(Device { id: "/dev/video0".into(), name: "Test Camera".into(), kind: Kind::Camera });
        return out;
    }
    for i in 0..10u8 {
        let dev = format!("/dev/video{i}");
        if !std::path::Path::new(&dev).exists() {
            continue;
        }
        // the kernel publishes each device's own name; without it, the device file will do
        let name = std::fs::read_to_string(format!("/sys/class/video4linux/video{i}/name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        // a capture device has a "video_capture" capability; cards also publish metadata nodes
        let caps = std::fs::read_to_string(format!("/sys/class/video4linux/video{i}/device_caps")).unwrap_or_default();
        let bits = u32::from_str_radix(caps.trim().trim_start_matches("0x"), 16).unwrap_or(0x1);
        if bits != 0 && bits & 0x1 == 0 {
            continue;                       // V4L2_CAP_VIDEO_CAPTURE is not set: not a camera
        }
        out.push(Device {
            id: dev.clone(),
            name: if name.is_empty() { dev } else { name },
            kind: Kind::Camera,
        });
    }
    out
}

/// Every scanner and camera on the computer.
pub fn devices() -> Vec<Device> {
    let mut v = scanners();
    v.extend(cameras());
    v
}

fn run(mut cmd: Command, wait: Duration, what: &str) -> Result<Vec<u8>, String> {
    let out = PathBuf::from(crate::services::temp_file("folio-capture", "png"));
    let _ = std::fs::remove_file(&out);
    cmd.arg(&out);
    let mut child = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("{what} couldn't be started: {e}"))?;
    let end = std::time::Instant::now() + wait;
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                if !st.success() {
                    let mut msg = String::new();
                    if let Some(mut e) = child.stderr.take() {
                        use std::io::Read;
                        let mut buf = Vec::new();
                        let _ = e.read_to_end(&mut buf);
                        msg = String::from_utf8_lossy(&buf).lines().rev().take(2).collect::<Vec<_>>().join(" ");
                    }
                    let _ = std::fs::remove_file(&out);
                    return Err(if msg.trim().is_empty() { format!("{what} didn't work.") } else { format!("{what}: {}", msg.trim()) });
                }
                break;
            }
            Ok(None) => {
                if std::time::Instant::now() > end {
                    let _ = child.kill();
                    let _ = std::fs::remove_file(&out);
                    return Err(format!("{what} took too long and was stopped."));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("{what} couldn't be waited for: {e}")),
        }
    }
    let bytes = std::fs::read(&out).map_err(|_| format!("{what} produced no picture."))?;
    let _ = std::fs::remove_file(&out);
    if bytes.len() < 32 {
        return Err(format!("{what} produced no picture."));
    }
    Ok(bytes)
}

/// Scan a page, as PNG bytes. `dpi` 75..1200; `colour` false scans in grey.
pub fn scan(device: &str, dpi: u32, colour: bool) -> Result<Vec<u8>, String> {
    if !have_scanner_support() {
        return Err(scanner_missing());
    }
    let mut cmd = program("scanimage");
    if !device.is_empty() {
        cmd.args(["-d", device]);
    }
    cmd.args(["--format=png", "--mode", if colour { "Color" } else { "Gray" }])
        .arg(format!("--resolution={}", dpi.clamp(75, 1200)))
        .arg("-o");
    run(cmd, Duration::from_secs(180), "The scanner")
}

/// Take one picture from a camera, as PNG bytes. `warmup` frames are thrown away first, because
/// the first frame out of a webcam is usually still finding its exposure.
pub fn snap(device: &str, warmup: u32) -> Result<Vec<u8>, String> {
    if !have_camera_support() {
        return Err(camera_missing());
    }
    let dev = if device.is_empty() { "/dev/video0".to_string() } else { device.to_string() };
    if stub().is_none() && !std::path::Path::new(&dev).exists() {
        return Err(format!("There is no camera at {dev}."));
    }
    let mut cmd = program("ffmpeg");
    cmd.args(["-nostdin", "-y", "-loglevel", "error", "-f", "v4l2", "-i", &dev])
        .args(["-frames:v", "1", "-vsync", "0"])
        .args(["-vf", &format!("select=gte(n\\,{})", warmup.min(60))])
        .arg("-f")
        .arg("image2");
    run(cmd, Duration::from_secs(30), "The camera")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_the_tools_switched_off_it_says_so_rather_than_guessing() {
        crate::services::set_setting("FOLIO_NO_CAPTURE", std::path::Path::new("1"));
        assert!(!have_scanner_support());
        assert!(!have_camera_support());
        assert!(scanners().is_empty());
        assert!(cameras().is_empty());
        assert!(devices().is_empty());
        let e = scan("", 300, true).expect_err("it should refuse");
        assert!(e.contains("sane-utils"), "{e}");
        let e = snap("", 4).expect_err("it should refuse");
        assert!(e.contains("ffmpeg"), "{e}");
        assert!(nothing_found().contains("Plug one in"));
        crate::services::set_setting("FOLIO_NO_CAPTURE", std::path::Path::new("0"));
    }
}
