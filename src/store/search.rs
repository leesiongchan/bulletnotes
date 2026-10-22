//! Connection-scoped literal FTS phrases and visit-ordered browsing.
use crate::domain::{
    connection::ConnectionId,
    error::StoreError,
    page::{PageKey, PagePath, Permission},
};
use rusqlite::{Connection, params};
mod highlights;
#[derive(Clone, Debug)]
pub struct SearchQuery {
    pub connection: ConnectionId,
    pub text: String,
    pub folder: Option<PagePath>,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchHighlights {
    pub title: Vec<std::ops::Range<usize>>,
    pub path: Vec<std::ops::Range<usize>>,
    pub excerpt: Vec<std::ops::Range<usize>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSummary {
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
    let offset = i64::try_from(query.offset).map_err(|_| StoreError::InvalidQuery)?;
    if !(1..=5000).contains(&query.limit) {
        return Err(StoreError::InvalidQuery);
    }
    let folder = query
        .folder
        .map(|path| format!("{}/", path.as_str()))
        .unwrap_or_default();
    let phrase = if query.text.trim().is_empty() {
        String::new()
    } else {
        format!("\"{}\"", query.text.replace('"', "\"\""))
    };
    let predicate = "p.connection=?1 AND p.remote_deleted=0 AND p.trashed=0 AND (?2='' OR substr(p.path,1,length(?2))=?2) AND (?3='' OR p.id IN (SELECT rowid FROM page_fts WHERE page_fts MATCH ?3))";
    let total = db.query_row(
        &format!("SELECT count(*) FROM pages p WHERE {predicate}"),
        params![query.connection.as_str(), folder, phrase],
        |row| row.get::<_, i64>(0),
    )?;
    let (catalogue_total, indexed) = db.query_row(
        "SELECT count(*),count(body) FROM pages WHERE connection=?1 AND remote_deleted=0 AND trashed=0",
        [query.connection.as_str()],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let total = usize::try_from(total).map_err(|_| StoreError::InvalidData)?;
    let indexed = usize::try_from(indexed).map_err(|_| StoreError::InvalidData)?;
    let catalogue_total = usize::try_from(catalogue_total).map_err(|_| StoreError::InvalidData)?;
    let mut statement=db.prepare(&format!("SELECT p.path,p.title,p.body,p.modified_ms,p.permission,p.last_visited_ms,p.id FROM pages p WHERE {predicate} ORDER BY p.last_visited_ms IS NULL,p.last_visited_ms DESC,p.path COLLATE BINARY LIMIT ?4 OFFSET ?5"))?;
    let rows = statement.query_map(
        params![
            query.connection.as_str(),
            folder,
            phrase,
            query.limit as i64,
            offset
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, i64>(6)?,
            ))
        },
    )?;
    let mut context = if phrase.is_empty() {
        None
    } else {
        Some(db.prepare(
        "SELECT highlight(page_fts,0,?1,?2),highlight(page_fts,1,?1,?2),snippet(page_fts,2,?1,?2,'…',32) FROM page_fts WHERE rowid=?3 AND page_fts MATCH ?4"
    )?)
    };
    let mut items = Vec::new();
    for row in rows {
        let (path, title, body, modified_ms, permission, last_visited_ms, row_id) = row?;
        let permission = match permission {
            0 => Permission::ReadOnly,
            1 => Permission::ReadWrite,
            _ => return Err(StoreError::InvalidData),
        };
        let mut excerpt = body.as_ref().map(|text| text.chars().take(160).collect());
        let mut matches = SearchHighlights::default();
        if let Some(statement) = context.as_mut() {
            let (open, close) = highlights::markers(&title, &path, body.as_deref());
            let (marked_title, marked_path, marked_body) =
                statement.query_row(params![open, close, row_id, phrase], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })?;
            let decoded = highlights::decode(&marked_title, &open, &close)
                .zip(highlights::decode(&marked_path, &open, &close))
                .zip(
                    marked_body
                        .as_deref()
                        .map(|text| highlights::decode(text, &open, &close))
                        .unwrap_or_else(|| Some((String::new(), Vec::new()))),
                );
            if let Some((
                ((plain_title, title_ranges), (plain_path, path_ranges)),
                (plain_body, body_ranges),
            )) = decoded
                && plain_title == title
                && plain_path == path
            {
                matches.title = title_ranges;
                matches.path = path_ranges;
                if body.is_some() && !body_ranges.is_empty() {
                    let (text, ranges) = highlights::visible_excerpt(plain_body, body_ranges);
                    excerpt = Some(text);
                    matches.excerpt = ranges;
                }
            }
        }
        items.push(PageSummary {
            highlights: matches,
            key: PageKey {
                connection: query.connection.clone(),
                path: PagePath::parse(&path).map_err(|_| StoreError::InvalidData)?,
            },
            title,
            excerpt,
            modified_ms: modified_ms
                .map(u64::try_from)
                .transpose()
                .map_err(|_| StoreError::InvalidData)?,
            cached: body.is_some(),
            permission,
            last_visited_ms: last_visited_ms
                .map(u64::try_from)
                .transpose()
                .map_err(|_| StoreError::InvalidData)?,
        });
    }
    Ok(SearchResults {
        items,
        total,
        indexed,
        catalogue_total,
        complete: indexed == catalogue_total
            && db.query_row(
                "SELECT EXISTS(SELECT 1 FROM catalogue_state WHERE connection=?1)",
                [query.connection.as_str()],
                |row| row.get::<_, bool>(0),
            )?,
    })
}
