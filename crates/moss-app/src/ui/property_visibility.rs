use super::{
    menu,
    theme::{Theme, button},
};
use crate::state::State;
use moss_core::{Database, PropertyKind, ViewKind};
use rune::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn hidden_ids(config: &Value) -> Option<BTreeSet<String>> {
    match config.get("hidden_property_ids") {
        None | Some(Value::Null) => Some(BTreeSet::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect(),
        _ => None,
    }
}

pub fn visible(db: &Database, view_index: usize, property_id: &str) -> bool {
    if db
        .properties
        .iter()
        .any(|p| p.id == property_id && p.kind == PropertyKind::Title)
    {
        return true;
    }
    db.views
        .get(view_index)
        .and_then(|view| hidden_ids(&view.config))
        .is_none_or(|hidden| !hidden.contains(property_id))
}

pub fn open(s: &mut State) {
    s.property_visibility_open = true;
    s.property_visibility_from_view_settings = false;
    s.property_visibility_search = TextEdit::new("");
    s.property_visibility_search.focused = true;
}

pub fn overlay(s: &State, t: Theme) -> Option<El> {
    if !s.property_visibility_open {
        return None;
    }
    let tab = s.tab()?;
    let db = tab.database.as_ref()?;
    let view = db.views.get(tab.view)?;
    if !matches!(
        view.kind,
        ViewKind::Table
            | ViewKind::Board
            | ViewKind::Calendar
            | ViewKind::Timeline
            | ViewKind::Gallery
            | ViewKind::List
    ) {
        return None;
    }
    let hidden = hidden_ids(&view.config);
    let query = s.property_visibility_search.text.to_lowercase();
    let search = super::inputs::field(
        &s.property_visibility_search,
        "list-properties-search",
        "Search for a property…",
        t,
    )
    .label("Search properties")
    .on_edit(|s: &mut State| &mut s.property_visibility_search)
    .on_input_change(|_: &mut State, tx| tx.instant());
    let header = row()
        .w_full()
        .h(32.)
        .px(10.)
        .center_y()
        .child(t.label("Property visibility", t.text).grow(1.))
        .child(
            row()
                .key("list-properties-close")
                .size(20., 20.)
                .center()
                .radius(4.)
                .hover(t.hover)
                .role("button")
                .label("Close")
                .focusable(true)
                .child(rune::icon(&menu::CLOSE, 16.).color(t.muted))
                .on_press(|s: &mut State| s.property_visibility_open = false),
        );
    let mut body = col().w_full().child(header.mb(4.)).child(search.mb(4.));
    if hidden.is_none() {
        body = body.child(
            t.label(
                "Property visibility cannot be changed: invalid saved settings.",
                t.text,
            )
            .wrap(),
        );
    } else {
        let hidden = hidden.unwrap();
        let properties = moss_core::record_layout::properties(db);
        for is_hidden in [false, true] {
            let matching: Vec<_> = properties
                .iter()
                .copied()
                .filter(|p| hidden.contains(&p.id) == is_hidden)
                .filter(|p| p.name.to_lowercase().contains(&query))
                .collect();
            let heading = if is_hidden {
                "Hidden in view"
            } else {
                "Shown in view"
            };
            let action = if is_hidden { "Show all" } else { "Hide all" };
            body = body.child(
                row()
                    .w_full()
                    .center_y()
                    .px(10.)
                    .pt(8.)
                    .child(t.label(heading, t.muted).grow(1.))
                    .child(
                        button(
                            if is_hidden {
                                "list-properties-show-all"
                            } else {
                                "list-properties-hide-all"
                            },
                            action,
                            t,
                        )
                        .on_press_tx(move |s: &mut State, tx| {
                            set_visibility(s, None, !is_hidden);
                            tx.instant();
                        }),
                    ),
            );
            if !is_hidden {
                if let Some(title) = db.properties.iter().find(|p| {
                    p.kind == PropertyKind::Title && p.name.to_lowercase().contains(&query)
                }) {
                    body = body.child(
                        menu::row_item(
                            "list-property-title",
                            &title.name,
                            super::property_icons::view(title.kind, t.muted),
                            Theme { text: t.muted, ..t },
                        )
                        .role("text")
                        .focusable(false)
                        .hover(t.menu)
                        .press(t.menu)
                        .label(format!("{}: always shown", title.name))
                        .child(t.label("Always shown", t.faint))
                        .child(menu::end(
                            "list-property-title-end",
                            None,
                            t,
                        )),
                    );
                }
            }
            for property in matching {
                let id = property.id.clone();
                let shown = !is_hidden;
                body = body.child(
                    menu::row_item(
                        format!("list-property-{id}"),
                        &property.name,
                        super::property_icons::view(property.kind, t.muted),
                        t,
                    )
                    .role("checkbox")
                    .checked(shown)
                    .child(menu::end(
                        format!("list-property-{id}-end"),
                        Some(if shown { &menu::EYE } else { &menu::EYE_OFF }),
                        t,
                    ))
                    .on_press_tx(move |s: &mut State, tx| {
                        set_visibility(s, Some(&id), shown);
                        tx.instant();
                    }),
                );
            }
        }
    }
    body = body.children(s.error.as_ref().map(|error| t.label(error, t.text).wrap()));
    Some(
        col()
            .key("list-property-visibility")
            .w(menu::WIDTH)
            .max_h(600.)
            .padding(8.)
            .fill(t.menu)
            .border(1., t.line)
            .radius(8.)
            .shadow(4., 16., Color::BLACK.alpha(0.15))
            .role("dialog")
            .label("Property visibility")
            .modal(true, |s: &mut State| s.property_visibility_open = false)
            .child(
                body.min_h(0.)
                    .shrink()
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .instant_changes(),
            )
            .anchored_fit(
                if s.property_visibility_from_view_settings {
                    "db-view-settings"
                } else {
                    "list-properties-button"
                },
                Side::Bottom,
                Align::End,
                4.,
            ),
    )
}

// `hide` changes either the named property or every non-title property.
pub(crate) fn set_visibility(s: &mut State, property: Option<&str>, hide: bool) {
    let Some(tab) = s.tab() else { return };
    let Some(current) = tab.database.as_ref() else {
        return;
    };
    let Some(view) = current.views.get(tab.view) else {
        return;
    };
    let view_id = view.id.clone();
    let kind = view.kind;
    let db_id = current.id.clone();
    let ids: Vec<String> = current
        .properties
        .iter()
        .filter(|p| p.kind != PropertyKind::Title)
        .filter(|p| property.is_none_or(|id| id == p.id))
        .map(|p| p.id.clone())
        .collect();
    if ids.is_empty() {
        return;
    }
    let result = s.store.edit_view_config(&db_id, &view_id, kind, |value| {
        if value.is_null() {
            *value = json!({});
        }
        let mut hidden = hidden_ids(value).ok_or("invalid saved settings")?;
        let config = value.as_object_mut().ok_or("invalid saved settings")?;
        for id in ids {
            if hide {
                hidden.insert(id);
            } else {
                hidden.remove(&id);
            }
        }
        config.insert("hidden_property_ids".into(), json!(hidden));
        Ok(())
    });
    match result {
        Ok(db) => {
            sync_database(s, &db);
            if s.editing_cell.is_none() && s.property_picker.is_none() {
                let selected_hidden = s
                    .selected_cell
                    .as_ref()
                    .and_then(|(_, property)| property.as_deref())
                    .is_some_and(|id| {
                        s.tab()
                            .and_then(|tab| {
                                tab.database
                                    .as_ref()
                                    .map(|current| !visible(current, tab.view, id))
                            })
                            .unwrap_or(false)
                    });
                if selected_hidden {
                    s.selected_cell = None;
                }
            }
            s.error = None;
        }
        Err(e) => s.error = Some(format!("Property visibility is not saved: {e}")),
    }
}

pub(crate) fn sync_database(s: &mut State, db: &Database) {
    for tab in &mut s.tabs {
        if tab
            .database
            .as_ref()
            .is_some_and(|current| current.id == db.id)
        {
            let selected_id = tab
                .database
                .as_ref()
                .and_then(|old| old.views.get(tab.view))
                .map(|v| v.id.clone());
            tab.view = selected_id
                .and_then(|id| db.views.iter().position(|view| view.id == id))
                .unwrap_or(0);
            tab.database = Some(db.clone());
        }
    }
}
