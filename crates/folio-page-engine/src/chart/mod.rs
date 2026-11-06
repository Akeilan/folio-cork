//! Charts in documents (c:chartSpace parts): the part and its cached values,
//! read into a chart to draw. The drawing itself belongs to the renderer.

pub mod model;
pub mod xml;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

fn cache() -> &'static Mutex<HashMap<(usize, [u32; 12]), Arc<model::Chart>>> {
    static C: OnceLock<Mutex<HashMap<(usize, [u32; 12]), Arc<model::Chart>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The chart a part's bytes hold (read once per version of the part).
pub fn chart_of(bytes: &Arc<Vec<u8>>, theme: &[u32; 12]) -> Option<Arc<model::Chart>> {
    let key = (Arc::as_ptr(bytes) as usize, *theme);
    if let Some(c) = cache().lock().ok()?.get(&key) {
        return Some(c.clone());
    }
    let text = String::from_utf8_lossy(bytes);
    let ch = Arc::new(model::Chart::parse(&text, theme).ok()?);
    let mut m = cache().lock().ok()?;
    if m.len() > 64 {
        m.clear();
    }
    m.insert(key, ch.clone());
    Some(ch)
}
