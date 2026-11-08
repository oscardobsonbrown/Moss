use super::*;
use moss_core::{PropertyKind, new_id};
use rune::{Env, Runtime};
use std::time::{Duration, Instant};

#[test]
fn map_selects_a_source_and_opens_a_record_outside_the_loaded_page() {
    let path = std::env::temp_dir().join(format!("moss-map-{}", new_id()));
    let mut state = crate::state::State::open(path.clone(), None).unwrap();
    state.new_page(true, None);
    let db = state.tab().unwrap().database.clone().unwrap();
    for i in 0..110 {
        state
            .store
            .create_record(&db.id, &format!("Record {i}"), &Default::default())
            .unwrap();
    }
    let db = state
        .store
        .add_property(&db.id, "Place", PropertyKind::Place)
        .unwrap();
    let place = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Place)
        .unwrap()
        .id
        .clone();
    let all = state
        .store
        .query_records(
            &db.id,
            &RecordQuery {
                limit: 200,
                ..Default::default()
            },
        )
        .unwrap();
    let target = all.last().unwrap().id.clone();
    let other = all[all.len() - 2].id.clone();
    state
        .store
        .set_record_place_if_unchanged(
            &target,
            &place,
            &Value::Null,
            Some(PlaceValue {
                name: "Test location".into(),
                latitude: 0.,
                longitude: 0.,
            }),
        )
        .unwrap();
    let db = state
        .store
        .create_view(&db.id, "Map", ViewKind::Map)
        .unwrap();
    let index = db
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Map)
        .unwrap();
    state.tabs[state.active].database = Some(db);
    assert!(state.select_database_view(index));
    assert!(!state.tab().unwrap().records.iter().any(|r| r.id == target));
    let mut rt = Runtime::new(state, crate::ui::view, (1200., 800.), Env::default());
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    let click = |rt: &mut Runtime<crate::state::State>, key: &str| {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1. / 120.);
    };
    rt.update(|s, _| s.select_map_source(&place));
    rt.frame(0.);
    let deadline = Instant::now() + Duration::from_secs(10);
    while rt.state.database_views.map.points.is_empty() {
        assert!(
            Instant::now() < deadline,
            "map failed: {:?}",
            rt.state.database_views.map.error
        );
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(rt.state.database_views.map.points[0].record_id, target);
    let (x, y) = rt.find("map-canvas").unwrap();
    let zoom = rt.state.database_views.map.viewport.zoom;
    rt.wheel_event(
        rune::WheelEvent::new(0., 20.)
            .at(x, y)
            .phase(rune::GesturePhase::Began),
    );
    rt.frame(1. / 60.);
    assert_eq!(
        rt.state.database_views.map.viewport.zoom, zoom,
        "normal scrolling must reach the page controls"
    );
    rt.wheel_event(
        rune::WheelEvent::new(0., 0.)
            .at(x, y)
            .phase(rune::GesturePhase::Ended),
    );
    rt.frame(0.5);
    rt.wheel_event(
        rune::WheelEvent::new(0., 20.)
            .at(x, y)
            .mods(rune::Mods {
                cmd: true,
                ..Default::default()
            })
            .phase(rune::GesturePhase::Began),
    );
    rt.frame(1. / 60.);
    assert_eq!(rt.state.database_views.map.viewport.zoom, zoom + 1.);
    click(&mut rt, "map-marker-0");
    assert_eq!(rt.state.record_peek.as_ref().unwrap().record.id, target);
    rt.update(|s, _| {
        assert!(s.close_peek());
        s.store
            .set_record_place_if_unchanged(
                &other,
                &place,
                &Value::Null,
                Some(PlaceValue {
                    name: "Test location".into(),
                    latitude: 0.,
                    longitude: 0.,
                }),
            )
            .unwrap();
        s.database_views.map.invalidate();
    });
    while rt.state.database_views.map.points.len() < 2 {
        assert!(Instant::now() < deadline);
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(2));
    }
    rt.frame(1. / 60.);
    click(&mut rt, "map-marker-0");
    rt.frame(1. / 60.);
    click(&mut rt, &format!("map-cluster-record-{other}"));
    assert_eq!(rt.state.record_peek.as_ref().unwrap().record.id, other);
    rt.update(|s, _| {
        assert!(s.close_peek());
    });
    rt.frame(1. / 60.);
    click(&mut rt, "map-marker-0");
    rt.frame(1. / 60.);
    click(&mut rt, &format!("map-cluster-record-{target}"));
    assert_eq!(rt.state.record_peek.as_ref().unwrap().record.id, target);
    rt.update(|s, _| {
        assert!(s.close_peek());
    });
    rt.frame(1. / 60.);
    click(&mut rt, "map-marker-0");
    rt.update(|s, _| {
        s.tabs[s.active].view = 0;
        assert!(crate::map_clusters::poll(s));
    });
    assert!(rt.state.database_views.map_cluster.is_none());
    drop(rt);
    let _ = std::fs::remove_dir_all(path);
}
#[test]
fn reset_ignores_result_after_newer_gesture() {
    let path = std::env::temp_dir().join(format!("moss-map-fit-{}", new_id()));
    let mut state = crate::state::State::open(path.clone(), None).unwrap();
    state.new_page(true, None);
    let db = state.tab().unwrap().database.clone().unwrap();
    let db = state
        .store
        .add_property(&db.id, "Place", PropertyKind::Place)
        .unwrap();
    let place = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Place)
        .unwrap()
        .id
        .clone();
    state
        .store
        .create_record(
            &db.id,
            "Test",
            &std::collections::BTreeMap::from([(
                place.clone(),
                json!({"name":"Test","latitude":2.,"longitude":5.}),
            )]),
        )
        .unwrap();
    let db = state
        .store
        .create_view(&db.id, "Map", ViewKind::Map)
        .unwrap();
    let index = db
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Map)
        .unwrap();
    state.tabs[state.active].database = Some(db);
    assert!(state.select_database_view(index));
    state.select_map_source(&place);
    let mut rt = Runtime::new(state, crate::ui::view, (1200., 800.), Env::default());
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    rt.update(|s, _| s.fit_map_pins());
    let deadline = Instant::now() + Duration::from_secs(5);
    while rt.state.database_views.map.fit_pending {
        assert!(Instant::now() < deadline);
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(2));
    }
    let fitted = rt.state.database_views.map.viewport;
    assert!((fitted.latitude - 2.).abs() < 0.0001 && (fitted.longitude - 5.).abs() < 0.0001);
    assert_eq!(fitted.zoom, 14.);
    rt.update(|s, _| s.fit_map_pins());
    assert!(rt.state.database_views.map.fit_pending);
    rt.update(|s, _| {
        s.database_views.map.viewport.longitude = 70.;
        s.map_position_changed();
    });
    assert!(!rt.state.database_views.map.fit_pending);
    for _ in 0..50 {
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(rt.state.database_views.map.viewport.longitude, 70.);
    drop(rt);
    let _ = std::fs::remove_dir_all(path);
}

