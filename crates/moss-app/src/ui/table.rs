use super::database::{active, cell_text};
use super::theme::Theme;
use crate::state::{State, Tab, display_value};
use moss_core::{Database, PropertyKind, Record};
use rune::*;
use std::collections::BTreeMap;

fn cell(
    s: &State,
    index: usize,
    record: &Record,
    property: Option<String>,
    text: String,
    width: f32,
    wrap: bool,
    page_icon: Option<&Tab>,
    t: Theme,
) -> El {
    let editing = active(s, record, property.as_deref());
    let selected = s
        .selected_cell
        .as_ref()
        .is_some_and(|(id, prop)| id == &record.id && prop.as_deref() == property.as_deref());
    let picker_open = s.property_picker.as_ref().is_some_and(|picker| {
        picker.record_id == record.id && Some(picker.property_id.as_str()) == property.as_deref()
    });
    let title = property.is_none();
    let prop = property.as_ref().and_then(|id| {
        s.tab()?
            .database
            .as_ref()?
            .properties
            .iter()
            .find(|p| &p.id == id)
    });
    let text = if let Some(prop) = prop {
        super::property_value::text(s, prop, record.values.get(&prop.id))
    } else if title && text.is_empty() {
        "Untitled".into()
    } else {
        text
    };
    let mut wrapper = row()
        .hover_group()
        .key(format!(
            "cell-{index}-{}",
            property.as_deref().unwrap_or("title")
        ))
        .w(width)
        .min_h(38.)
        .border(0.5, t.line);
    if editing && !picker_open {
        let style = TextStyle::new(14. * t.text_scale, 400.)
            .family(t.family)
            .lh(20. * t.text_scale);
        let input = if wrap {
            super::inputs::text(&s.cell, style, t)
        } else {
            super::inputs::line(&s.cell, style, t)
        }
        .key("database-cell-editor")
        .placeholder(if title { "Untitled" } else { "" }, t.input_hint)
        .grow(1.)
        .invalid(s.editing_cell.as_ref().is_some_and(|cell| cell.save_failed))
        .on_edit(|s: &mut State| &mut s.cell)
        .on_input_change(|s: &mut State, tx| {
            s.save_cell();
            tx.instant();
        })
        .on_submit(|s: &mut State, tx| {
            s.finish_cell(true);
            tx.instant();
        })
        .on_input_key(|s: &mut State, event, tx| {
            if event.key == Key::Tab {
                s.table_edit_tab(event.mods.shift);
                tx.instant();
                return true;
            }
            if event.key == Key::Escape {
                s.finish_cell(false);
                tx.instant();
                return true;
            }
            false
        });
        wrapper = wrapper.child(
            row()
                .w_full()
                .min_h(37.)
                .px(10.)
                .pr(if title { 4. } else { 10. })
                .gap(if title { 6. } else { 0. })
                .center_y()
                .border(
                    2.,
                    if s.editing_cell.as_ref().is_some_and(|cell| cell.save_failed) {
                        t.error
                    } else {
                        t.blue
                    },
                )
                .children(page_icon.map(|tab| {
                    super::record_icon::view(tab, record, format!("record-icon-table-{index}"), t)
                }))
                .child(input)
                .children(title.then(|| el().w(70.))),
        );
    } else {
        let record_id = record.id.clone();
        let checkbox = prop.is_some_and(|p| p.kind == PropertyKind::Checkbox);
        let checkbox_value = property.as_ref().and_then(|id| record.values.get(id));
        let valid_checkbox =
            checkbox_value.is_none_or(|value| value.is_null() || value.is_boolean());
        let checked = checkbox_value
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let picker_kind = prop.is_some_and(|p| {
            matches!(
                p.kind,
                PropertyKind::Date | PropertyKind::Status | PropertyKind::Select
            )
        });
        let relation = prop.is_some_and(|p| p.kind == PropertyKind::Relation);
        let place = prop.is_some_and(|p| p.kind == PropertyKind::Place);
        let computed =
            prop.is_some_and(|p| matches!(p.kind, PropertyKind::Formula | PropertyKind::Rollup));
        let key = format!(
            "cell-text-{index}-{}",
            property.as_deref().unwrap_or("title")
        );
        let value = if checkbox && !valid_checkbox {
            super::property_value::scalar_label("Invalid checkbox value", t)
                .key(key)
                .truncate()
        } else if checkbox {
            row()
                .key(key)
                .size(16., 16.)
                .center()
                .radius(3.)
                .border(1., if checked { t.blue } else { t.muted })
                .fill(if checked { t.blue } else { Color::TRANSPARENT })
                .children(checked.then(|| super::icons::glyph("✓", 12., Color::WHITE)))
        } else if wrap
            && prop.is_none_or(|p| !matches!(p.kind, PropertyKind::Status | PropertyKind::Select))
        {
            super::property_value::scalar_label(&text, t)
                .key(key)
                .min_w(0.)
                .grow(1.)
                .wrap()
        } else if let Some(prop) = prop {
            super::property_value::view(s, prop, record.values.get(&prop.id), t).key(key)
        } else {
            super::property_value::scalar_label(&text, t)
                .key(key)
                .grow(1.)
                .truncate()
        };
        let mut contents = row()
            .w_full()
            .min_h(37.)
            .px(10.)
            .pr(if title { 4. } else { 10. })
            .gap(if title { 6. } else { 0. })
            .center_y()
            .hover(t.hover)
            .role("button")
            .label(&text)
            .focusable(true)
            .children(page_icon.map(|tab| {
                super::record_icon::view(tab, record, format!("record-icon-table-{index}"), t)
            }))
            .child(value)
            .children(title.then(|| {
                row()
                    .key(format!("table-open-{index}"))
                    .size(70., 24.)
                    .center()
                    .gap(5.)
                    .radius(7.)
                    .fill(t.selected)
                    .border(1., t.line)
                    .hover(t.hover)
                    .press(t.selected)
                    .role("button")
                    .label("Open page")
                    .focusable(true)
                    .child(super::icons::glyph("open-peek", 15., t.muted))
                    .child(
                        rune::text("OPEN")
                            .font(TextStyle::new(14., 500.))
                            .color(t.text),
                    )
                    .group_opacity(if selected { 1. } else { 0. }, 1.)
                    .on_press({
                        let id = record.id.clone();
                        move |s: &mut State| {
                            if let Some(i) = s.tab().and_then(|tab| {
                                tab.records.iter().position(|record| record.id == id)
                            }) {
                                s.open_record(i);
                            }
                        }
                    })
            }))
            .on_press_tx(move |s: &mut State, tx| {
                s.selected_cell = None;
                if checkbox {
                    if let Some(id) = property.as_deref() {
                        s.toggle_table_checkbox(&record_id, id);
                    }
                } else if place {
                    if let Some(id) = property.as_deref() {
                        s.open_place_picker(&record_id, id);
                    }
                    tx.instant();
                    return;
                } else if relation {
                    if let Some(id) = property.as_deref() {
                        s.open_relation_picker(&record_id, id);
                    }
                    return;
                } else if computed {
                    return;
                } else if picker_kind {
                    if let Some(id) = property.as_deref() {
                        s.open_property_picker(&record_id, id);
                    }
                    return;
                } else {
                    s.dismiss_property_picker();
                    if let Some(i) = s.tab().and_then(|tab| {
                        tab.records.iter().position(|record| record.id == record_id)
                    }) {
                        s.edit_record(i, property.clone());
                    }
                }
                tx.instant();
            });
        if checkbox && valid_checkbox {
            contents = contents
                .role("checkbox")
                .checked(checked)
                .label(prop.map(|p| p.name.as_str()).unwrap_or("Checkbox"));
        }
        if selected {
            contents = contents.border(2., t.blue);
        }
        wrapper = wrapper.child(contents);
    }
    wrapper
}
pub(super) fn view(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let view = db.views.get(tab.view);
    let config = view.map(|v| &v.config);
    let wrapped = config
        .and_then(|v| v.get("wrap_cells"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let show_icons = config
        .and_then(|v| v.get("show_page_icon"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let columns = visible_columns(db, tab.view);
    let frozen = frozen_column(db, tab.view, &columns);
    if let Some(window) = &tab.table_window {
        let total = window.total.unwrap_or(window.window_len as u64) as usize;
        let start = window.visible.start.min(total);
        let end = window.visible.end.max(start).min(total);
        let mut content = col()
            .child(table_header(db, tab.view, t, frozen).sticky_top(0.).raise())
            .child(el().h(start as f32 * 38.));
        let mut visible_rows = col();
        for ordinal in start..end {
            let local = ordinal
                .checked_sub(window.start as usize)
                .filter(|index| *index < window.window_len && *index < tab.records.len());
            visible_rows = visible_rows.child(match local {
                Some(index) => table_record(s, tab, db, index, false, show_icons, frozen, t)
                    .h(38.)
                    .max_h(38.)
                    .clip_rounded(0.),
                None => row()
                    .key(format!("table-pending-{ordinal}"))
                    .h(38.)
                    .w_full()
                    .border(0.5, t.line),
            });
        }
        content = content
            .child(visible_rows)
            .child(el().h((total - end) as f32 * 38.));
        for index in window.window_len..tab.records.len() {
            content = content.child(table_record(
                s, tab, db, index, false, show_icons, frozen, t,
            ));
        }
        if config
            .and_then(|v| v.get("calculations"))
            .and_then(|v| v.as_object())
            .is_some_and(|v| !v.is_empty())
        {
            content = content.child(table_summary(tab, db, t, frozen));
        }
        return content.child(table_footer(t));
    }
    if tab.records.len() > 40 {
        let mut entries = vec![VirtualRow::Header];
        let group = view
            .and_then(|v| v.query.group_by.as_deref())
            .and_then(|id| db.properties.iter().find(|p| p.id == id));
        let mut last_group = None;
        for (index, record) in tab.records.iter().enumerate() {
            if let Some(group) = group {
                let value = if group.kind == PropertyKind::Title {
                    record.title.clone()
                } else {
                    cell_text(record, group)
                };
                if last_group.as_ref() != Some(&value) {
                    entries.push(VirtualRow::Group {
                        index,
                        id: record.id.clone(),
                        label: value.clone(),
                    });
                    last_group = Some(value);
                }
            }
            entries.push(VirtualRow::Record {
                index,
                id: record.id.clone(),
            });
        }
        if config
            .and_then(|v| v.get("calculations"))
            .and_then(|v| v.as_object())
            .is_some_and(|v| !v.is_empty())
        {
            entries.push(VirtualRow::Summary);
        }
        entries.push(VirtualRow::Footer);
        let width = columns
            .iter()
            .map(|p| {
                if p.kind == PropertyKind::Title {
                    280.
                } else {
                    200.
                }
            })
            .sum::<f32>()
            + 40.;
        let entries = std::rc::Rc::new(entries);
        let keys = entries.clone();
        return col()
            .key("table-virtual-rows")
            .w(width)
            .growth(GrowthSpec::default())
            .virtual_rows::<State>(
                entries.len(),
                38.,
                move |_, index| match &keys[index] {
                    VirtualRow::Header => "table-virtual-header".into(),
                    VirtualRow::Group { id, .. } => format!("table-virtual-group-{id}").into(),
                    VirtualRow::Record { id, .. } => format!("table-record-{id}").into(),
                    VirtualRow::Summary => "table-virtual-summary".into(),
                    VirtualRow::Footer => "table-virtual-footer".into(),
                },
                move |s, index| {
                    let Some(tab) = s.tab() else {
                        return el();
                    };
                    let Some(db) = tab.database.as_ref() else {
                        return el();
                    };
                    let config = db.views.get(tab.view).map(|view| &view.config);
                    let wrapped = config
                        .and_then(|v| v.get("wrap_cells"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let show_icons = config
                        .and_then(|v| v.get("show_page_icon"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    let columns = visible_columns(db, tab.view);
                    let frozen = frozen_column(db, tab.view, &columns);
                    match &entries[index] {
                        VirtualRow::Header => row().child(table_header(db, tab.view, t, frozen)),
                        VirtualRow::Group { index, label, .. } => table_group(*index, label, t),
                        VirtualRow::Record { index, id } => {
                            let current = tab
                                .records
                                .get(*index)
                                .filter(|record| record.id == *id)
                                .map(|_| *index)
                                .or_else(|| tab.records.iter().position(|record| record.id == *id));
                            current
                                .map(|i| {
                                    table_record(s, tab, db, i, wrapped, show_icons, frozen, t)
                                })
                                .unwrap_or_else(el)
                        }
                        VirtualRow::Summary => row().child(table_summary(tab, db, t, frozen)),
                        VirtualRow::Footer => row().child(table_footer(t)),
                    }
                },
            );
    }
    let head = table_header(db, tab.view, t, frozen);
    let mut content = col().child(head);
    let group = view
        .and_then(|v| v.query.group_by.as_deref())
        .and_then(|id| db.properties.iter().find(|p| p.id == id));
    let mut last_group: Option<String> = None;
    for (i, record) in tab.records.iter().enumerate() {
        if let Some(group) = group {
            let value = if group.kind == PropertyKind::Title {
                record.title.clone()
            } else {
                cell_text(record, group)
            };
            if last_group.as_ref() != Some(&value) {
                content = content.child(table_group(i, &value, t));
                last_group = Some(value);
            }
        }
        let line = table_record(s, tab, db, i, wrapped, show_icons, frozen, t);
        content = content.child(line);
    }
    if config
        .and_then(|v| v.get("calculations"))
        .and_then(|v| v.as_object())
        .is_some_and(|v| !v.is_empty())
    {
        content = content.child(table_summary(tab, db, t, frozen));
    }
    content.child(table_footer(t))
}

fn visible_columns(db: &Database, view: usize) -> Vec<&moss_core::Property> {
    moss_core::record_layout::columns(db)
        .into_iter()
        .filter(|p| super::property_visibility::visible(db, view, &p.id))
        .collect()
}

fn frozen_column(db: &Database, view: usize, columns: &[&moss_core::Property]) -> Option<usize> {
    let id = db.views.get(view)?.config.get("freeze_through")?.as_str()?;
    let ordered = moss_core::record_layout::columns(db);
    let anchor = ordered.iter().position(|p| p.id == id)?;
    columns.iter().rposition(|p| {
        ordered
            .iter()
            .position(|original| original.id == p.id)
            .is_some_and(|index| index <= anchor)
    })
}

fn table_header(db: &Database, view: usize, t: Theme, frozen: Option<usize>) -> El {
    let columns = visible_columns(db, view);
    let mut head = row().key("table-header").fill(t.sidebar);
    for (column, prop) in columns.iter().enumerate() {
        let title = prop.kind == PropertyKind::Title;
        let mut header = super::database_properties::header(prop, title, t);
        if frozen.is_some_and(|end| column <= end) {
            header = header
                .fill(t.sidebar)
                .sticky_left(
                    columns[..column]
                        .iter()
                        .map(|p| {
                            if p.kind == PropertyKind::Title {
                                280.
                            } else {
                                200.
                            }
                        })
                        .sum(),
                )
                .raise();
        }
        head = head.child(header);
    }
    head = head.child(super::database_properties::add_button(t));
    head
}

fn table_record(
    s: &State,
    tab: &Tab,
    db: &Database,
    i: usize,
    wrapped: bool,
    show_icons: bool,
    frozen: Option<usize>,
    t: Theme,
) -> El {
    let columns = visible_columns(db, tab.view);
    let record = &tab.records[i];
    let mut line = row();
    for (column, prop) in columns.iter().enumerate() {
        let title = prop.kind == PropertyKind::Title;
        let value = if title {
            record.title.clone()
        } else {
            cell_text(record, prop)
        };
        let icon = (title && show_icons).then_some(tab);
        let mut item = cell(
            s,
            i,
            record,
            (!title).then(|| prop.id.clone()),
            value,
            if title { 280. } else { 200. },
            wrapped,
            icon,
            t,
        );
        if frozen.is_some_and(|end| column <= end) {
            item = item
                .fill(t.bg)
                .sticky_left(
                    columns[..column]
                        .iter()
                        .map(|p| {
                            if p.kind == PropertyKind::Title {
                                280.
                            } else {
                                200.
                            }
                        })
                        .sum(),
                )
                .raise();
        }
        line = line.child(item);
    }
    line
}

fn table_footer(t: Theme) -> El {
    row()
        .key("table-new")
        .self_start()
        .h(30.)
        .px(9.)
        .gap(5.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label("New page")
        .focusable(true)
        .child(super::icons::glyph("+", 16., t.muted))
        .child(t.label("New page", t.muted))
        .on_press_tx(|s: &mut State, tx| {
            s.create_inline(BTreeMap::new(), false);
            tx.instant();
        })
}

#[derive(Clone)]
enum VirtualRow {
    Header,
    Group {
        index: usize,
        id: String,
        label: String,
    },
    Record {
        index: usize,
        id: String,
    },
    Summary,
    Footer,
}

fn table_group(index: usize, value: &str, t: Theme) -> El {
    row()
        .key(format!("table-group-{index}"))
        .h(32.)
        .center_y()
        .px(10.)
        .child(t.label(if value.is_empty() { "No value" } else { value }, t.muted))
}

fn table_summary(tab: &Tab, db: &Database, t: Theme, frozen: Option<usize>) -> El {
    let config = db.views.get(tab.view).map(|view| &view.config);
    let columns = visible_columns(db, tab.view);
    let mut summaries = row().key("table-summaries");
    for (column, prop) in columns.iter().enumerate() {
        let operation = config
            .and_then(|v| v.get("calculations"))
            .and_then(|v| v.get(&prop.id))
            .and_then(|v| v.as_str());
        let value = match tab.summary_values.get(&prop.id) {
            Some(crate::table_summaries::SummaryValue::Ready(value)) if value.is_null() => {
                "—".to_owned()
            }
            Some(crate::table_summaries::SummaryValue::Ready(value)) => display_value(value),
            Some(crate::table_summaries::SummaryValue::Error(error)) => {
                format!("Error: {error}")
            }
            _ if operation.is_some() => "Calculating…".to_owned(),
            _ => String::new(),
        };
        let id = prop.id.clone();
        let mut item = row()
            .key(format!("table-summary-{id}"))
            .w(if prop.kind == PropertyKind::Title {
                280.
            } else {
                200.
            })
            .h(32.)
            .px(10.)
            .center_y()
            .fill(t.bg)
            .role("button")
            .label(if operation.is_some() {
                format!("Calculation: {value}")
            } else {
                "Calculate".into()
            })
            .focusable(true)
            .children(operation.map(|op| {
                let name = match op {
                    "count_all" => "Count all",
                    "count_values" => "Count values",
                    "count_empty" => "Count empty",
                    "count_unique" => "Count unique",
                    "sum" => "Sum",
                    "average" => "Average",
                    "min" => "Minimum",
                    "max" => "Maximum",
                    _ => "Calculation",
                };
                t.label(format!("{name}: {value}"), t.muted).truncate()
            }))
            .on_press(move |s: &mut State| {
                s.open_database_property(Some(&id));
                if let Some(e) = &mut s.database_property_editor {
                    e.pane = crate::database_properties::PropertyPane::Calculate;
                }
            });
        if frozen.is_some_and(|end| column <= end) {
            item = item
                .sticky_left(
                    columns[..column]
                        .iter()
                        .map(|p| {
                            if p.kind == PropertyKind::Title {
                                280.
                            } else {
                                200.
                            }
                        })
                        .sum(),
                )
                .raise();
        }
        summaries = summaries.child(item);
    }
    summaries
}
