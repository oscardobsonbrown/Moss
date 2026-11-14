use super::theme::{Theme, button};
use crate::state::{State, Tab, display_value};
use chrono::NaiveDate;
use moss_core::{Database, Property, PropertyKind, Record, ViewKind};
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
    let is_gallery = db
        .views
        .get(tab.view)
        .is_some_and(|view| view.kind == ViewKind::Gallery);
    let toolbar = super::database_toolbar::view(s, t);
    let kind = db.views.get(tab.view).map(|v| v.kind);
    let body = match kind {
        Some(ViewKind::Table) => super::table::view(s, tab, db, t),
        Some(ViewKind::Board) => super::board::view(s, tab, db, t),
        Some(ViewKind::Calendar) => super::calendar::view(s, tab, db, t),
        Some(ViewKind::Timeline) => super::timeline::view(s, tab, db, t),
        Some(ViewKind::Gallery) => super::gallery::view(s, tab, db, t),
        Some(ViewKind::List) => list(s, tab, db, t),
        Some(ViewKind::Chart) => super::chart::view(s, tab, db, t),
        Some(ViewKind::Map) => super::map::view(s, tab, db, t),
        Some(ViewKind::Dashboard) => super::dashboard::view(s, tab, db, t),
        Some(other) => col().padding(20.).child(t.label(
            format!("{} view is not available yet.", other.label()),
            t.muted,
        )),
        None => col()
            .padding(20.)
            .child(t.label("No database view is available.", t.muted)),
    };
    let limit = db
        .views
        .get(tab.view)
        .map(crate::records::view_load_limit)
        .unwrap_or(100);
    let page_label = if let Some(window) = &tab.table_window {
        window.displayed_total().map_or_else(
            || "Preparing row count".to_owned(),
            |total| format!("{total} records"),
        )
    } else {
        format!(
            "Showing {}–{}",
            if tab.records.is_empty() {
                0
            } else {
                tab.offset + 1
            },
            tab.offset + tab.records.len() as u64
        )
    };
    let mut page = row()
        .w_full()
        .center_y()
        .gap(8.)
        .py(8.)
        .child(t.label(page_label, t.muted).key("database-page-count"));
    if tab.table_window.is_none() && tab.offset > 0 {
        page = page.child(button("db-previous", "Previous", t).on_press_tx(
            move |s: &mut State, tx| {
                let offset = s.tabs[s.active].offset.saturating_sub(u64::from(limit));
                s.move_record_page(offset);
                tx.instant();
            },
        ));
    }
    if tab.table_window.is_none() && tab.has_more_records {
        page = page.child(
            button("db-next", "Next", t).on_press_tx(move |s: &mut State, tx| {
                let offset = s.tabs[s.active].offset.saturating_add(u64::from(limit));
                s.move_record_page(offset);
                tx.instant();
            }),
        );
    }
    let content = col()
        .key("database")
        .w_full()
        .min_w(0.)
        .grow(1.)
        .min_h(0.)
        .gap(10.)
        .px(28.)
        .pt(if tab.page.cover.is_some() { 20. } else { 42. })
        .child(
            super::inputs::text(
                &tab.title,
                crate::fonts::page_style(tab.page.settings, 32., 700.)
                    .lh(40. * crate::fonts::scale(tab.page.settings)),
                t,
            )
            .key("database-title")
            .w_full()
            .placeholder("Untitled database", t.title_hint)
            .on_edit(|s: &mut State| &mut s.tabs[s.active].title)
            .on_input_change(|s: &mut State, _| s.save_title())
            .on_submit(|s: &mut State, tx| {
                s.tabs[s.active].title.focused = false;
                tx.instant();
            }),
        )
        .child(el().h(12.))
        .child(toolbar)
        .children(if s.record_peek.is_none() {
            super::cell_recovery::view(s, t)
        } else {
            None
        })
        .children(tab.table_window.as_ref().and_then(|window| {
            if let Some(error) = &window.error {
                Some(
                    row()
                        .key("table-window-error")
                        .min_h(30.)
                        .gap(8.)
                        .center_y()
                        .child(t.label(error, t.error))
                        .child(
                            row()
                                .key("table-window-retry")
                                .role("button")
                                .label("Retry")
                                .focusable(true)
                                .child(t.label("Retry", t.blue))
                                .on_press(|s: &mut State| {
                                    s.retry_table_window();
                                }),
                        ),
                )
            } else if window.loading {
                Some(
                    row()
                        .key("table-window-loading")
                        .min_h(30.)
                        .center_y()
                        .child(t.label("Loading rows…", t.muted)),
                )
            } else {
                None
            }
        }))
        .child({
            let viewport = col().key("db-body").w_full().min_w(0.).grow(1.).min_h(0.);
            let viewport = if kind == Some(ViewKind::Board) {
                viewport.on_drawn_layout_tx(|s: &mut State, r, tx| {
                    s.database_views.board_ui.viewport = r;
                    tx.live_only();
                })
            } else if is_gallery {
                viewport.on_drawn_layout_tx(|s: &mut State, r, tx| {
                    s.page_cover.gallery_viewport = r;
                    tx.live_only();
                })
            } else {
                viewport
            };
            if is_gallery || matches!(kind, Some(ViewKind::Map | ViewKind::Calendar)) {
                let mut viewport = viewport.scroll(ScrollSpec {
                    stick_bottom: false,
                });
                if kind == Some(ViewKind::Calendar) {
                    let signal = s.database_views.calendar.scroll.clone();
                    viewport = viewport
                        .on_scroll_offset(move |position| signal.set(position))
                        .on_wheel(|s: &mut State, event, tx| s.calendar_wheel(event, tx));
                }
                viewport.child(body)
            } else {
                let mut viewport = viewport.scroll_xy();
                if kind == Some(ViewKind::Board) {
                    let signal = s.database_views.board_ui.scroll.clone();
                    viewport = viewport.on_scroll_offset(move |position| signal.set(position));
                }
                if let Some(window) = &tab.table_window {
                    let signal = window.scroll.clone();
                    let handle = s
                        .handles
                        .lock()
                        .ok()
                        .and_then(|handles| handles.get(&s.workspace.id).cloned());
                    viewport = viewport.on_scroll_offset(move |at| {
                        if signal.get() != at {
                            signal.set(at);
                            if let Some(handle) = &handle {
                                handle.wake();
                            }
                        }
                    });
                }
                viewport.child(body)
            }
        })
        .children(
            (!matches!(
                kind,
                Some(
                    ViewKind::Board
                        | ViewKind::Calendar
                        | ViewKind::Chart
                        | ViewKind::Map
                        | ViewKind::Dashboard
                )
            ))
            .then_some(page),
        )
        .self_center();
    let content = if tab.page.settings.full_width {
        content
    } else {
        content.max_w(1000.)
    };
    col()
        .w_full()
        .grow(1.)
        .min_h(0.)
        .child(super::page_cover::view(s, &tab.page, t, "database"))
        .child(content)
}

