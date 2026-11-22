use super::property_value::{
    date_format_choices, date_format_label, explicit_option_color, option_color_choices,
    option_tag_dot, option_tag_fill, option_tag_style, status_style,
};
use super::theme::{Theme, button};
use crate::state::State;
use chrono::{Datelike, Months};
use moss_core::PropertyKind;
use rune::*;

pub fn cell_picker_popup(s: &State, t: Theme) -> Option<El> {
    match s.cell_picker.as_ref()? {
        crate::property_controls::CellPicker::Property(picker) => {
            let anchor = s.property_anchor(&picker.record_id, &picker.property_id, picker.kind)?;
            picker_view(s, t).map(|p| p.anchored_fit(anchor, Side::Bottom, Align::Start, 2.))
        }
        crate::property_controls::CellPicker::Relation(_) => {
            super::advanced_properties::relation_picker(s, t)
        }
        crate::property_controls::CellPicker::Place(_) => super::place_picker::popup(s, t),
    }
}

fn picker_action_row(key: &str, name: &str, icon: &'static Icon, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(30.)
        .px(8.)
        .gap(8.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(name)
        .focusable(true)
        .child(rune::icon(icon, 14.).color(t.muted))
        .child(t.label(name, t.text).grow(1.).truncate())
}

fn picker_view(s: &State, t: Theme) -> Option<El> {
    let p = s.property_picker()?;
    let is_date = p.kind == PropertyKind::Date;
    let current_prop = s
        .record_database(&p.record_id)
        .and_then(|db| db.properties.iter().find(|prop| prop.id == p.property_id));
    let placeholder = if is_date {
        "DD/MM/YYYY"
    } else {
        "Search or create option…"
    };
    let input = super::inputs::invalid(
        super::inputs::field(&p.input, "property-picker-input", placeholder, t),
        p.error.is_some(),
        t,
    )
    .label(if is_date { "Date" } else { "Search options" })
    .on_edit(|s: &mut State| &mut s.property_picker_mut().unwrap().input)
    .on_input_change(|s: &mut State, tx| {
        s.date_draft_changed();
        if let Some(p) = s.property_picker_mut() {
            p.selected = None;
            if !p.save_failed {
                p.error = None;
            }
        }
        tx.instant();
    })
    .on_input_key(|s: &mut State, event, tx| {
        if event.key == Key::Escape {
            s.dismiss_cell_picker();
            tx.instant();
            return true;
        }
        if matches!(event.key, Key::Up | Key::Down)
            && s.property_picker()
                .is_some_and(|p| p.kind != PropertyKind::Date)
        {
            let n = s.property_options().len();
            if let Some(p) = s.property_picker_mut() {
                if n > 0 {
                    p.selected = Some(if event.key == Key::Down {
                        p.selected.map_or(0, |i| (i + 1).min(n - 1))
                    } else {
                        p.selected.map_or(0, |i| i.saturating_sub(1))
                    });
                }
            }
            if let Some(index) = s.property_picker().and_then(|p| p.selected) {
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
    let mut panel = col()
        .key("property-picker")
        .role("dialog")
        .label("Property picker")
        .modal(true, |s: &mut State| s.dismiss_cell_picker())
        .w(if is_date { 256. } else { 248. })
        .min_h(0.)
        .padding(8.)
        .gap(6.)
        .fill(t.menu)
        .radius(8.)
        .border(1., t.line)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .transition(Transition::opacity());
    if p.option_editor.is_none() && !p.date_format_pane {
        panel = panel.child(row().w_full().child(input).instant_changes());
    }
    if let Some(date) = p.date.as_ref().filter(|_| !p.date_format_pane) {
        if date.has_end {
            panel = panel.child(date_input(
                s,
                &date.end_input,
                "date-end-input",
                "End date",
                0,
                t,
            ));
        }
        if date.has_time {
            panel = panel.child(date_input(
                s,
                &date.start_time_input,
                "date-start-time",
                "Start time (HH:MM)",
                1,
                t,
            ));
            if date.has_end {
                panel = panel.child(date_input(
                    s,
                    &date.end_time_input,
                    "date-end-time",
                    "End time (HH:MM)",
                    2,
                    t,
                ));
            }
        }
    }
    if let Some(error) = &p.error {
        panel = panel.child(t.label(error, t.error).px(8.).wrap());
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
    if is_date && p.date_format_pane {
        let active_format = current_prop
            .and_then(|prop| prop.config.get("date_format"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("full");
        let back = picker_action_row(
            "date-format-back",
            "Date format",
            &super::phosphor::CARET_LEFT,
            t,
        )
        .on_press_tx(|s: &mut State, tx| {
            if let Some(p) = s.property_picker_mut() {
                p.date_format_pane = false;
                p.error = None;
            }
            tx.instant();
        });
        let list = date_format_choices("picker-date-format", active_format, t, |s, format| {
            s.set_picker_date_format(format);
        });
        panel = panel
            .child(back)
            .child(el().w_full().h(1.).fill(t.line))
            .child(list.instant_changes());
    } else if is_date {
        let month = p.month;
        let current_format = current_prop
            .and_then(|prop| prop.config.get("date_format"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("full");
        let format_label = date_format_label(current_format);
        panel = panel.child(
            row()
                .w_full()
                .center_y()
                .px(4.)
                .child(t.label(month.format("%b %Y").to_string(), t.text).grow(1.))
                .child(button("date-today", "Today", t).on_press(|s: &mut State| {
                    s.choose_calendar_date(chrono::Local::now().date_naive());
                }))
                .child(
                    super::database::navigation_button("date-prev", "Previous month", "‹", t)
                        .on_press_tx(|s: &mut State, tx| {
                            if let Some(p) = s.property_picker_mut() {
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
                            if let Some(p) = s.property_picker_mut() {
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
        let names = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
        for offset in 0..7 {
            let day = names[(s.prefs.week_start() as usize + offset) % 7];
            weekdays = weekdays.child(t.label(day, t.muted).w(32.).center());
        }
        let mut calendar = col().key("date-calendar").w_full().child(weekdays);
        let start = crate::calendar_view::weekday_offset(month, s.prefs.week_start()) as i64;
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
        let format_row = row()
            .key("date-format-trigger")
            .w_full()
            .h(32.)
            .px(8.)
            .gap(6.)
            .center_y()
            .radius(4.)
            .hover(t.hover)
            .role("button")
            .label(format!("Date format: {format_label}"))
            .focusable(true)
            .child(t.label("Date format", t.text).grow(1.))
            .child(t.label(format_label, t.muted).shrink())
            .child(rune::icon(&super::phosphor::CARET_RIGHT, 14.).color(t.muted))
            .on_press_tx(|s: &mut State, tx| {
                if let Some(p) = s.property_picker_mut() {
                    p.date_format_pane = true;
                    p.error = None;
                }
                tx.instant();
            });
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
                .child(format_row)
                .child(
                    picker_action_row("date-clear", "Clear", &super::phosphor::X, t).on_press(
                        |s: &mut State| {
                            s.clear_date_picker();
                        },
                    ),
                )
                .instant_changes(),
        );
    } else if let Some(opt_edit) = &p.option_editor {
        let back = picker_action_row(
            "property-option-edit-back",
            "Edit option",
            &super::phosphor::CARET_LEFT,
            t,
        )
        .on_press_tx(|s: &mut State, tx| {
            if let Some(p) = s.property_picker_mut() {
                p.option_editor = None;
                p.input.focused = true;
                p.error = None;
            }
            tx.instant();
        });
        let name_field = super::inputs::field(
            &opt_edit.name_input,
            "property-option-name-input",
            "Option name",
            t,
        )
        .label("Option name")
        .on_edit(|s: &mut State| {
            &mut s
                .property_picker_mut()
                .unwrap()
                .option_editor
                .as_mut()
                .unwrap()
                .name_input
        })
        .on_input_change(|s: &mut State, tx| {
            if let Some(p) = s.property_picker_mut() {
                p.error = None;
            }
            tx.instant();
        })
        .on_input_key(|s: &mut State, event, tx| {
            if event.key == Key::Escape {
                if let Some(p) = s.property_picker_mut() {
                    p.option_editor = None;
                    p.input.focused = true;
                    p.error = None;
                }
                tx.instant();
                return true;
            }
            false
        })
        .on_submit(|s: &mut State, tx| {
            s.save_picker_option_editor(None);
            tx.instant();
        });
        let save_button = button("property-option-save", "Save", t)
            .w_full()
            .center()
            .fill(t.selected)
            .on_press_tx(|s: &mut State, tx| {
                s.save_picker_option_editor(None);
                tx.instant();
            });
        let color_list = option_color_choices(
            "property-color",
            &opt_edit.color,
            t,
            |s, color| {
                s.save_picker_option_editor(Some(color));
            },
        )
        .key("property-color-choices");
        panel = panel
            .child(back)
            .child(name_field.instant_changes())
            .child(save_button)
            .child(el().w_full().h(1.).fill(t.line))
            .child(t.label("Color", t.muted).px(8.))
            .child(color_list.instant_changes());
    } else {
        let mut options = col()
            .key("property-options")
            .w_full()
            .max_h(250.)
            .min_h(0.)
            .shrink()
            .gap(1.)
            .scroll(ScrollSpec {
                stick_bottom: false,
            });
        let matched = s.property_options();
        let show_status_groups = p.kind == PropertyKind::Status
            && matched
                .iter()
                .any(|o| moss_core::default_status_option_color(o).is_some());
        for (i, option) in matched.iter().enumerate() {
            let value = option.clone();
            let is_active = if p.kind == PropertyKind::MultiSelect {
                p.multi_selected.iter().any(|v| v == option)
            } else {
                s.cell.text == *option
            };
            let pill = if p.kind == PropertyKind::Status {
                let (group, bg, dot) = if let Some(explicit) =
                    current_prop.and_then(|prop| explicit_option_color(prop, option))
                {
                    let group = show_status_groups
                        .then(|| status_style(option, t).0)
                        .flatten();
                    (
                        group,
                        option_tag_fill(explicit, t.dark),
                        option_tag_dot(explicit, t.dark),
                    )
                } else if moss_core::default_status_option_color(option).is_some() {
                    let (group, bg, dot) = status_style(option, t);
                    (show_status_groups.then_some(group).flatten(), bg, dot)
                } else {
                    let color_name = current_prop
                        .map(|prop| moss_core::resolved_option_color(prop, option))
                        .unwrap_or("gray");
                    (
                        None,
                        option_tag_fill(color_name, t.dark),
                        option_tag_dot(color_name, t.dark),
                    )
                };
                if let Some(group) = group {
                    options = options.child(
                        t.label(group, t.muted)
                            .px(8.)
                            .pt(if i == 0 { 2. } else { 8. })
                            .pb(2.),
                    );
                }
                row()
                    .h(20.)
                    .max_w(156.)
                    .min_w(0.)
                    .px(6.)
                    .gap(5.)
                    .center_y()
                    .radius(10.)
                    .fill(bg)
                    .child(el().size(7., 7.).radius(4.).fill(dot))
                    .child(t.label(option, t.text).min_w(0.).truncate())
            } else {
                let bg = current_prop
                    .map(|prop| option_tag_style(prop, option, t))
                    .unwrap_or(t.selected);
                row()
                    .h(20.)
                    .max_w(156.)
                    .min_w(0.)
                    .px(6.)
                    .center_y()
                    .radius(4.)
                    .fill(bg)
                    .child(t.label(option, t.text).min_w(0.).truncate())
            };
            let kind = p.kind;
            let main_button = row()
                .key(format!("property-option-{i}"))
                .grow(1.)
                .min_w(0.)
                .h(30.)
                .pl(8.)
                .pr(4.)
                .gap(6.)
                .center_y()
                .role("button")
                .label(option)
                .focusable(true)
                .child(pill)
                .child(el().grow(1.))
                .on_press_tx(move |s: &mut State, tx| {
                    if kind == PropertyKind::MultiSelect {
                        s.toggle_multi_select_value(&value);
                        tx.instant();
                    } else {
                        s.choose_property_value(&value);
                    }
                });
            let opt_for_edit = option.clone();
            let edit_button = row()
                .key(format!("property-option-edit-{i}"))
                .size(24., 24.)
                .center()
                .radius(4.)
                .hover(t.selected)
                .role("button")
                .label(format!("Edit {option}"))
                .help("Edit name and color")
                .focusable(true)
                .group_opacity(0., 1.)
                .child(rune::icon(&super::phosphor::NOTE_PENCIL, 14.).color(t.muted))
                .on_press_tx(move |s: &mut State, tx| {
                    s.open_picker_option_editor(&opt_for_edit);
                    tx.instant();
                });
            let mut option_row = row()
                .hover_group()
                .w_full()
                .h(30.)
                .pr(6.)
                .center_y()
                .gap(2.)
                .radius(4.)
                .hover(t.hover)
                .child(main_button)
                .child(edit_button)
                .children(is_active.then(|| {
                    el().size(18., 24.)
                        .center()
                        .child(super::icons::glyph("✓", 14., t.text))
                }));
            if Some(i) == p.selected {
                option_row = option_row.fill(t.selected);
            }
            options = options.child(option_row);
        }
        let typed = p.input.text.trim().to_owned();
        let can_quick_create =
            !typed.is_empty() && !matched.iter().any(|o| o.eq_ignore_ascii_case(&typed));
        if can_quick_create {
            let create_label = typed.clone();
            let preview_pill = if p.kind == PropertyKind::Status {
                let color_name = current_prop
                    .map(|prop| moss_core::resolved_option_color(prop, &typed))
                    .unwrap_or("gray");
                row()
                    .h(20.)
                    .max_w(140.)
                    .min_w(0.)
                    .px(6.)
                    .gap(5.)
                    .center_y()
                    .radius(10.)
                    .fill(option_tag_fill(color_name, t.dark))
                    .child(
                        el().size(7., 7.)
                            .radius(4.)
                            .fill(option_tag_dot(color_name, t.dark)),
                    )
                    .child(t.label(&typed, t.text).min_w(0.).truncate())
            } else {
                row()
                    .h(20.)
                    .max_w(140.)
                    .min_w(0.)
                    .px(6.)
                    .center_y()
                    .radius(4.)
                    .fill(
                        current_prop
                            .map(|prop| option_tag_style(prop, &typed, t))
                            .unwrap_or(t.selected),
                    )
                    .child(t.label(&typed, t.text).min_w(0.).truncate())
            };
            options = options.child(
                row()
                    .key("property-option-create")
                    .w_full()
                    .h(30.)
                    .px(8.)
                    .gap(6.)
                    .center_y()
                    .radius(4.)
                    .hover(t.hover)
                    .role("button")
                    .label(format!("Create {typed}"))
                    .focusable(true)
                    .child(t.label("Create", t.muted))
                    .child(preview_pill)
                    .on_press_tx(move |s: &mut State, tx| {
                        s.create_and_choose_option(&create_label);
                        tx.instant();
                    }),
            );
        }
        panel = panel.child(options.instant_changes());
    }
    Some(panel)
}

fn date_input(s: &State, edit: &TextEdit, key: &str, name: &str, index: u8, t: Theme) -> El {
    super::inputs::invalid(
        super::inputs::field(edit, key, name, t),
        s.property_picker()
            .is_some_and(|picker| picker.error.is_some()),
        t,
    )
    .label(name)
    .on_edit(move |s: &mut State| {
        let date = s
            .property_picker_mut()
            .unwrap()
            .date
            .as_mut()
            .unwrap();
        match index {
            0 => &mut date.end_input,
            1 => &mut date.start_time_input,
            _ => &mut date.end_time_input,
        }
    })
    .on_input_change(|s: &mut State, tx| {
        s.date_draft_changed();
        if let Some(p) = s.property_picker_mut() {
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
        .px(8.)
        .center_y()
        .gap(8.)
        .radius(4.)
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
