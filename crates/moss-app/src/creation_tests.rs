use super::*;
use moss_core::Block;

#[test]
fn database_creation_keeps_draft_on_failure_and_saves_definition_on_retry() {
    let (mut s, path) = fixture();
    let existing = s.store.list_pages(&s.workspace.id).unwrap().len();
    let active_id = s.tab().unwrap().page.id.clone();
    s.modal = Some(state::Modal::Search);
    s.new_name.replace_text("Research database");
    s.new_page(true, Some("missing-parent".into()));
    assert_eq!(s.new_name.text, "Research database");
    assert!(matches!(s.modal, Some(state::Modal::Search)));
    assert_eq!(s.tab().unwrap().page.id, active_id);
    assert_eq!(s.store.list_pages(&s.workspace.id).unwrap().len(), existing);
    assert!(s.error.is_some());
    s.new_page(true, Some(active_id.clone()));
    let tab = s.tab().unwrap();
    let db = tab.database.as_ref().unwrap();
    assert_eq!(db.page_id, tab.page.id);
    assert_eq!(tab.page.parent_id.as_deref(), Some(active_id.as_str()));
    assert_eq!(db.properties.len(), 3);
    assert_eq!(db.views.len(), 6);
    assert!(s.new_name.text.is_empty());
    assert!(s.error.is_none());
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn new_page_is_inline_title_survives_reopen_and_sidebar_icons_align() {
    let (s, path) = fixture();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 600.),
        Env {
            system_dark: true,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    let before = rt.state.tabs.len();
    click(&mut rt, "sidebar-compose");
    assert_eq!(rt.state.tabs.len(), before + 1);
    assert!(rt.state.modal.is_none());
    assert!(rt.state.tab().unwrap().choosing_type);
    assert!(rt.find("create-document").is_some());
    assert!(rt.find("create-database").is_some());
    assert!(rt.find("create-dashboard").is_some());
    assert!(rt.find("home-button").is_none());
    assert!(rt.find("sidebar-new-page").is_none());
    let private = rt.presented("private-heading").unwrap().0;
    assert!(rt.find("add-page").is_none());
    let compose = rt.presented("sidebar-compose").unwrap().0;
    let search = rt.presented("search-button").unwrap().0;
    assert!(compose[1] < search[1] && search[1] < private[1]);
    let id = rt.state.tab().unwrap().page.id.clone();
    for icon in [
        "sidebar-search-icon".into(),
        "sidebar-icon-settings-button".into(),
        "sidebar-icon-trash-button".into(),
        "sidebar-workspace-icon".into(),
        format!("page-icon-{id}"),
    ] {
        assert_eq!(rt.presented(&icon).unwrap().0[0], private[0], "{icon}");
    }
    click(&mut rt, "new-page-title");
    press(&mut rt, Key::Text("Fresh notes".into()));
    assert_eq!(
        rt.state.store.get_page(&id).unwrap().unwrap().title,
        "Fresh notes"
    );
    drop(rt);
    let s = State::open(path.clone(), None).unwrap();
    assert_eq!(s.tab().unwrap().page.id, id);
    assert!(s.tab().unwrap().choosing_type);
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 600.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, "create-document");
    assert!(!rt.state.tab().unwrap().choosing_type);
    press(&mut rt, Key::Text("Saved page content".into()));
    assert_eq!(
        rt.state.store.get_blocks(&id).unwrap()[0].text,
        "Saved page content"
    );
    drop(rt);
    let s = State::open(path.clone(), None).unwrap();
    assert!(!s.tab().unwrap().choosing_type);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn inline_database_choice_uses_same_page_and_subpage_remains_reachable() {
    let (mut s, path) = fixture();
    s.begin_page(None);
    s.tabs[s.active].title.replace_text("Research");
    s.save_title();
    let id = s.tab().unwrap().page.id.clone();
    let count = s.store.list_pages(&s.workspace.id).unwrap().len();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, "create-database");
    assert!(rt.find("database-title").is_some());
    assert!(rt.find("page-type-chooser").is_none());
    let tab = rt.state.tab().unwrap();
    assert_eq!(tab.page.id, id);
    assert_eq!(tab.database.as_ref().unwrap().name, "Research");
    assert_eq!(tab.database.as_ref().unwrap().views.len(), 6);
    assert_eq!(
        rt.state.store.get_page(&id).unwrap().unwrap().kind,
        PageKind::Database
    );
    assert_eq!(
        rt.state
            .store
            .list_pages(&rt.state.workspace.id)
            .unwrap()
            .len(),
        count
    );
    click(&mut rt, "page-actions");
    click(&mut rt, "create-subpage");
    assert!(rt.state.modal.is_none());
    assert!(rt.state.tab().unwrap().choosing_type);
    assert_eq!(
        rt.state.tab().unwrap().page.parent_id.as_deref(),
        Some(id.as_str())
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_database_choice_keeps_page_and_document_choice_reads_new_content() {
    let (mut s, path) = fixture();
    s.begin_page(None);
    let page_id = s.tab().unwrap().page.id.clone();
    let content = Block::paragraph(&page_id, "Content added since this page opened", 0);
    s.store.save_blocks(&page_id, &[content.clone()]).unwrap();
    s.choose_page_type(true);
    assert!(s.error.is_some());
    assert!(s.tab().unwrap().choosing_type);
    assert!(s.tab().unwrap().database.is_none());
    assert_eq!(
        s.store.get_page(&page_id).unwrap().unwrap().kind,
        PageKind::Document
    );
    assert_eq!(s.store.get_blocks(&page_id).unwrap()[0], content);
    s.choose_page_type(false);
    assert!(!s.tab().unwrap().choosing_type);
    assert_eq!(s.tab().unwrap().blocks[0].edit.text, content.text);
    assert_eq!(s.store.get_blocks(&page_id).unwrap()[0], content);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn dashboard_creation_keeps_the_saved_page_and_opens_the_dashboard_view() {
    let (mut s,path)=fixture();
    s.begin_page(None);
    let id=s.tab().unwrap().page.id.clone();
    s.tabs[s.active].title.replace_text("Project dashboard");s.save_title();
    s.choose_dashboard_page();
    let tab=s.tab().unwrap();let db=tab.database.as_ref().unwrap();
    assert_eq!(tab.page.id,id);assert!(!tab.choosing_type);
    assert_eq!(db.views[0].kind,ViewKind::Dashboard);assert_eq!(db.views[0].name,"Dashboard");
    let saved=s.store.get_database(&db.id).unwrap().unwrap();assert_eq!(saved.views[0],db.views[0]);
    drop(s);std::fs::remove_dir_all(path).unwrap();
}
