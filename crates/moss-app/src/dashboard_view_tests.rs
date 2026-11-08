use super::*;
use moss_core::{
    DatabaseView, FilterOp, Property, PropertyKind, RecordQuery,
    dashboard::{DashboardFilter, DashboardWidget},
    new_id,
};
use serde_json::json;
fn source(store: &mut Store, workspace: &str, name: &str) -> Database {
    let db = Database {
        id: new_id(),
        page_id: String::new(),
        name: name.into(),
        properties: vec![
            Property {
                id: new_id(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            },
            Property {
                id: new_id(),
                name: "Tag".into(),
                kind: PropertyKind::Text,
                config: json!({}),
            },
        ],
        views: vec![DatabaseView {
            id: new_id(),
            name: "List".into(),
            kind: ViewKind::List,
            query: RecordQuery::default(),
            config: json!({}),
        }],
    };
    let page = store
        .create_database_page(workspace, None, name, "", db)
        .unwrap();
    store.get_database_by_page(&page.id).unwrap().unwrap()
}
fn widget(db: &Database) -> DashboardWidget {
    DashboardWidget {
        id: new_id(),
        database_id: db.id.clone(),
        view_id: db.views[0].id.clone(),
        width: 2,
        height: 300,
        query: None,
        layout: serde_json::Value::Null,
        extra: Default::default(),
    }
}
#[test]
fn widget_queries_are_independent_and_global_filter_does_not_change_sources() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Test", "").unwrap();
    let first = source(&mut store, &workspace.id, "First");
    let second = source(&mut store, &workspace.id, "Second");
    for db in [&first, &second] {
        store
            .create_record(
                &db.id,
                "Keep",
                &BTreeMap::from([(db.properties[1].id.clone(), json!("yes"))]),
            )
            .unwrap();
        store
            .create_record(
                &db.id,
                "Skip",
                &BTreeMap::from([(db.properties[1].id.clone(), json!("no"))]),
            )
            .unwrap();
    }
    let a = widget(&first);
    let b = widget(&second);
    let config = DashboardConfig {
        widgets: vec![a.clone(), b.clone()],
        global_filters: vec![DashboardFilter {
            property_name: "Tag".into(),
            property_kind: PropertyKind::Text,
            op: FilterOp::Eq,
            value: json!("yes"),
        }],
        ..Default::default()
    };
    let (_, data) = load(
        &store,
        &workspace.id,
        "",
        &config,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &|| false,
    );
    assert_eq!(data[&a.id].as_ref().unwrap().records[0].title, "Keep");
    assert_eq!(data[&b.id].as_ref().unwrap().records[0].title, "Keep");
    assert_eq!(data[&a.id].as_ref().unwrap().records.len(), 1);
    let source_filter = FilterExpr::Rule(moss_core::FilterRule {
        property_id: first.properties[1].id.clone(),
        op: FilterOp::Eq,
        value: json!("no"),
    });
    store
        .set_view_filter(
            &first.id,
            &first.views[0].id,
            None,
            Some(source_filter.clone()),
        )
        .unwrap();
    let mut with_extra = config.clone();
    with_extra.widgets[0].query = Some(RecordQuery {
        filter: Some(FilterExpr::Rule(moss_core::FilterRule {
            property_id: first.properties[0].id.clone(),
            op: FilterOp::Eq,
            value: json!("Keep"),
        })),
        ..Default::default()
    });
    let (_, after) = load(
        &store,
        &workspace.id,
        "",
        &with_extra,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &|| false,
    );
    assert!(after[&a.id].as_ref().unwrap().records.is_empty());
    assert_eq!(after[&b.id].as_ref().unwrap().records.len(), 1);
    assert_eq!(
        store.get_database(&first.id).unwrap().unwrap().views[0]
            .query
            .filter,
        Some(source_filter)
    );
    assert!(
        with_extra.widgets[0]
            .query
            .as_ref()
            .unwrap()
            .filter
            .is_some()
    );
    assert_eq!(
        store.get_database(&second.id).unwrap().unwrap().views[0]
            .query
            .filter,
        None
    );
}
#[test]
fn widget_lifecycle_persists_and_keeps_stable_ids() {
    let path = std::env::temp_dir().join(format!("moss-dashboard-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.new_page(true, None);
    let db = state.tab().unwrap().database.clone().unwrap();
    let updated = state
        .store
        .create_view(&db.id, "Dashboard", ViewKind::Dashboard)
        .unwrap();
    let dashboard = updated
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Dashboard)
        .unwrap();
    let db_id = updated.id.clone();
    let view_id = updated.views[0].id.clone();
    state.tabs[state.active].database = Some(updated);
    state.tabs[state.active].view = dashboard;
    state.dashboard_add_widget(db_id.clone(), view_id);
    let original = state.dashboard_target().unwrap().2.widgets[0].id.clone();
    state.dashboard_duplicate_widget(&original);
    let widgets = state.dashboard_target().unwrap().2.widgets;
    assert_eq!(widgets.len(), 2);
    assert_ne!(widgets[0].id, widgets[1].id);
    let second = widgets[1].id.clone();
    state.dashboard_reorder_widget(&second, &original);
    state.dashboard_resize_widget(&second, 60);
    state.dashboard_resize_width(&second, 1);
    let (_, loaded) = load(
        &state.store,
        &state.workspace.id,
        "",
        &state.dashboard_target().unwrap().2,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &|| false,
    );
    state.database_views.dashboard.widgets = loaded;
    let name = state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Title)
        .unwrap()
        .name
        .clone();
    state.database_views.dashboard.filter_property = Some((name, PropertyKind::Title));
    state.database_views.dashboard.filter_widget = Some(second.clone());
    state
        .database_views
        .dashboard
        .filter_input
        .replace_text("Keep");
    state.dashboard_apply_filter();
    assert!(
        state.dashboard_target().unwrap().2.widgets[0]
            .query
            .as_ref()
            .unwrap()
            .filter
            .is_some()
    );
    state.dashboard_reset_widget_query(&second);
    assert!(
        state.dashboard_target().unwrap().2.widgets[0]
            .query
            .is_none()
    );
    let config = state.dashboard_target().unwrap().2;
    assert_eq!(config.widgets[0].id, second);
    assert_eq!(config.widgets[0].height, 360);
    assert_eq!(config.widgets[0].width, 3);
    state.dashboard_remove_widget(&original);
    let saved_id = state.tab().unwrap().database.as_ref().unwrap().views[dashboard]
        .id
        .clone();
    drop(state);
    let reopened = Store::open(path.join("workspace.sqlite3")).unwrap();
    let saved = reopened.get_database(&db_id).unwrap().unwrap();
    let config = DashboardConfig::read(
        &saved
            .views
            .iter()
            .find(|v| v.id == saved_id)
            .unwrap()
            .config,
    )
    .unwrap();
    assert_eq!(config.widgets.len(), 1);
    assert_eq!(config.widgets[0].id, second);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn leaving_and_returning_discards_cancelled_worker_and_reloads() {
    let path = std::env::temp_dir().join(format!("moss-dashboard-switch-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.new_page(true, None);
    let db = state.tab().unwrap().database.clone().unwrap();
    let updated = state
        .store
        .create_view(&db.id, "Dashboard", ViewKind::Dashboard)
        .unwrap();
    let dashboard = updated
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Dashboard)
        .unwrap();
    let view_id = updated.views[dashboard].id.clone();
    let db_id = updated.id.clone();
    state.tabs[state.active].database = Some(updated);
    state.tabs[state.active].view = dashboard;
    let config = state.dashboard_target().unwrap().2;
    let key = format!(
        "{}:{}:{}:{}:{:?}:{}",
        db_id,
        view_id,
        state.store.local_change_count(),
        serde_json::to_string(&config).unwrap(),
        state.database_views.dashboard.map_viewports,
        state.database_views.dashboard.source_search
    );
    let (sender, receiver) = mpsc::channel();
    {
        let d = &mut state.database_views.dashboard;
        d.context = format!("{db_id}:{view_id}");
        d.key = key.clone();
        d.requested = key.clone();
        d.receiver = Some(receiver);
    }
    let mut tx = Tx::default();
    state.tabs[state.active].view = 0;
    poll(&mut state, 0., &mut tx);
    assert!(state.database_views.dashboard.cancelled);
    assert!(state.database_views.dashboard.key.is_empty());
    sender
        .send(Response {
            key,
            candidates: Ok(Vec::new()),
            widgets: BTreeMap::from([("stale".into(), Err("stale".into()))]),
        })
        .unwrap();
    state.tabs[state.active].view = dashboard;
    poll(&mut state, 0., &mut tx);
    assert!(!state.database_views.dashboard.widgets.contains_key("stale"));
    assert!(state.database_views.dashboard.receiver.is_some());
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn changing_dashboard_cancels_pending_chart_request() {
    let path = std::env::temp_dir().join(format!("moss-dashboard-cancel-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.new_page(true, None);
    let db = state.tab().unwrap().database.clone().unwrap();
    let db = state
        .store
        .create_view(&db.id, "First", ViewKind::Dashboard)
        .unwrap();
    let db = state
        .store
        .create_view(&db.id, "Second", ViewKind::Dashboard)
        .unwrap();
    let first = db.views.len() - 2;
    let second = db.views.len() - 1;
    let first_context = format!("{}:{}", db.id, db.views[first].id);
    state.tabs[state.active].database = Some(db);
    state.tabs[state.active].view = second;
    let (sender, receiver) = mpsc::channel();
    {
        let d = &mut state.database_views.dashboard;
        d.context = first_context;
        d.requested = "old-chart".into();
        d.receiver = Some(receiver);
    }
    let mut tx = Tx::default();
    poll(&mut state, 0., &mut tx);
    assert!(state.database_views.dashboard.cancelled);
    assert!(
        state
            .database_views
            .dashboard
            .generation
            .load(Ordering::Relaxed)
            > 0
    );
    sender
        .send(Response {
            key: "old-chart".into(),
            candidates: Ok(Vec::new()),
            widgets: BTreeMap::new(),
        })
        .unwrap();
    poll(&mut state, 0., &mut tx);
    assert_ne!(state.database_views.dashboard.key, "old-chart");
    assert!(state.database_views.dashboard.receiver.is_some());
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn map_widget_uses_saved_source_viewport_and_resets_after_source_change() {
    let mut store = Store::in_memory().unwrap();
    let workspace = store.create_workspace("Map", "").unwrap();
    let mut db = source(&mut store, &workspace.id, "Locations");
    let place_id = new_id();
    db.properties.push(Property {
        id: place_id.clone(),
        name: "Place".into(),
        kind: PropertyKind::Place,
        config: json!({}),
    });
    store.save_database(&db).unwrap();
    let db = store.create_view(&db.id, "Map", ViewKind::Map).unwrap();
    let map_id = db
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Map)
        .unwrap()
        .id
        .clone();
    let db = store
        .edit_view_config(&db.id, &map_id, ViewKind::Map, |config| {
            config["map_place_property"] = json!(place_id);
            config["map_center_lat"] = json!(-31.95);
            config["map_center_lon"] = json!(115.86);
            config["map_zoom"] = json!(10);
            Ok(())
        })
        .unwrap();
    let perth = store
        .create_record(&db.id, "Perth", &Default::default())
        .unwrap();
    store
        .set_record_place_if_unchanged(
            &perth.id,
            &place_id,
            &serde_json::Value::Null,
            Some(moss_core::PlaceValue {
                name: "Perth".into(),
                latitude: -31.95,
                longitude: 115.86,
            }),
        )
        .unwrap();
    let london = store
        .create_record(&db.id, "London", &Default::default())
        .unwrap();
    store
        .set_record_place_if_unchanged(
            &london.id,
            &place_id,
            &serde_json::Value::Null,
            Some(moss_core::PlaceValue {
                name: "London".into(),
                latitude: 51.50,
                longitude: -0.12,
            }),
        )
        .unwrap();
    let mut map_widget = widget(&db);
    map_widget.view_id = map_id.clone();
    let config = DashboardConfig {
        widgets: vec![map_widget.clone()],
        ..Default::default()
    };
    let (_, data) = load(
        &store,
        &workspace.id,
        "",
        &config,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &|| false,
    );
    let first = data[&map_widget.id].as_ref().unwrap();
    let base = first.map_base.unwrap();
    assert_eq!(
        (base.latitude, base.longitude, base.zoom),
        (-31.95, 115.86, 10.)
    );
    assert_eq!(first.map.as_ref().unwrap().points[0].record_id, perth.id);
    let pan = base.pan(30., 0.);
    let changed = store
        .edit_view_config(&db.id, &map_id, ViewKind::Map, |config| {
            config["map_center_lat"] = json!(51.50);
            config["map_center_lon"] = json!(-0.12);
            Ok(())
        })
        .unwrap();
    let (_, data) = load(
        &store,
        &workspace.id,
        "",
        &config,
        &BTreeMap::from([(map_widget.id.clone(), pan)]),
        &BTreeMap::from([(map_widget.id.clone(), base)]),
        &|| false,
    );
    let fresh = data[&map_widget.id].as_ref().unwrap();
    let new_base = fresh.map_base.unwrap();
    assert_eq!(
        (new_base.latitude, new_base.longitude, new_base.zoom),
        (51.50, -0.12, 10.)
    );
    assert_eq!(fresh.map.as_ref().unwrap().points[0].record_id, london.id);
    assert_eq!(
        changed
            .views
            .iter()
            .find(|v| v.id == map_id)
            .unwrap()
            .config["map_center_lon"],
        json!(-0.12)
    );
}

#[test]
fn map_drag_previews_without_requery_and_cancel_restores_viewport() {
    let path = std::env::temp_dir().join(format!("moss-dashboard-map-drag-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.new_page(true, None);
    let db = state.tab().unwrap().database.clone().unwrap();
    let db = state
        .store
        .create_view(&db.id, "Map", ViewKind::Map)
        .unwrap();
    let db = state
        .store
        .create_view(&db.id, "Dashboard", ViewKind::Dashboard)
        .unwrap();
    let dashboard = db
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Dashboard)
        .unwrap();
    let map = db
        .views
        .iter()
        .find(|v| v.kind == ViewKind::Map)
        .unwrap()
        .clone();
    state.tabs[state.active].database = Some(db.clone());
    state.tabs[state.active].view = dashboard;
    state.dashboard_add_widget(db.id.clone(), map.id.clone());
    let (_, view_id, config) = state.dashboard_target().unwrap();
    let id = config.widgets[0].id.clone();
    let base = crate::map_view::MapViewport {
        latitude: -31.95,
        longitude: 115.86,
        zoom: 10.,
        ..Default::default()
    };
    let d = &mut state.database_views.dashboard;
    d.context = format!("{}:{view_id}", db.id);
    d.map_bases.insert(id.clone(), base);
    d.widgets.insert(
        id.clone(),
        Ok(WidgetData {
            database: db.clone(),
            view: map,
            records: Vec::new(),
            has_more: false,
            chart: None,
            map: Some(moss_core::MapPoints {
                points: Vec::new(),
                truncated: false,
            }),
            map_base: Some(base),
        }),
    );
    let key = format!(
        "{}:{}:{}:{}:{:?}:{}",
        db.id,
        view_id,
        state.store.local_change_count(),
        serde_json::to_string(&config).unwrap(),
        state.database_views.dashboard.map_viewports,
        state.database_views.dashboard.source_search
    );
    state.database_views.dashboard.key = key.clone();
    state.database_views.dashboard.requested = key.clone();
    state.database_views.dashboard.data_signature = format!(
        "{}:{}:{}:{}:{}",
        db.id,
        view_id,
        state.store.local_change_count(),
        serde_json::to_string(&config).unwrap(),
        state.database_views.dashboard.source_search
    );
    let tile_viewport = crate::map_view::MapViewport {
        width: 320.,
        height: 220.,
        ..base
    };
    let loader = crate::map_view::MapTileLoader::fake_cached(tile_viewport, path.clone());
    let tile = loader.tiles[0].clone();
    state
        .database_views
        .dashboard
        .map_tiles
        .insert(id.clone(), loader);
    state
        .database_views
        .dashboard
        .tile_keys
        .insert(id.clone(), format!("{:?}", tile_viewport));
    state
        .database_views
        .dashboard
        .set_waker(std::sync::Arc::new(|| {}));
    state.dashboard_map_drag_start(&id);
    state.dashboard_map_drag_move(&id, 30., 10.);
    let first = state
        .database_views
        .dashboard
        .map_drag
        .as_ref()
        .unwrap()
        .preview;
    assert_eq!(first, base.pan(30., 10.));
    state.dashboard_map_drag_move(&id, 45., 12.);
    assert_eq!(
        state
            .database_views
            .dashboard
            .map_drag
            .as_ref()
            .unwrap()
            .preview,
        base.pan(45., 12.)
    );
    let mut tx = Tx::default();
    poll(&mut state, 0., &mut tx);
    assert_eq!(state.database_views.dashboard.requested, key);
    assert!(state.database_views.dashboard.receiver.is_none());
    let moved_tile = state.database_views.dashboard.map_tiles[&id]
        .tiles
        .iter()
        .find(|candidate| candidate.image.id() == tile.image.id())
        .expect("cached tile remains visible");
    assert!((moved_tile.position.left - tile.position.left - 45.).abs() < 0.1);
    assert!((moved_tile.position.top - tile.position.top - 12.).abs() < 0.1);
    state.dashboard_map_drag_end(&id, 45., 12., true);
    assert!(state.database_views.dashboard.map_drag.is_none());
    assert!(
        !state
            .database_views
            .dashboard
            .map_viewports
            .contains_key(&id)
    );
    state.dashboard_map_drag_start(&id);
    state.dashboard_map_drag_move(&id, 45., 12.);
    state.dashboard_map_drag_end(&id, 45., 12., false);
    assert_eq!(
        state.database_views.dashboard.map_viewports[&id],
        base.pan(45., 12.)
    );
    poll(&mut state, 0., &mut tx);
    assert!(state.database_views.dashboard.receiver.is_some());
    assert!(
        state.database_views.dashboard.widgets.contains_key(&id),
        "cached map stays visible while the released viewport loads"
    );
    drop(state);
    std::fs::remove_dir_all(path).unwrap();
}
