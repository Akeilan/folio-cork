//! Every change to a project: adding clips, splitting, trimming, moving,
//! deleting, speed, transitions, volume points, markers.
//!
//! Each one takes a checkpoint first, so Undo works the same way everywhere, and
//! each returns a sentence to show someone when it cannot be done.

use crate::project::*;

fn err<T>(m: impl Into<String>) -> Result<T, EditError> {
    Err(EditError(m.into()))
}

/// What a new clip is made from.
pub enum Source<'a> {
    /// A file in the bin.
    Media(&'a Media),
    /// A title, a lower third or a caption, with its words.
    Text(Kind, String),
}

fn text_clip(id: u64, kind: Kind, text: &str) -> Clip {
    let first = text.lines().next().unwrap_or("");
    let name: String = first.chars().take(40).collect();
    let mut c = Clip {
        id,
        kind,
        name: if name.is_empty() { kind.text().to_string() } else { name },
        duration: TITLE_SECONDS,
        text: text.to_string(),
        ..Clip::default()
    };
    match kind {
        Kind::Title => {
            c.font = "Sans Bold 64".into();
            c.text_color = "#ffffffff".into();
            c.bg_color = "#ff000000".into();
            c.halign = "center".into();
            c.valign = "center".into();
        }
        Kind::LowerThird => {
            c.font = "Sans Bold 36".into();
            c.text_color = "#ffffffff".into();
            c.bg_color = "#c0101828".into();
            c.halign = "left".into();
            c.valign = "bottom".into();
        }
        _ => {
            c.font = "Sans 30".into();
            c.text_color = "#ffffffff".into();
            c.bg_color = "#a0000000".into();
            c.halign = "center".into();
            c.valign = "bottom".into();
        }
    }
    c
}

fn media_clip(id: u64, m: &Media) -> Clip {
    Clip {
        id,
        kind: Kind::Media,
        media: m.uri.clone(),
        name: m.name.clone(),
        duration: if m.is_image { IMAGE_SECONDS } else { m.duration.max(0.04) },
        ..Clip::default()
    }
}

/// Default words for a new text clip.
pub fn default_text(kind: Kind) -> &'static str {
    match kind {
        Kind::Title => "Title",
        Kind::LowerThird => "Name\nRole",
        Kind::Caption => "Caption",
        Kind::Media => "",
    }
}

impl Project {
    /// How long a clip can run on the timeline: the media left after its in
    /// point, at its speed. Titles and stills can run as long as you like.
    pub fn max_duration(&self, c: &Clip) -> f64 {
        if c.kind != Kind::Media {
            return f64::INFINITY;
        }
        match self.media_by_uri(&c.media) {
            None => f64::INFINITY,
            Some(m) if m.is_image => f64::INFINITY,
            Some(m) => ((m.duration - c.inpoint) / c.speed.max(0.01)).max(0.04),
        }
    }

    fn id_next(&mut self) -> u64 {
        let n = self.data.next_id;
        self.data.next_id = n + 1;
        n
    }

