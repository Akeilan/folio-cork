//! Not run by default: dumps decoded WebP files for comparing with a reference decoder.

/// Decodes every .webp in $WEBP_DIR to <name>.rgba (u32 w, u32 h, frame count, then RGBA
/// frames), for comparing against a reference decoder outside the build.
#[test]
#[ignore]
fn dump_for_oracle() {
    let Ok(dir) = std::env::var("WEBP_DIR") else { return };
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map_or(true, |x| x != "webp") {
            continue;
        }
        let d = std::fs::read(&p).unwrap();
        let mut out = Vec::new();
        match folio_draw_engine::webp::decode_frames(&d) {
            Ok(f) => {
                out.extend_from_slice(&(f.w as u32).to_le_bytes());
                out.extend_from_slice(&(f.h as u32).to_le_bytes());
                out.extend_from_slice(&(f.frames.len() as u32).to_le_bytes());
                for (px, _) in &f.frames {
                    out.extend_from_slice(px);
                }
                let enc = if f.frames.len() == 1 {
                    folio_draw_engine::webp::encode(f.w, f.h, &f.frames[0].0)
                } else {
                    let refs: Vec<(&[u8], u32)> = f.frames.iter().map(|(p, d)| (p.as_slice(), *d)).collect();
                    folio_draw_engine::webp::encode_animation(f.w, f.h, &refs, f.loop_count)
                };
                std::fs::write(p.with_extension("enc"), enc).unwrap();
            }
            Err(e) => out.extend_from_slice(format!("ERR {e}").as_bytes()),
        }
        std::fs::write(p.with_extension("rgba"), out).unwrap();
    }
}
