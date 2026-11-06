//! Page counts against what the original recorded (docProps/app.xml) for
//! the corpus. Guards against regressions: the number of exact matches
//! may only go up.
use folio_page_engine::document::Document;
use folio_page_engine::fonts::FontCache;
use folio_page_engine::paginate::layout_document;

/// exact matches with the whole corpus (Apache POI's test documents plus the Word
/// corpus), and with POI's alone when the Word corpus isn't on this machine
const BASELINE: usize = 1231;
const BASELINE_POI: usize = 100;

#[test]
fn page_counts_match_word() {
    // deep documents recurse; give the run a stack that can take them
    std::thread::Builder::new().stack_size(64 << 20).spawn(body).unwrap().join().unwrap();
}

fn body() {
    let dirs = std::env::var("FOLIO_CORPUS")
        .unwrap_or("/home/claude/corpus/poi/test-data/document:/home/claude/corpus/word".into());
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for dir in dirs.split(':') {
        if let Ok(rd) = std::fs::read_dir(dir) {
            paths.extend(rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "docx").unwrap_or(false)));
        }
    }
    if paths.is_empty() {
        return;
    }
    let mut fonts = FontCache::new();
    let (mut exact, mut total) = (0, 0);
    let mut misses = Vec::new();
    paths.sort();
    for p in paths {
        let Ok(bytes) = std::fs::read(&p) else { continue };
        let Ok(doc) = Document::open(&bytes) else { continue };
        let app = doc.pkg.raw("docProps/app.xml").map(|b| String::from_utf8_lossy(b).to_string()).unwrap_or_default();
        let Some(want) = app.split("<Pages>").nth(1).and_then(|s| s.split('<').next()).and_then(|s| s.trim().parse::<usize>().ok()) else { continue };
        let got = layout_document(&doc, &mut fonts).pages.len();
        total += 1;
        if got == want { exact += 1 } else { misses.push(format!("{}: {got} vs {want}", p.file_name().unwrap().to_string_lossy())) }
    }
    eprintln!("exact {exact}/{total}");
    if let Ok(out) = std::env::var("FOLIO_MISSES") {
        std::fs::write(out, misses.join("\n")).ok();
    }
    let base = if std::path::Path::new("/home/claude/corpus/word").exists() { BASELINE } else { BASELINE_POI };
    assert!(exact >= base, "page counts regressed: {exact} < {base}");
}
