//! Pure UTF-8 source commands; native input owns mutation, history and permission.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
mod format;
mod lines;
mod lists;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Selection {
    pub anchor: usize,
    pub caret: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceCommand {
    ToggleTask,
    ToggleTaskAt { start: usize, end: usize },
    ContinueList,
    IndentList,
    OutdentList,
    Bold,
    Italic,
    Strikethrough,
    CycleHeading,
    MoveUp,
    MoveDown,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct SourceEdit {
    pub range: Range<usize>,
    pub replacement: String,
    pub selection: Selection,
    pub preserve_scroll: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum CommandResult {
    Native,
    NoChange,
    Edit(SourceEdit),
}
pub(super) fn compute(source: &str, selection: Selection, command: SourceCommand) -> CommandResult {
    if !boundary(source, selection.anchor) || !boundary(source, selection.caret) {
        return CommandResult::NoChange;
    }
    match command {
        SourceCommand::MoveUp | SourceCommand::MoveDown => {
            lines::compute(source, selection, command)
        }
        SourceCommand::Bold
        | SourceCommand::Italic
        | SourceCommand::Strikethrough
        | SourceCommand::CycleHeading => format::compute(source, selection, command),
        _ => lists::compute(source, selection, command),
    }
}
fn boundary(source: &str, offset: usize) -> bool {
    offset == source.len()
        || source
            .grapheme_indices(true)
            .any(|(start, _)| start == offset)
}
#[derive(Clone, Debug)]
struct Line {
    start: usize,
    content_end: usize,
    end: usize,
}
impl Line {
    fn content<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.content_end]
    }
    fn separator<'a>(&self, source: &'a str) -> &'a str {
        &source[self.content_end..self.end]
    }
}
#[derive(Debug)]
struct Item {
    range: Range<usize>,
    container: usize,
    marker: usize,
    paint_start: Option<usize>,
    body: usize,
    task: Option<Range<usize>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Style {
    Bold,
    Italic,
    Strike,
}
#[derive(Debug)]
struct Span {
    style: Style,
    full: Range<usize>,
    inner: Range<usize>,
}
struct Context<'a> {
    source: &'a str,
    lines: Vec<Line>,
    items: Vec<Item>,
    blocked: Vec<Range<usize>>,
    headings: Vec<Range<usize>>,
    spans: Vec<Span>,
}
impl<'a> Context<'a> {
    fn new(source: &'a str) -> Self {
        let mut lines = Vec::new();
        let mut start = 0;
        for text in source.split_inclusive('\n') {
            let end = start + text.len();
            let content_end = end
                - if text.ends_with("\r\n") {
                    2
                } else {
                    usize::from(text.ends_with('\n'))
                };
            lines.push(Line {
                start,
                content_end,
                end,
            });
            start = end;
        }
        if source.is_empty() || source.ends_with('\n') {
            lines.push(Line {
                start,
                content_end: start,
                end: start,
            });
        }
        let mut ctx = Self {
            source,
            lines,
            items: Vec::new(),
            blocked: Vec::new(),
            headings: Vec::new(),
            spans: Vec::new(),
        };
        let mut containers = Vec::new();
        let mut item_stack = Vec::new();
        let mut serial = 0;
        let options = Options::ENABLE_TASKLISTS
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
        for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
            match event {
                Event::Start(Tag::List(_)) => {
                    serial += 1;
                    containers.push(serial);
                }
                Event::End(TagEnd::List(_)) => {
                    containers.pop();
                }
                Event::Start(Tag::Item) => {
                    let paint_start =
                        matches!(source.as_bytes().get(range.start), Some(b'-' | b'*' | b'+'))
                            .then_some(range.start);
                    let mut start = range.start;
                    while matches!(source.as_bytes().get(start), Some(b'\r' | b'\n')) {
                        start += 1;
                    }
                    let line = &ctx.lines[ctx.line_at(start)];
                    let marker = whitespace_end(source, start.max(quote_end(source, line)));
                    let range = marker..range.end;
                    let mut end = marker;
                    if matches!(source.as_bytes().get(end), Some(b'-' | b'*' | b'+')) {
                        end += 1;
                    } else {
                        while source.as_bytes().get(end).is_some_and(u8::is_ascii_digit) {
                            end += 1;
                        }
                        end += 1;
                    }
                    let body = whitespace_end(source, end);
                    item_stack.push(ctx.items.len());
                    ctx.items.push(Item {
                        range,
                        marker,
                        paint_start,
                        body,
                        container: containers.last().copied().unwrap_or(0),
                        task: None,
                    });
                }
                Event::End(TagEnd::Item) => {
                    item_stack.pop();
                }
                Event::TaskListMarker(_) => {
                    if let Some(index) = item_stack.last() {
                        ctx.items[*index].body = whitespace_end(source, range.end);
                        ctx.items[*index].task = Some(range);
                    }
                }
                Event::Start(Tag::CodeBlock(_) | Tag::MetadataBlock(_)) | Event::Code(_) => {
                    ctx.blocked.push(range)
                }
                Event::Start(Tag::Heading { .. }) => ctx.headings.push(range),
                Event::Start(Tag::Strong) => ctx.spans.push(Span {
                    style: Style::Bold,
                    inner: range.start + 2..range.end - 2,
                    full: range,
                }),
                Event::Start(Tag::Emphasis) => ctx.spans.push(Span {
                    style: Style::Italic,
                    inner: range.start + 1..range.end - 1,
                    full: range,
                }),
                Event::Start(Tag::Strikethrough) => {
                    let width = source[range.clone()]
                        .bytes()
                        .take_while(|byte| *byte == b'~')
                        .take(2)
                        .count();
                    ctx.spans.push(Span {
                        style: Style::Strike,
                        inner: range.start + width..range.end - width,
                        full: range,
                    });
                }
                _ => {}
            }
        }
        ctx
    }
    fn line_at(&self, offset: usize) -> usize {
        self.lines
            .partition_point(|line| line.start <= offset)
            .saturating_sub(1)
    }
    fn selected_lines(&self, selection: Selection) -> Range<usize> {
        let start = selection.anchor.min(selection.caret);
        let end = selection.anchor.max(selection.caret);
        let first = self.line_at(start);
        let last = self.line_at(end);
        first..if end > start && self.lines[last].start == end {
            last
        } else {
            last + 1
        }
    }
    fn item(&self, line: &Line) -> Option<&Item> {
        self.items
            .iter()
            .filter(|item| item.range.start < line.end && item.range.end > line.start)
            .max_by_key(|item| item.range.start)
    }
    fn protected(&self, range: &Range<usize>) -> bool {
        self.blocked
            .iter()
            .any(|blocked| intersects(blocked, range))
    }
    fn heading(&self, line: &Line) -> Option<Range<usize>> {
        if !self
            .headings
            .iter()
            .any(|range| range.start >= line.start && range.start <= line.content_end)
        {
            return None;
        }
        let start = whitespace_end(self.source, quote_end(self.source, line));
        let end = start
            + self.source[start..line.content_end]
                .bytes()
                .take_while(|byte| *byte == b'#')
                .count();
        (end > start
            && end - start <= 6
            && (end == line.content_end
                || matches!(self.source.as_bytes().get(end), Some(b' ' | b'\t'))))
        .then_some(start..end)
    }
    fn body_start(&self, line: &Line) -> usize {
        if let Some(item) = self.item(line).filter(|item| item.marker >= line.start) {
            return item.body.min(line.content_end);
        }
        if let Some(heading) = self.heading(line) {
            return whitespace_end(self.source, heading.end).min(line.content_end);
        }
        quote_end(self.source, line)
    }
    fn semantic(&self, mut range: Range<usize>) -> Range<usize> {
        while let Some(span) = self.spans.iter().find(|span| span.full == range) {
            range = span.inner.clone();
        }
        range
    }
}
fn whitespace_end(source: &str, start: usize) -> usize {
    start
        + source[start..]
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count()
}
fn quote_end(source: &str, line: &Line) -> usize {
    let mut accepted = line.start;
    let mut pos = accepted;
    loop {
        let spaces = source[pos..line.content_end]
            .bytes()
            .take_while(|byte| *byte == b' ')
            .take(3)
            .count();
        pos += spaces;
        if pos >= line.content_end || source.as_bytes()[pos] != b'>' {
            return accepted;
        }
        pos += 1;
        if pos < line.content_end && matches!(source.as_bytes()[pos], b' ' | b'\t') {
            pos += 1;
        }
        accepted = pos;
    }
}
fn intersects(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}
struct Change {
    range: Range<usize>,
    replacement: String,
    sticky: bool,
}
fn change(range: Range<usize>, replacement: impl Into<String>) -> Change {
    Change {
        range,
        replacement: replacement.into(),
        sticky: true,
    }
}
fn map_offset(offset: usize, changes: &[Change]) -> usize {
    let mut delta: isize = 0;
    for change in changes {
        if offset < change.range.start {
            break;
        }
        if change.range.is_empty() {
            if offset > change.range.start || change.sticky {
                delta += change.replacement.len() as isize;
            }
        } else if offset >= change.range.end {
            delta += change.replacement.len() as isize - change.range.len() as isize;
        } else {
            return (change.range.start as isize + delta) as usize
                + (offset - change.range.start).min(change.replacement.len());
        }
    }
    (offset as isize + delta) as usize
}
fn edit(
    source: &str,
    selection: Selection,
    mut changes: Vec<Change>,
    preserve_scroll: bool,
) -> CommandResult {
    if changes.is_empty() {
        return CommandResult::NoChange;
    }
    changes.sort_by_key(|change| (change.range.start, change.range.end));
    let range = changes[0].range.start..changes.last().unwrap().range.end;
    let mut replacement = String::new();
    let mut cursor = range.start;
    for change in &changes {
        replacement.push_str(&source[cursor..change.range.start]);
        replacement.push_str(&change.replacement);
        cursor = change.range.end;
    }
    let selection = Selection {
        anchor: map_offset(selection.anchor, &changes),
        caret: map_offset(selection.caret, &changes),
    };
    if source[range.clone()] == replacement {
        CommandResult::NoChange
    } else {
        CommandResult::Edit(SourceEdit {
            range,
            replacement,
            selection,
            preserve_scroll,
        })
    }
}

