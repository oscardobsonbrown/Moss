use moss_core::{Database, PropertyKind, Record, Store};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Limit work to visible records; larger relations keep the count fallback.
pub fn load(
    store: &Store,
    db: &Database,
    records: &[Record],
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut remaining = 1000usize;
    let mut scanned = 0usize;
    for property in db
        .properties
        .iter()
        .filter(|p| p.kind == PropertyKind::Relation)
    {
        let Some(target) = property
            .config
            .get("target_database_id")
            .and_then(Value::as_str)
        else {
            continue;
        };
        for record in records {
            let Some(ids) = record.values.get(&property.id).and_then(Value::as_array) else {
                continue;
            };
            for id in ids.iter().filter_map(Value::as_str) {
                scanned += 1;
                if remaining == 0 || scanned > 5000 {
                    break;
                }
                if groups
                    .entry(target.to_owned())
                    .or_default()
                    .insert(id.to_owned())
                {
                    remaining -= 1;
                }
            }
            if remaining == 0 || scanned > 5000 {
                break;
            }
        }
        if remaining == 0 || scanned > 5000 {
            break;
        }
    }
    let mut titles = BTreeMap::new();
    for (target, ids) in groups {
        let Some(db) = store.get_database(&target)? else {
            continue;
        };
        let Some(page) = store.get_page(&db.page_id)? else {
            continue;
        };
        if page.deleted_at.is_some() {
            continue;
        }
        let ids: Vec<_> = ids.into_iter().collect();
        for batch in ids.chunks(200) {
            for record in store.relation_records_by_ids(&target, batch)? {
                titles.insert(
                    record.id,
                    if record.title.is_empty() {
                        "Untitled".into()
                    } else {
                        record.title
                    },
                );
            }
        }
    }
    Ok(titles)
}
