//! Working together (K3-09): several people on one board, with live cursors, connected
//! directly over the local network. No cloud and no accounts: the host shares an address and a
//! board code; everything sent is encrypted with a key made from that code.

use crate::editor::{Drag, Editor};
use crate::model::{Document, Id};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::ChaCha20Poly1305;
use folio_kit::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t")]
enum Msg {
    Hello { name: String },
    Doc { doc: Document, rev: u64, base: u64, from: String },
    Cursor { name: String, page: Id, x: f32, y: f32 },
    Bye { name: String },
}

struct Peer {
    stream: TcpStream,
    name: String,
}

pub struct Collab {
    host: bool,
    addr: String,
    code: String,
    name: String,
    key: [u8; 32],
    me: u32,
    counter: Arc<Mutex<u64>>,
    peers: Arc<Mutex<Vec<Peer>>>,
    rx: Receiver<(usize, Msg)>,
    rev: u64,
    sent_version: u64,
    last_change: Option<Instant>,
    cursors: HashMap<String, (Id, (f32, f32), Instant)>,
    last_cursor: Instant,
    pending: Option<(Document, u64)>,
    error: String,
}

fn derive_key(code: &str) -> [u8; 32] {
    let mut key = [0u8; 32];
    let norm: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(norm.as_bytes(), b"folio-cork-board", 60_000, &mut key);
    key
}

fn seed() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) as u64;
    let mut x = t ^ (std::process::id() as u64).rotate_left(32) ^ (&t as *const u64 as u64);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x
}

fn make_code() -> String {
    // 12 characters from an alphabet without look-alikes: about 60 bits.
    const A: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let mut x = seed();
    let mut y = seed().rotate_left(17) ^ 0x9E3779B97F4A7C15;
    let mut s = String::new();
    for i in 0..12 {
        if i > 0 && i % 4 == 0 {
            s.push('-');
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        y = y.wrapping_add(x);
        s.push(A[(y % A.len() as u64) as usize] as char);
    }
    s
}

fn local_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("10.254.254.254:9")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

fn send_frame(stream: &mut TcpStream, key: &[u8; 32], me: u32, counter: &Mutex<u64>, msg: &Msg) -> std::io::Result<()> {
    let plain = serde_json::to_vec(msg).map_err(|e| std::io::Error::other(e.to_string()))?;
    let n = {
        let mut c = counter.lock().unwrap();
        *c += 1;
        *c
    };
    let mut nonce = [0u8; 12];
    nonce[..4].copy_from_slice(&me.to_le_bytes());
    nonce[4..].copy_from_slice(&n.to_le_bytes());
    let cipher = ChaCha20Poly1305::new(key.into());
    let ct = cipher.encrypt(&nonce.into(), plain.as_slice()).map_err(|_| std::io::Error::other("encrypt"))?;
    let len = (ct.len() + 12) as u32;
    let mut buf = Vec::with_capacity(len as usize + 4);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&nonce);
    buf.extend_from_slice(&ct);
    stream.write_all(&buf)
}

fn read_frame(stream: &mut TcpStream, key: &[u8; 32]) -> Option<Msg> {
    let mut lb = [0u8; 4];
    stream.read_exact(&mut lb).ok()?;
    let len = u32::from_le_bytes(lb) as usize;
    if !(12..=256 * 1024 * 1024).contains(&len) {
        return None;
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).ok()?;
    let nonce: [u8; 12] = buf[..12].try_into().ok()?;
    let cipher = ChaCha20Poly1305::new(key.into());
    let plain = cipher.decrypt(&nonce.into(), &buf[12..]).ok()?;
    serde_json::from_slice(&plain).ok()
}

fn reader(mut stream: TcpStream, key: [u8; 32], idx: usize, tx: Sender<(usize, Msg)>) {
    std::thread::spawn(move || {
        while let Some(m) = read_frame(&mut stream, &key) {
            if tx.send((idx, m)).is_err() {
                break;
            }
        }
        let _ = tx.send((idx, Msg::Bye { name: String::new() }));
    });
}

