mod fonts;
mod records;
mod rich_text;
mod state;
mod ui;
use rune::*;
use state::State;

fn init() -> State {
    State::open(State::data_path(), None).expect("Could not open Rotion data")
}
fn key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if let Key::Command(c) = event.key {
        match c {
            'k' | 'p' => {
                tx.instant();
                s.open_search();
                return true;
            }
            'n' | 't' => {
                tx.instant();
                s.new_name = TextEdit::new("");
                s.new_name.focused = true;
                s.modal = Some(state::Modal::NewPage);
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
    if event.key == Key::Escape && s.modal.is_none() {
        tx.instant();
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
fn spec() -> AppSpec<State> {
    AppSpec {
        min_size: Some((850., 600.)),
        on_key: Some(key),
        poll: Some(records::poll),
        fonts: fonts::FONTS,
        families: fonts::FAMILIES,
        ..AppSpec::new("Rotion", (1440., 900.), init, ui::view)
    }
}
fn main() {
    rune::prewarm_gpu();
    let path = State::data_path();
    let first = match State::open(path.clone(), None) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Rotion could not open its local data: {e}");
            std::process::exit(1)
        }
    };
    let workspaces = first.workspaces.clone();
    let first_id = first.workspace.id.clone();
    let handles = first.handles.clone();
    let shared_preferences = first.shared_preferences.clone();
    let mut app = App::new(spec())
        .init(move || first)
        .titlebar(Titlebar::new(44.))
        .keep_running()
        .font_smoothing(FontSmoothing::Off)
        .on_focus(|s, focused, _| {
            if focused {
                s.refresh_pages();
                s.save_preferences();
            }
        })
        .on_close(|s, _| {
            s.flush_current();
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
                    .titlebar(Titlebar::new(44.))
                    .keep_running()
                    .windowless()
                    .font_smoothing(FontSmoothing::Off)
                    .on_focus(|s, focused, _| {
                        if focused {
                            s.refresh_pages();
                            s.save_preferences();
                        }
                    })
                    .on_close(|s, _| {
                        s.flush_current();
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
