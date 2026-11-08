use super::*;
use moss_core::{PropertyKind, new_id};
use rune::{Env, Runtime};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}
fn wait(rt: &mut Runtime<State>) {
    let until = Instant::now() + Duration::from_secs(10);
    while rt.state.database_views.chart.projection.is_none() {
        assert!(
            Instant::now() < until,
            "chart failed: {:?}",
            rt.state.database_views.chart.error
        );
        rt.frame(1. / 120.);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn chart_setup_and_drilldown_use_every_matching_record() {
    for (dark, scale) in [(false, 1.), (true, 2.)] {
        let path = std::env::temp_dir().join(format!("moss-chart-{}", new_id()));
        let mut s = State::open(path.clone(), None).unwrap();
        s.new_page(true, None);
        let db = s.tab().unwrap().database.clone().unwrap();
        let status = db
            .properties
            .iter()
            .find(|p| p.kind == PropertyKind::Status)
            .unwrap()
            .id
            .clone();
        for i in 0..120 {
            s.store
                .create_record(
                    &db.id,
                    &format!("Task {i}"),
                    &BTreeMap::from([(
                        status.clone(),
                        json!(if i % 2 == 0 { "Not started" } else { "Done" }),
                    )]),
                )
                .unwrap();
        }
        let db = s
            .store
            .create_view(&db.id, "Chart", ViewKind::Chart)
            .unwrap();
        let index = db.views.iter().position(|v| v.name == "Chart").unwrap();
        s.tabs[s.active].database = Some(db.clone());
        assert!(s.select_database_view(index));
        let mut rt = Runtime::new(
            s,
            crate::ui::view,
            (850., 600.),
            Env {
                system_dark: dark,
                scale,
            },
        );
        rt.on_key = Some(crate::key);
        rt.poll = Some(crate::poll);
        rt.frame(0.);
        click(&mut rt, &format!("chart-axis-{status}"));
        wait(&mut rt);
        let data = rt.state.database_views.chart.projection.as_ref().unwrap();
        assert_eq!(data.total, 120);
        assert_eq!(data.points.len(), 2);
        assert_eq!(data.points.iter().map(|p| p.count).sum::<u64>(), 120);
        let (x, y) = rt.find("chart-point-0").unwrap();
        rt.pointer_move(x, y);
        rt.frame(0.);
        assert!(rt.find("chart-hover-label").is_some());
        click(&mut rt, "chart-point-0");
        assert_eq!(rt.state.database_views.chart.drill_records.len(), 40);
        let first = rt.state.database_views.chart.drill_records[0].id.clone();
        rt.update(|_, tx| tx.scroll_into_view("chart-records-next"));
        rt.frame(0.);
        click(&mut rt, "chart-records-next");
        let until = Instant::now() + Duration::from_secs(10);
        while rt.state.database_views.chart.drill_loading {
            assert!(Instant::now() < until);
            rt.frame(1. / 120.);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(rt.state.database_views.chart.drill_records.len(), 20);
        assert!(
            !rt.state
                .database_views
                .chart
                .drill_records
                .iter()
                .any(|r| r.id == first)
        );
        click(&mut rt, "chart-records-previous");
        while rt.state.database_views.chart.drill_loading {
            assert!(Instant::now() < until);
            rt.frame(1. / 120.);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(rt.state.database_views.chart.drill_records[0].id, first);
        let id = rt.state.database_views.chart.drill_records[0].id.clone();
        rt.update(|_, tx| tx.scroll_into_view(format!("chart-record-{id}")));
        rt.frame(0.);
        click(&mut rt, &format!("chart-record-{id}"));
        assert_eq!(rt.state.record_peek.as_ref().unwrap().record.id, id);
        let saved = rt.state.store.get_database(&db.id).unwrap().unwrap();
        assert_eq!(
            saved.views[index].config["chart"]["category_property_id"],
            status
        );
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn stale_chart_config_is_rejected_without_overwriting_other_window() {
    let path = std::env::temp_dir().join(format!("moss-chart-stale-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    let db = s.tab().unwrap().database.clone().unwrap();
    let db = s
        .store
        .create_view(&db.id, "Chart", ViewKind::Chart)
        .unwrap();
    let index = db.views.iter().position(|v| v.name == "Chart").unwrap();
    s.tabs[s.active].database = Some(db.clone());
    assert!(s.select_database_view(index));
    let id = db.views[index].id.clone();
    let mut other = Store::open(path.join("workspace.sqlite3")).unwrap();
    other
        .edit_view_config(&db.id, &id, ViewKind::Chart, |v| {
            v["chart"] = json!({"kind":"number"});
            Ok(())
        })
        .unwrap();
    s.chart_kind(ChartType::Line);
    assert!(s.database_views.chart.save_error.is_some());
    assert_eq!(
        s.store.get_database(&db.id).unwrap().unwrap().views[index].config["chart"]["kind"],
        "number"
    );
    drop(other);
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}
