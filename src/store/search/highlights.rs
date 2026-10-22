//! FTS marker transport stays separate from literal display text.
use std::ops::Range;

pub(super) fn markers(title: &str, path: &str, body: Option<&str>) -> (String, String) {
    loop {
        let id = uuid::Uuid::new_v4();
        let open = format!("\u{1e}bn-{id}-open\u{1f}");
        let close = format!("\u{1e}bn-{id}-close\u{1f}");
        if [Some(title), Some(path), body]
            .into_iter()
            .flatten()
            .all(|text| !text.contains(&open) && !text.contains(&close))
        {
            return (open, close);
        }
    }
}
pub(super) fn decode(marked: &str, open: &str, close: &str) -> Option<(String, Vec<Range<usize>>)> {
    if open.is_empty() || close.is_empty() || open.contains(close) || close.contains(open) {
        return None;
    }
    let mut plain = String::with_capacity(marked.len());
    let mut ranges = Vec::new();
    let mut rest = marked;
    loop {
        let Some(at) = rest.find(open) else {
            if rest.contains(close) {
                return None;
            }
            plain.push_str(rest);
            return Some((plain, ranges));
        };
        if rest[..at].contains(close) {
            return None;
        }
        plain.push_str(&rest[..at]);
        rest = &rest[at + open.len()..];
        let end = rest.find(close)?;
        if end == 0 || rest[..end].contains(open) {
            return None;
        }
        let start = plain.len();
        plain.push_str(&rest[..end]);
        ranges.push(start..plain.len());
        rest = &rest[end + close.len()..];
    }
}
pub(super) fn visible_excerpt(
    text: String,
    matches: Vec<Range<usize>>,
) -> (String, Vec<Range<usize>>) {
    let Some(first) = matches.first() else {
        return (text, matches);
    };
    if first.start == 0 {
        return (text, matches);
    }
    let start = first.start;
    let Some(tail) = text.get(start..) else {
        return (text, Vec::new());
    };
    let excerpt = format!("…{tail}");
    let ranges = matches
        .into_iter()
        .map(|range| range.start - start + "…".len()..range.end - start + "…".len())
        .collect();
    (excerpt, ranges)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decode_rejects_unmatched_or_nested_markers() {
        for bad in ["a<o>b", "<c>tail", "<o>a<o>b<c><c>", "<o><c>"] {
            assert!(decode(bad, "<o>", "<c>").is_none(), "{bad}");
        }
        assert!(decode("plain", "", "x").is_none());
        assert_eq!(
            decode("é <o>日本語<c> 👩🏽‍💻 <o>café<c>", "<o>", "<c>"),
            Some(("é 日本語 👩🏽‍💻 café".into(), vec![3..12, 29..34]))
        );
    }
    #[test]
    fn delimiters_are_absent_from_every_literal_field() {
        let (open, close) = markers("title", "path", Some("body"));
        let (next_open, next_close) = markers(&open, &close, Some(&format!("{open}{close}")));
        for field in [&open, &close] {
            assert!(!field.contains(&next_open));
            assert!(!field.contains(&next_close));
        }
        assert_ne!(next_open, next_close);
    }
    #[test]
    fn excerpt_trimming_preserves_utf8_ranges() {
        assert_eq!(
            visible_excerpt(
                "context café tail".into(),
                std::iter::once(8..13).collect::<Vec<_>>()
            ),
            (
                "…café tail".into(),
                std::iter::once(3..8).collect::<Vec<_>>()
            )
        );
        assert_eq!(
            visible_excerpt(
                "café tail".into(),
                std::iter::once(0..5).collect::<Vec<_>>()
            ),
            (
                "café tail".into(),
                std::iter::once(0..5).collect::<Vec<_>>()
            )
        );
        assert_eq!(
            visible_excerpt("body".into(), vec![]),
            ("body".into(), vec![])
        );
    }
}
