use super::{
    board_card, database, phosphor,
    theme::{Theme, button},
};
use crate::{
    board_interaction::{DropZone, MenuTarget},
    state::{State, Tab},
};
use moss_core::{
    Database, Property, PropertyKind, Record,
    board::{BoardProjection, bucket_key, group_key, group_value},
};
use rune::*;
use serde_json::{Value, json};
use std::{collections::BTreeMap, rc::Rc};

pub fn configured(config: &Value, field: &str, key: &str) -> bool {
    config
        .get(field)
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(key)))
}
pub fn name(s: &State, property: &Property, value: &Value) -> String {
    if value.is_null() {
        format!("No {}", property.name.to_lowercase())
    } else {
        super::property_value::text(s, property, Some(value))
    }
}
fn bucket_element(group: &Value, subgroup: &Value) -> String {
    format!("board-bucket-{}", bucket_key(group, subgroup))
}
fn lane_element(subgroup: &Value) -> String {
    format!("board-lane-{}", group_key(subgroup))
}
fn zones(s: &State, p: &BoardProjection, config: &Value) -> Rc<Vec<DropZone>> {
    let groups = crate::view_management::ordered_board_groups(&p.groups, config, false);
    let visible_subs: Vec<_> =
        crate::view_management::ordered_board_groups(&p.subgroups, config, true)
            .into_iter()
            .filter(|sub| !configured(config, "board_hidden_subgroups", &group_key(sub)))
            .collect();
    let subs: Vec<_> = visible_subs
        .iter()
        .cloned()
        .filter(|sub| {
            let key = group_key(sub);
            !configured(config, "board_hidden_subgroups", &key)
                && !configured(config, "board_collapsed_subgroups", &key)
        })
        .collect();
    let hide_empty = config.get("board_hide_empty").and_then(Value::as_bool) == Some(true);
    let scroll = s.database_views.board_ui.scroll.get().offset[0];
    let viewport = s.database_views.board_ui.viewport[2].max(600.);
    let mut output = Vec::new();
    for sub in &subs {
        let mut x = 0.;
        for group in &groups {
            let key = group_key(group);
            if configured(config, "board_hidden_groups", &key)
                || hide_empty && !p.buckets.iter().any(|b| b.group == *group && b.total > 0)
            {
                continue;
            }
            let collapsed = configured(config, "board_collapsed_groups", &key);
            let width = if collapsed {
                180.
            } else {
                board_card::width(config) + 12.
            };
            let rendered = x + width >= scroll - 300. && x <= scroll + viewport + 300.;
            x += width + 12.;
            if collapsed || !rendered {
                continue;
            }
            let cards = p
                .buckets
                .iter()
                .find(|b| b.group == *group && b.subgroup == *sub)
                .into_iter()
                .flat_map(|b| &b.records)
                .map(|r| r.id.clone())
                .collect();
            output.push(DropZone {
                key: bucket_element(group, sub),
                group: group.clone(),
                subgroup: sub.clone(),
                lane: lane_element(sub),
                extend_bottom: visible_subs.last() == Some(sub),
                cards,
            });
        }
    }
    Rc::new(output)
}

