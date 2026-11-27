//! Searching by year: reading "1985", "1980s", "80s" or "1985-89" and keeping what came out then.
//!
//! Not every source says when something came out (the music on this computer does when its tags
//! do, the Internet Archive and podcast episodes do, many streaming services don't). When a year
//! filter is on, results without a year are left out, and callers say how many.

use crate::model::Item;

const FIRST: u32 = 1900;
const LAST: u32 = 2099;

/// The years to keep, both ends included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct YearRange {
    pub from: u32,
    pub to: u32,
}

fn year(text: &str) -> Option<u32> {
    let n: u32 = text.parse().ok().filter(|_| text.len() == 4)?;
    (FIRST..=LAST).contains(&n).then_some(n)
}

/// "80s" or "'80s": the 1980s. Only 50–99 (the 1900s) and 00–20 (the 2000s) are clear.
fn short_decade(text: &str) -> Option<u32> {
    let digits = text.trim_start_matches(['\'', '’']);
    if digits.len() != 2 {
        return None;
    }
    match digits.parse::<u32>().ok()? {
        n @ 50..=99 if n % 10 == 0 => Some(1900 + n),
        n @ 0..=20 if n % 10 == 0 => Some(2000 + n),
        _ => None,
    }
}

impl YearRange {
    pub fn single(y: u32) -> Self {
        YearRange { from: y, to: y }
    }

    /// Reads what a person types. `None` when it isn't a year, a decade or a range.
    pub fn parse(text: &str) -> Option<YearRange> {
        let t = text.trim().to_lowercase().replace(['–', '—', '‑'], "-");
        if t.is_empty() {
            return None;
        }
        if let Some(y) = year(&t) {
            return Some(Self::single(y));
        }
        // A decade: 1980s, 1980's, 80s, '80s.
        if let Some(d) = t.strip_suffix("'s").or_else(|| t.strip_suffix("’s")).or_else(|| t.strip_suffix('s')) {
            let start = year(d).filter(|y| y % 10 == 0).or_else(|| short_decade(d));
            return start.map(|s| YearRange { from: s, to: s + 9 });
        }
        // A range: 1985-1989, 1985-89, 1985 to 1989.
        let (a, b) = t.split_once('-').or_else(|| t.split_once(" to "))?;
        let (a, b) = (a.trim(), b.trim());
        let from = year(a)?;
        let to = match year(b) {
            Some(y) => y,
            None if b.len() == 2 => {
                let yy: u32 = b.parse().ok()?;
                let mut y = from - from % 100 + yy;
                if y < from {
                    y += 100;
                }
                y
            }
            None => return None,
        };
        if !(FIRST..=LAST).contains(&to) {
            return None;
        }
        Some(YearRange { from: from.min(to), to: from.max(to) })
    }

    pub fn contains(&self, y: u32) -> bool {
        y != 0 && (self.from..=self.to).contains(&y)
    }

    pub fn label(&self) -> String {
        if self.from == self.to {
            self.from.to_string()
        } else if self.from.is_multiple_of(10) && self.to == self.from + 9 {
            format!("{}s", self.from)
        } else {
            format!("{}–{}", self.from, self.to)
        }
    }
}

/// Keep what came out in the range. Also returns how many were left out for having no year.
pub fn keep(items: Vec<Item>, range: Option<YearRange>) -> (Vec<Item>, usize) {
    let Some(r) = range else { return (items, 0) };
    let mut unknown = 0;
    let kept = items
        .into_iter()
        .filter(|i| {
            if i.year == 0 {
                unknown += 1;
            }
            r.contains(i.year)
        })
        .collect();
    (kept, unknown)
}

/// The first four digits of a date such as "1985", "1985-07-13" or "Sat, 13 Jul 1985 10:00:00 GMT".
pub fn year_in(text: &str) -> u32 {
    let b = text.as_bytes();
    (0..b.len().saturating_sub(3))
        .filter(|&i| b[i..i + 4].iter().all(u8::is_ascii_digit) && (i == 0 || !b[i - 1].is_ascii_digit()) && b.get(i + 4).is_none_or(|c| !c.is_ascii_digit()))
        .filter_map(|i| year(&text[i..i + 4]))
        .next()
        .unwrap_or(0)
}

#[cfg(test)]
mod unit {
    use super::*;

    fn r(t: &str) -> Option<(u32, u32)> {
        YearRange::parse(t).map(|r| (r.from, r.to))
    }

    #[test]
    fn reads_years_decades_and_ranges() {
        assert_eq!(r("1985"), Some((1985, 1985)));
        assert_eq!(r(" 1985 "), Some((1985, 1985)));
        assert_eq!(r("1980s"), Some((1980, 1989)));
        assert_eq!(r("1980's"), Some((1980, 1989)));
        assert_eq!(r("80s"), Some((1980, 1989)));
        assert_eq!(r("'90s"), Some((1990, 1999)));
        assert_eq!(r("00s"), Some((2000, 2009)));
        assert_eq!(r("2010s"), Some((2010, 2019)));
        assert_eq!(r("1985-1989"), Some((1985, 1989)));
        assert_eq!(r("1985–89"), Some((1985, 1989)));
        assert_eq!(r("1985 to 1989"), Some((1985, 1989)));
        assert_eq!(r("1998-02"), Some((1998, 2002)));
        assert_eq!(r("1989-1985"), Some((1985, 1989)), "a backwards range is turned round");
    }

    #[test]
    fn leaves_other_text_alone() {
        for t in ["", "beatles", "Blade Runner 2049", "1985 live", "12345", "1850", "2100", "1984s", "35s", "abbey road"] {
            assert_eq!(r(t), None, "{t:?}");
        }
    }

    #[test]
    fn labels_read_back_what_was_asked() {
        assert_eq!(YearRange::parse("80s").unwrap().label(), "1980s");
        assert_eq!(YearRange::parse("1985-89").unwrap().label(), "1985–1989");
        assert_eq!(YearRange::parse("1969").unwrap().label(), "1969");
    }

    #[test]
    fn keeps_the_range_and_counts_the_unknown() {
        let item = |t: &str, y| Item { title: t.into(), year: y, ..Default::default() };
        let items = vec![item("a", 1984), item("b", 1985), item("c", 0), item("d", 1991)];
        let (kept, unknown) = keep(items.clone(), YearRange::parse("1980s"));
        assert_eq!(kept.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(unknown, 1);
        assert_eq!(keep(items, None).0.len(), 4, "no filter keeps everything");
    }

    #[test]
    fn finds_the_year_in_a_date() {
        assert_eq!(year_in("1985"), 1985);
        assert_eq!(year_in("1985-07-13"), 1985);
        assert_eq!(year_in("Sat, 13 Jul 1985 10:00:00 GMT"), 1985);
        assert_eq!(year_in("2019-12-01T00:00:00Z"), 2019);
        assert_eq!(year_in("0000-00-00"), 0);
        assert_eq!(year_in("track 12345"), 0);
        assert_eq!(year_in(""), 0);
    }
}
