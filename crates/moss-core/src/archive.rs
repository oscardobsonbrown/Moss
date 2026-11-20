use crate::comments::{CommentAnchor, CommentMessage};
use crate::{
    Block, BlockKind, Database, Page, PropertyKind, Record, Store, Workspace, new_id, search,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[path = "archive_file.rs"]
mod file_archive;

const FORMAT: &str = "moss-workspace";
const VERSION: u32 = 1;
const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid archive JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("archive file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid archive: {0}")]
    Invalid(String),
    #[error("attachments are not supported in this archive version")]
    AttachmentsUnsupported,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: String,
    version: u32,
    checksum_sha256: String,
    payload: Payload,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    workspace: Workspace,
    pages: Vec<ArchivedPage>,
    blocks: Vec<Block>,
    databases: Vec<Database>,
    records: Vec<Record>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    board_order: Vec<ArchivedBoardOrder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    record_trash_roots: Vec<RecordTrashRoot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cover_assets: Vec<CoverAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    comments: Vec<ArchivedComment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    history: Vec<crate::history::HistoryEntry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivedComment {
    id: String,
    page_id: String,
    anchor: CommentAnchor,
    resolved: bool,
    orphaned: bool,
    created_at: String,
    messages: Vec<CommentMessage>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivedBoardOrder {
    view_id: String,
    record_id: String,
    rank: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverAsset {
    digest: String,
    bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordTrashRoot {
    record_id: String,
    root_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivedPage {
    page: Page,
    trash_root: Option<String>,
}

fn invalid(s: &str) -> ArchiveError {
    ArchiveError::Invalid(s.into())
}
fn checksum(p: &Payload) -> Result<String, ArchiveError> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(p)?)))
}
fn decode_rows<T: serde::de::DeserializeOwned>(
    stmt: &mut rusqlite::Statement<'_>,
    arg: &str,
) -> Result<Vec<T>, ArchiveError> {
    let rows = stmt.query_map([arg], |r| r.get::<_, String>(0))?;
    rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
}
fn media(kind: BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::Video | BlockKind::Audio | BlockKind::Pdf | BlockKind::File
    )
}
fn unsupported(p: &Payload) -> bool {
    if p.blocks.iter().any(|b| media(b.kind)) {
        return true;
    }
    p.databases.iter().any(|d| {
        d.properties
            .iter()
            .filter(|p| p.kind == PropertyKind::Files)
            .any(|prop| {
                p.records.iter().filter(|r| r.database_id == d.id).any(|r| {
                    r.values
                        .get(&prop.id)
                        .is_some_and(|v| !v.is_null() && v != &Value::Array(vec![]))
                })
            })
    })
}

impl Store {
    /// Export one workspace as version 1 JSON, up to 64 MiB.
    /// Non-image media attachments and external relations are not supported.
    pub fn export_workspace(&self, workspace_id: &str) -> Result<String, ArchiveError> {
        let (mut payload, asset_ids) = self.collect_archive_payload(workspace_id)?;
        for digest in asset_ids {
            let bytes = self.read_cover_image(&digest)?;
            payload.cover_assets.push(CoverAsset { digest, bytes });
        }
        validate(&payload)?;
        let envelope = Envelope {
            format: FORMAT.into(),
            version: VERSION,
            checksum_sha256: checksum(&payload)?,
            payload,
        };
        let result = serde_json::to_string(&envelope)?;
        if result.len() > MAX_ARCHIVE_BYTES {
            return Err(invalid("archive exceeds size limit"));
        }
        Ok(result)
    }

