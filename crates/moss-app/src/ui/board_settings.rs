use super::{menu, phosphor, theme::Theme};
use crate::{state::State, view_management::ViewPane};
use moss_core::{Database, DatabaseView, Property, PropertyKind};
use rune::*;
use serde_json::{Value, json};

fn heading(name: &str, t: Theme) -> El {
    row()
        .w_full()
        .h(28.)
        .px(10.)
        .center_y()
        .child(t.label(name, t.muted))
}
fn setting(
    key: &str,
    name: &str,
    value: &str,
    icon: &'static Icon,
    pane: ViewPane,
    t: Theme,
) -> El {
    menu::row_item(key, name, rune::icon(icon, 16.).color(t.muted), t)
        .child(t.label(value, t.muted).max_w(90.).truncate())
        .child(menu::end(format!("{key}-end"), Some(&menu::CHEVRON), t))
        .on_press(move |s: &mut State| {
            if let Some(e) = &mut s.view_management {
                e.pane = pane;
            }
        })
}
fn toggle(key: &'static str, name: &str, icon: &'static Icon, value: bool, t: Theme) -> El {
    menu::row_item(
        format!("board-setting-{key}"),
        name,
        rune::icon(icon, 16.).color(t.muted),
        t,
    )
    .role("switch")
    .checked(value)
    .child(
        el().size(30., 18.)
            .radius(9.)
            .fill(if value { t.blue } else { t.faint })
            .child(
                el().size(14., 14.)
                    .radius(7.)
                    .fill(Color::WHITE)
                    .absolute(if value { 14. } else { 2. }, 2.),
            ),
    )
    .on_press(move |s: &mut State| {
        s.set_board_config(key, json!(!value));
    })
}
fn back(pane: ViewPane, t: Theme) -> El {
    menu::row_item(
        "board-settings-back",
        "Back",
        rune::icon(&menu::BACK, 16.).color(t.muted),
        t,
    )
    .child(menu::end("board-settings-back-end", None, t))
    .on_press(move |s: &mut State| {
        if let Some(e) = &mut s.view_management {
            e.pane = pane;
        }
    })
}
fn property_for<'a>(db: &'a Database, view: &DatabaseView, subgroup: bool) -> Option<&'a Property> {
    if subgroup {
        view.config
            .get("board_subgroup_by")
            .and_then(Value::as_str)
            .and_then(|id| db.properties.iter().find(|p| p.id == id))
    } else {
        view.query
            .group_by
            .as_deref()
            .and_then(|id| db.properties.iter().find(|p| p.id == id))
            .or_else(|| {
                view.query
                    .group_by
                    .is_none()
                    .then(|| {
                        db.properties
                            .iter()
                            .find(|p| matches!(p.kind, PropertyKind::Status | PropertyKind::Select))
                    })
                    .flatten()
            })
    }
}
fn choices(
    key: &'static str,
    values: &[(&'static str, &'static str)],
    selected: &str,
    return_pane: ViewPane,
    t: Theme,
) -> El {
    let mut list = col().w_full();
    for &(value, name) in values {
        list = list.child(
            menu::row_item(
                format!("board-{key}-{value}"),
                name,
                rune::icon(
                    if key == "card_size" {
                        &phosphor::ARROWS_OUT_SIMPLE
                    } else {
                        &phosphor::IMAGES_SQUARE
                    },
                    16.,
                )
                .color(t.muted),
                t,
            )
            .child(menu::end(
                format!("board-{key}-{value}-end"),
                (value == selected).then_some(&menu::CHECK),
                t,
            ))
            .on_press(move |s: &mut State| {
                if s.set_board_config(key, json!(value)) {
                    if let Some(e) = &mut s.view_management {
                        e.pane = return_pane;
                    }
                }
            }),
        );
    }
    list.child(back(return_pane, t))
}
fn action(key: impl Into<String>, name: &str, icon: &'static Icon, t: Theme) -> El {
    row()
        .key(key)
        .size(22., 24.)
        .center()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(name)
        .help(name)
        .focusable(true)
        .child(rune::icon(icon, 14.).color(t.muted))
}
fn configured(config: &Value, field: &str, key: &str) -> bool {
    config
        .get(field)
        .and_then(Value::as_array)
        .is_some_and(|values| values.iter().any(|v| v.as_str() == Some(key)))
}

