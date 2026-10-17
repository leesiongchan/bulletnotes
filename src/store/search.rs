//! Connection-scoped literal FTS phrases and visit-ordered browsing.
use crate::domain::{
    connection::ConnectionId,
    error::StoreError,
    page::{PageKey, PagePath, Permission},
};
use rusqlite::{Connection, params};
#[derive(Clone, Debug)]
pub struct SearchQuery {
    pub connection: ConnectionId,
    pub text: String,
    pub folder: Option<PagePath>,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSummary {
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
    let predicate = "p.connection=?1 AND (?2='' OR substr(p.path,1,length(?2))=?2) AND (?3='' OR p.id IN (SELECT rowid FROM page_fts WHERE page_fts MATCH ?3))";
    let total = db.query_row(
        &format!("SELECT count(*) FROM pages p WHERE {predicate}"),
        params![query.connection.as_str(), folder, phrase],
        |row| row.get::<_, i64>(0),
    )?;
    let (catalogue_total, indexed) = db.query_row(
        "SELECT count(*),count(body) FROM pages WHERE connection=?1",
        [query.connection.as_str()],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    let total = usize::try_from(total).map_err(|_| StoreError::InvalidData)?;
    let indexed = usize::try_from(indexed).map_err(|_| StoreError::InvalidData)?;
    let catalogue_total = usize::try_from(catalogue_total).map_err(|_| StoreError::InvalidData)?;
    let mut statement=db.prepare(&format!("SELECT p.path,p.title,p.body,p.modified_ms,p.permission,p.last_visited_ms FROM pages p WHERE {predicate} ORDER BY p.last_visited_ms IS NULL,p.last_visited_ms DESC,p.path COLLATE BINARY LIMIT ?4 OFFSET ?5"))?;
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
            ))
        },
    )?;
    let mut items = Vec::new();
    for row in rows {
        let (path, title, body, modified_ms, permission, last_visited_ms) = row?;
        let permission = match permission {
            0 => Permission::ReadOnly,
            1 => Permission::ReadWrite,
            _ => return Err(StoreError::InvalidData),
        };
        let excerpt = body.as_ref().map(|text| text.chars().take(160).collect());
        items.push(PageSummary {
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
        complete: indexed == catalogue_total,
    })
}
