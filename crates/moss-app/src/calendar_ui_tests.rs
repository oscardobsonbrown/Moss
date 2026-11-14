use super::*;
use chrono::Datelike;
use std::time::{Duration, Instant};

fn wait_calendar(
    rt: &mut Runtime<State>,
    ready: impl Fn(&moss_core::calendar::CalendarProjection) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        rt.frame(1. / 120.);
        let found = rt
            .state
            .tab()
            .and_then(|tab| {
                tab.database.as_ref().and_then(|db| {
                    db.views.get(tab.view).and_then(|view| {
                        rt.state
                            .database_views
                            .calendar
                            .projection_for(&db.id, &view.id)
                    })
                })
            })
            .is_some_and(&ready);
        if found {
            rt.invalidate();
            rt.frame(1. / 120.);
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Calendar query did not finish: {:?}",
            rt.state.database_views.calendar.error
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .0
}
fn drag(rt: &mut Runtime<State>, from: [f32; 2], to: [f32; 2]) {
    rt.pointer_down(from[0], from[1]);
    rt.pointer_move(from[0] + 8., from[1]);
    rt.frame(1. / 120.);
    rt.pointer_move(to[0], to[1]);
    rt.frame(1. / 120.);
    rt.pointer_up(to[0], to[1]);
    rt.frame(1. / 120.);
}

#[test]
fn calendar_status_badge_fits_its_text() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap();
    let record = rt
        .state
        .store
        .create_record(
            &db.id,
            "Task",
            &[
                (date.id.clone(), json!("2026-11-10")),
                (status.id.clone(), json!("Done")),
            ]
            .into(),
        )
        .unwrap();
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.query_records();
    wait_calendar(&mut rt, |p| p.records.iter().any(|r| r.id == record.id));
    let card = rect(&rt, "calendar-0");
    let badge = rect(&rt, &format!("property-value-{}", status.id));
    assert!(
        badge[2] < card[2] - 20.,
        "badge should fit its text: {badge:?}, {card:?}"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_cards_align_properties_and_show_linked_pages() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    let mut db = rt.state.tab().unwrap().database.clone().unwrap();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let relation = moss_core::Property {
        id: moss_core::new_id(),
        name: "Related".into(),
        kind: PropertyKind::Relation,
        config: json!({"target_database_id": db.id}),
    };
    db.properties.push(relation.clone());
    rt.state.store.save_database(&db).unwrap();
    rt.state.tabs[rt.state.active].database = Some(db.clone());
    let first = rt
        .state
        .store
        .create_record(&db.id, "Alpha", &Default::default())
        .unwrap();
    let second = rt
        .state
        .store
        .create_record(&db.id, "Beta", &Default::default())
        .unwrap();
    let source = rt
        .state
        .store
        .create_record(
            &db.id,
            "With links",
            &[
                (date.clone(), json!("2026-11-10")),
                (relation.id.clone(), json!([first.id, second.id])),
            ]
            .into(),
        )
        .unwrap();
    let other = rt
        .state
        .store
        .create_record(&db.id, "Plain", &[(date, json!("2026-11-11"))].into())
        .unwrap();
    let page = rt.state.store.ensure_record_page(&source.id).unwrap();
    rt.state
        .store
        .set_page_icon_if_unchanged(&rt.state.workspace.id, &page.id, "", "🌱")
        .unwrap();
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.query_records();
    wait_calendar(&mut rt, |p| {
        p.records.iter().any(|r| r.id == source.id) && p.records.iter().any(|r| r.id == other.id)
    });
    let projection = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap();
    let source_index = projection
        .records
        .iter()
        .position(|r| r.id == source.id)
        .unwrap();
    let other_index = projection
        .records
        .iter()
        .position(|r| r.id == other.id)
        .unwrap();
    let a = rect(&rt, &format!("calendar-{source_index}"));
    let b = rect(&rt, &format!("calendar-{other_index}"));
    assert!((a[1] - b[1]).abs() < 1. && (a[3] - b[3]).abs() < 1.);
    assert!(
        rt.find(&format!("record-icon-calendar-{source_index}"))
            .is_some()
    );
    assert!(
        rt.find(&format!("record-icon-calendar-{other_index}"))
            .is_none()
    );
    for index in 0..2 {
        assert!(
            rt.find(&format!("calendar-relation-{}-{index}", relation.id))
                .is_some()
        );
    }
    click(
        &mut rt,
        &format!("calendar-value-{}-{}-2026-11-10", source.id, relation.id),
    );
    assert!(rt.state.advanced_property.relation_picker.is_some());
    assert!(rt.state.record_peek.is_none());
    let related = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap()
        .records
        .iter()
        .find(|r| r.id == source.id)
        .unwrap();
    assert_eq!(related.values[&relation.id].as_array().unwrap().len(), 2);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_edits_properties_and_respects_page_open_mode() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let item = rt
        .state
        .store
        .create_record(
            &db.id,
            "Calendar item",
            &[
                (date.clone(), json!("2026-11-10")),
                (status.clone(), json!("Not started")),
            ]
            .into(),
        )
        .unwrap();
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.query_records();
    wait_calendar(&mut rt, |p| p.records.iter().any(|r| r.id == item.id));
    let anchor = format!("calendar-value-{}-{}-2026-11-10", item.id, status);
    click(&mut rt, &anchor);
    assert!(rt.find("property-picker").is_some());
    assert!(rt.state.record_peek.is_none());
    click(&mut rt, "property-option-1");
    assert_eq!(
        rt.state.store.get_record(&item.id).unwrap().unwrap().values[&status],
        "In progress"
    );
    wait_calendar(&mut rt, |p| {
        p.records
            .iter()
            .any(|r| r.id == item.id && r.values[&status] == "In progress")
    });
    let value = rect(&rt, &anchor);
    let card = rect(&rt, "calendar-0");
    let day = rect(&rt, "calendar-day-2026-11-12");
    let start = [value[0] + 3., value[1] + value[3] / 2.];
    let target = [day[0] + day[2] / 2., day[1] + 50.];
    rt.pointer_down(start[0], start[1]);
    rt.pointer_move(target[0], target[1]);
    rt.frame(1. / 120.);
    let preview = rect(&rt, "calendar-drag-preview");
    assert!((preview[2] - card[2]).abs() < 1., "{preview:?} {card:?}");
    assert!((preview[3] - card[3]).abs() < 1., "{preview:?} {card:?}");
    assert!((preview[0] - card[0] - (target[0] - start[0])).abs() < 1.);
    assert!((preview[1] - card[1] - (target[1] - start[1])).abs() < 1.);
    assert!(rt.find(&format!("calendar-drag-value-{status}")).is_some());
    assert!(rt.find(&format!("calendar-drag-value-{date}")).is_some());
    rt.pointer_up(target[0], target[1]);
    rt.frame(1. / 120.);
    let saved = rt.state.store.get_record(&item.id).unwrap().unwrap();
    assert_eq!(saved.values[&date], "2026-11-12");
    assert_eq!(saved.values[&status], "In progress");
    wait_calendar(&mut rt, |p| {
        p.records
            .iter()
            .any(|r| r.id == item.id && r.values[&date] == "2026-11-12")
    });
    ui::property_visibility::set_visibility(&mut rt.state, Some(&status), true);
    wait_calendar(&mut rt, |p| p.records.iter().any(|r| r.id == item.id));
    let card = rect(&rt, "calendar-0");
    rt.pointer_down(card[0] + 20., card[1] + 12.);
    rt.pointer_move(card[0] + 40., card[1] + 12.);
    rt.frame(1. / 120.);
    assert!(rt.find(&format!("calendar-drag-value-{date}")).is_some());
    assert!(rt.find(&format!("calendar-drag-value-{status}")).is_none());
    press(&mut rt, Key::Escape);
    rt.pointer_up(card[0] + 40., card[1] + 12.);
    rt.frame(1. / 120.);
    let view_id = db.views[2].id.clone();
    let db = rt
        .state
        .store
        .edit_view_config(&db.id, &view_id, ViewKind::Calendar, |config| {
            config["open_mode"] = json!("side");
            Ok(())
        })
        .unwrap();
    ui::property_visibility::sync_database(&mut rt.state, &db);
    rt.invalidate();
    rt.frame(1. / 120.);
    wait_calendar(&mut rt, |p| p.records.iter().any(|r| r.id == item.id));
    let card = rect(&rt, "calendar-0");
    rt.pointer_down(card[0] + 20., card[1] + 12.);
    rt.pointer_up(card[0] + 20., card[1] + 12.);
    rt.frame(1. / 120.);
    assert!(
        rt.state
            .record_peek
            .as_ref()
            .is_some_and(|peek| peek.mode == crate::state::PeekMode::Side)
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_loads_all_matching_dates_and_supports_layout_settings() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    let db_id = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .id
        .clone();
    let db = rt
        .state
        .store
        .add_property(&db_id, "Due", PropertyKind::Date)
        .unwrap();
    let second = db.properties.last().unwrap().id.clone();
    rt.state.tabs[rt.state.active].database = Some(db.clone());
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    for i in 0..125 {
        rt.state
            .store
            .create_record(&db_id, &format!("Undated {i:03}"), &Default::default())
            .unwrap();
    }
    let range = rt
        .state
        .store
        .create_record(
            &db_id,
            "Date range",
            &[(
                second.clone(),
                json!({"start":"2026-10-30", "end":"2026-11-03"}),
            )]
            .into(),
        )
        .unwrap();
    assert!(rt.state.query_records());
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "db-view-settings");
    click(&mut rt, "view-layout");
    click(&mut rt, &format!("calendar-property-{second}"));
    wait_calendar(&mut rt, |projection| {
        projection.records.iter().any(|r| r.id == range.id) && projection.undated_count == 125
    });
    let days: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| {
            e.key
                .as_deref()
                .is_some_and(|k| k.starts_with("calendar-day-"))
        })
        .collect();
    assert_eq!(days.len(), 35);
    let index = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap()
        .records
        .iter()
        .position(|r| r.id == range.id)
        .unwrap();
    let card = rect(&rt, &format!("calendar-{index}"));
    let day = rect(&rt, "calendar-day-2026-11-01");
    assert!(
        card[2] > day[2] * 2.5,
        "range should span three days: {card:?}, {day:?}"
    );
    click(&mut rt, "calendar-mode-week");
    rt.invalidate();
    rt.frame(1. / 120.);
    assert_eq!(
        rt.elements()
            .iter()
            .filter(|e| e
                .key
                .as_deref()
                .is_some_and(|k| k.starts_with("calendar-day-")))
            .count(),
        7
    );
    assert_eq!(
        rt.state.tab().unwrap().database.as_ref().unwrap().views[2].config["calendar_mode"],
        "week"
    );
    rt.state.view_management = None;
    rt.invalidate();
    rt.frame(0.3);
    click(&mut rt, "calendar-no-date");
    rt.frame(0.3);
    rt.state.undated_search.replace_text("Undated 124");
    rt.invalidate();
    rt.frame(1. / 120.);
    wait_calendar(&mut rt, |projection| {
        projection.undated.len() == 1 && projection.undated[0].title == "Undated 124"
    });
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref() == Some("Undated 124"))
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_drag_moves_resizes_and_schedules_without_losing_date_details() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let property = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let item = rt.state.store.create_record(&db.id, "Trip", &[(property.clone(), json!({"start":"2026-11-10T09:30", "end":"2026-11-12T17:15", "time_zone":"Australia/Perth"}))].into()).unwrap();
    let undated = rt
        .state
        .store
        .create_record(&db.id, "Unscheduled", &Default::default())
        .unwrap();
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    assert!(rt.state.query_records());
    wait_calendar(&mut rt, |p| p.records.iter().any(|r| r.id == item.id));
    let idx = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap()
        .records
        .iter()
        .position(|r| r.id == item.id)
        .unwrap();
    let card = rect(&rt, &format!("calendar-{idx}"));
    let target = rect(&rt, "calendar-day-2026-11-12");
    drag(
        &mut rt,
        [card[0] + 20., card[1] + 12.],
        [target[0] + target[2] / 2., target[1] + 50.],
    );
    let value = rt.state.store.get_record(&item.id).unwrap().unwrap().values[&property].clone();
    assert_eq!(value["start"], "2026-11-12T09:30");
    assert_eq!(value["end"], "2026-11-14T17:15");
    assert_eq!(value["time_zone"], "Australia/Perth");
    wait_calendar(&mut rt, |p| {
        p.records
            .iter()
            .any(|r| r.id == item.id && r.values.get(&property) == Some(&value))
    });
    let idx = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap()
        .records
        .iter()
        .position(|r| r.id == item.id)
        .unwrap();
    let last = rect(&rt, &format!("calendar-{idx}"));
    let target = rect(&rt, "calendar-day-2026-11-16");
    drag(
        &mut rt,
        [last[0] + last[2] - 3., last[1] + 12.],
        [target[0] + target[2] / 2., target[1] + 50.],
    );
    let value = rt.state.store.get_record(&item.id).unwrap().unwrap().values[&property].clone();
    assert_eq!(value["start"], "2026-11-12T09:30");
    assert_eq!(
        value["end"], "2026-11-16T17:15",
        "error={:?} card={last:?} target={target:?} days={:?}",
        rt.state.error, rt.state.database_views.calendar.day_rects
    );
    wait_calendar(&mut rt, |p| {
        p.records
            .iter()
            .any(|r| r.id == item.id && r.values.get(&property) == Some(&value))
    });
    let idx = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap()
        .records
        .iter()
        .position(|r| r.id == item.id)
        .unwrap();
    let card = rect(&rt, &format!("calendar-{idx}"));
    let day = rect(&rt, "calendar-day-2026-11-18");
    rt.pointer_down(card[0] + card[2] / 2., card[1] + 12.);
    rt.pointer_move(day[0] + day[2] / 2., day[1] + 50.);
    rt.frame(1. / 120.);
    assert!(rt.find("calendar-drag-preview").is_some());
    press(&mut rt, Key::Escape);
    rt.pointer_up(day[0] + day[2] / 2., day[1] + 50.);
    rt.frame(1. / 120.);
    assert!(rt.find("calendar-drag-preview").is_none());
    assert_eq!(
        rt.state.store.get_record(&item.id).unwrap().unwrap().values[&property],
        value
    );
    let card = rect(&rt, &format!("calendar-{idx}"));
    rt.pointer_down(card[0] + card[2] / 2., card[1] + 12.);
    rt.pointer_move(day[0] + day[2] / 2., day[1] + 50.);
    rt.frame(1. / 120.);
    let newer = json!({"start":"2026-11-20", "end":"2026-11-21"});
    rt.state
        .store
        .set_record_value_if_unchanged(&item.id, &property, &value, newer.clone())
        .unwrap();
    rt.pointer_up(day[0] + day[2] / 2., day[1] + 50.);
    rt.frame(1. / 120.);
    assert_eq!(
        rt.state.store.get_record(&item.id).unwrap().unwrap().values[&property],
        newer
    );
    assert!(
        rt.state
            .error
            .as_deref()
            .is_some_and(|e| e.contains("not saved"))
    );
    wait_calendar(&mut rt, |p| p.undated.iter().any(|r| r.id == undated.id));
    click(&mut rt, "calendar-no-date");
    rt.frame(0.3);
    let idx = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap()
        .undated
        .iter()
        .position(|r| r.id == undated.id)
        .unwrap();
    click(&mut rt, &format!("calendar-undated-row-{idx}"));
    assert!(
        rt.state.database_views.calendar.schedule.is_some(),
        "schedule missing, peek={}, open={}, suppress={}",
        rt.state.record_peek.is_some(),
        rt.state.undated_open,
        rt.state.database_views.calendar.suppress_press
    );
    click(&mut rt, "calendar-day-2026-11-17");
    assert!(
        rt.state.database_views.calendar.schedule.is_none(),
        "day click did not schedule; error={:?}",
        rt.state.error
    );
    assert_eq!(
        rt.state
            .store
            .get_record(&undated.id)
            .unwrap()
            .unwrap()
            .values[&property],
        "2026-11-17"
    );
    let status = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    let another = rt
        .state
        .store
        .create_record(
            &db.id,
            "Drag to date",
            &[(status.clone(), json!("Done"))].into(),
        )
        .unwrap();
    wait_calendar(&mut rt, |p| p.undated.iter().any(|r| r.id == another.id));
    click(&mut rt, "calendar-no-date");
    rt.frame(0.3);
    let idx = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap()
        .undated
        .iter()
        .position(|r| r.id == another.id)
        .unwrap();
    let row = rect(&rt, &format!("calendar-undated-row-{idx}"));
    let day = rect(&rt, "calendar-day-2026-11-19");
    rt.pointer_down(row[0] + 30., row[1] + row[3] / 2.);
    rt.pointer_move(day[0] + day[2] / 2., day[1] + 50.);
    rt.frame(1. / 120.);
    assert!(rt.find(&format!("calendar-drag-value-{status}")).is_some());
    rt.pointer_up(day[0] + day[2] / 2., day[1] + 50.);
    rt.frame(1. / 120.);
    assert_eq!(
        rt.state
            .store
            .get_record(&another.id)
            .unwrap()
            .unwrap()
            .values[&property],
        "2026-11-19"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_remembers_the_selected_period_after_reopen() {
    let (mut rt, path) = database_runtime(2);
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "calendar-next");
    assert_eq!(
        rt.state.tab().unwrap().calendar_month,
        chrono::NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()
    );
    let page = rt.state.tab().unwrap().page.id.clone();
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    let tab = reopened
        .tabs
        .iter()
        .find(|tab| tab.page.id == page)
        .unwrap();
    assert_eq!(
        tab.calendar_month,
        chrono::NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()
    );
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_layout_can_create_a_missing_date_property() {
    let (mut rt, path) = database_runtime(2);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .clone();
    let db = rt.state.store.delete_property(&db.id, &date).unwrap();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "db-view-settings");
    click(&mut rt, "view-layout");
    click(&mut rt, "calendar-create-date");
    let db = rt.state.tab().unwrap().database.as_ref().unwrap();
    let new_date = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap();
    assert_eq!(db.views[2].config["calendar_date_property"], new_date.id);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_ranges_span_days_and_stack_when_they_overlap() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let property = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let a = rt
        .state
        .store
        .create_record(
            &db.id,
            "Long range",
            &[(
                property.clone(),
                json!({"start":"2026-11-10", "end":"2026-11-17"}),
            )]
            .into(),
        )
        .unwrap();
    let b = rt
        .state
        .store
        .create_record(
            &db.id,
            "Overlap",
            &[(property, json!({"start":"2026-11-11", "end":"2026-11-12"}))].into(),
        )
        .unwrap();
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.query_records();
    wait_calendar(&mut rt, |p| p.records.len() == 2);
    let projection = rt
        .state
        .database_views
        .calendar
        .projection
        .as_ref()
        .unwrap();
    let ai = projection
        .records
        .iter()
        .position(|r| r.id == a.id)
        .unwrap();
    let bi = projection
        .records
        .iter()
        .position(|r| r.id == b.id)
        .unwrap();
    for (width, scale) in [(850., 1.), (850., 2.), (1440., 1.), (1440., 2.)] {
        rt.set_viewport((width, 1000.), scale);
        rt.frame(1. / 60.);
        rt.frame(1. / 60.);
        let grid = rect(&rt, "calendar-grid");
        let viewport = rect(&rt, "db-body");
        assert!((grid[2] - viewport[2]).abs() < 2., "{grid:?} {viewport:?}");
        assert!(
            grid[2] <= 944. + 2.,
            "Calendar should share the page width: {grid:?}"
        );
        let week0 = rect(&rt, "calendar-week-0");
        let week1 = rect(&rt, "calendar-week-1");
        assert!(
            (week0[1] + week0[3] - week1[1]).abs() < 1.,
            "{week0:?} {week1:?}"
        );
        let first = rect(&rt, &format!("calendar-{ai}"));
        let overlap = rect(&rt, &format!("calendar-{bi}"));
        let tue = rect(&rt, "calendar-day-2026-11-10");
        assert!(tue[3] >= 136.);
        let sat = rect(&rt, "calendar-day-2026-11-14");
        let continued = rect(&rt, &format!("calendar-{ai}-2026-11-15"));
        let end = rect(&rt, "calendar-day-2026-11-17");
        assert!((first[0] - tue[0] - 4.).abs() < 2., "{first:?} {tue:?}");
        assert!(
            (first[0] + first[2] - sat[0] - sat[2] + 4.).abs() < 2.,
            "{first:?} {sat:?}"
        );
        assert!(
            (continued[0] + continued[2] - end[0] - end[2] + 4.).abs() < 2.,
            "{continued:?} {end:?}"
        );
        assert!(
            (first[1] - overlap[1]).abs() >= 29.,
            "{first:?} {overlap:?}"
        );
    }
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_scrolls_to_the_next_and_previous_period_at_the_edges() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    rt.set_viewport((1200., 700.), 1.);
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    wait_calendar(&mut rt, |_| true);
    let view = rect(&rt, "db-body");
    let (x, y) = (view[0] + view[2] / 2., view[1] + view[3] / 2.);
    rt.wheel_event(rune::WheelEvent::new(0., -100.).at(x, y));
    rt.frame(0.2);
    assert_eq!(
        rt.state.tab().unwrap().calendar_month.month(),
        11,
        "scroll within a long month first"
    );
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., f32::MAX));
    rt.frame(0.2);
    rt.wheel_event(rune::WheelEvent::new(0., -100.).at(x, y));
    rt.frame(0.2);
    assert_eq!(rt.state.tab().unwrap().calendar_month.month(), 12);
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 0.));
    rt.frame(0.2);
    rt.wheel_event(
        rune::WheelEvent::new(0., 30.)
            .at(x, y)
            .precise(true)
            .phase(rune::GesturePhase::Began),
    );
    rt.frame(0.02);
    assert_eq!(rt.state.tab().unwrap().calendar_month.month(), 12);
    rt.wheel_event(rune::WheelEvent::new(0., 30.).at(x, y).precise(true));
    rt.frame(0.02);
    assert_eq!(rt.state.tab().unwrap().calendar_month.month(), 11);
    rt.wheel_event(rune::WheelEvent::new(0., 100.).at(x, y).precise(true));
    rt.frame(0.02);
    assert_eq!(
        rt.state.tab().unwrap().calendar_month.month(),
        11,
        "one trackpad gesture must not skip months"
    );
    let mut momentum = rune::WheelEvent::new(0., 100.)
        .at(x, y)
        .precise(true)
        .phase(rune::GesturePhase::Began);
    momentum.momentum = true;
    rt.wheel_event(momentum);
    rt.frame(0.02);
    assert_eq!(
        rt.state.tab().unwrap().calendar_month.month(),
        11,
        "momentum must not skip months"
    );
    rt.wheel_event(
        rune::WheelEvent::new(0., 0.)
            .at(x, y)
            .precise(true)
            .phase(rune::GesturePhase::Ended),
    );
    rt.frame(0.02);
    rt.state.tabs[rt.state.active]
        .database
        .as_mut()
        .unwrap()
        .views[2]
        .config["calendar_mode"] = json!("week");
    rt.invalidate();
    rt.frame(0.2);
    let before = rt.state.tab().unwrap().calendar_month;
    rt.update(|_, tx| tx.scroll_to_instant("db-body", 0., 0.));
    rt.frame(0.2);
    rt.wheel_event(
        rune::WheelEvent::new(0., 100.)
            .at(x, y)
            .phase(rune::GesturePhase::Began),
    );
    rt.frame(0.2);
    assert_eq!(
        rt.state.tab().unwrap().calendar_month,
        before - chrono::Duration::days(7)
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_pages_through_dense_date_ranges() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let property = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    for i in 0..1001 {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 1 + (i % 28)).unwrap();
        rt.state
            .store
            .create_record(
                &db.id,
                &format!("Entry {i}"),
                &[(property.clone(), json!(date.to_string()))].into(),
            )
            .unwrap();
    }
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.query_records();
    wait_calendar(&mut rt, |p| p.dated_count == 1001 && p.offset == 0);
    assert!(rt.find("calendar-page-next").is_some());
    click(&mut rt, "calendar-page-next");
    wait_calendar(&mut rt, |p| p.dated_count == 1001 && p.offset == 1000);
    assert_eq!(
        rt.state
            .database_views
            .calendar
            .projection
            .as_ref()
            .unwrap()
            .records
            .len(),
        1
    );
    click(&mut rt, "calendar-page-prev");
    wait_calendar(&mut rt, |p| p.dated_count == 1001 && p.offset == 0);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn calendar_ignores_stale_results_after_fast_navigation() {
    let (mut rt, path) = database_runtime(2);
    rt.poll = Some(poll);
    let db = rt.state.tab().unwrap().database.clone().unwrap();
    let property = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Date)
        .unwrap()
        .id
        .clone();
    let november = rt
        .state
        .store
        .create_record(
            &db.id,
            "November",
            &[(property.clone(), json!("2026-11-10"))].into(),
        )
        .unwrap();
    let january = rt
        .state
        .store
        .create_record(&db.id, "January", &[(property, json!("2027-01-15"))].into())
        .unwrap();
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.query_records();
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, "calendar-next");
    click(&mut rt, "calendar-next");
    wait_calendar(&mut rt, |p| p.records.iter().any(|r| r.id == january.id));
    assert!(
        !rt.state
            .database_views
            .calendar
            .projection
            .as_ref()
            .unwrap()
            .records
            .iter()
            .any(|r| r.id == november.id)
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn monday_preference_changes_calendar_and_date_picker_and_persists() {
    let (mut rt, path) = database_runtime(2);
    rt.state.tabs[rt.state.active].calendar_month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.state.modal = Some(state::Modal::Settings);
    rt.invalidate();
    rt.frame(0.3);
    click(&mut rt, "week-start-monday");
    assert!(rt.state.prefs.start_week_on_monday);
    press(&mut rt, Key::Escape);
    rt.invalidate();
    rt.frame(0.3);
    assert!(rt.find("calendar-day-2026-10-26").is_some());
    let monday = rect(&rt, "calendar-weekday-Mon");
    let sunday = rect(&rt, "calendar-weekday-Sun");
    assert!(monday[0] < sunday[0]);
    assert!(rt.state.select_database_view(0));
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    let id = rt.state.tab().unwrap().records[0].id.clone();
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
    rt.state.open_property_picker(&id, &date);
    rt.state.property_picker.as_mut().unwrap().month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    rt.invalidate();
    rt.frame(0.3);
    let first = rect(&rt, "date-2026-10-26");
    let sunday = rect(&rt, "date-2026-11-01");
    assert!(first[0] < sunday[0]);
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    assert!(reopened.prefs.start_week_on_monday);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}
