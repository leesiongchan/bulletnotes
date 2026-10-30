//! Connection-scoped literal substring matching and bounded result batches.
use crate::domain::{
    connection::ConnectionId,
    error::StoreError,
    page::{PageKey, PagePath, Permission},
};
use rusqlite::{Connection, params};
pub(crate) mod highlights;
mod literal;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchScope {
    #[default]
    AllPages,
    Scratchpads,
}
#[derive(Clone, Debug)]
pub struct SearchQuery {
    pub scope: SearchScope,
    pub connection: ConnectionId,
    pub text: String,
    pub folder: Option<PagePath>,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchHighlights {
    pub path: Vec<std::ops::Range<usize>>,
    pub excerpt: Vec<std::ops::Range<usize>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSummary {
    pub conflicted: bool,
    pub highlights: SearchHighlights,
    pub key: PageKey,
    pub title: String,
    pub excerpt: Option<String>,
    pub modified_ms: Option<u64>,
    pub cached: bool,
    pub permission: Permission,
    pub last_visited_ms: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResults {
    pub items: Vec<PageSummary>,
    pub total: usize,
    pub indexed: usize,
    pub catalogue_total: usize,
    pub complete: bool,
}

pub(super) fn search(db: &Connection, query: SearchQuery) -> Result<SearchResults, StoreError> {
    if !(1..=5000).contains(&query.limit) {
        return Err(StoreError::InvalidQuery);
    }
    let matcher = literal::LiteralQuery::new(&query.text)?;
    let folder = if query.scope == SearchScope::Scratchpads {
        String::new()
    } else {
        query
            .folder
            .map(|path| format!("{}/", path.as_str()))
            .unwrap_or_default()
    };
    let (catalogue_total, indexed) = db.query_row(
        "SELECT count(*),count(body) FROM pages WHERE connection=?1 AND remote_deleted=0 AND trashed=0",
        [query.connection.as_str()], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    let order = if query.scope == SearchScope::Scratchpads {
        "s.order_ms DESC,p.path COLLATE BINARY"
    } else {
        "p.last_visited_ms IS NULL,p.last_visited_ms DESC,p.path COLLATE BINARY"
    };
    let mut statement = db.prepare(&format!("SELECT p.path,p.title,p.body,p.modified_ms,p.permission,p.last_visited_ms,EXISTS(SELECT 1 FROM conflicts c WHERE c.connection=p.connection AND c.path=p.path) FROM pages p LEFT JOIN scratchpad_order s ON s.connection=p.connection AND s.path=p.path WHERE p.connection=?1 AND p.remote_deleted=0 AND p.trashed=0 AND (?2='' OR substr(p.path,1,length(?2))=?2) AND (?3=0 OR s.path IS NOT NULL) ORDER BY {order}"))?;
    let mut rows = statement.query(params![
        query.connection.as_str(),
        folder,
        query.scope == SearchScope::Scratchpads
    ])?;
    let mut total = 0usize;
    let mut items = Vec::new();
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        let title: String = row.get(1)?;
        let body: Option<String> = row.get(2)?;
        if let Some(matcher) = &matcher
            && !matcher.is_match(&title)
            && !matcher.is_match(&path)
            && !body.as_deref().is_some_and(|body| matcher.is_match(body))
        {
            continue;
        }
        let retain = total >= query.offset && items.len() < query.limit;
        total += 1;
        if !retain {
            continue;
        }
        let permission = match row.get::<_, i64>(4)? {
            0 => Permission::ReadOnly,
            1 => Permission::ReadWrite,
            _ => return Err(StoreError::InvalidData),
        };
        let mut matches = SearchHighlights::default();
        if let Some(matcher) = &matcher {
            matches.path = matcher.ranges(&path);
        }
        let excerpt = body.as_deref().map(|text| {
            let ranges = matcher.as_ref().map(|m| m.ranges(text)).unwrap_or_default();
            let (excerpt, ranges) = highlights::excerpt(text, &ranges, 320);
            matches.excerpt = ranges;
            excerpt
        });
        items.push(PageSummary {
            highlights: matches,
            key: PageKey {
                connection: query.connection.clone(),
                path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
            },
            title,
            excerpt,
            modified_ms: row
                .get::<_, Option<i64>>(3)?
                .map(u64::try_from)
                .transpose()
                .map_err(|_| StoreError::InvalidData)?,
            cached: body.is_some(),
            permission,
            last_visited_ms: row
                .get::<_, Option<i64>>(5)?
                .map(u64::try_from)
                .transpose()
                .map_err(|_| StoreError::InvalidData)?,
            conflicted: row.get(6)?,
        });
    }
    Ok(SearchResults {
        items,
        total,
        indexed: usize::try_from(indexed).map_err(|_| StoreError::InvalidData)?,
        catalogue_total: usize::try_from(catalogue_total).map_err(|_| StoreError::InvalidData)?,
        complete: indexed == catalogue_total
            && db.query_row(
                "SELECT EXISTS(SELECT 1 FROM catalogue_state WHERE connection=?1)",
                [query.connection.as_str()],
                |r| r.get::<_, bool>(0),
            )?,
    })
}
