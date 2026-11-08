use super::*;
use crate::{Database, DatabaseView, RecordQuery, new_id};
use std::collections::BTreeMap;

fn fixture() -> (Store, String) {
    let mut s = Store::in_memory().unwrap();
    let workspace = s.create_workspace("Chart", "").unwrap();
    let db = Database {
        id: new_id(),
        page_id: String::new(),
        name: "Tasks".into(),
        properties: vec![
            Property {
                id: "name".into(),
                name: "Name".into(),
                kind: PropertyKind::Title,
                config: json!({}),
            },
            Property {
                id: "status".into(),
                name: "Status".into(),
                kind: PropertyKind::Select,
                config: json!({}),
            },
            Property {
                id: "date".into(),
                name: "Date".into(),
                kind: PropertyKind::Date,
                config: json!({}),
            },
            Property {
                id: "amount".into(),
                name: "Amount".into(),
                kind: PropertyKind::Number,
                config: json!({}),
            },
        ],
        views: vec![DatabaseView {
            id: "chart".into(),
            name: "Chart".into(),
            kind: ViewKind::Chart,
            query: RecordQuery::default(),
            config: json!({}),
        }],
    };
    s.create_database_page(&workspace.id, None, "Tasks", "", db.clone())
        .unwrap();
    (s, db.id)
}
#[test]
fn chart_streams_full_filtered_query_and_buckets_dates() {
    let (mut s, id) = fixture();
    for i in 0..150 {
        let values = BTreeMap::from([
            (
                "status".into(),
                json!(if i % 2 == 0 { "Open" } else { "Done" }),
            ),
            (
                "date".into(),
                json!(if i < 100 { "2026-01-12" } else { "2026-02-03" }),
            ),
            ("amount".into(), if i == 0 { Value::Null } else { json!(i) }),
        ]);
        s.create_record(&id, "Task", &values).unwrap();
    }
    let config = ChartConfig {
        category_property_id: Some("date".into()),
        series_property_id: Some("status".into()),
        value_property_id: Some("amount".into()),
        aggregation: Aggregate::Sum,
        ..Default::default()
    };
    let projection = s.chart_projection(&id, "chart", &config, None, "").unwrap();
    assert_eq!(projection.total, 150);
    assert_eq!(projection.points.len(), 4);
    let january = projection
        .points
        .iter()
        .find(|p| p.category == "2026-01" && p.series.as_deref() == Some("Open"))
        .unwrap();
    assert_eq!(january.count, 50);
    assert_eq!(january.value, Some(2450.));
    assert_eq!(january.records.len(), 40);
    let first = s
        .chart_records(
            &id,
            "chart",
            &config,
            None,
            "",
            january.category_key.as_deref(),
            january.series_key.as_deref(),
            None,
        )
        .unwrap();
    assert_eq!(first.records, january.records);
    let second = s
        .chart_records(
            &id,
            "chart",
            &config,
            None,
            "",
            january.category_key.as_deref(),
            january.series_key.as_deref(),
            first.next.as_deref(),
        )
        .unwrap();
    assert_eq!(second.records.len(), 10);
    assert!(second.next.is_none());
    assert!(
        second
            .records
            .iter()
            .all(|record| !first.records.iter().any(|first| first.id == record.id))
    );
    let filter = FilterExpr::Rule(FilterRule {
        property_id: "status".into(),
        op: FilterOp::Eq,
        value: json!("Done"),
    });
    let filtered = s
        .chart_projection(&id, "chart", &config, Some(&filter), "")
        .unwrap();
    assert_eq!(filtered.total, 75);
    assert!(
        filtered
            .points
            .iter()
            .all(|p| p.series.as_deref() == Some("Done"))
    );
    let invalid = ChartConfig {
        category_property_id: Some("deleted".into()),
        ..config
    };
    assert!(
        s.chart_projection(&id, "chart", &invalid, None, "")
            .is_err()
    );
}
#[test]
fn chart_count_and_numeric_nulls_keep_zero_distinct() {
    let (mut s, id) = fixture();
    for n in [Value::Null, json!(0), json!(10)] {
        s.create_record(
            &id,
            "Task",
            &BTreeMap::from([("amount".into(), n), ("status".into(), json!("Open"))]),
        )
        .unwrap();
    }
    let mut config = ChartConfig {
        category_property_id: Some("status".into()),
        value_property_id: Some("amount".into()),
        aggregation: Aggregate::Average,
        ..Default::default()
    };
    assert_eq!(
        s.chart_projection(&id, "chart", &config, None, "")
            .unwrap()
            .points[0]
            .value,
        Some(5.)
    );
    config.aggregation = Aggregate::Count;
    assert_eq!(
        s.chart_projection(&id, "chart", &config, None, "")
            .unwrap()
            .points[0]
            .value,
        Some(3.)
    );
}
