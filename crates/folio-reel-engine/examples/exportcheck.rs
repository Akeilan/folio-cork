//! Build a project out of real files and export it, so the graph is proved
//! against the ffmpeg that is actually on the computer.
use folio_kit::platform::media;
use folio_reel_engine::edits::Source;
use folio_reel_engine::project::*;
use folio_reel_engine::render;
use std::path::{Path, PathBuf};

fn add(p: &mut Project, path: &str) -> Media {
    let info = media::probe(Path::new(path)).unwrap_or_else(|e| panic!("probe {}: {}", path, e));
    let m = Media {
        uri: path_to_uri(Path::new(path)),
        name: Path::new(path).file_name().unwrap().to_string_lossy().to_string(),
        duration: info.duration,
        width: info.width,
        height: info.height,
        fps: info.fps,
        has_video: info.has_video,
        has_audio: info.has_audio,
        is_image: info.is_image(),
        relative: String::new(),
    };
    p.add_media(m.clone());
    m
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/tmp/reelx".into());
    let kind = std::env::args().nth(2).unwrap_or_else(|| "all".into());
    if !media::have() {
        println!("no ffmpeg: {}", media::missing_message());
        return;
    }
    let mut p = Project::new();
    p.set_size(640, 360, Some(25.0));
    let a = add(&mut p, &format!("{}/a.mp4", dir));
    let c = add(&mut p, &format!("{}/c.mp4", dir));
    let music = add(&mut p, &format!("{}/music.mp3", dir));
    let still = add(&mut p, &format!("{}/s.png", dir));
    println!("probe a.mp4: {:.2}s {}x{} {:.0}fps video={} audio={}", a.duration, a.width, a.height, a.fps, a.has_video, a.has_audio);
    println!("probe s.png: image={} duration={:.2}", still.is_image, still.duration);

    let c1 = p.add_clip(1, Source::Media(&a), None, false).unwrap();
    p.trim(c1, Edge::End, 3.0, 0.0, None).unwrap();
    let c2 = p.add_clip(1, Source::Media(&c), None, false).unwrap();
    p.trim(c2, Edge::End, p.clip(c2).unwrap().start + 3.0, 0.0, None).unwrap();
    if kind == "wipe" {
        p.set_transition(c2, Some("wipe-right"), 1.0).unwrap();
    } else {
        p.set_transition(c2, Some("crossfade"), 1.0).unwrap();
    }
    let c3 = p.add_clip(1, Source::Media(&still), None, false).unwrap();
    p.trim(c3, Edge::End, p.clip(c3).unwrap().start + 2.0, 0.0, None).unwrap();
    {
        let (_, x) = p.find_mut(c3).unwrap();
        x.panzoom = "in".into();
        x.zoom = 1.2;
    }
    let t = p.add_clip(0, Source::Text(Kind::Title, "Hello".into()), Some(0.5), false).unwrap();
    p.find_mut(t).unwrap().1.duration = 2.0;
    let mu = p.add_clip(3, Source::Media(&music), Some(0.0), false).unwrap();
    {
        let (_, x) = p.find_mut(mu).unwrap();
        x.music_bed = true;
        x.volume = 0.6;
        x.fade_out = 1.0;
    }
    p.set_speed(c1, 1.5).unwrap();
    p.set_keyframes(c2, vec![(0.0, 0.2), (2.0, 1.0)]).unwrap();

    let out = PathBuf::from(format!("{}/out.mp4", dir));
    let target = render::Target { width: 640, height: 360, fps: 25.0, format: "mp4".into() };
    let cmd = render::export_command(&p, &target, &out).unwrap();
    println!("timeline {:.2}s, {} args", cmd.duration, cmd.args.len());
    let mut last = 0.0;
    match media::export(&cmd.args, cmd.duration, |secs| last = secs) {
        Ok(()) => {
            let got = media::probe(&out).unwrap();
            println!(
                "wrote {:.2}s {}x{} video={} audio={} (progress reached {:.1}s)",
                got.duration, got.width, got.height, got.video_codec, got.audio_codec, last
            );
            let f = media::frame(&out, 1.0, 160).unwrap();
            println!("frame at 1s: {}x{}, {} bytes", f.w, f.h, f.rgba.len());
            let w = media::waveform(&out, 40).unwrap();
            println!("waveform: {} points, peak {:.2}", w.len(), w.iter().cloned().fold(0.0, f32::max));
        }
        Err(e) => {
            println!("EXPORT FAILED: {}", e);
            std::fs::write(format!("{}/graph.txt", dir), cmd.args.join("\n")).ok();
            std::process::exit(1);
        }
    }
}
