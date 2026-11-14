use super::*;
use moss_core::board::{BoardProjection, bucket_key};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[path = "board_drag_preview_tests.rs"]
mod drag_preview_tests;

struct BoardFixture {
    rt: Runtime<State>,
    path: PathBuf,
    db: String,
    page: String,
    view: String,
    status: String,
    priority: Option<String>,
    ids: Vec<String>,
}
fn wait_board(rt: &mut Runtime<State>, test: impl Fn(&BoardProjection) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let ready = rt
            .state
            .tab()
            .and_then(|tab| {
                tab.database
                    .as_ref()
                    .and_then(|db| db.views.get(tab.view).map(|view| (db, view)))
            })
            .and_then(|(db, view)| {
                rt.state
                    .database_views
                    .board
                    .projection_for(&db.id, &view.id)
            })
            .is_some_and(|projection| !rt.state.database_views.board.loading && test(projection));
        if ready {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Board read did not complete: {:?} / {:?}",
            rt.state.database_views.board.error,
            rt.state.error
        );
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn board_fixture(
    rows: &[(&str, Value, Option<Value>)],
    subgroups: bool,
    dark: bool,
    scale: f32,
) -> BoardFixture {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    let mut db = s.tab().unwrap().database.as_ref().unwrap().clone();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let priority = if subgroups {
        db = s
            .store
            .add_select_property(&db.id, "Priority", &["High".into(), "Low".into()])
            .unwrap();
        Some(db.properties.last().unwrap().id.clone())
    } else {
        None
    };
    db.views[1].query.group_by = Some(status.clone());
    db.views[1].config = json!({"card_size":"small","hidden_property_ids":db.properties.iter().filter(|p|p.kind!=PropertyKind::Title && p.id!=status).map(|p|p.id.clone()).collect::<Vec<_>>()});
    if let Some(priority) = &priority {
        db.views[1].config["board_subgroup_by"] = json!(priority);
    }
    s.store.save_database(&db).unwrap();
    let mut ids = Vec::new();
    for (name, group, subgroup) in rows {
        let mut values = BTreeMap::from([(status.clone(), group.clone())]);
        if let Some(priority) = &priority {
            values.insert(priority.clone(), subgroup.clone().unwrap_or(Value::Null));
        }
        ids.push(s.store.create_record(&db.id, name, &values).unwrap().id);
    }
    s.tabs[s.active].database = Some(db.clone());
    s.tabs[s.active].page.settings.full_width = true;
    assert!(s.select_database_view(1));
    let view = db.views[1].id.clone();
    let page = db.page_id.clone();
    let id = db.id.clone();
    let mut rt = Runtime::with_text(
        s,
        ui::view,
        (1700., 1100.),
        Env {
            system_dark: dark,
            scale,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.poll = Some(poll);
    rt.frame(0.);
    wait_board(&mut rt, |p| p.total == rows.len() as u64);
    BoardFixture {
        rt,
        path,
        db: id,
        page,
        view,
        status,
        priority,
        ids,
    }
}
fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .0
}
fn title_point(rt: &Runtime<State>, id: &str) -> [f32; 2] {
    let r = rect(rt, &format!("board-card-{id}"));
    [r[0] + 48., r[1] + 16.]
}
fn bucket(rt: &Runtime<State>, group: Value, subgroup: Value) -> [f32; 4] {
    rect(
        rt,
        &format!("board-bucket-{}", bucket_key(&group, &subgroup)),
    )
}
fn drag_to(rt: &mut Runtime<State>, record: &str, to: [f32; 2]) {
    let from = title_point(rt, record);
    let path = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("board-card-{record}"))).unwrap().path;
    rt.pointer_down(from[0], from[1]);
    rt.pointer_move(from[0] + 8., from[1]);
    rt.frame(0.);
    rt.pointer_move(to[0], to[1]);
    rt.frame(0.);
    let after = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("board-card-{record}"))).map(|e| e.path);
    assert_eq!(after.as_ref(), Some(&path), "card path changed during drag");
    let destination = rt.state.database_views.board_ui.drag.as_ref().map(|drag| (drag.moved, drag.destination.clone()));
    rt.pointer_up(to[0], to[1]);
    rt.frame(0.);
    assert!(destination.as_ref().is_some_and(|(moved, at)| *moved && at.is_some()),
        "drag from {from:?} to {to:?}, destination {destination:?}, viewport {:?}, lanes {:?}, columns {:?}",
        rt.state.database_views.board_ui.viewport,
        rt.state.database_views.board_ui.lane_rects,
        rt.state.database_views.board_ui.drop_zones);
    assert!(rt.state.database_views.board_ui.drag.is_none(), "drag release was lost: destination {destination:?}");
    assert!(rt.state.database_views.board_ui.error.is_none(), "drop error: {:?} / {:?}", rt.state.database_views.board_ui.error, rt.state.error);
    assert!(
        rt.state.record_peek.is_none(),
        "a drag must not open the card"
    );
}
fn projected_in(p: &BoardProjection, id: &str, group: &Value, subgroup: &Value) -> bool {
    p.buckets.iter().any(|b| {
        b.group == *group && b.subgroup == *subgroup && b.records.iter().any(|r| r.id == id)
    })
}

