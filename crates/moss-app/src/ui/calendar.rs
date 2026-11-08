use super::{
    database::navigation_button,
    menu, phosphor,
    theme::{Theme, button},
};
use crate::{
    calendar_view,
    state::{State, Tab},
};
use chrono::{Datelike, Duration, NaiveDate};
use moss_core::{Database, Record};
use rune::*;
use std::collections::BTreeMap;

fn create(s: &mut State, property: &str, date: NaiveDate) {
    s.create_peek(BTreeMap::from([(
        property.to_owned(),
        serde_json::json!(date.to_string()),
    )]));
}

pub fn view(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(view) = db.views.get(tab.view) else {
        return col();
    };
    let Some(prop) = calendar_view::date_property(db, view) else {
        return col().padding(16.).child(t.label(
            "Choose a date property in Calendar layout settings.",
            t.muted,
        ));
    };
    let Some((first, last)) = calendar_view::bounds(tab, view, s.prefs.start_week_on_monday) else {
        return col();
    };
    let is_week = calendar_view::week(view);
    let projection = s.database_views.calendar.projection_for(&db.id, &view.id);
    let records = projection
        .map(|p| p.records.as_slice())
        .unwrap_or(&tab.records);
    let undated = projection.map(|p| p.undated_count).unwrap_or_else(|| {
        records
            .iter()
            .filter(|r| calendar_view::date_range(r, prop).is_none())
            .count() as u64
    });
    let today = chrono::Local::now().date_naive();
    let label = if is_week {
        format!("{} – {}", first.format("%d %b"), last.format("%d %b %Y"))
    } else {
        tab.calendar_month.format("%B %Y").to_string()
    };
    let header = row()
        .key("calendar-header")
        .w_full()
        .center_y()
        .gap(4.)
        .child(t.label(label, t.text).grow(1.))
        .child(
            button(
                "calendar-no-date",
                if projection.is_none() && s.database_views.calendar.loading {
                    "No date (…)".to_owned()
                } else {
                    format!("No date ({undated})")
                },
                t,
            )
            .on_press(|s: &mut State| {
                s.database_views.calendar.suppress_press = false;
                s.undated_open = !s.undated_open;
                s.undated_search = TextEdit::new("");
                s.undated_search.focused = s.undated_open;
            }),
        )
        .child(
            navigation_button("calendar-prev", "Previous", "‹", t).on_press_tx(
                move |s: &mut State, tx| {
                    if let Some(tab) = s.tabs.get_mut(s.active) {
                        let next = if is_week {
                            tab.calendar_month.checked_sub_signed(Duration::days(7))
                        } else {
                            tab.calendar_month
                                .checked_sub_months(chrono::Months::new(1))
                        };
                        if let Some(next) = next {
                            tab.calendar_month = next;
                        }
                    }
                    s.save_preferences();
                    tx.instant();
                },
            ),
        )
        .child(
            button("calendar-today", "Today", t).on_press_tx(move |s: &mut State, tx| {
                if let Some(tab) = s.tabs.get_mut(s.active) {
                    let now = chrono::Local::now().date_naive();
                    tab.calendar_month = if is_week {
                        now
                    } else {
                        now.with_day(1).unwrap_or(now)
                    };
                }
                s.save_preferences();
                tx.instant();
            }),
        )
        .child(
            navigation_button("calendar-next", "Next", "›", t).on_press_tx(
                move |s: &mut State, tx| {
                    if let Some(tab) = s.tabs.get_mut(s.active) {
                        let next = if is_week {
                            tab.calendar_month.checked_add_signed(Duration::days(7))
                        } else {
                            tab.calendar_month
                                .checked_add_months(chrono::Months::new(1))
                        };
                        if let Some(next) = next {
                            tab.calendar_month = next;
                        }
                    }
                    s.save_preferences();
                    tx.instant();
                },
            ),
        );
    let mut content = col()
        .key("calendar-grid")
        .w_full()
        .min_w(0.)
        .gap(4.)
        .child(header);
    if s.database_views.calendar.schedule.is_some() {
        content = content.child(t.label(
            "Choose a day to schedule the page. Press Escape to cancel.",
            t.muted,
        ));
    }
    if let Some(error) = &s.database_views.calendar.error {
        content = content.child(
            row()
                .w_full()
                .gap(8.)
                .center_y()
                .child(
                    t.label(format!("Calendar is not loaded: {error}"), t.error)
                        .grow(1.)
                        .wrap(),
                )
                .child(
                    button("calendar-retry", "Retry", t)
                        .on_press(|s: &mut State| s.database_views.calendar.invalidate()),
                ),
        );
    }
    if s.database_views.calendar.loading {
        content = content.child(t.label("Loading calendar…", t.muted));
    }
    if let Some(projection) = projection.filter(|p| p.dated_count > 1000) {
        let start = projection.offset + 1;
        let end = projection.offset + projection.records.len() as u64;
        let mut pager = row().w_full().gap(8.).center_y().child(
            t.label(
                format!("Showing {start}–{end} of {} pages", projection.dated_count),
                t.muted,
            )
            .grow(1.),
        );
        if projection.offset > 0 {
            pager = pager.child(button("calendar-page-prev", "Previous", t).on_press(
                |s: &mut State| {
                    s.database_views.calendar.offset =
                        s.database_views.calendar.offset.saturating_sub(1000);
                    s.database_views.calendar.invalidate();
                },
            ));
        }
        if end < projection.dated_count {
            pager = pager.child(button("calendar-page-next", "Next", t).on_press(
                |s: &mut State| {
                    s.database_views.calendar.offset =
                        s.database_views.calendar.offset.saturating_add(1000);
                    s.database_views.calendar.invalidate();
                },
            ));
        }
        content = content.child(pager);
    }
    let mut weekdays = row().w_full();
    let names = if s.prefs.start_week_on_monday {
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    } else {
        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
    };
    for name in names {
        weekdays = weekdays.child(
            row()
                .key(format!("calendar-weekday-{name}"))
                .grow(1.)
                .min_w(0.)
                .h(25.)
                .center()
                .child(t.label(name, t.muted)),
        );
    }
    content = content.child(weekdays);
    let mut weeks = col().w_full();
    let days = (last - first).num_days() as usize + 1;
    let column_width =
        (rune::memo::layout_with("calendar-grid", |rect| rect[2]).unwrap_or(700.) / 7.).max(1.);
    for week in 0..days / 7 {
        let week_start = first + Duration::days((week * 7) as i64);
        let week_end = week_start + Duration::days(6);
        let mut lanes: Vec<([bool; 7], f32)> = Vec::new();
        let mut segments = Vec::new();
        for (index, record) in records.iter().enumerate() {
            let Some((start, end)) = calendar_view::date_range(record, prop) else {
                continue;
            };
            if end < week_start || start > week_end {
                continue;
            }
            let begin = start.max(week_start);
            let finish = end.min(week_end);
            let left = (begin - week_start).num_days() as usize;
            let right = (finish - week_start).num_days() as usize;
            let height = 30.
                + db.properties
                    .iter()
                    .filter(|p| {
                        p.kind != moss_core::PropertyKind::Title
                            && p.id != prop.id
                            && super::property_visibility::visible(db, tab.view, &p.id)
                            && record.values.get(&p.id).is_some_and(|v| !v.is_null())
                    })
                    .count() as f32
                    * 23.;
            let lane = lanes
                .iter()
                .position(|(occupied, _)| (left..=right).all(|d| !occupied[d]))
                .unwrap_or_else(|| {
                    lanes.push(([false; 7], 0.));
                    lanes.len() - 1
                });
            for day in left..=right {
                lanes[lane].0[day] = true;
            }
            lanes[lane].1 = lanes[lane].1.max(height);
            segments.push((
                index,
                begin,
                finish,
                left,
                right,
                lane,
                start.max(first) == begin,
            ));
        }
        let week_height = (36. + lanes.iter().map(|(_, height)| height + 4.).sum::<f32>())
            .max(if is_week { 220. } else { 136. });
        let mut line = row()
            .key(format!("calendar-week-{week}"))
            .w_full()
            .h(week_height);
        for day in 0..7 {
            let date = first + Duration::days((week * 7 + day) as i64);
            let id = prop.id.clone();
            let add_id = id.clone();
            let day_label = if date.day() == 1 {
                date.format("%-d %b").to_string()
            } else {
                date.day().to_string()
            };
            let muted = date.month() != tab.calendar_month.month() && !is_week;
            let day_number = row()
                .size(26., 26.)
                .center()
                .radius(13.)
                .fill(if date == today {
                    t.error
                } else {
                    Color::TRANSPARENT
                })
                .child(t.label(
                    day_label,
                    if date == today {
                        Color::WHITE
                    } else if muted {
                        t.faint
                    } else {
                        t.text
                    },
                ));
            let add = row()
                .key(format!("calendar-add-{date}"))
                .size(26., 26.)
                .center()
                .radius(4.)
                .hover(t.hover)
                .role("button")
                .label(format!("New page on {date}"))
                .focusable(true)
                .group_opacity(0.25, 1.)
                .child(rune::icon(&phosphor::PLUS, 16.).color(t.muted))
                .on_press(move |s: &mut State| create(s, &add_id, date));
            let mut cell = col()
                .key(format!("calendar-day-{date}"))
                .grow(1.)
                .min_w(0.)
                .h(week_height)
                .padding(4.)
                .gap(3.)
                .border(0.5, t.line)
                .fill(
                    if s.database_views
                        .calendar
                        .drag
                        .as_ref()
                        .is_some_and(|drag| drag.moved && drag.target == Some(date))
                    {
                        t.selected
                    } else if date.weekday().number_from_monday() >= 6 {
                        t.sidebar
                    } else {
                        t.bg
                    },
                )
                .hover_group()
                .on_drawn_layout_tx(move |s: &mut State, rect, tx| {
                    s.database_views.calendar.day_rects.insert(date, rect);
                    tx.live_only();
                })
                .child(
                    row()
                        .w_full()
                        .center_y()
                        .child(day_number)
                        .child(el().grow(1.))
                        .child(add),
                );
            cell = if s.database_views.calendar.schedule.is_some() {
                cell.role("button")
                    .label(format!("Schedule on {date}"))
                    .focusable(true)
                    .on_press_tx(move |s: &mut State, tx| {
                        s.schedule_calendar_record(date);
                        tx.instant();
                    })
            } else {
                cell.on_double_press_tx(move |s: &mut State, tx| {
                    create(s, &id, date);
                    tx.instant();
                })
            };
            line = line.child(cell);
        }
        for (index, begin, finish, left, right, lane, primary) in segments {
            let top = 34.
                + lanes
                    .iter()
                    .take(lane)
                    .map(|(_, height)| height + 4.)
                    .sum::<f32>();
            line = line.child(
                card(
                    s,
                    tab,
                    db,
                    &records[index],
                    index,
                    begin,
                    finish,
                    primary,
                    &prop.id,
                    projection,
                    t,
                )
                .absolute(left as f32 * column_width + 4., top)
                .w((right - left + 1) as f32 * column_width - 8.),
            );
        }
        weeks = weeks.child(line);
    }
    content.child(weeks)
}

