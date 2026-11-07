use crate::{
    Block, BlockKind, Database, Page, PropertyKind, Record, Store, Workspace, new_id, search,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

const FORMAT: &str = "rotion-workspace";
const VERSION: u32 = 1;
const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid archive JSON: {0}")]
    Json(#[from] serde_json::Error),
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
        BlockKind::Image | BlockKind::Video | BlockKind::Audio | BlockKind::Pdf | BlockKind::File
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
    /// File and media attachments and external relations are not supported.
    pub fn export_workspace(&self, workspace_id: &str) -> Result<String, ArchiveError> {
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
        let blocks=decode_rows(&mut self.conn.prepare("SELECT b.data FROM blocks b JOIN pages p ON p.id=b.page_id WHERE p.workspace_id=?1 ORDER BY b.id")?,workspace_id)?;
        let databases=decode_rows(&mut self.conn.prepare("SELECT d.data FROM databases d JOIN pages p ON p.id=d.page_id WHERE p.workspace_id=?1 ORDER BY d.id")?,workspace_id)?;
        let records=decode_rows(&mut self.conn.prepare("SELECT r.data FROM records r JOIN databases d ON d.id=r.database_id JOIN pages p ON p.id=d.page_id WHERE p.workspace_id=?1 ORDER BY r.id")?,workspace_id)?;
        let payload = Payload {
            workspace,
            pages,
            blocks,
            databases,
            records,
        };
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

    /// Validate and import an archive as a new workspace in one transaction.
    /// The checksum detects corruption; it does not authenticate the source.
    /// Structured references are remapped when their complete value is an ID.
    pub fn import_workspace(&mut self, input: &str) -> Result<Workspace, ArchiveError> {
        if input.len() > MAX_ARCHIVE_BYTES {
            return Err(invalid("archive exceeds size limit"));
        }
        let mut envelope: Envelope = serde_json::from_str(input)?;
        if envelope.format != FORMAT || envelope.version != VERSION {
            return Err(invalid("unsupported format or version"));
        }
        if checksum(&envelope.payload)? != envelope.checksum_sha256 {
            return Err(invalid("checksum mismatch"));
        }
        validate(&envelope.payload)?;
        let map = make_map(&envelope.payload);
        remap(&mut envelope.payload, &map);
        let payload = envelope.payload;
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
    ids.into_iter().map(|id| (id, new_id())).collect()
}

fn validate(p: &Payload) -> Result<(), ArchiveError> {
    if unsupported(p) {
        return Err(ArchiveError::AttachmentsUnsupported);
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
    let pages: HashMap<&str, &ArchivedPage> =
        p.pages.iter().map(|x| (x.page.id.as_str(), x)).collect();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PageKind;
    use serde_json::json;

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
