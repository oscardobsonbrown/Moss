use super::theme::{Theme, button, label};
use crate::database_properties::PropertyPane;
use crate::state::State;
use moss_core::{FilterOp, Property, PropertyKind};
use rune::*;

const TYPES: [(PropertyKind, &str, &str); 13] = [
    (PropertyKind::Text, "text", "Text"),
    (PropertyKind::Number, "number", "Number"),
    (PropertyKind::Select, "select", "Select"),
    (PropertyKind::MultiSelect, "multi-select", "Multi-select"),
    (PropertyKind::Status, "status", "Status"),
    (PropertyKind::Date, "date", "Date"),
    (PropertyKind::Checkbox, "checkbox", "Checkbox"),
    (PropertyKind::Url, "url", "URL"),
    (PropertyKind::Email, "email", "Email"),
    (PropertyKind::Phone, "phone", "Phone"),
    (PropertyKind::Formula, "formula", "Formula"),
    (PropertyKind::Relation, "relation", "Relation"),
    (PropertyKind::Rollup, "rollup", "Rollup"),
];

use super::phosphor::{
    ARROW_LINE_LEFT as INSERT_LEFT, ARROW_LINE_RIGHT as INSERT_RIGHT, CARET_LEFT as BACK,
    CARET_RIGHT as CHEVRON, FUNNEL_SIMPLE as FILTER, PUSH_PIN as FREEZE, SIGMA as CALCULATE,
    SORT_ASCENDING as SORT, SQUARES_FOUR as GROUP, TEXT_ALIGN_LEFT as WRAP,
};

fn type_list(active: PropertyKind, max_h: Option<f32>, t: Theme) -> El {
    let mut list = col().w_full().min_h(0.).shrink();
    if let Some(h) = max_h {
        list = list.max_h(h).scroll(ScrollSpec {
            stick_bottom: false,
        });
    }
    list.children(TYPES.into_iter().map(|(kind, key, name)| {
        let selected = active == kind;
        row()
            .key(format!("property-type-{key}"))
            .w_full()
            .h(32.)
            .px(10.)
            .gap(8.)
            .center_y()
            .radius(4.)
            .fill(if selected { t.selected } else { t.menu })
            .hover(t.hover)
            .role("menuitemradio")
            .label(name)
            .checked(selected)
            .focusable(true)
            .child(
                el().size(20., 20.)
                    .center()
                    .child(super::property_icons::view(kind, t.muted)),
            )
            .child(
                text(name)
                    .font(TextStyle::new(14., 400.).lh(20.))
                    .color(t.text)
                    .grow(1.),
            )
            .children(selected.then(|| {
                el().size(20., 20.)
                    .center()
                    .child(super::icons::glyph("✓", 14., t.text))
            }))
            .on_press_tx(move |s: &mut State, tx| {
                s.choose_property_type(kind);
                tx.instant();
            })
    }))
}

fn action(key: &str, name: &str, icon: &'static Icon, t: Theme) -> El {
    row()
        .key(key)
        .w_full()
        .h(28.)
        .px(8.)
        .gap(6.)
        .center_y()
        .radius(4.)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label(name)
        .focusable(true)
        .child(
            el().size(20., 20.)
                .center()
                .child(rune::icon(icon, 16.).color(t.muted)),
        )
        .child(label(name, t.text).grow(1.).truncate())
}

pub fn header(property: &Property, title: bool, width: f32, t: Theme) -> El {
    let id = property.id.clone();
    row()
        .key(if title {
            "table-header-title".into()
        } else {
            format!("table-header-{id}")
        })
        .w(width)
        .min_h(38.)
        .px(10.)
        .gap(6.)
        .center_y()
        .border(1., t.line)
        .hover(t.hover)
        .role("button")
        .label(format!("Edit {} property", property.name))
        .focusable(true)
        .child(super::property_icons::header(property.kind, t.muted))
        .child(t.label(&property.name, t.muted).min_w(0.).truncate())
        .on_press(move |s: &mut State| s.open_database_property(Some(&id)))
}

