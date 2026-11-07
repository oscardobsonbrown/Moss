use super::*;
use rotion_core::*;
use serde_json::json;
use std::path::PathBuf;

fn fixture() -> (State, PathBuf) {
    let path = std::env::temp_dir().join(format!("rotion-test-{}", new_id()));
    (State::open(path.clone(), None).unwrap(), path)
}
fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
}
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("missing element {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}

#[test]
fn create_edit_format_search_and_reopen_a_page() {
    let (s, path) = fixture();
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
    click(&mut rt, "add-page");
    for _ in 0..30 {
        rt.frame(1. / 120.);
    }
    click(&mut rt, "new-page-name");
    press(&mut rt, Key::Text("Project notebook".into()));
    assert_eq!(rt.state.new_name.text, "Project notebook");
    press(&mut rt, Key::Enter);
    assert_eq!(rt.state.tab().unwrap().page.title, "Project notebook");
    let block = rt.state.tab().unwrap().blocks[0].block.id.clone();
    click(&mut rt, &format!("block-input-{block}"));
    press(&mut rt, Key::Text("Keep this note offline.".into()));
    rt.state.tabs[rt.state.active].blocks[0]
        .edit
        .set_selection(0, 4);
    press(&mut rt, Key::Command('b'));
    let id = rt.state.tab().unwrap().page.id.clone();
    assert!(
        rt.state.store.get_blocks(&id).unwrap()[0].config["html"]
            .as_str()
            .unwrap()
            .contains("<strong>")
    );
    rt.state.open_search();
    rt.state.search.replace_text("offline");
    rt.state.run_search();
    assert!(rt.state.hits.iter().any(|h| h.page_id == id));
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let tab = reopened.tabs.iter().find(|t| t.page.id == id).unwrap();
    assert_eq!(tab.blocks[0].edit.text, "Keep this note offline.");
    assert!(!tab.blocks[0].edit.marks().is_empty());
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn rich_split_keeps_marks_and_replaces_selection() {
    let (mut s, path) = fixture();
    s.tabs[s.active].blocks[0].edit =
        TextEdit::from_html("<p><strong>alpha beta gamma</strong></p>");
    s.tabs[s.active].blocks[0].edit.set_selection(5, 10);
    s.add_block(0);
    assert_eq!(s.tab().unwrap().blocks[0].edit.text, "alpha");
    assert_eq!(s.tab().unwrap().blocks[1].edit.text, " gamma");
    assert!(!s.tab().unwrap().blocks[0].edit.marks().is_empty());
    assert!(!s.tab().unwrap().blocks[1].edit.marks().is_empty());
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn empty_cells_stay_empty_and_structured_cells_are_protected() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.add_record();
    s.modal = None;
    let status = s
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
    s.edit_record(0, Some(status.clone()));
    assert_eq!(s.cell.text, "");
    s.save_record();
    assert_eq!(s.tab().unwrap().records[0].values[&status], json!(""));
    s.tabs[s.active].records[0]
        .values
        .insert(status.clone(), json!({"start":"2026-11-01"}));
    s.modal = None;
    s.edit_record(0, Some(status.clone()));
    assert!(s.modal.is_none());
    assert_eq!(
        s.tab().unwrap().records[0].values[&status],
        json!({"start":"2026-11-01"})
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn closing_an_earlier_tab_keeps_the_active_page() {
    let (mut s, path) = fixture();
    s.new_name.replace_text("Second");
    s.new_page(false, None);
    s.new_name.replace_text("Third");
    s.new_page(false, None);
    let id = s.tab().unwrap().page.id.clone();
    s.close_tab(0);
    assert_eq!(s.tab().unwrap().page.id, id);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn workspace_windows_share_theme_and_keep_each_others_tabs() {
    let (mut first, path) = fixture();
    let workspace = first
        .store
        .create_workspace("Other workspace", "O")
        .unwrap();
    first.save_preferences();
    let mut second = State::open(path.clone(), Some(&workspace.id)).unwrap();
    second.shared_preferences = first.shared_preferences.clone();
    second.new_name.replace_text("Second workspace page");
    second.new_page(false, None);
    first.new_name.replace_text("First workspace page");
    first.new_page(false, None);
    first.set_theme(Some(true));
    second.save_preferences();
    let saved: state::Preferences =
        serde_json::from_slice(&std::fs::read(path.join("preferences.json")).unwrap()).unwrap();
    assert_eq!(saved.dark, Some(true));
    assert_eq!(saved.tabs[&first.workspace.id].len(), 2);
    assert_eq!(saved.tabs[&second.workspace.id].len(), 1);
    drop(first);
    drop(second);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn all_six_views_show_the_same_saved_record_on_the_first_frame() {
    let (mut s, path) = fixture();
    s.new_name.replace_text("Tasks");
    s.new_page(true, None);
    s.add_record();
    s.cell.replace_text("Plan offline work");
    s.save_record();
    let db = s.tab().unwrap().database.as_ref().unwrap();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let mut record = s.tab().unwrap().records[0].clone();
    record.values.insert(status, json!("In progress"));
    record.values.insert(date, json!("2026-11-07"));
    s.store.save_record(&record).unwrap();
    s.query_records();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    for (i, visible) in [
        "cell-0-title",
        "record-card-0",
        "calendar-0",
        "timeline-0",
        "record-card-0",
        "list-0",
    ]
    .into_iter()
    .enumerate()
    {
        click(&mut rt, &format!("db-view-{i}"));
        assert_eq!(rt.state.tab().unwrap().view, i);
        assert!(
            rt.find(visible).is_some(),
            "view {i} has no record on its first frame"
        );
        assert_eq!(rt.state.tab().unwrap().records[0].id, record.id);
    }
    assert_eq!(
        rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .title,
        "Plan offline work"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn page_appearance_is_immediate_independent_and_survives_reopen() {
    let (s, path) = fixture();
    let first = s.tab().unwrap().page.id.clone();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::FONTS, fonts::FAMILIES),
    );
    rt.frame(0.);
    let width = |rt: &Runtime<State>| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(&format!("document-{first}")))
            .unwrap()
            .target[2]
    };
    let normal = width(&rt);
    click(&mut rt, "page-actions");
    for _ in 0..30 {
        rt.frame(1. / 120.);
    }
    click(&mut rt, "appearance-menu-item");
    for _ in 0..30 {
        rt.frame(1. / 120.);
    }
    click(&mut rt, "font-serif");
    assert_eq!(rt.state.tab().unwrap().page.settings.font, PageFont::Serif);
    assert!(rt.state.modal == Some(state::Modal::Appearance));
    click(&mut rt, "page-full-width");
    assert!(width(&rt) > normal + 200.);
    click(&mut rt, "page-small-text");
    let expected = PageSettings {
        font: PageFont::Serif,
        full_width: true,
        small_text: true,
    };
    assert_eq!(
        rt.state.store.get_page(&first).unwrap().unwrap().settings,
        expected
    );
    press(&mut rt, Key::Escape);
    assert!(rt.state.modal == Some(state::Modal::PageActions));
    press(&mut rt, Key::Escape);
    assert!(rt.state.modal.is_none());
    rt.state.new_name.replace_text("Another page");
    rt.state.new_page(false, None);
    assert_eq!(
        rt.state.tab().unwrap().page.settings,
        PageSettings::default()
    );
    rt.state.open_page(&first, false);
    assert_eq!(rt.state.tab().unwrap().page.settings, expected);
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    assert_eq!(
        reopened.store.get_page(&first).unwrap().unwrap().settings,
        expected
    );
    let archive = reopened
        .store
        .export_workspace(&reopened.workspace.id)
        .unwrap();
    drop(reopened);
    let mut imported = Store::in_memory().unwrap();
    let workspace = imported.import_workspace(&archive).unwrap();
    assert!(
        imported
            .list_pages(&workspace.id)
            .unwrap()
            .iter()
            .any(|p| p.settings == expected)
    );
    std::fs::remove_dir_all(path).unwrap();
}
