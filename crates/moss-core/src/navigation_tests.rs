use crate::*;
use std::collections::BTreeMap;

#[test]
fn manual_page_order_survives_paging_and_rejects_other_parents() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Order", "").unwrap();
    let pages: Vec<_> = (0..7)
        .map(|n| {
            s.create_page(&w.id, None, &format!("{n}"), "", PageKind::Document)
                .unwrap()
        })
        .collect();
    let child = s
        .create_page(&w.id, Some(&pages[0].id), "Child", "", PageKind::Document)
        .unwrap();
    s.reorder_sidebar_page(&w.id, &pages[6].id, Some(&pages[0].id))
        .unwrap();
    s.reorder_sidebar_page(&w.id, &pages[1].id, None).unwrap();
    let mut stale = pages[6].clone();
    stale.title = "Renamed".into();
    s.update_page(&stale).unwrap();
    assert!(
        s.reorder_sidebar_page(&w.id, &pages[0].id, Some(&child.id))
            .is_err()
    );
    let expected: Vec<_> = [6, 0, 2, 3, 4, 5, 1]
        .into_iter()
        .map(|i| pages[i].id.clone())
        .collect();
    let mut seen = Vec::new();
    let mut group = s
        .page_children(&w.id, None, PagePosition::Start, 2)
        .unwrap();
    loop {
        seen.extend(group.items.iter().map(|p| p.page.id.clone()));
        let Some(next) = group.next else { break };
        let next_group = s
            .page_children(&w.id, None, PagePosition::After(&next), 2)
            .unwrap();
        let previous = s
            .page_children(
                &w.id,
                None,
                PagePosition::Before(next_group.previous.as_ref().unwrap()),
                2,
            )
            .unwrap();
        assert_eq!(
            previous
                .items
                .iter()
                .map(|p| &p.page.id)
                .collect::<Vec<_>>(),
            group.items.iter().map(|p| &p.page.id).collect::<Vec<_>>()
        );
        group = next_group;
    }
    assert_eq!(seen, expected);
    for _ in 0..200 {
        s.reorder_sidebar_page(&w.id, &pages[6].id, Some(&pages[0].id))
            .unwrap();
        s.reorder_sidebar_page(&w.id, &pages[0].id, Some(&pages[6].id))
            .unwrap();
    }
}

#[test]
fn child_cursor_crosses_equal_and_distinct_timestamps_in_both_directions() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Cursor", "").unwrap();
    for n in 0..5 {
        let page = s
            .create_page(&w.id, None, &format!("Page {n}"), "", PageKind::Document)
            .unwrap();
        let time = format!("2026-01-0{}T00:00:00Z", n / 2 + 1);
        s.conn
            .execute(
                "UPDATE pages SET data=json_set(data,'$.created_at',?1) WHERE id=?2",
                rusqlite::params![time, page.id],
            )
            .unwrap();
    }
    let expected = s.list_pages(&w.id).unwrap();
    let first = s
        .page_children(&w.id, None, PagePosition::Start, 3)
        .unwrap();
    let next = s
        .page_children(
            &w.id,
            None,
            PagePosition::After(first.next.as_ref().unwrap()),
            3,
        )
        .unwrap();
    assert_eq!(next.items.len(), 2);
    assert!(next.next.is_none());
    assert_eq!(next.items[0].page.id, expected[3].id);
    assert_eq!(next.items[1].page.id, expected[4].id);
    let back = s
        .page_children(
            &w.id,
            None,
            PagePosition::Before(next.previous.as_ref().unwrap()),
            3,
        )
        .unwrap();
    assert!(back.previous.is_none());
    assert_eq!(
        back.items.iter().map(|p| &p.page.id).collect::<Vec<_>>(),
        expected[..3].iter().map(|p| &p.id).collect::<Vec<_>>()
    );
    assert!(back.next.is_some());
}

