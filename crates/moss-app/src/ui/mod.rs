mod advanced_properties;
mod block_commands;
mod board;
mod board_card;
mod board_menu;
mod board_settings;
mod calendar;
mod cell_recovery;
mod chart;
pub(crate) mod comments;
mod child_pages;
mod dashboard;
mod database;
mod database_new;
mod database_properties;
mod database_toolbar;
mod editor;
mod editor_colors;
mod editor_layout;
mod gallery;
mod history;
mod icons;
mod image_block;
mod inputs;
pub(crate) mod map;
mod map_canvas;
mod map_clusters;
mod map_controls;
mod menu;
mod metadata_menu;
mod overlays;
mod page_cover;
mod page_creation;
mod page_icon_picker;
mod page_settings;
mod phosphor;
mod place_picker;
mod property_controls;
mod property_icons;
mod property_value;
pub(crate) mod property_visibility;
mod record_icon;
mod record_metadata;
mod record_peek;
mod shell;
mod sidebar_actions;
mod sidebar_section_menu;
mod sidebar_sections;
mod sidebar_surface;
mod simple_table;
mod table;
mod text_toolbar;
mod theme;
mod timeline;
pub(crate) mod view_icons;
mod view_management;
mod workspace_picker;
use crate::state::State;
use rune::*;
pub(crate) const CHROME_EDGE: f32 = 40.;
pub(crate) const COLLAPSED_SIDEBAR_WIDTH: f32 = 44.;

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = s.prefs.dark.unwrap_or(cx.system_dark());
    let mut t = theme::Theme::new(dark);
    t.caret_visible = s.caret.visible;
    let window_width = cx.layout_with("root", |r| r[2]).unwrap_or(850.);
    let has_side_peek = s
        .record_peek
        .as_ref()
        .is_some_and(|peek| peek.mode == crate::state::PeekMode::Side);
    // A side peek needs room for both page panes at the minimum window size.
    let resize_width = window_width - if has_side_peek { 240. } else { 0. };
    let expanded_sidebar_width = s.sidebar_width(resize_width);
    let sidebar_width = if s.sidebar_collapsed() {
        COLLAPSED_SIDEBAR_WIDTH
    } else {
        expanded_sidebar_width
    };
    let available_width = (window_width - sidebar_width).max(0.);
    let center_peek_comments = s.record_peek.as_ref().is_some_and(|peek| {
        peek.mode == crate::state::PeekMode::Center
            && s.comments.page_id.as_ref().is_some_and(|id| &peek.page.id == id)
    });
    let center_peek_history = s.record_peek.as_ref().is_some_and(|peek| {
        peek.mode == crate::state::PeekMode::Center && s.history.open
            && s.history.page_id.as_ref() == Some(&peek.page.id)
    });
    let history_visible = s.history.open && !center_peek_history && s.history.page_id.as_ref().is_some_and(|id| {
        s.tab().is_some_and(|tab| &tab.page.id == id)
            || s.record_peek.as_ref().is_some_and(|peek| &peek.page.id == id)
    });
    let comments_width = if (!center_peek_comments && s.comments.open
        && s.comments.page_id.as_ref().is_some_and(|id| {
            s.tab().is_some_and(|tab| &tab.page.id == id)
                || s.record_peek.as_ref().is_some_and(|peek| &peek.page.id == id)
        })) || history_visible
    { 300_f32.min((available_width * 0.4).max(0.)) } else { 0. };
    let peek_width = if has_side_peek {
        (available_width / 2.).min(420.)
    } else {
        0.
    };
    cx.caption_color(if dark { Color::WHITE } else { Color::BLACK });
    cx.titlebar_height(CHROME_EDGE);
    col()
        .key("root")
        .w_full()
        .h_full()
        .fill(t.sidebar)
        .color(t.text)
        .motion(Spring::NONE)
        .hover_motion_within(Anim::INSTANT, Anim::INSTANT)
        .on_pointer_move(|s: &mut State, point| {
            s.sidebar_keyboard_controls = false;
            s.sidebar_reveal_pointer(point.x, point.y);
        })
        .on_key(|s: &mut State, event, tx| {
            if event.key == Key::Tab {
                s.sidebar_keyboard_controls = true;
            }
            if event.key == Key::Tab && s.sidebar_collapsed() && s.sidebar_reveal.visible {
                s.open_sidebar_reveal_for_keyboard();
            }
            tx.instant();
            false
        })
        .child(shell::titlebar(s, cx, t, sidebar_width))
        .child(
            row()
                .w_full()
                .grow(1.)
                .min_h(0.)
                .child(el().w(sidebar_width).min_w(sidebar_width).h_full())
                .child(shell::content(s, t, (available_width - peek_width - comments_width).max(0.)))
                .children(record_peek::side(s, t, peek_width))
                .children((!center_peek_comments).then(|| comments::panel(s, t, comments_width)).flatten())
                .children((!center_peek_history).then(|| history::panel(s, t, comments_width)).flatten()),
        )
        .children(
            (!s.sidebar_collapsed())
                .then(|| shell::sidebar_resize(s, t, sidebar_width, resize_width)),
        )
        // Sticky and frozen Table cells are raised. Raise the later root
        // overlays too, so both paint and pointer input follow overlay order.
        .child(sidebar_surface::view(s, t, expanded_sidebar_width).raise())
        .children(record_peek::center(s, t).map(El::raise))
        .children(text_toolbar::view(s, cx, t).map(El::raise))
        .children(database_new::menu(s, t).map(El::raise))
        .children(database::undated_overlay(s, t).map(El::raise))
        .children(gallery::settings(s, t).map(El::raise))
        .children(property_controls::anchored_picker(s, t).map(El::raise))
        .children(place_picker::popup(s, t).map(El::raise))
        .children(advanced_properties::relation_picker(s, t).map(El::raise))
        .children(property_visibility::overlay(s, t).map(El::raise))
        .children(overlays::overlay(s, t).map(El::raise))
        .children(metadata_menu::popup(s, t).map(El::raise))
        .children(database_properties::popup(s, t).map(El::raise))
        .children(sidebar_page_menu::popup(s, t).map(El::raise))
        .children(sidebar_section_menu::popup(s, t).map(El::raise))
        .children(view_management::popup(s, t).map(El::raise))
        .children(board_menu::popup(s, t).map(El::raise))
        .children(board_card::drag_preview(s, t))
        .children(calendar::drag_preview(s, t))
        .children(sidebar_sections::page_drag_preview(s, t))
        .children(page_icon_picker::popup(s, t).map(El::raise))
        .children(page_cover::popup(s, t).map(El::raise))
        .children(map_clusters::popup(s, t).map(El::raise))
        .children(map_controls::popup(s, t).map(El::raise))
        .children(block_commands::popup(s, t).map(El::raise))
}

mod sidebar_page_menu;
mod sidebar_tree;
