//! What Sheet remembers between runs, as plain `key=value` lines in its
//! settings folder.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub zoom: f64,
    pub recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { zoom: 1.0, recent: Vec::new() }
    }
}

fn file() -> PathBuf {
    folio_kit::platform::config_dir("folio-sheet").join("settings.txt")
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(file()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "zoom" => s.zoom = v.trim().parse().unwrap_or(s.zoom).clamp(0.1, 4.0),
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
        let text = format!("zoom={}\nrecent={}\n", self.zoom, self.recent.join("\x1f"));
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
