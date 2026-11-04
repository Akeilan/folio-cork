//! Graze without a window: address handling, tabs and shortcuts on the headless host.

use crate::app::GrazeApp;
use crate::nav;
use folio_platform::{App, Event, Headless, Key, Mods, WebCommand, WebEvent};

fn app() -> (GrazeApp, Headless) {
    let mut h = Headless::default();
    let mut a = GrazeApp::new(false, vec![]);
    a.testing = true;
    a.event(&Event::Resized { w: 1280, h: 800, scale: 1.0 }, &mut h);
    (a, h)
}

fn key(a: &mut GrazeApp, h: &mut Headless, k: Key, ctrl: bool, shift: bool) {
    let m = Mods { ctrl, shift, ..Mods::NONE };
    a.event(&Event::Key { key: k, mods: m, down: true, repeat: false }, h);
}

#[test]
fn addresses_and_searches() {
    assert_eq!(nav::resolve("example.org"), "https://example.org");
    assert_eq!(nav::resolve("example.org/a b"), format!("{}example.org%2Fa+b", nav::SEARCH));
    assert_eq!(nav::resolve("localhost:8080/x"), "http://localhost:8080/x");
    assert_eq!(nav::resolve("192.168.1.20"), "http://192.168.1.20");
    assert_eq!(nav::resolve("http://plain.example/"), "http://plain.example/");
    assert_eq!(nav::resolve("what is rust"), format!("{}what+is+rust", nav::SEARCH));
    assert_eq!(nav::resolve("c++ & c#"), format!("{}c%2B%2B+%26+c%23", nav::SEARCH));
    assert!(!nav::looks_like_address("readme.1"));
    assert!(nav::looks_like_address("about:blank"));
    assert!(folio_platform::is_local_host("127.0.0.1") && folio_platform::is_local_host("printer.local") && folio_platform::is_local_host("172.20.1.1"));
    assert!(!folio_platform::is_local_host("172.40.1.1") && !folio_platform::is_local_host("example.org"));
    assert_eq!(folio_platform::host_of("https://user@Example.org:8443/a?b").as_deref(), Some("Example.org"));
}

#[test]
fn starts_with_an_empty_tab_and_the_address_bar_ready() {
    let (a, h) = app();
    assert_eq!(a.tabs.len(), 1);
    assert!(a.address.focused && h.chrome_focus);
    assert_eq!(h.web_log, vec!["new 1", "show 1"]);
    assert!(h.chrome.0 >= 70, "chrome height {}", h.chrome.0);
}

#[test]
fn typing_an_address_loads_it_in_the_tab() {
    let (mut a, mut h) = app();
    a.event(&Event::Text("example.org".into()), &mut h);
    key(&mut a, &mut h, Key::Enter, false, false);
    assert!(h.web_log.contains(&"1 Load(\"https://example.org\")".to_string()), "{:?}", h.web_log);
    assert!(!a.address.focused);
}

#[test]
fn tabs_open_close_reopen_and_cycle() {
    let (mut a, mut h) = app();
    a.go("one.example", &mut h);
    key(&mut a, &mut h, Key::Char('t'), true, false);
    a.go("two.example", &mut h);
    assert_eq!((a.tabs.len(), a.active), (2, 1));
    key(&mut a, &mut h, Key::Tab, true, false);
    assert_eq!(a.active, 0);
    key(&mut a, &mut h, Key::Tab, true, true);
    assert_eq!(a.active, 1);
    key(&mut a, &mut h, Key::Char('w'), true, false);
    assert_eq!((a.tabs.len(), a.active), (1, 0));
    assert_eq!(a.closed, vec!["https://two.example".to_string()]);
    key(&mut a, &mut h, Key::Char('t'), true, true);
    assert_eq!(a.tabs.len(), 2);
    assert_eq!(a.tab().unwrap().uri, "https://two.example");
    // A link asking for a new tab opens it behind the current one.
    let v = a.tabs[0].view;
    a.event(&Event::Web(WebEvent::OpenTab { view: v, uri: "https://three.example/".into() }), &mut h);
    assert_eq!(a.tabs.len(), 3);
    assert_eq!(a.tab().unwrap().uri, "https://two.example");
}

#[test]
fn closing_the_last_tab_closes_the_window() {
    let (mut a, mut h) = app();
    key(&mut a, &mut h, Key::Char('w'), true, false);
    assert!(a.wants_exit() && h.closed);
}

#[test]
fn page_news_updates_the_tab_and_the_bar() {
    let (mut a, mut h) = app();
    a.go("example.org", &mut h);
    let v = a.tab().unwrap().view;
    for ev in [
        WebEvent::Loading { view: v, loading: true },
        WebEvent::Uri { view: v, uri: "https://example.org/".into() },
        WebEvent::Title { view: v, title: "Example".into() },
        WebEvent::History { view: v, back: true, forward: false },
        WebEvent::Progress { view: v, value: 0.5 },
    ] {
        a.event(&Event::Web(ev), &mut h);
    }
    let t = a.tab().unwrap();
    assert_eq!((t.title.as_str(), t.uri.as_str(), t.back, t.loading), ("Example", "https://example.org/", true, true));
    assert_eq!(a.address.text, "https://example.org/");
    assert_eq!(h.title, "Example - Folio Graze");
    // HTTPS-only: the explanation page keeps the plain address in the bar.
    a.event(&Event::Web(WebEvent::Insecure { view: v, uri: "http://plain.example/".into() }), &mut h);
    a.event(&Event::Web(WebEvent::Uri { view: v, uri: "https://plain.example/".into() }), &mut h);
    assert!(a.tab().unwrap().insecure);
    assert_eq!(a.address.text, "http://plain.example/");
}

#[test]
fn shortcuts_are_kept_from_the_page() {
    let (mut a, mut h) = app();
    a.go("example.org", &mut h);
    h.handled = false;
    key(&mut a, &mut h, Key::Char('l'), true, false);
    assert!(h.handled && a.address.focused);
    a.event(&Event::Web(WebEvent::PageFocus { view: 1 }), &mut h);
    assert!(!a.address.focused);
    h.handled = false;
    key(&mut a, &mut h, Key::Char('x'), false, false);
    assert!(!h.handled, "plain keys belong to the page");
    key(&mut a, &mut h, Key::Char('='), true, false);
    assert!(h.web_log.iter().any(|l| l == "1 Zoom(1.1)"));
    key(&mut a, &mut h, Key::Char('f'), true, false);
    a.event(&Event::Text("needle".into()), &mut h);
    assert!(h.web_log.iter().any(|l| l == "1 Find(\"needle\")"), "{:?}", h.web_log);
    key(&mut a, &mut h, Key::Escape, false, false);
    assert!(a.find.is_none() && h.web_log.iter().any(|l| l == &format!("1 {:?}", WebCommand::FindEnd)));
}

#[test]
fn chrome_is_drawn_and_the_page_area_left_clear() {
    let (mut a, mut h) = app();
    let mut px = vec![0u8; 1280 * 800 * 4];
    a.paint(&mut px, 1280, 800, &mut h);
    let at = |x: usize, y: usize| &px[(y * 1280 + x) * 4..(y * 1280 + x) * 4 + 4];
    assert_eq!(at(700, 4)[3], 255);
    assert!(at(700, 4)[1] > 150, "title bar not lime: {:?}", at(700, 4));
    assert_eq!(at(10, 300), &[0, 0, 0, 0]);
}
