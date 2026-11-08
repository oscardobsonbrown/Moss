use crate::{
    Page, Store,
    store::{decode, invalid},
};
use rusqlite::{Result, params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageCursor {
    pub created_at: String,
    pub id: String,
}

impl From<&Page> for PageCursor {
    fn from(page: &Page) -> Self {
        Self {
            created_at: page.created_at.clone(),
            id: page.id.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum PagePosition<'a> {
    Start,
    After(&'a PageCursor),
    Before(&'a PageCursor),
}

#[derive(Clone, Debug)]
pub struct PageChild {
    pub page: Page,
    pub has_children: bool,
}

#[derive(Clone, Debug)]
pub struct PageChildren {
    pub items: Vec<PageChild>,
    pub previous: Option<PageCursor>,
    pub next: Option<PageCursor>,
}

const SIBLINGS: &str = "FROM pages p WHERE p.workspace_id=?1 AND p.parent_id IS ?2
    AND p.deleted_at IS NULL AND json_extract(p.data,'$.kind') <> 'record'";
const CREATED: &str = "json_extract(p.data,'$.created_at')";

fn check_limit(limit: u32) -> Result<()> {
    if (1..=200).contains(&limit) {
        Ok(())
    } else {
        Err(invalid("page limit must be between 1 and 200"))
    }
}

impl Store {
    /// Read one bounded sibling page. Pass `previous` to Before and `next` to After.
    pub fn page_children(
        &self,
        workspace_id: &str,
        parent_id: Option<&str>,
        position: PagePosition<'_>,
        limit: u32,
    ) -> Result<PageChildren> {
        check_limit(limit)?;
        if let Some(parent_id) = parent_id {
            let parent = self
                .get_page(parent_id)?
                .ok_or_else(|| invalid("parent does not exist"))?;
            if parent.workspace_id != workspace_id || parent.deleted_at.is_some() {
                return Err(invalid("parent is outside the live workspace"));
            }
        }
        let (descending, cursor) = match position {
            PagePosition::Start => (false, None),
            PagePosition::After(cursor) => (false, Some(cursor)),
            PagePosition::Before(cursor) => (true, Some(cursor)),
        };
        let order = if descending { "DESC" } else { "ASC" };
        let read = |range: &str, sort: &str, count: u32| -> Result<Vec<PageChild>> {
            let sql = format!(
                "SELECT p.data, EXISTS(SELECT 1 FROM pages c
            WHERE c.workspace_id=?1 AND c.parent_id=p.id AND c.deleted_at IS NULL
              AND json_extract(c.data,'$.kind') <> 'record')
            {SIBLINGS}{range} ORDER BY {sort} LIMIT ?5"
            );
            self.conn
                .prepare_cached(&sql)?
                .query_map(
                    params![
                        workspace_id,
                        parent_id,
                        cursor.map(|c| c.created_at.as_str()),
                        cursor.map(|c| c.id.as_str()),
                        count
                    ],
                    |row| {
                        Ok(PageChild {
                            page: decode(row.get(0)?)?,
                            has_children: row.get(1)?,
                        })
                    },
                )?
                .collect()
        };
        let mut items = if cursor.is_some() {
            let op = if descending { "<" } else { ">" };
            // Separate equal timestamps so both the timestamp and ID have explicit index bounds.
            let mut items = read(
                &format!(" AND {CREATED}=?3 AND p.id {op} ?4"),
                &format!("p.id {order}"),
                limit + 1,
            )?;
            if items.len() <= limit as usize {
                items.extend(read(
                    &format!(" AND {CREATED} {op} ?3"),
                    &format!("{CREATED} {order},p.id {order}"),
                    limit + 1 - items.len() as u32,
                )?);
            }
            items
        } else {
            read("", &format!("{CREATED} ASC,p.id ASC"), limit + 1)?
        };
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        if descending {
            items.reverse();
        }
        let first = items.first().map(|item| PageCursor::from(&item.page));
        let last = items.last().map(|item| PageCursor::from(&item.page));
        let exists = |cursor: &PageCursor, op: &str| -> Result<bool> {
            let sql = format!(
                "SELECT EXISTS(
                SELECT 1 {SIBLINGS} AND {CREATED}=?3 AND p.id {op} ?4
                UNION ALL SELECT 1 {SIBLINGS} AND {CREATED} {op} ?3)"
            );
            self.conn.prepare_cached(&sql)?.query_row(
                params![workspace_id, parent_id, cursor.created_at, cursor.id],
                |row| row.get(0),
            )
        };
        let previous = match (position, first) {
            (PagePosition::Start, _) | (_, None) => None,
            (PagePosition::Before(_), first) => first.filter(|_| more),
            (_, Some(first)) => {
                if exists(&first, "<")? {
                    Some(first)
                } else {
                    None
                }
            }
        };
        let next = match (descending, last) {
            (_, None) => None,
            (false, last) => last.filter(|_| more),
            (true, Some(last)) => {
                if exists(&last, ">")? {
                    Some(last)
                } else {
                    None
                }
            }
        };
        Ok(PageChildren {
            items,
            previous,
            next,
        })
    }

    /// Keep empty-search suggestions independent of the loaded sidebar branches.
    pub fn page_suggestions(&self, workspace_id: &str, limit: u32) -> Result<Vec<Page>> {
        check_limit(limit)?;
        self.conn
            .prepare_cached(
                "SELECT data FROM pages WHERE workspace_id=?1
            AND deleted_at IS NULL AND json_extract(data,'$.kind') <> 'record'
            ORDER BY json_extract(data,'$.created_at'),id LIMIT ?2",
            )?
            .query_map(params![workspace_id, limit], |row| decode(row.get(0)?))?
            .collect()
    }
}
