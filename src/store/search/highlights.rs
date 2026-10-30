//! Source excerpts and display transforms preserve original UTF-8 match boundaries.
use std::ops::Range;

pub fn excerpt(
    text: &str,
    matches: &[Range<usize>],
    max_chars: usize,
) -> (String, Vec<Range<usize>>) {
    let start = matches.first().map_or(0, |range| range.start);
    let Some(tail) = text.get(start..) else {
        return (text.chars().take(max_chars).collect(), vec![]);
    };
    let end = start
        + tail
            .char_indices()
            .nth(max_chars)
            .map_or(tail.len(), |(at, _)| at);
    let prefix = if start > 0 { "…" } else { "" };
    let excerpt = format!("{prefix}{}", &text[start..end]);
    let ranges = matches
        .iter()
        .filter(|r| {
            r.start >= start
                && r.start < end
                && r.end > r.start
                && r.end <= text.len()
                && text.is_char_boundary(r.end)
        })
        .map(|r| r.start - start + prefix.len()..r.end.min(end) - start + prefix.len())
        .collect();
    (excerpt, ranges)
}

pub fn condense(text: &str, matches: &[Range<usize>]) -> (String, Vec<Range<usize>>) {
    let mut output = String::new();
    let mut offsets = vec![0; text.len() + 1];
    let mut chars = text.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        offsets[at] = output.len();
        if ch == '\r' || ch == '\n' {
            output.push_str(" | ");
            if ch == '\r' && chars.peek().is_some_and(|(_, ch)| *ch == '\n') {
                let (next, _) = chars.next().unwrap_or((at, ch));
                offsets[next] = offsets[at];
                offsets[next + 1] = output.len();
            }
        } else {
            output.push(ch);
        }
        offsets[at + ch.len_utf8()] = output.len();
    }
    let ranges = matches
        .iter()
        .filter(|r| {
            r.start < r.end
                && r.end <= text.len()
                && text.is_char_boundary(r.start)
                && text.is_char_boundary(r.end)
        })
        .map(|r| offsets[r.start]..offsets[r.end])
        .filter(|r| r.start < r.end)
        .collect();
    (output, ranges)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipping_and_condensing_preserve_utf8_ranges() {
        let source = "before café\r\n日本語 tail";
        let ranges = vec![7..12, 14..23];
        let (clipped, hits) = excerpt(source, &ranges, 320);
        assert_eq!(clipped, "…café\r\n日本語 tail");
        assert_eq!(&clipped[hits[0].clone()], "café");
        let (line, hits) = condense(&clipped, &hits);
        assert_eq!(line, "…café | 日本語 tail");
        assert_eq!(&line[hits[0].clone()], "café");
        assert_eq!(&line[hits[1].clone()], "日本語");
        assert!(excerpt(source, &ranges, 5).0.chars().count() <= 6);
    }
}
