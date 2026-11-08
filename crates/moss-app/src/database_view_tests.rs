use super::*;

#[test]
fn gallery_shows_and_updates_page_covers() {
    let (mut rt, path) = database_runtime(4);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.state.open_record(0);
    let peek = rt.state.record_peek.as_ref().unwrap();
    let record_id = peek.record.id.clone();
    let page = peek.page.clone();
    rt.state.open_page_cover(&page, "cover-test");
    let cover = moss_core::PageCover::Color { color: "#b8c5d6".into() };
    rt.state.set_cover_draft(Some(cover.clone()));
    assert_eq!(rt.state.tab().unwrap().gallery_covers.get(&record_id), Some(&cover));
    assert!(rt.state.close_peek());
    rt.invalidate();
    rt.frame(1. / 120.);
    let preview = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("gallery-preview-0")).unwrap();
    assert_eq!(preview.fill.r, rune::color::srgb_decode(184. / 255.));
    assert_eq!(preview.fill.g, rune::color::srgb_decode(197. / 255.));
    click(&mut rt, "gallery-settings-button");
    click(&mut rt, "gallery-preview-page_cover");
    assert_eq!(rt.state.tab().unwrap().database.as_ref().unwrap().views[4].config["gallery_preview"], "page_cover");
    rt.state.query_records();
    assert_eq!(rt.state.tab().unwrap().gallery_covers.get(&record_id), Some(&cover));
    rt.state.open_record(0);
    let page = rt.state.record_peek.as_ref().unwrap().page.clone();
    rt.state.open_page_cover(&page, "cover-test");
    rt.state.set_cover_draft(None);
    assert!(!rt.state.tab().unwrap().gallery_covers.contains_key(&record_id));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn gallery_loads_visible_image_covers() {
    use std::{sync::Arc, time::{Duration, Instant}};
    let (mut rt, path) = database_runtime(4);
    rt.state.page_cover.set_waker(Arc::new(|| {}));
    rt.poll = Some(crate::poll);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.state.open_record(0);
    let page = rt.state.record_peek.as_ref().unwrap().page.clone();
    let png = [137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,8,2,0,0,0,144,119,83,222,0,0,0,12,73,68,65,84,120,156,99,248,207,192,0,0,3,1,1,0,201,254,146,239,0,0,0,0,73,69,78,68,174,66,96,130];
    let asset = rt.state.store.import_cover_image(&png).unwrap();
    rt.state.open_page_cover(&page, "cover-test");
    rt.state.set_cover_draft(Some(moss_core::PageCover::Image { asset: asset.clone(), position: 0.5 }));
    assert!(rt.state.close_peek());
    rt.invalidate();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        rt.invalidate();
        rt.frame(1. / 60.);
        if rt.find("gallery-cover-image-0").is_some() { break; }
        assert!(Instant::now() < deadline, "image cover did not load: errors={:?}, images={}, drawn={:?}, viewport={:?}, covers={:?}", rt.state.page_cover.image_errors, rt.state.page_cover.images.len(), rt.state.page_cover.gallery_covers, rt.state.page_cover.gallery_viewport, rt.state.tab().unwrap().gallery_covers);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(rt.state.page_cover.images.contains_key(&asset));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn gallery_settings_persist_and_cards_wrap_to_available_width() {
    let (mut rt, path) = database_runtime(4);
    for name in [
        "One",
        "A much longer title that must wrap onto more than one line in a small gallery card",
        "Three",
        "Four",
    ] {
        rt.state.create_inline(Default::default(), false);
        rt.state.cell.replace_text(name);
        assert!(rt.state.finish_cell(false));
    }
    rt.invalidate();
    rt.frame(1. / 120.);
    let first = rt.presented("record-card-0").unwrap().0;
    let third = rt.presented("record-card-2").unwrap().0;
    assert!((first[2] - 346.).abs() < 1.);
    assert!(
        third[1] > first[1],
        "medium cards should wrap: {first:?} {third:?}"
    );
    click(&mut rt, "gallery-settings-button");
    click(&mut rt, "gallery-size-large");
    assert!((rt.presented("record-card-0").unwrap().0[2] - 460.).abs() < 1.);
    click(&mut rt, "gallery-size-small");
    let first = rt.presented("record-card-0").unwrap().0;
    let third = rt.presented("record-card-2").unwrap().0;
    assert!((first[2] - 220.).abs() < 1.);
    assert!(
        (first[1] - third[1]).abs() < 1.,
        "small cards should fit: {first:?} {third:?}"
    );
    assert!(rt.presented("gallery-title-1").unwrap().0[3] > 20.);
    rt.set_viewport((850., 1000.), 1.);
    rt.frame(1. / 120.);
    let first = rt.presented("record-card-0").unwrap().0;
    let third = rt.presented("record-card-2").unwrap().0;
    assert!(
        third[1] > first[1],
        "narrow viewport should wrap: {first:?} {third:?}"
    );
    rt.set_viewport((1440., 1000.), 1.);
    rt.frame(1. / 120.);
    let heights_with_preview: std::collections::BTreeMap<_, _> = rt.state.tab().unwrap().records.iter().enumerate()
        .map(|(index, record)| (record.id.clone(), rt.presented(&format!("record-card-{index}")).unwrap().0[3]))
        .collect();
    click(&mut rt, "gallery-preview-none");
    click(&mut rt, "gallery-limit-10");
    for (index, record) in rt.state.tab().unwrap().records.iter().enumerate() {
        let before = heights_with_preview[&record.id];
        let after = rt.presented(&format!("record-card-{index}")).unwrap().0[3];
        assert!((before - after - 125.).abs() < 1., "{}: {before} -> {after}", record.title);
    }
    let db_id = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .id
        .clone();
    let db = rt.state.store.get_database(&db_id).unwrap().unwrap();
    let view = &db.views[4];
    assert_eq!(view.config["gallery_card_size"], "small");
    assert_eq!(view.config["gallery_preview"], "none");
    assert_eq!(view.config["load_limit"], 10);
    rt.state.gallery_settings_open = false;
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "gallery-new");
    assert!(matches!(
        rt.state.record_peek.as_ref().unwrap().mode,
        state::PeekMode::Center
    ));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn gallery_load_limit_pages_without_skipping_records() {
    let (mut rt, path) = database_runtime(4);
    let db_id = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .id
        .clone();
    for i in 0..56 {
        rt.state
            .store
            .create_record(&db_id, &format!("Row {i:02}"), &Default::default())
            .unwrap();
    }
    rt.state.query_records();
    assert_eq!(rt.state.tab().unwrap().records.len(), 50);
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "gallery-settings-button");
    click(&mut rt, "gallery-limit-10");
    assert_eq!(rt.state.tab().unwrap().records.len(), 10);
    let first: Vec<_> = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .map(|r| r.id.clone())
        .collect();
    rt.state.gallery_settings_open = false;
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "db-next");
    assert_eq!(rt.state.tab().unwrap().offset, 10);
    let second: Vec<_> = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .map(|r| r.id.clone())
        .collect();
    assert_eq!(second.len(), 10);
    assert!(second.iter().all(|id| !first.contains(id)));
    click(&mut rt, "db-previous");
    assert_eq!(rt.state.tab().unwrap().offset, 0);
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .records
            .iter()
            .map(|r| r.id.clone())
            .collect::<Vec<_>>(),
        first
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn gallery_page_preview_loads_and_updates_after_peek_save() {
    let (mut rt, path) = database_runtime(4);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.state.open_record(0);
    let record = rt.state.record_peek.as_ref().unwrap().record.clone();
    let page_id = rt.state.record_peek.as_ref().unwrap().page.id.clone();
    assert!(rt.state.close_peek());
    let blocks = vec![moss_core::Block::paragraph(
        &page_id,
        "First line\nSecond line",
        0,
    )];
    rt.state.store.save_blocks(&page_id, &blocks).unwrap();
    rt.state.query_records();
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .gallery_previews
            .get(&record.id)
            .unwrap(),
        "First line\nSecond line"
    );
    rt.state.open_record(0);
    rt.state.record_peek.as_mut().unwrap().blocks[0]
        .edit
        .replace_text("Updated preview");
    rt.state.save_peek_blocks();
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .gallery_previews
            .get(&record.id)
            .unwrap(),
        "Updated preview"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn table_search_keeps_saved_view_filter() {
    let (mut rt, path) = database_runtime(0);
    let db_id = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .id
        .clone();
    let status = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    for (name, value) in [
        ("Match one", "Done"),
        ("Match two", "Not started"),
        ("Other", "Done"),
    ] {
        rt.state
            .store
            .create_record(
                &db_id,
                name,
                &std::collections::BTreeMap::from([(status.clone(), json!(value))]),
            )
            .unwrap();
    }
    let tab = &mut rt.state.tabs[rt.state.active];
    tab.database.as_mut().unwrap().views[0].query.filter = Some(FilterExpr::Rule(FilterRule {
        property_id: status,
        op: FilterOp::Eq,
        value: json!("Done"),
    }));
    rt.state.filter.replace_text("Match");
    assert!(rt.state.query_records());
    let records = &rt.state.tab().unwrap().records;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].title, "Match one");
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn paging_does_not_offer_empty_next_or_change_offset_on_failure() {
    let (mut rt, path) = database_runtime(4);
    let db_id = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .id
        .clone();
    for i in 0..20 {
        rt.state
            .store
            .create_record(&db_id, &format!("Row {i:02}"), &Default::default())
            .unwrap();
    }
    rt.state.tabs[rt.state.active]
        .database
        .as_mut()
        .unwrap()
        .views[4]
        .config["load_limit"] = json!(10);
    assert!(rt.state.query_records());
    rt.invalidate();
    rt.frame(1. / 120.);
    assert!(rt.find("db-next").is_some());
    click(&mut rt, "db-next");
    assert_eq!(rt.state.tab().unwrap().offset, 10);
    assert_eq!(rt.state.tab().unwrap().records.len(), 10);
    assert!(rt.find("db-next").is_none());
    let ids: Vec<_> = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .map(|r| r.id.clone())
        .collect();
    rt.state.tabs[rt.state.active]
        .database
        .as_mut()
        .unwrap()
        .views[4]
        .query
        .filter = Some(FilterExpr::Rule(FilterRule {
        property_id: "missing".into(),
        op: FilterOp::Eq,
        value: json!("x"),
    }));
    rt.state.move_record_page(0);
    assert_eq!(rt.state.tab().unwrap().offset, 10);
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .records
            .iter()
            .map(|r| r.id.clone())
            .collect::<Vec<_>>(),
        ids
    );
    assert!(rt.state.error.is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn picker_target_survives_a_previous_cell_blur() {
    let (mut rt, path) = database_runtime(0);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    let tab = rt.state.tab().unwrap();
    let id = tab.records[0].id.clone();
    let status = tab
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    rt.state.open_property_picker(&id, &status);
    rt.state.cell.focused = false;
    rt.frame(1. / 120.);
    assert!(rt.state.editing_cell.is_some());
    assert!(rt.state.choose_property_value("In progress"));
    assert_eq!(
        rt.state.store.get_record(&id).unwrap().unwrap().values[&status],
        "In progress"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn gallery_preview_tracks_full_page_edits_and_preserves_dirty_tab() {
    let (mut rt, path) = database_runtime(4);
    let gallery_tab = rt.state.active;
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.state.open_record(0);
    let id = rt.state.record_peek.as_ref().unwrap().record.id.clone();
    rt.state.expand_peek();
    let page_tab = rt.state.active;
    assert_ne!(page_tab, gallery_tab);
    rt.state.tabs[page_tab].blocks[0]
        .edit
        .replace_text("From full page");
    rt.state.save_blocks();
    assert_eq!(
        rt.state.tabs[gallery_tab]
            .gallery_previews
            .get(&id)
            .map(String::as_str),
        Some("From full page")
    );
    rt.state.active = gallery_tab;
    rt.state.open_record(0);
    rt.state.tabs[page_tab].blocks[0]
        .edit
        .replace_text("Unsaved draft");
    rt.state.tabs[page_tab].dirty_blocks = true;
    rt.state.record_peek.as_mut().unwrap().blocks[0]
        .edit
        .replace_text("Peek saved");
    rt.state.save_peek_blocks();
    assert_eq!(rt.state.tabs[page_tab].blocks[0].edit.text, "Unsaved draft");
    assert!(rt.state.tabs[page_tab].dirty_blocks);
    assert_eq!(
        rt.state.tabs[gallery_tab]
            .gallery_previews
            .get(&id)
            .map(String::as_str),
        Some("Peek saved")
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
