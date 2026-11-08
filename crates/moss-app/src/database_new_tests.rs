use super::*;

fn runtime() -> (Runtime<State>, PathBuf) {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.cell.replace_text("Example record");
    s.finish_cell(false);
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.set_full_keyboard_access(true);
    rt.frame(0.);
    (rt, path)
}

#[test]
fn new_menu_saves_opening_mode_per_view_and_full_page_opens_linked_record() {
    let (mut rt, path) = runtime();
    let database_page = rt.state.tab().unwrap().page.id.clone();
    let record_id = rt.state.tab().unwrap().records[0].id.clone();
    click(&mut rt, "db-new-options");
    assert!(rt.state.database_new_open);
    click(&mut rt, "db-new-mode-full");
    assert!(!rt.state.database_new_open);
    let db = rt.state.tab().unwrap().database.as_ref().unwrap();
    assert_eq!(db.views[0].config["open_mode"], "full");
    assert!(db.views[1].config.get("open_mode").is_none());
    let stored = rt.state.store.get_database(&db.id).unwrap().unwrap();
    assert_eq!(stored.views[0].config["open_mode"], "full");
    rt.frame(1.);
    // The same compact table control now opens the linked page as a full tab.
    let cell = rt.presented("cell-0-title").unwrap().0;
    rt.pointer_move(cell[0] + 15., cell[1] + 15.);
    rt.frame(0.);
    click(&mut rt, "table-open-0");
    assert!(rt.state.record_peek.is_none());
    assert_eq!(rt.state.tab().unwrap().page.kind, PageKind::Record);
    let linked = rt.state.store.ensure_record_page(&record_id).unwrap();
    assert_eq!(rt.state.tab().unwrap().page.id, linked.id);
    rt.state.open_page(&database_page, false);
    rt.invalidate();
    rt.frame(0.);
    let before = rt.state.tab().unwrap().records.len();
    click(&mut rt, "db-new");
    assert_eq!(rt.state.tab().unwrap().page.kind, PageKind::Record);
    assert_eq!(
        rt.state
            .tabs
            .iter()
            .find(|t| t.page.id == database_page)
            .unwrap()
            .records
            .len(),
        before + 1
    );
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let db = reopened
        .tabs
        .iter()
        .find(|t| t.page.id == database_page)
        .unwrap()
        .database
        .as_ref()
        .unwrap();
    assert_eq!(db.views[0].config["open_mode"], "full");
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn empty_menu_item_creates_record_and_failed_settings_keep_menu_open() {
    let (mut rt, path) = runtime();
    click(&mut rt, "db-new-options");
    click(&mut rt, "db-new-mode-center");
    rt.frame(1.);
    click(&mut rt, "db-new-options");
    let before = rt.state.tab().unwrap().records.len();
    click(&mut rt, "db-new-empty");
    assert_eq!(rt.state.tab().unwrap().records.len(), before + 1);
    assert!(rt.state.record_peek.as_ref().unwrap().mode == state::PeekMode::Center);
    rt.state.close_peek();
    rt.invalidate();
    rt.frame(1.);
    click(&mut rt, "db-new-options");
    // A concurrent view type change must not be overwritten by a stale menu.
    let mut changed = rt.state.tab().unwrap().database.as_ref().unwrap().clone();
    changed.views[0].kind = ViewKind::List;
    changed.views[0].config["unrelated"] = json!(42);
    rt.state.store.save_database(&changed).unwrap();
    click(&mut rt, "db-new-mode-side");
    assert!(rt.state.database_new_open);
    assert!(rt.state.error.is_some());
    let saved = rt.state.store.get_database(&changed.id).unwrap().unwrap();
    assert_eq!(saved.views[0].kind, ViewKind::List);
    assert_eq!(saved.views[0].config["unrelated"], 42);
    assert_eq!(saved.views[0].config["open_mode"], "center");
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn new_menu_pointer_fades_keyboard_is_instant_and_invalid_cell_blocks_full_creation() {
    let (mut rt, path) = runtime();
    click(&mut rt, "db-new-options");
    assert!(rt.presented("db-new-menu").unwrap().1 < 0.95);
    press(&mut rt, Key::Escape);
    assert!(rt.find("db-new-menu").is_none());
    press(&mut rt, Key::Text(" ".into()));
    assert!(rt.state.database_new_open);
    assert_eq!(rt.presented("db-new-menu").unwrap().1, 1.);
    click(&mut rt, "db-new-mode-full");
    rt.frame(1.);
    let date = rt
        .state
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
    rt.state.edit_record(0, Some(date));
    rt.state.cell.replace_text("invalid date");
    let before = rt.state.tab().unwrap().records.len();
    rt.state.add_record();
    assert_eq!(rt.state.tab().unwrap().records.len(), before);
    assert!(rt.state.error.is_some());
    assert_eq!(rt.state.cell.text, "invalid date");
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
