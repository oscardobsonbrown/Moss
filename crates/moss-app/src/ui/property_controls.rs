use super::property_value::status_style;
use super::theme::{Theme, button};
use crate::state::State;
use chrono::{Datelike, Months};
use moss_core::PropertyKind;
use rune::*;

pub fn anchored_picker(s: &State, t: Theme) -> Option<El> {
    let picker = s.property_picker.as_ref()?;
    let anchor = if s
        .record_peek
        .as_ref()
        .is_some_and(|p| p.record.id == picker.record_id)
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
            .position(|r| r.id == picker.record_id)?;
        format!("cell-{index}-{}", picker.property_id)
    };
    picker_view(s, t).map(|p| p.anchored_fit(anchor, Side::Bottom, Align::Start, 2.))
}

fn picker_view(s: &State, t: Theme) -> Option<El> {
    let p = s.property_picker.as_ref()?;
    let is_date = p.kind == PropertyKind::Date;
    let default_status = s
        .record_database(&p.record_id)
        .and_then(|db| db.properties.iter().find(|prop| prop.id == p.property_id))
        .and_then(|prop| prop.config.get("options"))
        .is_some_and(|options| {
            options == &serde_json::json!(["Not started", "In progress", "Done"])
        });
    let placeholder = if is_date {
        "DD/MM/YYYY"
    } else if p.kind == PropertyKind::Status && !s.cell.text.is_empty() {
        ""
    } else {
        "Search options…"
    };
    let input = super::inputs::invalid(
        super::inputs::field(
            &s.property_picker_input,
            "property-picker-input",
            placeholder,
            t,
        ),
        p.error.is_some(),
        t,
    )
    .label(if is_date { "Date" } else { "Search options" })
    .on_edit(|s: &mut State| &mut s.property_picker_input)
    .on_input_change(|s: &mut State, tx| {
        s.date_draft_changed();
        if let Some(p) = &mut s.property_picker {
            p.selected = None;
            if !p.save_failed {
                p.error = None;
            }
        }
        tx.instant();
    })
    .on_input_key(|s: &mut State, event, tx| {
        if event.key == Key::Escape {
            s.dismiss_property_picker();
            tx.instant();
            return true;
        }
        if matches!(event.key, Key::Up | Key::Down)
            && s.property_picker
                .as_ref()
                .is_some_and(|p| p.kind != PropertyKind::Date)
        {
            let n = s.property_options().len();
            if let Some(p) = &mut s.property_picker {
                if n > 0 {
                    p.selected = Some(if event.key == Key::Down {
                        p.selected.map_or(0, |i| (i + 1).min(n - 1))
                    } else {
                        p.selected.map_or(0, |i| i.saturating_sub(1))
                    });
                }
            }
            if let Some(index) = s.property_picker.as_ref().and_then(|p| p.selected) {
                tx.scroll_into_view(format!("property-option-{index}"));
            }
            tx.instant();
            return true;
        }
        false
    })
    .on_submit(|s: &mut State, tx| {
        s.submit_property_picker();
        tx.instant();
    });
    let input_row = if p.kind == PropertyKind::Status && !s.cell.text.is_empty() {
        let (_, bg, dot) = if default_status {
            status_style(&s.cell.text, t)
        } else {
            (None, t.selected, t.muted)
        };
        row()
            .w_full()
            .center_y()
            .gap(5.)
            .child(
                row()
                    .h(20.)
                    .max_w(120.)
                    .min_w(0.)
                    .px(6.)
                    .gap(5.)
                    .center_y()
                    .radius(10.)
                    .fill(bg)
                    .child(el().size(7., 7.).radius(4.).fill(dot))
                    .child(t.label(&s.cell.text, t.text).min_w(0.).truncate()),
            )
            .child(input.grow(1.).min_w(0.))
    } else {
        row().w_full().child(input)
    };
    let mut panel = col()
        .key("property-picker")
        .role("dialog")
        .label("Property picker")
        .modal(true, |s: &mut State| s.dismiss_property_picker())
        .w(if is_date { 248. } else { 240. })
        .min_h(0.)
        .padding(10.)
        .gap(8.)
        .fill(t.menu)
        .radius(6.)
        .border(1., t.line)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .transition(Transition::opacity())
        .child(input_row.instant_changes());
    if let Some(date) = &p.date {
        if date.has_end {
            panel = panel.child(date_input(
                s,
                &s.date_end_input,
                "date-end-input",
                "End date",
                0,
                t,
            ));
        }
        if date.has_time {
            panel = panel.child(date_input(
                s,
                &s.date_start_time_input,
                "date-start-time",
                "Start time (HH:MM)",
                1,
                t,
            ));
            if date.has_end {
                panel = panel.child(date_input(
                    s,
                    &s.date_end_time_input,
                    "date-end-time",
                    "End time (HH:MM)",
                    2,
                    t,
                ));
            }
        }
    }
    if let Some(error) = &p.error {
        panel = panel.child(t.label(error, t.text).wrap());
    }
    if p.save_failed {
        panel = panel.child(
            col()
                .gap(2.)
                .child(
                    button("property-retry", "Retry", t).on_press(|s: &mut State| {
                        s.retry_property_change();
                    }),
                )
                .child(
                    button("property-discard", "Discard unsaved change", t).on_press(
                        |s: &mut State| {
                            s.discard_failed_property_change();
                        },
                    ),
                ),
        );
    }
    if is_date {
        let month = p.month;
        panel = panel.child(
            row()
                .w_full()
                .center_y()
                .child(t.label(month.format("%b %Y").to_string(), t.text).grow(1.))
                .child(button("date-today", "Today", t).on_press(|s: &mut State| {
                    s.choose_calendar_date(chrono::Local::now().date_naive());
                }))
                .child(
                    super::database::navigation_button("date-prev", "Previous month", "‹", t)
                        .on_press_tx(|s: &mut State, tx| {
                            if let Some(p) = &mut s.property_picker {
                                if let Some(m) = p.month.checked_sub_months(Months::new(1)) {
                                    p.month = m;
                                }
                            }
                            tx.instant();
                        }),
                )
                .child(
                    super::database::navigation_button("date-next", "Next month", "›", t)
                        .on_press_tx(|s: &mut State, tx| {
                            if let Some(p) = &mut s.property_picker {
                                if let Some(m) = p.month.checked_add_months(Months::new(1)) {
                                    p.month = m;
                                }
                            }
                            tx.instant();
                        }),
                )
                .instant_changes(),
        );
        let mut weekdays = row().w_full().h(32.).center_y();
        let names = if s.prefs.start_week_on_monday {
            ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
        } else {
            ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
        };
        for day in names {
            weekdays = weekdays.child(t.label(day, t.muted).w(32.).center());
        }
        let mut calendar = col().key("date-calendar").w_full().child(weekdays);
        let start = if s.prefs.start_week_on_monday {
            month.weekday().num_days_from_monday()
        } else {
            month.weekday().num_days_from_sunday()
        } as i64;
        for week in 0..6 {
            let mut line = row().w_full().h(32.).center_y();
            for weekday in 0..7 {
                let day = month + chrono::Duration::days(week * 7 + weekday - start);
                let in_month = day.month() == month.month();
                let date = day.format("%Y-%m-%d").to_string();
                let chosen = s.cell.text == date;
                let today = day == chrono::Local::now().date_naive();
                let mut item = row()
                    .key(format!("date-{date}"))
                    .size(32., 30.)
                    .center()
                    .radius(if today && !chosen { 15. } else { 4. })
                    .hover(t.hover)
                    .role("button")
                    .label(date.clone())
                    .focusable(true)
                    .child(t.label(
                        day.day().to_string(),
                        if chosen || today {
                            Color::WHITE
                        } else if in_month {
                            t.text
                        } else {
                            t.faint
                        },
                    ));
                if chosen {
                    item = item.fill(t.blue);
                } else if today {
                    item = item.fill(super::property_value::color(0xe56c64));
                }
                line = line.child(item.on_press(move |s: &mut State| {
                    if let Ok(day) = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
                        s.choose_calendar_date(day);
                    }
                }));
            }
            calendar = calendar.child(line);
        }
        panel = panel.child(
            col()
                .key("date-scroll")
                .w_full()
                .min_h(0.)
                .shrink()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(calendar)
                .child(
                    date_switch(
                        "date-toggle-end",
                        "End date",
                        p.date.as_ref().is_some_and(|d| d.has_end),
                        t,
                    )
                    .on_press_tx(|s: &mut State, tx| {
                        s.toggle_date_end();
                        tx.instant();
                    }),
                )
                .child(
                    date_switch(
                        "date-toggle-time",
                        "Include time",
                        p.date.as_ref().is_some_and(|d| d.has_time),
                        t,
                    )
                    .on_press_tx(|s: &mut State, tx| {
                        s.toggle_date_time();
                        tx.instant();
                    }),
                )
                .child(button("date-clear", "Clear", t).on_press(|s: &mut State| {
                    s.clear_date_picker();
                }))
                .instant_changes(),
        );
    } else {
        let mut options = col()
            .key("property-options")
            .w_full()
            .max_h(250.)
            .min_h(0.)
            .shrink()
            .scroll(ScrollSpec {
                stick_bottom: false,
            });
        for (i, option) in s.property_options().into_iter().enumerate() {
            let value = option.clone();
            let mut item = row()
                .key(format!("property-option-{i}"))
                .w_full()
                .h(30.)
                .px(6.)
                .center_y()
                .radius(4.)
                .hover(t.hover)
                .role("button")
                .label(&option)
                .focusable(true);
            if p.kind == PropertyKind::Status {
                let (group, bg, dot) = if default_status {
                    status_style(&option, t)
                } else {
                    (None, t.selected, t.muted)
                };
                if let Some(group) = group {
                    options = options.child(t.label(group, t.muted).pt(8.));
                }
                item = item.child(
                    row()
                        .h(20.)
                        .max_w(200.)
                        .min_w(0.)
                        .px(6.)
                        .gap(5.)
                        .center_y()
                        .radius(10.)
                        .fill(bg)
                        .child(el().size(7., 7.).radius(4.).fill(dot))
                        .child(t.label(&option, t.text).min_w(0.).truncate()),
                );
            } else {
                item = item.child(t.label(&option, t.text).min_w(0.).truncate());
            }
            if Some(i) == p.selected {
                item = item.fill(t.selected);
            }
            options = options.child(item.on_press(move |s: &mut State| {
                s.choose_property_value(&value);
            }));
        }
        panel = panel.child(options.instant_changes());
        panel = panel.child(button("property-clear", "Clear", t).w_full().on_press(
            |s: &mut State| {
                s.choose_property_value("");
            },
        ));
    }
    Some(panel)
}

