//! What Reel remembers between runs, as plain `key=value` lines in its
//! settings folder.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub dark: bool,
    pub snapping: bool,
    pub waveforms: bool,
    pub show_bin: bool,
    pub show_props: bool,
    pub preset: String,
    pub recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            dark: false,
            snapping: true,
            waveforms: true,
            show_bin: true,
            show_props: true,
            preset: "Full HD video (MP4, 1080p)".into(),
            recent: Vec::new(),
        }
    }
}

fn file() -> PathBuf {
    folio_kit::platform::config_dir("folio-reel").join("settings.txt")
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(file()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "dark" => s.dark = v != "0",
                "snapping" => s.snapping = v != "0",
                "waveforms" => s.waveforms = v != "0",
                "show_bin" => s.show_bin = v != "0",
                "show_props" => s.show_props = v != "0",
                "preset" => s.preset = v.to_string(),
                "recent" => {
                    if !v.is_empty() {
                        s.recent = v.split('\x1f').map(|p| p.to_string()).collect();
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
        let b = |v: bool| if v { 1 } else { 0 };
        let text = format!(
            "dark={}\nsnapping={}\nwaveforms={}\nshow_bin={}\nshow_props={}\npreset={}\nrecent={}\n",
            b(self.dark),
            b(self.snapping),
            b(self.waveforms),
            b(self.show_bin),
            b(self.show_props),
            self.preset,
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
