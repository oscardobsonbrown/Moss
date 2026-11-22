use super::theme::Theme;
use crate::state::State;
use moss_core::{Database, PageSettings, PropertyKind, Record};
use rune::*;
use std::rc::Rc;

pub fn view(
    s: &State,
    db: &Database,
    record: &Record,
    settings: PageSettings,
    property_width: f32,
    prefix: &'static str,
    t: Theme,
) -> El {
    let scale = crate::fonts::scale(settings);
    let mut content = col().w_full().min_w(0.).gap(4.);
    let layout = moss_core::record_layout::layout(db).unwrap_or_default();
    let properties = moss_core::record_layout::properties(db);
    let visible: Vec<_> = properties
        .iter()
        .copied()
        .filter(|p| layout.visible(p, record))
        .collect();
    let zones: Rc<Vec<(String, f32)>> = Rc::new(
        visible
            .iter()
            .filter_map(|p| {
                let id = p.id.clone();
                rune::memo::layout_with(&format!("{prefix}-property-name-{id}"), move |r| {
                    (id.clone(), r[1] + r[3] / 2.)
                })
            })
            .collect(),
    );
    for prop in &visible {
        let id = prop.id.clone();
        let record_id = record.id.clone();
        let value_key = if prefix == "page" && prop.kind == PropertyKind::Place {
            format!("page-place-{id}")
        } else {
            format!("{prefix}-property-{id}")
        };
        let active = s.editing_cell.as_ref().is_some_and(|cell| {
            cell.record_id == record_id && cell.property_id.as_deref() == Some(id.as_str())
        });
        let property_value = record.values.get(&id);
        let displayed = super::property_value::text(s, prop, property_value);
        let picker_kind = crate::property_controls::is_popup_picker_kind(prop.kind);
        let relation_kind = prop.kind == PropertyKind::Relation;
        let computed_kind = matches!(prop.kind, PropertyKind::Formula | PropertyKind::Rollup);
        let invalid_checkbox = prop.kind == PropertyKind::Checkbox
            && property_value.is_some_and(|v| !v.is_null() && !v.is_boolean());
        let value = if invalid_checkbox && !active {
            row()
                .h(34.)
                .px(7.)
                .center_y()
                .child(t.label("Invalid checkbox value", t.muted))
        } else if prop.kind == PropertyKind::Checkbox && !active {
            let checked = property_value
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let id = id.clone();
            let record_id = record_id.clone();
            row()
                .key(value_key.clone())
                .w_full()
                .h(34.)
                .px(7.)
                .center_y()
                .radius(4.)
                .hover(t.hover)
                .role("checkbox")
                .label(&prop.name)
                .checked(checked)
                .focusable(true)
                .child(
                    row()
                        .size(16., 16.)
                        .center()
                        .radius(3.)
                        .border(1., if checked { t.blue } else { t.muted })
                        .fill(if checked { t.blue } else { Color::TRANSPARENT })
                        .children(checked.then(|| super::icons::glyph("✓", 12., Color::WHITE))),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    s.toggle_table_checkbox(&record_id, &id);
                    tx.instant();
                })
        } else if computed_kind {
            row()
                .key(value_key.clone())
                .w_full()
                .h(34.)
                .px(7.)
                .center_y()
                .child(if displayed.is_empty() {
                    t.label("Empty", t.faint)
                } else {
                    super::property_value::view(s, prop, property_value, t)
                })
        } else if active && !picker_kind && !relation_kind {
            super::inputs::invalid(
                super::inputs::field_with_style(
                    &s.cell,
                    format!("{prefix}-cell-{id}"),
                    "Empty",
                    crate::fonts::page_style(settings, 14., 400.).lh(20. * scale),
                    t,
                ),
                s.editing_cell.as_ref().is_some_and(|cell| cell.save_failed),
                t,
            )
            .label(&prop.name)
            .on_edit(|s: &mut State| &mut s.cell)
            .on_input_change(|s: &mut State, _| {
                s.save_cell();
            })
            .on_input_key(|s: &mut State, event, tx| {
                if event.key == Key::Escape {
                    s.finish_cell(false);
                    tx.instant();
                    return true;
                }
                false
            })
            .on_submit(|s: &mut State, tx| {
                s.finish_cell(false);
                tx.instant();
            })
        } else {
            row()
                .key(value_key.clone())
                .w_full()
                .h(34.)
                .px(7.)
                .center_y()
                .radius(4.)
                .hover(t.hover)
                .role("button")
                .label(format!("{}: {}", prop.name, displayed))
                .focusable(true)
                .child(if displayed.is_empty() {
                    t.label("Empty", t.faint)
                } else {
                    super::property_value::view(s, prop, property_value, t)
                })
                .on_press_tx(move |s: &mut State, tx| {
                    if let Some(kind) = s.open_property_editor(&record_id, &id) {
                        if !crate::property_controls::is_popup_picker_kind(kind) {
                            tx.instant();
                        }
                    }
                })
        };
        content = content.child(
            row()
                .key(format!("{prefix}-property-row-{}", prop.id))
                .w_full()
                .min_w(0.)
                .min_h(34.)
                .center_y()
                .child(super::property_icons::metadata_name(
                    s,
                    db,
                    prop,
                    property_width,
                    prefix,
                    zones.clone(),
                    t,
                ))
                .child(value.grow(1.).min_w(0.)),
        );
    }
    let hidden = properties.len() - visible.len();
    if hidden > 0 {
        if let Some(prop) = properties.first() {
            let db_id = db.id.clone();
            let property_id = prop.id.clone();
            content = content.child(
                super::theme::button(
                    format!("{prefix}-hidden-properties"),
                    format!("{hidden} more properties"),
                    t,
                )
                .on_press(move |s: &mut State| {
                    s.open_metadata_menu(
                        &db_id,
                        &property_id,
                        &format!("{prefix}-hidden-properties"),
                    );
                    if let Some(menu) = &mut s.metadata_menu {
                        menu.pane = crate::database_properties::MetadataPane::Layout;
                    }
                }),
            );
        }
    }
    let add_db_id = db.id.clone();
    let add_anchor = format!("{prefix}-add-property");
    content = content.child(
        row()
            .key(add_anchor.clone())
            .h(30.)
            .pr(8.)
            .gap(8.)
            .center_y()
            .radius(4.)
            .hover(t.hover)
            .press(t.selected)
            .role("button")
            .label("Add a property")
            .focusable(true)
            .child(
                el().size(16., 20.)
                    .center()
                    .child(rune::icon(&super::phosphor::PLUS, 14.).color(t.muted)),
            )
            .child(t.label("Add a property", t.muted))
            .on_press_tx(move |s: &mut State, tx| {
                s.open_metadata_add_property(&add_db_id, &add_anchor);
                tx.instant();
            }),
    );
    content
}
