use super::*;
use moss_core::{PropertyKind, new_id};
use std::path::PathBuf;

fn fixture(database: bool) -> (Runtime<State>, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-motion-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    if database {
        state.new_page(true, None);
    }
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1440., 1000.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.set_full_keyboard_access(true);
    rt.frame(0.);
    (rt, path)
}
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}
fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
}
fn focus(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..100 {
        if rt
            .elements()
            .iter()
            .any(|e| e.focused && e.key.as_deref() == Some(key))
        {
            return;
        }
        press(rt, Key::Tab);
    }
    panic!("could not focus {key}");
}
fn opacity(rt: &Runtime<State>, key: &str) -> f32 {
    rt.presented(key).unwrap().1
}

#[test]
fn pointer_dialog_fades_while_keyboard_shortcut_is_instant() {
    let (mut rt, path) = fixture(false);
    click(&mut rt, "search-button");
    let start = opacity(&rt, "modal");
    assert!(start < 0.95);
    rt.frame(0.05);
    assert!(opacity(&rt, "modal") > start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
    let (mut rt, path) = fixture(false);
    press(&mut rt, Key::Command('k'));
    assert_eq!(opacity(&rt, "modal"), 1.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn pointer_and_keyboard_view_chooser_use_distinct_motion() {
    let (mut rt, path) = fixture(true);
    rt.set_viewport((850., 700.), 1.);
    rt.frame(0.);
    rt.frame(0.);
    click(&mut rt, "db-views");
    let start = opacity(&rt, "modal");
    assert!(start < 0.95);
    rt.frame(0.05);
    assert!(opacity(&rt, "modal") > start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
    let (mut rt, path) = fixture(true);
    rt.set_viewport((850., 700.), 1.);
    rt.frame(0.);
    rt.frame(0.);
    focus(&mut rt, "db-views");
    press(&mut rt, Key::Text(" ".into()));
    assert_eq!(opacity(&rt, "modal"), 1.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn pointer_picker_fades_and_database_view_switch_is_instant() {
    let (mut rt, path) = fixture(true);
    let status = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    rt.state.create_inline(Default::default(), false);
    rt.state.finish_cell(false);
    rt.invalidate();
    rt.frame(1. / 120.);
    click(&mut rt, &format!("cell-0-{status}"));
    let start = opacity(&rt, "property-picker");
    assert!(start < 0.95);
    rt.frame(0.05);
    assert!(opacity(&rt, "property-picker") > start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
    let (mut rt, path) = fixture(true);
    click(&mut rt, "db-view-1");
    assert_eq!(rt.state.tab().unwrap().view, 1);
    assert_eq!(opacity(&rt, "board-new-Not started"), 1.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn pointer_tab_shows_content_before_indicator_finishes() {
    let (mut rt, path) = fixture(false);
    rt.state.new_name.replace_text("Second");
    rt.state.new_page(false, None);
    let first_id = rt.state.tabs[0].page.id.clone();
    rt.invalidate();
    rt.frame(1.);
    click(&mut rt, "tab-0");
    assert_eq!(rt.state.active, 0);
    assert_eq!(opacity(&rt, &format!("document-{first_id}")), 1.);
    let start = opacity(&rt, "tab-indicator-0");
    assert!(start < 0.95);
    rt.frame(0.05);
    assert!(opacity(&rt, "tab-indicator-0") > start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn keyboard_tab_switch_lands_content_and_indicator_immediately() {
    let (mut rt, path) = fixture(false);
    let first_title = rt.state.tabs[0].title.text.clone();
    let first_id = rt.state.tabs[0].page.id.clone();
    rt.state.new_name.replace_text("Second");
    rt.state.new_page(false, None);
    rt.invalidate();
    rt.frame(1.);
    let mut found = false;
    for _ in 0..100 {
        if rt.elements().iter().any(|e| {
            e.focused && e.role == "tab" && e.label.as_deref() == Some(first_title.as_str())
        }) {
            found = true;
            break;
        }
        press(&mut rt, Key::Tab);
    }
    assert!(found);
    press(&mut rt, Key::Text(" ".into()));
    assert_eq!(rt.state.active, 0);
    assert_eq!(opacity(&rt, &format!("document-{first_id}")), 1.);
    assert_eq!(opacity(&rt, "tab-indicator-0"), 1.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn keyboard_interrupts_pointer_dialog_open_and_reopens_instantly() {
    let (mut rt, path) = fixture(false);
    click(&mut rt, "search-button");
    assert!(opacity(&rt, "modal") < 0.95);
    press(&mut rt, Key::Escape);
    assert!(rt.find("modal").is_none());
    press(&mut rt, Key::Command('k'));
    assert_eq!(opacity(&rt, "modal"), 1.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn reduced_motion_dialog_uses_fade_without_position_change() {
    let (mut rt, path) = fixture(false);
    rt.set_system_reduced_motion(true);
    rt.frame(0.);
    click(&mut rt, "search-button");
    let (rect, start) = rt.presented("modal").unwrap();
    assert!(start < 0.95);
    rt.frame(0.05);
    let (later_rect, later) = rt.presented("modal").unwrap();
    assert!(later > start);
    assert_eq!(rect, later_rect);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn pointer_gallery_and_undated_popovers_fade_outside_instant_content() {
    let (mut rt, path) = fixture(true);
    assert!(rt.state.select_database_view(4));
    rt.invalidate();
    rt.frame(0.);
    click(&mut rt, "gallery-settings-button");
    let start = opacity(&rt, "gallery-settings");
    assert!(start < 0.95);
    rt.frame(0.05);
    assert!(opacity(&rt, "gallery-settings") > start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
    let (mut rt, path) = fixture(true);
    assert!(rt.state.select_database_view(2));
    rt.invalidate();
    rt.frame(0.);
    click(&mut rt, "db-view-settings");
    click(&mut rt, "calendar-no-date");
    let start = opacity(&rt, "calendar-no-date-panel");
    assert!(start < 0.95);
    rt.frame(0.05);
    assert!(opacity(&rt, "calendar-no-date-panel") > start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn keyboard_gallery_and_undated_popovers_open_instantly() {
    for (view, button, panel) in [
        (4, "gallery-settings-button", "gallery-settings"),
        (2, "calendar-no-date", "calendar-no-date-panel"),
    ] {
        let (mut rt, path) = fixture(true);
        assert!(rt.state.select_database_view(view));
        rt.invalidate();
        rt.frame(0.);
        if view == 2 {
            click(&mut rt, "db-view-settings");
            press(&mut rt, Key::Tab);
            press(&mut rt, Key::Tab);
            press(&mut rt, Key::End);
            for _ in 0..30 {
                if rt.elements().iter().any(|e| e.focused && e.key.as_deref() == Some(button)) { break; }
                press(&mut rt, Key::Up);
            }
        }
        focus(&mut rt, button);
        press(&mut rt, Key::Text(" ".into()));
        assert_eq!(opacity(&rt, panel), 1.);
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn pointer_new_record_peek_enters_without_delaying_database_content() {
    let (mut rt, path) = fixture(true);
    assert!(rt.state.select_database_view(4));
    rt.invalidate();
    rt.frame(0.);
    click(&mut rt, "gallery-new");
    let start = opacity(&rt, "record-peek-center");
    assert!(start < 0.95);
    assert_eq!(opacity(&rt, "database"), 1.);
    rt.frame(0.05);
    assert!(opacity(&rt, "record-peek-center") > start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn keyboard_selected_status_opens_picker_instantly() {
    let (mut rt, path) = fixture(true);
    let status = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    let id = rt.state.tab().unwrap().records[0].id.clone();
    rt.state.selected_cell = Some((id, Some(status)));
    rt.invalidate();
    rt.frame(0.);
    press(&mut rt, Key::Enter);
    assert_eq!(opacity(&rt, "property-picker"), 1.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn pointer_selection_and_peek_close_fade_out() {
    let (mut rt, path) = fixture(true);
    let status = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .properties
        .iter()
        .find(|p| p.kind == PropertyKind::Status)
        .unwrap()
        .id
        .clone();
    rt.state.create_inline(Default::default(), false);
    assert!(rt.state.finish_cell(false));
    rt.invalidate();
    rt.frame(0.);
    click(&mut rt, &format!("cell-0-{status}"));
    rt.frame(1.);
    click(&mut rt, "property-option-1");
    assert!(rt.state.property_picker.is_none());
    let start = opacity(&rt, "property-picker");
    assert!(start > 0.);
    rt.frame(0.05);
    assert!(opacity(&rt, "property-picker") < start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
    let (mut rt, path) = fixture(true);
    assert!(rt.state.select_database_view(4));
    rt.invalidate();
    rt.frame(0.);
    click(&mut rt, "gallery-new");
    rt.frame(1.);
    click(&mut rt, "peek-close");
    assert!(rt.state.record_peek.is_none());
    let start = opacity(&rt, "record-peek-center");
    assert!(start > 0.);
    rt.frame(0.05);
    assert!(opacity(&rt, "record-peek-center") < start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn list_visibility_pointer_fades_and_keyboard_is_instant() {
    let (mut rt, path) = fixture(true);
    assert!(rt.state.select_database_view(5));
    rt.invalidate();
    rt.frame(0.);
    click(&mut rt, "list-properties-button");
    let start = opacity(&rt, "list-property-visibility");
    assert!(start < 0.95);
    rt.frame(0.05);
    assert!(opacity(&rt, "list-property-visibility") > start);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
    let (mut rt, path) = fixture(true);
    assert!(rt.state.select_database_view(5));
    rt.invalidate();
    rt.frame(0.);
    focus(&mut rt, "list-properties-button");
    press(&mut rt, Key::Text(" ".into()));
    assert_eq!(opacity(&rt, "list-property-visibility"), 1.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