fn card(
    s: &State,
    tab: &Tab,
    db: &Database,
    record: &Record,
    index: usize,
    date: NaiveDate,
    through: NaiveDate,
    primary: bool,
    property_id: &str,
    projection: Option<&moss_core::calendar::CalendarProjection>,
    t: Theme,
) -> El {
    let title = if record.title.is_empty() {
        "Untitled"
    } else {
        &record.title
    };
    let record_id = record.id.clone();
    let db_id = db.id.clone();
    let drag_record = record.clone();
    let drag_property = property_id.to_owned();
    let icon = if let Some(a) = projection.and_then(|p| p.appearance.get(&record.id)) {
        super::record_icon::appearance(
            &a.icon,
            a.has_body_content,
            if primary {
                format!("record-icon-calendar-{index}")
            } else {
                format!("record-icon-calendar-{index}-{date}")
            },
            t,
        )
    } else {
        super::record_icon::view(
            tab,
            record,
            if primary {
                format!("record-icon-calendar-{index}")
            } else {
                format!("record-icon-calendar-{index}-{date}")
            },
            t,
        )
    };
    let mut content = col()
        .key(if primary {
            format!("calendar-{index}")
        } else {
            format!("calendar-{index}-{date}")
        })
        .min_w(0.)
        .padding(5.)
        .gap(3.)
        .radius(4.)
        .fill(t.bg)
        .border(1., t.line)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(title)
        .focusable(true)
        .child(
            row()
                .w_full()
                .min_w(0.)
                .gap(4.)
                .center_y()
                .child(icon)
                .child(t.label(title, t.text).min_w(0.).grow(1.).truncate()),
        )
        .on_drag(move |s: &mut State, event, tx| {
            s.drag_calendar_record(&drag_record, &drag_property, date, through, *event, tx)
        })
        .on_press(move |s: &mut State| {
            if s.database_views.calendar.suppress_press {
                s.database_views.calendar.suppress_press = false;
                return;
            }
            s.open_database_record(&db_id, &record_id, "center");
        });
    if let Some((start, end)) = db
        .properties
        .iter()
        .find(|p| p.id == property_id)
        .and_then(|p| calendar_view::date_range(record, p))
    {
        if date == start {
            content = content.child(
                el().key(format!("calendar-resize-start-{}", record.id))
                    .w(7.)
                    .pin(Some(0.), None, Some(0.), Some(0.))
                    .cursor(Cursor::ResizeColumn),
            );
        }
        if through == end {
            content = content.child(
                el().key(format!("calendar-resize-end-{}", record.id))
                    .w(7.)
                    .pin(None, Some(0.), Some(0.), Some(0.))
                    .cursor(Cursor::ResizeColumn),
            );
        }
    }
    for property in db.properties.iter().filter(|property| {
        property.kind != moss_core::PropertyKind::Title
            && property.id != property_id
            && super::property_visibility::visible(db, tab.view, &property.id)
    }) {
        if let Some(value) = record
            .values
            .get(&property.id)
            .filter(|value| !value.is_null())
        {
            content = content.child(
                row()
                    .w_full()
                    .min_w(0.)
                    .child(super::property_value::view(s, property, Some(value), t).min_w(0.)),
            );
        }
    }
    content
}

