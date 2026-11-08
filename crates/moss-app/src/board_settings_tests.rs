use super::*;
use std::time::{Duration, Instant};

fn board_settings_runtime(dark: bool, scale: f32) -> (Runtime<State>, PathBuf) {
    let (mut state, path) = fixture();
    state.new_page(true, None);
    state.tabs[state.active].view = 1;
    assert!(state.query_records());
    let mut rt = Runtime::with_text(
        state,
        ui::view,
        (850., 700.),
        Env {
            system_dark: dark,
            scale,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.poll = Some(poll);
    rt.frame(0.);
    (rt, path)
}
#[test]
fn board_layout_settings_use_saved_view_fields_and_survive_reopen() {
    for (dark, scale) in [(false, 1.), (true, 2.)] {
        let (mut rt, path) = board_settings_runtime(dark, scale);
        let db = rt.state.tab().unwrap().database.as_ref().unwrap().clone();
        let page = db.page_id.clone();
        let table_before = db.views[0].clone();
        let status = db.properties[1].id.clone();
        click(&mut rt, "db-view-settings");
        click(&mut rt, "view-layout");
        assert!(rt.find("board-card-size").is_some());
        click(&mut rt, "board-card-size");
        click(&mut rt, "board-card_size-large");
        click(&mut rt, "board-card-preview");
        click(&mut rt, "board-card_preview-content");
        for key in ["show_page_icon", "wrap_cards", "color_columns"] {
            click(&mut rt, &format!("board-setting-{key}"));
        }
        click(&mut rt, "board-card-properties");
        click(&mut rt, &format!("board-property-down-{status}"));
        let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
        assert_eq!(saved.views[0], table_before);
        let view = &saved.views[1];
        assert_eq!(view.config["card_size"], "large");
        assert_eq!(view.config["card_preview"], "content");
        assert_eq!(view.config["show_page_icon"], false);
        assert_eq!(view.config["wrap_cards"], false);
        assert_eq!(view.config["color_columns"], true);
        assert_eq!(view.config["board_property_order"][1], status);
        drop(rt);
        let reopened = State::open(path.clone(), None).unwrap();
        let stored = reopened.store.get_database_by_page(&page).unwrap().unwrap();
        assert_eq!(stored.views[1].config, view.config);
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn board_grouping_is_distinct_atomic_and_keeps_other_view_fields() {
    let (mut rt, path) = board_settings_runtime(false, 1.);
    let db = rt.state.tab().unwrap().database.as_ref().unwrap().clone();
    let status = db.properties[1].id.clone();
    let db = rt
        .state
        .store
        .add_select_property(&db.id, "Priority", &["High".into(), "Low".into()])
        .unwrap();
    let priority = db.properties.last().unwrap().id.clone();
    rt.state.tabs[rt.state.active].database = Some(db.clone());
    rt.invalidate();
    rt.frame(0.);
    click(&mut rt, "db-view-settings");
    click(&mut rt, "view-group");
    click(&mut rt, "board-group-by");
    click(&mut rt, &format!("board-group-property-{priority}"));
    click(&mut rt, "board-settings-back");
    click(&mut rt, "view-subgroup");
    click(&mut rt, "board-group-by");
    assert!(
        rt.find(&format!("board-group-property-{priority}"))
            .is_none()
    );
    click(&mut rt, &format!("board-group-property-{status}"));
    let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
    assert_eq!(
        saved.views[1].query.group_by.as_deref(),
        Some(priority.as_str())
    );
    assert_eq!(saved.views[1].config["board_subgroup_by"], status);
    rt.state.choose_board_grouping(Some(&priority), true);
    assert!(rt.state.view_management.as_ref().unwrap().error.is_some());
    assert_eq!(
        rt.state.store.get_database(&db.id).unwrap().unwrap().views[1],
        saved.views[1]
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn board_groups_can_hide_collapse_and_reorder_after_projection_loads() {
    let (mut rt, path) = board_settings_runtime(false, 1.);
    let deadline = Instant::now() + Duration::from_secs(5);
    while rt.state.database_views.board.projection.is_none() {
        assert!(Instant::now() < deadline);
        rt.frame(1. / 60.);
        std::thread::sleep(Duration::from_millis(1));
    }
    let first = moss_core::board::group_key(
        &rt.state
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .groups[0],
    );
    click(&mut rt, "db-view-settings");
    click(&mut rt, "view-group");
    click(&mut rt, "board-group-visibility-0");
    click(&mut rt, "board-group-collapse-0");
    click(&mut rt, "board-group-down-0");
    let db = rt.state.tab().unwrap().database.as_ref().unwrap();
    let config = &db.views[1].config;
    assert_eq!(config["board_hidden_groups"][0], first);
    assert_eq!(config["board_collapsed_groups"][0], first);
    assert_eq!(config["board_group_order"][1], first);
    click(&mut rt, "board-setting-board_hide_empty");
    assert_eq!(
        rt.state.tab().unwrap().database.as_ref().unwrap().views[1].config["board_hide_empty"],
        true
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn board_settings_guard_the_changed_field_and_preserve_unrelated_changes() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.tabs[s.active].view = 1;
    s.open_view_settings();
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let view = db.views[1].id.clone();
    s.store
        .edit_view_config(&db.id, &view, ViewKind::Board, |config| {
            config["unrelated"] = json!(17);
            Ok(())
        })
        .unwrap();
    assert!(s.set_board_config("card_size", json!("large")));
    assert_eq!(
        s.store.get_database(&db.id).unwrap().unwrap().views[1].config["unrelated"],
        17
    );
    s.store
        .edit_view_config(&db.id, &view, ViewKind::Board, |config| {
            config["card_size"] = json!("small");
            Ok(())
        })
        .unwrap();
    assert!(!s.set_board_config("card_size", json!("medium")));
    assert!(s.view_management.as_ref().unwrap().error.is_some());
    assert_eq!(
        s.store.get_database(&db.id).unwrap().unwrap().views[1].config["card_size"],
        "small"
    );
    s.reload_board_settings();
    assert!(s.view_management.as_ref().unwrap().error.is_none());
    assert!(s.set_board_config("card_size", json!("medium")));
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
