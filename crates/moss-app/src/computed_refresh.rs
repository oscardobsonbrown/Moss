use crate::{records::view_load_limit, state::State};
use moss_core::{FilterExpr, FilterOp, FilterRule, PropertyKind, ViewKind};
use serde_json::json;
use std::collections::HashMap;

/// Refresh visible rows after local writes without replacing an active cell draft.
pub fn poll(s: &mut State) -> bool {
    let count = s.store.local_change_count();
    let editing = s.editing_cell.is_some()
        || s.property_picker.is_some()
        || s.advanced_property.relation_picker.is_some()
        || s.record_peek.is_some();
    if count == s.computed_change_count && !(s.computed_refresh_deferred && !editing) {
        return false;
    }
    let mut changed = false;
    let mut error = None;
    for index in 0..s.tabs.len() {
        let Some(db) = s.tabs[index].database.clone() else {
            continue;
        };
        if s.tabs[index].table_window.is_some() && !crate::table_window::supported(&s.tabs[index]) {
            s.tabs[index].table_window = None;
        }
        if s.tabs[index].table_window.is_some() {
            crate::table_window::mark_changed(s, index);
            // Body saves must update visible page icons while the window worker
            // prepares its next snapshot. This query is bounded by the cache.
            let ids: Vec<_> = s.tabs[index].records.iter().map(|r| r.id.clone()).collect();
            match s.store.record_page_appearance(&s.workspace.id, &ids) {
                Ok(appearance) => {
                    let (icons, content) = crate::records::split_record_appearance(appearance);
                    let tab = &mut s.tabs[index];
                    changed |= tab.record_icons != icons || tab.record_has_content != content;
                    tab.record_icons = icons;
                    tab.record_has_content = content;
                }
                Err(e) => error = Some(format!("Page icons are not refreshed: {e}")),
            }
            if !(editing && index == s.active) {
                continue;
            }
        }
        if editing && index == s.active {
            let ids: Vec<_> = s.tabs[index].records.iter().map(|r| r.id.clone()).collect();
            match s.store.record_snapshots(&db.id, &ids) {
                Ok(rows) => {
                    let mut rows: HashMap<_, _> =
                        rows.into_iter().map(|r| (r.id.clone(), r)).collect();
                    for record in &mut s.tabs[index].records {
                        if let Some(fresh) = rows.remove(&record.id) {
                            changed |= *record != fresh;
                            *record = fresh;
                        }
                    }
                }
                Err(e) => error = Some(format!("Records are not refreshed: {e}")),
            }
            continue;
        }
        let tab = &s.tabs[index];
        let mut query = db
            .views
            .get(tab.view)
            .map(|v| v.query.clone())
            .unwrap_or_default();
        let limit = db.views.get(tab.view).map(view_load_limit).unwrap_or(100);
        query.offset = tab.offset;
        query.limit = limit + 1;
        if !tab.title_search.is_empty() {
            let search = FilterExpr::Rule(FilterRule {
                property_id: db
                    .properties
                    .iter()
                    .find(|p| p.kind == PropertyKind::Title)
                    .map(|p| p.id.clone())
                    .unwrap_or_default(),
                op: FilterOp::Contains,
                value: json!(tab.title_search),
            });
            query.filter = Some(match query.filter.take() {
                Some(saved) => FilterExpr::All(vec![saved, search]),
                None => search,
            });
        }
        match s.store.query_records(&db.id, &query) {
            Ok(mut rows) => {
                let has_more = rows.len() > limit as usize;
                rows.truncate(limit as usize);
                let pins = s.tabs[index].created_record_ids.clone();
                if !pins.is_empty() {
                    let mut fresh: HashMap<_, _> = match s.store.record_snapshots(&db.id, &pins) {
                        Ok(records) => records.into_iter().map(|r| (r.id.clone(), r)).collect(),
                        Err(e) => {
                            error = Some(format!("Created records are not refreshed: {e}"));
                            continue;
                        }
                    };
                    rows.retain(|record| !fresh.contains_key(&record.id));
                    for (position, old) in s.tabs[index].records.iter().enumerate() {
                        if let Some(record) = fresh.remove(&old.id) {
                            rows.insert(position.min(rows.len()), record);
                        }
                    }
                }
                let ids: Vec<_> = rows.iter().map(|r| r.id.clone()).collect();
                let view = db.views.get(tab.view).map(|v| v.kind);
                let appearance = {
                    match s.store.record_page_appearance(&s.workspace.id, &ids) {
                        Ok(value) => Some(value),
                        Err(e) => {
                            error = Some(format!("Page icons are not refreshed: {e}"));
                            None
                        }
                    }
                };
                let previews = if view == Some(ViewKind::Gallery) {
                    match s.store.record_page_previews(&s.workspace.id, &ids) {
                        Ok(value) => Some(value),
                        Err(e) => {
                            error = Some(format!("Page previews are not refreshed: {e}"));
                            None
                        }
                    }
                } else {
                    Some(Default::default())
                };
                let covers = if view == Some(ViewKind::Gallery) {
                    match s.store.record_page_covers(&s.workspace.id, &ids) {
                        Ok(value) => Some(value),
                        Err(e) => {
                            error = Some(format!("Page covers are not refreshed: {e}"));
                            None
                        }
                    }
                } else {
                    Some(Default::default())
                };
                let tab = &mut s.tabs[index];
                changed |= tab.records != rows || tab.has_more_records != has_more;
                tab.records = rows;
                tab.has_more_records = has_more;
                if let Some(appearance) = appearance {
                    let (icons, content) = crate::records::split_record_appearance(appearance);
                    changed |= tab.record_icons != icons;
                    tab.record_icons = icons;
                    changed |= tab.record_has_content != content;
                    tab.record_has_content = content;
                }
                if let Some(previews) = previews {
                    changed |= tab.gallery_previews != previews;
                    tab.gallery_previews = previews;
                }
                if let Some(covers) = covers {
                    changed |= tab.gallery_covers != covers;
                    tab.gallery_covers = covers;
                }
            }
            Err(e) => error = Some(format!("Records are not refreshed: {e}")),
        }
    }
    for index in 0..s.tabs.len() {
        if s.tabs[index]
            .database
            .as_ref()
            .and_then(|db| db.views.get(s.tabs[index].view))
            .is_some_and(|view| view.kind == ViewKind::Table)
        {
            s.refresh_table_summaries_for(index);
        }
    }
    if let Some(peek) = &mut s.record_peek {
        let id = peek.record.id.clone();
        match s
            .store
            .record_snapshots(&peek.record.database_id, &[id.clone()])
        {
            Ok(rows) => {
                if let Some(record) = rows.into_iter().find(|r| r.id == id) {
                    changed |= peek.record != record;
                    peek.record = record;
                }
            }
            Err(e) => error = Some(format!("Record is not refreshed: {e}")),
        }
    }
    for index in 0..s.tabs.len() {
        let Some(db) = s.tabs[index].database.as_ref() else {
            continue;
        };
        let mut records = s.tabs[index].records.clone();
        if let Some(peek) = &s.record_peek {
            if peek.record.database_id == db.id
                && !records.iter().any(|record| record.id == peek.record.id)
            {
                records.push(peek.record.clone());
            }
        }
        match crate::relation_titles::load(&s.store, db, &records) {
            Ok(titles) => {
                let tab = &mut s.tabs[index];
                changed |= tab.relation_titles != titles;
                tab.relation_titles = titles;
            }
            Err(e) => error = Some(format!("Related titles are not refreshed: {e}")),
        }
    }
    if let Some(error) = error {
        s.error = Some(error);
        changed = true;
    }
    s.computed_refresh_deferred = editing;
    s.computed_change_count = s.store.local_change_count();
    changed
}
