//! What a project looks and sounds like at one moment, and the ffmpeg command
//! that writes the whole thing out.
//!
//! Nothing here runs a program: it builds argument lists, which the app hands to
//! `folio_kit::platform::media`. That way the graph can be tested on a computer
//! with no ffmpeg on it at all.

use crate::project::*;
use std::path::PathBuf;

/// A piecewise-linear curve, flat beyond its ends.
pub fn piecewise(points: &[(f64, f64)], t: f64) -> f64 {
    if points.is_empty() {
        return 1.0;
    }
    if t <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        let ((a, va), (b, vb)) = (w[0], w[1]);
        if a <= t && t <= b {
            return if b - a < 1e-9 { va } else { va + (vb - va) * (t - a) / (b - a) };
        }
    }
    points[points.len() - 1].1
}

/// The clip's loudness through time: its volume, its volume points, its fades
/// (and any crossfade), and the music-bed dip. Seconds into the clip.
pub fn gain_points(p: &Project, c: &Clip, cross_in: f64, cross_out: f64) -> Vec<(f64, f64)> {
    let d = c.duration;
    let (fi, fo) = (c.fade_in.max(cross_in), c.fade_out.max(cross_out));
    let mut fade: Vec<(f64, f64)> = Vec::new();
    if fi > 0.0 {
        fade.push((0.0, 0.0));
        fade.push((fi.min(d), 1.0));
    }
    if fo > 0.0 {
        fade.push(((d - fo).max(0.0), 1.0));
        fade.push((d, 0.0));
    }
    fade.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let kf = c.keyframes.clone();
    let duck = if c.music_bed { p.duck_points(c.id, 0.3, 0.4) } else { Vec::new() };
    let mut times: Vec<f64> = vec![0.0, d];
    for pts in [&fade, &kf, &duck] {
        for (t, _) in pts {
            if *t >= 0.0 && *t <= d {
                times.push(*t);
            }
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    times
        .into_iter()
        .map(|t| {
            let g = c.volume
                * if fade.is_empty() { 1.0 } else { piecewise(&fade, t) }
                * if kf.is_empty() { 1.0 } else { piecewise(&kf, t) }
                * if duck.is_empty() { 1.0 } else { piecewise(&duck, t) };
            (t, g)
        })
        .collect()
}

/// How see-through the clip is through time, or nothing if it never changes.
pub fn alpha_points(c: &Clip, dip_in: f64, dip_out: f64) -> Option<Vec<(f64, f64)>> {
    let d = c.duration;
    let (fi, fo) = (c.fade_in.max(dip_in), c.fade_out.max(dip_out));
    if fi <= 0.0 && fo <= 0.0 {
        return None;
    }
    let mut pts = Vec::new();
    if fi > 0.0 {
        pts.push((0.0, 0.0));
        pts.push((fi, 1.0));
    } else {
        pts.push((0.0, 1.0));
    }
    if fo > 0.0 {
        pts.push((fi.max(d - fo), 1.0));
        pts.push((d, 0.0));
    } else {
        pts.push((d, 1.0));
    }
    Some(pts)
}

/// What a transition does to the two clips it sits between.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overlap {
    pub alpha_in: f64,
    pub alpha_out: f64,
    pub gain_in: f64,
    pub gain_out: f64,
}

/// ffmpeg's name for one of our transitions, and whether it runs backwards.
pub fn xfade_name(kind: &str) -> (&'static str, bool) {
    match kind {
        "wipe-right" => ("wiperight", false),
        "wipe-left" => ("wipeleft", false),
        "wipe-down" => ("wipedown", false),
        "wipe-up" => ("wipeup", false),
        "box" => ("rectcrop", false),
        "clock" => ("radial", false),
        "iris" => ("circleopen", false),
        _ => ("fade", false),
    }
}

/// Is this transition done by fading, rather than by a shape moving across?
pub fn is_fade(kind: &str) -> bool {
    kind == "crossfade" || kind == "dip-black"
}

/// Work out, for one picture track, what each transition asks of the clips
/// either side of it.
pub fn overlaps(clips: &[Clip]) -> Vec<(u64, Overlap)> {
    let mut out: Vec<(u64, Overlap)> = clips.iter().map(|c| (c.id, Overlap::default())).collect();
    for i in 1..clips.len() {
        let c = &clips[i];
        let Some(tr) = &c.transition else { continue };
        let d = tr.duration;
        match tr.kind.as_str() {
            "dip-black" => {
                out[i].1.alpha_in = out[i].1.alpha_in.max(d / 2.0);
                out[i - 1].1.alpha_out = out[i - 1].1.alpha_out.max(d / 2.0);
            }
            "crossfade" => {
                out[i].1.alpha_in = d;
                out[i].1.gain_in = d;
                out[i - 1].1.gain_out = d;
            }
            _ => {}
        }
    }
    out
}

/// The colour being keyed out, as red, green and blue from nothing to full.
pub fn chroma_target(name: &str) -> (u8, u8, u8) {
    let b = name.as_bytes();
    if b.len() == 7 && b[0] == b'#' {
        let hex = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
        return (hex(&name[1..3]), hex(&name[3..5]), hex(&name[5..7]));
    }
    if name == "blue" {
        (0, 0, 255)
    } else {
        (0, 255, 0)
    }
}

fn num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{:.4}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// ffmpeg's `atempo` only stretches sound between a half and twice. Four times
/// is two of them in a row, a quarter is two the other way.
pub fn atempo_chain(speed: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut left = speed.clamp(0.25, 4.0);
    if (left - 1.0).abs() < 1e-6 {
        return out;
    }
    while left > 2.0 + 1e-9 {
        out.push(2.0);
        left /= 2.0;
    }
    while left < 0.5 - 1e-9 {
        out.push(0.5);
        left /= 0.5;
    }
    if (left - 1.0).abs() > 1e-6 {
        out.push(left);
    }
    out
}

/// `volume` understands an expression in `t`; build one from the gain curve.
fn gain_expr(points: &[(f64, f64)]) -> String {
    if points.len() < 2 {
        let g = points.first().map(|(_, g)| *g).unwrap_or(1.0);
        return num(g);
    }
    // nested if(lt(t,b), interpolate, rest)
    let mut expr = num(points[points.len() - 1].1);
    for w in points.windows(2).rev() {
        let ((a, va), (b, vb)) = (w[0], w[1]);
        let piece = if (b - a).abs() < 1e-9 {
            num(va)
        } else {
            format!("({}+({})*(t-{})/{})", num(va), num(vb - va), num(a), num(b - a))
        };
        expr = format!("if(lt(t\\,{}),{},{})", num(b), piece, expr);
    }
    expr
}

/// One input file on the ffmpeg command line, and what it is for.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub args: Vec<String>,
    pub clip: u64,
}