pub fn undated_panel(s: &State, tab: &Tab, db: &Database, t: Theme) -> Option<El> {
    if !s.undated_open {
        return None;
    }
    let view = db.views.get(tab.view)?;
    let prop = calendar_view::date_property(db, view)?;
    let projection = s.database_views.calendar.projection_for(&db.id, &view.id);
    let records = projection
        .map(|p| p.undated.as_slice())
        .unwrap_or(&tab.records);
    let query = s.undated_search.text.to_lowercase();
    let mut rows = col().w_full().max_h(260.).scroll(ScrollSpec {
        stick_bottom: false,
    });
    for (index, record) in records.iter().enumerate() {
        if calendar_view::date_range(record, prop).is_some()
            || !record.title.to_lowercase().contains(&query)
        {
            continue;
        }
        let id = record.id.clone();
        let db_id = db.id.clone();
        let drag_record = record.clone();
        let drag_property = prop.id.clone();
        let drag_from = tab.calendar_month;
        let selected_record = record.clone();
        let selected_property = prop.id.clone();
        let title = if record.title.is_empty() {
            "Untitled"
        } else {
            &record.title
        };
        rows = rows.child(
            menu::row_item(
                format!("calendar-undated-row-{index}"),
                title,
                rune::icon(&phosphor::FILE, 16.).color(t.muted),
                t,
            )
            .child(
                button(format!("calendar-undated-open-{index}"), "Open", t).on_press(
                    move |s: &mut State| {
                        s.undated_open = false;
                        s.open_database_record(&db_id, &id, "center");
                    },
                ),
            )
            .on_drag(move |s: &mut State, event, tx| {
                s.drag_calendar_record(
                    &drag_record,
                    &drag_property,
                    drag_from,
                    drag_from,
                    *event,
                    tx,
                )
            })
            .on_press(move |s: &mut State| {
                if s.database_views.calendar.suppress_press {
                    s.database_views.calendar.suppress_press = false;
                    return;
                }
                s.choose_calendar_undated(&selected_record, &selected_property);
            }),
        );
    }
    Some(
        col()
            .key("calendar-no-date-panel")
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
            .child(
                super::inputs::field(
                    &s.undated_search,
                    "calendar-no-date-search",
                    "Search for a page…",
                    t,
                )
                .on_edit(|s: &mut State| &mut s.undated_search),
            )
            .child(
                t.label("Click or drag a page to add a date.", t.muted)
                    .px(8.),
            )
            .child(rows)
            .children(
                (projection.is_some_and(|p| p.undated_count > 100) && query.is_empty()).then(
                    || {
                        t.label("Showing the first 100. Search for more.", t.muted)
                            .px(8.)
                    },
                ),
            )
            .anchored("calendar-no-date", Side::Bottom, Align::End, 4.),
    )
}
