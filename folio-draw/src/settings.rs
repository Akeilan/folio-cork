//! What Draw remembers between runs, as plain `key=value` lines in its settings folder.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub size: f32,
    pub c1: [u8; 4],
    pub c2: [u8; 4],
    pub rulers: bool,
    pub grid: bool,
    pub layers: bool,
    pub wand_tol: f32,
    pub wand_contiguous: bool,
    pub sample_all: bool,
    pub stab: [[u8; 3]; 3],
    pub quick: Vec<String>,
    pub recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { size: 3.0, c1: [0, 0, 0, 255], c2: [255, 255, 255, 255], rulers: false, grid: false, layers: false, wand_tol: 32.0, wand_contiguous: true, sample_all: false, stab: [[0; 3]; 3], quick: vec!["save".into(), "undo".into(), "redo".into()], recent: vec![] }
    }
}

fn file() -> PathBuf {
    folio_kit::platform::config_dir("folio-draw").join("settings.txt")
}

fn color(v: &str) -> Option<[u8; 4]> {
    let v = v.trim().trim_start_matches('#');
    if v.len() != 8 {
        return None;
    }
    let n = u32::from_str_radix(v, 16).ok()?;
    Some([(n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8])
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        let Ok(text) = std::fs::read_to_string(file()) else { return s };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k.trim() {
                "size" => s.size = v.trim().parse().unwrap_or(s.size),
                "color1" => s.c1 = color(v).unwrap_or(s.c1),
                "color2" => s.c2 = color(v).unwrap_or(s.c2),
                "rulers" => s.rulers = v.trim() == "1",
                "grid" => s.grid = v.trim() == "1",
                "layers" => s.layers = v.trim() == "1",
                "wandtol" => s.wand_tol = v.trim().parse().unwrap_or(s.wand_tol),
                "wandcontiguous" => s.wand_contiguous = v.trim() == "1",
                "sampleall" => s.sample_all = v.trim() == "1",
                "stabilizer" => {
                    // slot,a,b,c
                    let n: Vec<u8> = v.trim().split(',').map(|x| x.trim().parse().unwrap_or(0)).collect();
                    if n.len() == 4 && (n[0] as usize) < 3 {
                        s.stab[n[0] as usize] = [n[1], n[2], n[3]];
                    }
                }
                "recent" => s.recent.push(v.to_string()),
                "quick" => s.quick = v.trim().split(',').filter(|x| !x.trim().is_empty()).map(|x| x.trim().to_string()).collect(),
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let c = |c: [u8; 4]| format!("#{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3]);
        let mut out = format!(
            "size={}\ncolor1={}\ncolor2={}\nrulers={}\ngrid={}\nlayers={}\nwandtol={}\nwandcontiguous={}\nsampleall={}\n",
            self.size,
            c(self.c1),
            c(self.c2),
            self.rulers as u8,
            self.grid as u8,
            self.layers as u8,
            self.wand_tol,
            self.wand_contiguous as u8,
            self.sample_all as u8
        );
        for (i, v) in self.stab.iter().enumerate() {
            if *v != [0, 0, 0] {
                out += &format!("stabilizer={},{},{},{}\n", i, v[0], v[1], v[2]);
            }
        }
        out += &format!("quick={}\n", self.quick.join(","));
        for r in &self.recent {
            out += &format!("recent={r}\n");
        }
        let f = file();
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = folio_kit::platform::atomic_write(&f, out.as_bytes());
    }

    pub fn add_recent(&mut self, p: &Path) {
        let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().to_string();
        self.recent.retain(|x| *x != p);
        self.recent.insert(0, p);
        self.recent.truncate(12);
    }
}
