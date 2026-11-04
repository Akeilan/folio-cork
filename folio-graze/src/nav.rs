//! What the address bar does with what you type (B3-01): go to an address, or search.

/// The search engine the address bar uses (B3-06 makes it a choice in Options).
pub const SEARCH: &str = "https://duckduckgo.com/?q=";

fn encode(q: &str) -> String {
    let mut out = String::new();
    for b in q.trim().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Does this look like an address rather than words to search for?
pub fn looks_like_address(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.contains(char::is_whitespace) {
        return false;
    }
    if s.contains("://") || s.starts_with("about:") || s.starts_with("file:") {
        return true;
    }
    let host = s.split(['/', '?', '#']).next().unwrap_or("");
    let name = host.rsplit_once(':').map(|(h, p)| if p.chars().all(|c| c.is_ascii_digit()) { h } else { host }).unwrap_or(host);
    name == "localhost" || name.parse::<std::net::Ipv4Addr>().is_ok() || (name.contains('.') && !name.starts_with('.') && !name.ends_with('.') && name.rsplit('.').next().map(|tld| tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic())).unwrap_or(false))
}

/// The page to load for what was typed. Addresses without a scheme get https:// (plain http
/// for this computer's own addresses, which have no certificates).
pub fn resolve(input: &str) -> String {
    let s = input.trim();
    if !looks_like_address(s) {
        return format!("{SEARCH}{}", encode(s));
    }
    if s.contains("://") || s.starts_with("about:") || s.starts_with("file:") {
        return s.to_string();
    }
    let host = s.split(['/', '?', '#', ':']).next().unwrap_or("");
    if folio_platform::is_local_host(host) {
        format!("http://{s}")
    } else {
        format!("https://{s}")
    }
}

/// The address as shown in the bar: the new-tab page shows nothing.
pub fn shown(uri: &str) -> String {
    if uri == "about:blank" { String::new() } else { uri.to_string() }
}
