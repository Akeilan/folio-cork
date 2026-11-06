use folio_page_engine::package::Package;
use folio_page_engine::xml::XmlDoc;

/// Every XML part survives open -> parse -> save unchanged in meaning.
#[test]
fn corpus_roundtrip() {
    // deep documents recurse; give the run a stack that can take them
    std::thread::Builder::new().stack_size(64 << 20).spawn(body).unwrap().join().unwrap();
}

fn body() {
    let dir = std::env::var("FOLIO_CORPUS").unwrap_or_else(|_| "/home/claude/corpus/poi/test-data/document".into());
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut n = 0;
    let mut bad = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("docx") {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        let Ok(mut pkg) = Package::open(&bytes) else { continue };
        let names = pkg.part_names();
        let mut parsed = Vec::new();
        for name in &names {
            if name.ends_with(".xml") || name.ends_with(".rels") {
                if pkg.xml_mut(name).is_ok() {
                    parsed.push(name.clone());
                }
            }
        }
        let out = pkg.write().unwrap();
        let mut again = Package::open(&out).unwrap();
        for name in &parsed {
            let a = XmlDoc::parse(Package::open(&bytes).unwrap().raw(name).unwrap()).unwrap().serialize();
            let b = again.xml(name).unwrap().serialize();
            if a != b {
                bad.push(format!("{:?} {}", p.file_name().unwrap(), name));
            }
        }
        n += 1;
    }
    assert!(bad.is_empty(), "round trip changed: {:?}", bad);
    eprintln!("{} documents round-tripped", n);
}
