use super::*;
pub(super) fn compute(source: &str, selection: Selection, command: SourceCommand) -> CommandResult {
    let ctx = Context::new(source);
    let line = &ctx.lines[ctx.line_at(selection.caret)];
    match command {
        SourceCommand::ToggleTask | SourceCommand::ToggleTaskAt { .. } => {
            let item = match command {
                SourceCommand::ToggleTaskAt { start, end } => ctx.items.iter().find(|item| {
                    item.task.as_ref().is_some_and(|task| {
                        let marker_start = item.paint_start.unwrap_or(task.start);
                        marker_start == start && whitespace_end(source, task.end) == end
                    })
                }),
                _ => ctx.item(line).filter(|item| item.marker >= line.start),
            };
            let Some(task) = item.and_then(|item| item.task.as_ref()) else {
                return CommandResult::NoChange;
            };
            let state = task.start + 1;
            let replacement = match source.as_bytes().get(state) {
                Some(b' ') => "x",
                Some(b'x' | b'X') => " ",
                _ => return CommandResult::NoChange,
            };
            edit(
                source,
                selection,
                vec![change(state..state + 1, replacement)],
                true,
            )
        }
        SourceCommand::ContinueList => {
            if selection.anchor != selection.caret {
                return CommandResult::Native;
            }
            let Some(item) = ctx.item(line) else {
                return CommandResult::Native;
            };
            if ctx.protected(&(line.start..line.end)) || selection.caret < item.body {
                return CommandResult::Native;
            }
            let first = &ctx.lines[ctx.line_at(item.marker)];
            if line.start == first.start
                && source[item.body..first.content_end]
                    .trim_matches([' ', '\t'])
                    .is_empty()
            {
                return edit(
                    source,
                    selection,
                    vec![change(item.marker..first.content_end, "")],
                    false,
                );
            }
            let marker = source.as_bytes()[item.marker];
            let mut prefix = source[first.start..item.marker].to_owned();
            if matches!(marker, b'-' | b'*' | b'+') {
                prefix.push(marker as char);
            } else {
                let end = item.marker
                    + source[item.marker..]
                        .bytes()
                        .take_while(u8::is_ascii_digit)
                        .count();
                let Some(next) = source[item.marker..end]
                    .parse::<u32>()
                    .ok()
                    .and_then(|number| number.checked_add(1))
                    .filter(|number| *number <= 999_999_999)
                else {
                    return CommandResult::Native;
                };
                prefix.push_str(&next.to_string());
                prefix.push(source.as_bytes()[end] as char);
            }
            prefix.push(' ');
            if item.task.is_some() {
                prefix.push_str("[ ] ");
            }
            let separator = if line.end > line.content_end {
                line.separator(source)
            } else {
                ctx.lines[..ctx.line_at(selection.caret)]
                    .iter()
                    .rev()
                    .find(|line| line.end > line.content_end)
                    .map_or("\n", |line| line.separator(source))
            };
            edit(
                source,
                selection,
                vec![change(
                    selection.caret..selection.caret,
                    format!("{separator}{prefix}"),
                )],
                false,
            )
        }
        SourceCommand::IndentList | SourceCommand::OutdentList => {
            let mut changes = Vec::new();
            let mut container = None;
            for index in ctx.selected_lines(selection) {
                let line = &ctx.lines[index];
                let Some(item) = ctx.item(line) else {
                    return CommandResult::Native;
                };
                if ctx.protected(&(line.start..line.end))
                    || container.is_some_and(|value| value != item.container)
                {
                    return CommandResult::Native;
                }
                container = Some(item.container);
                let start = quote_end(source, line);
                let end = whitespace_end(source, start).min(line.content_end);
                if command == SourceCommand::IndentList {
                    changes.push(change(
                        start..start,
                        if source[start..end].contains('\t') {
                            "\t"
                        } else {
                            "  "
                        },
                    ));
                } else {
                    let count = if source.as_bytes().get(start) == Some(&b'\t') {
                        1
                    } else {
                        source[start..end]
                            .bytes()
                            .take_while(|byte| *byte == b' ')
                            .take(2)
                            .count()
                    };
                    if count > 0 {
                        changes.push(change(start..start + count, ""));
                    }
                }
            }
            edit(source, selection, changes, false)
        }
        _ => CommandResult::NoChange,
    }
}
