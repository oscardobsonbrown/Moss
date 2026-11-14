mod advanced_properties;
mod block_commands;
mod board_interaction;
mod board_view;
mod calendar_view;
mod caret;
mod chart_view;
mod child_pages;
mod computed_refresh;
mod dashboard_view;
mod database_properties;
mod database_views;
mod drafts;
mod editor_commands;
mod emoji_catalog;
mod fonts;
mod map_clusters;
mod map_interaction;
mod map_view;
mod page_appearance;
mod page_blocks;
mod page_covers;
mod page_creation;
mod page_open;
mod page_personalization;
mod place_property;
mod property_controls;
mod record_open;
mod records;
mod relation_titles;
mod rich_text;
mod sidebar_favorites;
mod sidebar_page_menu;
mod sidebar_reveal;
mod sidebar_section_menu;
mod sidebar_sections;
mod sidebar_tree;
mod simple_table;
mod state;
mod table_columns;
mod table_summaries;
mod table_window;
mod ui;
mod view_management;
use rune::*;
use state::State;

fn init() -> State {
    State::open(State::data_path(), None).expect("Could not open Moss data")
}
fn key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.key == Key::Escape && s.sidebar_sections.page_drag.is_some() {
        s.sidebar_sections.page_drag = None;
        s.sidebar_sections.page_drag_cancelled = true;
        s.sidebar_sections.suppress_page_press = true;
        tx.instant();
        return true;
    }
    if event.key == Key::Escape && s.database_views.calendar.schedule.is_some() {
        s.database_views.calendar.schedule = None;
        tx.instant();
        return true;
    }
    if event.key == Key::Escape && s.place_picker.is_some() {
        s.dismiss_place_picker();
        tx.instant();
        return true;
    }
    if event.key == Key::Escape {
        let m = &mut s.database_views.map;
        if m.location_editor || m.source_open {
            return false;
        }
        if !m.search_results.is_empty() || m.search_loading || m.search_error.is_some() {
            m.search_changed();
            tx.instant();
            return true;
        }
    }
    if event.key == Key::Escape && s.database_views.map_cluster.is_some() {
        return false;
    }
    if event.key == Key::Escape && s.page_cover.editor.is_some() {
        s.finish_page_cover();
        tx.instant();
        return true;
    }
    if event.key == Key::Escape && s.block_commands.is_some() {
        s.dismiss_block_commands();
        tx.instant();
        return true;
    }
    if event.key == Key::Escape
        && (s.database_property_editor.is_some()
            || s.metadata_menu.is_some()
            || s.sidebar_page_menu.is_some()
            || s.page_icon_picker.is_some()
            || s.sidebar_section_menu.is_some())
    {
        return false;
    }
    if event.key == Key::Escape
        && s.sidebar_reveal.visible
        && s.modal.is_none()
        && s.sidebar_page_menu.is_none()
        && s.sidebar_section_menu.is_none()
        && s.page_icon_picker.is_none()
    {
        s.dismiss_sidebar_reveal_by_key();
        tx.instant();
        return true;
    }
    if let Key::Command(c) = event.key {
        match c {
            'k' | 'p' => {
                tx.instant();
                s.open_search();
                return true;
            }
            't' => {
                tx.instant();
                s.open_new_tab_search();
                return true;
            }
            'n' => {
                tx.instant();
                s.begin_page(None);
                return true;
            }
            'w' => {
                tx.instant();
                s.modal = None;
                if s.tabs.len() > 1 {
                    s.close_tab(s.active);
                } else if s.flush_current() {
                    tx.close_window();
                }
                return true;
            }
            'q' => {
                if s.flush_current() {
                    tx.quit();
                }
                return true;
            }
            _ => {}
        }
    }
    if tx.focused().is_none() && s.table_selection_key(event) {
        tx.instant();
        return true;
    }
    if event.key == Key::Escape && s.modal.is_none() {
        tx.instant();
        if s.property_picker.is_some() {
            s.dismiss_property_picker();
            return true;
        }
        if s.editing_cell.is_some() {
            s.finish_cell(false);
            return true;
        }
        if s.record_peek.is_some() {
            s.close_peek();
            return true;
        }
    }
    if event.key == Key::Escape && s.modal.is_some() {
        tx.instant();
        s.modal = None;
        return true;
    }
    false
}
fn poll(s: &mut State, time: f64, tx: &mut Tx) -> bool {
    let records = records::poll(s, time, tx);
    let sidebar = sidebar_reveal::poll(s, time, tx);
    let sidebar_drag = sidebar_sections::poll(s, time, tx);
    let views = database_views::poll(s, time, tx);
    let covers = page_covers::poll(s, time, tx);
    let place = place_property::poll(s, tx);
    let appearance = page_appearance::poll(s, time, tx);
    records || sidebar || sidebar_drag || views || covers || appearance || place
}
fn spec() -> AppSpec<State> {
    AppSpec {
        min_size: Some((850., 600.)),
        on_key: Some(key),
        poll: Some(poll),
        setup: Some(|s, wake| {
            s.database_views.map.set_waker(wake.clone());
            s.database_views.dashboard.set_waker(wake.clone());
            s.page_cover.set_waker(wake);
        }),
        fonts: fonts::all(),
        families: fonts::FAMILIES,
        ..AppSpec::new("Moss", (1440., 900.), init, ui::view)
    }
}
fn main() {
    if let Err(error) = rune::set_app_icon(include_bytes!("../assets/Moss.png").to_vec()) {
        eprintln!("Moss could not load its app icon: {error}");
    }
    rune::prewarm_gpu();
    let path = State::data_path();
    let first = match State::open(path.clone(), None) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Moss could not open its local data: {e}");
            std::process::exit(1)
        }
    };
    let workspaces = first.workspaces.clone();
    let first_id = first.workspace.id.clone();
    let handles = first.handles.clone();
    let shared_preferences = first.shared_preferences.clone();
    let mut app = App::new(spec())
        .init(move || first)
        .titlebar(Titlebar::new(ui::CHROME_EDGE))
        .keep_running()
        .font_smoothing(FontSmoothing::Off)
        .on_focus(|s, focused, _| {
            s.caret.set_window_focused(focused);
            if !focused {
                s.close_sidebar_reveal();
            }
            if focused {
                s.refresh_pages();
                s.refresh_open_child_pages();
                s.save_preferences();
            }
        })
        .on_close(|s, _| {
            if s.flush_current() { s.discard_all_drafts(); }
            s.save_preferences();
        });
    handles
        .lock()
        .unwrap()
        .insert(first_id.clone(), app.handle());
    for workspace in workspaces.into_iter().filter(|w| w.id != first_id) {
        match State::open(path.clone(), Some(&workspace.id)) {
            Ok(mut state) => {
                state.handles = handles.clone();
                state.shared_preferences = shared_preferences.clone();
                let id = workspace.id.clone();
                let secondary = App::new(spec())
                    .init(move || state)
                    .titlebar(Titlebar::new(ui::CHROME_EDGE))
                    .keep_running()
                    .windowless()
                    .font_smoothing(FontSmoothing::Off)
                    .on_focus(|s, focused, _| {
                        s.caret.set_window_focused(focused);
                        if !focused {
                            s.close_sidebar_reveal();
                        }
                        if focused {
                            s.refresh_pages();
                            s.refresh_open_child_pages();
                            s.save_preferences();
                        }
                    })
                    .on_close(|s, _| {
                        if s.flush_current() { s.discard_all_drafts(); }
                        s.save_preferences();
                    });
                let (handle, _) = app.window(WindowSpec::from_app(id.clone(), secondary));
                handles.lock().unwrap().insert(id, handle);
            }
            Err(e) => eprintln!("Could not prepare workspace {}: {e}", workspace.name),
        }
    }
    app.run();
}

#[cfg(test)]
mod tests;
