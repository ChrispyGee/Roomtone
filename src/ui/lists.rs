//! Sorting, jumping and searching in long lists: the parts that don't draw anything.

use crate::model::Item;

/// How lists can be sorted (the number is what's kept in `App::list_sort`; 0 is the list's
/// own order, which for the library pages is already alphabetical).
pub const SORT_DEFAULT: usize = 0;
pub const SORT_TITLE: usize = 1;
pub const SORT_ARTIST: usize = 2;
pub const SORT_LONGEST: usize = 3;
pub const SORT_TITLE_Z: usize = 4;
pub const SORT_ALBUM: usize = 5;
pub const SORT_SHORTEST: usize = 6;

/// Lists with at least this many rows get the A–Z strip.
pub const JUMP_MIN: usize = 40;

pub fn sort_items(items: &mut [Item], how: usize) {
    let lower = |s: &str| s.to_lowercase();
    match how {
        SORT_TITLE => items.sort_by_key(|a| lower(&a.title)),
        SORT_ARTIST => items.sort_by_key(|a| (lower(&a.artist), lower(&a.title))),
        SORT_LONGEST => items.sort_by(|a, b| b.duration.partial_cmp(&a.duration).unwrap_or(std::cmp::Ordering::Equal)),
        SORT_TITLE_Z => items.sort_by_key(|a| std::cmp::Reverse(lower(&a.title))),
        SORT_ALBUM => items.sort_by_key(|a| (lower(&a.album), a.track_no, lower(&a.title))),
        SORT_SHORTEST => items.sort_by(|a, b| {
            // Songs with no known length go last, not first.
            let len = |i: &Item| if i.duration > 0.0 { i.duration } else { f32::MAX };
            len(a).partial_cmp(&len(b)).unwrap_or(std::cmp::Ordering::Equal)
        }),
        _ => {}
    }
}

/// The letter a name files under: its first letter or digit, upper-cased; `#` for numbers and
/// anything else.
pub fn initial(s: &str) -> char {
    match s.trim_start_matches(|c: char| !c.is_alphanumeric()).chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().next().unwrap_or('#'),
        _ => '#',
    }
}

/// What to file each item under for this sort (see `effective_sort` for the list's own order),
/// or `None` if the order isn't alphabetical (longest first, say) so a jump strip would mislead.
pub fn jump_key(item: &Item, sort: usize) -> Option<String> {
    match sort {
        SORT_TITLE | SORT_TITLE_Z => Some(item.title.clone()),
        SORT_ARTIST => Some(item.artist.clone()),
        SORT_ALBUM => Some(item.album.clone()),
        _ => None,
    }
}

/// The sort to jump by: the chosen one, or, for a list's own order, whichever alphabetical
/// order it happens to be in (by title, or by artist), if any.
pub fn effective_sort(items: &[Item], chosen: usize) -> usize {
    if chosen != SORT_DEFAULT {
        return chosen;
    }
    if is_sorted_by(items, |i| &i.title) {
        SORT_TITLE
    } else if items.iter().filter(|i| !i.artist.is_empty()).count() * 2 > items.len() && is_sorted_by(items, |i| &i.artist) {
        SORT_ARTIST
    } else {
        SORT_DEFAULT
    }
}

/// The first position of each letter, in the order they appear in the list.
pub fn jump_targets(keys: &[String]) -> Vec<(char, usize)> {
    let mut out: Vec<(char, usize)> = vec![];
    for (i, k) in keys.iter().enumerate() {
        let c = initial(k);
        if !out.iter().any(|(l, _)| *l == c) {
            out.push((c, i));
        }
    }
    out
}

fn is_sorted_by(items: &[Item], field: fn(&Item) -> &String) -> bool {
    items.windows(2).all(|w| field(&w[0]).to_lowercase() <= field(&w[1]).to_lowercase())
}

/// Does a queue entry match what the person typed in the queue's search box?
pub fn queue_matches(item: &Item, needle: &str) -> bool {
    let n = needle.trim().to_lowercase();
    n.is_empty() || item.title.to_lowercase().contains(&n) || item.artist.to_lowercase().contains(&n) || item.album.to_lowercase().contains(&n)
}

#[cfg(test)]
mod unit {
    use super::*;

    fn song(title: &str, artist: &str, album: &str, secs: f32) -> Item {
        Item { title: title.into(), artist: artist.into(), album: album.into(), duration: secs, ..Default::default() }
    }

    fn titles(v: &[Item]) -> Vec<&str> {
        v.iter().map(|i| i.title.as_str()).collect()
    }

