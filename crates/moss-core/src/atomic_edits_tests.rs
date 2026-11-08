use crate::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn definition() -> Database {
    Database {
        id: new_id(),
        page_id: String::new(),
        name: "Tasks".into(),
        properties: vec![
            Property {
                id: "title".into(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            },
            Property {
                id: "date".into(),
                name: "Date".into(),
                kind: PropertyKind::Date,
                config: json!({}),
            },
            Property {
                id: "notes".into(),
                name: "Notes".into(),
                kind: PropertyKind::Text,
                config: json!({}),
            },
        ],
        views: vec![DatabaseView {
            id: "list".into(),
            name: "List".into(),
            kind: ViewKind::List,
            query: Default::default(),
            config: json!({"unknown": 42}),
        }],
    }
}

#[test]
fn failed_database_insert_leaves_no_page_or_search_entry() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Atomic", "").unwrap();
    let definition = definition();
    let page = s
        .create_database_page(&w.id, None, "Original", "", definition.clone())
        .unwrap();
    let count = s.list_pages(&w.id).unwrap().len();
    assert!(
        s.create_database_page(&w.id, None, "Orphan probe", "", definition.clone())
            .is_err()
    );
    assert_eq!(s.list_pages(&w.id).unwrap().len(), count);
    assert!(
        s.search("Orphan probe", Some(&w.id), 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        s.get_database(&definition.id).unwrap().unwrap().page_id,
        page.id
    );
    let mut invalid = definition.clone();
    invalid.id = new_id();
    invalid.properties.push(invalid.properties[0].clone());
    assert!(
        s.create_database_page(&w.id, None, "Invalid", "", invalid)
            .is_err()
    );
    assert_eq!(s.list_pages(&w.id).unwrap().len(), count);
}

#[test]
fn atomic_value_edits_preserve_other_fields_and_keep_search_in_sync() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Values", "").unwrap();
    let definition = definition();
    s.create_database_page(&w.id, None, "Tasks", "", definition.clone())
        .unwrap();
    let record = s
        .create_record(&definition.id, "Task", &BTreeMap::new())
        .unwrap();
    let page = s.ensure_record_page(&record.id).unwrap();
    s.set_record_value_if_unchanged(&record.id, "notes", &Value::Null, json!("keptmarker"))
        .unwrap();
    let saved = s
        .set_record_value_if_unchanged(&record.id, "date", &Value::Null, json!("2026-11-20"))
        .unwrap();
    assert_eq!(saved.values["notes"], "keptmarker");
    assert!(
        s.set_record_value_if_unchanged(&record.id, "date", &Value::Null, json!("2026-11-21"))
            .is_err()
    );
    assert_eq!(s.get_record(&record.id).unwrap().unwrap(), saved);
    assert!(
        s.search("keptmarker", Some(&w.id), 10)
            .unwrap()
            .iter()
            .any(|hit| hit.page_id == page.id)
    );
    assert!(
        s.set_record_value_if_unchanged(&record.id, "missing", &Value::Null, json!(1))
            .is_err()
    );
    s.delete_record(&record.id).unwrap();
    assert!(
        s.set_record_value_if_unchanged(&record.id, "date", &json!("2026-11-20"), Value::Null)
            .is_err()
    );
}

#[test]
fn view_config_edits_preserve_saved_fields_and_roll_back_on_failure() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Views", "").unwrap();
    let definition = definition();
    s.create_database_page(&w.id, None, "Tasks", "", definition.clone())
        .unwrap();
    s.edit_view_config(&definition.id, "list", ViewKind::List, |config| {
        config["hidden_property_ids"] = json!(["date"]);
        Ok(())
    })
    .unwrap();
    let saved = s
        .edit_view_config(&definition.id, "list", ViewKind::List, |config| {
            config["other"] = json!(true);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        saved.views[0].config,
        json!({"unknown":42,"hidden_property_ids":["date"],"other":true})
    );
    assert!(
        s.edit_view_config(&definition.id, "list", ViewKind::List, |config| {
            *config = Value::Null;
            Err("stop".into())
        })
        .is_err()
    );
    assert!(
        s.edit_view_config(&definition.id, "list", ViewKind::Gallery, |_| panic!(
            "wrong-layout closure ran"
        ))
        .is_err()
    );
    assert_eq!(s.get_database(&definition.id).unwrap().unwrap(), saved);
}

