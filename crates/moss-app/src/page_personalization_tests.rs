use super::*;
use moss_core::new_id;

#[test]
fn all_local_emoji_pass_atomic_validation_and_conflict_keeps_draft() {
    let path = std::env::temp_dir().join(format!("moss-icons-{}", new_id()));
    let mut s = State::open(path.clone(), None).unwrap();
    let page = s.tab().unwrap().page.clone();
    let mut expected = page.icon.clone();
    for emoji in emoji_catalog::matches("", None) {
        let saved = s
            .store
            .set_page_icon_if_unchanged(&s.workspace.id, &page.id, &expected, emoji.glyph)
            .unwrap_or_else(|e| panic!("{} {:?}: {e}", emoji.name, emoji.glyph));
        expected = saved.icon;
    }
    s.open_page_icon_picker(&page.id, "test-anchor");
    s.store
        .set_page_icon_if_unchanged(&s.workspace.id, &page.id, &expected, "⭐")
        .unwrap();
    s.choose_page_icon("🧑‍💻");
    assert!(s.page_icon_picker.as_ref().unwrap().error.is_some());
    s.dismiss_page_icon_picker();
    assert!(s.page_icon_picker.is_some());
    assert!(!s.flush_current());
    assert_eq!(s.store.get_page(&page.id).unwrap().unwrap().icon, "⭐");
    s.page_icon_picker = None;
    s.open_page_icon_picker(&page.id, "test-anchor");
    s.choose_page_icon("🧑‍💻");
    assert!(s.page_icon_picker.is_none());
    assert_eq!(s.store.get_page(&page.id).unwrap().unwrap().icon, "🧑‍💻");
    assert_eq!(s.recent_page_icons, ["🧑‍💻"]);
    s.open_page_icon_picker(&page.id, "test-anchor");
    s.choose_page_icon("");
    assert_eq!(s.store.get_page(&page.id).unwrap().unwrap().icon, "");
    s.open_page_icon_picker(&page.id, "test-anchor");
    assert!(s.flush_current());
    assert!(s.page_icon_picker.is_none());
    drop(s);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn picker_is_bounded_searchable_and_saves_emoji_at_both_scales() {
    for scale in [1., 2.] {
        let path = std::env::temp_dir().join(format!("moss-icon-ui-{}", new_id()));
        let mut state = State::open(path.clone(), None).unwrap();
        let id = state.tab().unwrap().page.id.clone();
        let anchor = format!("page-header-icon-{id}");
        state.open_page_icon_picker(&id, &anchor);
        let mut rt = rune::Runtime::new(
            state,
            crate::ui::view,
            (850., 600.),
            rune::Env {
                system_dark: false,
                scale,
            },
        );
        rt.on_key = Some(crate::key);
        rt.frame(0.);
        rt.frame(1.);
        let rect = rt.presented("page-icon-picker").unwrap().0;
        assert!(
            rect[0] >= 0.
                && rect[1] >= 0.
                && rect[0] + rect[2] <= 850.
                && rect[1] + rect[3] <= 600.,
            "{rect:?}"
        );
        assert_eq!(
            rt.elements()
                .iter()
                .filter(|e| e
                    .key
                    .as_deref()
                    .is_some_and(|key| key.starts_with("page-icon-option-")))
                .count(),
            emoji_catalog::PAGE_SIZE
        );
        let first = rt.presented("page-icon-option-0").unwrap().0;
        let eighth = rt.presented("page-icon-option-7").unwrap().0;
        let ninth = rt.presented("page-icon-option-8").unwrap().0;
        assert_eq!(first[1], eighth[1]);
        assert!(ninth[1] > first[1]);
        let (x, y) = rt.find("page-icon-next").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.);
        assert_eq!(rt.state.page_icon_picker.as_ref().unwrap().page, 1);
        let (x, y) = rt.find("page-icon-search").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.);
        rt.key(rune::KeyEvent {
            key: rune::Key::Text("flag: australia".into()),
            mods: rune::Mods::default(),
        });
        rt.frame(0.);
        assert_eq!(rt.state.page_icon_picker.as_ref().unwrap().page, 0);
        rt.key(rune::KeyEvent {
            key: rune::Key::Enter,
            mods: rune::Mods::default(),
        });
        rt.frame(0.);
        assert!(rt.state.page_icon_picker.is_none());
        assert_eq!(rt.state.store.get_page(&id).unwrap().unwrap().icon, "🇦🇺");
        drop(rt);
        std::fs::remove_dir_all(path).unwrap();
    }
}
