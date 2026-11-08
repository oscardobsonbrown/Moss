use super::theme::{Theme, button};
use crate::state::{State, Tab};
use moss_core::{
    Database, PropertyKind,
    chart::{Aggregate, ChartConfig, ChartProjection, ChartType, DateBucket},
};
use rune::*;
use std::collections::BTreeMap;

fn color(index: usize, t: Theme) -> Color {
    let palette = [
        t.blue,
        Color {
            r: 0.28,
            g: 0.59,
            b: 0.42,
            a: 1.,
        },
        Color {
            r: 0.72,
            g: 0.40,
            b: 0.19,
            a: 1.,
        },
        Color {
            r: 0.57,
            g: 0.36,
            b: 0.70,
            a: 1.,
        },
        t.error,
    ];
    palette[index % palette.len()]
}
fn number(value: Option<f64>) -> String {
    value.map_or_else(
        || "—".into(),
        |v| {
            if v.fract() == 0. {
                format!("{v:.0}")
            } else {
                format!("{v:.2}")
            }
        },
    )
}
fn point_label(p: &moss_core::chart::ChartPoint) -> String {
    match &p.series {
        Some(series) => format!("{} · {}", p.category, series),
        None => p.category.clone(),
    }
}
fn point_hit(element: El, index: usize, label: String, interactive: bool, t: Theme) -> El {
    if !interactive {
        return element;
    }
    element
        .role("button")
        .label(label)
        .focusable(true)
        .hover(t.hover)
        .on_hover_change(move |s: &mut State, inside, _| {
            s.database_views.chart.hovered = inside.then_some(index);
        })
        .on_press(move |s: &mut State| s.open_chart_category(index))
}
/// Pure read-only Chart drawing. Point keys follow projection order for embedding.
pub fn projection_scoped(data: &ChartProjection, t: Theme, width: f32, prefix: &str) -> El {
    render(data, t, width, prefix, false)
}
fn render(data: &ChartProjection, t: Theme, width: f32, prefix: &str, interactive: bool) -> El {
    if data.points.is_empty() {
        return col()
            .key(format!("{prefix}-empty"))
            .padding(20.)
            .child(t.label("No matching records.", t.muted));
    }
    if data.config.kind == ChartType::Number {
        return point_hit(
            col()
                .key(format!("{prefix}-number"))
                .padding(16.)
                .gap(4.)
                .child(t.sized_label(number(data.points[0].value), 36., t.text))
                .child(t.label(format!("{} records", data.total), t.muted)),
            0,
            "Total".into(),
            interactive,
            t,
        );
    }
    let width = width.max(260.);
    let max_abs = data
        .points
        .iter()
        .filter_map(|p| p.value)
        .map(f64::abs)
        .fold(0f64, f64::max)
        .max(1.);
    match data.config.kind {
        ChartType::VerticalBar => {
            let mut items = row()
                .key(format!("{prefix}-bars-vertical"))
                .gap(10.)
                .items_end();
            for (i, p) in data.points.iter().enumerate() {
                let v = p.value.unwrap_or(0.);
                let height = ((v.abs() / max_abs) as f32 * 100.).max(if v == 0. { 1. } else { 2. });
                items = items.child(point_hit(
                    col()
                        .key(format!("{prefix}-point-{i}"))
                        .w(64.)
                        .gap(3.)
                        .items_center()
                        .child(t.label(number(p.value), t.text))
                        .child(
                            col()
                                .h(200.)
                                .w(32.)
                                .items_center()
                                .child(el().h(if v >= 0. { 100. - height } else { 100. }))
                                .child(el().w(26.).h(height).fill(color(i, t)).radius(3.)),
                        )
                        .child(t.label(point_label(p), t.muted).w_full().truncate()),
                    i,
                    point_label(p),
                    interactive,
                    t,
                ));
            }
            row().w_full().scroll_x().child(items)
        }
        ChartType::HorizontalBar => {
            let mut items = col()
                .key(format!("{prefix}-bars-horizontal"))
                .w(width.max(300.))
                .gap(6.);
            for (i, p) in data.points.iter().enumerate() {
                let v = p.value.unwrap_or(0.);
                let half = ((width - 170.).max(100.)) / 2.;
                let amount = ((v.abs() / max_abs) as f32 * half).max(if v == 0. { 1. } else { 2. });
                items = items.child(point_hit(
                    row()
                        .key(format!("{prefix}-point-{i}"))
                        .w_full()
                        .gap(8.)
                        .center_y()
                        .child(t.label(point_label(p), t.text).w(105.).truncate())
                        .child(row().w(half).h(18.).child(el().grow(1.)).children(
                            (v < 0.).then(|| el().w(amount).h(18.).fill(color(i, t)).radius(3.)),
                        ))
                        .child(el().w(1.).h(20.).fill(t.line))
                        .child(row().w(half).h(18.).children(
                            (v >= 0.).then(|| el().w(amount).h(18.).fill(color(i, t)).radius(3.)),
                        ))
                        .child(t.label(number(p.value), t.muted)),
                    i,
                    point_label(p),
                    interactive,
                    t,
                ));
            }
            row().w_full().scroll_x().child(items)
        }
        ChartType::Line => {
            let mut categories: Vec<&str> =
                data.points.iter().map(|p| p.category.as_str()).collect();
            categories.sort();
            categories.dedup();
            let chart_width = (categories.len() as f32 * 56.).max(width - 20.).max(260.);
            let mut series: BTreeMap<Option<&str>, Vec<[f32; 2]>> = BTreeMap::new();
            for p in &data.points {
                if let Some(value) = p.value {
                    let x = 20.
                        + categories
                            .iter()
                            .position(|c| *c == p.category)
                            .unwrap_or(0) as f32
                            * ((chart_width - 40.)
                                / categories.len().saturating_sub(1).max(1) as f32);
                    let y = 110. - ((value / max_abs) as f32 * 90.);
                    series.entry(p.series.as_deref()).or_default().push([x, y]);
                }
            }
            let mut shape = Shape::new().view_box(chart_width, 220.).line(
                [20., 110.],
                [chart_width - 20., 110.],
                1.,
                LineCap::Butt,
                t.line,
            );
            let series_keys: Vec<_> = series.keys().copied().collect();
            for (i, points) in series.into_values().enumerate() {
                if points.len() > 1 {
                    shape = shape.stroke(points, 2., color(i, t));
                }
            }
            let mut graph = row()
                .w(chart_width)
                .h(220.)
                .child(canvas(shape).w(chart_width).h(220.));
            for (i, p) in data.points.iter().enumerate() {
                if let Some(value) = p.value {
                    let x = 20.
                        + categories
                            .iter()
                            .position(|c| *c == p.category)
                            .unwrap_or(0) as f32
                            * ((chart_width - 40.)
                                / categories.len().saturating_sub(1).max(1) as f32);
                    let y = 110. - ((value / max_abs) as f32 * 90.);
                    let series_index = series_keys
                        .iter()
                        .position(|series| *series == p.series.as_deref())
                        .unwrap_or(0);
                    graph = graph.child(point_hit(
                        el().key(format!("{prefix}-line-marker-{i}"))
                            .absolute(x - 8., y - 8.)
                            .size(16., 16.)
                            .center()
                            .child(el().size(6., 6.).radius(3.).fill(color(series_index, t))),
                        i,
                        point_label(p),
                        interactive,
                        t,
                    ));
                }
            }
            let mut labels = row().gap(6.);
            for (i, p) in data.points.iter().enumerate() {
                labels = labels.child(point_hit(
                    t.label(format!("{}: {}", point_label(p), number(p.value)), t.muted)
                        .key(format!("{prefix}-point-{i}")),
                    i,
                    point_label(p),
                    interactive,
                    t,
                ));
            }
            row()
                .w_full()
                .scroll_x()
                .child(col().w(chart_width).child(graph).child(labels))
        }
        ChartType::Donut => {
            if data.points.iter().any(|p| p.value.is_some_and(|v| v < 0.)) {
                return t
                    .label("Donut charts require non-negative values.", t.error)
                    .wrap();
            }
            let sum: f64 = data.points.iter().filter_map(|p| p.value).sum();
            if sum <= 0. {
                return t.label("No positive values to display.", t.muted);
            }
            let mut shape = Shape::new().view_box(200., 200.);
            let mut angle = -std::f64::consts::FRAC_PI_2;
            for (i, p) in data.points.iter().enumerate() {
                let sweep = p.value.unwrap_or(0.) / sum * std::f64::consts::TAU;
                if sweep > 0. {
                    let steps = ((sweep / 0.08).ceil() as usize).max(2);
                    let points = (0..=steps)
                        .map(|n| {
                            let a = angle + sweep * n as f64 / steps as f64;
                            [100. + 66. * a.cos() as f32, 100. + 66. * a.sin() as f32]
                        })
                        .collect::<Vec<_>>();
                    shape = shape.stroke_with(points, false, 26., LineCap::Butt, color(i, t));
                }
                angle += sweep;
            }
            let mut legend = col().gap(4.);
            for (i, p) in data.points.iter().enumerate() {
                legend = legend.child(point_hit(
                    row()
                        .key(format!("{prefix}-point-{i}"))
                        .gap(6.)
                        .center_y()
                        .child(el().size(8., 8.).fill(color(i, t)).radius(4.))
                        .child(t.label(format!("{}: {}", point_label(p), number(p.value)), t.text)),
                    i,
                    point_label(p),
                    interactive,
                    t,
                ));
            }
            row()
                .gap(16.)
                .center_y()
                .child(canvas(shape).size(200., 200.))
                .child(legend)
        }
        ChartType::Number => unreachable!(),
    }
}
fn supported(kind: PropertyKind) -> bool {
    matches!(
        kind,
        PropertyKind::Title
            | PropertyKind::Text
            | PropertyKind::Number
            | PropertyKind::Select
            | PropertyKind::Status
            | PropertyKind::Date
            | PropertyKind::Checkbox
            | PropertyKind::Url
            | PropertyKind::Email
            | PropertyKind::Phone
    )
}
fn setup(db: &Database, config: &ChartConfig, t: Theme) -> El {
    let mut panel = col()
        .key("chart-settings-panel")
        .w_full()
        .gap(5.)
        .padding(12.)
        .fill(t.sidebar)
        .radius(8.);
    panel = panel.child(t.label("Chart type", t.muted));
    let mut kinds = row().w_full().flex_wrap().gap(4.);
    for (kind, name, key) in [
        (ChartType::VerticalBar, "Vertical bars", "vertical"),
        (ChartType::HorizontalBar, "Horizontal bars", "horizontal"),
        (ChartType::Line, "Line", "line"),
        (ChartType::Donut, "Donut", "donut"),
        (ChartType::Number, "Number", "number"),
    ] {
        kinds = kinds.child(
            button(format!("chart-kind-{key}"), name, t)
                .fill(if config.kind == kind {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .on_press_tx(move |s: &mut State, tx| {
                    s.chart_kind(kind);
                    tx.instant();
                }),
        );
    }
    panel = panel.child(kinds);
    if config.kind != ChartType::Number {
        panel = panel.child(t.label("Category property", t.muted));
        let mut options = row().w_full().flex_wrap().gap(4.);
        for property in db.properties.iter().filter(|p| supported(p.kind)) {
            let id = property.id.clone();
            let selected = config.category_property_id.as_deref() == Some(id.as_str());
            options = options.child(
                button(format!("chart-axis-{id}"), &property.name, t)
                    .fill(if selected {
                        t.selected
                    } else {
                        Color::TRANSPARENT
                    })
                    .on_press_tx(move |s: &mut State, tx| {
                        s.chart_axis(id.clone());
                        tx.instant();
                    }),
            );
        }
        panel = panel.child(options).child(t.label("Series", t.muted));
        let mut choices = row().w_full().flex_wrap().gap(4.).child(
            button("chart-series-none", "None", t)
                .fill(if config.series_property_id.is_none() {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .on_press_tx(|s: &mut State, tx| {
                    s.chart_series(None);
                    tx.instant();
                }),
        );
        for property in db.properties.iter().filter(|p| supported(p.kind)) {
            let id = property.id.clone();
            let selected = config.series_property_id.as_deref() == Some(id.as_str());
            choices = choices.child(
                button(format!("chart-series-{id}"), &property.name, t)
                    .fill(if selected {
                        t.selected
                    } else {
                        Color::TRANSPARENT
                    })
                    .on_press_tx(move |s: &mut State, tx| {
                        s.chart_series(Some(id.clone()));
                        tx.instant();
                    }),
            );
        }
        panel = panel.child(choices);
        if db.properties.iter().any(|p| {
            Some(p.id.as_str()) == config.category_property_id.as_deref()
                && p.kind == PropertyKind::Date
        }) {
            panel = panel.child(t.label("Date grouping", t.muted));
            let mut buckets = row().w_full().flex_wrap().gap(4.);
            for (bucket, name, key) in [
                (DateBucket::Day, "Day", "day"),
                (DateBucket::Week, "Week", "week"),
                (DateBucket::Month, "Month", "month"),
                (DateBucket::Quarter, "Quarter", "quarter"),
                (DateBucket::Year, "Year", "year"),
            ] {
                buckets = buckets.child(
                    button(format!("chart-date-{key}"), name, t)
                        .fill(if config.date_bucket == bucket {
                            t.selected
                        } else {
                            Color::TRANSPARENT
                        })
                        .on_press_tx(move |s: &mut State, tx| {
                            s.chart_bucket(bucket);
                            tx.instant();
                        }),
                );
            }
            panel = panel.child(buckets);
        }
    }
    panel = panel.child(t.label("Aggregation", t.muted));
    let mut ops = row().w_full().flex_wrap().gap(4.);
    for (op, name, key) in [
        (Aggregate::Count, "Count", "count"),
        (Aggregate::Sum, "Sum", "sum"),
        (Aggregate::Average, "Average", "average"),
        (Aggregate::Min, "Minimum", "min"),
        (Aggregate::Max, "Maximum", "max"),
    ] {
        ops = ops.child(
            button(format!("chart-aggregate-{key}"), name, t)
                .fill(if config.aggregation == op {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .on_press_tx(move |s: &mut State, tx| {
                    s.chart_aggregate(op);
                    tx.instant();
                }),
        );
    }
    panel = panel.child(ops);
    if config.aggregation != Aggregate::Count {
        panel = panel.child(t.label("Numeric property", t.muted));
        let mut values = row().w_full().flex_wrap().gap(4.);
        for p in db.properties.iter().filter(|p| {
            matches!(
                p.kind,
                PropertyKind::Number | PropertyKind::Formula | PropertyKind::Rollup
            )
        }) {
            let id = p.id.clone();
            let selected = config.value_property_id.as_deref() == Some(id.as_str());
            values = values.child(
                button(format!("chart-value-{id}"), &p.name, t)
                    .fill(if selected {
                        t.selected
                    } else {
                        Color::TRANSPARENT
                    })
                    .on_press_tx(move |s: &mut State, tx| {
                        s.chart_value(id.clone());
                        tx.instant();
                    }),
            );
        }
        panel = panel.child(values);
    }
    panel
}
pub fn view(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let state = &s.database_views.chart;
    let view = &db.views[tab.view];
    let config = super::super::chart_view::config(&view.config).unwrap_or_default();
    let mut body = col().key("chart-view").w_full().gap(10.).min_w(0.).child(
        row()
            .w_full()
            .center_y()
            .child(t.label("Chart", t.text).grow(1.))
            .child(
                button("chart-settings", "Chart settings", t).on_press(|s: &mut State| {
                    s.database_views.chart.settings_open = !s.database_views.chart.settings_open
                }),
            ),
    );
    if state.settings_open
        || (config.kind != ChartType::Number && config.category_property_id.is_none())
    {
        body = body.child(setup(db, &config, t));
    }
    if state.loading {
        body = body.child(t.label("Loading chart…", t.muted));
    }
    if let Some(error) = &state.save_error {
        body = body.child(t.label(error, t.error).wrap());
    }
    if let Some(error) = &state.error {
        body = body.child(t.label(error, t.error).wrap()).child(
            button("chart-reset", "Reset chart settings", t)
                .on_press(|s: &mut State| s.reset_chart_config()),
        );
    }
    if let Some(data) = &state.projection {
        body = body.child(render(data, t, 520., "chart", true));
        if let Some(point) = state.hovered.and_then(|i| data.points.get(i)) {
            body = body.child(
                t.label(
                    format!("{}: {}", point_label(point), number(point.value)),
                    t.muted,
                )
                .key("chart-hover-label"),
            );
        }
        let mut categories = col().key("chart-categories").w_full().gap(2.);
        for (index, p) in data.points.iter().enumerate() {
            categories = categories.child(
                button(
                    format!("chart-category-{index}"),
                    format!(
                        "{}   {}  ·  {} records",
                        point_label(p),
                        number(p.value),
                        p.count
                    ),
                    t,
                )
                .w_full()
                .on_press(move |s: &mut State| s.open_chart_category(index)),
            );
        }
        body = body.child(categories);
        if let Some(point) = state.drilldown.and_then(|i| data.points.get(i)) {
            let mut rows = col()
                .key("chart-drilldown")
                .w_full()
                .gap(3.)
                .padding(10.)
                .fill(t.sidebar)
                .radius(6.)
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .child(
                            t.label(
                                format!("{} · {} records", point_label(point), point.count),
                                t.text,
                            )
                            .grow(1.),
                        )
                        .child(
                            button("chart-drilldown-close", "Close", t)
                                .on_press(|s: &mut State| s.database_views.chart.drilldown = None),
                        ),
                );
            if state.drill_loading {
                rows = rows.child(t.label("Loading records…", t.muted));
            }
            if let Some(error) = &state.drill_error {
                rows = rows.child(t.label(error, t.error).wrap());
            }
            let mut paging = row().w_full().gap(6.).center_y();
            if state.has_first() {
                paging = paging.child(
                    button("chart-records-first", "First", t)
                        .on_press(|s: &mut State| s.chart_first_page()),
                );
            }
            if state.has_previous() {
                paging = paging.child(
                    button("chart-records-previous", "Previous", t)
                        .on_press(|s: &mut State| s.chart_previous_page()),
                );
            }
            if state.drill_next.is_some() {
                paging = paging.child(
                    button("chart-records-next", "Next", t)
                        .on_press(|s: &mut State| s.chart_next_page()),
                );
            }
            rows = rows.child(paging);
            for record in &state.drill_records {
                let id = record.id.clone();
                let database = db.id.clone();
                let mode = view
                    .config
                    .get("open_mode")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("side")
                    .to_owned();
                rows = rows.child(
                    button(
                        format!("chart-record-{id}"),
                        if record.title.is_empty() {
                            "Untitled"
                        } else {
                            &record.title
                        },
                        t,
                    )
                    .w_full()
                    .on_press(move |s: &mut State| {
                        s.open_database_record(&database, &id, &mode);
                    }),
                );
            }
            body = body.child(rows);
        }
    }
    body
}
