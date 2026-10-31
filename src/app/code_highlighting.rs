//! Bounded, local syntax coloring. Markdown remains the source of truth.
use crate::{
    UiTheme,
    theme::{Color, contrast::contrast_ratio},
};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use slint::language::TextInputDecoration;
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

const BLOCK_BUDGET: usize = 32 * 1024;
const NOTE_BUDGET: usize = 128 * 1024;
thread_local! {
    static RESOURCES: (SyntaxSet, ThemeSet) = (SyntaxSet::load_defaults_newlines(), ThemeSet::load_defaults());
}

struct Block {
    range: std::ops::Range<usize>,
    language: String,
    bodies: Vec<std::ops::Range<usize>>,
}
struct CachedBlock {
    source: String,
    language: String,
    bodies: Vec<std::ops::Range<usize>>,
    palette: UiTheme,
    spans: Vec<TextInputDecoration>,
}
thread_local! {
    static BLOCK_CACHE: std::cell::RefCell<std::collections::VecDeque<CachedBlock>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

fn highlight_block(source: &str, palette: &UiTheme, block: &Block) -> Vec<TextInputDecoration> {
    let snippet = &source[block.range.clone()];
    if let Some(spans) = BLOCK_CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|entry| {
                entry.source == snippet
                    && entry.language == block.language
                    && entry.bodies == block.bodies
                    && entry.palette == *palette
            })
            .map(|entry| entry.spans.clone())
    }) {
        return spans;
    }
    let spans = RESOURCES.with(|(syntaxes, themes)| {
        let Some(syntax) = syntaxes.find_syntax_by_token(&block.language) else {
            return Vec::new();
        };
        let theme = &themes.themes[if palette.dark {
            "base16-ocean.dark"
        } else {
            "InspiredGitHub"
        }];
        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut result = Vec::new();
        let background = Color {
            r: palette.surface.red(),
            g: palette.surface.green(),
            b: palette.surface.blue(),
        };
        for range in &block.bodies {
            let mut offset = range.start;
            for line in LinesWithEndings::from(&snippet[range.clone()]) {
                let Ok(tokens) = highlighter.highlight_line(line, syntaxes) else {
                    offset += line.len();
                    continue;
                };
                for (style, text) in tokens {
                    let color = Color {
                        r: style.foreground.r,
                        g: style.foreground.g,
                        b: style.foreground.b,
                    };
                    let foreground = if contrast_ratio(color, background) >= 4.5 {
                        slint::Color::from_rgb_u8(color.r, color.g, color.b)
                    } else {
                        palette.ink
                    };
                    let end = offset + text.trim_end_matches(['\r', '\n']).len();
                    if offset < end {
                        let mut span = TextInputDecoration::default();
                        span.start = offset as i32;
                        span.end = end as i32;
                        span.foreground = foreground;
                        result.push(span);
                    }
                    offset += text.len();
                }
            }
        }
        result
    });
    BLOCK_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let mut bytes = snippet.len() + cache.iter().map(|entry| entry.source.len()).sum::<usize>();
        while cache.len() >= 64 || bytes > NOTE_BUDGET {
            let Some(removed) = cache.pop_front() else {
                break;
            };
            bytes -= removed.source.len();
        }
        cache.push_back(CachedBlock {
            source: snippet.to_owned(),
            language: block.language.clone(),
            bodies: block.bodies.clone(),
            palette: palette.clone(),
            spans: spans.clone(),
        });
    });
    spans
}

pub(super) fn tokens(
    source: &str,
    palette: &UiTheme,
    high_contrast: bool,
) -> Vec<TextInputDecoration> {
    if high_contrast
        || source.len() > NOTE_BUDGET
        || (!source.contains("```") && !source.contains("~~~"))
    {
        return Vec::new();
    }
    let mut block: Option<Block> = None;
    let mut result = Vec::new();
    for (event, range) in Parser::new_ext(
        source,
        pulldown_cmark::Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
    )
    .into_offset_iter()
    {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                block = info
                    .split_whitespace()
                    .next()
                    .filter(|_| range.len() <= BLOCK_BUDGET)
                    .map(|language| Block {
                        range,
                        language: language.to_owned(),
                        bodies: Vec::new(),
                    });
            }
            Event::Text(_) => {
                if let Some(block) = block.as_mut() {
                    block
                        .bodies
                        .push(range.start - block.range.start..range.end - block.range.start);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(block) = block.take() {
                    let shift = block.range.start as i32;
                    result.extend(highlight_block(source, palette, &block).into_iter().map(
                        |mut span| {
                            span.start += shift;
                            span.end += shift;
                            span
                        },
                    ));
                }
            }
            _ => {}
        }
    }
    result
}

pub(super) fn block_ranges(source: &str) -> Vec<std::ops::Range<usize>> {
    Parser::new_ext(
        source,
        pulldown_cmark::Options::ENABLE_YAML_STYLE_METADATA_BLOCKS,
    )
    .into_offset_iter()
    .filter_map(|(event, range)| matches!(event, Event::Start(Tag::CodeBlock(_))).then_some(range))
    .collect()
}

pub(super) fn layout_end(source: &str, end: usize) -> usize {
    match source.as_bytes().get(end) {
        Some(b'\r') if source.as_bytes().get(end + 1) == Some(&b'\n') => end + 2,
        Some(b'\n') => end + 1,
        None if !source.ends_with('\n') => end + 1,
        _ => end,
    }
}
