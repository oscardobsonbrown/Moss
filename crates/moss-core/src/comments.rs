use crate::{
    Block, Database, Store, new_id, now, search,
    store::{decode, encode, invalid},
};
use rusqlite::{Connection, OptionalExtension, Result, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommentAnchor {
    Page,
    Block {
        block_id: String,
    },
    Text {
        block_id: String,
        start: usize,
        end: usize,
        quote: String,
    },
    Cell {
        record_id: String,
        property_id: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommentMessage {
    pub id: String,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub text: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommentThread {
    pub id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub anchor: CommentAnchor,
    pub resolved: bool,
    pub created_at: String,
    pub messages: Vec<CommentMessage>,
    pub anchor_available: bool,
}

pub(crate) fn install_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS comment_threads(\
        id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,\
        page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE, anchor TEXT NOT NULL CHECK(json_valid(anchor)),\
        resolved INTEGER NOT NULL DEFAULT 0 CHECK(resolved IN (0,1)), created_at TEXT NOT NULL,\
        orphaned INTEGER NOT NULL DEFAULT 0 CHECK(orphaned IN (0,1)));\
        CREATE INDEX IF NOT EXISTS comment_threads_page ON comment_threads(workspace_id,page_id,created_at,id);\
        CREATE TABLE IF NOT EXISTS comment_messages(\
        id TEXT PRIMARY KEY, thread_id TEXT NOT NULL REFERENCES comment_threads(id) ON DELETE CASCADE,\
        author TEXT NOT NULL, body TEXT NOT NULL, created_at TEXT NOT NULL);\
        CREATE INDEX IF NOT EXISTS comment_messages_thread ON comment_messages(thread_id,created_at,id);")?;
    let has_orphaned = conn
        .prepare("PRAGMA table_info(comment_threads)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<Vec<_>>>()?
        .iter()
        .any(|name| name == "orphaned");
    if !has_orphaned {
        conn.execute("ALTER TABLE comment_threads ADD COLUMN orphaned INTEGER NOT NULL DEFAULT 0 CHECK(orphaned IN (0,1))", [])?;
    }
    let has_kind = conn
        .prepare("PRAGMA table_info(comment_messages)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<Vec<_>>>()?
        .iter()
        .any(|name| name == "kind");
    if !has_kind {
        conn.execute("ALTER TABLE comment_messages ADD COLUMN kind TEXT CHECK(kind IS NULL OR kind IN ('person','agent'))", [])?;
    }
    Ok(())
}

/// Update comments before replacing a page's blocks in the same transaction.
pub(crate) fn update_block_anchors(
    conn: &Connection,
    page_id: &str,
    saved: &[Block],
) -> Result<()> {
    use std::collections::HashMap;
    let current: HashMap<_, _> = {
        let mut stmt = conn.prepare("SELECT data FROM blocks WHERE page_id=?1")?;
        let blocks = stmt
            .query_map([page_id], |r| r.get::<_, String>(0))?
            .map(|r| decode::<Block>(r?))
            .collect::<Result<Vec<_>>>()?;
        blocks
            .into_iter()
            .map(|block| (block.id.clone(), block))
            .collect()
    };
    let updated: HashMap<_, _> = saved
        .iter()
        .map(|block| (block.id.as_str(), block))
        .collect();
    let threads = {
        let mut stmt =
            conn.prepare("SELECT id,anchor FROM comment_threads WHERE page_id=?1 AND orphaned=0")?;
        let rows = stmt.query_map([page_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>>>()?
    };
    for (id, raw) in threads {
        let mut anchor: CommentAnchor = decode(raw)?;
        let (block_id, text_range) = match &anchor {
            CommentAnchor::Block { block_id } => (block_id.clone(), false),
            CommentAnchor::Text { block_id, .. } => (block_id.clone(), true),
            _ => continue,
        };
        let (Some(old), Some(new)) = (current.get(&block_id), updated.get(block_id.as_str()))
        else {
            conn.execute("UPDATE comment_threads SET orphaned=1 WHERE id=?1", [&id])?;
            continue;
        };
        if !text_range || old.text == new.text {
            continue;
        }
        if let CommentAnchor::Text {
            start, end, quote, ..
        } = &mut anchor
        {
            if old.text.get(*start..*end) != Some(quote.as_str())
                || old.text.matches(quote.as_str()).count() != 1
                || new.text.matches(quote.as_str()).count() != 1
            {
                conn.execute("UPDATE comment_threads SET orphaned=1 WHERE id=?1", [&id])?;
                continue;
            }
            let prefix: usize = old
                .text
                .chars()
                .zip(new.text.chars())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a.len_utf8())
                .sum();
            let suffix: usize = old.text[prefix..]
                .chars()
                .rev()
                .zip(new.text[prefix..].chars().rev())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a.len_utf8())
                .sum();
            let old_end = old.text.len() - suffix;
            let new_end = new.text.len() - suffix;
            if *end <= prefix {
                continue;
            } else if *start >= old_end {
                *start = new_end + (*start - old_end);
                *end = new_end + (*end - old_end);
                if new.text.get(*start..*end) == Some(quote.as_str()) {
                    conn.execute(
                        "UPDATE comment_threads SET anchor=?1 WHERE id=?2",
                        params![encode(&anchor)?, id],
                    )?;
                    continue;
                }
            }
            conn.execute("UPDATE comment_threads SET orphaned=1 WHERE id=?1", [&id])?;
        }
    }
    Ok(())
}

fn valid_message(author: &str, text: &str) -> Result<()> {
    if author.trim().is_empty()
        || author.len() > 256
        || text.trim().is_empty()
        || text.len() > 65_536
    {
        return Err(invalid("comment author or text is invalid"));
    }
    Ok(())
}

fn available(
    conn: &Connection,
    workspace_id: &str,
    page_id: &str,
    anchor: &CommentAnchor,
) -> Result<bool> {
    let Some(page) = search::page(conn, page_id)? else {
        return Ok(false);
    };
    if page.workspace_id != workspace_id || page.deleted_at.is_some() {
        return Ok(false);
    }
    match anchor {
        CommentAnchor::Page => Ok(true),
        CommentAnchor::Block { block_id } | CommentAnchor::Text { block_id, .. } => {
            let raw: Option<String> = conn
                .query_row(
                    "SELECT data FROM blocks WHERE id=?1 AND page_id=?2",
                    params![block_id, page_id],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(raw) = raw else { return Ok(false) };
            let block: Block = decode(raw)?;
            match anchor {
                CommentAnchor::Text {
                    start, end, quote, ..
                } => Ok(start < end && block.text.get(*start..*end) == Some(quote.as_str())),
                _ => Ok(true),
            }
        }
        CommentAnchor::Cell {
            record_id,
            property_id,
        } => {
            let raw: Option<String> = conn.query_row(
                "SELECT d.data FROM records r JOIN databases d ON d.id=r.database_id WHERE r.id=?1 AND d.page_id=?2 AND r.deleted_at IS NULL",
                params![record_id, page_id], |r| r.get(0),
            ).optional()?;
            match raw {
                Some(raw) => {
                    let db: Database = decode(raw)?;
                    Ok(db.properties.iter().any(|p| p.id == *property_id))
                }
                None => Ok(false),
            }
        }
    }
}

impl Store {
    pub fn create_comment(
        &mut self,
        workspace_id: &str,
        page_id: &str,
        anchor: CommentAnchor,
        author: &str,
        text: &str,
    ) -> Result<CommentThread> {
        valid_message(author, text)?;
        if let CommentAnchor::Text { quote, .. } = &anchor {
            if quote.is_empty() || quote.len() > 65_536 {
                return Err(invalid("comment selection is invalid"));
            }
        }
        let kind = self
            .history_actor
            .as_ref()
            .filter(|actor| actor.name == author)
            .map(|actor| actor.kind.clone());
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !available(&tx, workspace_id, page_id, &anchor)? {
            return Err(invalid("comment target is unavailable"));
        }
        let id = new_id();
        let created_at = now();
        let message = CommentMessage {
            id: new_id(),
            author: author.to_owned(),
            kind,
            text: text.to_owned(),
            created_at: created_at.clone(),
        };
        tx.execute("INSERT INTO comment_threads(id,workspace_id,page_id,anchor,resolved,created_at) VALUES(?1,?2,?3,?4,0,?5)", params![id,workspace_id,page_id,encode(&anchor)?,created_at])?;
        tx.execute("INSERT INTO comment_messages(id,thread_id,author,body,created_at,kind) VALUES(?1,?2,?3,?4,?5,?6)", params![message.id,id,message.author,message.text,message.created_at,message.kind])?;
        tx.commit()?;
        Ok(CommentThread {
            id,
            workspace_id: workspace_id.into(),
            page_id: page_id.into(),
            anchor,
            resolved: false,
            created_at,
            messages: vec![message],
            anchor_available: true,
        })
    }

    pub fn list_comments(&self, workspace_id: &str, page_id: &str) -> Result<Vec<CommentThread>> {
        let mut stmt = self.conn.prepare("SELECT id,anchor,resolved,created_at,orphaned FROM comment_threads WHERE workspace_id=?1 AND page_id=?2 ORDER BY created_at,id")?;
        let rows = stmt.query_map(params![workspace_id, page_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, bool>(4)?,
            ))
        })?;
        let mut threads = Vec::new();
        for row in rows {
            let (id, raw, resolved, created_at, orphaned) = row?;
            let anchor: CommentAnchor = decode(raw)?;
            let mut messages_stmt = self.conn.prepare("SELECT id,author,body,created_at,kind FROM comment_messages WHERE thread_id=?1 ORDER BY created_at,id")?;
            let messages = messages_stmt
                .query_map([&id], |r| {
                    Ok(CommentMessage {
                        id: r.get(0)?,
                        author: r.get(1)?,
                        text: r.get(2)?,
                        created_at: r.get(3)?,
                        kind: r.get(4)?,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;
            threads.push(CommentThread {
                id,
                workspace_id: workspace_id.into(),
                page_id: page_id.into(),
                anchor_available: !orphaned
                    && available(&self.conn, workspace_id, page_id, &anchor)?,
                anchor,
                resolved,
                created_at,
                messages,
            });
        }
        Ok(threads)
    }

    pub fn reply_comment(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        author: &str,
        text: &str,
    ) -> Result<()> {
        valid_message(author, text)?;
        let kind = self
            .history_actor
            .as_ref()
            .filter(|actor| actor.name == author)
            .map(|actor| actor.kind.clone());
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let found: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM comment_threads WHERE id=?1 AND workspace_id=?2)",
            params![thread_id, workspace_id],
            |r| r.get(0),
        )?;
        if !found {
            return Err(invalid("comment thread is unavailable"));
        }
        tx.execute("INSERT INTO comment_messages(id,thread_id,author,body,created_at,kind) VALUES(?1,?2,?3,?4,?5,?6)", params![new_id(),thread_id,author,text,now(),kind])?;
        tx.commit()
    }

    pub fn resolve_comment(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        resolved: bool,
    ) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE comment_threads SET resolved=?1 WHERE id=?2 AND workspace_id=?3",
            params![resolved, thread_id, workspace_id],
        )?;
        if changed != 1 {
            return Err(invalid("comment thread is unavailable"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "comments_tests.rs"]
mod tests;
