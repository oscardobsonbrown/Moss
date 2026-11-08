use super::{menu, phosphor, theme::Theme};
use crate::{database_properties::MetadataPane, state::State};
use rune::*;

fn item(key: &str, name: &str, icon: &'static Icon, t: Theme) -> El {
    menu::row_item(key, name, rune::icon(icon, 16.).color(t.muted), t)
}

fn rule(t: Theme) -> El {
    el().w_full().h(1.).my(4.).fill(t.line)
}

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let state = s.metadata_menu.as_ref()?;
    let db = s.metadata_database(&state.database_id)?;
    let property = db.properties.iter().find(|p| p.id == state.property_id)?;
    let mut body = col()
        .w_full()
        .min_h(0.)
        .shrink()
        .gap(2.)
        .scroll(ScrollSpec {
            stick_bottom: false,
        });
    match state.pane {
        MetadataPane::Main => {
            body = body
                .child(
                    item("metadata-rename", "Rename", &phosphor::NOTE_PENCIL, t)
                        .on_press(|s: &mut State| s.edit_metadata_property(false, true)),
                )
                .child(
                    item(
                        "metadata-edit",
                        "Edit property",
                        &phosphor::SLIDERS_HORIZONTAL,
                        t,
                    )
                    .on_press(|s: &mut State| s.edit_metadata_property(false, false)),
                )
                .child(rule(t))
                .child(
                    item(
                        "metadata-visibility",
                        "Property visibility",
                        &phosphor::EYE_SLASH,
                        t,
                    )
                    .child(menu::end(
                        "metadata-visibility-arrow",
                        Some(&phosphor::CARET_RIGHT),
                        t,
                    ))
                    .on_press(|s: &mut State| {
                        if let Some(menu) = &mut s.metadata_menu {
                            menu.pane = MetadataPane::Visibility;
                        }
                    }),
                )
                .child(
                    item("metadata-delete", "Delete property", &phosphor::TRASH, t)
                        .on_press(|s: &mut State| s.edit_metadata_property(true, false)),
                )
                .child(rule(t))
                .child(
                    item(
                        "metadata-layout",
                        "Customise layout",
                        &phosphor::SQUARES_FOUR,
                        t,
                    )
                    .on_press(|s: &mut State| {
                        if let Some(menu) = &mut s.metadata_menu {
                            menu.pane = MetadataPane::Layout;
                        }
                    }),
                );
        }
        MetadataPane::Visibility => {
            body = body
                .child(
                    item(
                        "metadata-back",
                        "Property visibility",
                        &phosphor::CARET_LEFT,
                        t,
                    )
                    .on_press(|s: &mut State| {
                        if let Some(menu) = &mut s.metadata_menu {
                            menu.pane = MetadataPane::Main;
                        }
                    }),
                )
                .child(rule(t));
            let current = moss_core::record_layout::layout(db)
                .and_then(|layout| layout.visibility.get(&property.id).cloned())
                .unwrap_or_else(|| "always".into());
            for (mode, name) in [
                ("always", "Always show"),
                ("hide_empty", "Hide when empty"),
                ("hidden", "Always hide"),
            ] {
                let selected = current == mode;
                body = body.child(
                    item(
                        &format!("metadata-visibility-{mode}"),
                        name,
                        &phosphor::EYE,
                        t,
                    )
                    .role("menuitemradio")
                    .checked(selected)
                    .children(selected.then(|| {
                        menu::end(format!("metadata-check-{mode}"), Some(&phosphor::CHECK), t)
                    }))
                    .on_press(move |s: &mut State| s.set_metadata_visibility(mode)),
                );
            }
        }
        MetadataPane::Layout => {
            body = body
                .child(
                    item(
                        "metadata-back",
                        "Customise layout",
                        &phosphor::CARET_LEFT,
                        t,
                    )
                    .on_press(|s: &mut State| {
                        if let Some(menu) = &mut s.metadata_menu {
                            menu.pane = MetadataPane::Main;
                        }
                    }),
                )
                .child(rule(t));
            let properties = moss_core::record_layout::properties(db);
            let layout = moss_core::record_layout::layout(db);
            for (index, prop) in properties.iter().enumerate() {
                let id = prop.id.clone();
                let selected = layout
                    .as_ref()
                    .and_then(|layout| layout.visibility.get(&prop.id));
                let property = menu::row_item(
                    format!("metadata-layout-{}", prop.id),
                    &prop.name,
                    super::property_icons::view(prop.kind, t.muted),
                    t,
                )
                .grow(1.)
                .min_w(0.)
                .children(selected.is_some_and(|v| v == "hidden").then(|| {
                    menu::end(
                        format!("metadata-layout-hidden-{}", prop.id),
                        Some(&phosphor::EYE_SLASH),
                        t,
                    )
                }))
                .on_press(move |s: &mut State| {
                    if let Some(menu) = &mut s.metadata_menu {
                        menu.property_id = id.clone();
                        menu.pane = MetadataPane::Visibility;
                    }
                });
                let mut controls = row().center_y();
                if index > 0 {
                    let db_id = db.id.clone();
                    let id = prop.id.clone();
                    let before = properties[index - 1].id.clone();
                    controls = controls.child(
                        row()
                            .key(format!("metadata-layout-up-{id}"))
                            .size(24., 28.)
                            .center()
                            .radius(4.)
                            .hover(t.hover)
                            .role("button")
                            .label(format!("Move {} up", prop.name))
                            .focusable(true)
                            .child(rune::icon(&phosphor::ARROW_UP, 14.).color(t.muted))
                            .on_press(move |s: &mut State| {
                                s.move_metadata_property(&db_id, &id, Some(&before))
                            }),
                    );
                }
                if index + 1 < properties.len() {
                    let db_id = db.id.clone();
                    let id = prop.id.clone();
                    let before = properties.get(index + 2).map(|p| p.id.clone());
                    controls = controls.child(
                        row()
                            .key(format!("metadata-layout-down-{id}"))
                            .size(24., 28.)
                            .center()
                            .radius(4.)
                            .hover(t.hover)
                            .role("button")
                            .label(format!("Move {} down", prop.name))
                            .focusable(true)
                            .child(rune::icon(&phosphor::ARROW_DOWN, 14.).color(t.muted))
                            .on_press(move |s: &mut State| {
                                s.move_metadata_property(&db_id, &id, before.as_deref())
                            }),
                    );
                }
                body = body.child(
                    row()
                        .w_full()
                        .h(32.)
                        .center_y()
                        .child(property)
                        .child(controls),
                );
            }
        }
    }
    if let Some(error) = &state.error {
        body = body.child(t.label(error, t.error).wrap().px(8.));
    }
    Some(
        col()
            .key("metadata-menu")
            .w(230.)
            .max_h(400.)
            .min_h(0.)
            .padding(6.)
            .fill(t.menu)
            .radius(9.)
            .border(1., t.line)
            .shadow(4., 16., Color::BLACK.alpha(0.18))
            .role("menu")
            .label("Property options")
            .focus_group()
            .focus_return(state.anchor.clone())
            .modal(true, |s: &mut State| s.metadata_menu = None)
            .transition(Transition::opacity())
            .child(body)
            .anchored_fit(state.anchor.clone(), Side::Bottom, Align::Start, 2.),
    )
}
