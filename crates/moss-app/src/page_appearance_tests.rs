use super::*;
use crate::state::{Modal, State};
use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, el};

fn surface(s: &State, _: &mut Cx<State>) -> El {
    let x = if s.prefs.dark == Some(true) {
        30.
    } else {
        800.
    };
    let t = Theme::new(false);
    el().w_full()
        .h_full()
        .child(el().key("page-actions").absolute(x, 30.).size(28., 28.))
        .children(s.modal.is_some().then(|| super::overlay(s, t)))
}
fn setup(left: bool) -> (Runtime<State>, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("moss-appearance-{}", moss_core::new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    s.new_page(true, None);
    s.prefs.dark = Some(left);
    s.modal = Some(Modal::PageActions);
    let mut rt = Runtime::with_text(
        s,
        surface,
        (850., 650.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(crate::fonts::all(), crate::fonts::FAMILIES),
    );
    rt.on_key = Some(crate::key);
    rt.poll = Some(crate::poll);
    rt.frame(0.);
    (rt, path)
}
fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .0
}
fn move_to(rt: &mut Runtime<State>, key: &str) {
    let [x, y, w, h] = rect(rt, key);
    rt.pointer_move(x + w / 2., y + h / 2.);
    rt.frame(1. / 60.);
}
fn frames(rt: &mut Runtime<State>, n: usize) {
    for _ in 0..n {
        rt.frame(1. / 60.);
    }
}
fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    rt.frame(1. / 60.);
}
#[test]
fn pointer_travel_grace_exit_and_sibling_close_on_both_sides() {
    for left in [false, true] {
        let (mut rt, path) = setup(left);
        move_to(&mut rt, "appearance-menu-item");
        assert!(rt.state.modal == Some(Modal::Appearance));
        let row = rect(&rt, "appearance-menu-item");
        let panel = rect(&rt, "page-appearance");
        if left {
            assert!(
                panel[0] >= row[0] + row[2] - 1.,
                "panel should flip right: {row:?} {panel:?}"
            );
        } else {
            assert!(
                panel[0] + panel[2] <= row[0] + 1.,
                "panel should open left: {row:?} {panel:?}"
            );
        }
        let gap_x = if panel[0] < row[0] {
            (panel[0] + panel[2] + row[0]) / 2.
        } else {
            (row[0] + row[2] + panel[0]) / 2.
        };
        rt.pointer_move(gap_x, row[1] + 15.);
        frames(&mut rt, 4);
        assert!(rt.state.modal == Some(Modal::Appearance));
        move_to(&mut rt, "page-appearance");
        frames(&mut rt, 12);
        assert!(rt.state.modal == Some(Modal::Appearance));
        rt.pointer_move(450., 600.);
        frames(&mut rt, 12);
        assert!(rt.state.modal == Some(Modal::PageActions));
        move_to(&mut rt, "appearance-menu-item");
        assert!(rt.state.modal == Some(Modal::Appearance));
        move_to(&mut rt, "database-page-cover");
        frames(&mut rt, 4);
        assert!(rt.state.modal == Some(Modal::Appearance));
        move_to(&mut rt, "page-appearance");
        frames(&mut rt, 12);
        assert!(rt.state.modal == Some(Modal::Appearance));
        move_to(&mut rt, "database-page-cover");
        frames(&mut rt, 12);
        assert!(rt.state.modal == Some(Modal::PageActions));
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
#[test]
fn keyboard_open_survives_pointer_exit_and_escape_then_parent_escape() {
    let (mut rt, path) = setup(false);
    rt.set_full_keyboard_access(true);
    for _ in 0..30 {
        if rt
            .elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("appearance-menu-item") && e.focused)
        {
            break;
        }
        press(&mut rt, Key::Tab);
    }
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("appearance-menu-item") && e.focused)
    );
    press(&mut rt, Key::Right);
    assert!(rt.state.modal == Some(Modal::Appearance));
    assert!(rt.state.page_appearance.keyboard_open);
    rt.pointer_move(450., 600.);
    frames(&mut rt, 12);
    assert!(rt.state.modal == Some(Modal::Appearance));
    press(&mut rt, Key::Tab);
    assert!(rt.state.modal == Some(Modal::Appearance));
    let focused: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| e.focused)
        .filter_map(|e| e.key)
        .collect();
    assert!(
        focused.iter().any(|k| matches!(
            k.as_str(),
            "font-default" | "font-serif" | "font-mono" | "page-small-text" | "page-full-width"
        )),
        "Tab should enter appearance controls: {focused:?}"
    );
    press(&mut rt, Key::Escape);
    assert!(rt.state.modal == Some(Modal::PageActions));
    press(&mut rt, Key::Escape);
    assert!(rt.state.modal == None);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
fn keyboard_left_returns_to_parent_menu() {
    let (mut rt, path) = setup(false);
    rt.set_full_keyboard_access(true);
    for _ in 0..30 {
        if rt
            .elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("appearance-menu-item") && e.focused)
        {
            break;
        }
        press(&mut rt, Key::Tab);
    }
    press(&mut rt, Key::Right);
    assert!(rt.state.modal == Some(Modal::Appearance));
    press(&mut rt, Key::Tab);
    press(&mut rt, Key::Left);
    assert!(rt.state.modal == Some(Modal::PageActions));
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