pub fn body(s: &State, pane: ViewPane, t: Theme) -> Option<El> {
    let editor = s.view_management.as_ref()?;
    let db = s.tab()?.database.as_ref()?;
    let view = editor.expected.as_ref()?;
    let config = &view.config;
    let on = |key: &str, default: bool| config.get(key).and_then(Value::as_bool).unwrap_or(default);
    let content = match pane {
        ViewPane::BoardLayout => col()
            .w_full()
            .child(heading("Board layout", t))
            .child(setting(
                "board-layout-type",
                "Layout",
                "Board",
                &phosphor::KANBAN,
                ViewPane::Layout,
                t,
            ))
            .child(setting(
                "board-card-size",
                "Card size",
                match config.get("card_size").and_then(Value::as_str) {
                    Some("small") => "Small",
                    Some("large") => "Large",
                    _ => "Medium",
                },
                &phosphor::ARROWS_OUT_SIMPLE,
                ViewPane::BoardCardSize,
                t,
            ))
            .child(setting(
                "board-card-preview",
                "Card preview",
                match config.get("card_preview").and_then(Value::as_str) {
                    Some("content") => "Page content",
                    Some("cover") => "Page cover",
                    _ => "None",
                },
                &phosphor::IMAGES_SQUARE,
                ViewPane::BoardCardPreview,
                t,
            ))
            .child(toggle(
                "show_page_icon",
                "Show page icon",
                &phosphor::FILE,
                on("show_page_icon", true),
                t,
            ))
            .child(toggle(
                "wrap_cards",
                "Wrap card text",
                &phosphor::TEXT_ALIGN_LEFT,
                on("wrap_cards", true),
                t,
            ))
            .child(toggle(
                "color_columns",
                "Color columns",
                &phosphor::SQUARE,
                on("color_columns", false),
                t,
            ))
            .child(setting(
                "board-card-properties",
                "Property order",
                "",
                &phosphor::LIST_BULLETS,
                ViewPane::BoardProperties,
                t,
            ))
            .child(back(ViewPane::Main, t)),
        ViewPane::BoardCardSize => col().w_full().child(heading("Card size", t)).child(choices(
            "card_size",
            &[("small", "Small"), ("medium", "Medium"), ("large", "Large")],
            config
                .get("card_size")
                .and_then(Value::as_str)
                .unwrap_or("medium"),
            ViewPane::BoardLayout,
            t,
        )),
        ViewPane::BoardCardPreview => {
            col()
                .w_full()
                .child(heading("Card preview", t))
                .child(choices(
                    "card_preview",
                    &[
                        ("none", "None"),
                        ("content", "Page content"),
                        ("cover", "Page cover"),
                    ],
                    config
                        .get("card_preview")
                        .and_then(Value::as_str)
                        .unwrap_or("none"),
                    ViewPane::BoardLayout,
                    t,
                ))
        }
        ViewPane::BoardGroupProperty | ViewPane::BoardSubgroupProperty => {
            let subgroup = pane == ViewPane::BoardSubgroupProperty;
            let current = property_for(db, view, subgroup);
            let primary = property_for(db, view, false);
            let mut list = col().w_full().child(heading(
                if subgroup { "Subgroup by" } else { "Group by" },
                t,
            ));
            if subgroup {
                list = list.child(
                    menu::row_item(
                        "board-subgroup-none",
                        "None",
                        rune::icon(&phosphor::MINUS, 16.).color(t.muted),
                        t,
                    )
                    .child(menu::end(
                        "board-subgroup-none-end",
                        current.is_none().then_some(&menu::CHECK),
                        t,
                    ))
                    .on_press(|s: &mut State| s.choose_board_grouping(None, true)),
                );
            }
            for property in db.properties.iter().filter(|p| {
                moss_core::board::group_supported(p.kind)
                    && (!subgroup || primary.is_none_or(|primary| primary.id != p.id))
            }) {
                let id = property.id.clone();
                list = list.child(
                    menu::row_item(
                        format!("board-group-property-{id}"),
                        &property.name,
                        super::property_icons::view(property.kind, t.muted),
                        t,
                    )
                    .child(menu::end(
                        format!("board-group-property-{id}-end"),
                        current.filter(|p| p.id == id).map(|_| &menu::CHECK),
                        t,
                    ))
                    .on_press(move |s: &mut State| s.choose_board_grouping(Some(&id), subgroup)),
                );
            }
            list.child(back(
                if subgroup {
                    ViewPane::BoardSubgroup
                } else {
                    ViewPane::BoardGroup
                },
                t,
            ))
        }
        ViewPane::BoardGroup | ViewPane::BoardSubgroup => {
            let subgroup = pane == ViewPane::BoardSubgroup;
            let property = property_for(db, view, subgroup);
            let mut list = col()
                .w_full()
                .child(heading(if subgroup { "Subgroups" } else { "Groups" }, t))
                .child(setting(
                    "board-group-by",
                    if subgroup { "Subgroup by" } else { "Group by" },
                    property.map(|p| p.name.as_str()).unwrap_or("None"),
                    &phosphor::COLUMNS,
                    if subgroup {
                        ViewPane::BoardSubgroupProperty
                    } else {
                        ViewPane::BoardGroupProperty
                    },
                    t,
                ));
            if !subgroup {
                list = list.child(toggle(
                    "board_hide_empty",
                    "Hide empty groups",
                    &phosphor::EYE_SLASH,
                    on("board_hide_empty", false),
                    t,
                ));
            }
            if let Some(property) = property {
                let projection = s.database_views.board.projection.as_ref().filter(|p| {
                    if subgroup {
                        p.subgroup_property
                            .as_ref()
                            .is_some_and(|p| p.id == property.id)
                    } else {
                        p.group_property.id == property.id
                    }
                });
                if let Some(projection) = projection {
                    let groups = if subgroup {
                        &projection.subgroups
                    } else {
                        &projection.groups
                    };
                    let groups =
                        crate::view_management::ordered_board_groups(groups, config, subgroup);
                    let hidden_field = if subgroup {
                        "board_hidden_subgroups"
                    } else {
                        "board_hidden_groups"
                    };
                    let collapsed_field = if subgroup {
                        "board_collapsed_subgroups"
                    } else {
                        "board_collapsed_groups"
                    };
                    for (index, value) in groups.iter().enumerate() {
                        let key = moss_core::board::group_key(value);
                        let name = if value.is_null() {
                            format!("No {}", property.name.to_lowercase())
                        } else {
                            super::property_value::text(s, property, Some(value))
                        };
                        let hidden = configured(config, hidden_field, &key);
                        let collapsed = configured(config, collapsed_field, &key);
                        let id = key.clone();
                        let collapse_id = key.clone();
                        let up_id = key.clone();
                        let down_id = key.clone();
                        let mut group = row()
                            .key(format!("board-manage-group-{index}"))
                            .w_full()
                            .min_h(32.)
                            .px(10.)
                            .gap(4.)
                            .center_y()
                            .role("group")
                            .label(&name)
                            .child(t.label(name, t.text).grow(1.).min_w(0.).truncate())
                            .child(
                                action(
                                    format!("board-group-visibility-{index}"),
                                    if hidden { "Show group" } else { "Hide group" },
                                    if hidden {
                                        &phosphor::EYE_SLASH
                                    } else {
                                        &phosphor::EYE
                                    },
                                    t,
                                )
                                .on_press(move |s: &mut State| {
                                    s.toggle_board_group_setting(hidden_field, &id)
                                }),
                            )
                            .child(
                                action(
                                    format!("board-group-collapse-{index}"),
                                    if collapsed {
                                        "Expand group"
                                    } else {
                                        "Collapse group"
                                    },
                                    if collapsed {
                                        &phosphor::ARROWS_OUT_LINE_VERTICAL
                                    } else {
                                        &phosphor::ARROWS_IN_LINE_VERTICAL
                                    },
                                    t,
                                )
                                .on_press(move |s: &mut State| {
                                    s.toggle_board_group_setting(collapsed_field, &collapse_id)
                                }),
                            );
                        if index > 0 {
                            group = group.child(
                                action(
                                    format!("board-group-up-{index}"),
                                    "Move group up",
                                    &phosphor::ARROW_UP,
                                    t,
                                )
                                .on_press(move |s: &mut State| {
                                    s.move_board_group_order(subgroup, &up_id, false)
                                }),
                            );
                        } else {
                            group = group.child(el().size(22., 24.));
                        }
                        if index + 1 < groups.len() {
                            group = group.child(
                                action(
                                    format!("board-group-down-{index}"),
                                    "Move group down",
                                    &phosphor::ARROW_DOWN,
                                    t,
                                )
                                .on_press(move |s: &mut State| {
                                    s.move_board_group_order(subgroup, &down_id, true)
                                }),
                            );
                        } else {
                            group = group.child(el().size(22., 24.));
                        }
                        list = list.child(group);
                    }
                } else {
                    list = list.child(t.label("Loading groups…", t.muted).px(10.).py(8.));
                }
            }
            list.child(back(ViewPane::Main, t))
        }
        ViewPane::BoardProperties => {
            let order = moss_core::record_layout::properties(db);
            let mut list = col().w_full().child(heading("Property order", t)).child(
                t.label("Applies to pages and all views", t.muted)
                    .px(10.)
                    .py(4.)
                    .wrap(),
            );
            for (index, p) in order.iter().enumerate() {
                let up = p.id.clone();
                let down = p.id.clone();
                let mut property = menu::row_item(
                    format!("board-property-order-{}", p.id),
                    &p.name,
                    super::property_icons::view(p.kind, t.muted),
                    t,
                )
                .role("group")
                .focusable(false);
                if index > 0 {
                    property = property.child(
                        action(
                            format!("board-property-up-{}", p.id),
                            "Move property up",
                            &phosphor::ARROW_UP,
                            t,
                        )
                        .on_press(move |s: &mut State| s.move_board_property_order(&up, false)),
                    );
                } else {
                    property = property.child(el().size(22., 24.));
                }
                if index + 1 < order.len() {
                    property = property.child(
                        action(
                            format!("board-property-down-{}", p.id),
                            "Move property down",
                            &phosphor::ARROW_DOWN,
                            t,
                        )
                        .on_press(move |s: &mut State| s.move_board_property_order(&down, true)),
                    );
                } else {
                    property = property.child(el().size(22., 24.));
                }
                list = list.child(property);
            }
            list.child(back(ViewPane::BoardLayout, t))
        }
        _ => return None,
    };
    Some(content)
}
