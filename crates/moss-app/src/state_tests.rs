use super::*;

use rune::{Env, Key, KeyEvent, Mods, Runtime};

#[test]
fn stale_open_page_keeps_its_draft_when_another_editor_saves() {
    let path = std::env::temp_dir().join(format!("moss-shared-{}", new_id()));
    let mut first = State::open(path.clone(), None).unwrap();
    let page_id = first.tab().unwrap().page.id.clone();
    first.tabs[first.active].blocks[0].edit.replace_text("original");
    first.save_blocks();
    let mut second = State::open(path.clone(), Some(&first.workspace.id)).unwrap();
    second.open_page(&page_id, false);
    first.tabs[first.active].blocks[0].edit.replace_text("first editor");
    first.save_blocks();
    second.tabs[second.active].blocks[0].edit.replace_text("second draft");
    second.save_blocks();
    assert!(second.tab().unwrap().dirty_blocks);
    assert_eq!(second.tab().unwrap().blocks[0].edit.text, "second draft");
    assert_eq!(first.store.get_blocks(&page_id).unwrap()[0].text, "first editor");
    drop(first);
    drop(second);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn collaboration_refreshes_another_block_without_disturbing_focus() {
    let path = std::env::temp_dir().join(format!("moss-live-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    let page_id = state.tab().unwrap().page.id.clone();
    state.tabs[state.active].blocks[0].edit.replace_text("first");
    state.save_blocks();
    state.add_block(0);
    state.tabs[state.active].blocks[1].edit.replace_text("second");
    state.save_blocks();
    state.tabs[state.active].blocks[1].edit.focused = false;
    state.tabs[state.active].blocks[0].edit.focused = true;
    state.tabs[state.active].blocks[0].edit.set_caret(2);
    state.collaboration_live = true;
    let mut rt = Runtime::new(state, crate::ui::view, (1200., 900.), Env::default());
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    let mut other = Store::open(path.join("workspace.sqlite3")).unwrap();
    let expected = other.get_blocks(&page_id).unwrap();
    let mut changed = expected.clone();
    changed[1].text = "changed elsewhere".into();
    other.save_editor_blocks_if_unchanged(&page_id, &expected, &changed).unwrap();
    rt.frame(0.3);
    assert_eq!(rt.state.tab().unwrap().blocks[1].edit.text, "changed elsewhere");
    assert_eq!(rt.state.tab().unwrap().blocks[0].edit.focus, 2);
    let expected = rt.state.store.get_blocks(&page_id).unwrap();
    let mut changed = expected.clone();
    changed[1].text = "changed before disconnect".into();
    rt.state.store.save_editor_blocks_if_unchanged(&page_id, &expected, &changed).unwrap();
    rt.state.collaboration_live = false;
    rt.state.external_refresh_pending = true;
    rt.frame(0.6);
    assert_eq!(rt.state.tab().unwrap().blocks[1].edit.text, "changed before disconnect");
    drop(other);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
}

#[test]
fn new_tab_search_cancel_and_result_open() {
    let path = std::env::temp_dir().join(format!("moss-search-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    let target = s
        .store
        .create_page(
            &s.workspace.id,
            None,
            "Unique search target",
            "",
            PageKind::Document,
        )
        .unwrap();
    s.refresh_pages();
    let mut rt = Runtime::new(
        s,
        crate::ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(crate::key);
    rt.frame(0.);
    let count = rt.state.tabs.len();
    let page_id = rt.state.tab().unwrap().page.id.clone();
    assert!(rt.find("home-button").is_none());
    assert_eq!(rt.state.tab().unwrap().page.id, page_id);
    assert_eq!(rt.state.tabs.len(), count);
    let (x, y) = rt.find("new-tab").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
    assert!(rt.state.search_new_tab);
    assert_eq!(rt.state.tabs.len(), count);
    press(&mut rt, Key::Escape);
    let (_, search_y) = rt.find("search-button").unwrap();
    let (x, workspace_y) = rt.find("workspace-picker").unwrap();
    assert!(workspace_y > search_y);
    rt.pointer_down(x, workspace_y);
    rt.pointer_up(x, workspace_y);
    rt.frame(1. / 120.);
    assert!(matches!(rt.state.modal, Some(Modal::Workspace)));
    press(&mut rt, Key::Escape);
    press(&mut rt, Key::Command('t'));
    assert!(rt.state.search_new_tab);
    assert_eq!(rt.state.tabs.len(), count);
    assert!(rt.find("search-input").is_some());
    press(&mut rt, Key::Escape);
    assert!(rt.state.modal.is_none());
    assert_eq!(rt.state.tabs.len(), count);
    press(&mut rt, Key::Command('t'));
    press(&mut rt, Key::Enter);
    assert!(rt.state.modal.is_none());
    assert!(rt.state.tab().unwrap().choosing_type);
    assert_eq!(rt.state.tabs.len(), count + 1);
    rt.state.close_tab(rt.state.active);
    rt.invalidate();
    rt.frame(0.);
    press(&mut rt, Key::Command('t'));
    press(&mut rt, Key::Text("Unique search target".into()));
    assert!(rt.state.hits.iter().any(|hit| hit.page_id == target.id));
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.tabs.len(), count + 1);
    assert_eq!(rt.state.tab().unwrap().page.id, target.id);
    assert!(rt.state.modal.is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_cell_save_blocks_search_navigation_and_page_creation() {
    let path = std::env::temp_dir().join(format!("moss-search-save-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    let target = s
        .store
        .create_page(
            &s.workspace.id,
            None,
            "Search target",
            "",
            PageKind::Document,
        )
        .unwrap();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    let date = s
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    s.edit_record(0, Some(date));
    s.cell.replace_text("invalid date");
    let old_page = s.tab().unwrap().page.id.clone();
    let old_count = s.store.list_pages(&s.workspace.id).unwrap().len();
    s.open_new_tab_search();
    s.search.replace_text("Search target");
    s.run_search();
    let index = s.hits.iter().position(|h| h.page_id == target.id).unwrap();
    s.open_search_hit(index);
    assert_eq!(s.tab().unwrap().page.id, old_page);
    assert_eq!(s.cell.text, "invalid date");
    assert!(s.editing_cell.is_some());
    assert!(s.modal.is_none());
    s.modal = Some(Modal::Search);
    s.new_name.replace_text("Should not be created");
    s.new_page(false, None);
    assert_eq!(
        s.store.list_pages(&s.workspace.id).unwrap().len(),
        old_count
    );
    assert_eq!(s.cell.text, "invalid date");
    assert!(s.editing_cell.is_some());
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn initial_gallery_view_loads_record_previews() {
    let path = std::env::temp_dir().join(format!("moss-gallery-open-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    let db_page = s.tab().unwrap().page.id.clone();
    let record_id = s.tab().unwrap().records[0].id.clone();
    let record_page = s.store.ensure_record_page(&record_id).unwrap();
    s.store
        .save_blocks(
            &record_page.id,
            &[Block::paragraph(&record_page.id, "First block", 0)],
        )
        .unwrap();
    let db = s.tab().unwrap().database.clone().unwrap();
    let gallery = db
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Gallery)
        .unwrap();
    assert!(s.select_database_view(gallery));
    s.close_tab(s.active);
    s.open_page(&db_page, true);
    assert_eq!(
        s.tab()
            .unwrap()
            .gallery_previews
            .get(&record_id)
            .map(String::as_str),
        Some("First block")
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn ordinary_search_replaces_current_tab_and_scope_resets() {
    let path = std::env::temp_dir().join(format!("moss-search-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    let target = s
        .store
        .create_page(
            &s.workspace.id,
            None,
            "Find this page",
            "",
            PageKind::Document,
        )
        .unwrap();
    s.refresh_pages();
    let count = s.tabs.len();
    s.all_workspaces = true;
    s.open_search();
    assert!(!s.all_workspaces && !s.search_new_tab);
    s.search.replace_text("Find this page");
    s.run_search();
    let index = s
        .hits
        .iter()
        .position(|hit| hit.page_id == target.id)
        .unwrap();
    s.open_search_hit(index);
    assert_eq!(s.tabs.len(), count);
    assert_eq!(s.tab().unwrap().page.id, target.id);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