pub fn add_button(t: Theme) -> El {
    row()
        .key("table-add-property")
        .size(40., 38.)
        .center()
        .border(1., t.line)
        .hover(t.hover)
        .press(t.selected)
        .role("button")
        .label("Add property")
        .focusable(true)
        .child(super::icons::glyph("+", 18., t.muted))
        .on_press(|s: &mut State| s.open_database_property(None))
}

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let editor = s.database_property_editor.as_ref()?;
    let property = editor.property.as_ref();
    let is_new = property.is_none();
    let anchor = editor
        .anchor_override
        .clone()
        .unwrap_or_else(|| match property {
            Some(p) if p.kind == PropertyKind::Title => "table-header-title".to_owned(),
            Some(p) => format!("table-header-{}", p.id),
            None => editor
                .insert_relative
                .as_ref()
                .map(|(id, _)| {
                    let title = s.tab().and_then(|t| t.database.as_ref()).is_some_and(|d| {
                        d.properties
                            .iter()
                            .any(|p| p.id == *id && p.kind == PropertyKind::Title)
                    });
                    if title {
                        "table-header-title".to_owned()
                    } else {
                        format!("table-header-{id}")
                    }
                })
                .unwrap_or_else(|| "table-add-property".to_owned()),
        });
    let mut body = col()
        .key("property-menu-scroll")
        .w_full()
        .min_h(0.)
        .shrink()
        .gap(4.)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .instant_changes();
    if editor.confirm_delete {
        let name = property.map_or("property", |p| p.name.as_str());
        body = body
            .child(text(format!("Delete {name}?")).font(TextStyle::new(14., 600.)).color(t.text).wrap())
            .child(label("This deletes the property and its values from every record. This cannot be undone.", t.muted).wrap())
            .child(row().w_full().gap(6.)
                .child(button("property-delete-cancel", "Cancel", t).grow(1.).center()
                    .on_press(|s: &mut State| {
                        if let Some(editor) = &mut s.database_property_editor { editor.confirm_delete = false; }
                    }))
                .child(row().key("property-delete-confirm").grow(1.).h(30.).center().radius(4.).fill(t.error)
                    .hover(t.error.alpha(0.85)).role("button").label("Delete property").focusable(true)
                    .child(text("Delete property").font(TextStyle::new(14., 500.)).color(Color::WHITE))
                    .on_press_tx(|s: &mut State, tx| { s.delete_database_property(); tx.instant(); })));
    } else if editor.pane == PropertyPane::ConfirmType {
        let name = property.map_or("property", |p| p.name.as_str());
        let from_kind = property.map_or(editor.kind, |p| p.kind);
        let target_kind = editor.pending_kind.unwrap_or(editor.kind);
        let target_label = type_name(target_kind);
        body = body
            .child(
                text(format!("Change {name} to {target_label}?"))
                    .font(TextStyle::new(14., 600.))
                    .color(t.text)
                    .wrap(),
            )
            .child(label(type_change_warning(from_kind, target_kind), t.muted).wrap())
            .child(
                row()
                    .w_full()
                    .gap(6.)
                    .child(
                        button("property-type-cancel", "Cancel", t)
                            .grow(1.)
                            .center()
                            .on_press_tx(|s: &mut State, tx| {
                                s.cancel_change_property_type();
                                tx.instant();
                            }),
                    )
                    .child(
                        row()
                            .key("property-type-confirm")
                            .grow(1.)
                            .h(30.)
                            .center()
                            .radius(4.)
                            .fill(t.blue)
                            .hover(t.blue.alpha(0.85))
                            .role("button")
                            .label("Change type")
                            .focusable(true)
                            .child(
                                text("Change type")
                                    .font(TextStyle::new(14., 500.))
                                    .color(Color::WHITE),
                            )
                            .on_press_tx(|s: &mut State, tx| {
                                s.confirm_change_property_type();
                                tx.instant();
                            }),
                    ),
            );
    } else if editor.pane == PropertyPane::Type {
        body = body
            .child(
                action("property-type-back", "Type", &BACK, t)
                    .h(28.)
                    .w_full()
                    .on_press_tx(|s: &mut State, tx| {
                        if let Some(e) = &mut s.database_property_editor {
                            e.pane = PropertyPane::Main;
                            e.error = None;
                        }
                        tx.instant();
                    }),
            )
            .child(el().h(1.).w_full().fill(t.line))
            .child(type_list(editor.kind, Some(340.), t));
    } else if editor.pane == PropertyPane::OptionColor {
        let opt_name = editor.color_option.clone().unwrap_or_default();
        let active_color = property
            .map(|p| moss_core::resolved_option_color(p, &opt_name))
            .unwrap_or("gray");
        let opt = opt_name.clone();
        let colors = super::property_value::option_color_choices(
            "editor-color",
            active_color,
            t,
            move |s, color| {
                s.set_editor_option_color(&opt, color);
            },
        );
        body = body
            .child(
                action(
                    "editor-option-color-back",
                    &format!("Color: {opt_name}"),
                    &BACK,
                    t,
                )
                .h(28.)
                .w_full()
                .on_press_tx(|s: &mut State, tx| {
                    if let Some(e) = &mut s.database_property_editor {
                        e.pane = PropertyPane::Main;
                        e.color_option = None;
                        e.error = None;
                    }
                    tx.instant();
                }),
            )
            .child(el().h(1.).w_full().fill(t.line))
            .child(colors);
    } else if editor.pane == PropertyPane::DateFormat {
        let active_format = property
            .and_then(|p| p.config.get("date_format"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("full");
        let formats = super::property_value::date_format_choices(
            "editor-date-format",
            active_format,
            t,
            |s, format| {
                s.set_editor_date_format(format);
            },
        );
        body = body
            .child(
                action("editor-date-format-back", "Date format", &BACK, t)
                    .h(28.)
                    .w_full()
                    .on_press_tx(|s: &mut State, tx| {
                        if let Some(e) = &mut s.database_property_editor {
                            e.pane = PropertyPane::Main;
                            e.error = None;
                        }
                        tx.instant();
                    }),
            )
            .child(el().h(1.).w_full().fill(t.line))
            .child(formats);
    } else if editor.pane == PropertyPane::Filter {
        let text_kind = property.is_some_and(|p| {
            matches!(
                p.kind,
                PropertyKind::Title
                    | PropertyKind::Text
                    | PropertyKind::Url
                    | PropertyKind::Email
                    | PropertyKind::Phone
            )
        });
        let numeric = property.is_some_and(|p| p.kind == PropertyKind::Number);
        let scalar = property.is_some_and(|p| {
            matches!(
                p.kind,
                PropertyKind::Title
                    | PropertyKind::Text
                    | PropertyKind::Number
                    | PropertyKind::Select
                    | PropertyKind::Status
                    | PropertyKind::Date
                    | PropertyKind::Checkbox
                    | PropertyKind::Url
                    | PropertyKind::Email
                    | PropertyKind::Phone
            )
        });
        let ops = [
            (FilterOp::Eq, "Equals", "eq"),
            (FilterOp::Ne, "Does not equal", "ne"),
            (FilterOp::Contains, "Contains", "contains"),
            (FilterOp::NotContains, "Does not contain", "not-contains"),
            (FilterOp::Gt, "Greater than", "gt"),
            (FilterOp::Lt, "Less than", "lt"),
            (FilterOp::IsEmpty, "Is empty", "empty"),
            (FilterOp::IsNotEmpty, "Is not empty", "not-empty"),
        ];
        body =
            body.child(label("Filter", t.text))
                .children(
                    ops.into_iter()
                        .filter(|(op, _, _)| match op {
                            FilterOp::Eq | FilterOp::Ne => scalar,
                            FilterOp::Contains | FilterOp::NotContains => text_kind,
                            FilterOp::Gt | FilterOp::Lt => numeric,
                            _ => true,
                        })
                        .map(|(op, name, id)| {
                            let active = editor.filter_op == op;
                            button(format!("table-filter-op-{id}"), name, t)
                                .gap(6.)
                                .children(active.then(|| super::icons::glyph("✓", 14., t.text)))
                                .h(28.)
                                .w_full()
                                .on_press(move |s: &mut State| {
                                    if let Some(e) = &mut s.database_property_editor {
                                        e.filter_op = op;
                                        e.error = None;
                                    }
                                })
                        }),
                )
                .children(
                    (!matches!(editor.filter_op, FilterOp::IsEmpty | FilterOp::IsNotEmpty)).then(
                        || {
                            super::inputs::field(
                                &editor.filter_input,
                                "table-filter-value",
                                "Value",
                                t,
                            )
                            .on_edit(|s: &mut State| {
                                &mut s.database_property_editor.as_mut().unwrap().filter_input
                            })
                            .on_submit(|s: &mut State, tx| {
                                s.set_table_filter();
                                tx.instant();
                            })
                        },
                    ),
                )
                .child(
                    button("table-filter-apply", "Apply filter", t)
                        .w_full()
                        .on_press(|s: &mut State| {
                            s.set_table_filter();
                        }),
                )
                .child(
                    button("table-filter-remove", "Remove filter", t)
                        .h(28.)
                        .w_full()
                        .on_press(|s: &mut State| {
                            s.clear_table_filter();
                        }),
                )
                .child(button("table-filter-back", "Back", t).w_full().on_press(
                    |s: &mut State| {
                        if let Some(e) = &mut s.database_property_editor {
                            e.pane = PropertyPane::Main;
                        }
                    },
                ));
    } else if editor.pane == PropertyPane::Calculate {
        let numeric = property.is_some_and(|p| {
            matches!(
                p.kind,
                PropertyKind::Number | PropertyKind::Formula | PropertyKind::Rollup
            )
        });
        let choices = [
            (None, "None"),
            (Some("count_all"), "Count all"),
            (Some("count_values"), "Count values"),
            (Some("count_empty"), "Count empty"),
            (Some("count_unique"), "Count unique"),
            (Some("sum"), "Sum"),
            (Some("average"), "Average"),
            (Some("min"), "Minimum"),
            (Some("max"), "Maximum"),
        ];
        body = body
            .children(
                choices
                    .into_iter()
                    .filter(|(operation, _)| {
                        numeric || !matches!(operation, Some("sum" | "average" | "min" | "max"))
                    })
                    .map(|(operation, name)| {
                        button(
                            format!("table-calculate-{}", operation.unwrap_or("none")),
                            name,
                            t,
                        )
                        .h(28.)
                        .w_full()
                        .on_press(move |s: &mut State| {
                            s.set_table_calculation(operation);
                        })
                    }),
            )
            .child(
                button("table-calculate-back", "Back", t)
                    .h(28.)
                    .w_full()
                    .on_press(|s: &mut State| {
                        if let Some(e) = &mut s.database_property_editor {
                            e.pane = PropertyPane::Main;
                        }
                    }),
            );
    } else if editor.pane == PropertyPane::Sort {
        body = body
            .children(
                [
                    (Some(false), "Ascending", "table-sort-ascending"),
                    (Some(true), "Descending", "table-sort-descending"),
                    (None, "Remove sort", "table-sort-remove"),
                ]
                .into_iter()
                .map(|(direction, name, key)| {
                    button(key, name, t)
                        .h(28.)
                        .w_full()
                        .on_press(move |s: &mut State| {
                            s.set_table_sort(direction);
                        })
                }),
            )
            .child(
                button("table-sort-back", "Back", t)
                    .h(28.)
                    .w_full()
                    .on_press(|s: &mut State| {
                        if let Some(e) = &mut s.database_property_editor {
                            e.pane = PropertyPane::Main;
                        }
                    }),
            );
    } else {
        let name_input = if is_new {
            super::inputs::invalid(
                super::inputs::field(
                    &editor.name_input,
                    "property-name-input",
                    "Property name",
                    t,
                ),
                editor.error.is_some(),
                t,
            )
        } else {
            super::inputs::line(
                &editor.name_input,
                TextStyle::new(14., 400.).lh(20.),
                t,
            )
            .key("property-name-input")
            .grow(1.)
            .min_w(0.)
            .placeholder("Property name", t.input_hint)
        }
        .label("Property name")
        .on_edit(|s: &mut State| &mut s.database_property_editor.as_mut().unwrap().name_input)
        .on_submit(|s: &mut State, tx| {
            s.save_database_property();
            tx.instant();
        });
        let name = if is_new {
            name_input
        } else {
            row()
                .w_full()
                .h(32.)
                .px(10.)
                .gap(6.)
                .center_y()
                .radius(6.)
                .fill(t.sidebar)
                .border(
                    1.,
                    if editor.error.is_some() {
                        t.error
                    } else {
                        t.line
                    },
                )
                .focus_within_ring(
                    1.,
                    -1.,
                    if editor.error.is_some() {
                        t.error
                    } else {
                        t.blue
                    },
                )
                .child(
                    el().size(20., 20.)
                        .center()
                        .help(format!("{} property", type_name(editor.kind)))
                        .child(super::property_icons::view(editor.kind, t.muted)),
                )
                .child(name_input)
        };
        body = body.child(name);
        if is_new {
            body = body.child(label("Property type", t.muted).px(10.)).child(
                type_list(
                    editor.kind,
                    Some(
                        if matches!(
                            editor.kind,
                            PropertyKind::Formula | PropertyKind::Relation | PropertyKind::Rollup
                        ) {
                            112.
                        } else {
                            248.
                        },
                    ),
                    t,
                ),
            );
            if matches!(editor.kind, PropertyKind::Select | PropertyKind::MultiSelect) {
                body = body.child(
                    super::inputs::field(
                        &editor.options_input,
                        "property-options-input",
                        "Options, separated by commas",
                        t,
                    )
                    .label("Select options")
                    .on_edit(|s: &mut State| {
                        &mut s.database_property_editor.as_mut().unwrap().options_input
                    })
                    .on_submit(|s: &mut State, tx| {
                        s.save_database_property();
                        tx.instant();
                    }),
                );
            }
        }
        if !is_new
            && !editor.rename_only
            && property.is_some_and(|p| moss_core::property_type_convertible(p.kind))
        {
            let current_type = type_name(editor.kind);
            body = body.child(
                row()
                    .key("property-type-trigger")
                    .w_full()
                    .h(28.)
                    .px(8.)
                    .gap(6.)
                    .center_y()
                    .radius(4.)
                    .hover(t.hover)
                    .press(t.selected)
                    .role("button")
                    .label(format!("Type: {current_type}"))
                    .focusable(true)
                    .child(
                        el().size(20., 20.)
                            .center()
                            .child(super::property_icons::view(editor.kind, t.muted)),
                    )
                    .child(label("Type", t.text).grow(1.).truncate())
                    .child(label(current_type, t.muted).shrink())
                    .child(
                        el().size(16., 20.)
                            .center()
                            .child(rune::icon(&CHEVRON, 14.).color(t.muted)),
                    )
                    .on_press_tx(|s: &mut State, tx| {
                        if let Some(e) = &mut s.database_property_editor {
                            e.pane = PropertyPane::Type;
                            e.error = None;
                        }
                        tx.instant();
                    }),
            );
        }
        if !is_new && !editor.rename_only {
            if let Some(prop) = property.filter(|p| {
                p.kind == PropertyKind::Date && editor.kind == PropertyKind::Date
            }) {
                let current_format = prop
                    .config
                    .get("date_format")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("full");
                let format_label = super::property_value::date_format_label(current_format);
                body = body.child(
                    row()
                        .key("property-date-format-trigger")
                        .w_full()
                        .h(28.)
                        .px(8.)
                        .gap(6.)
                        .center_y()
                        .radius(4.)
                        .hover(t.hover)
                        .press(t.selected)
                        .role("button")
                        .label(format!("Date format: {format_label}"))
                        .focusable(true)
                        .child(
                            el().size(20., 20.)
                                .center()
                                .child(rune::icon(&super::phosphor::CALENDAR_BLANK, 16.).color(t.muted)),
                        )
                        .child(label("Date format", t.text).grow(1.).truncate())
                        .child(label(format_label, t.muted).shrink())
                        .child(
                            el().size(16., 20.)
                                .center()
                                .child(rune::icon(&CHEVRON, 14.).color(t.muted)),
                        )
                        .on_press_tx(|s: &mut State, tx| {
                            if let Some(e) = &mut s.database_property_editor {
                                e.pane = PropertyPane::DateFormat;
                                e.error = None;
                            }
                            tx.instant();
                        }),
                );
            }
            if let Some(prop) = property.filter(|p| {
                matches!(
                    p.kind,
                    PropertyKind::Select | PropertyKind::MultiSelect | PropertyKind::Status
                ) && editor.kind == p.kind
            }) {
                let options = moss_core::property_options(prop);
                if !options.is_empty() {
                    let list = col()
                        .w_full()
                        .gap(2.)
                        .children(options.into_iter().enumerate().map(|(i, opt)| {
                            let bg = super::property_value::option_tag_fill(&opt.color, t.dark);
                            let opt_click = opt.name.clone();
                            row()
                                .key(format!("editor-option-color-{i}"))
                                .w_full()
                                .h(28.)
                                .px(8.)
                                .gap(6.)
                                .center_y()
                                .radius(4.)
                                .hover(t.hover)
                                .role("button")
                                .label(format!("Change color for {}", opt.name))
                                .focusable(true)
                                .child(
                                    row()
                                        .h(20.)
                                        .max_w(190.)
                                        .min_w(0.)
                                        .px(6.)
                                        .center_y()
                                        .radius(4.)
                                        .fill(bg)
                                        .child(t.label(&opt.name, t.text).min_w(0.).truncate()),
                                )
                                .child(el().grow(1.))
                                .child(
                                    el().size(16., 20.)
                                        .center()
                                        .child(rune::icon(&CHEVRON, 14.).color(t.muted)),
                                )
                                .on_press_tx(move |s: &mut State, tx| {
                                    if let Some(e) = &mut s.database_property_editor {
                                        e.color_option = Some(opt_click.clone());
                                        e.pane = PropertyPane::OptionColor;
                                        e.error = None;
                                    }
                                    tx.instant();
                                })
                        }));
                    body = body
                        .child(label("Options", t.muted).px(8.))
                        .child(list);
                }
            }
        }
        if !editor.rename_only {
            body = body.children(super::advanced_properties::editor_fields(s, t));
        }
        body = body.child(
            button(
                "property-save",
                if is_new { "Add property" } else { "Save" },
                t,
            )
            .w_full()
            .center()
            .fill(t.selected)
            .on_press_tx(|s: &mut State, tx| {
                s.save_database_property();
                tx.instant();
            }),
        );
        if !editor.rename_only && property.is_some_and(|p| p.kind != PropertyKind::Title) {
            body = body.child(el().h(1.).w_full().fill(t.line)).child(
                button(
                    "property-delete",
                    "Delete property",
                    Theme { text: t.error, ..t },
                )
                .w_full()
                .on_press(|s: &mut State| {
                    if let Some(editor) = &mut s.database_property_editor {
                        editor.confirm_delete = true;
                        editor.error = None;
                    }
                }),
            );
        }
        if let Some(property) = property.filter(|_| {
            !editor
                .anchor_override
                .as_deref()
                .is_some_and(|anchor| anchor.starts_with("peek-") || anchor.starts_with("page-"))
        }) {
            let config = s
                .tab()
                .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
                .map(|v| &v.config);
            let wrapped = config
                .and_then(|v| v.get("wrap_cells"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let frozen = config
                .and_then(|v| v.get("freeze_through"))
                .and_then(|v| v.as_str())
                == Some(property.id.as_str());
            let prop_id = property.id.clone();
            let grouped = s
                .tab()
                .and_then(|tab| tab.database.as_ref().and_then(|db| db.views.get(tab.view)))
                .and_then(|view| view.query.group_by.as_deref())
                == Some(property.id.as_str());
            body = body.child(el().h(1.).w_full().fill(t.line));
            if property.kind == PropertyKind::Title {
                let visible = config
                    .and_then(|v| v.get("show_page_icon"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                body = body.child(
                    row()
                        .key("table-show-page-icon")
                        .px(8.)
                        .gap(6.)
                        .center_y()
                        .radius(4.)
                        .hover(t.hover)
                        .role("switch")
                        .label("Show page icon")
                        .checked(visible)
                        .focusable(true)
                        .h(28.)
                        .w_full()
                        .child(el().size(20., 20.))
                        .child(label("Show page icon", t.text).grow(1.))
                        .child(
                            row()
                                .size(28., 16.)
                                .px(2.)
                                .center_y()
                                .radius(8.)
                                .fill(if visible { t.blue } else { t.faint })
                                .children(visible.then(|| el().grow(1.)))
                                .child(el().size(12., 12.).radius(6.).fill(Color::WHITE)),
                        )
                        .on_press(move |s: &mut State| {
                            s.set_table_config("show_page_icon", (!visible).into());
                        }),
                );
            }
            body = body
                .child(
                    action("table-filter", "Filter", &FILTER, t)
                        .h(28.)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if let Some(e) = &mut s.database_property_editor {
                                e.pane = PropertyPane::Filter;
                                if !matches!(
                                    e.kind,
                                    PropertyKind::Title
                                        | PropertyKind::Text
                                        | PropertyKind::Number
                                        | PropertyKind::Select
                                        | PropertyKind::Status
                                        | PropertyKind::Date
                                        | PropertyKind::Checkbox
                                        | PropertyKind::Url
                                        | PropertyKind::Email
                                        | PropertyKind::Phone
                                ) {
                                    e.filter_op = FilterOp::IsEmpty;
                                }
                                e.filter_input.focused = true;
                            }
                        }),
                )
                .child(
                    action("table-sort", "Sort", &SORT, t)
                        .h(28.)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if let Some(e) = &mut s.database_property_editor {
                                e.pane = PropertyPane::Sort;
                            }
                        }),
                )
                .child(
                    action(
                        "table-group",
                        if grouped { "Ungroup" } else { "Group" },
                        &GROUP,
                        t,
                    )
                    .h(28.)
                    .w_full()
                    .on_press(|s: &mut State| {
                        s.set_table_group();
                    }),
                )
                .child(
                    action("table-calculate", "Calculate", &CALCULATE, t)
                        .h(28.)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if let Some(e) = &mut s.database_property_editor {
                                e.pane = PropertyPane::Calculate;
                            }
                        }),
                )
                .child(
                    action(
                        "table-freeze",
                        if frozen {
                            "Unfreeze"
                        } else {
                            "Freeze through column"
                        },
                        &FREEZE,
                        t,
                    )
                    .h(28.)
                    .w_full()
                    .on_press(move |s: &mut State| {
                        s.set_table_config(
                            "freeze_through",
                            if frozen {
                                serde_json::Value::Null
                            } else {
                                prop_id.clone().into()
                            },
                        );
                    }),
                )
                .child(
                    action(
                        "table-wrap",
                        if wrapped {
                            "Unwrap content"
                        } else {
                            "Wrap content"
                        },
                        &WRAP,
                        t,
                    )
                    .h(28.)
                    .w_full()
                    .on_press(move |s: &mut State| {
                        s.set_table_config("wrap_cells", (!wrapped).into());
                    }),
                )
                .child(
                    action("table-insert-left", "Insert left", &INSERT_LEFT, t)
                        .h(28.)
                        .w_full()
                        .on_press(|s: &mut State| s.insert_table_property(false)),
                )
                .child(
                    action("table-insert-right", "Insert right", &INSERT_RIGHT, t)
                        .h(28.)
                        .w_full()
                        .on_press(|s: &mut State| s.insert_table_property(true)),
                );
        }
    }
    body = body.children(
        editor
            .error
            .as_ref()
            .map(|error| label(error, t.error).wrap()),
    );
    Some(
        col()
            .key("database-property-menu")
            .w(280.)
            .max_h(510.)
            .min_h(0.)
            .padding(8.)
            .fill(t.menu)
            .radius(9.)
            .border(1., t.line)
            .shadow(4., 16., Color::BLACK.alpha(0.18))
            .role("dialog")
            .label(if is_new {
                "New property"
            } else {
                "Edit property"
            })
            .focus_return(anchor.clone())
            .modal(true, |s: &mut State| s.close_database_property())
            .transition(Transition::opacity())
            .child(body)
            .anchored_fit(anchor, Side::Bottom, Align::Start, 4.),
    )
}

