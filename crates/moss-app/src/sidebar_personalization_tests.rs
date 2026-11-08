use super::*;
use moss_core::PageKind;

fn page(s: &mut State, parent: Option<&str>, name: &str) -> Page {
    s.store
        .create_page(&s.workspace.id, parent, name, "📄", PageKind::Document)
        .unwrap()
}

#[test]
fn inactive_icon_picker_keeps_drafts_and_updates_open_caches() {
    let (mut s, path) = fixture();
    let parent = page(&mut s, None, "Project");
    let child = page(&mut s, Some(&parent.id), "Notes");
    s.refresh_pages();
    s.toggle_sidebar_page(&parent.id);
    assert!(s.toggle_sidebar_favorite(&child.id));
    s.open_page(&parent.id, true);
    s.open_page(&child.id, true);
    let mut second = State::open(path.clone(), None).unwrap();
    second.open_page(&child.id, true);
    let duplicate = second
        .tabs
        .iter()
        .position(|tab| tab.page.id == child.id)
        .unwrap();
    s.tabs.push(second.tabs.remove(duplicate));
    drop(second);
    s.open_page(&parent.id, false);
    let active = s.active;
    s.tabs[active].title.replace_text("Unsaved title");
    s.tabs[active].dirty_title = true;
    s.tabs[active].blocks[0].edit.replace_text("Unsaved body");
    s.tabs[active].dirty_blocks = true;
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1200., 800.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, &format!("page-icon-{}", child.id));
    assert_eq!(rt.state.tab().unwrap().page.id, parent.id);
    assert_eq!(
        rt.state.page_icon_picker.as_ref().unwrap().page_id,
        child.id
    );
    assert!(rt.find("page-icon-picker").is_some());
    click(&mut rt, "page-icon-search");
    press(&mut rt, Key::Text("rocket".into()));
    click(&mut rt, "page-icon-option-0");
    assert_eq!(
        rt.state.store.get_page(&child.id).unwrap().unwrap().icon,
        "🚀"
    );
    assert_eq!(
        rt.state
            .tabs
            .iter()
            .filter(|t| t.page.id == child.id)
            .count(),
        2
    );
    assert!(
        rt.state
            .tabs
            .iter()
            .filter(|t| t.page.id == child.id)
            .all(|t| t.page.icon == "🚀")
    );
    assert_eq!(
        rt.state
            .sidebar_favorites
            .pages
            .iter()
            .find(|p| p.id == child.id)
            .unwrap()
            .icon,
        "🚀"
    );
    assert!(rt.state.sidebar_tree.rows.iter().any(|row| matches!(row, crate::sidebar_tree::SidebarRow::Page { page, .. } if page.id == child.id && page.icon == "🚀")));
    assert_eq!(
        rt.state
            .tab()
            .unwrap()
            .child_pages
            .as_ref()
            .unwrap()
            .items
            .iter()
            .find(|c| c.page.id == child.id)
            .unwrap()
            .page
            .icon,
        "🚀"
    );
    assert_eq!(rt.state.tab().unwrap().title.text, "Unsaved title");
    assert_eq!(rt.state.tab().unwrap().blocks[0].edit.text, "Unsaved body");
    assert!(rt.state.tab().unwrap().dirty_title && rt.state.tab().unwrap().dirty_blocks);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn sidebar_row_geometry_and_actions_preserve_navigation() {
    for dark in [false, true] {
        for scale in [1., 2.] {
            let (mut s, path) = fixture();
            let parent = page(&mut s, None, "Parent");
            let child = page(&mut s, Some(&parent.id), "Child");
            s.refresh_pages();
            s.toggle_sidebar_page(&parent.id);
            assert!(s.toggle_sidebar_favorite(&parent.id));
            let active = s.tab().unwrap().page.id.clone();
            let mut rt = Runtime::new(
                s,
                ui::view,
                (1200., 800.),
                Env {
                    system_dark: dark,
                    scale,
                },
            );
            rt.on_key = Some(key);
            rt.frame(0.);
            let rect = |key: &str| rt.presented(key).unwrap().0;
            let root_icon = rect(&format!("page-icon-{}", parent.id));
            let root_title = rect(&format!("page-title-{}", parent.id));
            let child_icon = rect(&format!("page-icon-{}", child.id));
            let child_title = rect(&format!("page-title-{}", child.id));
            let favorite_icon = rect(&format!("favorite-icon-{}", parent.id));
            let favorite_title = rect(&format!("favorite-title-{}", parent.id));
            assert_eq!(root_icon[2], 20.);
            assert_eq!(root_title[0] - root_icon[0] - root_icon[2], 8.);
            assert_eq!(child_icon[0] - root_icon[0], 16.);
            assert_eq!(child_title[0] - child_icon[0] - child_icon[2], 8.);
            assert_eq!(favorite_title[0] - favorite_icon[0] - favorite_icon[2], 8.);
            for (icon, title) in [
                (root_icon, root_title),
                (child_icon, child_title),
                (favorite_icon, favorite_title),
            ] {
                assert!((icon[1] + icon[3] / 2. - title[1] - title[3] / 2.).abs() < 1.);
            }
            click(&mut rt, &format!("page-actions-{}", child.id));
            assert_eq!(rt.state.tab().unwrap().page.id, active);
            assert_eq!(
                rt.state.sidebar_page_menu.as_ref().unwrap().page.id,
                child.id
            );
            rt.state.close_sidebar_page_menu();
            rt.invalidate();
            rt.frame(0.);
            click(&mut rt, &format!("page-add-{}", parent.id));
            assert_eq!(
                rt.state.tab().unwrap().page.parent_id.as_deref(),
                Some(parent.id.as_str())
            );
            drop(rt);
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}

#[test]
fn record_icon_update_refreshes_loaded_database_cache() {
    let (mut s, path) = fixture();
    s.new_page(true, None);
    s.create_inline(Default::default(), false);
    s.finish_cell(false);
    let record = s.tab().unwrap().records[0].clone();
    let page = s.store.ensure_record_page(&record.id).unwrap();
    s.query_records();
    s.open_page_icon_picker(&page.id, "test-anchor");
    s.choose_page_icon("🚀");
    assert_eq!(s.store.get_page(&page.id).unwrap().unwrap().icon, "🚀");
    assert_eq!(
        s.tab()
            .unwrap()
            .record_icons
            .get(&record.id)
            .map(String::as_str),
        Some("🚀")
    );
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn favorite_menu_icon_picker_and_removal_keep_current_page() {
    let (mut s, path) = fixture();
    let target = page(&mut s, None, "Favorite candidate");
    s.refresh_pages();
    let active = s.tab().unwrap().page.id.clone();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1200., 800.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, &format!("page-actions-{}", target.id));
    rt.frame(1. / 120.);
    click(&mut rt, "sidebar-page-favorite");
    assert_eq!(rt.state.tab().unwrap().page.id, active);
    assert!(rt.state.sidebar_page_menu.is_none());
    assert!(rt.find(&format!("favorite-open-{}", target.id)).is_some());
    click(&mut rt, &format!("favorite-actions-{}", target.id));
    rt.frame(1. / 120.);
    click(&mut rt, "sidebar-page-icon");
    let picker = rt.state.page_icon_picker.as_ref().unwrap();
    assert_eq!(picker.page_id, target.id);
    assert_eq!(picker.anchor, format!("favorite-icon-{}", target.id));
    assert!(rt.state.sidebar_page_menu.is_none());
    assert_eq!(rt.state.tab().unwrap().page.id, active);
    press(&mut rt, Key::Escape);
    assert!(rt.state.page_icon_picker.is_none());
    click(&mut rt, &format!("favorite-actions-{}", target.id));
    rt.frame(1. / 120.);
    click(&mut rt, "sidebar-page-favorite");
    assert!(!rt.state.is_sidebar_favorite(&target.id));
    assert!(rt.find(&format!("favorite-open-{}", target.id)).is_none());
    assert!(rt.find(&format!("page-open-{}", target.id)).is_some());
    assert_eq!(rt.state.tab().unwrap().page.id, active);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn narrow_sidebar_keeps_deep_titles_and_actions_inside_rows() {
    let (mut s, path) = fixture();
    let mut parent: Option<String> = None;
    let mut ids = Vec::new();
    for _ in 0..10 {
        let child = page(
            &mut s,
            parent.as_deref(),
            &"A long nested page title ".repeat(8),
        );
        parent = Some(child.id.clone());
        ids.push(child.id);
    }
    s.refresh_pages();
    for id in &ids {
        s.toggle_sidebar_page(id);
    }
    s.prefs.sidebar_widths.insert(s.workspace.id.clone(), 200.);
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 800.),
        Env {
            system_dark: false,
            scale: 2.,
        },
    );
    rt.frame(0.);
    rt.frame(1. / 120.);
    rt.frame(1. / 120.);
    let mut action_x = None;
    for id in &ids {
        let row = rt.presented(&format!("page-highlight-{id}")).unwrap().0;
        for key in [
            format!("page-icon-{id}"),
            format!("page-open-{id}"),
            format!("page-add-{id}"),
            format!("page-actions-{id}"),
        ] {
            let item = rt.presented(&key).unwrap().0;
            assert!(
                item[0] >= row[0] && item[0] + item[2] <= row[0] + row[2] + 0.1,
                "{key}: {item:?} row {row:?}"
            );
        }
        let title = rt.presented(&format!("page-open-{id}")).unwrap().0;
        assert!(title[2] >= 39.9, "{title:?}");
        let x = rt.presented(&format!("page-actions-{id}")).unwrap().0[0];
        if let Some(previous) = action_x {
            assert!((x - previous as f32).abs() < 0.1);
        }
        action_x = Some(x);
    }
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn trailing_page_actions_stay_near_the_right_edge_for_parents_and_nested_pages() {
    for width in [200., 270., 480.] {
        let (mut s, path) = fixture();
        let parent = page(&mut s, None, "Parent");
        let child = page(&mut s, Some(&parent.id), "Nested page with a long title");
        s.refresh_pages();
        s.toggle_sidebar_page(&parent.id);
        assert!(s.toggle_sidebar_favorite(&parent.id));
        s.prefs.sidebar_widths.insert(s.workspace.id.clone(), width);
        let mut rt = Runtime::new(
            s,
            ui::view,
            (1200., 800.),
            Env {
                system_dark: false,
                scale: 1.,
            },
        );
        rt.on_key = Some(key);
        rt.frame(0.);
        rt.frame(1. / 120.);
        for (prefix, id) in [
            ("page", &parent.id),
            ("page", &child.id),
            ("favorite", &parent.id),
        ] {
            let row = rt.presented(&format!("{prefix}-highlight-{id}")).unwrap().0;
            let actions = rt.presented(&format!("{prefix}-actions-{id}")).unwrap().0;
            let add = rt.presented(&format!("{prefix}-add-{id}")).unwrap().0;
            assert!(
                (row[0] + row[2] - add[0] - add[2] - 3.).abs() < 0.1,
                "{prefix} width={width}: {row:?} {add:?}"
            );
            assert!(
                (add[0] - actions[0] - actions[2]).abs() < 0.1,
                "{actions:?} {add:?}"
            );
            assert_eq!(actions[2], 22.);
            assert_eq!(add[2], 22.);
        }
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn nested_page_favorite_action_adds_and_removes_without_navigation() {
    let (mut s, path) = fixture();
    let parent = page(&mut s, None, "Parent");
    let child = page(&mut s, Some(&parent.id), "Nested favorite");
    s.refresh_pages();
    s.toggle_sidebar_page(&parent.id);
    let active = s.tab().unwrap().page.id.clone();
    let mut rt = Runtime::new(
        s,
        ui::view,
        (1200., 800.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    click(&mut rt, &format!("page-actions-{}", child.id));
    rt.frame(1. / 120.);
    assert_eq!(
        rt.state.sidebar_page_menu.as_ref().unwrap().page.id,
        child.id
    );
    click(&mut rt, "sidebar-page-favorite");
    assert!(rt.state.is_sidebar_favorite(&child.id));
    assert!(rt.find(&format!("favorite-open-{}", child.id)).is_some());
    assert_eq!(rt.state.tab().unwrap().page.id, active);
    click(&mut rt, &format!("favorite-actions-{}", child.id));
    rt.frame(1. / 120.);
    click(&mut rt, "sidebar-page-favorite");
    assert!(!rt.state.is_sidebar_favorite(&child.id));
    assert!(rt.find(&format!("page-open-{}", child.id)).is_some());
    assert_eq!(rt.state.tab().unwrap().page.id, active);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
