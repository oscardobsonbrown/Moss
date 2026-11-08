use super::theme::{Theme, label};
use crate::state::State;
use moss_core::PropertyKind;
use rune::*;

fn choice(key: String, name: &str, selected: bool, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(28.)
        .px(8.)
        .gap(6.)
        .center_y()
        .radius(4.)
        .fill(if selected { t.selected } else { t.menu })
        .hover(t.hover)
        .role("menuitemradio")
        .label(name)
        .checked(selected)
        .focusable(true)
        .child(label(name, t.text).grow(1.).truncate())
        .children(selected.then(|| super::icons::glyph("✓", 14., t.text)))
}

pub fn editor_fields(s: &State, t: Theme) -> Option<El> {
    let editor = s.database_property_editor.as_ref()?;
    let draft = &s.advanced_property;
    let mut body = col().w_full().gap(6.);
    match editor.kind {
        PropertyKind::Formula => {
            let input = super::inputs::field_with_style(
                &draft.expression,
                "formula-expression",
                "Formula",
                TextStyle::new(14., 400.).mono().lh(20.),
                t,
            )
            .label("Formula expression")
            .on_edit(|s: &mut State| &mut s.advanced_property.expression)
            .on_input_change(|_: &mut State, tx| tx.instant());
            let preview = if draft.expression.text.trim().is_empty() {
                "Enter a formula.".into()
            } else if let Some(tab) = s.tab() {
                if let Some(db) = &tab.database {
                    match moss_engine::formula::dependencies(&draft.expression.text, &db.properties)
                    {
                        Err(error) => format!("Formula: {error}"),
                        Ok(_) => match tab.records.first() {
                            Some(record) => {
                                let mut values = record.values.clone();
                                if let Some(title) =
                                    db.properties.iter().find(|p| p.kind == PropertyKind::Title)
                                {
                                    values.insert(
                                        title.id.clone(),
                                        serde_json::Value::String(record.title.clone()),
                                    );
                                }
                                match moss_engine::formula::evaluate(
                                    &draft.expression.text,
                                    &values,
                                    &db.properties,
                                ) {
                                    Ok(value) => {
                                        format!("Preview: {}", crate::state::display_value(&value))
                                    }
                                    Err(error) => format!("Formula: {error}"),
                                }
                            }
                            None => "Formula is valid. Add a record to preview it.".into(),
                        },
                    }
                } else {
                    "Database is not available.".into()
                }
            } else {
                "Database is not available.".into()
            };
            body = body
                .child(label("Formula", t.muted))
                .child(input)
                .child(label(preview, t.muted).wrap());
        }
        PropertyKind::Relation => {
            let search = super::inputs::field(
                &draft.database_search,
                "relation-database-search",
                "Search databases",
                t,
            )
            .label("Search databases")
            .on_edit(|s: &mut State| &mut s.advanced_property.database_search)
            .on_input_change(|s: &mut State, tx| {
                s.search_advanced_databases();
                tx.instant();
            });
            let list = col()
                .w_full()
                .max_h(112.)
                .min_h(0.)
                .shrink()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(draft.databases.iter().map(|db| {
                    let id = db.id.clone();
                    let name = db.name.clone();
                    choice(
                        format!("relation-database-{id}"),
                        &db.name,
                        draft.target_database_id.as_deref() == Some(&id),
                        t,
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        s.advanced_property.target_database_id = Some(id.clone());
                        s.advanced_property.target_database_name = Some(name.clone());
                        tx.instant();
                    })
                }));
            body = body
                .child(label("Related database", t.muted))
                .children(
                    draft
                        .target_database_name
                        .as_ref()
                        .map(|name| label(format!("Selected: {name}"), t.text).truncate()),
                )
                .child(search)
                .child(list);
        }
        PropertyKind::Rollup => {
            let relations = s
                .tab()
                .and_then(|tab| tab.database.as_ref())
                .map(|db| {
                    db.properties
                        .iter()
                        .filter(|p| p.kind == PropertyKind::Relation)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let relation_list = col()
                .w_full()
                .max_h(84.)
                .min_h(0.)
                .shrink()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(relations.into_iter().map(|p| {
                    let id = p.id.clone();
                    choice(
                        format!("rollup-relation-{id}"),
                        &p.name,
                        draft.relation_property_id.as_deref() == Some(&id),
                        t,
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        s.advanced_property.relation_property_id = Some(id.clone());
                        s.advanced_property.target_property_id = None;
                        s.refresh_advanced_target_properties();
                        tx.instant();
                    })
                }));
            let properties = col()
                .w_full()
                .max_h(84.)
                .min_h(0.)
                .shrink()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(draft.target_properties.iter().map(|p| {
                    let id = p.id.clone();
                    choice(
                        format!("rollup-property-{id}"),
                        &p.name,
                        draft.target_property_id.as_deref() == Some(&id),
                        t,
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        s.advanced_property.target_property_id = Some(id.clone());
                        tx.instant();
                    })
                }));
            let operations = [
                ("original", "Show original"),
                ("unique", "Show unique"),
                ("count", "Count all"),
                ("count_values", "Count values"),
                ("count_empty", "Count empty"),
                ("percent_empty", "Percent empty"),
                ("sum", "Sum"),
                ("average", "Average"),
                ("min", "Minimum"),
                ("max", "Maximum"),
            ];
            let operations = col()
                .w_full()
                .max_h(84.)
                .min_h(0.)
                .shrink()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .children(operations.into_iter().map(|(id, name)| {
                    choice(
                        format!("rollup-operation-{id}"),
                        name,
                        draft.operation == id,
                        t,
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        s.advanced_property.operation = id.into();
                        tx.instant();
                    })
                }));
            body = body
                .child(label("Relation", t.muted))
                .child(relation_list)
                .child(label("Property", t.muted))
                .child(properties)
                .child(label("Calculate", t.muted))
                .child(operations);
        }
        _ => return None,
    }
    Some(
        body.children(
            draft
                .error
                .as_ref()
                .map(|error| label(error, t.error).wrap()),
        ),
    )
}

