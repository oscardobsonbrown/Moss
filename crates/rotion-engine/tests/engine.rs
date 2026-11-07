use chrono::NaiveDate;
use rotion_core::{
    FilterExpr, FilterOp, FilterRule, Property, PropertyKind, Record, RecordQuery, SortRule,
};
use rotion_engine::{
    formula::evaluate,
    query::{apply_query, group_records},
    rollup::{RollupOp, aggregate},
    timeline::{Dependency, ShiftMode, Task, shift_dependencies},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn record(id: &str, score: i64) -> Record {
    Record {
        id: id.into(),
        database_id: "db".into(),
        page_id: None,
        title: id.into(),
        values: BTreeMap::from([("score".into(), json!(score))]),
        created_at: String::new(),
        updated_at: String::new(),
        deleted_at: None,
    }
}
fn day(n: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, n).unwrap()
}
#[test]
fn formula_properties_arithmetic_and_errors() {
    let property = Property {
        id: "p1".into(),
        name: "Price".into(),
        kind: PropertyKind::Number,
        config: Value::Null,
    };
    let values = BTreeMap::from([("p1".into(), json!(4))]);
    assert_eq!(
        evaluate("prop(\"Price\") * 2 + 1", &values, &[property.clone()]).unwrap(),
        json!(9.0)
    );
    assert_eq!(
        evaluate("if(true, prop(\"p1\"), 1 / 0)", &values, &[property]).unwrap(),
        json!(4)
    );
    assert!(evaluate("1 +", &values, &[]).is_err());
    assert!(evaluate("unknown(1)", &values, &[]).is_err());
    assert!(evaluate("prop(\"missing\")", &values, &[]).is_err());
    assert_eq!(
        evaluate(
            "dateAdd(parseDate(\"2026-11-09\"), 7, \"days\")",
            &values,
            &[]
        )
        .unwrap(),
        json!("2026-11-16")
    );
    assert_eq!(
        evaluate("sum([1, 2, 3])", &values, &[]).unwrap(),
        json!(6.0)
    );
}
#[test]
fn rollup_aggregates_and_rejects_invalid_numbers() {
    assert_eq!(
        aggregate(RollupOp::Unique, &[json!(1), json!(1), json!(2)]).unwrap(),
        json!([1, 2])
    );
    assert_eq!(
        aggregate(RollupOp::Average, &[json!(2), json!(4)]).unwrap(),
        json!(3.0)
    );
    assert!(aggregate(RollupOp::Sum, &[json!("bad")]).is_err());
}
#[test]
fn query_filters_sorts_pages_and_groups() {
    let records = vec![record("a", 2), record("b", 5), record("c", 3)];
    let query = RecordQuery {
        filter: Some(FilterExpr::Rule(FilterRule {
            property_id: "score".into(),
            op: FilterOp::Gt,
            value: json!(2),
        })),
        sorts: vec![SortRule {
            property_id: "score".into(),
            descending: true,
        }],
        group_by: None,
        limit: 1,
        offset: 0,
    };
    assert_eq!(apply_query(&records, &query)[0].id, "b");
    assert_eq!(group_records(&records, "score").len(), 3);
}
#[test]
fn dependency_shift_chain_and_cycle() {
    let tasks = vec![
        Task {
            id: "a".into(),
            start: day(9),
            end: day(9),
        },
        Task {
            id: "b".into(),
            start: day(12),
            end: day(12),
        },
        Task {
            id: "c".into(),
            start: day(13),
            end: day(13),
        },
    ];
    let edges = vec![
        Dependency {
            before: "a".into(),
            after: "b".into(),
        },
        Dependency {
            before: "b".into(),
            after: "c".into(),
        },
    ];
    let shifted =
        shift_dependencies(&tasks, &edges, "a", day(16), ShiftMode::Overlap, true).unwrap();
    assert_eq!(shifted[1].start, day(17));
    assert_eq!(shifted[2].start, day(18));
    let mut cycle = edges.clone();
    cycle.push(Dependency {
        before: "c".into(),
        after: "a".into(),
    });
    assert!(shift_dependencies(&tasks, &cycle, "a", day(16), ShiftMode::Overlap, false).is_err());
    assert_eq!(
        shift_dependencies(&tasks, &edges, "a", day(16), ShiftMode::None, false).unwrap()[1].start,
        day(12)
    );
}
