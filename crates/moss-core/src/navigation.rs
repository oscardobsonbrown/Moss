use crate::{
    Page, Store,
    store::{decode, invalid},
};
use rusqlite::{OptionalExtension, Result, params};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageCursor {
    pub sort_key: String,
    pub id: String,
}

impl From<&Page> for PageCursor {
    fn from(page: &Page) -> Self {
        Self {
            sort_key: page
                .sidebar_order
                .clone()
                .unwrap_or_else(|| format!("{}:{}", page.created_at, page.id)),
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
const ORDER_KEY: &str = "coalesce(json_extract(p.data,'$.sidebar_order'),json_extract(p.data,'$.created_at') || ':' || p.id)";

fn between(lower: &str, upper: Option<&str>) -> Result<String> {
    if !lower.bytes().all(|c| (32..=126).contains(&c))
        || upper.is_some_and(|s| !s.bytes().all(|c| (32..=126).contains(&c)) || lower >= s)
    {
        return Err(invalid("invalid sidebar page order"));
    }
    let (a, mut b) = (lower.as_bytes(), upper.map(str::as_bytes));
    let mut key = Vec::new();
    let mut i = 0;
    loop {
        let low = a.get(i).copied().unwrap_or(32);
        let high = b.and_then(|v| v.get(i)).copied().unwrap_or(127);
        if high > low + 1 {
            key.push(low + (high - low) / 2);
            break;
        }
        if high < low {
            return Err(invalid("invalid sidebar page order"));
        }
        key.push(low);
        if high == low + 1 {
            b = None;
        }
        i += 1;
    }
    String::from_utf8(key).map_err(|_| invalid("invalid sidebar page order"))
}

fn check_limit(limit: u32) -> Result<()> {
    if (1..=200).contains(&limit) {
        Ok(())
    } else {
        Err(invalid("page limit must be between 1 and 200"))
    }
}

impl Store {
    /// Move a page among its siblings without changing its parent.
    pub fn reorder_sidebar_page(
        &mut self,
        workspace_id: &str,
        id: &str,
        before: Option<&str>,
    ) -> Result<()> {
        self.move_sidebar_page_inner(workspace_id, id, None, before)
    }

    /// Move a page to a new parent and position in one transaction.
    pub fn move_sidebar_page(
        &mut self,
        workspace_id: &str,
        id: &str,
        parent: Option<&str>,
        before: Option<&str>,
    ) -> Result<()> {
        self.move_sidebar_page_inner(workspace_id, id, Some(parent), before)
    }

    fn move_sidebar_page_inner(
        &mut self,
        workspace_id: &str,
        id: &str,
        parent: Option<Option<&str>>,
        before: Option<&str>,
    ) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let page = crate::search::page(&tx, id)?.ok_or_else(|| invalid("page does not exist"))?;
        let parent = parent.unwrap_or(page.parent_id.as_deref());
        if page.workspace_id != workspace_id
            || page.deleted_at.is_some()
            || page.kind == crate::PageKind::Record
        {
            return Err(invalid("page is outside the live workspace"));
        }
        if let Some(parent_id) = parent {
            let target = crate::search::page(&tx, parent_id)?
                .ok_or_else(|| invalid("parent page does not exist"))?;
            if target.workspace_id != workspace_id || target.deleted_at.is_some() {
                return Err(invalid("parent is outside the live workspace"));
            }
            let cycle: bool = tx.query_row(
                "WITH RECURSIVE ancestors(id,parent_id) AS (SELECT id,parent_id FROM pages WHERE id=?1 UNION ALL SELECT p.id,p.parent_id FROM pages p JOIN ancestors a ON p.id=a.parent_id) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id=?2)",
                params![parent_id, id], |row| row.get(0),
            )?;
            if cycle {
                return Err(invalid("parent cycle"));
            }
        }
        if before == Some(id) {
            return if page.parent_id.as_deref() == parent {
                Ok(())
            } else {
                Err(invalid("target is not a sibling"))
            };
        }
        let target = if let Some(id) = before {
            let target = crate::search::page(&tx, id)?
                .ok_or_else(|| invalid("target page does not exist"))?;
            if target.workspace_id != workspace_id
                || target.parent_id.as_deref() != parent
                || target.deleted_at.is_some()
                || target.kind == crate::PageKind::Record
            {
                return Err(invalid("target page is not a live sibling"));
            }
            Some(PageCursor::from(&target))
        } else {
            None
        };
        let range = if target.is_some() {
            format!(" AND ({ORDER_KEY}, p.id) < (?4, ?5)")
        } else {
            String::new()
        };
        let sql = format!(
            "SELECT {ORDER_KEY} {SIBLINGS} AND p.id <> ?3 {range}
            ORDER BY {ORDER_KEY} DESC, p.id DESC LIMIT 1"
        );
        let previous: Option<String> = if let Some(target) = &target {
            tx.query_row(
                &sql,
                params![workspace_id, parent, id, target.sort_key, target.id],
                |row| row.get(0),
            )
            .optional()?
        } else {
            tx.query_row(&sql, params![workspace_id, parent, id], |row| row.get(0))
                .optional()?
        };
        let key = between(
            previous.as_deref().unwrap_or(""),
            target.as_ref().map(|c| c.sort_key.as_str()),
        )?;
        tx.execute(
            "UPDATE pages SET parent_id=?1,data=json_set(data,'$.parent_id',?1,'$.sidebar_order',?2) WHERE id=?3",
            params![parent, key, id],
        )?;
        tx.commit()
    }

    /// Return the subset of `parent_ids` that currently have at least one live non-record child.
    pub fn parents_with_children(
        &self,
        workspace_id: &str,
        parent_ids: &[String],
    ) -> Result<std::collections::BTreeSet<String>> {
        let mut found = std::collections::BTreeSet::new();
        if parent_ids.is_empty() {
            return Ok(found);
        }
        // Keep parameter lists bounded well below SQLite's variable limit.
        for chunk in parent_ids.chunks(200) {
            let placeholders = (0..chunk.len())
                .map(|i| format!("?{}", i + 2))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT DISTINCT parent_id FROM pages
                 WHERE workspace_id=?1 AND deleted_at IS NULL
                   AND json_extract(data,'$.kind') <> 'record'
                   AND parent_id IN ({placeholders})"
            );
            let mut stmt = self.conn.prepare_cached(&sql)?;
            let mut values: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(chunk.len() + 1);
            values.push(&workspace_id);
            for id in chunk {
                values.push(id);
            }
            for row in stmt.query_map(rusqlite::params_from_iter(values), |row| {
                row.get::<_, String>(0)
            })? {
                found.insert(row?);
            }
        }
        Ok(found)
    }

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
                        cursor.map(|c| c.sort_key.as_str()),
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
            // Separate equal keys so both the key and ID have explicit index bounds.
            let mut items = read(
                &format!(" AND {ORDER_KEY}=?3 AND p.id {op} ?4"),
                &format!("p.id {order}"),
                limit + 1,
            )?;
            if items.len() <= limit as usize {
                items.extend(read(
                    &format!(" AND {ORDER_KEY} {op} ?3"),
                    &format!("{ORDER_KEY} {order},p.id {order}"),
                    limit + 1 - items.len() as u32,
                )?);
            }
            items
        } else {
            read("", &format!("{ORDER_KEY} ASC,p.id ASC"), limit + 1)?
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
                SELECT 1 {SIBLINGS} AND {ORDER_KEY}=?3 AND p.id {op} ?4
                UNION ALL SELECT 1 {SIBLINGS} AND {ORDER_KEY} {op} ?3)"
            );
            self.conn.prepare_cached(&sql)?.query_row(
                params![workspace_id, parent_id, cursor.sort_key, cursor.id],
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
