//! The engine reads a folder of music, keeps it, and decodes a song.
use folio_chord_engine::library::{Kind, Library};
use folio_chord_engine::player::Player;
use folio_chord_engine::smart::{Op, Rule, Rules};
use std::path::{Path, PathBuf};

fn music_dir() -> Option<PathBuf> {
    let p = PathBuf::from("/home/claude/winbuild/t2/music");
    p.is_dir().then_some(p)
}

#[test]
fn reads_tags_from_a_file() {
    let Some(dir) = music_dir() else { return };
    let f = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| folio_chord_engine::tags::is_music(p));
    let Some(f) = f else { return };
    let t = folio_chord_engine::tags::read(&f).expect("tags");
    assert!(!t.title.is_empty(), "the song has a title");
    assert!(t.duration > 0.0, "and a length");
    assert!(t.sample_rate >= 8000, "and a sample rate");
}

#[test]
fn scans_a_folder_and_writes_the_library_back() {
    let Some(dir) = music_dir() else { return };
    let tmp = folio_kit::platform::temp_dir().join(format!("chord-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let path = tmp.join("library.txt");
    let _ = std::fs::remove_file(&path);
    let mut lib = Library::open(&path);
    lib.add_folder(&dir);
    let (added, _) = lib.rescan();
    assert!(added > 0, "the folder has songs in it");
    let id = lib.tracks[0].id;
    lib.set_rating(id, 4);
    lib.played(id);
    let name = lib.tracks[0].tags.title.clone();
    lib.save().expect("save");

    let again = Library::open(&path);
    assert_eq!(again.tracks.len(), lib.tracks.len(), "every song came back");
    let t = again.track(id).expect("the song came back");
    assert_eq!(t.tags.title, name);
    assert_eq!(t.rating, 80, "the four stars came back");
    assert_eq!(t.play_count, 1, "and the play count");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_smart_playlist_picks_songs_out() {
    let Some(dir) = music_dir() else { return };
    let tmp = folio_kit::platform::temp_dir().join(format!("chord-smart-{}", std::process::id()));
    let path = tmp.join("library.txt");
    let _ = std::fs::remove_file(&path);
    let mut lib = Library::open(&path);
    lib.add_folder(&dir);
    lib.rescan();
    let genre = lib.tracks.iter().find(|t| !t.tags.genre.is_empty()).map(|t| t.tags.genre.clone());
    let Some(genre) = genre else { return };
    let id = lib.new_playlist("Just that genre", Kind::Smart);
    if let Some(p) = lib.playlists.iter_mut().find(|p| p.id == id) {
        p.rules = Rules { all: true, rules: vec![Rule { field: "genre".into(), op: Op::Is, value: genre.clone() }], ..Default::default() };
    }
    let picked = lib.playlist_tracks(id);
    assert!(!picked.is_empty(), "the rule found songs");
    for i in &picked {
        assert_eq!(lib.track(*i).unwrap().tags.genre, genre);
    }
    lib.save().expect("save");
    let again = Library::open(&path);
    assert_eq!(again.playlist(id).unwrap().rules.rules.len(), 1, "the rule came back");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn the_player_decodes_a_song() {
    let Some(dir) = music_dir() else { return };
    let f = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| folio_chord_engine::tags::is_music(p));
    let Some(f) = f else { return };
    let p = Player::new(48000, 2);
    p.play(1, Path::new(&f), 0.0);
    let mut out = vec![0.0f32; 4096];
    let mut heard = false;
    for _ in 0..200 {
        folio_kit::platform::sleep(std::time::Duration::from_millis(10));
        p.fill(&mut out);
        if out.iter().any(|s| s.abs() > 0.001) {
            heard = true;
            break;
        }
    }
    assert!(heard, "the song made sound");
    let st = p.status();
    assert!(st.position > 0.0, "and the position moved: {st:?}");
}
