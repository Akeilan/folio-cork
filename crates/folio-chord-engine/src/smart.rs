//! Smart Playlists: rules over the library, as iTunes wrote them.
//!
//! A rule set is kept as text, one rule a line, so the library file stays
//! readable: `field op value`, with `match=all|any`, `limit=25 by random` and
//! `live=1` on their own lines.

use crate::library::Track;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Is,
    IsNot,
    Contains,
    DoesNotContain,
    BeginsWith,
    EndsWith,
    GreaterThan,
    LessThan,
    InTheLast,
    NotInTheLast,
}

impl Op {
    pub fn text(self) -> &'static str {
        match self {
            Op::Is => "is",
            Op::IsNot => "is-not",
            Op::Contains => "contains",
            Op::DoesNotContain => "lacks",
            Op::BeginsWith => "begins",
            Op::EndsWith => "ends",
            Op::GreaterThan => "greater",
            Op::LessThan => "less",
            Op::InTheLast => "within",
            Op::NotInTheLast => "not-within",
        }
    }

    pub fn parse(s: &str) -> Op {
        match s {
            "is-not" => Op::IsNot,
            "contains" => Op::Contains,
            "lacks" => Op::DoesNotContain,
            "begins" => Op::BeginsWith,
            "ends" => Op::EndsWith,
            "greater" => Op::GreaterThan,
            "less" => Op::LessThan,
            "within" => Op::InTheLast,
            "not-within" => Op::NotInTheLast,
            _ => Op::Is,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub field: String,
    pub op: Op,
    pub value: String,
}

/// How the songs are picked and how many of them.
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub all: bool,
    pub rules: Vec<Rule>,
    /// 0 = every song that matches
    pub limit: usize,
    /// "random", "played", "added", "rating", "title", "artist", "album", "plays"
    pub limit_by: String,
    pub live: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { all: true, rules: Vec::new(), limit: 0, limit_by: "random".into(), live: true }
    }
}

impl Rules {
    pub fn text(&self) -> String {
        if self.rules.is_empty() && self.limit == 0 {
            return String::new();
        }
        let mut out = vec![format!("match={}", if self.all { "all" } else { "any" })];
        for r in &self.rules {
            out.push(format!("rule={} {} {}", r.field, r.op.text(), r.value));
        }
        if self.limit > 0 {
            out.push(format!("limit={} {}", self.limit, self.limit_by));
        }
        out.push(format!("live={}", if self.live { 1 } else { 0 }));
        out.join("\n")
    }

    pub fn parse(text: &str) -> Rules {
        let mut r = Rules::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            match k {
                "match" => r.all = v != "any",
                "live" => r.live = v == "1",
                "limit" => {
                    let mut it = v.split_whitespace();
                    r.limit = it.next().and_then(|n| n.parse().ok()).unwrap_or(0);
                    r.limit_by = it.next().unwrap_or("random").to_string();
                }
                "rule" => {
                    let mut it = v.splitn(3, ' ');
                    let field = it.next().unwrap_or("").to_string();
                    let op = Op::parse(it.next().unwrap_or("is"));
                    let value = it.next().unwrap_or("").to_string();
                    if !field.is_empty() {
                        r.rules.push(Rule { field, op, value });
                    }
                }
                _ => {}
            }
        }
        r
    }

    /// The songs that match, in the order the limit asks for.
    pub fn pick(&self, tracks: &[Track]) -> Vec<u64> {
        let mut hits: Vec<&Track> = tracks
            .iter()
            .filter(|t| {
                if self.rules.is_empty() {
                    return true;
                }
                let mut any = false;
                for r in &self.rules {
                    let ok = matches(t, r);
                    if self.all && !ok {
                        return false;
                    }
                    any |= ok;
                }
                self.all || any
            })
            .collect();
        match self.limit_by.as_str() {
            "played" => hits.sort_by(|a, b| b.last_played.total_cmp(&a.last_played)),
            "added" => hits.sort_by(|a, b| b.date_added.total_cmp(&a.date_added)),
            "rating" => hits.sort_by(|a, b| b.rating.cmp(&a.rating)),
            "plays" => hits.sort_by(|a, b| b.play_count.cmp(&a.play_count)),
            "title" => hits.sort_by(|a, b| a.tags.title.to_lowercase().cmp(&b.tags.title.to_lowercase())),
            "artist" => hits.sort_by(|a, b| a.artist().to_lowercase().cmp(&b.artist().to_lowercase())),
            "album" => hits.sort_by(|a, b| a.tags.album.to_lowercase().cmp(&b.tags.album.to_lowercase())),
            _ => {}
        }
        let mut ids: Vec<u64> = hits.iter().map(|t| t.id).collect();
        if self.limit_by == "random" && self.limit > 0 {
            // a shuffle that does not need the clock twice for the same library
            let mut seed = folio_kit::platform::random_u64() | 1;
            let n = ids.len();
            for i in (1..n).rev() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let j = (seed >> 33) as usize % (i + 1);
                ids.swap(i, j);
            }
        }
        if self.limit > 0 {
            ids.truncate(self.limit);
        }
        ids
    }
}