impl Collab {
    /// Share the open board. Others join with the address and code this shows.
    pub fn host(ed: &Editor, name: &str) -> Result<Collab, String> {
        let listener = TcpListener::bind("0.0.0.0:0").map_err(|e| format!("Can't open a network port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let code = make_code();
        let key = derive_key(&code);
        let (tx, rx) = channel();
        let peers: Arc<Mutex<Vec<Peer>>> = Arc::new(Mutex::new(vec![]));
        let counter = Arc::new(Mutex::new(0u64));
        let me = seed() as u32;
        {
            let peers = peers.clone();
            let counter = counter.clone();
            let doc = ed.doc.clone();
            let tx = tx.clone();
            std::thread::spawn(move || {
                let first_doc = Arc::new(Mutex::new(doc));
                let _ = &first_doc;
                for s in listener.incoming().flatten() {
                    let mut s = s;
                    let _ = s.set_nodelay(true);
                    // The first frame must decrypt with the board code.
                    let Some(Msg::Hello { name }) = read_frame(&mut s, &key) else { continue };
                    let idx = {
                        let mut p = peers.lock().unwrap();
                        p.push(Peer { stream: s.try_clone().unwrap(), name: name.clone() });
                        p.len() - 1
                    };
                    let _ = &counter;
                    reader(s, key, idx, tx.clone());
                    let _ = tx.send((idx, Msg::Hello { name }));
                }
            });
        }
        let addr = format!("{}:{port}", local_ip());
        Ok(Collab {
            host: true,
            addr,
            code,
            name: name.into(),
            key,
            me,
            counter,
            peers,
            rx,
            rev: 1,
            sent_version: ed.version,
            last_change: None,
            cursors: HashMap::new(),
            last_cursor: Instant::now(),
            pending: None,
            error: String::new(),
        })
    }

    pub fn join(addr: &str, code: &str, name: &str) -> Result<Collab, String> {
        let addr = addr.trim();
        if addr.is_empty() {
            return Err("Type the address shown on the computer sharing the board.".into());
        }
        let key = derive_key(code);
        let sock: std::net::SocketAddr = addr.parse().map_err(|_| "The address should look like 192.168.1.20:40123.".to_string())?;
        let mut s = TcpStream::connect_timeout(&sock, Duration::from_secs(4)).map_err(|e| format!("Can't reach {addr}: {e}"))?;
        let _ = s.set_nodelay(true);
        let counter = Arc::new(Mutex::new(0u64));
        let me = seed() as u32;
        send_frame(&mut s, &key, me, &counter, &Msg::Hello { name: name.into() }).map_err(|e| e.to_string())?;
        let (tx, rx) = channel();
        reader(s.try_clone().map_err(|e| e.to_string())?, key, 0, tx);
        let peers = Arc::new(Mutex::new(vec![Peer { stream: s, name: "host".into() }]));
        Ok(Collab {
            host: false,
            addr: addr.into(),
            code: code.into(),
            name: name.into(),
            key,
            me,
            counter,
            peers,
            rx,
            rev: 0,
            sent_version: 0,
            last_change: None,
            cursors: HashMap::new(),
            last_cursor: Instant::now(),
            pending: None,
            error: String::new(),
        })
    }

    fn broadcast(&self, msg: &Msg, except: Option<usize>) {
        let mut p = self.peers.lock().unwrap();
        for (i, peer) in p.iter_mut().enumerate() {
            if Some(i) == except {
                continue;
            }
            let _ = send_frame(&mut peer.stream, &self.key, self.me, &self.counter, msg);
        }
    }

    pub fn info(&self) -> String {
        if self.host {
            format!("Others on your network can join this board.\n\nIn Folio Cork, Board > Join Board, and type:\n\nAddress:  {}\nCode:  {}\n\nEverything sent is encrypted with the code. Sharing stops when you close Cork or press Stop Sharing.", self.addr, self.code)
        } else {
            format!("Connected to {}.", self.addr)
        }
    }

    pub fn status(&self) -> String {
        let n = self.peers.lock().map(|p| p.len()).unwrap_or(0);
        if !self.error.is_empty() {
            return self.error.clone();
        }
        if self.host {
            format!("Sharing · {n} joined")
        } else {
            "Working together".into()
        }
    }

    pub fn cursor(&mut self, page: Id, p: (f32, f32)) {
        if self.last_cursor.elapsed() < Duration::from_millis(50) {
            return;
        }
        self.last_cursor = Instant::now();
        self.broadcast(&Msg::Cursor { name: self.name.clone(), page, x: p.0, y: p.1 }, None);
    }

    pub fn cursors(&self) -> Vec<(String, Id, (f32, f32), Color)> {
        let palette = [0x1E88E5, 0xD81B60, 0x43A047, 0xF4511E, 0x8E24AA, 0x00897B];
        self.cursors
            .iter()
            .filter(|(_, v)| v.2.elapsed() < Duration::from_secs(20))
            .map(|(n, v)| {
                let h = n.bytes().fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
                (n.clone(), v.0, v.1, Color::hex(palette[h % palette.len()]))
            })
            .collect()
    }

    /// Exchange changes. Called on every tick.
    pub fn poll(&mut self, ed: &mut Editor) {
        let busy = !matches!(ed.drag, Drag::None) || ed.edit.is_some();
        while let Ok((idx, m)) = self.rx.try_recv() {
            match m {
                Msg::Hello { name } => {
                    if self.host {
                        let msg = Msg::Doc { doc: ed.doc.clone(), rev: self.rev, base: self.rev, from: self.name.clone() };
                        if let Some(p) = self.peers.lock().unwrap().get_mut(idx) {
                            let _ = send_frame(&mut p.stream, &self.key, self.me, &self.counter, &msg);
                        }
                        ed.status = format!("{name} joined.");
                    }
                }
                Msg::Doc { doc, rev, base, from } => {
                    if self.host {
                        // Accept edits made on the latest board; otherwise send the latest back.
                        if base == self.rev {
                            self.rev += 1;
                            self.apply(ed, doc.clone(), busy);
                            self.broadcast(&Msg::Doc { doc, rev: self.rev, base: self.rev, from }, None);
                        } else if let Some(p) = self.peers.lock().unwrap().get_mut(idx) {
                            let _ = send_frame(&mut p.stream, &self.key, self.me, &self.counter, &Msg::Doc { doc: ed.doc.clone(), rev: self.rev, base: self.rev, from: self.name.clone() });
                        }
                    } else if from != self.name || rev > self.rev {
                        self.rev = rev;
                        self.apply(ed, doc, busy);
                    } else {
                        self.rev = rev;
                    }
                }
                Msg::Cursor { name, page, x, y } => {
                    if name != self.name {
                        self.cursors.insert(name.clone(), (page, (x, y), Instant::now()));
                    }
                    if self.host {
                        self.broadcast(&Msg::Cursor { name, page, x, y }, Some(idx));
                    }
                }
                Msg::Bye { .. } => {
                    if !self.host {
                        self.error = "The shared board closed.".into();
                    } else if let Ok(mut p) = self.peers.lock() {
                        if idx < p.len() {
                            let n = p[idx].name.clone();
                            self.cursors.remove(&n);
                        }
                    }
                }
            }
        }
        if !busy {
            if let Some((doc, ver)) = self.pending.take() {
                let _ = ver;
                self.apply(ed, doc, false);
            }
        }
        // Send our changes once editing settles a moment.
        if ed.version != self.sent_version && !busy {
            let now = Instant::now();
            match self.last_change {
                None => self.last_change = Some(now),
                Some(t) if now.duration_since(t) > Duration::from_millis(150) => {
                    self.last_change = None;
                    self.sent_version = ed.version;
                    let msg = Msg::Doc { doc: ed.doc.clone(), rev: self.rev + 1, base: self.rev, from: self.name.clone() };
                    if self.host {
                        self.rev += 1;
                        self.broadcast(&Msg::Doc { doc: ed.doc.clone(), rev: self.rev, base: self.rev, from: self.name.clone() }, None);
                    } else {
                        self.broadcast(&msg, None);
                    }
                }
                _ => {}
            }
        }
    }

    fn apply(&mut self, ed: &mut Editor, doc: Document, busy: bool) {
        if busy {
            self.pending = Some((doc, 0));
            return;
        }
        let page_id = ed.pg().id;
        ed.doc = doc;
        ed.doc.fix_ids();
        ed.page = ed.doc.pages.iter().position(|p| p.id == page_id).unwrap_or(0).min(ed.doc.pages.len().saturating_sub(1));
        let page = ed.pg().clone();
        ed.sel.retain(|id| page.get(*id).is_some());
        ed.version += 1;
        self.sent_version = ed.version;
        ed.dirty = true;
    }
}

impl Drop for Collab {
    fn drop(&mut self) {
        self.broadcast(&Msg::Bye { name: self.name.clone() }, None);
        if let Ok(p) = self.peers.lock() {
            for peer in p.iter() {
                let _ = peer.stream.shutdown(std::net::Shutdown::Both);
            }
        }
    }
}
