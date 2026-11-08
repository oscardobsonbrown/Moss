use super::*;

fn settle(rt: &mut Runtime<State>) {
    rt.frame(0.15);
}
fn new_page(s: &mut State, parent: Option<&str>, name: &str) -> Page {
    s.store
        .create_page(&s.workspace.id, parent, name, "📄", PageKind::Document)
        .unwrap()
}
fn order(s: &State) -> Vec<String> {
    s.sidebar_sections
        .sections
        .iter()
        .map(|section| section.id.clone())
        .collect()
}
fn make_runtime(mut state: State, dark: bool, scale: f32) -> Runtime<State> {
    state.refresh_pages();
    let mut rt = Runtime::new(
        state,
        ui::view,
        (1000., 800.),
        Env {
            system_dark: dark,
            scale,
        },
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    settle(&mut rt);
    rt
}
fn create_section(rt: &mut Runtime<State>, name: &str) -> String {
    click(rt, "sidebar-new-section");
    settle(rt);
    click(rt, "sidebar-section-name");
    press(rt, Key::Text(name.into()));
    click(rt, "sidebar-section-save");
    settle(rt);
    rt.state
        .sidebar_sections
        .sections
        .iter()
        .find(|section| section.name == name)
        .unwrap()
        .id
        .clone()
}

#[test]
fn section_input_dismiss_reopen_and_external_removal_are_safe() {
    let (s, path) = fixture();
    let mut rt = make_runtime(s, false, 1.);
    click(&mut rt, "sidebar-new-section");
    settle(&mut rt);
    click(&mut rt, "sidebar-section-name");
    press(&mut rt, Key::Text("Draft".into()));
    press(&mut rt, Key::Escape);
    settle(&mut rt);
    assert!(rt.state.sidebar_section_menu.is_none());
    let section = create_section(&mut rt, "Reading");
    click(&mut rt, &format!("section-actions-{section}"));
    settle(&mut rt);
    click(&mut rt, "sidebar-section-add-page");
    settle(&mut rt);
    click(&mut rt, "sidebar-section-search");
    press(&mut rt, Key::Text("project".into()));
    press(&mut rt, Key::Escape);
    settle(&mut rt);
    assert!(rt.state.sidebar_section_menu.is_none());
    click(&mut rt, &format!("section-actions-{section}"));
    settle(&mut rt);
    click(&mut rt, "sidebar-section-rename");
    settle(&mut rt);
    click(&mut rt, "sidebar-section-name");
    let mut other = State::open(path.clone(), Some(&rt.state.workspace.id)).unwrap();
    other.shared_preferences = rt.state.shared_preferences.clone();
    other.refresh_sidebar_sections();
    assert!(other.remove_sidebar_section(&section));
    assert!(rt.state.refresh_sidebar_sections());
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.state.sidebar_section_menu.is_none());
    assert!(!rt.state.sidebar_section_name.focused);
    drop(other);
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn create_rename_reorder_collapse_and_reopen_section() {
    for (dark, scale) in [(false, 1.), (true, 2.)] {
        let (s, path) = fixture();
        let mut rt = make_runtime(s, dark, scale);
        let section = create_section(&mut rt, "Reading");
        click(&mut rt, &format!("section-actions-{section}"));
        settle(&mut rt);
        click(&mut rt, "sidebar-section-rename");
        settle(&mut rt);
        click(&mut rt, "sidebar-section-name");
        press(&mut rt, Key::Command('a'));
        press(&mut rt, Key::Text("Research".into()));
        click(&mut rt, "sidebar-section-save");
        settle(&mut rt);
        assert_eq!(
            rt.state
                .sidebar_sections
                .sections
                .iter()
                .find(|x| x.id == section)
                .unwrap()
                .name,
            "Research"
        );
        let initial = order(&rt.state);
        click(&mut rt, &format!("section-actions-{section}"));
        settle(&mut rt);
        click(&mut rt, "sidebar-section-up");
        settle(&mut rt);
        let moved = order(&rt.state);
        assert_eq!(
            moved.iter().position(|id| id == &section).unwrap() + 1,
            initial.iter().position(|id| id == &section).unwrap()
        );
        click(&mut rt, &format!("section-actions-{section}"));
        settle(&mut rt);
        click(&mut rt, "sidebar-section-down");
        settle(&mut rt);
        assert_eq!(order(&rt.state), initial);
        click(&mut rt, &format!("section-toggle-{section}"));
        assert!(
            rt.state
                .sidebar_sections
                .sections
                .iter()
                .find(|x| x.id == section)
                .unwrap()
                .collapsed
        );
        click(&mut rt, &format!("section-toggle-{section}"));
        assert!(
            !rt.state
                .sidebar_sections
                .sections
                .iter()
                .find(|x| x.id == section)
                .unwrap()
                .collapsed
        );
        let workspace = rt.state.workspace.id.clone();
        drop(rt);
        let reopened = State::open(path.clone(), Some(&workspace)).unwrap();
        assert_eq!(order(&reopened), initial);
        assert_eq!(
            reopened
                .sidebar_sections
                .sections
                .iter()
                .find(|x| x.id == section)
                .unwrap()
                .name,
            "Research"
        );
        let mut rt = make_runtime(reopened, dark, scale);
        click(&mut rt, &format!("section-actions-{section}"));
        settle(&mut rt);
        click(&mut rt, "sidebar-section-remove");
        assert!(!order(&rt.state).contains(&section));
        drop(rt);
        let reopened = State::open(path.clone(), Some(&workspace)).unwrap();
        assert!(!order(&reopened).contains(&section));
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn nested_page_can_be_added_and_removed_from_section_by_both_menus() {
    let (mut s, path) = fixture();
    let parent = new_page(&mut s, None, "Parent");
    let child = new_page(&mut s, Some(&parent.id), "Unique nested reference");
    s.refresh_pages();
    s.toggle_sidebar_page(&parent.id);
    let mut rt = make_runtime(s, false, 1.);
    let section = create_section(&mut rt, "Study");
    click(&mut rt, &format!("section-actions-{section}"));
    settle(&mut rt);
    click(&mut rt, "sidebar-section-add-page");
    settle(&mut rt);
    click(&mut rt, "sidebar-section-search");
    press(&mut rt, Key::Text("Unique nested reference".into()));
    click(&mut rt, &format!("section-pick-{}", child.id));
    assert!(
        rt.state
            .sidebar_sections
            .sections
            .iter()
            .find(|x| x.id == section)
            .unwrap()
            .page_ids
            .contains(&child.id)
    );
    press(&mut rt, Key::Escape);
    settle(&mut rt);
    assert!(
        rt.find(&format!("section-{section}-open-{}", child.id))
            .is_some()
    );
    assert_eq!(
        rt.state
            .store
            .get_page(&child.id)
            .unwrap()
            .unwrap()
            .parent_id
            .as_deref(),
        Some(parent.id.as_str())
    );
    click(&mut rt, &format!("page-actions-{}", child.id));
    settle(&mut rt);
    click(&mut rt, "sidebar-page-section");
    settle(&mut rt);
    click(&mut rt, &format!("section-choose-{section}"));
    assert!(
        !rt.state
            .sidebar_sections
            .sections
            .iter()
            .find(|x| x.id == section)
            .unwrap()
            .page_ids
            .contains(&child.id)
    );
    click(&mut rt, &format!("section-choose-{section}"));
    assert!(
        rt.state
            .sidebar_sections
            .sections
            .iter()
            .find(|x| x.id == section)
            .unwrap()
            .page_ids
            .contains(&child.id)
    );
    assert_eq!(
        rt.state
            .store
            .get_page(&child.id)
            .unwrap()
            .unwrap()
            .parent_id
            .as_deref(),
        Some(parent.id.as_str())
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn recents_tracks_page_opens_and_tab_reactivation_and_shows_in_rail() {
    let (mut s, path) = fixture();
    let pages: Vec<_> = (0..4)
        .map(|i| new_page(&mut s, None, &format!("Recent {i}")))
        .collect();
    let mut rt = make_runtime(s, false, 1.);
    for page in &pages {
        click(&mut rt, &format!("page-actions-{}", page.id));
        settle(&mut rt);
        click(&mut rt, "sidebar-page-open-tab");
        settle(&mut rt);
    }
    let ids = || pages.iter().rev().map(|p| p.id.clone()).collect::<Vec<_>>();
    let recent = &rt.state.sidebar_sections.pages["recents"];
    assert_eq!(
        recent.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
        ids()
    );
    let index = rt
        .state
        .tabs
        .iter()
        .position(|tab| tab.page.id == pages[1].id)
        .unwrap();
    click(&mut rt, &format!("tab-{index}"));
    assert_eq!(
        rt.state.sidebar_sections.pages["recents"][0].id,
        pages[1].id
    );
    let section = create_section(&mut rt, "Pinned");
    assert!(rt.state.add_page_to_sidebar_section(&section, &pages[2].id));
    rt.state.toggle_sidebar();
    rt.invalidate();
    rt.frame(0.);
    assert!(rt.find(&format!("rail-recent-{}", pages[1].id)).is_some());
    assert!(
        rt.find(&format!("rail-section-{section}-{}", pages[2].id))
            .is_some()
    );
    assert!(rt.find("rail-group-recents").is_some());
    assert!(rt.find(&format!("rail-group-{section}")).is_some());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn section_drag_reorders_and_focus_loss_cancels_a_second_drag() {
    let (s, path) = fixture();
    let mut rt = make_runtime(s, false, 1.);
    let initial = order(&rt.state);
    let (x, y) = rt.find("section-toggle-private").unwrap();
    let (_, target_y) = rt.find("section-toggle-recents").unwrap();
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x, target_y - 10.);
    rt.frame(0.);
    assert!(rt.state.sidebar_sections.dragging.is_some());
    rt.pointer_up(x, target_y - 10.);
    settle(&mut rt);
    let moved = order(&rt.state);
    assert_eq!(moved[0], "private");
    assert_ne!(moved, initial);
    let (x, y) = rt.find("section-toggle-private").unwrap();
    let (_, target_y) = rt.find("section-toggle-favorites").unwrap();
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x, target_y + 15.);
    rt.frame(0.);
    assert!(rt.state.sidebar_sections.dragging.is_some());
    rt.window_focus(false);
    rt.frame(0.);
    assert_eq!(order(&rt.state), moved);
    assert!(rt.state.sidebar_sections.dragging.is_none());
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn section_drag_uses_window_coordinates_after_sidebar_scroll() {
    let (mut s, path) = fixture();
    let ids: Vec<_> = (0..12)
        .map(|i| s.create_sidebar_section(&format!("Group {i}")).unwrap())
        .collect();
    let mut rt = make_runtime(s, false, 1.);
    rt.set_viewport((850., 600.), 1.);
    rt.update(|_, tx| tx.scroll_to_y("sidebar-scroll", f32::MAX));
    rt.frame(0.);
    settle(&mut rt);
    let source = &ids[11];
    let before = &ids[10];
    let (x, y) = rt.find(&format!("section-toggle-{source}")).unwrap();
    let (_, target_y) = rt.find(&format!("section-toggle-{before}")).unwrap();
    let scroll = rt.presented("sidebar-scroll").unwrap().0;
    assert!(y >= scroll[1] && y < scroll[1] + scroll[3]);
    assert!(target_y >= scroll[1] && target_y < scroll[1] + scroll[3]);
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_move(x, target_y - 6.);
    rt.frame(0.);
    assert!(rt.state.sidebar_sections.dragging.is_some());
    rt.pointer_up(x, target_y - 6.);
    settle(&mut rt);
    let result = order(&rt.state);
    assert_eq!(
        result.iter().position(|id| id == source).unwrap() + 1,
        result.iter().position(|id| id == before).unwrap()
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn expanded_private_rows_do_not_overlap_the_following_custom_section() {
    let (mut s, path) = fixture();
    let parent = new_page(&mut s, None, "Private parent");
    let child = new_page(&mut s, Some(&parent.id), "Private child");
    s.refresh_pages();
    s.toggle_sidebar_page(&parent.id);
    let section = s.create_sidebar_section("After Private").unwrap();
    assert!(s.add_page_to_sidebar_section(&section, &child.id));
    let last_private = s
        .sidebar_tree
        .rows
        .iter()
        .rev()
        .find_map(|row| match row {
            crate::sidebar_tree::SidebarRow::Page { page, .. } => Some(page.id.clone()),
            _ => None,
        })
        .unwrap();
    let mut rt = make_runtime(s, false, 1.);
    rt.frame(1. / 120.);
    let private = rt
        .presented(&format!("page-highlight-{last_private}"))
        .unwrap()
        .0;
    let header = rt
        .presented(&format!("section-header-{section}"))
        .unwrap()
        .0;
    let shortcut = rt
        .presented(&format!("section-{section}-highlight-{}", child.id))
        .unwrap()
        .0;
    assert!(
        header[1] >= private[1] + private[3] - 0.1,
        "private={private:?} header={header:?}"
    );
    assert!(
        shortcut[1] >= header[1] + header[3] - 0.1,
        "header={header:?} shortcut={shortcut:?}"
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn many_section_shortcuts_keep_mounted_rows_bounded() {
    use crate::sidebar_sections::{SidebarSection, SidebarSectionKind};
    let (mut s, path) = fixture();
    let template = s.tab().unwrap().page.clone();
    for group in 0..12 {
        let section = format!("synthetic-{group}");
        let pages: Vec<_> = (0..100)
            .map(|index| {
                let mut page = template.clone();
                page.id = format!("synthetic-page-{group}-{index}");
                page.title = format!("Page {group} {index}");
                page
            })
            .collect();
        s.sidebar_sections.sections.push(SidebarSection {
            id: section.clone(),
            name: format!("Group {group}"),
            kind: SidebarSectionKind::Custom,
            page_ids: pages.iter().map(|p| p.id.clone()).collect(),
            collapsed: false,
        });
        s.sidebar_sections.pages.insert(section, pages);
    }
    let mut rt = Runtime::new(
        s,
        ui::view,
        (850., 600.),
        Env {
            system_dark: false,
            scale: 1.,
        },
    );
    rt.frame(0.);
    settle(&mut rt);
    let mounted = |rt: &Runtime<State>| {
        rt.elements()
            .iter()
            .filter(|e| {
                e.key.as_deref().is_some_and(|key| {
                    key.starts_with("section-synthetic-") && key.contains("-open-")
                })
            })
            .count()
    };
    let initial = mounted(&rt);
    assert!(initial < 100, "{initial} shortcut rows mounted at the top");
    assert!(
        rt.find("section-synthetic-11-open-synthetic-page-11-99")
            .is_none()
    );
    rt.update(|_, tx| tx.scroll_to_y("sidebar-scroll", f32::MAX));
    rt.frame(0.);
    settle(&mut rt);
    let end = mounted(&rt);
    assert!(end < 100, "{end} shortcut rows mounted at the end");
    assert!(
        rt.find("section-synthetic-11-open-synthetic-page-11-99")
            .is_some()
    );
    assert!(
        rt.find("section-synthetic-0-open-synthetic-page-0-0")
            .is_none()
    );
    drop(rt);
    std::fs::remove_dir_all(path).unwrap();
}
