use crate::state::State;
use moss_core::{PropertyKind, ViewKind, new_id};
use rune::{Env, Runtime};
use serde_json::json;
use std::time::{Duration, Instant};

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 60.);
}
fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .0
}
#[test]
fn map_navigation_remains_without_extra_search_and_location_rows() {
    for (width, height, scale, dark) in [(850., 600., 1., false), (1100., 800., 2., true)] {
        let path = std::env::temp_dir().join(format!("moss-map-controls-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        let db = s.tab().unwrap().database.as_ref().unwrap().clone();
        let db = s
            .store
            .add_property(&db.id, "Location", PropertyKind::Place)
            .unwrap();
        let prop = db.properties.last().unwrap().id.clone();
        s.store
            .create_record(
                &db.id,
                "Project",
                &[(
                    prop.clone(),
                    json!({"name":"Perth", "latitude":-31.95, "longitude":115.86}),
                )]
                .into(),
            )
            .unwrap();
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
                c["map_place_property"] = json!(prop);
                Ok(())
            })
            .unwrap();
        s.tabs[s.active].database = Some(db);
        s.select_database_view(index);
        let mut rt = Runtime::with_text(
            s,
            crate::ui::view,
            (width, height),
            Env {
                system_dark: dark,
                scale,
            },
            rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
        );
        rt.on_key = Some(crate::key);
        rt.poll = Some(crate::poll);
        rt.frame(0.);
        let deadline = Instant::now() + Duration::from_secs(5);
        while rt.state.database_views.map.candidates.is_empty() {
            assert!(Instant::now() < deadline);
            rt.frame(1. / 60.);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!rt.elements().iter().any(|e| {
            e.key.as_deref().is_some_and(|key| {
                key.starts_with("page-cover-open-") && key.ends_with("-database")
            })
        }));
        assert!(
            !rt.elements()
                .iter()
                .any(|e| e.label.as_deref() == Some("Add page icon"))
        );
        let canvas = rect(&rt, "map-canvas");
        let body = rect(&rt, "db-body");
        assert!(
            (canvas[2] - body[2]).abs() < 1.,
            "map must fill the database width: canvas {canvas:?}, body {body:?}"
        );
        let nav = rect(&rt, "map-navigation");
        assert!(
            nav[0] >= canvas[0]
                && nav[1] >= canvas[1]
                && nav[0] + nav[2] <= canvas[0] + canvas[2] + 0.1
                && nav[1] + nav[3] <= canvas[1] + canvas[3] + 0.1,
            "nav {nav:?} canvas {canvas:?}"
        );
        assert!((canvas[0] + canvas[2] - (nav[0] + nav[2]) - 4.).abs() < 1.);
        assert!((canvas[1] - body[1]).abs() < 1.);
        for key in ["map-place-search", "map-source", "map-location-open"] {
            assert!(rt.find(key).is_none(), "removed row control {key}");
        }
        assert!(rect(&rt, "map-zoom-out")[0] < rect(&rt, "map-zoom-in")[0]);
        assert!(rect(&rt, "map-zoom-in")[0] < rect(&rt, "map-reset")[0]);
        assert!(rt.find("map-record-search").is_none());
        assert!(rt.find("map-latitude").is_none());
        let zoom = rt.state.database_views.map.viewport.zoom;
        let writes = rt.state.store.local_change_count();
        click(&mut rt, "map-zoom-in");
        assert_eq!(rt.state.database_views.map.viewport.zoom, zoom + 1.);
        assert_eq!(
            rt.state.store.local_change_count(),
            writes,
            "button must not save on every camera event"
        );
        rt.frame(0.3);
        assert!(rt.state.store.local_change_count() > writes);
        click(&mut rt, "map-reset");
        while rt.state.database_views.map.fit_pending {
            assert!(Instant::now() < deadline);
            rt.frame(1. / 60.);
            std::thread::sleep(Duration::from_millis(1));
        }
        let fitted = rt.state.database_views.map.viewport;
        assert!(
            (fitted.latitude + 31.95).abs() < 0.001 && (fitted.longitude - 115.86).abs() < 0.001
        );
        assert_eq!(fitted.zoom, 14.);
        click(&mut rt, "page-actions");
        click(&mut rt, "database-page-cover");
        assert!(rt.state.page_cover.editor.is_some());
        rt.update(|s, _| {
            assert!(s.finish_page_cover());
        });
        rt.frame(0.);
        click(&mut rt, "page-actions");
        click(&mut rt, "database-page-icon");
        assert!(rt.state.page_icon_picker.is_some());
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
