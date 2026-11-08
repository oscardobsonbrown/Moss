use chrono::NaiveDate;
use moss_core::{
    FilterExpr, FilterOp, FilterRule, Property, PropertyKind, Record, RecordQuery, SortRule,
};
use moss_engine::{
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

#[test]
fn boolean_calls_short_circuit_and_missing_properties_are_null() {
    let property = Property {
        id: "p".into(),
        name: "Optional".into(),
        kind: PropertyKind::Number,
        config: Value::Null,
    };
    let values = BTreeMap::new();
    assert_eq!(
        evaluate("empty(prop(\"Optional\"))", &values, &[property.clone()]).unwrap(),
        json!(true)
    );
    assert_eq!(
        evaluate("and(false, 1 / 0 > 0)", &values, &[]).unwrap(),
        json!(false)
    );
    assert_eq!(
        evaluate("or(true, 1 / 0 > 0)", &values, &[]).unwrap(),
        json!(true)
    );
    assert_eq!(
        evaluate("ifs(false, 1 / 0, true, 7, 0)", &values, &[]).unwrap(),
        json!(7.0)
    );
    assert!(evaluate("and()", &values, &[]).is_err());
    assert!(evaluate("or(false, 1 / 0 > 0)", &values, &[]).is_err());
    let duplicate = Property {
        id: "other".into(),
        ..property.clone()
    };
    assert!(evaluate("prop(\"Optional\")", &values, &[property, duplicate]).is_err());
}

#[test]
fn query_compares_numeric_representations_and_sorts_missing_last() {
    let mut missing = record("missing", 0);
    missing.values.clear();
    let rows = vec![record("low", 2), missing, record("high", 5)];
    let filter = FilterExpr::Rule(FilterRule {
        property_id: "score".into(),
        op: FilterOp::Eq,
        value: json!(2.0),
    });
    assert_eq!(
        apply_query(
            &rows,
            &RecordQuery {
                filter: Some(filter),
                ..RecordQuery::default()
            }
        )[0]
        .id,
        "low"
    );
    for descending in [false, true] {
        let query = RecordQuery {
            sorts: vec![SortRule {
                property_id: "score".into(),
                descending,
            }],
            ..RecordQuery::default()
        };
        assert_eq!(apply_query(&rows, &query).last().unwrap().id, "missing");
    }
}

#[test]
fn dependency_maintains_gap_and_checks_cycles_even_without_shifting() {
    let tasks = vec![
        Task {
            id: "a".into(),
            start: day(9),
            end: day(10),
        },
        Task {
            id: "b".into(),
            start: day(13),
            end: day(14),
        },
    ];
    let edges = vec![Dependency {
        before: "a".into(),
        after: "b".into(),
    }];
    let shifted =
        shift_dependencies(&tasks, &edges, "a", day(11), ShiftMode::MaintainGap, false).unwrap();
    assert_eq!((shifted[1].start, shifted[1].end), (day(15), day(16)));
    let cycle = vec![
        edges[0].clone(),
        Dependency {
            before: "b".into(),
            after: "a".into(),
        },
    ];
    assert!(shift_dependencies(&tasks, &cycle, "a", day(11), ShiftMode::None, false).is_err());
}

#[test]
fn formula_list_helpers_evaluate_without_mutating_input() {
    let values = BTreeMap::new();
    assert_eq!(evaluate("first([1, 2])", &values, &[]).unwrap(), json!(1.0));
    assert_eq!(evaluate("last([1, 2])", &values, &[]).unwrap(), json!(2.0));
    assert_eq!(
        evaluate("at([1, 2], 9)", &values, &[]).unwrap(),
        Value::Null
    );
    assert_eq!(
        evaluate("join([\"a\", \"b\"], \"-\")", &values, &[]).unwrap(),
        json!("a-b")
    );
    assert_eq!(
        evaluate("unique([1, 1, 2])", &values, &[]).unwrap(),
        json!([1.0, 2.0])
    );
    assert_eq!(
        evaluate("reverse([1, 2])", &values, &[]).unwrap(),
        json!([2.0, 1.0])
    );
    assert!(evaluate("at([1], -1)", &values, &[]).is_err());
}

#[test]
fn formula_supports_chained_methods_for_supported_functions() {
    let values = BTreeMap::new();
    assert_eq!(
        evaluate("\"hello\".upper().concat(\"!\")", &values, &[]).unwrap(),
        json!("HELLO!")
    );
    assert_eq!(
        evaluate("[1, 2, 3].reverse().first()", &values, &[]).unwrap(),
        json!(3.0)
    );
    assert_eq!(evaluate("1.25.round()", &values, &[]).unwrap(), json!(1.0));
    assert_eq!(evaluate("1.round()", &values, &[]).unwrap(), json!(1.0));
    assert_eq!(
        evaluate("[1].map(current)", &values, &[]).unwrap(),
        json!([1.0])
    );
}

#[test]
fn notion_style_list_scopes_and_text_helpers() {
    let values = BTreeMap::new();
    assert_eq!(
        evaluate("[1, 2, 3].map(current + index)", &values, &[]).unwrap(),
        json!([1.0, 3.0, 5.0])
    );
    assert_eq!(
        evaluate("filter([1, 2, 3], current > 1)", &values, &[]).unwrap(),
        json!([2.0, 3.0])
    );
    assert_eq!(
        evaluate("findIndex([1, 2], current > 9)", &values, &[]).unwrap(),
        json!(-1)
    );
    assert_eq!(
        evaluate("some([1, 2], current == 2)", &values, &[]).unwrap(),
        json!(true)
    );
    assert_eq!(
        evaluate("every([1, 2], current > 0)", &values, &[]).unwrap(),
        json!(true)
    );
    assert_eq!(
        evaluate("substring(\"Notion\", 0, 3)", &values, &[]).unwrap(),
        json!("Not")
    );
    assert_eq!(
        evaluate("slice([1, 2, 3], 1)", &values, &[]).unwrap(),
        json!([2.0, 3.0])
    );
    assert_eq!(
        evaluate("split(\"a,b\", \",\")", &values, &[]).unwrap(),
        json!(["a", "b"])
    );
    assert_eq!(
        evaluate("includes([1, 2], 2)", &values, &[]).unwrap(),
        json!(true)
    );
    assert_eq!(
        evaluate("flat([[1, 2], [3]])", &values, &[]).unwrap(),
        json!([1.0, 2.0, 3.0])
    );
    assert_eq!(evaluate("empty(0)", &values, &[]).unwrap(), json!(true));
}

#[test]
fn formula_regular_expression_functions_report_unsupported_patterns() {
    let values = BTreeMap::new();
    assert_eq!(
        evaluate("test(\"Notion\", \"Not\")", &values, &[]).unwrap(),
        json!(true)
    );
    assert_eq!(
        evaluate("match(\"a1b2\", \"[0-9]+\")", &values, &[]).unwrap(),
        json!(["1", "2"])
    );
    assert_eq!(
        evaluate("replaceAll(\"a1b2\", \"[0-9]+\", \"x\")", &values, &[]).unwrap(),
        json!("axbx")
    );
    assert!(evaluate("test(\"x\", \"(?=x)\")", &values, &[]).is_err());
}

#[test]
fn notion_numeric_helpers_support_lists_and_precision() {
    let values = BTreeMap::new();
    assert_eq!(
        evaluate("round(1234, -2)", &values, &[]).unwrap(),
        json!(1200.0)
    );
    assert_eq!(
        evaluate("round(1.234, 2)", &values, &[]).unwrap(),
        json!(1.23)
    );
    assert_eq!(
        evaluate("sum([1,2,3],4,5)", &values, &[]).unwrap(),
        json!(15.0)
    );
    assert_eq!(
        evaluate("median([1,2,3],4)", &values, &[]).unwrap(),
        json!(2.5)
    );
    assert_eq!(
        evaluate("toNumber(true) + sqrt(4)", &values, &[]).unwrap(),
        json!(3.0)
    );
    assert_eq!(evaluate("sign(0)", &values, &[]).unwrap(), json!(0.0));
    assert!(evaluate("sqrt(-1)", &values, &[]).is_err());
}

#[test]
fn formula_ternary_and_strict_equality_short_circuit() {
    let values = BTreeMap::new();
    assert_eq!(
        evaluate("2 === 2 ? 7 : 1 / 0", &values, &[]).unwrap(),
        json!(7.0)
    );
    assert_eq!(
        evaluate("2 !== 3 ? 4 : 5", &values, &[]).unwrap(),
        json!(4.0)
    );
    assert_eq!(
        evaluate("false ? 1 : true ? 2 : 3", &values, &[]).unwrap(),
        json!(2.0)
    );
}
