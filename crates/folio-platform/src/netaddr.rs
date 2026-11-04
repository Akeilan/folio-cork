//! Address helpers shared by the web backend and the browser.

/// Is this address this computer or the local network, where HTTPS-only doesn't apply?
pub fn is_local_host(host: &str) -> bool {
    let h = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") || h == "::1" || h.starts_with("127.") || h.starts_with("10.") || h.starts_with("192.168.") || {
        let p: Vec<&str> = h.split('.').collect();
        p.len() == 4 && p[0] == "172" && p[1].parse::<u8>().map(|n| (16..32).contains(&n)).unwrap_or(false)
    }
}

/// The host part of an http(s) address.
pub fn host_of(uri: &str) -> Option<String> {
    let rest = uri.split_once("://")?.1;
    let auth = rest.split(['/', '?', '#']).next()?;
    let auth = auth.rsplit('@').next()?;
    let host = if auth.starts_with('[') { auth.split(']').next().map(|h| format!("{h}]"))? } else { auth.split(':').next()?.to_string() };
    Some(host)
}