#[cfg(test)]
fn run(source: &str, selection: Selection, command: SourceCommand) -> (String, Selection) {
    match compute(source, selection, command) {
        CommandResult::Edit(edit) => {
            let mut source = source.to_owned();
            source.replace_range(edit.range, &edit.replacement);
            (source, edit.selection)
        }
        _ => (source.to_owned(), selection),
    }
}
#[cfg(test)]
mod tests {
    use super::{SourceCommand::*, *};
    fn sel(anchor: usize, caret: usize) -> Selection {
        Selection { anchor, caret }
    }
    fn task_at(start: usize, end: usize) -> SourceCommand {
        ToggleTaskAt { start, end }
    }
    #[test]
    fn task_pointer_changes_only_state_byte_and_preserves_selection() {
        assert_eq!(
            run("1. [X] café", sel(12, 12), task_at(3, 7)),
            ("1. [ ] café".into(), sel(12, 12))
        );
        assert_eq!(
            run("- [ ] one", sel(9, 2), task_at(0, 6)),
            ("- [x] one".into(), sel(9, 2))
        );
        assert_eq!(run("- [ ] one", sel(9, 9), task_at(0, 5)).0, "- [ ] one");
    }
    #[test]
    fn task_shortcut_uses_caret_line_even_with_selection() {
        assert_eq!(
            run("- [ ] a\n- [x] b", sel(0, 15), ToggleTask).0,
            "- [ ] a\n- [ ] b"
        );
        assert_eq!(
            run("> - [X] café", sel(13, 13), ToggleTask).0,
            "> - [ ] café"
        );
        assert_eq!(
            run("```\n- [ ] no\n```", sel(10, 10), ToggleTask).0,
            "```\n- [ ] no\n```"
        );
    }
    #[test]
    fn return_continues_checked_tasks_as_unchecked() {
        assert_eq!(
            run("- [x] one", sel(9, 9), ContinueList),
            ("- [x] one\n- [ ] ".into(), sel(16, 16))
        );
        assert_eq!(
            run("> 9) café!", sel(10, 10), ContinueList).0,
            "> 9) café\n> 10) !"
        );
        assert_eq!(
            run("* one\r\n* two", sel(12, 12), ContinueList).0,
            "* one\r\n* two\r\n* "
        );
    }
    #[test]
    fn return_exits_empty_nested_item() {
        assert_eq!(
            run("  - ", sel(4, 4), ContinueList),
            ("  ".into(), sel(2, 2))
        );
        assert_eq!(run("> - [x]  ", sel(9, 9), ContinueList).0, "> ");
    }
    #[test]
    fn non_list_selection_raw_marker_and_large_order_keep_native_return() {
        for (source, selection) in [
            ("plain", sel(2, 2)),
            ("- one", sel(2, 4)),
            ("- one", sel(1, 1)),
            ("999999999. x", sel(12, 12)),
            ("    - code", sel(10, 10)),
            ("\\- escaped", sel(10, 10)),
        ] {
            assert_eq!(
                compute(source, selection, ContinueList),
                CommandResult::Native,
                "{source}"
            );
        }
    }
    #[test]
    fn indent_and_outdent_preserve_selection_and_exclude_line_start_endpoint() {
        assert_eq!(
            run("- a\n- b", sel(4, 0), IndentList),
            ("  - a\n- b".into(), sel(6, 2))
        );
        assert_eq!(
            run("> - a\n> - b", sel(0, 11), IndentList).0,
            ">   - a\n>   - b"
        );
        assert_eq!(
            run("  - a\n  - b", sel(11, 0), OutdentList),
            ("- a\n- b".into(), sel(7, 0))
        );
        assert_eq!(run("- a\n\t- b", sel(8, 8), IndentList).0, "- a\n\t\t- b");
        assert_eq!(
            compute("- a\n\nplain", sel(0, 10), IndentList),
            CommandResult::Native
        );
    }
    #[test]
    fn selected_list_prefixes_remain_outside_inline_formatting() {
        assert_eq!(
            run("- one\n- two", sel(0, 11), Bold),
            ("- **one**\n- **two**".into(), sel(4, 17))
        );
        assert_eq!(
            run("> # café\n> two", sel(0, 15), Italic).0,
            "> # *café*\n> *two*"
        );
        assert_eq!(run("- one", sel(1, 5), Bold).0, "- one");
    }
    #[test]
    fn inline_shortcuts_toggle_unicode_words_and_keep_directed_selection() {
        assert_eq!(run("café", sel(3, 3), Bold), ("**café**".into(), sel(5, 5)));
        assert_eq!(run("word", sel(4, 0), Italic), ("*word*".into(), sel(5, 1)));
        assert_eq!(
            run("word", sel(2, 2), Strikethrough),
            ("~~word~~".into(), sel(4, 4))
        );
        assert_eq!(run("word ", sel(5, 5), Bold).0, "word ");
        assert_eq!(run("word", sel(4, 4), Bold).0, "**word**");
        assert_eq!(run("**word**", sel(2, 6), Bold), ("word".into(), sel(0, 4)));
        assert_eq!(run("**word**", sel(0, 8), Bold), ("word".into(), sel(0, 4)));
    }
    #[test]
    fn combined_styles_remove_only_requested_delimiters() {
        assert_eq!(run("***word***", sel(5, 5), Bold).0, "*word*");
        assert_eq!(run("***word***", sel(5, 5), Italic).0, "**word**");
        assert_eq!(run("**one**\ntwo", sel(0, 11), Bold).0, "**one**\n**two**");
    }
    #[test]
    fn formatting_declines_partial_spans_code_metadata_and_split_graphemes() {
        for (source, selection) in [
            ("**word**", sel(3, 5)),
            ("**word**", sel(1, 6)),
            ("`word`", sel(3, 3)),
            ("```\nword\n```", sel(6, 6)),
            ("---\ntitle: word\n---\ntext", sel(10, 10)),
            ("aé", sel(2, 2)),
        ] {
            assert_eq!(run(source, selection, Bold).0, source);
        }
    }
    #[test]
    fn headings_cycle_only_opening_prefix_on_caret_line() {
        for (source, want) in [
            ("word", "# word"),
            ("# word", "## word"),
            ("## word", "### word"),
            ("### word", "word"),
            ("###### word ##", "word ##"),
            ("", "# "),
            ("> ## word", "> ### word"),
        ] {
            assert_eq!(
                run(source, sel(source.len(), source.len()), CycleHeading).0,
                want
            );
        }
        assert_eq!(run("- item", sel(4, 4), CycleHeading).0, "- item");
    }
    #[test]
    fn move_preserves_crlf_terminator_and_directed_selection() {
        assert_eq!(
            run("a\r\nb", sel(0, 0), MoveDown),
            ("b\r\na".into(), sel(3, 3))
        );
        assert_eq!(
            run("a\r\nb\nc", sel(4, 3), MoveDown),
            ("a\r\nc\nb".into(), sel(6, 5))
        );
        assert_eq!(
            run("a\nb\nc", sel(4, 2), MoveUp),
            ("b\na\nc".into(), sel(2, 0))
        );
    }
    #[test]
    fn terminal_separator_is_not_a_movable_extra_line() {
        assert_eq!(run("a\n", sel(0, 0), MoveDown).0, "a\n");
        assert_eq!(run("a\n\n", sel(0, 0), MoveDown).0, "\na\n");
        assert_eq!(run("a\nb\n", sel(4, 4), MoveUp).0, "b\na\n");
        assert_eq!(run("", sel(0, 0), MoveDown).0, "");
    }
    #[test]
    fn parser_recognized_single_tilde_strike_keeps_unicode_content() {
        assert_eq!(run("~café~", sel(3, 3), Strikethrough).0, "café");
        assert_eq!(run("~a~", sel(1, 1), Strikethrough).0, "a");
    }
    #[test]
    fn return_on_continuation_never_deletes_the_item_header() {
        assert_eq!(
            run("- \n  body", sel(9, 9), ContinueList).0,
            "- \n  body\n- "
        );
    }
    #[test]
    fn identical_lines_do_not_create_an_undo_entry() {
        assert_eq!(
            compute("same\nsame", sel(1, 1), MoveDown),
            CommandResult::NoChange
        );
    }
}
