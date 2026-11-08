use super::*;
use rune::{Cx, Env, Runtime};
fn surface(s: &State, _: &mut Cx<State>) -> El {
    let tab = s.tab().unwrap();
    col().w_full().h_full().child(view(
        s,
        tab,
        tab.database.as_ref().unwrap(),
        Theme::new(false),
    ))
}
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("Missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 60.);
}
#[test]
fn widget_record_open_uses_source_identity() {
    let path = std::env::temp_dir().join(format!("moss-dashboard-ui-{}", moss_core::new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.clone().unwrap();
    let record = s
        .store
        .create_record(&db.id, "Widget record", &Default::default())
        .unwrap();
    let db = s
        .store
        .create_view(&db.id, "Dashboard", ViewKind::Dashboard)
        .unwrap();
    let index = db
        .views
        .iter()
        .position(|v| v.kind == ViewKind::Dashboard)
        .unwrap();
    let source = db.views[0].clone();
    s.tabs[s.active].database = Some(db.clone());
    s.tabs[s.active].view = index;
    s.dashboard_add_widget(db.id.clone(), source.id.clone());
    let config =
        DashboardConfig::read(&s.tab().unwrap().database.as_ref().unwrap().views[index].config)
            .unwrap();
    let id = config.widgets[0].id.clone();
    s.database_views.dashboard.widgets.insert(
        id.clone(),
        Ok(WidgetData {
            database: db,
            view: source,
            records: vec![record.clone()],
            has_more: false,
            chart: None,
            map: None,
            map_base: None,
        }),
    );
    let mut rt = Runtime::with_text(
        s,
        surface,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.frame(0.);
    click(&mut rt, "dashboard-edit");
    assert!(
        rt.state.database_views.dashboard.editing,
        "edit did not open"
    );
    let pos = rt.find("dashboard-add");
    click(&mut rt, "dashboard-add");
    assert!(
        rt.state.database_views.dashboard.adding,
        "add did not open: add={:?} edit={:?} find={pos:?}",
        rt.presented("dashboard-add"),
        rt.presented("dashboard-edit")
    );
    click(&mut rt, "dashboard-source-search");
    rt.key(rune::KeyEvent {
        key: rune::Key::Text("project".into()),
        mods: rune::Mods::default(),
    });
    rt.frame(1. / 60.);
    assert_eq!(
        rt.state.database_views.dashboard.source_input.text,
        "project"
    );
    click(&mut rt, "dashboard-source-search-submit");
    assert_eq!(rt.state.database_views.dashboard.source_search, "project");
    click(&mut rt, "dashboard-add");
    click(&mut rt, "dashboard-edit");
    click(&mut rt, &format!("dashboard-record-{id}-{}", record.id));
    assert_eq!(rt.state.record_peek.as_ref().unwrap().record.id, record.id);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn map_widget_scroll_clips_content_and_keeps_markers_reachable() {
    for height in [120, 300] {
        for editing in [false, true] {
            let path =
                std::env::temp_dir().join(format!("moss-dashboard-map-{}", moss_core::new_id()));
            let mut s = State::open(path.clone(), None).unwrap();
            s.new_page(true, None);
            let db = s.tab().unwrap().database.clone().unwrap();
            let db = s.store.create_view(&db.id, "Map", ViewKind::Map).unwrap();
            let db = s
                .store
                .create_view(&db.id, "Dashboard", ViewKind::Dashboard)
                .unwrap();
            let dashboard = db
                .views
                .iter()
                .position(|v| v.kind == ViewKind::Dashboard)
                .unwrap();
            let source = db
                .views
                .iter()
                .find(|v| v.kind == ViewKind::Map)
                .unwrap()
                .clone();
            s.tabs[s.active].database = Some(db.clone());
            s.tabs[s.active].view = dashboard;
            s.dashboard_add_widget(db.id.clone(), source.id.clone());
            let id = s.tab().unwrap().database.as_ref().unwrap().views[dashboard].config["dashboard"]["widgets"][0]["id"].as_str().unwrap().to_owned();
            s.dashboard_resize_widget(&id, height - 300);
            s.database_views.dashboard.editing = editing;
            let base = crate::map_view::MapViewport::default();
            let point = moss_core::MapPoint {
                record_id: "fixture-record".into(),
                title: "Fixture".into(),
                place: moss_core::PlaceValue {
                    name: "Origin".into(),
                    latitude: 0.,
                    longitude: 0.,
                },
            };
            s.database_views.dashboard.widgets.insert(
                id.clone(),
                Ok(WidgetData {
                    database: db,
                    view: source,
                    records: Vec::new(),
                    has_more: false,
                    chart: None,
                    map: Some(moss_core::MapPoints {
                        points: vec![point],
                        truncated: false,
                    }),
                    map_base: Some(base),
                }),
            );
            let error = "A long tile failure message ".repeat(20);
            let mut loader = crate::map_view::MapTileLoader::new(std::sync::Arc::new(|| {}));
            loader.error = Some(error.clone());
            s.database_views
                .dashboard
                .map_tiles
                .insert(id.clone(), loader);
            let mut rt = Runtime::with_text(
                s,
                surface,
                (1440., 900.),
                Env {
                    system_dark: false,
                    scale: 1.,
                },
                rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
            );
            rt.frame(0.);
            let card_key = format!("dashboard-widget-{id}");
            let canvas_key = format!("dashboard-map-{id}-canvas");
            let error_key = format!("dashboard-map-error-{id}");
            let lookup = |rt: &Runtime<State>, key: &str| {
                rt.elements()
                    .into_iter()
                    .find(|e| e.key.as_deref() == Some(key))
                    .unwrap()
            };
            let card = lookup(&rt, &card_key);
            let canvas = lookup(&rt, &canvas_key);
            let message = lookup(&rt, &error_key);
            assert_eq!(card.rect[3], f32::from(height));
            assert!((canvas.rect[3] - (f32::from(height) - 80.).max(40.)).abs() < 0.1);
            assert!(message.rect[0] + message.rect[2] <= card.rect[0] + card.rect[2] + 0.1);
            assert_eq!(message.help.as_deref(), Some(error.as_str()));
            assert!(
                canvas.visible[3] <= 0.
                    || (canvas.visible[1] >= card.visible[1] - 0.1
                        && canvas.visible[1] + canvas.visible[3]
                            <= card.visible[1] + card.visible[3] + 0.1),
                "height={height} editing={editing} card={:?} canvas={:?}",
                card.visible,
                canvas.visible
            );
            rt.update(|_, tx| tx.scroll_to(&card_key, 0., 10_000.));
            for _ in 0..30 {
                rt.frame(1. / 60.);
            }
            let card = lookup(&rt, &card_key);
            let canvas = lookup(&rt, &canvas_key);
            assert!(
                canvas.visible[3] > 0.
                    && canvas.visible[1] >= card.visible[1] - 0.1
                    && canvas.visible[1] + canvas.visible[3]
                        <= card.visible[1] + card.visible[3] + 0.1
            );
            let marker = lookup(&rt, &format!("dashboard-map-{id}-marker-0"));
            assert!(
                marker.visible[2] > 0. && marker.visible[3] > 0.,
                "marker should be reachable after scrolling: {marker:?}"
            );
            if height == 300 && !editing {
                let before_x = marker.rect[0];
                let x = canvas.visible[0] + 12.;
                let y = canvas.visible[1] + 8.;
                rt.pointer_down(x, y);
                rt.pointer_move(x + 30., y + 8.);
                rt.frame(1. / 60.);
                let drag = rt
                    .state
                    .database_views
                    .dashboard
                    .map_drag
                    .as_ref()
                    .expect("drag preview");
                assert!((drag.preview.longitude - drag.start.longitude).abs() > 0.001);
                let moved = lookup(&rt, &format!("dashboard-map-{id}-marker-0"));
                assert!(
                    (moved.rect[0] - before_x - 30.).abs() < 1.0,
                    "marker before={before_x} after={:?}",
                    moved.rect
                );
                rt.key(rune::KeyEvent {
                    key: rune::Key::Escape,
                    mods: rune::Mods::default(),
                });
                rt.frame(1. / 60.);
                assert!(rt.state.database_views.dashboard.map_drag.is_none());
                assert!(
                    !rt.state
                        .database_views
                        .dashboard
                        .map_viewports
                        .contains_key(&id)
                );
            }
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}
