use super::{menu, theme::Theme};
use crate::{state::State, view_management::ViewPane};
use moss_core::ViewKind;
use rune::*;

const KINDS: [ViewKind; 9] = [
    ViewKind::Table,
    ViewKind::Board,
    ViewKind::Calendar,
    ViewKind::Timeline,
    ViewKind::Gallery,
    ViewKind::List,
    ViewKind::Chart,
    ViewKind::Map,
    ViewKind::Dashboard,
];

use super::phosphor::{
    ARROW_DOWN as DOWN, ARROW_UP as UP, ARROWS_OUT_SIMPLE as FULL_PAGE, COLUMNS as GROUP,
    COPY as DUPLICATE, FUNNEL_SIMPLE as FILTER, PLUS as NEW, SIDEBAR_SIMPLE as PEEK,
    SORT_ASCENDING as SORT, SQUARE as CENTER_PEEK, TRASH as DELETE_ITEM,
};

fn item(key: &str, label: &str, value: &str, glyph: &'static Icon, color: Color, t: Theme) -> El {
    let mark = (!value.is_empty()).then_some(&menu::CHEVRON);
    menu::row_item(
        key,
        label,
        rune::icon(glyph, 16.).color(if color == t.error { t.error } else { t.muted }),
        Theme { text: color, ..t },
    )
    .children(
        (!value.is_empty() && value != "›")
            .then(|| t.label(value, t.muted).max_w(88.).min_w(0.).truncate()),
    )
    .child(menu::end(format!("{key}-end"), mark, t))
}
fn choice(
    key: impl Into<String>,
    label: &str,
    glyph: &'static Icon,
    selected: bool,
    t: Theme,
) -> El {
    let key = key.into();
    menu::row_item(key.clone(), label, rune::icon(glyph, 16.).color(t.muted), t).child(menu::end(
        format!("{key}-end"),
        selected.then_some(&menu::CHECK),
        t,
    ))
}