fn field_of<'a>(t: &'a Track, name: &str) -> String {
    match name {
        "title" => t.tags.title.clone(),
        "artist" => t.artist().to_string(),
        "albumartist" => t.display_album_artist().to_string(),
        "album" => t.tags.album.clone(),
        "genre" => t.tags.genre.clone(),
        "composer" => t.tags.composer.clone(),
        "grouping" => t.tags.grouping.clone(),
        "comments" => t.tags.comments.clone(),
        "kind" => t.tags.kind.clone(),
        "year" => t.tags.year.to_string(),
        "bpm" => t.tags.bpm.to_string(),
        "bitrate" => t.tags.bitrate.to_string(),
        "plays" => t.play_count.to_string(),
        "skips" => t.skip_count.to_string(),
        "rating" => t.stars().to_string(),
        "duration" => t.tags.duration.round().to_string(),
        "added" => t.date_added.to_string(),
        "lastplayed" => t.last_played.to_string(),
        "checked" => if t.checked { "1".into() } else { "0".into() },
        _ => String::new(),
    }
}

fn matches(t: &Track, r: &Rule) -> bool {
    let got = field_of(t, &r.field);
    let want = r.value.trim();
    let numeric = matches!(r.field.as_str(), "year" | "bpm" | "bitrate" | "plays" | "skips" | "rating" | "duration");
    let dateish = matches!(r.field.as_str(), "added" | "lastplayed");
    match r.op {
        Op::Is => {
            if numeric {
                got.parse::<f64>().unwrap_or(0.0) == want.parse::<f64>().unwrap_or(f64::NAN)
            } else {
                got.eq_ignore_ascii_case(want)
            }
        }
        Op::IsNot => !got.eq_ignore_ascii_case(want),
        Op::Contains => got.to_lowercase().contains(&want.to_lowercase()),
        Op::DoesNotContain => !got.to_lowercase().contains(&want.to_lowercase()),
        Op::BeginsWith => got.to_lowercase().starts_with(&want.to_lowercase()),
        Op::EndsWith => got.to_lowercase().ends_with(&want.to_lowercase()),
        Op::GreaterThan => got.parse::<f64>().unwrap_or(0.0) > want.parse::<f64>().unwrap_or(0.0),
        Op::LessThan => got.parse::<f64>().unwrap_or(0.0) < want.parse::<f64>().unwrap_or(0.0),
        Op::InTheLast | Op::NotInTheLast => {
            let days: f64 = want.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
            let when = got.parse::<f64>().unwrap_or(0.0);
            let within = when > 0.0 && crate::library::now() - when <= days * 86400.0;
            let _ = dateish;
            if r.op == Op::InTheLast {
                within
            } else {
                !within
            }
        }
    }
}

/// The six Smart Playlists a new library starts with, as iTunes had them.
pub fn starters() -> Vec<(String, Rules)> {
    let r = |all: bool, rules: Vec<Rule>, limit: usize, by: &str| Rules { all, rules, limit, limit_by: by.into(), live: true };
    let rule = |f: &str, op: Op, v: &str| Rule { field: f.into(), op, value: v.into() };
    vec![
        ("90's Music".to_string(), r(true, vec![rule("year", Op::GreaterThan, "1989"), rule("year", Op::LessThan, "2000")], 0, "artist")),
        ("My Top Rated".to_string(), r(true, vec![rule("rating", Op::GreaterThan, "3")], 0, "rating")),
        ("Recently Added".to_string(), r(true, vec![rule("added", Op::InTheLast, "14 days")], 25, "added")),
        ("Recently Played".to_string(), r(true, vec![rule("lastplayed", Op::InTheLast, "14 days")], 25, "played")),
        ("Top 25 Most Played".to_string(), r(true, vec![rule("plays", Op::GreaterThan, "0")], 25, "plays")),
        ("Music Videos".to_string(), r(true, vec![rule("kind", Op::Contains, "video")], 0, "title")),
    ]
}