/// A whole ffmpeg command: the inputs, the filter graph, and the output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Command {
    pub args: Vec<String>,
    pub duration: f64,
}

/// What the picture should be, and what to write it as.
#[derive(Clone, Debug)]
pub struct Target {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    /// "mp4" or "webm".
    pub format: String,
}

impl Target {
    pub fn of(p: &Project, format: &str, preset: &str) -> Target {
        let (w, h) = PRESETS
            .iter()
            .find(|(n, _, _)| *n == preset)
            .map(|(_, w, h)| (*w, *h))
            .unwrap_or((p.data.width, p.data.height));
        Target { width: w, height: h, fps: p.data.fps, format: format.to_string() }
    }
}

struct Build {
    inputs: Vec<String>,
    filters: Vec<String>,
    n: usize,
}

impl Build {
    fn input(&mut self, args: &[String]) -> usize {
        self.inputs.extend(args.iter().cloned());
        let i = self.n;
        self.n += 1;
        i
    }
    fn push(&mut self, f: String) {
        self.filters.push(f);
    }
}

/// How a clip's source is opened: where to start, how much, and looping a still.
fn clip_input(p: &Project, c: &Clip) -> Option<Vec<String>> {
    let m = p.media_by_uri(&c.media)?;
    let path = m.path()?;
    let span = c.duration * c.speed;
    let mut a: Vec<String> = Vec::new();
    if m.is_image {
        a.push("-loop".into());
        a.push("1".into());
        a.push("-t".into());
        a.push(num(c.duration));
    } else {
        a.push("-ss".into());
        a.push(num(c.inpoint));
        a.push("-t".into());
        a.push(num(span));
    }
    a.push("-i".into());
    a.push(path.to_string_lossy().to_string());
    Some(a)
}