pub(super) fn cell_text(record: &Record, prop: &Property) -> String {
    record
        .values
        .get(&prop.id)
        .map(display_value)
        .unwrap_or_default()
}
pub(super) fn active(s: &State, record: &Record, property: Option<&str>) -> bool {
    s.editing_cell.as_ref().is_some_and(|target| {
        target.record_id == record.id && target.property_id.as_deref() == property
    })
}

pub(super) fn navigation_button(key: &str, name: &str, icon: &str, t: Theme) -> El {
    row()
        .key(key)
        .w(28.)
        .h(30.)
        .center()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(name)
        .focusable(true)
        .child(super::icons::glyph(icon, 14., t.text))
}

pub(super) fn new_page_button(key: impl Into<String>, t: Theme) -> El {
    row()
        .key(key)
        .h(30.)
        .px(9.)
        .gap(4.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label("+ New page")
        .focusable(true)
        .child(super::icons::glyph("+", 14., t.text))
        .child(t.label("New page", t.text))
}

pub(super) fn editor(s: &State, width: f32, t: Theme) -> El {
    super::inputs::invalid(
        super::inputs::field_with_style(
            &s.cell,
            "database-cell-editor",
            "Untitled",
            TextStyle::new(14. * t.text_scale, 400.)
                .family(t.family)
                .lh(20. * t.text_scale),
            t,
        ),
        s.editing_cell.as_ref().is_some_and(|cell| cell.save_failed),
        t,
    )
    .w(width)
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

fn list(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let mut content = col().w_full();
    let properties = moss_core::record_layout::properties(db);
    for (i, record) in tab.records.iter().enumerate() {
        if active(s, record, None) {
            content = content.child(
                row()
                    .gap(6.)
                    .center_y()
                    .px(10.)
                    .child(super::record_icon::view(
                        tab,
                        record,
                        format!("record-icon-list-{i}"),
                        t,
                    ))
                    .child(editor(s, 278., t)),
            );
            continue;
        }
        let mut line = row()
            .w_full()
            .h(38.)
            .gap(14.)
            .px(10.)
            .center_y()
            .border(0.5, t.line)
            .child(super::record_icon::view(
                tab,
                record,
                format!("record-icon-list-{i}"),
                t,
            ))
            .child(t.label(
                if record.title.is_empty() {
                    "Untitled"
                } else {
                    &record.title
                },
                t.text,
            ));
        for prop in properties
            .iter()
            .copied()
            .filter(|prop| super::property_visibility::visible(db, tab.view, &prop.id))
        {
            let value = record.values.get(&prop.id);
            if prop.kind == PropertyKind::Place {
                let record_id = record.id.clone();
                let property_id = prop.id.clone();
                let label = if value.is_none_or(serde_json::Value::is_null) {
                    t.label("Empty", t.faint)
                } else {
                    super::property_value::view(s, prop, value, t)
                };
                line = line.child(
                    row()
                        .key(format!("list-value-{i}-{property_id}"))
                        .role("button")
                        .label(format!("Edit {}", prop.name))
                        .focusable(true)
                        .child(label)
                        .on_press(move |s: &mut State| {
                            s.open_place_picker(&record_id, &property_id)
                        }),
                );
            } else if value.is_some_and(|v| !v.is_null()) {
                line = line.child(
                    super::property_value::view(s, prop, value, t)
                        .key(format!("list-value-{i}-{}", prop.id)),
                );
            }
        }
        content = content.child(
            line.key(format!("list-{i}"))
                .role("button")
                .label(&record.title)
                .focusable(true)
                .hover(t.hover)
                .on_press(move |s: &mut State| {
                    s.open_record(i);
                }),
        );
    }
    content.child(
        new_page_button("list-new", t).on_press_tx(|s: &mut State, tx| {
            s.create_inline(BTreeMap::new(), false);
            tx.instant();
        }),
    )
}
pub(super) fn date_property(db: &Database) -> Option<&Property> {
    db.properties.iter().find(|p| p.kind == PropertyKind::Date)
}
pub(super) fn record_date(record: &Record, property: &Property) -> Option<NaiveDate> {
    let value = record.values.get(&property.id)?;
    let raw = value
        .as_str()
        .or_else(|| value.get("start").and_then(|v| v.as_str()))?;
    NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()
}
pub(super) fn undated_button(tab: &Tab, prop: &Property, prefix: &str, t: Theme) -> El {
    let count = tab
        .records
        .iter()
        .filter(|record| record_date(record, prop).is_none())
        .count();
    button(
        format!("{prefix}-no-date"),
        &format!("No date ({count})"),
        t,
    )
    .on_press(|s: &mut State| {
        s.undated_open = !s.undated_open;
        s.undated_search = TextEdit::new("");
        s.undated_search.focused = s.undated_open;
    })
}

pub fn undated_overlay(s: &State, t: Theme) -> Option<El> {
    let tab = s.tab()?;
    let db = tab.database.as_ref()?;
    let prefix = match db.views.get(tab.view)?.kind {
        ViewKind::Calendar => return super::calendar::undated_panel(s, tab, db, t),
        ViewKind::Timeline => "timeline",
        _ => return None,
    };
    undated_panel(s, tab, date_property(db)?, prefix, t)
}
fn undated_panel(s: &State, tab: &Tab, prop: &Property, prefix: &str, t: Theme) -> Option<El> {
    if !s.undated_open {
        return None;
    }
    let search = super::inputs::field(
        &s.undated_search,
        format!("{prefix}-no-date-search"),
        "Search",
        t,
    )
    .on_edit(|s: &mut State| &mut s.undated_search);
    let query = s.undated_search.text.to_lowercase();
    let mut panel = col()
        .key(format!("{prefix}-no-date-panel"))
        .w(280.)
        .padding(8.)
        .gap(4.)
        .fill(t.menu)
        .radius(8.)
        .border(1., t.line)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .role("dialog")
        .label("Pages without dates")
        .modal(true, |s: &mut State| s.undated_open = false)
        .child(search);
    for (index, record) in tab.records.iter().enumerate() {
        if record_date(record, prop).is_some() || !record.title.to_lowercase().contains(&query) {
            continue;
        }
        let title = if record.title.is_empty() {
            "Untitled"
        } else {
            &record.title
        };
        panel = panel.child(
            row()
                .w_full()
                .center_y()
                .gap(8.)
                .child(t.label(title, t.text).grow(1.).truncate())
                .child(
                    button(format!("{prefix}-undated-open-{index}"), "Open", t).on_press_tx(
                        move |s: &mut State, _| {
                            s.undated_open = false;
                            s.open_record(index);
                        },
                    ),
                ),
        );
    }
    Some(panel.anchored(format!("{prefix}-no-date"), Side::Bottom, Align::End, 4.))
}
