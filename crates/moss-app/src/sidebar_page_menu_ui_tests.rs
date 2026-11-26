use super::*;
use moss_core::{PageKind, new_id};
use std::path::PathBuf;

struct Fixture {
    rt: Runtime<State>,
    path: PathBuf,
    active: String,
    parent: String,
    child: String,
}

fn fixture() -> Fixture {
    let path = std::env::temp_dir().join(format!("moss-sidebar-menu-{}", new_id()));
    let mut state = State::open(path.clone(), None).unwrap();
    let active = state.tab().unwrap().page.id.clone();
    let workspace = state.workspace.id.clone();
    let parent = state
        .store
        .create_page(&workspace, None, "Menu target", "", PageKind::Document)
        .unwrap();
    let child = state
        .store
        .create_page(
            &workspace,
            Some(&parent.id),
            "Nested page",
            "",
            PageKind::Document,
        )
        .unwrap();
    state.refresh_pages();
    let mut rt = Runtime::with_text(
        state,
        ui::view,
        (850., 600.),
        Env {
            system_dark: false,
            scale: 1.,
        },
        rune::text::TextSystem::new(fonts::all(), fonts::FAMILIES),
    );
    rt.on_key = Some(key);
    rt.frame(0.);
    Fixture {
        rt,
        path,
        active,
        parent: parent.id,
        child: child.id,
    }
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..60 {
        rt.frame(1. / 120.);
    }
}
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1. / 120.);
}
fn context(rt: &mut Runtime<State>, id: &str) -> (f32, f32) {
    let (x, y) = rt.find(&format!("page-highlight-{id}")).unwrap();
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    rt.frame(1. / 120.);
    settle(rt);
    (x, y)
}
fn press(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Mods::default(),
    });
    rt.frame(1. / 120.);
}
fn close(f: Fixture) {
    drop(f.rt);
    std::fs::remove_dir_all(f.path).unwrap();
}

#[test]
fn inactive_page_context_menu_keeps_selection_and_dismisses() {
    let mut f = fixture();
    let edit = &mut f.rt.state.tabs[f.rt.state.active].blocks[0].edit;
    edit.replace_text("Selected text");
    edit.set_selection(0, 8);
    f.rt.frame(0.);
    let (px, py) = context(&mut f.rt, &f.parent);
    let edit = &f.rt.state.tab().unwrap().blocks[0].edit;
    assert_eq!(
        (edit.anchor.min(edit.focus), edit.anchor.max(edit.focus)),
        (0, 8)
    );
    assert_eq!(f.rt.state.tab().unwrap().page.id, f.active);
    let elements = f.rt.elements();
    let menu = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("sidebar-page-menu") && !e.exiting)
        .unwrap();
    let [x, y, w, h] = menu.target;
    assert!(x >= 0. && y >= 0. && x + w <= 850. && y + h <= 600.);
    assert!((x - px).abs() < 4. || (x + w - px).abs() < 4.);
    assert!((y - py).abs() < 4. || (y + h - py).abs() < 4.);
    assert!(
        elements
            .iter()
            .any(|e| e.text.as_deref() == Some("Menu target"))
    );
    assert!(
        elements.iter().any(
            |e| e.key.as_deref() == Some(&format!("page-open-{}", f.active)) && e.item_selected
        )
    );
    press(&mut f.rt, Key::Escape);
    settle(&mut f.rt);
    assert!(f.rt.find("sidebar-page-menu").is_none());
    assert!(
        f.rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some(&format!("page-open-{}", f.parent)) && e.focused)
    );
    context(&mut f.rt, &f.parent);
    f.rt.pointer_down(800., 50.);
    f.rt.pointer_up(800., 50.);
    f.rt.frame(1. / 120.);
    settle(&mut f.rt);
    assert!(f.rt.find("sidebar-page-menu").is_none());
    assert_eq!(f.rt.state.tab().unwrap().page.id, f.active);
    close(f);
}

#[test]
fn rename_and_open_new_tab_use_the_menu_target() {
    let mut f = fixture();
    context(&mut f.rt, &f.parent);
    click(&mut f.rt, "sidebar-page-rename");
    click(&mut f.rt, "sidebar-page-name");
    press(&mut f.rt, Key::Command('a'));
    press(&mut f.rt, Key::Text("Renamed page".into()));
    click(&mut f.rt, "sidebar-page-rename-save");
    settle(&mut f.rt);
    assert_eq!(f.rt.state.tab().unwrap().page.id, f.active);
    assert_eq!(
        f.rt.state.store.get_page(&f.parent).unwrap().unwrap().title,
        "Renamed page"
    );
    context(&mut f.rt, &f.parent);
    click(&mut f.rt, "sidebar-page-open-tab");
    settle(&mut f.rt);
    assert_eq!(f.rt.state.tab().unwrap().page.id, f.parent);
    assert!(f.rt.state.tabs.iter().any(|tab| tab.page.id == f.active));
    assert!(f.rt.find("sidebar-page-menu").is_none());
    close(f);
}

