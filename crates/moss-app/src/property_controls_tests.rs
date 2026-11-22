use super::*;
use moss_core::{PropertyKind, new_id};
use serde_json::json;

#[test]
fn picker_saves_choices_and_protects_dates() {
    let path = std::env::temp_dir().join(format!("moss-picker-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    let db = s.tab().unwrap().database.as_ref().unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &status);
    s.property_picker_mut().unwrap().input.replace_text("progress");
    s.submit_property_picker();
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&status],
        "In progress"
    );
    s.open_property_picker(&id, &date);
    s.property_picker_mut().unwrap().input.replace_text("bad date");
    s.submit_property_picker();
    assert!(s.property_picker().unwrap().error.is_some());
    assert!(
        !s.store
            .get_record(&id)
            .unwrap()
            .unwrap()
            .values
            .contains_key(&date)
    );
    s.property_picker_mut().unwrap().input.replace_text("2026-11-12");
    s.submit_property_picker();
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-11-12"
    );
    let mut record = s.store.get_record(&id).unwrap().unwrap();
    record.values.insert(
        date.clone(),
        json!({"start":"2026-11-12", "end":"2026-11-14"}),
    );
    s.store.save_record(&record).unwrap();
    s.open_property_picker(&id, &date);
    assert!(
        s.property_picker()
            .unwrap()
            .date
            .as_ref()
            .unwrap()
            .has_end
    );
    s.dismiss_cell_picker();
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date]["end"],
        "2026-11-14"
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn picker_escape_discards_draft_and_keeps_peek_open() {
    let path = std::env::temp_dir().join(format!("moss-picker-ui-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    s.create_peek(Default::default());
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
    let id = s.record_peek.as_ref().unwrap().record.id.clone();
    s.open_property_picker(&id, &date);
    s.property_picker_mut().unwrap().input.replace_text("2026-11-12");
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (1440., 900.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(crate::key);
    rt.frame(0.);
    assert!(rt.find("property-picker-input").is_some());
    rt.key(rune::KeyEvent {
        key: rune::Key::Escape,
        mods: Default::default(),
    });
    rt.frame(1. / 120.);
    assert!(rt.state.property_picker().is_none());
    assert!(rt.state.record_peek.is_some());
    assert!(
        !rt.state
            .store
            .get_record(&id)
            .unwrap()
            .unwrap()
            .values
            .contains_key(&date)
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn status_keyboard_choice_selects_next_table_row() {
    let path = std::env::temp_dir().join(format!("moss-picker-next-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    let tab = s.tab().unwrap();
    let first = tab.records[0].id.clone();
    let second = tab.records[1].id.clone();
    let status = tab
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    s.open_property_picker(&first, &status);
    s.property_picker_mut().unwrap().selected = Some(1);
    s.submit_property_picker();
    assert_eq!(
        s.store.get_record(&first).unwrap().unwrap().values[&status],
        "In progress"
    );
    assert_eq!(s.selected_cell, Some((second, Some(status))));
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn failed_picker_save_keeps_draft_until_retry_or_discard() {
    let path = std::env::temp_dir().join(format!("moss-picker-failed-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    assert!(!s.choose_property_value("invalid"));
    s.dismiss_cell_picker();
    assert_eq!(s.cell.text, "invalid");
    assert!(s.property_picker().unwrap().save_failed);
    s.open_search();
    assert!(s.property_picker().is_some());
    assert!(!matches!(s.modal, Some(crate::state::Modal::Search)));
    assert!(!s.flush_current());
    assert!(s.choose_property_value("2026-11-12"));
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-11-12"
    );
    s.open_property_picker(&id, &date);
    assert!(!s.choose_property_value("invalid"));
    s.discard_failed_property_change();
    assert!(s.property_picker().is_none());
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-11-12"
    );
    s.open_property_picker(&id, &date);
    s.editing_cell = None;
    assert!(!s.choose_property_value("2026-11-13"));
    assert!(s.property_picker().unwrap().save_failed);
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-11-12"
    );
    s.discard_failed_property_change();
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn search_shortcut_dismisses_uncommitted_picker() {
    let path = std::env::temp_dir().join(format!("moss-picker-search-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    s.property_picker_mut().unwrap().input.replace_text("12/11/2026");
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (1440., 900.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(crate::key);
    rt.frame(0.);
    rt.key(rune::KeyEvent {
        key: rune::Key::Command('t'),
        mods: Default::default(),
    });
    rt.frame(1. / 120.);
    assert!(rt.state.property_picker().is_none());
    assert!(matches!(rt.state.modal, Some(crate::state::Modal::Search)));
    assert!(rt.state.search_new_tab && rt.state.search.focused);
    assert!(
        !rt.state
            .store
            .get_record(&id)
            .unwrap()
            .unwrap()
            .values
            .contains_key(&date)
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn november_calendar_shows_six_even_weeks() {
    let path = std::env::temp_dir().join(format!("moss-calendar-layout-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    s.property_picker_mut().unwrap().month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (1440., 900.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    let elements = rt.elements();
    let find = |key: &str| {
        elements
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap()
            .rect
    };
    let first = find("date-2026-11-01");
    let next = find("date-2026-11-08");
    let last = find("date-2026-12-12");
    assert!((next[1] - first[1] - 32.).abs() < 0.1);
    assert!((last[1] - first[1] - 160.).abs() < 0.1);
    assert!((first[2] - 32.).abs() < 0.1 && (first[3] - 30.).abs() < 0.1);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn long_status_options_fit_and_keyboard_reaches_scrolled_choice() {
    let path = std::env::temp_dir().join(format!("moss-picker-long-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    let mut db = s.tab().unwrap().database.clone().unwrap();
    let status = db
        .properties
        .iter_mut()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap();
    let status_id = status.id.clone();
    let options: Vec<String> = (0..20)
        .map(|i| format!("Long project status with many words number {i}"))
        .collect();
    status.config = json!({"options": options});
    s.store.save_database(&db).unwrap();
    s.tabs[s.active].database = Some(db);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &status_id);
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (850., 600.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(crate::key);
    rt.frame(0.);
    let elements = rt.elements();
    let get = |key: &str| {
        elements
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap()
    };
    let panel = get("property-picker");
    let input = get("property-picker-input");
    let options_box = get("property-options");
    assert!(panel.rect[0] >= 0. && panel.rect[0] + panel.rect[2] <= 850.);
    assert!(panel.rect[1] >= 0. && panel.rect[1] + panel.rect[3] <= 600.);
    assert!(input.rect[2] >= 50. && options_box.rect[3] <= 250.1);
    assert!(input.focused);
    for _ in 0..15 {
        rt.key(rune::KeyEvent {
            key: rune::Key::Down,
            mods: Default::default(),
        });
        rt.frame(1. / 120.);
    }
    assert_eq!(
        rt.state.property_picker().unwrap().selected,
        Some(14)
    );
    let elements = rt.elements();
    let selected = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("property-option-14"))
        .unwrap();
    assert!(selected.visible[3] > 0.);
    rt.key(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: Default::default(),
    });
    rt.frame(1. / 120.);
    assert_eq!(
        rt.state.store.get_record(&id).unwrap().unwrap().values[&status_id],
        "Long project status with many words number 14"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn date_picker_fits_minimum_window_and_scrolls_all_six_weeks() {
    let path = std::env::temp_dir().join(format!("moss-date-fit-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    s.property_picker_mut().unwrap().month =
        chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (850., 600.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    let elements = rt.elements();
    let get = |key: &str| {
        elements
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap()
    };
    let panel = get("property-picker");
    assert!(panel.rect[0] >= 0. && panel.rect[0] + panel.rect[2] <= 850.);
    assert!(panel.rect[1] >= 0. && panel.rect[1] + panel.rect[3] <= 600.);
    assert!(get("property-picker-input").focused);
    assert!(get("date-next").visible[3] > 0.);
    let scroll = get("date-scroll");
    let (x, y) = (scroll.visible[0] + 20., scroll.visible[1] + 20.);
    rt.wheel(x, y, -600.);
    rt.frame(1. / 120.);
    let elements = rt.elements();
    let last = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("date-2026-12-12"))
        .unwrap();
    assert!(last.visible[2] > 0. && last.visible[3] > 0.);
    let (x, y) = rt.find("date-2026-12-12").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
    assert_eq!(
        rt.state.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-12-12"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn today_and_a_different_selected_day_are_distinct() {
    let path = std::env::temp_dir().join(format!("moss-today-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    let today = chrono::Local::now().date_naive();
    let other = if today.day() == 1 {
        today + chrono::Duration::days(1)
    } else {
        today - chrono::Duration::days(1)
    };
    s.cell.replace_text(&other.to_string());
    s.property_picker_mut().unwrap().month = today.with_day(1).unwrap();
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (1440., 900.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    let elements = rt.elements();
    let today_key = format!("date-{today}");
    let chosen_key = format!("date-{other}");
    let current = elements
        .iter()
        .find(|e| e.key.as_deref() == Some(&today_key))
        .unwrap();
    let selected = elements
        .iter()
        .find(|e| e.key.as_deref() == Some(&chosen_key))
        .unwrap();
    assert!(current.radius > selected.radius);
    assert!(current.fill.r > current.fill.b && selected.fill.b > selected.fill.r);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn date_range_time_validation_preserves_metadata_and_offsets() {
    let path = std::env::temp_dir().join(format!("moss-range-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    s.toggle_date_end();
    s.toggle_date_time();
    s.property_picker_mut().unwrap().input.replace_text("12/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_input.replace_text("11/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().start_time_input.replace_text("09:30");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_time_input.replace_text("10:30");
    s.submit_property_picker();
    assert!(s.property_picker().unwrap().error.is_some());
    assert!(
        !s.store
            .get_record(&id)
            .unwrap()
            .unwrap()
            .values
            .contains_key(&date)
    );
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_input.replace_text("12/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_time_input.replace_text("08:30");
    s.submit_property_picker();
    assert!(s.property_picker().unwrap().error.is_some());
    assert!(
        !s.store
            .get_record(&id)
            .unwrap()
            .unwrap()
            .values
            .contains_key(&date)
    );
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_input.replace_text("13/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_time_input.replace_text("10:30");
    s.submit_property_picker();
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        json!({"start":"2026-11-12T09:30", "end":"2026-11-13T10:30"})
    );
    let mut record = s.store.get_record(&id).unwrap().unwrap();
    record.values.insert(date.clone(), json!({"start":"2026-11-12T09:30:15+08:00", "end":"2026-11-13T10:30:45+08:00", "time_zone":"Australia/Perth", "extra":{"keep":true}}));
    s.store.save_record(&record).unwrap();
    s.sync_record(&record);
    s.open_property_picker(&id, &date);
    s.property_picker_mut().unwrap().input.replace_text("14/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_input.replace_text("15/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().start_time_input.replace_text("11:25");
    s.submit_property_picker();
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        json!({"start":"2026-11-14T11:25:15+08:00", "end":"2026-11-15T10:30:45+08:00", "time_zone":"Australia/Perth", "extra":{"keep":true}})
    );
    s.open_property_picker(&id, &date);
    s.property_picker_mut().unwrap().input.replace_text("16/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_input.replace_text("17/11/2026");
    let mut changed = s.store.get_record(&id).unwrap().unwrap();
    changed.values.insert(date.clone(), json!("2026-11-20"));
    s.store.save_record(&changed).unwrap();
    s.submit_property_picker();
    assert!(s.property_picker().unwrap().save_failed);
    s.dismiss_cell_picker();
    assert!(s.property_picker().is_some());
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-11-20"
    );
    s.discard_failed_property_change();
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn date_range_and_time_controls_save_from_the_picker() {
    let path = std::env::temp_dir().join(format!("moss-date-ui-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (1440., 900.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(crate::key);
    rt.frame(1. / 120.);
    let press = |rt: &mut rune::Runtime<State>, key: rune::Key| {
        rt.key(rune::KeyEvent {
            key,
            mods: Default::default(),
        });
        rt.frame(1. / 120.);
    };
    let click = |rt: &mut rune::Runtime<State>, key: &str| {
        let (x, y) = rt.find(key).unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
    };
    press(&mut rt, rune::Key::Text("12/11/2026".into()));
    click(&mut rt, "date-toggle-end");
    click(&mut rt, "date-end-input");
    press(&mut rt, rune::Key::Text("13/11/2026".into()));
    click(&mut rt, "date-toggle-time");
    click(&mut rt, "date-start-time");
    press(&mut rt, rune::Key::Text("09:30".into()));
    click(&mut rt, "date-end-time");
    press(&mut rt, rune::Key::Text("10:45".into()));
    press(&mut rt, rune::Key::Enter);
    assert_eq!(
        rt.state.store.get_record(&id).unwrap().unwrap().values[&date],
        json!({"start":"2026-11-12T09:30", "end":"2026-11-13T10:45"})
    );
    assert!(rt.state.property_picker().is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn expanded_date_controls_fit_minimum_window() {
    let path = std::env::temp_dir().join(format!("moss-date-expanded-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    s.toggle_date_end();
    s.toggle_date_time();
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (850., 600.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    let elements = rt.elements();
    let get = |key: &str| {
        elements
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap()
    };
    let panel = get("property-picker");
    assert!(panel.rect[1] >= 0. && panel.rect[1] + panel.rect[3] <= 600.);
    for key in [
        "property-picker-input",
        "date-end-input",
        "date-start-time",
        "date-end-time",
        "date-next",
    ] {
        assert!(get(key).visible[3] > 0., "{key} is clipped");
    }
    assert!(get("date-scroll").visible[3] >= 30.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn failed_extended_date_keeps_retry_visible_in_minimum_window() {
    let path = std::env::temp_dir().join(format!("moss-date-failure-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    s.toggle_date_end();
    s.toggle_date_time();
    s.property_picker_mut().unwrap().input.replace_text("12/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_input.replace_text("13/11/2026");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().start_time_input.replace_text("09:30");
    s.property_picker_mut().unwrap().date.as_mut().unwrap().end_time_input.replace_text("10:30");
    let mut record = s.store.get_record(&id).unwrap().unwrap();
    record.values.insert(date.clone(), json!("2026-11-20"));
    s.store.save_record(&record).unwrap();
    s.submit_property_picker();
    assert!(s.property_picker().unwrap().save_failed);
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (850., 600.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    let elements = rt.elements();
    let get = |key: &str| {
        elements
            .iter()
            .find(|e| e.key.as_deref() == Some(key))
            .unwrap()
    };
    let panel = get("property-picker");
    assert!(panel.rect[1] >= 0. && panel.rect[1] + panel.rect[3] <= 600.);
    assert!(get("property-retry").visible[3] > 0. && get("property-discard").visible[3] > 0.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn date_retry_uses_current_fields_and_clear_stays_explicit() {
    let path = std::env::temp_dir().join(format!("moss-date-retry-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    let mut record = s.store.get_record(&id).unwrap().unwrap();
    record.values.insert(date.clone(), json!("2026-11-01"));
    s.store.save_record(&record).unwrap();
    s.open_property_picker(&id, &date);
    s.property_picker_mut().unwrap().input.replace_text("02/11/2026");
    record.values.insert(date.clone(), json!("2026-11-20"));
    s.store.save_record(&record).unwrap();
    s.submit_property_picker();
    assert!(s.property_picker().unwrap().save_failed);
    record.values.insert(date.clone(), json!("2026-11-01"));
    s.store.save_record(&record).unwrap();
    s.property_picker_mut().unwrap().input.replace_text("03/11/2026");
    s.date_draft_changed();
    s.retry_property_change();
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-11-03"
    );

    s.open_property_picker(&id, &date);
    record.values.insert(date.clone(), json!("2026-11-20"));
    s.store.save_record(&record).unwrap();
    s.clear_date_picker();
    assert!(s.property_picker().unwrap().save_failed);
    record.values.insert(date.clone(), json!("2026-11-03"));
    s.store.save_record(&record).unwrap();
    s.retry_property_change();
    assert!(s.store.get_record(&id).unwrap().unwrap().values[&date].is_null());
    s.open_property_picker(&id, &date);
    record.values.insert(date.clone(), json!("2026-11-20"));
    s.store.save_record(&record).unwrap();
    s.clear_date_picker();
    assert!(s.property_picker().unwrap().save_failed);
    record.values.insert(date.clone(), Value::Null);
    s.store.save_record(&record).unwrap();
    s.property_picker_mut().unwrap().input.replace_text("04/11/2026");
    s.date_draft_changed();
    s.retry_property_change();
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-11-04"
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn date_range_checks_seconds_and_rejects_different_offsets() {
    let path = std::env::temp_dir().join(format!("moss-date-order-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    let mut record = s.store.get_record(&id).unwrap().unwrap();
    let bad_seconds =
        json!({"start":"2026-11-12T09:30:50.5+08:00", "end":"2026-11-12T09:30:10.7+08:00"});
    record.values.insert(date.clone(), bad_seconds.clone());
    s.store.save_record(&record).unwrap();
    s.open_property_picker(&id, &date);
    s.submit_property_picker();
    assert!(s.property_picker().unwrap().error.is_some());
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        bad_seconds
    );
    s.dismiss_cell_picker();
    let offsets = json!({"start":"2026-11-12T09:30:00+08:00", "end":"2026-11-13T10:30:00+09:00"});
    record.values.insert(date.clone(), offsets.clone());
    s.store.save_record(&record).unwrap();
    s.open_property_picker(&id, &date);
    s.submit_property_picker();
    assert!(
        s.property_picker()
            .unwrap()
            .error
            .as_deref()
            .unwrap()
            .contains("same offset")
    );
    assert_eq!(
        s.store.get_record(&id).unwrap().unwrap().values[&date],
        offsets
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn editing_after_failed_clear_makes_retry_save_the_visible_date() {
    let path = std::env::temp_dir().join(format!("moss-date-retry-ui-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
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
    let id = s.tab().unwrap().records[0].id.clone();
    s.open_property_picker(&id, &date);
    let mut record = s.store.get_record(&id).unwrap().unwrap();
    record.values.insert(date.clone(), json!("2026-11-20"));
    s.store.save_record(&record).unwrap();
    s.clear_date_picker();
    assert!(s.property_picker().unwrap().save_failed);
    record.values.insert(date.clone(), Value::Null);
    s.store.save_record(&record).unwrap();
    let mut rt = rune::Runtime::new(
        s,
        crate::ui::view,
        (1440., 900.),
        rune::Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(crate::key);
    rt.frame(1. / 120.);
    rt.key(rune::KeyEvent {
        key: rune::Key::Text("04/11/2026".into()),
        mods: Default::default(),
    });
    rt.frame(1. / 120.);
    let (x, y) = rt.find("property-retry").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
    assert_eq!(
        rt.state.store.get_record(&id).unwrap().unwrap().values[&date],
        "2026-11-04"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
