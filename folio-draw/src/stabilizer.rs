//! Stroke stabilizer for the pencil, brushes and eraser: StreamLine, Stabilization and Motion
//! filtering, each 0 to 100, applied to pointer positions before they reach the engine.
//!
//! Distances are in screen pixels, so the feel is the same at any zoom. Only square root is used,
//! which is exact in IEEE 754, so every machine stabilizes a stroke the same way.

pub const NAMES: [&str; 3] = ["StreamLine", "Stabilization", "Motion filtering"];

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx * dx + dy * dy).sqrt()
}

#[derive(Clone, Debug)]
pub struct Stabilizer {
    string: f32,
    window: usize,
    deadzone: f32,
    raw: (f32, f32),
    recent: Vec<(f32, f32)>,
    held: Vec<(f32, f32)>,
    out: (f32, f32),
    last_in: (f32, f32),
}

impl Stabilizer {
    /// `v` is StreamLine, Stabilization and Motion filtering, each 0 to 100; `zoom` is the
    /// picture's scale on screen.
    pub fn new(v: [u8; 3], zoom: f32) -> Stabilizer {
        let z = if zoom > 0.0 { zoom } else { 1.0 };
        Stabilizer {
            string: v[0] as f32 / 100.0 * 40.0 / z,
            window: 1 + (v[1] as f32 / 100.0 * 15.0).round() as usize,
            deadzone: v[2] as f32 / 100.0 * 8.0 / z,
            raw: (0.0, 0.0),
            recent: vec![],
            held: vec![],
            out: (0.0, 0.0),
            last_in: (0.0, 0.0),
        }
    }

    pub fn active(&self) -> bool {
        self.string > 0.0 || self.window > 1 || self.deadzone > 0.0
    }

    pub fn start(&mut self, x: f32, y: f32) {
        self.raw = (x, y);
        self.recent = vec![(x, y)];
        self.held.clear();
        self.out = (x, y);
        self.last_in = (x, y);
    }

    /// A new pointer position; the point to draw to, or None to draw nothing yet.
    pub fn feed(&mut self, x: f32, y: f32) -> Option<(f32, f32)> {
        self.last_in = (x, y);
        let (mut x, mut y) = (x, y);
        // motion filtering: wait for the hand to really move, then take where it has been on average
        if self.deadzone > 0.0 {
            self.held.push((x, y));
            if dist((x, y), self.raw) < self.deadzone {
                return None;
            }
            let n = self.held.len() as f32;
            x = self.held.iter().map(|p| p.0).sum::<f32>() / n;
            y = self.held.iter().map(|p| p.1).sum::<f32>() / n;
            self.held.clear();
        }
        self.raw = (x, y);
        // stabilization: the average of the last few positions
        self.recent.push((x, y));
        if self.recent.len() > self.window {
            let cut = self.recent.len() - self.window;
            self.recent.drain(..cut);
        }
        let n = self.recent.len() as f32;
        let mut t = (self.recent.iter().map(|p| p.0).sum::<f32>() / n, self.recent.iter().map(|p| p.1).sum::<f32>() / n);
        // streamline: the pen is pulled along on a string
        let o = self.out;
        if self.string > 0.0 {
            let d = dist(t, o);
            if d <= self.string {
                return None;
            }
            let f = (d - self.string) / d;
            t = (o.0 + (t.0 - o.0) * f, o.1 + (t.1 - o.1) * f);
        }
        if t == self.out {
            return None;
        }
        self.out = t;
        Some(t)
    }

    /// At the end of the stroke the line runs on to where the pointer was let go.
    pub fn finish(&mut self) -> Vec<(f32, f32)> {
        if self.last_in == self.out {
            return vec![];
        }
        let o = self.out;
        let e = self.last_in;
        let n = ((dist(e, o) / 2.0).floor() as usize).max(1);
        let pts: Vec<(f32, f32)> =
            (1..=n).map(|i| (o.0 + (e.0 - o.0) * i as f32 / n as f32, o.1 + (e.1 - o.1) * i as f32 / n as f32)).collect();
        self.out = e;
        pts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_set_is_inactive() {
        assert!(!Stabilizer::new([0, 0, 0], 1.0).active());
        assert!(Stabilizer::new([10, 0, 0], 1.0).active());
        assert!(Stabilizer::new([0, 10, 0], 1.0).active());
        assert!(Stabilizer::new([0, 0, 10], 1.0).active());
    }

    #[test]
    fn streamline_trails_behind_and_catches_up() {
        let mut s = Stabilizer::new([100, 0, 0], 1.0);
        s.start(0.0, 0.0);
        assert_eq!(s.feed(10.0, 0.0), None, "inside the string, nothing is drawn");
        let p = s.feed(100.0, 0.0).expect("past the string, the pen moves");
        assert!(p.0 > 55.0 && p.0 < 65.0, "trails the string's length behind: {p:?}");
        let rest = s.finish();
        assert_eq!(rest.last().copied(), Some((100.0, 0.0)), "the stroke runs on to the pointer");
    }

    #[test]
    fn motion_filtering_ignores_a_shaky_hand() {
        let mut s = Stabilizer::new([0, 0, 100], 1.0);
        s.start(0.0, 0.0);
        assert_eq!(s.feed(1.0, 1.0), None);
        assert_eq!(s.feed(0.0, 2.0), None);
        assert!(s.feed(40.0, 0.0).is_some(), "a real movement gets through");
    }

    #[test]
    fn stabilization_averages_the_last_few_points() {
        let mut s = Stabilizer::new([0, 100, 0], 1.0);
        s.start(0.0, 0.0);
        let mut last = (0.0, 0.0);
        for i in 1..=8 {
            if let Some(p) = s.feed(i as f32 * 10.0, 0.0) {
                last = p;
            }
        }
        assert!(last.0 < 60.0, "the line lags behind the pointer at 80: {last:?}");
    }

    #[test]
    fn zoom_keeps_the_feel_the_same_on_screen() {
        let a = Stabilizer::new([50, 0, 50], 1.0);
        let b = Stabilizer::new([50, 0, 50], 4.0);
        assert!((a.string / 4.0 - b.string).abs() < 1e-6);
        assert!((a.deadzone / 4.0 - b.deadzone).abs() < 1e-6);
    }
}
