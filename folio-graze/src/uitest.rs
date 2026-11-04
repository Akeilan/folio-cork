//! `folio-graze --ui-test DIR`: the browser driven from inside, against pages served by a
//! small web server on this computer (no internet needed).

use crate::app::GrazeApp;
use folio_platform::{net, Event, Host, Key, Mods};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Default, Debug)]
pub struct ServerLog {
    /// (host header, path, had a cookie)
    pub hits: Vec<(String, String, bool)>,
}

fn serve(log: Arc<Mutex<ServerLog>>) -> u16 {
    let l = net::listen_local().expect("test server");
    let port = l.local_addr().unwrap().port();
    folio_platform::spawn("graze-test-server", move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut buf = vec![0u8; 8192];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();
            let host = req.lines().find_map(|l| l.strip_prefix("Host: ").or_else(|| l.strip_prefix("host: "))).unwrap_or("").trim().to_string();
            let cookie = req.lines().any(|l| l.to_ascii_lowercase().starts_with("cookie:"));
            log.lock().unwrap().hits.push((host.clone(), path.clone(), cookie));
            let other = host.replace("localhost", "127.0.0.1");
            let (body, extra) = match path.as_str() {
                "/p1" => ("<title>Page one</title><a id=a href=/p2>two</a> <a id=b href=/p2 target=_blank>blank</a><p>banana and another banana</p>".to_string(), String::new()),
                "/p2" => ("<title>Page two</title><body style='background:#e8f4ff'><p>two</p></body>".to_string(), String::new()),
                "/tp" => (format!("<title>Third party</title><iframe src='http://{other}/setc'></iframe>"), String::new()),
                "/setc" => ("<p>tracker</p>".to_string(), "Set-Cookie: t=1; SameSite=None; Path=/\r\n".to_string()),
                "/fp" => ("<title>First party</title>".to_string(), "Set-Cookie: f=1; Path=/\r\n".to_string()),
                "/fpcheck" => ("<title>Checked</title>".to_string(), String::new()),
                _ => ("<title>Not found</title>".to_string(), String::new()),
            };
            let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}", body.len());
            let _ = s.write_all(resp.as_bytes());
        }
    });
    port
}

pub enum St {
    Done,
    Wait,
    Fail(String),
}

type Step = Box<dyn FnMut(&mut GrazeApp, &mut dyn Host, &mut Ctx) -> St>;

pub struct Ctx {
    pub dir: PathBuf,
    pub port: u16,
    pub log: Arc<Mutex<ServerLog>>,
    pub calls: u32,
    pub mark: usize,
}

pub struct UiTest {
    steps: Vec<(String, Step)>,
    i: usize,
    ctx: Ctx,
    pub done: bool,
    pub fails: Vec<String>,
    dir: PathBuf,
}

fn key(app: &mut GrazeApp, h: &mut dyn Host, k: Key, mods: Mods) {
    use folio_platform::App;
    app.event(&Event::Key { key: k.clone(), mods, down: true, repeat: false }, h);
    app.event(&Event::Key { key: k, mods, down: false, repeat: false }, h);
}

fn typ(app: &mut GrazeApp, h: &mut dyn Host, s: &str) {
    use folio_platform::App;
    app.event(&Event::Text(s.to_string()), h);
}

const CTRL: Mods = Mods { ctrl: true, shift: false, alt: false, logo: false };
const CTRL_SHIFT: Mods = Mods { ctrl: true, shift: true, alt: false, logo: false };
const ALT: Mods = Mods { ctrl: false, shift: false, alt: true, logo: false };

fn title(app: &GrazeApp) -> String {
    app.tab().map(|t| t.title.clone()).unwrap_or_default()
}

