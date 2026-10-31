//! Small per-OS services: printing and opening files. Each OS gets its own branch here.

use std::path::Path;
use std::process::Command;

#[cfg(unix)]
pub fn print_pdf(path: &Path, copies: u32, printer: Option<&str>) -> Result<(), String> {
    // CUPS is the print system on Linux and macOS.
    let mut c = Command::new("lp");
    if let Some(p) = printer {
        c.arg("-d").arg(p);
    }
    c.arg("-n").arg(copies.max(1).to_string()).arg(path);
    match c.output() {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(_) => Err("Printing isn't available: the system's print service (CUPS) isn't installed.".into()),
    }
}

#[cfg(not(unix))]
pub fn print_pdf(_path: &Path, _copies: u32, _printer: Option<&str>) -> Result<(), String> {
    Err("Printing isn't available on this system yet. Export to PDF and print that instead.".into())
}

#[cfg(unix)]
pub fn printers() -> Vec<String> {
    let mut out = Vec::new();
    let default = Command::new("lpstat").arg("-d").output().ok().and_then(|o| {
        let s = String::from_utf8_lossy(&o.stdout).to_string();
        s.split(':').nth(1).map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
    });
    if let Some(d) = &default {
        out.push(d.clone());
    }
    if let Ok(o) = Command::new("lpstat").arg("-a").output() {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some(name) = line.split_whitespace().next() {
                if !out.iter().any(|x| x == name) {
                    out.push(name.to_string());
                }
            }
        }
    }
    out
}

#[cfg(not(unix))]
pub fn printers() -> Vec<String> {
    Vec::new()
}

pub fn open_externally(path: &Path) -> Result<(), String> {
    let r = if cfg!(target_os = "macos") {
        Command::new("open").arg(path).spawn()
    } else if cfg!(windows) {
        Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn()
    } else {
        Command::new("xdg-open").arg(path).spawn()
    };
    r.map(|_| ()).map_err(|e| e.to_string())
}
