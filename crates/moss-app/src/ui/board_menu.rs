use super::{
    board, menu, phosphor,
    theme::{Theme, button},
};
use crate::{
    board_interaction::{MenuPane, MenuTarget},
    state::State,
};
use moss_core::{
    PropertyKind,
    board::{group_key, group_value},
};
use rune::*;
use serde_json::Value;

fn action(key: &str, label: &str, icon: &'static Icon, t: Theme) -> El {
    menu::row_item(key, label, rune::icon(icon, 16.).color(t.muted), t)
}
fn close(s: &mut State) {
    s.database_views.board_ui.menu = None;
    s.database_views.board_ui.error = None;
}
fn setting_action(s: &mut State, run: impl FnOnce(&mut State)) {
    s.error = None;
    run(s);
    if let Some(error) = &s.error {
        s.database_views.board_ui.error = Some(error.clone());
    } else {
        close(s);
    }
}

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let open = s.database_views.board_ui.menu.as_ref()?;
    let tab = s.tab()?;
    let db = tab.database.as_ref()?;
    let view = db.views.get(tab.view)?;
    if view.kind != moss_core::ViewKind::Board {
        return None;
    }
    let p = s.database_views.board.projection_for(&db.id, &view.id);
    let mut body = col().w_full().gap(2.);
    match (&open.target, open.pane) {
        (MenuTarget::Card(record), MenuPane::Main) => {
            let r = record.clone();
            body = body.child(t.label("Page", t.muted).px(10.).py(5.)).child(
                action("board-menu-open", "Open", &phosphor::ARROW_UP_RIGHT, t).on_press(
                    move |s: &mut State| {
                        close(s);
                        s.open_database_record(&r.database_id, &r.id, "side");
                    },
                ),
            );
            let r = record.clone();
            body = body.child(
                action("board-menu-full", "Open as page", &phosphor::FILE, t).on_press(
                    move |s: &mut State| {
                        close(s);
                        s.open_database_record(&r.database_id, &r.id, "full");
                    },
                ),
            );
            let id = record.id.clone();
            body = body.child(
                action("board-menu-rename", "Rename", &phosphor::TEXT_AA, t).on_press(
                    move |s: &mut State| {
                        close(s);
                        s.edit_record_property(&id, None);
                    },
                ),
            );
            for property in db.properties.iter().filter(|property| {
                property.kind == PropertyKind::Place
                    && super::property_visibility::visible(db, tab.view, &property.id)
                    && record.values.get(&property.id).is_none_or(|value| {
                        value.is_null() || value.as_str().is_some_and(str::is_empty)
                    })
            }) {
                let record_id = record.id.clone();
                let property_id = property.id.clone();
                body = body.child(
                    action(
                        &format!("board-menu-set-place-{property_id}"),
                        &format!("Set {}", property.name),
                        &phosphor::MAP_PIN,
                        t,
                    )
                    .on_press(move |s: &mut State| {
                        close(s);
                        s.open_place_picker(&record_id, &property_id);
                    }),
                );
            }
            body = body.child(
                action("board-menu-move", "Move to group", &phosphor::COLUMNS, t).on_press(
                    |s: &mut State| {
                        let ui = &mut s.database_views.board_ui;
                        ui.name = TextEdit::new("");
                        ui.name.focused = true;
                        if let Some(menu) = &mut ui.menu {
                            menu.pane = MenuPane::Move;
                        }
                    },
                ),
            );
            if p.and_then(|p| p.subgroup_property.as_ref()).is_some() {
                body = body.child(
                    action(
                        "board-menu-subgroup",
                        "Move to subgroup",
                        &phosphor::ROWS,
                        t,
                    )
                    .on_press(|s: &mut State| {
                        let ui = &mut s.database_views.board_ui;
                        ui.name = TextEdit::new("");
                        ui.name.focused = true;
                        if let Some(menu) = &mut ui.menu {
                            menu.pane = MenuPane::Subgroup;
                        }
                    }),
                );
            }
            let id = record.id.clone();
            body = body.child(
                action("board-menu-duplicate", "Duplicate", &phosphor::COPY, t)
                    .on_press(move |s: &mut State| s.duplicate_board_card_action(&id)),
            );
            let id = record.id.clone();
            body = body.child(el().w_full().h(1.).fill(t.line).my(4.)).child(
                action(
                    "board-menu-trash",
                    "Move to Trash",
                    &phosphor::TRASH,
                    Theme { text: t.error, ..t },
                )
                .on_press(move |s: &mut State| {
                    if s.trash_board_card(&id) {
                        close(s);
                    } else {
                        s.database_views.board_ui.error = s.error.clone();
                    }
                }),
            );
        }
        (MenuTarget::Card(record), pane @ (MenuPane::Move | MenuPane::Subgroup)) => {
            let p = p?;
            let secondary = pane == MenuPane::Subgroup;
            let property = if secondary {
                p.subgroup_property.as_ref()?
            } else {
                &p.group_property
            };
            let values = if secondary { &p.subgroups } else { &p.groups };
            body = body.child(
                super::inputs::field(
                    &s.database_views.board_ui.name,
                    "board-move-search",
                    "Find a group…",
                    t,
                )
                .on_edit(|s: &mut State| &mut s.database_views.board_ui.name)
                .on_input_change(|_: &mut State, tx| tx.instant()),
            );
            let search = s.database_views.board_ui.name.text.to_lowercase();
            let mut choices = col().w_full().max_h(288.).scroll(ScrollSpec {
                stick_bottom: false,
            });
            for (index, value) in
                crate::view_management::ordered_board_groups(values, &view.config, secondary)
                    .into_iter()
                    .enumerate()
            {
                let name = board::name(s, property, &value);
                if !name.to_lowercase().contains(&search) {
                    continue;
                }
                let r = record.clone();
                let group_property = p.group_property.clone();
                let subgroup_property = p.subgroup_property.clone();
                let (group, sub) = if secondary {
                    (group_value(record, &group_property), value.clone())
                } else {
                    (
                        value.clone(),
                        subgroup_property
                            .as_ref()
                            .map(|p| group_value(record, p))
                            .unwrap_or(Value::Null),
                    )
                };
                let current = group_value(record, property) == value;
                choices = choices.child(
                    action(
                        &format!("board-move-choice-{index}"),
                        &name,
                        &phosphor::COLUMNS,
                        t,
                    )
                    .child(menu::end(
                        format!("board-move-choice-{index}-end"),
                        current.then_some(&phosphor::CHECK),
                        t,
                    ))
                    .on_press(move |s: &mut State| {
                        if s.commit_board_move(
                            &r,
                            &group_property,
                            subgroup_property.as_ref(),
                            group.clone(),
                            sub.clone(),
                            None,
                        ) {
                            close(s);
                        } else {
                            s.database_views.board_ui.error = s.error.clone();
                        }
                    }),
                );
            }
            body = body.child(choices);
        }
        (
            MenuTarget::Group {
                property,
                value,
                subgroup,
            },
            MenuPane::Name,
        ) => {
            body = body.child(
                t.label(
                    if value.is_null() {
                        "New group"
                    } else {
                        "Group name"
                    },
                    t.muted,
                )
                .px(10.)
                .py(4.),
            );
            let property = property.clone();
            let old = (!value.is_null()).then(|| value.clone());
            let property2 = property.clone();
            let old2 = old.clone();
            let submit = move |s: &mut State| {
                let name = s.database_views.board_ui.name.text.clone();
                if s.save_board_group_name(&property, old.as_ref(), &name) {
                    close(s);
                } else {
                    s.database_views.board_ui.error = s.error.clone();
                }
            };
            body = body
                .child(
                    super::inputs::field(
                        &s.database_views.board_ui.name,
                        "board-group-name",
                        "Group name",
                        t,
                    )
                    .on_edit(|s: &mut State| &mut s.database_views.board_ui.name)
                    .on_submit(move |s: &mut State, tx| {
                        let name = s.database_views.board_ui.name.text.clone();
                        if s.save_board_group_name(&property2, old2.as_ref(), &name) {
                            close(s);
                        } else {
                            s.database_views.board_ui.error = s.error.clone();
                        }
                        tx.instant();
                    }),
                )
                .child(button("board-group-save", "Save", t).on_press(submit));
            let _ = subgroup;
        }
        (
            MenuTarget::Group {
                property,
                value,
                subgroup,
            },
            _,
        ) => {
            let key = group_key(value);
            let secondary = *subgroup;
            body = body.child(
                t.label(board::name(s, property, value), t.muted)
                    .px(10.)
                    .py(4.),
            );
            if !value.is_null()
                && matches!(property.kind, PropertyKind::Select | PropertyKind::Status)
            {
                let name = value.as_str().unwrap_or("").to_owned();
                body = body.child(
                    action("board-group-rename", "Rename group", &phosphor::TEXT_AA, t).on_press(
                        move |s: &mut State| {
                            let ui = &mut s.database_views.board_ui;
                            ui.name = TextEdit::new(&name);
                            ui.name.focused = true;
                            ui.name.set_selection(0, ui.name.text.len());
                            if let Some(menu) = &mut ui.menu {
                                menu.pane = MenuPane::Name;
                            }
                        },
                    ),
                );
            }
            let collapse_field = if secondary {
                "board_collapsed_subgroups"
            } else {
                "board_collapsed_groups"
            };
            let hide_field = if secondary {
                "board_hidden_subgroups"
            } else {
                "board_hidden_groups"
            };
            let collapsed = board::configured(&view.config, collapse_field, &key);
            let k = key.clone();
            body = body.child(
                action(
                    "board-group-collapse",
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
                    setting_action(s, |s| s.toggle_board_group_setting(collapse_field, &k));
                }),
            );
            let k = key.clone();
            body = body.child(
                action("board-group-hide", "Hide group", &phosphor::EYE_SLASH, t).on_press(
                    move |s: &mut State| {
                        setting_action(s, |s| s.toggle_board_group_setting(hide_field, &k));
                    },
                ),
            );
            let k = key.clone();
            body = body.child(
                action(
                    "board-group-before",
                    if secondary { "Move up" } else { "Move left" },
                    if secondary {
                        &phosphor::ARROW_UP
                    } else {
                        &phosphor::ARROW_LEFT
                    },
                    t,
                )
                .on_press(move |s: &mut State| {
                    setting_action(s, |s| s.move_board_group_order(secondary, &k, false));
                }),
            );
            body = body.child(
                action(
                    "board-group-after",
                    if secondary { "Move down" } else { "Move right" },
                    if secondary {
                        &phosphor::ARROW_DOWN
                    } else {
                        &phosphor::ARROW_RIGHT
                    },
                    t,
                )
                .on_press(move |s: &mut State| {
                    setting_action(s, |s| s.move_board_group_order(secondary, &key, true));
                }),
            );
        }
        _ => {}
    }
    if open.pane != MenuPane::Main {
        body = body.child(
            action("board-menu-back", "Back", &phosphor::ARROW_LEFT, t).on_press(
                |s: &mut State| {
                    s.database_views.board_ui.error = None;
                    if let Some(menu) = &mut s.database_views.board_ui.menu {
                        menu.pane = MenuPane::Main;
                    }
                },
            ),
        );
    }
    let anchor = open.anchor.clone();
    let popup = col()
        .key("board-actions-menu")
        .w(menu::WIDTH)
        .max_h(500.)
        .padding(6.)
        .gap(4.)
        .fill(t.menu)
        .radius(8.)
        .border(1., t.line)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .role("menu")
        .label("Board actions")
        .focus_group()
        .focus_return(&anchor)
        .modal(true, close)
        .transition(Transition::opacity())
        .child(body)
        .children(
            s.database_views
                .board_ui
                .error
                .as_ref()
                .map(|error| t.label(error, t.error).w_full().wrap().px(8.)),
        );
    Some(if let Some([x, y]) = open.point {
        el().pin(Some(0.), Some(0.), Some(0.), Some(0.))
            .child(el().key("board-context-anchor").size(0., 0.).absolute(x, y))
            .child(popup.anchored_fit("board-context-anchor", Side::Bottom, Align::Start, 0.))
    } else {
        popup.anchored_fit(anchor, Side::Bottom, Align::Start, 4.)
    })
}