/// The picture of one clip, normalised to the target size and placed at its own
/// time on the timeline. Gives back the label of the resulting stream.
fn clip_video(b: &mut Build, p: &Project, c: &Clip, t: &Target, ov: Overlap, shift: f64, label: &str, titles: &[(u64, PathBuf)]) -> Option<String> {
    let media = p.media_by_uri(&c.media);
    let src = if c.kind.is_text() {
        // the picture we drew for this title, or nothing at all to show
        let drawn = titles.iter().find(|(id, _)| *id == c.id).map(|(_, path)| path.clone())?;
        let i = b.input(&[
            "-loop".into(),
            "1".into(),
            "-t".into(),
            num(c.duration),
            "-i".into(),
            drawn.to_string_lossy().to_string(),
        ]);
        format!("{}:v", i)
    } else {
        let args = clip_input(p, c)?;
        let i = b.input(&args);
        format!("{}:v", i)
    };
    let mut chain: Vec<String> = Vec::new();
    // crop first, in the media's own pixels, as the earlier Reel did
    if c.crop.any() {
        if let Some(m) = media {
            if m.width > 0 && m.height > 0 {
                let (mw, mh) = (m.width as f64, m.height as f64);
                let (l, r) = ((c.crop.l * mw) as i64, (c.crop.r * mw) as i64);
                let (tp, bt) = ((c.crop.t * mh) as i64, (c.crop.b * mh) as i64);
                chain.push(format!("crop=iw-{}:ih-{}:{}:{}", l + r, tp + bt, l, tp));
            }
        }
    }
    if c.chroma {
        let (r, g, bl) = chroma_target(&c.chroma_color);
        let tol = (c.chroma_tolerance / 100.0).clamp(0.0, 1.0);
        let soft = (c.chroma_softness / 100.0).clamp(0.0, 1.0);
        chain.push(format!("colorkey=0x{:02X}{:02X}{:02X}:{}:{}", r, g, bl, num(tol), num(soft)));
    }
    if !c.color.is_default() {
        chain.push(format!(
            "eq=brightness={}:contrast={}:saturation={}",
            num(c.color.brightness.clamp(-1.0, 1.0)),
            num(c.color.contrast.clamp(0.0, 4.0)),
            num(c.color.saturation.clamp(0.0, 3.0))
        ));
        if c.color.hue.abs() > 1e-6 {
            chain.push(format!("hue=h={}", num(c.color.hue * 180.0)));
        }
    }
    chain.push(format!(
        "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:-1:-1:color=black@0.0",
        t.width, t.height, t.width, t.height
    ));
    chain.push(format!("fps={}", num(t.fps)));
    chain.push("format=yuva420p".into());
    // speed
    if (c.speed - 1.0).abs() > 1e-6 && !c.kind.is_text() {
        chain.push(format!("setpts=PTS/{}", num(c.speed)));
    }
    // zoom and pan-and-zoom
    let z = c.zoom.max(1.0);
    if z > 1.0 || !c.panzoom.is_empty() {
        let d = c.duration.max(0.04);
        let (from, to, fx0, fx1): (f64, f64, f64, f64) = match c.panzoom.as_str() {
            "in" => (z, z * 1.3, 0.5, 0.5),
            "out" => (z * 1.3, z, 0.5, 0.5),
            "left" => (z * 1.2, z * 1.2, 1.0, 0.0),
            "right" => (z * 1.2, z * 1.2, 0.0, 1.0),
            _ => (z, z, 0.5, 0.5),
        };
        let zexpr = if (from - to).abs() < 1e-9 {
            num(from)
        } else {
            format!("({}+({})*t/{})", num(from), num(to - from), num(d))
        };
        let fxexpr = if (fx0 - fx1).abs() < 1e-9 {
            num(fx0)
        } else {
            format!("({}+({})*t/{})", num(fx0), num(fx1 - fx0), num(d))
        };
        chain.push(format!(
            "scale=w=iw*{z}:h=ih*{z}:eval=frame,crop=w={w}:h={h}:x=(iw-{w})*{fx}:y=(ih-{h})/2",
            z = zexpr,
            w = t.width,
            h = t.height,
            fx = fxexpr
        ));
    }
    // fades, and the opacity a crossfade or a dip to black asks for
    if let Some(pts) = alpha_points(c, ov.alpha_in, ov.alpha_out) {
        let fi = pts.iter().find(|(_, v)| *v >= 1.0).map(|(t, _)| *t).unwrap_or(0.0);
        if fi > 0.0 {
            chain.push(format!("fade=t=in:st=0:d={}:alpha=1", num(fi)));
        }
        let fo = c.duration - pts.iter().rev().find(|(_, v)| *v >= 1.0).map(|(t, _)| *t).unwrap_or(c.duration);
        if fo > 0.0 {
            chain.push(format!("fade=t=out:st={}:d={}:alpha=1", num((c.duration - fo).max(0.0)), num(fo)));
        }
    }
    // where it sits on the timeline
    let at = (c.start + shift).max(0.0);
    chain.push(format!("setpts=PTS-STARTPTS+{}/TB", num(at)));
    b.push(format!("[{}]{}[{}]", src, chain.join(","), label));
    Some(label.to_string())
}

