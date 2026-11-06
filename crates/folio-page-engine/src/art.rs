//! Page borders (w:pgBorders): lines, and art borders, the small pictures
//! repeated around the page. The original's art is its own; Folio draws its
//! own pictures of the same subjects (apples, hearts, stars, trees...), in
//! the same sizes and places, so pages keep their look and layout.

use crate::xml::{NodeId, XmlDoc};

#[derive(Clone, Debug, PartialEq)]
pub struct PageBorder {
    /// a line style ("single", "double"...) or an art name ("apples"...)
    pub val: String,
    /// points (lines: eighths in the file; art: whole points)
    pub width: f64,
    /// distance (points) from the text or the page edge
    pub space: f64,
    pub color: Option<u32>,
    pub shadow: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageBorders {
    /// measured from the page edge (else from the text)
    pub from_page: bool,
    /// "allPages", "firstPage", "notFirstPage"
    pub display: String,
    /// drawn in front of the text
    pub front: bool,
    /// top, left, bottom, right
    pub sides: [Option<PageBorder>; 4],
}

const LINE_STYLES: &[&str] = &["single", "thick", "double", "dotted", "dashed", "dotDash", "dotDotDash", "triple",
    "thinThickSmallGap", "thickThinSmallGap", "thinThickThinSmallGap", "thinThickMediumGap", "thickThinMediumGap",
    "thinThickThinMediumGap", "thinThickLargeGap", "thickThinLargeGap", "thinThickThinLargeGap", "wave",
    "doubleWave", "dashSmallGap", "dashDotStroked", "threeDEmboss", "threeDEngrave", "outset", "inset"];

pub fn is_line(val: &str) -> bool {
    LINE_STYLES.contains(&val)
}

pub fn parse(dom: &XmlDoc, n: NodeId) -> Option<PageBorders> {
    let mut b = PageBorders {
        from_page: dom.attr(n, "w:offsetFrom") == Some("page"),
        display: dom.attr(n, "w:display").unwrap_or("allPages").to_string(),
        front: dom.attr(n, "w:zOrder") != Some("back"),
        sides: [None, None, None, None],
    };
    for c in dom.elements(n) {
        let i = match dom.name(c) {
            "w:top" => 0,
            "w:left" | "w:start" => 1,
            "w:bottom" => 2,
            "w:right" | "w:end" => 3,
            _ => continue,
        };
        let val = dom.attr(c, "w:val").unwrap_or("none").to_string();
        if val == "none" || val == "nil" {
            continue;
        }
        let sz = dom.attr(c, "w:sz").and_then(|v| v.parse::<f64>().ok()).unwrap_or(4.0);
        let width = if is_line(&val) { sz / 8.0 } else { sz.clamp(1.0, 31.0) };
        b.sides[i] = Some(PageBorder {
            width,
            space: dom.attr(c, "w:space").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0),
            color: dom.attr(c, "w:color").filter(|v| *v != "auto").and_then(|v| u32::from_str_radix(v, 16).ok()),
            shadow: matches!(dom.attr(c, "w:shadow"), Some("1" | "true" | "on")),
            val,
        });
    }
    if b.sides.iter().all(|s| s.is_none()) {
        None
    } else {
        Some(b)
    }
}


/// One picture of an art border in its cell (x, y, w, h).
#[allow(clippy::too_many_arguments)]
pub const ART_NAMES: &[&str] = &["apples", "balloons3Colors", "balloonsHotAir", "basicBlackDots", "basicBlackSquares",
    "basicWideOutline", "birthdayCake", "cakeSlice", "checkedBarBlack", "christmasTree", "confettiStreamers",
    "diamondsGray", "doubleDiamonds", "flowersDaisies", "flowersRoses", "gifts", "heartBalloon", "heartGray", "hearts",
    "mapleLeaf", "packages", "pumpkin1", "snowflakes", "starsBlack", "stars", "sun", "triangles", "weavingBraid",
    "waveline", "zigZag"];
