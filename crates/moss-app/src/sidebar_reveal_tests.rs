use super::*;

fn setup(width: f32, dark: bool, scale: f32, peek: bool) -> (Runtime<State>, PathBuf) {
    let (mut state, path) = fixture();
    if peek {
        state.new_page(true, None);
        state.create_inline(Default::default(), false);
        assert!(state.finish_cell(false));
        state.create_peek(Default::default());
        state.record_peek.as_mut().unwrap().mode = state::PeekMode::Side;
    }
    state.set_sidebar_width(320., width);
    state.toggle_sidebar();
    assert!(state.sidebar_collapsed());
    let mut rt = Runtime::new(
        state,
        ui::view,
        (width, 900.),
        Env {
            system_dark: dark,
            scale,
        },
    );
    rt.on_key = Some(key);
    rt.poll = Some(poll);
    rt.frame(0.);
    (rt, path)
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .0
}
fn frames(rt: &mut Runtime<State>, count: usize) {
    for _ in 0..count {
        rt.frame(1. / 60.);
    }
}
fn focus(rt: &mut Runtime<State>, key: &str) {
    rt.set_full_keyboard_access(true);
    let mut seen = Vec::new();
    for _ in 0..100 {
        if rt
            .elements()
            .iter()
            .any(|node| node.focused && node.key.as_deref() == Some(key))
        {
            return;
        }
        if seen.len() < 24 {
            seen.push((
                rt.elements()
                    .iter()
                    .find(|node| node.focused)
                    .and_then(|node| node.key.clone()),
                rt.state.sidebar_reveal.visible,
                rt.state.sidebar_reveal.keyboard_open,
                rt.find("search-button").is_some(),
            ));
        }
        press(rt, Key::Tab);
    }
    panic!("could not focus {key}; sequence={seen:?}");
}

#[test]
fn hover_reveal_and_grace_keep_content_and_preferences_stable() {
    for width in [850., 1440.] {
        for dark in [false, true] {
            for scale in [1., 2.] {
                let (mut rt, path) = setup(width, dark, scale, false);
                let workspace = rt.state.workspace.id.clone();
                let before = std::fs::read(path.join("preferences.json")).unwrap();
                let content = rect(&rt, "content");
                let top = rect(&rt, "titlebar");
                assert_eq!(top[3], 40.);
                assert_eq!(rect(&rt, "sidebar-rail")[2], 44.);
                assert_eq!(content[0], 44.);
                assert_eq!(content[1], top[1] + top[3]);
                assert!(rt.find("sidebar-compose").is_some());
                assert!(rt.find("add-page").is_none());
                rt.pointer_move(28., 100.);
                frames(&mut rt, 20);
                assert!(rt.state.sidebar_reveal.visible);
                assert!(rt.find("sidebar-preview").is_some());
                assert_eq!(rect(&rt, "content"), content);
                rt.pointer_move(200., 100.);
                frames(&mut rt, 2);
                assert!(rt.state.sidebar_reveal.visible);
                rt.pointer_move(width - 10., 450.);
                frames(&mut rt, 3);
                assert!(
                    rt.state.sidebar_reveal.visible,
                    "grace should keep panel open"
                );
                rt.pointer_move(200., 100.);
                frames(&mut rt, 12);
                assert!(
                    rt.state.sidebar_reveal.visible,
                    "re-entry should cancel close"
                );
                rt.pointer_move(width - 10., 450.);
                frames(&mut rt, 20);
                assert!(!rt.state.sidebar_reveal.visible);
                assert_eq!(rect(&rt, "content"), content);
                assert!(rt.state.sidebar_collapsed());
                assert_eq!(rt.state.prefs.sidebar_widths[&workspace], 320.);
                assert_eq!(
                    std::fs::read(path.join("preferences.json")).unwrap(),
                    before
                );
                drop(rt);
                let reopened = State::open(path.clone(), Some(&workspace)).unwrap();
                assert!(reopened.sidebar_collapsed());
                assert_eq!(reopened.prefs.sidebar_widths[&workspace], 320.);
                drop(reopened);
                std::fs::remove_dir_all(path).unwrap();
            }
        }
    }
}