fn close(s: &mut State) {
    if s.view_management
        .as_ref()
        .is_some_and(|e| e.error.is_some())
    {
        return;
    }
    if s.view_management
        .as_ref()
        .is_some_and(|e| e.expected.is_some())
        && !s.autosave_view_name(true)
    {
        return;
    }
    s.view_management = None;
}

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let editor = s.view_management.as_ref()?;
    if s.tab()?.database.as_ref()?.id != editor.database_id {
        return None;
    }
    let anchor = if editor.expected.is_some() || editor.from_settings {
        "db-view-settings"
    } else {
        "db-add-view"
    };
    let mut body = col().w_full();
    match editor.pane {
        ViewPane::Main => {
            body = body.child(
                row()
                    .w_full()
                    .h(32.)
                    .px(10.)
                    .mb(4.)
                    .center_y()
                    .child(t.label("View settings", t.text).grow(1.))
                    .child(
                        row()
                            .key("view-settings-close")
                            .size(20., 20.)
                            .center()
                            .radius(4.)
                            .hover(t.hover)
                            .role("button")
                            .label("Close")
                            .focusable(true)
                            .child(rune::icon(&menu::CLOSE, 16.).color(t.muted))
                            .on_press(|s: &mut State| close(s)),
                    ),
            );
            body = body.child(
                row()
                    .w_full()
                    .px(10.)
                    .gap(8.)
                    .mb(8.)
                    .center_y()
                    .child(menu::icon_slot(
                        "view-name-icon",
                        super::view_icons::element(editor.kind, 16., t.muted),
                    ))
                    .child(
                        super::inputs::field(&s.view_name_input, "view-name-input", "View name", t)
                            .grow(1.)
                            .on_edit(|s: &mut State| &mut s.view_name_input)
                            .on_input_change(|s: &mut State, _| {
                                s.autosave_view_name(false);
                            })
                            .on_submit(|s: &mut State, tx| {
                                s.autosave_view_name(true);
                                tx.instant();
                            }),
                    ),
            );
            body = body.child(
                item(
                    "view-layout",
                    "Layout",
                    editor.kind.label(),
                    super::view_icons::icon(editor.kind),
                    t.text,
                    t,
                )
                .on_press(|s: &mut State| {
                    if !s.autosave_view_name(true) {
                        return;
                    }
                    if let Some(e) = &mut s.view_management {
                        e.pane = match e.kind {
                            ViewKind::Board => ViewPane::BoardLayout,
                            ViewKind::Calendar => ViewPane::CalendarLayout,
                            _ => ViewPane::Layout,
                        };
                    }
                }),
            );
            if KINDS.contains(&editor.kind) {
                body = body.child(
                    item("view-properties", "Properties", "›", &menu::EYE, t.text, t)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if !s.autosave_view_name(true) {
                                return;
                            }
                            s.view_management = None;
                            super::property_visibility::open(s);
                            s.property_visibility_from_view_settings = true;
                        }),
                );
            }
            if KINDS.contains(&editor.kind) {
                for (key, label, pane, glyph) in [
                    ("view-filter", "Filter", ViewPane::Filter, &FILTER),
                    ("view-sort", "Sort", ViewPane::Sort, &SORT),
                    ("view-group", "Group", ViewPane::Group, &GROUP),
                ] {
                    if pane == ViewPane::Group
                        && !matches!(editor.kind, ViewKind::Table | ViewKind::Board)
                    {
                        continue;
                    }
                    body = body.child(item(key, label, "›", glyph, t.text, t).on_press(
                        move |s: &mut State| {
                            if !s.autosave_view_name(true) {
                                return;
                            }
                            if let Some(e) = &mut s.view_management {
                                e.pane = if pane == ViewPane::Group && e.kind == ViewKind::Board {
                                    ViewPane::BoardGroup
                                } else {
                                    pane
                                };
                            }
                        },
                    ));
                }
            }
            if editor.kind == ViewKind::Board {
                body = body.child(
                    item("view-subgroup", "Sub-group", "›", &GROUP, t.text, t).on_press(
                        |s: &mut State| {
                            if !s.autosave_view_name(true) {
                                return;
                            }
                            if let Some(e) = &mut s.view_management {
                                e.pane = ViewPane::BoardSubgroup;
                            }
                        },
                    ),
                );
            }
            body = body.child(
                item("view-open-mode", "Open pages in", "›", &PEEK, t.text, t)
                    .w_full()
                    .on_press(|s: &mut State| {
                        if !s.autosave_view_name(true) {
                            return;
                        }
                        if let Some(e) = &mut s.view_management {
                            e.pane = ViewPane::OpenMode;
                        }
                    }),
            );
            body = body.child(el().w_full().h(1.).fill(t.line).my(4.));
            body = body.child(
                item("view-create-new", "New view", "", &NEW, t.text, t)
                    .on_press(|s: &mut State| s.new_view_from_settings()),
            );
            body = body.child(
                item(
                    "view-duplicate",
                    "Duplicate view",
                    "",
                    &DUPLICATE,
                    t.text,
                    t,
                )
                .w_full()
                .on_press(|s: &mut State| {
                    if s.autosave_view_name(true) {
                        s.duplicate_current_view();
                    }
                }),
            );
            let index = editor
                .expected
                .as_ref()
                .and_then(|view| editor.order.iter().position(|id| id == &view.id));
            if index.is_some_and(|index| index > 0) {
                body = body.child(
                    item("view-move-up", "Move up", "", &UP, t.text, t)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if s.autosave_view_name(true) {
                                s.move_current_view(false);
                            }
                        }),
                );
            }
            if index.is_some_and(|index| index + 1 < editor.order.len()) {
                body = body.child(
                    item("view-move-down", "Move down", "", &DOWN, t.text, t)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if s.autosave_view_name(true) {
                                s.move_current_view(true);
                            }
                        }),
                );
            }
            if editor.confirm_delete {
                body = body.child(
                    item(
                        "view-delete-confirm",
                        "Confirm delete",
                        "",
                        &DELETE_ITEM,
                        t.error,
                        t,
                    )
                    .w_full()
                    .on_press(|s: &mut State| s.delete_current_view()),
                );
                body = body.child(
                    item("view-delete-cancel", "Cancel", "", &menu::CLOSE, t.text, t)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if let Some(e) = &mut s.view_management {
                                e.confirm_delete = false;
                            }
                        }),
                );
            } else {
                body = body.child(
                    item("view-delete", "Delete view", "", &DELETE_ITEM, t.error, t)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if !s.autosave_view_name(true) {
                                return;
                            }
                            if let Some(e) = &mut s.view_management {
                                e.confirm_delete = true;
                            }
                        }),
                );
            }
        }
        pane @ (ViewPane::BoardLayout
        | ViewPane::BoardCardSize
        | ViewPane::BoardCardPreview
        | ViewPane::BoardGroup
        | ViewPane::BoardSubgroup
        | ViewPane::BoardGroupProperty
        | ViewPane::BoardSubgroupProperty
        | ViewPane::BoardProperties) => {
            body = super::board_settings::body(s, pane, t)?;
        }
        ViewPane::CalendarLayout => {
            let tab = s.tab()?;
            let db = tab.database.as_ref()?;
            let view = db.views.get(tab.view)?;
            body = body.child(
                item("calendar-layout-back", "Back", "", &menu::BACK, t.text, t).on_press(
                    |s: &mut State| {
                        if let Some(e) = &mut s.view_management {
                            e.pane = ViewPane::Main;
                        }
                    },
                ),
            );
            body = body.child(
                item(
                    "calendar-layout-type",
                    "Layout",
                    "Calendar",
                    super::view_icons::icon(ViewKind::Calendar),
                    t.text,
                    t,
                )
                .on_press(|s: &mut State| {
                    if let Some(e) = &mut s.view_management {
                        e.pane = ViewPane::Layout;
                    }
                }),
            );
            body = body.child(t.label("Show calendar as", t.muted).px(10.).py(6.));
            for (label, value) in [("Month", "month"), ("Week", "week")] {
                let selected = crate::calendar_view::week(view) == (value == "week");
                body = body.child(
                    choice(
                        format!("calendar-mode-{value}"),
                        label,
                        &super::phosphor::CALENDAR_BLANK,
                        selected,
                        t,
                    )
                    .on_press(move |s: &mut State| {
                        if s.set_calendar_config("calendar_mode", serde_json::json!(value))
                            && value == "week"
                        {
                            let today = chrono::Local::now().date_naive();
                            if let Some(tab) = s.tabs.get_mut(s.active) {
                                use chrono::Datelike;
                                if tab.calendar_month.year() == today.year()
                                    && tab.calendar_month.month() == today.month()
                                {
                                    tab.calendar_month = today;
                                }
                            }
                            s.save_preferences();
                        }
                    }),
                );
            }
            body = body.child(t.label("Show calendar by", t.muted).px(10.).py(6.));
            let selected = crate::calendar_view::date_property(db, view).map(|p| p.id.as_str());
            for property in db
                .properties
                .iter()
                .filter(|p| p.kind == moss_core::PropertyKind::Date)
            {
                let id = property.id.clone();
                body = body.child(
                    choice(
                        format!("calendar-property-{id}"),
                        &property.name,
                        &super::phosphor::CALENDAR_BLANK,
                        selected == Some(id.as_str()),
                        t,
                    )
                    .on_press(move |s: &mut State| {
                        s.set_calendar_config("calendar_date_property", serde_json::json!(id));
                    }),
                );
            }
            body = body.child(
                menu::row_item(
                    "calendar-create-date",
                    "New date property",
                    rune::icon(&super::phosphor::PLUS, 16.).color(t.muted),
                    t,
                )
                .on_press(|s: &mut State| s.create_calendar_date_property()),
            );
        }
        ViewPane::Layout | ViewPane::Create => {
            body = body.child(row().w_full().h(28.).px(10.).center_y().child(t.label(
                if editor.pane == ViewPane::Create {
                    "New view"
                } else {
                    "Layout"
                },
                t.muted,
            )));
            if editor.pane == ViewPane::Create {
                body = body.child(
                    super::inputs::field(&s.view_name_input, "view-name-input", "View name", t)
                        .mb(8.)
                        .on_edit(|s: &mut State| &mut s.view_name_input)
                        .on_submit(|s: &mut State, tx| {
                            s.save_view_name();
                            tx.instant();
                        }),
                );
            }
            for kind in KINDS {
                body = body.child(
                    choice(
                        format!("view-layout-{}", kind.label().to_lowercase()),
                        kind.label(),
                        super::view_icons::icon(kind),
                        editor.kind == kind,
                        t,
                    )
                    .on_press(move |s: &mut State| {
                        if s.view_management
                            .as_ref()
                            .is_some_and(|e| e.pane == ViewPane::Create)
                        {
                            if let Some(e) = &mut s.view_management {
                                if s.view_name_input.text == e.kind.label() {
                                    s.view_name_input.replace_text(kind.label());
                                }
                                e.kind = kind;
                            }
                        } else {
                            s.change_view_layout(kind);
                        }
                    }),
                );
            }
            if editor.pane != ViewPane::Create {
                body = body.child(
                    item("view-back", "Back", "", &menu::BACK, t.text, t)
                        .w_full()
                        .on_press(|s: &mut State| {
                            if let Some(e) = &mut s.view_management {
                                e.pane = ViewPane::Main;
                            }
                        }),
                );
            }
        }
        ViewPane::OpenMode => {
            body = body.child(
                row()
                    .w_full()
                    .h(28.)
                    .px(10.)
                    .center_y()
                    .child(t.label("Open pages in", t.muted)),
            );
            let saved = editor
                .expected
                .as_ref()
                .and_then(|view| view.config.get("open_mode"))
                .and_then(|value| value.as_str())
                .unwrap_or(
                    if matches!(editor.kind, ViewKind::Gallery | ViewKind::Calendar) {
                        "center"
                    } else {
                        "side"
                    },
                );
            for (mode, label) in [
                ("side", "Side peek"),
                ("center", "Center peek"),
                ("full", "Full page"),
            ] {
                body = body.child(
                    choice(
                        format!("view-open-{mode}"),
                        label,
                        match mode {
                            "center" => &CENTER_PEEK,
                            "full" => &FULL_PAGE,
                            _ => &PEEK,
                        },
                        saved == mode,
                        t,
                    )
                    .on_press(move |s: &mut State| s.set_view_open_mode(mode)),
                );
            }
            body = body.child(
                item("view-back", "Back", "", &menu::BACK, t.text, t)
                    .w_full()
                    .on_press(|s: &mut State| {
                        if let Some(e) = &mut s.view_management {
                            e.pane = ViewPane::Main;
                        }
                    }),
            );
        }
        pane => {
            let db = s.tab()?.database.as_ref()?;
            body = body.child(
                row()
                    .w_full()
                    .h(28.)
                    .px(10.)
                    .center_y()
                    .child(t.label("Choose a property", t.muted)),
            );
            for property in db.properties.iter().filter(|property| {
                pane != ViewPane::Group
                    || editor.kind != ViewKind::Board
                    || matches!(
                        property.kind,
                        moss_core::PropertyKind::Status
                            | moss_core::PropertyKind::Select
                            | moss_core::PropertyKind::Text
                            | moss_core::PropertyKind::Number
                            | moss_core::PropertyKind::Checkbox
                            | moss_core::PropertyKind::Date
                            | moss_core::PropertyKind::Url
                            | moss_core::PropertyKind::Email
                            | moss_core::PropertyKind::Phone
                    )
            }) {
                let id = property.id.clone();
                body = body.child(
                    menu::row_item(
                        format!("view-property-{id}"),
                        &property.name,
                        super::property_icons::view(property.kind, t.muted),
                        t,
                    )
                    .child(menu::end(format!("view-property-{id}-end"), None, t))
                    .on_press(move |s: &mut State| {
                        s.view_management = None;
                        s.open_database_property(Some(&id));
                        if let Some(e) = &mut s.database_property_editor {
                            e.anchor_override = Some("db-view-settings".into());
                            if pane == ViewPane::Filter
                                && !matches!(
                                    e.kind,
                                    moss_core::PropertyKind::Title
                                        | moss_core::PropertyKind::Text
                                        | moss_core::PropertyKind::Number
                                        | moss_core::PropertyKind::Select
                                        | moss_core::PropertyKind::Status
                                        | moss_core::PropertyKind::Date
                                        | moss_core::PropertyKind::Checkbox
                                        | moss_core::PropertyKind::Url
                                        | moss_core::PropertyKind::Email
                                        | moss_core::PropertyKind::Phone
                                )
                            {
                                e.filter_op = moss_core::FilterOp::IsEmpty;
                            }
                            e.pane = match pane {
                                ViewPane::Filter => {
                                    crate::database_properties::PropertyPane::Filter
                                }
                                ViewPane::Sort => crate::database_properties::PropertyPane::Sort,
                                _ => crate::database_properties::PropertyPane::Main,
                            };
                        }
                        if pane == ViewPane::Group {
                            s.set_table_group();
                        }
                    }),
                );
            }
            body = body.child(
                item("view-back", "Back", "", &menu::BACK, t.text, t)
                    .w_full()
                    .on_press(|s: &mut State| {
                        if let Some(e) = &mut s.view_management {
                            e.pane = ViewPane::Main;
                        }
                    }),
            );
        }
    }
    Some(
        col()
            .key("view-management-menu")
            .w(menu::WIDTH)
            .max_h(600.)
            .padding(8.)
            .gap(4.)
            .fill(t.menu)
            .radius(8.)
            .border(1., t.line)
            .shadow(4., 16., Color::BLACK.alpha(0.15))
            .role("dialog")
            .label("View settings")
            .focus_group()
            .focus_return(anchor)
            .modal(true, |s: &mut State| close(s))
            .transition(Transition::opacity())
            .children(editor.error.as_ref().map(|error| {
                col()
                    .w_full()
                    .gap(4.)
                    .child(t.label(error, t.text).wrap())
                    .children((editor.kind == ViewKind::Board).then(|| {
                        item(
                            "board-reload-settings",
                            "Reload saved settings",
                            "",
                            &menu::RETRY,
                            t.text,
                            t,
                        )
                        .on_press(|s: &mut State| s.reload_board_settings())
                    }))
                    .children(
                        (editor
                            .expected
                            .as_ref()
                            .is_none_or(|view| s.view_name_input.text.trim() != view.name))
                        .then(|| {
                            item("view-retry", "Retry name", "", &menu::RETRY, t.text, t)
                                .w_full()
                                .on_press(|s: &mut State| {
                                    if s.view_management
                                        .as_ref()
                                        .is_some_and(|e| e.expected.is_none())
                                    {
                                        s.save_view_name();
                                    } else {
                                        s.autosave_view_name(true);
                                    }
                                })
                        }),
                    )
                    .child(
                        item("view-discard", "Discard draft", "", &menu::CLOSE, t.text, t)
                            .w_full()
                            .on_press(|s: &mut State| s.view_management = None),
                    )
            }))
            .child(
                body.max_h(if editor.error.is_some() { 440. } else { 520. })
                    .min_h(0.)
                    .shrink()
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .instant_changes(),
            )
            .children((editor.pane == ViewPane::Create).then(|| {
                item("view-create", "Create view", "", &NEW, t.text, t)
                    .w_full()
                    .on_press(|s: &mut State| {
                        s.save_view_name();
                    })
            }))
            .anchored_fit(anchor, Side::Bottom, Align::End, 4.),
    )
}
