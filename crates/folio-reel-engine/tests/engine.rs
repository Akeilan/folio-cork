//! What Reel's engine has to get right.
use folio_reel_engine::edits::{default_text, Source};
use folio_reel_engine::project::*;
use folio_reel_engine::render;
use std::path::PathBuf;

fn vid(name: &str, secs: f64) -> Media {
    Media {
        uri: format!("file:///tmp/{}", name),
        name: name.to_string(),
        duration: secs,
        width: 640,
        height: 360,
        fps: 25.0,
        has_video: true,
        has_audio: true,
        ..Media::default()
    }
}

fn still(name: &str) -> Media {
    Media { uri: format!("file:///tmp/{}", name), name: name.into(), width: 800, height: 600, has_video: true, is_image: true, ..Media::default() }
}

fn sound(name: &str, secs: f64) -> Media {
    Media { uri: format!("file:///tmp/{}", name), name: name.into(), duration: secs, has_audio: true, ..Media::default() }
}

fn with(media: &[Media]) -> Project {
    let mut p = Project::new();
    for m in media {
        p.add_media(m.clone());
    }
    p
}

#[test]
fn a_new_project_is_full_hd_at_thirty() {
    let p = Project::new();
    assert_eq!((p.data.width, p.data.height), (1920, 1080));
    assert_eq!(p.data.fps, 30.0);
    assert_eq!(p.duration(), 0.0);
    assert!(!p.can_undo());
}

#[test]
fn video_lands_on_the_video_track_and_sound_on_the_audio_track() {
    let p = with(&[vid("a.mp4", 10.0), sound("b.wav", 4.0)]);
    assert_eq!(p.media_track(p.media_by_uri("file:///tmp/a.mp4").unwrap()), 1);
    assert_eq!(p.media_track(p.media_by_uri("file:///tmp/b.wav").unwrap()), 2);
}

#[test]
fn a_still_gets_five_seconds() {
    let mut p = with(&[still("s.png")]);
    let m = p.media_by_uri("file:///tmp/s.png").unwrap().clone();
    let id = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    assert_eq!(p.clip(id).unwrap().duration, IMAGE_SECONDS);
}

#[test]
fn clips_queue_up_rather_than_land_on_each_other() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), Some(0.0), false).unwrap();
    assert_eq!(p.clip(b).unwrap().start, 5.0);
    assert_eq!(p.duration(), 10.0);
}

#[test]
fn an_insert_pushes_what_follows_later() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), Some(0.0), true).unwrap();
    assert_eq!(p.clip(b).unwrap().start, 0.0);
    assert_eq!(p.clip(a).unwrap().start, 5.0);
}

#[test]
fn sound_cannot_go_on_the_video_track() {
    let mut p = with(&[sound("b.wav", 4.0)]);
    let m = p.media_by_uri("file:///tmp/b.wav").unwrap().clone();
    let e = p.add_clip(0, Source::Media(&m), None, false).unwrap_err();
    assert!(e.0.contains("Titles track is for"), "{}", e.0);
}

#[test]
fn a_silent_film_cannot_go_on_the_audio_track() {
    let mut p = with(&[still("s.png")]);
    let m = p.media_by_uri("file:///tmp/s.png").unwrap().clone();
    let e = p.add_clip(2, Source::Media(&m), None, false).unwrap_err();
    assert!(e.0.contains("no sound"), "{}", e.0);
}

#[test]
fn a_title_keeps_its_first_line_as_its_name() {
    let mut p = Project::new();
    let id = p.add_clip(0, Source::Text(Kind::Title, "Opening\nsecond line".into()), None, false).unwrap();
    let c = p.clip(id).unwrap();
    assert_eq!(c.name, "Opening");
    assert_eq!(c.duration, TITLE_SECONDS);
    assert_eq!(c.valign, "center");
    assert_eq!(default_text(Kind::Caption), "Caption");
}