#[test]
fn trash_inactive_parent_closes_its_descendant_tabs_only() {
    let mut f = fixture();
    f.rt.state.open_page(&f.parent, true);
    f.rt.state.open_page(&f.child, true);
    f.rt.state.open_page(&f.active, false);
    f.rt.frame(0.);
    context(&mut f.rt, &f.parent);
    click(&mut f.rt, "sidebar-page-trash");
    settle(&mut f.rt);
    assert_eq!(f.rt.state.tab().unwrap().page.id, f.active);
    assert!(
        !f.rt
            .state
            .tabs
            .iter()
            .any(|tab| tab.page.id == f.parent || tab.page.id == f.child)
    );
    assert!(
        f.rt.state
            .store
            .list_trash(&f.rt.state.workspace.id)
            .unwrap()
            .iter()
            .any(|page| page.id == f.parent)
    );
    assert!(f.rt.find("sidebar-page-menu").is_none());
    close(f);
}

#[test]
fn stale_target_rename_keeps_the_menu_and_draft() {
    let mut f = fixture();
    context(&mut f.rt, &f.parent);
    click(&mut f.rt, "sidebar-page-rename");
    click(&mut f.rt, "sidebar-page-name");
    press(&mut f.rt, Key::Command('a'));
    press(&mut f.rt, Key::Text("Attempted rename".into()));
    f.rt.state.store.delete_page(&f.parent).unwrap();
    click(&mut f.rt, "sidebar-page-rename-save");
    assert!(f.rt.find("sidebar-page-menu").is_some());
    assert!(f.rt.elements().iter().any(|e| {
        e.text
            .as_deref()
            .is_some_and(|text| text.to_lowercase().contains("name"))
    }));
    assert_eq!(
        f.rt.state.store.get_page(&f.parent).unwrap().unwrap().title,
        "Menu target"
    );
    assert_eq!(f.rt.state.tab().unwrap().page.id, f.active);
    close(f);
}

#[test]
fn new_subpage_uses_menu_target_as_parent_and_opens_new_tab() {
    let mut f = fixture();
    context(&mut f.rt, &f.parent);
    click(&mut f.rt, "sidebar-page-new-subpage");
    settle(&mut f.rt);
    click(&mut f.rt, "new-page-title");
    press(&mut f.rt, Key::Text("New child".into()));
    press(&mut f.rt, Key::Enter);
    let page = &f.rt.state.tab().unwrap().page;
    assert_eq!(page.title, "New child");
    assert_eq!(page.parent_id.as_deref(), Some(f.parent.as_str()));
    assert!(f.rt.state.tabs.iter().any(|tab| tab.page.id == f.active));
    close(f);
}

#[test]
fn rename_popover_icon_button_opens_picker_without_closing_rename() {
    let mut f = fixture();
    context(&mut f.rt, &f.parent);
    click(&mut f.rt, "sidebar-page-rename");
    settle(&mut f.rt);
    assert!(f.rt.find("sidebar-page-rename-icon").is_some());
    assert!(f.rt.find("sidebar-page-name").is_some());
    assert!(f.rt.find("sidebar-page-rename-save").is_some());

    // Clicking the icon button opens the icon picker anchored below it while keeping Rename open.
    click(&mut f.rt, "sidebar-page-rename-icon");
    settle(&mut f.rt);
    assert!(f.rt.state.sidebar_page_menu.is_some());
    assert!(f.rt.state.sidebar_page_menu.as_ref().unwrap().renaming);
    let picker = f.rt.state.page_icon_picker.as_ref().expect("icon picker");
    assert_eq!(picker.page_id, f.parent);
    assert_eq!(picker.anchor, "sidebar-page-rename-icon");

    // Picking an emoji updates the page icon and keeps the Rename popover open.
    click(&mut f.rt, "page-icon-search");
    press(&mut f.rt, Key::Text("rocket".into()));
    click(&mut f.rt, "page-icon-option-0");
    settle(&mut f.rt);
    assert!(f.rt.state.page_icon_picker.is_none());
    assert!(f.rt.state.sidebar_page_menu.as_ref().unwrap().renaming);
    assert_eq!(
        f.rt.state.sidebar_page_menu.as_ref().unwrap().page.icon,
        "🚀"
    );
    assert_eq!(
        f.rt.state.store.get_page(&f.parent).unwrap().unwrap().icon,
        "🚀"
    );

    // Submitting the rename input saves the updated name and closes the popover.
    click(&mut f.rt, "sidebar-page-name");
    press(&mut f.rt, Key::Command('a'));
    press(&mut f.rt, Key::Text("Command Centre".into()));
    press(&mut f.rt, Key::Enter);
    settle(&mut f.rt);
    assert!(f.rt.state.sidebar_page_menu.is_none());
    assert_eq!(
        f.rt.state.store.get_page(&f.parent).unwrap().unwrap().title,
        "Command Centre"
    );
    assert_eq!(f.rt.state.tab().unwrap().page.id, f.active);
    close(f);
}