    fn order(&mut self, ti: usize) {
        self.data.tracks[ti].sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// The first start at or after `at` where `length` fits on the track.
    pub fn first_free(&self, ti: usize, at: f64, length: f64) -> f64 {
        let mut t = at;
        let mut clips: Vec<&Clip> = self.data.tracks[ti].iter().collect();
        clips.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
        for c in clips {
            if c.end() <= t + EPS {
                continue;
            }
            if c.start >= t + length - EPS {
                break;
            }
            t = c.end();
        }
        t
    }

    /// Insert edit: everything from `at` on moves later by `length`; a clip
    /// running across `at` is cut there first.
    fn make_room(&mut self, ti: usize, at: f64, length: f64, keep: u64) {
        let cut: Vec<u64> = self.data.tracks[ti]
            .iter()
            .filter(|c| c.id != keep && c.start < at - EPS && at < c.end() - 2.0 * EPS)
            .map(|c| c.id)
            .collect();
        for id in cut {
            self.cut_in_two(ti, id, at);
        }
        for c in self.data.tracks[ti].iter_mut() {
            if c.id != keep && c.start >= at - EPS {
                c.start += length;
            }
        }
        self.order(ti);
    }

    /// Where a clip from this source belongs if nobody says.
    pub fn default_track(&self, s: &Source) -> usize {
        match s {
            Source::Text(..) => 0,
            Source::Media(m) => self.media_track(m),
        }
    }

    /// Put a clip on a track. `at` of `None` means after the last clip there;
    /// `insert` pushes what follows later instead of finding a gap.
    pub fn add_clip(&mut self, ti: usize, source: Source, at: Option<f64>, insert: bool) -> Result<u64, EditError> {
        if ti >= TRACKS.len() {
            return err("there's no such track");
        }
        if let Source::Media(m) = &source {
            if ti == 0 && !(m.has_video || m.is_image) {
                return err("The Titles track is for pictures, video and titles.");
            }
            if (ti == 2 || ti == 3) && !m.has_audio {
                return err(format!("“{}” has no sound to put on the {} track.", m.name, track_name(TRACKS[ti])));
            }
        } else if ti != 0 {
            return err("Titles go on the Titles track.");
        }
        self.checkpoint("Add Clip", None);
        let id = self.id_next();
        let mut c = match &source {
            Source::Media(m) => media_clip(id, m),
            Source::Text(k, t) => text_clip(id, *k, t),
        };
        let at = match at {
            Some(a) => a.max(0.0),
            None => self.data.tracks[ti].iter().map(|x| x.end()).fold(0.0, f64::max),
        };
        if insert {
            self.make_room(ti, at, c.duration, id);
            c.start = at;
        } else {
            c.start = self.first_free(ti, at, c.duration);
        }
        self.data.tracks[ti].push(c);
        self.order(ti);
        self.dirty = true;
        Ok(id)
    }

    /// Subtitles as caption clips on the Titles track, each at its own time.
    pub fn add_subtitles(&mut self, entries: &[(f64, f64, String)]) -> Vec<u64> {
        if entries.is_empty() {
            return Vec::new();
        }
        self.checkpoint("Import Subtitles", None);
        let mut made = Vec::new();
        for (start, end, words) in entries {
            let id = self.id_next();
            let mut c = text_clip(id, Kind::Caption, words);
            c.duration = (end - start).max(0.1);
            c.start = self.first_free(0, start.max(0.0), c.duration);
            self.data.tracks[0].push(c);
            self.order(0);
            made.push(id);
        }
        self.dirty = true;
        made
    }

    // ---------------------------------------------------------------- cutting
    fn cut_in_two(&mut self, ti: usize, id: u64, t: f64) -> Option<u64> {
        let pos = self.data.tracks[ti].iter().position(|c| c.id == id)?;
        let nid = self.id_next();
        let mut b = self.data.tracks[ti][pos].clone();
        let c = &mut self.data.tracks[ti][pos];
        let off = t - c.start;
        c.duration = off;
        let keep: Vec<(f64, f64)> = c.keyframes.iter().copied().filter(|(k, _)| *k < off).collect();
        let pass: Vec<(f64, f64)> = c.keyframes.iter().filter(|(k, _)| *k >= off).map(|(k, v)| (k - off, *v)).collect();
        c.keyframes = keep;
        c.fade_out = 0.0;
        let speed = c.speed;
        let kind = c.kind;
        let inpoint = c.inpoint;
        b.id = nid;
        b.start = t;
        b.duration -= off;
        b.inpoint = if kind == Kind::Media { inpoint + off * speed } else { 0.0 };
        b.transition = None;
        b.fade_in = 0.0;
        b.keyframes = pass;
        self.data.tracks[ti].push(b);
        self.order(ti);
        Some(nid)
    }

    /// Cut one clip at time `t`. Gives back the second half.
    pub fn split(&mut self, id: u64, t: f64) -> Result<u64, EditError> {
        let Some((ti, c)) = self.find(id) else { return err("no such clip") };
        if !(c.start + 0.04 <= t && t <= c.end() - 0.04) {
            return err("The playhead isn't over that clip.");
        }
        self.checkpoint("Split", None);
        let nid = self.cut_in_two(ti, id, t).ok_or_else(|| EditError("no such clip".into()))?;
        self.dirty = true;
        Ok(nid)
    }

    /// Cut every clip under time `t`, or only the ones named.
    pub fn split_at(&mut self, t: f64, ids: Option<&[u64]>) -> Result<Vec<u64>, EditError> {
        let hit: Vec<(usize, u64)> = self
            .clips_at(t)
            .into_iter()
            .filter(|(_, c)| ids.map(|v| v.contains(&c.id)).unwrap_or(true) && c.start + 0.04 <= t && t <= c.end() - 0.04)
            .map(|(ti, c)| (ti, c.id))
            .collect();
        if hit.is_empty() {
            return err("There's no clip under the playhead to split.");
        }
        self.checkpoint("Split", None);
        let mut out = Vec::new();
        for (ti, id) in hit {
            if let Some(n) = self.cut_in_two(ti, id, t) {
                out.push(n);
            }
        }
        self.dirty = true;
        Ok(out)
    }

    /// Take clips off the timeline. `ripple` closes the gap they leave.
    pub fn delete(&mut self, ids: &[u64], ripple: bool) {
        let live: Vec<u64> = ids.iter().copied().filter(|id| self.find(*id).is_some()).collect();
        if live.is_empty() {
            return;
        }
        self.checkpoint(if ripple { "Ripple Delete" } else { "Delete" }, None);
        for id in live {
            let Some((ti, c)) = self.find(id) else { continue };
            let (start, gap) = (c.start, c.duration);
            let next = self.neighbours(id).1;
            self.data.tracks[ti].retain(|x| x.id != id);
            if let Some(n) = next {
                if let Some((_, c)) = self.find_mut(n) {
                    c.transition = None;
                }
            }
            if ripple {
                for x in self.data.tracks[ti].iter_mut() {
                    if x.start >= start - EPS {
                        x.start = (x.start - gap).max(0.0);
                    }
                }
                self.order(ti);
            }
        }
        self.dirty = true;
    }

    /// Move a clip, perhaps to another track. It snaps to edges and to the
    /// playhead, and never lands on another clip: it takes the nearest free place.
    pub fn move_clip(&mut self, id: u64, start: f64, to: Option<usize>, tolerance: f64, playhead: Option<f64>) -> Result<(), EditError> {
        let Some((from, c)) = self.find(id) else { return err("no such clip") };
        let (kind, media, length) = (c.kind, c.media.clone(), c.duration);
        let ti = to.unwrap_or(from);
        if ti != from {
            if kind.is_text() && ti != 0 {
                return err("Titles stay on the Titles track.");
            }
            if kind == Kind::Media {
                let m = self.media_by_uri(&media).cloned().unwrap_or_default();
                if (ti == 2 || ti == 3) && !m.has_audio {
                    return err("That clip has no sound.");
                }
                if (ti == 0 || ti == 1) && !(m.has_video || m.is_image) {
                    return err("That clip has no picture.");
                }
            }
        }
        self.checkpoint("Move", None);
        let mut start = start.max(0.0);
        if tolerance > 0.0 {
            let extra: Vec<f64> = playhead.into_iter().collect();
            let a = self.snap(start, tolerance, &[id], &extra);
            let b = self.snap(start + length, tolerance, &[id], &extra) - length;
            let moved: Vec<f64> = [a, b].into_iter().filter(|s| (s - start).abs() > 1e-9).collect();
            if let Some(best) = moved.into_iter().min_by(|x, y| {
                (x - start).abs().partial_cmp(&(y - start).abs()).unwrap_or(std::cmp::Ordering::Equal)
            }) {
                start = best;
            }
        }
        let pos = self.data.tracks[from].iter().position(|x| x.id == id).unwrap();
        let mut c = self.data.tracks[from].remove(pos);
        c.transition = None;
        let free = |at: f64, lst: &[Clip]| lst.iter().all(|x| x.end() <= at + EPS || x.start >= at + length - EPS);
        if !free(start, &self.data.tracks[ti]) {
            let lst = &self.data.tracks[ti];
            let mut cands: Vec<f64> = lst.iter().map(|x| x.end()).chain(lst.iter().map(|x| x.start - length)).collect();
            cands.retain(|p| *p >= -EPS && free(p.max(0.0), lst));
            if cands.is_empty() {
                cands.push(lst.iter().map(|x| x.end()).fold(0.0, f64::max));
            }
            start = cands
                .into_iter()
                .min_by(|x, y| (x - start).abs().partial_cmp(&(y - start).abs()).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap();
        }
        c.start = start.max(0.0);
        self.data.tracks[ti].push(c);
        self.order(ti);
        self.dirty = true;
        Ok(())
    }

    /// Drag a clip's start or end edge. The media's own length and the
    /// neighbouring clips are the limits.
    pub fn trim(&mut self, id: u64, edge: Edge, t: f64, tolerance: f64, playhead: Option<f64>) -> Result<(), EditError> {
        let Some((ti, _)) = self.find(id) else { return err("no such clip") };
        let extra: Vec<f64> = playhead.into_iter().collect();
        let t = if tolerance > 0.0 { self.snap(t, tolerance, &[id], &extra) } else { t };
        let (prev, next) = self.neighbours(id);
        let prev_end = prev.and_then(|p| self.clip(p)).map(|p| (p.end(), p.transition.as_ref().map(|x| x.duration).unwrap_or(0.0)));
        let next_start = next.and_then(|n| self.clip(n)).map(|n| (n.start, n.transition.as_ref().map(|x| x.duration).unwrap_or(0.0)));
        let cap = self.clip(id).map(|c| self.max_duration(c)).unwrap_or(f64::INFINITY);
        let still = self
            .clip(id)
            .map(|c| c.kind != Kind::Media || self.media_by_uri(&c.media).map(|m| m.is_image).unwrap_or(true))
            .unwrap_or(true);
        self.checkpoint("Trim", None);
        let Some((_, c)) = self.find_mut(id) else { return err("no such clip") };
        let sp = c.speed.max(0.01);
        match edge {
            Edge::Start => {
                let mut lo = match prev_end {
                    Some((end, over)) => end - over,
                    None => 0.0,
                };
                if !still {
                    lo = lo.max(c.start - c.inpoint / sp);
                }
                let t = t.max(lo).min(c.end() - 0.1);
                let delta = t - c.start;
                if c.kind == Kind::Media {
                    c.inpoint = (c.inpoint + delta * sp).max(0.0);
                }
                c.start = t;
                c.duration -= delta;
            }
            Edge::End => {
                let hi = match next_start {
                    Some((s, over)) => (s + over).min(c.start + cap),
                    None => c.start + cap,
                };
                let t = t.min(hi).max(c.start + 0.1);
                c.duration = t - c.start;
            }
        }
        c.fade_in = c.fade_in.min(c.duration);
        c.fade_out = c.fade_out.min(c.duration);
        self.order(ti);
        self.dirty = true;
        Ok(())
    }

    /// Speed, a quarter to four times. The clip gets shorter or longer, and the
    /// clips after it move so the cut stays where it was.
    pub fn set_speed(&mut self, id: u64, speed: f64) -> Result<(), EditError> {
        let Some((ti, c)) = self.find(id) else { return err("Speed is for video and sound clips.") };
        if c.kind != Kind::Media {
            return err("Speed is for video and sound clips.");
        }
        let speed = speed.clamp(0.25, 4.0);
        let m = self.media_by_uri(&c.media).cloned().unwrap_or_default();
        let mut span = c.duration * c.speed;
        if !m.is_image {
            span = span.min((m.duration - c.inpoint).max(0.04));
        }
        let new = span / speed;
        let (delta, old_end) = (new - c.duration, c.end());
        self.checkpoint("Speed", None);
        if let Some((_, c)) = self.find_mut(id) {
            c.speed = speed;
            c.duration = new;
        }
        for x in self.data.tracks[ti].iter_mut() {
            if x.id != id && x.start >= old_end - EPS {
                x.start += delta;
            }
        }
        self.order(ti);
        self.dirty = true;
        Ok(())
    }

    /// A transition into this clip from the one before it. A crossfade or wipe
    /// overlaps the two, so this clip and everything after it move earlier;
    /// Fade to Black fades one out and the next in, where they already are.
    pub fn set_transition(&mut self, id: u64, kind: Option<&str>, duration: f64) -> Result<(), EditError> {
        let Some((ti, c)) = self.find(id) else { return err("A transition goes between two clips: pick the second one.") };
        let Some(prev) = self.neighbours(id).0 else {
            return err("A transition goes between two clips: pick the second one.");
        };
        if ti != 1 && ti != 0 {
            return err("Transitions are for the Video and Titles tracks; on sound, use fades.");
        }
        let (start, own) = (c.start, c.duration);
        let old = c.transition.clone();
        let prev = self.clip(prev).cloned().unwrap_or_default_clip();
        let shift = match &old {
            Some(t) if t.kind != "dip-black" => t.duration,
            _ => 0.0,
        };
        let new_shift;
        let set;
        match kind {
            None | Some("none") | Some("") => {
                new_shift = 0.0;
                set = None;
            }
            Some(k) => {
                let gap = start - prev.end();
                if gap > 0.05 && k != "dip-black" {
                    return err("The two clips must touch for a transition: close the gap first.");
                }
                let d = duration.min(own / 2.0).min(prev.duration / 2.0 + shift).max(0.1);
                new_shift = if k == "dip-black" { 0.0 } else { d };
                set = Some(Trans { kind: k.to_string(), duration: d });
            }
        }
        self.checkpoint("Transition", None);
        if let Some((_, c)) = self.find_mut(id) {
            c.transition = set;
        }
        let delta = shift - new_shift;
        if delta.abs() > EPS {
            for x in self.data.tracks[ti].iter_mut() {
                if x.start >= start - EPS {
                    x.start += delta;
                }
            }
        }
        self.order(ti);
        self.dirty = true;
        Ok(())
    }

    /// Volume points: (time in the clip, level from none to twice).
    pub fn set_keyframes(&mut self, id: u64, mut points: Vec<(f64, f64)>) -> Result<(), EditError> {
        points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        self.checkpoint("Volume Points", None);
        let Some((_, c)) = self.find_mut(id) else { return err("no such clip") };
        c.keyframes = points;
        self.dirty = true;
        Ok(())
    }

    /// Where the music dips: under every clip on the Audio track.
    pub fn duck_points(&self, music: u64, level: f64, ramp: f64) -> Vec<(f64, f64)> {
        let Some(m) = self.clip(music) else { return Vec::new() };
        let mut speech: Vec<(f64, f64)> = self.data.tracks[2].iter().filter(|x| !x.muted).map(|x| (x.start, x.end())).collect();
        speech.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (a, b) in speech {
            match merged.last_mut() {
                Some(last) if a <= last.1 + ramp * 2.0 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        let (s, e) = (m.start, m.end());
        let mut pts = vec![(0.0f64, 1.0f64)];
        for (a, b) in merged {
            if b <= s || a >= e {
                continue;
            }
            for (t, g) in [(a - ramp, 1.0), (a, level), (b, level), (b + ramp, 1.0)] {
                if s - EPS <= t && t <= e + EPS {
                    pts.push((t - s, g));
                } else if t < s && (g - level).abs() < EPS {
                    pts.push((0.0, level));
                }
            }
        }
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        pts
    }

    // ---------------------------------------------------------------- copy and paste
    /// Clips lifted out for the clipboard: the clip, its track, how far after
    /// the first one it sat, and its media entry if the paste may need it.
    pub fn copy_out(&self, ids: &[u64]) -> Vec<(usize, Clip, f64, Option<Media>)> {
        let mut picked: Vec<(usize, &Clip)> = self.all_clips().into_iter().filter(|(_, c)| ids.contains(&c.id)).collect();
        picked.sort_by(|a, b| a.1.start.partial_cmp(&b.1.start).unwrap_or(std::cmp::Ordering::Equal));
        let first = picked.first().map(|(_, c)| c.start).unwrap_or(0.0);
        picked
            .into_iter()
            .map(|(ti, c)| (ti, c.clone(), c.start - first, self.media_by_uri(&c.media).cloned()))
            .collect()
    }

    /// Clips back on their tracks from time `at`, keeping their spacing. A
    /// pasted group becomes a group of its own.
    pub fn paste(&mut self, items: &[(usize, Clip, f64, Option<Media>)], at: f64) -> Vec<u64> {
        if items.is_empty() {
            return Vec::new();
        }
        self.checkpoint("Paste", None);
        let mut items: Vec<&(usize, Clip, f64, Option<Media>)> = items.iter().collect();
        items.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        let mut regroup: Vec<(u64, u64)> = Vec::new();
        let mut out = Vec::new();
        for (ti, clip, off, media) in items {
            let mut c = clip.clone();
            if c.group != 0 {
                let g = match regroup.iter().find(|(old, _)| *old == c.group) {
                    Some((_, new)) => *new,
                    None => {
                        let n = self.id_next();
                        regroup.push((c.group, n));
                        n
                    }
                };
                c.group = g;
            }
            if c.kind == Kind::Media && self.media_by_uri(&c.media).is_none() {
                match media {
                    Some(m) => self.data.media.push(m.clone()),
                    None => continue,
                }
            }
            c.id = self.id_next();
            c.transition = None;
            c.start = self.first_free(*ti, (at + off).max(0.0), c.duration);
            out.push(c.id);
            self.data.tracks[*ti].push(c);
            self.order(*ti);
        }
        self.dirty = true;
        out
    }

    /// Move a whole group together, or stop if any of it cannot go.
    pub fn move_group(&mut self, gid: u64, delta: f64, tolerance: f64, playhead: Option<f64>) -> Result<(), EditError> {
        let ids = self.group_members(gid);
        if ids.is_empty() {
            return err("no such group");
        }
        let starts: Vec<(u64, usize, f64)> = ids
            .iter()
            .filter_map(|id| self.find(*id).map(|(ti, c)| (*id, ti, c.start)))
            .collect();
        let lowest = starts.iter().map(|(_, _, s)| *s).fold(f64::INFINITY, f64::min);
        let mut delta = delta.max(-lowest);
        if tolerance > 0.0 {
            let extra: Vec<f64> = playhead.into_iter().collect();
            let first = lowest + delta;
            let snapped = self.snap(first, tolerance, &ids, &extra);
            delta += snapped - first;
        }
        if delta.abs() < EPS {
            return Ok(());
        }
        // the whole group must land clear of everything that is not in it
        for (id, ti, start) in &starts {
            let c = self.clip(*id).unwrap();
            let (a, b) = (start + delta, start + delta + c.duration);
            let clash = self.data.tracks[*ti]
                .iter()
                .any(|x| !ids.contains(&x.id) && x.end() > a + EPS && x.start < b - EPS);
            if clash {
                return err("There isn't room there for the whole group.");
            }
        }
        self.checkpoint("Move Group", None);
        for (id, ti, _) in &starts {
            if let Some(c) = self.data.tracks[*ti].iter_mut().find(|c| c.id == *id) {
                c.start = (c.start + delta).max(0.0);
                c.transition = None;
            }
            self.order(*ti);
        }
        self.dirty = true;
        Ok(())
    }

    // ---------------------------------------------------------------- markers
    /// The markers, earliest first.
    pub fn markers(&self) -> Vec<&Marker> {
        let mut v: Vec<&Marker> = self.data.markers.iter().collect();
        v.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        v
    }

    pub fn marker(&self, id: u64) -> Option<&Marker> {
        self.data.markers.iter().find(|m| m.id == id)
    }

    pub fn marker_at(&self, t: f64, tolerance: f64) -> Option<&Marker> {
        let mut best: Option<&Marker> = None;
        let mut dist = tolerance;
        for m in self.markers() {
            let d = (m.t - t).abs();
            if d <= dist {
                dist = d;
                best = Some(m);
            }
        }
        best
    }

    /// A marker at `t`. One already there is renamed rather than doubled up.
    pub fn add_marker(&mut self, t: f64, name: &str, color: &str) -> u64 {
        let t = t.max(0.0);
        if let Some(here) = self.marker_at(t, EPS) {
            let id = here.id;
            if !name.is_empty() {
                self.set_marker(id, Some(name), Some(color), None);
            }
            return id;
        }
        self.checkpoint("Add marker", None);
        let id = self.id_next();
        let color = if MARKER_COLORS.iter().any(|(c, _)| *c == color) { color } else { "yellow" };
        self.data.markers.push(Marker { id, t, name: name.to_string(), color: color.to_string() });
        self.dirty = true;
        id
    }

    pub fn set_marker(&mut self, id: u64, name: Option<&str>, color: Option<&str>, t: Option<f64>) -> bool {
        let Some(m) = self.marker(id) else { return false };
        let same = name.map(|n| m.name == n).unwrap_or(true)
            && color.map(|c| m.color == c).unwrap_or(true)
            && t.map(|x| (m.t - x).abs() < EPS).unwrap_or(true);
        if same {
            return true;
        }
        self.checkpoint("Change marker", None);
        let m = self.data.markers.iter_mut().find(|m| m.id == id).unwrap();
        if let Some(n) = name {
            m.name = n.to_string();
        }
        if let Some(c) = color {
            m.color = c.to_string();
        }
        if let Some(x) = t {
            m.t = x.max(0.0);
        }
        self.dirty = true;
        true
    }

    pub fn remove_markers(&mut self, ids: &[u64]) -> usize {
        let live: Vec<u64> = ids.iter().copied().filter(|i| self.marker(*i).is_some()).collect();
        if live.is_empty() {
            return 0;
        }
        self.checkpoint(if live.len() == 1 { "Delete marker" } else { "Delete markers" }, None);
        self.data.markers.retain(|m| !live.contains(&m.id));
        self.dirty = true;
        live.len()
    }

    pub fn clear_markers(&mut self) -> usize {
        let ids: Vec<u64> = self.data.markers.iter().map(|m| m.id).collect();
        self.remove_markers(&ids)
    }

    pub fn next_marker(&self, t: f64) -> Option<&Marker> {
        self.markers().into_iter().find(|m| m.t > t + EPS)
    }

    pub fn prev_marker(&self, t: f64) -> Option<&Marker> {
        self.markers().into_iter().rev().find(|m| m.t < t - EPS)
    }
}

/// A clip that is there, or an empty one, so a missing neighbour reads as a gap.
trait OrDefaultClip {
    fn unwrap_or_default_clip(self) -> Clip;
}

impl OrDefaultClip for Option<Clip> {
    fn unwrap_or_default_clip(self) -> Clip {
        self.unwrap_or_default()
    }
}