#[test]
fn board_status_badge_fits_its_text_at_both_display_scales() {
    for (dark, scale) in [(false, 1.), (true, 2.)] {
        let f = board_fixture(&[("Alpha", json!("Not started"), None)], false, dark, scale);
        let elements = f.rt.elements();
        let (index, badge) = elements.iter().enumerate().find(|(_, e)| {
            e.key.as_deref() == Some(&format!("property-value-{}", f.status))
        }).unwrap();
        let label = elements.iter().find(|e| e.parent == Some(index) && e.text.as_deref() == Some("Not started")).unwrap();
        let expected = label.rect[2] + 7. + 5. + 12.;
        assert!((badge.rect[2] - expected).abs() < 1., "badge {:?}, text {:?}", badge.rect, label.rect);
        drop(f.rt);
        std::fs::remove_dir_all(f.path).unwrap();
    }
}

#[test]
fn board_cards_show_filled_properties_and_stack_related_pages() {
    let mut f = board_fixture(&[("Alpha", json!("In progress"), None)], false, false, 1.);
    let mut db = f.rt.state.tab().unwrap().database.clone().unwrap();
    let date = db.properties.iter().find(|p| p.kind == PropertyKind::Date).unwrap().id.clone();
    let relation = moss_core::Property {
        id: moss_core::new_id(),
        name: "Related".into(),
        kind: PropertyKind::Relation,
        config: json!({"target_database_id": db.id}),
    };
    db.properties.push(relation.clone());
    db.views[1].config["hidden_property_ids"] = json!([]);
    f.rt.state.store.save_database(&db).unwrap();
    let first = f.rt.state.store.create_record(&db.id, "Beta", &Default::default()).unwrap();
    let second = f.rt.state.store.create_record(&db.id, "Gamma", &Default::default()).unwrap();
    f.rt.state.store.set_relation_value_if_unchanged(&f.ids[0], &relation, &Value::Null, &[first.id, second.id]).unwrap();
    f.rt.state.tabs[f.rt.state.active].database = Some(db);
    f.rt.state.query_records();
    f.rt.state.database_views.board.invalidate();
    wait_board(&mut f.rt, |p| p.total == 3);
    f.rt.invalidate();
    f.rt.frame(1. / 120.);
    assert!(f.rt.find(&format!("board-icon-{}", f.ids[0])).is_none());
    assert!(f.rt.find(&format!("board-value-{}-{date}", f.ids[0])).is_none());
    let first = rect(&f.rt, &format!("board-relation-{}-0", relation.id));
    let second = rect(&f.rt, &format!("board-relation-{}-1", relation.id));
    assert!(second[1] >= first[1] + first[3]);
    click(&mut f.rt, &format!("board-value-{}-{}", f.ids[0], relation.id));
    assert!(f.rt.state.advanced_property.relation_picker.is_some());
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_drag_can_start_on_a_property_and_click_still_opens_its_editor() {
    let mut f = board_fixture(&[("Alpha", json!("Not started"), None)], false, false, 1.);
    let id = f.ids[0].clone();
    let key = format!("board-value-{id}-{}", f.status);
    let from = rect(&f.rt, &key);
    let to = bucket(&f.rt, json!("Done"), Value::Null);
    f.rt.pointer_down(from[0] + 35., from[1] + 10.);
    f.rt.frame(1. / 120.);
    f.rt.pointer_move(to[0] + 40., to[1] + 20.);
    f.rt.frame(1. / 120.);
    f.rt.pointer_up(to[0] + 40., to[1] + 20.);
    f.rt.frame(1. / 120.);
    assert_eq!(f.rt.state.store.get_record(&id).unwrap().unwrap().values[&f.status], "Done");
    assert!(f.rt.state.property_picker.is_none());
    assert!(f.rt.state.record_peek.is_none());
    wait_board(&mut f.rt, |p| projected_in(p, &id, &json!("Done"), &Value::Null));
    click(&mut f.rt, &key);
    assert!(f.rt.state.property_picker.is_some());
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_drop_accepts_the_new_page_row_and_whitespace_below_short_columns() {
    let mut f = board_fixture(&[("Alpha", json!("Not started"), None)], false, false, 1.);
    let id = f.ids[0].clone();
    let footer = rect(&f.rt, "board-new-Done");
    let from = title_point(&f.rt, &id);
    let to = [footer[0] + 40., footer[1] + 12.];
    f.rt.pointer_down(from[0], from[1]);
    f.rt.frame(1. / 120.);
    for step in 1..=12 {
        let fraction = step as f32 / 12.;
        f.rt.pointer_move(from[0] + (to[0] - from[0]) * fraction, from[1] + (to[1] - from[1]) * fraction);
        f.rt.frame(1. / 120.);
    }
    f.rt.pointer_up(to[0], to[1]);
    f.rt.frame(1. / 120.);
    assert_eq!(f.rt.state.store.get_record(&id).unwrap().unwrap().values[&f.status], "Done");
    wait_board(&mut f.rt, |p| projected_in(p, &id, &json!("Done"), &Value::Null));
    let empty = bucket(&f.rt, Value::Null, Value::Null);
    let viewport = rect(&f.rt, "db-body");
    let bottom = viewport[1] + viewport[3] - 60.;
    assert!(bottom > empty[1] + empty[3]);
    drag_to(&mut f.rt, &id, [empty[0] + 50., bottom]);
    assert!(f.rt.state.store.get_record(&id).unwrap().unwrap().values[&f.status].is_null());
    assert!(f.rt.state.record_peek.is_none());
    assert!(f.rt.state.editing_cell.is_none());
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_drag_still_works_after_switching_through_table_with_the_same_viewport() {
    let mut f = board_fixture(&[("Alpha", json!("Not started"), None)], false, false, 1.);
    let id = f.ids[0].clone();
    assert!(f.rt.state.select_database_view(0));
    f.rt.frame(1. / 120.);
    assert!(f.rt.state.select_database_view(1));
    f.rt.frame(1. / 120.);
    wait_board(&mut f.rt, |p| p.total == 1);
    assert!(f.rt.state.database_views.board_ui.viewport[3] > 0.);
    let target = bucket(&f.rt, json!("Done"), Value::Null);
    drag_to(&mut f.rt, &id, [target[0] + 50., target[1] + 20.]);
    assert_eq!(f.rt.state.store.get_record(&id).unwrap().unwrap().values[&f.status], "Done");
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_loaded_record_refresh_keeps_the_drag_capture_until_release() {
    let mut f = board_fixture(&[("Alpha", json!("Not started"), None), ("Beta", json!("Done"), None)], false, false, 1.);
    let id = f.ids[0].clone();
    let from = title_point(&f.rt, &id);
    let to = bucket(&f.rt, json!("Done"), Value::Null);
    f.rt.pointer_down(from[0], from[1]);
    f.rt.pointer_move(from[0] + 8., from[1]);
    f.rt.frame(1. / 120.);
    f.rt.state.tabs[f.rt.state.active].records.reverse();
    f.rt.invalidate();
    f.rt.frame(1. / 120.);
    f.rt.pointer_move(to[0] + 40., to[1] + 20.);
    f.rt.frame(1. / 120.);
    f.rt.pointer_up(to[0] + 40., to[1] + 20.);
    f.rt.frame(1. / 120.);
    assert!(f.rt.state.database_views.board_ui.drag.is_none());
    assert_eq!(f.rt.state.store.get_record(&id).unwrap().unwrap().values[&f.status], "Done");
    assert!(f.rt.state.record_peek.is_none());
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_pointer_moves_between_groups_and_into_empty_group_then_table_and_reopen_agree() {
    for (dark, scale) in [(false, 1.), (true, 2.)] {
        let mut f = board_fixture(
            &[
                ("Alpha", json!("Not started"), None),
                ("Beta", json!("Done"), None),
            ],
            false,
            dark,
            scale,
        );
        let id = f.ids[0].clone();
        let target = bucket(&f.rt, json!("Done"), Value::Null);
        drag_to(
            &mut f.rt,
            &id,
            [target[0] + 50., target[1] + target[3] - 3.],
        );
        assert_eq!(
            f.rt.state.store.get_record(&id).unwrap().unwrap().values[&f.status],
            "Done"
        );
        wait_board(&mut f.rt, |p| {
            projected_in(p, &id, &json!("Done"), &Value::Null)
        });
        let target = bucket(&f.rt, Value::Null, Value::Null);
        drag_to(&mut f.rt, &id, [target[0] + 50., target[1] + 22.]);
        assert!(f.rt.state.store.get_record(&id).unwrap().unwrap().values[&f.status].is_null());
        wait_board(&mut f.rt, |p| {
            projected_in(p, &id, &Value::Null, &Value::Null)
        });
        assert!(f.rt.state.select_database_view(0));
        let table =
            f.rt.state
                .tab()
                .unwrap()
                .records
                .iter()
                .find(|r| r.id == id)
                .unwrap();
        assert!(table.values[&f.status].is_null());
        drop(f.rt);
        let reopened = State::open(f.path.clone(), None).unwrap();
        assert!(reopened.store.get_record(&id).unwrap().unwrap().values[&f.status].is_null());
        assert_eq!(
            reopened
                .store
                .get_database_by_page(&f.page)
                .unwrap()
                .unwrap()
                .id,
            f.db
        );
        drop(reopened);
        std::fs::remove_dir_all(f.path).unwrap();
    }
}
#[test]
fn board_subgroup_drop_changes_both_properties_and_cancel_does_not_write() {
    let mut f = board_fixture(
        &[
            ("Alpha", json!("Not started"), Some(json!("High"))),
            ("Beta", json!("Done"), Some(json!("Low"))),
        ],
        true,
        false,
        1.,
    );
    let id = f.ids[0].clone();
    let from = title_point(&f.rt, &id);
    f.rt.pointer_down(from[0], from[1]);
    f.rt.pointer_move(from[0] + 8., from[1]);
    f.rt.frame(1. / 120.);
    // Lifting the only source card closes its row before the pointer reaches the next subgroup.
    let target = bucket(&f.rt, json!("Done"), json!("Low"));
    let to = [target[0] + 50., target[1] + target[3] - 3.];
    f.rt.pointer_move(to[0], to[1]);
    f.rt.frame(1. / 120.);
    f.rt.pointer_up(to[0], to[1]);
    f.rt.frame(1. / 120.);
    let record = f.rt.state.store.get_record(&id).unwrap().unwrap();
    assert_eq!(record.values[&f.status], "Done");
    assert_eq!(record.values[f.priority.as_ref().unwrap()], "Low");
    wait_board(&mut f.rt, |p| {
        projected_in(p, &id, &json!("Done"), &json!("Low"))
    });
    for focus_loss in [false, true] {
        let before = f.rt.state.store.local_change_count();
        let from = title_point(&f.rt, &id);
        f.rt.pointer_down(from[0], from[1]);
        f.rt.pointer_move(from[0] + 16., from[1] + 8.);
        f.rt.frame(0.);
        if focus_loss {
            f.rt.window_focus(false);
            f.rt.frame(0.);
            f.rt.window_focus(true);
        } else {
            press(&mut f.rt, Key::Escape);
        }
        f.rt.pointer_up(from[0] + 16., from[1] + 8.);
        f.rt.frame(0.);
        assert_eq!(
            f.rt.state.store.local_change_count(),
            before,
            "cancel must not write"
        );
        assert!(f.rt.state.database_views.board_ui.drag.is_none());
        assert!(!f.rt.state.database_views.board.frozen);
        assert!(f.rt.state.record_peek.is_none());
    }
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}
#[test]
fn board_pointer_reorders_in_one_column_and_saved_order_survives_reopen() {
    let mut f = board_fixture(
        &[
            ("Alpha", json!("Done"), None),
            ("Beta", json!("Done"), None),
            ("Gamma", json!("Done"), None),
        ],
        false,
        false,
        1.,
    );
    let records =
        f.rt.state
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .buckets
            .iter()
            .find(|b| b.group == json!("Done"))
            .unwrap()
            .records
            .clone();
    let first = &records[0].id;
    let last = &records[2].id;
    let target = rect(&f.rt, &format!("board-card-{first}"));
    drag_to(&mut f.rt, last, [target[0] + 50., target[1] + 2.]);
    wait_board(&mut f.rt, |p| {
        p.buckets
            .iter()
            .find(|b| b.group == json!("Done"))
            .is_some_and(|b| b.records.first().is_some_and(|r| r.id == *last))
    });
    let db = f.db.clone();
    let view = f.view.clone();
    let path = f.path.clone();
    drop(f.rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let projected = reopened
        .store
        .query_board(&db, &view, "", &BTreeMap::new())
        .unwrap();
    let records = &projected
        .buckets
        .iter()
        .find(|b| b.group == json!("Done"))
        .unwrap()
        .records;
    assert_eq!(records[0].id, *last);
    assert_eq!(records[1].id, *first);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn board_full_count_and_per_column_pages_cover_records_after_loaded_page_with_bounded_nodes() {
    let owned: Vec<_> = (0..125).map(|i| format!("Task {i:03}")).collect();
    let rows: Vec<_> = owned
        .iter()
        .map(|name| (name.as_str(), json!("Done"), None))
        .collect();
    let mut f = board_fixture(&rows, false, false, 1.);
    assert!(f.rt.state.tab().unwrap().records.len() < 125);
    let projected = f.rt.state.database_views.board.projection.as_ref().unwrap();
    assert_eq!(projected.total, 125);
    let group = projected
        .buckets
        .iter()
        .find(|b| b.group == json!("Done"))
        .unwrap();
    assert_eq!(group.total, 125);
    assert_eq!(group.records.len(), 50);
    assert!(
        f.rt.elements()
            .iter()
            .any(|node| node.text.as_deref() == Some("125")),
        "group header must show the full matching count"
    );
    for expected in [50, 100] {
        f.rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., f32::MAX));
        for _ in 0..4 {
            f.rt.frame(1. / 60.);
        }
        click(
            &mut f.rt,
            &format!(
                "board-page-next-{}",
                bucket_key(&json!("Done"), &Value::Null)
            ),
        );
        wait_board(&mut f.rt, |p| {
            p.buckets
                .iter()
                .find(|b| b.group == json!("Done"))
                .is_some_and(|b| b.offset == expected)
        });
    }
    let projected = f.rt.state.database_views.board.projection.as_ref().unwrap();
    let group = projected
        .buckets
        .iter()
        .find(|b| b.group == json!("Done"))
        .unwrap();
    assert_eq!(group.total, 125);
    assert_eq!(group.records.len(), 25);
    assert!(
        f.rt.elements().len() < 3000,
        "mount only a bounded card window"
    );
    assert!(
        group.records.iter().any(|record| !f
            .rt
            .state
            .tab()
            .unwrap()
            .records
            .iter()
            .any(|loaded| loaded.id == record.id)),
        "last Board page must include records beyond the generic loaded page"
    );
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_card_property_and_context_actions_keep_the_target_id_and_do_not_open_the_page() {
    let mut f = board_fixture(&[("Alpha", json!("Done"), None)], false, false, 1.);
    let id = f.ids[0].clone();
    click(&mut f.rt, &format!("board-value-{id}-{}", f.status));
    assert!(f.rt.find("property-picker").is_some());
    assert!(f.rt.state.record_peek.is_none());
    press(&mut f.rt, Key::Escape);
    assert!(f.rt.state.property_picker.is_none());
    let point = title_point(&f.rt, &id);
    f.rt.secondary_down(point[0], point[1]);
    f.rt.secondary_up(point[0], point[1]);
    f.rt.frame(1.);
    assert!(f.rt.find("board-actions-menu").is_some());
    assert!(f.rt.state.record_peek.is_none());
    click(&mut f.rt, "board-menu-rename");
    assert_eq!(f.rt.state.editing_cell.as_ref().unwrap().record_id, id);
    click(&mut f.rt, "database-cell-editor");
    press(&mut f.rt, Key::Command('a'));
    press(&mut f.rt, Key::Text("Renamed".into()));
    press(&mut f.rt, Key::Escape);
    assert_eq!(
        f.rt.state.store.get_record(&id).unwrap().unwrap().title,
        "Renamed"
    );
    assert!(f.rt.state.record_peek.is_none());
    wait_board(&mut f.rt, |p| {
        p.buckets
            .iter()
            .flat_map(|b| &b.records)
            .any(|record| record.id == id && record.title == "Renamed")
    });
    click(&mut f.rt, &format!("board-card-menu-{id}"));
    f.rt.frame(1.);
    click(&mut f.rt, "board-menu-duplicate");
    wait_board(&mut f.rt, |p| p.total == 2);
    let copy =
        f.rt.state
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .buckets
            .iter()
            .flat_map(|b| &b.records)
            .find(|record| record.id != id)
            .unwrap()
            .clone();
    assert_eq!(copy.title, "Renamed (copy)");
    assert_eq!(copy.values[&f.status], "Done");
    assert!(f.rt.state.record_peek.is_none());
    click(&mut f.rt, &format!("board-card-menu-{id}"));
    f.rt.frame(1.);
    click(&mut f.rt, "board-menu-trash");
    assert!(
        f.rt.state
            .store
            .get_record(&id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_some()
    );
    assert!(
        f.rt.state
            .store
            .get_record(&copy.id)
            .unwrap()
            .unwrap()
            .deleted_at
            .is_none()
    );
    wait_board(&mut f.rt, |p| p.total == 1);
    assert!(f.rt.find(&format!("board-card-{id}")).is_none());
    assert!(f.rt.find(&format!("board-card-{}", copy.id)).is_some());
    assert!(f.rt.state.record_peek.is_none());
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_group_menu_adds_renames_collapses_and_hides_a_real_typed_group() {
    let mut f = board_fixture(&[("Alpha", json!("Done"), None)], false, false, 1.);
    click(&mut f.rt, "board-add-group-0");
    f.rt.frame(1.);
    click(&mut f.rt, "board-group-name");
    press(&mut f.rt, Key::Text("Review".into()));
    click(&mut f.rt, "board-group-save");
    wait_board(&mut f.rt, |p| p.groups.contains(&json!("Review")));
    click(&mut f.rt, "board-new-Review");
    press(&mut f.rt, Key::Text("Review task".into()));
    press(&mut f.rt, Key::Escape);
    wait_board(&mut f.rt, |p| p.total == 2);
    let record =
        f.rt.state
            .store
            .query_records(
                &f.db,
                &RecordQuery {
                    limit: 200,
                    ..Default::default()
                },
            )
            .unwrap()
            .into_iter()
            .find(|record| record.title == "Review task")
            .unwrap();
    assert_eq!(record.values[&f.status], "Review");
    let index =
        f.rt.state
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .groups
            .iter()
            .position(|group| *group == json!("Review"))
            .unwrap();
    click(&mut f.rt, &format!("board-group-menu-null-{index}"));
    f.rt.frame(1.);
    click(&mut f.rt, "board-group-rename");
    click(&mut f.rt, "board-group-name");
    press(&mut f.rt, Key::Command('a'));
    press(&mut f.rt, Key::Text("Ready".into()));
    click(&mut f.rt, "board-group-save");
    wait_board(&mut f.rt, |p| {
        p.groups.contains(&json!("Ready")) && !p.groups.contains(&json!("Review"))
    });
    assert_eq!(
        f.rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .values[&f.status],
        "Ready"
    );
    let index =
        f.rt.state
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .groups
            .iter()
            .position(|group| *group == json!("Ready"))
            .unwrap();
    click(&mut f.rt, &format!("board-group-menu-null-{index}"));
    f.rt.frame(1.);
    click(&mut f.rt, "board-group-collapse");
    assert!(f.rt.state.tab().unwrap().database.as_ref().unwrap().views[1].config["board_collapsed_groups"].as_array().unwrap().contains(&json!(moss_core::board::group_key(&json!("Ready")))));
    assert!(f.rt.find(&format!("board-card-{}", record.id)).is_none());
    click(&mut f.rt, &format!("board-group-menu-null-{index}"));
    f.rt.frame(1.);
    click(&mut f.rt, "board-group-hide");
    assert!(
        f.rt.state.tab().unwrap().database.as_ref().unwrap().views[1].config["board_hidden_groups"]
            .as_array()
            .unwrap()
            .contains(&json!(moss_core::board::group_key(&json!("Ready"))))
    );
    assert!(
        f.rt.find(&format!("board-group-menu-null-{index}"))
            .is_none()
    );
    assert_eq!(
        f.rt.state
            .store
            .get_record(&record.id)
            .unwrap()
            .unwrap()
            .values[&f.status],
        "Ready"
    );
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_many_columns_and_subgroups_keep_nodes_bounded_and_last_card_reachable() {
    let mut f = board_fixture(&[], true, false, 1.);
    let columns: Vec<_> = (0..20).map(|i| format!("Column {i:02}")).collect();
    let lanes: Vec<_> = (0..10).map(|i| format!("Lane {i:02}")).collect();
    let mut db =
        f.rt.state
            .store
            .add_select_property(&f.db, "Column", &columns)
            .unwrap();
    let column = db.properties.last().unwrap().id.clone();
    db.properties
        .iter_mut()
        .find(|property| Some(&property.id) == f.priority.as_ref())
        .unwrap()
        .config["options"] = json!(lanes);
    db.views[1].query.group_by = Some(column.clone());
    db.views[1].config["hidden_property_ids"] = json!(
        db.properties
            .iter()
            .filter(|property| property.kind != PropertyKind::Title)
            .map(|p| p.id.clone())
            .collect::<Vec<_>>()
    );
    f.rt.state.store.save_database(&db).unwrap();
    let mut last = String::new();
    for column_value in &columns {
        for lane in &lanes {
            last =
                f.rt.state
                    .store
                    .create_record(
                        &f.db,
                        &format!("{column_value} / {lane}"),
                        &BTreeMap::from([
                            (column.clone(), json!(column_value)),
                            (f.priority.as_ref().unwrap().clone(), json!(lane)),
                            (f.status.clone(), json!("Done")),
                        ]),
                    )
                    .unwrap()
                    .id;
        }
    }
    f.rt.state.tabs[f.rt.state.active].database = Some(db);
    assert!(f.rt.state.query_records());
    f.rt.state.database_views.board.invalidate();
    f.rt.invalidate();
    f.rt.frame(0.);
    wait_board(&mut f.rt, |p| p.total == 200);
    assert!(
        f.rt.elements().len() < 3000,
        "render a bounded number of rows and columns at the start"
    );
    f.rt.update(|_, tx| tx.scroll_to_instant("db-body", f32::MAX, f32::MAX));
    for _ in 0..8 {
        f.rt.frame(1. / 60.);
    }
    assert!(
        f.rt.find(&format!("board-card-{last}")).is_some(),
        "the final row and column must be reachable after both scroll axes move"
    );
    assert!(
        f.rt.elements().len() < 3000,
        "render a bounded number of rows and columns at the end"
    );
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_heading_drag_cancelled_by_view_switch_does_not_write_the_other_view_order() {
    let mut f = board_fixture(&[("Alpha", json!("Done"), None)], false, false, 1.);
    let mut db =
        f.rt.state
            .store
            .create_view(&f.db, "Other Board", ViewKind::Board)
            .unwrap();
    let other = db.views.len() - 1;
    db.views[other].query.group_by = Some(f.status.clone());
    db.views[other].config = json!({"marker":"keep"});
    f.rt.state.store.save_database(&db).unwrap();
    f.rt.state.tabs[f.rt.state.active].database = Some(db.clone());
    f.rt.invalidate();
    f.rt.frame(0.);
    wait_board(&mut f.rt, |p| p.total == 1);
    let header = rect(&f.rt, "board-heading-null-0");
    let from = [header[0] + 55., header[1] + 16.];
    f.rt.pointer_down(from[0], from[1]);
    f.rt.pointer_move(from[0] + 50., from[1]);
    f.rt.frame(0.);
    assert!(f.rt.state.select_database_view(other));
    f.rt.frame(0.);
    let changes = f.rt.state.store.local_change_count();
    f.rt.pointer_move(from[0] + 500., from[1]);
    f.rt.pointer_up(from[0] + 500., from[1]);
    f.rt.frame(0.);
    assert_eq!(
        f.rt.state.store.local_change_count(),
        changes,
        "the old heading drag must not save against the new view"
    );
    let saved = f.rt.state.store.get_database(&f.db).unwrap().unwrap();
    assert_eq!(saved.views[other].config, db.views[other].config);
    assert_eq!(saved.views[1].config, db.views[1].config);
    assert!(!f.rt.state.database_views.board.frozen);
    assert!(f.rt.state.database_views.board_ui.group_drag.is_none());
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_creation_inherits_guaranteed_filters_rejects_conflicting_columns_and_appends() {
    let mut f = board_fixture(
        &[
            ("Alpha", json!("Done"), None),
            ("Beta", json!("Done"), None),
        ],
        false,
        false,
        1.,
    );
    let db =
        f.rt.state
            .store
            .add_property(&f.db, "Notes", PropertyKind::Text)
            .unwrap();
    let notes = db.properties.last().unwrap().id.clone();
    for id in &f.ids {
        f.rt.state
            .store
            .set_record_value_if_unchanged(id, &notes, &Value::Null, json!("keep"))
            .unwrap();
    }
    let filter = FilterExpr::All(vec![
        FilterExpr::Rule(FilterRule {
            property_id: f.status.clone(),
            op: FilterOp::Eq,
            value: json!("Done"),
        }),
        FilterExpr::Rule(FilterRule {
            property_id: notes.clone(),
            op: FilterOp::Eq,
            value: json!("keep"),
        }),
    ]);
    let db =
        f.rt.state
            .store
            .set_view_filter(&f.db, &f.view, None, Some(filter))
            .unwrap();
    f.rt.state.tabs[f.rt.state.active].database = Some(db);
    assert!(f.rt.state.query_records());
    f.rt.state.database_views.board.invalidate();
    f.rt.invalidate();
    f.rt.frame(0.);
    wait_board(&mut f.rt, |p| p.total == 2);
    let before: Vec<_> =
        f.rt.state
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .buckets
            .iter()
            .find(|b| b.group == json!("Done"))
            .unwrap()
            .records
            .iter()
            .map(|r| r.id.clone())
            .collect();
    click(&mut f.rt, "db-new");
    let toolbar = f.rt.state.record_peek.as_ref().unwrap().record.clone();
    assert_eq!(toolbar.values[&f.status], "Done");
    assert_eq!(toolbar.values[&notes], "keep");
    click(&mut f.rt, "peek-close");
    wait_board(&mut f.rt, |p| p.total == 3);
    let count =
        f.rt.state
            .store
            .query_records(
                &f.db,
                &RecordQuery {
                    limit: 200,
                    ..Default::default()
                },
            )
            .unwrap()
            .len();
    click(&mut f.rt, "board-new-Not started");
    assert!(
        f.rt.state
            .error
            .as_ref()
            .is_some_and(|error| error.contains("conflict"))
    );
    assert_eq!(
        f.rt.state
            .store
            .query_records(
                &f.db,
                &RecordQuery {
                    limit: 200,
                    ..Default::default()
                }
            )
            .unwrap()
            .len(),
        count
    );
    click(&mut f.rt, "board-new-Done");
    let footer = f.rt.state.editing_cell.as_ref().unwrap().record_id.clone();
    press(&mut f.rt, Key::Text("Footer task".into()));
    press(&mut f.rt, Key::Escape);
    let record = f.rt.state.store.get_record(&footer).unwrap().unwrap();
    assert_eq!(record.values[&f.status], "Done");
    assert_eq!(record.values[&notes], "keep");
    assert!(f.rt.state.error.is_none());
    assert!(
        f.rt.state.database_views.board_ui.error.is_none(),
        "valid creation should clear the old conflict message"
    );
    wait_board(&mut f.rt, |p| p.total == 4);
    let mut expected = before;
    expected.push(toolbar.id);
    expected.push(footer);
    let order: Vec<_> =
        f.rt.state
            .database_views
            .board
            .projection
            .as_ref()
            .unwrap()
            .buckets
            .iter()
            .find(|b| b.group == json!("Done"))
            .unwrap()
            .records
            .iter()
            .map(|r| r.id.clone())
            .collect();
    assert_eq!(order, expected);
    let db = f.db.clone();
    let view = f.view.clone();
    drop(f.rt);
    let reopened = State::open(f.path.clone(), None).unwrap();
    let projected = reopened
        .store
        .query_board(&db, &view, "", &BTreeMap::new())
        .unwrap();
    let order: Vec<_> = projected
        .buckets
        .iter()
        .find(|b| b.group == json!("Done"))
        .unwrap()
        .records
        .iter()
        .map(|r| r.id.clone())
        .collect();
    assert_eq!(order, expected);
    drop(reopened);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn board_place_property_opens_shared_editor() {
    let mut f = board_fixture(&[("Card", json!("Not started"), None)], false, false, 1.);
    let db = f.rt.state.store.add_property(&f.db, "Place", PropertyKind::Place).unwrap();
    let property = db.properties.last().unwrap().id.clone();
    f.rt.state.tabs[f.rt.state.active].database = Some(db);
    f.rt.state.database_views.board.invalidate();
    wait_board(&mut f.rt, |p| p.total == 1);
    f.rt.invalidate(); f.rt.frame(1. / 120.);
    assert!(f.rt.find(&format!("board-value-{}-{property}", f.ids[0])).is_none());
    click(&mut f.rt, &format!("board-card-menu-{}", f.ids[0]));
    f.rt.frame(1.);
    click(&mut f.rt, &format!("board-menu-set-place-{property}"));
    assert!(f.rt.state.place_picker.is_some(), "place picker missing: {:?}, menu {}, peek {}", f.rt.state.error, f.rt.state.database_views.board_ui.menu.is_some(), f.rt.state.record_peek.is_some());
    assert!(f.rt.find("place-search-input").is_some());
    f.rt.state.database_views.map.place_query.replace_text("Fremantle");
    f.rt.invalidate(); f.rt.frame(1. / 120.);
    click(&mut f.rt, "place-save-text");
    assert_eq!(f.rt.state.store.get_record(&f.ids[0]).unwrap().unwrap().values[&property], "Fremantle");
    drop(f.rt); std::fs::remove_dir_all(f.path).unwrap();
}