#[test]
fn a_split_divides_the_media_as_well_as_the_time() {
    let mut p = with(&[vid("a.mp4", 10.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.split(a, 4.0).unwrap();
    assert_eq!(p.clip(a).unwrap().duration, 4.0);
    assert_eq!(p.clip(b).unwrap().start, 4.0);
    assert_eq!(p.clip(b).unwrap().duration, 6.0);
    assert_eq!(p.clip(b).unwrap().inpoint, 4.0);
}

#[test]
fn a_split_off_the_clip_is_refused_in_words() {
    let mut p = with(&[vid("a.mp4", 10.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    assert!(p.split(a, 99.0).unwrap_err().0.contains("playhead"));
}

#[test]
fn volume_points_follow_the_half_they_belong_to() {
    let mut p = with(&[vid("a.mp4", 10.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.set_keyframes(a, vec![(1.0, 0.5), (7.0, 1.5)]).unwrap();
    let b = p.split(a, 4.0).unwrap();
    assert_eq!(p.clip(a).unwrap().keyframes, vec![(1.0, 0.5)]);
    assert_eq!(p.clip(b).unwrap().keyframes, vec![(3.0, 1.5)]);
}

#[test]
fn a_ripple_delete_closes_the_gap() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.delete(&[a], true);
    assert_eq!(p.clip(b).unwrap().start, 0.0);
}

#[test]
fn a_plain_delete_leaves_the_gap() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.delete(&[a], false);
    assert_eq!(p.clip(b).unwrap().start, 5.0);
}

#[test]
fn trimming_the_start_moves_the_in_point_with_it() {
    let mut p = with(&[vid("a.mp4", 10.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.trim(a, Edge::Start, 3.0, 0.0, None).unwrap();
    let c = p.clip(a).unwrap();
    assert_eq!((c.start, c.inpoint, c.duration), (3.0, 3.0, 7.0));
}

#[test]
fn a_clip_cannot_be_trimmed_past_the_end_of_its_media() {
    let mut p = with(&[vid("a.mp4", 6.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.trim(a, Edge::End, 30.0, 0.0, None).unwrap();
    assert_eq!(p.clip(a).unwrap().duration, 6.0);
}

#[test]
fn a_still_can_be_stretched_as_far_as_you_like() {
    let mut p = with(&[still("s.png")]);
    let m = p.media_by_uri("file:///tmp/s.png").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.trim(a, Edge::End, 40.0, 0.0, None).unwrap();
    assert_eq!(p.clip(a).unwrap().duration, 40.0);
}

#[test]
fn speed_shortens_the_clip_and_moves_what_follows() {
    let mut p = with(&[vid("a.mp4", 8.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.set_speed(a, 2.0).unwrap();
    assert_eq!(p.clip(a).unwrap().duration, 4.0);
    assert_eq!(p.clip(b).unwrap().start, 4.0);
}

#[test]
fn speed_stays_between_a_quarter_and_four() {
    let mut p = with(&[vid("a.mp4", 8.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.set_speed(a, 99.0).unwrap();
    assert_eq!(p.clip(a).unwrap().speed, 4.0);
}

#[test]
fn a_crossfade_overlaps_the_two_clips() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let _a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.set_transition(b, Some("crossfade"), 1.0).unwrap();
    assert_eq!(p.clip(b).unwrap().start, 4.0);
    assert_eq!(p.duration(), 9.0);
}

#[test]
fn fade_to_black_leaves_the_clips_where_they_are() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let _a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.set_transition(b, Some("dip-black"), 1.0).unwrap();
    assert_eq!(p.clip(b).unwrap().start, 5.0);
}

#[test]
fn a_transition_needs_a_clip_before_it() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    assert!(p.set_transition(a, Some("crossfade"), 1.0).unwrap_err().0.contains("second one"));
}

#[test]
fn transitions_are_not_for_the_sound_tracks() {
    let mut p = with(&[sound("b.wav", 4.0)]);
    let m = p.media_by_uri("file:///tmp/b.wav").unwrap().clone();
    p.add_clip(2, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(2, Source::Media(&m), None, false).unwrap();
    assert!(p.set_transition(b, Some("crossfade"), 1.0).unwrap_err().0.contains("use fades"));
}

#[test]
fn undo_puts_everything_back_and_redo_brings_it_again() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.set_speed(a, 2.0).unwrap();
    assert_eq!(p.undo_label(), Some("Speed"));
    p.undo();
    assert_eq!(p.clip(a).unwrap().speed, 1.0);
    p.redo();
    assert_eq!(p.clip(a).unwrap().speed, 2.0);
}

#[test]
fn a_dragged_slider_is_one_undo_step() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let before = p.can_undo();
    for v in [0.2, 0.4, 0.6] {
        p.checkpoint("Volume", Some("volume"));
        let id = p.track(1)[0].id;
        p.find_mut(id).unwrap().1.volume = v;
    }
    assert!(before);
    p.undo();
    assert_eq!(p.track(1)[0].volume, 1.0);
}

#[test]
fn a_move_to_another_track_is_refused_when_there_is_nothing_to_show() {
    let mut p = with(&[sound("b.wav", 4.0)]);
    let m = p.media_by_uri("file:///tmp/b.wav").unwrap().clone();
    let a = p.add_clip(2, Source::Media(&m), None, false).unwrap();
    assert!(p.move_clip(a, 0.0, Some(1), 0.0, None).unwrap_err().0.contains("no picture"));
}

#[test]
fn a_clip_dropped_on_another_takes_the_nearest_free_place() {
    let mut p = with(&[vid("a.mp4", 5.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let _a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.move_clip(b, 1.0, None, 0.0, None).unwrap();
    assert_eq!(p.clip(b).unwrap().start, 5.0);
}

#[test]
fn moving_snaps_to_an_edge_nearby() {
    let mut p = with(&[vid("a.mp4", 5.0), vid("c.mp4", 3.0)]);
    let a = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let c = p.media_by_uri("file:///tmp/c.mp4").unwrap().clone();
    p.add_clip(1, Source::Media(&a), None, false).unwrap();
    let x = p.add_clip(0, Source::Media(&c), Some(10.0), false).unwrap();
    p.move_clip(x, 4.8, None, 0.5, None).unwrap();
    assert_eq!(p.clip(x).unwrap().start, 5.0);
}

#[test]
fn a_group_moves_together_or_not_at_all() {
    let mut p = with(&[vid("a.mp4", 4.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(2, Source::Media(&m), None, false).unwrap();
    let g = p.group(&[a, b]).unwrap();
    p.move_group(g, 2.0, 0.0, None).unwrap();
    assert_eq!(p.clip(a).unwrap().start, 2.0);
    assert_eq!(p.clip(b).unwrap().start, 2.0);
    p.ungroup(&[a]);
    assert_eq!(p.group_of(a), 0);
}

#[test]
fn copy_and_paste_keeps_the_spacing() {
    let mut p = with(&[vid("a.mp4", 2.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), Some(5.0), false).unwrap();
    let items = p.copy_out(&[a, b]);
    let made = p.paste(&items, 10.0);
    assert_eq!(made.len(), 2);
    let starts: Vec<f64> = made.iter().map(|id| p.clip(*id).unwrap().start).collect();
    assert_eq!(starts, vec![10.0, 15.0]);
}

#[test]
fn markers_can_be_put_down_renamed_and_walked_through() {
    let mut p = Project::new();
    let a = p.add_marker(2.0, "One", "red");
    p.add_marker(8.0, "Two", "blue");
    assert_eq!(p.markers().len(), 2);
    assert_eq!(p.add_marker(2.0, "Again", "green"), a);
    assert_eq!(p.marker(a).unwrap().name, "Again");
    assert_eq!(p.next_marker(2.0).unwrap().t, 8.0);
    assert_eq!(p.prev_marker(8.0).unwrap().t, 2.0);
    assert_eq!(p.clear_markers(), 2);
}

#[test]
fn a_marker_colour_we_do_not_know_comes_out_yellow() {
    let mut p = Project::new();
    let id = p.add_marker(1.0, "x", "chartreuse");
    assert_eq!(p.marker(id).unwrap().color, "yellow");
}

#[test]
fn subtitles_arrive_as_captions_at_their_own_times() {
    let mut p = Project::new();
    let made = p.add_subtitles(&[(0.0, 2.0, "Hello".into()), (3.0, 5.0, "There".into())]);
    assert_eq!(made.len(), 2);
    assert_eq!(p.clip(made[1]).unwrap().start, 3.0);
    assert_eq!(p.clip(made[1]).unwrap().kind, Kind::Caption);
}

#[test]
fn a_project_saved_and_opened_again_is_the_same_project() {
    let mut p = with(&[vid("a.mp4", 9.0), sound("b.wav", 4.0)]);
    let a = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let b = p.media_by_uri("file:///tmp/b.wav").unwrap().clone();
    let c1 = p.add_clip(1, Source::Media(&a), None, false).unwrap();
    p.add_clip(2, Source::Media(&b), None, false).unwrap();
    let t = p.add_clip(0, Source::Text(Kind::Title, "Hello".into()), Some(1.0), false).unwrap();
    p.set_keyframes(c1, vec![(1.0, 0.4)]).unwrap();
    p.add_marker(3.0, "here", "red");
    p.find_mut(t).unwrap().1.panzoom = "in".into();
    let text = folio_reel_engine::project::to_json(&p);
    let q = folio_reel_engine::project::read(&text, None).unwrap();
    assert_eq!(q.data.width, 1920);
    assert_eq!(q.track(1).len(), 1);
    assert_eq!(q.track(2).len(), 1);
    assert_eq!(q.track(0).len(), 1);
    assert_eq!(q.clip(c1).unwrap().keyframes, vec![(1.0, 0.4)]);
    assert_eq!(q.clip(t).unwrap().panzoom, "in");
    assert_eq!(q.markers().len(), 1);
    assert_eq!(q.data.media.len(), 2);
}

#[test]
fn a_file_that_is_not_a_project_says_so() {
    assert!(folio_reel_engine::project::read("{\"hello\": 1}", None).unwrap_err().0.contains("isn't a Folio Reel project"));
    assert!(folio_reel_engine::project::read("not json at all", None).is_err());
}

#[test]
fn a_project_from_the_older_reel_opens() {
    // the shape the Python Reel writes, with keys this build has never seen
    let text = r#"{"version": 1, "width": 1280, "height": 720, "fps": 25,
      "media": [{"uri": "file:///tmp/a.mp4", "name": "a.mp4", "duration": 8.0, "width": 640,
                 "height": 360, "fps": 25.0, "has_video": true, "has_audio": true, "is_image": false}],
      "tracks": {"titles": [], "video": [{"id": 1, "kind": "media", "media": "file:///tmp/a.mp4",
                 "name": "a.mp4", "start": 0.0, "inpoint": 0.0, "duration": 8.0, "speed": 1.0,
                 "volume": 1.0, "fade_in": 0.0, "fade_out": 0.5, "zoom": 1.0, "panzoom": null,
                 "transition": null, "music_bed": false, "keyframes": [], "something_new": 7}],
                 "audio": [], "music": []},
      "markers": [{"id": 2, "t": 1.5, "name": "cue", "color": "green"}], "next_id": 3}"#;
    let p = folio_reel_engine::project::read(text, None).unwrap();
    assert_eq!((p.data.width, p.data.height, p.data.fps), (1280, 720, 25.0));
    assert_eq!(p.track(1)[0].fade_out, 0.5);
    assert_eq!(p.markers()[0].name, "cue");
    // and what it did not know is still there when it saves
    let again = folio_reel_engine::project::to_json(&p);
    assert!(again.contains("something_new"), "{}", again);
}

#[test]
fn a_uri_survives_a_space_in_the_name() {
    let u = path_to_uri(&PathBuf::from("/tmp/my film.mp4"));
    assert!(u.contains("%20"), "{}", u);
    assert_eq!(uri_to_path(&u).unwrap(), PathBuf::from("/tmp/my film.mp4"));
}

#[test]
fn timecode_reads_as_people_write_it() {
    assert_eq!(fmt_time(0.0, None), "0:00.0");
    assert_eq!(fmt_time(65.4, None), "1:05.4");
    assert_eq!(fmt_time(3725.0, None), "1:02:05");
    assert_eq!(fmt_time(1.5, Some(25.0)), "0:00:01:13");
}

#[test]
fn the_music_dips_under_the_voice() {
    let mut p = with(&[sound("music.mp3", 30.0), sound("voice.wav", 5.0)]);
    let music = p.media_by_uri("file:///tmp/music.mp3").unwrap().clone();
    let voice = p.media_by_uri("file:///tmp/voice.wav").unwrap().clone();
    let m = p.add_clip(3, Source::Media(&music), Some(0.0), false).unwrap();
    p.add_clip(2, Source::Media(&voice), Some(10.0), false).unwrap();
    p.find_mut(m).unwrap().1.music_bed = true;
    let pts = p.duck_points(m, 0.3, 0.4);
    assert!(pts.iter().any(|(t, g)| (*t - 10.0).abs() < 0.01 && (*g - 0.3).abs() < 0.01), "{:?}", pts);
    assert!(pts.iter().any(|(t, g)| (*t - 15.4).abs() < 0.01 && (*g - 1.0).abs() < 0.01), "{:?}", pts);
    // and the gain curve picks it up
    let c = p.clip(m).unwrap();
    let g = render::gain_points(&p, c, 0.0, 0.0);
    assert!(g.iter().any(|(_, v)| (*v - 0.3).abs() < 0.01), "{:?}", g);
}

#[test]
fn a_gain_curve_reads_flat_outside_its_ends() {
    let pts = [(1.0, 0.0), (2.0, 1.0)];
    assert_eq!(render::piecewise(&pts, 0.0), 0.0);
    assert_eq!(render::piecewise(&pts, 1.5), 0.5);
    assert_eq!(render::piecewise(&pts, 9.0), 1.0);
}

#[test]
fn an_export_of_nothing_says_so() {
    let p = Project::new();
    let t = render::Target::of(&p, "mp4", "1080p");
    assert!(render::export_command(&p, &t, &PathBuf::from("/tmp/out.mp4")).unwrap_err().0.contains("nothing on the timeline"));
}

#[test]
fn the_export_command_has_the_inputs_the_graph_and_the_codecs() {
    let mut p = with(&[vid("a.mp4", 6.0), sound("b.wav", 4.0)]);
    let a = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let b = p.media_by_uri("file:///tmp/b.wav").unwrap().clone();
    p.add_clip(1, Source::Media(&a), None, false).unwrap();
    p.add_clip(2, Source::Media(&b), None, false).unwrap();
    let t = render::Target::of(&p, "mp4", "720p");
    let cmd = render::export_command(&p, &t, &PathBuf::from("/tmp/out.mp4")).unwrap();
    let line = cmd.args.join(" ");
    assert!(line.contains("-filter_complex"));
    assert!(line.contains("libx264"), "{}", line);
    assert!(line.contains("aac"), "{}", line);
    assert!(line.contains("1280x720"), "{}", line);
    assert!(line.contains("[vout]") && line.contains("[aout]"));
    assert_eq!(cmd.duration, 6.0);
}

#[test]
fn a_silent_project_asks_for_no_sound_track() {
    let mut p = with(&[still("s.png")]);
    let m = p.media_by_uri("file:///tmp/s.png").unwrap().clone();
    p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let t = render::Target::of(&p, "mp4", "1080p");
    let cmd = render::export_command(&p, &t, &PathBuf::from("/tmp/out.mp4")).unwrap();
    let line = cmd.args.join(" ");
    assert!(!line.contains("[aout]"), "{}", line);
    assert!(line.contains("-loop 1"), "{}", line);
}

#[test]
fn the_web_preset_asks_for_vp9_and_opus() {
    let mut p = with(&[vid("a.mp4", 3.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let t = render::Target::of(&p, "webm", "1080p");
    let line = render::export_command(&p, &t, &PathBuf::from("/tmp/out.webm")).unwrap().args.join(" ");
    assert!(line.contains("libvpx-vp9"), "{}", line);
    assert!(line.contains("libopus"), "{}", line);
}

#[test]
fn a_wipe_becomes_ffmpegs_own_transition() {
    let mut p = with(&[vid("a.mp4", 4.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.set_transition(b, Some("wipe-right"), 1.0).unwrap();
    let t = render::Target::of(&p, "mp4", "1080p");
    let line = render::export_command(&p, &t, &PathBuf::from("/tmp/out.mp4")).unwrap().args.join(" ");
    assert!(line.contains("xfade=transition=wiperight"), "{}", line);
    assert_eq!(render::xfade_name("iris").0, "circleopen");
    assert!(render::is_fade("crossfade") && !render::is_fade("box"));
}

#[test]
fn a_crossfade_fades_the_upper_clip_in() {
    let mut p = with(&[vid("a.mp4", 4.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    p.add_clip(1, Source::Media(&m), None, false).unwrap();
    let b = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.set_transition(b, Some("crossfade"), 1.0).unwrap();
    let clips = p.track(1).to_vec();
    let marks = render::overlaps(&clips);
    let ov = marks.iter().find(|(id, _)| *id == b).unwrap().1;
    assert_eq!(ov.alpha_in, 1.0);
    let line = render::export_command(&p, &render::Target::of(&p, "mp4", "1080p"), &PathBuf::from("/tmp/o.mp4")).unwrap().args.join(" ");
    assert!(line.contains("fade=t=in:st=0:d=1:alpha=1"), "{}", line);
}

#[test]
fn colour_crop_and_chroma_all_reach_the_graph() {
    let mut p = with(&[vid("a.mp4", 4.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    {
        let (_, c) = p.find_mut(a).unwrap();
        c.color = Color { brightness: 0.1, contrast: 1.2, saturation: 0.8, hue: 0.0 };
        c.crop = Crop { l: 0.1, r: 0.1, t: 0.0, b: 0.0 };
        c.chroma = true;
        c.chroma_color = "blue".into();
        c.zoom = 1.5;
    }
    let line = render::export_command(&p, &render::Target::of(&p, "mp4", "1080p"), &PathBuf::from("/tmp/o.mp4")).unwrap().args.join(" ");
    assert!(line.contains("eq=brightness=0.1"), "{}", line);
    assert!(line.contains("crop=iw-128:ih-0:64:0"), "{}", line);
    assert!(line.contains("colorkey=0x0000FF"), "{}", line);
    assert_eq!(render::chroma_target("#ff8800"), (255, 136, 0));
}

#[test]
fn a_volume_curve_becomes_an_expression_in_time() {
    let mut p = with(&[sound("b.wav", 10.0)]);
    let m = p.media_by_uri("file:///tmp/b.wav").unwrap().clone();
    let a = p.add_clip(2, Source::Media(&m), None, false).unwrap();
    p.set_keyframes(a, vec![(0.0, 0.0), (5.0, 1.0)]).unwrap();
    let line = render::export_command(&p, &render::Target::of(&p, "mp4", "1080p"), &PathBuf::from("/tmp/o.mp4")).unwrap().args.join(" ");
    assert!(line.contains("volume='if(lt(t"), "{}", line);
    assert!(line.contains(":eval=frame"), "{}", line);
}

#[test]
fn a_muted_clip_is_left_out_of_the_mix() {
    let mut p = with(&[sound("b.wav", 4.0)]);
    let m = p.media_by_uri("file:///tmp/b.wav").unwrap().clone();
    let a = p.add_clip(2, Source::Media(&m), None, false).unwrap();
    p.find_mut(a).unwrap().1.muted = true;
    let line = render::export_command(&p, &render::Target::of(&p, "mp4", "1080p"), &PathBuf::from("/tmp/o.mp4")).unwrap().args.join(" ");
    assert!(!line.contains("[aout]"), "{}", line);
}

#[test]
fn where_the_playhead_is_tells_which_clip_shows_and_how_far_in() {
    let mut p = with(&[vid("a.mp4", 10.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    let a = p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.trim(a, Edge::Start, 2.0, 0.0, None).unwrap();
    let seen = render::visible_at(&p, 4.0);
    assert_eq!(seen, vec![(1, a)]);
    assert_eq!(render::media_time(p.clip(a).unwrap(), 4.0), 4.0);
    assert!(render::visible_at(&p, 0.5).is_empty());
}

#[test]
fn taking_a_file_out_of_the_bin_takes_its_clips_too() {
    let mut p = with(&[vid("a.mp4", 4.0)]);
    let m = p.media_by_uri("file:///tmp/a.mp4").unwrap().clone();
    p.add_clip(1, Source::Media(&m), None, false).unwrap();
    p.remove_media("file:///tmp/a.mp4");
    assert!(p.track(1).is_empty());
    assert!(p.data.media.is_empty());
}

#[test]
fn the_presets_are_the_five_we_offer() {
    assert_eq!(EXPORT_PRESETS.len(), 5);
    assert_eq!(TRANSITIONS.len(), 9);
    assert_eq!(TRACKS.len(), 4);
    assert_eq!(track_name("music"), "Music");
    assert_eq!(marker_rgb("red").0, 0.85);
}

#[test]
fn the_sound_only_command_writes_plain_samples() {
    let mut p = with(&[sound("b.wav", 4.0)]);
    let m = p.media_by_uri("file:///tmp/b.wav").unwrap().clone();
    p.add_clip(2, Source::Media(&m), None, false).unwrap();
    let line = render::audio_command(&p, &PathBuf::from("/tmp/mix.raw")).unwrap().args.join(" ");
    assert!(line.contains("-f s16le"), "{}", line);
    assert!(line.contains("-ar 48000 -ac 2"), "{}", line);
    assert!(!line.contains("libx264"), "{}", line);
}

#[test]
fn a_silent_timeline_has_no_sound_to_play() {
    let mut p = with(&[still("s.png")]);
    let m = p.media_by_uri("file:///tmp/s.png").unwrap().clone();
    p.add_clip(1, Source::Media(&m), None, false).unwrap();
    assert!(render::audio_command(&p, &PathBuf::from("/tmp/mix.raw")).unwrap_err().0.contains("no sound"));
}

#[test]
fn speed_beyond_what_ffmpeg_stretches_is_done_in_steps() {
    assert!(render::atempo_chain(1.0).is_empty());
    assert_eq!(render::atempo_chain(2.0), vec![2.0]);
    assert_eq!(render::atempo_chain(4.0), vec![2.0, 2.0]);
    assert_eq!(render::atempo_chain(0.25), vec![0.5, 0.5]);
    let three = render::atempo_chain(3.0);
    let product: f64 = three.iter().product();
    assert!((product - 3.0).abs() < 1e-9, "{three:?}");
    assert!(three.iter().all(|v| (0.5..=2.0).contains(v)), "{three:?}");
}

#[test]
fn a_title_is_drawn_by_us_and_handed_over_as_a_picture() {
    let mut p = Project::new();
    let id = p.add_clip(0, Source::Text(Kind::Title, "Our Holiday".into()), Some(0.0), false).unwrap();
    let dir = std::env::temp_dir().join("folio-reel-engine-test");
    let mut text = folio_kit::text::Text::bundled();
    let drawn = folio_reel_engine::titles::write_all(&p, 320, 180, &dir, &mut text);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].0, id);
    assert!(drawn[0].1.is_file());
    let pm = folio_reel_engine::titles::draw(p.clip(id).unwrap(), 320, 180, &mut text);
    assert_eq!((pm.width(), pm.height()), (320, 180));
    // the words left some ink on the picture
    let lit = pm.pixels().iter().filter(|px| px.alpha() > 0).count();
    assert!(lit > 100, "{lit} pixels");
    let t = render::Target { width: 320, height: 180, fps: 25.0, format: "mp4".into() };
    let line = render::export_command_with_titles(&p, &t, &PathBuf::from("/tmp/o.mp4"), &drawn).unwrap().args.join(" ");
    assert!(line.contains("title-"), "{line}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_title_with_no_picture_drawn_is_left_out_rather_than_drawn_blank() {
    let mut p = Project::new();
    p.add_clip(0, Source::Text(Kind::Title, "Hello".into()), Some(0.0), false).unwrap();
    let t = render::Target::of(&p, "mp4", "720p");
    let line = render::export_command(&p, &t, &PathBuf::from("/tmp/o.mp4")).unwrap().args.join(" ");
    assert!(!line.contains("title-"), "{line}");
}
