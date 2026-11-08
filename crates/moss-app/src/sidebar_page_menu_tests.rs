use super::*;
use moss_core::PageKind;
use rune::PointerPress;

fn point() -> PointerPress {
    PointerPress { x: 20., y: 30. }
}

#[test]
fn renaming_other_page_preserves_active_selection_and_reports_conflict() {
    let (mut s, path) = fixture();
    let active_id = s.tab().unwrap().page.id.clone();
    let page = s
        .store
        .create_page(&s.workspace.id, None, "Original", "", PageKind::Document)
        .unwrap();
    let active = s.active;
    s.tabs[active].blocks[0].edit.set_selection(0, 4);
    s.open_sidebar_page_menu(&page.id, point());
    assert_eq!(s.tab().unwrap().page.id, active_id);
    assert_eq!(s.tab().unwrap().blocks[0].edit.range(), (0, 4));
    s.sidebar_page_name.replace_text("Renamed");
    assert!(s.rename_sidebar_page());
    assert_eq!(
        s.store.get_page(&page.id).unwrap().unwrap().title,
        "Renamed"
    );
    assert_eq!(s.tab().unwrap().blocks[0].edit.range(), (0, 4));

    s.open_sidebar_page_menu(&page.id, point());
    s.sidebar_page_name.replace_text("My draft");
    s.store
        .rename_page_if_unchanged(&page.id, "Renamed", "Other window")
        .unwrap();
    assert!(!s.rename_sidebar_page());
    assert_eq!(s.sidebar_page_name.text, "My draft");
    assert!(s.sidebar_page_menu.as_ref().unwrap().error.is_some());
    assert_eq!(
        s.store.get_page(&page.id).unwrap().unwrap().title,
        "Other window"
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn trashing_a_parent_closes_descendant_tabs_but_keeps_unrelated_active_page() {
    let (mut s, path) = fixture();
    let unrelated = s.tab().unwrap().page.id.clone();
    let parent = s
        .store
        .create_page(&s.workspace.id, None, "Parent", "", PageKind::Document)
        .unwrap();
    let child = s
        .store
        .create_page(
            &s.workspace.id,
            Some(&parent.id),
            "Child",
            "",
            PageKind::Document,
        )
        .unwrap();
    s.open_page(&parent.id, true);
    s.open_page(&child.id, true);
    s.open_page(&unrelated, false);
    let active = s.active;
    s.tabs[active].blocks[0].edit.set_selection(0, 4);
    s.open_sidebar_page_menu(&parent.id, point());
    assert!(s.trash_page_by_id(&parent.id));
    assert_eq!(s.tab().unwrap().page.id, unrelated);
    assert_eq!(s.tab().unwrap().blocks[0].edit.range(), (0, 4));
    assert!(
        s.tabs
            .iter()
            .all(|tab| tab.page.id != parent.id && tab.page.id != child.id)
    );
    assert!(
        s.store
            .get_page(&parent.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    assert!(
        s.store
            .get_page(&child.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_descendant_save_keeps_draft_and_parent_out_of_trash() {
    let (mut s, path) = fixture();
    let parent = s
        .store
        .create_page(&s.workspace.id, None, "Parent", "", PageKind::Document)
        .unwrap();
    let child = s
        .store
        .create_page(
            &s.workspace.id,
            Some(&parent.id),
            "Child",
            "",
            PageKind::Document,
        )
        .unwrap();
    s.open_page(&child.id, true);
    let index = s.active;
    s.tabs[index].blocks[0].edit.replace_text("Unsaved draft");
    s.tabs[index].dirty_blocks = true;
    s.open_sidebar_page_menu(&parent.id, point());
    s.store.delete_page(&child.id).unwrap();
    assert!(!s.trash_page_by_id(&parent.id));
    assert_eq!(s.tabs[index].blocks[0].edit.text, "Unsaved draft");
    assert!(s.tabs[index].dirty_blocks);
    assert!(s.sidebar_page_menu.as_ref().unwrap().error.is_some());
    assert!(
        s.store
            .get_page(&parent.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_none()
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
