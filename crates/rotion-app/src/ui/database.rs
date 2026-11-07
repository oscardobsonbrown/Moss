use super::theme::{Theme, button, primary_button};
use crate::state::{Modal, State, Tab, display_value};
use chrono::{Datelike, NaiveDate};
use rotion_core::{Database, Property, PropertyKind, Record, ViewKind};
use rune::*;
use std::collections::BTreeMap;

// shortcut: views use the current record page; replace with complete indexed view projections before release.
pub fn view(s: &State, t: Theme) -> El {
    let Some(tab) = s.tab() else {
        return col();
    };
    let Some(db) = &tab.database else {
        return col();
    };
    let t = Theme {
        family: crate::fonts::family(tab.page.settings.font),
        text_scale: crate::fonts::scale(tab.page.settings),
        ..t
    };
    let mut tabs = row().gap(4.).center_y().grow(1.);
    for (i, item) in db.views.iter().enumerate() {
        let active = tab.view == i;
        tabs = tabs.child(
            button(format!("db-view-{i}"), &item.name, t)
                .fill(if active {
                    t.selected
                } else {
                    Color::TRANSPARENT
                })
                .on_press_tx(move |s: &mut State, tx| {
                    if !s.finish_cell(false) || !s.close_peek() {
                        return;
                    }
                    if let Some(tab) = s.tabs.get_mut(s.active) {
                        tab.view = i;
                        tab.offset = 0;
                    }
                    s.query_records();
                    tx.instant();
                }),
        );
    }
    let search = text_input(
        &s.filter,
        TextStyle::new(13. * t.text_scale, 400.).family(t.family),
    )
    .key("db-filter")
    .single_line()
    .w(160.)
    .h(30.)
    .px(8.)
    .radius(4.)
    .fill(t.sidebar)
    .color(t.text)
    .placeholder("Search records", t.muted)
    .on_edit(|s: &mut State| &mut s.filter)
    .on_input_change(|s: &mut State, tx| {
        if let Some(tab) = s.tabs.get_mut(s.active) {
            tab.offset = 0;
        }
        s.query_records();
        tx.instant();
    });
    let toolbar = row()
        .w_full()
        .gap(6.)
        .center_y()
        .child(tabs)
        .child(search)
        .child(
            button("db-views", "Views", t).on_press_tx(|s: &mut State, tx| {
                s.modal = Some(Modal::Views);
                tx.instant();
            }),
        )
        .child(
            primary_button("db-new", "New", t).on_press_tx(|s: &mut State, _| {
                s.add_record();
            }),
        );
    let kind = db.views.get(tab.view).map(|v| v.kind);
    let body = match kind {
        Some(ViewKind::Table) => table(s, tab, db, t),
        Some(ViewKind::Board) => board(s, tab, db, t),
        Some(ViewKind::Calendar) => calendar(tab, db, t),
        Some(ViewKind::Timeline) => timeline(s, tab, db, t),
        Some(ViewKind::Gallery) => gallery(s, tab, db, t),
        Some(ViewKind::List) => list(s, tab, db, t),
        Some(other) => col().padding(20.).child(t.label(
            format!("{} view is not available yet.", other.label()),
            14.,
            t.muted,
        )),
        None => col()
            .padding(20.)
            .child(t.label("No database view is available.", 14., t.muted)),
    };
    let mut page = row().w_full().center_y().gap(8.).py(8.).child(t.label(
        format!(
            "Showing {}–{}",
            if tab.records.is_empty() {
                0
            } else {
                tab.offset + 1
            },
            tab.offset + tab.records.len() as u64
        ),
        12.,
        t.muted,
    ));
    if tab.offset > 0 {
        page = page.child(
            button("db-previous", "Previous", t).on_press_tx(|s: &mut State, tx| {
                if let Some(tab) = s.tabs.get_mut(s.active) {
                    tab.offset = tab.offset.saturating_sub(100);
                }
                s.query_records();
                tx.instant();
            }),
        );
    }
    if tab.records.len() >= 100 {
        page = page.child(
            button("db-next", "Next", t).on_press_tx(|s: &mut State, tx| {
                if let Some(tab) = s.tabs.get_mut(s.active) {
                    tab.offset = tab.offset.saturating_add(100);
                }
                s.query_records();
                tx.instant();
            }),
        );
    }
    let content = col()
        .key("database")
        .w_full()
        .grow(1.)
        .min_h(0.)
        .gap(10.)
        .px(28.)
        .pt(42.)
        .child(
            text_input(
                &tab.title,
                crate::fonts::page_style(tab.page.settings, 32., 700.),
            )
            .key("database-title")
            .single_line()
            .w_full()
            .color(t.text)
            .placeholder("Untitled database", t.faint)
            .on_edit(|s: &mut State| &mut s.tabs[s.active].title)
            .on_input_change(|s: &mut State, _| s.save_title()),
        )
        .child(el().h(12.))
        .child(toolbar)
        .child(
            col()
                .key("db-body")
                .w_full()
                .grow(1.)
                .min_h(0.)
                .scroll_xy()
                .child(body),
        )
        .child(page)
        .self_center();
    if tab.page.settings.full_width {
        content
    } else {
        content.max_w(1000.)
    }
}

