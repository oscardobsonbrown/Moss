use super::{menu, phosphor, theme::Theme};
use crate::state::State;
use rune::*;
use std::time::{Duration, Instant};

pub fn popup(s: &State, t: Theme) -> Option<El> {
    let picker = s.place_picker()?;
    let map = &s.database_views.map;
    let input = super::inputs::field(
        &map.place_query,
        "place-search-input",
        "Search for a place or address…",
        t,
    )
    .label("Search for a place or address")
    .on_edit(|s: &mut State| &mut s.database_views.map.place_query)
    .on_input_change(|s: &mut State, tx| {
        s.database_views.map.search_changed();
        s.database_views.map.search_selected = 0;
        if let Some(picker) = s.place_picker_mut() {
            picker.error = None;
            picker.search_at = Some(Instant::now() + Duration::from_millis(400));
        }
        tx.wake_after(Duration::from_millis(400));
    })
    .on_input_key(|s: &mut State, event, tx| {
        let map = &mut s.database_views.map;
        match event.key {
            Key::Down if !map.search_results.is_empty() => {
                map.search_selected = (map.search_selected + 1).min(map.search_results.len() - 1)
            }
            Key::Up if !map.search_results.is_empty() => {
                map.search_selected = map.search_selected.saturating_sub(1)
            }
            _ => return false,
        }
        tx.instant();
        true
    })
    .on_submit(|s: &mut State, tx| {
        let map = &mut s.database_views.map;
        if let Some(place) = map.search_results.get(map.search_selected).cloned() {
            s.save_place_choice(Some(place), false);
        } else {
            if let Some(picker) = s.place_picker_mut() {
                picker.search_at = None;
            }
            s.database_views.map.search();
            tx.wake_after(Duration::from_millis(50));
        }
        tx.instant();
    });
    let mut rows = col().w_full().max_h(240.).scroll(ScrollSpec {
        stick_bottom: false,
    });
    if map.search_loading {
        rows = rows.child(t.label("Searching places…", t.muted).padding(8.));
    }
    for (index, place) in map.search_results.iter().enumerate() {
        let value = place.clone();
        rows = rows.child(
            menu::row_item(
                format!("place-result-{index}"),
                &place.name,
                rune::icon(&phosphor::MAP_PIN, 16.).color(t.muted),
                t,
            )
            .fill(if index == map.search_selected {
                t.selected
            } else {
                Color::TRANSPARENT
            })
            .help(&place.name)
            .on_press(move |s: &mut State| s.save_place_choice(Some(value.clone()), false)),
        );
    }
    if map.search_error.is_some() {
        rows = rows.child(
            t.label(
                "Place search is unavailable. You can save the address as text.",
                t.muted,
            )
            .wrap()
            .padding(8.),
        );
    }
    if let Some(error) = &picker.error {
        rows = rows.child(t.label(error, t.error).wrap().padding(8.));
    }
    let mut panel = col()
        .key("place-picker")
        .w(328.)
        .max_h(460.)
        .padding(8.)
        .gap(6.)
        .fill(t.menu)
        .radius(8.)
        .border(1., t.line)
        .shadow(4., 16., Color::BLACK.alpha(0.15))
        .role("dialog")
        .label("Edit place")
        .focus_group()
        .child(input)
        .child(t.label("Search uses Photon.", t.muted).px(4.))
        .child(rows);
    if !map.place_query.text.trim().is_empty() {
        panel = panel.child(
            menu::row_item(
                "place-save-text",
                "Save as text",
                rune::icon(&phosphor::TEXT_T, 16.).color(t.muted),
                t,
            )
            .help("Text places do not appear on the map until a location is selected.")
            .on_press(|s: &mut State| s.save_place_choice(None, true)),
        );
        panel = panel.child(t.label("Text does not create a map pin.", t.muted).px(10.));
    }
    if !picker.expected.is_null() {
        panel = panel.child(
            menu::row_item(
                "place-clear",
                "Clear place",
                rune::icon(&phosphor::X, 16.).color(t.muted),
                t,
            )
            .on_press(|s: &mut State| s.save_place_choice(None, false)),
        );
    }
    Some(
        panel
            .focus_return(&picker.anchor)
            .modal(true, |s: &mut State| s.dismiss_cell_picker())
            .anchored_fit(&picker.anchor, Side::Bottom, Align::Start, 2.),
    )
}
