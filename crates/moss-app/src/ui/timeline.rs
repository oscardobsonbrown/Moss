use super::database::{active, date_property, editor, record_date, undated_button};
use super::theme::Theme;
use crate::state::{State, Tab};
use chrono::{Datelike, NaiveDate};
use moss_core::Database;
use rune::*;
use std::collections::BTreeMap;

// Provisional Paper geometry; confirm against a matched native capture.
const NAME_WIDTH: f32 = 280.;
const DAY_WIDTH: f32 = 40.;
const ROW_HEIGHT: f32 = 36.;
const DAYS: i64 = 30;

fn name_cell(key: impl Into<String>, height: f32, t: Theme) -> El {
    row()
        .key(key)
        .w(NAME_WIDTH)
        .h(height)
        .fill(t.bg)
        .clip_rounded(0.)
        .sticky_left(0.)
        // The hidden grid must not receive a drag through an empty name cell.
        .on_drag(|_: &mut State, _, tx| tx.live_only())
        .on_press(|_: &mut State| {})
        .child(
            el().w(0.5)
                .pin(None, Some(0.), Some(0.), Some(0.))
                .fill(t.line),
        )
}

fn date_grid(first: NaiveDate, t: Theme) -> El {
    let today = chrono::Local::now().date_naive();
    let mut grid = row().pin(Some(0.), Some(0.), Some(0.), Some(0.));
    for day in 0..DAYS {
        let Some(date) = first.checked_add_signed(chrono::Duration::days(day)) else {
            break;
        };
        let weekend = date.weekday().number_from_monday() >= 6;
        grid = grid.child(
            el().w(DAY_WIDTH)
                .h_full()
                .fill(if weekend {
                    t.hover.alpha(0.45)
                } else {
                    Color::TRANSPARENT
                })
                .child(
                    el().w(0.5)
                        .pin(None, Some(0.), Some(0.), Some(0.))
                        .fill(t.line),
                )
                .children((date == today).then(|| {
                    el().w(1.)
                        .pin(Some(DAY_WIDTH / 2.), Some(0.), None, Some(0.))
                        .fill(Color {
                            r: 0.7,
                            g: 0.06,
                            b: 0.04,
                            a: 1.,
                        })
                })),
        );
    }
    grid
}

fn header(first: NaiveDate, t: Theme) -> El {
    let mut months = row().h(32.);
    let mut day = 0;
    while day < DAYS {
        let Some(date) = first.checked_add_signed(chrono::Duration::days(day)) else {
            break;
        };
        let mut end = day + 1;
        while end < DAYS
            && first
                .checked_add_signed(chrono::Duration::days(end))
                .is_some_and(|next| next.month() == date.month())
        {
            end += 1;
        }
        months = months.child(
            row()
                .w((end - day) as f32 * DAY_WIDTH)
                .h(32.)
                .px(8.)
                .center_y()
                .child(
                    t.label(date.format("%B %Y").to_string(), t.text)
                        .min_w(0.)
                        .truncate(),
                ),
        );
        day = end;
    }
    let mut dates = row().h(24.);
    for day in 0..DAYS {
        if let Some(date) = first.checked_add_signed(chrono::Duration::days(day)) {
            dates = dates.child(
                row()
                    .key(format!("timeline-date-{date}"))
                    .w(DAY_WIDTH)
                    .h(24.)
                    .center()
                    .child(t.label(date.day().to_string(), t.muted)),
            );
        }
    }
    row()
        .key("timeline-axis")
        .h(56.)
        .fill(t.bg)
        .sticky_top(0.)
        .raise()
        .child(
            name_cell("timeline-name-header", 56., t)
                .px(8.)
                .center_y()
                .child(t.label("Name", t.muted)),
        )
        .child(col().w(DAYS as f32 * DAY_WIDTH).child(months).child(dates))
}

