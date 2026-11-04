//! Settings, recent files, My Shapes, and the copy-paste bridge between Cork windows.

use crate::model::{ImageData, Shape};
use folio_kit::Color;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    folio_kit::platform::config_dir("folio-cork")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub author: String,
    pub recent: Vec<String>,
    pub grid: bool,
    pub snap: bool,
    pub guides: bool,
    pub minimap: bool,
    pub rulers: bool,
    pub page_breaks: bool,
    pub shapes_pane: bool,
    pub side_pane: bool,
    pub show_resolved: bool,
    pub ribbon_collapsed: bool,
    pub last_fill: Color,
    pub last_line: Color,
    pub grid_size: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "system".into(),
            author: String::new(),
            recent: vec![],
            grid: true,
            snap: true,
            guides: true,
            minimap: true,
            rulers: true,
            page_breaks: true,
            shapes_pane: true,
            side_pane: false,
            show_resolved: false,
            ribbon_collapsed: false,
            last_fill: Color::hex(0xFFC000),
            last_line: Color::hex(0xC00000),
            grid_size: 12.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MyShape {
    pub name: String,
    pub shapes: Vec<Shape>,
    pub images: Vec<(String, ImageData)>,
}

impl Settings {
    pub fn load() -> Settings {
        std::fs::read_to_string(dir().join("settings.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let d = dir();
        let _ = std::fs::create_dir_all(&d);
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = folio_kit::platform::atomic_write(&d.join("settings.json"), s.as_bytes());
        }
    }

    pub fn add_recent(&mut self, p: &Path) {
        let p = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().to_string();
        self.recent.retain(|x| *x != p);
        self.recent.insert(0, p);
        self.recent.truncate(12);
        self.save();
    }

    pub fn my_shapes(&self) -> Vec<MyShape> {
        // Read again only when the file changed (another Cork window may have added one); the
        // shapes pane asks on every frame.
        let path = dir().join("my-shapes.json");
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        MY.with(|c| {
            let mut c = c.borrow_mut();
            if c.0 != stamp || c.2 != path {
                let v = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
                *c = (stamp, v, path);
            }
            c.1.clone()
        })
    }

    fn write_my(&self, v: &[MyShape]) {
        MY.with(|c| c.borrow_mut().0 = None);
        let d = dir();
        let _ = std::fs::create_dir_all(&d);
        if let Ok(s) = serde_json::to_string(v) {
            let _ = folio_kit::platform::atomic_write(&d.join("my-shapes.json"), s.as_bytes());
        }
    }

    pub fn add_my_shape(&mut self, name: &str, shapes: Vec<Shape>, images: Vec<(String, ImageData)>) {
        let mut v = self.my_shapes();
        let mut n = name.chars().take(40).collect::<String>();
        let base = n.clone();
        let mut i = 2;
        while v.iter().any(|m| m.name == n) {
            n = format!("{base} {i}");
            i += 1;
        }
        v.push(MyShape { name: n, shapes, images });
        self.write_my(&v);
    }

    pub fn remove_my_shape(&mut self, name: &str) {
        let mut v = self.my_shapes();
        v.retain(|m| m.name != name);
        self.write_my(&v);
    }
}

fn hash(b: &[u8]) -> u64 {
    let mut h: u64 = 1469598103934665603;
    for x in b.iter().step_by(7) {
        h = (h ^ *x as u64).wrapping_mul(1099511628211);
    }
    h ^ b.len() as u64
}

#[derive(Serialize, Deserialize)]
struct Clip {
    hash: u64,
    w: u32,
    h: u32,
    shapes: Vec<Shape>,
    images: Vec<(String, ImageData)>,
}

/// Keep copied shapes beside the picture put on the clipboard, so a later paste of that same
/// picture into Cork brings back editable shapes (also across Cork windows).
pub fn store_clip(shapes: &[Shape], images: &[(String, ImageData)], img: &folio_kit::platform::Image) {
    let c = Clip { hash: hash(&img.rgba), w: img.w, h: img.h, shapes: shapes.to_vec(), images: images.to_vec() };
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    if let Ok(s) = serde_json::to_string(&c) {
        let _ = folio_kit::platform::atomic_write(&d.join("clipboard.json"), s.as_bytes());
    }
}

pub fn match_clip(img: &folio_kit::platform::Image) -> Option<(Vec<Shape>, Vec<(String, ImageData)>)> {
    let s = std::fs::read_to_string(dir().join("clipboard.json")).ok()?;
    let c: Clip = serde_json::from_str(&s).ok()?;
    if c.w == img.w && c.h == img.h && c.hash == hash(&img.rgba) {
        Some((c.shapes, c.images))
    } else {
        None
    }
}

thread_local! {
    static MY: std::cell::RefCell<(Option<std::time::SystemTime>, Vec<MyShape>, std::path::PathBuf)> = Default::default();
}
