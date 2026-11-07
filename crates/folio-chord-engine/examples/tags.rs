fn main() {
    for a in std::env::args().skip(1) {
        match folio_chord_engine::tags::read(std::path::Path::new(&a)) {
            Some(t) => println!("{a}\n  {:?}", t),
            None => println!("{a}\n  (unreadable)"),
        }
    }
}