#[test]
fn escape_with_pointer_over_rail_requires_leave_and_reentry() {
    let (mut rt, path) = setup(850., false, 1., false);
    rt.pointer_move(28., 100.);
    frames(&mut rt, 20);
    assert!(rt.state.sidebar_reveal.visible);
    press(&mut rt, Key::Escape);
    frames(&mut rt, 20);
    assert!(!rt.state.sidebar_reveal.visible);
    rt.pointer_move(800., 450.);
    frames(&mut rt, 2);
    rt.pointer_move(28., 100.);
    frames(&mut rt, 20);
    assert!(rt.state.sidebar_reveal.visible);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn workspace_menu_keeps_the_sidebar_revealed_and_remains_usable() {
    for dark in [false, true] {
        for scale in [1., 2.] {
            let (mut rt, path) = setup(850., dark, scale, false);
            rt.state.set_sidebar_width(220., 850.);
            rt.state.save_preferences();
            rt.invalidate();
            rt.frame(0.);
            rt.pointer_move(28., 100.);
            frames(&mut rt, 20);
            click(&mut rt, "workspace-picker");
            frames(&mut rt, 20);
            assert!(matches!(rt.state.modal, Some(state::Modal::Workspace)));
            let panel = rect(&rt, "sidebar-preview");
            let settings = rect(&rt, "workspace-settings");
            let x = panel[0] + panel[2] + 2.;
            let y = settings[1] + settings[3] / 2.;
            rt.pointer_move(x, y);
            frames(&mut rt, 20);
            assert!(rt.state.sidebar_reveal.visible);
            let settings_x = settings[0] + settings[2] / 2.;
            rt.pointer_move(settings_x, y);
            rt.pointer_down(settings_x, y);
            rt.pointer_up(settings_x, y);
            frames(&mut rt, 2);
            assert!(matches!(rt.state.modal, Some(state::Modal::Settings)));
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn keyboard_pin_and_pointer_exit_do_not_leave_preview_stuck() {
    let (mut rt, path) = setup(850., false, 1., false);
    focus(&mut rt, "rail-workspace");
    press(&mut rt, Key::Text(" ".into()));
    frames(&mut rt, 3);
    assert!(rt.state.sidebar_reveal.visible);
    assert!(matches!(rt.state.modal, Some(state::Modal::Workspace)));
    press(&mut rt, Key::Escape);
    frames(&mut rt, 20);
    focus(&mut rt, "search-button");
    rt.pointer_move(800., 450.);
    rt.pointer_down(800., 450.);
    rt.pointer_up(800., 450.);
    frames(&mut rt, 30);
    assert!(
        !rt.state.sidebar_reveal.visible,
        "outside click should release keyboard preview"
    );
    focus(&mut rt, "sidebar-expand");
    press(&mut rt, Key::Text(" ".into()));
    frames(&mut rt, 2);
    assert!(!rt.state.sidebar_collapsed());
    assert!(rt.find("sidebar").is_some());
    assert!(rt.find("sidebar-compose").is_some());
    assert!(rt.find("add-page").is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn narrow_resize_and_side_peek_do_not_reflow_on_hover() {
    let (mut rt, path) = setup(1440., true, 2., true);
    rt.set_viewport((850., 600.), 2.);
    frames(&mut rt, 3);
    let content = rect(&rt, "content");
    let peek = rect(&rt, "record-peek-side");
    rt.pointer_move(28., 100.);
    frames(&mut rt, 20);
    assert!(rt.state.sidebar_reveal.visible);
    assert_eq!(rect(&rt, "content"), content);
    assert_eq!(rect(&rt, "record-peek-side"), peek);
    assert!(rt.state.sidebar_collapsed());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn keyboard_focus_inside_preview_survives_pointer_exit() {
    let (mut rt, path) = setup(850., false, 1., false);
    rt.pointer_move(28., 100.);
    frames(&mut rt, 20);
    focus(&mut rt, "search-button");
    rt.pointer_move(800., 450.);
    frames(&mut rt, 30);
    assert!(
        rt.state.sidebar_reveal.visible,
        "focused sidebar control must remain available"
    );
    assert!(
        rt.elements()
            .iter()
            .any(|node| node.focused && node.key.as_deref() == Some("search-button"))
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn header_compose_is_the_new_page_entry_in_both_sidebar_states() {
    for collapsed in [false, true] {
        let (mut rt, path) = setup(850., false, 1., false);
        if !collapsed {
            rt.update(|s, _| s.toggle_sidebar());
            frames(&mut rt, 20);
        }
        assert_eq!(rt.state.sidebar_collapsed(), collapsed);
        assert!(rt.find("add-page").is_none());
        let before = rt.state.tabs.len();
        click(&mut rt, "sidebar-compose");
        assert_eq!(
            rt.state.tabs.len(),
            before + 1,
            "collapsed={collapsed} error={:?}",
            rt.state.error
        );
        assert!(rt.state.tab().unwrap().choosing_type);
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn keyboard_pin_opens_the_saved_sidebar() {
    let (mut rt, path) = setup(850., false, 1., false);
    focus(&mut rt, "sidebar-expand");
    press(&mut rt, Key::Text(" ".into()));
    frames(&mut rt, 20);
    assert!(!rt.state.sidebar_collapsed());
    assert!(rt.find("sidebar").is_some());
    assert!(rt.find("sidebar-compose").is_some());
    assert!(rt.find("add-page").is_none());
    let workspace = rt.state.workspace.id.clone();
    drop(rt);
    let reopened = State::open(path.clone(), Some(&workspace)).unwrap();
    assert!(!reopened.sidebar_collapsed());
    assert_eq!(reopened.prefs.sidebar_widths[&workspace], 320.);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn escape_closes_keyboard_preview_before_background_peek() {
    let (mut rt, path) = setup(850., false, 1., true);
    rt.pointer_move(28., 100.);
    frames(&mut rt, 20);
    focus(&mut rt, "search-button");
    assert!(rt.state.sidebar_reveal.keyboard_open);
    press(&mut rt, Key::Escape);
    frames(&mut rt, 20);
    assert!(!rt.state.sidebar_reveal.visible);
    assert!(
        rt.state.record_peek.is_some(),
        "Escape must not close the background peek first"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn collapsed_rail_keeps_loaded_page_and_favorite_icons_and_dividers() {
    let (mut rt, path) = setup(850., false, 1., false);
    let workspace = rt.state.workspace.id.clone();
    let mut ids = Vec::new();
    for index in 0..5 {
        let page = rt
            .state
            .store
            .create_page(
                &workspace,
                None,
                &format!("Rail {index}"),
                "🌱",
                PageKind::Document,
            )
            .unwrap();
        ids.push(page.id);
    }
    rt.state.refresh_pages();
    assert!(rt.state.toggle_sidebar_favorite(&ids[0]));
    rt.invalidate();
    rt.frame(0.);
    for id in &ids {
        assert!(
            rt.find(&format!("rail-page-{id}")).is_some(),
            "missing loaded page icon {id}; tree rows={}, rail keys={:?}",
            rt.state.sidebar_tree.rows.len(),
            rt.elements()
                .iter()
                .filter_map(|node| node.key.as_ref())
                .filter(|key| key.starts_with("rail-"))
                .collect::<Vec<_>>()
        );
    }
    assert!(rt.find(&format!("rail-favorite-{}", ids[0])).is_some());
    for key in [
        "rail-group-favorites",
        "rail-group-private",
        "rail-group-utilities",
    ] {
        let line = rect(&rt, key);
        assert!(
            line[2] > line[3] && line[3] <= 1.,
            "separator {key}: {line:?}"
        );
    }
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
