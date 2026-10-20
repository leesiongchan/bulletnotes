//! Experimental source-range highlighting; not a released Slint API.
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};
use slint::language::TextInputDecoration;

slint::include_modules!();

pub const SOURCE: &str =
    "# Header 1\n## Header 2\n### Header 3\n- [ ] Task\n- List\n[link](https://google.com)\n";

/// Derive colors without rewriting or serializing the Markdown document.
pub fn markdown_decorations(source: &str) -> Vec<TextInputDecoration> {
    let mut spans = Vec::new();
    let mut previous_end = 0;
    for (event, range) in Parser::new(source).into_offset_iter() {
        let color = match event {
            Event::Start(Tag::Heading { level, .. }) => match level {
                HeadingLevel::H1 => (255, 112, 39),
                HeadingLevel::H2 => (199, 78, 26),
                _ => (224, 121, 43),
            },
            Event::Start(Tag::Link { .. }) => (151, 71, 16),
            _ => continue,
        };
        let end = range.start + source[range.clone()].trim_end_matches(['\r', '\n']).len();
        if range.start < previous_end || range.start >= end {
            continue;
        }
        let (Ok(start), Ok(end_offset)) = (i32::try_from(range.start), i32::try_from(end)) else {
            continue;
        };
        let mut span = TextInputDecoration::default();
        span.start = start;
        span.end = end_offset;
        span.foreground = slint::Color::from_rgb_u8(color.0, color.1, color.2);
        spans.push(span);
        // Heading color owns its entire range, including any nested link.
        previous_end = end;
    }
    spans
}

/// Experimental marker parser used by the standalone fixture only.
pub fn markdown_markers(source: &str) -> Vec<slint::language::TextInputMarker> {
    use pulldown_cmark::{Options, TagEnd};
    use slint::language::{TextInputMarker, TextInputMarkerKind as Kind};
    let whitespace_end = |start: usize| {
        start
            + source[start..]
                .bytes()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count()
    };
    let make_marker = |start: usize, end: usize, kind| {
        let mut marker = TextInputMarker::default();
        marker.start = i32::try_from(start).ok()?;
        marker.end = i32::try_from(end).ok()?;
        marker.kind = kind;
        marker.foreground = slint::Color::from_rgb_u8(60, 46, 24);
        Some(marker)
    };
    let mut markers: Vec<TextInputMarker> = Vec::new();
    let mut items: Vec<Option<usize>> = Vec::new();
    let options = Options::ENABLE_TASKLISTS | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Item) => {
                let index =
                    if matches!(source.as_bytes().get(range.start), Some(b'-' | b'*' | b'+')) {
                        make_marker(range.start, whitespace_end(range.start + 1), Kind::Bullet).map(
                            |marker| {
                                let index = markers.len();
                                markers.push(marker);
                                index
                            },
                        )
                    } else {
                        None
                    };
                items.push(index);
            }
            Event::TaskListMarker(checked) => {
                let Some(current) = items.last_mut() else {
                    continue;
                };
                let start = current.map_or(range.start, |index| markers[index].start as usize);
                let kind = if checked {
                    Kind::CheckedCheckbox
                } else {
                    Kind::Checkbox
                };
                if let Some(marker) = make_marker(start, whitespace_end(range.end), kind) {
                    if let Some(index) = *current {
                        markers[index] = marker;
                    } else {
                        *current = Some(markers.len());
                        markers.push(marker);
                    }
                }
            }
            Event::End(TagEnd::Item) => {
                items.pop();
            }
            _ => {}
        }
    }
    markers
}

/// Generated content for the paint-only marker feasibility window.
pub const MARKER_SOURCE: &str = "# Source-backed markers\n- [ ] Task\n- [x] Done\n- List\n  - Nested café 日本語\n\n- A longer list entry wraps while its source prefix keeps its original width.\n";
