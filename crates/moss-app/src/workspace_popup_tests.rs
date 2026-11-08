use super::*;
use moss_core::new_id;
use std::path::PathBuf;

fn runtime(width: f32, window_width: f32, dark: bool) -> (Runtime<State>, PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-workspace-popup-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    state.store.create_workspace("Other workspace", "").unwrap();
    state.workspaces = state.store.list_workspaces().unwrap();
    state.set_sidebar_width(width, window_width);
    state.prefs.dark = Some(dark);
    let mut rt = Runtime::with_text(
        state,
        ui::view,
        (window_width, 600.),
        Env {
            system_dark: dark,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    (rt, path)
}

fn click(rt: &mut Runtime<State>, name: &str) {
    let (x, y) = rt.find(name).unwrap_or_else(|| panic!("missing {name}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..60 {
        rt.frame(1. / 120.);
    }
}

fn info(rt: &Runtime<State>, name: &str) -> rune::runtime::ElementInfo {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(name) && !e.exiting)
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn escape(rt: &mut Runtime<State>) {
    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
}

#[test]
fn workspace_menu_is_anchored_inside_the_window_at_both_sidebar_widths() {
    for dark in [false, true] {
        for (width, window_width) in [(200., 850.), (480., 850.), (480., 900.)] {
            let (mut rt, path) = runtime(width, window_width, dark);
            assert_eq!(
                rt.state.sidebar_width(window_width),
                width.min(window_width - 400.)
            );
            click(&mut rt, "workspace-picker");
            settle(&mut rt);
            let [x, y, w, h] = info(&rt, "workspace-menu").target;
            let [_, trigger_y, _, _] = info(&rt, "workspace-picker").target;
            assert!(x >= 0. && y >= 0. && x + w <= window_width && y + h <= 600.);
            assert!(y + h <= trigger_y + 1., "menu must be above trigger");
            assert!(rt.find("modal-scrim").is_none());
            assert!(rt.find("close-modal").is_none());
            assert_eq!(info(&rt, "workspace-picker").expanded, Some(true));
            let current = format!("workspace-{}", rt.state.workspace.id);
            assert_eq!(info(&rt, &current).role, "menuitemradio");
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn workspace_menu_aligns_its_icon_and_text_columns_with_the_sidebar() {
    for dark in [false, true] {
        for (width, scale) in [(200., 1.), (270., 2.), (480., 1.)] {
            let (mut rt, path) = runtime(width, 1000., dark);
            rt.set_viewport((1000., 600.), scale);
            rt.frame(0.);
            click(&mut rt, "workspace-picker");
            settle(&mut rt);
            let current = rt.state.workspace.id.clone();
            let trigger = info(&rt, "workspace-picker").target;
            let menu = info(&rt, "workspace-menu").target;
            assert!(
                (trigger[0] - menu[0]).abs() < 1. && (trigger[2] - menu[2]).abs() < 1.,
                "{trigger:?} {menu:?}"
            );
            let icon = info(&rt, "sidebar-workspace-icon").target;
            for key in [
                "workspace-header-icon".to_owned(),
                "workspace-settings-icon".to_owned(),
                format!("workspace-icon-{current}"),
            ] {
                let slot = info(&rt, &key).target;
                assert!(
                    (slot[0] - icon[0]).abs() < 1. && (slot[2] - 20.).abs() < 1.,
                    "{key}: {slot:?} {icon:?}"
                );
            }
            let heading = info(&rt, "workspace-header-name").target;
            for key in [
                "workspace-settings-label".to_owned(),
                format!("workspace-name-{current}"),
            ] {
                let label = info(&rt, &key).target;
                assert!(
                    (label[0] - heading[0]).abs() < 1.,
                    "{key}: {label:?} {heading:?}"
                );
            }
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn workspace_menu_dismissal_and_actions_preserve_context() {
    let (mut rt, path) = runtime(200., 850., false);
    click(&mut rt, "workspace-picker");
    settle(&mut rt);
    escape(&mut rt);
    settle(&mut rt);
    assert!(rt.find("workspace-menu").is_none());
    assert!(info(&rt, "workspace-picker").focused);

    click(&mut rt, "workspace-picker");
    settle(&mut rt);
    rt.pointer_down(800., 50.);
    rt.pointer_up(800., 50.);
    rt.frame(1. / 120.);
    settle(&mut rt);
    assert!(rt.find("workspace-menu").is_none());

    click(&mut rt, "workspace-picker");
    settle(&mut rt);
    let current = format!("workspace-{}", rt.state.workspace.id);
    click(&mut rt, &current);
    settle(&mut rt);
    assert!(rt.find("workspace-menu").is_none());

    click(&mut rt, "workspace-picker");
    settle(&mut rt);
    let other = rt
        .state
        .workspaces
        .iter()
        .find(|w| w.id != rt.state.workspace.id)
        .unwrap()
        .id
        .clone();
    click(&mut rt, &format!("workspace-{other}"));
    settle(&mut rt);
    assert!(rt.find("workspace-menu").is_some());
    assert!(
        rt.state
            .error
            .as_deref()
            .unwrap_or("")
            .contains("not available")
    );
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.text.as_deref().unwrap_or("").contains("not available"))
    );

    click(&mut rt, "workspace-settings");
    settle(&mut rt);
    assert!(rt.state.modal == Some(state::Modal::Settings));
    assert!(rt.find("workspace-menu").is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn workspace_menu_pointer_fades_and_keyboard_opens_instantly() {
    let (mut rt, path) = runtime(200., 850., false);
    click(&mut rt, "workspace-picker");
    let pointer_opacity = info(&rt, "workspace-menu").opacity;
    assert!(
        pointer_opacity < 1.,
        "pointer open should fade: {pointer_opacity}"
    );
    settle(&mut rt);
    escape(&mut rt);
    settle(&mut rt);
    assert!(info(&rt, "workspace-picker").focused);
    rt.key(KeyEvent {
        key: Key::Text(" ".into()),
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
    let keyboard_opacity = info(&rt, "workspace-menu").opacity;
    assert!(
        keyboard_opacity >= 0.99,
        "keyboard open should be instant: {keyboard_opacity}"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
