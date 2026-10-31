//! Source-only Markdown decorations; neither editing nor serialization belongs here.
use crate::{
    UiTheme,
    theme::{self, contrast::contrast_ratio},
};
use pulldown_cmark::{Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};
use slint::language::TextInputDecoration;

#[derive(Clone, Copy, PartialEq)]
struct SourceStyle {
    foreground: slint::Color,
    bold: bool,
    italic: bool,
    strikethrough: bool,
    underline: bool,
    code: bool,
}

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
    let readable = |color| {
        if high_contrast || contrast_ratio(rgb(color), rgb(palette.paper)) < 4.5 {
            palette.ink
        } else {
            color
        }
    };
    let accent = readable(palette.accent);
    let role_color = |configured: slint::Color, fallback| {
        if high_contrast {
            palette.ink
        } else if configured.alpha() != 0 {
            configured
        } else {
            fallback
        }
    };
    let link_color = role_color(palette.markdown.link, accent);
    let plain = SourceStyle {
        foreground: palette.ink,
        bold: false,
        italic: false,
        strikethrough: false,
        underline: false,
        code: false,
    };
    let quiet = SourceStyle {
        foreground: role_color(palette.markdown.syntax, readable(palette.muted)),
        ..plain
    };
    let mut spans: Vec<TextInputDecoration> = Vec::new();
    let mut last_style = plain;
    let mut emit = |range: std::ops::Range<usize>, style: SourceStyle| {
        if range.start >= range.end || style == plain {
            return;
        }
        let end = range.start + source[range.clone()].trim_end_matches(['\r', '\n']).len();
        let (Ok(start), Ok(end)) = (i32::try_from(range.start), i32::try_from(end)) else {
            return;
        };
        if start == end {
            return;
        }
        if let Some(last) = spans.last_mut()
            && last.end == start
            && last_style == style
        {
            last.end = end;
            return;
        }
        let mut span = TextInputDecoration::default();
        span.start = start;
        span.end = end;
        span.foreground = style.foreground;
        span.bold = style.bold;
        span.italic = style.italic;
        span.strikethrough = style.strikethrough;
        span.underline = style.underline;
        span.code = style.code;
        spans.push(span);
        last_style = style;
    };
    // Parser nesting composes styles; emitting only leaves and intervening syntax
    // keeps the renderer's source ranges sorted and disjoint.
    let mut stack = vec![(plain, plain)];
    let mut cursor = 0;
    let options = Options::ENABLE_YAML_STYLE_METADATA_BLOCKS | Options::ENABLE_STRIKETHROUGH;
    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        let (mut body, mut syntax) = *stack.last().unwrap_or(&(plain, plain));
        match event {
            Event::Start(tag) => {
                emit(cursor..range.start, syntax);
                cursor = cursor.max(range.start);
                match tag {
                    Tag::Heading { level, .. } => {
                        body.foreground = match level {
                            HeadingLevel::H1 => role_color(palette.markdown.heading1, accent),
                            HeadingLevel::H2 => role_color(
                                palette.markdown.heading2,
                                readable(accent.mix(&palette.ink, 0.8)),
                            ),
                            _ => {
                                let lighter = accent.mix(&palette.paper, 0.95);
                                if contrast_ratio(rgb(lighter), rgb(palette.paper)) >= 4.5 {
                                    role_color(palette.markdown.heading3, readable(lighter))
                                } else {
                                    role_color(palette.markdown.heading3, accent)
                                }
                            }
                        };
                        body.bold = matches!(level, HeadingLevel::H1 | HeadingLevel::H2);
                        syntax = body;
                    }
                    Tag::Strong => {
                        body.bold = true;
                        syntax = quiet;
                    }
                    Tag::Emphasis => {
                        body.italic = true;
                        syntax = quiet;
                    }
                    Tag::Strikethrough => {
                        body.strikethrough = true;
                        body.foreground = quiet.foreground;
                        syntax = quiet;
                    }
                    Tag::Link { link_type, .. } => {
                        if link_type == LinkType::Inline {
                            emit(range.start..range.start + 1, quiet);
                            cursor = range.start + 1;
                        } else {
                            body.foreground = link_color;
                            body.underline = true;
                        }
                        syntax = quiet;
                    }
                    Tag::CodeBlock(_) => {
                        body = plain;
                        syntax = quiet;
                    }
                    Tag::MetadataBlock(_) => {
                        body = plain;
                        syntax = plain;
                    }
                    _ => {}
                }
                stack.push((body, syntax));
            }
            Event::End(tag) => {
                let tail = source.get(cursor..range.end).unwrap_or_default();
                if tag == TagEnd::Link && tail.starts_with("](") && tail.ends_with(')') {
                    emit(cursor..cursor + 2, quiet);
                    emit(
                        cursor + 2..range.end - 1,
                        SourceStyle {
                            foreground: link_color,
                            underline: true,
                            ..plain
                        },
                    );
                    emit(range.end - 1..range.end, quiet);
                } else {
                    emit(cursor..range.end, syntax);
                }
                cursor = cursor.max(range.end);
                stack.pop();
            }
            Event::Code(_) => {
                emit(cursor..range.start, syntax);
                let ticks = source[range.clone()]
                    .bytes()
                    .take_while(|byte| *byte == b'`')
                    .count();
                emit(range.start..range.start + ticks, quiet);
                emit(
                    range.start + ticks..range.end - ticks,
                    SourceStyle { code: true, ..body },
                );
                emit(range.end - ticks..range.end, quiet);
                cursor = range.end;
            }
            _ => {
                emit(cursor..range.start, syntax);
                emit(range.clone(), body);
                cursor = cursor.max(range.end);
            }
        }
    }
    spans.extend(super::code_highlighting::tokens(
        source,
        palette,
        high_contrast,
    ));
    spans.sort_by_key(|span| span.start);
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
        marker.compact = true;
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
        assert!(spans.iter().any(
            |span| span.bold && source[span.start as usize..span.end as usize].contains("café")
        ));
        assert!(spans.iter().any(|span| span.underline
            && &source[span.start as usize..span.end as usize] == "https://example.org"));
        assert!(spans.windows(2).all(|pair| pair[0].end <= pair[1].start));
        for span in spans {
            assert!(span.start < span.end);
            assert!(!source[span.start as usize..span.end as usize].contains('\r'));
        }
    }

    #[test]
    fn code_styling_does_not_interpret_literal_markdown() {
        let source = "\\# literal\n\n`[code](https://example.com)`\n\n```\n# Heading\n[link](https://example.com)\n```\n";
        let spans = decorations(source, &UiTheme::default(), false);
        assert!(spans.iter().any(|span| span.code));
        assert!(
            spans
                .iter()
                .all(|span| !span.bold && !span.italic && !span.underline)
        );
        assert!(decorations("\\# literal\n\\*literal*", &UiTheme::default(), false).is_empty());
    }

    #[test]
    fn nested_emphasis_composes_in_disjoint_unicode_source_runs() {
        let source = "# **bold *日本語***\r\n***café 👩🏽‍💻***\r\n~~**done**~~";
        let spans = decorations(source, &UiTheme::default(), false);
        for (needle, bold, italic, strike) in [
            ("日本語", true, true, false),
            ("café 👩🏽‍💻", true, true, false),
            ("done", true, false, true),
        ] {
            let at = source.find(needle).unwrap() as i32;
            let span = spans
                .iter()
                .find(|span| span.start <= at && at < span.end)
                .unwrap();
            assert_eq!(
                (span.bold, span.italic, span.strikethrough),
                (bold, italic, strike),
                "{needle}"
            );
        }
        assert!(spans.windows(2).all(|pair| pair[0].end <= pair[1].start));
    }

    #[test]
    fn heading_hierarchy_uses_weight_without_changing_the_source() {
        let source = "# First\n## Second\n### Third\n";
        let spans = decorations(source, &UiTheme::default(), false);
        for (needle, bold) in [("First", true), ("Second", true), ("Third", false)] {
            let at = source.find(needle).unwrap() as i32;
            let span = spans.iter().find(|span| span.start <= at && at < span.end);
            assert_eq!(span.is_some_and(|span| span.bold), bold, "{needle}");
        }
    }

    #[test]
    fn empty_link_labels_still_style_the_destination() {
        let source = "[](https://example.com)";
        let spans = decorations(source, &UiTheme::default(), false);
        assert!(spans.iter().any(|span| span.underline
            && &source[span.start as usize..span.end as usize] == "https://example.com"));
    }
}
