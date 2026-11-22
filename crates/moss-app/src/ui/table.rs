use super::database::{active, cell_text};
use super::theme::Theme;
use crate::state::{State, Tab, display_value};
use moss_core::{Database, PropertyKind, Record};
use rune::*;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

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
    let picker_open = s.property_picker().is_some_and(|picker| {
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
        } else if title {
            let label = super::property_value::scalar_label(&text, t)
                .font(
                    TextStyle::new(14. * t.text_scale, 600.)
                        .family(t.family)
                        .lh(20. * t.text_scale),
                )
                .key(key)
                .min_w(0.)
                .grow(1.);
            if wrap { label.wrap() } else { label.truncate() }
        } else if wrap
            && prop.is_none_or(|p| {
                !matches!(
                    p.kind,
                    PropertyKind::Status
                        | PropertyKind::Select
                        | PropertyKind::MultiSelect
                        | PropertyKind::Relation
                )
            })
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
        let mut contents = wrapper
            .px(10.)
            .pr(if title { 4. } else { 10. })
            .gap(if title { 6. } else { 0. })
            .center_y()
            .hover(if title { Color::TRANSPARENT } else { t.hover })
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
                    .fill(t.bg)
                    .border(1., t.line)
                    .shadow(1., 3., Color::BLACK.alpha(0.08))
                    .hover(t.hover)
                    .press(t.selected)
                    .role("button")
                    .label("Open page")
                    .focusable(true)
                    .child(super::icons::glyph("open-peek", 15., t.muted))
                    .child(
                        rune::text("OPEN")
                            .font(TextStyle::new(14., 500.))
                            .color(t.muted),
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
                if let Some(id) = property.as_deref() {
                    if let Some(kind) = s.open_property_editor(&record_id, id) {
                        if !crate::property_controls::is_popup_picker_kind(kind)
                            && !matches!(
                                kind,
                                PropertyKind::Relation | PropertyKind::Formula | PropertyKind::Rollup
                            )
                        {
                            tx.instant();
                        }
                    }
                } else {
                    s.dismiss_cell_picker();
                    if let Some(i) = s.tab().and_then(|tab| {
                        tab.records.iter().position(|record| record.id == record_id)
                    }) {
                        s.edit_record(i, None);
                    }
                    tx.instant();
                }
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
        wrapper = contents;
    }
    wrapper
}
pub(super) fn view(s: &State, tab: &Tab, db: &Database, t: Theme) -> Vec<El> {
    let view = db.views.get(tab.view);
    let config = view.map(|view| &view.config);
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
    let width = columns
        .iter()
        .map(|p| crate::table_columns::width(s, db, tab.view, p))
        .sum::<f32>()
        + 40.;
    let mut children = vec![
        table_header(s, db, tab.view, t, frozen, &columns)
            .sticky_top(0.)
            .raise(),
    ];
    let window = tab.table_window.as_ref();
    let cached_len = window.map_or(tab.records.len(), |window| {
        window.window_len.min(tab.records.len())
    });
    let cache_start = window.map_or(0, |window| window.start as usize);
    let total = window.map_or(cached_len, |window| {
        window.total.unwrap_or(window.window_len as u64) as usize
    });
    let cache_end = cache_start.saturating_add(cached_len).min(total);
    let (start, end) = match window {
        Some(window) if !window.visible.is_empty() => {
            let visible_start = window.visible.start.min(total);
            let visible_end = window.visible.end.min(total);
            if visible_end < cache_start || visible_start > cache_end {
                (visible_start, visible_end)
            } else {
                (cache_start.min(visible_start), cache_end.max(visible_end))
            }
        }
        _ => (cache_start, cache_end),
    };
    let trailing = total.saturating_sub(end);
    let group = view
        .and_then(|view| view.query.group_by.as_deref())
        .and_then(|id| db.properties.iter().find(|p| p.id == id));
    let mut entries = Vec::with_capacity(end.saturating_sub(start));
    let mut last_group = None;
    for ordinal in start..end {
        let local = ordinal
            .checked_sub(cache_start)
            .filter(|index| *index < cached_len);
        let Some((index, record)) =
            local.and_then(|index| tab.records.get(index).map(|record| (index, record)))
        else {
            entries.push(VirtualRow::Pending { ordinal });
            continue;
        };
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
    for (index, record) in tab.records.iter().enumerate().skip(cached_len) {
        entries.push(VirtualRow::Pinned {
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
    let footer_count = window
        .map(crate::table_window::TableWindow::displayed_total)
        .unwrap_or_else(|| Some(tab.records.len() as u64));
    let footer_width = rune::memo::window_layout_with("db-body", |r| r[2])
        .filter(|vw| *vw > 0.)
        .map_or(width, |vw| vw.min(width));
    entries.push(VirtualRow::Footer {
        count: footer_count,
        width_px: footer_width.round() as u32,
    });
    let tail_offset = trailing as f32 * 38.;
    let entries = std::rc::Rc::new(entries);
    let keys = entries.clone();
    let key_version = entries.clone();
    let mut layout_hash = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(db)
        .unwrap_or_default()
        .hash(&mut layout_hash);
    tab.relation_titles.hash(&mut layout_hash);
    t.family.hash(&mut layout_hash);
    t.text_scale.to_bits().hash(&mut layout_hash);
    t.bg.r.to_bits().hash(&mut layout_hash);
    let layout_version = layout_hash.finish();
    let column_ids: Vec<String> = columns.iter().map(|property| property.id.clone()).collect();
    let fixed_height = window.is_some();
    if start > 0 {
        children.push(el().h(start as f32 * 38.));
    }
    children.push(
        col()
            .key("table-virtual-rows")
            .w(width)
            .pb(trailing as f32 * 38.)
            .growth(GrowthSpec::default())
            .virtual_rows::<State>(
                entries.len(),
                38.,
                move |_, index| match &keys[index] {
                    VirtualRow::Group { id, .. } => format!("table-virtual-group-{id}").into(),
                    VirtualRow::Pending { ordinal } => format!("table-pending-{ordinal}").into(),
                    VirtualRow::Record { index, .. } if fixed_height => {
                        format!("table-window-slot-{index}").into()
                    }
                    VirtualRow::Record { id, .. } | VirtualRow::Pinned { id, .. } => {
                        format!("table-record-{id}").into()
                    }
                    VirtualRow::Summary => "table-summary".into(),
                    VirtualRow::Footer { .. } => format!("table-footer-{}", keys.len()).into(),
                },
                move |s, index| {
                    let Some(tab) = s.tab() else { return el() };
                    let Some(db) = tab.database.as_ref() else {
                        return el();
                    };
                    let columns: Vec<_> = column_ids
                        .iter()
                        .filter_map(|id| db.properties.iter().find(|p| &p.id == id))
                        .collect();
                    let pinned = matches!(&entries[index], VirtualRow::Pinned { .. });
                    match &entries[index] {
                        VirtualRow::Group { index, label, .. } => table_group(*index, label, t),
                        VirtualRow::Pending { .. } => row().h(38.).w_full().border(0.5, t.line),
                        VirtualRow::Record { index, id } | VirtualRow::Pinned { index, id } => {
                            let current = tab
                                .records
                                .get(*index)
                                .filter(|record| record.id == *id)
                                .map(|_| *index)
                                .or_else(|| tab.records.iter().position(|record| record.id == *id));
                            current
                                .map(|i| {
                                    let record = &tab.records[i];
                                    let mut record_hash = std::collections::hash_map::DefaultHasher::new();
                                    serde_json::to_string(record).unwrap_or_default().hash(&mut record_hash);
                                    let record_version = record_hash.finish();
                                    let interactive = pinned
                                        || s.database_views.table_move.is_some()
                                        || s.database_views.table_resize.is_some()
                                        || s.editing_cell
                                            .as_ref()
                                            .is_some_and(|target| target.record_id == record.id)
                                        || s.selected_cell
                                            .as_ref()
                                            .is_some_and(|(id, _)| id == &record.id)
                                        || s.cell_picker
                                            .as_ref()
                                            .is_some_and(|picker| picker.record_id() == record.id);
                                    let render = || {
                                        table_record(
                                            s,
                                            tab,
                                            db,
                                            i,
                                            wrapped,
                                            show_icons,
                                            frozen,
                                            t,
                                            &columns,
                                            layout_version,
                                            record_version,
                                        )
                                    };
                                    let line = if interactive {
                                        render()
                                    } else {
                                        let version = (
                                            layout_version,
                                            i,
                                            record_version,
                                            tab.record_icons.get(&record.id),
                                            tab.record_has_content.get(&record.id),
                                        );
                                        rune::memo(
                                            format!("table-record-{}", record.id),
                                            version,
                                            render,
                                        )
                                    };
                                    if pinned {
                                        line.offset(0., tail_offset).offset_motion(Anim::INSTANT)
                                    } else if fixed_height {
                                        line.h(38.).max_h(38.).clip_rounded(0.)
                                    } else {
                                        line
                                    }
                                })
                                .unwrap_or_else(el)
                        }
                        VirtualRow::Summary => row()
                            .child(table_summary(s, tab, db, t, frozen, &columns))
                            .offset(0., tail_offset)
                            .offset_motion(Anim::INSTANT),
                        VirtualRow::Footer { count, width_px } => row()
                            .child(table_footer(*count, *width_px as f32, t))
                            .offset(0., tail_offset)
                            .offset_motion(Anim::INSTANT),
                    }
                },
            )
            .virtual_rows_version(&*key_version),
    );
    children
}

fn visible_columns(db: &Database, view: usize) -> Vec<&moss_core::Property> {
    moss_core::record_layout::columns_for_view(db, view)
        .into_iter()
        .filter(|p| super::property_visibility::visible(db, view, &p.id))
        .collect()
}

fn frozen_column(db: &Database, view: usize, columns: &[&moss_core::Property]) -> Option<usize> {
    let id = db.views.get(view)?.config.get("freeze_through")?.as_str()?;
    let ordered = moss_core::record_layout::columns_for_view(db, view);
    let anchor = ordered.iter().position(|p| p.id == id)?;
    columns.iter().rposition(|p| {
        ordered
            .iter()
            .position(|original| original.id == p.id)
            .is_some_and(|index| index <= anchor)
    })
}

fn drag_column(
    item: El,
    s: &State,
    db: &Database,
    view: usize,
    property: &moss_core::Property,
    columns: &[&moss_core::Property],
    background: Color,
) -> El {
    let (offset, dragged) =
        crate::table_columns::drag_offset(s, db, view, property, columns).unwrap_or((0., false));
    let item = item.offset(offset, 0.).offset_motion(if dragged {
        Anim::INSTANT
    } else {
        tok::motion::SNAP_EXIT.into()
    });
    if dragged {
        item.fill(background).raise()
    } else {
        item
    }
}

fn table_header(
    s: &State,
    db: &Database,
    view: usize,
    t: Theme,
    frozen: Option<usize>,
    columns: &[&moss_core::Property],
) -> El {
    let mut head = row().key("table-header").fill(t.sidebar);
    let rects: Vec<_> = columns
        .iter()
        .filter_map(|p| {
            let key = if p.kind == PropertyKind::Title {
                "table-header-title".to_owned()
            } else {
                format!("table-header-{}", p.id)
            };
            rune::memo::window_layout_with(&key, |rect| [rect[0], 0., rect[2], 0.])
                .map(|rect| (p.id.clone(), rect))
        })
        .collect();
    for (column, prop) in columns.iter().enumerate() {
        let title = prop.kind == PropertyKind::Title;
        let id = prop.id.clone();
        let resize_id = id.clone();
        let zones = rects.clone();
        let mut header = super::database_properties::header(
            prop,
            title,
            crate::table_columns::width(s, db, view, prop),
            t,
        )
        .child(
            el().key(format!("table-resize-{id}"))
                .w(8.)
                .pin(None, Some(0.), Some(0.), Some(0.))
                .cursor(Cursor::ResizeColumn)
                .hover_group()
                .child(
                    el().key(format!("table-resize-line-{id}"))
                        .w(2.)
                        .h_full()
                        .absolute(3., 0.)
                        .fill(t.blue)
                        .group_opacity(0., 1.),
                )
                .on_drag(move |s: &mut State, drag, tx| {
                    s.resize_table_column(&resize_id, drag);
                    tx.instant();
                }),
        )
        .on_drag(move |s: &mut State, drag, tx| {
            s.move_table_column(&id, drag, &zones);
            tx.animated();
        });
        if frozen.is_some_and(|end| column <= end) {
            header = header
                .fill(t.sidebar)
                .sticky_left(
                    columns[..column]
                        .iter()
                        .map(|p| crate::table_columns::width(s, db, view, p))
                        .sum(),
                )
                .raise();
        }
        header = drag_column(header, s, db, view, prop, columns, t.sidebar);
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
    columns: &[&moss_core::Property],
    layout_version: u64,
    record_version: u64,
) -> El {
    let record = &tab.records[i];
    let interactive = s
        .editing_cell
        .as_ref()
        .is_some_and(|target| target.record_id == record.id)
        || s.selected_cell
            .as_ref()
            .is_some_and(|(id, _)| id == &record.id)
        || s.cell_picker
            .as_ref()
            .is_some_and(|picker| picker.record_id() == record.id);
    let mut line = row();
    for (column, prop) in columns.iter().enumerate() {
        let title = prop.kind == PropertyKind::Title;
        let value = if title {
            record.title.clone()
        } else {
            String::new()
        };
        let icon = (title && show_icons).then_some(tab);
        let width = crate::table_columns::width(s, db, tab.view, prop);
        let render = || {
            cell(
                s,
                i,
                record,
                (!title).then(|| prop.id.clone()),
                value,
                width,
                wrapped,
                icon,
                t,
            )
        };
        let mut item = if interactive {
            render()
        } else {
            let version = (
                layout_version,
                record_version,
                width.to_bits(),
                tab.record_icons.get(&record.id),
                tab.record_has_content.get(&record.id),
            );
            rune::memo(
                format!("cell-{i}-{}", if title { "title" } else { &prop.id }),
                version,
                render,
            )
        };
        if frozen.is_some_and(|end| column <= end) {
            item = item
                .fill(t.bg)
                .sticky_left(
                    columns[..column]
                        .iter()
                        .map(|p| crate::table_columns::width(s, db, tab.view, p))
                        .sum(),
                )
                .raise();
        }
        item = drag_column(item, s, db, tab.view, prop, columns, t.bg);
        line = line.child(item);
    }
    line
}

fn table_footer(count: Option<u64>, width: f32, t: Theme) -> El {
    let count_label = count.map_or_else(
        || "Preparing row count".to_owned(),
        |total| format!("{total} records"),
    );
    let new_button = row()
        .key("table-new")
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
        });
    row()
        .w(width)
        .h(30.)
        .center_y()
        .sticky_left(0.)
        .child(new_button)
        .child(el().grow(1.))
        .child(
            t.label(count_label, t.muted)
                .key("database-page-count")
                .pr(10.),
        )
}

#[derive(Clone, Hash)]
enum VirtualRow {
    Pending {
        ordinal: usize,
    },
    Group {
        index: usize,
        id: String,
        label: String,
    },
    Record {
        index: usize,
        id: String,
    },
    Pinned {
        index: usize,
        id: String,
    },
    Summary,
    Footer {
        count: Option<u64>,
        width_px: u32,
    },
}

fn table_group(index: usize, value: &str, t: Theme) -> El {
    row()
        .key(format!("table-group-{index}"))
        .h(32.)
        .center_y()
        .px(10.)
        .child(t.label(if value.is_empty() { "No value" } else { value }, t.muted))
}

fn table_summary(
    s: &State,
    tab: &Tab,
    db: &Database,
    t: Theme,
    frozen: Option<usize>,
    columns: &[&moss_core::Property],
) -> El {
    let config = db.views.get(tab.view).map(|view| &view.config);
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
            .w(crate::table_columns::width(s, db, tab.view, prop))
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
                        .map(|p| crate::table_columns::width(s, db, tab.view, p))
                        .sum(),
                )
                .raise();
        }
        item = drag_column(item, s, db, tab.view, prop, columns, t.bg);
        summaries = summaries.child(item);
    }
    summaries
}
