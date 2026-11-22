use super::{
    menu,
    theme::{Theme, button},
};
use crate::state::State;
use moss_core::{PlaceValue, PropertyKind};
use rune::*;
use std::time::Duration;

fn choose_place(s: &mut State, place: PlaceValue) {
    let m = &mut s.database_views.map;
    m.viewport.latitude = place.latitude.clamp(-85.05112878, 85.05112878);
    m.viewport.longitude = place.longitude;
    m.viewport.zoom = 12.;
    m.place_query.replace_text(&place.name);
    m.place_query.focused = false;
    m.selected_place = Some(place);
    m.search_results.clear();
    m.invalidate();
    s.map_position_changed();
}
fn submit_search(s: &mut State, tx: &mut Tx) {
    s.database_views.map.search();
    tx.wake_after(Duration::from_millis(50));
}
pub fn search(s: &State, t: Theme) -> El {
    let m = &s.database_views.map;
    let input = super::inputs::field(&m.place_query, "map-place-search", "Search places…", t)
        .label("Search places")
        .on_edit(|s: &mut State| &mut s.database_views.map.place_query)
        .on_input_change(|s: &mut State, tx| {
            let m = &mut s.database_views.map;
            m.selected_place = None;
            m.search_selected = 0;
            m.search_changed();
            tx.instant();
        })
        .on_input_key(|s: &mut State, event, tx| {
            let m = &mut s.database_views.map;
            if m.search_results.is_empty() {
                return false;
            }
            match event.key {
                Key::Down => {
                    m.search_selected = (m.search_selected + 1).min(m.search_results.len() - 1)
                }
                Key::Up => m.search_selected = m.search_selected.saturating_sub(1),
                Key::Enter => {
                    let place =
                        m.search_results[m.search_selected.min(m.search_results.len() - 1)].clone();
                    choose_place(s, place);
                }
                _ => return false,
            }
            tx.instant();
            true
        })
        .on_submit(submit_search);
    row()
        .w_full()
        .min_w(0.)
        .gap(6.)
        .child(input.grow(1.).min_w(0.))
        .child(
            super::map::zoom_button(
                "map-search-submit",
                "Search places",
                &super::phosphor::MAGNIFYING_GLASS,
                t,
            )
            .on_press_tx(submit_search),
        )
}
fn results(s: &State, t: Theme) -> El {
    let m = &s.database_views.map;
    let mut list = col().w_full().max_h(200.).scroll(ScrollSpec {
        stick_bottom: false,
    });
    if m.search_loading {
        list = list.child(t.label("Searching places…", t.muted).padding(8.));
    }
    for (index, place) in m.search_results.iter().enumerate() {
        let place = place.clone();
        list = list.child(
            menu::row_item(
                format!("map-search-result-{index}"),
                &place.name,
                rune::icon(&super::phosphor::MAP_PIN, 16.).color(t.muted),
                t,
            )
            .fill(if index == m.search_selected {
                t.selected
            } else {
                Color::TRANSPARENT
            })
            .help(&place.name)
            .on_press(move |s: &mut State| choose_place(s, place.clone())),
        );
    }
    if let Some(error) = &m.search_error {
        list = list.child(t.label(error, t.error).w_full().wrap().padding(8.));
    }
    list
}
fn panel(key: &str, label: &str, t: Theme) -> El {
    col()
        .key(key)
        .w(328.)
        .max_h(500.)
        .padding(10.)
        .gap(8.)
        .fill(t.menu)
        .radius(8.)
        .border(1., t.line)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .role("dialog")
        .label(label)
        .focus_group()
        .transition(Transition::opacity())
}
pub fn popup(s: &State, t: Theme) -> Option<El> {
    if s.place_picker().is_some() {
        return None;
    }
    let m = &s.database_views.map;
    if m.source_open {
        let db = s.tab()?.database.as_ref()?;
        let mut list = panel("map-source-menu", "Location property", t)
            .child(t.label("Location property", t.muted).px(10.));
        for p in db
            .properties
            .iter()
            .filter(|p| p.kind == PropertyKind::Place)
        {
            let id = p.id.clone();
            list = list.child(
                menu::row_item(
                    format!("map-source-{}", p.id),
                    &p.name,
                    rune::icon(&super::phosphor::MAP_PIN, 16.).color(t.muted),
                    t,
                )
                .child(menu::end(
                    format!("map-source-{}-selected", p.id),
                    m.property
                        .as_ref()
                        .filter(|s| **s == p.id)
                        .map(|_| &super::phosphor::CHECK),
                    t,
                ))
                .on_press(move |s: &mut State| s.select_map_source(&id)),
            );
        }
        list = list.child(
            menu::row_item(
                "map-source-create",
                "New location property",
                rune::icon(&super::phosphor::PLUS, 16.).color(t.muted),
                t,
            )
            .on_press(|s: &mut State| s.create_map_source()),
        );
        return Some(
            list.focus_return("map-source")
                .modal(true, |s: &mut State| {
                    s.database_views.map.source_open = false
                })
                .anchored_fit("map-source", Side::Bottom, Align::Start, 4.),
        );
    }
    if m.location_editor {
        return Some(location_editor(s, t));
    }
    if m.search_loading || !m.search_results.is_empty() || m.search_error.is_some() {
        return Some(
            panel("map-place-results", "Place search results", t)
                .child(results(s, t))
                .anchored_fit("map-place-search", Side::Bottom, Align::Start, 4.),
        );
    }
    None
}
fn location_editor(s: &State, t: Theme) -> El {
    let m = &s.database_views.map;
    let mut body = col().w_full().gap(8.).child(
        row()
            .w_full()
            .center_y()
            .child(t.label("Set record location", t.text).grow(1.))
            .child(
                super::map::zoom_button(
                    "map-location-close",
                    "Close location editor",
                    &super::phosphor::X,
                    t,
                )
                .on_press(|s: &mut State| {
                    let m = &mut s.database_views.map;
                    m.location_editor = false;
                    m.place_query.focused = false;
                    m.record_query.focused = false;
                }),
            ),
    );
    if m.property.is_none() {
        body = body
            .child(
                t.label("Choose a location property first.", t.muted)
                    .w_full()
                    .wrap(),
            )
            .child(
                button("map-location-source", "Choose property", t).on_press(|s: &mut State| {
                    s.database_views.map.location_editor = false;
                    s.database_views.map.source_open = true;
                }),
            );
    } else if m.selected_record.is_none() {
        body = body.child(t.label("1. Choose a record", t.muted)).child(
            super::inputs::field(&m.record_query, "map-record-search", "Find a record…", t)
                .label("Find a record")
                .on_edit(|s: &mut State| &mut s.database_views.map.record_query)
                .on_input_change(|s: &mut State, tx| {
                    s.database_views.map.invalidate();
                    tx.wake_after(Duration::ZERO);
                }),
        );
        let mut rows = col().w_full().max_h(180.).scroll(ScrollSpec {
            stick_bottom: false,
        });
        for record in &m.candidates {
            let id = record.id.clone();
            rows = rows.child(
                menu::row_item(
                    format!("map-record-{id}"),
                    if record.title.is_empty() {
                        "Untitled"
                    } else {
                        &record.title
                    },
                    rune::icon(&super::phosphor::FILE, 16.).color(t.muted),
                    t,
                )
                .on_press(move |s: &mut State| {
                    s.choose_map_record(&id);
                    s.database_views.map.record_query.focused = false;
                    s.database_views.map.place_query.focused = true;
                }),
            );
        }
        if m.candidates.is_empty() {
            rows = rows.child(
                t.label(
                    if m.loading {
                        "Loading records…"
                    } else {
                        "No matching records."
                    },
                    t.muted,
                )
                .padding(8.),
            );
        }
        body = body.child(rows).child(
            t.label("Up to 100 results. Type to find another record.", t.muted)
                .w_full()
                .wrap(),
        );
    } else {
        let title = m
            .candidates
            .iter()
            .find(|r| Some(&r.id) == m.selected_record.as_ref())
            .map(|r| r.title.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("Selected record");
        body = body
            .child(
                row()
                    .w_full()
                    .center_y()
                    .gap(8.)
                    .child(t.label(title, t.text).grow(1.).min_w(0.).truncate())
                    .child(button("map-location-change-record", "Change", t).on_press(
                        |s: &mut State| {
                            let m = &mut s.database_views.map;
                            m.selected_record = None;
                            m.selected_place = None;
                            m.place_query.focused = false;
                            m.record_query.focused = true;
                        },
                    )),
            )
            .child(t.label("2. Choose a place", t.muted))
            .child(search(s, t))
            .child(results(s, t));
        if let Some(place) = &m.selected_place {
            body = body.child(t.label(&place.name, t.text).w_full().wrap());
        }
        body = body.child(
            menu::row_item(
                "map-coordinate-toggle",
                "Enter coordinates",
                rune::icon(
                    if m.coordinate_entry {
                        &super::phosphor::CARET_DOWN
                    } else {
                        &super::phosphor::CARET_RIGHT
                    },
                    16.,
                )
                .color(t.muted),
                t,
            )
            .on_press(|s: &mut State| {
                s.database_views.map.coordinate_entry = !s.database_views.map.coordinate_entry
            }),
        );
        if m.coordinate_entry {
            body = body
                .child(
                    super::inputs::field(&m.place_name, "map-place-name", "Place name", t)
                        .label("Place name")
                        .on_edit(|s: &mut State| &mut s.database_views.map.place_name),
                )
                .child(
                    row()
                        .w_full()
                        .gap(8.)
                        .child(
                            super::inputs::field(&m.latitude, "map-latitude", "Latitude", t)
                                .grow(1.)
                                .min_w(0.)
                                .label("Latitude")
                                .on_edit(|s: &mut State| &mut s.database_views.map.latitude),
                        )
                        .child(
                            super::inputs::field(&m.longitude, "map-longitude", "Longitude", t)
                                .grow(1.)
                                .min_w(0.)
                                .label("Longitude")
                                .on_edit(|s: &mut State| &mut s.database_views.map.longitude),
                        ),
                )
                .child(
                    button("map-use-coordinates", "Use coordinates", t).on_press(
                        |s: &mut State| {
                            if s.database_views.map.set_coordinates() {
                                s.map_position_changed();
                            }
                        },
                    ),
                );
        }
        if m.selected_place.is_some() {
            body = body.child(
                button(
                    "map-save-place",
                    "Save location",
                    Theme {
                        text: Color::WHITE,
                        ..t
                    },
                )
                .fill(t.blue)
                .hover(t.blue)
                .on_press(|s: &mut State| {
                    s.save_map_place();
                    if s.database_views.map.search_error.is_none() {
                        s.database_views.map.location_editor = false;
                        s.database_views.map.place_query.focused = false;
                        s.database_views.map.record_query.focused = false;
                    }
                }),
            );
        }
    }
    panel("map-location-editor", "Set record location", t)
        .child(body.min_h(0.).shrink().scroll(ScrollSpec {
            stick_bottom: false,
        }))
        .focus_return("map-location-open")
        .modal(true, |s: &mut State| {
            let m = &mut s.database_views.map;
            m.location_editor = false;
            m.place_query.focused = false;
            m.record_query.focused = false;
        })
        .anchored_fit("map-location-open", Side::Bottom, Align::End, 4.)
}

pub fn navigation(key: impl Into<String>, zoom_in: El, zoom_out: El, reset: El, t: Theme) -> El {
    row()
        .key(key)
        .center_y()
        .padding(2.)
        .gap(1.)
        .fill(t.menu)
        .radius(6.)
        .border(1., t.line)
        .shadow(2., 8., Color::BLACK.alpha(0.12))
        .child(zoom_out)
        .child(el().w(1.).h(18.).fill(t.line))
        .child(zoom_in)
        .child(el().w(1.).h(18.).fill(t.line))
        .child(reset)
        .pin(None, Some(4.), Some(4.), None)
        .raise()
}
pub fn navigation_for_map(s: &State, t: Theme) -> El {
    navigation(
        "map-navigation",
        super::map::zoom_button("map-zoom-in", "Zoom in", &super::phosphor::PLUS, t).on_press_tx(
            |s: &mut State, tx| {
                let v = s.database_views.map.viewport;
                s.map_zoom_at(1, (v.width / 2.) as f32, (v.height / 2.) as f32);
                tx.instant();
            },
        ),
        super::map::zoom_button("map-zoom-out", "Zoom out", &super::phosphor::MINUS, t)
            .on_press_tx(|s: &mut State, tx| {
                let v = s.database_views.map.viewport;
                s.map_zoom_at(-1, (v.width / 2.) as f32, (v.height / 2.) as f32);
                tx.instant();
            }),
        super::map::zoom_button(
            "map-reset",
            if s.database_views.map.fit_pending {
                "Fitting pins…"
            } else {
                "Reset view to fit all pins"
            },
            &super::phosphor::CORNERS_OUT,
            t,
        )
        .opacity(if s.database_views.map.fit_pending {
            0.5
        } else {
            1.
        })
        .on_press_tx(|s: &mut State, tx| {
            if !s.database_views.map.fit_pending {
                s.fit_map_pins();
            }
            tx.wake_after(Duration::from_millis(20));
            tx.instant();
        }),
        t,
    )
}
