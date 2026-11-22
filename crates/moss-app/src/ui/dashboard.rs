use super::theme::{Theme, button};
use crate::{
    dashboard_view::WidgetData,
    state::{State, Tab},
};
use moss_core::{
    Database, ViewKind,
    chart::{Aggregate, ChartType, DateBucket},
    dashboard::{DashboardConfig, DashboardWidget},
};
use rune::*;

pub fn view(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(view) = db.views.get(tab.view) else {
        return col();
    };
    let Ok(config) = DashboardConfig::read(&view.config) else {
        return col().child(t.label("Dashboard settings could not be read.", t.error));
    };
    let state = &s.database_views.dashboard;
    let mut bar = row().w_full().center_y().gap(8.).instant_changes().child(
        button(
            "dashboard-edit",
            if state.editing { "Done" } else { "Edit" },
            t,
        )
        .on_press(|s: &mut State| {
            s.database_views.dashboard.editing = !s.database_views.dashboard.editing;
            s.database_views.dashboard.adding = false;
            s.database_views.dashboard.chart_settings = None;
            s.database_views.dashboard.chart_settings_source = None;
        }),
    );
    bar = bar.child(
        button("dashboard-add", "Add widget", t)
            .instant_changes()
            .on_press(|s: &mut State| {
                s.database_views.dashboard.editing = true;
                s.database_views.dashboard.adding = !s.database_views.dashboard.adding;
            }),
    );
    bar = bar.child(
        button("dashboard-global-filter", "Filter widgets", t).on_press(|s: &mut State| {
            s.database_views.dashboard.filter_widget = None;
            s.database_views.dashboard.filter_property = None;
            s.database_views.dashboard.filtering = !s.database_views.dashboard.filtering
        }),
    );
    let mut body = col()
        .key("dashboard")
        .w_full()
        .grow(1.)
        .min_h(0.)
        .gap(12.)
        .child(bar);
    if let Some(error) = &state.error {
        body = body.child(row().gap(6.).child(t.label(error, t.error)).child(
            button("dashboard-retry", "Retry", t).on_press(|s: &mut State| s.dashboard_retry()),
        ));
    }
    for (index, filter) in config.global_filters.iter().enumerate() {
        body = body.child(
            row()
                .gap(6.)
                .center_y()
                .child(t.label(
                    format!("{} = {}", filter.property_name, filter.value),
                    t.muted,
                ))
                .child(
                    button(format!("dashboard-filter-remove-{index}"), "Remove", t)
                        .on_press(move |s: &mut State| s.dashboard_remove_filter(index)),
                ),
        );
    }
    if state.filtering {
        let mut pane = col()
            .key("dashboard-filter-panel")
            .gap(4.)
            .padding(8.)
            .border(1., t.line)
            .radius(6.);
        let mut seen = std::collections::HashSet::new();
        for (id, data) in state
            .widgets
            .iter()
            .filter_map(|(id, v)| v.as_ref().ok().map(|data| (id, data)))
        {
            if state
                .filter_widget
                .as_ref()
                .is_some_and(|selected| selected != id)
            {
                continue;
            }
            for property in &data.database.properties {
                if !matches!(
                    property.kind,
                    moss_core::PropertyKind::Title
                        | moss_core::PropertyKind::Text
                        | moss_core::PropertyKind::Number
                        | moss_core::PropertyKind::Date
                        | moss_core::PropertyKind::Checkbox
                        | moss_core::PropertyKind::Select
                        | moss_core::PropertyKind::Status
                ) || !seen.insert((property.name.clone(), format!("{:?}", property.kind)))
                {
                    continue;
                }
                let name = property.name.clone();
                let kind = property.kind;
                pane = pane.child(
                    button(
                        format!("dashboard-filter-property-{}", property.id),
                        &name,
                        t,
                    )
                    .on_press(move |s: &mut State| {
                        s.database_views.dashboard.filter_property = Some((name.clone(), kind))
                    }),
                );
            }
        }
        if let Some((name, _)) = &state.filter_property {
            pane = pane
                .child(t.label(format!("Value for {name}"), t.muted))
                .child(
                    super::inputs::field(&state.filter_input, "dashboard-filter-value", "Value", t)
                        .on_edit(|s: &mut State| &mut s.database_views.dashboard.filter_input),
                )
                .child(
                    button("dashboard-filter-apply", "Apply", t)
                        .on_press(|s: &mut State| s.dashboard_apply_filter()),
                );
        }
        body = body.child(pane);
    }
    if state.adding {
        let mut sources = col()
            .key("dashboard-sources")
            .w_full()
            .max_h(280.)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .gap(2.)
            .padding(8.)
            .border(1., t.line)
            .radius(6.);
        sources = sources.child(
            super::inputs::field(
                &state.source_input,
                "dashboard-source-search",
                "Search databases",
                t,
            )
            .on_edit(|s: &mut State| &mut s.database_views.dashboard.source_input)
            .on_submit(|s: &mut State, tx| {
                s.database_views.dashboard.source_search =
                    s.database_views.dashboard.source_input.text.clone();
                tx.instant();
            }),
        );
        sources = sources.child(
            button("dashboard-source-search-submit", "Search", t).on_press(|s: &mut State| {
                s.database_views.dashboard.source_search =
                    s.database_views.dashboard.source_input.text.clone();
            }),
        );
        for source in &state.candidates {
            sources = sources.child(t.label(&source.name, t.muted));
            for saved in source
                .views
                .iter()
                .filter(|v| v.kind != ViewKind::Dashboard)
            {
                let db_id = source.id.clone();
                let view_id = saved.id.clone();
                let chart_db = db_id.clone();
                let chart_view = view_id.clone();
                sources = sources.child(
                    row()
                        .w_full()
                        .center_y()
                        .gap(6.)
                        .child(
                            button(
                                format!("dashboard-source-{view_id}"),
                                format!("{} · {}", saved.name, saved.kind.label()),
                                t,
                            )
                            .on_press(move |s: &mut State| {
                                s.dashboard_add_widget(db_id.clone(), view_id.clone())
                            }),
                        )
                        .child(
                            button(format!("dashboard-figure-{chart_view}"), "Add figure", t)
                                .on_press(move |s: &mut State| {
                                    s.dashboard_add_figure(chart_db.clone(), chart_view.clone())
                                }),
                        ),
                );
            }
        }
        body = body.child(sources);
    }
    if config.widgets.is_empty() && !state.adding {
        body = body.child(
            col()
                .w_full()
                .padding(24.)
                .gap(6.)
                .border(1., t.line)
                .radius(8.)
                .child(t.label("Build your dashboard", t.text))
                .child(t.label(
                    "Add a database view or a figure to see your data here.",
                    t.muted,
                )),
        );
    }
    let mut grid = row()
        .key("dashboard-widgets")
        .w_full()
        .min_w(0.)
        .flex_wrap()
        .items_start()
        .gap(12.);
    for widget in &config.widgets {
        grid = grid.child(widget_view(s, widget, &config.widgets, t));
    }
    body.child(grid)
}
fn widget_view(s: &State, widget: &DashboardWidget, widgets: &[DashboardWidget], t: Theme) -> El {
    let id = widget.id.clone();
    let data = s.database_views.dashboard.widgets.get(&id);
    let title = data
        .and_then(|v| v.as_ref().ok())
        .map(|d| {
            if let Some(chart) = &d.chart {
                let measure = match chart.config.aggregation {
                    Aggregate::Count => "Count".to_owned(),
                    aggregation => {
                        let label = match aggregation {
                            Aggregate::Sum => "Sum",
                            Aggregate::Average => "Average",
                            Aggregate::Min => "Min",
                            Aggregate::Max => "Max",
                            Aggregate::Count => unreachable!(),
                        };
                        let property = d
                            .database
                            .properties
                            .iter()
                            .find(|p| Some(&p.id) == chart.config.value_property_id.as_ref())
                            .map(|p| p.name.as_str())
                            .unwrap_or("value");
                        format!("{label} {property}")
                    }
                };
                let category = d
                    .database
                    .properties
                    .iter()
                    .find(|p| Some(&p.id) == chart.config.category_property_id.as_ref());
                if chart.config.kind != ChartType::Number {
                    if let Some(category) = category {
                        return format!("{} · {} by {}", d.database.name, measure, category.name);
                    }
                }
                format!("{} · {}", d.database.name, measure)
            } else {
                format!("{} · {}", d.database.name, d.view.name)
            }
        })
        .unwrap_or_else(|| "Widget".into());
    let mut header = row()
        .w_full()
        .h(32.)
        .center_y()
        .gap(4.)
        .child(t.label(title, t.text).grow(1.).truncate());
    if let Some(Ok(data)) = data {
        let page = data.database.page_id.clone();
        header = header.child(
            button(format!("dashboard-open-source-{id}"), "Open source", t)
                .on_press(move |s: &mut State| s.open_page(&page, true)),
        );
    }
    let mut controls = row().w_full().min_w(0.).flex_wrap().gap(2.);
    if s.database_views.dashboard.editing {
        let drag_id = id.clone();
        let rects: Vec<_> = widgets
            .iter()
            .filter(|w| w.id != id)
            .filter_map(|w| {
                let target = w.id.clone();
                rune::memo::window_layout_with(&format!("dashboard-widget-{}", w.id), move |rect| {
                    (
                        target.clone(),
                        rect[0] + rect[2] / 2.,
                        rect[1] + rect[3] / 2.,
                    )
                })
            })
            .collect();
        header = header.on_drag(move |s: &mut State, drag, tx| {
            if drag.phase == Phase::Up
                && !drag.cancelled()
                && (drag.dx.abs() > 4. || drag.dy.abs() > 4.)
            {
                let [x, y] = drag.window_point();
                let before = rects
                    .iter()
                    .find(|(_, cx, cy)| y < *cy && (y < *cy - 40. || x < *cx))
                    .map(|(id, _, _)| id.as_str())
                    .unwrap_or("");
                s.dashboard_reorder_widget(&drag_id, before);
            }
            tx.instant();
        });
        let filter_id = id.clone();
        let settings_id = id.clone();
        controls = controls.child(
            button(
                format!("dashboard-chart-settings-{id}"),
                "Chart settings",
                t,
            )
            .on_press(move |s: &mut State| s.dashboard_toggle_chart_settings(&settings_id)),
        );
        controls = controls.child(
            button(format!("dashboard-widget-filter-{id}"), "Filter", t).on_press(
                move |s: &mut State| {
                    s.database_views.dashboard.filter_widget = Some(filter_id.clone());
                    s.database_views.dashboard.filter_property = None;
                    s.database_views.dashboard.filtering = true;
                },
            ),
        );
        if widget.query.is_some() {
            let reset_id = id.clone();
            controls = controls.child(
                button(format!("dashboard-widget-reset-{id}"), "Reset filters", t)
                    .on_press(move |s: &mut State| s.dashboard_reset_widget_query(&reset_id)),
            );
        }
        let smaller = id.clone();
        let larger = id.clone();
        controls = controls
            .child(
                button(format!("dashboard-narrow-{id}"), "Narrow", t)
                    .on_press(move |s: &mut State| s.dashboard_resize_width(&smaller, -1)),
            )
            .child(
                button(format!("dashboard-wide-{id}"), "Wide", t)
                    .on_press(move |s: &mut State| s.dashboard_resize_width(&larger, 1)),
            );
        let remove = id.clone();
        let duplicate = id.clone();
        let up = id.clone();
        let down = id.clone();
        controls = controls
            .child(
                button(format!("dashboard-up-{id}"), "Move up", t)
                    .on_press(move |s: &mut State| s.dashboard_move_widget(&up, -1)),
            )
            .child(
                button(format!("dashboard-down-{id}"), "Move down", t)
                    .on_press(move |s: &mut State| s.dashboard_move_widget(&down, 1)),
            )
            .child(
                button(format!("dashboard-copy-{id}"), "Duplicate", t)
                    .on_press(move |s: &mut State| s.dashboard_duplicate_widget(&duplicate)),
            )
            .child(
                button(format!("dashboard-remove-{id}"), "Remove", t)
                    .on_press(move |s: &mut State| s.dashboard_remove_widget(&remove)),
            );
    }
    let mut card = col()
        .key(format!("dashboard-widget-{id}"))
        .instant_changes()
        .w(170. * f32::from(widget.width))
        .max_w(680.)
        .min_w(0.)
        .h(widget.height as f32)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .padding(10.)
        .gap(4.)
        .border(1., t.line)
        .radius(6.)
        .child(header)
        .child(controls);
    if s.database_views.dashboard.editing
        && s.database_views.dashboard.chart_settings.as_deref() == Some(&id)
    {
        if let Some(database) = &s.database_views.dashboard.chart_settings_source {
            let source = database.views.iter().find(|v| v.id == widget.view_id);
            let configured = widget.layout.get("chart").is_some()
                || source.is_some_and(|view| view.kind == ViewKind::Chart);
            let chart_value = if widget.layout.get("chart").is_some() {
                &widget.layout
            } else if let Some(view) = source {
                &view.config
            } else {
                &widget.layout
            };
            if let Ok(chart) = crate::chart_view::config(chart_value) {
                let mut settings = col().w_full().gap(4.).child(t.label("Display", t.muted));
                let mut types = row().w_full().flex_wrap().gap(4.);
                for (kind, label, key) in [
                    (ChartType::Number, "Number", "number"),
                    (ChartType::VerticalBar, "Bar", "bar"),
                    (ChartType::HorizontalBar, "Horizontal", "horizontal"),
                    (ChartType::Line, "Line", "line"),
                    (ChartType::Donut, "Donut", "donut"),
                ] {
                    let chart_id = id.clone();
                    let selected = configured && chart.kind == kind;
                    types = types.child(
                        button(format!("dashboard-chart-type-{id}-{key}"), label, t)
                            .role("radio")
                            .checked(selected)
                            .fill(if selected {
                                t.selected
                            } else {
                                Color::TRANSPARENT
                            })
                            .on_press(move |s: &mut State| s.dashboard_chart_kind(&chart_id, kind)),
                    );
                }
                settings = settings.child(types);
                if configured && chart.kind != ChartType::Number {
                    let mut categories = row().w_full().flex_wrap().gap(4.);
                    for property in database.properties.iter().filter(|p| {
                        matches!(
                            p.kind,
                            moss_core::PropertyKind::Title
                                | moss_core::PropertyKind::Text
                                | moss_core::PropertyKind::Number
                                | moss_core::PropertyKind::Select
                                | moss_core::PropertyKind::Status
                                | moss_core::PropertyKind::Date
                                | moss_core::PropertyKind::Checkbox
                                | moss_core::PropertyKind::Url
                                | moss_core::PropertyKind::Email
                                | moss_core::PropertyKind::Phone
                        )
                    }) {
                        let chart_id = id.clone();
                        let property_id = property.id.clone();
                        let selected =
                            chart.category_property_id.as_deref() == Some(property.id.as_str());
                        categories = categories.child(
                            button(
                                format!("dashboard-chart-category-{id}-{}", property.id),
                                &property.name,
                                t,
                            )
                            .role("radio")
                            .checked(selected)
                            .fill(if selected {
                                t.selected
                            } else {
                                Color::TRANSPARENT
                            })
                            .on_press(move |s: &mut State| {
                                s.dashboard_chart_category(&chart_id, property_id.clone())
                            }),
                        );
                    }
                    settings = settings
                        .child(t.label("Group by", t.muted))
                        .child(categories);
                    if database.properties.iter().any(|p| {
                        p.kind == moss_core::PropertyKind::Date
                            && chart.category_property_id.as_deref() == Some(p.id.as_str())
                    }) {
                        let mut buckets = row().w_full().flex_wrap().gap(4.);
                        for (bucket, label, key) in [
                            (DateBucket::Day, "Day", "day"),
                            (DateBucket::Week, "Week", "week"),
                            (DateBucket::Month, "Month", "month"),
                            (DateBucket::Quarter, "Quarter", "quarter"),
                            (DateBucket::Year, "Year", "year"),
                        ] {
                            let chart_id = id.clone();
                            let selected = chart.date_bucket == bucket;
                            buckets = buckets.child(
                                button(format!("dashboard-chart-bucket-{id}-{key}"), label, t)
                                    .role("radio")
                                    .checked(selected)
                                    .fill(if selected {
                                        t.selected
                                    } else {
                                        Color::TRANSPARENT
                                    })
                                    .on_press(move |s: &mut State| {
                                        s.dashboard_chart_bucket(&chart_id, bucket)
                                    }),
                            );
                        }
                        settings = settings.child(t.label("Interval", t.muted)).child(buckets);
                    }
                }
                let mut aggregates = row().w_full().flex_wrap().gap(4.);
                let chart_id = id.clone();
                let selected = configured && chart.aggregation == Aggregate::Count;
                aggregates = aggregates.child(
                    button(format!("dashboard-chart-count-{id}"), "Count", t)
                        .role("radio")
                        .checked(selected)
                        .fill(if selected {
                            t.selected
                        } else {
                            Color::TRANSPARENT
                        })
                        .on_press(move |s: &mut State| {
                            s.dashboard_chart_aggregate(&chart_id, Aggregate::Count, None)
                        }),
                );
                for property in database.properties.iter().filter(|p| {
                    matches!(
                        p.kind,
                        moss_core::PropertyKind::Number
                            | moss_core::PropertyKind::Formula
                            | moss_core::PropertyKind::Rollup
                    )
                }) {
                    for (aggregation, label, key) in [
                        (Aggregate::Sum, "Sum", "sum"),
                        (Aggregate::Average, "Average", "average"),
                        (Aggregate::Min, "Min", "min"),
                        (Aggregate::Max, "Max", "max"),
                    ] {
                        let chart_id = id.clone();
                        let property_id = property.id.clone();
                        let selected = configured
                            && chart.aggregation == aggregation
                            && chart.value_property_id.as_deref() == Some(property.id.as_str());
                        aggregates = aggregates.child(
                            button(
                                format!("dashboard-chart-{key}-{id}-{}", property.id),
                                format!("{label} {}", property.name),
                                t,
                            )
                            .role("radio")
                            .checked(selected)
                            .fill(if selected {
                                t.selected
                            } else {
                                Color::TRANSPARENT
                            })
                            .on_press(move |s: &mut State| {
                                s.dashboard_chart_aggregate(
                                    &chart_id,
                                    aggregation,
                                    Some(property_id.clone()),
                                )
                            }),
                        );
                    }
                }
                card = card.child(
                    settings
                        .child(t.label("Measure", t.muted))
                        .child(aggregates),
                );
            }
        }
    }
    card = match data {
        None => card.child(t.label("Loading…", t.muted)),
        Some(Err(error)) => card.child(t.label(error, t.error)).child(
            button(format!("dashboard-widget-retry-{id}"), "Retry", t)
                .on_press(|s: &mut State| s.dashboard_retry()),
        ),
        Some(Ok(data)) => {
            if let Some(chart) = &data.chart {
                let mut chart_ui = col()
                    .grow(1.)
                    .min_h(0.)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .child(super::chart::projection_scoped(
                        chart,
                        t,
                        (170. * f32::from(widget.width) - 20.).max(100.),
                        &format!("dashboard-chart-{id}"),
                    ));
                for (index, point) in chart.points.iter().enumerate() {
                    let widget_id = id.clone();
                    chart_ui = chart_ui.child(
                        button(
                            format!("dashboard-chart-group-{id}-{index}"),
                            format!("{} · {}", point.category, point.count),
                            t,
                        )
                        .on_press(move |s: &mut State| {
                            s.database_views.dashboard.chart_drilldown =
                                Some((widget_id.clone(), index))
                        }),
                    );
                    if s.database_views.dashboard.chart_drilldown.as_ref()
                        == Some(&(id.clone(), index))
                    {
                        for record in &point.records {
                            let db = data.database.id.clone();
                            let record_id = record.id.clone();
                            chart_ui = chart_ui.child(
                                button(
                                    format!("dashboard-chart-record-{id}-{}", record.id),
                                    &record.title,
                                    t,
                                )
                                .on_press(move |s: &mut State| {
                                    s.open_database_record(&db, &record_id, "center")
                                }),
                            );
                        }
                        if point.count > point.records.len() as u64 {
                            chart_ui = chart_ui
                                .child(t.label("Open source for remaining records.", t.muted));
                        }
                    }
                }
                card.child(chart_ui)
            } else if let Some(map) = &data.map {
                let viewport = crate::map_view::MapViewport {
                    width: (170. * f32::from(widget.width) - 20.) as f64,
                    height: (widget.height as f64 - 80.).max(40.),
                    ..s.database_views
                        .dashboard
                        .map_drag
                        .as_ref()
                        .filter(|drag| drag.widget_id == id)
                        .map(|drag| drag.preview)
                        .or_else(|| s.database_views.dashboard.map_viewports.get(&id).copied())
                        .or(data.map_base)
                        .unwrap_or_default()
                };
                let mode = data
                    .view
                    .config
                    .get("open_mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("center");
                let pan_id = id.clone();
                let zoom_in = id.clone();
                let zoom_out = id.clone();
                let reset_id = id.clone();
                let controls = super::map_controls::navigation(
                    format!("dashboard-map-navigation-{id}"),
                    super::map::zoom_button(
                        format!("dashboard-map-in-{id}"),
                        "Zoom in",
                        &super::phosphor::PLUS,
                        t,
                    )
                    .on_press(move |s: &mut State| s.dashboard_map_zoom(&zoom_in, 1)),
                    super::map::zoom_button(
                        format!("dashboard-map-out-{id}"),
                        "Zoom out",
                        &super::phosphor::MINUS,
                        t,
                    )
                    .on_press(move |s: &mut State| s.dashboard_map_zoom(&zoom_out, -1)),
                    super::map::zoom_button(
                        format!("dashboard-map-reset-{id}"),
                        "Reset to saved source view",
                        &super::phosphor::CORNERS_OUT,
                        t,
                    )
                    .on_press(move |s: &mut State| {
                        let d = &mut s.database_views.dashboard;
                        d.map_drag = None;
                        d.map_viewports.remove(&reset_id);
                    }),
                    t,
                );
                let loader = s.database_views.dashboard.map_tiles.get(&id);
                let surface = s.database_views.dashboard.map_surfaces.get(&id);
                if let Some(surface) = surface {
                    surface.sync_from_view(viewport, t.dark, t.scale);
                }
                if loader.is_none() && surface.is_none() {
                    card = card.child(t.label("Map tiles loading", t.muted));
                }
                let map_error = loader
                    .and_then(|loader| loader.error.clone())
                    .or_else(|| surface.and_then(|surface| surface.error()));
                if let Some(error) = map_error.as_deref() {
                    card = card.child(
                        t.label(format!("Map tiles unavailable: {error}"), t.muted)
                            .key(format!("dashboard-map-error-{id}"))
                            .w_full()
                            .min_w(0.)
                            .truncate()
                            .help(error),
                    );
                }
                card.child(
                    super::map::canvas(
                        &map.points,
                        loader.map(|loader| loader.tiles.as_slice()).unwrap_or(&[]),
                        surface.map(|surface| surface.native_surface()),
                        viewport,
                        t,
                        &data.database.id,
                        mode,
                        &format!("dashboard-map-{id}"),
                    )
                    .child(controls)
                    .cursor(
                        if s.database_views
                            .dashboard
                            .map_drag
                            .as_ref()
                            .is_some_and(|drag| drag.widget_id == id)
                        {
                            Cursor::Grabbing
                        } else {
                            Cursor::Grab
                        },
                    )
                    .on_drag(move |s: &mut State, drag, tx| {
                        if drag.phase == Phase::Down {
                            s.dashboard_map_drag_start(&pan_id);
                        } else if drag.cancelled() || drag.phase == Phase::Up {
                            s.dashboard_map_drag_end(&pan_id, drag.dx, drag.dy, drag.cancelled());
                        } else {
                            s.dashboard_map_drag_move(&pan_id, drag.dx, drag.dy);
                        }
                        tx.instant();
                    }),
                )
            } else {
                card.child(render_records(data, &id, s.prefs.week_start(), t))
            }
        }
    };
    if s.database_views.dashboard.editing {
        let target = id.clone();
        card = card.child(
            button(format!("dashboard-height-{id}"), "Resize", t).on_drag(
                move |s: &mut State, drag, tx| {
                    if drag.phase == Phase::Up && !drag.cancelled() {
                        s.dashboard_resize_widget(&target, drag.dy.round() as i16);
                    }
                    tx.instant();
                },
            ),
        );
    }
    card
}
fn record_button(data: &WidgetData, widget_id: &str, record: &moss_core::Record, t: Theme) -> El {
    let database = data.database.id.clone();
    let id = record.id.clone();
    let mode = data
        .view
        .config
        .get("open_mode")
        .and_then(|v| v.as_str())
        .unwrap_or(
            if matches!(data.view.kind, ViewKind::Gallery | ViewKind::Calendar) {
                "center"
            } else {
                "side"
            },
        )
        .to_owned();
    button(
        format!("dashboard-record-{widget_id}-{id}"),
        &record.title,
        t,
    )
    .w_full()
    .on_press(move |s: &mut State| s.open_database_record(&database, &id, &mode))
}
fn render_records(data: &WidgetData, widget_id: &str, week_start: u8, t: Theme) -> El {
    let outer = col().grow(1.).min_h(0.).w_full().gap(4.);
    let mut content = col().grow(1.).min_h(0.).w_full().scroll(ScrollSpec {
        stick_bottom: false,
    });
    match data.view.kind {
        ViewKind::Table => {
            let index = data
                .database
                .views
                .iter()
                .position(|v| v.id == data.view.id)
                .unwrap_or(0);
            let props: Vec<_> = moss_core::record_layout::columns_for_view(&data.database, index)
                .into_iter()
                .filter(|p| super::property_visibility::visible(&data.database, index, &p.id))
                .collect();
            let mut heading = row().w_full().h(28.).gap(8.).fill(t.sidebar);
            for p in &props {
                heading = heading.child(t.label(&p.name, t.muted).w(120.).truncate());
            }
            content = content.child(heading);
            for record in &data.records {
                let mut cells = row().w_full().h(32.).gap(8.).center_y().border(1., t.line);
                for p in &props {
                    if p.kind == moss_core::PropertyKind::Title {
                        cells = cells.child(record_button(data, widget_id, record, t).w(120.));
                    } else {
                        let label = if p.kind == moss_core::PropertyKind::Relation {
                            record
                                .values
                                .get(&p.id)
                                .and_then(|v| v.as_array())
                                .map(|ids| format!("{} linked pages", ids.len()))
                                .unwrap_or_default()
                        } else {
                            record
                                .values
                                .get(&p.id)
                                .map(crate::state::display_value)
                                .unwrap_or_default()
                        };
                        cells = cells.child(t.label(label, t.text).w(120.).truncate());
                    }
                }
                content = content.child(cells);
            }
        }
        ViewKind::Board => {
            let group = data
                .view
                .query
                .group_by
                .as_ref()
                .and_then(|id| data.database.properties.iter().find(|p| &p.id == id))
                .or_else(|| {
                    data.database.properties.iter().find(|p| {
                        matches!(
                            p.kind,
                            moss_core::PropertyKind::Status | moss_core::PropertyKind::Select
                        )
                    })
                });
            let mut groups: std::collections::BTreeMap<String, Vec<&moss_core::Record>> =
                Default::default();
            for record in &data.records {
                let value = group
                    .and_then(|p| record.values.get(&p.id))
                    .map(crate::state::display_value)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "No value".into());
                groups.entry(value).or_default().push(record);
            }
            let mut columns = row().gap(8.).items_start();
            for (name, records) in groups {
                let mut column = col().w(150.).gap(4.).child(t.label(name, t.muted));
                for record in records {
                    column = column.child(
                        row()
                            .border(1., t.line)
                            .radius(4.)
                            .child(record_button(data, widget_id, record, t)),
                    );
                }
                columns = columns.child(column);
            }
            content = content.child(columns);
        }
        ViewKind::Gallery => {
            let mut cards = row().w_full().flex_wrap().gap(8.);
            for record in &data.records {
                cards = cards.child(
                    col()
                        .w(150.)
                        .h(72.)
                        .border(1., t.line)
                        .radius(5.)
                        .child(record_button(data, widget_id, record, t)),
                );
            }
            content = content.child(cards);
        }
        ViewKind::Calendar | ViewKind::Timeline => {
            content = content.child(date_records(data, widget_id, week_start, t));
        }
        ViewKind::List | ViewKind::Feed => {
            let index = data
                .database
                .views
                .iter()
                .position(|v| v.id == data.view.id)
                .unwrap_or(0);
            let properties = moss_core::record_layout::properties(&data.database);
            for record in &data.records {
                let mut line = row()
                    .w_full()
                    .center_y()
                    .gap(6.)
                    .child(record_button(data, widget_id, record, t).grow(1.));
                for property in properties.iter().copied().filter(|p| {
                    p.kind != moss_core::PropertyKind::Relation
                        && super::property_visibility::visible(&data.database, index, &p.id)
                }) {
                    if let Some(value) = record.values.get(&property.id) {
                        line = line.child(
                            t.label(crate::state::display_value(value), t.muted)
                                .truncate(),
                        );
                    }
                }
                content = content.child(line);
            }
        }
        _ => {
            content = content.child(t.label("Widget layout is unavailable.", t.muted));
        }
    }
    if data.has_more {
        content = content.child(t.label(
            "Showing the first 100 matching records. Open the source for more.",
            t.muted,
        ));
    }
    outer.child(content)
}
fn date_records(data: &WidgetData, widget_id: &str, week_start: u8, t: Theme) -> El {
    use chrono::{Datelike, NaiveDate};
    let property = data
        .view
        .config
        .get("date_property_id")
        .and_then(|v| v.as_str())
        .and_then(|id| data.database.properties.iter().find(|p| p.id == id))
        .or_else(|| {
            data.database
                .properties
                .iter()
                .find(|p| p.kind == moss_core::PropertyKind::Date)
        });
    let Some(property) = property else {
        return col().child(t.label("Choose a Date property in the source view.", t.error));
    };
    let date = |record: &moss_core::Record| -> Option<NaiveDate> {
        let value = record.values.get(&property.id)?;
        let raw = value
            .as_str()
            .or_else(|| value.get("start").and_then(|v| v.as_str()))?;
        NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()
    };
    if data.view.kind == ViewKind::Timeline {
        let earliest = data
            .records
            .iter()
            .filter_map(date)
            .min()
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let origin = earliest - chrono::Duration::days(3);
        let mut rows = col().gap(4.).child(t.label(
            format!("{} — {}", origin, origin + chrono::Duration::days(30)),
            t.muted,
        ));
        for record in &data.records {
            if let Some(start) = date(record) {
                let end = record
                    .values
                    .get(&property.id)
                    .and_then(|v| v.get("end"))
                    .and_then(|v| v.as_str())
                    .and_then(|raw| NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok())
                    .unwrap_or(start);
                let left = ((start - origin).num_days().clamp(0, 30) as f32) * 6.;
                let width = ((end - start).num_days().max(0) as f32 * 6. + 70.).clamp(70., 200.);
                rows = rows.child(
                    row().w_full().child(el().w(left)).child(
                        row()
                            .w(width)
                            .fill(t.selected)
                            .radius(4.)
                            .child(record_button(data, widget_id, record, t)),
                    ),
                );
            } else {
                rows = rows.child(
                    row()
                        .gap(6.)
                        .child(t.label("No date", t.muted))
                        .child(record_button(data, widget_id, record, t)),
                );
            }
        }
        return rows;
    }

    let month = data
        .records
        .iter()
        .filter_map(date)
        .min()
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    let first = month.with_day(1).unwrap();
    let offset = crate::calendar_view::weekday_offset(first, week_start) as i64;
    let mut grid = col()
        .gap(2.)
        .child(t.label(format!("{} {}", first.format("%B"), first.year()), t.text));
    let mut header = row().w_full();
    for index in 0..7 {
        let day = crate::calendar_view::WEEKDAYS[(week_start as usize + index) % 7];
        header = header.child(t.label(day, t.muted).grow(1.));
    }
    grid = grid.child(header);
    for week in 0..6 {
        let mut row = row().w_full().items_start();
        for day in 0..7 {
            let current = first + chrono::Duration::days(week * 7 + day - offset);
            let mut cell = col()
                .grow(1.)
                .min_w(0.)
                .min_h(34.)
                .border(1., t.line)
                .child(t.label(
                    current.day().to_string(),
                    if current.month() == first.month() {
                        t.text
                    } else {
                        t.faint
                    },
                ));
            for record in data.records.iter().filter(|r| date(r) == Some(current)) {
                cell = cell.child(record_button(data, widget_id, record, t));
            }
            row = row.child(cell);
        }
        grid = grid.child(row);
    }
    grid
}

#[cfg(test)]
#[path = "dashboard_tests.rs"]
mod tests;
