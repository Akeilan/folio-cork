//! What Chord remembers between runs, as plain `key=value` lines in its
//! settings folder.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: u8,
    pub eq_on: bool,
    pub eq: [f64; 10],
    pub crossfade: f64,
    pub sound_check: bool,
    pub recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { volume: 0.8, shuffle: false, repeat: 0, eq_on: false, eq: [0.0; 10], crossfade: 0.0, sound_check: true, recent: Vec::new() }
    }
}

fn file() -> PathBuf {
    folio_kit::platform::config_dir("folio-chord").join("settings.txt")
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(file()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "volume" => s.volume = v.trim().parse::<f64>().unwrap_or(s.volume).clamp(0.0, 1.0),
                "shuffle" => s.shuffle = v.trim() == "1",
                "repeat" => s.repeat = v.trim().parse().unwrap_or(0).min(2),
                "eqon" => s.eq_on = v.trim() == "1",
                "crossfade" => s.crossfade = v.trim().parse::<f64>().unwrap_or(0.0).clamp(0.0, 12.0),
                "soundcheck" => s.sound_check = v.trim() == "1",
                "eq" => {
                    for (i, p) in v.trim().split(',').enumerate().take(10) {
                        s.eq[i] = p.parse().unwrap_or(0.0);
                    }
                }
                "recent" => {
                    if !v.trim().is_empty() {
                        s.recent = v.trim().split('\x1f').map(|p| p.to_string()).collect();
                    }
                }
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let p = file();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let eq = self.eq.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(",");
        let text = format!(
            "volume={}\nshuffle={}\nrepeat={}\neqon={}\neq={eq}\ncrossfade={}\nsoundcheck={}\nrecent={}\n",
            self.volume,
            if self.shuffle { 1 } else { 0 },
            self.repeat,
            if self.eq_on { 1 } else { 0 },
            self.crossfade,
            if self.sound_check { 1 } else { 0 },
            self.recent.join("\x1f")
        );
        let _ = std::fs::write(p, text);
    }

    /// Remember a file as the most recent one, keeping the last eight.
    pub fn remember(&mut self, p: &Path) {
        let s = p.to_string_lossy().to_string();
        self.recent.retain(|r| r != &s);
        self.recent.insert(0, s);
        self.recent.truncate(8);
        self.save();
    }
}
