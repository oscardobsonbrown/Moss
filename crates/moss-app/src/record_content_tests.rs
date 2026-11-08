use super::*;

#[test]
fn peek_and_full_page_body_saves_update_cached_content_presence() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db_tab = s.active;
    s.create_peek(Default::default());
    let peek = s.record_peek.as_ref().unwrap();
    let record = peek.record.id.clone();
    let page = peek.page.id.clone();
    assert!(
        !s.tabs[db_tab]
            .record_has_content
            .get(&record)
            .copied()
            .unwrap_or(false)
    );
    s.record_peek.as_mut().unwrap().blocks[0]
        .edit
        .replace_text("A note");
    s.save_peek_blocks();
    assert_eq!(s.tabs[db_tab].record_has_content.get(&record), Some(&true));
    s.record_peek.as_mut().unwrap().blocks[0]
        .edit
        .replace_text("");
    s.save_peek_blocks();
    assert_eq!(s.tabs[db_tab].record_has_content.get(&record), Some(&false));
    s.expand_peek();
    assert_eq!(s.tab().unwrap().page.id, page);
    let active = s.active;
    s.tabs[active].blocks[0]
        .edit
        .replace_text("Body in full page");
    s.save_blocks();
    assert_eq!(s.tabs[db_tab].record_has_content.get(&record), Some(&true));
    s.tabs[active].blocks[0].edit.replace_text("");
    s.save_blocks();
    assert_eq!(s.tabs[db_tab].record_has_content.get(&record), Some(&false));
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn computed_refresh_reloads_content_presence_for_loaded_records() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.create_peek(Default::default());
    let record = s.record_peek.as_ref().unwrap().record.id.clone();
    let page = s.record_peek.as_ref().unwrap().page.id.clone();
    assert!(s.close_peek());
    assert!(s.query_records());
    s.computed_change_count = s.store.local_change_count();
    let mut block = moss_core::Block::paragraph(&page, "Heading", 0);
    block.kind = moss_core::BlockKind::Heading1;
    s.store.save_blocks(&page, &[block]).unwrap();
    assert!(crate::computed_refresh::poll(&mut s));
    assert_eq!(
        s.tab().unwrap().record_has_content.get(&record),
        Some(&true)
    );
    s.store.save_blocks(&page, &[]).unwrap();
    assert!(crate::computed_refresh::poll(&mut s));
    assert_eq!(
        s.tab().unwrap().record_has_content.get(&record),
        Some(&false)
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