#[test]
fn child_pages_are_bounded_ordered_reversible_and_workspace_scoped() {
    let mut s = Store::in_memory().unwrap();
    let workspace = s.create_workspace("Tree", "").unwrap();
    let other = s.create_workspace("Other", "").unwrap();
    let mut roots = Vec::new();
    for n in 0..7 {
        roots.push(
            s.create_page(
                &workspace.id,
                None,
                &format!("Root {n}"),
                "",
                PageKind::Document,
            )
            .unwrap(),
        );
    }
    let child = s
        .create_page(
            &workspace.id,
            Some(&roots[0].id),
            "Child",
            "",
            PageKind::Document,
        )
        .unwrap();
    let trashed_child = s
        .create_page(
            &workspace.id,
            Some(&roots[1].id),
            "Trashed",
            "",
            PageKind::Document,
        )
        .unwrap();
    s.delete_page(&trashed_child.id).unwrap();
    s.delete_page(&roots[6].id).unwrap();
    let foreign = s
        .create_page(&other.id, None, "Foreign", "", PageKind::Document)
        .unwrap();
    // Equal timestamps exercise the ID tie-breaker at every page boundary.
    s.conn
        .execute(
            "UPDATE pages SET data=json_set(data,'$.created_at','2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    let expected: Vec<_> = s
        .list_pages(&workspace.id)
        .unwrap()
        .into_iter()
        .filter(|p| p.parent_id.is_none())
        .map(|p| p.id)
        .collect();
    let mut batch = s
        .page_children(&workspace.id, None, PagePosition::Start, 2)
        .unwrap();
    assert!(batch.previous.is_none());
    let mut seen = Vec::new();
    loop {
        assert!(batch.items.len() <= 2);
        seen.extend(batch.items.iter().map(|item| item.page.id.clone()));
        let Some(cursor) = batch.next.clone() else {
            break;
        };
        let next = s
            .page_children(&workspace.id, None, PagePosition::After(&cursor), 2)
            .unwrap();
        let back = s
            .page_children(
                &workspace.id,
                None,
                PagePosition::Before(next.previous.as_ref().unwrap()),
                2,
            )
            .unwrap();
        assert_eq!(
            back.items
                .iter()
                .map(|item| &item.page.id)
                .collect::<Vec<_>>(),
            batch
                .items
                .iter()
                .map(|item| &item.page.id)
                .collect::<Vec<_>>()
        );
        batch = next;
        assert!(seen.len() <= expected.len());
    }
    assert_eq!(seen, expected);
    let all = s
        .page_children(&workspace.id, None, PagePosition::Start, 20)
        .unwrap();
    assert!(
        all.items
            .iter()
            .find(|item| item.page.id == roots[0].id)
            .unwrap()
            .has_children
    );
    assert!(
        !all.items
            .iter()
            .find(|item| item.page.id == roots[1].id)
            .unwrap()
            .has_children
    );
    let children = s
        .page_children(&workspace.id, Some(&roots[0].id), PagePosition::Start, 2)
        .unwrap();
    assert_eq!(children.items[0].page.id, child.id);
    assert!(!children.items[0].has_children);
    assert!(
        s.page_children(&workspace.id, Some(&foreign.id), PagePosition::Start, 2)
            .is_err()
    );
    assert!(
        s.page_children(&workspace.id, Some(&roots[6].id), PagePosition::Start, 2)
            .is_err()
    );
    assert!(
        s.page_children(&workspace.id, Some("missing"), PagePosition::Start, 2)
            .is_err()
    );
    for limit in [0, 201, u32::MAX] {
        assert!(
            s.page_children(&workspace.id, None, PagePosition::Start, limit)
                .is_err()
        );
        assert!(s.page_suggestions(&workspace.id, limit).is_err());
    }
}

#[test]
fn suggestions_include_nested_pages_but_exclude_trash_and_record_pages() {
    let mut s = Store::in_memory().unwrap();
    let w = s.create_workspace("Search", "").unwrap();
    let owner = s
        .create_page(&w.id, None, "Database", "", PageKind::Database)
        .unwrap();
    let db = Database {
        id: new_id(),
        page_id: owner.id.clone(),
        name: "Database".into(),
        properties: vec![Property {
            id: "title".into(),
            name: "Name".into(),
            kind: PropertyKind::Title,
            config: serde_json::json!({}),
        }],
        views: vec![],
    };
    s.save_database(&db).unwrap();
    let record = s.create_record(&db.id, "Record", &BTreeMap::new()).unwrap();
    let record_page = s.ensure_record_page(&record.id).unwrap();
    let child = s
        .create_page(&w.id, Some(&owner.id), "Nested", "", PageKind::Document)
        .unwrap();
    let record_child = s
        .create_page(
            &w.id,
            Some(&record_page.id),
            "Record child",
            "",
            PageKind::Document,
        )
        .unwrap();
    let trash = s
        .create_page(&w.id, None, "Deleted", "", PageKind::Document)
        .unwrap();
    s.delete_page(&trash.id).unwrap();
    let suggestions = s.page_suggestions(&w.id, 20).unwrap();
    assert_eq!(suggestions.len(), 3);
    assert!(suggestions.iter().any(|p| p.id == child.id));
    assert!(
        !suggestions
            .iter()
            .any(|p| p.id == record_page.id || p.id == trash.id)
    );
    assert_eq!(s.page_suggestions(&w.id, 1).unwrap().len(), 1);
    let record_only = s
        .page_children(&w.id, Some(&owner.id), PagePosition::Start, 20)
        .unwrap();
    assert_eq!(record_only.items.len(), 1);
    let record_children = s
        .page_children(&w.id, Some(&record_page.id), PagePosition::Start, 20)
        .unwrap();
    assert_eq!(record_children.items[0].page.id, record_child.id);
    s.delete_page(&child.id).unwrap();
    let roots = s
        .page_children(&w.id, None, PagePosition::Start, 20)
        .unwrap();
    assert!(!roots.items[0].has_children);
}