fn properties(db: &Database) -> impl Iterator<Item = &Property> {
    db.properties
        .iter()
        .filter(|p| p.kind != PropertyKind::Title)
}
fn cell_text(record: &Record, prop: &Property) -> String {
    record
        .values
        .get(&prop.id)
        .map(display_value)
        .unwrap_or_default()
}
fn active(s: &State, record: &Record, property: Option<&str>) -> bool {
    s.editing_cell.as_ref().is_some_and(|target| {
        target.record_id == record.id && target.property_id.as_deref() == property
    })
}

fn editor(s: &State, width: f32, t: Theme) -> El {
    text_input(
        &s.cell,
        TextStyle::new(13. * t.text_scale, 400.).family(t.family),
    )
    .key("database-cell-editor")
    .placeholder("Untitled", t.faint)
    .single_line()
    .w(width)
    .h(34.)
    .border(1., t.blue)
    .px(8.)
    .color(t.text)
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
        if event.key == Key::Escape {
            s.finish_cell(false);
            tx.instant();
            return true;
        }
        false
    })
}

fn cell(
    s: &State,
    index: usize,
    record: &Record,
    property: Option<String>,
    text: String,
    width: f32,
    t: Theme,
) -> El {
    if active(s, record, property.as_deref()) {
        return editor(s, width, t);
    }
    let title = property.is_none();
    let text = if title && text.is_empty() {
        "Untitled".into()
    } else {
        text
    };
    row()
        .hover_group()
        .key(format!(
            "cell-{index}-{}",
            property.as_deref().unwrap_or("title")
        ))
        .role("button")
        .label(&text)
        .focusable(true)
        .w(width)
        .h(34.)
        .px(9.)
        .center_y()
        .border(0.5, t.line)
        .hover(t.hover)
        .child(t.label(text, 13., t.text).grow(1.).truncate())
        .children(title.then(|| {
            button(format!("table-open-{index}"), "Open", t)
                .group_opacity(0., 1.)
                .on_press_tx(move |s: &mut State, tx| {
                    s.open_record(index);
                    tx.instant();
                })
        }))
        .on_press_tx(move |s: &mut State, tx| {
            s.edit_record(index, property.clone());
            tx.instant();
        })
}
fn table(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let mut head = row().child(
        row()
            .w(220.)
            .h(34.)
            .px(9.)
            .center_y()
            .border(0.5, t.line)
            .child(t.label("Name", 13., t.muted)),
    );
    for prop in properties(db) {
        head = head.child(
            row()
                .w(160.)
                .h(34.)
                .px(9.)
                .center_y()
                .border(0.5, t.line)
                .child(t.label(&prop.name, 13., t.muted)),
        );
    }
    let mut content = col().child(head);
    for (i, record) in tab.records.iter().enumerate() {
        let mut line = row().child(cell(s, i, record, None, record.title.clone(), 220., t));
        for prop in properties(db) {
            line = line.child(cell(
                s,
                i,
                record,
                Some(prop.id.clone()),
                cell_text(record, prop),
                160.,
                t,
            ));
        }
        content = content.child(line);
    }
    content.child(
        button("table-new", "+ New page", t).on_press_tx(|s: &mut State, tx| {
            s.create_inline(BTreeMap::new(), false);
            tx.instant();
        }),
    )
}
fn card(s: &State, index: usize, record: &Record, db: &Database, t: Theme) -> El {
    if active(s, record, None) {
        return col()
            .w(210.)
            .padding(12.)
            .radius(6.)
            .border(1., t.line)
            .fill(t.bg)
            .child(editor(s, 185., t));
    }
    let mut content = col().gap(5.).child(t.label(
        if record.title.is_empty() {
            "Untitled"
        } else {
            &record.title
        },
        14.,
        t.text,
    ));
    for prop in properties(db) {
        let value = cell_text(record, prop);
        if !value.is_empty() {
            content = content.child(t.label(format!("{}: {}", prop.name, value), 12., t.muted));
        }
    }
    col()
        .key(format!("record-card-{index}"))
        .role("button")
        .label(&record.title)
        .focusable(true)
        .w(210.)
        .padding(12.)
        .radius(6.)
        .border(1., t.line)
        .fill(t.bg)
        .hover(t.hover)
        .child(content)
        .on_press_tx(move |s: &mut State, tx| {
            s.open_record(index);
            tx.instant();
        })
}
fn list(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let mut content = col().w_full();
    for (i, record) in tab.records.iter().enumerate() {
        if active(s, record, None) {
            content = content.child(editor(s, 300., t));
            continue;
        }
        let mut line = row()
            .w_full()
            .h(38.)
            .gap(14.)
            .px(10.)
            .center_y()
            .border(0.5, t.line)
            .child(t.label(
                if record.title.is_empty() {
                    "Untitled"
                } else {
                    &record.title
                },
                14.,
                t.text,
            ));
        for prop in properties(db) {
            let v = cell_text(record, prop);
            if !v.is_empty() {
                line = line.child(t.label(v, 12., t.muted));
            }
        }
        content = content.child(
            line.key(format!("list-{i}"))
                .role("button")
                .label(&record.title)
                .focusable(true)
                .hover(t.hover)
                .on_press_tx(move |s: &mut State, tx| {
                    s.open_record(i);
                    tx.instant();
                }),
        );
    }
    content.child(
        button("list-new", "+ New page", t).on_press_tx(|s: &mut State, tx| {
            s.create_inline(BTreeMap::new(), false);
            tx.instant();
        }),
    )
}
fn gallery(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let mut content = col().gap(12.);
    for (group, records) in tab.records.chunks(3).enumerate() {
        let mut line = row().gap(12.);
        for (j, record) in records.iter().enumerate() {
            line = line.child(card(s, group * 3 + j, record, db, t));
        }
        content = content.child(line);
    }
    content.child(
        button("gallery-new", "+ New page", t).on_press_tx(|s: &mut State, tx| {
            s.create_peek(BTreeMap::new());
            tx.instant();
        }),
    )
}
fn board(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(status) = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status || p.kind == PropertyKind::Select)
    else {
        return col().padding(16.).child(t.label(
            "Add a status or select property to group this board.",
            14.,
            t.muted,
        ));
    };
    let mut groups: Vec<String> = status
        .config
        .get("options")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| {
                    v.as_str()
                        .or_else(|| v.get("name").and_then(|n| n.as_str()))
                })
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    for record in &tab.records {
        let v = cell_text(record, status);
        if !v.is_empty() && !groups.contains(&v) {
            groups.push(v);
        }
    }
    if tab.records.iter().any(|r| cell_text(r, status).is_empty()) || groups.is_empty() {
        groups.push(String::new());
    }
    let mut columns = row().gap(12.);
    for group in groups {
        let matches: Vec<_> = tab
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| cell_text(r, status) == group)
            .collect();
        let name = if group.is_empty() {
            "No status"
        } else {
            &group
        };
        let mut column = col()
            .w(220.)
            .gap(8.)
            .padding(6.)
            .fill(t.sidebar)
            .radius(6.)
            .child(t.label(format!("{}  {}", name, matches.len()), 13., t.muted));
        for (i, record) in matches {
            column = column.child(card(s, i, record, db, t));
        }
        let id = status.id.clone();
        let value = group.clone();
        column = column.child(
            button(format!("board-new-{group}"), "+ New page", t).on_press_tx(
                move |s: &mut State, tx| {
                    let mut values = BTreeMap::new();
                    values.insert(
                        id.clone(),
                        if value.is_empty() {
                            serde_json::Value::Null
                        } else {
                            serde_json::Value::String(value.clone())
                        },
                    );
                    s.create_inline(values, true);
                    tx.instant();
                },
            ),
        );
        columns = columns.child(column);
    }
    columns
}
fn date_property(db: &Database) -> Option<&Property> {
    db.properties.iter().find(|p| p.kind == PropertyKind::Date)
}
fn record_date(record: &Record, property: &Property) -> Option<NaiveDate> {
    let value = record.values.get(&property.id)?;
    let raw = value
        .as_str()
        .or_else(|| value.get("start").and_then(|v| v.as_str()))?;
    NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()
}
fn calendar(tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(prop) = date_property(db) else {
        return col().padding(16.).child(t.label(
            "Add a date property to use Calendar.",
            14.,
            t.muted,
        ));
    };
    let Some(first) = tab.calendar_month.with_day(1) else {
        return col();
    };
    let Some(next) = first.checked_add_months(chrono::Months::new(1)) else {
        return col();
    };
    let count = next.signed_duration_since(first).num_days() as u32;
    let start = first.weekday().num_days_from_sunday();
    let mut content = col().gap(4.).child(
        row()
            .gap(8.)
            .center_y()
            .child(
                button("calendar-prev", "‹", t).on_press_tx(|s: &mut State, tx| {
                    if let Some(tab) = s.tabs.get_mut(s.active) {
                        if let Some(date) = tab
                            .calendar_month
                            .checked_sub_months(chrono::Months::new(1))
                        {
                            tab.calendar_month = date;
                        }
                    }
                    tx.instant();
                }),
            )
            .child(t.label(first.format("%B %Y").to_string(), 16., t.text))
            .child(
                button("calendar-next", "›", t).on_press_tx(|s: &mut State, tx| {
                    if let Some(tab) = s.tabs.get_mut(s.active) {
                        if let Some(date) = tab
                            .calendar_month
                            .checked_add_months(chrono::Months::new(1))
                        {
                            tab.calendar_month = date;
                        }
                    }
                    tx.instant();
                }),
            ),
    );
    let mut weekdays = row();
    for name in ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] {
        weekdays = weekdays.child(
            row()
                .w(130.)
                .h(25.)
                .px(6.)
                .child(t.label(name, 12., t.muted)),
        );
    }
    content = content.child(weekdays);
    for week in 0..(start + count).div_ceil(7) {
        let mut line = row();
        for day in 0..7 {
            let slot = week * 7 + day;
            let mut box_ = col()
                .w(130.)
                .min_h(75.)
                .padding(6.)
                .border(0.5, t.line)
                .gap(3.);
            if slot >= start && slot - start < count {
                let number = slot - start + 1;
                let date = first.with_day(number).unwrap();
                let id = prop.id.clone();
                box_ = box_
                    .key(format!("calendar-day-{date}"))
                    .on_double_press_tx(move |s: &mut State, tx| {
                        let mut values = BTreeMap::new();
                        values.insert(id.clone(), serde_json::Value::String(date.to_string()));
                        s.create_peek(values);
                        tx.instant();
                    })
                    .child(t.label(number.to_string(), 12., t.muted));
                for (i, record) in tab.records.iter().enumerate() {
                    if record_date(record, prop) == Some(date) {
                        box_ = box_.child(
                            button(format!("calendar-{i}"), &record.title, t).on_press_tx(
                                move |s: &mut State, tx| {
                                    s.open_record(i);
                                    tx.instant();
                                },
                            ),
                        );
                    }
                }
            }
            line = line.child(box_);
        }
        content = content.child(line);
    }
    content
}
fn timeline(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(prop) = date_property(db) else {
        return col().padding(16.).child(t.label(
            "Add a date property to use Timeline.",
            14.,
            t.muted,
        ));
    };
    let first = tab.timeline_start;
    let scale = 28.;
    let days = 30;
    let mut header = row().child(row().w(180.).child(t.label("Name", 13., t.muted)));
    for day in 0..days {
        if let Some(date) = first.checked_add_signed(chrono::Duration::days(day)) {
            header = header.child(row().w(scale).child(t.label(
                date.day().to_string(),
                11.,
                t.muted,
            )));
        }
    }
    let controls = row()
        .gap(8.)
        .center_y()
        .child(
            button("timeline-prev", "‹", t).on_press_tx(|s: &mut State, tx| {
                if let Some(tab) = s.tabs.get_mut(s.active) {
                    if let Some(date) = tab
                        .timeline_start
                        .checked_sub_signed(chrono::Duration::days(30))
                    {
                        tab.timeline_start = date;
                    }
                }
                tx.instant();
            }),
        )
        .child(t.label(first.format("%d %b %Y").to_string(), 13., t.muted))
        .child(
            button("timeline-next", "›", t).on_press_tx(|s: &mut State, tx| {
                if let Some(tab) = s.tabs.get_mut(s.active) {
                    if let Some(date) = tab
                        .timeline_start
                        .checked_add_signed(chrono::Duration::days(30))
                    {
                        tab.timeline_start = date;
                    }
                }
                tx.instant();
            }),
        );
    let mut content = col().gap(4.).child(controls).child(header);
    for (i, record) in tab.records.iter().enumerate() {
        let start = record_date(record, prop);
        let end = record
            .values
            .get(&prop.id)
            .and_then(|v| v.get("end"))
            .and_then(|v| v.as_str())
            .and_then(|v| v.get(..10))
            .and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
            .or(start);
        let dated_inline = start.is_some() && active(s, record, None);
        let title = if active(s, record, None) && !dated_inline {
            editor(s, 180., t)
        } else {
            button(format!("timeline-open-{i}"), &record.title, t)
                .w(180.)
                .on_press_tx(move |s: &mut State, tx| {
                    s.open_record(i);
                    tx.instant();
                })
        };
        let mut line = row().h(38.).center_y().border(0.5, t.line).child(title);
        if let (Some(start), Some(end)) = (start, end) {
            let left = start
                .signed_duration_since(first)
                .num_days()
                .max(0)
                .min(days) as f32;
            let right = (end.signed_duration_since(first).num_days() + 1)
                .max(0)
                .min(days) as f32;
            if right > left {
                let bar = row()
                    .w((right - left) * scale)
                    .h(30.)
                    .radius(4.)
                    .fill(t.blue.alpha(0.2))
                    .center_y();
                let bar = if dated_inline {
                    bar.child(editor(s, 180., t).fill(t.bg))
                } else {
                    bar.px(5.)
                        .role("button")
                        .label(&record.title)
                        .focusable(true)
                        .child(
                            t.label(
                                if record.title.is_empty() {
                                    "Untitled"
                                } else {
                                    &record.title
                                },
                                13.,
                                t.text,
                            )
                            .truncate(),
                        )
                        .on_press_tx(move |s: &mut State, tx| {
                            s.open_record(i);
                            tx.instant();
                        })
                };
                line = line.child(el().w(left * scale)).child(bar);
            }
        } else {
            line = line.child(t.label("No date", 12., t.muted));
        }
        content = content.child(line);
    }
    let id = prop.id.clone();
    let mut track = row()
        .key("timeline-create-track")
        .w(days as f32 * scale)
        .h(38.)
        .border(0.5, t.line);
    if let Some((start, end)) = s.timeline_drag {
        let offset = start.signed_duration_since(first).num_days() as f32 * scale;
        let width = (end.signed_duration_since(start).num_days() + 1) as f32 * scale;
        track = track
            .child(el().w(offset))
            .child(el().w(width).h(24.).fill(t.blue.alpha(0.5)).radius(4.));
    }
    let track = track.on_drag(move |s: &mut State, drag, tx| {
        if drag.cancelled() {
            s.timeline_drag = None;
            tx.instant();
            return;
        }
        let a = (drag.start_x / scale).floor().clamp(0., days as f32 - 1.) as i64;
        let b = (drag.x / scale).floor().clamp(0., days as f32 - 1.) as i64;
        if let (Some(start), Some(end)) = (
            first.checked_add_signed(chrono::Duration::days(a.min(b))),
            first.checked_add_signed(chrono::Duration::days(a.max(b))),
        ) {
            if drag.phase == Phase::Up {
                s.timeline_drag = None;
                let mut values = BTreeMap::new();
                values.insert(
                    id.clone(),
                    serde_json::json!({"start":start.to_string(), "end":end.to_string()}),
                );
                s.create_inline(values, false);
            } else {
                s.timeline_drag = Some((start, end));
            }
            tx.instant();
        }
    });
    content = content.child(row().child(el().w(180.)).child(track));
    content.child(
        button("timeline-new", "+ New page", t).on_press_tx(|s: &mut State, tx| {
            s.create_inline(BTreeMap::new(), false);
            tx.instant();
        }),
    )
}
