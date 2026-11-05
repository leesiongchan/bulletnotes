//! SilverBullet wikilink targets resolved against the local page catalogue.
//!
//! Resolution follows SilverBullet's documented order
//! (<https://docs.silverbullet.md/Link>, accessed 2026-11-05; the page is unversioned):
//! an exact path wins, then a case-insensitive name or folder-suffix lookup, and a
//! link that matches nothing names a new page at the space root.
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
        Some((page, position)) if is_position(position) => page,
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

/// `@123` (character offset) or `@L12` / `@L12C3` (line and column), in either case.
fn is_position(suffix: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if digits(suffix) {
        return true;
    }
    let Some(line) = suffix.strip_prefix(['L', 'l']) else {
        return false;
    };
    match line.split_once(['C', 'c']) {
        Some((line, column)) => digits(line) && digits(column),
        None => digits(line),
    }
}

/// Resolve `link`, written on `source`, against `(path, visible)` catalogue rows.
/// `listed` says whether those rows include a complete remote listing; without one a
/// missing page may exist remotely, so it is not created.
///
/// Exact paths match any row, including trashed or remotely deleted pages, so those keep
/// their existing "unavailable" handling instead of being shadowed by a new page. Name
/// lookup only considers visible pages. Where SilverBullet asks the user to pick between
/// equally good candidates, this picks its first-ranked candidate.
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
        if let Some((best, _)) = candidates
            .into_iter()
            .filter(|(_, tail)| !spelled || *tail == link.name)
            .min_by_key(|(path, _)| {
                (
                    Reverse(shared_depth(folder(path.as_str()), source_folder)),
                    path.as_str().matches('/').count(),
                    path.as_str(),
                )
            })
        {
            return Ok(Resolution::Open(best.clone()));
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
    fn ambiguous_names_prefer_spelling_then_proximity_then_depth() {
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
        assert_eq!(
            open("Tag", "b/c/x.md", &pages),
            Ok(Resolution::Open(path("b/c/Tag.md")))
        );
        assert_eq!(
            open("Tag", "z.md", &pages),
            Ok(Resolution::Open(path("a/Tag.md")))
        );
        assert_eq!(
            open("Tag", "d/x.md", &pages),
            Ok(Resolution::Open(path("d/e/Tag.md")))
        );
    }
}
