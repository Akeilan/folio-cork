//! What Page remembers between runs, as plain `key=value` lines in its
//! settings folder.

use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub zoom: f32,
    pub marks: bool,
    /// 0 Print Layout, 1 Draft, 2 Web Layout
    pub view_mode: u8,
    pub recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { zoom: 1.0, marks: false, view_mode: 0, recent: Vec::new() }
    }
}

fn file() -> PathBuf {
    folio_kit::platform::config_dir("folio-page").join("settings.txt")
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(file()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "zoom" => s.zoom = v.trim().parse().unwrap_or(s.zoom).clamp(0.1, 4.0),
                "marks" => s.marks = v.trim() == "1",
                "view" => s.view_mode = v.trim().parse().unwrap_or(0).min(2),
                "recent" => {
                    if !v.trim().is_empty() {
                        s.recent.push(v.trim().to_string());
                    }
                }
                _ => {}
            }
        }
        s.recent.truncate(10);
        s
    }

    pub fn save(&self) {
        let mut out = String::new();
        out.push_str(&format!("zoom={}\n", self.zoom));
        out.push_str(&format!("marks={}\n", if self.marks { 1 } else { 0 }));
        out.push_str(&format!("view={}\n", self.view_mode));
        for r in self.recent.iter().take(10) {
            out.push_str(&format!("recent={r}\n"));
        }
        let f = file();
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = folio_kit::platform::atomic_write(&f, out.as_bytes());
    }

    /// A document that has just been opened or saved goes to the top of the
    /// list, and only once.
    pub fn remember(&mut self, path: &std::path::Path) {
        let p = path.to_string_lossy().to_string();
        self.recent.retain(|r| *r != p);
        self.recent.insert(0, p);
        self.recent.truncate(10);
    }
}