    #[test]
    fn sorting_in_every_way() {
        let base = vec![song("b side", "Zed", "Low", 200.0), song("A song", "Amy", "Zoo", 0.0), song("c", "Amy", "Apple", 90.0)];
        let sorted = |how| {
            let mut v = base.clone();
            sort_items(&mut v, how);
            titles(&v).iter().map(|s| s.to_string()).collect::<Vec<_>>()
        };
        assert_eq!(sorted(SORT_DEFAULT), ["b side", "A song", "c"], "its own order");
        assert_eq!(sorted(SORT_TITLE), ["A song", "b side", "c"], "case doesn't matter");
        assert_eq!(sorted(SORT_TITLE_Z), ["c", "b side", "A song"]);
        assert_eq!(sorted(SORT_ARTIST), ["A song", "c", "b side"], "then by title within an artist");
        assert_eq!(sorted(SORT_ALBUM), ["c", "b side", "A song"]);
        assert_eq!(sorted(SORT_LONGEST), ["b side", "c", "A song"]);
        assert_eq!(sorted(SORT_SHORTEST), ["c", "b side", "A song"], "no length known goes last");
    }

    #[test]
    fn letters() {
        assert_eq!(initial("apple"), 'A');
        assert_eq!(initial("  “Quoted” title"), 'Q', "leading punctuation is skipped");
        assert_eq!(initial("99 Problems"), '#');
        assert_eq!(initial(""), '#');
        assert_eq!(initial("élan"), 'É');
        assert_eq!(initial("Ωmega"), 'Ω');
    }

    #[test]
    fn the_jump_strip_points_at_the_first_of_each_letter() {
        let keys: Vec<String> = ["1999", "Abba", "Air", "Bee", "Bob", "Zed"].iter().map(|s| s.to_string()).collect();
        assert_eq!(jump_targets(&keys), [('#', 0), ('A', 1), ('B', 3), ('Z', 5)]);
        assert!(jump_targets(&[]).is_empty());
    }

    #[test]
    fn only_alphabetical_orders_get_a_strip() {
        let i = song("Title", "Artist", "Album", 1.0);
        assert_eq!(jump_key(&i, SORT_TITLE).as_deref(), Some("Title"));
        assert_eq!(jump_key(&i, SORT_TITLE_Z).as_deref(), Some("Title"));
        assert_eq!(jump_key(&i, SORT_ARTIST).as_deref(), Some("Artist"));
        assert_eq!(jump_key(&i, SORT_ALBUM).as_deref(), Some("Album"));
        assert_eq!(jump_key(&i, SORT_LONGEST), None);
        assert_eq!(jump_key(&i, SORT_SHORTEST), None);
        assert_eq!(jump_key(&i, SORT_DEFAULT), None, "a list's own order is looked at first, with effective_sort");
    }

    #[test]
    fn a_lists_own_order_is_used_when_it_is_alphabetical() {
        let by_title = vec![song("Abba", "Z", "", 0.0), song("bee", "Y", "", 0.0), song("Cat", "X", "", 0.0)];
        let by_artist = vec![song("Zed", "Abba", "", 0.0), song("Yak", "Bee", "", 0.0), song("Xu", "Cat", "", 0.0)];
        let neither = vec![song("Zed", "Cat", "", 0.0), song("Abba", "Abba", "", 0.0), song("Yak", "Bee", "", 0.0)];
        assert_eq!(effective_sort(&by_title, SORT_DEFAULT), SORT_TITLE);
        assert_eq!(effective_sort(&by_artist, SORT_DEFAULT), SORT_ARTIST);
        assert_eq!(effective_sort(&neither, SORT_DEFAULT), SORT_DEFAULT, "the speakers' own order isn't alphabetical");
        assert_eq!(effective_sort(&neither, SORT_ALBUM), SORT_ALBUM, "a chosen sort wins");
        let no_artists = vec![song("Zed", "", "", 0.0), song("Abba", "", "", 0.0)];
        assert_eq!(effective_sort(&no_artists, SORT_DEFAULT), SORT_DEFAULT, "artists that are mostly blank say nothing");
    }

    #[test]
    fn searching_the_queue() {
        let i = song("Morning Fog", "Marlow Quartet", "Low Tide", 1.0);
        assert!(queue_matches(&i, ""));
        assert!(queue_matches(&i, "  "));
        assert!(queue_matches(&i, "fog") && queue_matches(&i, "QUARTET") && queue_matches(&i, "low t"));
        assert!(!queue_matches(&i, "sunrise"));
    }
}
