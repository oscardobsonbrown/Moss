use super::{database, phosphor, theme::Theme};
use crate::{
    board_interaction::{DropZone, MenuTarget},
    state::{State, Tab},
};
use moss_core::{Database, PageCover, PropertyKind, Record, board::BoardProjection};
use rune::*;
use serde_json::Value;
use std::rc::Rc;

pub fn width(config: &Value) -> f32 {
    match config.get("card_size").and_then(Value::as_str) {
        Some("small") => 200.,
        Some("large") => 320.,
        _ => 260.,
    }
}
pub fn estimated_height(db: &Database, tab: &Tab, record: &Record) -> f32 {
    let config = &db.views[tab.view].config;
    let properties = db
        .properties
        .iter()
        .filter(|p| {
            p.kind != PropertyKind::Title
                && super::property_visibility::visible(db, tab.view, &p.id)
        })
        .count();
    let preview = match config.get("card_preview").and_then(Value::as_str) {
        Some("content") => 104.,
        Some("cover") => 112.,
        _ => 0.,
    };
    let wrap = config
        .get("wrap_cards")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let title = if wrap && record.title.len() as f32 > (width(config) - 70.) / 7. {
        20.
    } else {
        0.
    };
    42. + title + properties as f32 * 29. + preview
}
fn preview(
    s: &State,
    record: &Record,
    projection: &BoardProjection,
    config: &Value,
    w: f32,
    t: Theme,
    floating: bool,
) -> Option<El> {
    match config
        .get("card_preview")
        .and_then(Value::as_str)
        .unwrap_or("none")
    {
        "content" => Some(
            col()
                .key(format!("board-preview-{}", record.id))
                .w_full()
                .h(104.)
                .padding(12.)
                .clip_rounded(0.)
                .fill(t.sidebar)
                .child(
                    t.label(
                        projection
                            .previews
                            .get(&record.id)
                            .map(String::as_str)
                            .unwrap_or(""),
                        t.muted,
                    )
                    .w_full()
                    .wrap(),
                ),
        ),
        "cover" => {
            let cover = projection.covers.get(&record.id)?;
            let mut frame = el()
                .key(format!("board-preview-{}", record.id))
                .w_full()
                .h(112.)
                .clip_rounded(0.)
                .fill(t.sidebar);
            match cover {
                PageCover::Color { color } => {
                    if let Some(color) = super::page_cover::parse_color(color) {
                        frame = frame.fill(color);
                    }
                }
                PageCover::Gradient { start, end } => {
                    if let (Some(a), Some(b)) = (
                        super::page_cover::parse_color(start),
                        super::page_cover::parse_color(end),
                    ) {
                        frame = frame.gradient(a, b);
                    }
                }
                PageCover::Image { asset, position } => {
                    let id = record.id.clone();
                    let asset_copy = asset.clone();
                    if !floating {
                        frame = frame.on_drawn_layout_tx(move |s: &mut State, r, tx| {
                            s.database_views
                                .board_ui
                                .covers
                                .insert(id.clone(), (asset_copy.clone(), r));
                            tx.live_only();
                        });
                    }
                    if let Some(image) = s.page_cover.images.get(asset) {
                        if let Some([iw, ih]) = image.size() {
                            let scale = (w / iw as f32).max(112. / ih as f32);
                            let iw = iw as f32 * scale;
                            let ih = ih as f32 * scale;
                            frame = frame.child(
                                el().absolute((w - iw) / 2., -(ih - 112.) * position.clamp(0., 1.))
                                    .size(iw, ih)
                                    .pixels(image.pixels()),
                            );
                        }
                    }
                }
            }
            Some(frame)
        }
        _ => None,
    }
}
pub fn view(
    s: &State,
    tab: &Tab,
    db: &Database,
    p: &BoardProjection,
    record: &Record,
    zones: Rc<Vec<DropZone>>,
    t: Theme,
) -> El {
    card(s, tab, db, p, record, zones, t, false)
}

pub fn drag_preview(s: &State, t: Theme) -> Option<El> {
    let drag = s
        .database_views
        .board_ui
        .drag
        .as_ref()
        .filter(|d| d.moved)?;
    let tab = s.tab()?;
    let db = tab.database.as_ref()?;
    let view = db.views.get(tab.view)?;
    let fallback;
    let p = if let Some(p) = s.database_views.board.projection_for(&db.id, &view.id) {
        p
    } else {
        fallback = moss_core::board::project_loaded(db, view, &tab.records).ok()?;
        &fallback
    };
    let t = Theme {
        family: crate::fonts::family(tab.page.settings.font),
        text_scale: crate::fonts::scale(tab.page.settings),
        ..t
    };
    Some(
        card(s, tab, db, p, &drag.record, Rc::new(Vec::new()), t, true)
            .key("board-drag-preview")
            .absolute(
                drag.origin[0] + drag.delta[0],
                drag.origin[1] + drag.delta[1],
            )
            .opacity(0.85)
            .inert()
            .raise(),
    )
}