#[test]
fn two_connections_cannot_both_replace_the_same_original_value() {
    let path = std::env::temp_dir().join(format!("moss-atomic-{}.sqlite", new_id()));
    let mut s = Store::open(&path).unwrap();
    let w = s.create_workspace("Race", "").unwrap();
    let definition = definition();
    s.create_database_page(&w.id, None, "Tasks", "", definition.clone())
        .unwrap();
    let record = s
        .create_record(&definition.id, "Task", &BTreeMap::new())
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = ["2026-11-20", "2026-11-21"]
        .into_iter()
        .map(|value| {
            let path = path.clone();
            let id = record.id.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut s = Store::open(path).unwrap();
                barrier.wait();
                s.set_record_value_if_unchanged(&id, "date", &Value::Null, json!(value))
                    .map_err(|error| error.to_string())
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let winner = results.into_iter().find_map(Result::ok).unwrap();
    assert_eq!(
        s.get_record(&record.id).unwrap().unwrap().values["date"],
        winner.values["date"]
    );
    drop(s);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn empty_document_converts_atomically_without_changing_identity() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Conversion", "").unwrap();
    let parent = s
        .create_page(&w.id, None, "Parent", "", PageKind::Document)
        .unwrap();
    let mut page = s
        .create_page(
            &w.id,
            Some(&parent.id),
            "Conversion marker",
            "old",
            PageKind::Document,
        )
        .unwrap();
    page.settings.small_text = true;
    s.update_page(&page).unwrap();
    let def = definition();
    let result = s.create_database_on_page(&page.id, def.clone()).unwrap();
    assert_eq!(result.id, page.id);
    assert_eq!(result.title, page.title);
    assert_eq!(result.parent_id, page.parent_id);
    assert_eq!(result.settings, page.settings);
    assert_eq!(result.kind, PageKind::Database);
    assert_eq!(result.icon, "▦");
    let db = s.get_database(&def.id).unwrap().unwrap();
    assert_eq!(db.page_id, page.id);
    assert_eq!(db.name, page.title);
    assert!(
        s.search("Conversion marker", Some(&w.id), 10)
            .unwrap()
            .iter()
            .any(|hit| hit.page_id == page.id && hit.kind == PageKind::Database)
    );
}

#[test]
fn document_conversion_rejects_content_trash_and_rolls_back_insert_failure() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Conversion", "").unwrap();
    let duplicate = definition();
    s.create_database_page(&w.id, None, "Existing", "", duplicate.clone())
        .unwrap();
    let page = s
        .create_page(&w.id, None, "Unchanged marker", "old", PageKind::Document)
        .unwrap();
    assert!(s.create_database_on_page(&page.id, duplicate).is_err());
    assert_eq!(s.get_page(&page.id).unwrap().unwrap(), page);
    assert!(s.get_database_by_page(&page.id).unwrap().is_none());
    assert!(
        s.search("Unchanged marker", Some(&w.id), 10)
            .unwrap()
            .iter()
            .any(|hit| hit.page_id == page.id && hit.kind == PageKind::Document)
    );
    let block = Block::paragraph(&page.id, "content", 0);
    s.save_blocks(&page.id, &[block]).unwrap();
    assert!(s.create_database_on_page(&page.id, definition()).is_err());
    assert_eq!(s.get_page(&page.id).unwrap().unwrap(), page);
    let trashed = s
        .create_page(&w.id, None, "Trash", "", PageKind::Document)
        .unwrap();
    s.delete_page(&trashed.id).unwrap();
    assert!(
        s.create_database_on_page(&trashed.id, definition())
            .is_err()
    );
    assert!(s.get_database_by_page(&trashed.id).unwrap().is_none());
    let empty = s
        .create_page(&w.id, None, "", "", PageKind::Document)
        .unwrap();
    let empty_def = definition();
    let converted = s
        .create_database_on_page(&empty.id, empty_def.clone())
        .unwrap();
    assert!(converted.title.is_empty());
    assert_eq!(
        s.get_database(&empty_def.id).unwrap().unwrap().name,
        "Untitled database"
    );
    let mut invalid = definition();
    invalid.page_id = page.id.clone();
    assert!(s.create_database_on_page(&page.id, invalid).is_err());
}
