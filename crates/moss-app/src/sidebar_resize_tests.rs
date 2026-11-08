use super::*;

fn runtime(database: bool) -> (Runtime<State>, PathBuf) {
    let (mut s, path) = fixture();
    if database {
        s.new_page(true, None);
        s.create_inline(Default::default(), false);
        s.finish_cell(false);
    }
    s.save_preferences();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1440., 900.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    (rt, path)
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key)
        .unwrap_or_else(|| panic!("missing {key}"))
        .0
}

fn aligned(rt: &Runtime<State>, width: f32, viewport: f32) {
    assert_eq!(rect(rt, "sidebar")[2], width);
    assert_eq!(rect(rt, "content")[0], width);
    assert_eq!(rect(rt, "content")[2], viewport - width);
    let back = rect(rt, "history-back");
    let forward = rect(rt, "history-forward");
    let tabs = rect(rt, "tab-strip");
    assert_eq!(back[0], (width + 8.).max(rt.chrome_insets().leading + 8.));
    assert_eq!(forward[0], back[0] + back[2]);
    assert_eq!(tabs[0], forward[0] + forward[2]);
    assert_eq!(rect(rt, "sidebar-resize")[0], width - 3.);
}

#[test]
fn drag_reflows_immediately_saves_on_release_and_cancels_without_drift() {
    let (mut rt, path) = runtime(false);
    let preferences = path.join("preferences.json");
    let before = std::fs::read(&preferences).unwrap();
    let (x, y) = rt.find("sidebar-resize").unwrap();
    rt.pointer_move(x, y);
    assert_eq!(rt.cursor(), Cursor::ResizeColumn);
    rt.pointer_down(x, y);
    rt.frame(0.);
    for delta in [90., 160., 260.] {
        rt.pointer_move(x + delta, y);
        rt.frame(1. / 120.);
        aligned(&rt, (270. + delta).min(480.), 1440.);
        assert_eq!(
            std::fs::read(&preferences).unwrap(),
            before,
            "drag frames must not write preferences"
        );
    }
    rt.pointer_up(x + 260., y);
    rt.frame(0.);
    assert_ne!(std::fs::read(&preferences).unwrap(), before);
    rt.set_viewport((850., 600.), 1.);
    rt.frame(0.);
    aligned(&rt, 450., 850.);
    rt.set_viewport((1440., 900.), 1.);
    rt.frame(0.);
    aligned(&rt, 480., 1440.);

    let (x, y) = rt.find("sidebar-resize").unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.);
    rt.pointer_move(0., y);
    rt.frame(0.);
    aligned(&rt, 200., 1440.);
    rt.window_focus(false);
    rt.frame(0.);
    aligned(&rt, 480., 1440.);
    drop(rt);
    let reopened = State::open(path.clone(), None).unwrap();
    assert_eq!(reopened.sidebar_width(1440.), 480.);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn keyboard_resize_and_workspace_widths_survive_reopen_and_old_preferences() {
    let (mut rt, path) = runtime(false);
    click(&mut rt, "sidebar-resize");
    press(&mut rt, Key::Right);
    aligned(&rt, 280., 1440.);
    press(&mut rt, Key::Left);
    aligned(&rt, 270., 1440.);
    press(&mut rt, Key::Home);
    aligned(&rt, 200., 1440.);
    press(&mut rt, Key::End);
    aligned(&rt, 480., 1440.);

    let first_id = rt.state.workspace.id.clone();
    let other = rt.state.store.create_workspace("Other", "O").unwrap();
    let mut second = State::open(path.clone(), Some(&other.id)).unwrap();
    second.shared_preferences = rt.state.shared_preferences.clone();
    second.set_sidebar_width(320., 1440.);
    second.save_preferences();
    rt.state.save_preferences();
    let saved: state::Preferences =
        serde_json::from_slice(&std::fs::read(path.join("preferences.json")).unwrap()).unwrap();
    assert_eq!(saved.sidebar_widths[&first_id], 480.);
    assert_eq!(saved.sidebar_widths[&other.id], 320.);
    let mut old = serde_json::to_value(saved).unwrap();
    old.as_object_mut().unwrap().remove("sidebar_widths");
    let old: state::Preferences = serde_json::from_value(old).unwrap();
    assert!(old.sidebar_widths.is_empty());
    drop(second);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn database_views_and_side_peek_fit_after_sidebar_and_window_resize() {
    let (mut rt, path) = runtime(true);
    rt.state.set_sidebar_width(480., 1440.);
    rt.set_viewport((850., 600.), 1.);
    let views = rt
        .state
        .tab()
        .unwrap()
        .database
        .as_ref()
        .unwrap()
        .views
        .len();
    for view in 0..views {
        rt.state.select_database_view(view);
        rt.invalidate();
        rt.frame(0.);
        aligned(&rt, 450., 850.);
        for key in ["db-toolbar", "db-new", "db-filter", "db-body"]
            .into_iter()
            .chain(rt.find("db-views").is_some().then_some("db-views"))
        {
            let r = rect(&rt, key);
            assert!(r[0] >= 450. && r[0] + r[2] <= 851., "{view} {key}: {r:?}");
        }
        if rt.find("gallery-grid").is_some() {
            let r = rect(&rt, "gallery-new");
            assert!(r[0] + r[2] <= 851., "Gallery must fit its viewport: {r:?}");
        }
    }
    rt.state.select_database_view(0);
    rt.state.create_peek(Default::default());
    rt.state.record_peek.as_mut().unwrap().mode = state::PeekMode::Side;
    rt.invalidate();
    rt.frame(1.);
    let pane = rect(&rt, "content");
    let peek = rect(&rt, "record-peek-side");
    assert_eq!(pane[0], 210.);
    assert_eq!(pane[2], 320.);
    assert_eq!(peek[0], pane[0] + pane[2]);
    assert_eq!(peek[0] + peek[2], 850.);
    for key in ["db-new", "db-filter"]
        .into_iter()
        .chain(rt.find("db-views").is_some().then_some("db-views"))
    {
        let r = rect(&rt, key);
        assert!(
            r[0] >= pane[0] && r[0] + r[2] <= peek[0] + 1.,
            "{key}: {r:?}"
        );
    }
    rt.state.close_peek();
    rt.invalidate();
    rt.frame(1.);
    aligned(&rt, 450., 850.);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