/// The sound of one clip, at its place on the timeline.
fn clip_audio(b: &mut Build, p: &Project, c: &Clip, ov: Overlap, label: &str) -> Option<String> {
    if c.kind.is_text() || c.muted {
        return None;
    }
    let m = p.media_by_uri(&c.media)?;
    if !m.has_audio {
        return None;
    }
    let args = clip_input(p, c)?;
    let i = b.input(&args);
    let mut chain: Vec<String> = vec!["aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo".into()];
    for step in atempo_chain(c.speed) {
        chain.push(format!("atempo={}", num(step)));
    }
    let pts = gain_points(p, c, ov.gain_in, ov.gain_out);
    let flat = pts.iter().all(|(_, g)| (g - pts[0].1).abs() < 1e-9);
    if flat {
        if (pts[0].1 - 1.0).abs() > 1e-9 {
            chain.push(format!("volume={}", num(pts[0].1)));
        }
    } else {
        chain.push(format!("volume='{}':eval=frame", gain_expr(&pts)));
    }
    let delay = (c.start * 1000.0).round() as i64;
    if delay > 0 {
        chain.push(format!("adelay={}:all=1", delay));
    }
    chain.push("apad".into());
    b.push(format!("[{}:a]{}[{}]", i, chain.join(","), label));
    Some(label.to_string())
}

/// A wipe, a box, a clock or an iris: the overlap of the two clips, handed to
/// ffmpeg's own transition, to be laid over the join.
fn shape_transition(b: &mut Build, p: &Project, prev: &Clip, c: &Clip, t: &Target, label: &str) -> Option<String> {
    let tr = c.transition.as_ref()?;
    let d = tr.duration;
    let (name, _) = xfade_name(&tr.kind);
    let at = c.start;
    // the tail of the clip before, and the head of this one
    let tail = {
        let mut a = clip_input(p, prev)?;
        // start d seconds from the end of what the clip shows
        let into = (prev.duration - d).max(0.0) * prev.speed;
        if let Some(i) = a.iter().position(|x| x == "-ss") {
            let was: f64 = a[i + 1].parse().unwrap_or(0.0);
            a[i + 1] = num(was + into);
        }
        if let Some(i) = a.iter().position(|x| x == "-t") {
            a[i + 1] = num(d * prev.speed);
        }
        a
    };
    let head = {
        let mut a = clip_input(p, c)?;
        if let Some(i) = a.iter().position(|x| x == "-t") {
            a[i + 1] = num(d * c.speed);
        }
        a
    };
    let ia = b.input(&tail);
    let ib = b.input(&head);
    let norm = format!(
        "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:-1:-1,fps={},format=yuv420p,setpts=PTS-STARTPTS",
        t.width, t.height, t.width, t.height, num(t.fps)
    );
    b.push(format!("[{}:v]{}[{}a]", ia, norm, label));
    b.push(format!("[{}:v]{}[{}b]", ib, norm, label));
    b.push(format!(
        "[{la}a][{la}b]xfade=transition={name}:duration={d}:offset=0,format=yuva420p,setpts=PTS-STARTPTS+{at}/TB[{la}]",
        la = label,
        name = name,
        d = num(d),
        at = num(at)
    ));
    Some(label.to_string())
}