fn new_card(
    s: &mut State,
    property: &Property,
    subgroup: Option<&Property>,
    group: &Value,
    sub: &Value,
) {
    let mut values = BTreeMap::from([(property.id.clone(), group.clone())]);
    if let Some(p) = subgroup {
        values.insert(p.id.clone(), sub.clone());
    }
    s.create_inline(values, true);
    s.database_views.board.invalidate();
}
fn group_header(
    s: &State,
    p: &BoardProjection,
    value: &Value,
    index: usize,
    subgroup: &Value,
    t: Theme,
    config: &Value,
) -> El {
    let key = group_key(value);
    let label = name(s, &p.group_property, value);
    let collapsed = configured(config, "board_collapsed_groups", &key);
    let total: u64 = if p.subgroup_property.is_none() {
        p.group_totals.get(&key).copied().unwrap_or(0)
    } else {
        p.buckets
            .iter()
            .filter(|b| b.group == *value && b.subgroup == *subgroup)
            .map(|b| b.total)
            .sum()
    };
    let collapse_key = key.clone();
    let mut heading = row()
        .key(format!("board-heading-{}-{index}", group_key(subgroup)))
        .w_full()
        .h(32.)
        .gap(4.)
        .center_y()
        .hover_group()
        .child(
            row()
                .size(20., 24.)
                .center()
                .radius(3.)
                .hover(t.hover)
                .role("button")
                .label(if collapsed {
                    "Expand group"
                } else {
                    "Collapse group"
                })
                .focusable(true)
                .child(
                    rune::icon(
                        if collapsed {
                            &phosphor::CARET_RIGHT
                        } else {
                            &phosphor::CARET_DOWN
                        },
                        12.,
                    )
                    .color(t.muted),
                )
                .on_press(move |s: &mut State| {
                    s.toggle_board_group_setting("board_collapsed_groups", &collapse_key);
                }),
        );
    let (_, color, _) = super::property_value::status_style(&label, t);
    let drag_key = key.clone();
    let rects: Vec<_> = crate::view_management::ordered_board_groups(&p.groups, config, false)
        .iter()
        .enumerate()
        .filter_map(|(i, g)| {
            rune::memo::window_layout_with(
                &format!("board-heading-{}-{i}", group_key(subgroup)),
                |r| r,
            )
            .map(|r| (group_key(g), r))
        })
        .collect();
    heading = heading.child(
        row()
            .grow(1.)
            .min_w(0.)
            .h(24.)
            .center_y()
            .gap(6.)
            .child(
                t.label(label.clone(), t.text)
                    .px(5.)
                    .radius(3.)
                    .fill(color)
                    .truncate()
                    .min_w(0.),
            )
            .child(t.label(total.to_string(), t.muted))
            .on_drag(move |s: &mut State, drag, tx| {
                if drag.cancelled() {
                    s.cancel_board_group_drag();
                    tx.instant();
                    return;
                }
                if drag.phase == Phase::Down {
                    let context = crate::board_interaction::context(s);
                    let expected = s
                        .tab()
                        .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
                        .and_then(|v| v.config.get("board_group_order"))
                        .cloned();
                    let ui = &mut s.database_views.board_ui;
                    ui.group_drag = Some((drag_key.clone(), false));
                    ui.group_context = context;
                    ui.group_expected = expected;
                    s.database_views.board.frozen = true;
                    return;
                }
                if s.database_views
                    .board_ui
                    .group_drag
                    .as_ref()
                    .is_none_or(|(id, _)| id != &drag_key)
                    || s.database_views.board_ui.group_context
                        != crate::board_interaction::context(s)
                {
                    s.cancel_board_group_drag();
                    tx.instant();
                    return;
                }
                if drag.dx.hypot(drag.dy) >= 4. {
                    s.database_views.board_ui.group_point = Some(drag.window_point());
                    let x = drag.window_point()[0];
                    let target = rects
                        .iter()
                        .find(|(id, r)| id != &drag_key && x < r[0] + r[2] / 2.)
                        .map(|(id, _)| id.clone());
                    s.database_views.board_ui.group_drag = Some((drag_key.clone(), false));
                    s.database_views.board_ui.group_drop = target.clone();
                    if drag.phase == Phase::Up {
                        let expected = s
                            .tab()
                            .and_then(|tab| {
                                tab.database.as_ref().and_then(|db| db.views.get(tab.view))
                            })
                            .and_then(|v| v.config.get("board_group_order"))
                            .cloned();
                        if expected != s.database_views.board_ui.group_expected {
                            s.cancel_board_group_drag();
                            s.database_views.board_ui.error =
                                Some("Group order changed. Try the move again.".into());
                            tx.instant();
                            return;
                        }
                        if let Some(db) = s.tab().and_then(|tab| tab.database.as_ref()) {
                            let view = &db.views[s.tab().unwrap().view];
                            let projection = s.database_views.board.projection.as_ref();
                            if let Some(projection) = projection {
                                let mut order: Vec<_> =
                                    crate::view_management::ordered_board_groups(
                                        &projection.groups,
                                        &view.config,
                                        false,
                                    )
                                    .iter()
                                    .map(group_key)
                                    .collect();
                                order.retain(|id| id != &drag_key);
                                let at = target
                                    .as_ref()
                                    .and_then(|id| order.iter().position(|k| k == id))
                                    .unwrap_or(order.len());
                                order.insert(at, drag_key.clone());
                                s.set_board_config("board_group_order", json!(order));
                            }
                        }
                    }
                }
                if drag.phase == Phase::Up {
                    s.cancel_board_group_drag();
                }
                tx.instant();
            }),
    );
    let prop = p.group_property.clone();
    let val = value.clone();
    let anchor = format!("board-group-menu-{}-{index}", group_key(subgroup));
    let button_anchor = anchor.clone();
    heading.child(
        row()
            .key(anchor)
            .size(22., 24.)
            .center()
            .radius(3.)
            .hover(t.hover)
            .role("button")
            .label(format!("Actions for {label}"))
            .focusable(true)
            .child(
                rune::icon(&phosphor::DOTS_THREE, 16.)
                    .color(t.muted)
                    .group_opacity(if s.sidebar_keyboard_controls { 1. } else { 0. }, 1.),
            )
            .on_press(move |s: &mut State| {
                s.open_board_menu(
                    MenuTarget::Group {
                        property: prop.clone(),
                        value: val.clone(),
                        subgroup: false,
                    },
                    button_anchor.clone(),
                    None,
                )
            }),
    )
}
fn column(
    s: &State,
    tab: &Tab,
    db: &Database,
    p: &BoardProjection,
    group: &Value,
    sub: &Value,
    index: usize,
    subindex: usize,
    zones: Rc<Vec<DropZone>>,
    t: Theme,
) -> El {
    let config = &db.views[tab.view].config;
    let key = group_key(group);
    let collapsed = configured(config, "board_collapsed_groups", &key);
    let width = if collapsed {
        180.
    } else {
        board_card::width(config) + 12.
    };
    let bucket = p
        .buckets
        .iter()
        .find(|b| b.group == *group && b.subgroup == *sub);
    let total = bucket.map(|b| b.total).unwrap_or(0);
    let offset = bucket.map(|b| b.offset).unwrap_or(0);
    let mut records: Vec<&Record> = bucket
        .map(|b| b.records.iter().collect())
        .unwrap_or_default();
    if let Some(edit) = &s.editing_cell {
        if let Some(record) = tab.records.iter().find(|r| {
            r.id == edit.record_id
                && group_value(r, &p.group_property) == *group
                && p.subgroup_property
                    .as_ref()
                    .is_none_or(|property| group_value(r, property) == *sub)
        }) {
            if !records.iter().any(|r| r.id == record.id) {
                records.push(record);
            }
        }
    }
    let color = super::property_value::status_style(&name(s, &p.group_property, group), t).1;
    let mut column = col()
        .key(if p.subgroup_property.is_some() {
            format!("board-group-{subindex}-{index}")
        } else {
            format!("board-group-{index}")
        })
        .w(width)
        .gap(0.)
        .padding(6.)
        .radius(5.)
        .fill(
            if config.get("color_columns").and_then(Value::as_bool) == Some(true) {
                color.alpha(0.35)
            } else {
                Color::TRANSPARENT
            },
        )
        .child(
            group_header(s, p, group, index, sub, t, config).mb(if collapsed { 0. } else { 6. }),
        );
    if collapsed {
        return column;
    }
    let drop_key = bucket_element(group, sub);
    column = column.on_drawn_layout_tx(move |s: &mut State, rect, tx| {
        s.database_views
            .board_ui
            .drop_zones
            .insert(drop_key.clone(), rect);
        tx.live_only();
    });
    let mut cards = col()
        .key(bucket_element(group, sub))
        .w_full()
        .min_h(0.)
        .gap(6.);
    let mut has_flow_cards = false;
    let viewport = s.database_views.board_ui.viewport;
    let mut y = rune::memo::window_layout_with(&bucket_element(group, sub), |r| r[1])
        .unwrap_or(viewport[1] + 38.);
    for record in &records {
        let record = if s
            .editing_cell
            .as_ref()
            .is_some_and(|edit| edit.record_id == record.id)
        {
            tab.records
                .iter()
                .find(|r| r.id == record.id)
                .unwrap_or(record)
        } else {
            record
        };
        let estimate = board_card::estimated_height(db, tab, record);
        let h = s
            .database_views
            .board_ui
            .card_heights
            .get(&record.id)
            .copied()
            .unwrap_or(estimate);
        let editing = s
            .editing_cell
            .as_ref()
            .is_some_and(|edit| edit.record_id == record.id);
        let dragging = s
            .database_views
            .board_ui
            .drag
            .as_ref()
            .is_some_and(|drag| drag.record.id == record.id && drag.moved);
        has_flow_cards |= !dragging;
        if viewport[3] <= 0.
            || y + h >= viewport[1] - 240. && y <= viewport[1] + viewport[3] + 240.
            || editing
            || dragging
        {
            cards = cards.child(board_card::view(s, tab, db, p, record, zones.clone(), t));
        } else {
            cards = cards.child(
                el().key(format!("board-card-space-{}", record.id))
                    .w_full()
                    .h(h),
            );
        }
        if !dragging {
            y += h + 6.;
        }
    }
    if let Some(at) = s
        .database_views
        .board_ui
        .drag
        .as_ref()
        .filter(|d| d.moved)
        .and_then(|d| d.destination.as_ref())
        .filter(|at| at.group == *group && at.subgroup == *sub && at.before.is_none())
    {
        let _ = at;
        cards = cards.child(el().key("board-drop-end").w_full().h(3.).fill(t.blue));
        has_flow_cards = true;
    }
    column = column.child(cards.mb(if has_flow_cards { 6. } else { 0. }));
    if offset > 0 || total > offset + records.len() as u64 {
        let previous_group = group.clone();
        let previous_sub = sub.clone();
        let next_group = group.clone();
        let next_sub = sub.clone();
        let mut paging = row().w_full().center_y().gap(6.).child(
            t.label(
                format!(
                    "{}–{} of {total}",
                    offset + 1,
                    offset + records.len() as u64
                ),
                t.muted,
            )
            .grow(1.),
        );
        if offset > 0 {
            paging = paging.child(
                button(
                    format!("board-page-prev-{}", bucket_key(group, sub)),
                    "Previous",
                    t,
                )
                .on_press(move |s: &mut State| {
                    s.page_board_bucket(&previous_group, &previous_sub, false)
                }),
            );
        }
        if total > offset + records.len() as u64 {
            paging = paging.child(
                button(
                    format!("board-page-next-{}", bucket_key(group, sub)),
                    "Next",
                    t,
                )
                .on_press(move |s: &mut State| s.page_board_bucket(&next_group, &next_sub, true)),
            );
        }
        column = column.child(paging.mb(6.));
    }
    let property = p.group_property.clone();
    let subgroup = p.subgroup_property.clone();
    let target = group.clone();
    let subvalue = sub.clone();
    let label = if group.is_null() {
        String::new()
    } else {
        super::property_value::text(s, &property, Some(group))
    };
    let newkey = if subgroup.is_some() {
        format!("board-new-{}-{label}", group_key(sub))
    } else {
        format!("board-new-{label}")
    };
    column.child(
        database::new_page_button(newkey, t)
            .self_start()
            .on_press_tx(move |s: &mut State, tx| {
                new_card(s, &property, subgroup.as_ref(), &target, &subvalue);
                tx.instant();
            }),
    )
}
pub fn view(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let view = &db.views[tab.view];
    let fallback = if s
        .database_views
        .board
        .projection_for(&db.id, &view.id)
        .is_none()
    {
        match moss_core::board::project_loaded(db, view, &tab.records) {
            Ok(p) => Some(Rc::new(p)),
            Err(e) => {
                return col()
                    .padding(16.)
                    .gap(8.)
                    .child(t.label(e.to_string(), t.muted))
                    .child(
                        button("board-repair-group", "Choose group property", t).on_press(
                            |s: &mut State| {
                                s.open_view_settings();
                                if let Some(e) = &mut s.view_management {
                                    e.pane = crate::view_management::ViewPane::BoardGroup;
                                }
                            },
                        ),
                    );
            }
        }
    } else {
        None
    };
    let p = s
        .database_views
        .board
        .projection_for(&db.id, &view.id)
        .or_else(|| fallback.as_deref())
        .unwrap();
    let config = &view.config;
    let zones = zones(s, p, config);
    let groups = crate::view_management::ordered_board_groups(&p.groups, config, false);
    let subs = if p.subgroup_property.is_some() {
        crate::view_management::ordered_board_groups(&p.subgroups, config, true)
    } else {
        vec![Value::Null]
    };
    let hide_empty = config
        .get("board_hide_empty")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut board = col().key("board-root").gap(18.);
    for (subindex, sub) in subs.iter().enumerate() {
        let subkey = group_key(sub);
        if configured(config, "board_hidden_subgroups", &subkey) {
            continue;
        }
        let mut section = col().gap(6.);
        if let Some(property) = &p.subgroup_property {
            let collapsed = configured(config, "board_collapsed_subgroups", &subkey);
            let key = subkey.clone();
            let property = property.clone();
            let value = sub.clone();
            let anchor = format!("board-subgroup-menu-{subindex}");
            let source = anchor.clone();
            section = section.child(
                row()
                    .h(32.)
                    .w(360.)
                    .sticky_left(0.)
                    .fill(t.bg)
                    .gap(6.)
                    .center_y()
                    .child(
                        row()
                            .h(30.)
                            .min_w(0.)
                            .grow(1.)
                            .center_y()
                            .gap(6.)
                            .role("button")
                            .label(if collapsed {
                                "Expand subgroup"
                            } else {
                                "Collapse subgroup"
                            })
                            .focusable(true)
                            .hover(t.hover)
                            .radius(4.)
                            .child(
                                rune::icon(
                                    if collapsed {
                                        &phosphor::CARET_RIGHT
                                    } else {
                                        &phosphor::CARET_DOWN
                                    },
                                    14.,
                                )
                                .color(t.muted),
                            )
                            .child(t.label(name(s, &property, sub), t.text).truncate())
                            .on_press(move |s: &mut State| {
                                s.toggle_board_group_setting("board_collapsed_subgroups", &key);
                            }),
                    )
                    .child(
                        row()
                            .key(anchor)
                            .size(24., 24.)
                            .center()
                            .role("button")
                            .label("Subgroup actions")
                            .focusable(true)
                            .hover(t.hover)
                            .radius(4.)
                            .child(rune::icon(&phosphor::DOTS_THREE, 16.).color(t.muted))
                            .on_press(move |s: &mut State| {
                                s.open_board_menu(
                                    MenuTarget::Group {
                                        property: property.clone(),
                                        value: value.clone(),
                                        subgroup: true,
                                    },
                                    source.clone(),
                                    None,
                                )
                            }),
                    ),
            );
            if collapsed {
                board = board.child(section);
                continue;
            }
        }
        let lane_key = lane_element(sub);
        let mut columns =
            row()
                .gap(12.)
                .items_start()
                .on_drawn_layout_tx(move |s: &mut State, rect, tx| {
                    s.database_views
                        .board_ui
                        .lane_rects
                        .insert(lane_key.clone(), rect);
                    tx.live_only();
                });
        let mut x = 0.;
        let scroll = s.database_views.board_ui.scroll.get().offset[0];
        let viewport = s.database_views.board_ui.viewport[2].max(600.);
        for (index, group) in groups.iter().enumerate() {
            if configured(config, "board_hidden_groups", &group_key(group)) {
                continue;
            }
            if hide_empty && !p.buckets.iter().any(|b| b.group == *group && b.total > 0) {
                continue;
            }
            let collapsed = configured(config, "board_collapsed_groups", &group_key(group));
            let width = if collapsed {
                180.
            } else {
                board_card::width(config) + 12.
            };
            let has_drag_source = s.database_views.board_ui.drag.as_ref().is_some_and(|drag| {
                group_value(&drag.record, &p.group_property) == *group
                    && p.subgroup_property
                        .as_ref()
                        .is_none_or(|property| group_value(&drag.record, property) == *sub)
            });
            if !has_drag_source && (x + width < scroll - 300. || x > scroll + viewport + 300.) {
                let height = if collapsed {
                    44.
                } else {
                    p.buckets
                        .iter()
                        .find(|b| b.group == *group && b.subgroup == *sub)
                        .map(|b| {
                            b.records
                                .iter()
                                .map(|r| {
                                    s.database_views
                                        .board_ui
                                        .card_heights
                                        .get(&r.id)
                                        .copied()
                                        .unwrap_or_else(|| board_card::estimated_height(db, tab, r))
                                        + 6.
                                })
                                .sum::<f32>()
                        })
                        .unwrap_or(0.)
                        + 80.
                };
                columns = columns.child(
                    el().key(format!("board-column-space-{subindex}-{index}"))
                        .w(width)
                        .h(height),
                );
            } else {
                columns = columns.child(column(
                    s,
                    tab,
                    db,
                    p,
                    group,
                    sub,
                    index,
                    subindex,
                    zones.clone(),
                    t,
                ));
            }
            x += width + 12.;
        }
        if matches!(
            p.group_property.kind,
            PropertyKind::Select | PropertyKind::Status
        ) {
            let property = p.group_property.clone();
            let key = format!("board-add-group-{subindex}");
            let anchor = key.clone();
            columns = columns.child(button(key, "New group", t).self_start().on_press(
                move |s: &mut State| {
                    s.open_board_menu(
                        MenuTarget::Group {
                            property: property.clone(),
                            value: Value::Null,
                            subgroup: false,
                        },
                        anchor.clone(),
                        None,
                    );
                    s.database_views.board_ui.name = TextEdit::new("");
                    s.database_views.board_ui.name.focused = true;
                    if let Some(menu) = &mut s.database_views.board_ui.menu {
                        menu.pane = crate::board_interaction::MenuPane::Name;
                    }
                },
            ));
        }
        board = board.child(section.child(columns));
    }
    if let Some(error) = &s.database_views.board.error {
        board = board.child(
            row().gap(8.).child(t.label(error, t.error)).child(
                button("board-retry", "Retry", t)
                    .on_press(|s: &mut State| s.database_views.board.invalidate()),
            ),
        );
    }
    if p.groups_truncated {
        board = board.child(t.label(
            "More groups match. Use a filter to narrow this board.",
            t.muted,
        ));
    }
    if let Some(error) = &s.database_views.board_ui.error {
        board = board.child(
            row().gap(8.).child(t.label(error, t.error).wrap()).child(
                button("board-action-error-close", "Dismiss", t)
                    .on_press(|s: &mut State| s.database_views.board_ui.error = None),
            ),
        );
    }
    board
}
