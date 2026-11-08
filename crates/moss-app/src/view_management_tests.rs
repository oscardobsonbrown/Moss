use super::*;

#[test]
fn view_menu_rows_share_columns_across_panes_themes_and_scales() {
    fn rows(rt: &Runtime<State>, keys: &[String]) {
        for key in keys {
            let row = rt.presented(key).unwrap().0;
            let icon = rt.presented(&format!("{key}-icon")).unwrap().0;
            let label = rt.presented(&format!("{key}-label")).unwrap().0;
            let end = rt.presented(&format!("{key}-end")).unwrap().0;
            assert!((row[3] - 32.).abs() < 0.1, "{key}: {row:?}");
            assert!((icon[0] - row[0] - 10.).abs() < 0.1, "{key}: {icon:?}");
            assert!((label[0] - icon[0] - icon[2] - 8.).abs() < 0.1);
            assert!((end[0] + end[2] - row[0] - row[2] + 10.).abs() < 0.1);
            assert!(label[0] + label[2] <= end[0] - 7.9);
            for slot in [icon, end] {
                assert!((slot[2] - 20.).abs() < 0.1 && (slot[3] - 20.).abs() < 0.1);
                assert!((slot[1] + slot[3] / 2. - row[1] - row[3] / 2.).abs() < 0.1);
            }
        }
    }
    for dark in [false, true] {
        for scale in [1., 2.] {
            let (mut state, path) = fixture();
            state.new_page(true, None);
            let active = state.active;
            let db = state.tabs[active].database.as_mut().unwrap();
            db.properties[1].name = "A property with a very long name ".repeat(6);
            let properties: Vec<_> = db.properties.iter().map(|p| p.id.clone()).collect();
            let mut rt = Runtime::with_text(
                state,
                ui::view,
                (850., 900.),
                Env {
                    system_dark: dark,
                    scale,
                },
                rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
            );
            rt.on_key = Some(key);
            rt.frame(0.);
            click(&mut rt, "db-view-settings");
            rt.frame(1.);
            rows(
                &rt,
                &[
                    "view-layout",
                    "view-properties",
                    "view-filter",
                    "view-sort",
                    "view-group",
                    "view-open-mode",
                ]
                .map(str::to_owned),
            );
            click(&mut rt, "view-layout");
            rt.frame(1.);
            rows(
                &rt,
                &["table", "board", "calendar", "timeline", "gallery", "list"]
                    .map(|kind| format!("view-layout-{kind}")),
            );
            click(&mut rt, "view-back");
            click(&mut rt, "view-open-mode");
            rt.frame(1.);
            rows(
                &rt,
                &["side", "center", "full"].map(|mode| format!("view-open-{mode}")),
            );
            click(&mut rt, "view-back");
            click(&mut rt, "view-filter");
            rt.frame(1.);
            rows(
                &rt,
                &properties
                    .iter()
                    .map(|id| format!("view-property-{id}"))
                    .collect::<Vec<_>>(),
            );
            click(&mut rt, "view-back");
            click(&mut rt, "view-properties");
            rt.frame(1.);
            rows(
                &rt,
                &properties
                    .iter()
                    .skip(1)
                    .map(|id| format!("list-property-{id}"))
                    .collect::<Vec<_>>(),
            );
            click(&mut rt, "list-properties-close");
            rt.state.open_view_chooser();
            rt.invalidate();
            rt.frame(1.);
            rows(
                &rt,
                &(0..6)
                    .map(|i| format!("select-view-{i}"))
                    .collect::<Vec<_>>(),
            );
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn view_lifecycle_preserves_identity_and_rejects_stale_changes() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let database_id = s.tab().unwrap().database.as_ref().unwrap().id.clone();
    let original = s.tab().unwrap().database.as_ref().unwrap().views[0]
        .id
        .clone();
    s.open_new_view();
    s.view_name_input.replace_text("Planning");
    s.view_management.as_mut().unwrap().kind = ViewKind::Board;
    assert!(s.save_view_name());
    let created = s.tab().unwrap().database.as_ref().unwrap().views[s.tab().unwrap().view].clone();
    assert_eq!(created.name, "Planning");
    assert_eq!(created.kind, ViewKind::Board);
    s.open_view_settings();
    s.view_name_input.replace_text("Roadmap");
    assert!(s.save_view_name());
    assert_eq!(
        s.tab().unwrap().database.as_ref().unwrap().views[s.tab().unwrap().view].id,
        created.id
    );
    s.open_view_settings();
    s.change_view_layout(ViewKind::Gallery);
    assert_eq!(
        s.tab().unwrap().database.as_ref().unwrap().views[s.tab().unwrap().view].kind,
        ViewKind::Gallery
    );
    s.duplicate_current_view();
    let duplicate =
        s.tab().unwrap().database.as_ref().unwrap().views[s.tab().unwrap().view].clone();
    assert_ne!(duplicate.id, created.id);
    assert_eq!(duplicate.kind, ViewKind::Gallery);
    s.open_view_settings();
    s.move_current_view(false);
    assert_eq!(
        s.tab().unwrap().database.as_ref().unwrap().views[s.tab().unwrap().view].id,
        duplicate.id
    );
    s.delete_current_view();
    assert!(
        s.tab()
            .unwrap()
            .database
            .as_ref()
            .unwrap()
            .views
            .iter()
            .all(|v| v.id != duplicate.id)
    );
    assert!(
        s.tab()
            .unwrap()
            .database
            .as_ref()
            .unwrap()
            .views
            .iter()
            .any(|v| v.id == original)
    );
    s.open_view_settings();
    let stale = s
        .view_management
        .as_ref()
        .unwrap()
        .expected
        .clone()
        .unwrap();
    s.store
        .rename_view(&database_id, &stale, "External change")
        .unwrap();
    s.view_name_input.replace_text("Draft kept");
    assert!(!s.save_view_name());
    assert_eq!(s.view_name_input.text, "Draft kept");
    assert!(s.view_management.as_ref().unwrap().error.is_some());
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn opening_mode_updates_only_selected_view_and_preserves_other_settings() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.open_view_settings();
    s.set_view_open_mode("full");
    let db = s.tab().unwrap().database.as_ref().unwrap();
    assert_eq!(db.views[0].config["open_mode"], "full");
    assert!(db.views[1].config.get("open_mode").is_none());
    let view = s
        .view_management
        .as_ref()
        .unwrap()
        .expected
        .clone()
        .unwrap();
    let id = db.id.clone();
    s.store
        .edit_view_config(&id, &view.id, view.kind, |config| {
            config["open_mode"] = json!("center");
            Ok(())
        })
        .unwrap();
    s.set_view_open_mode("side");
    assert!(s.view_management.as_ref().unwrap().error.is_some());
    assert_eq!(
        s.store.get_database(&id).unwrap().unwrap().views[0].config["open_mode"],
        "center"
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn table_tab_skips_a_hidden_property() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    assert!(s.finish_cell(false));
    let record = s.tab().unwrap().records[0].id.clone();
    let db = s.tab().unwrap().database.as_ref().unwrap();
    let properties = db.properties.clone();
    let hidden = properties[1].id.clone();
    let next = properties[2].id.clone();
    s.open_database_property(Some(&properties[0].id));
    s.set_table_config("hidden_property_ids", json!([hidden]));
    s.close_database_property();
    s.selected_cell = Some((record, None));
    assert!(s.table_selection_key(&KeyEvent {
        key: Key::Tab,
        mods: Mods::default()
    }));
    assert_eq!(
        s.selected_cell.as_ref().unwrap().1.as_deref(),
        Some(next.as_str())
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn deleting_the_last_view_keeps_the_popup_and_error() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let mut db = s.tab().unwrap().database.clone().unwrap();
    while db.views.len() > 1 {
        db = s.store.delete_view(&db.id, &db.views[1]).unwrap();
    }
    s.tabs[s.active].database = Some(db.clone());
    s.tabs[s.active].view = 0;
    s.open_view_settings();
    s.view_management.as_mut().unwrap().confirm_delete = true;
    s.delete_current_view();
    assert!(s.view_management.as_ref().unwrap().error.is_some());
    assert_eq!(
        s.store.get_database(&db.id).unwrap().unwrap().views.len(),
        1
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn settings_popup_keeps_failed_draft_and_create_view_works_at_minimum_size() {
    for dark in [false, true] {
        let (mut s, path) = fixture();
        s.new_page(true, None);
        let mut rt = Runtime::new(
            s,
            ui::view,
            (850., 600.),
            Env {
                system_dark: dark,
                scale: 1.,
            },
        );
        rt.on_key = Some(key);
        rt.frame(0.);
        click(&mut rt, "db-view-settings");
        rt.frame(1.);
        let rect = rt.presented("view-management-menu").unwrap().0;
        assert!(
            rect[0] >= 0.
                && rect[1] >= 0.
                && rect[0] + rect[2] <= 850.
                && rect[1] + rect[3] <= 600.,
            "{rect:?}"
        );
        rt.state.view_name_input.replace_text("");
        click(&mut rt, "view-name-input");
        press(&mut rt, Key::Enter);
        assert!(rt.state.view_management.as_ref().unwrap().error.is_some());
        press(&mut rt, Key::Escape);
        assert!(rt.state.view_management.is_some());
        rt.pointer_down(820., 570.);
        rt.pointer_up(820., 570.);
        rt.frame(0.);
        assert!(rt.state.view_management.is_some());
        click(&mut rt, "view-discard");
        assert!(rt.state.view_management.is_none());
        click(&mut rt, "db-add-view");
        click(&mut rt, "view-layout-gallery");
        assert_eq!(rt.state.view_name_input.text, "Gallery");
        let create = rt.presented("view-create").unwrap().0;
        assert!(
            create[1] >= 0. && create[1] + create[3] <= 600.,
            "{create:?}"
        );
        click(&mut rt, "view-create");
        let tab = rt.state.tab().unwrap();
        let view = &tab.database.as_ref().unwrap().views[tab.view];
        assert_eq!(view.kind, ViewKind::Gallery);
        assert_eq!(view.name, "Gallery");
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn settings_menu_fades_for_pointer_and_opens_instantly_for_keyboard() {
    for keyboard in [false, true] {
        let (mut s, path) = fixture();
        s.new_page(true, None);
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
        rt.set_full_keyboard_access(true);
        rt.frame(0.);
        if keyboard {
            for _ in 0..100 {
                if rt
                    .elements()
                    .iter()
                    .any(|e| e.focused && e.key.as_deref() == Some("db-view-settings"))
                {
                    break;
                }
                press(&mut rt, Key::Tab);
            }
            assert!(
                rt.elements()
                    .iter()
                    .any(|e| e.focused && e.key.as_deref() == Some("db-view-settings"))
            );
            press(&mut rt, Key::Text(" ".into()));
            assert_eq!(rt.presented("view-management-menu").unwrap().1, 1.);
        } else {
            click(&mut rt, "db-view-settings");
            let start = rt.presented("view-management-menu").unwrap().1;
            assert!(start < 0.95);
            rt.frame(0.05);
            assert!(rt.presented("view-management-menu").unwrap().1 > start);
        }
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn name_autosaves_and_settings_can_start_a_new_view() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db_id = s.tab().unwrap().database.as_ref().unwrap().id.clone();
    let view_id = s.tab().unwrap().database.as_ref().unwrap().views[0]
        .id
        .clone();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1000., 700.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, "db-view-settings");
    rt.frame(1.);
    click(&mut rt, "view-name-input");
    press(&mut rt, Key::Command('a'));
    press(&mut rt, Key::Text("Tasks".into()));
    assert_eq!(
        rt.state
            .store
            .get_database(&db_id)
            .unwrap()
            .unwrap()
            .views
            .iter()
            .find(|v| v.id == view_id)
            .unwrap()
            .name,
        "Tasks"
    );
    assert!(rt.state.view_management.is_some());
    click(&mut rt, "view-create-new");
    assert!(
        rt.state
            .view_management
            .as_ref()
            .unwrap()
            .expected
            .is_none()
    );
    assert!(rt.find("view-create").is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn board_settings_filter_and_sort_change_the_saved_view_and_visible_rows() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let db_id = s.tab().unwrap().database.as_ref().unwrap().id.clone();
    let title = s.tab().unwrap().database.as_ref().unwrap().properties[0]
        .id
        .clone();
    for name in ["Alpha", "Beta", "Alpine"] {
        s.store
            .create_record(&db_id, name, &Default::default())
            .unwrap();
    }
    assert!(s.select_database_view(1));
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
    click(&mut rt, "db-view-settings");
    click(&mut rt, "view-filter");
    click(&mut rt, &format!("view-property-{title}"));
    rt.frame(1.);
    let rect = rt.presented("database-property-menu").unwrap().0;
    assert!(
        rect[0] >= 0. && rect[1] >= 0. && rect[0] + rect[2] <= 850. && rect[1] + rect[3] <= 600.,
        "{rect:?}"
    );
    click(&mut rt, "table-filter-op-contains");
    click(&mut rt, "table-filter-value");
    press(&mut rt, Key::Text("Al".into()));
    click(&mut rt, "table-filter-apply");
    let names: Vec<_> = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .map(|r| r.title.as_str())
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"Alpha") && names.contains(&"Alpine"));
    click(&mut rt, "db-view-settings");
    click(&mut rt, "view-sort");
    click(&mut rt, &format!("view-property-{title}"));
    click(&mut rt, "table-sort-descending");
    let names: Vec<_> = rt
        .state
        .tab()
        .unwrap()
        .records
        .iter()
        .map(|r| r.title.as_str())
        .collect();
    assert_eq!(names, ["Alpine", "Alpha"]);
    let saved = rt.state.store.get_database(&db_id).unwrap().unwrap();
    assert!(saved.views[1].query.filter.is_some());
    assert_eq!(saved.views[1].query.sorts.len(), 1);
    let date = saved
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    click(&mut rt, "db-view-settings");
    click(&mut rt, "view-group");
    click(&mut rt, "board-group-by");
    assert!(rt.find(&format!("board-group-property-{title}")).is_none());
    click(&mut rt, &format!("board-group-property-{date}"));
    let grouped = rt.state.store.get_database(&db_id).unwrap().unwrap();
    assert_eq!(
        grouped.views[1].query.group_by.as_deref(),
        Some(date.as_str())
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn selected_view_survives_reopen_and_reorder_then_falls_back_if_deleted() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let page = s.tab().unwrap().page.id.clone();
    let db_id = s.tab().unwrap().database.as_ref().unwrap().id.clone();
    assert!(s.select_database_view(1));
    let selected = s.tab().unwrap().database.as_ref().unwrap().views[1].clone();
    let db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let order: Vec<_> = db.views.iter().map(|v| v.id.clone()).collect();
    s.store.move_view(&db_id, &selected, &order, None).unwrap();
    drop(s);
    let mut s = State::open(path.clone(), None).unwrap();
    let tab = s.tabs.iter().find(|tab| tab.page.id == page).unwrap();
    assert_eq!(
        tab.database.as_ref().unwrap().views[tab.view].id,
        selected.id
    );
    let latest = s.store.get_database(&db_id).unwrap().unwrap();
    let actual = latest
        .views
        .iter()
        .find(|view| view.id == selected.id)
        .unwrap()
        .clone();
    let remaining = s.store.delete_view(&db_id, &actual).unwrap();
    let fallback = remaining.views[0].id.clone();
    drop(s);
    let s = State::open(path.clone(), None).unwrap();
    let tab = s.tabs.iter().find(|tab| tab.page.id == page).unwrap();
    assert_eq!(tab.database.as_ref().unwrap().views[tab.view].id, fallback);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn old_preferences_without_selected_views_remain_readable() {
    let preferences: state::Preferences = serde_json::from_value(json!({
        "dark": null, "workspace": "old", "tabs": {}, "active": {}
    }))
    .unwrap();
    assert!(preferences.selected_views.is_empty());
}
