//! The go-between for when two computers cannot see each other (K3-09).
//!
//! Direct sharing needs the person joining to be able to reach the person hosting. On one network
//! that is true; across two it usually is not, and neither end can do anything about the other's
//! router. So Folio Cork can also be run as a go-between on a computer both ends *can* reach, and
//! the two meet there:
//!
//! ```text
//! folio-cork --relay 7600
//! ```
//!
//! What it carries, it cannot read. The board's words are already sealed end to end with a key
//! made from the board code (see collab.rs); the go-between never sees the code and is never
//! given the key. All it is told is a room: the SHA-256 of the code, which it cannot turn back
//! into the code. It pairs the people in a room and copies bytes between them.
//!
//! The person hosting opens one connection and says "I am hosting room R". Each person joining
//! opens a connection and says "I want room R". The go-between then tells the host, over that
//! first connection, that someone has arrived; the host opens one more connection for that
//! person, and from there the two streams are simply joined. That is the same shape as the
//! listener the host uses on a local network, so the board itself does not know the difference.

use folio_kit::platform::net::{self, TcpListener, TcpStream};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the two ends and the go-between agree on. Bumped if the shape below ever changes.
pub const VERSION: u8 = 1;
pub const ROLE_HOST: u8 = b'H';
pub const ROLE_GUEST: u8 = b'G';
pub const ROLE_BACK: u8 = b'B';     // the host coming back for one person who arrived

/// What the go-between says back. A direct host says none of these, which is how the two are
/// told apart without asking the person which kind of address they were given.
pub const HERE: u8 = b'Y';            // you have reached a go-between and the board is here
pub const NO_SUCH_BOARD: u8 = b'N';   // a go-between, but nobody is sharing that board
const ARRIVED: u8 = 1;                // to the host: someone is waiting for you

/// The room a board code names, which cannot be turned back into the code.
pub fn room_of(code: &str) -> [u8; 32] {
    let norm: String = code.chars().filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase()).collect();
    let mut h = Sha256::new();
    h.update(b"folio-cork-room\0");
    h.update(norm.as_bytes());
    h.finalize().into()
}

/// Say who you are on a fresh connection to the go-between.
pub fn greet(s: &mut TcpStream, role: u8, room: &[u8; 32]) -> std::io::Result<()> {
    let mut out = [0u8; 34];
    out[0] = VERSION;
    out[1] = role;
    out[2..].copy_from_slice(room);
    s.write_all(&out)?;
    s.flush()
}

fn hear(s: &mut TcpStream) -> std::io::Result<(u8, [u8; 32])> {
    let mut buf = [0u8; 34];
    s.read_exact(&mut buf)?;
    if buf[0] != VERSION {
        return Err(std::io::Error::other("a different version of Folio Cork"));
    }
    let mut room = [0u8; 32];
    room.copy_from_slice(&buf[2..]);
    Ok((buf[1], room))
}

#[derive(Default)]
struct Rooms {
    /// The host's first connection for each room: where arrivals are announced.
    hosts: HashMap<[u8; 32], TcpStream>,
    /// People who have arrived and are waiting for the host to come back for them.
    waiting: HashMap<[u8; 32], Vec<TcpStream>>,
}

/// Run as the go-between on this port until stopped. Returns only on a listening failure.
pub fn serve(port: u16, on_event: &dyn Fn(&str)) -> std::io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port))?;
    on_event(&format!("Folio Cork is the go-between on port {port}. It carries boards it cannot read."));
    serve_on(listener, on_event)
}

