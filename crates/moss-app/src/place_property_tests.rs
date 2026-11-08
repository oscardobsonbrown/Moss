use super::*;
use moss_core::PlaceValue;

#[test]
fn place_can_be_edited_in_table_peek_gallery_and_list() {
    let (mut rt, path) = database_runtime(0);
    rt.poll = Some(poll);
    let db_id = rt.state.tab().unwrap().database.as_ref().unwrap().id.clone();
    let db = rt.state.store.add_property(&db_id, "Place", PropertyKind::Place).unwrap();
    let property = db.properties.last().unwrap().id.clone();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.state.query_records();
    let id = rt.state.tab().unwrap().records[0].id.clone();
    rt.invalidate(); rt.frame(0.3);
    click(&mut rt, &format!("cell-0-{property}"));
    assert!(rt.find("place-search-input").is_some());
    rt.state.database_views.map.place_query.replace_text("Perth");
    let place = PlaceValue { name: "Perth, Australia".into(), latitude: -31.95, longitude: 115.86 };
    rt.state.database_views.map.search_results = vec![place.clone()];
    rt.invalidate(); rt.frame(1. / 120.);
    click(&mut rt, "place-result-0");
    assert_eq!(rt.state.store.get_record(&id).unwrap().unwrap().values[&property], json!(place));
    rt.state.open_record(0);
    rt.invalidate(); rt.frame(0.3);
    click(&mut rt, &format!("peek-property-{property}"));
    assert!(rt.find("place-search-input").is_some(), "picker open={}, error={:?}, peek={}", rt.state.place_picker.is_some(), rt.state.error, rt.state.record_peek.is_some());
    rt.state.database_views.map.place_query.replace_text("123 Example Street");
    rt.invalidate(); rt.frame(1. / 120.);
    click(&mut rt, "place-save-text");
    assert_eq!(rt.state.store.get_record(&id).unwrap().unwrap().values[&property], "123 Example Street");
    assert!(rt.state.close_peek());
    for view in [4, 5] {
        assert!(rt.state.select_database_view(view));
        rt.invalidate(); rt.frame(1. / 120.);
        let key = if view == 4 { format!("gallery-value-0-{property}") } else { format!("list-value-0-{property}") };
        click(&mut rt, &key);
        assert!(rt.find("place-search-input").is_some());
        assert!(rt.state.record_peek.is_none());
        press(&mut rt, Key::Escape);
        assert!(rt.state.place_picker.is_none());
    }
    rt.state.open_database_record(&db_id, &id, "full");
    rt.invalidate(); rt.frame(0.3);
    click(&mut rt, &format!("page-place-{property}"));
    assert!(rt.find("place-search-input").is_some());
    let expected = rt.state.store.get_record(&id).unwrap().unwrap().values[&property].clone();
    rt.state.store.set_record_place_text_if_unchanged(&id, &property, &expected, "A newer address").unwrap();
    rt.state.database_views.map.place_query.replace_text("An older edit");
    rt.state.save_place_choice(None, true);
    assert!(rt.state.place_picker.as_ref().unwrap().error.is_some());
    assert_eq!(rt.state.store.get_record(&id).unwrap().unwrap().values[&property], "A newer address");
    rt.state.dismiss_place_picker();
    rt.state.open_place_picker(&id, &property);
    rt.state.save_place_choice(None, false);
    assert!(rt.state.store.get_record(&id).unwrap().unwrap().values[&property].is_null());
    drop(rt); std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn place_search_finishes_outside_map_and_offline_text_is_saved() {
    use std::time::{Duration, Instant};
    let (mut rt, path) = database_runtime(0);
    rt.poll = Some(poll);
    let db_id = rt.state.tab().unwrap().database.as_ref().unwrap().id.clone();
    let db = rt.state.store.add_property(&db_id, "Place", PropertyKind::Place).unwrap();
    let property = db.properties.last().unwrap().id.clone();
    rt.state.tabs[rt.state.active].database = Some(db);
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.state.query_records();
    let id = rt.state.tab().unwrap().records[0].id.clone();
    rt.invalidate(); rt.frame(0.2);
    click(&mut rt, &format!("cell-0-{property}"));
    press(&mut rt, Key::Text("123 Test Street".into()));
    press(&mut rt, Key::Enter);
    let deadline = Instant::now() + Duration::from_secs(5);
    while rt.state.database_views.map.search_error.is_none() {
        assert!(Instant::now() < deadline, "place search did not finish");
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(rt.state.place_picker.is_some());
    rt.invalidate(); rt.frame(1. / 120.);
    click(&mut rt, "place-save-text");
    assert_eq!(rt.state.store.get_record(&id).unwrap().unwrap().values[&property], "123 Test Street");
    drop(rt); std::fs::remove_dir_all(path).unwrap();
}
