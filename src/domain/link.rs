//! SilverBullet wikilink targets resolved against the local page catalogue.
//!
//! Resolution follows SilverBullet's documented order
//! (<https://docs.silverbullet.md/Link>, accessed 2026-11-05; the page is unversioned):
//! an exact path wins, then a case-insensitive name or folder-suffix lookup (with a
//! choice when several pages fit), and a link that matches nothing names a new page.
use super::page::PagePath;
use std::cmp::Reverse;

/// The page part of a wikilink, without header or position suffixes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkRef {
    name: String,
    path: PagePath,
    absolute: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    Open(PagePath),
    Create(PagePath),
    /// Several pages match equally well, best-ranked first; the user picks one.
    Choose(Vec<PagePath>),
}

/// Where in its page a link points: `Page#Heading`, `Page@123` or `Page@L12C3`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Anchor {
    Heading(String),
    Position(Position),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    /// 0-based character index.
    Offset(usize),
    /// 1-based line and column, counted in characters.
    Line { line: usize, column: usize },
}
impl Position {
    /// Byte offset of this position in `body`, clamped to the end of its line or body.
    pub fn offset_in(self, body: &str) -> usize {
        match self {
            Self::Offset(index) => body
                .char_indices()
                .nth(index)
                .map_or(body.len(), |(offset, _)| offset),
            Self::Line { line, column } => {
                let mut start = 0;
                for _ in 1..line.max(1) {
                    match body[start..].find('\n') {
                        Some(end) => start += end + 1,
                        None => return body.len(),
                    }
                }
                let text = body[start..].split('\n').next().unwrap_or("");
                let text = text.strip_suffix('\r').unwrap_or(text);
                start
                    + text
                        .char_indices()
                        .nth(column.saturating_sub(1))
                        .map_or(text.len(), |(offset, _)| offset)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkError {
    MetaPage,
    Document,
    InvalidPath,
    Unlisted,
}
impl LinkError {
    pub fn notice(self) -> &'static str {
        match self {
            Self::MetaPage => "Meta page links open in SilverBullet only.",
            Self::Document => "Only Markdown pages open in Bulletnotes.",
            Self::InvalidPath => "This link does not name a valid page.",
            Self::Unlisted => {
                "This page may exist remotely. Try the link again after the page list downloads."
            }
        }
    }
}

/// Parse a wikilink destination. `Ok(None)` means the link points at the current page.
pub fn parse(raw: &str) -> Result<Option<LinkRef>, LinkError> {
    let raw = raw.trim();
    if raw.starts_with('^') {
        return Err(LinkError::MetaPage);
    }
    // A header may itself contain `#`, so the first one ends the page name.
    let core = raw.split_once('#').map_or(raw, |(core, _)| core);
    let core = match core.rsplit_once('@') {
        Some((page, position)) if parse_position(position).is_some() => page,
        _ => core,
    }
    .trim();
    let (core, absolute) = match core.strip_prefix('/') {
        Some(rest) => (rest, true),
        None => (core, false),
    };
    if core.is_empty() {
        return if absolute {
            Err(LinkError::InvalidPath)
        } else {
            Ok(None)
        };
    }
    let name = core.strip_suffix(".md").unwrap_or(core);
    let path = PagePath::parse(&format!("{name}.md")).map_err(|_| LinkError::InvalidPath)?;
    Ok(Some(LinkRef {
        name: name.to_owned(),
        path,
        absolute,
    }))
}

/// Where in its page a link points, if anywhere. A header may itself contain `#` or `@`.
pub fn anchor(raw: &str) -> Option<Anchor> {
    let raw = raw.trim();
    if let Some((_, header)) = raw.split_once('#') {
        let header = header.trim();
        return (!header.is_empty()).then(|| Anchor::Heading(header.to_owned()));
    }
    let (_, position) = raw.rsplit_once('@')?;
    parse_position(position).map(Anchor::Position)
}

/// `123` (character offset) or `L12` / `L12C3` (line and column), in either case.
fn parse_position(suffix: &str) -> Option<Position> {
    let number = |s: &str| {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    if let Some(index) = number(suffix) {
        return Some(Position::Offset(index));
    }
    let line = suffix.strip_prefix(['L', 'l'])?;
    let (line, column) = match line.split_once(['C', 'c']) {
        Some((line, column)) => (number(line)?, number(column)?),
        None => (number(line)?, 1),
    };
    Some(Position::Line { line, column })
}

/// Resolve `link`, written on `source`, against `(path, visible)` catalogue rows.
/// `listed` says whether those rows include a complete remote listing; without one a
/// missing page may exist remotely, so it is not created.
///
/// Exact paths match any row, including trashed or remotely deleted pages, so those keep
/// their existing "unavailable" handling instead of being shadowed by a new page. Name
/// lookup only considers visible pages. Candidates spelled exactly like the link win; if
/// several remain, they are returned ranked for the user to choose, as SilverBullet does.
pub fn resolve(
    link: &LinkRef,
    source: &PagePath,
    pages: &[(PagePath, bool)],
    listed: bool,
) -> Result<Resolution, LinkError> {
    if pages.iter().any(|(path, _)| *path == link.path) {
        return Ok(Resolution::Open(link.path.clone()));
    }
    if !link.absolute {
        // Compare whole trailing folder segments, so `api/Auth` never matches `webapi/Auth`.
        let segments = link.name.split('/').count();
        let tail = |path: &PagePath| {
            let parts: Vec<&str> = path.as_str().strip_suffix(".md")?.split('/').collect();
            Some(parts[parts.len().checked_sub(segments)?..].join("/"))
        };
        let wanted = link.name.to_lowercase();
        let candidates: Vec<(&PagePath, String)> = pages
            .iter()
            .filter(|(_, visible)| *visible)
            .filter_map(|(path, _)| tail(path).map(|tail| (path, tail)))
            .filter(|(_, tail)| tail.to_lowercase() == wanted)
            .collect();
        let spelled = candidates.iter().any(|(_, tail)| *tail == link.name);
        let source_folder = folder(source.as_str());
        let mut pool: Vec<&PagePath> = candidates
            .into_iter()
            .filter(|(_, tail)| !spelled || *tail == link.name)
            .map(|(path, _)| path)
            .collect();
        pool.sort_by_key(|path| {
            (
                Reverse(shared_depth(folder(path.as_str()), source_folder)),
                path.as_str().matches('/').count(),
                path.as_str(),
            )
        });
        match pool.as_slice() {
            [] => {}
            [only] => return Ok(Resolution::Open((*only).clone())),
            ranked => {
                return Ok(Resolution::Choose(
                    ranked.iter().map(|path| (*path).clone()).collect(),
                ));
            }
        }
    }
    if looks_like_document(&link.name) {
        return Err(LinkError::Document);
    }
    if !listed {
        return Err(LinkError::Unlisted);
    }
    Ok(Resolution::Create(link.path.clone()))
}

/// Whether the link `raw`, written on `source`, can lead to `target`: it opens the
/// page, or the page is one of an ambiguous link's choices.
pub fn links_to(
    raw: &str,
    source: &PagePath,
    target: &PagePath,
    pages: &[(PagePath, bool)],
) -> bool {
    let Ok(Some(link)) = parse(raw) else {
        return false;
    };
    match resolve(&link, source, pages, true) {
        Ok(Resolution::Open(path)) => path == *target,
        Ok(Resolution::Choose(paths)) => paths.contains(target),
        _ => false,
    }
}

/// Whether following `raw` would create a page. Without a complete listing nothing
/// counts as missing, because the page may exist remotely.
pub fn dangles(raw: &str, pages: &[(PagePath, bool)], listed: bool) -> bool {
    matches!(parse(raw), Ok(Some(link))
        if matches!(resolve(&link, &link.path, pages, listed), Ok(Resolution::Create(_))))
}

/// An unfinished `[[name` ending at the caret, on the caret's line.
#[derive(Debug, PartialEq, Eq)]
pub struct Completion {
    /// Byte offset just after `[[`; a chosen name replaces `start..caret`.
    pub start: usize,
    /// The name typed so far.
    pub query: String,
}

/// Characters that end a link name: brackets, alias, header, position and newline.
const NAME_END: [char; 6] = [']', '[', '|', '#', '@', '\n'];

/// The link name being typed at `caret`. A name the caret sits in that already
/// continues to `]]`, an alias, a header or a position is a finished link and is
/// left alone.
pub fn completion_at(source: &str, caret: usize) -> Option<Completion> {
    let before = source.get(..caret)?;
    let line = &before[before.rfind('\n').map_or(0, |at| at + 1)..];
    let start = caret - line.len() + line.rfind("[[")? + 2;
    let query = &source[start..caret];
    if query.contains(NAME_END) || query.trim().is_empty() {
        return None;
    }
    let rest = &source[caret..];
    if rest
        .find(NAME_END)
        .is_some_and(|at| matches!(rest.as_bytes()[at], b']' | b'|' | b'#' | b'@'))
    {
        return None;
    }
    Some(Completion {
        start,
        query: query.trim_start().to_owned(),
    })
}

/// Page names matching `query`, best first: a name prefix, then a folder or word
/// prefix, then any substring, each preferring shallower and shorter names.
pub fn suggest(query: &str, pages: &[(PagePath, bool)], limit: usize) -> Vec<String> {
    let wanted = query.to_lowercase();
    let mut ranked: Vec<(u8, usize, &str)> = pages
        .iter()
        .filter(|(_, visible)| *visible)
        .filter_map(|(path, _)| path.as_str().strip_suffix(".md"))
        .filter_map(|name| {
            let lower = name.to_lowercase();
            let tier = if lower.starts_with(&wanted) {
                0
            } else if lower
                .match_indices(&wanted)
                .any(|(at, _)| lower[..at].ends_with(['/', ' ', '-', '_']))
            {
                1
            } else if lower.contains(&wanted) {
                2
            } else {
                return None;
            };
            Some((tier, name.matches('/').count(), name))
        })
        .collect();
    ranked.sort_by(|a, b| (a.0, a.1, a.2.len(), a.2).cmp(&(b.0, b.1, b.2.len(), b.2)));
    ranked
        .into_iter()
        .take(limit)
        .map(|(.., name)| name.to_owned())
        .collect()
}

fn folder(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

fn shared_depth(a: &str, b: &str) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    a.split('/')
        .zip(b.split('/'))
        .take_while(|(a, b)| a == b)
        .count()
}

/// `report.pdf` names a document, while `v1.2` or `Mr. Smith` still name pages.
fn looks_like_document(name: &str) -> bool {
    let last = name.rsplit('/').next().unwrap_or(name);
    last.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && (1..=5).contains(&ext.len())
            && ext.bytes().all(|b| b.is_ascii_alphanumeric())
            && ext.bytes().any(|b| b.is_ascii_alphabetic())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> PagePath {
        PagePath::parse(value).unwrap()
    }
    fn link(raw: &str) -> LinkRef {
        parse(raw).unwrap().unwrap()
    }
    fn open(raw: &str, source: &str, pages: &[(&str, bool)]) -> Result<Resolution, LinkError> {
        let pages: Vec<_> = pages.iter().map(|(p, v)| (path(p), *v)).collect();
        resolve(&link(raw), &path(source), &pages, true)
    }

    #[test]
    fn suffixes_and_current_page_links_are_understood() {
        assert_eq!(link("Daily Notes").path, path("Daily Notes.md"));
        assert_eq!(link(" Plan#Goals#2026 ").path, path("Plan.md"));
        assert_eq!(link("CHANGELOG@123").path, path("CHANGELOG.md"));
        assert_eq!(link("CHANGELOG@L12C42").path, path("CHANGELOG.md"));
        assert_eq!(link("me@example").path, path("me@example.md"));
        assert_eq!(link("Page.md").path, path("Page.md"));
        assert!(link("/Inbox").absolute);
        assert_eq!(parse("#Heading"), Ok(None));
        assert_eq!(parse("@5"), Ok(None));
        assert_eq!(parse(""), Ok(None));
    }

    #[test]
    fn anchors_name_a_heading_or_a_position() {
        let heading = |text: &str| Some(Anchor::Heading(text.into()));
        assert_eq!(anchor("Plan#Goals"), heading("Goals"));
        assert_eq!(anchor(" #Intro "), heading("Intro"));
        assert_eq!(anchor("Plan#C# notes @ 9"), heading("C# notes @ 9"));
        assert_eq!(anchor("Plan"), None);
        assert_eq!(anchor("Plan#"), None);
        assert_eq!(anchor("me@example"), None);
        assert_eq!(
            anchor("CHANGELOG@123"),
            Some(Anchor::Position(Position::Offset(123)))
        );
        assert_eq!(
            anchor("CHANGELOG@l12C42"),
            Some(Anchor::Position(Position::Line {
                line: 12,
                column: 42
            }))
        );
        assert_eq!(
            anchor("@L3"),
            Some(Anchor::Position(Position::Line { line: 3, column: 1 }))
        );
    }

    #[test]
    fn positions_map_to_byte_offsets_and_clamp() {
        let body = "a😀b\r\nsecond line\nthird";
        assert_eq!(Position::Offset(0).offset_in(body), 0);
        assert_eq!(Position::Offset(2).offset_in(body), 5, "after the emoji");
        assert_eq!(Position::Offset(999).offset_in(body), body.len());
        let line = |line, column| Position::Line { line, column }.offset_in(body);
        assert_eq!(line(1, 1), 0);
        assert_eq!(line(1, 99), 6, "clamped before CRLF");
        assert_eq!(line(2, 1), body.find("second").unwrap());
        assert_eq!(line(2, 8), body.find("line").unwrap());
        assert_eq!(line(3, 3), body.find("ird").unwrap());
        assert_eq!(line(9, 1), body.len());
        assert_eq!(line(0, 0), 0);
    }

    #[test]
    fn unsupported_targets_are_refused_with_a_reason() {
        assert_eq!(parse("^Library/Std"), Err(LinkError::MetaPage));
        assert_eq!(parse("/"), Err(LinkError::InvalidPath));
        assert_eq!(parse("a/../b"), Err(LinkError::InvalidPath));
        assert_eq!(parse("a//b"), Err(LinkError::InvalidPath));
        assert_eq!(parse("a\\b"), Err(LinkError::InvalidPath));
        assert_eq!(open("report.pdf", "a.md", &[]), Err(LinkError::Document));
        assert_eq!(
            open("v1.2", "a.md", &[]),
            Ok(Resolution::Create(path("v1.2.md")))
        );
        assert_eq!(
            open("Mr. Smith", "a.md", &[]),
            Ok(Resolution::Create(path("Mr. Smith.md")))
        );
    }

    #[test]
    fn missing_pages_are_only_created_after_a_complete_listing() {
        let pages = [(path("folder/Note.md"), true)];
        let resolve = |raw, listed| resolve(&link(raw), &path("a.md"), &pages, listed);
        assert_eq!(
            resolve("note", false),
            Ok(Resolution::Open(path("folder/Note.md")))
        );
        assert_eq!(resolve("Other", false), Err(LinkError::Unlisted));
        assert_eq!(
            resolve("Other", true),
            Ok(Resolution::Create(path("Other.md")))
        );
    }

    #[test]
    fn exact_paths_win_even_when_hidden() {
        let pages = [("Note.md", false), ("folder/Note.md", true)];
        assert_eq!(
            open("Note", "folder/a.md", &pages),
            Ok(Resolution::Open(path("Note.md")))
        );
    }

    #[test]
    fn names_and_folder_suffixes_find_nested_pages() {
        let pages = [
            ("docs/api/Auth.md", true),
            ("webapi/Auth.md", true),
            ("some/folder/Note.md", true),
        ];
        assert_eq!(
            open("note", "x.md", &pages),
            Ok(Resolution::Open(path("some/folder/Note.md")))
        );
        assert_eq!(
            open("api/Auth", "x.md", &pages),
            Ok(Resolution::Open(path("docs/api/Auth.md")))
        );
        assert_eq!(
            open("/Note", "x.md", &pages),
            Ok(Resolution::Create(path("Note.md")))
        );
        assert_eq!(
            open("Note", "x.md", &[("some/folder/Note.md", false)]),
            Ok(Resolution::Create(path("Note.md")))
        );
    }

    #[test]
    fn exact_spelling_settles_a_name_and_remaining_ties_are_ranked_choices() {
        let pages = [
            ("a/Tag.md", true),
            ("b/tag.md", true),
            ("b/c/Tag.md", true),
            ("d/e/Tag.md", true),
        ];
        assert_eq!(
            open("tag", "z.md", &pages),
            Ok(Resolution::Open(path("b/tag.md")))
        );
        let choices = |source| match open("Tag", source, &pages) {
            Ok(Resolution::Choose(paths)) => paths,
            other => panic!("expected choices, got {other:?}"),
        };
        let ranked = |paths: &[&str]| paths.iter().map(|p| path(p)).collect::<Vec<_>>();
        // Nearest shared folder first, then the shallowest path, then alphabetical.
        assert_eq!(
            choices("b/c/x.md"),
            ranked(&["b/c/Tag.md", "a/Tag.md", "d/e/Tag.md"])
        );
        assert_eq!(
            choices("z.md"),
            ranked(&["a/Tag.md", "b/c/Tag.md", "d/e/Tag.md"])
        );
        assert_eq!(
            choices("d/x.md"),
            ranked(&["d/e/Tag.md", "a/Tag.md", "b/c/Tag.md"])
        );
    }

    #[test]
    fn backlinks_count_links_that_open_or_may_mean_the_page() {
        let pages: Vec<_> = ["Projects/Plan.md", "Home/Notes.md", "Work/Notes.md"]
            .iter()
            .map(|p| (path(p), true))
            .collect();
        let to = |raw, target| links_to(raw, &path("a.md"), &path(target), &pages);
        assert!(to("plan#Goals", "Projects/Plan.md"));
        assert!(to("/Projects/Plan@3", "Projects/Plan.md"));
        assert!(
            to("Notes", "Work/Notes.md"),
            "one of an ambiguous link's choices"
        );
        assert!(!to("Home/Notes", "Work/Notes.md"));
        assert!(!to("Someday", "Projects/Plan.md"));
        assert!(
            !to("#Top", "a.md"),
            "a link within the page is not a backlink"
        );
    }

    #[test]
    fn only_targets_a_complete_listing_lacks_dangle() {
        let pages = [(path("Projects/Plan.md"), true), (path("Old.md"), false)];
        assert!(!dangles("plan", &pages, true));
        assert!(
            !dangles("Old", &pages, true),
            "an exact hidden path still opens"
        );
        assert!(dangles("Someday", &pages, true));
        assert!(!dangles("Someday", &pages, false), "may exist remotely");
        assert!(!dangles("report.pdf", &pages, true));
        assert!(!dangles("#Heading", &pages, true));
        assert!(!dangles("^Meta", &pages, true));
    }

    #[test]
    fn completion_covers_an_unfinished_name_before_the_caret() {
        let at = |source: &str| {
            let caret = source.find('|').unwrap();
            let source = source.replacen('|', "", 1);
            completion_at(&source, caret).map(|c| (c.start, c.query))
        };
        assert_eq!(at("See [[Pro|"), Some((6, "Pro".into())));
        assert_eq!(at("See [[Pro| and [[x]]"), Some((6, "Pro".into())));
        assert_eq!(at("[[Pr|\nnext]]"), Some((2, "Pr".into())));
        assert_eq!(at("[[a]] then [[b c|"), Some((13, "b c".into())));
        assert_eq!(at("[[|"), None, "nothing typed yet");
        assert_eq!(at("[[ |"), None);
        assert_eq!(at("[[Pro|]]"), None, "a finished link");
        assert_eq!(at("[[Pr|o]]"), None);
        assert_eq!(at("[[Pr|o#Top]]"), None);
        assert_eq!(at("[[Pr|o|label]]"), None);
        assert_eq!(at("[[a]] tail|"), None);
        assert_eq!(at("[[a\nb|"), None, "links stay on one line");
        assert_eq!(at("[[é|"), Some((2, "é".into())));
    }

    #[test]
    fn suggestions_prefer_prefixes_then_word_starts_then_substrings() {
        let pages: Vec<_> = [
            ("Projects/Plan.md", true),
            ("Plan.md", true),
            ("Work/Planning notes.md", true),
            ("Airplane.md", true),
            ("Hidden plan.md", false),
            ("plan.pdf", true),
            ("Other.md", true),
        ]
        .iter()
        .map(|(p, v)| (path(p), *v))
        .collect();
        assert_eq!(
            suggest("plan", &pages, 8),
            ["Plan", "Projects/Plan", "Work/Planning notes", "Airplane"]
        );
        assert_eq!(suggest("PROJ", &pages, 8), ["Projects/Plan"]);
        assert_eq!(suggest("plan", &pages, 2), ["Plan", "Projects/Plan"]);
        assert!(suggest("zzz", &pages, 8).is_empty());
    }
}
