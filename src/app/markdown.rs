//! Source-only Markdown decorations; neither editing nor serialization belongs here.
use crate::{
    UiTheme,
    theme::{self, contrast::contrast_ratio},
};
use pulldown_cmark::{Event, Options, Parser, Tag};
use slint::language::TextInputDecoration;

pub(super) fn decorations(
    source: &str,
    palette: &UiTheme,
    high_contrast: bool,
) -> Vec<TextInputDecoration> {
    let rgb = |color: slint::Color| theme::Color {
        r: color.red(),
        g: color.green(),
        b: color.blue(),
    };
    let foreground =
        if high_contrast || contrast_ratio(rgb(palette.accent), rgb(palette.paper)) < 4.5 {
            palette.ink
        } else {
            palette.accent
        };
    let mut spans = Vec::new();
    let mut previous_end = 0;
    for (event, range) in
        Parser::new_ext(source, Options::ENABLE_YAML_STYLE_METADATA_BLOCKS).into_offset_iter()
    {
        if !matches!(event, Event::Start(Tag::Heading { .. } | Tag::Link { .. })) {
            continue;
        }
        let end = range.start + source[range.clone()].trim_end_matches(['\r', '\n']).len();
        // An outer heading owns nested links; Slint accepts disjoint source ranges.
        if range.start < previous_end || range.start >= end {
            continue;
        }
        let (Ok(start), Ok(end_offset)) = (i32::try_from(range.start), i32::try_from(end)) else {
            continue;
        };
        let mut span = TextInputDecoration::default();
        span.start = start;
        span.end = end_offset;
        span.foreground = foreground;
        spans.push(span);
        previous_end = end;
    }
    spans
}

pub(super) fn markers(source: &str, palette: &UiTheme) -> Vec<slint::language::TextInputMarker> {
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
        marker.foreground = palette.ink;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_keep_original_prefix_offsets_and_task_states() {
        use slint::language::TextInputMarkerKind as Kind;
        let source = "---\nitems: - [ ] metadata\n---\n- [ ] café\r\n- [X] 日本語\n  * nested\n> + quoted\n1. [ ] ordered\n\n```\n- [ ] code\n```\n\\- escaped\n";
        let palette = UiTheme::default();
        let markers = markers(source, &palette);
        let prefixes: Vec<_> = markers
            .iter()
            .map(|m| {
                assert_eq!(m.foreground, palette.ink);
                (&source[m.start as usize..m.end as usize], m.kind)
            })
            .collect();
        assert_eq!(
            prefixes,
            vec![
                ("- [ ] ", Kind::Checkbox),
                ("- [X] ", Kind::CheckedCheckbox),
                ("* ", Kind::Bullet),
                ("+ ", Kind::Bullet),
                ("[ ] ", Kind::Checkbox),
            ]
        );
        assert!(markers.windows(2).all(|pair| pair[0].end <= pair[1].start));
    }

    #[test]
    fn plain_code_and_escaped_prefixes_produce_no_markers() {
        let source = "ordinary\n\n`- [ ] inline`\n\n    - [ ] indented code\n\n~~~\n- fenced\n~~~\n\\- escaped\n";
        assert!(markers(source, &UiTheme::default()).is_empty());
    }

    #[test]
    fn original_unicode_and_crlf_offsets_are_sorted_and_disjoint() {
        let source = "# café [site](https://example.com)\r\n\r\n[日本語](https://example.org)\r\n";
        let spans = decorations(source, &UiTheme::default(), false);
        assert_eq!(spans.len(), 2);
        assert_eq!(
            &source[spans[0].start as usize..spans[0].end as usize],
            "# café [site](https://example.com)"
        );
        assert_eq!(
            &source[spans[1].start as usize..spans[1].end as usize],
            "[日本語](https://example.org)"
        );
        assert!(spans[0].end <= spans[1].start);
    }

    #[test]
    fn escaped_and_code_markdown_remain_plain() {
        let source = "\\# literal\n\n`[code](https://example.com)`\n\n```\n# Heading\n[link](https://example.com)\n```\n";
        assert!(decorations(source, &UiTheme::default(), false).is_empty());
    }
}