fn card(
    s: &State,
    tab: &Tab,
    db: &Database,
    p: &BoardProjection,
    record: &Record,
    zones: Rc<Vec<DropZone>>,
    t: Theme,
    floating: bool,
) -> El {
    let config = &db.views[tab.view].config;
    let w = width(config);
    let wrapped = config
        .get("wrap_cards")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let active = !floating && database::active(s, record, None);
    let title = if record.title.is_empty() {
        "Untitled"
    } else {
        &record.title
    };
    let mut title_row = row().w_full().gap(6.).items_start();
    if config
        .get("show_page_icon")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        let appearance = p.appearance.get(&record.id);
        let icon = appearance
            .map(|a| a.icon.as_str())
            .filter(|icon| !icon.is_empty())
            .or_else(|| {
                tab.record_icons
                    .get(&record.id)
                    .map(String::as_str)
                    .filter(|icon| !icon.is_empty())
            });
        let has_content = appearance.map(|a| a.has_body_content).unwrap_or_else(|| {
            tab.record_has_content
                .get(&record.id)
                .copied()
                .unwrap_or(false)
        });
        let icon_label = icon
            .map(|icon| format!("Page icon {icon}"))
            .unwrap_or_else(|| {
                if has_content {
                    "Page with content".into()
                } else {
                    "Empty page".into()
                }
            });
        let mark = if let Some(icon) = icon {
            t.sized_label(icon, 16., t.text)
        } else {
            rune::icon(
                if appearance.map(|a| a.has_body_content).unwrap_or_else(|| {
                    tab.record_has_content
                        .get(&record.id)
                        .copied()
                        .unwrap_or(false)
                }) {
                    &phosphor::FILE_TEXT
                } else {
                    &phosphor::FILE
                },
                16.,
            )
            .color(t.muted)
        };
        title_row = title_row.child(
            row()
                .key(format!("board-icon-{}", record.id))
                .size(16., 16.)
                .center()
                .role("img")
                .label(icon_label)
                .child(mark),
        );
    }
    title_row = title_row.child(if active {
        database::editor(s, w - 54., t)
    } else {
        let label = t.label(title, t.text).min_w(0.).grow(1.);
        if wrapped {
            label.wrap().max_h(60.).clip_rounded(0.)
        } else {
            label.truncate()
        }
    });
    let action_key = format!("board-card-menu-{}", record.id);
    let action_anchor = action_key.clone();
    let menu_record = record.clone();
    title_row = title_row.child(
        row()
            .key(action_key)
            .size(22., 22.)
            .center()
            .radius(4.)
            .hover(t.selected)
            .role("button")
            .label("Page actions")
            .focusable(true)
            .child(
                rune::icon(&phosphor::DOTS_THREE, 16.)
                    .color(t.muted)
                    .group_opacity(if s.sidebar_keyboard_controls { 1. } else { 0. }, 1.),
            )
            .on_press(move |s: &mut State| {
                s.open_board_menu(
                    MenuTarget::Card(menu_record.clone()),
                    action_anchor.clone(),
                    None,
                )
            }),
    );
    let mut body = col().w_full().gap(5.).padding(10.).child(title_row);
    let mut props: Vec<_> = db
        .properties
        .iter()
        .filter(|prop| {
            prop.kind != PropertyKind::Title
                && super::property_visibility::visible(db, tab.view, &prop.id)
        })
        .collect();
    if let Some(order) = config.get("board_property_order").and_then(Value::as_array) {
        props.sort_by_key(|p| {
            order
                .iter()
                .position(|v| v.as_str() == Some(p.id.as_str()))
                .unwrap_or(usize::MAX)
        });
    }
    for prop in props {
        let value = record.values.get(&prop.id);
        let key = format!("board-value-{}-{}", record.id, prop.id);
        let editing = !floating && database::active(s, record, Some(&prop.id));
        let mut property_row = row()
            .key(key)
            .w_full()
            .min_h(24.)
            .gap(6.)
            .center_y()
            .radius(3.)
            .hover(t.hover)
            .child(
                row()
                    .size(16., 20.)
                    .center()
                    .child(super::property_icons::view(prop.kind, t.muted)),
            );
        property_row = property_row.child(if editing {
            database::editor(s, w - 44., t)
        } else if value.is_none_or(Value::is_null) || value.and_then(Value::as_str) == Some("") {
            t.label("Empty", t.faint).grow(1.)
        } else {
            let value = super::property_value::view(s, prop, value, t)
                .min_w(0.)
                .max_h(if wrapped { 54. } else { 24. })
                .clip_rounded(0.);
            if matches!(prop.kind, PropertyKind::Select | PropertyKind::Status) {
                row().min_w(0.).grow(1.).child(value)
            } else {
                value.grow(1.)
            }
        });
        let id = record.id.clone();
        let property = prop.id.clone();
        let kind = prop.kind;
        if !editing
            && matches!(
                kind,
                PropertyKind::Select
                    | PropertyKind::Status
                    | PropertyKind::Date
                    | PropertyKind::Text
                    | PropertyKind::Number
                    | PropertyKind::Url
                    | PropertyKind::Email
                    | PropertyKind::Phone
                    | PropertyKind::Checkbox
                    | PropertyKind::Relation
                    | PropertyKind::Place
            )
        {
            let drag_record = record.clone();
            let group = p.group_property.clone();
            let subgroup = p.subgroup_property.clone();
            let drop_zones = zones.clone();
            property_row = property_row
                .role("button")
                .label(format!("Edit {}", prop.name))
                .focusable(true)
                .on_drag(move |s: &mut State, drag, tx| {
                    s.board_card_drag(
                        &drag_record,
                        &group,
                        subgroup.as_ref(),
                        &drop_zones,
                        drag,
                        tx,
                    )
                })
                .on_press_tx(move |s: &mut State, tx| {
                    if s.database_views.board_ui.suppress_press {
                        s.database_views.board_ui.suppress_press = false;
                        tx.instant();
                        return;
                    }
                    if matches!(
                        kind,
                        PropertyKind::Select | PropertyKind::Status | PropertyKind::Date
                    ) {
                        s.open_property_picker(&id, &property);
                    } else if kind == PropertyKind::Place {
                        s.open_place_picker(&id, &property);
                    } else if kind == PropertyKind::Relation {
                        s.open_relation_picker(&id, &property);
                    } else if kind == PropertyKind::Checkbox {
                        if let Ok(Some(record)) = s.store.get_record(&id) {
                            let old = record.values.get(&property).cloned().unwrap_or(Value::Null);
                            match s.store.set_record_value_if_unchanged(
                                &id,
                                &property,
                                &old,
                                Value::Bool(!old.as_bool().unwrap_or(false)),
                            ) {
                                Ok(record) => {
                                    s.sync_record(&record);
                                    s.database_views.board.invalidate();
                                }
                                Err(e) => s.error = Some(e.to_string()),
                            }
                        }
                    } else {
                        s.edit_record_property(&id, Some(property.clone()));
                    }
                    tx.instant();
                });
        }
        body = body.child(property_row);
    }
    let drag_record = record.clone();
    let group = p.group_property.clone();
    let subgroup = p.subgroup_property.clone();
    let open_record = record.clone();
    let context_record = record.clone();
    let context_anchor = format!("board-card-{}", record.id);
    let mode = config
        .get("open_mode")
        .and_then(Value::as_str)
        .unwrap_or("side")
        .to_owned();
    let mut card = col()
        .key(context_anchor.clone())
        .w(w)
        .min_w(0.)
        .radius(5.)
        .clip_rounded(5.)
        .border(1., t.line)
        .fill(t.bg)
        .hover(t.hover)
        .hover_group()
        .role("button")
        .label(title)
        .focusable(true)
        .children(preview(s, record, p, config, w, t, floating))
        .child(body)
        .on_drag(move |s: &mut State, drag, tx| {
            s.board_card_drag(&drag_record, &group, subgroup.as_ref(), &zones, drag, tx)
        })
        .on_press_tx(move |s: &mut State, tx| {
            if s.database_views.board_ui.suppress_press {
                s.database_views.board_ui.suppress_press = false;
                return;
            }
            s.open_database_record(&open_record.database_id, &open_record.id, &mode);
            tx.instant();
        })
        .on_secondary_press(move |s: &mut State, p, tx| {
            s.open_board_menu(
                MenuTarget::Card(context_record.clone()),
                context_anchor.clone(),
                Some([p.x, p.y]),
            );
            tx.animated();
        });
    if floating {
        return card;
    }
    let lifted = s
        .database_views
        .board_ui
        .drag
        .as_ref()
        .is_some_and(|drag| drag.record.id == record.id && drag.moved);
    if lifted {
        card = card.opacity(0.);
    }
    let measured = record.id.clone();
    let drawn = measured.clone();
    let mut slot = col()
        .key(format!("board-card-slot-{}", record.id))
        .w(w)
        .on_drawn_layout_tx(move |s: &mut State, r, tx| {
            s.database_views
                .board_ui
                .card_rects
                .insert(drawn.clone(), r);
            tx.live_only();
        })
        .on_layout_tx(move |s: &mut State, r, tx| {
            let ui = &mut s.database_views.board_ui;
            if r[3] > 0.
                && ui
                    .card_heights
                    .get(&measured)
                    .is_none_or(|h| (h - r[3]).abs() > 0.5)
            {
                ui.card_heights.insert(measured.clone(), r[3]);
                ui.height_version = ui.height_version.wrapping_add(1);
            }
            tx.live_only();
        })
        .children(
            s.database_views
                .board_ui
                .drag
                .as_ref()
                .filter(|drag| drag.moved)
                .and_then(|drag| drag.destination.as_ref())
                .filter(|at| at.before.as_deref() == Some(record.id.as_str()))
                .map(|_| el().w_full().h(3.).fill(t.blue)),
        )
        .child(card);
    if lifted {
        slot = slot.absolute(0., 0.);
    }
    slot
}