    fn collect_archive_payload(
        &self,
        workspace_id: &str,
    ) -> Result<(Payload, Vec<String>), ArchiveError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT data FROM workspaces WHERE id=?1",
                [workspace_id],
                |r| r.get(0),
            )
            .optional()?;
        let workspace: Workspace =
            serde_json::from_str(&raw.ok_or_else(|| invalid("workspace not found"))?)?;
        let pages = {
            let mut s = self
                .conn
                .prepare("SELECT data,trash_root FROM pages WHERE workspace_id=?1 ORDER BY id")?;
            let rows = s.query_map([workspace_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })?;
            rows.map(|r| {
                let (raw, trash_root) = r?;
                Ok(ArchivedPage {
                    page: serde_json::from_str(&raw)?,
                    trash_root,
                })
            })
            .collect::<Result<Vec<_>, ArchiveError>>()?
        };
        let blocks: Vec<Block>=decode_rows(&mut self.conn.prepare("SELECT b.data FROM blocks b JOIN pages p ON p.id=b.page_id WHERE p.workspace_id=?1 ORDER BY b.id")?,workspace_id)?;
        let databases=decode_rows(&mut self.conn.prepare("SELECT d.data FROM databases d JOIN pages p ON p.id=d.page_id WHERE p.workspace_id=?1 ORDER BY d.id")?,workspace_id)?;
        let records=decode_rows(&mut self.conn.prepare("SELECT r.data FROM records r JOIN databases d ON d.id=r.database_id JOIN pages p ON p.id=d.page_id WHERE p.workspace_id=?1 ORDER BY r.id")?,workspace_id)?;
        let record_trash_roots = {
            let mut stmt = self.conn.prepare("SELECT t.record_id,t.root_id FROM record_trash_roots t JOIN records r ON r.id=t.record_id JOIN databases d ON d.id=r.database_id JOIN pages p ON p.id=d.page_id WHERE p.workspace_id=?1 ORDER BY t.record_id")?;
            let rows = stmt.query_map([workspace_id], |r| {
                Ok(RecordTrashRoot {
                    record_id: r.get(0)?,
                    root_id: r.get(1)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let board_order = {
            let mut statement=self.conn.prepare("SELECT bo.view_id,bo.record_id,bo.rank FROM board_record_order bo JOIN records r ON r.id=bo.record_id JOIN databases d ON d.id=r.database_id JOIN pages p ON p.id=d.page_id WHERE p.workspace_id=?1 AND EXISTS(SELECT 1 FROM json_each(d.data,'$.views') v WHERE json_extract(v.value,'$.id')=bo.view_id) ORDER BY bo.view_id,bo.record_id")?;
            statement
                .query_map([workspace_id], |row| {
                    Ok(ArchivedBoardOrder {
                        view_id: row.get(0)?,
                        record_id: row.get(1)?,
                        rank: row.get(2)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let comments = {
            let mut stmt = self.conn.prepare("SELECT id,page_id,anchor,resolved,orphaned,created_at FROM comment_threads WHERE workspace_id=?1 ORDER BY id")?;
            let rows = stmt.query_map([workspace_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, bool>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })?;
            let mut comments = Vec::new();
            for row in rows {
                let (id, page_id, anchor, resolved, orphaned, created_at) = row?;
                let mut messages = self.conn.prepare("SELECT id,author,body,created_at,kind FROM comment_messages WHERE thread_id=?1 ORDER BY created_at,id")?;
                let messages = messages
                    .query_map([&id], |r| {
                        Ok(CommentMessage {
                            id: r.get(0)?,
                            author: r.get(1)?,
                            text: r.get(2)?,
                            created_at: r.get(3)?,
                            kind: r.get(4)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                comments.push(ArchivedComment {
                    id,
                    page_id,
                    anchor: serde_json::from_str(&anchor)?,
                    resolved,
                    orphaned,
                    created_at,
                    messages,
                });
            }
            comments
        };
        let history = {
            let mut stmt = self.conn.prepare("SELECT sequence,workspace_id,page_id,entity_kind,entity_id,property_id,actor,occurred_at,before_json,after_json FROM change_history WHERE workspace_id=?1 ORDER BY sequence")?;
            let rows = stmt.query_map([workspace_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })?;
            let mut history = Vec::new();
            for row in rows {
                let (
                    sequence,
                    workspace_id,
                    page_id,
                    entity_kind,
                    entity_id,
                    property_id,
                    actor,
                    occurred_at,
                    before,
                    after,
                ) = row?;
                history.push(crate::history::HistoryEntry {
                    sequence,
                    workspace_id,
                    page_id,
                    entity_kind,
                    entity_id,
                    property_id,
                    actor: actor.map(|s| serde_json::from_str(&s)).transpose()?,
                    occurred_at,
                    before: before.map(|s| serde_json::from_str(&s)).transpose()?,
                    after: after.map(|s| serde_json::from_str(&s)).transpose()?,
                });
            }
            history
        };
        let payload = Payload {
            workspace,
            pages,
            blocks,
            databases,
            records,
            board_order,
            record_trash_roots,
            cover_assets: Vec::new(),
            comments,
            history,
        };
        let ids = asset_ids(&payload)?;
        Ok((payload, ids))
    }

    /// Validate and import an archive as a new workspace in one transaction.
    /// The checksum detects corruption; it does not authenticate the source.
    /// Structured references are remapped when their complete value is an ID.
    pub fn import_workspace(&mut self, input: &str) -> Result<Workspace, ArchiveError> {
        if input.len() > MAX_ARCHIVE_BYTES {
            return Err(invalid("archive exceeds size limit"));
        }
        let envelope: Envelope = serde_json::from_str(input)?;
        if envelope.format != FORMAT || envelope.version != VERSION {
            return Err(invalid("unsupported format or version"));
        }
        if checksum(&envelope.payload)? != envelope.checksum_sha256 {
            return Err(invalid("checksum mismatch"));
        }
        validate(&envelope.payload)?;
        for asset in &envelope.payload.cover_assets {
            let digest = crate::page_covers::save_asset(&self.conn, &asset.bytes)?;
            if digest != asset.digest {
                return Err(invalid("cover asset checksum mismatch"));
            }
        }
        self.import_archive_payload(envelope.payload)
    }

    fn import_archive_payload(&mut self, mut payload: Payload) -> Result<Workspace, ArchiveError> {
        // Untouched cards use their original ID as the order key. Retain that key when IDs change.
        let mut ranked: HashSet<_> = payload
            .board_order
            .iter()
            .map(|row| (row.view_id.clone(), row.record_id.clone()))
            .collect();
        for record in &payload.records {
            if let Some(db) = payload
                .databases
                .iter()
                .find(|db| db.id == record.database_id)
            {
                for view in db
                    .views
                    .iter()
                    .filter(|view| view.kind == crate::ViewKind::Board)
                {
                    if ranked.insert((view.id.clone(), record.id.clone())) {
                        payload.board_order.push(ArchivedBoardOrder {
                            view_id: view.id.clone(),
                            record_id: record.id.clone(),
                            rank: record.id.as_bytes().to_vec(),
                        });
                    }
                }
            }
        }
        let map = make_map(&payload);
        remap(&mut payload, &map);
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO workspaces(id,data) VALUES(?1,?2)",
            params![
                payload.workspace.id,
                serde_json::to_string(&payload.workspace)?
            ],
        )?;
        // Parents must exist before children for immediate foreign-key checks.
        let mut ordered: Vec<&ArchivedPage> = Vec::new();
        let mut done = HashSet::new();
        while ordered.len() < payload.pages.len() {
            let before = ordered.len();
            for item in &payload.pages {
                if !done.contains(&item.page.id)
                    && item
                        .page
                        .parent_id
                        .as_ref()
                        .map_or(true, |id| done.contains(id))
                {
                    done.insert(item.page.id.clone());
                    ordered.push(item);
                }
            }
            if before == ordered.len() {
                return Err(invalid("page hierarchy cycle"));
            }
        }
        for item in ordered {
            let p = &item.page;
            tx.execute("INSERT INTO pages(id,workspace_id,parent_id,deleted_at,trash_root,data) VALUES(?1,?2,?3,?4,?5,?6)",params![p.id,p.workspace_id,p.parent_id,p.deleted_at,item.trash_root,serde_json::to_string(p)?])?;
            search::index(&tx, &format!("page:{}", p.id), p, &p.title)?;
        }
        let pages: HashMap<&str, &Page> = payload
            .pages
            .iter()
            .map(|p| (p.page.id.as_str(), &p.page))
            .collect();
        for b in &payload.blocks {
            let p = pages[b.page_id.as_str()];
            tx.execute(
                "INSERT INTO blocks(id,page_id,sort_order,data) VALUES(?1,?2,?3,?4)",
                params![b.id, b.page_id, b.sort_order, serde_json::to_string(b)?],
            )?;
            search::index(
                &tx,
                &format!("block:{}", b.id),
                p,
                &format!("{} {}", b.text, b.config),
            )?;
        }
        for d in &payload.databases {
            tx.execute(
                "INSERT INTO databases(id,page_id,data) VALUES(?1,?2,?3)",
                params![d.id, d.page_id, serde_json::to_string(d)?],
            )?;
        }
        let dbs: HashMap<&str, &Database> = payload
            .databases
            .iter()
            .map(|d| (d.id.as_str(), d))
            .collect();
        for r in &payload.records {
            tx.execute("INSERT INTO records(id,database_id,page_id,title,values_json,deleted_at,data) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![r.id,r.database_id,r.page_id,r.title,serde_json::to_string(&r.values)?,r.deleted_at,serde_json::to_string(r)?])?;
            if r.deleted_at.is_none() {
                let p = pages[r
                    .page_id
                    .as_deref()
                    .unwrap_or(dbs[r.database_id.as_str()].page_id.as_str())];
                search::index(
                    &tx,
                    &format!("record:{}", r.id),
                    p,
                    &format!("{} {}", r.title, serde_json::to_string(&r.values)?),
                )?;
            }
        }
        for row in &payload.board_order {
            tx.execute(
                "INSERT INTO board_record_order(view_id,record_id,rank) VALUES(?1,?2,?3)",
                params![row.view_id, row.record_id, row.rank],
            )?;
        }
        for tag in &payload.record_trash_roots {
            tx.execute(
                "INSERT INTO record_trash_roots(record_id,root_id) VALUES (?1,?2)",
                params![tag.record_id, tag.root_id],
            )?;
        }
        for comment in &payload.comments {
            tx.execute("INSERT INTO comment_threads(id,workspace_id,page_id,anchor,resolved,orphaned,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![comment.id,payload.workspace.id,comment.page_id,serde_json::to_string(&comment.anchor)?,comment.resolved,comment.orphaned,comment.created_at])?;
            for message in &comment.messages {
                tx.execute("INSERT INTO comment_messages(id,thread_id,author,body,created_at,kind) VALUES(?1,?2,?3,?4,?5,?6)", params![message.id,comment.id,message.author,message.text,message.created_at,message.kind])?;
            }
        }
        for entry in &payload.history {
            tx.execute("INSERT INTO change_history(workspace_id,page_id,entity_kind,entity_id,property_id,actor,occurred_at,before_json,after_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![entry.workspace_id,entry.page_id,entry.entity_kind,entry.entity_id,entry.property_id,entry.actor.as_ref().map(serde_json::to_string).transpose()?,entry.occurred_at,entry.before.as_ref().map(serde_json::to_string).transpose()?,entry.after.as_ref().map(serde_json::to_string).transpose()?])?;
        }
        if let Some(database) = payload.databases.first() {
            crate::advanced_properties::graph(&tx, &payload.workspace.id, database, None)?;
        }
        for batch in payload.records.chunks(256) {
            let ids: Vec<String> = batch.iter().map(|record| record.id.clone()).collect();
            crate::calculation::refresh(&tx, &ids)?;
        }
        tx.commit()?;
        Ok(payload.workspace)
    }
}

fn make_map(p: &Payload) -> HashMap<String, String> {
    let mut ids = vec![p.workspace.id.clone()];
    ids.extend(p.pages.iter().map(|x| x.page.id.clone()));
    ids.extend(p.blocks.iter().map(|x| x.id.clone()));
    for d in &p.databases {
        ids.push(d.id.clone());
        ids.extend(d.properties.iter().map(|x| x.id.clone()));
        ids.extend(d.views.iter().map(|x| x.id.clone()));
    }
    ids.extend(p.records.iter().map(|x| x.id.clone()));
    for entry in &p.history {
        ids.push(entry.entity_id.clone());
        if let Some(property) = &entry.property_id {
            if entry.entity_kind != "page" {
                ids.push(property.clone());
            }
        }
        if entry.entity_kind == "relation" {
            for snapshot in [entry.before.as_ref(), entry.after.as_ref()]
                .into_iter()
                .flatten()
            {
                if let Some(values) = snapshot.as_array() {
                    ids.extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
                }
            }
        }
        if entry.entity_kind == "block" {
            for snapshot in [entry.before.as_ref(), entry.after.as_ref()]
                .into_iter()
                .flatten()
            {
                if let Some(id) = snapshot
                    .get("config")
                    .and_then(|v| v.get("page_id"))
                    .and_then(Value::as_str)
                {
                    ids.push(id.to_owned());
                }
            }
        }
    }
    for comment in &p.comments {
        ids.push(comment.id.clone());
        ids.extend(comment.messages.iter().map(|message| message.id.clone()));
        match &comment.anchor {
            CommentAnchor::Page => {}
            CommentAnchor::Block { block_id } | CommentAnchor::Text { block_id, .. } => {
                ids.push(block_id.clone())
            }
            CommentAnchor::Cell {
                record_id,
                property_id,
            } => {
                ids.push(record_id.clone());
                ids.push(property_id.clone());
            }
        }
    }
    ids.into_iter().map(|id| (id, new_id())).collect()
}

fn validate(p: &Payload) -> Result<(), ArchiveError> {
    let mut assets = HashSet::new();
    for asset in &p.cover_assets {
        if !crate::page_covers::valid_digest(&asset.digest)
            || !crate::page_covers::validate_image(&asset.bytes)
            || format!("{:x}", Sha256::digest(&asset.bytes)) != asset.digest
            || !assets.insert(asset.digest.as_str())
        {
            return Err(invalid("invalid cover asset"));
        }
    }
    validate_with_assets(p, &assets)
}

fn asset_ids(p: &Payload) -> Result<Vec<String>, ArchiveError> {
    let mut ids = std::collections::BTreeSet::new();
    for item in &p.pages {
        if let Some(crate::PageCover::Image { asset, .. }) = &item.page.cover {
            ids.insert(asset.clone());
        }
    }
    for block in &p.blocks {
        if block.kind == BlockKind::Image {
            if let Some(asset) = block.config.get("asset") {
                ids.insert(
                    asset
                        .as_str()
                        .ok_or_else(|| invalid("invalid image asset"))?
                        .to_owned(),
                );
            }
        }
    }
    for entry in &p.history {
        if entry.entity_kind == "block" {
            for snapshot in [entry.before.as_ref(), entry.after.as_ref()]
                .into_iter()
                .flatten()
            {
                let block: Block = serde_json::from_value(snapshot.clone())?;
                if block.kind == BlockKind::Image {
                    if let Some(asset) = block.config.get("asset") {
                        ids.insert(
                            asset
                                .as_str()
                                .ok_or_else(|| invalid("invalid history image asset"))?
                                .to_owned(),
                        );
                    }
                }
            }
        }
    }
    if ids.iter().any(|id| !crate::page_covers::valid_digest(id)) {
        return Err(invalid("invalid asset digest"));
    }
    Ok(ids.into_iter().collect())
}

fn validate_with_assets(p: &Payload, assets: &HashSet<&str>) -> Result<(), ArchiveError> {
    let records: HashMap<_, _> = p
        .records
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect();
    let owners: HashMap<_, _> = p
        .databases
        .iter()
        .flat_map(|db| {
            db.views
                .iter()
                .map(move |view| (view.id.as_str(), db.id.as_str()))
        })
        .collect();
    let mut order_pairs = HashSet::new();
    for row in &p.board_order {
        let record = records
            .get(row.record_id.as_str())
            .ok_or_else(|| invalid("Board order record missing"))?;
        if owners.get(row.view_id.as_str()).copied() != Some(record.database_id.as_str())
            || row.rank.is_empty()
            || row.rank.len() > 512
            || !order_pairs.insert((&row.view_id, &row.record_id))
        {
            return Err(invalid("invalid Board card order"));
        }
    }

    for item in &p.pages {
        if let Some(cover) = &item.page.cover {
            if !crate::page_covers::valid_cover(cover) {
                return Err(invalid("invalid page cover"));
            }
            if let crate::PageCover::Image { asset, .. } = cover {
                if !assets.contains(asset.as_str()) {
                    return Err(invalid("cover asset missing"));
                }
            }
        }
    }
    for block in &p.blocks {
        if block.kind == BlockKind::Image {
            if let Some(asset) = block.config.get("asset") {
                let asset = asset
                    .as_str()
                    .ok_or_else(|| invalid("invalid image asset"))?;
                if !assets.contains(asset) {
                    return Err(invalid("image asset missing"));
                }
            }
        }
    }
    if unsupported(p) {
        return Err(ArchiveError::AttachmentsUnsupported);
    }
    if asset_ids(p)?.iter().any(|id| !assets.contains(id.as_str())) {
        return Err(invalid("history asset missing"));
    }
    let mut ids = HashSet::new();
    let mut add = |id: &str| -> Result<(), ArchiveError> {
        if id.is_empty() || !ids.insert(id.to_owned()) {
            return Err(invalid("empty or duplicate ID"));
        }
        Ok(())
    };
    add(&p.workspace.id)?;
    for x in &p.pages {
        add(&x.page.id)?;
    }
    for x in &p.blocks {
        add(&x.id)?;
    }
    for d in &p.databases {
        add(&d.id)?;
        for x in &d.properties {
            add(&x.id)?;
        }
        for x in &d.views {
            add(&x.id)?;
        }
    }
    for x in &p.records {
        add(&x.id)?;
    }
    for comment in &p.comments {
        add(&comment.id)?;
        for message in &comment.messages {
            add(&message.id)?;
        }
    }
    let pages: HashMap<&str, &ArchivedPage> =
        p.pages.iter().map(|x| (x.page.id.as_str(), x)).collect();
    let mut last_sequence = 0;
    for entry in &p.history {
        if entry.sequence <= last_sequence
            || entry.workspace_id != p.workspace.id
            || !pages.contains_key(entry.page_id.as_str())
            || entry.entity_id.is_empty()
            || !matches!(
                entry.entity_kind.as_str(),
                "block"
                    | "record"
                    | "page"
                    | "record_title"
                    | "relation"
                    | "record_created"
                    | "record_duplicated"
                    | "record_trashed"
                    | "record_restored"
            )
            || chrono::DateTime::parse_from_rfc3339(&entry.occurred_at).is_err()
            || entry.actor.as_ref().is_some_and(|actor| {
                actor.id.trim().is_empty()
                    || actor.name.trim().is_empty()
                    || !matches!(actor.kind.as_str(), "person" | "agent")
            })
        {
            return Err(invalid("invalid history entry"));
        }
        if entry.entity_kind == "block" {
            for snapshot in [entry.before.as_ref(), entry.after.as_ref()]
                .into_iter()
                .flatten()
            {
                let block: Block = serde_json::from_value(snapshot.clone())?;
                if block.id != entry.entity_id || block.page_id != entry.page_id {
                    return Err(invalid("invalid history block"));
                }
                if media(block.kind) {
                    return Err(ArchiveError::AttachmentsUnsupported);
                }
            }
        }
        last_sequence = entry.sequence;
    }
    for x in &p.pages {
        if x.page.workspace_id != p.workspace.id {
            return Err(invalid("page belongs to another workspace"));
        }
        if x.page
            .parent_id
            .as_ref()
            .is_some_and(|id| !pages.contains_key(id.as_str()))
        {
            return Err(invalid("missing parent page"));
        }
        if x.trash_root
            .as_ref()
            .is_some_and(|id| !pages.contains_key(id.as_str()))
        {
            return Err(invalid("missing trash root"));
        }
        let mut seen = HashSet::new();
        let mut cursor = Some(x.page.id.as_str());
        while let Some(id) = cursor {
            if !seen.insert(id) {
                return Err(invalid("page hierarchy cycle"));
            }
            cursor = pages[id].page.parent_id.as_deref();
        }
    }
    for b in &p.blocks {
        if !pages.contains_key(b.page_id.as_str()) {
            return Err(invalid("missing block page"));
        }
    }
    for comment in &p.comments {
        if !pages.contains_key(comment.page_id.as_str())
            || comment.messages.is_empty()
            || chrono::DateTime::parse_from_rfc3339(&comment.created_at).is_err()
        {
            return Err(invalid("invalid comment thread"));
        }
        for message in &comment.messages {
            if message.author.trim().is_empty()
                || message.author.len() > 256
                || message.text.trim().is_empty()
                || message.text.len() > 65_536
                || message
                    .kind
                    .as_deref()
                    .is_some_and(|kind| !matches!(kind, "person" | "agent"))
                || chrono::DateTime::parse_from_rfc3339(&message.created_at).is_err()
            {
                return Err(invalid("invalid comment message"));
            }
        }
        match &comment.anchor {
            CommentAnchor::Text {
                block_id,
                start,
                end,
                quote,
            } => {
                if block_id.is_empty() || start >= end || quote.is_empty() || quote.len() > 65_536 {
                    return Err(invalid("invalid comment anchor"));
                }
                if !comment.orphaned
                    && p.blocks
                        .iter()
                        .find(|block| block.id == *block_id)
                        .is_some_and(|block| {
                            block.page_id != comment.page_id
                                || block.text.get(*start..*end) != Some(quote)
                        })
                {
                    return Err(invalid("invalid comment anchor"));
                }
            }
            CommentAnchor::Block { block_id } if block_id.is_empty() => {
                return Err(invalid("invalid comment anchor"));
            }
            CommentAnchor::Cell {
                record_id,
                property_id,
            } if record_id.is_empty() || property_id.is_empty() => {
                return Err(invalid("invalid comment anchor"));
            }
            _ => {}
        }
    }
    let mut db_pages = HashSet::new();
    let dbs: HashMap<&str, &Database> = p.databases.iter().map(|d| (d.id.as_str(), d)).collect();
    for d in &p.databases {
        if !pages.contains_key(d.page_id.as_str()) || !db_pages.insert(&d.page_id) {
            return Err(invalid("missing or duplicate database page"));
        }
        let props: HashSet<&str> = d.properties.iter().map(|x| x.id.as_str()).collect();
        for v in &d.views {
            if v.query
                .sorts
                .iter()
                .any(|s| !props.contains(s.property_id.as_str()))
                || v.query
                    .group_by
                    .as_ref()
                    .is_some_and(|id| !props.contains(id.as_str()))
            {
                return Err(invalid("view references missing property"));
            }
            if let Some(filter) = &v.query.filter {
                validate_filter(filter, &props)?;
            }
        }
        for prop in &d.properties {
            if prop.kind == PropertyKind::Relation {
                if let Some(target) = prop
                    .config
                    .get("target_database_id")
                    .and_then(Value::as_str)
                {
                    if !dbs.contains_key(target) {
                        return Err(invalid("external relation target is unsupported"));
                    }
                }
            }
        }
    }
    let record_ids: HashSet<&str> = p.records.iter().map(|r| r.id.as_str()).collect();
    for r in &p.records {
        let d = dbs
            .get(r.database_id.as_str())
            .ok_or_else(|| invalid("missing record database"))?;
        if r.page_id
            .as_ref()
            .is_some_and(|id| !pages.contains_key(id.as_str()))
        {
            return Err(invalid("missing record page"));
        }
        for (key, value) in &r.values {
            let prop = d
                .properties
                .iter()
                .find(|x| &x.id == key)
                .ok_or_else(|| invalid("missing record property"))?;
            if prop.kind == PropertyKind::Relation {
                validate_relation(value, &record_ids, &pages)?;
            }
        }
    }
    let mut tagged = HashSet::new();
    for tag in &p.record_trash_roots {
        let record = p
            .records
            .iter()
            .find(|r| r.id == tag.record_id)
            .ok_or_else(|| invalid("missing trashed record"))?;
        if record.deleted_at.is_none()
            || !tagged.insert(&tag.record_id)
            || (tag.root_id != tag.record_id && !pages.contains_key(tag.root_id.as_str()))
        {
            return Err(invalid("invalid record trash root"));
        }
    }
    Ok(())
}

fn validate_filter(f: &crate::FilterExpr, props: &HashSet<&str>) -> Result<(), ArchiveError> {
    match f {
        crate::FilterExpr::Rule(r) if !props.contains(r.property_id.as_str()) => {
            Err(invalid("filter references missing property"))
        }
        crate::FilterExpr::Rule(_) => Ok(()),
        crate::FilterExpr::All(items) | crate::FilterExpr::Any(items) => {
            for x in items {
                validate_filter(x, props)?;
            }
            Ok(())
        }
    }
}
fn validate_relation(
    v: &Value,
    records: &HashSet<&str>,
    pages: &HashMap<&str, &ArchivedPage>,
) -> Result<(), ArchiveError> {
    match v {
        Value::Null => Ok(()),
        Value::String(id) if records.contains(id.as_str()) || pages.contains_key(id.as_str()) => {
            Ok(())
        }
        Value::Array(items) => {
            for x in items {
                validate_relation(x, records, pages)?;
            }
            Ok(())
        }
        _ => Err(invalid("external or unsupported relation value")),
    }
}
fn id(s: &mut String, map: &HashMap<String, String>) {
    if let Some(new) = map.get(s) {
        *s = new.clone();
    }
}
fn optional_id(s: &mut Option<String>, map: &HashMap<String, String>) {
    if let Some(s) = s {
        id(s, map);
    }
}
fn value(v: &mut Value, map: &HashMap<String, String>) {
    match v {
        Value::String(s) => id(s, map),
        Value::Array(a) => {
            for x in a {
                value(x, map);
            }
        }
        Value::Object(o) => {
            let old = std::mem::take(o);
            for (mut k, mut v) in old {
                id(&mut k, map);
                value(&mut v, map);
                o.insert(k, v);
            }
        }
        _ => {}
    }
}
fn filter(f: &mut crate::FilterExpr, map: &HashMap<String, String>) {
    match f {
        crate::FilterExpr::Rule(r) => {
            id(&mut r.property_id, map);
            value(&mut r.value, map);
        }
        crate::FilterExpr::All(items) | crate::FilterExpr::Any(items) => {
            for x in items {
                filter(x, map);
            }
        }
    }
}
fn remap(p: &mut Payload, map: &HashMap<String, String>) {
    id(&mut p.workspace.id, map);
    for x in &mut p.pages {
        id(&mut x.page.id, map);
        id(&mut x.page.workspace_id, map);
        optional_id(&mut x.page.parent_id, map);
        optional_id(&mut x.trash_root, map);
    }
    for b in &mut p.blocks {
        id(&mut b.id, map);
        id(&mut b.page_id, map);
        value(&mut b.config, map);
    }
    for d in &mut p.databases {
        id(&mut d.id, map);
        id(&mut d.page_id, map);
        for prop in &mut d.properties {
            id(&mut prop.id, map);
            value(&mut prop.config, map);
        }
        for v in &mut d.views {
            id(&mut v.id, map);
            value(&mut v.config, map);
            if let Some(f) = &mut v.query.filter {
                filter(f, map);
            }
            for s in &mut v.query.sorts {
                id(&mut s.property_id, map);
            }
            optional_id(&mut v.query.group_by, map);
        }
    }
    for row in &mut p.board_order {
        id(&mut row.view_id, map);
        id(&mut row.record_id, map);
    }
    for tag in &mut p.record_trash_roots {
        id(&mut tag.record_id, map);
        id(&mut tag.root_id, map);
    }
    for r in &mut p.records {
        id(&mut r.id, map);
        id(&mut r.database_id, map);
        optional_id(&mut r.page_id, map);
        let old = std::mem::take(&mut r.values);
        for (mut k, mut v) in old {
            id(&mut k, map);
            value(&mut v, map);
            r.values.insert(k, v);
        }
    }
    for comment in &mut p.comments {
        id(&mut comment.id, map);
        id(&mut comment.page_id, map);
        match &mut comment.anchor {
            CommentAnchor::Page => {}
            CommentAnchor::Block { block_id } | CommentAnchor::Text { block_id, .. } => {
                id(block_id, map)
            }
            CommentAnchor::Cell {
                record_id,
                property_id,
            } => {
                id(record_id, map);
                id(property_id, map);
            }
        }
        for message in &mut comment.messages {
            id(&mut message.id, map);
        }
    }
    for entry in &mut p.history {
        id(&mut entry.workspace_id, map);
        id(&mut entry.page_id, map);
        id(&mut entry.entity_id, map);
        if entry.entity_kind != "page" {
            optional_id(&mut entry.property_id, map);
        }
        for snapshot in [&mut entry.before, &mut entry.after].into_iter().flatten() {
            if entry.entity_kind == "block" {
                if let Some(object) = snapshot.as_object_mut() {
                    if let Some(Value::String(block_id)) = object.get_mut("id") {
                        id(block_id, map);
                    }
                    if let Some(Value::String(page_id)) = object.get_mut("page_id") {
                        id(page_id, map);
                    }
                    if let Some(config) = object.get_mut("config") {
                        value(config, map);
                    }
                }
            } else if entry.entity_kind == "relation" {
                value(snapshot, map);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PageKind;
    use serde_json::json;

    #[test]
    fn empty_trash_metadata_is_omitted_from_v1_checksum() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Legacy", "").unwrap();
        let text = store.export_workspace(&workspace.id).unwrap();
        let json: Value = serde_json::from_str(&text).unwrap();
        assert!(json["payload"].get("record_trash_roots").is_none());
        let parsed: Envelope = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.checksum_sha256, checksum(&parsed.payload).unwrap());
        store.import_workspace(&text).unwrap();
    }

    #[test]
    fn workspace_round_trip_remaps_relations_and_preserves_trash() {
        let mut store = Store::in_memory().unwrap();
        let original = store.seed_demo_workspaces().unwrap().remove(0);
        let pages = store.list_pages(&original.id).unwrap();
        let db_page = pages.iter().find(|p| p.kind == PageKind::Database).unwrap();
        let db = store.get_database_by_page(&db_page.id).unwrap().unwrap();
        let records = store
            .query_records(&db.id, &crate::RecordQuery::default())
            .unwrap();
        let relation = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Relation)
            .unwrap();
        let source = records
            .iter()
            .find(|r| r.values.contains_key(&relation.id))
            .unwrap();
        let target = source.values[&relation.id][0].as_str().unwrap();
        let trashed = store
            .create_page(
                &original.id,
                None,
                "Archived sample",
                "",
                PageKind::Document,
            )
            .unwrap();
        store.delete_page(&trashed.id).unwrap();
        let encoded = store.export_workspace(&original.id).unwrap();
        let imported = store.import_workspace(&encoded).unwrap();
        assert_ne!(original.id, imported.id);
        let imported_pages = store.list_pages(&imported.id).unwrap();
        let imported_db_page = imported_pages
            .iter()
            .find(|p| p.kind == PageKind::Database)
            .unwrap();
        let imported_db = store
            .get_database_by_page(&imported_db_page.id)
            .unwrap()
            .unwrap();
        assert_ne!(db.id, imported_db.id);
        let new_relation = imported_db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Relation)
            .unwrap();
        assert_ne!(relation.id, new_relation.id);
        assert_eq!(new_relation.config["target_database_id"], imported_db.id);
        let new_records = store
            .query_records(&imported_db.id, &crate::RecordQuery::default())
            .unwrap();
        let new_source = new_records
            .iter()
            .find(|r| r.title == source.title)
            .unwrap();
        let new_target = new_source.values[&new_relation.id][0].as_str().unwrap();
        assert_ne!(new_target, target);
        assert!(new_records.iter().any(|r| r.id == new_target));
        assert_eq!(
            store.list_trash(&imported.id).unwrap()[0].title,
            "Archived sample"
        );
        assert_eq!(
            store
                .query_records(&db.id, &crate::RecordQuery::default())
                .unwrap(),
            records
        );
        assert!(
            !store
                .search(&source.title, Some(&imported.id), 10)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn rejects_tampering_and_media_without_writing() {
        let mut store = Store::in_memory().unwrap();
        let workspace = store.create_workspace("Test", "").unwrap();
        let page = store
            .create_page(&workspace.id, None, "Page", "", PageKind::Document)
            .unwrap();
        let encoded = store.export_workspace(&workspace.id).unwrap();
        let mut envelope: Value = serde_json::from_str(&encoded).unwrap();
        envelope["payload"]["workspace"]["name"] = json!("Changed");
        assert!(store.import_workspace(&envelope.to_string()).is_err());
        assert_eq!(store.list_workspaces().unwrap().len(), 1);
        let mut block = Block::paragraph(&page.id, "attachment", 0);
        block.kind = BlockKind::File;
        store.save_blocks(&page.id, &[block]).unwrap();
        assert!(matches!(
            store.export_workspace(&workspace.id),
            Err(ArchiveError::AttachmentsUnsupported)
        ));
    }
}
