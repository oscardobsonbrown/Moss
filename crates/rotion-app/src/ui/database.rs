use super::theme::{Theme, button, label, primary_button};
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
                    if let Some(tab) = s.tabs.get_mut(s.active) {
                        tab.view = i;
                        tab.offset = 0;
                    }
                    s.query_records();
                    tx.instant();
                }),
        );
    }
    let search = text_input(&s.filter, TextStyle::new(13., 400.))
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
        Some(ViewKind::Table) => table(tab, db, t),
        Some(ViewKind::Board) => board(tab, db, t),
        Some(ViewKind::Calendar) => calendar(tab, db, t),
        Some(ViewKind::Timeline) => timeline(tab, db, t),
        Some(ViewKind::Gallery) => gallery(tab, db, t),
        Some(ViewKind::List) => list(tab, db, t),
        Some(other) => col().padding(20.).child(label(
            format!("{} view is not available yet.", other.label()),
            14.,
            t.muted,
        )),
        None => col()
            .padding(20.)
            .child(label("No database view is available.", 14., t.muted)),
    };
    let mut page = row().w_full().center_y().gap(8.).py(8.).child(label(
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
    col()
        .key("database")
        .w_full()
        .grow(1.)
        .min_h(0.)
        .gap(10.)
        .px(28.)
        .pt(42.)
        .child(
            text_input(&tab.title, TextStyle::new(32., 700.))
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
fn cell(index: usize, property: Option<String>, text: String, width: f32, t: Theme) -> El {
    let key = format!("cell-{index}-{}", property.as_deref().unwrap_or("title"));
    row()
        .key(key)
        .role("button")
        .label(&text)
        .focusable(true)
        .w(width)
        .h(34.)
        .px(9.)
        .center_y()
        .border(0.5, t.line)
        .hover(t.hover)
        .child(label(text, 13., t.text))
        .on_press_tx(move |s: &mut State, _| {
            s.edit_record(index, property.clone());
        })
}
fn table(tab: &Tab, db: &Database, t: Theme) -> El {
    let mut head = row().child(
        row()
            .w(220.)
            .h(34.)
            .px(9.)
            .center_y()
            .border(0.5, t.line)
            .child(label("Name", 13., t.muted)),
    );
    for prop in properties(db) {
        head = head.child(
            row()
                .w(160.)
                .h(34.)
                .px(9.)
                .center_y()
                .border(0.5, t.line)
                .child(label(&prop.name, 13., t.muted)),
        );
    }
    let mut content = col().child(head);
    for (i, record) in tab.records.iter().enumerate() {
        let mut line = row().child(cell(i, None, record.title.clone(), 220., t));
        for prop in properties(db) {
            line = line.child(cell(
                i,
                Some(prop.id.clone()),
                cell_text(record, prop),
                160.,
                t,
            ));
        }
        content = content.child(line);
    }
    content.child(
        button("table-new", "+ New page", t).on_press_tx(|s: &mut State, _| {
            s.add_record();
        }),
    )
}
fn card(index: usize, record: &Record, db: &Database, t: Theme) -> El {
    let mut content = col().gap(5.).child(label(&record.title, 14., t.text));
    for prop in properties(db) {
        let value = cell_text(record, prop);
        if !value.is_empty() {
            content = content.child(label(format!("{}: {}", prop.name, value), 12., t.muted));
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
        .on_press_tx(move |s: &mut State, _| {
            s.edit_record(index, None);
        })
}
fn list(tab: &Tab, db: &Database, t: Theme) -> El {
    let mut content = col().w_full();
    for (i, record) in tab.records.iter().enumerate() {
        let mut line = row()
            .w_full()
            .h(38.)
            .gap(14.)
            .px(10.)
            .center_y()
            .border(0.5, t.line)
            .child(label(&record.title, 14., t.text));
        for prop in properties(db) {
            let v = cell_text(record, prop);
            if !v.is_empty() {
                line = line.child(label(v, 12., t.muted));
            }
        }
        content = content.child(
            line.key(format!("list-{i}"))
                .role("button")
                .label(&record.title)
                .focusable(true)
                .hover(t.hover)
                .on_press_tx(move |s: &mut State, _| {
                    s.edit_record(i, None);
                }),
        );
    }
    content.child(
        button("list-new", "+ New page", t).on_press_tx(|s: &mut State, _| {
            s.add_record();
        }),
    )
}
fn gallery(tab: &Tab, db: &Database, t: Theme) -> El {
    let mut content = col().gap(12.);
    for chunk in tab.records.chunks(3).enumerate() {
        let (group, records) = chunk;
        let mut line = row().gap(12.);
        for (j, record) in records.iter().enumerate() {
            line = line.child(card(group * 3 + j, record, db, t));
        }
        content = content.child(line);
    }
    content.child(
        button("gallery-new", "+ New page", t).on_press_tx(|s: &mut State, _| {
            s.add_record();
        }),
    )
}
fn board(tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(status) = db
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status || p.kind == PropertyKind::Select)
    else {
        return col().padding(16.).child(label(
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
            .child(label(format!("{}  {}", name, matches.len()), 13., t.muted));
        for (i, record) in matches {
            column = column.child(card(i, record, db, t));
        }
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
        return col().padding(16.).child(label(
            "Add a date property to use Calendar.",
            14.,
            t.muted,
        ));
    };
    let mut months: BTreeMap<(i32, u32), Vec<(usize, NaiveDate)>> = BTreeMap::new();
    let mut undated = Vec::new();
    for (i, record) in tab.records.iter().enumerate() {
        if let Some(date) = record_date(record, prop) {
            months
                .entry((date.year(), date.month()))
                .or_default()
                .push((i, date));
        } else {
            undated.push(i);
        }
    }
    let mut content = col().gap(14.);
    for ((year, month), entries) in months {
        let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
            continue;
        };
        let Some(next) = first.checked_add_months(chrono::Months::new(1)) else {
            continue;
        };
        let count = next.signed_duration_since(first).num_days() as u32;
        let start = first.weekday().num_days_from_sunday();
        content = content.child(label(first.format("%B %Y").to_string(), 16., t.text));
        let mut weekdays = row();
        for name in ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] {
            weekdays = weekdays.child(row().w(130.).h(25.).px(6.).child(label(name, 12., t.muted)));
        }
        content = content.child(weekdays);
        let weeks = (start + count).div_ceil(7);
        for week in 0..weeks {
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
                    box_ = box_.child(label(number.to_string(), 12., t.muted));
                    for (i, date) in &entries {
                        if date.day() == number {
                            let i = *i;
                            let r = &tab.records[i];
                            box_ = box_.child(
                                button(format!("calendar-{i}"), &r.title, t).on_press_tx(
                                    move |s: &mut State, _| {
                                        s.edit_record(i, None);
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
    }
    if !undated.is_empty() {
        content = content.child(label("No date", 13., t.muted));
        for i in undated {
            content = content.child(card(i, &tab.records[i], db, t));
        }
    }
    if tab.records.is_empty() {
        content = content.child(label("No records on this page.", 14., t.muted));
    }
    content
}
fn timeline(tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(prop) = date_property(db) else {
        return col().padding(16.).child(label(
            "Add a date property to use Timeline.",
            14.,
            t.muted,
        ));
    };
    let mut records: Vec<_> = tab
        .records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let start = record_date(r, prop);
            let end = r
                .values
                .get(&prop.id)
                .and_then(|v| v.get("end"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.get(..10))
                .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
            (
                i,
                start,
                end.filter(|end| start.is_some_and(|start| *end >= start))
                    .or(start),
            )
        })
        .collect();
    records.sort_by_key(|(_, start, _)| (start.is_none(), *start));
    let first = records.iter().filter_map(|(_, start, _)| *start).min();
    let last = records.iter().filter_map(|(_, _, end)| *end).max();
    let scale = first
        .zip(last)
        .map(|(first, last)| {
            (900. / (last.signed_duration_since(first).num_days() + 1) as f32).min(28.)
        })
        .unwrap_or(28.);
    let mut content =
        col()
            .gap(4.)
            .child(label(format!("Timeline by {}", prop.name), 14., t.muted));
    if let (Some(first), Some(last)) = (first, last) {
        content = content.child(label(format!("{} — {}", first, last), 12., t.muted));
    }
    for (i, start, end) in records {
        let record = &tab.records[i];
        let mut line = row().gap(8.).center_y().h(38.).border(0.5, t.line).child(
            row()
                .w(150.)
                .px(8.)
                .child(label(&record.title, 13., t.text)),
        );
        if let (Some(first), Some(start), Some(end)) = (first, start, end) {
            let offset = start.signed_duration_since(first).num_days() as f32 * scale;
            let width = ((end.signed_duration_since(start).num_days() + 1) as f32 * scale).max(8.);
            line = line
                .child(el().w(offset))
                .child(row().w(width).h(18.).radius(4.).fill(t.blue));
            line = line.child(label(
                if start == end {
                    start.to_string()
                } else {
                    format!("{} – {}", start, end)
                },
                12.,
                t.muted,
            ));
        } else {
            line = line.child(label("No date", 12., t.muted));
        }
        content = content.child(
            line.key(format!("timeline-{i}"))
                .role("button")
                .label(&record.title)
                .focusable(true)
                .hover(t.hover)
                .on_press_tx(move |s: &mut State, _| {
                    s.edit_record(i, None);
                }),
        );
    }
    if tab.records.is_empty() {
        content = content.child(label("No records on this page.", 14., t.muted));
    }
    content
}
