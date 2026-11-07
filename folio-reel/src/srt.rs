//! Subtitles as people have them: SRT and WebVTT blocks.

/// Each block as (start, end, words). Anything that isn't a block is skipped.
pub fn parse(text: &str) -> Vec<(f64, f64, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() || line == "WEBVTT" {
            continue;
        }
        // a number on its own is an SRT counter; the times are on the next line
        let times = if line.contains("-->") {
            line.to_string()
        } else if lines.peek().map(|n| n.contains("-->")).unwrap_or(false) {
            lines.next().unwrap().trim().to_string()
        } else {
            continue;
        };
        let Some((a, b)) = times.split_once("-->") else { continue };
        let (Some(start), Some(end)) = (clock(a.trim()), clock(b.trim().split_whitespace().next().unwrap_or(""))) else { continue };
        let mut words: Vec<String> = Vec::new();
        while let Some(n) = lines.peek() {
            if n.trim().is_empty() {
                break;
            }
            words.push(lines.next().unwrap().trim().to_string());
        }
        if end > start && !words.is_empty() {
            out.push((start, end, words.join("\n")));
        }
    }
    out
}

/// `00:01:02,500` or `1:02.5`, in seconds.
fn clock(s: &str) -> Option<f64> {
    let s = s.replace(',', ".");
    let mut total = 0.0;
    let parts: Vec<&str> = s.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    for p in &parts {
        let v: f64 = p.trim().parse().ok()?;
        total = total * 60.0 + v;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    #[test]
    fn srt_blocks_come_through() {
        let text = "1\n00:00:01,000 --> 00:00:03,500\nHello there\n\n2\n00:00:04,000 --> 00:00:05,000\nSecond\nline\n";
        let v = super::parse(text);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], (1.0, 3.5, "Hello there".to_string()));
        assert_eq!(v[1].2, "Second\nline");
    }

    #[test]
    fn vtt_blocks_come_through_too() {
        let v = super::parse("WEBVTT\n\n00:01.000 --> 00:02.000 line:90%\nQuick\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].0, 1.0);
    }

    #[test]
    fn rubbish_gives_nothing() {
        assert!(super::parse("just some prose\nwith no times").is_empty());
    }
}
