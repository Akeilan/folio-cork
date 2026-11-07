//! System services other than the window (X3-13): files, sockets, threads, the user's name,
//! random numbers, memory use and the office converter. Apps call these instead of the OS.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Write a file so that a failure never damages the old one: write beside it, flush to disk,
/// then rename over it.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.saving"));
    let res = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    res.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Can't save {}: {e}", path.display())
    })
}

/// Folders holding the fonts that ship with Folio (X3-16): `fonts` beside the executable,
/// `../share/folio/fonts` from it (the installed layout), and `FOLIO_FONTS` when set.
pub fn font_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = setting("FOLIO_FONTS") {
        v.push(PathBuf::from(d));
    }
    if let Ok(exe) = std::env::current_exe() {
        v.push(exe.with_file_name("fonts"));
        if let Some(p) = exe.parent().and_then(|p| p.parent()) {
            v.push(p.join("share/folio/fonts"));
        }
    }
    v.retain(|d| d.is_dir());
    v
}

/// Where an app's own files are kept: a folder of that name beside the
/// executable, `../share/<app>` from it (the installed layout), and the
/// system's own share folder. Only the ones that exist.
pub fn data_dirs(app: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        v.push(exe.with_file_name(app));
        if let Some(p) = exe.parent().and_then(|p| p.parent()) {
            v.push(p.join("share").join(app));
        }
    }
    v.push(PathBuf::from("/usr/share").join(app));
    v.push(PathBuf::from("/usr/local/share").join(app));
    v.retain(|d| d.is_dir());
    v.dedup();
    v
}