pub fn view(s: &State, tab: &Tab, db: &Database, t: Theme) -> El {
    let Some(prop) = date_property(db) else {
        return col()
            .padding(16.)
            .child(t.label("Add a date property to use Timeline.", t.muted));
    };
    let first = tab.timeline_start;
    let scale = DAY_WIDTH;
    let days = DAYS;
    let controls = row()
        .key("timeline-controls")
        .self_start()
        .sticky_left(0.)
        .gap(8.)
        .center_y()
        .child(
            super::database::navigation_button("timeline-prev", "Previous period", "‹", t)
                .on_press_tx(|s: &mut State, tx| {
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
        .child(t.label(first.format("%d %b %Y").to_string(), t.muted))
        .child(
            super::database::navigation_button("timeline-next", "Next period", "›", t).on_press_tx(
                |s: &mut State, tx| {
                    if let Some(tab) = s.tabs.get_mut(s.active) {
                        if let Some(date) = tab
                            .timeline_start
                            .checked_add_signed(chrono::Duration::days(30))
                        {
                            tab.timeline_start = date;
                        }
                    }
                    tx.instant();
                },
            ),
        );
    let controls = controls.child(undated_button(tab, prop, "timeline", t));
    let mut content = col()
        .min_w(NAME_WIDTH + DAYS as f32 * DAY_WIDTH)
        .gap(0.)
        .child(controls)
        .child(header(first, t));
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
            editor(s, NAME_WIDTH, t).h(ROW_HEIGHT)
        } else {
            super::record_icon::title_button(
                tab,
                record,
                i,
                format!("timeline-open-{i}"),
                "timeline",
                t,
            )
            .w(NAME_WIDTH)
            .h(ROW_HEIGHT)
        };
        let mut track = row()
            .key(format!("timeline-track-{i}"))
            .w(days as f32 * scale)
            .h(ROW_HEIGHT)
            .clip_rounded(0.)
            .child(date_grid(first, t));
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
                    .key(format!("timeline-bar-{i}"))
                    .h(32.)
                    .radius(4.)
                    .fill(t.bg)
                    .border(1., t.line)
                    .shadow(1., 3., Color::BLACK.alpha(0.05))
                    .center_y();
                let bar = if dated_inline {
                    bar.child(
                        editor(s, ((right - left) * scale - 2.).max(0.), t)
                            .h(32.)
                            .fill(t.bg),
                    )
                } else {
                    bar.px(5.)
                        .role("button")
                        .label(&record.title)
                        .focusable(true)
                        .gap(6.)
                        .child(super::record_icon::view(
                            tab,
                            record,
                            format!("record-icon-timeline-bar-{i}"),
                            t,
                        ))
                        .child(
                            t.label(
                                if record.title.is_empty() {
                                    "Untitled"
                                } else {
                                    &record.title
                                },
                                t.text,
                            )
                            .truncate(),
                        )
                        .on_press(move |s: &mut State| {
                            s.open_record(i);
                        })
                };
                track = track.child(bar.absolute(left * scale, 2.));
            }
        } else {
            track = track.child(t.label("No date", t.muted).absolute(8., 8.));
        }
        content = content.child(
            row()
                .h(ROW_HEIGHT)
                .border(0.5, t.line)
                .child(name_cell(format!("timeline-name-{i}"), ROW_HEIGHT, t).child(title))
                .child(track),
        );
    }
    let id = prop.id.clone();
    let mut track = row()
        .key("timeline-create-track")
        .w(days as f32 * scale)
        .h(ROW_HEIGHT)
        .border(0.5, t.line)
        .child(date_grid(first, t));
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
    content = content.child(
        row()
            .child(name_cell("timeline-create-name", ROW_HEIGHT, t))
            .child(track),
    );
    content.child(
        super::database::new_page_button("timeline-new", t)
            .self_start()
            .sticky_left(0.)
            .on_press_tx(|s: &mut State, tx| {
                s.create_inline(BTreeMap::new(), false);
                tx.instant();
            }),
    )
}
