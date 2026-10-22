use super::*;
pub(super) fn compute(source: &str, selection: Selection, command: SourceCommand) -> CommandResult {
    if source.is_empty() {
        return CommandResult::NoChange;
    }
    let ctx = Context::new(source);
    let count = ctx.lines.len() - usize::from(source.ends_with('\n'));
    let range = ctx.selected_lines(selection);
    let first = range.start.min(count - 1);
    let last = range.end.saturating_sub(1).min(count - 1);
    let up = command == SourceCommand::MoveUp;
    if (up && first == 0) || (!up && last + 1 == count) {
        return CommandResult::NoChange;
    }
    let low = if up { first - 1 } else { first };
    let high = if up { last } else { last + 1 };
    let mut order: Vec<_> = (low..=high).collect();
    if up {
        order.rotate_left(1);
    } else {
        order.rotate_right(1);
    }
    let mut replacement = String::new();
    let mut positions = vec![0; count];
    let start = ctx.lines[low].start;
    for (offset, old) in order.iter().copied().enumerate() {
        positions[old] = start + replacement.len();
        replacement.push_str(ctx.lines[old].content(source));
        if offset < high - low {
            replacement.push_str(ctx.lines[low + offset].separator(source));
        }
    }
    let map = |offset: usize| {
        let mut index = ctx.line_at(offset).min(count - 1);
        let upper = selection.anchor.max(selection.caret);
        if offset == upper
            && selection.anchor != selection.caret
            && index > first
            && ctx.lines[index].start == offset
        {
            index -= 1;
        }
        let line = &ctx.lines[index];
        let position = order.iter().position(|old| *old == index).unwrap();
        let within = offset.saturating_sub(line.start);
        positions[index]
            + within.min(line.content_end - line.start)
            + if within > line.content_end - line.start {
                ctx.lines[low + position].end - ctx.lines[low + position].content_end
            } else {
                0
            }
    };
    if source[start..ctx.lines[high].content_end] == replacement {
        return CommandResult::NoChange;
    }
    CommandResult::Edit(SourceEdit {
        range: start..ctx.lines[high].content_end,
        replacement,
        selection: Selection {
            anchor: map(selection.anchor),
            caret: map(selection.caret),
        },
        preserve_scroll: false,
    })
}