/// A folder for this process's temporary files.
/// Where the program was started. Used to make a bare file name absolute.
pub fn cwd() -> Option<PathBuf> {
    std::env::current_dir().ok()
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

/// A temporary file name unique to this process.
pub fn temp_file(stem: &str, ext: &str) -> PathBuf {
    temp_dir().join(format!("{stem}-{}.{ext}", std::process::id()))
}

/// The signed-in user's name, for comments and authorship.
pub fn user_name() -> String {
    for v in ["FOLIO_USER", "USER", "USERNAME", "LOGNAME"] {
        if let Ok(s) = std::env::var(v) {
            if !s.trim().is_empty() {
                return s;
            }
        }
    }
    "Me".into()
}

/// An environment setting, for the few things Folio reads that way (tests, overrides).
pub fn setting(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

/// Set an environment setting for this process and the tools it starts.
pub fn set_setting(name: &str, value: &Path) {
    std::env::set_var(name, value);
}

/// Bytes from the system's secure random source.
pub fn random_bytes(buf: &mut [u8]) -> Result<(), String> {
    getrandom::fill(buf).map_err(|e| format!("No random source: {e}"))
}

pub fn random_u64() -> u64 {
    let mut b = [0u8; 8];
    if random_bytes(&mut b).is_err() {
        // Never used for keys: callers that need secrecy check random_bytes themselves.
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) as u64;
        return t ^ (std::process::id() as u64).rotate_left(32);
    }
    u64::from_le_bytes(b)
}

/// Memory this process is using, in bytes (resident set), when the system says.
pub fn memory_used() -> Option<u64> {
    if cfg!(target_os = "linux") {
        let s = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = s.lines().find(|l| l.starts_with("VmRSS:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb * 1024)
    } else {
        None
    }
}

/// Peak memory of this process in bytes, when the system says.
pub fn memory_peak() -> Option<u64> {
    if cfg!(target_os = "linux") {
        let s = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb * 1024)
    } else {
        None
    }
}

/// Start a named background thread.
pub fn spawn<F: FnOnce() + Send + 'static>(name: &str, f: F) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new().name(name.into()).spawn(f).expect("thread")
}

pub fn sleep(d: Duration) {
    std::thread::sleep(d)
}

/// Convert a document with the system's office suite (LibreOffice), when installed. Returns
/// the converted file.
pub fn convert_with_office(path: &Path, to: &str) -> Result<PathBuf, String> {
    use std::process::Command;
    let out = temp_dir().join(format!("folio-convert-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&out);
    let cmd = ["soffice", "libreoffice"]
        .iter()
        .find(|c| Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false))
        .copied()
        .ok_or_else(|| "LibreOffice isn't installed.".to_string())?;
    let r = Command::new(cmd).args(["--headless", "--convert-to", to, "--outdir"]).arg(&out).arg(path).output().map_err(|e| e.to_string())?;
    let file = out.join(path.with_extension(to).file_name().unwrap_or_default());
    if !r.status.success() || !file.exists() {
        return Err("LibreOffice couldn't convert the file.".into());
    }
    Ok(file)
}

/// Open a file or a folder in whatever the desktop uses for it (xdg-open on Linux).
pub fn open_path(path: &Path) -> Result<(), String> {
    use std::process::Command;
    if !path.exists() {
        return Err("That file isn't there.".into());
    }
    Command::new("xdg-open").arg(path).spawn().map(|_| ()).map_err(|_| "Nothing on this computer is set up to open it.".to_string())
}

/// A new message in the desktop's mail program with `path` attached.
pub fn mail_file(path: &Path, subject: &str) -> Result<(), String> {
    use std::process::Command;
    if !path.exists() {
        return Err("That file isn't there.".into());
    }
    let r = Command::new("xdg-email").args(["--subject", subject, "--attach"]).arg(path).spawn();
    if r.is_ok() {
        return Ok(());
    }
    // no xdg-email: open an empty message and leave the file for the person to attach
    Command::new("xdg-open")
        .arg(format!("mailto:?subject={}", subject.replace(' ', "%20")))
        .spawn()
        .map(|_| ())
        .map_err(|_| "No mail program is set up on this computer.".to_string())
}

/// Make a picture the desktop's background. Linux: GNOME and the desktops built on it (Zorin,
/// Cinnamon, MATE) through their own settings; others say so rather than guessing.
pub fn set_wallpaper(path: &Path) -> Result<(), String> {
    use std::process::Command;
    let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let uri = format!("file://{}", path.display());
    let schemes: [(&str, &[&str]); 3] = [
        ("org.gnome.desktop.background", &["picture-uri", "picture-uri-dark"]),
        ("org.cinnamon.desktop.background", &["picture-uri"]),
        ("org.mate.background", &["picture-filename"]),
    ];
    for (scheme, keys) in schemes {
        let known = Command::new("gsettings").args(["writable", scheme, keys[0]]).output();
        let Ok(o) = known else { continue };
        if !o.status.success() || !String::from_utf8_lossy(&o.stdout).trim().eq("true") {
            continue;
        }
        for key in keys {
            let value = if *key == "picture-filename" { path.display().to_string() } else { uri.clone() };
            let r = Command::new("gsettings").args([&"set", &scheme, key, value.as_str()]).status();
            if r.map(|st| !st.success()).unwrap_or(true) {
                return Err("The desktop wouldn't take the picture as its background.".into());
            }
        }
        return Ok(());
    }
    Err("This desktop doesn't offer a way to set the background from a program.".to_string())
}

/// Sockets: plain TCP and the address other computers on the network reach this one by.
pub mod net {
    pub use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
    use std::time::Duration;

    /// Listen on every network interface on a free port.
    pub fn listen_any() -> std::io::Result<TcpListener> {
        TcpListener::bind("0.0.0.0:0")
    }

    /// Listen on this computer only (tests).
    pub fn listen_local() -> std::io::Result<TcpListener> {
        TcpListener::bind("127.0.0.1:0")
    }

    pub fn connect(addr: &SocketAddr, timeout: Duration) -> std::io::Result<TcpStream> {
        TcpStream::connect_timeout(addr, timeout)
    }

    /// This computer's address on the local network (no packet is sent).
    pub fn local_ip() -> String {
        std::net::UdpSocket::bind("0.0.0.0:0")
            .and_then(|s| {
                s.connect("10.254.254.254:9")?;
                s.local_addr()
            })
            .map(|a| a.ip().to_string())
            .unwrap_or_else(|_| "127.0.0.1".into())
    }
}

/// Open a web or mail address in whatever the desktop uses for it.
pub fn open_address(url: &str) -> Result<(), String> {
    use std::process::Command;
    let url = url.trim();
    if url.is_empty() {
        return Err("No address.".into());
    }
    let r = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else if cfg!(windows) {
        Command::new("cmd").args(["/C", "start", ""]).arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
    r.map(|_| ()).map_err(|_| "No web browser is set up on this computer.".to_string())
}