/// Build the whole command. `out` is the file to write, and `titles` are the
/// pictures `titles::write_all` drew for the text clips.
pub fn export_command_with_titles(p: &Project, t: &Target, out: &std::path::Path, titles: &[(u64, PathBuf)]) -> Result<Command, EditError> {
    let total = p.duration();
    if total <= 0.0 {
        return Err(EditError("There's nothing on the timeline to export.".into()));
    }
    let mut b = Build { inputs: Vec::new(), filters: Vec::new(), n: 0 };
    // the picture starts as black, for the whole length
    let base = b.input(&[
        "-f".into(),
        "lavfi".into(),
        "-t".into(),
        num(total),
        "-i".into(),
        format!("color=c=black:s={}x{}:r={}", t.width, t.height, num(t.fps)),
    ]);
    let mut video = format!("{}:v", base);
    b.push(format!("[{}]format=yuva420p[vbase]", video));
    video = "vbase".to_string();

    // video first, then titles over it
    let mut layer = 0usize;
    for ti in [1usize, 0usize] {
        let clips: Vec<Clip> = p.track(ti).to_vec();
        let marks = overlaps(&clips);
        for (i, c) in clips.iter().enumerate() {
            let ov = marks.iter().find(|(id, _)| *id == c.id).map(|(_, o)| *o).unwrap_or_default();
            // a shape transition shortens this clip's own start, so the shape shows instead
            let shaped = i > 0 && c.transition.as_ref().map(|x| !is_fade(&x.kind)).unwrap_or(false);
            let mut c2 = c.clone();
            let mut shift = 0.0;
            if shaped {
                let d = c.transition.as_ref().unwrap().duration;
                c2.inpoint += d * c.speed;
                c2.duration -= d;
                shift = d;
            }
            if c2.duration <= 0.0 {
                continue;
            }
            let label = format!("v{}", layer);
            layer += 1;
            if let Some(s) = clip_video(&mut b, p, &c2, t, ov, shift, &label, titles) {
                let next = format!("vo{}", layer);
                b.push(format!("[{}][{}]overlay=eof_action=pass:shortest=0[{}]", video, s, next));
                video = next;
            }
            if shaped {
                // the join itself
                let prev = &clips[i - 1];
                let label = format!("x{}", layer);
                layer += 1;
                if let Some(s) = shape_transition(&mut b, p, prev, c, t, &label) {
                    let next = format!("vo{}", layer);
                    b.push(format!("[{}][{}]overlay=eof_action=pass:shortest=0[{}]", video, s, next));
                    video = next;
                }
            }
        }
    }
    b.push(format!("[{}]trim=duration={},format=yuv420p[vout]", video, num(total)));

    // every sound, mixed
    let mut sounds: Vec<String> = Vec::new();
    for ti in 0..TRACKS.len() {
        let clips: Vec<Clip> = p.track(ti).to_vec();
        let marks = overlaps(&clips);
        for c in &clips {
            let ov = marks.iter().find(|(id, _)| *id == c.id).map(|(_, o)| *o).unwrap_or_default();
            let label = format!("a{}", sounds.len());
            if let Some(s) = clip_audio(&mut b, p, c, ov, &label) {
                sounds.push(s);
            }
        }
    }
    let has_sound = !sounds.is_empty();
    if has_sound {
        let ins: String = sounds.iter().map(|s| format!("[{}]", s)).collect();
        b.push(format!(
            "{}amix=inputs={}:normalize=0:dropout_transition=0,atrim=duration={},aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo[aout]",
            ins,
            sounds.len(),
            num(total)
        ));
    }

    let mut args: Vec<String> = vec!["-y".into(), "-nostdin".into()];
    args.extend(b.inputs);
    args.push("-filter_complex".into());
    args.push(b.filters.join(";"));
    args.push("-map".into());
    args.push("[vout]".into());
    if has_sound {
        args.push("-map".into());
        args.push("[aout]".into());
    }
    if t.format == "webm" {
        args.extend(["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "32", "-row-mt", "1"].iter().map(|s| s.to_string()));
        if has_sound {
            args.extend(["-c:a", "libopus", "-b:a", "128k"].iter().map(|s| s.to_string()));
        }
    } else {
        args.extend(["-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p"].iter().map(|s| s.to_string()));
        if has_sound {
            args.extend(["-c:a", "aac", "-b:a", "192k"].iter().map(|s| s.to_string()));
        }
        args.push("-movflags".into());
        args.push("+faststart".into());
    }
    args.push("-r".into());
    args.push(num(t.fps));
    args.push("-t".into());
    args.push(num(total));
    args.push(out.to_string_lossy().to_string());
    Ok(Command { args, duration: total })
}

/// Just the sound of the whole timeline, as plain samples. The preview plays
/// this while it shows pictures, so what you hear is what will be written out.
pub fn audio_command(p: &Project, out: &std::path::Path) -> Result<Command, EditError> {
    let total = p.duration();
    if total <= 0.0 {
        return Err(EditError("There's nothing on the timeline.".into()));
    }
    let mut b = Build { inputs: Vec::new(), filters: Vec::new(), n: 0 };
    let mut sounds: Vec<String> = Vec::new();
    for ti in 0..TRACKS.len() {
        let clips: Vec<Clip> = p.track(ti).to_vec();
        let marks = overlaps(&clips);
        for c in &clips {
            let ov = marks.iter().find(|(id, _)| *id == c.id).map(|(_, o)| *o).unwrap_or_default();
            let label = format!("a{}", sounds.len());
            if let Some(s) = clip_audio(&mut b, p, c, ov, &label) {
                sounds.push(s);
            }
        }
    }
    if sounds.is_empty() {
        return Err(EditError("There's no sound on the timeline.".into()));
    }
    let ins: String = sounds.iter().map(|s| format!("[{}]", s)).collect();
    b.push(format!(
        "{}amix=inputs={}:normalize=0:dropout_transition=0,atrim=duration={}[aout]",
        ins,
        sounds.len(),
        num(total)
    ));
    let mut args: Vec<String> = vec!["-y".into(), "-nostdin".into()];
    args.extend(b.inputs);
    args.push("-filter_complex".into());
    args.push(b.filters.join(";"));
    args.extend(["-map", "[aout]", "-f", "s16le", "-acodec", "pcm_s16le", "-ar", "48000", "-ac", "2"].iter().map(|s| s.to_string()));
    args.push(out.to_string_lossy().to_string());
    Ok(Command { args, duration: total })
}

/// The same, for a film whose titles have not been drawn (they are then left
/// out of the picture, as there is nothing to show).
pub fn export_command(p: &Project, t: &Target, out: &std::path::Path) -> Result<Command, EditError> {
    export_command_with_titles(p, t, out, &[])
}

/// Which clips show at time `t`, topmost last, for the preview.
pub fn visible_at(p: &Project, t: f64) -> Vec<(usize, u64)> {
    let mut v: Vec<(usize, u64)> = Vec::new();
    for ti in [1usize, 0usize] {
        for c in p.track(ti) {
            if c.start - EPS <= t && t < c.end() - EPS {
                v.push((ti, c.id));
            }
        }
    }
    v
}

/// Where in the media a clip is at timeline time `t`.
pub fn media_time(c: &Clip, t: f64) -> f64 {
    (c.inpoint + (t - c.start).max(0.0) * c.speed).max(0.0)
}