/// The same, on a listener already open (what the tests use).
pub fn serve_on(listener: TcpListener, on_event: &dyn Fn(&str)) -> std::io::Result<()> {
    let rooms: Arc<Mutex<Rooms>> = Arc::new(Mutex::new(Rooms::default()));
    for s in listener.incoming().flatten() {
        let rooms = rooms.clone();
        let mut s = s;
        let _ = s.set_nodelay(true);
        let said = |m: &str| on_event(m);
        let Ok((role, room)) = hear(&mut s) else {
            said("someone said hello in a way this version does not know");
            continue;
        };
        match role {
            ROLE_HOST => {
                rooms.lock().unwrap().hosts.insert(room, s);
                said("a board is being shared");
            }
            ROLE_GUEST => {
                // tell the host someone is here, and keep them until the host comes back
                let mut r = rooms.lock().unwrap();
                let live = match r.hosts.get_mut(&room) {
                    None => false,
                    Some(h) => {
                        let ok = h.write_all(&[ARRIVED]).and_then(|_| h.flush()).is_ok();
                        if !ok {
                            r.hosts.remove(&room);
                        }
                        ok
                    }
                };
                // Answer either way. That one byte is what tells the person joining they have
                // reached a go-between at all, rather than something else listening on that port.
                let _ = s.write_all(&[if live { HERE } else { NO_SUCH_BOARD }]);
                let _ = s.flush();
                if !live {
                    said("someone asked for a board nobody is sharing here");
                    continue;
                }
                r.waiting.entry(room).or_default().push(s);
                said("someone has joined a board");
            }
            ROLE_BACK => {
                let other = rooms.lock().unwrap().waiting.get_mut(&room).and_then(|v| {
                    if v.is_empty() { None } else { Some(v.remove(0)) }
                });
                match other {
                    Some(g) => pipe(s, g),
                    None => said("a host came back for someone who had already gone"),
                }
            }
            _ => said("someone said hello in a way this version does not know"),
        }
    }
    Ok(())
}

/// Copy bytes both ways until either end stops.
fn pipe(a: TcpStream, b: TcpStream) {
    for (r, w) in [(a.try_clone(), b.try_clone()), (b.try_clone(), a.try_clone())] {
        let (Ok(mut r), Ok(mut w)) = (r, w) else { continue };
        folio_kit::platform::spawn("folio-cork-relay", move || {
            let mut buf = [0u8; 16 * 1024];
            loop {
                match r.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if w.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = r.shutdown(net::Shutdown::Both);
            let _ = w.shutdown(net::Shutdown::Both);
        });
    }
}

/// Open the host's first connection to the go-between: the one arrivals are announced on.
pub fn host_at(addr: &str, code: &str) -> Result<(TcpStream, net::SocketAddr), String> {
    let sock = resolve(addr)?;
    let mut s = net::connect(&sock, Duration::from_secs(6))
        .map_err(|e| format!("Can't reach the go-between at {addr}: {e}"))?;
    let _ = s.set_nodelay(true);
    greet(&mut s, ROLE_HOST, &room_of(code)).map_err(|e| e.to_string())?;
    Ok((s, sock))
}

/// The host coming back for one person who has arrived.
pub fn back_for_one(sock: &net::SocketAddr, code: &str) -> Result<TcpStream, String> {
    let mut s = net::connect(sock, Duration::from_secs(6)).map_err(|e| e.to_string())?;
    let _ = s.set_nodelay(true);
    greet(&mut s, ROLE_BACK, &room_of(code)).map_err(|e| e.to_string())?;
    Ok(s)
}

/// Someone joining a board through the go-between. Err(NOT_A_GO_BETWEEN) when whatever is
/// listening there did not answer as one.
pub fn guest_at(addr: &str, code: &str) -> Result<TcpStream, String> {
    let sock = resolve(addr)?;
    let mut s = net::connect(&sock, Duration::from_secs(6))
        .map_err(|e| format!("Can't reach {addr}: {e}"))?;
    let _ = s.set_nodelay(true);
    greet(&mut s, ROLE_GUEST, &room_of(code)).map_err(|e| e.to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut answer = [0u8; 1];
    match s.read_exact(&mut answer) {
        Ok(()) if answer[0] == HERE => {
            let _ = s.set_read_timeout(None);
            Ok(s)
        }
        Ok(()) if answer[0] == NO_SUCH_BOARD => Err(
            "That go-between has nobody sharing a board with that code. Check the code, and that \
             the person sharing it is still sharing.".into()),
        _ => Err(NOT_A_GO_BETWEEN.into()),
    }
}

/// What guest_at says when the thing at that address is not a go-between.
pub const NOT_A_GO_BETWEEN: &str = "not a go-between";

fn resolve(addr: &str) -> Result<net::SocketAddr, String> {
    let a = addr.trim();
    if let Ok(s) = a.parse::<net::SocketAddr>() {
        return Ok(s);
    }
    net::first_address(a).ok_or_else(|| {
        format!("“{a}” isn't an address the go-between can be reached at. It should look like \
                 boards.example.com:7600 or 203.0.113.4:7600.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_hides_the_code_it_came_from() {
        let a = room_of("ABCD-EFGH-JKMN");
        assert_eq!(a, room_of("abcd efgh jkmn"), "spacing and case do not matter");
        assert_ne!(a, room_of("ABCD-EFGH-JKMP"), "a different code is a different room");
        assert_eq!(a.len(), 32);
    }
}