pub fn relation_picker(s: &State, t: Theme) -> Option<El> {
    let picker = s.advanced_property.relation_picker.as_ref()?;
    let anchor = if s
        .record_peek
        .as_ref()
        .is_some_and(|peek| peek.record.id == picker.record_id)
    {
        format!("peek-property-{}", picker.property_id)
    } else if s
        .tab()
        .and_then(|tab| tab.record_context.as_ref())
        .is_some_and(|(_, record)| record.id == picker.record_id)
    {
        format!("page-property-{}", picker.property_id)
    } else if s
        .tab()
        .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
        .is_some_and(|view| view.kind == moss_core::ViewKind::Board)
    {
        format!("board-value-{}-{}", picker.record_id, picker.property_id)
    } else {
        let index = s
            .tab()?
            .records
            .iter()
            .position(|record| record.id == picker.record_id)?;
        format!("cell-{index}-{}", picker.property_id)
    };
    let input = super::inputs::field(
        &s.advanced_property.relation_search,
        "relation-record-search",
        "Search records",
        t,
    )
    .label("Search related records")
    .on_edit(|s: &mut State| &mut s.advanced_property.relation_search)
    .on_input_change(|s: &mut State, tx| {
        s.search_relation_candidates();
        tx.instant();
    });
    let selected = col()
        .w_full()
        .max_h(100.)
        .min_h(0.)
        .shrink()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .children(picker.selected.iter().take(50).map(|id| {
            let name = picker
                .selected_names
                .get(id)
                .map(|name| {
                    if name.is_empty() {
                        "Untitled"
                    } else {
                        name.as_str()
                    }
                })
                .unwrap_or("Unavailable record");
            let id = id.clone();
            choice(format!("relation-selected-{id}"), name, true, t).on_press_tx(
                move |s: &mut State, tx| {
                    s.toggle_relation_candidate(&id);
                    tx.instant();
                },
            )
        }));
    let candidates = col()
        .w_full()
        .max_h(180.)
        .min_h(0.)
        .shrink()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .children(picker.candidates.iter().map(|record| {
            let id = record.id.clone();
            let name = if record.title.is_empty() {
                "Untitled"
            } else {
                &record.title
            };
            choice(
                format!("relation-record-{id}"),
                name,
                picker.selected.contains(&id),
                t,
            )
            .on_press_tx(move |s: &mut State, tx| {
                s.toggle_relation_candidate(&id);
                tx.instant();
            })
        }));
    let remaining = picker.selected.len().saturating_sub(50);
    Some(
        col()
            .key("relation-picker")
            .w(280.)
            .max_h(470.)
            .min_h(0.)
            .padding(10.)
            .gap(6.)
            .fill(t.menu)
            .radius(6.)
            .border(1., t.line)
            .shadow(4., 16., Color::BLACK.alpha(0.15))
            .role("dialog")
            .label("Related records")
            .focus_return(anchor.clone())
            .modal(true, |s: &mut State| s.close_relation_picker())
            .transition(Transition::opacity())
            .child(input)
            .child(label("Selected", t.muted))
            .child(selected)
            .children((remaining > 0).then(|| label(format!("{remaining} more selected"), t.muted)))
            .child(label("Records", t.muted))
            .child(candidates)
            .children(picker.error.as_ref().map(|e| label(e, t.error).wrap()))
            .children(picker.save_failed.then(|| {
                super::theme::button("relation-discard", "Discard changes", t)
                    .w_full()
                    .center()
                    .on_press_tx(|s: &mut State, tx| {
                        s.discard_relation_picker();
                        tx.instant();
                    })
            }))
            .child(
                super::theme::button("relation-apply", "Apply", t)
                    .w_full()
                    .center()
                    .fill(t.selected)
                    .on_press_tx(|s: &mut State, tx| {
                        s.save_relation_picker();
                        tx.instant();
                    }),
            )
            .anchored_fit(anchor, Side::Bottom, Align::Start, 2.),
    )
}