fn type_change_warning(from: PropertyKind, to: PropertyKind) -> &'static str {
    if matches!(from, PropertyKind::Formula | PropertyKind::Rollup) {
        return match to {
            PropertyKind::Formula | PropertyKind::Rollup | PropertyKind::Relation => {
                "This replaces the current calculation and clears existing values for this property."
            }
            PropertyKind::Date => {
                "This removes the calculation. Current values that match a date format are kept; other values are cleared."
            }
            PropertyKind::Number => {
                "This removes the calculation. Current numeric values are kept; other values are cleared."
            }
            PropertyKind::Checkbox => {
                "This removes the calculation. Values matching true or false are kept; other values are cleared."
            }
            _ => "This removes the calculation and keeps the current values as static data.",
        };
    }
    if from == PropertyKind::Relation {
        return match to {
            PropertyKind::Formula | PropertyKind::Rollup | PropertyKind::Relation => {
                "This removes the existing relation links and clears values for this property."
            }
            PropertyKind::Date | PropertyKind::Number | PropertyKind::Checkbox => {
                "This removes the existing relation links and clears incompatible values."
            }
            _ => "This removes the existing relation links and keeps the related item names as text.",
        };
    }
    match to {
        PropertyKind::Date => {
            "Existing values that match a date format will convert to dates. Values that do not match will be cleared."
        }
        PropertyKind::Number => {
            "Existing numeric values will convert to numbers. Non-numeric values will be cleared."
        }
        PropertyKind::Checkbox => {
            "Values matching true or false will convert to checkboxes. Other values will be cleared."
        }
        PropertyKind::Formula => {
            "This clears existing stored values for this property and calculates values from the formula."
        }
        PropertyKind::Rollup => {
            "This clears existing stored values for this property and calculates values from the rollup."
        }
        PropertyKind::Relation => {
            "This clears existing stored values for this property so records can link to the related database."
        }
        _ => "Existing values will be converted to the new property type.",
    }
}

fn type_name(kind: PropertyKind) -> &'static str {
    TYPES
        .iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, _, name)| *name)
        .unwrap_or(match kind {
            PropertyKind::Title => "Title",
            PropertyKind::MultiSelect => "Multi-select",
            PropertyKind::Person => "Person",
            PropertyKind::Files => "Files",
            PropertyKind::Relation => "Relation",
            PropertyKind::Rollup => "Rollup",
            PropertyKind::Formula => "Formula",
            PropertyKind::CreatedTime => "Created time",
            PropertyKind::EditedTime => "Edited time",
            PropertyKind::UniqueId => "ID",
            PropertyKind::Place => "Place",
            PropertyKind::Button => "Button",
            _ => "Property",
        })
}
