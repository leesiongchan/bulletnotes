use super::*;

pub(super) fn compute(source: &str, selection: Selection, command: SourceCommand) -> CommandResult {
    let ctx = Context::new(source);
    if command == SourceCommand::CycleHeading {
        return heading(&ctx, selection);
    }
    let (style, delimiter) = match command {
        SourceCommand::Bold => (Style::Bold, "**"),
        SourceCommand::Italic => (Style::Italic, "*"),
        SourceCommand::Strikethrough => (Style::Strike, "~~"),
        _ => return CommandResult::NoChange,
    };
    let collapsed = selection.anchor == selection.caret;
    let range = if collapsed {
        let Some((start, word)) = source.unicode_word_indices().find(|(start, word)| {
            *start <= selection.caret && selection.caret <= start + word.len()
        }) else {
            return CommandResult::NoChange;
        };
        start..start + word.len()
    } else {
        selection.anchor.min(selection.caret)..selection.anchor.max(selection.caret)
    };
    if ctx.protected(&range) {
        return CommandResult::NoChange;
    }
    let mut targets = Vec::new();
    for index in ctx.selected_lines(Selection {
        anchor: range.start,
        caret: range.end,
    }) {
        let line = &ctx.lines[index];
        let body = ctx.body_start(line);
        let start = range.start.max(line.start);
        let end = range.end.min(line.content_end);
        if start > line.start && start < body || end > line.start && end < body {
            return CommandResult::NoChange;
        }
        let target = trim(source, start.max(body)..end);
        if target.is_empty() {
            continue;
        }
        let target = normalize(&ctx, target);
        for span in &ctx.spans {
            if delimiter_cut(&target, span) {
                return CommandResult::NoChange;
            }
        }
        targets.push(target);
    }
    if targets.is_empty() {
        return CommandResult::NoChange;
    }
    let bounds = targets[0].start..targets.last().unwrap().end;
    let mut segments: Vec<(Range<usize>, Vec<usize>)> = Vec::new();
    for target in targets {
        let matching: Vec<_> = ctx
            .spans
            .iter()
            .enumerate()
            .filter(|(_, span)| span.style == style && ctx.semantic(span.inner.clone()) == target)
            .map(|(index, _)| index)
            .collect();
        if !matching.is_empty() {
            segments.push((target, matching));
            continue;
        }
        let mut contained: Vec<_> = ctx
            .spans
            .iter()
            .enumerate()
            .filter(|(_, span)| span.style == style && intersects(&span.full, &target))
            .collect();
        if contained
            .iter()
            .any(|(_, span)| span.full.start < target.start || span.full.end > target.end)
        {
            return CommandResult::NoChange;
        }
        contained.sort_by_key(|(_, span)| (span.full.start, std::cmp::Reverse(span.full.end)));
        let mut cursor = target.start;
        for (index, span) in contained {
            if span.full.start < cursor {
                continue;
            }
            let plain = trim(source, cursor..span.full.start);
            if !plain.is_empty() {
                segments.push((ctx.semantic(plain), Vec::new()));
            }
            segments.push((ctx.semantic(span.inner.clone()), vec![index]));
            cursor = span.full.end;
        }
        let plain = trim(source, cursor..target.end);
        if !plain.is_empty() {
            segments.push((ctx.semantic(plain), Vec::new()));
        }
    }
    let remove = segments.iter().all(|(_, spans)| !spans.is_empty());
    let mut changes = Vec::new();
    for (target, spans) in segments {
        if remove {
            for index in spans {
                let span = &ctx.spans[index];
                changes.push(change(span.full.start..span.inner.start, ""));
                changes.push(change(span.inner.end..span.full.end, ""));
            }
        } else if spans.is_empty() {
            changes.push(change(target.start..target.start, delimiter));
            changes.push(Change {
                range: target.end..target.end,
                replacement: delimiter.into(),
                sticky: false,
            });
        }
    }
    changes.sort_by_key(|change| (change.range.start, change.range.end));
    changes.dedup_by(|a, b| a.range == b.range);
    let mapped = if collapsed {
        selection
    } else if selection.anchor < selection.caret {
        Selection {
            anchor: bounds.start,
            caret: bounds.end,
        }
    } else {
        Selection {
            anchor: bounds.end,
            caret: bounds.start,
        }
    };
    edit(source, mapped, changes, false)
}

fn trim(source: &str, range: Range<usize>) -> Range<usize> {
    if range.start >= range.end {
        return range.start..range.start;
    }
    let text = &source[range.clone()];
    let start = range.start + text.len() - text.trim_start_matches([' ', '\t']).len();
    let end = range.start + text.trim_end_matches([' ', '\t']).len();
    start..end.max(start)
}
fn normalize(ctx: &Context<'_>, mut range: Range<usize>) -> Range<usize> {
    loop {
        let Some(span) = ctx.spans.iter().find(|span| {
            (range.start == span.full.start || range.start == span.inner.start)
                && (range.end == span.full.end || range.end == span.inner.end)
                && range != span.inner
        }) else {
            return range;
        };
        range = span.inner.clone();
    }
}
fn delimiter_cut(target: &Range<usize>, span: &Span) -> bool {
    let cuts_byte = |offset| {
        span.full.start < offset && offset < span.inner.start
            || span.inner.end < offset && offset < span.full.end
    };
    cuts_byte(target.start)
        || cuts_byte(target.end)
        || target.start <= span.full.start
            && target.end > span.full.start
            && target.end < span.inner.end
        || target.start > span.inner.start
            && target.start < span.full.end
            && target.end >= span.full.end
}
fn heading(ctx: &Context<'_>, selection: Selection) -> CommandResult {
    let source = ctx.source;
    let line = &ctx.lines[ctx.line_at(selection.caret)];
    let caret = selection.caret..selection.caret + 1;
    // Probe the empty EOF line so the parser distinguishes open fences from closed blocks.
    let protected = if line.start == source.len() {
        Context::new(&format!("{source}x")).protected(&caret)
    } else {
        ctx.protected(&caret)
    };
    if protected || ctx.item(line).is_some() {
        return CommandResult::NoChange;
    }
    let change = if let Some(prefix) = ctx.heading(line) {
        if prefix.len() >= 3 {
            change(
                prefix.start..whitespace_end(source, prefix.end).min(line.content_end),
                "",
            )
        } else {
            change(prefix.start..prefix.start, "#")
        }
    } else {
        let start = whitespace_end(source, quote_end(source, line)).min(line.content_end);
        change(start..start, "# ")
    };
    edit(source, selection, vec![change], false)
}