impl UiTest {
    pub fn new(dir: PathBuf) -> UiTest {
        let log = Arc::new(Mutex::new(ServerLog::default()));
        let port = serve(log.clone());
        let mut steps: Vec<(String, Step)> = vec![];
        macro_rules! step {
            ($name:expr, $f:expr) => {
                steps.push(($name.to_string(), Box::new($f)));
            };
        }
        step!("the window opens with one tab and the address bar ready", |a: &mut GrazeApp, _h: &mut dyn Host, _c: &mut Ctx| {
            if a.tabs.len() == 1 && a.address.focused { St::Done } else { St::Fail(format!("{} tabs, address focused {}", a.tabs.len(), a.address.focused)) }
        });
        step!("typing an address and Enter loads the page", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                typ(a, h, &format!("localhost:{}/p1", c.port));
                key(a, h, Key::Enter, Mods::NONE);
            }
            if title(a) == "Page one" { St::Done } else { St::Wait }
        });
        step!("following a link", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                let v = a.tab().unwrap().view;
                h.web(v, folio_platform::WebCommand::Script("document.getElementById('a').click(); 1".into()));
            }
            if title(a) == "Page two" && a.tab().unwrap().back { St::Done } else { St::Wait }
        });
        step!("the page is drawn", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                a.last_script = None;
                let v = a.tab().unwrap().view;
                h.web(v, folio_platform::WebCommand::Snapshot(c.dir.join("graze-page.png")));
            }
            match a.last_script.as_deref() {
                Some("snapshot") => {
                    let Ok(b) = std::fs::read(c.dir.join("graze-page.png")) else { return St::Fail("no picture".into()) };
                    let Some(pm) = folio_kit::gfx::decode_image(&b) else { return St::Fail("unreadable picture".into()) };
                    // Page two's background is #E8F4FF.
                    match pm.pixel(pm.width() - 20, pm.height() - 20) {
                        Some(p) if p.red() == 0xE8 && p.green() == 0xF4 && p.blue() == 0xFF => St::Done,
                        p => St::Fail(format!("page colour {:?}", p.map(|p| (p.red(), p.green(), p.blue())))),
                    }
                }
                Some(other) => St::Fail(other.to_string()),
                None => St::Wait,
            }
        });
        step!("Alt+Left goes back", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                key(a, h, Key::Left, ALT);
            }
            if title(a) == "Page one" { St::Done } else { St::Wait }
        });
        step!("Alt+Right goes forward", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                key(a, h, Key::Right, ALT);
            }
            if title(a) == "Page two" { St::Done } else { St::Wait }
        });
        step!("a target=_blank link opens a tab in the background", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                key(a, h, Key::Left, ALT);
            }
            if c.calls == 10 {
                let v = a.tab().unwrap().view;
                h.web(v, folio_platform::WebCommand::Script("document.getElementById('b').click(); 1".into()));
            }
            if a.tabs.len() == 2 && a.active == 0 && a.tabs[1].title == "Page two" { St::Done } else { St::Wait }
        });
        step!("Ctrl+Tab, Ctrl+W and Ctrl+Shift+T", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                key(a, h, Key::Tab, CTRL);
                if a.active != 1 {
                    return St::Fail("Ctrl+Tab didn't move to the next tab".into());
                }
                key(a, h, Key::Char('w'), CTRL);
                if a.tabs.len() != 1 {
                    return St::Fail("Ctrl+W didn't close the tab".into());
                }
                key(a, h, Key::Char('t'), CTRL_SHIFT);
            }
            if a.tabs.len() == 2 && title(a) == "Page two" { St::Done } else { St::Wait }
        });
        step!("Ctrl+T, then words in the address bar search", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                key(a, h, Key::Char('t'), CTRL);
                if a.tabs.len() != 3 || !a.address.focused {
                    return St::Fail("Ctrl+T didn't open a tab with the address bar ready".into());
                }
                typ(a, h, "folio graze");
                key(a, h, Key::Enter, Mods::NONE);
            }
            let u = a.tab().unwrap().uri.clone();
            if u.starts_with(crate::nav::SEARCH) && u.ends_with("folio+graze") { St::Done } else { St::Fail(format!("searched for {u}")) }
        });
        step!("dragging a tab moves it", |a: &mut GrazeApp, h: &mut dyn Host, _c: &mut Ctx| {
            use folio_platform::App;
            let first = a.tabs[0].view;
            let s = a.s;
            let y = 18.0 * s;
            let x0 = 42.0 * s + 40.0 * s;
            a.event(&Event::MouseMove { x: x0, y }, h);
            a.event(&Event::MouseDown { x: x0, y, button: folio_platform::Button::Left, clicks: 1 }, h);
            let w = a.w;
            let tw = ((w - 3.0 * 46.0 * s - 44.0 * s - 42.0 * s) / 3.0).clamp(80.0 * s, 230.0 * s);
            a.event(&Event::MouseMove { x: x0 + tw * 1.5, y }, h);
            a.event(&Event::MouseUp { x: x0 + tw * 1.5, y, button: folio_platform::Button::Left }, h);
            if a.tabs[1].view == first && a.active == 1 { St::Done } else { St::Fail("the tab didn't move".into()) }
        });
        step!("find in page counts matches", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                let i = a.tabs.iter().position(|t| t.title == "Page one").unwrap_or(0);
                a.select(i, h);
                key(a, h, Key::Char('f'), CTRL);
                typ(a, h, "banana");
            }
            if a.tab().and_then(|t| t.found) == Some(2) { St::Done } else { St::Wait }
        });
        step!("Escape closes the find bar", |a: &mut GrazeApp, h: &mut dyn Host, _c: &mut Ctx| {
            key(a, h, Key::Escape, Mods::NONE);
            if a.find.is_none() { St::Done } else { St::Fail("still open".into()) }
        });
        step!("zoom in and back to actual size", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                key(a, h, Key::Char('='), CTRL);
                if (a.tab().unwrap().zoom - 1.1).abs() > 0.001 {
                    return St::Fail(format!("zoom {}", a.tab().unwrap().zoom));
                }
                key(a, h, Key::Char('0'), CTRL);
            }
            if (a.tab().unwrap().zoom - 1.0).abs() < 0.001 { St::Done } else { St::Wait }
        });
        step!("the reload button reloads", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            use folio_platform::App;
            let count = |c: &Ctx| c.log.lock().unwrap().hits.iter().filter(|x| x.1 == "/p1").count();
            if c.calls == 0 {
                c.mark = count(c);
                let s = a.s;
                let (x, y) = (8.0 * s + 2.0 * 34.0 * s + 16.0 * s, 36.0 * s + 21.0 * s);
                a.event(&Event::MouseMove { x, y }, h);
                a.event(&Event::MouseDown { x, y, button: folio_platform::Button::Left, clicks: 1 }, h);
                a.event(&Event::MouseUp { x, y, button: folio_platform::Button::Left }, h);
            }
            if count(c) > c.mark { St::Done } else { St::Wait }
        });
        step!("first-party cookies are kept", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                a.go(&format!("localhost:{}/fp", c.port), h);
            }
            if c.calls == 15 {
                a.go(&format!("localhost:{}/fpcheck", c.port), h);
            }
            let l = c.log.lock().unwrap();
            match l.hits.iter().find(|x| x.1 == "/fpcheck") {
                Some(x) if x.2 => St::Done,
                Some(_) => St::Fail("the site's own cookie wasn't sent back".into()),
                None => St::Wait,
            }
        });
        step!("third-party cookies are blocked", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                a.go(&format!("localhost:{}/tp", c.port), h);
            }
            if c.calls == 20 {
                a.go(&format!("localhost:{}/tp?again", c.port), h);
            }
            let l = c.log.lock().unwrap();
            let tracker: Vec<bool> = l.hits.iter().filter(|x| x.1 == "/setc").map(|x| x.2).collect();
            if tracker.len() >= 2 {
                if tracker.iter().any(|&c| c) { St::Fail("the third-party cookie came back".into()) } else { St::Done }
            } else {
                St::Wait
            }
        });
        step!("HTTPS-only: a site without encryption shows why instead", |a: &mut GrazeApp, h: &mut dyn Host, c: &mut Ctx| {
            if c.calls == 0 {
                a.go("http://folio-graze.invalid/", h);
            }
            let t = a.tab().unwrap();
            if t.insecure && t.title == "Not secure" { St::Done } else { St::Wait }
        });
        step!("Ctrl+Shift+N opens a private window", |a: &mut GrazeApp, h: &mut dyn Host, _c: &mut Ctx| {
            key(a, h, Key::Char('n'), CTRL_SHIFT);
            if a.spawned_private == 1 { St::Done } else { St::Fail("no private window".into()) }
        });
        step!("lime title bar, page visible under the chrome", |a: &mut GrazeApp, _h: &mut dyn Host, _c: &mut Ctx| {
            let Some(pm) = a.frame.as_ref() else { return St::Wait };
            let s = a.s;
            let px = pm.pixel((a.w / 2.0) as u32 + 200, (6.0 * s) as u32);
            let below = pm.pixel(10, (a.chrome_h() + 20.0 * s) as u32);
            match (px, below) {
                (Some(p), Some(b)) if p.green() > 150 && p.red() < 170 && p.blue() < 120 && b.alpha() == 0 => St::Done,
                (p, b) => St::Fail(format!("title {:?} below {:?}", p.map(|p| (p.red(), p.green(), p.blue())), b.map(|b| b.alpha()))),
            }
        });
        UiTest { steps, i: 0, ctx: Ctx { dir: dir.clone(), port, log, calls: 0, mark: 0 }, done: false, fails: vec![], dir }
    }

    pub fn step(&mut self, app: &mut GrazeApp, host: &mut dyn Host) {
        if self.i >= self.steps.len() {
            if let Some(pm) = app.frame.as_ref() {
                let _ = std::fs::write(self.dir.join("graze-chrome.png"), folio_kit::gfx::encode_png(pm));
            }
            println!("{} passed, {} failed", self.steps.len() - self.fails.len(), self.fails.len());
            for f in &self.fails {
                println!("FAIL {f}");
            }
            self.done = true;
            return;
        }
        let (name, f) = &mut self.steps[self.i];
        let name = name.clone();
        let r = f(app, host, &mut self.ctx);
        self.ctx.calls += 1;
        let r = match r {
            St::Wait if self.ctx.calls > 150 => St::Fail(format!("timed out (title {:?}, uri {:?})", app.tab().map(|t| t.title.clone()), app.tab().map(|t| t.uri.clone()))),
            r => r,
        };
        match r {
            St::Wait => return,
            St::Done => println!("ok   {name}"),
            St::Fail(e) => {
                println!("FAIL {name}: {e}");
                self.fails.push(format!("{name}: {e}"));
            }
        }
        self.i += 1;
        self.ctx.calls = 0;
    }

    pub fn code(&self) -> i32 {
        if self.fails.is_empty() { 0 } else { 1 }
    }
}