fn interaction_fixture() -> (crate::state::State, std::path::PathBuf, String, String) {
    let path = std::env::temp_dir().join(format!("moss-map-input-{}", new_id()));
    let mut s = crate::state::State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.clone().unwrap();
    let db = s
        .store
        .add_property(&db.id, "Place", PropertyKind::Place)
        .unwrap();
    let place = db.properties.last().unwrap().id.clone();
    let db = s.store.create_view(&db.id, "Map", ViewKind::Map).unwrap();
    let index = db
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Map)
        .unwrap();
    let view = db.views[index].id.clone();
    let db = s
        .store
        .edit_view_config(&db.id, &view, ViewKind::Map, |c| {
            c["map_place_property"] = json!(place);
            Ok(())
        })
        .unwrap();
    let id = db.id.clone();
    s.store
        .create_record(
            &id,
            "Map pin",
            &BTreeMap::from([(
                place,
                json!({"name":"Test pin", "latitude":0., "longitude":0.}),
            )]),
        )
        .unwrap();
    s.tabs[s.active].database = Some(db);
    assert!(s.select_database_view(index));
    (s, path, id, view)
}

#[test]
fn repeated_map_saves_round_trip_coordinates_and_keep_real_conflicts() {
    let (mut s, path, db, view) = interaction_fixture();
    // This float lost one bit with the default JSON decoder and caused a false conflict.
    let longitude = 115.86000000000001;
    let value = json!(longitude);
    assert_eq!(
        value,
        serde_json::from_str::<Value>(&value.to_string()).unwrap()
    );
    s.database_views.map.viewport.longitude = longitude;
    s.database_views.map.viewport.latitude = -31.95;
    s.database_views.map.viewport.zoom = 12.125;
    s.map_position_changed();
    assert!(s.finish_map_interaction(), "{:?}", s.error);
    for _ in 0..5 {
        s.database_views.map.viewport = s.database_views.map.viewport.pan(13.25, -2.5);
        s.map_position_changed();
        assert!(s.finish_map_interaction(), "{:?}", s.error);
    }
    let saved = s.store.get_database(&db).unwrap().unwrap();
    let cached = s.tab().unwrap().database.as_ref().unwrap();
    assert_eq!(cached.views, saved.views);
    let page = cached.page_id.clone();
    let saved_camera = s.database_views.map.viewport;
    drop(s);
    let mut s = crate::state::State::open(path.clone(), None).unwrap();
    s.open_page(&page, false);
    let index = s
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .views
        .iter()
        .position(|v| v.id == view)
        .unwrap();
    assert!(s.select_database_view(index));
    let mut rt = Runtime::new(s, crate::ui::view, (1100., 800.), Env::default());
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    let loaded = rt.state.database_views.map.viewport;
    assert_eq!(
        (loaded.latitude, loaded.longitude, loaded.zoom),
        (
            saved_camera.latitude,
            saved_camera.longitude,
            saved_camera.zoom
        )
    );
    rt.state.database_views.map.viewport = loaded.pan(7., 3.);
    rt.state.map_position_changed();
    assert!(rt.state.finish_map_interaction(), "{:?}", rt.state.error);
    let mut s = rt.state;
    s.store
        .edit_view_config(&db, &view, ViewKind::Map, |c| {
            c["map_center_lon"] = json!(0.);
            c["unrelated_setting"] = json!("preserve");
            Ok(())
        })
        .unwrap();
    s.database_views.map.viewport = s.database_views.map.viewport.pan(10., 0.);
    let local = s.database_views.map.viewport;
    s.map_position_changed();
    assert!(!s.finish_map_interaction());
    assert_eq!(
        s.database_views.map.viewport, local,
        "keep the unsaved camera"
    );
    let saved = s.store.get_database(&db).unwrap().unwrap();
    let config = &saved.views.iter().find(|v| v.id == view).unwrap().config;
    assert_eq!(config["map_center_lon"], json!(0.));
    assert_eq!(config["unrelated_setting"], json!("preserve"));
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn trackpad_pans_both_axes_and_pinches_smoothly_without_page_scroll_or_query_churn() {
    use rune::{GesturePhase, MagnifyEvent, WheelEvent};
    let (s, path, _, _) = interaction_fixture();
    let mut rt = Runtime::new(s, crate::ui::view, (1100., 800.), Env::default());
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    let deadline = Instant::now() + Duration::from_secs(5);
    while rt.state.database_views.map.points.is_empty() {
        assert!(Instant::now() < deadline);
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(1));
    }
    let loader = MapTileLoader::fake_cached(rt.state.database_views.map.viewport, path.clone());
    let tile = &loader.tiles[0].position;
    let tile_key = format!("map-tile-{}-{}-{}-{}", tile.z, tile.world_x, tile.x, tile.y);
    rt.state.database_views.map.tile_loader = Some(loader);
    rt.frame(0.);
    let pin_before = rt.presented("map-marker-0").unwrap().0;
    let tile_before = rt.presented(&tile_key).unwrap().0;
    let (x, y) = rt.find("map-canvas").unwrap();
    let before = rt.state.database_views.map.viewport;
    let epoch = rt.state.database_views.map.epoch;
    let count = rt.state.store.local_change_count();
    let body = rt.presented("map-canvas").unwrap().0;
    for i in 0..20 {
        rt.wheel_event(
            WheelEvent::new(2., -1.)
                .precise(true)
                .at(x, y)
                .phase(if i == 0 {
                    GesturePhase::Began
                } else {
                    GesturePhase::Moved
                }),
        );
        rt.frame(1. / 120.);
    }
    let panned = rt.state.database_views.map.viewport;
    let expected = before.pan(40., -20.);
    assert!((panned.latitude - expected.latitude).abs() < 1e-10);
    assert!((panned.longitude - expected.longitude).abs() < 1e-10);
    assert_eq!(panned.zoom, before.zoom);
    for (key, original) in [
        ("map-marker-0", pin_before),
        (tile_key.as_str(), tile_before),
    ] {
        let moved = rt.presented(key).unwrap().0;
        assert!(
            (moved[0] - original[0] - 40.).abs() < 0.01,
            "{key}: {original:?} -> {moved:?}"
        );
        assert!(
            (moved[1] - original[1] + 20.).abs() < 0.01,
            "{key}: {original:?} -> {moved:?}"
        );
    }
    assert_eq!(
        rt.presented("map-canvas").unwrap().0,
        body,
        "page must not scroll"
    );
    assert_eq!(
        rt.state.database_views.map.epoch, epoch,
        "source query waits for idle"
    );
    assert_eq!(
        rt.state.store.local_change_count(),
        count,
        "no write for each tick"
    );
    rt.wheel_event(
        WheelEvent::default()
            .precise(true)
            .at(x, y)
            .phase(GesturePhase::Ended),
    );
    rt.frame(0.);
    rt.magnify_event(MagnifyEvent::new(0.02).at(x, y).phase(GesturePhase::Began));
    rt.frame(0.);
    let zoomed = rt.state.database_views.map.viewport;
    assert!(zoomed.zoom > panned.zoom && zoomed.zoom < panned.zoom + 0.1);
    rt.magnify_event(MagnifyEvent::new(0.).at(x, y).phase(GesturePhase::Ended));
    rt.frame(0.);
    rt.wheel_event(
        WheelEvent::new(5., -2.)
            .precise(true)
            .at(x, y)
            .phase(GesturePhase::Began),
    );
    rt.frame(0.);
    assert_eq!(rt.state.database_views.map.viewport, zoomed.pan(5., -2.));
    // A lost pinch Ended event must not lock the next wheel gesture.
    rt.magnify_event(MagnifyEvent::new(0.01).at(x, y).phase(GesturePhase::Began));
    rt.frame(0.);
    let zoomed = rt.state.database_views.map.viewport;
    rt.wheel_event(
        WheelEvent::new(3., 2.)
            .precise(true)
            .at(x, y)
            .phase(GesturePhase::Began),
    );
    rt.frame(0.);
    assert_eq!(rt.state.database_views.map.viewport, zoomed.pan(3., 2.));
    rt.frame(0.3);
    rt.frame(0.);
    assert!(rt.state.store.local_change_count() > count);
    assert!(
        rt.state.database_views.map.error.is_none(),
        "{:?}",
        rt.state.database_views.map.error
    );
    assert!(rt.state.database_views.map.epoch > epoch);
    let start = rt.state.database_views.map.viewport;
    rt.pointer_down(x + 80., y);
    rt.pointer_move(x + 125., y + 15.);
    rt.pointer_up(x + 125., y + 15.);
    rt.frame(0.);
    let moved = start.pan(45., 15.);
    assert!((rt.state.database_views.map.viewport.longitude - moved.longitude).abs() < 1e-10);
    assert!((rt.state.database_views.map.viewport.latitude - moved.latitude).abs() < 1e-10);
    rt.frame(0.3);
    assert!(
        rt.state.database_views.map.error.is_none(),
        "{:?}",
        rt.state.database_views.map.error
    );
    let saved = rt.state.database_views.map.viewport;
    assert!(rt.state.select_database_view(0));
    rt.invalidate();
    rt.frame(0.);
    let map_index = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Map)
        .unwrap();
    assert!(rt.state.select_database_view(map_index));
    rt.invalidate();
    rt.frame(0.);
    assert_eq!(rt.state.database_views.map.viewport, saved);
    rt.wheel_event(
        WheelEvent::new(4., -3.)
            .precise(true)
            .at(x, y)
            .phase(GesturePhase::Began),
    );
    rt.frame(0.);
    assert_eq!(rt.state.database_views.map.viewport, saved.pan(4., -3.));
    assert!(rt.state.finish_map_interaction());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