fn date_input(s: &State, edit: &TextEdit, key: &str, name: &str, index: u8, t: Theme) -> El {
    super::inputs::invalid(
        super::inputs::field(edit, key, name, t),
        s.property_picker
            .as_ref()
            .is_some_and(|picker| picker.error.is_some()),
        t,
    )
    .label(name)
    .on_edit(move |s: &mut State| match index {
        0 => &mut s.date_end_input,
        1 => &mut s.date_start_time_input,
        _ => &mut s.date_end_time_input,
    })
    .on_input_change(|s: &mut State, tx| {
        s.date_draft_changed();
        if let Some(p) = &mut s.property_picker {
            if !p.save_failed {
                p.error = None;
            }
        }
        tx.instant();
    })
    .on_submit(|s: &mut State, tx| {
        s.submit_property_picker();
        tx.instant();
    })
}
fn date_switch(key: &str, name: &str, on: bool, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(32.)
        .center_y()
        .gap(8.)
        .role("switch")
        .label(name)
        .checked(on)
        .focusable(true)
        .hover(t.hover)
        .child(t.label(name, t.text).grow(1.))
        .child(
            row()
                .size(32., 18.)
                .px(2.)
                .center_y()
                .radius(9.)
                .fill(if on { t.blue } else { t.selected })
                .children(on.then(|| el().w(12.)))
                .child(el().size(14., 14.).radius(7.).fill(Color::WHITE)),
        )
}
